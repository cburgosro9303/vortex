//! Authentication provider trait and types.
//!
//! This module defines the interface for authentication providers
//! that can resolve authentication configurations into actual
//! credentials for HTTP requests.

use std::future::Future;
use std::pin::Pin;
use vortex_domain::{AuthConfig, AuthError, AuthResolution, OAuth2Token};

/// Trait for authentication providers.
///
/// Implementations handle the actual authentication process,
/// whether that's simply formatting a bearer token or performing
/// a full `OAuth2` flow.
pub trait AuthProvider: Send + Sync {
    /// Resolve an auth configuration into concrete credentials.
    ///
    /// This may involve:
    /// - Formatting bearer tokens or basic auth headers
    /// - Fetching `OAuth2` tokens (from cache or server)
    /// - Triggering browser-based authorization
    ///
    /// # Arguments
    /// * `config` - The authentication configuration to resolve.
    ///
    /// # Returns
    /// An `AuthResolution` that can be applied to the request.
    fn resolve<'a>(
        &'a self,
        config: &'a AuthConfig,
    ) -> Pin<Box<dyn Future<Output = AuthResolution> + Send + 'a>>;

    /// Refresh an `OAuth2` token.
    ///
    /// # Arguments
    /// * `config` - The `OAuth2` configuration.
    /// * `refresh_token` - The refresh token to use.
    ///
    /// # Returns
    /// A new token or an error.
    fn refresh_token<'a>(
        &'a self,
        config: &'a AuthConfig,
        refresh_token: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<OAuth2Token, AuthError>> + Send + 'a>>;

    /// Revoke a token (if supported by the provider).
    ///
    /// # Arguments
    /// * `config` - The `OAuth2` configuration.
    /// * `token` - The token to revoke.
    fn revoke_token<'a>(
        &'a self,
        config: &'a AuthConfig,
        token: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), AuthError>> + Send + 'a>>;

    /// Get the current token for an `OAuth2` config (from cache).
    fn get_cached_token(&self, config: &AuthConfig) -> Option<OAuth2Token>;

    /// Clear cached token for a config.
    fn clear_cached_token(&self, config: &AuthConfig);
}

/// Authorization state for tracking `OAuth2` flows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationState {
    /// Not started.
    Idle,
    /// Waiting for user to authorize in browser.
    WaitingForBrowser {
        /// The authorization URL opened in browser.
        auth_url: String,
    },
    /// Exchanging code for token.
    ExchangingCode,
    /// Authorization completed successfully.
    Completed {
        /// The obtained token (access token only, not full struct).
        access_token_preview: String,
    },
    /// Authorization failed.
    Failed {
        /// Error message.
        error: String,
    },
    /// User cancelled.
    Cancelled,
}

impl AuthorizationState {
    /// Check if the flow is in progress.
    #[must_use]
    pub const fn is_in_progress(&self) -> bool {
        matches!(self, Self::WaitingForBrowser { .. } | Self::ExchangingCode)
    }

    /// Check if the flow completed (success or failure).
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        matches!(
            self,
            Self::Completed { .. } | Self::Failed { .. } | Self::Cancelled
        )
    }

    /// Get a user-friendly message.
    #[must_use]
    pub const fn message(&self) -> &str {
        match self {
            Self::Idle => "Ready to authenticate",
            Self::WaitingForBrowser { .. } => "Waiting for authorization in browser...",
            Self::ExchangingCode => "Exchanging authorization code...",
            Self::Completed { .. } => "Authorization successful",
            Self::Failed { .. } => "Authorization failed",
            Self::Cancelled => "Authorization cancelled",
        }
    }
}

/// Events emitted during `OAuth2` flows for UI updates.
#[derive(Debug, Clone)]
pub enum AuthEvent {
    /// Flow started.
    Started {
        /// Cache key for this auth config.
        config_key: String,
    },
    /// Browser was opened for authorization.
    BrowserOpened {
        /// The URL opened in the browser.
        url: String,
    },
    /// Callback received from browser.
    CallbackReceived,
    /// Token exchange started.
    ExchangingToken,
    /// Token obtained successfully.
    TokenObtained {
        /// Preview of the token (first few chars).
        token_preview: String,
        /// Seconds until expiry.
        expires_in: Option<u64>,
    },
    /// Token refreshed.
    TokenRefreshed {
        /// Preview of the new token.
        token_preview: String,
        /// Seconds until expiry.
        expires_in: Option<u64>,
    },
    /// Flow failed.
    Failed {
        /// Error message.
        error: String,
    },
    /// Flow cancelled.
    Cancelled,
}

impl AuthEvent {
    /// Get a preview of an access token (first 8 chars + ...).
    #[must_use]
    pub fn token_preview(token: &str) -> String {
        if token.len() > 12 {
            format!("{}...", &token[..8])
        } else {
            token.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authorization_state_transitions() {
        let state = AuthorizationState::Idle;
        assert!(!state.is_in_progress());
        assert!(!state.is_finished());

        let state = AuthorizationState::WaitingForBrowser {
            auth_url: "https://auth.example.com".to_string(),
        };
        assert!(state.is_in_progress());
        assert!(!state.is_finished());

        let state = AuthorizationState::Completed {
            access_token_preview: "abc...".to_string(),
        };
        assert!(!state.is_in_progress());
        assert!(state.is_finished());
    }

    #[test]
    fn test_auth_event_token_preview() {
        let preview = AuthEvent::token_preview("abcdefghijklmnop");
        assert_eq!(preview, "abcdefgh...");

        let preview = AuthEvent::token_preview("short");
        assert_eq!(preview, "short");
    }
}
