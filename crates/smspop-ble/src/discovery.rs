//! 找设备、等链路、取 ANCS 服务。
//!
//! 这三个动作都**必须带超时**：WinRT 的枚举和连接调用在状态不一致时
//! 可能挂住很久（实测遇到过 39 分钟没有任何返回），
//! 不设上限的话整个 Bluetooth 线程就废在那里了。

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use log::{info, warn};
use smspop_core::ancs::ANCS_SERVICE_UUID;
use smspop_core::uuid::Uuid;
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCommunicationStatus, GattDeviceService,
};
use windows::Devices::Bluetooth::{
    BluetoothCacheMode, BluetoothConnectionStatus, BluetoothLEDevice,
};
use windows::Devices::Enumeration::DeviceInformation;

use crate::util::buffer_to_vec;
use crate::util::{block_on_timeout, guid};
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

const BATTERY_SERVICE_UUID: Uuid = Uuid::from_bytes([
    0x00, 0x00, 0x18, 0x0F, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB,
]);
const BATTERY_LEVEL_UUID: Uuid = Uuid::from_bytes([
    0x00, 0x00, 0x2A, 0x19, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB,
]);

/// 找到的 ANCS 设备候选。
pub struct DiscoveredDevice {
    pub device: BluetoothLEDevice,
    pub id: String,
    pub name: String,
    pub connected: bool,
}

/// 找出所有暴露 ANCS 服务的设备。
///
/// 找不到就一直等（每 2 秒重试），直到超时或者被要求停止。
pub fn find_devices(timeout: Duration, stop: &AtomicBool) -> Result<Vec<DiscoveredDevice>> {
    let deadline = Instant::now() + timeout;
    let selector = GattDeviceService::GetDeviceSelectorFromUuid(guid(ANCS_SERVICE_UUID))?;

    loop {
        if stop.load(Ordering::Relaxed) {
            return Err("已要求停止".into());
        }

        let devices = block_on_timeout(
            async { DeviceInformation::FindAllAsyncAqsFilter(&selector)?.await },
            Duration::from_secs(8),
        )??;

        let count = devices.Size()?;
        if count > 0 {
            let mut found = Vec::with_capacity(count as usize);
            for index in 0..count {
                let info = devices.GetAt(index)?;
                let device = match block_on_timeout(
                    async { BluetoothLEDevice::FromIdAsync(&info.Id()?)?.await },
                    Duration::from_secs(8),
                )? {
                    Ok(device) => device,
                    Err(error) => {
                        warn!("打开第 {index} 个 ANCS 候选失败，跳过：{error}");
                        continue;
                    }
                };
                let id = device_address(&device);
                let name = device_name(&device);
                let connected = is_connected(&device);
                info!(
                    "发现 ANCS 候选 [{}] name={} address={} connected={}",
                    index, name, id, connected
                );
                found.push(DiscoveredDevice {
                    device,
                    id,
                    name,
                    connected,
                });
            }
            if !found.is_empty() {
                return Ok(found);
            }
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
        let outcome = block_on_timeout(
            async {
                device
                    .GetGattServicesForUuidWithCacheModeAsync(
                        guid(ANCS_SERVICE_UUID),
                        BluetoothCacheMode::Uncached,
                    )?
                    .await
            },
            Duration::from_secs(8),
        )?;

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

/// 尝试读取标准 BLE Battery Service。任何不支持或读取失败都返回 None，
/// 绝不影响 ANCS 主链路。
pub fn try_read_battery_level(device: &BluetoothLEDevice) -> Option<u8> {
    let services_result = block_on_timeout(
        async {
            device
                .GetGattServicesForUuidWithCacheModeAsync(
                    guid(BATTERY_SERVICE_UUID),
                    BluetoothCacheMode::Uncached,
                )
                .ok()?
                .await
                .ok()
        },
        Duration::from_secs(2),
    )
    .ok()??;
    if services_result.Status().ok()? != GattCommunicationStatus::Success {
        return None;
    }
    let services = services_result.Services().ok()?;
    if services.Size().ok()? == 0 {
        return None;
    }
    let service = services.GetAt(0).ok()?;

    let characteristics_result = block_on_timeout(
        async {
            service
                .GetCharacteristicsForUuidWithCacheModeAsync(
                    guid(BATTERY_LEVEL_UUID),
                    BluetoothCacheMode::Uncached,
                )
                .ok()?
                .await
                .ok()
        },
        Duration::from_secs(2),
    )
    .ok()??;
    if characteristics_result.Status().ok()? != GattCommunicationStatus::Success {
        return None;
    }
    let characteristics = characteristics_result.Characteristics().ok()?;
    if characteristics.Size().ok()? == 0 {
        return None;
    }
    let characteristic = characteristics.GetAt(0).ok()?;
    let read_result = block_on_timeout(
        async {
            characteristic
                .ReadValueWithCacheModeAsync(BluetoothCacheMode::Uncached)
                .ok()?
                .await
                .ok()
        },
        Duration::from_secs(2),
    )
    .ok()??;
    if read_result.Status().ok()? != GattCommunicationStatus::Success {
        return None;
    }
    let bytes = buffer_to_vec(&read_result.Value().ok()?).ok()?;
    bytes.first().copied().filter(|level| *level <= 100)
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
