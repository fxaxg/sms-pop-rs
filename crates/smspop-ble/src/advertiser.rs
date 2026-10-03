//! 广播 ANCS 服务请求。
//!
//! iPhone 靠广播里的这一小段字节认出"对面是个想收通知的 ANCS 配件"。
//! 少一个字节都不行 —— 少了 iPhone 就完全不理你，而且**没有任何报错**。

use log::{info, warn};
use smspop_core::ancs::{AD_TYPE_SERVICE_SOLICITATION_128BIT, ANCS_SERVICE_UUID};
use windows::Devices::Bluetooth::Advertisement::{
    BluetoothLEAdvertisement, BluetoothLEAdvertisementDataSection,
    BluetoothLEAdvertisementPublisher, BluetoothLEManufacturerData,
};

use crate::util::to_buffer;
use crate::Result;

/// 我们自己的厂商数据公司标识。
///
/// 蓝牙规范要求一个可连接 / 可发现的广播里**至少有两个数据段**，
/// 只放 ANCS 服务请求的话有些栈会拒绝，所以再加一段厂商数据凑数。
const COMPANY_ID: u16 = 0x010E;

/// 开始广播。返回的 publisher **必须一直活着**，否则广播就停了。
pub fn start() -> Result<BluetoothLEAdvertisementPublisher> {
    let advertisement = BluetoothLEAdvertisement::new()?;

    // AD type 0x15 = 128 位服务请求，载荷是 ANCS 服务 UUID 的**空口字节序**
    let solicitation = BluetoothLEAdvertisementDataSection::Create(
        AD_TYPE_SERVICE_SOLICITATION_128BIT,
        &to_buffer(&ANCS_SERVICE_UUID.to_le_bytes())?,
    )?;
    advertisement.DataSections()?.Append(&solicitation)?;

    let manufacturer = BluetoothLEManufacturerData::Create(COMPANY_ID, &to_buffer(&[0x01, 0x00])?)?;
    advertisement.ManufacturerData()?.Append(&manufacturer)?;

    let publisher = BluetoothLEAdvertisementPublisher::Create(&advertisement)?;
    publisher.Start()?;

    info!("ANCS 服务请求广播已启动");
    Ok(publisher)
}

/// 停止广播（退出时调用）。
pub fn stop(publisher: &BluetoothLEAdvertisementPublisher) {
    if let Err(error) = publisher.Stop() {
        warn!("停止 ANCS 广播失败：{error}");
    }
}
