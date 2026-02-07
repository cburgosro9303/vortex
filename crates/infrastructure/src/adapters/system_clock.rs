//! System clock adapter

use chrono::{DateTime, Utc};
use vortex_application::ports::Clock;

/// System clock implementation using the system time.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl SystemClock {
    /// Creates a new system clock.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_clock() {
        let clock = SystemClock::new();
        let now = clock.now();
        // Just verify it returns a reasonable timestamp
        assert!(now.timestamp() > 0);
    }
}
