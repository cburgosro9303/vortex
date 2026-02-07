//! Authentication types for requests and collections.

use serde::{Deserialize, Serialize};

/// Authentication configuration.
///
/// The `type` field is used as the discriminator for JSON serialization.
/// All string values may contain `{{variables}}` for dynamic resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PersistenceAuth {
    /// Bearer token authentication.
    Bearer {
        /// The bearer token value.
        token: String,
    },

    /// HTTP Basic authentication.
    Basic {
        /// Username for basic auth.
        username: String,
        /// Password for basic auth.
        password: String,
    },

    /// API Key authentication.
    ApiKey {
        /// Header or query parameter name.
        key: String,
        /// The API key value.
        value: String,
        /// Where to send the key: "header" or "query".
        location: ApiKeyLocation,
    },

    /// `OAuth2` Client Credentials flow.
    Oauth2ClientCredentials {
        /// Token endpoint URL.
        token_url: String,
        /// Client ID.
        client_id: String,
        /// Client secret.
        client_secret: String,
        /// OAuth scopes (space-separated).
        #[serde(skip_serializing_if = "Option::is_none")]
        scope: Option<String>,
    },

    /// `OAuth2` Authorization Code flow.
    Oauth2AuthCode {
        /// Authorization endpoint URL.
        auth_url: String,
        /// Token endpoint URL.
        token_url: String,
        /// Client ID.
        client_id: String,
        /// Client secret.
        client_secret: String,
        /// Redirect URI for callback.
        redirect_uri: String,
        /// OAuth scopes (space-separated).
        #[serde(skip_serializing_if = "Option::is_none")]
        scope: Option<String>,
    },
}

impl PersistenceAuth {
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

    /// Creates an API key authentication in a header.
    #[must_use]
    pub fn api_key_header(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self::ApiKey {
            key: key.into(),
            value: value.into(),
            location: ApiKeyLocation::Header,
        }
    }

    /// Creates an API key authentication in query params.
    #[must_use]
    pub fn api_key_query(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self::ApiKey {
            key: key.into(),
            value: value.into(),
            location: ApiKeyLocation::Query,
        }
    }
}

/// Location for API key authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyLocation {
    /// Send API key in HTTP header.
    Header,
    /// Send API key in query parameters.
    Query,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_bearer() {
        let auth = PersistenceAuth::bearer("my-token");
        match auth {
            PersistenceAuth::Bearer { token } => assert_eq!(token, "my-token"),
            _ => panic!("Expected Bearer auth"),
        }
    }

    #[test]
    fn test_auth_basic() {
        let auth = PersistenceAuth::basic("user", "pass");
        match auth {
            PersistenceAuth::Basic { username, password } => {
                assert_eq!(username, "user");
                assert_eq!(password, "pass");
            }
            _ => panic!("Expected Basic auth"),
        }
    }

    #[test]
    fn test_auth_api_key_header() {
        let auth = PersistenceAuth::api_key_header("X-API-Key", "secret");
        match auth {
            PersistenceAuth::ApiKey {
                key,
                value,
                location,
            } => {
                assert_eq!(key, "X-API-Key");
                assert_eq!(value, "secret");
                assert_eq!(location, ApiKeyLocation::Header);
            }
            _ => panic!("Expected ApiKey auth"),
        }
    }
}
