//! 本机的 GATT 服务端。
//!
//! 为什么需要它：ANCS 的角色是交叉的 —— iPhone 是链路层 Central，
//! 本机是 Peripheral。**Windows 只有在"跑着一个 GATT 服务端"的时候才是可连接的**，
//! iPhone 才连得上来、才有可能配对。
//!
//! ★ 那个"需要加密才可读"的特征是刻意加的：
//!   客户端一读它就触发配对，从而建立 LE bonding ——
//!   **没有 bonding 的话，ANCS 订阅会返回 Success，但一条通知都收不到**
//!   （这个坑实测确认过）。

use log::{info, warn};
use smspop_core::uuid::Uuid;
use windows::core::HSTRING;
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristicProperties, GattLocalCharacteristicParameters, GattProtectionLevel,
    GattServiceProvider, GattServiceProviderAdvertisementStatus,
    GattServiceProviderAdvertisingParameters,
};

use crate::util::{block_on, guid};
use crate::Result;

/// 我们自己的"存在感"服务，只是为了让本机可连接、并把配对逼出来。
const PRESENCE_SERVICE: Uuid = Uuid::from_bytes([
    0x00, 0x00, 0xFF, 0xF0, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB,
]);

const PRESENCE_CHARACTERISTIC: Uuid = Uuid::from_bytes([
    0x00, 0x00, 0xFF, 0xF1, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB,
]);

/// 广播状态是不是真的在跑。
pub fn is_advertising(provider: &GattServiceProvider) -> bool {
    provider
        .AdvertisementStatus()
        .map(|status| status == GattServiceProviderAdvertisementStatus::Started)
        .unwrap_or(false)
}

/// 起一个可连接的 GATT 服务端。
///
/// 返回的 provider **必须一直活着**，否则服务端连同广播一起消失。
pub fn start() -> Result<GattServiceProvider> {
    let result =
        block_on(async { GattServiceProvider::CreateAsync(guid(PRESENCE_SERVICE))?.await })?;
    let provider = result.ServiceProvider()?;

    let parameters = GattLocalCharacteristicParameters::new()?;
    parameters.SetCharacteristicProperties(GattCharacteristicProperties::Read)?;

    // ★ 关键的一行：读取需要加密 → 客户端一读就触发配对 → 建立 bonding
    parameters.SetReadProtectionLevel(GattProtectionLevel::EncryptionRequired)?;
    parameters.SetUserDescription(&HSTRING::from("SmsPop presence"))?;

    let created = block_on(async {
        provider
            .Service()?
            .CreateCharacteristicAsync(guid(PRESENCE_CHARACTERISTIC), &parameters)?
            .await
    })?;

    if created.Characteristic().is_err() {
        warn!("创建 presence 特征失败：{:?}", created.Error());
    }

    provider.StartAdvertisingWithParameters(&advertising_parameters()?)?;

    info!("GATT 服务端已启动（可连接 / 可发现）");
    Ok(provider)
}

fn advertising_parameters() -> windows::core::Result<GattServiceProviderAdvertisingParameters> {
    let advertising = GattServiceProviderAdvertisingParameters::new()?;
    advertising.SetIsConnectable(true)?;
    advertising.SetIsDiscoverable(true)?;
    Ok(advertising)
}

/// 确认广播真的在跑，不在就重试。
///
/// ★ 为什么需要这个：GATT 服务端的广播和 ANCS 的 `BluetoothLEAdvertisementPublisher`
///   会抢同一个广告位，GATT 那条经常被挤成 `Aborted`。
///   而 **`Aborted` 期间本机是不可连接的 —— iPhone 根本连不上**，
///   表现就是"一直停在等待连接"，而且完全看不出原因。
pub fn ensure_advertising(provider: &GattServiceProvider) -> Result<()> {
    for attempt in 1..=5 {
        if is_advertising(provider) {
            info!("GATT 广播已确认在运行（第 {attempt} 次检查）");
            return Ok(());
        }

        if let Ok(status) = provider.AdvertisementStatus() {
            warn!("GATT 广播当前为 {status:?}，第 {attempt} 次重试…");
        }

        let _ = provider.StopAdvertising();
        provider.StartAdvertisingWithParameters(&advertising_parameters()?)?;

        std::thread::sleep(std::time::Duration::from_millis(700));
    }

    Err("GATT 广播始终没能进入 Started —— 本机不可连接，iPhone 连不上".into())
}

/// 停止广播（退出时调用）。
pub fn stop(provider: &GattServiceProvider) {
    if let Err(error) = provider.StopAdvertising() {
        warn!("停止 GATT 广播失败：{error}");
    }
}
