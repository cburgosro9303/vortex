# Sprint 03 - Environments and Variables System

**Objective:** Implement a complete environment and variable management system enabling context switching (dev/staging/prod) with variable scopes, secret handling, and real-time resolution preview.

**Duration:** 2-3 weeks
**Milestone:** M3

---

## Table of Contents

1. [Scope](#scope)
2. [Out of Scope](#out-of-scope)
3. [Domain Models](#domain-models)
4. [Variable Resolution Engine](#variable-resolution-engine)
5. [Use Cases](#use-cases)
6. [Secrets Management](#secrets-management)
7. [UI Components](#ui-components)
8. [File Persistence](#file-persistence)
9. [Implementation Order](#implementation-order)
10. [Acceptance Criteria](#acceptance-criteria)
11. [Risks and Mitigations](#risks-and-mitigations)

---

## Scope

- Domain models for `Environment`, `Variable`, and `VariableScope`
- Variable resolution engine with `{{variable}}` syntax parsing
- Built-in dynamic variables (`$uuid`, `$timestamp`, etc.)
- Secrets storage in `.vortex/secrets.json` (gitignored)
- UI for environment selection and variable editing
- Real-time preview of resolved variables in URL bar
- Unresolved variable detection and warnings

## Out of Scope

- OAuth token management (Sprint 04)
- Automated tests/assertions (Sprint 05)
- Environment import/export
- Variable encryption at rest
- Keychain/credential manager integration (future enhancement)

---

## Domain Models

All domain types belong in the `domain` crate with no external dependencies beyond `serde` for serialization.

### Variable Struct

```rust
// domain/src/variable.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a single variable with its value and metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variable {
    /// The variable value. Empty string if secret value is stored elsewhere.
    pub value: String,

    /// If true, the actual value is stored in secrets.json, not in the environment file.
    /// The `value` field may be empty or contain a placeholder.
    #[serde(default)]
    pub secret: bool,
}

impl Variable {
    /// Creates a new non-secret variable.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            secret: false,
        }
    }

    /// Creates a new secret variable (value stored separately).
    pub fn secret(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            secret: true,
        }
    }
}

impl Default for Variable {
    fn default() -> Self {
        Self {
            value: String::new(),
            secret: false,
        }
    }
}

/// A collection of variables keyed by name.
pub type VariableMap = HashMap<String, Variable>;
```

### VariableScope Enum

```rust
// domain/src/variable.rs (continued)

/// Defines the scope/origin of a variable for resolution precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    pub fn precedence(&self) -> u8 {
        *self as u8
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
```

### Environment Struct

```rust
// domain/src/environment.rs

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashMap;
use crate::variable::{Variable, VariableMap};

/// Represents a named environment (e.g., Development, Staging, Production).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    /// Unique identifier for the environment.
    pub id: Uuid,

    /// Human-readable name (e.g., "Development", "Production").
    pub name: String,

    /// Schema version for migration support.
    pub schema_version: u32,

    /// Variables defined in this environment.
    /// Key is variable name, value contains the variable data.
    pub variables: VariableMap,
}

impl Environment {
    /// Creates a new environment with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
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
    pub fn get_variable(&self, name: &str) -> Option<&Variable> {
        self.variables.get(name)
    }

    /// Removes a variable by name.
    pub fn remove_variable(&mut self, name: &str) -> Option<Variable> {
        self.variables.remove(name)
    }

    /// Returns the number of variables in this environment.
    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }

    /// Returns names of all variables marked as secret.
    pub fn secret_variable_names(&self) -> Vec<&str> {
        self.variables
            .iter()
            .filter(|(_, v)| v.secret)
            .map(|(k, _)| k.as_str())
            .collect()
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new("New Environment")
    }
}
```

### Globals Struct

```rust
// domain/src/globals.rs

use serde::{Deserialize, Serialize};
use crate::variable::VariableMap;

/// Global variables shared across all collections and environments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Globals {
    /// Schema version for migration support.
    pub schema_version: u32,

    /// Global variables available everywhere.
    pub variables: VariableMap,
}

impl Default for Globals {
    fn default() -> Self {
        Self {
            schema_version: 1,
            variables: Default::default(),
        }
    }
}
```

### Secrets Storage Struct

```rust
// domain/src/secrets.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Local secrets storage (never committed to version control).
/// File: .vortex/secrets.json
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SecretsStore {
    /// Schema version for migration support.
    pub schema_version: u32,

    /// Secrets organized by environment name.
    /// Key: environment name (e.g., "development", "production")
    /// Value: map of variable name to secret value
    pub secrets: HashMap<String, HashMap<String, String>>,
}

impl SecretsStore {
    /// Creates an empty secrets store.
    pub fn new() -> Self {
        Self {
            schema_version: 1,
            secrets: HashMap::new(),
        }
    }

    /// Gets a secret value for a specific environment and variable name.
    pub fn get_secret(&self, environment: &str, variable_name: &str) -> Option<&str> {
        self.secrets
            .get(environment)
            .and_then(|env_secrets| env_secrets.get(variable_name))
            .map(|s| s.as_str())
    }

    /// Sets a secret value for a specific environment and variable name.
    pub fn set_secret(
        &mut self,
        environment: impl Into<String>,
        variable_name: impl Into<String>,
        value: impl Into<String>
    ) {
        self.secrets
            .entry(environment.into())
            .or_default()
            .insert(variable_name.into(), value.into());
    }

    /// Removes a secret value.
    pub fn remove_secret(&mut self, environment: &str, variable_name: &str) -> Option<String> {
        self.secrets
            .get_mut(environment)
            .and_then(|env_secrets| env_secrets.remove(variable_name))
    }

    /// Returns all secret names for a given environment.
    pub fn secret_names(&self, environment: &str) -> Vec<&str> {
        self.secrets
            .get(environment)
            .map(|env| env.keys().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
}
```

### Resolution Context Struct

```rust
// domain/src/resolution.rs

use std::collections::HashMap;
use crate::variable::{VariableMap, VariableScope, ResolvedVariable};
use crate::environment::Environment;
use crate::globals::Globals;
use crate::secrets::SecretsStore;

/// Holds all variable sources for resolution.
/// Variables are resolved in order of precedence (highest wins):
/// 1. Built-in ($uuid, $timestamp, etc.)
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

    /// Active environment name (for secret lookup).
    pub environment_name: String,

    /// Secrets store reference (highest precedence for user variables).
    pub secrets: HashMap<String, String>,
}

impl ResolutionContext {
    /// Creates a new resolution context from the given sources.
    pub fn new(
        globals: &Globals,
        collection_variables: &VariableMap,
        environment: &Environment,
        secrets_store: &SecretsStore,
    ) -> Self {
        let env_secrets = secrets_store
            .secrets
            .get(&environment.name.to_lowercase())
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

    /// Resolves a variable name to its value and scope.
    /// Returns None if the variable is not found in any scope.
    pub fn resolve(&self, name: &str) -> Option<ResolvedVariable> {
        // Built-in variables have highest precedence (handled by resolver engine)

        // Secrets (highest user-defined precedence)
        if let Some(value) = self.secrets.get(name) {
            return Some(ResolvedVariable {
                name: name.to_string(),
                value: value.clone(),
                scope: VariableScope::Secret,
            });
        }

        // Environment variables
        if let Some(var) = self.environment.get(name) {
            // If marked as secret but no secret value found, use the stored value
            return Some(ResolvedVariable {
                name: name.to_string(),
                value: var.value.clone(),
                scope: VariableScope::Environment,
            });
        }

        // Collection variables
        if let Some(var) = self.collection.get(name) {
            return Some(ResolvedVariable {
                name: name.to_string(),
                value: var.value.clone(),
                scope: VariableScope::Collection,
            });
        }

        // Global variables (lowest precedence)
        if let Some(var) = self.globals.get(name) {
            return Some(ResolvedVariable {
                name: name.to_string(),
                value: var.value.clone(),
                scope: VariableScope::Global,
            });
        }

        None
    }
}
```

---

## Variable Resolution Engine

The resolution engine parses `{{variable}}` syntax and resolves variables according to the defined precedence.

### Parser Module

```rust
// application/src/variable_resolver/parser.rs

use std::ops::Range;

/// Represents a parsed variable reference in a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableReference {
    /// The variable name (without {{ }}).
    pub name: String,

    /// Whether this is a built-in variable (starts with $).
    pub is_builtin: bool,

    /// Byte range in the original string where this reference appears.
    pub span: Range<usize>,
}

/// Parses a string and extracts all variable references.
///
/// Supports:
/// - `{{variable_name}}` - user-defined variables
/// - `{{$uuid}}` - built-in dynamic variables
///
/// # Example
/// ```
/// let refs = parse_variables("Hello {{name}}, your ID is {{$uuid}}");
/// assert_eq!(refs.len(), 2);
/// assert_eq!(refs[0].name, "name");
/// assert_eq!(refs[1].name, "$uuid");
/// assert!(refs[1].is_builtin);
/// ```
pub fn parse_variables(input: &str) -> Vec<VariableReference> {
    let mut references = Vec::new();
    let mut chars = input.char_indices().peekable();

    while let Some((i, ch)) = chars.next() {
        if ch == '{' {
            // Check for {{
            if let Some((_, next_ch)) = chars.peek() {
                if *next_ch == '{' {
                    chars.next(); // consume second {
                    let start = i;
                    let mut name = String::new();

                    // Read until }}
                    while let Some((_, ch)) = chars.next() {
                        if ch == '}' {
                            if let Some((end_idx, '}')) = chars.peek() {
                                let end = *end_idx + 1;
                                chars.next(); // consume second }

                                let trimmed_name = name.trim().to_string();
                                if !trimmed_name.is_empty() {
                                    references.push(VariableReference {
                                        name: trimmed_name.clone(),
                                        is_builtin: trimmed_name.starts_with('$'),
                                        span: start..end,
                                    });
                                }
                                break;
                            }
                        }
                        name.push(ch);
                    }
                }
            }
        }
    }

    references
}

/// Validates a variable name.
/// Valid names: alphanumeric, underscore, and optionally starting with $ for built-ins.
pub fn is_valid_variable_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let name = if name.starts_with('$') {
        &name[1..]
    } else {
        name
    };

    if name.is_empty() {
        return false;
    }

    name.chars().all(|c| c.is_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_variable() {
        let refs = parse_variables("{{name}}");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "name");
        assert!(!refs[0].is_builtin);
    }

    #[test]
    fn test_parse_builtin_variable() {
        let refs = parse_variables("{{$uuid}}");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "$uuid");
        assert!(refs[0].is_builtin);
    }

    #[test]
    fn test_parse_multiple_variables() {
        let refs = parse_variables("{{base_url}}/api/{{version}}/users/{{$uuid}}");
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0].name, "base_url");
        assert_eq!(refs[1].name, "version");
        assert_eq!(refs[2].name, "$uuid");
    }

    #[test]
    fn test_parse_with_whitespace() {
        let refs = parse_variables("{{ name }}");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "name");
    }

    #[test]
    fn test_no_variables() {
        let refs = parse_variables("Hello, World!");
        assert!(refs.is_empty());
    }

    #[test]
    fn test_unclosed_variable() {
        let refs = parse_variables("{{name");
        assert!(refs.is_empty());
    }

    #[test]
    fn test_empty_variable() {
        let refs = parse_variables("{{}}");
        assert!(refs.is_empty());
    }
}
```

### Built-in Variables Generator

```rust
// application/src/variable_resolver/builtins.rs

use chrono::{DateTime, Utc};
use uuid::Uuid;
use rand::Rng;

/// Generates values for built-in dynamic variables.
/// These variables are prefixed with $ and generate new values on each resolution.
pub struct BuiltinVariables;

impl BuiltinVariables {
    /// Resolves a built-in variable name to its value.
    /// Returns None if the name is not a recognized built-in.
    pub fn resolve(name: &str) -> Option<String> {
        match name {
            "$uuid" => Some(Self::generate_uuid()),
            "$timestamp" => Some(Self::generate_timestamp()),
            "$isoTimestamp" => Some(Self::generate_iso_timestamp()),
            "$randomInt" => Some(Self::generate_random_int()),
            "$randomString" => Some(Self::generate_random_string()),
            "$randomEmail" => Some(Self::generate_random_email()),
            "$randomUuid" => Some(Self::generate_uuid()), // alias for $uuid
            _ => None,
        }
    }

    /// Returns a list of all available built-in variable names with descriptions.
    pub fn available() -> Vec<(&'static str, &'static str)> {
        vec![
            ("$uuid", "Random UUID v4"),
            ("$timestamp", "Unix timestamp in seconds"),
            ("$isoTimestamp", "ISO 8601 timestamp (UTC)"),
            ("$randomInt", "Random integer 0-1000"),
            ("$randomString", "Random alphanumeric string (16 chars)"),
            ("$randomEmail", "Random email address"),
        ]
    }

    /// Generates a random UUID v4.
    fn generate_uuid() -> String {
        Uuid::new_v4().to_string()
    }

    /// Generates current Unix timestamp in seconds.
    fn generate_timestamp() -> String {
        Utc::now().timestamp().to_string()
    }

    /// Generates current timestamp in ISO 8601 format.
    fn generate_iso_timestamp() -> String {
        Utc::now().to_rfc3339()
    }

    /// Generates a random integer between 0 and 1000.
    fn generate_random_int() -> String {
        let mut rng = rand::thread_rng();
        rng.gen_range(0..=1000).to_string()
    }

    /// Generates a random 16-character alphanumeric string.
    fn generate_random_string() -> String {
        use rand::distributions::Alphanumeric;
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect()
    }

    /// Generates a random email address.
    fn generate_random_email() -> String {
        let random_part: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(8)
            .map(char::from)
            .collect();
        format!("{}@example.com", random_part.to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uuid_generation() {
        let uuid = BuiltinVariables::resolve("$uuid").unwrap();
        assert!(Uuid::parse_str(&uuid).is_ok());
    }

    #[test]
    fn test_timestamp_generation() {
        let ts = BuiltinVariables::resolve("$timestamp").unwrap();
        let parsed: i64 = ts.parse().unwrap();
        assert!(parsed > 0);
    }

    #[test]
    fn test_unknown_builtin() {
        assert!(BuiltinVariables::resolve("$unknown").is_none());
    }
}
```

### Resolution Engine

```rust
// application/src/variable_resolver/engine.rs

use crate::variable_resolver::parser::{parse_variables, VariableReference};
use crate::variable_resolver::builtins::BuiltinVariables;
use domain::resolution::ResolutionContext;
use domain::variable::{VariableScope, ResolvedVariable};
use std::collections::HashMap;

/// Result of variable resolution for a string.
#[derive(Debug, Clone)]
pub struct ResolutionResult {
    /// The resolved string with all variables substituted.
    pub resolved: String,

    /// Variables that were successfully resolved.
    pub resolved_variables: Vec<ResolvedVariable>,

    /// Variable names that could not be resolved.
    pub unresolved: Vec<String>,

    /// Whether all variables were successfully resolved.
    pub is_complete: bool,
}

/// The variable resolution engine.
/// Resolves `{{variable}}` references according to precedence rules.
pub struct VariableResolver {
    context: ResolutionContext,
    /// Cache for built-in variables to ensure consistency within a single resolution.
    builtin_cache: HashMap<String, String>,
}

impl VariableResolver {
    /// Creates a new resolver with the given context.
    pub fn new(context: ResolutionContext) -> Self {
        Self {
            context,
            builtin_cache: HashMap::new(),
        }
    }

    /// Updates the resolution context.
    pub fn set_context(&mut self, context: ResolutionContext) {
        self.context = context;
        self.builtin_cache.clear();
    }

    /// Clears the built-in variable cache.
    /// Call this before resolving a new request to generate fresh dynamic values.
    pub fn clear_builtin_cache(&mut self) {
        self.builtin_cache.clear();
    }

    /// Resolves all variables in the input string.
    pub fn resolve(&mut self, input: &str) -> ResolutionResult {
        let references = parse_variables(input);

        if references.is_empty() {
            return ResolutionResult {
                resolved: input.to_string(),
                resolved_variables: Vec::new(),
                unresolved: Vec::new(),
                is_complete: true,
            };
        }

        let mut resolved_vars = Vec::new();
        let mut unresolved = Vec::new();
        let mut result = String::with_capacity(input.len());
        let mut last_end = 0;

        for var_ref in &references {
            // Append text before this variable
            result.push_str(&input[last_end..var_ref.span.start]);

            // Resolve the variable
            if let Some(resolved) = self.resolve_variable(&var_ref.name) {
                result.push_str(&resolved.value);
                resolved_vars.push(resolved);
            } else {
                // Keep the original {{variable}} for unresolved
                result.push_str(&input[var_ref.span.clone()]);
                unresolved.push(var_ref.name.clone());
            }

            last_end = var_ref.span.end;
        }

        // Append remaining text after last variable
        result.push_str(&input[last_end..]);

        ResolutionResult {
            resolved: result,
            resolved_variables: resolved_vars,
            unresolved: unresolved.clone(),
            is_complete: unresolved.is_empty(),
        }
    }

    /// Resolves a single variable by name.
    fn resolve_variable(&mut self, name: &str) -> Option<ResolvedVariable> {
        // 1. Built-in variables (highest precedence)
        if name.starts_with('$') {
            let value = if let Some(cached) = self.builtin_cache.get(name) {
                cached.clone()
            } else if let Some(generated) = BuiltinVariables::resolve(name) {
                self.builtin_cache.insert(name.to_string(), generated.clone());
                generated
            } else {
                return None;
            };

            return Some(ResolvedVariable {
                name: name.to_string(),
                value,
                scope: VariableScope::BuiltIn,
            });
        }

        // 2-5. User-defined variables via context
        self.context.resolve(name)
    }

    /// Checks which variables in the input would be unresolved.
    /// Useful for validation before sending a request.
    pub fn find_unresolved(&self, input: &str) -> Vec<String> {
        let references = parse_variables(input);
        let mut unresolved = Vec::new();

        for var_ref in references {
            if var_ref.is_builtin {
                if BuiltinVariables::resolve(&var_ref.name).is_none() {
                    unresolved.push(var_ref.name);
                }
            } else if self.context.resolve(&var_ref.name).is_none() {
                unresolved.push(var_ref.name);
            }
        }

        unresolved
    }

    /// Extracts all variable names from the input without resolving them.
    pub fn extract_variable_names(input: &str) -> Vec<String> {
        parse_variables(input)
            .into_iter()
            .map(|r| r.name)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::environment::Environment;
    use domain::globals::Globals;
    use domain::secrets::SecretsStore;
    use domain::variable::Variable;
    use std::collections::HashMap;

    fn create_test_context() -> ResolutionContext {
        let mut globals = Globals::default();
        globals.variables.insert("app_name".to_string(), Variable::new("TestApp"));

        let mut collection_vars = HashMap::new();
        collection_vars.insert("base_path".to_string(), Variable::new("/api/v1"));

        let mut env = Environment::new("development");
        env.set_variable("base_url", Variable::new("http://localhost:3000"));
        env.set_variable("api_key", Variable::secret(""));

        let mut secrets = SecretsStore::new();
        secrets.set_secret("development", "api_key", "sk-secret-123");

        ResolutionContext::new(&globals, &collection_vars, &env, &secrets)
    }

    #[test]
    fn test_resolve_user_variable() {
        let context = create_test_context();
        let mut resolver = VariableResolver::new(context);

        let result = resolver.resolve("{{base_url}}/users");
        assert_eq!(result.resolved, "http://localhost:3000/users");
        assert!(result.is_complete);
    }

    #[test]
    fn test_resolve_secret_variable() {
        let context = create_test_context();
        let mut resolver = VariableResolver::new(context);

        let result = resolver.resolve("Bearer {{api_key}}");
        assert_eq!(result.resolved, "Bearer sk-secret-123");
        assert_eq!(result.resolved_variables[0].scope, VariableScope::Secret);
    }

    #[test]
    fn test_resolve_builtin_variable() {
        let context = create_test_context();
        let mut resolver = VariableResolver::new(context);

        let result = resolver.resolve("{{$uuid}}");
        assert!(result.is_complete);
        // UUID should be valid
        assert!(uuid::Uuid::parse_str(&result.resolved).is_ok());
    }

    #[test]
    fn test_unresolved_variable() {
        let context = create_test_context();
        let mut resolver = VariableResolver::new(context);

        let result = resolver.resolve("{{unknown_var}}");
        assert!(!result.is_complete);
        assert_eq!(result.unresolved, vec!["unknown_var"]);
        assert_eq!(result.resolved, "{{unknown_var}}");
    }

    #[test]
    fn test_precedence_secret_over_environment() {
        let context = create_test_context();
        let mut resolver = VariableResolver::new(context);

        // api_key is defined in both environment (empty) and secrets (sk-secret-123)
        let result = resolver.resolve("{{api_key}}");
        assert_eq!(result.resolved, "sk-secret-123");
        assert_eq!(result.resolved_variables[0].scope, VariableScope::Secret);
    }

    #[test]
    fn test_multiple_variables() {
        let context = create_test_context();
        let mut resolver = VariableResolver::new(context);

        let result = resolver.resolve("{{base_url}}{{base_path}}/users");
        assert_eq!(result.resolved, "http://localhost:3000/api/v1/users");
        assert_eq!(result.resolved_variables.len(), 2);
    }
}
```

---

## Use Cases

All use cases belong in the `application` crate.

### ResolveVariables Use Case

```rust
// application/src/use_cases/resolve_variables.rs

use crate::variable_resolver::engine::{VariableResolver, ResolutionResult};
use domain::resolution::ResolutionContext;
use domain::request::Request;

/// Input for the ResolveVariables use case.
pub struct ResolveVariablesInput {
    /// The resolution context containing all variable sources.
    pub context: ResolutionContext,
    /// The request to resolve.
    pub request: Request,
}

/// Output containing the resolved request and resolution details.
pub struct ResolveVariablesOutput {
    /// The request with all variables resolved.
    pub resolved_request: Request,
    /// URLs that had unresolved variables.
    pub url_unresolved: Vec<String>,
    /// Headers that had unresolved variables.
    pub headers_unresolved: Vec<(String, Vec<String>)>,
    /// Body that had unresolved variables.
    pub body_unresolved: Vec<String>,
    /// Whether all variables were resolved.
    pub is_complete: bool,
}

/// Resolves all variables in a request before execution.
pub struct ResolveVariablesUseCase;

impl ResolveVariablesUseCase {
    /// Executes the use case.
    pub fn execute(input: ResolveVariablesInput) -> ResolveVariablesOutput {
        let mut resolver = VariableResolver::new(input.context);
        resolver.clear_builtin_cache(); // Fresh values for each request

        let mut request = input.request.clone();
        let mut url_unresolved = Vec::new();
        let mut headers_unresolved = Vec::new();
        let mut body_unresolved = Vec::new();

        // Resolve URL
        let url_result = resolver.resolve(&request.url);
        request.url = url_result.resolved;
        url_unresolved = url_result.unresolved;

        // Resolve headers
        let mut resolved_headers = std::collections::HashMap::new();
        for (key, value) in &request.headers {
            let key_result = resolver.resolve(key);
            let value_result = resolver.resolve(value);

            resolved_headers.insert(key_result.resolved, value_result.resolved);

            if !key_result.is_complete || !value_result.is_complete {
                let mut unresolved = key_result.unresolved;
                unresolved.extend(value_result.unresolved);
                headers_unresolved.push((key.clone(), unresolved));
            }
        }
        request.headers = resolved_headers;

        // Resolve query params
        let mut resolved_params = std::collections::HashMap::new();
        for (key, value) in &request.query_params {
            let key_result = resolver.resolve(key);
            let value_result = resolver.resolve(value);
            resolved_params.insert(key_result.resolved, value_result.resolved);
        }
        request.query_params = resolved_params;

        // Resolve body
        if let Some(body) = &request.body {
            let body_result = resolver.resolve(&body.content_as_string());
            body_unresolved = body_result.unresolved;
            request.body = Some(body.with_resolved_content(body_result.resolved));
        }

        // Resolve auth
        request.auth = request.auth.map(|auth| auth.resolve_with(&mut resolver));

        let is_complete = url_unresolved.is_empty()
            && headers_unresolved.is_empty()
            && body_unresolved.is_empty();

        ResolveVariablesOutput {
            resolved_request: request,
            url_unresolved,
            headers_unresolved,
            body_unresolved,
            is_complete,
        }
    }
}
```

### LoadEnvironment Use Case

```rust
// application/src/use_cases/load_environment.rs

use domain::environment::Environment;
use crate::ports::environment_repository::EnvironmentRepository;
use std::path::PathBuf;

/// Errors that can occur when loading an environment.
#[derive(Debug, thiserror::Error)]
pub enum LoadEnvironmentError {
    #[error("Environment not found: {0}")]
    NotFound(String),

    #[error("Failed to read environment file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Failed to parse environment file: {0}")]
    ParseError(String),
}

/// Input for the LoadEnvironment use case.
pub struct LoadEnvironmentInput {
    /// Path to the workspace root.
    pub workspace_path: PathBuf,
    /// Name of the environment to load (without .json extension).
    pub environment_name: String,
}

/// Output containing the loaded environment.
pub struct LoadEnvironmentOutput {
    pub environment: Environment,
}

/// Loads an environment from disk.
pub struct LoadEnvironmentUseCase<R: EnvironmentRepository> {
    repository: R,
}

impl<R: EnvironmentRepository> LoadEnvironmentUseCase<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(
        &self,
        input: LoadEnvironmentInput
    ) -> Result<LoadEnvironmentOutput, LoadEnvironmentError> {
        let environment = self.repository
            .load(&input.workspace_path, &input.environment_name)
            .await?;

        Ok(LoadEnvironmentOutput { environment })
    }
}
```

### SaveEnvironment Use Case

```rust
// application/src/use_cases/save_environment.rs

use domain::environment::Environment;
use crate::ports::environment_repository::EnvironmentRepository;
use std::path::PathBuf;

/// Errors that can occur when saving an environment.
#[derive(Debug, thiserror::Error)]
pub enum SaveEnvironmentError {
    #[error("Failed to write environment file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Failed to serialize environment: {0}")]
    SerializeError(String),
}

/// Input for the SaveEnvironment use case.
pub struct SaveEnvironmentInput {
    /// Path to the workspace root.
    pub workspace_path: PathBuf,
    /// The environment to save.
    pub environment: Environment,
}

/// Saves an environment to disk.
pub struct SaveEnvironmentUseCase<R: EnvironmentRepository> {
    repository: R,
}

impl<R: EnvironmentRepository> SaveEnvironmentUseCase<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(
        &self,
        input: SaveEnvironmentInput
    ) -> Result<(), SaveEnvironmentError> {
        self.repository
            .save(&input.workspace_path, &input.environment)
            .await?;

        Ok(())
    }
}
```

### SwitchEnvironment Use Case

```rust
// application/src/use_cases/switch_environment.rs

use domain::environment::Environment;
use domain::secrets::SecretsStore;
use domain::resolution::ResolutionContext;
use domain::globals::Globals;
use domain::variable::VariableMap;
use crate::ports::environment_repository::EnvironmentRepository;
use crate::ports::secrets_repository::SecretsRepository;
use std::path::PathBuf;

/// Errors that can occur when switching environments.
#[derive(Debug, thiserror::Error)]
pub enum SwitchEnvironmentError {
    #[error("Environment not found: {0}")]
    NotFound(String),

    #[error("Failed to load environment: {0}")]
    LoadError(String),

    #[error("Failed to load secrets: {0}")]
    SecretsError(String),
}

/// Input for the SwitchEnvironment use case.
pub struct SwitchEnvironmentInput {
    /// Path to the workspace root.
    pub workspace_path: PathBuf,
    /// Name of the environment to switch to.
    pub environment_name: String,
    /// Current globals.
    pub globals: Globals,
    /// Current collection variables.
    pub collection_variables: VariableMap,
}

/// Output containing the new resolution context.
pub struct SwitchEnvironmentOutput {
    /// The loaded environment.
    pub environment: Environment,
    /// The new resolution context ready for use.
    pub resolution_context: ResolutionContext,
}

/// Switches the active environment and creates a new resolution context.
pub struct SwitchEnvironmentUseCase<E: EnvironmentRepository, S: SecretsRepository> {
    environment_repo: E,
    secrets_repo: S,
}

impl<E: EnvironmentRepository, S: SecretsRepository> SwitchEnvironmentUseCase<E, S> {
    pub fn new(environment_repo: E, secrets_repo: S) -> Self {
        Self {
            environment_repo,
            secrets_repo,
        }
    }

    pub async fn execute(
        &self,
        input: SwitchEnvironmentInput,
    ) -> Result<SwitchEnvironmentOutput, SwitchEnvironmentError> {
        // Load the environment
        let environment = self.environment_repo
            .load(&input.workspace_path, &input.environment_name)
            .await
            .map_err(|e| SwitchEnvironmentError::LoadError(e.to_string()))?;

        // Load secrets
        let secrets = self.secrets_repo
            .load(&input.workspace_path)
            .await
            .unwrap_or_default();

        // Create the resolution context
        let resolution_context = ResolutionContext::new(
            &input.globals,
            &input.collection_variables,
            &environment,
            &secrets,
        );

        Ok(SwitchEnvironmentOutput {
            environment,
            resolution_context,
        })
    }
}
```

### Repository Ports

```rust
// application/src/ports/environment_repository.rs

use domain::environment::Environment;
use std::path::Path;
use async_trait::async_trait;

/// Repository for environment persistence.
#[async_trait]
pub trait EnvironmentRepository: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Loads an environment by name.
    async fn load(&self, workspace: &Path, name: &str) -> Result<Environment, Self::Error>;

    /// Saves an environment.
    async fn save(&self, workspace: &Path, environment: &Environment) -> Result<(), Self::Error>;

    /// Lists all available environments.
    async fn list(&self, workspace: &Path) -> Result<Vec<String>, Self::Error>;

    /// Deletes an environment.
    async fn delete(&self, workspace: &Path, name: &str) -> Result<(), Self::Error>;
}

// application/src/ports/secrets_repository.rs

use domain::secrets::SecretsStore;
use std::path::Path;
use async_trait::async_trait;

/// Repository for secrets persistence.
#[async_trait]
pub trait SecretsRepository: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Loads the secrets store.
    async fn load(&self, workspace: &Path) -> Result<SecretsStore, Self::Error>;

    /// Saves the secrets store.
    async fn save(&self, workspace: &Path, secrets: &SecretsStore) -> Result<(), Self::Error>;
}
```

---

## Secrets Management

### Secrets File Structure

Location: `.vortex/secrets.json` (must be in `.gitignore`)

```json
{
  "schema_version": 1,
  "secrets": {
    "development": {
      "api_key": "sk-dev-xxx-xxx-xxx",
      "client_secret": "dev-secret-value"
    },
    "staging": {
      "api_key": "sk-staging-xxx-xxx-xxx",
      "client_secret": "staging-secret-value"
    },
    "production": {
      "api_key": "sk-prod-xxx-xxx-xxx",
      "client_secret": "prod-secret-value"
    }
  }
}
```

### File-based Secrets Repository Implementation

```rust
// infrastructure/src/repositories/file_secrets_repository.rs

use application::ports::secrets_repository::SecretsRepository;
use domain::secrets::SecretsStore;
use std::path::Path;
use async_trait::async_trait;
use tokio::fs;

/// File-based implementation of the secrets repository.
pub struct FileSecretsRepository;

#[derive(Debug, thiserror::Error)]
pub enum FileSecretsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl FileSecretsRepository {
    pub fn new() -> Self {
        Self
    }

    fn secrets_path(workspace: &Path) -> std::path::PathBuf {
        workspace.join(".vortex").join("secrets.json")
    }
}

#[async_trait]
impl SecretsRepository for FileSecretsRepository {
    type Error = FileSecretsError;

    async fn load(&self, workspace: &Path) -> Result<SecretsStore, Self::Error> {
        let path = Self::secrets_path(workspace);

        if !path.exists() {
            return Ok(SecretsStore::default());
        }

        let content = fs::read_to_string(&path).await?;
        let secrets: SecretsStore = serde_json::from_str(&content)?;

        Ok(secrets)
    }

    async fn save(&self, workspace: &Path, secrets: &SecretsStore) -> Result<(), Self::Error> {
        let path = Self::secrets_path(workspace);

        // Ensure .vortex directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(secrets)?;
        fs::write(&path, content).await?;

        Ok(())
    }
}
```

### Optional Keychain Integration (Future Enhancement)

```rust
// infrastructure/src/repositories/keychain_secrets_repository.rs

// NOTE: This is a placeholder for future keychain integration.
// For MVP, use FileSecretsRepository.

#[cfg(target_os = "macos")]
pub mod macos {
    use security_framework::passwords::{get_generic_password, set_generic_password, delete_generic_password};

    const SERVICE_NAME: &str = "com.vortex.secrets";

    pub fn get_secret(environment: &str, key: &str) -> Option<String> {
        let account = format!("{}:{}", environment, key);
        get_generic_password(SERVICE_NAME, &account)
            .ok()
            .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
    }

    pub fn set_secret(environment: &str, key: &str, value: &str) -> Result<(), String> {
        let account = format!("{}:{}", environment, key);
        set_generic_password(SERVICE_NAME, &account, value.as_bytes())
            .map_err(|e| e.to_string())
    }

    pub fn delete_secret(environment: &str, key: &str) -> Result<(), String> {
        let account = format!("{}:{}", environment, key);
        delete_generic_password(SERVICE_NAME, &account)
            .map_err(|e| e.to_string())
    }
}

#[cfg(target_os = "windows")]
pub mod windows {
    // TODO: Implement using Windows Credential Manager
}

#[cfg(target_os = "linux")]
pub mod linux {
    // TODO: Implement using libsecret/Secret Service API
}
```

---

## UI Components

All UI components are implemented in Slint and connected to Rust backend.

### Environment Selector Dropdown

```slint
// ui/components/environment_selector.slint

import { ComboBox, VerticalBox, HorizontalBox } from "std-widgets.slint";

export struct EnvironmentInfo {
    name: string,
    variable_count: int,
    is_active: bool,
}

export component EnvironmentSelector inherits HorizontalBox {
    in property <[EnvironmentInfo]> environments;
    in property <int> selected_index: 0;
    out property <string> selected_name: environments[selected_index].name;

    callback environment_changed(int, string);
    callback edit_environments_clicked();

    spacing: 8px;
    padding: 4px;

    Text {
        text: "Environment:";
        vertical-alignment: center;
        color: #858585;
        font-size: 12px;
    }

    ComboBox {
        model: environments.name;
        current-index <=> selected_index;

        selected(index) => {
            environment_changed(index, environments[index].name);
        }
    }

    Rectangle {
        width: 24px;
        height: 24px;
        border-radius: 4px;
        background: touch-area.has-hover ? #2d2d2d : transparent;

        touch-area := TouchArea {
            clicked => {
                edit_environments_clicked();
            }
        }

        Text {
            text: "\u{2699}"; // gear icon
            font-size: 14px;
            horizontal-alignment: center;
            vertical-alignment: center;
        }
    }
}
```

### Environment Variables Editor

```slint
// ui/components/environment_editor.slint

import { VerticalBox, HorizontalBox, LineEdit, Button, ListView, CheckBox } from "std-widgets.slint";

export struct VariableEntry {
    name: string,
    value: string,
    is_secret: bool,
    is_modified: bool,
}

export component EnvironmentEditor inherits VerticalBox {
    in-out property <[VariableEntry]> variables;
    in property <string> environment_name;
    in property <bool> is_loading: false;

    callback variable_changed(int, string, string, bool);
    callback variable_deleted(int);
    callback add_variable_clicked();
    callback save_clicked();

    spacing: 8px;
    padding: 16px;

    // Header
    HorizontalBox {
        spacing: 8px;

        Text {
            text: "Variables for: " + environment_name;
            font-size: 14px;
            font-weight: 600;
            vertical-alignment: center;
        }

        Rectangle { horizontal-stretch: 1; }

        Button {
            text: "+ Add Variable";
            clicked => { add_variable_clicked(); }
        }

        Button {
            text: "Save";
            enabled: !is_loading;
            clicked => { save_clicked(); }
        }
    }

    // Column headers
    HorizontalBox {
        spacing: 8px;
        height: 32px;

        Text {
            width: 200px;
            text: "Variable";
            font-size: 12px;
            color: #858585;
        }

        Text {
            horizontal-stretch: 1;
            text: "Value";
            font-size: 12px;
            color: #858585;
        }

        Text {
            width: 60px;
            text: "Secret";
            font-size: 12px;
            color: #858585;
            horizontal-alignment: center;
        }

        Rectangle { width: 32px; }
    }

    // Variable list
    ListView {
        for variable[index] in variables: HorizontalBox {
            spacing: 8px;
            height: 36px;

            LineEdit {
                width: 200px;
                text: variable.name;
                placeholder-text: "Variable name";
                edited(text) => {
                    variable_changed(index, text, variable.value, variable.is_secret);
                }
            }

            LineEdit {
                horizontal-stretch: 1;
                text: variable.is_secret ? "********" : variable.value;
                placeholder-text: variable.is_secret ? "Secret value (stored locally)" : "Value";
                input-type: variable.is_secret ? InputType.password : InputType.text;
                edited(text) => {
                    variable_changed(index, variable.name, text, variable.is_secret);
                }
            }

            CheckBox {
                width: 60px;
                checked: variable.is_secret;
                toggled => {
                    variable_changed(index, variable.name, variable.value, self.checked);
                }
            }

            Rectangle {
                width: 32px;
                height: 32px;
                border-radius: 4px;
                background: delete-touch.has-hover ? #3d3d3d : transparent;

                delete-touch := TouchArea {
                    clicked => {
                        variable_deleted(index);
                    }
                }

                Text {
                    text: "\u{00D7}"; // multiplication sign as X
                    font-size: 18px;
                    color: #f14c4c;
                    horizontal-alignment: center;
                    vertical-alignment: center;
                }
            }
        }
    }

    // Secret indicator legend
    HorizontalBox {
        spacing: 4px;
        height: 24px;

        Text {
            text: "\u{1F512}"; // lock emoji
            font-size: 12px;
        }
        Text {
            text: "= Secret (stored in .vortex/secrets.json, not committed to Git)";
            font-size: 11px;
            color: #858585;
        }
    }
}
```

### Variable Preview in URL Bar

```slint
// ui/components/url_bar_with_preview.slint

import { HorizontalBox, VerticalBox, LineEdit, Button, ComboBox } from "std-widgets.slint";

export struct UnresolvedVariable {
    name: string,
    position: int,
}

export component UrlBarWithPreview inherits VerticalBox {
    in-out property <string> url;
    in property <string> resolved_url;
    in property <[UnresolvedVariable]> unresolved_variables;
    in property <string> method: "GET";
    in property <[string]> methods: ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
    in property <bool> is_loading: false;
    in property <bool> show_preview: true;

    callback method_changed(string);
    callback url_changed(string);
    callback send_clicked();

    spacing: 4px;

    // Main URL bar
    HorizontalBox {
        spacing: 8px;
        height: 40px;

        ComboBox {
            width: 100px;
            model: methods;
            current-value: method;
            selected(value) => {
                method_changed(value);
            }
        }

        LineEdit {
            horizontal-stretch: 1;
            text <=> url;
            placeholder-text: "Enter request URL with {{variables}}";
            font-family: "JetBrains Mono";
            edited(text) => {
                url_changed(text);
            }
        }

        Button {
            text: is_loading ? "Cancel" : "Send";
            primary: !is_loading;
            enabled: url != "";
            clicked => {
                send_clicked();
            }
        }
    }

    // Resolved URL preview
    if show_preview && resolved_url != url: HorizontalBox {
        spacing: 8px;
        height: 28px;

        Text {
            text: "Preview:";
            font-size: 11px;
            color: #858585;
            vertical-alignment: center;
        }

        Text {
            horizontal-stretch: 1;
            text: resolved_url;
            font-size: 11px;
            font-family: "JetBrains Mono";
            color: #4ec9b0;
            overflow: elide;
            vertical-alignment: center;
        }
    }

    // Unresolved variable warnings
    if unresolved_variables.length > 0: HorizontalBox {
        spacing: 8px;
        height: 28px;

        Rectangle {
            width: 16px;
            height: 16px;
            border-radius: 8px;
            background: #ce9178;

            Text {
                text: "!";
                font-size: 12px;
                font-weight: 700;
                color: white;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }

        Text {
            horizontal-stretch: 1;
            text: "Unresolved: " + unresolved_variables.name;
            font-size: 11px;
            color: #ce9178;
            vertical-alignment: center;
        }
    }
}
```

### Unresolved Variable Warning Component

```slint
// ui/components/unresolved_warning.slint

import { VerticalBox, HorizontalBox, Button } from "std-widgets.slint";

export component UnresolvedWarning inherits Rectangle {
    in property <[string]> unresolved_variables;
    in property <string> context: "request"; // "url", "headers", "body"

    callback define_variable_clicked(string);
    callback dismiss_clicked();

    visible: unresolved_variables.length > 0;
    height: visible ? content.preferred-height + 16px : 0px;
    background: #3d2c1c;
    border-radius: 4px;

    content := VerticalBox {
        padding: 8px;
        spacing: 8px;

        HorizontalBox {
            spacing: 8px;

            Text {
                text: "\u{26A0}"; // warning sign
                font-size: 14px;
                color: #ce9178;
            }

            Text {
                horizontal-stretch: 1;
                text: unresolved_variables.length == 1
                    ? "1 unresolved variable in " + context
                    : unresolved_variables.length + " unresolved variables in " + context;
                font-size: 12px;
                color: #ce9178;
                font-weight: 500;
            }

            TouchArea {
                width: 20px;
                height: 20px;
                clicked => { dismiss_clicked(); }

                Text {
                    text: "\u{00D7}";
                    font-size: 16px;
                    color: #858585;
                    horizontal-alignment: center;
                    vertical-alignment: center;
                }
            }
        }

        for variable in unresolved_variables: HorizontalBox {
            spacing: 8px;
            height: 28px;

            Text {
                text: "{{" + variable + "}}";
                font-family: "JetBrains Mono";
                font-size: 12px;
                color: #dcdcaa;
                vertical-alignment: center;
            }

            Rectangle { horizontal-stretch: 1; }

            Button {
                text: "Define";
                clicked => { define_variable_clicked(variable); }
            }
        }
    }
}
```

### Manage Environments Dialog

```slint
// ui/dialogs/manage_environments_dialog.slint

import { Dialog, VerticalBox, HorizontalBox, ListView, Button, LineEdit } from "std-widgets.slint";
import { EnvironmentEditor, VariableEntry } from "../components/environment_editor.slint";

export struct EnvironmentListItem {
    name: string,
    variable_count: int,
    is_selected: bool,
}

export component ManageEnvironmentsDialog inherits Dialog {
    in property <[EnvironmentListItem]> environments;
    in-out property <int> selected_environment_index: 0;
    in-out property <[VariableEntry]> current_variables;
    in property <string> selected_environment_name:
        environments.length > 0 ? environments[selected_environment_index].name : "";

    callback environment_selected(int);
    callback create_environment_clicked();
    callback delete_environment_clicked(int);
    callback rename_environment_clicked(int, string);
    callback variable_changed(int, string, string, bool);
    callback variable_deleted(int);
    callback add_variable_clicked();
    callback save_clicked();
    callback close_clicked();

    title: "Manage Environments";
    width: 800px;
    height: 600px;

    HorizontalBox {
        spacing: 16px;
        padding: 16px;

        // Environment list (left panel)
        VerticalBox {
            width: 200px;
            spacing: 8px;

            Text {
                text: "Environments";
                font-size: 14px;
                font-weight: 600;
            }

            Button {
                text: "+ New Environment";
                clicked => { create_environment_clicked(); }
            }

            ListView {
                vertical-stretch: 1;

                for env[index] in environments: Rectangle {
                    height: 40px;
                    background: env.is_selected ? #094771 : (touch.has-hover ? #2d2d2d : transparent);
                    border-radius: 4px;

                    touch := TouchArea {
                        clicked => {
                            environment_selected(index);
                        }
                    }

                    HorizontalBox {
                        padding: 8px;
                        spacing: 8px;

                        VerticalBox {
                            horizontal-stretch: 1;

                            Text {
                                text: env.name;
                                font-size: 13px;
                                font-weight: env.is_selected ? 600 : 400;
                            }

                            Text {
                                text: env.variable_count + " variables";
                                font-size: 11px;
                                color: #858585;
                            }
                        }

                        if env.is_selected: TouchArea {
                            width: 24px;
                            clicked => { delete_environment_clicked(index); }

                            Text {
                                text: "\u{1F5D1}"; // wastebasket
                                font-size: 14px;
                                horizontal-alignment: center;
                                vertical-alignment: center;
                            }
                        }
                    }
                }
            }
        }

        // Separator
        Rectangle {
            width: 1px;
            background: #3d3d3d;
        }

        // Variables editor (right panel)
        EnvironmentEditor {
            horizontal-stretch: 1;
            environment_name: selected_environment_name;
            variables <=> current_variables;

            variable_changed(idx, name, value, secret) => {
                variable_changed(idx, name, value, secret);
            }
            variable_deleted(idx) => { variable_deleted(idx); }
            add_variable_clicked => { add_variable_clicked(); }
            save_clicked => { save_clicked(); }
        }
    }

    // Footer
    HorizontalBox {
        padding: 16px;
        spacing: 8px;

        Rectangle { horizontal-stretch: 1; }

        Button {
            text: "Close";
            clicked => { close_clicked(); }
        }
    }
}
```

### UI State Model

```rust
// ui/src/state/environment_state.rs

use slint::{Model, ModelRc, VecModel, SharedString};
use std::rc::Rc;
use domain::environment::Environment;
use domain::variable::Variable;

/// UI state for environment management.
#[derive(Clone)]
pub struct EnvironmentState {
    /// List of available environments.
    environments: Rc<VecModel<EnvironmentInfo>>,
    /// Currently selected environment index.
    selected_index: i32,
    /// Variables for the selected environment.
    variables: Rc<VecModel<VariableEntry>>,
    /// Whether data is being loaded/saved.
    is_loading: bool,
    /// Whether there are unsaved changes.
    has_changes: bool,
}

#[derive(Clone)]
pub struct EnvironmentInfo {
    pub name: SharedString,
    pub variable_count: i32,
    pub is_active: bool,
}

#[derive(Clone)]
pub struct VariableEntry {
    pub name: SharedString,
    pub value: SharedString,
    pub is_secret: bool,
    pub is_modified: bool,
}

impl EnvironmentState {
    pub fn new() -> Self {
        Self {
            environments: Rc::new(VecModel::default()),
            selected_index: 0,
            variables: Rc::new(VecModel::default()),
            is_loading: false,
            has_changes: false,
        }
    }

    /// Updates the environment list from domain models.
    pub fn set_environments(&mut self, envs: Vec<Environment>, active_name: &str) {
        let items: Vec<EnvironmentInfo> = envs
            .iter()
            .map(|e| EnvironmentInfo {
                name: SharedString::from(&e.name),
                variable_count: e.variable_count() as i32,
                is_active: e.name == active_name,
            })
            .collect();

        self.environments = Rc::new(VecModel::from(items));
    }

    /// Sets the variables for display/editing.
    pub fn set_variables(&mut self, vars: &std::collections::HashMap<String, Variable>) {
        let items: Vec<VariableEntry> = vars
            .iter()
            .map(|(name, var)| VariableEntry {
                name: SharedString::from(name.as_str()),
                value: SharedString::from(&var.value),
                is_secret: var.secret,
                is_modified: false,
            })
            .collect();

        self.variables = Rc::new(VecModel::from(items));
        self.has_changes = false;
    }

    /// Gets the environments model for Slint binding.
    pub fn environments_model(&self) -> ModelRc<EnvironmentInfo> {
        self.environments.clone().into()
    }

    /// Gets the variables model for Slint binding.
    pub fn variables_model(&self) -> ModelRc<VariableEntry> {
        self.variables.clone().into()
    }

    /// Marks a variable as modified.
    pub fn mark_variable_modified(&mut self, index: usize) {
        if let Some(mut entry) = self.variables.row_data(index) {
            entry.is_modified = true;
            self.variables.set_row_data(index, entry);
            self.has_changes = true;
        }
    }

    /// Adds a new empty variable.
    pub fn add_variable(&mut self) {
        self.variables.push(VariableEntry {
            name: SharedString::from("new_variable"),
            value: SharedString::new(),
            is_secret: false,
            is_modified: true,
        });
        self.has_changes = true;
    }

    /// Removes a variable by index.
    pub fn remove_variable(&mut self, index: usize) {
        self.variables.remove(index);
        self.has_changes = true;
    }
}
```

---

## File Persistence

### Environment Repository Implementation

```rust
// infrastructure/src/repositories/file_environment_repository.rs

use application::ports::environment_repository::EnvironmentRepository;
use domain::environment::Environment;
use std::path::Path;
use async_trait::async_trait;
use tokio::fs;

pub struct FileEnvironmentRepository;

#[derive(Debug, thiserror::Error)]
pub enum FileEnvironmentError {
    #[error("Environment not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl FileEnvironmentRepository {
    pub fn new() -> Self {
        Self
    }

    fn environments_dir(workspace: &Path) -> std::path::PathBuf {
        workspace.join("environments")
    }

    fn environment_path(workspace: &Path, name: &str) -> std::path::PathBuf {
        Self::environments_dir(workspace).join(format!("{}.json", name.to_lowercase()))
    }
}

#[async_trait]
impl EnvironmentRepository for FileEnvironmentRepository {
    type Error = FileEnvironmentError;

    async fn load(&self, workspace: &Path, name: &str) -> Result<Environment, Self::Error> {
        let path = Self::environment_path(workspace, name);

        if !path.exists() {
            return Err(FileEnvironmentError::NotFound(name.to_string()));
        }

        let content = fs::read_to_string(&path).await?;
        let environment: Environment = serde_json::from_str(&content)?;

        Ok(environment)
    }

    async fn save(&self, workspace: &Path, environment: &Environment) -> Result<(), Self::Error> {
        let path = Self::environment_path(workspace, &environment.name);

        // Ensure environments directory exists
        let dir = Self::environments_dir(workspace);
        fs::create_dir_all(&dir).await?;

        // Serialize with deterministic ordering
        let content = serialize_deterministic(environment)?;
        fs::write(&path, content).await?;

        Ok(())
    }

    async fn list(&self, workspace: &Path) -> Result<Vec<String>, Self::Error> {
        let dir = Self::environments_dir(workspace);

        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut environments = Vec::new();
        let mut entries = fs::read_dir(&dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    environments.push(name.to_string());
                }
            }
        }

        environments.sort();
        Ok(environments)
    }

    async fn delete(&self, workspace: &Path, name: &str) -> Result<(), Self::Error> {
        let path = Self::environment_path(workspace, name);

        if !path.exists() {
            return Err(FileEnvironmentError::NotFound(name.to_string()));
        }

        fs::remove_file(&path).await?;
        Ok(())
    }
}

/// Serializes with deterministic field ordering for clean diffs.
fn serialize_deterministic<T: serde::Serialize>(value: &T) -> Result<String, serde_json::Error> {
    use serde_json::ser::{PrettyFormatter, Serializer};

    let mut buf = Vec::new();
    let formatter = PrettyFormatter::with_indent(b"  ");
    let mut ser = Serializer::with_formatter(&mut buf, formatter);
    value.serialize(&mut ser)?;

    let mut s = String::from_utf8(buf).expect("JSON is valid UTF-8");
    s.push('\n'); // trailing newline

    Ok(s)
}
```

---

## Implementation Order

Execute these steps in sequence. Each step should pass tests before proceeding.

### Phase 1: Domain Models (Days 1-2)

**Step 1.1: Create Variable Types**
- Create `domain/src/variable.rs`
- Implement `Variable` struct with `value` and `secret` fields
- Implement `VariableScope` enum with precedence
- Implement `ResolvedVariable` struct
- Add unit tests

**Step 1.2: Create Environment Model**
- Create `domain/src/environment.rs`
- Implement `Environment` struct with UUID, name, variables
- Add methods for variable manipulation
- Add unit tests

**Step 1.3: Create Supporting Models**
- Create `domain/src/globals.rs` for `Globals` struct
- Create `domain/src/secrets.rs` for `SecretsStore` struct
- Create `domain/src/resolution.rs` for `ResolutionContext`
- Add unit tests for each

### Phase 2: Variable Resolution Engine (Days 3-5)

**Step 2.1: Create Variable Parser**
- Create `application/src/variable_resolver/parser.rs`
- Implement `parse_variables()` function
- Handle edge cases (unclosed, empty, whitespace)
- Add comprehensive unit tests

**Step 2.2: Create Built-in Variables Generator**
- Create `application/src/variable_resolver/builtins.rs`
- Implement `BuiltinVariables` with all dynamic variables
- Add documentation and tests

**Step 2.3: Create Resolution Engine**
- Create `application/src/variable_resolver/engine.rs`
- Implement `VariableResolver` with caching
- Implement precedence rules
- Add integration tests covering all scenarios

### Phase 3: Use Cases (Days 6-8)

**Step 3.1: Create Repository Ports**
- Create `application/src/ports/environment_repository.rs`
- Create `application/src/ports/secrets_repository.rs`
- Define async traits with error types

**Step 3.2: Implement LoadEnvironment**
- Create `application/src/use_cases/load_environment.rs`
- Implement use case with repository dependency
- Add tests with mock repository

**Step 3.3: Implement SaveEnvironment**
- Create `application/src/use_cases/save_environment.rs`
- Implement use case
- Add tests

**Step 3.4: Implement SwitchEnvironment**
- Create `application/src/use_cases/switch_environment.rs`
- Implement environment + secrets loading
- Create resolution context
- Add tests

**Step 3.5: Implement ResolveVariables**
- Create `application/src/use_cases/resolve_variables.rs`
- Resolve URL, headers, body, auth
- Track unresolved variables
- Add comprehensive tests

### Phase 4: Infrastructure (Days 9-10)

**Step 4.1: Implement File Environment Repository**
- Create `infrastructure/src/repositories/file_environment_repository.rs`
- Implement CRUD operations
- Use deterministic serialization
- Add integration tests with temp files

**Step 4.2: Implement File Secrets Repository**
- Create `infrastructure/src/repositories/file_secrets_repository.rs`
- Handle missing secrets file gracefully
- Add integration tests

### Phase 5: UI Components (Days 11-14)

**Step 5.1: Create Environment Selector**
- Create `ui/components/environment_selector.slint`
- Implement dropdown with environment list
- Connect to Rust state

**Step 5.2: Create URL Bar with Preview**
- Create `ui/components/url_bar_with_preview.slint`
- Show resolved URL preview
- Show unresolved variable warnings
- Style variable highlighting

**Step 5.3: Create Environment Editor**
- Create `ui/components/environment_editor.slint`
- Implement variable list with add/edit/delete
- Handle secret toggle
- Create `ui/dialogs/manage_environments_dialog.slint`

**Step 5.4: Create Unresolved Warning Component**
- Create `ui/components/unresolved_warning.slint`
- Show actionable warnings
- "Define" button integration

**Step 5.5: Integrate UI State**
- Create `ui/src/state/environment_state.rs`
- Connect Slint models to domain types
- Wire up callbacks to use cases

### Phase 6: Integration and Testing (Days 15-16)

**Step 6.1: End-to-End Integration**
- Connect all layers
- Test complete flow: load workspace -> select environment -> resolve request
- Test environment switching

**Step 6.2: Edge Case Testing**
- Empty environment
- Missing secrets file
- Circular variable references (should not occur, but verify)
- Invalid variable names
- Very long variable values

---

## Acceptance Criteria

### Functional Requirements

- [ ] User can create, edit, and delete environments
- [ ] User can define variables with name/value pairs
- [ ] User can mark variables as secrets
- [ ] Secret values are stored in `.vortex/secrets.json`, not in environment files
- [ ] User can switch between environments via dropdown
- [ ] Switching environments updates variable resolution immediately
- [ ] Variables in URL bar show resolved preview
- [ ] Unresolved variables are highlighted with warning
- [ ] Built-in variables (`$uuid`, `$timestamp`, etc.) generate fresh values per request
- [ ] Variable resolution follows correct precedence order

### Non-Functional Requirements

- [ ] Environment switch completes in < 50ms
- [ ] Variable resolution for typical request completes in < 10ms
- [ ] UI remains responsive during file I/O (async operations)
- [ ] No secret values appear in environment JSON files
- [ ] Files are serialized deterministically (stable diffs)

### Test Coverage

- [ ] Unit tests for all domain models
- [ ] Unit tests for variable parser (including edge cases)
- [ ] Unit tests for resolution engine with all precedence scenarios
- [ ] Integration tests for repository implementations
- [ ] UI component tests (if Slint testing is available)

---

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Variable injection in URLs causing malformed requests | High | Validate resolved URLs before sending; escape special characters |
| Secret values accidentally logged or displayed | High | Never log secret values; mask in UI; code review |
| Large number of variables causing slow resolution | Medium | Use HashMap for O(1) lookup; cache resolved values |
| Concurrent file access (multiple Vortex instances) | Medium | Use file locking; last-write-wins for MVP |
| Invalid JSON in environment files | Low | Validate on load; show user-friendly error |
| Circular variable references | Low | Variables cannot reference other variables in MVP (future feature) |

---

## Dependencies

### Rust Crates

```toml
# domain/Cargo.toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
uuid = { version = "1.0", features = ["v4", "serde"] }

# application/Cargo.toml
[dependencies]
domain = { path = "../domain" }
async-trait = "0.1"
thiserror = "1.0"
chrono = { version = "0.4", features = ["serde"] }
rand = "0.8"
uuid = { version = "1.0", features = ["v4"] }

# infrastructure/Cargo.toml
[dependencies]
domain = { path = "../domain" }
application = { path = "../application" }
tokio = { version = "1.0", features = ["fs", "sync"] }
serde_json = "1.0"
async-trait = "0.1"
thiserror = "1.0"

# ui/Cargo.toml
[dependencies]
slint = "1.0"
domain = { path = "../domain" }
application = { path = "../application" }
```

---

## Related Documents

- [02-file-format-spec.md](./02-file-format-spec.md) - File format specifications
- [03-ui-ux-specification.md](./03-ui-ux-specification.md) - UI/UX design guidelines
- Sprint 02 - Basic request execution (prerequisite)
- Sprint 04 - OAuth and authentication (next)
