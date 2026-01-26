//! Environment variable types

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single environment variable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variable {
    /// Variable name (used in placeholders like {{name}})
    pub name: String,
    /// Variable value
    pub value: String,
    /// Whether this is a secret (should be masked in UI)
    #[serde(default)]
    pub is_secret: bool,
    /// Whether this variable is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

const fn default_enabled() -> bool {
    true
}

impl Variable {
    /// Creates a new enabled variable.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            is_secret: false,
            enabled: true,
        }
    }

    /// Creates a new secret variable.
    #[must_use]
    pub fn secret(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            is_secret: true,
            enabled: true,
        }
    }
}

/// An environment containing a set of variables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    /// Unique identifier
    pub id: Uuid,
    /// Environment name (e.g., "Development", "Production")
    pub name: String,
    /// Variables in this environment
    #[serde(default)]
    pub variables: Vec<Variable>,
}

impl Environment {
    /// Creates a new empty environment.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            name: name.into(),
            variables: Vec::new(),
        }
    }

    /// Adds a variable to the environment.
    pub fn add_variable(&mut self, variable: Variable) {
        self.variables.push(variable);
    }

    /// Gets a variable by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Variable> {
        self.variables.iter().find(|v| v.name == name && v.enabled)
    }

    /// Resolves a placeholder value.
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<&str> {
        self.get(name).map(|v| v.value.as_str())
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new("Default")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_resolve() {
        let mut env = Environment::new("Test");
        env.add_variable(Variable::new("host", "api.example.com"));
        env.add_variable(Variable::new("port", "8080"));

        assert_eq!(env.resolve("host"), Some("api.example.com"));
        assert_eq!(env.resolve("port"), Some("8080"));
        assert_eq!(env.resolve("unknown"), None);
    }

    #[test]
    fn test_disabled_variable() {
        let mut env = Environment::new("Test");
        let mut var = Variable::new("disabled", "value");
        var.enabled = false;
        env.add_variable(var);

        assert_eq!(env.resolve("disabled"), None);
    }
}
