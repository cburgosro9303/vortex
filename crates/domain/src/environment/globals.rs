//! Global variables shared across all collections and environments

use serde::{Deserialize, Serialize};

use super::variable::{Variable, VariableMap};

/// Global variables shared across all collections and environments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Globals {
    /// Schema version for migration support.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,

    /// Global variables available everywhere.
    #[serde(default)]
    pub variables: VariableMap,
}

const fn default_schema_version() -> u32 {
    1
}

impl Globals {
    /// Creates a new empty globals store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            schema_version: 1,
            variables: VariableMap::new(),
        }
    }

    /// Adds or updates a variable.
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

    /// Returns the number of variables.
    #[must_use]
    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }

    /// Resolves a variable value (returns value only if variable is enabled).
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
}

impl Default for Globals {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_globals_new() {
        let globals = Globals::new();
        assert_eq!(globals.schema_version, 1);
        assert!(globals.variables.is_empty());
    }

    #[test]
    fn test_globals_set_variable() {
        let mut globals = Globals::new();
        globals.set_variable("app_name", Variable::new("Vortex"));

        assert_eq!(globals.variable_count(), 1);
        assert_eq!(globals.resolve("app_name"), Some("Vortex"));
    }

    #[test]
    fn test_globals_remove_variable() {
        let mut globals = Globals::new();
        globals.add_variable("app_name", "Vortex");

        let removed = globals.remove_variable("app_name");
        assert!(removed.is_some());
        assert_eq!(globals.variable_count(), 0);
    }

    #[test]
    fn test_globals_resolve_disabled() {
        let mut globals = Globals::new();
        let mut var = Variable::new("disabled_value");
        var.enabled = false;
        globals.set_variable("disabled", var);

        assert_eq!(globals.resolve("disabled"), None);
    }
}
