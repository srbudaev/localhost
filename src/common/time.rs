use std::time::{Duration, Instant};

pub struct Timeout {
    deadline: Instant,
}

impl Timeout {
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            deadline: Instant::now() + Duration::from_secs(timeout_secs),
        }
    }

    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.deadline
    }

    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
}

