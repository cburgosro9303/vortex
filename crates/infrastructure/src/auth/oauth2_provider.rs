//! `OAuth2` authentication provider implementation.
//!
//! This module provides `OAuth2` Client Credentials and Authorization Code
//! flow implementations.

#![allow(missing_docs)]

use serde::Deserialize;
use std::sync::Arc;
use vortex_application::{AuthProvider, TokenStore};
use vortex_domain::{AuthConfig, AuthError, AuthResolution, OAuth2Token};

/// Content-Type for form-urlencoded data.
const FORM_CONTENT_TYPE: &str = "application/x-www-form-urlencoded";

/// `OAuth2` token response from token endpoint.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    scope: Option<String>,
}

/// `OAuth2` error response.
#[derive(Debug, Deserialize)]
struct TokenErrorResponse {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

/// `OAuth2` authentication provider.
///
/// Handles `OAuth2` Client Credentials and Authorization Code flows
/// with automatic token caching and refresh.
pub struct OAuth2Provider {
    token_store: Arc<TokenStore>,
    http_client: reqwest::Client,
    /// Callback server port for Authorization Code flow.
    callback_port: u16,
}

impl OAuth2Provider {
    /// Create a new `OAuth2` provider.
    #[must_use]
    pub fn new() -> Self {
        Self {
            token_store: Arc::new(TokenStore::new()),
            http_client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            callback_port: 8080,
        }
    }

    /// Create with custom token store (for sharing between providers).
    #[must_use]
    pub fn with_token_store(token_store: Arc<TokenStore>) -> Self {
        Self {
            token_store,
            http_client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            callback_port: 8080,
        }
    }

    /// Set the callback port for Authorization Code flow.
    #[must_use]
    pub const fn with_callback_port(mut self, port: u16) -> Self {
        self.callback_port = port;
        self
    }

    /// Get access to the token store.
    #[must_use]
    pub fn token_store(&self) -> &TokenStore {
        &self.token_store
    }

    /// Execute Client Credentials flow.
    async fn client_credentials_flow(
        &self,
        token_url: &str,
        client_id: &str,
        client_secret: &str,
        scope: Option<&str>,
    ) -> Result<OAuth2Token, AuthError> {
        let mut params = vec![
            ("grant_type".to_string(), "client_credentials".to_string()),
            ("client_id".to_string(), client_id.to_string()),
            ("client_secret".to_string(), client_secret.to_string()),
        ];

        if let Some(s) = scope {
            params.push(("scope".to_string(), s.to_string()));
        }

        let body = serde_urlencoded::to_string(&params).map_err(|e| AuthError::NetworkError {
            message: format!("Failed to encode form: {e}"),
        })?;

        let response = self
            .http_client
            .post(token_url)
            .header("Content-Type", FORM_CONTENT_TYPE)
            .body(body)
            .send()
            .await
            .map_err(|e: reqwest::Error| AuthError::NetworkError {
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            if let Ok(error_response) = serde_json::from_str::<TokenErrorResponse>(&error_text) {
                return Err(AuthError::OAuth2AuthorizationFailed {
                    message: error_response
                        .error_description
                        .unwrap_or(error_response.error),
                });
            }
            return Err(AuthError::OAuth2AuthorizationFailed {
                message: format!("Token request failed: {error_text}"),
            });
        }

        let token_response: TokenResponse =
            response
                .json()
                .await
                .map_err(|e: reqwest::Error| AuthError::NetworkError {
                    message: format!("Failed to parse token response: {e}"),
                })?;

        let scopes: Vec<String> = token_response
            .scope
            .map(|s: String| s.split_whitespace().map(String::from).collect())
            .unwrap_or_default();

        Ok(OAuth2Token::new(
            token_response.access_token,
            token_response.token_type,
            token_response.expires_in,
            token_response.refresh_token,
            scopes,
        ))
    }

    /// Execute refresh token flow.
    async fn refresh_token_flow(
        &self,
        token_url: &str,
        client_id: &str,
        client_secret: &str,
        refresh_token: &str,
    ) -> Result<OAuth2Token, AuthError> {
        let params = [
            ("grant_type".to_string(), "refresh_token".to_string()),
            ("client_id".to_string(), client_id.to_string()),
            ("client_secret".to_string(), client_secret.to_string()),
            ("refresh_token".to_string(), refresh_token.to_string()),
        ];

        let body = serde_urlencoded::to_string(&params).map_err(|e| AuthError::NetworkError {
            message: format!("Failed to encode form: {e}"),
        })?;

        let response = self
            .http_client
            .post(token_url)
            .header("Content-Type", FORM_CONTENT_TYPE)
            .body(body)
            .send()
            .await
            .map_err(|e: reqwest::Error| AuthError::NetworkError {
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AuthError::RefreshFailed {
                message: error_text,
            });
        }

        let token_response: TokenResponse =
            response
                .json()
                .await
                .map_err(|e: reqwest::Error| AuthError::NetworkError {
                    message: format!("Failed to parse token response: {e}"),
                })?;

        let scopes: Vec<String> = token_response
            .scope
            .map(|s: String| s.split_whitespace().map(String::from).collect())
            .unwrap_or_default();

        Ok(OAuth2Token::new(
            token_response.access_token,
            token_response.token_type,
            token_response.expires_in,
            token_response.refresh_token,
            scopes,
        ))
    }

    /// Resolve Bearer auth (simple token formatting).
    fn resolve_bearer(token: &str, prefix: &str) -> AuthResolution {
        AuthResolution::Header {
            name: "Authorization".to_string(),
            value: format!("{prefix} {token}"),
        }
    }

    /// Resolve Basic auth (base64 encoding).
    fn resolve_basic(username: &str, password: &str) -> AuthResolution {
        use base64::Engine;
        let credentials = format!("{username}:{password}");
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());
        AuthResolution::Header {
            name: "Authorization".to_string(),
            value: format!("Basic {encoded}"),
        }
    }

    /// Resolve API Key auth.
    fn resolve_api_key(
        key: &str,
        name: &str,
        location: vortex_domain::ApiKeyLocation,
    ) -> AuthResolution {
        match location {
            vortex_domain::ApiKeyLocation::Header => AuthResolution::Header {
                name: name.to_string(),
                value: key.to_string(),
            },
            vortex_domain::ApiKeyLocation::Query => AuthResolution::QueryParam {
                name: name.to_string(),
                value: key.to_string(),
            },
        }
    }
}

impl Default for OAuth2Provider {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthProvider for OAuth2Provider {
    fn resolve<'a>(
        &'a self,
        config: &'a AuthConfig,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AuthResolution> + Send + 'a>> {
        Box::pin(async move {
            match config {
                AuthConfig::None => AuthResolution::None,

                AuthConfig::Bearer { token, prefix } => Self::resolve_bearer(token, prefix),

                AuthConfig::Basic { username, password } => Self::resolve_basic(username, password),

                AuthConfig::ApiKey {
                    key,
                    name,
                    location,
                } => Self::resolve_api_key(key, name, *location),

                AuthConfig::OAuth2ClientCredentials {
                    token_url,
                    client_id,
                    client_secret,
                    scope,
                    ..
                } => {
                    // Check cache first
                    if let Some(cache_key) = config.cache_key()
                        && let Some(token) = self.token_store.get_valid(&cache_key).await
                    {
                        return AuthResolution::Header {
                            name: "Authorization".to_string(),
                            value: token.authorization_header(),
                        };
                    }

                    // Fetch new token
                    match self
                        .client_credentials_flow(
                            token_url,
                            client_id,
                            client_secret,
                            scope.as_deref(),
                        )
                        .await
                    {
                        Ok(token) => {
                            let auth_header = token.authorization_header();

                            // Cache the token
                            if let Some(cache_key) = config.cache_key() {
                                self.token_store.store(cache_key, token).await;
                            }

                            AuthResolution::Header {
                                name: "Authorization".to_string(),
                                value: auth_header,
                            }
                        }
                        Err(e) => AuthResolution::Failed { error: e },
                    }
                }

                AuthConfig::OAuth2AuthorizationCode { .. } => {
                    // Authorization Code flow requires user interaction
                    // For now, return pending - full implementation would open browser
                    AuthResolution::Pending {
                        message: "Authorization Code flow requires browser authentication. Click 'Authorize' to continue.".to_string(),
                    }
                }
            }
        })
    }

    fn refresh_token<'a>(
        &'a self,
        config: &'a AuthConfig,
        refresh_token: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<OAuth2Token, AuthError>> + Send + 'a>,
    > {
        Box::pin(async move {
            #[allow(clippy::manual_let_else)]
            let token_url = match config {
                AuthConfig::OAuth2ClientCredentials { token_url, .. }
                | AuthConfig::OAuth2AuthorizationCode { token_url, .. } => token_url,
                _ => {
                    return Err(AuthError::InvalidConfiguration {
                        message: "Config is not an OAuth2 type".to_string(),
                    });
                }
            };

            #[allow(clippy::manual_let_else)]
            let (client_id, client_secret) = match config {
                AuthConfig::OAuth2ClientCredentials {
                    client_id,
                    client_secret,
                    ..
                }
                | AuthConfig::OAuth2AuthorizationCode {
                    client_id,
                    client_secret,
                    ..
                } => (client_id, client_secret),
                _ => {
                    return Err(AuthError::InvalidConfiguration {
                        message: "Config is not an OAuth2 type".to_string(),
                    });
                }
            };

            let new_token = self
                .refresh_token_flow(token_url, client_id, client_secret, refresh_token)
                .await?;

            // Update cache
            if let Some(cache_key) = config.cache_key() {
                self.token_store.store(cache_key, new_token.clone()).await;
            }

            Ok(new_token)
        })
    }

    fn revoke_token<'a>(
        &'a self,
        _config: &'a AuthConfig,
        _token: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), AuthError>> + Send + 'a>>
    {
        Box::pin(async move {
            // Token revocation is provider-specific and not always supported
            // For now, just succeed - could implement RFC 7009 in future
            Ok(())
        })
    }

    fn get_cached_token(&self, _config: &AuthConfig) -> Option<OAuth2Token> {
        // This is sync, but we need async to get from store
        // In practice, use resolve() for async access
        None
    }

    fn clear_cached_token(&self, _config: &AuthConfig) {
        // Would need async or spawn to clear from store
        // In practice, manage via token_store directly
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_bearer() {
        let result = OAuth2Provider::resolve_bearer("my-token", "Bearer");
        match result {
            AuthResolution::Header { name, value } => {
                assert_eq!(name, "Authorization");
                assert_eq!(value, "Bearer my-token");
            }
            _ => panic!("Expected Header resolution"),
        }
    }

    #[test]
    fn test_resolve_basic() {
        let result = OAuth2Provider::resolve_basic("user", "pass");
        match result {
            AuthResolution::Header { name, value } => {
                assert_eq!(name, "Authorization");
                // "user:pass" base64 encoded is "dXNlcjpwYXNz"
                assert_eq!(value, "Basic dXNlcjpwYXNz");
            }
            _ => panic!("Expected Header resolution"),
        }
    }

    #[test]
    fn test_resolve_api_key_header() {
        let result = OAuth2Provider::resolve_api_key(
            "secret-key",
            "X-API-Key",
            vortex_domain::ApiKeyLocation::Header,
        );
        match result {
            AuthResolution::Header { name, value } => {
                assert_eq!(name, "X-API-Key");
                assert_eq!(value, "secret-key");
            }
            _ => panic!("Expected Header resolution"),
        }
    }

    #[test]
    fn test_resolve_api_key_query() {
        let result = OAuth2Provider::resolve_api_key(
            "secret-key",
            "api_key",
            vortex_domain::ApiKeyLocation::Query,
        );
        match result {
            AuthResolution::QueryParam { name, value } => {
                assert_eq!(name, "api_key");
                assert_eq!(value, "secret-key");
            }
            _ => panic!("Expected QueryParam resolution"),
        }
    }

    #[tokio::test]
    async fn test_oauth2_provider_creation() {
        let provider = OAuth2Provider::new();
        assert_eq!(provider.callback_port, 8080);
    }
}
