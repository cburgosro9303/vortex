//! Request specification type

use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use super::{Headers, HttpMethod, RequestBody};
use crate::auth::AuthConfig;

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
    /// Request body
    #[serde(default)]
    pub body: RequestBody,
    /// Authentication configuration
    #[serde(default)]
    pub auth: AuthConfig,
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
            body: RequestBody::none(),
            auth: AuthConfig::default(),
        }
    }

    /// Creates a GET request with the given URL.
    #[must_use]
    pub fn get(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            name: name.into(),
            description: None,
            method: HttpMethod::Get,
            url: url.into(),
            headers: Headers::new(),
            body: RequestBody::none(),
            auth: AuthConfig::default(),
        }
    }

    /// Validates the URL and returns parsed version if valid.
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
    }

    #[test]
    fn test_get_request() {
        let req = RequestSpec::get("Users", "https://api.example.com/users");
        assert_eq!(req.method, HttpMethod::Get);
        assert_eq!(req.url, "https://api.example.com/users");
    }

    #[test]
    fn test_has_variables() {
        let mut req = RequestSpec::new("Test");
        req.url = "https://{{host}}/api/{{version}}/users".to_string();
        assert!(req.has_variables());

        req.url = "https://api.example.com/users".to_string();
        assert!(!req.has_variables());
    }
}
