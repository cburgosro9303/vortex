//! Switch environment use case

use std::path::Path;

use vortex_domain::environment::{Environment, Globals, ResolutionContext, VariableMap};

use crate::ports::{EnvironmentError, EnvironmentRepository, SecretsError, SecretsRepository};

/// Errors that can occur when switching environments.
#[derive(Debug, thiserror::Error)]
pub enum SwitchEnvironmentError {
    /// Environment not found.
    #[error("Environment not found: {0}")]
    NotFound(String),

    /// Failed to load environment.
    #[error("Failed to load environment: {0}")]
    LoadError(String),

    /// Failed to load secrets.
    #[error("Failed to load secrets: {0}")]
    SecretsError(String),
}

impl From<EnvironmentError> for SwitchEnvironmentError {
    fn from(error: EnvironmentError) -> Self {
        match error {
            EnvironmentError::NotFound(name) => Self::NotFound(name),
            _ => Self::LoadError(error.to_string()),
        }
    }
}

impl From<SecretsError> for SwitchEnvironmentError {
    fn from(error: SecretsError) -> Self {
        Self::SecretsError(error.to_string())
    }
}

/// Output containing the new resolution context.
pub struct SwitchEnvironmentOutput {
    /// The loaded environment.
    pub environment: Environment,
    /// The new resolution context ready for use.
    pub resolution_context: ResolutionContext,
}

/// Switches the active environment and creates a new resolution context.
pub struct SwitchEnvironment<E, S> {
    environment_repo: E,
    secrets_repo: S,
}

impl<E: EnvironmentRepository, S: SecretsRepository> SwitchEnvironment<E, S> {
    /// Creates a new `SwitchEnvironment` use case.
    pub const fn new(environment_repo: E, secrets_repo: S) -> Self {
        Self {
            environment_repo,
            secrets_repo,
        }
    }

    /// Executes the use case.
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    /// * `environment_name` - Name of the environment to switch to
    /// * `globals` - Current globals
    /// * `collection_variables` - Current collection variables
    ///
    /// # Errors
    /// Returns an error if the environment cannot be loaded.
    pub async fn execute(
        &self,
        workspace: &Path,
        environment_name: &str,
        globals: &Globals,
        collection_variables: &VariableMap,
    ) -> Result<SwitchEnvironmentOutput, SwitchEnvironmentError> {
        // Load the environment
        let environment = self
            .environment_repo
            .load(workspace, environment_name)
            .await?;

        // Load secrets (don't fail if secrets file doesn't exist)
        let secrets = self.secrets_repo.load(workspace).await.unwrap_or_default();

        // Create the resolution context
        let resolution_context =
            ResolutionContext::from_sources(globals, collection_variables, &environment, &secrets);

        Ok(SwitchEnvironmentOutput {
            environment,
            resolution_context,
        })
    }

    /// Executes the use case with minimal context (just environment and secrets).
    ///
    /// # Arguments
    /// * `workspace` - Path to the workspace root
    /// * `environment_name` - Name of the environment to switch to
    #[allow(clippy::missing_errors_doc)]
    pub async fn execute_simple(
        &self,
        workspace: &Path,
        environment_name: &str,
    ) -> Result<SwitchEnvironmentOutput, SwitchEnvironmentError> {
        self.execute(
            workspace,
            environment_name,
            &Globals::new(),
            &VariableMap::new(),
        )
        .await
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::significant_drop_tightening)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use vortex_domain::environment::{SecretsStore, Variable};

    struct MockEnvRepository {
        environments: Mutex<HashMap<String, Environment>>,
    }

    impl MockEnvRepository {
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
    impl EnvironmentRepository for MockEnvRepository {
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

    struct MockSecretsRepository {
        secrets: Mutex<SecretsStore>,
    }

    impl MockSecretsRepository {
        fn new() -> Self {
            Self {
                secrets: Mutex::new(SecretsStore::new()),
            }
        }

        fn set(&self, environment: &str, name: &str, value: &str) {
            let mut store = self.secrets.lock().expect("Lock poisoned");
            store.set_secret(environment, name, value);
        }
    }

    #[async_trait]
    impl SecretsRepository for MockSecretsRepository {
        async fn load(&self, _: &Path) -> Result<SecretsStore, SecretsError> {
            let store = self.secrets.lock().expect("Lock poisoned");
            Ok(store.clone())
        }

        async fn save(&self, _: &Path, secrets: &SecretsStore) -> Result<(), SecretsError> {
            let mut store = self.secrets.lock().expect("Lock poisoned");
            *store = secrets.clone();
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_switch_environment_success() {
        let env_repo = MockEnvRepository::new();
        let secrets_repo = MockSecretsRepository::new();

        let mut env = Environment::new("Development");
        env.set_variable("base_url", Variable::new("http://localhost:3000"));
        env.set_variable("api_key", Variable::secret(""));
        env_repo.add(env);

        secrets_repo.set("development", "api_key", "sk-dev-123");

        let use_case = SwitchEnvironment::new(env_repo, secrets_repo);
        let result = use_case
            .execute_simple(&PathBuf::from("/test"), "development")
            .await;

        assert!(result.is_ok());
        let output = result.expect("Should succeed");
        assert_eq!(output.environment.name, "Development");

        // Verify resolution context works
        assert_eq!(
            output.resolution_context.resolve_value("base_url"),
            Some("http://localhost:3000".to_string())
        );
        assert_eq!(
            output.resolution_context.resolve_value("api_key"),
            Some("sk-dev-123".to_string())
        );
    }

    #[tokio::test]
    async fn test_switch_environment_not_found() {
        let env_repo = MockEnvRepository::new();
        let secrets_repo = MockSecretsRepository::new();

        let use_case = SwitchEnvironment::new(env_repo, secrets_repo);
        let result = use_case
            .execute_simple(&PathBuf::from("/test"), "nonexistent")
            .await;

        assert!(result.is_err());
        assert!(matches!(result, Err(SwitchEnvironmentError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_switch_environment_with_globals() {
        let env_repo = MockEnvRepository::new();
        let secrets_repo = MockSecretsRepository::new();

        let mut env = Environment::new("Production");
        env.set_variable("api_url", Variable::new("https://api.example.com"));
        env_repo.add(env);

        let mut globals = Globals::new();
        globals.add_variable("app_name", "MyApp");

        let mut collection = VariableMap::new();
        collection.insert("version".to_string(), Variable::new("v1"));

        let use_case = SwitchEnvironment::new(env_repo, secrets_repo);
        let result = use_case
            .execute(&PathBuf::from("/test"), "production", &globals, &collection)
            .await;

        assert!(result.is_ok());
        let output = result.expect("Should succeed");

        // All scopes should be available
        assert_eq!(
            output.resolution_context.resolve_value("api_url"),
            Some("https://api.example.com".to_string())
        );
        assert_eq!(
            output.resolution_context.resolve_value("app_name"),
            Some("MyApp".to_string())
        );
        assert_eq!(
            output.resolution_context.resolve_value("version"),
            Some("v1".to_string())
        );
    }
}
