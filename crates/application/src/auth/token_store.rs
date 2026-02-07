//! In-memory token storage with expiry tracking.
//!
//! This module provides a thread-safe store for `OAuth2` tokens with
//! automatic expiry detection and refresh scheduling.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use vortex_domain::OAuth2Token;

/// Thread-safe in-memory token store.
#[derive(Debug, Clone)]
pub struct TokenStore {
    tokens: Arc<RwLock<HashMap<String, OAuth2Token>>>,
    /// Seconds before expiry to trigger refresh.
    refresh_buffer_seconds: i64,
}

impl TokenStore {
    /// Create a new token store with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
            refresh_buffer_seconds: 60, // Refresh 60 seconds before expiry
        }
    }

    /// Create with custom refresh buffer.
    #[must_use]
    pub fn with_refresh_buffer(refresh_buffer_seconds: i64) -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
            refresh_buffer_seconds,
        }
    }

    /// Store a token with the given key.
    pub async fn store(&self, key: String, token: OAuth2Token) {
        let mut tokens = self.tokens.write().await;
        tokens.insert(key, token);
    }

    /// Get a token by key, returns None if not found.
    pub async fn get(&self, key: &str) -> Option<OAuth2Token> {
        let tokens = self.tokens.read().await;
        tokens.get(key).cloned()
    }

    /// Get a valid (non-expired) token, or None if expired/missing.
    pub async fn get_valid(&self, key: &str) -> Option<OAuth2Token> {
        let tokens = self.tokens.read().await;
        tokens.get(key).and_then(|t| {
            if t.is_expired_or_expiring(0) {
                None
            } else {
                Some(t.clone())
            }
        })
    }

    /// Check if a token needs refresh (exists but expiring soon).
    pub async fn needs_refresh(&self, key: &str) -> bool {
        let tokens = self.tokens.read().await;
        tokens
            .get(key)
            .is_some_and(|t| t.is_expired_or_expiring(self.refresh_buffer_seconds) && t.can_refresh())
    }

    /// Remove a token.
    pub async fn remove(&self, key: &str) -> Option<OAuth2Token> {
        let mut tokens = self.tokens.write().await;
        tokens.remove(key)
    }

    /// Clear all tokens.
    pub async fn clear(&self) {
        let mut tokens = self.tokens.write().await;
        tokens.clear();
    }

    /// Get all token keys.
    pub async fn keys(&self) -> Vec<String> {
        let tokens = self.tokens.read().await;
        tokens.keys().cloned().collect()
    }

    /// Get token status for UI display.
    pub async fn get_status(&self, key: &str) -> TokenStatus {
        let tokens = self.tokens.read().await;
        tokens.get(key).map_or(TokenStatus::NotAuthenticated, |token| if token.is_expired_or_expiring(0) {
                    TokenStatus::Expired {
                        can_refresh: token.can_refresh(),
                    }
                } else if token.is_expired_or_expiring(self.refresh_buffer_seconds) {
                    TokenStatus::Expiring {
                        seconds_remaining: token.seconds_until_expiry().unwrap_or(0),
                        can_refresh: token.can_refresh(),
                    }
                } else {
                    TokenStatus::Valid {
                        seconds_remaining: token.seconds_until_expiry(),
                    }
                })
    }

    /// Get count of stored tokens.
    pub async fn count(&self) -> usize {
        let tokens = self.tokens.read().await;
        tokens.len()
    }
}

impl Default for TokenStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Status of a token for UI display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenStatus {
    /// No token exists for this key.
    NotAuthenticated,
    /// Token is valid and not expiring soon.
    Valid {
        /// Seconds until expiry, or None if no expiry.
        seconds_remaining: Option<i64>,
    },
    /// Token is valid but will expire soon.
    Expiring {
        /// Seconds until expiry.
        seconds_remaining: i64,
        /// Whether the token can be refreshed.
        can_refresh: bool,
    },
    /// Token has expired.
    Expired {
        /// Whether the token can be refreshed.
        can_refresh: bool,
    },
}

impl TokenStatus {
    /// Returns true if the token is valid (not expired).
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        matches!(self, Self::Valid { .. } | Self::Expiring { .. })
    }

    /// Returns true if the token needs attention (expiring or expired).
    #[must_use]
    pub const fn needs_attention(&self) -> bool {
        matches!(self, Self::Expiring { .. } | Self::Expired { .. })
    }

    /// Get a user-friendly display message.
    #[must_use]
    pub fn display_message(&self) -> String {
        match self {
            Self::NotAuthenticated => "Not authenticated".to_string(),
            Self::Valid {
                seconds_remaining: Some(secs),
            } => {
                if *secs > 3600 {
                    format!("Valid for {} hours", secs / 3600)
                } else if *secs > 60 {
                    format!("Valid for {} minutes", secs / 60)
                } else {
                    format!("Valid for {secs} seconds")
                }
            }
            Self::Valid {
                seconds_remaining: None,
            } => "Valid (no expiry)".to_string(),
            Self::Expiring {
                seconds_remaining,
                can_refresh,
            } => {
                let refresh_hint = if *can_refresh {
                    " (will auto-refresh)"
                } else {
                    ""
                };
                format!("Expiring in {seconds_remaining} seconds{refresh_hint}")
            }
            Self::Expired { can_refresh } => {
                if *can_refresh {
                    "Expired (can refresh)".to_string()
                } else {
                    "Expired".to_string()
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_get_token() {
        let store = TokenStore::new();
        let token = OAuth2Token::new(
            "access123".to_string(),
            "Bearer".to_string(),
            Some(3600),
            None,
            vec![],
        );

        store.store("test-key".to_string(), token.clone()).await;

        let retrieved = store.get("test-key").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().access_token, "access123");
    }

    #[tokio::test]
    async fn test_get_valid_token() {
        let store = TokenStore::new();
        let token = OAuth2Token::new(
            "access123".to_string(),
            "Bearer".to_string(),
            Some(3600),
            None,
            vec![],
        );

        store.store("test-key".to_string(), token).await;

        let valid = store.get_valid("test-key").await;
        assert!(valid.is_some());
    }

    #[tokio::test]
    async fn test_remove_token() {
        let store = TokenStore::new();
        let token = OAuth2Token::new(
            "access123".to_string(),
            "Bearer".to_string(),
            Some(3600),
            None,
            vec![],
        );

        store.store("test-key".to_string(), token).await;
        assert!(store.get("test-key").await.is_some());

        store.remove("test-key").await;
        assert!(store.get("test-key").await.is_none());
    }

    #[tokio::test]
    async fn test_clear_tokens() {
        let store = TokenStore::new();

        store
            .store(
                "key1".to_string(),
                OAuth2Token::new("a".to_string(), "Bearer".to_string(), None, None, vec![]),
            )
            .await;
        store
            .store(
                "key2".to_string(),
                OAuth2Token::new("b".to_string(), "Bearer".to_string(), None, None, vec![]),
            )
            .await;

        assert_eq!(store.count().await, 2);

        store.clear().await;
        assert_eq!(store.count().await, 0);
    }

    #[tokio::test]
    async fn test_token_status_not_authenticated() {
        let store = TokenStore::new();
        let status = store.get_status("missing").await;
        assert_eq!(status, TokenStatus::NotAuthenticated);
    }

    #[tokio::test]
    async fn test_token_status_valid() {
        let store = TokenStore::new();
        let token = OAuth2Token::new(
            "access123".to_string(),
            "Bearer".to_string(),
            Some(3600), // 1 hour
            None,
            vec![],
        );

        store.store("test-key".to_string(), token).await;

        let status = store.get_status("test-key").await;
        assert!(status.is_valid());
        assert!(!status.needs_attention());
    }

    #[test]
    fn test_token_status_display_messages() {
        assert_eq!(
            TokenStatus::NotAuthenticated.display_message(),
            "Not authenticated"
        );

        assert!(
            TokenStatus::Valid {
                seconds_remaining: Some(7200)
            }
            .display_message()
            .contains("hours")
        );

        assert!(
            TokenStatus::Expiring {
                seconds_remaining: 30,
                can_refresh: true
            }
            .display_message()
            .contains("auto-refresh")
        );
    }
}
