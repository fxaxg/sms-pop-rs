use std::time::{Duration, Instant};
#[derive(Default)]
pub struct OtpCandidateStore {
    candidate: Option<(String, Instant)>,
}
impl OtpCandidateStore {
    pub fn offer(&mut self, code: String, received_at: Instant) {
        self.candidate = Some((code, received_at));
    }
    pub fn current(&mut self, now: Instant) -> Option<&str> {
        if self
            .candidate
            .as_ref()
            .is_some_and(|(_, t)| now.saturating_duration_since(*t) >= Duration::from_secs(120))
        {
            self.clear();
        }
        self.candidate.as_ref().map(|(code, _)| code.as_str())
    }
    pub fn clear(&mut self) {
        self.candidate = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn expires_at_120_seconds_not_119() {
        let t = Instant::now();
        let mut s = OtpCandidateStore::default();
        s.offer("123456".into(), t);
        assert_eq!(s.current(t + Duration::from_secs(119)), Some("123456"));
        assert_eq!(s.current(t + Duration::from_secs(120)), None);
    }
    #[test]
    fn new_code_replaces_and_clear_removes() {
        let t = Instant::now();
        let mut s = OtpCandidateStore::default();
        s.offer("111111".into(), t);
        s.offer("222222".into(), t);
        assert_eq!(s.current(t), Some("222222"));
        s.clear();
        assert_eq!(s.current(t), None);
    }
}
