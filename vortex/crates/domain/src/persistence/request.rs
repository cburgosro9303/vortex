//! Request type for file-based persistence (*.json in requests/).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::auth::PersistenceAuth;
use super::body::PersistenceRequestBody;
use super::common::{Id, PersistenceHttpMethod, RequestSettings, CURRENT_SCHEMA_VERSION};
use super::test_assertion::TestAssertion;

/// A saved HTTP request stored as a JSON file.
///
/// This represents the on-disk format for individual requests.
/// Variables use `{{variable_name}}` syntax and are resolved at execution time.
///
/// Fields are ordered alphabetically for deterministic serialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedRequest {
    /// Request-specific authentication. Overrides folder/collection auth if set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<PersistenceAuth>,

    /// Request body (JSON, form data, raw text, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<PersistenceRequestBody>,

    /// HTTP headers as key-value pairs.
    /// Values may contain `{{variables}}`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,

    /// Unique identifier (UUID v4).
    pub id: Id,

    /// HTTP method (GET, POST, PUT, etc.).
    pub method: PersistenceHttpMethod,

    /// Human-readable request name.
    pub name: String,

    /// URL query parameters as key-value pairs.
    /// Values may contain `{{variables}}`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub query_params: BTreeMap<String, String>,

    /// Schema version for migration support.
    pub schema_version: u32,

    /// Request-specific settings (timeout, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<RequestSettings>,

    /// Test assertions to run after request execution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests: Vec<TestAssertion>,

    /// Request URL. May contain `{{variables}}`.
    pub url: String,
}

impl SavedRequest {
    /// Creates a new request with required fields.
    #[must_use]
    pub fn new(
        id: Id,
        name: impl Into<String>,
        method: PersistenceHttpMethod,
        url: impl Into<String>,
    ) -> Self {
        Self {
            auth: None,
            body: None,
            headers: BTreeMap::new(),
            id,
            method,
            name: name.into(),
            query_params: BTreeMap::new(),
            schema_version: CURRENT_SCHEMA_VERSION,
            settings: None,
            tests: Vec::new(),
            url: url.into(),
        }
    }

    /// Adds a header to the request.
    #[must_use]
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Adds a query parameter to the request.
    #[must_use]
    pub fn with_query_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query_params.insert(key.into(), value.into());
        self
    }

    /// Sets the request body.
    #[must_use]
    pub fn with_body(mut self, body: PersistenceRequestBody) -> Self {
        self.body = Some(body);
        self
    }

    /// Sets the request authentication.
    #[must_use]
    pub fn with_auth(mut self, auth: PersistenceAuth) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Sets request-specific settings.
    #[must_use]
    pub fn with_settings(mut self, settings: RequestSettings) -> Self {
        self.settings = Some(settings);
        self
    }

    /// Adds a test assertion.
    #[must_use]
    pub fn with_test(mut self, test: TestAssertion) -> Self {
        self.tests.push(test);
        self
    }
}

impl Default for SavedRequest {
    fn default() -> Self {
        Self::new(
            String::new(),
            "New Request",
            PersistenceHttpMethod::Get,
            "https://api.example.com",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saved_request_new() {
        let request = SavedRequest::new(
            "request-id".to_string(),
            "Get Users",
            PersistenceHttpMethod::Get,
            "https://api.example.com/users",
        );

        assert_eq!(request.name, "Get Users");
        assert_eq!(request.method, PersistenceHttpMethod::Get);
        assert_eq!(request.url, "https://api.example.com/users");
        assert_eq!(request.schema_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn test_saved_request_with_headers() {
        let request = SavedRequest::new(
            "id".to_string(),
            "Test",
            PersistenceHttpMethod::Post,
            "https://api.example.com",
        )
        .with_header("Content-Type", "application/json")
        .with_header("Authorization", "Bearer {{token}}");

        assert_eq!(request.headers.len(), 2);
        assert_eq!(
            request.headers.get("Content-Type"),
            Some(&"application/json".to_string())
        );
    }

    #[test]
    fn test_saved_request_with_body() {
        let request = SavedRequest::new(
            "id".to_string(),
            "Create User",
            PersistenceHttpMethod::Post,
            "https://api.example.com/users",
        )
        .with_body(PersistenceRequestBody::json(serde_json::json!({
            "name": "John Doe",
            "email": "john@example.com"
        })));

        assert!(request.body.is_some());
    }

    #[test]
    fn test_saved_request_default() {
        let request = SavedRequest::default();
        assert_eq!(request.name, "New Request");
        assert_eq!(request.method, PersistenceHttpMethod::Get);
    }
}
