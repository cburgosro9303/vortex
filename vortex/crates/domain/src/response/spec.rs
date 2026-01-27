//! Response specification type
//!
//! Contains types for representing HTTP responses including
//! status codes, headers, body, and timing information.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::request::Headers;

/// HTTP status code with semantic helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StatusCode(pub u16);

impl StatusCode {
    /// Creates a new `StatusCode`.
    #[must_use]
    pub const fn new(code: u16) -> Self {
        Self(code)
    }

    /// Returns the numeric status code.
    #[must_use]
    pub const fn as_u16(&self) -> u16 {
        self.0
    }

    /// Returns true if this is a 1xx informational status.
    #[must_use]
    pub const fn is_informational(&self) -> bool {
        self.0 >= 100 && self.0 < 200
    }

    /// Returns true if this is a 2xx success status.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.0 >= 200 && self.0 < 300
    }

    /// Returns true if this is a 3xx redirection status.
    #[must_use]
    pub const fn is_redirection(&self) -> bool {
        self.0 >= 300 && self.0 < 400
    }

    /// Returns true if this is a 4xx client error status.
    #[must_use]
    pub const fn is_client_error(&self) -> bool {
        self.0 >= 400 && self.0 < 500
    }

    /// Returns true if this is a 5xx server error status.
    #[must_use]
    pub const fn is_server_error(&self) -> bool {
        self.0 >= 500 && self.0 < 600
    }

    /// Returns true if this is any error status (4xx or 5xx).
    #[must_use]
    pub const fn is_error(&self) -> bool {
        self.0 >= 400 && self.0 < 600
    }

    /// Returns the canonical reason phrase for common status codes.
    #[must_use]
    pub const fn reason_phrase(&self) -> &'static str {
        match self.0 {
            100 => "Continue",
            101 => "Switching Protocols",
            200 => "OK",
            201 => "Created",
            202 => "Accepted",
            204 => "No Content",
            301 => "Moved Permanently",
            302 => "Found",
            304 => "Not Modified",
            307 => "Temporary Redirect",
            308 => "Permanent Redirect",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            408 => "Request Timeout",
            409 => "Conflict",
            422 => "Unprocessable Entity",
            429 => "Too Many Requests",
            500 => "Internal Server Error",
            501 => "Not Implemented",
            502 => "Bad Gateway",
            503 => "Service Unavailable",
            504 => "Gateway Timeout",
            _ => "Unknown",
        }
    }

    /// Returns a CSS-friendly color category for UI display.
    #[must_use]
    pub const fn color_category(&self) -> StatusColorCategory {
        match self.0 {
            100..=199 => StatusColorCategory::Informational,
            200..=299 => StatusColorCategory::Success,
            300..=399 => StatusColorCategory::Redirection,
            400..=499 => StatusColorCategory::ClientError,
            500..=599 => StatusColorCategory::ServerError,
            _ => StatusColorCategory::Unknown,
        }
    }
}

impl std::fmt::Display for StatusCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.0, self.reason_phrase())
    }
}

impl From<u16> for StatusCode {
    fn from(code: u16) -> Self {
        Self(code)
    }
}

/// Color category for status code display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusColorCategory {
    /// Blue - 1xx
    Informational,
    /// Green - 2xx
    Success,
    /// Blue - 3xx
    Redirection,
    /// Orange - 4xx
    ClientError,
    /// Red - 5xx
    ServerError,
    /// Gray - unknown
    Unknown,
}

/// HTTP response specification.
///
/// Contains all information received from an HTTP call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseSpec {
    /// HTTP status code.
    pub status: u16,
    /// Status text (e.g., "OK", "Not Found")
    pub status_text: String,
    /// Response headers as a map.
    #[serde(default)]
    pub headers_map: HashMap<String, String>,
    /// Response headers (legacy, for compatibility).
    #[serde(default)]
    pub headers: Headers,
    /// Response body as string.
    pub body: String,
    /// Response body as raw bytes (for binary responses).
    #[serde(default, with = "serde_bytes_base64")]
    pub body_bytes: Vec<u8>,
    /// Response time.
    #[serde(with = "duration_millis")]
    pub duration: Duration,
    /// Response size in bytes.
    pub size: usize,
    /// Content-Type header value (extracted for convenience).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

impl ResponseSpec {
    /// Creates a new `ResponseSpec` from raw response data.
    #[must_use]
    pub fn new(
        status: impl Into<StatusCode>,
        headers: HashMap<String, String>,
        body: Vec<u8>,
        duration: Duration,
    ) -> Self {
        let status_code = status.into();
        let size = body.len();
        let content_type = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.clone());

        let body_string = String::from_utf8(body.clone())
            .unwrap_or_else(|_| String::from_utf8_lossy(&body).into_owned());

        Self {
            status: status_code.as_u16(),
            status_text: status_code.reason_phrase().to_string(),
            headers_map: headers,
            headers: Headers::new(),
            body: body_string,
            body_bytes: body,
            duration,
            size,
            content_type,
        }
    }

    /// Returns the status as a `StatusCode` struct.
    #[must_use]
    pub const fn status_code(&self) -> StatusCode {
        StatusCode::new(self.status)
    }

    /// Returns true if the status code indicates success (2xx).
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    /// Returns true if the status code indicates a client error (4xx).
    #[must_use]
    pub const fn is_client_error(&self) -> bool {
        self.status >= 400 && self.status < 500
    }

    /// Returns true if the status code indicates a server error (5xx).
    #[must_use]
    pub const fn is_server_error(&self) -> bool {
        self.status >= 500 && self.status < 600
    }

    /// Attempts to convert the body to a UTF-8 string.
    ///
    /// Returns `None` if the body is not valid UTF-8.
    #[must_use]
    pub fn body_as_string(&self) -> Option<String> {
        if self.body.is_empty() && !self.body_bytes.is_empty() {
            String::from_utf8(self.body_bytes.clone()).ok()
        } else {
            Some(self.body.clone())
        }
    }

    /// Returns the body as a lossy UTF-8 string.
    ///
    /// Invalid UTF-8 sequences are replaced with the replacement character.
    #[must_use]
    pub fn body_as_string_lossy(&self) -> String {
        if self.body.is_empty() && !self.body_bytes.is_empty() {
            String::from_utf8_lossy(&self.body_bytes).into_owned()
        } else {
            self.body.clone()
        }
    }

    /// Attempts to parse the body as JSON.
    #[must_use]
    pub fn body_as_json(&self) -> Option<serde_json::Value> {
        serde_json::from_str(&self.body).ok()
    }

    /// Returns true if the content type indicates JSON.
    #[must_use]
    pub fn is_json(&self) -> bool {
        self.content_type
            .as_ref()
            .is_some_and(|ct| ct.contains("application/json") || ct.contains("+json"))
    }

    /// Returns true if the content type indicates text.
    #[must_use]
    pub fn is_text(&self) -> bool {
        self.content_type
            .as_ref()
            .is_some_and(|ct| ct.starts_with("text/") || ct.contains("xml") || self.is_json())
    }

    /// Returns a human-readable size string (e.g., "1.2 KB").
    #[must_use]
    pub fn size_display(&self) -> String {
        format_bytes(self.size)
    }

    /// Returns a human-readable duration string (e.g., "124 ms").
    #[must_use]
    pub fn duration_display(&self) -> String {
        let millis = self.duration.as_millis();
        if millis < 1000 {
            format!("{millis} ms")
        } else {
            format!("{:.2} s", self.duration.as_secs_f64())
        }
    }

    /// Gets a header value by name (case-insensitive).
    #[must_use]
    pub fn get_header(&self, name: &str) -> Option<&String> {
        self.headers_map
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v)
    }
}

/// Formats bytes into a human-readable string.
fn format_bytes(bytes: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = KB * 1024;
    const GB: usize = MB * 1024;

    #[allow(clippy::cast_precision_loss)]
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

impl Default for ResponseSpec {
    fn default() -> Self {
        Self {
            status: 0,
            status_text: String::new(),
            headers_map: HashMap::new(),
            headers: Headers::new(),
            body: String::new(),
            body_bytes: Vec::new(),
            duration: Duration::ZERO,
            size: 0,
            content_type: None,
        }
    }
}

mod duration_millis {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    #[allow(clippy::cast_possible_truncation)]
    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Truncation is acceptable: durations over ~584 million years are not realistic
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

mod serde_bytes_base64 {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Simple hex encoding for compatibility
        let hex = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        hex.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(serde::de::Error::custom))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_code_categories() {
        assert!(StatusCode::new(100).is_informational());
        assert!(StatusCode::new(200).is_success());
        assert!(StatusCode::new(201).is_success());
        assert!(StatusCode::new(301).is_redirection());
        assert!(StatusCode::new(404).is_client_error());
        assert!(StatusCode::new(500).is_server_error());
        assert!(StatusCode::new(404).is_error());
        assert!(StatusCode::new(500).is_error());
        assert!(!StatusCode::new(200).is_error());
    }

    #[test]
    fn test_status_code_display() {
        assert_eq!(StatusCode::new(200).to_string(), "200 OK");
        assert_eq!(StatusCode::new(404).to_string(), "404 Not Found");
        assert_eq!(
            StatusCode::new(500).to_string(),
            "500 Internal Server Error"
        );
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1_048_576), "1.00 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.00 GB");
    }

    #[test]
    fn test_response_new() {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = ResponseSpec::new(
            200,
            headers,
            b"Hello, World!".to_vec(),
            Duration::from_millis(100),
        );

        assert_eq!(response.status, 200);
        assert_eq!(response.status_text, "OK");
        assert_eq!(response.body, "Hello, World!");
        assert_eq!(response.size, 13);
        assert!(response.is_json());
        assert!(response.is_success());
    }

    #[test]
    fn test_response_body_methods() {
        let response = ResponseSpec::new(
            200,
            HashMap::new(),
            b"test body".to_vec(),
            Duration::from_millis(50),
        );

        assert_eq!(response.body_as_string(), Some("test body".to_string()));
        assert_eq!(response.body_as_string_lossy(), "test body");
    }

    #[test]
    fn test_response_duration_display() {
        let response = ResponseSpec {
            duration: Duration::from_millis(150),
            ..Default::default()
        };
        assert_eq!(response.duration_display(), "150 ms");

        let response2 = ResponseSpec {
            duration: Duration::from_millis(1500),
            ..Default::default()
        };
        assert_eq!(response2.duration_display(), "1.50 s");
    }

    #[test]
    fn test_status_checks() {
        let response_200 = ResponseSpec {
            status: 200,
            ..Default::default()
        };
        assert!(response_200.is_success());
        assert!(!response_200.is_client_error());
        assert!(!response_200.is_server_error());

        let response_404 = ResponseSpec {
            status: 404,
            ..Default::default()
        };
        assert!(!response_404.is_success());
        assert!(response_404.is_client_error());

        let response_500 = ResponseSpec {
            status: 500,
            ..Default::default()
        };
        assert!(response_500.is_server_error());
    }

    #[test]
    fn test_get_header() {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());

        let response = ResponseSpec::new(200, headers, vec![], Duration::ZERO);

        assert_eq!(
            response.get_header("content-type"),
            Some(&"application/json".to_string())
        );
        assert_eq!(
            response.get_header("X-Custom-Header"),
            Some(&"custom-value".to_string())
        );
        assert_eq!(response.get_header("Missing"), None);
    }
}
