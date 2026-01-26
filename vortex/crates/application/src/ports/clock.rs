//! Clock port for time-related operations

use chrono::{DateTime, Utc};

/// Port for getting the current time.
///
/// This abstraction allows testing time-dependent code by providing
/// a mock implementation.
pub trait Clock: Send + Sync {
    /// Returns the current UTC timestamp.
    fn now(&self) -> DateTime<Utc>;
}
