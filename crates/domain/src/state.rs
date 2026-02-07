//! Request execution state types for UI binding.
//!
//! This module defines the state machine for request execution,
//! enabling the UI to display appropriate feedback at each stage.

use serde::{Deserialize, Serialize};

use crate::response::ResponseSpec;

/// Represents the current state of a request in the UI.
///
/// This enum enables the UI to show appropriate feedback:
/// - `Idle`: Ready to send, show Send button
/// - `Loading`: Request in flight, show spinner and Cancel
/// - `Success`: Response received, show response panel
/// - `Error`: Request failed, show error message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
#[derive(Default)]
pub enum RequestState {
    /// No request has been sent yet, or reset after cancel.
    #[default]
    Idle,

    /// Request is in progress.
    Loading {
        /// When the request started (for elapsed time display).
        /// Skipped in serialization as Instant is not serializable.
        #[serde(skip)]
        started_at: Option<std::time::Instant>,
    },

    /// Request completed successfully.
    Success {
        /// The response data.
        response: Box<ResponseSpec>,
    },

    /// Request failed with an error.
    Error {
        /// Error category for display.
        kind: RequestErrorKind,
        /// Human-readable error message.
        message: String,
        /// Optional technical details.
        details: Option<String>,
    },
}

impl RequestState {
    /// Creates a new Loading state with the current timestamp.
    #[must_use]
    pub fn loading() -> Self {
        Self::Loading {
            started_at: Some(std::time::Instant::now()),
        }
    }

    /// Creates a Success state from a response.
    #[must_use]
    pub fn success(response: ResponseSpec) -> Self {
        Self::Success {
            response: Box::new(response),
        }
    }

    /// Creates an Error state.
    #[must_use]
    pub fn error(kind: RequestErrorKind, message: impl Into<String>) -> Self {
        Self::Error {
            kind,
            message: message.into(),
            details: None,
        }
    }

    /// Creates an Error state with details.
    #[must_use]
    pub fn error_with_details(
        kind: RequestErrorKind,
        message: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self::Error {
            kind,
            message: message.into(),
            details: Some(details.into()),
        }
    }

    /// Returns true if the state is Idle.
    #[must_use]
    pub const fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Returns true if a request is in progress.
    #[must_use]
    pub const fn is_loading(&self) -> bool {
        matches!(self, Self::Loading { .. })
    }

    /// Returns true if the last request succeeded.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// Returns true if the last request failed.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    /// Returns the response if in Success state.
    #[must_use]
    pub fn response(&self) -> Option<&ResponseSpec> {
        match self {
            Self::Success { response } => Some(response),
            _ => None,
        }
    }

    /// Returns the elapsed time if loading.
    #[must_use]
    pub fn elapsed(&self) -> Option<std::time::Duration> {
        match self {
            Self::Loading {
                started_at: Some(t),
            } => Some(t.elapsed()),
            _ => None,
        }
    }
}

/// Categories of request errors for user-friendly display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestErrorKind {
    /// Invalid URL format.
    InvalidUrl,

    /// DNS resolution failed.
    DnsError,

    /// Could not establish connection.
    ConnectionFailed,

    /// Connection was refused by the server.
    ConnectionRefused,

    /// Request timed out.
    Timeout,

    /// TLS/SSL error.
    TlsError,

    /// Invalid request body (e.g., malformed JSON).
    InvalidBody,

    /// Too many redirects.
    TooManyRedirects,

    /// Request was cancelled by user.
    Cancelled,

    /// Unknown or unexpected error.
    Unknown,
}

impl RequestErrorKind {
    /// Returns user-friendly suggestions for this error type.
    #[must_use]
    pub const fn suggestions(&self) -> &[&'static str] {
        match self {
            Self::InvalidUrl => &[
                "Check that the URL starts with http:// or https://",
                "Verify there are no typos in the URL",
            ],
            Self::DnsError => &[
                "Check if the hostname is correct",
                "Verify your internet connection",
                "Try using an IP address instead",
            ],
            Self::ConnectionFailed | Self::ConnectionRefused => &[
                "Check if the server is running",
                "Verify the port number is correct",
                "Check your firewall settings",
            ],
            Self::Timeout => &[
                "The server may be slow or overloaded",
                "Try increasing the timeout value",
                "Check your network connection",
            ],
            Self::TlsError => &[
                "The server's SSL certificate may be invalid",
                "Check if the certificate has expired",
                "Verify the hostname matches the certificate",
            ],
            Self::InvalidBody => &[
                "Check that the JSON syntax is valid",
                "Verify all required fields are present",
            ],
            Self::TooManyRedirects => &[
                "The server may have a redirect loop",
                "Try the final URL directly",
            ],
            Self::Cancelled => &["Request was cancelled"],
            Self::Unknown => &[
                "An unexpected error occurred",
                "Check the error details for more information",
            ],
        }
    }

    /// Returns a human-readable title for this error type.
    #[must_use]
    pub const fn title(&self) -> &'static str {
        match self {
            Self::InvalidUrl => "Invalid URL",
            Self::DnsError => "DNS Resolution Failed",
            Self::ConnectionFailed => "Connection Failed",
            Self::ConnectionRefused => "Connection Refused",
            Self::Timeout => "Request Timeout",
            Self::TlsError => "SSL/TLS Error",
            Self::InvalidBody => "Invalid Request Body",
            Self::TooManyRedirects => "Too Many Redirects",
            Self::Cancelled => "Request Cancelled",
            Self::Unknown => "Unknown Error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_state_idle() {
        let state = RequestState::Idle;
        assert!(state.is_idle());
        assert!(!state.is_loading());
        assert!(!state.is_success());
        assert!(!state.is_error());
    }

    #[test]
    fn test_request_state_loading() {
        let state = RequestState::loading();
        assert!(state.is_loading());
        assert!(!state.is_idle());
        assert!(state.elapsed().is_some());
    }

    #[test]
    fn test_request_state_success() {
        let response = ResponseSpec {
            status: 200,
            status_text: "OK".to_string(),
            ..Default::default()
        };
        let state = RequestState::success(response);
        assert!(state.is_success());
        assert!(state.response().is_some());
        assert_eq!(state.response().map(|r| r.status), Some(200));
    }

    #[test]
    fn test_request_state_error() {
        let state = RequestState::error(RequestErrorKind::Timeout, "Request timed out");
        assert!(state.is_error());

        if let RequestState::Error { kind, message, .. } = state {
            assert_eq!(kind, RequestErrorKind::Timeout);
            assert_eq!(message, "Request timed out");
        }
    }

    #[test]
    fn test_request_state_error_with_details() {
        let state = RequestState::error_with_details(
            RequestErrorKind::ConnectionFailed,
            "Connection failed",
            "Connection refused on port 443",
        );

        if let RequestState::Error {
            kind,
            message,
            details,
        } = state
        {
            assert_eq!(kind, RequestErrorKind::ConnectionFailed);
            assert_eq!(message, "Connection failed");
            assert_eq!(details, Some("Connection refused on port 443".to_string()));
        }
    }

    #[test]
    fn test_error_kind_suggestions() {
        let suggestions = RequestErrorKind::ConnectionRefused.suggestions();
        assert!(!suggestions.is_empty());
        assert!(suggestions[0].contains("server"));
    }

    #[test]
    fn test_error_kind_title() {
        assert_eq!(RequestErrorKind::Timeout.title(), "Request Timeout");
        assert_eq!(RequestErrorKind::InvalidUrl.title(), "Invalid URL");
    }
}
