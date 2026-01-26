# Sprint 06 - Tests, Collection Runner & Reporting

**Objective:** Implement automated test assertions for requests, batch execution of collections, and comprehensive reporting for CI integration.

**Milestone:** M6
**Duration:** 5-7 days
**Prerequisites:** Sprint 05 completed (Auth + Advanced Bodies)

---

## Table of Contents

1. [Scope](#scope)
2. [Out of Scope](#out-of-scope)
3. [Architecture Overview](#architecture-overview)
4. [Implementation Order](#implementation-order)
5. [Task Details](#task-details)
   - [T01: Test Assertion Types](#t01-test-assertion-types)
   - [T02: Assertion Evaluator](#t02-assertion-evaluator)
   - [T03: Test Result Types](#t03-test-result-types)
   - [T04: JSON Path Support](#t04-json-path-support)
   - [T05: Collection Runner](#t05-collection-runner)
   - [T06: Run Progress Events](#t06-run-progress-events)
   - [T07: Report Generation](#t07-report-generation)
   - [T08: JUnit XML Export](#t08-junit-xml-export)
   - [T09: UI Test Editor](#t09-ui-test-editor)
   - [T10: UI Collection Runner Panel](#t10-ui-collection-runner-panel)
   - [T11: UI Results View](#t11-ui-results-view)
6. [Acceptance Criteria](#acceptance-criteria)
7. [Verification Commands](#verification-commands)
8. [Risks and Mitigations](#risks-and-mitigations)

---

## Scope

- Test assertion definitions per request (status, headers, body, JSON path, timing)
- Assertion evaluator engine that validates responses against assertions
- Collection runner with sequential/parallel execution modes
- Progress events and cancellation support
- JSON and JUnit XML report generation for CI integration
- UI for editing tests, running collections, and viewing results

## Out of Scope

- Public CLI interface (future sprint)
- External CI service integrations (webhooks, etc.)
- Script-based assertions (JavaScript/Lua)
- Pre-request scripts
- Data-driven testing with CSV/JSON files

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              UI Layer                                    │
│  ┌─────────────────┐  ┌──────────────────┐  ┌───────────────────────┐  │
│  │  Test Editor    │  │  Runner Panel    │  │  Results View         │  │
│  │  - Add/Edit     │  │  - Start/Stop    │  │  - Pass/Fail list     │  │
│  │  - Type picker  │  │  - Progress bar  │  │  - Details expand     │  │
│  │  - Validation   │  │  - Live status   │  │  - Export buttons     │  │
│  └────────┬────────┘  └────────┬─────────┘  └───────────┬───────────┘  │
│           │                    │                        │               │
└───────────┼────────────────────┼────────────────────────┼───────────────┘
            │                    │                        │
            ▼                    ▼                        ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                          Application Layer                               │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    CollectionRunner                              │   │
│  │  - Execute requests in order                                     │   │
│  │  - Apply RunnerConfig (parallel, retry, delay)                   │   │
│  │  - Emit RunProgress events                                       │   │
│  │  - Support cancellation via CancellationToken                    │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                   AssertionEvaluator                             │   │
│  │  - Evaluate TestAssertion against ResponseSpec                   │   │
│  │  - Return TestResult with pass/fail/error status                 │   │
│  │  - Measure assertion evaluation time                             │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    ReportGenerator                               │   │
│  │  - Aggregate results into RunReport                              │   │
│  │  - Serialize to JSON format                                      │   │
│  │  - Serialize to JUnit XML format                                 │   │
│  └─────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
            │                    │                        │
            ▼                    ▼                        ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                           Domain Layer                                   │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────────────┐    │
│  │ TestAssertion  │  │  TestResult    │  │  RunReport             │    │
│  │ (8 variants)   │  │  (outcome +    │  │  (summary + details)   │    │
│  │                │  │   duration)    │  │                        │    │
│  └────────────────┘  └────────────────┘  └────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Implementation Order

Tasks must be completed in this order due to dependencies:

```
[T01] Test Assertion Types (domain)
  │
  ├──► [T02] Assertion Evaluator (application)
  │      │
  │      └──► [T04] JSON Path Support (infrastructure)
  │
  └──► [T03] Test Result Types (domain)
        │
        └──► [T05] Collection Runner (application)
              │
              ├──► [T06] Run Progress Events (application)
              │
              └──► [T07] Report Generation (application)
                    │
                    └──► [T08] JUnit XML Export (infrastructure)

[T09] UI Test Editor ──► [T10] UI Runner Panel ──► [T11] UI Results View
```

---

## Task Details

### T01: Test Assertion Types

Define all assertion types in the domain layer.

#### File: `crates/domain/src/testing/mod.rs`

```rust
//! Testing domain types for request assertions and results.

mod assertion;
mod result;

pub use assertion::{TestAssertion, JsonPathOperator};
pub use result::{TestResult, TestOutcome, AssertionError};
```

#### File: `crates/domain/src/testing/assertion.rs`

```rust
//! Test assertion definitions.

use serde::{Deserialize, Serialize};

/// Operator for JSON path comparisons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JsonPathOperator {
    /// Exact equality
    Equals,
    /// Not equal
    NotEquals,
    /// Contains substring (for strings) or element (for arrays)
    Contains,
    /// Greater than (numeric)
    GreaterThan,
    /// Less than (numeric)
    LessThan,
    /// Greater than or equal (numeric)
    GreaterThanOrEqual,
    /// Less than or equal (numeric)
    LessThanOrEqual,
    /// Matches regex pattern
    Matches,
}

impl Default for JsonPathOperator {
    fn default() -> Self {
        Self::Equals
    }
}

/// A test assertion to validate against a response.
///
/// Each variant corresponds to a specific type of validation that can be
/// performed on an HTTP response. Assertions are evaluated after a request
/// completes and produce a `TestResult`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TestAssertion {
    /// Assert exact status code match.
    ///
    /// Example: `{ "type": "status", "name": "Status is 200", "expected": 200 }`
    Status {
        /// Human-readable name for the assertion
        name: String,
        /// Expected HTTP status code
        expected: u16,
    },

    /// Assert status code falls within a range (inclusive).
    ///
    /// Example: `{ "type": "status_range", "name": "Status is 2xx", "min": 200, "max": 299 }`
    StatusRange {
        /// Human-readable name for the assertion
        name: String,
        /// Minimum status code (inclusive)
        min: u16,
        /// Maximum status code (inclusive)
        max: u16,
    },

    /// Assert a header exists in the response.
    ///
    /// Example: `{ "type": "header_exists", "name": "Has Content-Type", "header": "Content-Type" }`
    HeaderExists {
        /// Human-readable name for the assertion
        name: String,
        /// Header name to check (case-insensitive)
        header: String,
    },

    /// Assert a header has a specific value.
    ///
    /// Example: `{ "type": "header_equals", "name": "Is JSON", "header": "Content-Type", "expected": "application/json" }`
    HeaderEquals {
        /// Human-readable name for the assertion
        name: String,
        /// Header name to check (case-insensitive)
        header: String,
        /// Expected header value
        expected: String,
        /// Whether to perform case-insensitive comparison
        #[serde(default)]
        case_insensitive: bool,
    },

    /// Assert response body contains a substring.
    ///
    /// Example: `{ "type": "body_contains", "name": "Has success", "expected": "success" }`
    BodyContains {
        /// Human-readable name for the assertion
        name: String,
        /// Substring to search for
        expected: String,
        /// Whether to perform case-insensitive search
        #[serde(default)]
        case_insensitive: bool,
    },

    /// Assert a JSON path exists in the response body.
    ///
    /// Example: `{ "type": "json_path_exists", "name": "Has user ID", "path": "$.data.user.id" }`
    JsonPathExists {
        /// Human-readable name for the assertion
        name: String,
        /// JSON path expression (e.g., "$.data.user.id")
        path: String,
    },

    /// Assert a JSON path has a specific value.
    ///
    /// Example: `{ "type": "json_path_equals", "name": "Name is John", "path": "$.name", "expected": "John" }`
    JsonPathEquals {
        /// Human-readable name for the assertion
        name: String,
        /// JSON path expression
        path: String,
        /// Expected value (as JSON)
        expected: serde_json::Value,
        /// Comparison operator
        #[serde(default)]
        operator: JsonPathOperator,
    },

    /// Assert response time is under a threshold.
    ///
    /// Example: `{ "type": "response_time", "name": "Fast response", "max_ms": 500 }`
    ResponseTime {
        /// Human-readable name for the assertion
        name: String,
        /// Maximum response time in milliseconds
        max_ms: u64,
    },
}

impl TestAssertion {
    /// Returns the human-readable name of this assertion.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Status { name, .. }
            | Self::StatusRange { name, .. }
            | Self::HeaderExists { name, .. }
            | Self::HeaderEquals { name, .. }
            | Self::BodyContains { name, .. }
            | Self::JsonPathExists { name, .. }
            | Self::JsonPathEquals { name, .. }
            | Self::ResponseTime { name, .. } => name,
        }
    }

    /// Returns the assertion type as a string.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Status { .. } => "status",
            Self::StatusRange { .. } => "status_range",
            Self::HeaderExists { .. } => "header_exists",
            Self::HeaderEquals { .. } => "header_equals",
            Self::BodyContains { .. } => "body_contains",
            Self::JsonPathExists { .. } => "json_path_exists",
            Self::JsonPathEquals { .. } => "json_path_equals",
            Self::ResponseTime { .. } => "response_time",
        }
    }

    /// Creates a status assertion.
    #[must_use]
    pub fn status(name: impl Into<String>, expected: u16) -> Self {
        Self::Status {
            name: name.into(),
            expected,
        }
    }

    /// Creates a status range assertion for 2xx responses.
    #[must_use]
    pub fn status_success(name: impl Into<String>) -> Self {
        Self::StatusRange {
            name: name.into(),
            min: 200,
            max: 299,
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

    /// Creates a JSON path exists assertion.
    #[must_use]
    pub fn json_path_exists(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self::JsonPathExists {
            name: name.into(),
            path: path.into(),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_serialize_status_assertion() {
        let assertion = TestAssertion::status("Status is 200", 200);
        let json = serde_json::to_string(&assertion).unwrap();
        assert!(json.contains(r#""type":"status""#));
        assert!(json.contains(r#""expected":200"#));
    }

    #[test]
    fn test_deserialize_json_path_equals() {
        let json = r#"{
            "type": "json_path_equals",
            "name": "User name is John",
            "path": "$.data.user.name",
            "expected": "John"
        }"#;
        let assertion: TestAssertion = serde_json::from_str(json).unwrap();
        assert_eq!(assertion.name(), "User name is John");
        assert_eq!(assertion.type_name(), "json_path_equals");
    }

    #[test]
    fn test_deserialize_with_operator() {
        let json = r#"{
            "type": "json_path_equals",
            "name": "Count greater than 10",
            "path": "$.count",
            "expected": 10,
            "operator": "greater_than"
        }"#;
        let assertion: TestAssertion = serde_json::from_str(json).unwrap();
        if let TestAssertion::JsonPathEquals { operator, .. } = assertion {
            assert_eq!(operator, JsonPathOperator::GreaterThan);
        } else {
            panic!("Wrong assertion type");
        }
    }
}
```

#### Update: `crates/domain/src/lib.rs`

Add the testing module:

```rust
//! Vortex Domain - Core business types

pub mod auth;
pub mod collection;
pub mod environment;
pub mod error;
pub mod request;
pub mod response;
pub mod testing;  // Add this line

pub use error::{DomainError, DomainResult};
```

---

### T02: Assertion Evaluator

Implement the evaluator that runs assertions against responses.

#### File: `crates/application/src/testing/mod.rs`

```rust
//! Testing application layer - assertion evaluation and collection running.

mod evaluator;
mod runner;
mod report;

pub use evaluator::AssertionEvaluator;
pub use runner::{CollectionRunner, RunnerConfig, RunProgress, CancellationToken};
pub use report::{ReportGenerator, RunReport, RequestRunResult};
```

#### File: `crates/application/src/testing/evaluator.rs`

```rust
//! Assertion evaluator implementation.

use std::time::{Duration, Instant};

use vortex_domain::response::ResponseSpec;
use vortex_domain::testing::{TestAssertion, TestResult, TestOutcome, AssertionError, JsonPathOperator};

/// Port for JSON path evaluation.
///
/// This trait abstracts the JSON path library, allowing the application layer
/// to be independent of specific implementations.
pub trait JsonPathEvaluator: Send + Sync {
    /// Evaluates a JSON path expression against a JSON value.
    ///
    /// # Arguments
    ///
    /// * `json` - The JSON string to query
    /// * `path` - The JSON path expression (e.g., "$.data.user.id")
    ///
    /// # Returns
    ///
    /// Returns `Ok(Some(value))` if the path exists and has a value,
    /// `Ok(None)` if the path does not exist,
    /// or `Err` if the JSON is invalid or the path expression is malformed.
    fn evaluate(&self, json: &str, path: &str) -> Result<Option<serde_json::Value>, String>;

    /// Checks if a JSON path exists in the given JSON.
    fn exists(&self, json: &str, path: &str) -> Result<bool, String> {
        self.evaluate(json, path).map(|v| v.is_some())
    }
}

/// Evaluates test assertions against HTTP responses.
pub struct AssertionEvaluator<J: JsonPathEvaluator> {
    json_path: J,
}

impl<J: JsonPathEvaluator> AssertionEvaluator<J> {
    /// Creates a new assertion evaluator with the given JSON path evaluator.
    pub const fn new(json_path: J) -> Self {
        Self { json_path }
    }

    /// Evaluates a single assertion against a response.
    ///
    /// # Arguments
    ///
    /// * `assertion` - The assertion to evaluate
    /// * `response` - The HTTP response to validate
    ///
    /// # Returns
    ///
    /// A `TestResult` indicating whether the assertion passed, failed, or errored.
    pub fn evaluate(&self, assertion: &TestAssertion, response: &ResponseSpec) -> TestResult {
        let start = Instant::now();
        let outcome = self.evaluate_inner(assertion, response);
        let duration = start.elapsed();

        TestResult {
            assertion_name: assertion.name().to_string(),
            assertion_type: assertion.type_name().to_string(),
            outcome,
            duration,
        }
    }

    /// Evaluates all assertions for a response.
    ///
    /// # Arguments
    ///
    /// * `assertions` - The assertions to evaluate
    /// * `response` - The HTTP response to validate
    ///
    /// # Returns
    ///
    /// A vector of `TestResult` for each assertion.
    pub fn evaluate_all(
        &self,
        assertions: &[TestAssertion],
        response: &ResponseSpec,
    ) -> Vec<TestResult> {
        assertions
            .iter()
            .map(|a| self.evaluate(a, response))
            .collect()
    }

    fn evaluate_inner(&self, assertion: &TestAssertion, response: &ResponseSpec) -> TestOutcome {
        match assertion {
            TestAssertion::Status { expected, .. } => {
                self.eval_status(response.status, *expected)
            }

            TestAssertion::StatusRange { min, max, .. } => {
                self.eval_status_range(response.status, *min, *max)
            }

            TestAssertion::HeaderExists { header, .. } => {
                self.eval_header_exists(response, header)
            }

            TestAssertion::HeaderEquals {
                header,
                expected,
                case_insensitive,
                ..
            } => self.eval_header_equals(response, header, expected, *case_insensitive),

            TestAssertion::BodyContains {
                expected,
                case_insensitive,
                ..
            } => self.eval_body_contains(&response.body, expected, *case_insensitive),

            TestAssertion::JsonPathExists { path, .. } => {
                self.eval_json_path_exists(&response.body, path)
            }

            TestAssertion::JsonPathEquals {
                path,
                expected,
                operator,
                ..
            } => self.eval_json_path_equals(&response.body, path, expected, operator),

            TestAssertion::ResponseTime { max_ms, .. } => {
                self.eval_response_time(response.duration, *max_ms)
            }
        }
    }

    fn eval_status(&self, actual: u16, expected: u16) -> TestOutcome {
        if actual == expected {
            TestOutcome::Passed
        } else {
            TestOutcome::Failed(AssertionError {
                expected: expected.to_string(),
                actual: actual.to_string(),
                message: format!("Expected status {expected}, got {actual}"),
            })
        }
    }

    fn eval_status_range(&self, actual: u16, min: u16, max: u16) -> TestOutcome {
        if actual >= min && actual <= max {
            TestOutcome::Passed
        } else {
            TestOutcome::Failed(AssertionError {
                expected: format!("{min}-{max}"),
                actual: actual.to_string(),
                message: format!("Expected status in range {min}-{max}, got {actual}"),
            })
        }
    }

    fn eval_header_exists(&self, response: &ResponseSpec, header: &str) -> TestOutcome {
        let header_lower = header.to_lowercase();
        let exists = response
            .headers
            .all()
            .iter()
            .any(|h| h.name.to_lowercase() == header_lower);

        if exists {
            TestOutcome::Passed
        } else {
            TestOutcome::Failed(AssertionError {
                expected: format!("Header '{header}' to exist"),
                actual: "Header not found".to_string(),
                message: format!("Header '{header}' does not exist in response"),
            })
        }
    }

    fn eval_header_equals(
        &self,
        response: &ResponseSpec,
        header: &str,
        expected: &str,
        case_insensitive: bool,
    ) -> TestOutcome {
        let header_lower = header.to_lowercase();
        let found = response
            .headers
            .all()
            .iter()
            .find(|h| h.name.to_lowercase() == header_lower);

        match found {
            Some(h) => {
                let matches = if case_insensitive {
                    h.value.eq_ignore_ascii_case(expected)
                } else {
                    h.value == expected
                };

                if matches {
                    TestOutcome::Passed
                } else {
                    TestOutcome::Failed(AssertionError {
                        expected: expected.to_string(),
                        actual: h.value.clone(),
                        message: format!(
                            "Header '{header}' expected '{}', got '{}'",
                            expected, h.value
                        ),
                    })
                }
            }
            None => TestOutcome::Failed(AssertionError {
                expected: format!("Header '{header}' = '{expected}'"),
                actual: "Header not found".to_string(),
                message: format!("Header '{header}' does not exist in response"),
            }),
        }
    }

    fn eval_body_contains(
        &self,
        body: &str,
        expected: &str,
        case_insensitive: bool,
    ) -> TestOutcome {
        let contains = if case_insensitive {
            body.to_lowercase().contains(&expected.to_lowercase())
        } else {
            body.contains(expected)
        };

        if contains {
            TestOutcome::Passed
        } else {
            TestOutcome::Failed(AssertionError {
                expected: format!("Body to contain '{expected}'"),
                actual: Self::truncate_body(body, 100),
                message: format!("Body does not contain '{expected}'"),
            })
        }
    }

    fn eval_json_path_exists(&self, body: &str, path: &str) -> TestOutcome {
        match self.json_path.exists(body, path) {
            Ok(true) => TestOutcome::Passed,
            Ok(false) => TestOutcome::Failed(AssertionError {
                expected: format!("Path '{path}' to exist"),
                actual: "Path not found".to_string(),
                message: format!("JSON path '{path}' does not exist"),
            }),
            Err(e) => TestOutcome::Error(format!("JSON path error: {e}")),
        }
    }

    fn eval_json_path_equals(
        &self,
        body: &str,
        path: &str,
        expected: &serde_json::Value,
        operator: &JsonPathOperator,
    ) -> TestOutcome {
        match self.json_path.evaluate(body, path) {
            Ok(Some(actual)) => self.compare_values(&actual, expected, operator, path),
            Ok(None) => TestOutcome::Failed(AssertionError {
                expected: format!("Path '{path}' to exist with value"),
                actual: "Path not found".to_string(),
                message: format!("JSON path '{path}' does not exist"),
            }),
            Err(e) => TestOutcome::Error(format!("JSON path error: {e}")),
        }
    }

    fn compare_values(
        &self,
        actual: &serde_json::Value,
        expected: &serde_json::Value,
        operator: &JsonPathOperator,
        path: &str,
    ) -> TestOutcome {
        match operator {
            JsonPathOperator::Equals => {
                if actual == expected {
                    TestOutcome::Passed
                } else {
                    TestOutcome::Failed(AssertionError {
                        expected: expected.to_string(),
                        actual: actual.to_string(),
                        message: format!("Path '{path}': expected {expected}, got {actual}"),
                    })
                }
            }

            JsonPathOperator::NotEquals => {
                if actual != expected {
                    TestOutcome::Passed
                } else {
                    TestOutcome::Failed(AssertionError {
                        expected: format!("not {expected}"),
                        actual: actual.to_string(),
                        message: format!("Path '{path}': expected not {expected}, got {actual}"),
                    })
                }
            }

            JsonPathOperator::Contains => self.eval_contains(actual, expected, path),

            JsonPathOperator::GreaterThan
            | JsonPathOperator::LessThan
            | JsonPathOperator::GreaterThanOrEqual
            | JsonPathOperator::LessThanOrEqual => {
                self.eval_numeric_comparison(actual, expected, operator, path)
            }

            JsonPathOperator::Matches => self.eval_regex_match(actual, expected, path),
        }
    }

    fn eval_contains(
        &self,
        actual: &serde_json::Value,
        expected: &serde_json::Value,
        path: &str,
    ) -> TestOutcome {
        let contains = match (actual, expected) {
            (serde_json::Value::String(s), serde_json::Value::String(e)) => s.contains(e.as_str()),
            (serde_json::Value::Array(arr), _) => arr.contains(expected),
            _ => false,
        };

        if contains {
            TestOutcome::Passed
        } else {
            TestOutcome::Failed(AssertionError {
                expected: format!("to contain {expected}"),
                actual: actual.to_string(),
                message: format!("Path '{path}': value does not contain {expected}"),
            })
        }
    }

    fn eval_numeric_comparison(
        &self,
        actual: &serde_json::Value,
        expected: &serde_json::Value,
        operator: &JsonPathOperator,
        path: &str,
    ) -> TestOutcome {
        let (Some(actual_num), Some(expected_num)) = (actual.as_f64(), expected.as_f64()) else {
            return TestOutcome::Error(format!(
                "Numeric comparison requires numbers, got {actual} and {expected}"
            ));
        };

        let result = match operator {
            JsonPathOperator::GreaterThan => actual_num > expected_num,
            JsonPathOperator::LessThan => actual_num < expected_num,
            JsonPathOperator::GreaterThanOrEqual => actual_num >= expected_num,
            JsonPathOperator::LessThanOrEqual => actual_num <= expected_num,
            _ => unreachable!(),
        };

        if result {
            TestOutcome::Passed
        } else {
            let op_str = match operator {
                JsonPathOperator::GreaterThan => ">",
                JsonPathOperator::LessThan => "<",
                JsonPathOperator::GreaterThanOrEqual => ">=",
                JsonPathOperator::LessThanOrEqual => "<=",
                _ => unreachable!(),
            };
            TestOutcome::Failed(AssertionError {
                expected: format!("{op_str} {expected_num}"),
                actual: actual_num.to_string(),
                message: format!("Path '{path}': {actual_num} is not {op_str} {expected_num}"),
            })
        }
    }

    fn eval_regex_match(
        &self,
        actual: &serde_json::Value,
        expected: &serde_json::Value,
        path: &str,
    ) -> TestOutcome {
        let (Some(text), Some(pattern)) = (actual.as_str(), expected.as_str()) else {
            return TestOutcome::Error("Regex match requires string values".to_string());
        };

        match regex::Regex::new(pattern) {
            Ok(re) => {
                if re.is_match(text) {
                    TestOutcome::Passed
                } else {
                    TestOutcome::Failed(AssertionError {
                        expected: format!("to match '{pattern}'"),
                        actual: text.to_string(),
                        message: format!("Path '{path}': '{text}' does not match '{pattern}'"),
                    })
                }
            }
            Err(e) => TestOutcome::Error(format!("Invalid regex pattern: {e}")),
        }
    }

    fn eval_response_time(&self, duration: Duration, max_ms: u64) -> TestOutcome {
        let actual_ms = duration.as_millis() as u64;
        if actual_ms <= max_ms {
            TestOutcome::Passed
        } else {
            TestOutcome::Failed(AssertionError {
                expected: format!("<= {max_ms}ms"),
                actual: format!("{actual_ms}ms"),
                message: format!("Response time {actual_ms}ms exceeds maximum {max_ms}ms"),
            })
        }
    }

    fn truncate_body(body: &str, max_len: usize) -> String {
        if body.len() <= max_len {
            body.to_string()
        } else {
            format!("{}...", &body[..max_len])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vortex_domain::request::Headers;

    /// Mock JSON path evaluator for testing.
    struct MockJsonPath {
        results: std::collections::HashMap<String, Option<serde_json::Value>>,
    }

    impl MockJsonPath {
        fn new() -> Self {
            Self {
                results: std::collections::HashMap::new(),
            }
        }

        fn with_result(mut self, path: &str, value: Option<serde_json::Value>) -> Self {
            self.results.insert(path.to_string(), value);
            self
        }
    }

    impl JsonPathEvaluator for MockJsonPath {
        fn evaluate(&self, _json: &str, path: &str) -> Result<Option<serde_json::Value>, String> {
            Ok(self.results.get(path).cloned().flatten())
        }
    }

    fn create_response(status: u16, body: &str) -> ResponseSpec {
        ResponseSpec {
            status,
            status_text: "OK".to_string(),
            headers: Headers::new(),
            body: body.to_string(),
            duration: Duration::from_millis(100),
            size: body.len(),
        }
    }

    #[test]
    fn test_status_assertion_pass() {
        let evaluator = AssertionEvaluator::new(MockJsonPath::new());
        let assertion = TestAssertion::status("Status is 200", 200);
        let response = create_response(200, "");

        let result = evaluator.evaluate(&assertion, &response);
        assert!(matches!(result.outcome, TestOutcome::Passed));
    }

    #[test]
    fn test_status_assertion_fail() {
        let evaluator = AssertionEvaluator::new(MockJsonPath::new());
        let assertion = TestAssertion::status("Status is 200", 200);
        let response = create_response(404, "");

        let result = evaluator.evaluate(&assertion, &response);
        assert!(matches!(result.outcome, TestOutcome::Failed(_)));
    }

    #[test]
    fn test_response_time_pass() {
        let evaluator = AssertionEvaluator::new(MockJsonPath::new());
        let assertion = TestAssertion::response_time("Fast", 500);
        let response = create_response(200, "");

        let result = evaluator.evaluate(&assertion, &response);
        assert!(matches!(result.outcome, TestOutcome::Passed));
    }

    #[test]
    fn test_json_path_exists() {
        let json_path = MockJsonPath::new()
            .with_result("$.data.id", Some(serde_json::json!(123)));
        let evaluator = AssertionEvaluator::new(json_path);
        let assertion = TestAssertion::json_path_exists("Has ID", "$.data.id");
        let response = create_response(200, r#"{"data":{"id":123}}"#);

        let result = evaluator.evaluate(&assertion, &response);
        assert!(matches!(result.outcome, TestOutcome::Passed));
    }
}
```

---

### T03: Test Result Types

Define result types in the domain layer.

#### File: `crates/domain/src/testing/result.rs`

```rust
//! Test result types.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Error details for a failed assertion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssertionError {
    /// What was expected
    pub expected: String,
    /// What was actually found
    pub actual: String,
    /// Human-readable error message
    pub message: String,
}

/// The outcome of evaluating a test assertion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TestOutcome {
    /// The assertion passed.
    Passed,
    /// The assertion failed with details.
    Failed(AssertionError),
    /// An error occurred during evaluation (e.g., invalid JSON path).
    Error(String),
}

impl TestOutcome {
    /// Returns true if the outcome is a pass.
    #[must_use]
    pub const fn is_passed(&self) -> bool {
        matches!(self, Self::Passed)
    }

    /// Returns true if the outcome is a failure.
    #[must_use]
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    /// Returns true if the outcome is an error.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

/// The result of evaluating a single test assertion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestResult {
    /// Name of the assertion that was evaluated
    pub assertion_name: String,
    /// Type of the assertion (e.g., "status", "json_path_equals")
    pub assertion_type: String,
    /// The outcome of the evaluation
    pub outcome: TestOutcome,
    /// Time taken to evaluate the assertion
    #[serde(with = "duration_micros")]
    pub duration: Duration,
}

impl TestResult {
    /// Returns true if the test passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.outcome.is_passed()
    }

    /// Returns true if the test failed.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.outcome.is_failed()
    }

    /// Returns the error message if the test failed or errored.
    #[must_use]
    pub fn error_message(&self) -> Option<&str> {
        match &self.outcome {
            TestOutcome::Passed => None,
            TestOutcome::Failed(e) => Some(&e.message),
            TestOutcome::Error(e) => Some(e),
        }
    }
}

mod duration_micros {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_micros() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let micros = u64::deserialize(deserializer)?;
        Ok(Duration::from_micros(micros))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outcome_checks() {
        assert!(TestOutcome::Passed.is_passed());
        assert!(!TestOutcome::Passed.is_failed());

        let failed = TestOutcome::Failed(AssertionError {
            expected: "200".to_string(),
            actual: "404".to_string(),
            message: "Status mismatch".to_string(),
        });
        assert!(failed.is_failed());
        assert!(!failed.is_passed());

        let error = TestOutcome::Error("Parse error".to_string());
        assert!(error.is_error());
    }

    #[test]
    fn test_result_serialization() {
        let result = TestResult {
            assertion_name: "Status is 200".to_string(),
            assertion_type: "status".to_string(),
            outcome: TestOutcome::Passed,
            duration: Duration::from_micros(150),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains(r#""status":"passed""#));

        let parsed: TestResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.assertion_name, "Status is 200");
    }
}
```

---

### T04: JSON Path Support

Implement JSON path evaluation in the infrastructure layer.

#### Update: `crates/infrastructure/Cargo.toml`

Add the `serde_json_path` dependency:

```toml
[package]
name = "vortex-infrastructure"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
description = "Infrastructure adapters for Vortex API Client"

[dependencies]
vortex-domain = { workspace = true }
vortex-application = { workspace = true }
chrono = { workspace = true }
serde_json = { workspace = true }
serde_json_path = "0.7"  # Add this

[dev-dependencies]
pretty_assertions = { workspace = true }

[lints]
workspace = true
```

#### File: `crates/infrastructure/src/adapters/json_path.rs`

```rust
//! JSON path evaluation adapter using serde_json_path.

use serde_json::Value;
use serde_json_path::JsonPath;
use vortex_application::testing::JsonPathEvaluator;

/// JSON path evaluator using serde_json_path library.
///
/// Supports JSONPath expressions as defined by RFC 9535.
#[derive(Debug, Default, Clone)]
pub struct SerdeJsonPathEvaluator;

impl SerdeJsonPathEvaluator {
    /// Creates a new JSON path evaluator.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl JsonPathEvaluator for SerdeJsonPathEvaluator {
    fn evaluate(&self, json: &str, path: &str) -> Result<Option<Value>, String> {
        // Parse the JSON
        let value: Value = serde_json::from_str(json)
            .map_err(|e| format!("Invalid JSON: {e}"))?;

        // Parse the JSON path expression
        let json_path = JsonPath::parse(path)
            .map_err(|e| format!("Invalid JSON path '{path}': {e}"))?;

        // Query the JSON
        let results = json_path.query(&value);
        let nodes: Vec<&Value> = results.all();

        // Return the first match or None
        match nodes.first() {
            Some(v) => Ok(Some((*v).clone())),
            None => Ok(None),
        }
    }

    fn exists(&self, json: &str, path: &str) -> Result<bool, String> {
        // Parse the JSON
        let value: Value = serde_json::from_str(json)
            .map_err(|e| format!("Invalid JSON: {e}"))?;

        // Parse the JSON path expression
        let json_path = JsonPath::parse(path)
            .map_err(|e| format!("Invalid JSON path '{path}': {e}"))?;

        // Check if any nodes match
        let results = json_path.query(&value);
        Ok(!results.all().is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_simple_path() {
        let evaluator = SerdeJsonPathEvaluator::new();
        let json = r#"{"name": "John", "age": 30}"#;

        let result = evaluator.evaluate(json, "$.name").unwrap();
        assert_eq!(result, Some(json!("John")));

        let result = evaluator.evaluate(json, "$.age").unwrap();
        assert_eq!(result, Some(json!(30)));
    }

    #[test]
    fn test_nested_path() {
        let evaluator = SerdeJsonPathEvaluator::new();
        let json = r#"{"data": {"user": {"id": 123, "name": "Jane"}}}"#;

        let result = evaluator.evaluate(json, "$.data.user.id").unwrap();
        assert_eq!(result, Some(json!(123)));

        let result = evaluator.evaluate(json, "$.data.user.name").unwrap();
        assert_eq!(result, Some(json!("Jane")));
    }

    #[test]
    fn test_array_access() {
        let evaluator = SerdeJsonPathEvaluator::new();
        let json = r#"{"items": [{"id": 1}, {"id": 2}, {"id": 3}]}"#;

        let result = evaluator.evaluate(json, "$.items[0].id").unwrap();
        assert_eq!(result, Some(json!(1)));

        let result = evaluator.evaluate(json, "$.items[2].id").unwrap();
        assert_eq!(result, Some(json!(3)));
    }

    #[test]
    fn test_nonexistent_path() {
        let evaluator = SerdeJsonPathEvaluator::new();
        let json = r#"{"name": "John"}"#;

        let result = evaluator.evaluate(json, "$.nonexistent").unwrap();
        assert_eq!(result, None);

        let exists = evaluator.exists(json, "$.nonexistent").unwrap();
        assert!(!exists);
    }

    #[test]
    fn test_exists() {
        let evaluator = SerdeJsonPathEvaluator::new();
        let json = r#"{"data": {"id": null}}"#;

        // Path exists even if value is null
        let exists = evaluator.exists(json, "$.data.id").unwrap();
        assert!(exists);

        let exists = evaluator.exists(json, "$.data.missing").unwrap();
        assert!(!exists);
    }

    #[test]
    fn test_invalid_json() {
        let evaluator = SerdeJsonPathEvaluator::new();
        let result = evaluator.evaluate("not json", "$.foo");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_path() {
        let evaluator = SerdeJsonPathEvaluator::new();
        let result = evaluator.evaluate("{}", "invalid path");
        assert!(result.is_err());
    }
}
```

#### Update: `crates/infrastructure/src/adapters/mod.rs`

```rust
//! Infrastructure adapters

mod system_clock;
mod json_path;

pub use system_clock::SystemClock;
pub use json_path::SerdeJsonPathEvaluator;
```

---

### T05: Collection Runner

Implement the collection runner in the application layer.

#### File: `crates/application/src/testing/runner.rs`

```rust
//! Collection runner implementation.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use uuid::Uuid;

use vortex_domain::collection::{Collection, CollectionItem};
use vortex_domain::request::RequestSpec;
use vortex_domain::response::ResponseSpec;
use vortex_domain::testing::{TestAssertion, TestResult};

use crate::ports::HttpClient;
use crate::{ApplicationError, ApplicationResult};

use super::evaluator::{AssertionEvaluator, JsonPathEvaluator};
use super::report::RequestRunResult;

/// Configuration for the collection runner.
#[derive(Debug, Clone)]
pub struct RunnerConfig {
    /// Whether to run requests in parallel (true) or sequential (false).
    pub parallel: bool,
    /// Whether to stop on the first failure.
    pub stop_on_failure: bool,
    /// Number of retries for failed requests.
    pub retry_count: u32,
    /// Delay between requests in milliseconds (sequential mode only).
    pub delay_between_ms: u64,
    /// Maximum concurrent requests (parallel mode only).
    pub max_concurrency: usize,
    /// Timeout per request in milliseconds (0 = use default).
    pub request_timeout_ms: u64,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            parallel: false,
            stop_on_failure: false,
            retry_count: 0,
            delay_between_ms: 0,
            max_concurrency: 5,
            request_timeout_ms: 0,
        }
    }
}

impl RunnerConfig {
    /// Creates a sequential runner configuration.
    #[must_use]
    pub const fn sequential() -> Self {
        Self {
            parallel: false,
            stop_on_failure: false,
            retry_count: 0,
            delay_between_ms: 0,
            max_concurrency: 5,
            request_timeout_ms: 0,
        }
    }

    /// Creates a parallel runner configuration.
    #[must_use]
    pub const fn parallel(max_concurrency: usize) -> Self {
        Self {
            parallel: true,
            stop_on_failure: false,
            retry_count: 0,
            delay_between_ms: 0,
            max_concurrency,
            request_timeout_ms: 0,
        }
    }

    /// Sets the stop on failure flag.
    #[must_use]
    pub const fn with_stop_on_failure(mut self, stop: bool) -> Self {
        self.stop_on_failure = stop;
        self
    }

    /// Sets the retry count.
    #[must_use]
    pub const fn with_retry_count(mut self, count: u32) -> Self {
        self.retry_count = count;
        self
    }

    /// Sets the delay between requests.
    #[must_use]
    pub const fn with_delay(mut self, ms: u64) -> Self {
        self.delay_between_ms = ms;
        self
    }
}

/// Token for cancelling a running collection.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Creates a new cancellation token.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Cancels the operation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Returns true if cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// Progress events emitted during collection execution.
#[derive(Debug, Clone)]
pub enum RunProgress {
    /// The run has started.
    Started {
        /// Unique ID for this run
        run_id: Uuid,
        /// Total number of requests to execute
        total_requests: usize,
    },
    /// A request is about to be executed.
    RequestStarted {
        /// Index of the request (0-based)
        index: usize,
        /// ID of the request
        request_id: Uuid,
        /// Name of the request
        request_name: String,
    },
    /// A request has completed (success or failure).
    RequestCompleted {
        /// Index of the request (0-based)
        index: usize,
        /// ID of the request
        request_id: Uuid,
        /// The result of the request execution
        result: RequestRunResult,
    },
    /// The run has finished.
    Finished {
        /// Unique ID for this run
        run_id: Uuid,
        /// Total duration of the run
        duration: Duration,
        /// Number of passed requests
        passed: usize,
        /// Number of failed requests
        failed: usize,
    },
    /// The run was cancelled.
    Cancelled {
        /// Unique ID for this run
        run_id: Uuid,
        /// Number of requests completed before cancellation
        completed: usize,
    },
}

/// Executes collections of requests with test assertions.
pub struct CollectionRunner<H: HttpClient, J: JsonPathEvaluator> {
    http_client: H,
    evaluator: AssertionEvaluator<J>,
    config: RunnerConfig,
}

impl<H: HttpClient, J: JsonPathEvaluator> CollectionRunner<H, J> {
    /// Creates a new collection runner.
    pub fn new(http_client: H, json_path: J, config: RunnerConfig) -> Self {
        Self {
            http_client,
            evaluator: AssertionEvaluator::new(json_path),
            config,
        }
    }

    /// Runs all requests in a collection.
    ///
    /// # Arguments
    ///
    /// * `collection` - The collection to run
    /// * `assertions` - Map of request ID to assertions
    /// * `progress_tx` - Channel to send progress updates
    /// * `cancel_token` - Token to check for cancellation
    ///
    /// # Returns
    ///
    /// A vector of results for each request executed.
    pub async fn run(
        &self,
        collection: &Collection,
        assertions: &std::collections::HashMap<Uuid, Vec<TestAssertion>>,
        progress_tx: mpsc::Sender<RunProgress>,
        cancel_token: &CancellationToken,
    ) -> ApplicationResult<Vec<RequestRunResult>> {
        let run_id = Uuid::new_v4();
        let requests = self.collect_requests(collection);
        let total = requests.len();

        // Send started event
        let _ = progress_tx.send(RunProgress::Started {
            run_id,
            total_requests: total,
        }).await;

        let start = Instant::now();
        let mut results = Vec::with_capacity(total);
        let mut passed = 0;
        let mut failed = 0;

        for (index, request) in requests.iter().enumerate() {
            // Check for cancellation
            if cancel_token.is_cancelled() {
                let _ = progress_tx.send(RunProgress::Cancelled {
                    run_id,
                    completed: index,
                }).await;
                return Err(ApplicationError::Cancelled);
            }

            // Send request started event
            let _ = progress_tx.send(RunProgress::RequestStarted {
                index,
                request_id: request.id,
                request_name: request.name.clone(),
            }).await;

            // Execute request with retries
            let request_assertions = assertions.get(&request.id)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            let result = self.execute_with_retry(request, request_assertions).await;

            // Update counters
            if result.all_tests_passed() {
                passed += 1;
            } else {
                failed += 1;
            }

            // Send request completed event
            let _ = progress_tx.send(RunProgress::RequestCompleted {
                index,
                request_id: request.id,
                result: result.clone(),
            }).await;

            results.push(result);

            // Check stop on failure
            if self.config.stop_on_failure && failed > 0 {
                break;
            }

            // Apply delay between requests (sequential mode)
            if !self.config.parallel && self.config.delay_between_ms > 0 && index < total - 1 {
                tokio::time::sleep(Duration::from_millis(self.config.delay_between_ms)).await;
            }
        }

        // Send finished event
        let _ = progress_tx.send(RunProgress::Finished {
            run_id,
            duration: start.elapsed(),
            passed,
            failed,
        }).await;

        Ok(results)
    }

    /// Collects all requests from a collection recursively.
    fn collect_requests(&self, collection: &Collection) -> Vec<RequestSpec> {
        fn collect_from_items(items: &[CollectionItem], requests: &mut Vec<RequestSpec>) {
            for item in items {
                match item {
                    CollectionItem::Request(req) => requests.push(req.clone()),
                    CollectionItem::Folder(folder) => collect_from_items(&folder.items, requests),
                }
            }
        }

        let mut requests = Vec::new();
        collect_from_items(&collection.items, &mut requests);
        requests
    }

    /// Executes a single request with retry logic.
    async fn execute_with_retry(
        &self,
        request: &RequestSpec,
        assertions: &[TestAssertion],
    ) -> RequestRunResult {
        let mut last_error = None;
        let mut attempts = 0;

        loop {
            attempts += 1;
            let start = Instant::now();

            match self.http_client.execute(request).await {
                Ok(response) => {
                    let test_results = self.evaluator.evaluate_all(assertions, &response);
                    return RequestRunResult {
                        request_id: request.id,
                        request_name: request.name.clone(),
                        response: Some(response),
                        test_results,
                        error: None,
                        duration: start.elapsed(),
                        attempts,
                    };
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                    if attempts > self.config.retry_count {
                        return RequestRunResult {
                            request_id: request.id,
                            request_name: request.name.clone(),
                            response: None,
                            test_results: vec![],
                            error: last_error,
                            duration: start.elapsed(),
                            attempts,
                        };
                    }
                    // Wait before retry
                    tokio::time::sleep(Duration::from_millis(100 * u64::from(attempts))).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_config_default() {
        let config = RunnerConfig::default();
        assert!(!config.parallel);
        assert!(!config.stop_on_failure);
        assert_eq!(config.retry_count, 0);
    }

    #[test]
    fn test_runner_config_builder() {
        let config = RunnerConfig::sequential()
            .with_stop_on_failure(true)
            .with_retry_count(3)
            .with_delay(100);

        assert!(!config.parallel);
        assert!(config.stop_on_failure);
        assert_eq!(config.retry_count, 3);
        assert_eq!(config.delay_between_ms, 100);
    }

    #[test]
    fn test_cancellation_token() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());

        token.cancel();
        assert!(token.is_cancelled());
    }
}
```

---

### T06: Run Progress Events

The `RunProgress` enum is already defined in the runner module above. This task is complete.

---

### T07: Report Generation

Implement report generation in the application layer.

#### File: `crates/application/src/testing/report.rs`

```rust
//! Report generation for collection runs.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use vortex_domain::response::ResponseSpec;
use vortex_domain::testing::TestResult;

/// Result of executing a single request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestRunResult {
    /// ID of the request
    pub request_id: Uuid,
    /// Name of the request
    pub request_name: String,
    /// The response (if successful)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<ResponseSpec>,
    /// Results of test assertions
    pub test_results: Vec<TestResult>,
    /// Error message (if request failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Total duration including retries
    #[serde(with = "duration_millis")]
    pub duration: Duration,
    /// Number of attempts (1 = no retries)
    pub attempts: u32,
}

impl RequestRunResult {
    /// Returns true if the request succeeded and all tests passed.
    #[must_use]
    pub fn all_tests_passed(&self) -> bool {
        self.error.is_none() && self.test_results.iter().all(|r| r.passed())
    }

    /// Returns the number of passed tests.
    #[must_use]
    pub fn passed_count(&self) -> usize {
        self.test_results.iter().filter(|r| r.passed()).count()
    }

    /// Returns the number of failed tests.
    #[must_use]
    pub fn failed_count(&self) -> usize {
        self.test_results.iter().filter(|r| r.failed()).count()
    }

    /// Returns true if the request itself failed (network error, etc.).
    #[must_use]
    pub fn has_request_error(&self) -> bool {
        self.error.is_some()
    }
}

/// Summary statistics for a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    /// Total number of requests
    pub total_requests: usize,
    /// Number of requests that passed all tests
    pub passed_requests: usize,
    /// Number of requests with at least one failed test
    pub failed_requests: usize,
    /// Number of requests that errored (network failure, etc.)
    pub errored_requests: usize,
    /// Total number of test assertions
    pub total_assertions: usize,
    /// Number of passed assertions
    pub passed_assertions: usize,
    /// Number of failed assertions
    pub failed_assertions: usize,
    /// Total duration of the run
    #[serde(with = "duration_millis")]
    pub duration: Duration,
}

/// Complete report for a collection run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunReport {
    /// Unique ID for this run
    pub run_id: Uuid,
    /// Name of the collection that was run
    pub collection_name: String,
    /// ID of the collection
    pub collection_id: Uuid,
    /// Environment name (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// When the run started
    pub started_at: DateTime<Utc>,
    /// When the run finished
    pub finished_at: DateTime<Utc>,
    /// Summary statistics
    pub summary: RunSummary,
    /// Individual request results
    pub results: Vec<RequestRunResult>,
    /// Vortex version that generated the report
    pub vortex_version: String,
}

impl RunReport {
    /// Returns true if all requests passed all tests.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.summary.failed_requests == 0 && self.summary.errored_requests == 0
    }

    /// Returns the pass rate as a percentage (0.0 - 100.0).
    #[must_use]
    pub fn pass_rate(&self) -> f64 {
        if self.summary.total_requests == 0 {
            return 100.0;
        }
        (self.summary.passed_requests as f64 / self.summary.total_requests as f64) * 100.0
    }
}

/// Generates reports from collection run results.
pub struct ReportGenerator;

impl ReportGenerator {
    /// Creates a new report generator.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Generates a complete run report.
    pub fn generate(
        &self,
        collection_name: String,
        collection_id: Uuid,
        environment: Option<String>,
        results: Vec<RequestRunResult>,
        started_at: DateTime<Utc>,
        finished_at: DateTime<Utc>,
    ) -> RunReport {
        let summary = self.calculate_summary(&results, finished_at - started_at);

        RunReport {
            run_id: Uuid::new_v4(),
            collection_name,
            collection_id,
            environment,
            started_at,
            finished_at,
            summary,
            results,
            vortex_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    fn calculate_summary(
        &self,
        results: &[RequestRunResult],
        duration: chrono::Duration,
    ) -> RunSummary {
        let total_requests = results.len();
        let mut passed_requests = 0;
        let mut failed_requests = 0;
        let mut errored_requests = 0;
        let mut total_assertions = 0;
        let mut passed_assertions = 0;
        let mut failed_assertions = 0;

        for result in results {
            if result.has_request_error() {
                errored_requests += 1;
            } else if result.all_tests_passed() {
                passed_requests += 1;
            } else {
                failed_requests += 1;
            }

            total_assertions += result.test_results.len();
            passed_assertions += result.passed_count();
            failed_assertions += result.failed_count();
        }

        RunSummary {
            total_requests,
            passed_requests,
            failed_requests,
            errored_requests,
            total_assertions,
            passed_assertions,
            failed_assertions,
            duration: duration.to_std().unwrap_or(Duration::ZERO),
        }
    }

    /// Serializes the report to JSON format.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_json(&self, report: &RunReport) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(report)
    }

    /// Serializes the report to compact JSON format.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_json_compact(&self, report: &RunReport) -> Result<String, serde_json::Error> {
        serde_json::to_string(report)
    }
}

impl Default for ReportGenerator {
    fn default() -> Self {
        Self::new()
    }
}

mod duration_millis {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
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

#[cfg(test)]
mod tests {
    use super::*;
    use vortex_domain::testing::{TestOutcome, AssertionError};

    fn create_passed_result(name: &str) -> RequestRunResult {
        RequestRunResult {
            request_id: Uuid::new_v4(),
            request_name: name.to_string(),
            response: None,
            test_results: vec![TestResult {
                assertion_name: "Status is 200".to_string(),
                assertion_type: "status".to_string(),
                outcome: TestOutcome::Passed,
                duration: Duration::from_micros(100),
            }],
            error: None,
            duration: Duration::from_millis(150),
            attempts: 1,
        }
    }

    fn create_failed_result(name: &str) -> RequestRunResult {
        RequestRunResult {
            request_id: Uuid::new_v4(),
            request_name: name.to_string(),
            response: None,
            test_results: vec![TestResult {
                assertion_name: "Status is 200".to_string(),
                assertion_type: "status".to_string(),
                outcome: TestOutcome::Failed(AssertionError {
                    expected: "200".to_string(),
                    actual: "404".to_string(),
                    message: "Status mismatch".to_string(),
                }),
                duration: Duration::from_micros(100),
            }],
            error: None,
            duration: Duration::from_millis(150),
            attempts: 1,
        }
    }

    #[test]
    fn test_request_run_result_helpers() {
        let passed = create_passed_result("test");
        assert!(passed.all_tests_passed());
        assert_eq!(passed.passed_count(), 1);
        assert_eq!(passed.failed_count(), 0);

        let failed = create_failed_result("test");
        assert!(!failed.all_tests_passed());
        assert_eq!(failed.passed_count(), 0);
        assert_eq!(failed.failed_count(), 1);
    }

    #[test]
    fn test_report_generation() {
        let generator = ReportGenerator::new();
        let results = vec![
            create_passed_result("Request 1"),
            create_passed_result("Request 2"),
            create_failed_result("Request 3"),
        ];

        let now = Utc::now();
        let report = generator.generate(
            "Test Collection".to_string(),
            Uuid::new_v4(),
            Some("Development".to_string()),
            results,
            now,
            now + chrono::Duration::seconds(5),
        );

        assert_eq!(report.summary.total_requests, 3);
        assert_eq!(report.summary.passed_requests, 2);
        assert_eq!(report.summary.failed_requests, 1);
        assert!(!report.all_passed());
    }

    #[test]
    fn test_pass_rate() {
        let generator = ReportGenerator::new();
        let results = vec![
            create_passed_result("Request 1"),
            create_failed_result("Request 2"),
        ];

        let now = Utc::now();
        let report = generator.generate(
            "Test".to_string(),
            Uuid::new_v4(),
            None,
            results,
            now,
            now,
        );

        assert!((report.pass_rate() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_json_serialization() {
        let generator = ReportGenerator::new();
        let results = vec![create_passed_result("Request 1")];

        let now = Utc::now();
        let report = generator.generate(
            "Test".to_string(),
            Uuid::new_v4(),
            None,
            results,
            now,
            now,
        );

        let json = generator.to_json(&report).unwrap();
        assert!(json.contains("Test"));
        assert!(json.contains("passed_requests"));
    }
}
```

#### Update: `crates/application/src/lib.rs`

```rust
//! Vortex Application - Use cases and ports

pub mod error;
pub mod ports;
pub mod testing;  // Add this line

pub use error::{ApplicationError, ApplicationResult};
```

---

### T08: JUnit XML Export

Implement JUnit XML report generation for CI integration.

#### File: `crates/infrastructure/src/adapters/junit_reporter.rs`

```rust
//! JUnit XML report generation for CI integration.

use std::io::Write;

use vortex_application::testing::{RunReport, RequestRunResult};
use vortex_domain::testing::TestOutcome;

/// Generates JUnit XML reports compatible with CI tools.
///
/// The generated XML follows the JUnit XML format used by Jenkins, GitLab CI,
/// GitHub Actions, and other CI systems.
pub struct JUnitReporter;

impl JUnitReporter {
    /// Creates a new JUnit reporter.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Generates a JUnit XML report from a run report.
    ///
    /// # Errors
    ///
    /// Returns an error if XML generation fails.
    pub fn generate(&self, report: &RunReport) -> Result<String, std::fmt::Error> {
        let mut xml = String::new();
        self.write_xml(&mut xml, report)?;
        Ok(xml)
    }

    /// Writes the JUnit XML to a writer.
    ///
    /// # Errors
    ///
    /// Returns an error if writing fails.
    pub fn write_to<W: Write>(&self, writer: &mut W, report: &RunReport) -> std::io::Result<()> {
        let xml = self.generate(report).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
        })?;
        writer.write_all(xml.as_bytes())
    }

    fn write_xml(&self, out: &mut String, report: &RunReport) -> std::fmt::Result {
        use std::fmt::Write;

        // XML declaration
        writeln!(out, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;

        // Testsuites root element
        writeln!(
            out,
            r#"<testsuites name="{}" tests="{}" failures="{}" errors="{}" time="{:.3}">"#,
            Self::escape_xml(&report.collection_name),
            report.summary.total_assertions,
            report.summary.failed_assertions,
            report.summary.errored_requests,
            report.summary.duration.as_secs_f64()
        )?;

        // Each request is a testsuite
        for result in &report.results {
            self.write_testsuite(out, result)?;
        }

        writeln!(out, "</testsuites>")?;
        Ok(())
    }

    fn write_testsuite(
        &self,
        out: &mut String,
        result: &RequestRunResult,
    ) -> std::fmt::Result {
        use std::fmt::Write;

        let tests = result.test_results.len();
        let failures = result.failed_count();
        let errors = if result.has_request_error() { 1 } else { 0 };

        writeln!(
            out,
            r#"  <testsuite name="{}" tests="{}" failures="{}" errors="{}" time="{:.3}">"#,
            Self::escape_xml(&result.request_name),
            tests,
            failures,
            errors,
            result.duration.as_secs_f64()
        )?;

        // Request-level error
        if let Some(ref error) = result.error {
            writeln!(
                out,
                r#"    <testcase name="Request Execution" classname="{}">"#,
                Self::escape_xml(&result.request_name)
            )?;
            writeln!(
                out,
                r#"      <error message="{}"><![CDATA[{}]]></error>"#,
                Self::escape_xml(error),
                error
            )?;
            writeln!(out, "    </testcase>")?;
        }

        // Individual test assertions
        for test_result in &result.test_results {
            writeln!(
                out,
                r#"    <testcase name="{}" classname="{}" time="{:.6}">"#,
                Self::escape_xml(&test_result.assertion_name),
                Self::escape_xml(&result.request_name),
                test_result.duration.as_secs_f64()
            )?;

            match &test_result.outcome {
                TestOutcome::Passed => {
                    // No child elements for passed tests
                }
                TestOutcome::Failed(error) => {
                    writeln!(
                        out,
                        r#"      <failure message="{}" type="AssertionError">"#,
                        Self::escape_xml(&error.message)
                    )?;
                    writeln!(out, "Expected: {}", Self::escape_xml(&error.expected))?;
                    writeln!(out, "Actual: {}", Self::escape_xml(&error.actual))?;
                    writeln!(out, "      </failure>")?;
                }
                TestOutcome::Error(message) => {
                    writeln!(
                        out,
                        r#"      <error message="{}" type="EvaluationError"><![CDATA[{}]]></error>"#,
                        Self::escape_xml(message),
                        message
                    )?;
                }
            }

            writeln!(out, "    </testcase>")?;
        }

        writeln!(out, "  </testsuite>")?;
        Ok(())
    }

    fn escape_xml(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }
}

impl Default for JUnitReporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use uuid::Uuid;
    use chrono::Utc;
    use vortex_domain::testing::{TestResult, AssertionError};
    use vortex_application::testing::RunSummary;

    fn create_test_report() -> RunReport {
        let now = Utc::now();
        RunReport {
            run_id: Uuid::new_v4(),
            collection_name: "Test Collection".to_string(),
            collection_id: Uuid::new_v4(),
            environment: Some("Development".to_string()),
            started_at: now,
            finished_at: now + chrono::Duration::seconds(5),
            summary: RunSummary {
                total_requests: 2,
                passed_requests: 1,
                failed_requests: 1,
                errored_requests: 0,
                total_assertions: 3,
                passed_assertions: 2,
                failed_assertions: 1,
                duration: Duration::from_secs(5),
            },
            results: vec![
                RequestRunResult {
                    request_id: Uuid::new_v4(),
                    request_name: "Get Users".to_string(),
                    response: None,
                    test_results: vec![
                        TestResult {
                            assertion_name: "Status is 200".to_string(),
                            assertion_type: "status".to_string(),
                            outcome: TestOutcome::Passed,
                            duration: Duration::from_micros(100),
                        },
                    ],
                    error: None,
                    duration: Duration::from_millis(150),
                    attempts: 1,
                },
                RequestRunResult {
                    request_id: Uuid::new_v4(),
                    request_name: "Create User".to_string(),
                    response: None,
                    test_results: vec![
                        TestResult {
                            assertion_name: "Status is 201".to_string(),
                            assertion_type: "status".to_string(),
                            outcome: TestOutcome::Failed(AssertionError {
                                expected: "201".to_string(),
                                actual: "400".to_string(),
                                message: "Expected status 201, got 400".to_string(),
                            }),
                            duration: Duration::from_micros(100),
                        },
                    ],
                    error: None,
                    duration: Duration::from_millis(200),
                    attempts: 1,
                },
            ],
            vortex_version: "0.1.0".to_string(),
        }
    }

    #[test]
    fn test_generate_junit_xml() {
        let reporter = JUnitReporter::new();
        let report = create_test_report();

        let xml = reporter.generate(&report).unwrap();

        assert!(xml.contains(r#"<?xml version="1.0" encoding="UTF-8"?>"#));
        assert!(xml.contains("testsuites"));
        assert!(xml.contains("testsuite"));
        assert!(xml.contains("testcase"));
        assert!(xml.contains("Test Collection"));
        assert!(xml.contains("Get Users"));
        assert!(xml.contains("Create User"));
        assert!(xml.contains("failure"));
    }

    #[test]
    fn test_escape_xml_special_chars() {
        let reporter = JUnitReporter::new();
        let escaped = JUnitReporter::escape_xml("<test & \"value\">");
        assert_eq!(escaped, "&lt;test &amp; &quot;value&quot;&gt;");
    }
}
```

#### Update: `crates/infrastructure/src/adapters/mod.rs`

```rust
//! Infrastructure adapters

mod system_clock;
mod json_path;
mod junit_reporter;

pub use system_clock::SystemClock;
pub use json_path::SerdeJsonPathEvaluator;
pub use junit_reporter::JUnitReporter;
```

---

### T09: UI Test Editor

Define Slint components for test editing.

#### File: `crates/ui/src/ui/test_editor.slint`

```slint
// Test Editor Component
// Allows users to add, edit, and remove test assertions for requests

import { VerticalBox, HorizontalBox, Button, LineEdit, ComboBox, ListView } from "std-widgets.slint";
import { Theme } from "main_window.slint";

// Available assertion types
export global TestAssertionTypes {
    out property <[string]> types: [
        "Status Code",
        "Status Range",
        "Header Exists",
        "Header Equals",
        "Body Contains",
        "JSON Path Exists",
        "JSON Path Equals",
        "Response Time"
    ];
}

// Model for a single test assertion in the list
export struct TestAssertionItem {
    id: string,
    name: string,
    type_name: string,
    description: string,
    enabled: bool,
}

// Test Builder Dialog for creating new assertions
export component TestBuilderDialog inherits Rectangle {
    in-out property <bool> visible: false;
    in-out property <string> assertion-name: "";
    in-out property <int> selected-type: 0;
    in-out property <string> expected-value: "";
    in-out property <string> header-name: "";
    in-out property <string> json-path: "";
    in-out property <int> min-value: 200;
    in-out property <int> max-value: 299;
    in-out property <int> max-ms: 500;

    callback add-assertion();
    callback cancel();

    background: Theme.background-secondary;
    border-radius: 8px;
    drop-shadow-blur: 20px;
    drop-shadow-color: #00000080;

    if visible: VerticalBox {
        padding: Theme.spacing-md;
        spacing: Theme.spacing-sm;

        Text {
            text: "Add Test Assertion";
            font-size: 16px;
            font-weight: 600;
            color: Theme.text-primary;
        }

        // Assertion type selector
        HorizontalBox {
            spacing: Theme.spacing-sm;
            Text {
                text: "Type:";
                color: Theme.text-secondary;
                vertical-alignment: center;
            }
            type-combo := ComboBox {
                model: TestAssertionTypes.types;
                current-index <=> selected-type;
            }
        }

        // Assertion name
        HorizontalBox {
            spacing: Theme.spacing-sm;
            Text {
                text: "Name:";
                color: Theme.text-secondary;
                vertical-alignment: center;
            }
            LineEdit {
                text <=> assertion-name;
                placeholder-text: "e.g., Status is 200";
            }
        }

        // Dynamic fields based on type
        if selected-type == 0: HorizontalBox {
            spacing: Theme.spacing-sm;
            Text {
                text: "Expected Status:";
                color: Theme.text-secondary;
                vertical-alignment: center;
            }
            LineEdit {
                text <=> expected-value;
                placeholder-text: "200";
                input-type: number;
            }
        }

        if selected-type == 1: HorizontalBox {
            spacing: Theme.spacing-sm;
            Text {
                text: "Min:";
                color: Theme.text-secondary;
                vertical-alignment: center;
            }
            LineEdit {
                text: min-value;
                input-type: number;
                width: 60px;
            }
            Text {
                text: "Max:";
                color: Theme.text-secondary;
                vertical-alignment: center;
            }
            LineEdit {
                text: max-value;
                input-type: number;
                width: 60px;
            }
        }

        if selected-type == 2 || selected-type == 3: HorizontalBox {
            spacing: Theme.spacing-sm;
            Text {
                text: "Header:";
                color: Theme.text-secondary;
                vertical-alignment: center;
            }
            LineEdit {
                text <=> header-name;
                placeholder-text: "Content-Type";
            }
        }

        if selected-type == 3: HorizontalBox {
            spacing: Theme.spacing-sm;
            Text {
                text: "Expected:";
                color: Theme.text-secondary;
                vertical-alignment: center;
            }
            LineEdit {
                text <=> expected-value;
                placeholder-text: "application/json";
            }
        }

        if selected-type == 4: HorizontalBox {
            spacing: Theme.spacing-sm;
            Text {
                text: "Contains:";
                color: Theme.text-secondary;
                vertical-alignment: center;
            }
            LineEdit {
                text <=> expected-value;
                placeholder-text: "success";
            }
        }

        if selected-type == 5 || selected-type == 6: HorizontalBox {
            spacing: Theme.spacing-sm;
            Text {
                text: "JSON Path:";
                color: Theme.text-secondary;
                vertical-alignment: center;
            }
            LineEdit {
                text <=> json-path;
                placeholder-text: "$.data.user.id";
                font-family: "monospace";
            }
        }

        if selected-type == 6: HorizontalBox {
            spacing: Theme.spacing-sm;
            Text {
                text: "Expected:";
                color: Theme.text-secondary;
                vertical-alignment: center;
            }
            LineEdit {
                text <=> expected-value;
                placeholder-text: "John";
            }
        }

        if selected-type == 7: HorizontalBox {
            spacing: Theme.spacing-sm;
            Text {
                text: "Max Time (ms):";
                color: Theme.text-secondary;
                vertical-alignment: center;
            }
            LineEdit {
                text: max-ms;
                input-type: number;
                width: 80px;
            }
        }

        // Action buttons
        HorizontalBox {
            spacing: Theme.spacing-sm;
            alignment: end;

            Button {
                text: "Cancel";
                clicked => { cancel(); }
            }
            Button {
                text: "Add Test";
                primary: true;
                clicked => { add-assertion(); }
            }
        }
    }
}

// Single assertion item in the list
component TestAssertionRow inherits Rectangle {
    in property <TestAssertionItem> assertion;
    in property <bool> is-selected: false;

    callback toggle-enabled();
    callback select();
    callback delete();

    height: 40px;
    background: is-selected ? Theme.background-tertiary : transparent;
    border-radius: 4px;

    HorizontalBox {
        padding: Theme.spacing-sm;
        spacing: Theme.spacing-sm;

        // Enable/disable checkbox
        Rectangle {
            width: 20px;
            height: 20px;
            background: assertion.enabled ? Theme.accent-success : Theme.background-tertiary;
            border-radius: 4px;

            TouchArea {
                clicked => { toggle-enabled(); }
            }

            Text {
                text: assertion.enabled ? "✓" : "";
                color: white;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }

        // Assertion name and type
        TouchArea {
            clicked => { select(); }

            VerticalBox {
                spacing: 2px;
                alignment: center;

                Text {
                    text: assertion.name;
                    color: Theme.text-primary;
                    font-size: 13px;
                }
                Text {
                    text: assertion.type_name;
                    color: Theme.text-muted;
                    font-size: 11px;
                }
            }
        }

        // Delete button
        Rectangle {
            width: 24px;
            height: 24px;
            background: transparent;
            border-radius: 4px;

            TouchArea {
                clicked => { delete(); }
            }

            Text {
                text: "×";
                color: Theme.text-muted;
                font-size: 16px;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }
    }
}

// Main Test Editor component
export component TestEditor inherits VerticalBox {
    in-out property <[TestAssertionItem]> assertions: [];
    in-out property <int> selected-index: -1;
    in-out property <bool> show-builder: false;

    callback add-assertion(TestAssertionItem);
    callback update-assertion(int, TestAssertionItem);
    callback delete-assertion(int);
    callback toggle-assertion-enabled(int);

    padding: Theme.spacing-sm;
    spacing: Theme.spacing-sm;

    // Header
    HorizontalBox {
        spacing: Theme.spacing-sm;

        Text {
            text: "Test Assertions";
            font-size: 14px;
            font-weight: 600;
            color: Theme.text-primary;
            vertical-alignment: center;
        }

        Rectangle { horizontal-stretch: 1; }

        Button {
            text: "+ Add Test";
            clicked => { show-builder = true; }
        }
    }

    // Assertions list
    if assertions.length == 0: Rectangle {
        height: 100px;
        background: Theme.background-secondary;
        border-radius: 4px;

        Text {
            text: "No test assertions defined.\nClick '+ Add Test' to create one.";
            color: Theme.text-muted;
            horizontal-alignment: center;
            vertical-alignment: center;
        }
    }

    if assertions.length > 0: ListView {
        for assertion[index] in assertions: TestAssertionRow {
            assertion: assertion;
            is-selected: index == selected-index;
            toggle-enabled => { toggle-assertion-enabled(index); }
            select => { selected-index = index; }
            delete => { delete-assertion(index); }
        }
    }

    // Test builder dialog overlay
    if show-builder: Rectangle {
        background: #00000060;

        TestBuilderDialog {
            visible: true;
            width: 400px;
            height: 300px;
            x: (parent.width - self.width) / 2;
            y: (parent.height - self.height) / 2;

            cancel => { show-builder = false; }
            add-assertion => {
                // This would be connected to Rust logic
                show-builder = false;
            }
        }
    }
}
```

---

### T10: UI Collection Runner Panel

Define Slint components for the collection runner.

#### File: `crates/ui/src/ui/runner_panel.slint`

```slint
// Collection Runner Panel
// Displays progress and controls for running collections

import { VerticalBox, HorizontalBox, Button, ProgressIndicator, CheckBox, ComboBox } from "std-widgets.slint";
import { Theme } from "main_window.slint";

// Status of the runner
export enum RunnerStatus {
    idle,
    running,
    paused,
    completed,
    cancelled,
    error,
}

// Progress information
export struct RunProgress {
    current: int,
    total: int,
    current-request-name: string,
    passed: int,
    failed: int,
    elapsed-ms: int,
}

// Runner configuration options
export struct RunnerOptions {
    parallel: bool,
    stop-on-failure: bool,
    retry-count: int,
    delay-ms: int,
}

// Status badge component
component StatusBadge inherits Rectangle {
    in property <RunnerStatus> status;

    width: 80px;
    height: 24px;
    border-radius: 12px;
    background: status == RunnerStatus.idle ? Theme.background-tertiary :
                status == RunnerStatus.running ? Theme.accent-primary :
                status == RunnerStatus.completed ? Theme.accent-success :
                status == RunnerStatus.cancelled ? Theme.accent-warning :
                status == RunnerStatus.error ? Theme.accent-error :
                Theme.background-tertiary;

    Text {
        text: status == RunnerStatus.idle ? "Idle" :
              status == RunnerStatus.running ? "Running" :
              status == RunnerStatus.completed ? "Done" :
              status == RunnerStatus.cancelled ? "Cancelled" :
              status == RunnerStatus.error ? "Error" : "Unknown";
        color: white;
        font-size: 11px;
        font-weight: 500;
        horizontal-alignment: center;
        vertical-alignment: center;
    }
}

// Progress bar with stats
component RunProgressBar inherits Rectangle {
    in property <RunProgress> progress;
    in property <RunnerStatus> status;

    height: 60px;
    background: Theme.background-secondary;
    border-radius: 8px;

    VerticalBox {
        padding: Theme.spacing-sm;
        spacing: Theme.spacing-xs;

        // Current request name
        Text {
            text: status == RunnerStatus.running ?
                  "Running: " + progress.current-request-name :
                  status == RunnerStatus.completed ? "Completed" : "";
            color: Theme.text-secondary;
            font-size: 12px;
        }

        // Progress bar
        Rectangle {
            height: 8px;
            background: Theme.background-tertiary;
            border-radius: 4px;

            Rectangle {
                width: progress.total > 0 ?
                       parent.width * (progress.current / progress.total) :
                       0px;
                height: parent.height;
                background: Theme.accent-primary;
                border-radius: 4px;
            }
        }

        // Stats row
        HorizontalBox {
            spacing: Theme.spacing-md;

            Text {
                text: progress.current + " / " + progress.total;
                color: Theme.text-secondary;
                font-size: 11px;
            }

            Text {
                text: "✓ " + progress.passed;
                color: Theme.accent-success;
                font-size: 11px;
            }

            Text {
                text: "✗ " + progress.failed;
                color: Theme.accent-error;
                font-size: 11px;
            }

            Rectangle { horizontal-stretch: 1; }

            Text {
                text: (progress.elapsed-ms / 1000) + "." + mod(progress.elapsed-ms, 1000) / 100 + "s";
                color: Theme.text-muted;
                font-size: 11px;
            }
        }
    }
}

// Runner options panel
component RunnerOptionsPanel inherits Rectangle {
    in-out property <RunnerOptions> options: {
        parallel: false,
        stop-on-failure: false,
        retry-count: 0,
        delay-ms: 0,
    };
    in-out property <bool> expanded: false;

    height: expanded ? 140px : 40px;
    background: Theme.background-secondary;
    border-radius: 8px;

    VerticalBox {
        padding: Theme.spacing-sm;
        spacing: Theme.spacing-xs;

        // Header with expand toggle
        HorizontalBox {
            spacing: Theme.spacing-sm;

            TouchArea {
                clicked => { expanded = !expanded; }

                HorizontalBox {
                    spacing: Theme.spacing-xs;
                    Text {
                        text: expanded ? "▼" : "▶";
                        color: Theme.text-muted;
                        font-size: 10px;
                        vertical-alignment: center;
                    }
                    Text {
                        text: "Runner Options";
                        color: Theme.text-secondary;
                        font-size: 13px;
                        vertical-alignment: center;
                    }
                }
            }
        }

        // Options (when expanded)
        if expanded: VerticalBox {
            spacing: Theme.spacing-xs;

            HorizontalBox {
                spacing: Theme.spacing-md;

                CheckBox {
                    text: "Parallel execution";
                    checked <=> options.parallel;
                }

                CheckBox {
                    text: "Stop on failure";
                    checked <=> options.stop-on-failure;
                }
            }

            HorizontalBox {
                spacing: Theme.spacing-sm;

                Text {
                    text: "Retries:";
                    color: Theme.text-secondary;
                    vertical-alignment: center;
                }
                ComboBox {
                    model: ["0", "1", "2", "3"];
                    current-index: options.retry-count;
                    width: 60px;
                }

                Text {
                    text: "Delay (ms):";
                    color: Theme.text-secondary;
                    vertical-alignment: center;
                }
                ComboBox {
                    model: ["0", "100", "500", "1000"];
                    width: 80px;
                }
            }
        }
    }
}

// Main Runner Panel component
export component RunnerPanel inherits VerticalBox {
    in property <string> collection-name: "No collection selected";
    in-out property <RunnerStatus> status: RunnerStatus.idle;
    in-out property <RunProgress> progress: {
        current: 0,
        total: 0,
        current-request-name: "",
        passed: 0,
        failed: 0,
        elapsed-ms: 0,
    };
    in-out property <RunnerOptions> options;

    callback start-run();
    callback cancel-run();
    callback export-report(string); // "json" or "junit"

    padding: Theme.spacing-md;
    spacing: Theme.spacing-md;

    // Header
    HorizontalBox {
        spacing: Theme.spacing-sm;

        Text {
            text: "Collection Runner";
            font-size: 16px;
            font-weight: 600;
            color: Theme.text-primary;
            vertical-alignment: center;
        }

        Rectangle { horizontal-stretch: 1; }

        StatusBadge {
            status: status;
        }
    }

    // Collection name
    Text {
        text: collection-name;
        color: Theme.text-secondary;
        font-size: 13px;
    }

    // Progress (when running or completed)
    if status != RunnerStatus.idle: RunProgressBar {
        progress: progress;
        status: status;
    }

    // Options panel
    RunnerOptionsPanel {
        options <=> options;
    }

    // Action buttons
    HorizontalBox {
        spacing: Theme.spacing-sm;

        if status == RunnerStatus.idle || status == RunnerStatus.completed || status == RunnerStatus.cancelled: Button {
            text: "▶ Run Collection";
            primary: true;
            clicked => { start-run(); }
        }

        if status == RunnerStatus.running: Button {
            text: "■ Cancel";
            clicked => { cancel-run(); }
        }

        Rectangle { horizontal-stretch: 1; }

        if status == RunnerStatus.completed: HorizontalBox {
            spacing: Theme.spacing-xs;

            Button {
                text: "Export JSON";
                clicked => { export-report("json"); }
            }
            Button {
                text: "Export JUnit";
                clicked => { export-report("junit"); }
            }
        }
    }
}
```

---

### T11: UI Results View

Define Slint components for displaying test results.

#### File: `crates/ui/src/ui/results_view.slint`

```slint
// Results View Component
// Displays test results with pass/fail indicators and details

import { VerticalBox, HorizontalBox, Button, ListView, ScrollView } from "std-widgets.slint";
import { Theme } from "main_window.slint";

// Result status for a single assertion
export enum AssertionStatus {
    passed,
    failed,
    error,
}

// Model for a test result
export struct TestResultItem {
    assertion-name: string,
    assertion-type: string,
    status: AssertionStatus,
    expected: string,
    actual: string,
    message: string,
    duration-us: int,
}

// Model for a request result
export struct RequestResultItem {
    request-id: string,
    request-name: string,
    status-code: int,
    duration-ms: int,
    passed: bool,
    test-results: [TestResultItem],
    error-message: string,
    expanded: bool,
}

// Summary statistics
export struct ResultsSummary {
    total-requests: int,
    passed-requests: int,
    failed-requests: int,
    errored-requests: int,
    total-assertions: int,
    passed-assertions: int,
    failed-assertions: int,
    duration-ms: int,
}

// Status icon component
component StatusIcon inherits Rectangle {
    in property <AssertionStatus> status;

    width: 18px;
    height: 18px;
    border-radius: 9px;
    background: status == AssertionStatus.passed ? Theme.accent-success :
                status == AssertionStatus.failed ? Theme.accent-error :
                Theme.accent-warning;

    Text {
        text: status == AssertionStatus.passed ? "✓" :
              status == AssertionStatus.failed ? "✗" : "!";
        color: white;
        font-size: 11px;
        font-weight: 600;
        horizontal-alignment: center;
        vertical-alignment: center;
    }
}

// Single test result row
component TestResultRow inherits Rectangle {
    in property <TestResultItem> result;
    in property <bool> show-details: false;

    height: show-details ? 80px : 32px;
    background: transparent;

    VerticalBox {
        spacing: Theme.spacing-xs;

        HorizontalBox {
            spacing: Theme.spacing-sm;
            padding: Theme.spacing-xs;

            StatusIcon {
                status: result.status;
            }

            Text {
                text: result.assertion-name;
                color: Theme.text-primary;
                font-size: 12px;
                vertical-alignment: center;
            }

            Rectangle { horizontal-stretch: 1; }

            Text {
                text: result.assertion-type;
                color: Theme.text-muted;
                font-size: 11px;
                vertical-alignment: center;
            }

            Text {
                text: (result.duration-us / 1000) + "." + mod(result.duration-us, 1000) / 100 + "ms";
                color: Theme.text-muted;
                font-size: 11px;
                vertical-alignment: center;
            }
        }

        // Details (when expanded and failed)
        if show-details && result.status != AssertionStatus.passed: Rectangle {
            background: Theme.background-tertiary;
            border-radius: 4px;
            height: 44px;

            VerticalBox {
                padding: Theme.spacing-xs;
                spacing: 2px;

                Text {
                    text: "Expected: " + result.expected;
                    color: Theme.text-secondary;
                    font-size: 11px;
                    font-family: "monospace";
                }
                Text {
                    text: "Actual: " + result.actual;
                    color: Theme.accent-error;
                    font-size: 11px;
                    font-family: "monospace";
                }
            }
        }
    }
}

// Request result card
component RequestResultCard inherits Rectangle {
    in-out property <RequestResultItem> result;

    callback toggle-expand();

    background: Theme.background-secondary;
    border-radius: 8px;

    VerticalBox {
        padding: Theme.spacing-sm;
        spacing: Theme.spacing-xs;

        // Header row
        TouchArea {
            clicked => { toggle-expand(); }

            HorizontalBox {
                spacing: Theme.spacing-sm;

                // Expand/collapse icon
                Text {
                    text: result.expanded ? "▼" : "▶";
                    color: Theme.text-muted;
                    font-size: 10px;
                    vertical-alignment: center;
                }

                // Pass/fail indicator
                Rectangle {
                    width: 8px;
                    height: 8px;
                    border-radius: 4px;
                    background: result.error-message != "" ? Theme.accent-warning :
                                result.passed ? Theme.accent-success : Theme.accent-error;
                }

                // Request name
                Text {
                    text: result.request-name;
                    color: Theme.text-primary;
                    font-size: 13px;
                    font-weight: 500;
                    vertical-alignment: center;
                }

                Rectangle { horizontal-stretch: 1; }

                // Status code
                if result.status-code > 0: Text {
                    text: result.status-code;
                    color: result.status-code >= 200 && result.status-code < 300 ? Theme.accent-success :
                           result.status-code >= 400 ? Theme.accent-error : Theme.text-secondary;
                    font-size: 12px;
                    font-weight: 500;
                    vertical-alignment: center;
                }

                // Duration
                Text {
                    text: result.duration-ms + "ms";
                    color: Theme.text-muted;
                    font-size: 11px;
                    vertical-alignment: center;
                }

                // Test count
                Text {
                    text: result.test-results.length + " tests";
                    color: Theme.text-muted;
                    font-size: 11px;
                    vertical-alignment: center;
                }
            }
        }

        // Error message (if any)
        if result.error-message != "": Rectangle {
            background: #f38ba820;
            border-radius: 4px;
            height: 28px;

            Text {
                text: "Error: " + result.error-message;
                color: Theme.accent-error;
                font-size: 11px;
                padding: Theme.spacing-xs;
            }
        }

        // Expanded test results
        if result.expanded: VerticalBox {
            spacing: 2px;
            padding-left: Theme.spacing-md;

            for test-result in result.test-results: TestResultRow {
                result: test-result;
                show-details: test-result.status != AssertionStatus.passed;
            }
        }
    }
}

// Summary bar
component SummaryBar inherits Rectangle {
    in property <ResultsSummary> summary;

    height: 48px;
    background: Theme.background-secondary;
    border-radius: 8px;

    HorizontalBox {
        padding: Theme.spacing-md;
        spacing: Theme.spacing-lg;

        // Pass rate circle
        Rectangle {
            width: 40px;
            height: 40px;
            border-radius: 20px;
            background: summary.failed-requests == 0 && summary.errored-requests == 0 ?
                        Theme.accent-success : Theme.accent-error;

            Text {
                text: summary.total-requests > 0 ?
                      Math.round(summary.passed-requests * 100 / summary.total-requests) + "%" : "0%";
                color: white;
                font-size: 11px;
                font-weight: 600;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }

        // Stats
        VerticalBox {
            spacing: 2px;
            alignment: center;

            HorizontalBox {
                spacing: Theme.spacing-md;

                Text {
                    text: "Requests: " + summary.total-requests;
                    color: Theme.text-primary;
                    font-size: 12px;
                }
                Text {
                    text: "✓ " + summary.passed-requests;
                    color: Theme.accent-success;
                    font-size: 12px;
                }
                Text {
                    text: "✗ " + summary.failed-requests;
                    color: Theme.accent-error;
                    font-size: 12px;
                }
                if summary.errored-requests > 0: Text {
                    text: "⚠ " + summary.errored-requests;
                    color: Theme.accent-warning;
                    font-size: 12px;
                }
            }

            HorizontalBox {
                spacing: Theme.spacing-md;

                Text {
                    text: "Assertions: " + summary.total-assertions;
                    color: Theme.text-secondary;
                    font-size: 11px;
                }
                Text {
                    text: "✓ " + summary.passed-assertions;
                    color: Theme.accent-success;
                    font-size: 11px;
                }
                Text {
                    text: "✗ " + summary.failed-assertions;
                    color: Theme.accent-error;
                    font-size: 11px;
                }
            }
        }

        Rectangle { horizontal-stretch: 1; }

        // Duration
        Text {
            text: (summary.duration-ms / 1000) + "." + mod(summary.duration-ms, 1000) / 100 + "s";
            color: Theme.text-muted;
            font-size: 14px;
            vertical-alignment: center;
        }
    }
}

// Main Results View component
export component ResultsView inherits VerticalBox {
    in property <ResultsSummary> summary;
    in-out property <[RequestResultItem]> results: [];

    callback expand-all();
    callback collapse-all();
    callback export-json();
    callback export-junit();

    padding: Theme.spacing-md;
    spacing: Theme.spacing-md;

    // Header
    HorizontalBox {
        spacing: Theme.spacing-sm;

        Text {
            text: "Test Results";
            font-size: 16px;
            font-weight: 600;
            color: Theme.text-primary;
            vertical-alignment: center;
        }

        Rectangle { horizontal-stretch: 1; }

        Button {
            text: "Expand All";
            clicked => { expand-all(); }
        }
        Button {
            text: "Collapse All";
            clicked => { collapse-all(); }
        }
    }

    // Summary bar
    SummaryBar {
        summary: summary;
    }

    // Results list
    ScrollView {
        VerticalBox {
            spacing: Theme.spacing-sm;

            for result[index] in results: RequestResultCard {
                result: result;
                toggle-expand => {
                    result.expanded = !result.expanded;
                }
            }
        }
    }

    // Export buttons
    HorizontalBox {
        spacing: Theme.spacing-sm;
        alignment: end;

        Button {
            text: "Export JSON Report";
            clicked => { export-json(); }
        }
        Button {
            text: "Export JUnit XML";
            clicked => { export-junit(); }
        }
    }
}
```

---

## Acceptance Criteria

All the following must pass before Sprint 06 is complete:

| Criterion | Verification |
|-----------|--------------|
| All 8 assertion types are implemented | Unit tests pass for each type |
| Assertions evaluate correctly against responses | Integration tests pass |
| JSON path queries work with nested structures | Unit tests with complex JSON |
| Collection runner executes requests in order | Run a 3-request collection |
| Stop on failure works correctly | Test with failing request in middle |
| Retry logic works | Test with initially failing then succeeding request |
| Cancellation stops the run | Cancel during execution |
| JSON report generates valid JSON | Validate with JSON schema |
| JUnit XML is valid | Validate with XML parser |
| CI tools accept JUnit output | Test with GitHub Actions |
| UI shows test editor | Visual inspection |
| UI shows runner progress | Run collection and observe |
| UI shows results with pass/fail | Verify after run completes |

---

## Verification Commands

Run these commands from the workspace root to verify the sprint is complete:

```bash
# 1. Check formatting
cargo fmt --all -- --check

# 2. Run clippy
cargo clippy --workspace --all-targets -- -D warnings

# 3. Build all crates
cargo build --workspace

# 4. Run all tests
cargo test --workspace

# 5. Run specific test module
cargo test --package vortex-domain testing::

# 6. Run application tests
cargo test --package vortex-application testing::

# 7. Run infrastructure tests
cargo test --package vortex-infrastructure json_path::

# 8. Verify Slint compilation
cargo build --package vortex-ui
```

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| JSON path library incompatibilities | Pin `serde_json_path` version, test with Postman JSONPath examples |
| Flaky tests due to timing assertions | Use mock responses with controlled timing |
| Large collections cause memory issues | Implement streaming for results, limit in-memory storage |
| JUnit XML format variations | Test with multiple CI systems, follow standard schema |
| UI thread blocking during runs | Execute runner on tokio runtime, communicate via channels |
| Race conditions in parallel execution | Use proper synchronization primitives |

---

## Notes for AI Agents

1. **Implementation order is critical**: Domain types must be created before application logic.

2. **Testing is essential**: Each component must have unit tests before moving to the next.

3. **JSON Path syntax**: Use RFC 9535 JSONPath syntax (e.g., `$.data.items[0].id`).

4. **Slint file dependencies**: The UI files import from `main_window.slint` for the Theme global.

5. **Module exports**: Remember to update `mod.rs` and `lib.rs` files when adding new modules.

6. **Serde attributes**: Use `#[serde(tag = "type", rename_all = "snake_case")]` for enum serialization.

7. **Error handling**: Never panic in library code; always return `Result`.

8. **Duration serialization**: Use milliseconds for JSON, microseconds for detailed timing.

9. **Report format**: JSON reports should be CI-tool compatible; test with actual CI systems.

10. **Cancellation**: Use `CancellationToken` for cooperative cancellation; check frequently in loops.

---

## Milestone: M6 Completion

When all acceptance criteria pass, Milestone M6 (Tests and Collection Runner) is complete, and the project is ready for Sprint 07 (Extensibility).
