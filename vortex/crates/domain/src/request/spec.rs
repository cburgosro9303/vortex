//! Request specification type
//!
//! Contains the complete specification for an HTTP request,
//! including URL, method, headers, body, and configuration.

use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use super::{Header, Headers, HttpMethod, QueryParam, QueryParams, RequestBody};
use crate::auth::AuthConfig;

/// Default timeout in milliseconds (30 seconds).
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// Complete specification for an HTTP request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestSpec {
    /// Unique identifier for this request
    pub id: Uuid,
    /// Human-readable name
    pub name: String,
    /// Optional description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// HTTP method
    pub method: HttpMethod,
    /// Target URL (may contain variable placeholders)
    pub url: String,
    /// HTTP headers
    #[serde(default)]
    pub headers: Headers,
    /// Query parameters (appended to URL)
    #[serde(default)]
    pub query_params: QueryParams,
    /// Request body
    #[serde(default)]
    pub body: RequestBody,
    /// Authentication configuration
    #[serde(default)]
    pub auth: AuthConfig,
    /// Request timeout in milliseconds
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

const fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}

impl RequestSpec {
    /// Creates a new request specification with default values.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            name: name.into(),
            description: None,
            method: HttpMethod::default(),
            url: String::new(),
            headers: Headers::new(),
            query_params: QueryParams::new(),
            body: RequestBody::none(),
            auth: AuthConfig::default(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    /// Creates a GET request with the given URL.
    #[must_use]
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            name: "GET Request".to_string(),
            description: None,
            method: HttpMethod::Get,
            url: url.into(),
            headers: Headers::new(),
            query_params: QueryParams::new(),
            body: RequestBody::none(),
            auth: AuthConfig::default(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    /// Creates a POST request with the given URL.
    #[must_use]
    pub fn post(url: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            name: "POST Request".to_string(),
            description: None,
            method: HttpMethod::Post,
            url: url.into(),
            headers: Headers::new(),
            query_params: QueryParams::new(),
            body: RequestBody::none(),
            auth: AuthConfig::default(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    /// Creates a PUT request with the given URL.
    #[must_use]
    pub fn put(url: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            name: "PUT Request".to_string(),
            description: None,
            method: HttpMethod::Put,
            url: url.into(),
            headers: Headers::new(),
            query_params: QueryParams::new(),
            body: RequestBody::none(),
            auth: AuthConfig::default(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    /// Creates a DELETE request with the given URL.
    #[must_use]
    pub fn delete(url: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            name: "DELETE Request".to_string(),
            description: None,
            method: HttpMethod::Delete,
            url: url.into(),
            headers: Headers::new(),
            query_params: QueryParams::new(),
            body: RequestBody::none(),
            auth: AuthConfig::default(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    /// Adds a header to the request.
    #[must_use]
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.add(Header::new(key, value));
        self
    }

    /// Adds a query parameter to the request.
    #[must_use]
    pub fn with_query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query_params.add(QueryParam::new(key, value));
        self
    }

    /// Sets the request body.
    #[must_use]
    pub fn with_body(mut self, body: RequestBody) -> Self {
        self.body = body;
        self
    }

    /// Sets the timeout in milliseconds.
    #[must_use]
    pub const fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Sets the request name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Returns only enabled headers.
    pub fn enabled_headers(&self) -> impl Iterator<Item = &Header> {
        self.headers.enabled()
    }

    /// Returns only enabled query parameters.
    pub fn enabled_query_params(&self) -> impl Iterator<Item = &QueryParam> {
        self.query_params.enabled()
    }

    /// Builds the full URL with query parameters.
    ///
    /// Appends enabled query parameters to the base URL.
    #[must_use]
    pub fn full_url(&self) -> String {
        let enabled_params: Vec<_> = self.enabled_query_params().collect();
        if enabled_params.is_empty() {
            return self.url.clone();
        }

        let query_string = enabled_params
            .iter()
            .map(|p| {
                format!(
                    "{}={}",
                    urlencoding_key(&p.key),
                    urlencoding_value(&p.value)
                )
            })
            .collect::<Vec<_>>()
            .join("&");

        if self.url.contains('?') {
            format!("{}&{}", self.url, query_string)
        } else {
            format!("{}?{}", self.url, query_string)
        }
    }

    /// Validates the URL and returns a parsed version if valid.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is malformed.
    pub fn parse_url(&self) -> Result<Url, url::ParseError> {
        Url::parse(&self.url)
    }

    /// Returns true if the URL contains variable placeholders.
    #[must_use]
    pub fn has_variables(&self) -> bool {
        self.url.contains("{{") && self.url.contains("}}")
    }
}

/// Simple URL encoding for keys (percent-encode special characters).
fn urlencoding_key(s: &str) -> String {
    s.replace(' ', "+")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('#', "%23")
        .replace('?', "%3F")
}

/// Simple URL encoding for values (percent-encode special characters).
fn urlencoding_value(s: &str) -> String {
    s.replace(' ', "+")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('#', "%23")
        .replace('?', "%3F")
}

impl Default for RequestSpec {
    fn default() -> Self {
        Self::new("New Request")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_request() {
        let req = RequestSpec::new("Test Request");
        assert_eq!(req.name, "Test Request");
        assert_eq!(req.method, HttpMethod::Get);
        assert_eq!(req.timeout_ms, 30_000);
    }

    #[test]
    fn test_get_request() {
        let req = RequestSpec::get("https://api.example.com/users");
        assert_eq!(req.method, HttpMethod::Get);
        assert_eq!(req.url, "https://api.example.com/users");
    }

    #[test]
    fn test_post_request() {
        let req = RequestSpec::post("https://api.example.com/users");
        assert_eq!(req.method, HttpMethod::Post);
    }

    #[test]
    fn test_builder_pattern() {
        let req = RequestSpec::get("https://api.example.com/users")
            .with_header("Accept", "application/json")
            .with_header("Authorization", "Bearer token123")
            .with_query("page", "1")
            .with_query("limit", "10")
            .with_timeout(5000);

        assert_eq!(req.headers.len(), 2);
        assert_eq!(req.query_params.len(), 2);
        assert_eq!(req.timeout_ms, 5000);
    }

    #[test]
    fn test_full_url_without_params() {
        let req = RequestSpec::get("https://api.example.com/users");
        assert_eq!(req.full_url(), "https://api.example.com/users");
    }

    #[test]
    fn test_full_url_with_params() {
        let req = RequestSpec::get("https://api.example.com/users")
            .with_query("page", "1")
            .with_query("limit", "10");

        assert_eq!(
            req.full_url(),
            "https://api.example.com/users?page=1&limit=10"
        );
    }

    #[test]
    fn test_full_url_with_existing_query() {
        let req =
            RequestSpec::get("https://api.example.com/users?sort=name").with_query("page", "1");

        assert_eq!(
            req.full_url(),
            "https://api.example.com/users?sort=name&page=1"
        );
    }

    #[test]
    fn test_urlencoding() {
        let req = RequestSpec::get("https://api.example.com/search")
            .with_query("q", "hello world")
            .with_query("filter", "a&b=c");

        assert_eq!(
            req.full_url(),
            "https://api.example.com/search?q=hello+world&filter=a%26b%3Dc"
        );
    }

    #[test]
    fn test_has_variables() {
        let mut req = RequestSpec::new("Test");
        req.url = "https://{{host}}/api/{{version}}/users".to_string();
        assert!(req.has_variables());

        req.url = "https://api.example.com/users".to_string();
        assert!(!req.has_variables());
    }

    #[test]
    fn test_with_body() {
        let req = RequestSpec::post("https://api.example.com/users")
            .with_body(RequestBody::json(r#"{"name": "John"}"#));

        assert!(!req.body.is_empty());
        assert_eq!(req.body.content_type(), Some("application/json"));
    }
}
