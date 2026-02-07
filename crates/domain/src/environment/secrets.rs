//! Local secrets storage (never committed to version control)
//!
//! File location: .vortex/secrets.json

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Local secrets storage (never committed to version control).
/// File: .vortex/secrets.json
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretsStore {
    /// Schema version for migration support.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,

    /// Secrets organized by environment name.
    /// Key: environment name (e.g., "development", "production")
    /// Value: map of variable name to secret value
    #[serde(default)]
    pub secrets: HashMap<String, HashMap<String, String>>,
}

const fn default_schema_version() -> u32 {
    1
}

impl SecretsStore {
    /// Creates an empty secrets store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            schema_version: 1,
            secrets: HashMap::new(),
        }
    }

    /// Gets a secret value for a specific environment and variable name.
    #[must_use]
    pub fn get_secret(&self, environment: &str, variable_name: &str) -> Option<&str> {
        self.secrets
            .get(&environment.to_lowercase())
            .and_then(|env_secrets| env_secrets.get(variable_name))
            .map(String::as_str)
    }

    /// Sets a secret value for a specific environment and variable name.
    pub fn set_secret(
        &mut self,
        environment: impl Into<String>,
        variable_name: impl Into<String>,
        value: impl Into<String>,
    ) {
        let env_key = environment.into().to_lowercase();
        self.secrets
            .entry(env_key)
            .or_default()
            .insert(variable_name.into(), value.into());
    }

    /// Removes a secret value.
    pub fn remove_secret(&mut self, environment: &str, variable_name: &str) -> Option<String> {
        self.secrets
            .get_mut(&environment.to_lowercase())
            .and_then(|env_secrets| env_secrets.remove(variable_name))
    }

    /// Returns all secret names for a given environment.
    #[must_use]
    pub fn secret_names(&self, environment: &str) -> Vec<&str> {
        self.secrets
            .get(&environment.to_lowercase())
            .map(|env| env.keys().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// Returns all environment names that have secrets.
    #[must_use]
    pub fn environment_names(&self) -> Vec<&str> {
        self.secrets.keys().map(String::as_str).collect()
    }

    /// Returns the number of secrets for a given environment.
    #[must_use]
    pub fn secret_count(&self, environment: &str) -> usize {
        self.secrets
            .get(&environment.to_lowercase())
            .map_or(0, HashMap::len)
    }

    /// Returns all secrets for a given environment as a map.
    #[must_use]
    pub fn get_environment_secrets(&self, environment: &str) -> Option<&HashMap<String, String>> {
        self.secrets.get(&environment.to_lowercase())
    }

    /// Removes all secrets for a given environment.
    pub fn remove_environment(&mut self, environment: &str) -> Option<HashMap<String, String>> {
        self.secrets.remove(&environment.to_lowercase())
    }
}

impl Default for SecretsStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_secrets_store_new() {
        let store = SecretsStore::new();
        assert_eq!(store.schema_version, 1);
        assert!(store.secrets.is_empty());
    }

    #[test]
    fn test_set_and_get_secret() {
        let mut store = SecretsStore::new();
        store.set_secret("development", "api_key", "sk-dev-123");

        assert_eq!(
            store.get_secret("development", "api_key"),
            Some("sk-dev-123")
        );
    }

    #[test]
    fn test_case_insensitive_environment() {
        let mut store = SecretsStore::new();
        store.set_secret("Development", "api_key", "sk-dev-123");

        // Should find with different case
        assert_eq!(
            store.get_secret("development", "api_key"),
            Some("sk-dev-123")
        );
        assert_eq!(
            store.get_secret("DEVELOPMENT", "api_key"),
            Some("sk-dev-123")
        );
    }

    #[test]
    fn test_remove_secret() {
        let mut store = SecretsStore::new();
        store.set_secret("development", "api_key", "sk-dev-123");
        store.set_secret("development", "client_secret", "cs-123");

        let removed = store.remove_secret("development", "api_key");
        assert_eq!(removed, Some("sk-dev-123".to_string()));
        assert_eq!(store.get_secret("development", "api_key"), None);
        assert_eq!(
            store.get_secret("development", "client_secret"),
            Some("cs-123")
        );
    }

    #[test]
    fn test_secret_names() {
        let mut store = SecretsStore::new();
        store.set_secret("development", "api_key", "sk-dev-123");
        store.set_secret("development", "client_secret", "cs-123");
        store.set_secret("production", "api_key", "sk-prod-456");

        let dev_secrets = store.secret_names("development");
        assert_eq!(dev_secrets.len(), 2);
        assert!(dev_secrets.contains(&"api_key"));
        assert!(dev_secrets.contains(&"client_secret"));

        let prod_secrets = store.secret_names("production");
        assert_eq!(prod_secrets.len(), 1);
    }

    #[test]
    fn test_environment_names() {
        let mut store = SecretsStore::new();
        store.set_secret("development", "api_key", "sk-dev-123");
        store.set_secret("production", "api_key", "sk-prod-456");

        let env_names = store.environment_names();
        assert_eq!(env_names.len(), 2);
    }

    #[test]
    fn test_secret_count() {
        let mut store = SecretsStore::new();
        store.set_secret("development", "api_key", "sk-dev-123");
        store.set_secret("development", "client_secret", "cs-123");

        assert_eq!(store.secret_count("development"), 2);
        assert_eq!(store.secret_count("production"), 0);
    }

    #[test]
    fn test_remove_environment() {
        let mut store = SecretsStore::new();
        store.set_secret("development", "api_key", "sk-dev-123");
        store.set_secret("development", "client_secret", "cs-123");

        let removed = store.remove_environment("development");
        assert!(removed.is_some());
        assert_eq!(removed.as_ref().map(HashMap::len), Some(2));
        assert_eq!(store.secret_count("development"), 0);
    }

    #[test]
    fn test_serialization() {
        let mut store = SecretsStore::new();
        store.set_secret("development", "api_key", "sk-dev-123");

        let json = serde_json::to_string(&store).expect("Failed to serialize");
        let deserialized: SecretsStore =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(store, deserialized);
    }
}
