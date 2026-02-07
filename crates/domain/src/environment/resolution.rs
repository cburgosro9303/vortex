//! Resolution context for variable substitution
//!
//! Holds all variable sources for resolution with proper precedence.

use std::collections::HashMap;

use super::globals::Globals;
use super::secrets::SecretsStore;
use super::variable::{Environment, ResolvedVariable, VariableMap, VariableScope};

/// Holds all variable sources for resolution.
/// Variables are resolved in order of precedence (highest wins):
/// 1. Built-in ($uuid, $timestamp, etc.) - handled by resolver engine
/// 2. Secrets (.vortex/secrets.json)
/// 3. Environment (environments/*.json)
/// 4. Collection (collection.json variables)
/// 5. Global (globals.json)
#[derive(Debug, Clone, Default)]
pub struct ResolutionContext {
    /// Global variables (lowest precedence for user variables).
    pub globals: VariableMap,

    /// Collection-level variables.
    pub collection: VariableMap,

    /// Active environment variables.
    pub environment: VariableMap,

    /// Active environment name (for display purposes).
    pub environment_name: String,

    /// Secrets for the active environment (highest precedence for user variables).
    pub secrets: HashMap<String, String>,
}

impl ResolutionContext {
    /// Creates a new empty resolution context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a resolution context from the given sources.
    #[must_use]
    pub fn from_sources(
        globals: &Globals,
        collection_variables: &VariableMap,
        environment: &Environment,
        secrets_store: &SecretsStore,
    ) -> Self {
        let env_secrets = secrets_store
            .get_environment_secrets(&environment.name)
            .cloned()
            .unwrap_or_default();

        Self {
            globals: globals.variables.clone(),
            collection: collection_variables.clone(),
            environment: environment.variables.clone(),
            environment_name: environment.name.clone(),
            secrets: env_secrets,
        }
    }

    /// Creates a resolution context with just environment and secrets.
    #[must_use]
    pub fn from_environment(environment: &Environment, secrets_store: &SecretsStore) -> Self {
        let env_secrets = secrets_store
            .get_environment_secrets(&environment.name)
            .cloned()
            .unwrap_or_default();

        Self {
            globals: VariableMap::new(),
            collection: VariableMap::new(),
            environment: environment.variables.clone(),
            environment_name: environment.name.clone(),
            secrets: env_secrets,
        }
    }

    /// Resolves a variable name to its value and scope.
    /// Returns None if the variable is not found in any scope.
    /// Note: Built-in variables are handled by the resolver engine, not here.
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<ResolvedVariable> {
        // Secrets (highest user-defined precedence)
        if let Some(value) = self.secrets.get(name) {
            return Some(ResolvedVariable {
                name: name.to_string(),
                value: value.clone(),
                scope: VariableScope::Secret,
            });
        }

        // Environment variables
        if let Some(var) = self.environment.get(name)
            && var.enabled {
                return Some(ResolvedVariable {
                    name: name.to_string(),
                    value: var.value.clone(),
                    scope: VariableScope::Environment,
                });
            }

        // Collection variables
        if let Some(var) = self.collection.get(name)
            && var.enabled {
                return Some(ResolvedVariable {
                    name: name.to_string(),
                    value: var.value.clone(),
                    scope: VariableScope::Collection,
                });
            }

        // Global variables (lowest precedence)
        if let Some(var) = self.globals.get(name)
            && var.enabled {
                return Some(ResolvedVariable {
                    name: name.to_string(),
                    value: var.value.clone(),
                    scope: VariableScope::Global,
                });
            }

        None
    }

    /// Resolves a variable name to just its value.
    #[must_use]
    pub fn resolve_value(&self, name: &str) -> Option<String> {
        self.resolve(name).map(|r| r.value)
    }

    /// Returns all variable names across all scopes.
    #[must_use]
    pub fn all_variable_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .globals
            .keys()
            .chain(self.collection.keys())
            .chain(self.environment.keys())
            .chain(self.secrets.keys())
            .cloned()
            .collect();

        names.sort();
        names.dedup();
        names
    }

    /// Returns the count of variables across all scopes.
    #[must_use]
    pub fn total_variable_count(&self) -> usize {
        self.all_variable_names().len()
    }

    /// Sets the globals source.
    #[must_use] 
    pub fn with_globals(mut self, globals: &Globals) -> Self {
        self.globals.clone_from(&globals.variables);
        self
    }

    /// Sets the collection variables source.
    #[must_use] 
    pub fn with_collection(mut self, collection: &VariableMap) -> Self {
        self.collection.clone_from(collection);
        self
    }

    /// Sets the environment source.
    #[must_use] 
    pub fn with_environment(mut self, environment: &Environment) -> Self {
        self.environment.clone_from(&environment.variables);
        self.environment_name.clone_from(&environment.name);
        self
    }

    /// Sets the secrets source.
    #[must_use] 
    pub fn with_secrets(mut self, secrets_store: &SecretsStore, environment_name: &str) -> Self {
        self.secrets = secrets_store
            .get_environment_secrets(environment_name)
            .cloned()
            .unwrap_or_default();
        self
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::environment::Variable;

    fn create_test_globals() -> Globals {
        let mut globals = Globals::new();
        globals.set_variable("app_name", Variable::new("TestApp"));
        globals.set_variable("api_version", Variable::new("v1"));
        globals
    }

    fn create_test_environment() -> Environment {
        let mut env = Environment::new("development");
        env.set_variable("base_url", Variable::new("http://localhost:3000"));
        env.set_variable("api_version", Variable::new("v2")); // Override global
        env.set_variable("api_key", Variable::secret(""));
        env
    }

    fn create_test_secrets() -> SecretsStore {
        let mut secrets = SecretsStore::new();
        secrets.set_secret("development", "api_key", "sk-secret-123");
        secrets
    }

    #[test]
    fn test_resolution_context_new() {
        let ctx = ResolutionContext::new();
        assert!(ctx.globals.is_empty());
        assert!(ctx.collection.is_empty());
        assert!(ctx.environment.is_empty());
        assert!(ctx.secrets.is_empty());
    }

    #[test]
    fn test_resolve_global_variable() {
        let globals = create_test_globals();
        let env = Environment::new("test");
        let secrets = SecretsStore::new();

        let ctx = ResolutionContext::from_sources(&globals, &VariableMap::new(), &env, &secrets);

        let resolved = ctx.resolve("app_name").expect("Should resolve");
        assert_eq!(resolved.value, "TestApp");
        assert_eq!(resolved.scope, VariableScope::Global);
    }

    #[test]
    fn test_resolve_environment_variable() {
        let globals = Globals::new();
        let env = create_test_environment();
        let secrets = SecretsStore::new();

        let ctx = ResolutionContext::from_sources(&globals, &VariableMap::new(), &env, &secrets);

        let resolved = ctx.resolve("base_url").expect("Should resolve");
        assert_eq!(resolved.value, "http://localhost:3000");
        assert_eq!(resolved.scope, VariableScope::Environment);
    }

    #[test]
    fn test_resolve_secret_variable() {
        let globals = Globals::new();
        let env = create_test_environment();
        let secrets = create_test_secrets();

        let ctx = ResolutionContext::from_sources(&globals, &VariableMap::new(), &env, &secrets);

        let resolved = ctx.resolve("api_key").expect("Should resolve");
        assert_eq!(resolved.value, "sk-secret-123");
        assert_eq!(resolved.scope, VariableScope::Secret);
    }

    #[test]
    fn test_precedence_environment_over_global() {
        let globals = create_test_globals();
        let env = create_test_environment();
        let secrets = SecretsStore::new();

        let ctx = ResolutionContext::from_sources(&globals, &VariableMap::new(), &env, &secrets);

        // api_version is defined in both global (v1) and environment (v2)
        let resolved = ctx.resolve("api_version").expect("Should resolve");
        assert_eq!(resolved.value, "v2");
        assert_eq!(resolved.scope, VariableScope::Environment);
    }

    #[test]
    fn test_precedence_secret_over_environment() {
        let globals = Globals::new();
        let env = create_test_environment();
        let secrets = create_test_secrets();

        let ctx = ResolutionContext::from_sources(&globals, &VariableMap::new(), &env, &secrets);

        // api_key is defined in both environment (empty) and secrets (sk-secret-123)
        let resolved = ctx.resolve("api_key").expect("Should resolve");
        assert_eq!(resolved.value, "sk-secret-123");
        assert_eq!(resolved.scope, VariableScope::Secret);
    }

    #[test]
    fn test_resolve_not_found() {
        let ctx = ResolutionContext::new();
        assert!(ctx.resolve("nonexistent").is_none());
    }

    #[test]
    fn test_resolve_disabled_variable() {
        let mut env = Environment::new("test");
        let mut var = Variable::new("disabled_value");
        var.enabled = false;
        env.set_variable("disabled", var);

        let ctx = ResolutionContext::from_sources(
            &Globals::new(),
            &VariableMap::new(),
            &env,
            &SecretsStore::new(),
        );

        assert!(ctx.resolve("disabled").is_none());
    }

    #[test]
    fn test_all_variable_names() {
        let globals = create_test_globals();
        let env = create_test_environment();
        let secrets = create_test_secrets();

        let ctx = ResolutionContext::from_sources(&globals, &VariableMap::new(), &env, &secrets);

        let names = ctx.all_variable_names();
        assert!(names.contains(&"app_name".to_string()));
        assert!(names.contains(&"base_url".to_string()));
        assert!(names.contains(&"api_key".to_string()));
        assert!(names.contains(&"api_version".to_string()));
    }

    #[test]
    fn test_resolve_value() {
        let mut env = Environment::new("test");
        env.set_variable("host", Variable::new("localhost"));

        let ctx = ResolutionContext::from_sources(
            &Globals::new(),
            &VariableMap::new(),
            &env,
            &SecretsStore::new(),
        );

        assert_eq!(ctx.resolve_value("host"), Some("localhost".to_string()));
        assert_eq!(ctx.resolve_value("nonexistent"), None);
    }

    #[test]
    fn test_builder_pattern() {
        let globals = create_test_globals();
        let env = create_test_environment();
        let secrets = create_test_secrets();

        let ctx = ResolutionContext::new()
            .with_globals(&globals)
            .with_environment(&env)
            .with_secrets(&secrets, &env.name);

        assert_eq!(ctx.resolve_value("app_name"), Some("TestApp".to_string()));
        assert_eq!(
            ctx.resolve_value("base_url"),
            Some("http://localhost:3000".to_string())
        );
        assert_eq!(
            ctx.resolve_value("api_key"),
            Some("sk-secret-123".to_string())
        );
    }
}
