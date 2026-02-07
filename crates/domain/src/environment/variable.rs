//! Environment variable types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Represents a single variable with its value and metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variable {
    /// The variable value. Empty string if secret value is stored elsewhere.
    pub value: String,

    /// If true, the actual value is stored in secrets.json, not in the environment file.
    /// The `value` field may be empty or contain a placeholder.
    #[serde(default)]
    pub secret: bool,

    /// Whether this variable is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

const fn default_enabled() -> bool {
    true
}

impl Variable {
    /// Creates a new non-secret variable.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            secret: false,
            enabled: true,
        }
    }

    /// Creates a new secret variable (value stored separately).
    #[must_use]
    pub fn secret(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            secret: true,
            enabled: true,
        }
    }

    /// Creates a disabled variable.
    #[must_use]
    pub fn disabled(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            secret: false,
            enabled: false,
        }
    }

    /// Returns the value if the variable is enabled.
    #[must_use]
    pub fn enabled_value(&self) -> Option<&str> {
        if self.enabled {
            Some(&self.value)
        } else {
            None
        }
    }
}

impl Default for Variable {
    fn default() -> Self {
        Self {
            value: String::new(),
            secret: false,
            enabled: true,
        }
    }
}

/// A collection of variables keyed by name.
pub type VariableMap = HashMap<String, Variable>;

/// Defines the scope/origin of a variable for resolution precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum VariableScope {
    /// Variables defined in globals.json - lowest precedence
    Global = 0,
    /// Variables defined in collection.json
    Collection = 1,
    /// Variables defined in environment files (environments/*.json)
    Environment = 2,
    /// Secret values from .vortex/secrets.json
    Secret = 3,
    /// Built-in dynamic variables ($uuid, $timestamp, etc.) - highest precedence
    BuiltIn = 4,
}

impl VariableScope {
    /// Returns the precedence level (higher = takes priority).
    #[must_use]
    pub const fn precedence(&self) -> u8 {
        *self as u8
    }

    /// Returns a human-readable name for the scope.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Global => "Global",
            Self::Collection => "Collection",
            Self::Environment => "Environment",
            Self::Secret => "Secret",
            Self::BuiltIn => "Built-in",
        }
    }
}

/// A resolved variable with its value and origin scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedVariable {
    /// The variable name (without {{ }}).
    pub name: String,
    /// The resolved value.
    pub value: String,
    /// The scope from which this value was resolved.
    pub scope: VariableScope,
}

impl ResolvedVariable {
    /// Creates a new resolved variable.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>, scope: VariableScope) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            scope,
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
    /// Schema version for migration support.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Variables in this environment.
    /// Key is variable name, value contains the variable data.
    #[serde(default)]
    pub variables: VariableMap,
}

const fn default_schema_version() -> u32 {
    1
}

impl Environment {
    /// Creates a new environment with the given name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            name: name.into(),
            schema_version: 1,
            variables: HashMap::new(),
        }
    }

    /// Adds or updates a variable in this environment.
    pub fn set_variable(&mut self, name: impl Into<String>, variable: Variable) {
        self.variables.insert(name.into(), variable);
    }

    /// Gets a variable by name.
    #[must_use]
    pub fn get_variable(&self, name: &str) -> Option<&Variable> {
        self.variables.get(name)
    }

    /// Removes a variable by name.
    pub fn remove_variable(&mut self, name: &str) -> Option<Variable> {
        self.variables.remove(name)
    }

    /// Returns the number of variables in this environment.
    #[must_use]
    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }

    /// Returns names of all variables marked as secret.
    #[must_use]
    pub fn secret_variable_names(&self) -> Vec<&str> {
        self.variables
            .iter()
            .filter(|(_, v)| v.secret)
            .map(|(k, _)| k.as_str())
            .collect()
    }

    /// Returns names of all enabled variables.
    #[must_use]
    pub fn enabled_variable_names(&self) -> Vec<&str> {
        self.variables
            .iter()
            .filter(|(_, v)| v.enabled)
            .map(|(k, _)| k.as_str())
            .collect()
    }

    /// Resolves a placeholder value (returns value only if variable is enabled).
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<&str> {
        self.get_variable(name)
            .filter(|v| v.enabled)
            .map(|v| v.value.as_str())
    }

    /// Adds a variable with name and value.
    pub fn add_variable(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.variables.insert(name.into(), Variable::new(value));
    }

    /// Adds a secret variable with name and value.
    pub fn add_secret(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.variables.insert(name.into(), Variable::secret(value));
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new("New Environment")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_new() {
        let var = Variable::new("test_value");
        assert_eq!(var.value, "test_value");
        assert!(!var.secret);
        assert!(var.enabled);
    }

    #[test]
    fn test_variable_secret() {
        let var = Variable::secret("secret_value");
        assert_eq!(var.value, "secret_value");
        assert!(var.secret);
        assert!(var.enabled);
    }

    #[test]
    fn test_variable_disabled() {
        let var = Variable::disabled("disabled_value");
        assert_eq!(var.value, "disabled_value");
        assert!(!var.secret);
        assert!(!var.enabled);
        assert_eq!(var.enabled_value(), None);
    }

    #[test]
    fn test_environment_new() {
        let env = Environment::new("Development");
        assert_eq!(env.name, "Development");
        assert_eq!(env.schema_version, 1);
        assert!(env.variables.is_empty());
    }

    #[test]
    fn test_environment_set_variable() {
        let mut env = Environment::new("Test");
        env.set_variable("host", Variable::new("localhost"));
        env.set_variable("port", Variable::new("8080"));

        assert_eq!(env.variable_count(), 2);
        assert_eq!(env.get_variable("host").map(|v| v.value.as_str()), Some("localhost"));
        assert_eq!(env.resolve("port"), Some("8080"));
    }

    #[test]
    fn test_environment_remove_variable() {
        let mut env = Environment::new("Test");
        env.set_variable("host", Variable::new("localhost"));

        let removed = env.remove_variable("host");
        assert!(removed.is_some());
        assert_eq!(env.variable_count(), 0);
    }

    #[test]
    fn test_environment_secret_variables() {
        let mut env = Environment::new("Test");
        env.set_variable("host", Variable::new("localhost"));
        env.set_variable("api_key", Variable::secret("sk-123"));
        env.set_variable("token", Variable::secret("tkn-456"));

        let secrets = env.secret_variable_names();
        assert_eq!(secrets.len(), 2);
        assert!(secrets.contains(&"api_key"));
        assert!(secrets.contains(&"token"));
    }

    #[test]
    fn test_environment_resolve_disabled() {
        let mut env = Environment::new("Test");
        let mut var = Variable::new("disabled_value");
        var.enabled = false;
        env.set_variable("disabled", var);

        assert_eq!(env.resolve("disabled"), None);
    }

    #[test]
    fn test_variable_scope_precedence() {
        assert!(VariableScope::BuiltIn.precedence() > VariableScope::Secret.precedence());
        assert!(VariableScope::Secret.precedence() > VariableScope::Environment.precedence());
        assert!(VariableScope::Environment.precedence() > VariableScope::Collection.precedence());
        assert!(VariableScope::Collection.precedence() > VariableScope::Global.precedence());
    }

    #[test]
    fn test_resolved_variable() {
        let resolved = ResolvedVariable::new("base_url", "https://api.example.com", VariableScope::Environment);
        assert_eq!(resolved.name, "base_url");
        assert_eq!(resolved.value, "https://api.example.com");
        assert_eq!(resolved.scope, VariableScope::Environment);
    }
}
