//! 一个极小的 UUID 类型。
//!
//! 为什么不用 `uuid` crate：我们只需要几个常量，为这个引依赖不划算。
//! 而"字节序"这件事**必须有个地方写清楚** ——
//! 蓝牙空口上的 128 位 UUID 是**整段反转**的，这是很容易踩、又很难查的坑。
//!
//! 所以这里统一约定：内存里永远存 RFC 4122 顺序（大端），
//! 什么时候要小端，显式调 [`Uuid::to_le_bytes`]。

use std::fmt;

/// 按 RFC 4122 顺序（大端）保存的 128 位 UUID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Uuid([u8; 16]);

impl Uuid {
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// RFC 4122 顺序的原始字节。
    pub const fn as_rfc4122(&self) -> [u8; 16] {
        self.0
    }

    /// 蓝牙广播 / GATT 里用的顺序：**整段字节反转**。
    ///
    /// 例：`7905F431-B5CE-4E99-A40F-4B1E122D00D0`
    /// 在空口上是 `D0 00 2D 12 1E 4B 0F A4 99 4E CE B5 31 F4 05 79`。
    pub const fn to_le_bytes(&self) -> [u8; 16] {
        let b = self.0;
        [
            b[15], b[14], b[13], b[12], b[11], b[10], b[9], b[8], b[7], b[6], b[5], b[4], b[3],
            b[2], b[1], b[0],
        ]
    }

    /// 转成 `u128`（大端解释）。
    ///
    /// Windows 侧的 `GUID::from_u128` 吃的就是这个 —— 它按大端理解 u128，
    /// 所以这层转换正好把我们的 RFC 4122 字节喂对。
    pub fn to_u128(&self) -> u128 {
        u128::from_be_bytes(self.0)
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let b = self.0;
        write!(
            f,
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13],
            b[14], b[15]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ANCS 服务 UUID —— 这个常量错了整条链路都走不通，所以钉死。
    const ANCS_SERVICE: Uuid = Uuid::from_bytes([
        0x79, 0x05, 0xF4, 0x31, 0xB5, 0xCE, 0x4E, 0x99, 0xA4, 0x0F, 0x4B, 0x1E, 0x12, 0x2D, 0x00,
        0xD0,
    ]);

    #[test]
    fn 空口顺序是整段反转() {
        assert_eq!(
            ANCS_SERVICE.to_le_bytes(),
            [
                0xD0, 0x00, 0x2D, 0x12, 0x1E, 0x4B, 0x0F, 0xA4, 0x99, 0x4E, 0xCE, 0xB5, 0x31, 0xF4,
                0x05, 0x79,
            ]
        );
    }

    #[test]
    // 故意按 UUID 的 8-4-4-4-12 分组，好和文档里的写法对上
    #[allow(clippy::unusual_byte_groupings)]
    fn 转_u128_给_windows_的_guid_用() {
        assert_eq!(
            ANCS_SERVICE.to_u128(),
            0x7905_F431_B5CE_4E99_A40F_4B1E122D00D0
        );
    }

    #[test]
    fn 显示成标准写法() {
        assert_eq!(
            ANCS_SERVICE.to_string(),
            "7905f431-b5ce-4e99-a40f-4b1e122d00d0"
        );
    }
}
