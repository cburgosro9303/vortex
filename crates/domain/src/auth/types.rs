//! Authentication configuration types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
        /// The bearer token (may contain variables like `{{access_token}}`)
        token: String,
        /// Optional prefix, defaults to "Bearer"
        #[serde(default = "default_bearer_prefix")]
        prefix: String,
    },
    /// Basic authentication
    Basic {
        /// Username (may contain variables)
        username: String,
        /// Password (may contain variables)
        password: String,
    },
    /// `OAuth2` Client Credentials flow
    #[serde(rename = "oauth2_client_credentials")]
    OAuth2ClientCredentials {
        /// Token endpoint URL
        token_url: String,
        /// Client ID
        client_id: String,
        /// Client secret
        client_secret: String,
        /// Space-separated scopes
        #[serde(default)]
        scope: Option<String>,
        /// Additional parameters to send with token request
        #[serde(default)]
        extra_params: BTreeMap<String, String>,
    },
    /// `OAuth2` Authorization Code flow
    #[serde(rename = "oauth2_auth_code")]
    OAuth2AuthorizationCode {
        /// Authorization endpoint URL
        auth_url: String,
        /// Token endpoint URL
        token_url: String,
        /// Client ID
        client_id: String,
        /// Client secret
        client_secret: String,
        /// Redirect URI for the callback
        redirect_uri: String,
        /// Space-separated scopes
        #[serde(default)]
        scope: Option<String>,
        /// Additional parameters for authorization request
        #[serde(default)]
        extra_params: BTreeMap<String, String>,
    },
}

fn default_bearer_prefix() -> String {
    "Bearer".to_string()
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

    /// Returns true if this is an `OAuth2` config that needs token acquisition.
    #[must_use]
    pub const fn is_oauth2(&self) -> bool {
        matches!(
            self,
            Self::OAuth2ClientCredentials { .. } | Self::OAuth2AuthorizationCode { .. }
        )
    }

    /// Creates a bearer token authentication.
    #[must_use]
    pub fn bearer(token: impl Into<String>) -> Self {
        Self::Bearer {
            token: token.into(),
            prefix: default_bearer_prefix(),
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

    /// Generates a unique key for token caching.
    /// For `OAuth2` configs, this creates a hash-like key from the configuration.
    #[must_use]
    pub fn cache_key(&self) -> Option<String> {
        match self {
            Self::OAuth2ClientCredentials {
                token_url,
                client_id,
                scope,
                ..
            } => {
                let scope_part = scope.as_deref().unwrap_or("");
                Some(format!("cc:{token_url}:{client_id}:{scope_part}"))
            }
            Self::OAuth2AuthorizationCode {
                auth_url,
                client_id,
                scope,
                ..
            } => {
                let scope_part = scope.as_deref().unwrap_or("");
                Some(format!("ac:{auth_url}:{client_id}:{scope_part}"))
            }
            _ => None,
        }
    }
}

/// `OAuth2` token with metadata for expiry tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2Token {
    /// The access token string
    pub access_token: String,
    /// Token type (usually "Bearer")
    pub token_type: String,
    /// When the token expires (if known)
    pub expires_at: Option<DateTime<Utc>>,
    /// Refresh token for obtaining new access tokens
    pub refresh_token: Option<String>,
    /// Scopes granted by this token
    #[serde(default)]
    pub scopes: Vec<String>,
    /// When this token was obtained
    pub obtained_at: DateTime<Utc>,
}

impl OAuth2Token {
    /// Create a new token with current timestamp.
    #[must_use]
    pub fn new(
        access_token: String,
        token_type: String,
        expires_in_secs: Option<u64>,
        refresh_token: Option<String>,
        scopes: Vec<String>,
    ) -> Self {
        let now = Utc::now();
        let expires_at = expires_in_secs.map(|secs| now + chrono::Duration::seconds(secs.cast_signed()));

        Self {
            access_token,
            token_type,
            expires_at,
            refresh_token,
            scopes,
            obtained_at: now,
        }
    }

    /// Check if the token is expired or will expire within the given buffer.
    #[must_use]
    pub fn is_expired_or_expiring(&self, buffer_seconds: i64) -> bool {
        self.expires_at.is_some_and(|expires_at| {
                let buffer = chrono::Duration::seconds(buffer_seconds);
                Utc::now() + buffer >= expires_at
            })
    }

    /// Check if the token can be refreshed.
    #[must_use]
    pub const fn can_refresh(&self) -> bool {
        self.refresh_token.is_some()
    }

    /// Time until expiry in seconds, or None if no expiry.
    #[must_use]
    pub fn seconds_until_expiry(&self) -> Option<i64> {
        self.expires_at.map(|exp| (exp - Utc::now()).num_seconds())
    }

    /// Returns the Authorization header value.
    #[must_use]
    pub fn authorization_header(&self) -> String {
        format!("{} {}", self.token_type, self.access_token)
    }
}

/// Result of an authentication resolution.
#[derive(Debug, Clone)]
pub enum AuthResolution {
    /// No authentication needed.
    None,
    /// Add this header to the request.
    Header {
        /// Header name (e.g., "Authorization").
        name: String,
        /// Header value (e.g., "Bearer token123").
        value: String,
    },
    /// Add this query parameter.
    QueryParam {
        /// Query parameter name.
        name: String,
        /// Query parameter value.
        value: String,
    },
    /// Authentication is pending (e.g., waiting for OAuth callback).
    Pending {
        /// Status message for the user.
        message: String,
    },
    /// Authentication failed.
    Failed {
        /// The authentication error.
        error: AuthError,
    },
}

/// Authentication errors.
#[derive(Debug, Clone)]
pub enum AuthError {
    /// Token expired and no refresh token available.
    TokenExpiredNoRefresh,
    /// Failed to refresh token.
    RefreshFailed {
        /// Error description.
        message: String,
    },
    /// `OAuth2` authorization failed.
    OAuth2AuthorizationFailed {
        /// Error description.
        message: String,
    },
    /// Invalid `OAuth2` configuration.
    InvalidConfiguration {
        /// Error description.
        message: String,
    },
    /// User cancelled authentication.
    UserCancelled,
    /// Callback server error.
    CallbackServerError {
        /// Error description.
        message: String,
    },
    /// Network error.
    NetworkError {
        /// Error description.
        message: String,
    },
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TokenExpiredNoRefresh => {
                write!(f, "Token expired and no refresh token available")
            }
            Self::RefreshFailed { message } => write!(f, "Failed to refresh token: {message}"),
            Self::OAuth2AuthorizationFailed { message } => {
                write!(f, "OAuth2 authorization failed: {message}")
            }
            Self::InvalidConfiguration { message } => {
                write!(f, "Invalid OAuth2 configuration: {message}")
            }
            Self::UserCancelled => write!(f, "User cancelled authentication"),
            Self::CallbackServerError { message } => {
                write!(f, "Callback server error: {message}")
            }
            Self::NetworkError { message } => write!(f, "Network error: {message}"),
        }
    }
}

impl std::error::Error for AuthError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_none() {
        let auth = AuthConfig::None;
        assert!(!auth.is_configured());
        assert!(!auth.is_oauth2());
    }

    #[test]
    fn test_bearer_auth() {
        let auth = AuthConfig::bearer("my-token");
        assert!(auth.is_configured());
        assert!(!auth.is_oauth2());
        let AuthConfig::Bearer { token, prefix } = auth else {
            unreachable!("Expected Bearer auth variant");
        };
        assert_eq!(token, "my-token");
        assert_eq!(prefix, "Bearer");
    }

    #[test]
    fn test_oauth2_client_credentials() {
        let auth = AuthConfig::OAuth2ClientCredentials {
            token_url: "https://auth.example.com/token".to_string(),
            client_id: "my-client".to_string(),
            client_secret: "my-secret".to_string(),
            scope: Some("read write".to_string()),
            extra_params: BTreeMap::new(),
        };
        assert!(auth.is_configured());
        assert!(auth.is_oauth2());
        assert!(auth.cache_key().is_some());
    }

    #[test]
    fn test_oauth2_token_expiry() {
        let token = OAuth2Token::new(
            "access123".to_string(),
            "Bearer".to_string(),
            Some(3600),
            Some("refresh456".to_string()),
            vec!["read".to_string()],
        );

        assert!(!token.is_expired_or_expiring(0));
        assert!(token.can_refresh());
        assert!(token.seconds_until_expiry().is_some());
        assert_eq!(token.authorization_header(), "Bearer access123");
    }

    #[test]
    fn test_oauth2_token_no_expiry() {
        let token = OAuth2Token::new(
            "access123".to_string(),
            "Bearer".to_string(),
            None,
            None,
            vec![],
        );

        assert!(!token.is_expired_or_expiring(0));
        assert!(!token.can_refresh());
        assert!(token.seconds_until_expiry().is_none());
    }
}
