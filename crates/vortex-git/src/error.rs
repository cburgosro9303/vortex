//! Error types for configuration sources.

use std::path::PathBuf;

/// Errors that can occur when working with configuration sources.
#[derive(Debug, thiserror::Error)]
pub enum ConfigSourceError {
    /// The requested application was not found.
    #[error("application not found: {0}")]
    ApplicationNotFound(String),

    /// The requested profile was not found.
    #[error("profile not found: {0}")]
    ProfileNotFound(String),

    /// The requested label (branch/tag/commit) was not found.
    #[error("label not found: {0}")]
    LabelNotFound(String),

    /// The configuration source is not available.
    #[error("source unavailable: {reason}")]
    SourceUnavailable { reason: String },

    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A Git operation failed.
    #[error("git error: {0}")]
    Git(String),

    /// Failed to parse a configuration file.
    #[error("parse error in {path}: {reason}")]
    Parse { path: PathBuf, reason: String },

    /// The configuration format is not supported.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    /// A timeout occurred while waiting for an operation.
    #[error("operation timed out after {seconds}s")]
    Timeout { seconds: u64 },

    /// The source is currently refreshing.
    #[error("source is refreshing, try again later")]
    Refreshing,

    /// Invalid configuration.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
}

impl ConfigSourceError {
    /// Creates a new Git error.
    pub fn git(msg: impl Into<String>) -> Self {
        Self::Git(msg.into())
    }

    /// Creates a new parse error.
    pub fn parse(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        Self::Parse {
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// Creates a new source unavailable error.
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self::SourceUnavailable {
            reason: reason.into(),
        }
    }

    /// Returns true if this is a transient error that might succeed on retry.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::SourceUnavailable { .. } | Self::Timeout { .. } | Self::Refreshing
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ConfigSourceError::ApplicationNotFound("myapp".to_string());
        assert_eq!(err.to_string(), "application not found: myapp");

        let err = ConfigSourceError::LabelNotFound("feature/test".to_string());
        assert_eq!(err.to_string(), "label not found: feature/test");

        let err = ConfigSourceError::git("failed to clone");
        assert_eq!(err.to_string(), "git error: failed to clone");

        let err = ConfigSourceError::parse("/config/app.yml", "invalid YAML");
        assert_eq!(
            err.to_string(),
            "parse error in /config/app.yml: invalid YAML"
        );
    }

    #[test]
    fn test_is_transient() {
        assert!(ConfigSourceError::unavailable("network error").is_transient());
        assert!(ConfigSourceError::Timeout { seconds: 30 }.is_transient());
        assert!(ConfigSourceError::Refreshing.is_transient());
        assert!(!ConfigSourceError::ApplicationNotFound("app".to_string()).is_transient());
        assert!(!ConfigSourceError::git("error").is_transient());
    }
}
