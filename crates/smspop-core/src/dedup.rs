//! 通知去重。
//!
//! ANCS 里同一条通知可能先 `Added` 再 `Modified`（内容一样），不去重就会弹两次。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::model::PhoneNotification;

/// 默认去重窗口。
pub const DEFAULT_RETENTION: Duration = Duration::from_secs(10 * 60);

/// 按 `设备 + NotificationUID` 去重。
///
/// 内部带锁：BLE 线程和 UI 线程都可能碰到它，省得调用方记着"只能在某个线程调用"。
#[derive(Debug)]
pub struct NotificationDeduplicator {
    retention: Duration,
    seen: Mutex<HashMap<String, Instant>>,
}

impl Default for NotificationDeduplicator {
    fn default() -> Self {
        Self::new(DEFAULT_RETENTION)
    }
}

impl NotificationDeduplicator {
    pub fn new(retention: Duration) -> Self {
        Self {
            retention,
            seen: Mutex::new(HashMap::new()),
        }
    }

    /// `true` 表示这条已经出现过了，应当忽略。
    pub fn is_duplicate(&self, notification: &PhoneNotification) -> bool {
        self.is_duplicate_at(notification, Instant::now())
    }

    /// 指定"当前时间"的版本 —— 只有测试会用，这样不用真的等 10 分钟。
    pub fn is_duplicate_at(&self, notification: &PhoneNotification, now: Instant) -> bool {
        let key = build_key(notification);

        // 锁中毒（有线程在持锁时 panic）不该让整条链路停摆，直接取回数据
        let mut seen = self
            .seen
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        seen.retain(|_, first_seen| now.duration_since(*first_seen) < self.retention);

        if seen.contains_key(&key) {
            return true;
        }

        seen.insert(key, now);
        false
    }

    pub fn clear(&self) {
        self.seen
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
    }

    /// 当前记住的条数（排障用）。
    pub fn len(&self) -> usize {
        self.seen
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn build_key(notification: &PhoneNotification) -> String {
    format!("{}|{}", notification.device_id, notification.uid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ancs::Attributes;

    fn notification(device: &str, uid: u32) -> PhoneNotification {
        PhoneNotification::from_ancs(
            device,
            None,
            Attributes {
                uid,
                ..Default::default()
            },
        )
    }

    #[test]
    fn 同一条通知第二次就算重复() {
        let dedup = NotificationDeduplicator::default();
        let first = notification("dev", 42);

        assert!(!dedup.is_duplicate(&first));
        assert!(dedup.is_duplicate(&first));
        assert!(dedup.is_duplicate(&first));
    }

    #[test]
    fn 不同_uid_互不影响() {
        let dedup = NotificationDeduplicator::default();

        assert!(!dedup.is_duplicate(&notification("dev", 1)));
        assert!(!dedup.is_duplicate(&notification("dev", 2)));
        assert_eq!(dedup.len(), 2);
    }

    #[test]
    fn 不同设备上的同一个_uid_不算重复() {
        let dedup = NotificationDeduplicator::default();

        assert!(!dedup.is_duplicate(&notification("dev-a", 7)));
        assert!(!dedup.is_duplicate(&notification("dev-b", 7)));
    }

    #[test]
    fn 超过保留窗口后不再是重复() {
        let dedup = NotificationDeduplicator::new(Duration::from_secs(600));
        let start = Instant::now();

        assert!(!dedup.is_duplicate_at(&notification("dev", 9), start));
        assert!(dedup.is_duplicate_at(&notification("dev", 9), start + Duration::from_secs(599)));

        // 过了窗口，而且这次查询会把旧记录清掉
        assert!(!dedup.is_duplicate_at(&notification("dev", 9), start + Duration::from_secs(601)));
    }

    #[test]
    fn 清理只清过期的() {
        let dedup = NotificationDeduplicator::new(Duration::from_secs(100));
        let start = Instant::now();
        let t150 = start + Duration::from_secs(150);

        dedup.is_duplicate_at(&notification("dev", 1), start);
        dedup.is_duplicate_at(&notification("dev", 2), start + Duration::from_secs(90));

        // 这次查询会把已过期的 uid=1 清掉，再插入 uid=3
        dedup.is_duplicate_at(&notification("dev", 3), t150);
        assert_eq!(dedup.len(), 2);

        // uid=2 只过了 60 秒，还在窗口内 → 仍然是重复
        assert!(dedup.is_duplicate_at(&notification("dev", 2), t150));

        // uid=1 已经过了 150 秒，超出 100 秒窗口 → 又变成"没见过的"
        assert!(!dedup.is_duplicate_at(&notification("dev", 1), t150));
    }

    #[test]
    fn clear_之后又能重新收() {
        let dedup = NotificationDeduplicator::default();
        let one = notification("dev", 5);

        assert!(!dedup.is_duplicate(&one));
        dedup.clear();
        assert!(!dedup.is_duplicate(&one));
    }
}
