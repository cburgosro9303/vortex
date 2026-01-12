//! Background synchronization and refresh scheduling.
//!
//! This module provides functionality for automatically refreshing
//! Git repositories on a configurable schedule.

mod scheduler;
mod state;

pub use scheduler::{RefreshConfig, RefreshHandle, RefreshScheduler};
pub use state::GitState;
