//! Application error types

use thiserror::Error;
use vortex_domain::DomainError;

/// Application-level errors.
#[derive(Debug, Error)]
pub enum ApplicationError {
    /// A domain validation error occurred.
    #[error("domain error: {0}")]
    Domain(#[from] DomainError),

    /// An HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(String),

    /// A storage operation failed.
    #[error("storage error: {0}")]
    Storage(String),

    /// The requested resource was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// An internal error occurred.
    #[error("internal error: {0}")]
    Internal(String),

    /// The operation timed out.
    #[error("operation timed out")]
    Timeout,

    /// The operation was cancelled.
    #[error("operation cancelled")]
    Cancelled,
}

/// Result type alias for application operations.
pub type ApplicationResult<T> = Result<T, ApplicationError>;
