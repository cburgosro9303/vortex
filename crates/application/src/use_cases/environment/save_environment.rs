//! Save environment use case

use std::path::Path;

use vortex_domain::environment::Environment;

use crate::ports::{EnvironmentError, EnvironmentRepository};

/// Errors that can occur when saving an environment.
#[derive(Debug, thiserror::Error)]
pub enum SaveEnvironmentError {
    /// Failed to write environment file.
    #[error("Failed to write environment file: {0}")]
    IoError(String),

    /// Failed to serialize environment.
    #[error("Failed to serialize environment: {0}")]
    SerializeError(String),
}

impl From<EnvironmentError> for SaveEnvironmentError {
    fn from(error: EnvironmentError) -> Self {
        match error {
            EnvironmentError::Io(e) => Self::IoError(e.to_string()),
            EnvironmentError::Serialization(e) => Self::SerializeError(e),
            EnvironmentError::NotFound(e) | EnvironmentError::Invalid(e) => Self::IoError(e),
        }
    }
}

/// Saves an environment to disk.
pub struct SaveEnvironment<R> {
    repository: R,
}

impl<R: EnvironmentRepository> SaveEnvironment<R> {
    /// Creates a new `SaveEnvironment` use case.
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    /// Executes the use case.
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    /// * `environment` - The environment to save
    ///
    /// # Errors
    /// Returns an error if the environment cannot be saved.
    pub async fn execute(
        &self,
        workspace: &Path,
        environment: &Environment,
    ) -> Result<(), SaveEnvironmentError> {
        self.repository.save(workspace, environment).await?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::significant_drop_tightening
)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct MockRepository {
        environments: Arc<Mutex<HashMap<String, Environment>>>,
    }

    impl MockRepository {
        fn new() -> Self {
            Self {
                environments: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        fn get(&self, name: &str) -> Option<Environment> {
            let envs = self.environments.lock().expect("Lock poisoned");
            envs.get(&name.to_lowercase()).cloned()
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
    async fn test_save_environment_success() {
        let repo = MockRepository::new();
        let use_case = SaveEnvironment::new(repo.clone());

        let mut env = Environment::new("Production");
        env.add_variable("api_url", "https://api.example.com");

        let result = use_case.execute(&PathBuf::from("/test"), &env).await;

        assert!(result.is_ok());

        // Verify it was saved
        let saved = repo.get("production");
        assert!(saved.is_some());
        assert_eq!(saved.expect("Should exist").name, "Production");
    }

    #[tokio::test]
    async fn test_save_environment_overwrites() {
        let repo = MockRepository::new();
        let use_case = SaveEnvironment::new(repo.clone());

        let mut env1 = Environment::new("Test");
        env1.add_variable("key", "value1");
        use_case
            .execute(&PathBuf::from("/test"), &env1)
            .await
            .expect("Should save");

        let mut env2 = Environment::new("Test");
        env2.add_variable("key", "value2");
        use_case
            .execute(&PathBuf::from("/test"), &env2)
            .await
            .expect("Should save");

        let saved = repo.get("test").expect("Should exist");
        assert_eq!(saved.resolve("key"), Some("value2"));
    }
}
