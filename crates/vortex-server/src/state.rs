//! Application state.

use std::sync::Arc;

use vortex_git::{ConfigSource, GitBackend};

use crate::cache::ConfigCache;

/// Application state shared across all handlers.
#[derive(Clone)]
pub struct AppState {
    /// The configuration source (Git backend).
    config_source: Arc<dyn ConfigSource>,
    /// Cache layer for configurations.
    cache: Option<ConfigCache>,
}

impl AppState {
    /// Creates a new AppState with the given config source and optional cache.
    pub fn new(config_source: Arc<dyn ConfigSource>, cache: Option<ConfigCache>) -> Self {
        Self {
            config_source,
            cache,
        }
    }

    /// Creates an AppState from a GitBackend with optional cache.
    pub fn from_git_backend(backend: GitBackend, cache: Option<ConfigCache>) -> Self {
        Self {
            config_source: Arc::new(backend),
            cache,
        }
    }

    /// Creates an AppState without cache (for testing).
    pub fn without_cache(config_source: Arc<dyn ConfigSource>) -> Self {
        Self {
            config_source,
            cache: None,
        }
    }

    /// Returns a reference to the config source.
    pub fn config_source(&self) -> &dyn ConfigSource {
        self.config_source.as_ref()
    }

    /// Returns a reference to the cache if enabled.
    pub fn cache(&self) -> Option<&ConfigCache> {
        self.cache.as_ref()
    }

    /// Returns whether cache is enabled.
    pub fn is_cache_enabled(&self) -> bool {
        self.cache.is_some()
    }
}
