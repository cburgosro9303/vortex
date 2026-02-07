//! Environment repository port
//!
//! Defines the interface for environment persistence.

use async_trait::async_trait;
use std::path::Path;

use vortex_domain::environment::Environment;

/// Errors that can occur during environment operations.
#[derive(Debug, thiserror::Error)]
pub enum EnvironmentError {
    /// Environment not found.
    #[error("Environment not found: {0}")]
    NotFound(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Invalid environment data.
    #[error("Invalid environment: {0}")]
    Invalid(String),
}

/// Repository trait for environment persistence.
#[async_trait]
pub trait EnvironmentRepository: Send + Sync {
    /// Loads an environment by name from the workspace.
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    /// * `name` - Environment name (without .json extension)
    ///
    /// # Errors
    /// Returns `EnvironmentError::NotFound` if the environment doesn't exist.
    async fn load(&self, workspace: &Path, name: &str) -> Result<Environment, EnvironmentError>;

    /// Saves an environment to the workspace.
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    /// * `environment` - The environment to save
    ///
    /// # Errors
    /// Returns an error if the file cannot be written.
    async fn save(
        &self,
        workspace: &Path,
        environment: &Environment,
    ) -> Result<(), EnvironmentError>;

    /// Lists all available environment names in the workspace.
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    ///
    /// # Returns
    /// A vector of environment names (without .json extension).
    async fn list(&self, workspace: &Path) -> Result<Vec<String>, EnvironmentError>;

    /// Deletes an environment from the workspace.
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    /// * `name` - Environment name to delete
    ///
    /// # Errors
    /// Returns `EnvironmentError::NotFound` if the environment doesn't exist.
    async fn delete(&self, workspace: &Path, name: &str) -> Result<(), EnvironmentError>;

    /// Checks if an environment exists.
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    /// * `name` - Environment name to check
    async fn exists(&self, workspace: &Path, name: &str) -> Result<bool, EnvironmentError> {
        match self.load(workspace, name).await {
            Ok(_) => Ok(true),
            Err(EnvironmentError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Creates a new environment with default settings.
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    /// * `name` - Name for the new environment
    ///
    /// # Errors
    /// Returns an error if an environment with the same name already exists.
    async fn create(&self, workspace: &Path, name: &str) -> Result<Environment, EnvironmentError> {
        if self.exists(workspace, name).await? {
            return Err(EnvironmentError::Invalid(format!(
                "Environment '{name}' already exists"
            )));
        }

        let environment = Environment::new(name);
        self.save(workspace, &environment).await?;
        Ok(environment)
    }

    /// Renames an environment.
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    /// * `old_name` - Current environment name
    /// * `new_name` - New environment name
    async fn rename(
        &self,
        workspace: &Path,
        old_name: &str,
        new_name: &str,
    ) -> Result<Environment, EnvironmentError> {
        if self.exists(workspace, new_name).await? {
            return Err(EnvironmentError::Invalid(format!(
                "Environment '{new_name}' already exists"
            )));
        }

        let mut environment = self.load(workspace, old_name).await?;
        environment.name = new_name.to_string();
        self.save(workspace, &environment).await?;
        self.delete(workspace, old_name).await?;
        Ok(environment)
    }
}
