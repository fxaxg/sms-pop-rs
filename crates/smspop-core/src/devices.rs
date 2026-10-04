//! 多设备选择策略。只决定优先级，不接触 Windows 蓝牙 API。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnownDevice {
    /// BLE 地址。对同一台电脑上的 iPhone 足够稳定，用作注册表主键。
    pub id: String,
    pub name: String,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    #[serde(default)]
    pub preferred: bool,
    #[serde(default)]
    pub last_seen_unix_ms: Option<u64>,
    #[serde(default)]
    pub last_connected_unix_ms: Option<u64>,
    #[serde(default)]
    pub last_verified_unix_ms: Option<u64>,
}

const fn enabled_by_default() -> bool {
    true
}

/// 候选设备优先级：启用的首选设备优先，其后按最近验证、最近连接、最近发现排序。
pub fn sort_candidate_ids<'a>(
    candidate_ids: impl IntoIterator<Item = &'a str>,
    known: &[KnownDevice],
) -> Vec<String> {
    let mut ids: Vec<String> = candidate_ids
        .into_iter()
        .filter(|id| {
            known
                .iter()
                .find(|device| device.id.eq_ignore_ascii_case(id))
                .is_none_or(|device| device.enabled)
        })
        .map(str::to_string)
        .collect();

    ids.sort_by(|left, right| {
        let left = known
            .iter()
            .find(|device| device.id.eq_ignore_ascii_case(left));
        let right = known
            .iter()
            .find(|device| device.id.eq_ignore_ascii_case(right));
        rank(right).cmp(&rank(left))
    });
    ids
}

fn rank(device: Option<&KnownDevice>) -> (bool, u64, u64, u64, bool) {
    device.map_or((false, 0, 0, 0, false), |device| {
        (
            device.preferred,
            device.last_verified_unix_ms.unwrap_or(0),
            device.last_connected_unix_ms.unwrap_or(0),
            device.last_seen_unix_ms.unwrap_or(0),
            true,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, preferred: bool, verified: u64, enabled: bool) -> KnownDevice {
        KnownDevice {
            id: id.into(),
            name: id.into(),
            enabled,
            preferred,
            last_seen_unix_ms: None,
            last_connected_unix_ms: None,
            last_verified_unix_ms: Some(verified),
        }
    }

    #[test]
    fn 首选在线时优先首选() {
        let known = vec![device("a", false, 20, true), device("b", true, 1, true)];
        assert_eq!(sort_candidate_ids(["a", "b"], &known), ["b", "a"]);
    }

    #[test]
    fn 首选不在线时自动选择最近验证设备() {
        let known = vec![
            device("preferred", true, 30, true),
            device("old", false, 10, true),
            device("recent", false, 20, true),
        ];
        assert_eq!(
            sort_candidate_ids(["old", "recent"], &known),
            ["recent", "old"]
        );
    }

    #[test]
    fn 暂停设备不会成为候选() {
        let known = vec![device("a", true, 20, false), device("b", false, 1, true)];
        assert_eq!(sort_candidate_ids(["a", "b"], &known), ["b"]);
    }
}
