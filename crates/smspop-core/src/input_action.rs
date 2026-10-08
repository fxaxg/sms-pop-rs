pub trait InputBackend {
    type Target: PartialEq;
    fn trusted(&self) -> bool;
    fn target(&self) -> Result<Self::Target, String>;
    fn send(&mut self, target: &Self::Target, code: &str) -> Result<(), String>;
}
pub fn perform<B: InputBackend>(
    backend: &mut B,
    code: &str,
    valid: impl Fn() -> bool,
) -> Result<(), String> {
    if !backend.trusted() {
        return Err(
            "请在系统设置 → 隐私与安全性 → 辅助功能中允许 SmsPop，然后重试或手动复制".into(),
        );
    }
    let target = backend.target()?;
    if !valid() {
        return Err("验证码已过期或被新消息替换，请重新获取".into());
    }
    if !backend.trusted() || target != backend.target()? {
        return Err("焦点或权限已变化，请重新操作".into());
    }
    if !valid() {
        return Err("验证码已过期".into());
    }
    backend.send(&target, code)
}
#[derive(Default)]
pub struct PressLatch {
    down: bool,
}
impl PressLatch {
    pub fn event(&mut self, pressed: bool) -> bool {
        let fire = pressed && !self.down;
        self.down = pressed;
        fire
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    struct Backend {
        trusted: bool,
        targets: [Option<u32>; 2],
        reads: Cell<usize>,
        writes: Vec<String>,
    }
    impl InputBackend for Backend {
        type Target = u32;
        fn trusted(&self) -> bool {
            self.trusted
        }
        fn target(&self) -> Result<u32, String> {
            let i = self.reads.get().min(1);
            self.reads.set(i + 1);
            self.targets[i].ok_or("Not editable".into())
        }
        fn send(&mut self, _: &u32, code: &str) -> Result<(), String> {
            self.writes.push(code.into());
            Ok(())
        }
    }
    fn backend() -> Backend {
        Backend {
            trusted: true,
            targets: [Some(1), Some(1)],
            reads: Cell::new(0),
            writes: vec![],
        }
    }
    #[test]
    fn denied_permission_does_not_write() {
        let mut b = backend();
        b.trusted = false;
        assert!(perform(&mut b, "123456", || true).is_err());
        assert!(b.writes.is_empty());
    }
    #[test]
    fn unknown_target_does_not_write() {
        let mut b = backend();
        b.targets = [None, None];
        assert!(perform(&mut b, "123456", || true).is_err());
        assert!(b.writes.is_empty());
    }
    #[test]
    fn changed_focus_does_not_write() {
        let mut b = backend();
        b.targets[1] = Some(2);
        assert!(perform(&mut b, "123456", || true).is_err());
        assert!(b.writes.is_empty());
    }
    #[test]
    fn expired_candidate_does_not_write() {
        let mut b = backend();
        assert!(perform(&mut b, "123456", || false).is_err());
        assert!(b.writes.is_empty());
    }
    #[test]
    fn eligible_action_sends_only_code() {
        let mut b = backend();
        perform(&mut b, "123456", || true).unwrap();
        assert_eq!(b.writes, vec!["123456"]);
    }
    #[test]
    fn repeated_press_waits_for_release() {
        let mut l = PressLatch::default();
        assert!(!l.event(false));
        assert!(l.event(true));
        assert!(!l.event(true));
        assert!(!l.event(false));
        assert!(l.event(true));
    }
}
