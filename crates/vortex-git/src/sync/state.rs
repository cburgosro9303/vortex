//! Git repository state tracking.

use std::time::Instant;

use parking_lot::RwLock;

/// Tracks the state of a Git repository for synchronization purposes.
#[derive(Debug)]
pub struct GitState {
    /// The current commit SHA.
    commit: RwLock<Option<String>>,
    /// The last successful refresh time.
    last_refresh: RwLock<Option<Instant>>,
    /// The last error message, if any.
    last_error: RwLock<Option<String>>,
    /// Number of consecutive failures.
    failure_count: RwLock<u32>,
}

impl GitState {
    /// Creates a new GitState.
    pub fn new() -> Self {
        Self {
            commit: RwLock::new(None),
            last_refresh: RwLock::new(None),
            last_error: RwLock::new(None),
            failure_count: RwLock::new(0),
        }
    }

    /// Returns the current commit SHA.
    pub fn commit(&self) -> Option<String> {
        self.commit.read().clone()
    }

    /// Sets the current commit SHA.
    pub fn set_commit(&self, sha: impl Into<String>) {
        let mut commit = self.commit.write();
        *commit = Some(sha.into());
    }

    /// Returns the time since the last successful refresh.
    pub fn last_refresh(&self) -> Option<Instant> {
        *self.last_refresh.read()
    }

    /// Returns the duration since the last refresh.
    pub fn time_since_refresh(&self) -> Option<std::time::Duration> {
        self.last_refresh.read().map(|t| t.elapsed())
    }

    /// Records a successful refresh.
    pub fn record_success(&self, commit: impl Into<String>) {
        let mut commit_lock = self.commit.write();
        let mut last_refresh = self.last_refresh.write();
        let mut last_error = self.last_error.write();
        let mut failure_count = self.failure_count.write();

        *commit_lock = Some(commit.into());
        *last_refresh = Some(Instant::now());
        *last_error = None;
        *failure_count = 0;
    }

    /// Records a failed refresh.
    pub fn record_failure(&self, error: impl Into<String>) {
        let mut last_error = self.last_error.write();
        let mut failure_count = self.failure_count.write();

        *last_error = Some(error.into());
        *failure_count += 1;
    }

    /// Returns the last error message.
    pub fn last_error(&self) -> Option<String> {
        self.last_error.read().clone()
    }

    /// Returns the number of consecutive failures.
    pub fn failure_count(&self) -> u32 {
        *self.failure_count.read()
    }

    /// Returns true if the repository has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.commit.read().is_some()
    }

    /// Returns true if the repository is in a healthy state.
    pub fn is_healthy(&self) -> bool {
        self.is_initialized() && self.last_error.read().is_none()
    }

    /// Returns true if refresh is needed based on the given interval.
    pub fn needs_refresh(&self, interval: std::time::Duration) -> bool {
        match self.time_since_refresh() {
            Some(elapsed) => elapsed >= interval,
            None => true,
        }
    }

    /// Resets all state.
    pub fn reset(&self) {
        let mut commit = self.commit.write();
        let mut last_refresh = self.last_refresh.write();
        let mut last_error = self.last_error.write();
        let mut failure_count = self.failure_count.write();

        *commit = None;
        *last_refresh = None;
        *last_error = None;
        *failure_count = 0;
    }
}

impl Default for GitState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_new_state() {
        let state = GitState::new();
        assert!(state.commit().is_none());
        assert!(state.last_refresh().is_none());
        assert!(!state.is_initialized());
        assert!(!state.is_healthy());
    }

    #[test]
    fn test_record_success() {
        let state = GitState::new();
        state.record_success("abc123");

        assert_eq!(state.commit(), Some("abc123".to_string()));
        assert!(state.last_refresh().is_some());
        assert!(state.is_initialized());
        assert!(state.is_healthy());
        assert_eq!(state.failure_count(), 0);
    }

    #[test]
    fn test_record_failure() {
        let state = GitState::new();
        state.record_failure("network error");
        state.record_failure("timeout");

        assert_eq!(state.failure_count(), 2);
        assert_eq!(state.last_error(), Some("timeout".to_string()));
        assert!(!state.is_healthy());
    }

    #[test]
    fn test_success_resets_failure() {
        let state = GitState::new();
        state.record_failure("error 1");
        state.record_failure("error 2");
        assert_eq!(state.failure_count(), 2);

        state.record_success("abc123");
        assert_eq!(state.failure_count(), 0);
        assert!(state.last_error().is_none());
    }

    #[test]
    fn test_needs_refresh() {
        let state = GitState::new();

        // Needs refresh when never refreshed
        assert!(state.needs_refresh(Duration::from_secs(60)));

        // After success, doesn't need immediate refresh
        state.record_success("abc123");
        assert!(!state.needs_refresh(Duration::from_secs(60)));

        // Would need refresh after interval passes (can't easily test without sleep)
    }

    #[test]
    fn test_reset() {
        let state = GitState::new();
        state.record_success("abc123");
        state.record_failure("error");

        state.reset();

        assert!(state.commit().is_none());
        assert!(state.last_refresh().is_none());
        assert!(state.last_error().is_none());
        assert_eq!(state.failure_count(), 0);
    }
}
