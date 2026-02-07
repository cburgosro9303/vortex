//! Variable resolution engine
//!
//! Resolves `{{variable}}` references according to precedence rules.

use std::collections::HashMap;

use vortex_domain::environment::{ResolutionContext, ResolvedVariable, VariableScope};

use super::builtins::BuiltinVariables;
use super::parser::parse_variables;

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

impl ResolutionResult {
    /// Creates a result for input with no variables.
    #[must_use]
    pub fn no_variables(input: &str) -> Self {
        Self {
            resolved: input.to_string(),
            resolved_variables: Vec::new(),
            unresolved: Vec::new(),
            is_complete: true,
        }
    }

    /// Returns the count of resolved variables.
    #[must_use]
    pub fn resolved_count(&self) -> usize {
        self.resolved_variables.len()
    }

    /// Returns the count of unresolved variables.
    #[must_use]
    pub fn unresolved_count(&self) -> usize {
        self.unresolved.len()
    }
}

/// The variable resolution engine.
/// Resolves `{{variable}}` references according to precedence rules.
pub struct VariableResolver {
    context: ResolutionContext,
    /// Cache for built-in variables to ensure consistency within a single resolution session.
    builtin_cache: HashMap<String, String>,
}

impl VariableResolver {
    /// Creates a new resolver with the given context.
    #[must_use]
    pub fn new(context: ResolutionContext) -> Self {
        Self {
            context,
            builtin_cache: HashMap::new(),
        }
    }

    /// Creates a new resolver with an empty context.
    #[must_use]
    pub fn empty() -> Self {
        Self::new(ResolutionContext::new())
    }

    /// Updates the resolution context.
    pub fn set_context(&mut self, context: ResolutionContext) {
        self.context = context;
        self.builtin_cache.clear();
    }

    /// Returns a reference to the current context.
    #[must_use]
    pub fn context(&self) -> &ResolutionContext {
        &self.context
    }

    /// Returns a mutable reference to the current context.
    pub fn context_mut(&mut self) -> &mut ResolutionContext {
        &mut self.context
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
            return ResolutionResult::no_variables(input);
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
    #[must_use]
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
    #[must_use]
    pub fn extract_variable_names(input: &str) -> Vec<String> {
        parse_variables(input).into_iter().map(|r| r.name).collect()
    }

    /// Resolves just the value of a single variable (without full result info).
    #[must_use]
    pub fn resolve_value(&mut self, name: &str) -> Option<String> {
        self.resolve_variable(name).map(|r| r.value)
    }

    /// Preview resolution without caching built-ins.
    /// Useful for showing preview in UI without affecting actual resolution.
    #[must_use]
    pub fn preview(&self, input: &str) -> ResolutionResult {
        let references = parse_variables(input);

        if references.is_empty() {
            return ResolutionResult::no_variables(input);
        }

        let mut resolved_vars = Vec::new();
        let mut unresolved = Vec::new();
        let mut result = String::with_capacity(input.len());
        let mut last_end = 0;

        for var_ref in &references {
            result.push_str(&input[last_end..var_ref.span.start]);

            // For preview, we still generate built-ins but don't cache them
            let resolved = if var_ref.is_builtin {
                BuiltinVariables::resolve(&var_ref.name).map(|value| ResolvedVariable {
                    name: var_ref.name.clone(),
                    value,
                    scope: VariableScope::BuiltIn,
                })
            } else {
                self.context.resolve(&var_ref.name)
            };

            if let Some(r) = resolved {
                result.push_str(&r.value);
                resolved_vars.push(r);
            } else {
                result.push_str(&input[var_ref.span.clone()]);
                unresolved.push(var_ref.name.clone());
            }

            last_end = var_ref.span.end;
        }

        result.push_str(&input[last_end..]);

        ResolutionResult {
            resolved: result,
            resolved_variables: resolved_vars,
            unresolved: unresolved.clone(),
            is_complete: unresolved.is_empty(),
        }
    }
}

impl Default for VariableResolver {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vortex_domain::environment::{Environment, Globals, SecretsStore, Variable, VariableMap};

    fn create_test_context() -> ResolutionContext {
        let mut globals = Globals::new();
        globals.set_variable("app_name", Variable::new("TestApp"));

        let mut collection_vars = VariableMap::new();
        collection_vars.insert("base_path".to_string(), Variable::new("/api/v1"));

        let mut env = Environment::new("development");
        env.set_variable("base_url", Variable::new("http://localhost:3000"));
        env.set_variable("api_key", Variable::secret(""));

        let mut secrets = SecretsStore::new();
        secrets.set_secret("development", "api_key", "sk-secret-123");

        ResolutionContext::from_sources(&globals, &collection_vars, &env, &secrets)
    }

    #[test]
    fn test_resolve_no_variables() {
        let context = create_test_context();
        let mut resolver = VariableResolver::new(context);

        let result = resolver.resolve("Hello, World!");
        assert_eq!(result.resolved, "Hello, World!");
        assert!(result.is_complete);
        assert!(result.resolved_variables.is_empty());
        assert!(result.unresolved.is_empty());
    }

    #[test]
    fn test_resolve_user_variable() {
        let context = create_test_context();
        let mut resolver = VariableResolver::new(context);

        let result = resolver.resolve("{{base_url}}/users");
        assert_eq!(result.resolved, "http://localhost:3000/users");
        assert!(result.is_complete);
        assert_eq!(result.resolved_count(), 1);
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
        assert_eq!(result.resolved_variables[0].scope, VariableScope::BuiltIn);
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

    #[test]
    fn test_mixed_resolved_unresolved() {
        let context = create_test_context();
        let mut resolver = VariableResolver::new(context);

        let result = resolver.resolve("{{base_url}}/{{unknown}}/users");
        assert_eq!(result.resolved, "http://localhost:3000/{{unknown}}/users");
        assert!(!result.is_complete);
        assert_eq!(result.resolved_count(), 1);
        assert_eq!(result.unresolved_count(), 1);
    }

    #[test]
    fn test_builtin_cache_consistency() {
        let context = create_test_context();
        let mut resolver = VariableResolver::new(context);

        // Same built-in variable should return same value within one session
        let result1 = resolver.resolve("{{$uuid}}");
        let result2 = resolver.resolve("{{$uuid}}");

        assert_eq!(result1.resolved, result2.resolved);
    }

    #[test]
    fn test_clear_builtin_cache() {
        let context = create_test_context();
        let mut resolver = VariableResolver::new(context);

        let result1 = resolver.resolve("{{$uuid}}");
        resolver.clear_builtin_cache();
        let result2 = resolver.resolve("{{$uuid}}");

        // After clearing cache, new UUID should be generated (very unlikely to be same)
        assert_ne!(result1.resolved, result2.resolved);
    }

    #[test]
    fn test_find_unresolved() {
        let context = create_test_context();
        let resolver = VariableResolver::new(context);

        let unresolved = resolver.find_unresolved("{{base_url}}/{{unknown}}/{{$uuid}}");
        assert_eq!(unresolved, vec!["unknown"]);
    }

    #[test]
    fn test_extract_variable_names() {
        let names = VariableResolver::extract_variable_names("{{a}} and {{b}} and {{$uuid}}");
        assert_eq!(names, vec!["a", "b", "$uuid"]);
    }

    #[test]
    fn test_resolve_value() {
        let context = create_test_context();
        let mut resolver = VariableResolver::new(context);

        assert_eq!(
            resolver.resolve_value("base_url"),
            Some("http://localhost:3000".to_string())
        );
        assert_eq!(resolver.resolve_value("unknown"), None);
    }

    #[test]
    fn test_set_context() {
        let mut resolver = VariableResolver::empty();

        // Initially empty
        let result = resolver.resolve("{{base_url}}");
        assert!(!result.is_complete);

        // After setting context
        resolver.set_context(create_test_context());
        let result = resolver.resolve("{{base_url}}");
        assert!(result.is_complete);
    }

    #[test]
    fn test_preview() {
        let context = create_test_context();
        let resolver = VariableResolver::new(context);

        let preview1 = resolver.preview("{{$uuid}}");
        let preview2 = resolver.preview("{{$uuid}}");

        // Preview doesn't cache, so UUIDs should be different
        // (though technically there's a tiny chance they're the same)
        assert!(preview1.is_complete);
        assert!(preview2.is_complete);
    }

    #[test]
    fn test_url_with_variables() {
        let context = create_test_context();
        let mut resolver = VariableResolver::new(context);

        let result =
            resolver.resolve("{{base_url}}{{base_path}}/users?app={{app_name}}&key={{api_key}}");
        assert_eq!(
            result.resolved,
            "http://localhost:3000/api/v1/users?app=TestApp&key=sk-secret-123"
        );
        assert!(result.is_complete);
    }

    #[test]
    fn test_json_body_with_variables() {
        let context = create_test_context();
        let mut resolver = VariableResolver::new(context);

        let result = resolver.resolve(r#"{"app": "{{app_name}}", "url": "{{base_url}}"}"#);
        assert_eq!(
            result.resolved,
            r#"{"app": "TestApp", "url": "http://localhost:3000"}"#
        );
    }
}
