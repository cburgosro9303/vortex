//! Export format types.
//!
//! This module provides types for exporting requests and collections
//! to various formats like OpenAPI, HAR, and cURL.

use serde::{Deserialize, Serialize};

/// Supported export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    /// OpenAPI 3.0 specification.
    #[default]
    OpenApi3,
    /// HTTP Archive (HAR) format.
    Har,
    /// cURL command.
    Curl,
    /// Postman Collection v2.1.
    PostmanCollection,
    /// Insomnia export format.
    Insomnia,
}

impl ExportFormat {
    /// Get all available formats.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::OpenApi3,
            Self::Har,
            Self::Curl,
            Self::PostmanCollection,
            Self::Insomnia,
        ]
    }

    /// Get the file extension for this format.
    #[must_use]
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::OpenApi3 => "yaml",
            Self::Har => "har",
            Self::Curl => "sh",
            Self::PostmanCollection => "json",
            Self::Insomnia => "json",
        }
    }

    /// Get the MIME type for this format.
    #[must_use]
    pub const fn mime_type(&self) -> &'static str {
        match self {
            Self::OpenApi3 => "application/x-yaml",
            Self::Har => "application/json",
            Self::Curl => "text/x-shellscript",
            Self::PostmanCollection => "application/json",
            Self::Insomnia => "application/json",
        }
    }

    /// Get the display name for this format.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::OpenApi3 => "OpenAPI 3.0",
            Self::Har => "HTTP Archive (HAR)",
            Self::Curl => "cURL Command",
            Self::PostmanCollection => "Postman Collection",
            Self::Insomnia => "Insomnia",
        }
    }

    /// Get a description of this format.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::OpenApi3 => "OpenAPI 3.0 specification for API documentation",
            Self::Har => "HTTP Archive format for browser dev tools",
            Self::Curl => "Shell script with cURL commands",
            Self::PostmanCollection => "Postman Collection v2.1 format",
            Self::Insomnia => "Insomnia REST client format",
        }
    }
}

/// Export options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    /// The format to export to.
    #[serde(default)]
    pub format: ExportFormat,
    /// Whether to include request bodies.
    #[serde(default = "default_true")]
    pub include_body: bool,
    /// Whether to include response examples.
    #[serde(default)]
    pub include_responses: bool,
    /// Whether to include environment variables.
    #[serde(default)]
    pub include_environment: bool,
    /// Whether to include authentication.
    #[serde(default = "default_true")]
    pub include_auth: bool,
    /// Whether to include headers.
    #[serde(default = "default_true")]
    pub include_headers: bool,
    /// Whether to pretty print output.
    #[serde(default = "default_true")]
    pub pretty_print: bool,
    /// OpenAPI specific: API title.
    #[serde(default)]
    pub api_title: Option<String>,
    /// OpenAPI specific: API version.
    #[serde(default)]
    pub api_version: Option<String>,
    /// OpenAPI specific: API description.
    #[serde(default)]
    pub api_description: Option<String>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: ExportFormat::default(),
            include_body: true,
            include_responses: false,
            include_environment: false,
            include_auth: true,
            include_headers: true,
            pretty_print: true,
            api_title: None,
            api_version: None,
            api_description: None,
        }
    }
}

fn default_true() -> bool {
    true
}

impl ExportOptions {
    /// Create new export options.
    #[must_use]
    pub fn new(format: ExportFormat) -> Self {
        Self {
            format,
            ..Default::default()
        }
    }

    /// Set OpenAPI metadata.
    #[must_use]
    pub fn with_api_info(
        mut self,
        title: impl Into<String>,
        version: impl Into<String>,
        description: Option<String>,
    ) -> Self {
        self.api_title = Some(title.into());
        self.api_version = Some(version.into());
        self.api_description = description;
        self
    }
}

/// Result of an export operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    /// The exported content.
    pub content: String,
    /// The format used.
    pub format: ExportFormat,
    /// Number of requests exported.
    pub request_count: usize,
    /// Warnings generated during export.
    #[serde(default)]
    pub warnings: Vec<ExportWarning>,
}

impl ExportResult {
    /// Create a new export result.
    #[must_use]
    pub fn new(content: String, format: ExportFormat, request_count: usize) -> Self {
        Self {
            content,
            format,
            request_count,
            warnings: Vec::new(),
        }
    }

    /// Add a warning.
    pub fn add_warning(&mut self, warning: ExportWarning) {
        self.warnings.push(warning);
    }

    /// Check if there were any warnings.
    #[must_use]
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Get the suggested filename.
    #[must_use]
    pub fn suggested_filename(&self, base_name: &str) -> String {
        format!("{}.{}", base_name, self.format.extension())
    }
}

/// Warning generated during export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportWarning {
    /// Warning message.
    pub message: String,
    /// The request or item that generated the warning.
    pub source: Option<String>,
    /// Warning severity.
    pub severity: WarningSeverity,
}

impl ExportWarning {
    /// Create a new warning.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: None,
            severity: WarningSeverity::Warning,
        }
    }

    /// Set the source.
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set as info severity.
    #[must_use]
    pub const fn as_info(mut self) -> Self {
        self.severity = WarningSeverity::Info;
        self
    }
}

/// Warning severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WarningSeverity {
    /// Informational.
    Info,
    /// Warning (may affect export quality).
    #[default]
    Warning,
    /// Error (export may be incomplete).
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_format_all() {
        let formats = ExportFormat::all();
        assert_eq!(formats.len(), 5);
        assert!(formats.contains(&ExportFormat::OpenApi3));
        assert!(formats.contains(&ExportFormat::Har));
    }

    #[test]
    fn test_export_format_extension() {
        assert_eq!(ExportFormat::OpenApi3.extension(), "yaml");
        assert_eq!(ExportFormat::Har.extension(), "har");
        assert_eq!(ExportFormat::Curl.extension(), "sh");
    }

    #[test]
    fn test_export_format_display() {
        assert_eq!(ExportFormat::OpenApi3.display_name(), "OpenAPI 3.0");
        assert_eq!(ExportFormat::Har.display_name(), "HTTP Archive (HAR)");
    }

    #[test]
    fn test_export_options_new() {
        let options = ExportOptions::new(ExportFormat::Har);
        assert_eq!(options.format, ExportFormat::Har);
        assert!(options.include_body);
        assert!(options.include_headers);
        assert!(options.pretty_print);
    }

    #[test]
    fn test_export_options_with_api_info() {
        let options = ExportOptions::new(ExportFormat::OpenApi3).with_api_info(
            "My API",
            "1.0.0",
            Some("Description".to_string()),
        );

        assert_eq!(options.api_title, Some("My API".to_string()));
        assert_eq!(options.api_version, Some("1.0.0".to_string()));
        assert_eq!(options.api_description, Some("Description".to_string()));
    }

    #[test]
    fn test_export_result() {
        let mut result = ExportResult::new("content".to_string(), ExportFormat::Har, 5);
        assert_eq!(result.request_count, 5);
        assert!(!result.has_warnings());

        result.add_warning(ExportWarning::new("Test warning"));
        assert!(result.has_warnings());
    }

    #[test]
    fn test_suggested_filename() {
        let result = ExportResult::new(String::new(), ExportFormat::OpenApi3, 0);
        assert_eq!(result.suggested_filename("my-api"), "my-api.yaml");

        let result = ExportResult::new(String::new(), ExportFormat::Har, 0);
        assert_eq!(result.suggested_filename("requests"), "requests.har");
    }

    #[test]
    fn test_export_warning() {
        let warning = ExportWarning::new("Missing body")
            .with_source("POST /users")
            .as_info();

        assert_eq!(warning.message, "Missing body");
        assert_eq!(warning.source, Some("POST /users".to_string()));
        assert_eq!(warning.severity, WarningSeverity::Info);
    }
}
