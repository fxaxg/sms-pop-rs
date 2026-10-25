//! 候选条的监听/停留时序。时间使用调用方提供的相对时长，无平台依赖。
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Visibility {
    pub visible: bool,
    pub until: Duration,
}

pub struct Tracker {
    lifetime: Duration,
    duration: Duration,
    target: Option<u64>,
    until: Duration,
}

impl Tracker {
    pub fn new(lifetime: Duration, duration: Duration) -> Self {
        Self {
            lifetime,
            duration,
            target: None,
            until: Duration::ZERO,
        }
    }

    /// None 表示整个监听期已到期。暂时探测不到焦点只隐藏，不重置计时。
    pub fn observe(&mut self, elapsed: Duration, target: Option<u64>) -> Option<Visibility> {
        if elapsed >= self.lifetime {
            return None;
        }
        if target.is_some() && target != self.target {
            self.target = target;
            self.until = elapsed.saturating_add(self.duration).min(self.lifetime);
        }
        Some(Visibility {
            visible: target.is_some() && elapsed < self.until,
            until: self.until,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn s(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    #[test]
    fn 切换输入框重置停留但同一输入框不重置() {
        let mut tracker = Tracker::new(s(60), s(8));
        assert_eq!(
            tracker.observe(s(1), Some(1)),
            Some(Visibility {
                visible: true,
                until: s(9)
            })
        );
        assert_eq!(tracker.observe(s(7), Some(1)).unwrap().until, s(9));
        assert!(!tracker.observe(s(9), Some(1)).unwrap().visible);
        assert!(!tracker.observe(s(15), Some(1)).unwrap().visible);
        assert_eq!(
            tracker.observe(s(16), Some(2)),
            Some(Visibility {
                visible: true,
                until: s(24)
            })
        );
    }

    #[test]
    fn 暂时失去焦点不延长计时且总监听时间不会被重置() {
        let mut tracker = Tracker::new(s(60), s(8));
        tracker.observe(s(1), Some(1));
        assert!(!tracker.observe(s(2), None).unwrap().visible);
        assert_eq!(tracker.observe(s(3), Some(1)).unwrap().until, s(9));
        assert_eq!(
            tracker.observe(s(59), Some(2)),
            Some(Visibility {
                visible: true,
                until: s(60)
            })
        );
        assert!(tracker.observe(s(60), Some(3)).is_none());
    }
}
