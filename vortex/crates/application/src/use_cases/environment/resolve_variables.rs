//! Resolve variables use case

use vortex_domain::environment::ResolutionContext;
use vortex_domain::request::RequestSpec;

use crate::variable_resolver::{ResolutionResult, VariableResolver};

/// Output containing the resolved request and resolution details.
pub struct ResolveVariablesOutput {
    /// The request with all variables resolved.
    pub resolved_request: RequestSpec,
    /// URL resolution result.
    pub url_result: ResolutionResult,
    /// Whether all variables were resolved.
    pub is_complete: bool,
    /// All unresolved variable names across the request.
    pub all_unresolved: Vec<String>,
}

/// Resolves all variables in a request before execution.
pub struct ResolveVariables {
    resolver: VariableResolver,
}

impl ResolveVariables {
    /// Creates a new `ResolveVariables` use case.
    pub fn new(context: ResolutionContext) -> Self {
        Self {
            resolver: VariableResolver::new(context),
        }
    }

    /// Creates with an empty context (for testing).
    pub fn empty() -> Self {
        Self {
            resolver: VariableResolver::empty(),
        }
    }

    /// Updates the resolution context.
    pub fn set_context(&mut self, context: ResolutionContext) {
        self.resolver.set_context(context);
    }

    /// Returns a reference to the internal resolver.
    pub fn resolver(&self) -> &VariableResolver {
        &self.resolver
    }

    /// Returns a mutable reference to the internal resolver.
    pub fn resolver_mut(&mut self) -> &mut VariableResolver {
        &mut self.resolver
    }

    /// Executes the use case, resolving all variables in the request.
    ///
    /// # Arguments
    /// * `request` - The request to resolve
    ///
    /// # Returns
    /// The resolved request and metadata about the resolution.
    pub fn execute(&mut self, request: &RequestSpec) -> ResolveVariablesOutput {
        // Clear built-in cache for fresh values
        self.resolver.clear_builtin_cache();

        let mut resolved_request = request.clone();
        let mut all_unresolved = Vec::new();

        // Resolve URL
        let url_result = self.resolver.resolve(&request.url);
        resolved_request.url = url_result.resolved.clone();
        all_unresolved.extend(url_result.unresolved.clone());

        // Resolve headers
        let mut resolved_headers = vortex_domain::request::Headers::new();
        for header in request.headers.all() {
            let name_result = self.resolver.resolve(&header.name);
            let value_result = self.resolver.resolve(&header.value);

            all_unresolved.extend(name_result.unresolved);
            all_unresolved.extend(value_result.unresolved);

            let mut resolved_header = vortex_domain::request::Header::new(
                name_result.resolved,
                value_result.resolved,
            );
            resolved_header.enabled = header.enabled;
            resolved_headers.add(resolved_header);
        }
        resolved_request.headers = resolved_headers;

        // Resolve query params
        let mut resolved_params = vortex_domain::request::QueryParams::new();
        for param in request.query_params.all() {
            let key_result = self.resolver.resolve(&param.key);
            let value_result = self.resolver.resolve(&param.value);

            all_unresolved.extend(key_result.unresolved);
            all_unresolved.extend(value_result.unresolved);

            let mut resolved_param = vortex_domain::request::QueryParam::new(
                key_result.resolved,
                value_result.resolved,
            );
            resolved_param.enabled = param.enabled;
            resolved_params.add(resolved_param);
        }
        resolved_request.query_params = resolved_params;

        // Resolve body
        if !request.body.is_empty() {
            let body_result = self.resolver.resolve(&request.body.content);
            all_unresolved.extend(body_result.unresolved);

            resolved_request.body = vortex_domain::request::RequestBody {
                kind: request.body.kind.clone(),
                content: body_result.resolved,
            };
        }

        // Resolve auth
        resolved_request.auth = self.resolve_auth(&request.auth, &mut all_unresolved);

        // Remove duplicates from unresolved list
        all_unresolved.sort();
        all_unresolved.dedup();

        let is_complete = all_unresolved.is_empty();

        ResolveVariablesOutput {
            resolved_request,
            url_result,
            is_complete,
            all_unresolved,
        }
    }

    /// Resolves variables in auth configuration.
    fn resolve_auth(
        &mut self,
        auth: &vortex_domain::auth::AuthConfig,
        unresolved: &mut Vec<String>,
    ) -> vortex_domain::auth::AuthConfig {
        use vortex_domain::auth::AuthConfig;

        match auth {
            AuthConfig::None => AuthConfig::None,
            AuthConfig::ApiKey {
                key,
                name,
                location,
            } => {
                let key_result = self.resolver.resolve(key);
                let name_result = self.resolver.resolve(name);
                unresolved.extend(key_result.unresolved);
                unresolved.extend(name_result.unresolved);

                AuthConfig::ApiKey {
                    key: key_result.resolved,
                    name: name_result.resolved,
                    location: *location,
                }
            }
            AuthConfig::Bearer { token } => {
                let token_result = self.resolver.resolve(token);
                unresolved.extend(token_result.unresolved);

                AuthConfig::Bearer {
                    token: token_result.resolved,
                }
            }
            AuthConfig::Basic { username, password } => {
                let user_result = self.resolver.resolve(username);
                let pass_result = self.resolver.resolve(password);
                unresolved.extend(user_result.unresolved);
                unresolved.extend(pass_result.unresolved);

                AuthConfig::Basic {
                    username: user_result.resolved,
                    password: pass_result.resolved,
                }
            }
        }
    }

    /// Preview resolution for the URL only (for UI display).
    pub fn preview_url(&self, url: &str) -> ResolutionResult {
        self.resolver.preview(url)
    }

    /// Find unresolved variables in a string.
    pub fn find_unresolved(&self, input: &str) -> Vec<String> {
        self.resolver.find_unresolved(input)
    }
}

impl Default for ResolveVariables {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vortex_domain::auth::AuthConfig;
    use vortex_domain::environment::{Environment, Globals, SecretsStore, Variable};
    use vortex_domain::request::RequestBody;

    fn create_test_context() -> ResolutionContext {
        let mut globals = Globals::new();
        globals.set_variable("app_name", Variable::new("TestApp"));

        let mut env = Environment::new("development");
        env.set_variable("base_url", Variable::new("http://localhost:3000"));
        env.set_variable("api_key", Variable::secret(""));
        env.set_variable("version", Variable::new("v1"));

        let mut secrets = SecretsStore::new();
        secrets.set_secret("development", "api_key", "sk-secret-123");

        ResolutionContext::from_sources(
            &globals,
            &std::collections::HashMap::new(),
            &env,
            &secrets,
        )
    }

    #[test]
    fn test_resolve_simple_url() {
        let context = create_test_context();
        let mut use_case = ResolveVariables::new(context);

        let request = RequestSpec::get("{{base_url}}/api/{{version}}/users");
        let output = use_case.execute(&request);

        assert!(output.is_complete);
        assert_eq!(output.resolved_request.url, "http://localhost:3000/api/v1/users");
    }

    #[test]
    fn test_resolve_with_unresolved() {
        let context = create_test_context();
        let mut use_case = ResolveVariables::new(context);

        let request = RequestSpec::get("{{base_url}}/api/{{unknown}}");
        let output = use_case.execute(&request);

        assert!(!output.is_complete);
        assert!(output.all_unresolved.contains(&"unknown".to_string()));
        assert_eq!(output.resolved_request.url, "http://localhost:3000/api/{{unknown}}");
    }

    #[test]
    fn test_resolve_headers() {
        let context = create_test_context();
        let mut use_case = ResolveVariables::new(context);

        let request = RequestSpec::get("{{base_url}}/api")
            .with_header("Authorization", "Bearer {{api_key}}")
            .with_header("X-App-Name", "{{app_name}}");

        let output = use_case.execute(&request);

        assert!(output.is_complete);

        let auth_header = output
            .resolved_request
            .headers
            .all()
            .iter()
            .find(|h| h.name == "Authorization");
        assert_eq!(
            auth_header.map(|h| h.value.as_str()),
            Some("Bearer sk-secret-123")
        );

        let app_header = output
            .resolved_request
            .headers
            .all()
            .iter()
            .find(|h| h.name == "X-App-Name");
        assert_eq!(app_header.map(|h| h.value.as_str()), Some("TestApp"));
    }

    #[test]
    fn test_resolve_query_params() {
        let context = create_test_context();
        let mut use_case = ResolveVariables::new(context);

        let request = RequestSpec::get("{{base_url}}/api")
            .with_query("app", "{{app_name}}")
            .with_query("version", "{{version}}");

        let output = use_case.execute(&request);

        assert!(output.is_complete);

        let params: Vec<_> = output.resolved_request.query_params.all().iter().collect();
        assert!(params.iter().any(|p| p.key == "app" && p.value == "TestApp"));
        assert!(params.iter().any(|p| p.key == "version" && p.value == "v1"));
    }

    #[test]
    fn test_resolve_body() {
        let context = create_test_context();
        let mut use_case = ResolveVariables::new(context);

        let mut request = RequestSpec::post("{{base_url}}/api");
        request.body = RequestBody::json(r#"{"app": "{{app_name}}", "key": "{{api_key}}"}"#);

        let output = use_case.execute(&request);

        assert!(output.is_complete);
        assert_eq!(
            output.resolved_request.body.content,
            r#"{"app": "TestApp", "key": "sk-secret-123"}"#
        );
    }

    #[test]
    fn test_resolve_bearer_auth() {
        let context = create_test_context();
        let mut use_case = ResolveVariables::new(context);

        let mut request = RequestSpec::get("{{base_url}}/api");
        request.auth = AuthConfig::bearer("{{api_key}}");

        let output = use_case.execute(&request);

        assert!(output.is_complete);
        match output.resolved_request.auth {
            AuthConfig::Bearer { token } => assert_eq!(token, "sk-secret-123"),
            _ => panic!("Expected Bearer auth"),
        }
    }

    #[test]
    fn test_resolve_basic_auth() {
        let mut env = Environment::new("test");
        env.add_variable("username", "admin");
        env.add_variable("password", "secret");

        let context = ResolutionContext::from_environment(&env, &SecretsStore::new());
        let mut use_case = ResolveVariables::new(context);

        let mut request = RequestSpec::get("http://localhost/api");
        request.auth = AuthConfig::basic("{{username}}", "{{password}}");

        let output = use_case.execute(&request);

        assert!(output.is_complete);
        match output.resolved_request.auth {
            AuthConfig::Basic { username, password } => {
                assert_eq!(username, "admin");
                assert_eq!(password, "secret");
            }
            _ => panic!("Expected Basic auth"),
        }
    }

    #[test]
    fn test_builtin_variables() {
        let mut use_case = ResolveVariables::empty();

        let request = RequestSpec::get("http://localhost/api/{{$uuid}}");
        let output = use_case.execute(&request);

        assert!(output.is_complete);
        // URL should contain a valid UUID
        let url = &output.resolved_request.url;
        let uuid_part = url.strip_prefix("http://localhost/api/").unwrap();
        assert!(uuid::Uuid::parse_str(uuid_part).is_ok());
    }

    #[test]
    fn test_preview_url() {
        let context = create_test_context();
        let use_case = ResolveVariables::new(context);

        let result = use_case.preview_url("{{base_url}}/api/{{version}}");

        assert!(result.is_complete);
        assert_eq!(result.resolved, "http://localhost:3000/api/v1");
    }

    #[test]
    fn test_find_unresolved() {
        let context = create_test_context();
        let use_case = ResolveVariables::new(context);

        let unresolved = use_case.find_unresolved("{{base_url}}/{{unknown}}/{{also_unknown}}");

        assert_eq!(unresolved.len(), 2);
        assert!(unresolved.contains(&"unknown".to_string()));
        assert!(unresolved.contains(&"also_unknown".to_string()));
    }
}
