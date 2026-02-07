//! Pre-request and post-response scripting.
//!
//! This module provides types for defining and executing scripts
//! that run before requests or after responses.

use serde::{Deserialize, Serialize};

/// A script that can be executed before a request or after a response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Script {
    /// The script content.
    pub content: String,
    /// Whether the script is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Script language/type.
    #[serde(default)]
    pub language: ScriptLanguage,
}

fn default_enabled() -> bool {
    true
}

impl Default for Script {
    fn default() -> Self {
        Self {
            content: String::new(),
            enabled: true,
            language: ScriptLanguage::default(),
        }
    }
}

impl Script {
    /// Create a new empty script.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new script with content.
    #[must_use]
    pub fn with_content(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            enabled: true,
            language: ScriptLanguage::default(),
        }
    }

    /// Check if the script is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty()
    }

    /// Check if the script should run.
    #[must_use]
    pub fn should_run(&self) -> bool {
        self.enabled && !self.is_empty()
    }
}

/// Script language type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScriptLanguage {
    /// Simple Vortex DSL (default).
    #[default]
    VortexDsl,
}

/// A script command that can be executed.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptCommand {
    /// Set a variable: set("name", "value")
    SetVariable {
        /// Variable name.
        name: String,
        /// Variable value.
        value: String,
    },
    /// Set a header: setHeader("name", "value")
    SetHeader {
        /// Header name.
        name: String,
        /// Header value.
        value: String,
    },
    /// Set a query parameter: setParam("name", "value")
    SetQueryParam {
        /// Parameter name.
        name: String,
        /// Parameter value.
        value: String,
    },
    /// Log a message: log("message")
    Log {
        /// The message to log.
        message: String,
    },
    /// Skip the request: skip()
    Skip,
    /// Delay execution: delay(ms)
    Delay {
        /// Delay in milliseconds.
        millis: u64,
    },
    /// Assert a condition: assert(condition, message)
    Assert {
        /// The condition expression.
        condition: String,
        /// Optional message if assertion fails.
        message: Option<String>,
    },
}

/// Result of script execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptResult {
    /// Whether execution was successful.
    pub success: bool,
    /// Variables that were set.
    pub variables: Vec<(String, String)>,
    /// Headers that were set.
    pub headers: Vec<(String, String)>,
    /// Query parameters that were set.
    pub query_params: Vec<(String, String)>,
    /// Log messages generated.
    pub logs: Vec<String>,
    /// Whether the request should be skipped.
    pub skip_request: bool,
    /// Delay to apply before request (milliseconds).
    pub delay_millis: u64,
    /// Error message if execution failed.
    pub error: Option<String>,
}

impl Default for ScriptResult {
    fn default() -> Self {
        Self {
            success: true,
            variables: Vec::new(),
            headers: Vec::new(),
            query_params: Vec::new(),
            logs: Vec::new(),
            skip_request: false,
            delay_millis: 0,
            error: None,
        }
    }
}

impl ScriptResult {
    /// Create a successful empty result.
    #[must_use]
    pub fn success() -> Self {
        Self::default()
    }

    /// Create a failed result with an error message.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            error: Some(message.into()),
            ..Default::default()
        }
    }

    /// Add a variable to the result.
    pub fn add_variable(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.variables.push((name.into(), value.into()));
    }

    /// Add a header to the result.
    pub fn add_header(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.headers.push((name.into(), value.into()));
    }

    /// Add a query parameter to the result.
    pub fn add_query_param(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.query_params.push((name.into(), value.into()));
    }

    /// Add a log message.
    pub fn add_log(&mut self, message: impl Into<String>) {
        self.logs.push(message.into());
    }
}

/// Pre-request and post-response scripts for a request.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RequestScripts {
    /// Script to run before the request.
    #[serde(default, skip_serializing_if = "Script::is_empty")]
    pub pre_request: Script,
    /// Script to run after the response.
    #[serde(default, skip_serializing_if = "Script::is_empty")]
    pub post_response: Script,
}

impl RequestScripts {
    /// Create new empty scripts.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if both scripts are empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pre_request.is_empty() && self.post_response.is_empty()
    }

    /// Set the pre-request script.
    #[must_use]
    pub fn with_pre_request(mut self, script: Script) -> Self {
        self.pre_request = script;
        self
    }

    /// Set the post-response script.
    #[must_use]
    pub fn with_post_response(mut self, script: Script) -> Self {
        self.post_response = script;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_new() {
        let script = Script::new();
        assert!(script.is_empty());
        assert!(script.enabled);
    }

    #[test]
    fn test_script_with_content() {
        let script = Script::with_content("set(\"key\", \"value\")");
        assert!(!script.is_empty());
        assert!(script.should_run());
    }

    #[test]
    fn test_script_disabled() {
        let mut script = Script::with_content("log(\"hello\")");
        script.enabled = false;
        assert!(!script.should_run());
    }

    #[test]
    fn test_script_result_default() {
        let result = ScriptResult::default();
        assert!(result.success);
        assert!(result.error.is_none());
        assert!(!result.skip_request);
    }

    #[test]
    fn test_script_result_error() {
        let result = ScriptResult::error("Something went wrong");
        assert!(!result.success);
        assert_eq!(result.error, Some("Something went wrong".to_string()));
    }

    #[test]
    fn test_script_result_add_variable() {
        let mut result = ScriptResult::success();
        result.add_variable("token", "abc123");
        assert_eq!(
            result.variables,
            vec![("token".to_string(), "abc123".to_string())]
        );
    }

    #[test]
    fn test_request_scripts_is_empty() {
        let scripts = RequestScripts::new();
        assert!(scripts.is_empty());
    }

    #[test]
    fn test_request_scripts_with_pre_request() {
        let scripts = RequestScripts::new().with_pre_request(Script::with_content("log(\"pre\")"));
        assert!(!scripts.is_empty());
    }
}
