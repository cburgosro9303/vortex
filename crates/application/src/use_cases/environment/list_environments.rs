//! List environments use case

use std::path::Path;

use crate::ports::{EnvironmentError, EnvironmentRepository};

/// Output containing the list of environments.
pub struct ListEnvironmentsOutput {
    /// Environment names available in the workspace.
    pub environments: Vec<String>,
}

/// Lists all environments in a workspace.
pub struct ListEnvironments<R> {
    repository: R,
}

impl<R: EnvironmentRepository> ListEnvironments<R> {
    /// Creates a new `ListEnvironments` use case.
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    /// Executes the use case.
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    ///
    /// # Returns
    /// A list of environment names.
    #[allow(clippy::missing_errors_doc)]
    pub async fn execute(
        &self,
        workspace: &Path,
    ) -> Result<ListEnvironmentsOutput, EnvironmentError> {
        let environments = self.repository.list(workspace).await?;
        Ok(ListEnvironmentsOutput { environments })
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
    use std::sync::Mutex;
    use vortex_domain::environment::Environment;

    struct MockRepository {
        environments: Mutex<HashMap<String, Environment>>,
    }

    impl MockRepository {
        fn new() -> Self {
            Self {
                environments: Mutex::new(HashMap::new()),
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
            let mut names: Vec<String> = envs.keys().cloned().collect();
            names.sort();
            Ok(names)
        }

        async fn delete(&self, _: &Path, name: &str) -> Result<(), EnvironmentError> {
            let mut envs = self.environments.lock().expect("Lock poisoned");
            envs.remove(&name.to_lowercase())
                .map(|_| ())
                .ok_or_else(|| EnvironmentError::NotFound(name.to_string()))
        }
    }

    #[tokio::test]
    async fn test_list_environments_empty() {
        let repo = MockRepository::new();
        let use_case = ListEnvironments::new(repo);

        let result = use_case.execute(&PathBuf::from("/test")).await;
        assert!(result.is_ok());

        let output = result.expect("Should succeed");
        assert!(output.environments.is_empty());
    }

    #[tokio::test]
    async fn test_list_environments_with_entries() {
        let repo = MockRepository::new();
        repo.add(Environment::new("Development"));
        repo.add(Environment::new("Production"));
        repo.add(Environment::new("Staging"));

        let use_case = ListEnvironments::new(repo);
        let result = use_case.execute(&PathBuf::from("/test")).await;

        assert!(result.is_ok());
        let output = result.expect("Should succeed");
        assert_eq!(output.environments.len(), 3);
        // Should be sorted
        assert!(output.environments.contains(&"development".to_string()));
        assert!(output.environments.contains(&"production".to_string()));
        assert!(output.environments.contains(&"staging".to_string()));
    }
}
