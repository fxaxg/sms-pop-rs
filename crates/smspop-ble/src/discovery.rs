//! 找设备、等链路、取 ANCS 服务。
//!
//! 这三个动作都**必须带超时**：WinRT 的枚举和连接调用在状态不一致时
//! 可能挂住很久（实测遇到过 39 分钟没有任何返回），
//! 不设上限的话整个 Bluetooth 线程就废在那里了。

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use log::{info, warn};
use smspop_core::ancs::ANCS_SERVICE_UUID;
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCommunicationStatus, GattDeviceService,
};
use windows::Devices::Bluetooth::{
    BluetoothCacheMode, BluetoothConnectionStatus, BluetoothLEDevice,
};
use windows::Devices::Enumeration::DeviceInformation;

use crate::util::{block_on, guid};
use crate::{BleError, Result};

/// 取 ANCS 服务失败后重试几次、每次间隔多久。
const SERVICE_ATTEMPTS: usize = 8;
const SERVICE_RETRY_DELAY: Duration = Duration::from_secs(2);

/// 链路刚建立时 GATT 层可能还没就绪 —— 实测这时候取服务会报
/// `0x80070020`（共享冲突）。等一下再取就正常了。
///
/// ★ 这是 Rust 版新发现的：C# 版没有这个等待，
///   它只在"杀掉残留进程"之后碰巧能过，其实真因是时序。
pub const SETTLE_BEFORE_SERVICE: Duration = Duration::from_millis(1500);

/// 找一个暴露 ANCS 服务的设备。
///
/// 找不到就一直等（每 2 秒重试），直到超时或者被要求停止。
pub fn find_device(timeout: Duration, stop: &AtomicBool) -> Result<BluetoothLEDevice> {
    let deadline = Instant::now() + timeout;
    let selector = GattDeviceService::GetDeviceSelectorFromUuid(guid(ANCS_SERVICE_UUID))?;

    loop {
        if stop.load(Ordering::Relaxed) {
            return Err("已要求停止".into());
        }

        let devices =
            block_on(async { DeviceInformation::FindAllAsyncAqsFilter(&selector)?.await })?;

        let count = devices.Size()?;
        if count > 0 {
            for index in 0..count {
                if let Ok(info) = devices.GetAt(index) {
                    info!(
                        "发现暴露 ANCS 服务的设备 [{}] name={} id={}",
                        index,
                        info.Name().map(|n| n.to_string()).unwrap_or_default(),
                        info.Id().map(|i| i.to_string()).unwrap_or_default()
                    );
                }
            }

            let info = devices.GetAt(0)?;
            let device = block_on(async { BluetoothLEDevice::FromIdAsync(&info.Id()?)?.await })?;
            return Ok(device);
        }

        if Instant::now() > deadline {
            return Err(BleError::Message(
                "找不到任何暴露 ANCS 服务的设备。\n\
                 请先在 iPhone 的「设置 > 蓝牙」里把这台电脑配对。"
                    .to_string(),
            ));
        }

        sleep_unless_stopped(Duration::from_secs(2), stop);
    }
}

/// 等 iPhone 把链路连上来。
pub fn wait_for_connection(
    device: &BluetoothLEDevice,
    timeout: Duration,
    stop: &AtomicBool,
) -> Result<()> {
    if is_connected(device) {
        return Ok(());
    }

    info!("等 iPhone 连过来（如果一直不动，请在 iPhone 上打开一次「设置 > 蓝牙」）");

    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        if stop.load(Ordering::Relaxed) {
            return Err("已要求停止".into());
        }

        if is_connected(device) {
            return Ok(());
        }

        std::thread::sleep(Duration::from_millis(500));
    }

    Err("等待链路超时".into())
}

/// 取 ANCS 服务，带重试。
pub fn get_ancs_service(device: &BluetoothLEDevice) -> Result<GattDeviceService> {
    let mut last_error = String::from("（还没试过）");

    for attempt in 1..=SERVICE_ATTEMPTS {
        let outcome = block_on(async {
            device
                .GetGattServicesForUuidWithCacheModeAsync(
                    guid(ANCS_SERVICE_UUID),
                    BluetoothCacheMode::Uncached,
                )?
                .await
        });

        match outcome {
            Ok(result) => match result.Status() {
                Ok(GattCommunicationStatus::Success) => match result.Services() {
                    Ok(services) if services.Size()? > 0 => {
                        info!("取到 ANCS 服务（第 {attempt} 次尝试）");
                        return Ok(services.GetAt(0)?);
                    }
                    Ok(_) => last_error = "服务列表是空的".to_string(),
                    Err(error) => last_error = format!("读 Services 失败：{error}"),
                },
                Ok(other) => last_error = format!("状态 {other:?}"),
                Err(error) => last_error = format!("读 Status 失败：{error}"),
            },
            Err(error) => last_error = format!("{error}"),
        }

        warn!("第 {attempt} 次取 ANCS 服务失败：{last_error}；{SERVICE_RETRY_DELAY:?} 后重试");
        std::thread::sleep(SERVICE_RETRY_DELAY);
    }

    Err(BleError::Message(format!(
        "取不到 ANCS 服务，最后错误：{last_error}\n\
         提示：0x80070020 表示有别的程序正占用这台蓝牙设备\
         （例如另开了一个 SmsPop，或者蓝牙调试工具没关）"
    )))
}

pub fn is_connected(device: &BluetoothLEDevice) -> bool {
    device
        .ConnectionStatus()
        .map(|status| status == BluetoothConnectionStatus::Connected)
        .unwrap_or(false)
}

/// 设备地址（用作通知的 device_id，去重时要用）。
pub fn device_address(device: &BluetoothLEDevice) -> String {
    device
        .BluetoothAddress()
        .map(|address| format!("{address:012X}"))
        .unwrap_or_else(|_| "unknown".to_string())
}

pub fn device_name(device: &BluetoothLEDevice) -> String {
    device
        .Name()
        .map(|name| name.to_string())
        .unwrap_or_default()
}

/// 可被打断的 sleep。
pub fn sleep_unless_stopped(duration: Duration, stop: &AtomicBool) {
    let deadline = Instant::now() + duration;

    while Instant::now() < deadline {
        if stop.load(Ordering::Relaxed) {
            return;
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        std::thread::sleep(remaining.min(Duration::from_millis(200)));
    }
}
