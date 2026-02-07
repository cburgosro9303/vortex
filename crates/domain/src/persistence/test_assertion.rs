//! Test assertion types for request validation.
//!
//! Note: Full test execution is Sprint 06 scope.
//! This sprint only defines the types for serialization.

use serde::{Deserialize, Serialize};

/// A test assertion to run after request execution.
///
/// The `type` field is used as the discriminator for JSON serialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TestAssertion {
    /// Assert exact status code.
    Status {
        /// Test name for display.
        name: String,
        /// Expected status code.
        expected: u16,
    },

    /// Assert status code is within range.
    StatusRange {
        /// Test name for display.
        name: String,
        /// Minimum status code (inclusive).
        min: u16,
        /// Maximum status code (inclusive).
        max: u16,
    },

    /// Assert header exists.
    HeaderExists {
        /// Test name for display.
        name: String,
        /// Header name to check.
        header: String,
    },

    /// Assert header has specific value.
    HeaderEquals {
        /// Test name for display.
        name: String,
        /// Header name to check.
        header: String,
        /// Expected header value.
        expected: String,
    },

    /// Assert body contains substring.
    BodyContains {
        /// Test name for display.
        name: String,
        /// Expected substring in body.
        expected: String,
    },

    /// Assert JSON path exists in response.
    JsonPathExists {
        /// Test name for display.
        name: String,
        /// `JSONPath` expression (e.g., `$.data.id`).
        path: String,
    },

    /// Assert JSON path has specific value.
    JsonPathEquals {
        /// Test name for display.
        name: String,
        /// `JSONPath` expression.
        path: String,
        /// Expected value at path.
        expected: serde_json::Value,
    },

    /// Assert response time is under threshold.
    ResponseTime {
        /// Test name for display.
        name: String,
        /// Maximum allowed response time in milliseconds.
        max_ms: u64,
    },
}

impl TestAssertion {
    /// Creates a status code assertion.
    #[must_use]
    pub fn status(name: impl Into<String>, expected: u16) -> Self {
        Self::Status {
            name: name.into(),
            expected,
        }
    }

    /// Creates a status range assertion.
    #[must_use]
    pub fn status_range(name: impl Into<String>, min: u16, max: u16) -> Self {
        Self::StatusRange {
            name: name.into(),
            min,
            max,
        }
    }

    /// Creates a header exists assertion.
    #[must_use]
    pub fn header_exists(name: impl Into<String>, header: impl Into<String>) -> Self {
        Self::HeaderExists {
            name: name.into(),
            header: header.into(),
        }
    }

    /// Creates a response time assertion.
    #[must_use]
    pub fn response_time(name: impl Into<String>, max_ms: u64) -> Self {
        Self::ResponseTime {
            name: name.into(),
            max_ms,
        }
    }

    /// Creates a body contains assertion.
    #[must_use]
    pub fn body_contains(name: impl Into<String>, expected: impl Into<String>) -> Self {
        Self::BodyContains {
            name: name.into(),
            expected: expected.into(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_status_assertion() {
        let assertion = TestAssertion::status("Should return 200", 200);
        match assertion {
            TestAssertion::Status { name, expected } => {
                assert_eq!(name, "Should return 200");
                assert_eq!(expected, 200);
            }
            _ => panic!("Expected Status assertion"),
        }
    }

    #[test]
    fn test_status_range_assertion() {
        let assertion = TestAssertion::status_range("Should return 2xx", 200, 299);
        match assertion {
            TestAssertion::StatusRange { name, min, max } => {
                assert_eq!(name, "Should return 2xx");
                assert_eq!(min, 200);
                assert_eq!(max, 299);
            }
            _ => panic!("Expected StatusRange assertion"),
        }
    }

    #[test]
    fn test_response_time_assertion() {
        let assertion = TestAssertion::response_time("Should respond under 500ms", 500);
        match assertion {
            TestAssertion::ResponseTime { name, max_ms } => {
                assert_eq!(name, "Should respond under 500ms");
                assert_eq!(max_ms, 500);
            }
            _ => panic!("Expected ResponseTime assertion"),
        }
    }
}
