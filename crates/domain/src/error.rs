//! Domain error types

use thiserror::Error;

/// Domain-level errors that can occur during validation or processing.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// The provided URL is invalid or malformed.
    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    /// A required header name is invalid.
    #[error("invalid header name: {0}")]
    InvalidHeaderName(String),

    /// A required header value is invalid.
    #[error("invalid header value: {0}")]
    InvalidHeaderValue(String),

    /// The HTTP method is not supported.
    #[error("unsupported HTTP method: {0}")]
    UnsupportedMethod(String),

    /// A variable reference is malformed.
    #[error("invalid variable reference: {0}")]
    InvalidVariableReference(String),

    /// The request body is invalid for the given content type.
    #[error("invalid body: {0}")]
    InvalidBody(String),

    /// A collection item has an invalid structure.
    #[error("invalid collection item: {0}")]
    InvalidCollectionItem(String),

    /// An identifier is invalid or empty.
    #[error("invalid identifier: {0}")]
    InvalidIdentifier(String),
}

/// Result type alias for domain operations.
pub type DomainResult<T> = Result<T, DomainError>;
