//! Authentication configuration types

use serde::{Deserialize, Serialize};

/// Authentication configuration for a request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
    /// No authentication
    #[default]
    None,
    /// API Key authentication
    ApiKey {
        /// The API key value
        key: String,
        /// Header or query parameter name
        name: String,
        /// Where to add the key
        location: ApiKeyLocation,
    },
    /// Bearer token authentication
    Bearer {
        /// The bearer token
        token: String,
    },
    /// Basic authentication
    Basic {
        /// Username
        username: String,
        /// Password
        password: String,
    },
}

/// Location for API key authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyLocation {
    /// Add to request headers
    #[default]
    Header,
    /// Add to query parameters
    Query,
}

impl AuthConfig {
    /// Returns true if authentication is configured.
    #[must_use]
    pub const fn is_configured(&self) -> bool {
        !matches!(self, Self::None)
    }

    /// Creates a bearer token authentication.
    #[must_use]
    pub fn bearer(token: impl Into<String>) -> Self {
        Self::Bearer {
            token: token.into(),
        }
    }

    /// Creates a basic authentication.
    #[must_use]
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self::Basic {
            username: username.into(),
            password: password.into(),
        }
    }

    /// Creates an API key authentication in header.
    #[must_use]
    pub fn api_key_header(name: impl Into<String>, key: impl Into<String>) -> Self {
        Self::ApiKey {
            key: key.into(),
            name: name.into(),
            location: ApiKeyLocation::Header,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_none() {
        let auth = AuthConfig::None;
        assert!(!auth.is_configured());
    }

    #[test]
    fn test_bearer_auth() {
        let auth = AuthConfig::bearer("my-token");
        assert!(auth.is_configured());
        let AuthConfig::Bearer { token } = auth else {
            unreachable!("Expected Bearer auth variant");
        };
        assert_eq!(token, "my-token");
    }
}
