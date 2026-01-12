//! Application state.

use std::sync::Arc;

use vortex_git::{ConfigSource, GitBackend};

/// Application state shared across all handlers.
#[derive(Clone)]
pub struct AppState {
    /// The configuration source (Git backend).
    config_source: Arc<dyn ConfigSource>,
}

impl AppState {
    /// Creates a new AppState with the given config source.
    pub fn new(config_source: Arc<dyn ConfigSource>) -> Self {
        Self { config_source }
    }

    /// Creates an AppState from a GitBackend.
    pub fn from_git_backend(backend: GitBackend) -> Self {
        Self {
            config_source: Arc::new(backend),
        }
    }

    /// Returns a reference to the config source.
    pub fn config_source(&self) -> &dyn ConfigSource {
        self.config_source.as_ref()
    }
}
