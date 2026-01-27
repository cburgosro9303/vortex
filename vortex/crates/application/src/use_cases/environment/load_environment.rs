//! Load environment use case

use std::path::Path;

use vortex_domain::environment::Environment;

use crate::ports::{EnvironmentError, EnvironmentRepository};

/// Errors that can occur when loading an environment.
#[derive(Debug, thiserror::Error)]
pub enum LoadEnvironmentError {
    /// Environment not found.
    #[error("Environment not found: {0}")]
    NotFound(String),

    /// Failed to read environment file.
    #[error("Failed to read environment file: {0}")]
    IoError(String),

    /// Failed to parse environment file.
    #[error("Failed to parse environment file: {0}")]
    ParseError(String),
}

impl From<EnvironmentError> for LoadEnvironmentError {
    fn from(error: EnvironmentError) -> Self {
        match error {
            EnvironmentError::NotFound(name) => Self::NotFound(name),
            EnvironmentError::Io(e) => Self::IoError(e.to_string()),
            EnvironmentError::Serialization(e) => Self::ParseError(e),
            EnvironmentError::Invalid(e) => Self::ParseError(e),
        }
    }
}

/// Output containing the loaded environment.
pub struct LoadEnvironmentOutput {
    /// The loaded environment.
    pub environment: Environment,
}

/// Loads an environment from disk.
pub struct LoadEnvironment<R> {
    repository: R,
}

impl<R: EnvironmentRepository> LoadEnvironment<R> {
    /// Creates a new `LoadEnvironment` use case.
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    /// Executes the use case.
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    /// * `name` - Name of the environment to load (without .json extension)
    ///
    /// # Errors
    /// Returns an error if the environment cannot be loaded.
    pub async fn execute(
        &self,
        workspace: &Path,
        name: &str,
    ) -> Result<LoadEnvironmentOutput, LoadEnvironmentError> {
        let environment = self.repository.load(workspace, name).await?;

        Ok(LoadEnvironmentOutput { environment })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::path::PathBuf;
    use std::sync::Mutex;

    struct MockRepository {
        environments: Mutex<std::collections::HashMap<String, Environment>>,
    }

    impl MockRepository {
        fn new() -> Self {
            Self {
                environments: Mutex::new(std::collections::HashMap::new()),
            }
        }

        fn add(&self, env: Environment) {
            let mut envs = self.environments.lock().expect("Lock poisoned");
            envs.insert(env.name.to_lowercase(), env);
        }
    }

    #[async_trait]
    impl EnvironmentRepository for MockRepository {
        async fn load(&self, _: &Path, name: &str) -> Result<Environment, EnvironmentError> {
            let envs = self.environments.lock().expect("Lock poisoned");
            envs.get(&name.to_lowercase())
                .cloned()
                .ok_or_else(|| EnvironmentError::NotFound(name.to_string()))
        }

        async fn save(&self, _: &Path, env: &Environment) -> Result<(), EnvironmentError> {
            let mut envs = self.environments.lock().expect("Lock poisoned");
            envs.insert(env.name.to_lowercase(), env.clone());
            Ok(())
        }

        async fn list(&self, _: &Path) -> Result<Vec<String>, EnvironmentError> {
            let envs = self.environments.lock().expect("Lock poisoned");
            Ok(envs.keys().cloned().collect())
        }

        async fn delete(&self, _: &Path, name: &str) -> Result<(), EnvironmentError> {
            let mut envs = self.environments.lock().expect("Lock poisoned");
            envs.remove(&name.to_lowercase())
                .map(|_| ())
                .ok_or_else(|| EnvironmentError::NotFound(name.to_string()))
        }
    }

    #[tokio::test]
    async fn test_load_environment_success() {
        let repo = MockRepository::new();
        let mut env = Environment::new("Development");
        env.add_variable("host", "localhost");
        repo.add(env);

        let use_case = LoadEnvironment::new(repo);
        let result = use_case
            .execute(&PathBuf::from("/test"), "development")
            .await;

        assert!(result.is_ok());
        let output = result.expect("Should succeed");
        assert_eq!(output.environment.name, "Development");
    }

    #[tokio::test]
    async fn test_load_environment_not_found() {
        let repo = MockRepository::new();
        let use_case = LoadEnvironment::new(repo);

        let result = use_case
            .execute(&PathBuf::from("/test"), "nonexistent")
            .await;

        assert!(result.is_err());
        assert!(matches!(result, Err(LoadEnvironmentError::NotFound(_))));
    }
}
