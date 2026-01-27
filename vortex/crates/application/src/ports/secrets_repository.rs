//! Secrets repository port
//!
//! Defines the interface for secrets persistence.

use async_trait::async_trait;
use std::path::Path;

use vortex_domain::environment::SecretsStore;

/// Errors that can occur during secrets operations.
#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Repository trait for secrets persistence.
#[async_trait]
pub trait SecretsRepository: Send + Sync {
    /// Loads the secrets store from the workspace.
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    ///
    /// # Returns
    /// The secrets store. Returns an empty store if the file doesn't exist.
    async fn load(&self, workspace: &Path) -> Result<SecretsStore, SecretsError>;

    /// Saves the secrets store to the workspace.
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    /// * `secrets` - The secrets store to save
    ///
    /// # Errors
    /// Returns an error if the file cannot be written.
    async fn save(&self, workspace: &Path, secrets: &SecretsStore) -> Result<(), SecretsError>;

    /// Gets a single secret value.
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    /// * `environment` - Environment name
    /// * `name` - Secret variable name
    async fn get_secret(
        &self,
        workspace: &Path,
        environment: &str,
        name: &str,
    ) -> Result<Option<String>, SecretsError> {
        let store = self.load(workspace).await?;
        Ok(store.get_secret(environment, name).map(String::from))
    }

    /// Sets a single secret value.
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    /// * `environment` - Environment name
    /// * `name` - Secret variable name
    /// * `value` - Secret value
    async fn set_secret(
        &self,
        workspace: &Path,
        environment: &str,
        name: &str,
        value: &str,
    ) -> Result<(), SecretsError> {
        let mut store = self.load(workspace).await?;
        store.set_secret(environment, name, value);
        self.save(workspace, &store).await
    }

    /// Removes a single secret.
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    /// * `environment` - Environment name
    /// * `name` - Secret variable name to remove
    async fn remove_secret(
        &self,
        workspace: &Path,
        environment: &str,
        name: &str,
    ) -> Result<Option<String>, SecretsError> {
        let mut store = self.load(workspace).await?;
        let removed = store.remove_secret(environment, name);
        self.save(workspace, &store).await?;
        Ok(removed)
    }

    /// Removes all secrets for an environment.
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    /// * `environment` - Environment name
    async fn remove_environment_secrets(
        &self,
        workspace: &Path,
        environment: &str,
    ) -> Result<(), SecretsError> {
        let mut store = self.load(workspace).await?;
        store.remove_environment(environment);
        self.save(workspace, &store).await
    }
}
