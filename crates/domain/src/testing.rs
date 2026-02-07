//! Response testing and assertions.
//!
//! This module provides types for defining and executing tests on HTTP responses.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A test assertion to run against a response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Assertion {
    /// Check response status code.
    StatusCode {
        /// Expected status code or range.
        expected: StatusExpectation,
    },
    /// Check response time.
    ResponseTime {
        /// Maximum allowed time in milliseconds.
        max_ms: u64,
    },
    /// Check header exists and optionally its value.
    HeaderExists {
        /// Header name (case-insensitive).
        name: String,
        /// Optional expected value.
        value: Option<String>,
    },
    /// Check header value matches pattern.
    HeaderMatches {
        /// Header name.
        name: String,
        /// Regex pattern to match.
        pattern: String,
    },
    /// Check body contains text.
    BodyContains {
        /// Text to search for.
        text: String,
        /// Case-insensitive search.
        #[serde(default)]
        ignore_case: bool,
    },
    /// Check body matches regex pattern.
    BodyMatches {
        /// Regex pattern.
        pattern: String,
    },
    /// Check JSON path exists and optionally its value.
    JsonPath {
        /// JSONPath expression (e.g., "$.data.id").
        path: String,
        /// Expected value (as JSON).
        expected: Option<serde_json::Value>,
    },
    /// Check JSON path value matches condition.
    JsonPathMatches {
        /// JSONPath expression.
        path: String,
        /// Comparison operator.
        operator: ComparisonOperator,
        /// Value to compare against.
        value: serde_json::Value,
    },
    /// Check body equals expected value.
    BodyEquals {
        /// Expected body content.
        expected: String,
    },
    /// Check body is valid JSON.
    IsJson,
    /// Check body is valid XML.
    IsXml,
    /// Check content type.
    ContentType {
        /// Expected content type (partial match).
        expected: String,
    },
    /// Check body length.
    BodyLength {
        /// Comparison operator.
        operator: ComparisonOperator,
        /// Length to compare against.
        length: usize,
    },
}

impl Assertion {
    /// Get a human-readable description of this assertion.
    #[must_use]
    pub fn description(&self) -> String {
        match self {
            Self::StatusCode { expected } => format!("Status code {}", expected.description()),
            Self::ResponseTime { max_ms } => format!("Response time < {}ms", max_ms),
            Self::HeaderExists {
                name,
                value: Some(v),
            } => {
                format!("Header '{}' equals '{}'", name, v)
            }
            Self::HeaderExists { name, value: None } => format!("Header '{}' exists", name),
            Self::HeaderMatches { name, pattern } => {
                format!("Header '{}' matches /{}/", name, pattern)
            }
            Self::BodyContains { text, .. } => format!("Body contains '{}'", text),
            Self::BodyMatches { pattern } => format!("Body matches /{}/", pattern),
            Self::JsonPath {
                path,
                expected: Some(v),
            } => {
                format!("JSON {} equals {}", path, v)
            }
            Self::JsonPath {
                path,
                expected: None,
            } => format!("JSON {} exists", path),
            Self::JsonPathMatches {
                path,
                operator,
                value,
            } => {
                format!("JSON {} {} {}", path, operator.symbol(), value)
            }
            Self::BodyEquals { .. } => "Body equals expected".to_string(),
            Self::IsJson => "Body is valid JSON".to_string(),
            Self::IsXml => "Body is valid XML".to_string(),
            Self::ContentType { expected } => format!("Content-Type contains '{}'", expected),
            Self::BodyLength { operator, length } => {
                format!("Body length {} {}", operator.symbol(), length)
            }
        }
    }
}

/// Expected status code value or range.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum StatusExpectation {
    /// Exact status code.
    Exact(u16),
    /// Range of status codes (e.g., 200-299).
    Range {
        /// Minimum status code (inclusive).
        min: u16,
        /// Maximum status code (inclusive).
        max: u16,
    },
    /// One of multiple status codes.
    OneOf(Vec<u16>),
}

impl StatusExpectation {
    /// Check if a status code matches this expectation.
    #[must_use]
    pub fn matches(&self, status: u16) -> bool {
        match self {
            Self::Exact(expected) => status == *expected,
            Self::Range { min, max } => status >= *min && status <= *max,
            Self::OneOf(codes) => codes.contains(&status),
        }
    }

    /// Get description of the expectation.
    #[must_use]
    pub fn description(&self) -> String {
        match self {
            Self::Exact(code) => format!("= {}", code),
            Self::Range { min, max } => format!("in {}-{}", min, max),
            Self::OneOf(codes) => {
                let codes_str: Vec<_> = codes.iter().map(ToString::to_string).collect();
                format!("in [{}]", codes_str.join(", "))
            }
        }
    }

    /// Create a "success" expectation (200-299).
    #[must_use]
    pub const fn success() -> Self {
        Self::Range { min: 200, max: 299 }
    }

    /// Create an exact status expectation.
    #[must_use]
    pub const fn exact(code: u16) -> Self {
        Self::Exact(code)
    }
}

impl Default for StatusExpectation {
    fn default() -> Self {
        Self::success()
    }
}

/// Comparison operators for value assertions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonOperator {
    /// Equal to.
    Equals,
    /// Not equal to.
    NotEquals,
    /// Greater than.
    GreaterThan,
    /// Greater than or equal to.
    GreaterThanOrEqual,
    /// Less than.
    LessThan,
    /// Less than or equal to.
    LessThanOrEqual,
    /// Contains (for strings/arrays).
    Contains,
    /// Matches regex pattern.
    Matches,
}

impl ComparisonOperator {
    /// Get the symbol for this operator.
    #[must_use]
    pub const fn symbol(&self) -> &'static str {
        match self {
            Self::Equals => "==",
            Self::NotEquals => "!=",
            Self::GreaterThan => ">",
            Self::GreaterThanOrEqual => ">=",
            Self::LessThan => "<",
            Self::LessThanOrEqual => "<=",
            Self::Contains => "contains",
            Self::Matches => "matches",
        }
    }
}

/// Result of running a single assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    /// The assertion that was run.
    pub assertion: Assertion,
    /// Whether the assertion passed.
    pub passed: bool,
    /// Actual value found (for display).
    pub actual: Option<String>,
    /// Error message if failed.
    pub error: Option<String>,
}

impl AssertionResult {
    /// Create a passed result.
    #[must_use]
    pub fn pass(assertion: Assertion) -> Self {
        Self {
            assertion,
            passed: true,
            actual: None,
            error: None,
        }
    }

    /// Create a passed result with actual value.
    #[must_use]
    pub fn pass_with_value(assertion: Assertion, actual: impl Into<String>) -> Self {
        Self {
            assertion,
            passed: true,
            actual: Some(actual.into()),
            error: None,
        }
    }

    /// Create a failed result.
    #[must_use]
    pub fn fail(assertion: Assertion, error: impl Into<String>) -> Self {
        Self {
            assertion,
            passed: false,
            actual: None,
            error: Some(error.into()),
        }
    }

    /// Create a failed result with actual value.
    #[must_use]
    pub fn fail_with_value(
        assertion: Assertion,
        actual: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            assertion,
            passed: false,
            actual: Some(actual.into()),
            error: Some(error.into()),
        }
    }
}

/// A test suite containing multiple assertions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TestSuite {
    /// Unique identifier.
    #[serde(default = "generate_id")]
    pub id: Uuid,
    /// Test suite name.
    pub name: String,
    /// Assertions to run.
    #[serde(default)]
    pub assertions: Vec<Assertion>,
    /// Whether to stop on first failure.
    #[serde(default)]
    pub stop_on_failure: bool,
}

fn generate_id() -> Uuid {
    Uuid::now_v7()
}

impl TestSuite {
    /// Create a new empty test suite.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            name: name.into(),
            assertions: Vec::new(),
            stop_on_failure: false,
        }
    }

    /// Add an assertion to the suite.
    pub fn add(&mut self, assertion: Assertion) {
        self.assertions.push(assertion);
    }

    /// Add an assertion (builder pattern).
    pub fn with_assertion(mut self, assertion: Assertion) -> Self {
        self.assertions.push(assertion);
        self
    }

    /// Check if the suite is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.assertions.is_empty()
    }

    /// Get the number of assertions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.assertions.len()
    }
}

/// Results from running a test suite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResults {
    /// Test suite that was run.
    pub suite_name: String,
    /// Individual assertion results.
    pub results: Vec<AssertionResult>,
    /// Total number of assertions.
    pub total: usize,
    /// Number of passed assertions.
    pub passed: usize,
    /// Number of failed assertions.
    pub failed: usize,
    /// Execution time in milliseconds.
    pub duration_ms: u64,
}

impl TestResults {
    /// Create new test results.
    #[must_use]
    pub fn new(
        suite_name: impl Into<String>,
        results: Vec<AssertionResult>,
        duration_ms: u64,
    ) -> Self {
        let total = results.len();
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = total - passed;

        Self {
            suite_name: suite_name.into(),
            results,
            total,
            passed,
            failed,
            duration_ms,
        }
    }

    /// Check if all tests passed.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }

    /// Get pass rate as percentage.
    #[must_use]
    pub fn pass_rate(&self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            (self.passed as f64 / self.total as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_expectation_exact() {
        let exp = StatusExpectation::exact(200);
        assert!(exp.matches(200));
        assert!(!exp.matches(201));
    }

    #[test]
    fn test_status_expectation_range() {
        let exp = StatusExpectation::success();
        assert!(exp.matches(200));
        assert!(exp.matches(201));
        assert!(exp.matches(299));
        assert!(!exp.matches(300));
        assert!(!exp.matches(199));
    }

    #[test]
    fn test_status_expectation_one_of() {
        let exp = StatusExpectation::OneOf(vec![200, 201, 204]);
        assert!(exp.matches(200));
        assert!(exp.matches(201));
        assert!(exp.matches(204));
        assert!(!exp.matches(202));
    }

    #[test]
    fn test_assertion_description() {
        let assertion = Assertion::StatusCode {
            expected: StatusExpectation::exact(200),
        };
        assert_eq!(assertion.description(), "Status code = 200");

        let assertion = Assertion::BodyContains {
            text: "success".to_string(),
            ignore_case: false,
        };
        assert_eq!(assertion.description(), "Body contains 'success'");
    }

    #[test]
    fn test_test_suite_builder() {
        let suite = TestSuite::new("API Tests")
            .with_assertion(Assertion::StatusCode {
                expected: StatusExpectation::success(),
            })
            .with_assertion(Assertion::IsJson);

        assert_eq!(suite.name, "API Tests");
        assert_eq!(suite.len(), 2);
    }

    #[test]
    fn test_test_results() {
        let results = vec![
            AssertionResult::pass(Assertion::StatusCode {
                expected: StatusExpectation::exact(200),
            }),
            AssertionResult::fail(Assertion::IsJson, "Invalid JSON"),
        ];

        let test_results = TestResults::new("Suite", results, 100);
        assert_eq!(test_results.total, 2);
        assert_eq!(test_results.passed, 1);
        assert_eq!(test_results.failed, 1);
        assert!(!test_results.all_passed());
        assert_eq!(test_results.pass_rate(), 50.0);
    }
}
