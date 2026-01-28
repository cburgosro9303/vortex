//! HTTP Client port
//!
//! Defines the interface for HTTP operations and detailed error types.

use std::future::Future;
use std::pin::Pin;

use thiserror::Error;
use vortex_domain::{RequestErrorKind, request::RequestSpec, response::ResponseSpec};

/// Error type for HTTP client operations.
///
/// Provides detailed error information for user-friendly display
/// and error categorization.
#[derive(Debug, Clone, Error)]
pub enum HttpClientError {
    /// Invalid URL format.
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    /// DNS resolution failed.
    #[error("DNS resolution failed for {host}: {message}")]
    DnsError {
        /// The hostname that failed to resolve
        host: String,
        /// Detailed error message
        message: String,
    },

    /// Could not establish connection.
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Connection was refused by the server.
    #[error("Connection refused by {host}:{port}")]
    ConnectionRefused {
        /// The target host
        host: String,
        /// The target port
        port: u16,
    },

    /// Request timed out.
    #[error("Request timed out after {timeout_ms}ms")]
    Timeout {
        /// The timeout value in milliseconds
        timeout_ms: u64,
    },

    /// TLS/SSL error.
    #[error("TLS error: {0}")]
    TlsError(String),

    /// Invalid request body (e.g., malformed JSON).
    #[error("Invalid request body: {0}")]
    InvalidBody(String),

    /// Too many redirects.
    #[error("Too many redirects (max: {max})")]
    TooManyRedirects {
        /// Maximum number of redirects allowed
        max: u32,
    },

    /// Request was cancelled.
    #[error("Request cancelled")]
    Cancelled,

    /// Unknown or unexpected error.
    #[error("HTTP error: {0}")]
    Other(String),
}

impl HttpClientError {
    /// Converts this error to a domain `RequestErrorKind`.
    #[must_use]
    pub const fn to_error_kind(&self) -> RequestErrorKind {
        match self {
            Self::InvalidUrl(_) => RequestErrorKind::InvalidUrl,
            Self::DnsError { .. } => RequestErrorKind::DnsError,
            Self::ConnectionFailed(_) => RequestErrorKind::ConnectionFailed,
            Self::ConnectionRefused { .. } => RequestErrorKind::ConnectionRefused,
            Self::Timeout { .. } => RequestErrorKind::Timeout,
            Self::TlsError(_) => RequestErrorKind::TlsError,
            Self::InvalidBody(_) => RequestErrorKind::InvalidBody,
            Self::TooManyRedirects { .. } => RequestErrorKind::TooManyRedirects,
            Self::Cancelled => RequestErrorKind::Cancelled,
            Self::Other(_) => RequestErrorKind::Unknown,
        }
    }
}

/// Port for executing HTTP requests.
///
/// This trait abstracts the HTTP client implementation, allowing
/// the application layer to be independent of specific HTTP libraries.
///
/// Implementations must be thread-safe (`Send + Sync`) to support
/// concurrent request execution.
pub trait HttpClient: Send + Sync {
    /// Executes an HTTP request and returns the response.
    ///
    /// This method is async and handles:
    /// - URL construction with query parameters
    /// - Header setting
    /// - Body serialization
    /// - Timeout enforcement
    /// - Redirect following
    ///
    /// # Arguments
    ///
    /// * `request` - The request specification to execute
    ///
    /// # Returns
    ///
    /// * `Ok(ResponseSpec)` - The response on success
    /// * `Err(HttpClientError)` - Detailed error on failure
    fn execute(
        &self,
        request: &RequestSpec,
    ) -> Pin<Box<dyn Future<Output = Result<ResponseSpec, HttpClientError>> + Send + '_>>;
}

/// A cancellation token for aborting in-flight requests.
///
/// Used to implement the Cancel button in the UI.
/// The token is cloneable and can be shared across threads.
#[derive(Clone)]
pub struct CancellationToken {
    inner: tokio::sync::watch::Sender<bool>,
}

impl CancellationToken {
    /// Creates a new cancellation token pair.
    ///
    /// Returns a tuple of (sender, receiver) where:
    /// - The sender (`CancellationToken`) is used to signal cancellation
    /// - The receiver (`CancellationReceiver`) is used to check/wait for cancellation
    #[must_use]
    pub fn new() -> (Self, CancellationReceiver) {
        let (tx, rx) = tokio::sync::watch::channel(false);
        (Self { inner: tx }, CancellationReceiver { inner: rx })
    }

    /// Signals cancellation to all receivers.
    ///
    /// This is a no-op if cancellation was already signaled.
    pub fn cancel(&self) {
        let _ = self.inner.send(true);
    }

    /// Returns true if cancellation has been signaled.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        *self.inner.borrow()
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new().0
    }
}

impl std::fmt::Debug for CancellationToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancellationToken")
            .field("is_cancelled", &self.is_cancelled())
            .finish()
    }
}

/// Receiver side of a cancellation token.
///
/// Used by async tasks to check or wait for cancellation.
#[derive(Clone)]
pub struct CancellationReceiver {
    inner: tokio::sync::watch::Receiver<bool>,
}

impl CancellationReceiver {
    /// Returns true if cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        *self.inner.borrow()
    }

    /// Waits until cancellation is requested.
    ///
    /// Returns immediately if cancellation was already signaled.
    pub async fn cancelled(&mut self) {
        while !*self.inner.borrow() {
            if self.inner.changed().await.is_err() {
                break;
            }
        }
    }
}

impl std::fmt::Debug for CancellationReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancellationReceiver")
            .field("is_cancelled", &self.is_cancelled())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_client_error_to_kind() {
        assert_eq!(
            HttpClientError::InvalidUrl("bad url".to_string()).to_error_kind(),
            RequestErrorKind::InvalidUrl
        );
        assert_eq!(
            HttpClientError::Timeout { timeout_ms: 5000 }.to_error_kind(),
            RequestErrorKind::Timeout
        );
        assert_eq!(
            HttpClientError::Cancelled.to_error_kind(),
            RequestErrorKind::Cancelled
        );
    }

    #[tokio::test]
    async fn test_cancellation_token() {
        let (token, receiver) = CancellationToken::new();

        assert!(!token.is_cancelled());
        assert!(!receiver.is_cancelled());

        token.cancel();

        assert!(token.is_cancelled());
        assert!(receiver.is_cancelled());
    }

    #[tokio::test]
    async fn test_cancellation_receiver_wait() {
        let (token, mut receiver) = CancellationToken::new();

        // Spawn a task that waits for cancellation
        let handle = tokio::spawn(async move {
            receiver.cancelled().await;
            true
        });

        // Give the task time to start waiting
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Signal cancellation
        token.cancel();

        // The task should complete
        let result = tokio::time::timeout(std::time::Duration::from_millis(100), handle).await;
        assert!(result.is_ok());
    }
}
