//! Error types for Vortex Config.
//!
//! This module defines the error hierarchy used throughout
//! the Vortex Config system. All errors implement the standard
//! `std::error::Error` trait via `thiserror`.
//!
//! # Error Handling Philosophy
//!
//! Vortex follows Rust's explicit error handling approach:
//! - Functions that can fail return `Result<T, VortexError>`
//! - Errors are values, not control flow
//! - Errors should be handled at appropriate boundaries
//!
//! # Example
//!
//! ```
//! use vortex_core::{Result, VortexError};
//!
//! fn get_config(app: &str) -> Result<String> {
//!     if app.is_empty() {
//!         return Err(VortexError::invalid_application(
//!             "",
//!             "Application name cannot be empty",
//!         ));
//!     }
//!     Ok(format!("Config for {}", app))
//! }
//!
//! match get_config("myapp") {
//!     Ok(config) => println!("Got config: {}", config),
//!     Err(e) => eprintln!("Error: {}", e),
//! }
//! ```

use std::io;
use thiserror::Error;

/// Main error type for Vortex Config operations.
///
/// This enum covers all error conditions that can occur when
/// working with configuration in Vortex. Each variant includes
/// context information to help diagnose the issue.
///
/// # Example
///
/// ```
/// use vortex_core::VortexError;
///
/// let error = VortexError::config_not_found("myapp", "prod", Some("v1.0".into()));
/// assert!(error.is_not_found());
/// println!("{}", error); // "Configuration not found..."
/// ```
#[derive(Debug, Error)]
pub enum VortexError {
    /// Configuration was not found for the given coordinates.
    #[error(
        "Configuration not found for application '{application}', profile '{profile}', label '{}'",
        label.as_deref().unwrap_or("default")
    )]
    ConfigNotFound {
        /// Application name that was requested
        application: String,
        /// Profile that was requested
        profile: String,
        /// Label (version) that was requested, if any
        label: Option<String>,
    },

    /// Application name is invalid or empty.
    #[error("Invalid application name '{name}': {reason}")]
    InvalidApplication {
        /// The invalid name provided
        name: String,
        /// Why it's invalid
        reason: String,
    },

    /// Profile name is invalid.
    #[error("Invalid profile name '{name}': {reason}")]
    InvalidProfile {
        /// The invalid profile name
        name: String,
        /// Why it's invalid
        reason: String,
    },

    /// Label (version/branch) is invalid.
    #[error("Invalid label '{name}': {reason}")]
    InvalidLabel {
        /// The invalid label
        name: String,
        /// Why it's invalid
        reason: String,
    },

    /// A required property was not found.
    #[error("Property '{key}' not found in configuration")]
    PropertyNotFound {
        /// The key that was requested
        key: String,
    },

    /// Error parsing configuration content.
    #[error("Failed to parse configuration from '{source_name}': {message}")]
    ParseError {
        /// Source of the configuration (filename, URL, etc.)
        source_name: String,
        /// Description of the parse error
        message: String,
        /// Underlying error, if any
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Error accessing a configuration source/backend.
    #[error("Source error for '{source_name}': {message}")]
    SourceError {
        /// Name of the source that failed
        source_name: String,
        /// Description of what went wrong
        message: String,
        /// Underlying error
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Validation error for configuration values.
    #[error("Validation error for field '{field}': {message}")]
    ValidationError {
        /// Field that failed validation
        field: String,
        /// Description of the validation failure
        message: String,
    },

    /// I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Generic internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl VortexError {
    // ============================================
    // Convenience constructors
    // ============================================

    /// Creates a ConfigNotFound error.
    ///
    /// # Example
    ///
    /// ```
    /// use vortex_core::VortexError;
    ///
    /// let error = VortexError::config_not_found("myapp", "prod", None);
    /// assert!(error.is_not_found());
    /// ```
    pub fn config_not_found(
        application: impl Into<String>,
        profile: impl Into<String>,
        label: Option<String>,
    ) -> Self {
        Self::ConfigNotFound {
            application: application.into(),
            profile: profile.into(),
            label,
        }
    }

    /// Creates an InvalidApplication error.
    pub fn invalid_application(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidApplication {
            name: name.into(),
            reason: reason.into(),
        }
    }

    /// Creates an InvalidProfile error.
    pub fn invalid_profile(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidProfile {
            name: name.into(),
            reason: reason.into(),
        }
    }

    /// Creates an InvalidLabel error.
    pub fn invalid_label(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidLabel {
            name: name.into(),
            reason: reason.into(),
        }
    }

    /// Creates a PropertyNotFound error.
    pub fn property_not_found(key: impl Into<String>) -> Self {
        Self::PropertyNotFound { key: key.into() }
    }

    /// Creates a ParseError without a cause.
    pub fn parse_error(source: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ParseError {
            source_name: source.into(),
            message: message.into(),
            cause: None,
        }
    }

    /// Creates a ParseError with a cause.
    pub fn parse_error_with_cause<E>(
        source: impl Into<String>,
        message: impl Into<String>,
        cause: E,
    ) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::ParseError {
            source_name: source.into(),
            message: message.into(),
            cause: Some(Box::new(cause)),
        }
    }

    /// Creates a SourceError without a cause.
    pub fn source_error(source_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::SourceError {
            source_name: source_name.into(),
            message: message.into(),
            cause: None,
        }
    }

    /// Creates a SourceError with a cause.
    pub fn source_error_with_cause<E>(
        source_name: impl Into<String>,
        message: impl Into<String>,
        cause: E,
    ) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::SourceError {
            source_name: source_name.into(),
            message: message.into(),
            cause: Some(Box::new(cause)),
        }
    }

    /// Creates a ValidationError.
    pub fn validation_error(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ValidationError {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Creates an Internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    // ============================================
    // Query methods
    // ============================================

    /// Returns true if this error indicates the config was not found.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::ConfigNotFound { .. })
    }

    /// Returns true if this is a validation error.
    pub fn is_validation_error(&self) -> bool {
        matches!(self, Self::ValidationError { .. })
    }

    /// Returns true if this is a parse error.
    pub fn is_parse_error(&self) -> bool {
        matches!(self, Self::ParseError { .. })
    }

    /// Returns true if this is a source/backend error.
    pub fn is_source_error(&self) -> bool {
        matches!(self, Self::SourceError { .. })
    }

    /// Returns true if this is an I/O error.
    pub fn is_io_error(&self) -> bool {
        matches!(self, Self::Io(_))
    }
}

/// Type alias for Results with VortexError.
///
/// Use this type for all Vortex operations that can fail.
///
/// # Example
///
/// ```
/// use vortex_core::Result;
///
/// fn process_config(name: &str) -> Result<()> {
///     // Implementation
///     Ok(())
/// }
/// ```
pub type Result<T> = std::result::Result<T, VortexError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_not_found_display() {
        let error = VortexError::config_not_found("myapp", "prod", Some("v1.0".into()));
        let msg = format!("{}", error);

        assert!(msg.contains("myapp"));
        assert!(msg.contains("prod"));
        assert!(msg.contains("v1.0"));
    }

    #[test]
    fn test_config_not_found_without_label() {
        let error = VortexError::config_not_found("myapp", "prod", None);
        let msg = format!("{}", error);

        assert!(msg.contains("default")); // Label por defecto
    }

    #[test]
    fn test_property_not_found() {
        let error = VortexError::property_not_found("database.url");

        assert!(matches!(error, VortexError::PropertyNotFound { .. }));
        assert!(format!("{}", error).contains("database.url"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let vortex_error: VortexError = io_error.into();

        assert!(matches!(vortex_error, VortexError::Io(_)));
        assert!(vortex_error.is_io_error());
    }

    #[test]
    fn test_error_source_chain() {
        let io_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let parse_error =
            VortexError::parse_error_with_cause("config.yml", "Could not read file", io_error);

        // Verificar que source() está implementado
        use std::error::Error;
        assert!(parse_error.source().is_some());
    }

    #[test]
    fn test_is_not_found() {
        let not_found = VortexError::config_not_found("app", "dev", None);
        let parse_error = VortexError::parse_error("file", "bad format");

        assert!(not_found.is_not_found());
        assert!(!parse_error.is_not_found());
    }

    #[test]
    fn test_is_validation_error() {
        let validation = VortexError::validation_error("port", "must be positive");
        let source = VortexError::source_error("git", "clone failed");

        assert!(validation.is_validation_error());
        assert!(!source.is_validation_error());
    }

    #[test]
    fn test_result_with_question_mark() {
        fn inner() -> Result<()> {
            Err(VortexError::internal("test"))
        }

        fn outer() -> Result<String> {
            inner()?; // Propaga el error
            Ok("success".into())
        }

        assert!(outer().is_err());
    }

    #[test]
    fn test_invalid_application() {
        let error = VortexError::invalid_application("", "cannot be empty");
        let msg = format!("{}", error);

        assert!(msg.contains("cannot be empty"));
    }

    #[test]
    fn test_source_error_with_cause() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let error = VortexError::source_error_with_cause("git-backend", "fetch failed", io_error);

        use std::error::Error;
        assert!(error.source().is_some());
        assert!(error.is_source_error());
    }
}
