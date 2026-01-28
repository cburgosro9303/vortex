//! Postman Environment Type Definitions
//!
//! This module defines the types that represent a Postman Environment JSON file.

use serde::{Deserialize, Serialize};

/// Root structure for Postman Environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanEnvironment {
    /// Environment ID
    #[serde(default)]
    pub id: Option<String>,
    /// Environment name
    pub name: String,
    /// Environment variables
    #[serde(default)]
    pub values: Vec<PostmanEnvVariable>,
    /// Postman-specific ID
    #[serde(rename = "_postman_variable_scope", default)]
    pub postman_variable_scope: Option<String>,
    /// Postman exported using
    #[serde(rename = "_postman_exported_at", default)]
    pub postman_exported_at: Option<String>,
    /// Postman exported using
    #[serde(rename = "_postman_exported_using", default)]
    pub postman_exported_using: Option<String>,
}

/// Postman environment variable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanEnvVariable {
    /// Variable key/name
    pub key: String,
    /// Variable value
    #[serde(default)]
    pub value: String,
    /// Whether the variable is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Variable type (default, secret, any)
    #[serde(rename = "type", default)]
    pub var_type: Option<String>,
}

fn default_true() -> bool {
    true
}

impl PostmanEnvVariable {
    /// Check if this variable is a secret type
    pub fn is_secret(&self) -> bool {
        self.var_type.as_deref() == Some("secret")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_environment() {
        let json = r#"{
            "id": "env-123",
            "name": "Development",
            "values": [
                {"key": "BASE_URL", "value": "https://dev.api.com", "enabled": true},
                {"key": "API_KEY", "value": "secret123", "enabled": true, "type": "secret"}
            ],
            "_postman_variable_scope": "environment"
        }"#;

        let env: PostmanEnvironment = serde_json::from_str(json).unwrap();
        assert_eq!(env.name, "Development");
        assert_eq!(env.values.len(), 2);
        assert!(!env.values[0].is_secret());
        assert!(env.values[1].is_secret());
    }

    #[test]
    fn test_parse_minimal_environment() {
        let json = r#"{"name": "Test", "values": []}"#;
        let env: PostmanEnvironment = serde_json::from_str(json).unwrap();
        assert_eq!(env.name, "Test");
        assert!(env.values.is_empty());
    }

    #[test]
    fn test_default_enabled() {
        let json = r#"{"key": "foo", "value": "bar"}"#;
        let var: PostmanEnvVariable = serde_json::from_str(json).unwrap();
        assert!(var.enabled); // Should default to true
    }
}
