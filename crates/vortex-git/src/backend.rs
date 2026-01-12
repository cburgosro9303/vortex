//! Git backend implementation.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, info};

use crate::error::ConfigSourceError;
use crate::reader::ConfigFileResolver;
use crate::repository::{GitBackendConfig, GitRef, GitRepository};
use crate::source::{ConfigQuery, ConfigResult, ConfigSource};
use crate::sync::{GitState, RefreshConfig, RefreshHandle, RefreshScheduler};

/// A Git-based configuration source.
///
/// This backend clones a Git repository and reads configuration files
/// following Spring Cloud Config conventions.
pub struct GitBackend {
    /// The Git repository.
    repository: Arc<GitRepository>,
    /// The current state.
    state: Arc<GitState>,
    /// The backend configuration.
    config: GitBackendConfig,
    /// The file resolver.
    resolver: ConfigFileResolver,
    /// Optional refresh handle.
    refresh_handle: Option<RefreshHandle>,
}

impl GitBackend {
    /// Creates a new Git backend.
    ///
    /// This will clone the repository if it doesn't exist locally.
    pub async fn new(config: GitBackendConfig) -> Result<Self, ConfigSourceError> {
        let repository = Arc::new(GitRepository::new(config.clone()));
        let state = Arc::new(GitState::new());

        // Ensure repository is cloned
        repository.ensure_cloned().await?;

        // Checkout default branch
        let default_ref = GitRef::branch(config.default_label());
        let commit = repository.checkout(&default_ref).await?;
        state.record_success(&commit);

        let resolver =
            ConfigFileResolver::new(config.local_path().clone(), config.search_paths().to_vec());

        info!(
            "Git backend initialized: {} at commit {}",
            config.uri(),
            &commit[..8]
        );

        Ok(Self {
            repository,
            state,
            config,
            resolver,
            refresh_handle: None,
        })
    }

    /// Creates a new Git backend with auto-refresh enabled.
    pub async fn with_auto_refresh(
        config: GitBackendConfig,
        refresh_config: RefreshConfig,
    ) -> Result<Self, ConfigSourceError> {
        let mut backend = Self::new(config).await?;

        let scheduler = RefreshScheduler::new(
            Arc::clone(&backend.repository),
            Arc::clone(&backend.state),
            refresh_config,
        );

        backend.refresh_handle = Some(scheduler.start());

        Ok(backend)
    }

    /// Returns the current commit SHA.
    pub fn current_commit(&self) -> Option<String> {
        self.state.commit()
    }

    /// Returns the repository state.
    pub fn state(&self) -> &GitState {
        &self.state
    }

    /// Returns the configuration.
    pub fn config(&self) -> &GitBackendConfig {
        &self.config
    }

    /// Stops auto-refresh if enabled.
    pub fn stop_auto_refresh(&mut self) {
        if let Some(handle) = self.refresh_handle.take() {
            handle.stop();
        }
    }
}

#[async_trait]
impl ConfigSource for GitBackend {
    async fn fetch(&self, query: &ConfigQuery) -> Result<ConfigResult, ConfigSourceError> {
        // Determine the label to use
        let label = query.effective_label(self.config.default_label());
        let git_ref = GitRef::parse(label);

        debug!("Fetching config for {} with label {}", query, label);

        // Checkout the requested reference
        let commit = self.repository.checkout(&git_ref).await?;

        // Resolve configuration files
        let sources = self.resolver.resolve(query, label)?;

        // Build result
        let mut result = ConfigResult::new(query.application(), query.profiles().to_vec(), label);
        result.set_version(&commit);
        result.add_property_sources(sources);

        debug!("Resolved {} property sources for {}", result.len(), query);

        Ok(result)
    }

    async fn health_check(&self) -> Result<(), ConfigSourceError> {
        if !self.state.is_healthy()
            && let Some(error) = self.state.last_error()
        {
            return Err(ConfigSourceError::unavailable(error));
        }

        // Verify repository is accessible
        self.repository.head_commit().await?;

        Ok(())
    }

    fn name(&self) -> &str {
        "git"
    }

    async fn refresh(&self) -> Result<(), ConfigSourceError> {
        info!("Manual refresh requested");

        // Fetch latest changes
        self.repository.fetch().await?;

        // Get and record new commit
        let commit = self.repository.head_commit().await?;
        self.state.record_success(&commit);

        info!("Refresh complete, now at commit {}", &commit[..8]);

        Ok(())
    }

    fn supports_refresh(&self) -> bool {
        true
    }
}

impl Drop for GitBackend {
    fn drop(&mut self) {
        self.stop_auto_refresh();
    }
}

impl std::fmt::Debug for GitBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitBackend")
            .field("uri", &self.config.uri())
            .field("local_path", &self.config.local_path())
            .field("current_commit", &self.current_commit())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    // Integration tests would go here
    // They require actual Git repositories to test properly
}
