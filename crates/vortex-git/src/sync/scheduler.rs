//! Background refresh scheduler.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::watch;
use tokio::time::interval;
use tracing::{debug, info, warn};

use super::GitState;
use crate::error::ConfigSourceError;
use crate::repository::GitRepository;

/// Configuration for the refresh scheduler.
#[derive(Debug, Clone)]
pub struct RefreshConfig {
    /// Interval between refresh attempts.
    pub interval: Duration,
    /// Maximum number of consecutive failures before backing off.
    pub max_failures: u32,
    /// Backoff multiplier for failures.
    pub backoff_multiplier: f64,
    /// Maximum backoff duration.
    pub max_backoff: Duration,
}

impl Default for RefreshConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            max_failures: 3,
            backoff_multiplier: 2.0,
            max_backoff: Duration::from_secs(300),
        }
    }
}

/// Handle for controlling a running refresh scheduler.
pub struct RefreshHandle {
    /// Sender to signal shutdown.
    shutdown_tx: watch::Sender<bool>,
}

impl RefreshHandle {
    /// Signals the scheduler to stop.
    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(true);
    }

}

impl Drop for RefreshHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Background scheduler for refreshing Git repositories.
pub struct RefreshScheduler {
    /// The repository to refresh.
    repository: Arc<GitRepository>,
    /// The current state.
    state: Arc<GitState>,
    /// Configuration.
    config: RefreshConfig,
    /// Current backoff duration.
    current_backoff: Arc<Mutex<Duration>>,
}

impl RefreshScheduler {
    /// Creates a new refresh scheduler.
    pub fn new(
        repository: Arc<GitRepository>,
        state: Arc<GitState>,
        config: RefreshConfig,
    ) -> Self {
        Self {
            repository,
            state,
            current_backoff: Arc::new(Mutex::new(config.interval)),
            config,
        }
    }

    /// Creates a scheduler with default configuration.
    pub fn with_defaults(repository: Arc<GitRepository>, state: Arc<GitState>) -> Self {
        Self::new(repository, state, RefreshConfig::default())
    }

    /// Starts the background refresh task.
    ///
    /// Returns a handle that can be used to stop the scheduler.
    pub fn start(self) -> RefreshHandle {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let handle = RefreshHandle { shutdown_tx };

        tokio::spawn(self.run(shutdown_rx));

        handle
    }

    /// Runs the scheduler loop.
    async fn run(self, mut shutdown_rx: watch::Receiver<bool>) {
        let initial_interval = self.config.interval;
        let mut interval_timer = interval(initial_interval);

        info!(
            "Starting refresh scheduler with interval {:?}",
            initial_interval
        );

        loop {
            tokio::select! {
                _ = interval_timer.tick() => {
                    self.do_refresh().await;

                    // Adjust interval based on current backoff
                    let current = *self.current_backoff.lock();
                    if current != interval_timer.period() {
                        interval_timer = interval(current);
                    }
                }
                result = shutdown_rx.changed() => {
                    if result.is_err() || *shutdown_rx.borrow() {
                        info!("Refresh scheduler shutting down");
                        break;
                    }
                }
            }
        }
    }

    /// Performs a single refresh operation.
    async fn do_refresh(&self) {
        debug!("Starting scheduled refresh");

        match self.refresh_repository().await {
            Ok(commit) => {
                self.state.record_success(&commit);
                self.reset_backoff();
                debug!("Refresh successful, commit: {}", commit);
            }
            Err(e) => {
                self.state.record_failure(e.to_string());
                self.increase_backoff();
                warn!("Refresh failed: {}", e);
            }
        }
    }

    /// Refreshes the repository and returns the current commit.
    async fn refresh_repository(&self) -> Result<String, ConfigSourceError> {
        // Fetch latest changes
        self.repository.fetch().await?;

        // Get current commit
        self.repository.head_commit().await
    }

    /// Resets the backoff to the base interval.
    fn reset_backoff(&self) {
        let mut backoff = self.current_backoff.lock();
        *backoff = self.config.interval;
    }

    /// Increases the backoff duration after a failure.
    fn increase_backoff(&self) {
        let mut backoff = self.current_backoff.lock();
        let failure_count = self.state.failure_count();

        if failure_count >= self.config.max_failures {
            let new_backoff = Duration::from_secs_f64(
                backoff.as_secs_f64() * self.config.backoff_multiplier,
            );
            *backoff = new_backoff.min(self.config.max_backoff);

            debug!(
                "Increased backoff to {:?} after {} failures",
                *backoff, failure_count
            );
        }
    }

    /// Manually triggers a refresh.
    pub async fn trigger_refresh(&self) -> Result<String, ConfigSourceError> {
        info!("Manual refresh triggered");
        let result = self.refresh_repository().await;

        match &result {
            Ok(commit) => {
                self.state.record_success(commit);
                self.reset_backoff();
            }
            Err(e) => {
                self.state.record_failure(e.to_string());
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refresh_config_default() {
        let config = RefreshConfig::default();
        assert_eq!(config.interval, Duration::from_secs(30));
        assert_eq!(config.max_failures, 3);
        assert_eq!(config.backoff_multiplier, 2.0);
        assert_eq!(config.max_backoff, Duration::from_secs(300));
    }

    #[test]
    fn test_refresh_handle_stop() {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let handle = RefreshHandle { shutdown_tx };

        assert!(!*shutdown_rx.borrow());
        handle.stop();
        assert!(shutdown_rx.has_changed().unwrap_or(false) || *shutdown_rx.borrow());
    }
}
