//! 一次 ANCS 会话：订阅三个特征、串行化 Control Point、重组 Data Source。
//!
//! 这个文件是整条链路里坑最密集的地方，下面每一条都是真机实测踩出来的：
//!
//! * **必须先订阅 Data Source 再订阅 Notification Source**（顺序反了收不到）
//! * **不能在 `ValueChanged` 回调线程里写 Control Point** ——
//!   Windows 蓝牙栈会拒绝（实测报 `0x806500A2`）。所以回调只把 UID 丢进 channel，
//!   由一条独立线程去写。
//! * 通知刚到的瞬间 iOS 可能还没登记完，所以首次写入前要等一下并重试
//! * Data Source 的长度是**字节**，而且**可能分片** —— 累积到收满 tuple 才算完整
//! * 同一时刻**只能有一个 Control Point 请求在飞**，所以写入必须串行

use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use log::{debug, info, warn};
use smspop_core::ancs::{self, ParseStatus};
use smspop_core::model::PhoneNotification;
use windows::Devices::Bluetooth::BluetoothLEDevice;
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristic, GattClientCharacteristicConfigurationDescriptorValue,
    GattCommunicationStatus, GattDeviceService, GattValueChangedEventArgs, GattWriteOption,
};
use windows::Foundation::TypedEventHandler;

use crate::discovery;
use crate::util::{buffer_to_vec, to_buffer};
use crate::{BleError, Result};

/// 收到一条通知时调用。注意这是**回调线程**，调用方自己负责切线程。
pub type NotificationSink = Arc<dyn Fn(PhoneNotification) + Send + Sync>;

/// 写入 Control Point 的重试次数。
const WRITE_ATTEMPTS: usize = 3;
/// 首次写入前的等待 —— 通知刚到就写，iOS 常常还没登记完。
const FIRST_WRITE_DELAY: Duration = Duration::from_millis(120);
const RETRY_WRITE_DELAY: Duration = Duration::from_millis(300);

/// 会话内多个回调之间共享的状态。
#[derive(Default)]
struct Shared {
    /// Data Source 的累积缓冲（一条响应可能分几包推回来）。
    data_buffer: Mutex<Vec<u8>>,
    /// 本次请求期望收到几个 tuple —— 收满才算完整。
    expected_tuples: Mutex<usize>,
}

/// 一个活着的 ANCS 会话。drop 或 [`AncsSession::close`] 会退订并停掉写入线程。
pub struct AncsSession {
    service: GattDeviceService,
    notification_source: GattCharacteristic,
    data_source: GattCharacteristic,

    /// ★ 这两个 handler 必须活到会话结束。丢了它们等于取消订阅。
    _ns_handler: TypedEventHandler<GattCharacteristic, GattValueChangedEventArgs>,
    _ds_handler: TypedEventHandler<GattCharacteristic, GattValueChangedEventArgs>,

    /// 给写入线程送 UID。drop 掉它就等于让线程退出。
    uid_tx: Option<Sender<[u8; 4]>>,
    writer: Option<JoinHandle<()>>,
}

impl AncsSession {
    /// 打开会话：取特征 → 挂回调 → 订阅 → 起写入线程。
    ///
    /// `device` 必须已经连上（调用方先跑 [`discovery::wait_for_connection`]）。
    pub fn open(
        device: &BluetoothLEDevice,
        device_id: &str,
        device_name: &str,
        sink: NotificationSink,
    ) -> Result<Self> {
        let service = discovery::get_ancs_service(device)?;

        let notification_source = first_characteristic(&service, ancs::NOTIFICATION_SOURCE_UUID)?;
        let control_point = first_characteristic(&service, ancs::CONTROL_POINT_UUID)?;
        let data_source = first_characteristic(&service, ancs::DATA_SOURCE_UUID)?;

        // 让 Windows 在链路"看起来断了但其实还在"的时候主动把它拉回来。
        // 两边配对状态不一致时实测有用（几秒内就恢复）。失败不影响主流程。
        match service.Session() {
            Ok(session) => {
                if let Err(error) = session.SetMaintainConnection(true) {
                    debug!("设置 MaintainConnection 失败（不影响使用）：{error}");
                }
            }
            Err(error) => debug!("取 GattSession 失败（不影响使用）：{error}"),
        }

        let shared = Arc::new(Shared::default());

        // ------------------------------------------------------------------
        // Data Source：累积分片，能解析出完整响应就交给上层
        // ------------------------------------------------------------------
        let ds_shared = Arc::clone(&shared);
        let ds_sink = Arc::clone(&sink);
        let ds_device_id = device_id.to_string();
        let ds_device_name = Some(device_name.to_string());

        let ds_handler = TypedEventHandler::<GattCharacteristic, GattValueChangedEventArgs>::new(
            move |_sender, args| {
                let Some(args) = args.as_ref() else {
                    return Ok(());
                };

                let chunk = buffer_to_vec(&args.CharacteristicValue()?)?;
                debug!("Data Source 收到 {} 字节分片", chunk.len());

                {
                    let mut buffer = ds_shared
                        .data_buffer
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    buffer.extend_from_slice(&chunk);
                }

                let expected = *ds_shared
                    .expected_tuples
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());

                let snapshot = ds_shared
                    .data_buffer
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clone();

                let (status, attributes, consumed) = ancs::try_parse_response(&snapshot, expected);

                if status == ParseStatus::Complete {
                    ds_shared
                        .data_buffer
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .drain(..consumed);

                    if let Some(attributes) = attributes {
                        // 隐私：这里**不能**打日志记正文，只记长度和 uid
                        info!(
                            "回读到通知 uid={} 正文长度={}",
                            attributes.uid,
                            attributes.message.as_deref().map(str::len).unwrap_or(0)
                        );

                        let notification = PhoneNotification::from_ancs(
                            ds_device_id.clone(),
                            ds_device_name.clone(),
                            attributes,
                        );

                        ds_sink(notification);
                    }
                }

                Ok(())
            },
        );

        data_source.ValueChanged(&ds_handler)?;

        // ------------------------------------------------------------------
        // Notification Source：只解析事件，真正写 Control Point 交给独立线程
        // ------------------------------------------------------------------
        let (uid_tx, uid_rx) = mpsc::channel::<[u8; 4]>();

        // 闭包要把 sender 拿走一份，struct 里还要留一份用来在关闭时断开
        let ns_uid_tx = uid_tx.clone();

        let ns_handler = TypedEventHandler::<GattCharacteristic, GattValueChangedEventArgs>::new(
            move |_sender, args| {
                let Some(args) = args.as_ref() else {
                    return Ok(());
                };

                let bytes = buffer_to_vec(&args.CharacteristicValue()?)?;

                let Some(event) = ancs::parse_notification_source(&bytes) else {
                    warn!("Notification Source 载荷长度异常：{}", bytes.len());
                    return Ok(());
                };

                // Removed 只是"通知被划掉了"，我们不关心，也不要去回读属性
                if !event.should_fetch_attributes() {
                    debug!(
                        "跳过事件 {:?} flags=0x{:02X} uid={}",
                        event.event_id, event.flags, event.uid
                    );
                    return Ok(());
                }

                info!(
                    "通知事件 {:?} category={} uid={}",
                    event.event_id, event.category, event.uid
                );

                // ★ 关键：只丢 UID 过去，绝不在这里直接写 Control Point
                let _ = ns_uid_tx.send([bytes[4], bytes[5], bytes[6], bytes[7]]);
                Ok(())
            },
        );

        notification_source.ValueChanged(&ns_handler)?;

        // ------------------------------------------------------------------
        // 订阅 —— ★ 顺序重要：先 Data Source，再 Notification Source
        // ------------------------------------------------------------------
        let ds_status = subscribe(&data_source)?;
        let ns_status = subscribe(&notification_source)?;

        info!("订阅结果：NotificationSource={ds_status:?} DataSource={ns_status:?}");

        if ds_status != GattCommunicationStatus::Success
            || ns_status != GattCommunicationStatus::Success
        {
            return Err(BleError::Message(format!(
                "订阅 ANCS 失败（NotificationSource={ds_status:?} DataSource={ns_status:?}）"
            )));
        }

        // ------------------------------------------------------------------
        // Control Point 写入线程（串行化 + 重试）
        // ------------------------------------------------------------------
        let writer_shared = Arc::clone(&shared);
        let writer = thread::Builder::new()
            .name("control-point-writer".to_string())
            .spawn(move || control_point_writer(uid_rx, control_point, writer_shared))
            .map_err(|error| {
                BleError::Message(format!("起 Control Point 写入线程失败：{error}"))
            })?;

        info!("ANCS 已订阅，开始收通知");

        Ok(Self {
            service,
            notification_source,
            data_source,
            _ns_handler: ns_handler,
            _ds_handler: ds_handler,
            uid_tx: Some(uid_tx),
            writer: Some(writer),
        })
    }

    /// 链路还在不在。
    pub fn is_connected(&self) -> bool {
        // 特征还活着就说明服务还在；链路状态由调用方通过 device 查询
        self.notification_source.Uuid().is_ok()
    }

    /// 主动退订并停掉写入线程。
    pub fn close(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        // 关掉 channel → 写入线程跳出循环
        self.uid_tx = None;

        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }

        unsubscribe(&self.data_source);
        unsubscribe(&self.notification_source);

        let _ = &self.service;
        info!("ANCS 会话已关闭");
    }
}

impl Drop for AncsSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// 订阅（开通知）。
fn subscribe(characteristic: &GattCharacteristic) -> Result<GattCommunicationStatus> {
    Ok(crate::util::block_on(async {
        characteristic
            .WriteClientCharacteristicConfigurationDescriptorAsync(
                GattClientCharacteristicConfigurationDescriptorValue::Notify,
            )?
            .await
    })?)
}

fn unsubscribe(characteristic: &GattCharacteristic) {
    let result = crate::util::block_on(async {
        characteristic
            .WriteClientCharacteristicConfigurationDescriptorAsync(
                GattClientCharacteristicConfigurationDescriptorValue::None,
            )?
            .await
    });

    if let Err(error) = result {
        debug!("退订失败（通常无所谓）：{error}");
    }
}

/// 取某个 UUID 的第一个特征。
fn first_characteristic(
    service: &GattDeviceService,
    uuid: smspop_core::uuid::Uuid,
) -> Result<GattCharacteristic> {
    let characteristics = service.GetCharacteristics(crate::util::guid(uuid))?;

    if characteristics.Size()? == 0 {
        return Err(BleError::Message(format!("ANCS 服务里找不到特征 {uuid}")));
    }

    Ok(characteristics.GetAt(0)?)
}

/// Control Point 的写入线程。
///
/// ★ 为什么必须是独立线程：见文件头的说明 —— 不能在 `ValueChanged` 回调里直接写。
///
/// 另外每次都先重置累积状态、设好期望的 tuple 数，再发请求，
/// 这样 Data Source 那边收到分片时才知道"收满几个才算完整"。
fn control_point_writer(
    uid_rx: mpsc::Receiver<[u8; 4]>,
    control_point: GattCharacteristic,
    state: Arc<Shared>,
) {
    while let Ok(raw_uid) = uid_rx.recv() {
        let mut succeeded = false;

        for attempt in 1..=WRITE_ATTEMPTS {
            thread::sleep(if attempt == 1 {
                FIRST_WRITE_DELAY
            } else {
                RETRY_WRITE_DELAY
            });

            let request = ancs::build_get_attributes_request(raw_uid);

            *state
                .expected_tuples
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = ancs::REQUESTED_ATTRIBUTE_COUNT;

            state
                .data_buffer
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clear();

            let buffer = match to_buffer(&request) {
                Ok(buffer) => buffer,
                Err(error) => {
                    warn!("构造 Control Point 请求失败：{error}");
                    break;
                }
            };

            let result = crate::util::block_on(async {
                control_point
                    .WriteValueWithResultAndOptionAsync(
                        &buffer,
                        GattWriteOption::WriteWithResponse,
                    )?
                    .await
            });

            match result {
                Ok(write_result) => {
                    let status = write_result.Status();

                    if status == Ok(GattCommunicationStatus::Success) {
                        debug!("Control Point 写入成功（第 {attempt} 次）");
                        succeeded = true;
                        break;
                    }

                    // 协议错误通常意味着 UID 不匹配 —— 通知太新，重试就好
                    if let Ok(reference) = write_result.ProtocolError() {
                        if let Ok(code) = reference.Value() {
                            debug!("Control Point 协议错误 0x{code:02X}（第 {attempt} 次）");
                        }
                    }

                    warn!("Control Point 写入未成功（第 {attempt} 次）：{status:?}");
                }
                Err(error) => warn!("Control Point 写入异常（第 {attempt} 次）：{error}"),
            }
        }

        if !succeeded {
            warn!("Control Point 三次都没写成功，放弃这条通知");
        }
    }

    debug!("Control Point 写入线程退出");
}
