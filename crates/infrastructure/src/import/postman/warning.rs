//! Import Warning System
//!
//! This module provides types for tracking import warnings and issues.

use serde::{Deserialize, Serialize};

/// Warning severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WarningSeverity {
    /// Informational - feature was skipped but not critical
    Info,
    /// Warning - something may not work as expected
    Warning,
    /// Error - import partially failed but continued
    Error,
}

impl std::fmt::Display for WarningSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// An import warning or issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportWarning {
    /// Path to the problematic item (e.g., "collection/folder/request")
    pub path: String,
    /// Human-readable description of the issue
    pub message: String,
    /// Severity level
    pub severity: WarningSeverity,
}

impl ImportWarning {
    /// Create a new warning
    pub fn new(
        path: impl Into<String>,
        message: impl Into<String>,
        severity: WarningSeverity,
    ) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
            severity,
        }
    }

    /// Create an info-level warning
    pub fn info(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(path, message, WarningSeverity::Info)
    }

    /// Create a warning-level warning
    pub fn warning(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(path, message, WarningSeverity::Warning)
    }

    /// Create an error-level warning
    pub fn error(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(path, message, WarningSeverity::Error)
    }

    /// Check if this is an error
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self.severity, WarningSeverity::Error)
    }
}

impl std::fmt::Display for ImportWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}: {}", self.severity, self.path, self.message)
    }
}

/// Aggregate statistics from warnings
#[derive(Debug, Default)]
pub struct WarningStats {
    /// Count of informational warnings
    pub info_count: usize,
    /// Count of warning-level warnings
    pub warning_count: usize,
    /// Count of error-level warnings
    pub error_count: usize,
}

impl WarningStats {
    /// Calculate stats from a list of warnings
    #[must_use]
    pub fn from_warnings(warnings: &[ImportWarning]) -> Self {
        let mut stats = Self::default();
        for w in warnings {
            match w.severity {
                WarningSeverity::Info => stats.info_count += 1,
                WarningSeverity::Warning => stats.warning_count += 1,
                WarningSeverity::Error => stats.error_count += 1,
            }
        }
        stats
    }

    /// Total count of all warnings
    #[must_use]
    pub const fn total(&self) -> usize {
        self.info_count + self.warning_count + self.error_count
    }

    /// Check if there are any errors
    #[must_use]
    pub const fn has_errors(&self) -> bool {
        self.error_count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_warning_creation() {
        let warning = ImportWarning::warning("test/path", "Something went wrong");
        assert_eq!(warning.path, "test/path");
        assert_eq!(warning.severity, WarningSeverity::Warning);
    }

    #[test]
    fn test_warning_stats() {
        let warnings = vec![
            ImportWarning::info("a", "info"),
            ImportWarning::warning("b", "warn"),
            ImportWarning::warning("c", "warn"),
            ImportWarning::error("d", "err"),
        ];

        let stats = WarningStats::from_warnings(&warnings);
        assert_eq!(stats.info_count, 1);
        assert_eq!(stats.warning_count, 2);
        assert_eq!(stats.error_count, 1);
        assert_eq!(stats.total(), 4);
        assert!(stats.has_errors());
    }
}
