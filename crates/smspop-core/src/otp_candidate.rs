use std::time::{Duration, Instant, SystemTime};
#[derive(Default)]
pub struct OtpCandidateStore {
    candidate: Option<(String, Instant, SystemTime)>,
}
impl OtpCandidateStore {
    pub fn offer(&mut self, code: String, received_at: Instant) {
        self.offer_at(code, received_at, SystemTime::now());
    }
    pub fn offer_at(&mut self, code: String, received_at: Instant, wall: SystemTime) {
        self.candidate = Some((code, received_at, wall));
    }
    pub fn current(&mut self, now: Instant) -> Option<&str> {
        self.current_at(now, SystemTime::now())
    }
    pub fn current_at(&mut self, now: Instant, wall: SystemTime) -> Option<&str> {
        if self.candidate.as_ref().is_some_and(|(_, t, w)| {
            now.saturating_duration_since(*t) >= Duration::from_secs(120)
                || wall
                    .duration_since(*w)
                    .map_or(true, |elapsed| elapsed >= Duration::from_secs(120))
        }) {
            self.clear();
        }
        self.candidate.as_ref().map(|(code, _, _)| code.as_str())
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

#[cfg(test)]
mod suspend_tests {
    use super::*;
    use std::time::SystemTime;
    #[test]
    fn sleep_time_expires_candidate_even_when_instant_pauses() {
        let t = Instant::now();
        let wall = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        let mut s = OtpCandidateStore::default();
        s.offer_at("123456".into(), t, wall);
        assert_eq!(
            s.current_at(t + Duration::from_secs(1), wall + Duration::from_secs(3600)),
            None
        );
    }
    #[test]
    fn clock_rollback_invalidates_candidate() {
        let t = Instant::now();
        let wall = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        let mut s = OtpCandidateStore::default();
        s.offer_at("123456".into(), t, wall);
        assert_eq!(
            s.current_at(t + Duration::from_secs(1), wall - Duration::from_secs(1)),
            None
        );
    }
}
