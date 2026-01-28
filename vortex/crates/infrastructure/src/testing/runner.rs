//! Test runner implementation.
//!
//! Executes assertions against HTTP responses and produces test results.

use std::time::Instant;

use regex::Regex;
use vortex_domain::response::ResponseSpec;
use vortex_domain::testing::{
    Assertion, AssertionResult, ComparisonOperator, StatusExpectation, TestResults, TestSuite,
};

/// Test runner that executes assertions against responses.
#[derive(Debug, Default)]
pub struct TestRunner {
    /// Whether to stop on first failure.
    stop_on_failure: bool,
}

impl TestRunner {
    /// Create a new test runner.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            stop_on_failure: false,
        }
    }

    /// Set whether to stop on first failure.
    #[must_use]
    pub const fn with_stop_on_failure(mut self, stop: bool) -> Self {
        self.stop_on_failure = stop;
        self
    }

    /// Run a test suite against a response.
    #[must_use]
    pub fn run(&self, suite: &TestSuite, response: &ResponseSpec) -> TestResults {
        let start = Instant::now();
        let mut results = Vec::with_capacity(suite.assertions.len());

        for assertion in &suite.assertions {
            let result = self.run_assertion(assertion, response);
            let failed = !result.passed;
            results.push(result);

            if failed && (self.stop_on_failure || suite.stop_on_failure) {
                break;
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        TestResults::new(&suite.name, results, duration_ms)
    }

    /// Run a single assertion against a response.
    #[must_use]
    pub fn run_assertion(&self, assertion: &Assertion, response: &ResponseSpec) -> AssertionResult {
        match assertion {
            Assertion::StatusCode { expected } => self.check_status_code(assertion, response, expected),
            Assertion::ResponseTime { max_ms } => self.check_response_time(assertion, response, *max_ms),
            Assertion::HeaderExists { name, value } => {
                self.check_header_exists(assertion, response, name, value.as_deref())
            }
            Assertion::HeaderMatches { name, pattern } => {
                self.check_header_matches(assertion, response, name, pattern)
            }
            Assertion::BodyContains { text, ignore_case } => {
                self.check_body_contains(assertion, response, text, *ignore_case)
            }
            Assertion::BodyMatches { pattern } => self.check_body_matches(assertion, response, pattern),
            Assertion::JsonPath { path, expected } => {
                self.check_json_path(assertion, response, path, expected.as_ref())
            }
            Assertion::JsonPathMatches {
                path,
                operator,
                value,
            } => self.check_json_path_matches(assertion, response, path, *operator, value),
            Assertion::BodyEquals { expected } => self.check_body_equals(assertion, response, expected),
            Assertion::IsJson => self.check_is_json(assertion, response),
            Assertion::IsXml => self.check_is_xml(assertion, response),
            Assertion::ContentType { expected } => {
                self.check_content_type(assertion, response, expected)
            }
            Assertion::BodyLength { operator, length } => {
                self.check_body_length(assertion, response, *operator, *length)
            }
        }
    }

    fn check_status_code(
        &self,
        assertion: &Assertion,
        response: &ResponseSpec,
        expected: &StatusExpectation,
    ) -> AssertionResult {
        let actual = response.status;
        if expected.matches(actual) {
            AssertionResult::pass_with_value(assertion.clone(), actual.to_string())
        } else {
            AssertionResult::fail_with_value(
                assertion.clone(),
                actual.to_string(),
                format!("Expected status {}, got {}", expected.description(), actual),
            )
        }
    }

    fn check_response_time(
        &self,
        assertion: &Assertion,
        response: &ResponseSpec,
        max_ms: u64,
    ) -> AssertionResult {
        let actual_ms = response.duration.as_millis() as u64;
        if actual_ms <= max_ms {
            AssertionResult::pass_with_value(assertion.clone(), format!("{}ms", actual_ms))
        } else {
            AssertionResult::fail_with_value(
                assertion.clone(),
                format!("{}ms", actual_ms),
                format!("Response took {}ms, expected <= {}ms", actual_ms, max_ms),
            )
        }
    }

    fn check_header_exists(
        &self,
        assertion: &Assertion,
        response: &ResponseSpec,
        name: &str,
        expected_value: Option<&str>,
    ) -> AssertionResult {
        match response.get_header(name) {
            Some(actual_value) => {
                if let Some(expected) = expected_value {
                    if actual_value == expected {
                        AssertionResult::pass_with_value(assertion.clone(), actual_value.clone())
                    } else {
                        AssertionResult::fail_with_value(
                            assertion.clone(),
                            actual_value.clone(),
                            format!("Header '{}' value mismatch: expected '{}', got '{}'", name, expected, actual_value),
                        )
                    }
                } else {
                    AssertionResult::pass_with_value(assertion.clone(), actual_value.clone())
                }
            }
            None => AssertionResult::fail(
                assertion.clone(),
                format!("Header '{}' not found", name),
            ),
        }
    }

    fn check_header_matches(
        &self,
        assertion: &Assertion,
        response: &ResponseSpec,
        name: &str,
        pattern: &str,
    ) -> AssertionResult {
        match response.get_header(name) {
            Some(actual_value) => match Regex::new(pattern) {
                Ok(regex) => {
                    if regex.is_match(actual_value) {
                        AssertionResult::pass_with_value(assertion.clone(), actual_value.clone())
                    } else {
                        AssertionResult::fail_with_value(
                            assertion.clone(),
                            actual_value.clone(),
                            format!("Header '{}' value '{}' does not match pattern '{}'", name, actual_value, pattern),
                        )
                    }
                }
                Err(e) => AssertionResult::fail(
                    assertion.clone(),
                    format!("Invalid regex pattern '{}': {}", pattern, e),
                ),
            },
            None => AssertionResult::fail(
                assertion.clone(),
                format!("Header '{}' not found", name),
            ),
        }
    }

    fn check_body_contains(
        &self,
        assertion: &Assertion,
        response: &ResponseSpec,
        text: &str,
        ignore_case: bool,
    ) -> AssertionResult {
        let body = &response.body;
        let contains = if ignore_case {
            body.to_lowercase().contains(&text.to_lowercase())
        } else {
            body.contains(text)
        };

        if contains {
            AssertionResult::pass(assertion.clone())
        } else {
            let preview = if body.len() > 100 {
                format!("{}...", &body[..100])
            } else {
                body.clone()
            };
            AssertionResult::fail_with_value(
                assertion.clone(),
                preview,
                format!("Body does not contain '{}'", text),
            )
        }
    }

    fn check_body_matches(
        &self,
        assertion: &Assertion,
        response: &ResponseSpec,
        pattern: &str,
    ) -> AssertionResult {
        match Regex::new(pattern) {
            Ok(regex) => {
                if regex.is_match(&response.body) {
                    AssertionResult::pass(assertion.clone())
                } else {
                    let preview = if response.body.len() > 100 {
                        format!("{}...", &response.body[..100])
                    } else {
                        response.body.clone()
                    };
                    AssertionResult::fail_with_value(
                        assertion.clone(),
                        preview,
                        format!("Body does not match pattern '{}'", pattern),
                    )
                }
            }
            Err(e) => AssertionResult::fail(
                assertion.clone(),
                format!("Invalid regex pattern '{}': {}", pattern, e),
            ),
        }
    }

    fn check_json_path(
        &self,
        assertion: &Assertion,
        response: &ResponseSpec,
        path: &str,
        expected: Option<&serde_json::Value>,
    ) -> AssertionResult {
        // Parse JSON body
        let json = match serde_json::from_str::<serde_json::Value>(&response.body) {
            Ok(json) => json,
            Err(e) => {
                return AssertionResult::fail(
                    assertion.clone(),
                    format!("Failed to parse body as JSON: {}", e),
                )
            }
        };

        // Query JSON path
        match query_json_path(&json, path) {
            Ok(Some(value)) => {
                if let Some(expected_value) = expected {
                    if &value == expected_value {
                        AssertionResult::pass_with_value(assertion.clone(), value.to_string())
                    } else {
                        AssertionResult::fail_with_value(
                            assertion.clone(),
                            value.to_string(),
                            format!("JSON path '{}' value mismatch: expected {}, got {}", path, expected_value, value),
                        )
                    }
                } else {
                    AssertionResult::pass_with_value(assertion.clone(), value.to_string())
                }
            }
            Ok(None) => AssertionResult::fail(
                assertion.clone(),
                format!("JSON path '{}' not found", path),
            ),
            Err(e) => AssertionResult::fail(
                assertion.clone(),
                format!("Invalid JSON path '{}': {}", path, e),
            ),
        }
    }

    fn check_json_path_matches(
        &self,
        assertion: &Assertion,
        response: &ResponseSpec,
        path: &str,
        operator: ComparisonOperator,
        expected: &serde_json::Value,
    ) -> AssertionResult {
        // Parse JSON body
        let json = match serde_json::from_str::<serde_json::Value>(&response.body) {
            Ok(json) => json,
            Err(e) => {
                return AssertionResult::fail(
                    assertion.clone(),
                    format!("Failed to parse body as JSON: {}", e),
                )
            }
        };

        // Query JSON path
        match query_json_path(&json, path) {
            Ok(Some(value)) => {
                if compare_json_values(&value, operator, expected) {
                    AssertionResult::pass_with_value(assertion.clone(), value.to_string())
                } else {
                    AssertionResult::fail_with_value(
                        assertion.clone(),
                        value.to_string(),
                        format!(
                            "JSON path '{}' comparison failed: {} {} {}",
                            path,
                            value,
                            operator.symbol(),
                            expected
                        ),
                    )
                }
            }
            Ok(None) => AssertionResult::fail(
                assertion.clone(),
                format!("JSON path '{}' not found", path),
            ),
            Err(e) => AssertionResult::fail(
                assertion.clone(),
                format!("Invalid JSON path '{}': {}", path, e),
            ),
        }
    }

    fn check_body_equals(
        &self,
        assertion: &Assertion,
        response: &ResponseSpec,
        expected: &str,
    ) -> AssertionResult {
        if response.body == expected {
            AssertionResult::pass(assertion.clone())
        } else {
            let preview = if response.body.len() > 100 {
                format!("{}...", &response.body[..100])
            } else {
                response.body.clone()
            };
            AssertionResult::fail_with_value(
                assertion.clone(),
                preview,
                "Body does not match expected value".to_string(),
            )
        }
    }

    fn check_is_json(&self, assertion: &Assertion, response: &ResponseSpec) -> AssertionResult {
        match serde_json::from_str::<serde_json::Value>(&response.body) {
            Ok(_) => AssertionResult::pass(assertion.clone()),
            Err(e) => AssertionResult::fail(
                assertion.clone(),
                format!("Body is not valid JSON: {}", e),
            ),
        }
    }

    fn check_is_xml(&self, assertion: &Assertion, response: &ResponseSpec) -> AssertionResult {
        // Simple XML check: must start with < and contain valid structure
        let body = response.body.trim();
        if body.starts_with('<') && (body.starts_with("<?xml") || body.starts_with('<')) {
            // More thorough check: balanced tags
            if body.ends_with('>') && body.matches('<').count() == body.matches('>').count() {
                return AssertionResult::pass(assertion.clone());
            }
        }
        AssertionResult::fail(
            assertion.clone(),
            "Body does not appear to be valid XML".to_string(),
        )
    }

    fn check_content_type(
        &self,
        assertion: &Assertion,
        response: &ResponseSpec,
        expected: &str,
    ) -> AssertionResult {
        match &response.content_type {
            Some(actual) => {
                if actual.contains(expected) {
                    AssertionResult::pass_with_value(assertion.clone(), actual.clone())
                } else {
                    AssertionResult::fail_with_value(
                        assertion.clone(),
                        actual.clone(),
                        format!("Content-Type '{}' does not contain '{}'", actual, expected),
                    )
                }
            }
            None => AssertionResult::fail(
                assertion.clone(),
                "No Content-Type header present".to_string(),
            ),
        }
    }

    fn check_body_length(
        &self,
        assertion: &Assertion,
        response: &ResponseSpec,
        operator: ComparisonOperator,
        expected_length: usize,
    ) -> AssertionResult {
        let actual_length = response.body.len();
        let matches = match operator {
            ComparisonOperator::Equals => actual_length == expected_length,
            ComparisonOperator::NotEquals => actual_length != expected_length,
            ComparisonOperator::GreaterThan => actual_length > expected_length,
            ComparisonOperator::GreaterThanOrEqual => actual_length >= expected_length,
            ComparisonOperator::LessThan => actual_length < expected_length,
            ComparisonOperator::LessThanOrEqual => actual_length <= expected_length,
            ComparisonOperator::Contains | ComparisonOperator::Matches => false,
        };

        if matches {
            AssertionResult::pass_with_value(assertion.clone(), actual_length.to_string())
        } else {
            AssertionResult::fail_with_value(
                assertion.clone(),
                actual_length.to_string(),
                format!(
                    "Body length {} does not {} {}",
                    actual_length,
                    operator.symbol(),
                    expected_length
                ),
            )
        }
    }
}

/// Query a JSON value using a simple JSONPath-like syntax.
/// Supports: $.field, $.field.nested, $.array[0], $.array[*]
fn query_json_path(json: &serde_json::Value, path: &str) -> Result<Option<serde_json::Value>, String> {
    let path = path.trim();
    if !path.starts_with('$') {
        return Err("JSON path must start with '$'".to_string());
    }

    let path = &path[1..]; // Remove $
    if path.is_empty() {
        return Ok(Some(json.clone()));
    }

    let path = path.strip_prefix('.').unwrap_or(path);
    let mut current = json.clone();

    for segment in split_path_segments(path) {
        // Check for array index
        if let Some((name, index)) = parse_array_access(&segment) {
            if !name.is_empty() {
                current = match current.get(&name) {
                    Some(v) => v.clone(),
                    None => return Ok(None),
                };
            }
            if index == "*" {
                // Return all array elements
                return Ok(Some(current));
            }
            let idx: usize = index.parse().map_err(|_| format!("Invalid array index: {}", index))?;
            current = match current.get(idx) {
                Some(v) => v.clone(),
                None => return Ok(None),
            };
        } else {
            current = match current.get(&segment) {
                Some(v) => v.clone(),
                None => return Ok(None),
            };
        }
    }

    Ok(Some(current))
}

/// Split a path into segments, respecting array brackets.
fn split_path_segments(path: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_bracket = false;

    for ch in path.chars() {
        match ch {
            '.' if !in_bracket => {
                if !current.is_empty() {
                    segments.push(std::mem::take(&mut current));
                }
            }
            '[' => {
                in_bracket = true;
                current.push(ch);
            }
            ']' => {
                in_bracket = false;
                current.push(ch);
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        segments.push(current);
    }

    segments
}

/// Parse array access like "field[0]" into ("field", "0").
fn parse_array_access(segment: &str) -> Option<(String, String)> {
    if let Some(bracket_start) = segment.find('[') {
        if segment.ends_with(']') {
            let name = segment[..bracket_start].to_string();
            let index = segment[bracket_start + 1..segment.len() - 1].to_string();
            return Some((name, index));
        }
    }
    None
}

/// Compare two JSON values using the given operator.
fn compare_json_values(
    actual: &serde_json::Value,
    operator: ComparisonOperator,
    expected: &serde_json::Value,
) -> bool {
    use serde_json::Value;

    match operator {
        ComparisonOperator::Equals => actual == expected,
        ComparisonOperator::NotEquals => actual != expected,
        ComparisonOperator::GreaterThan => compare_numeric(actual, expected, |a, b| a > b),
        ComparisonOperator::GreaterThanOrEqual => compare_numeric(actual, expected, |a, b| a >= b),
        ComparisonOperator::LessThan => compare_numeric(actual, expected, |a, b| a < b),
        ComparisonOperator::LessThanOrEqual => compare_numeric(actual, expected, |a, b| a <= b),
        ComparisonOperator::Contains => {
            match (actual, expected) {
                (Value::String(s), Value::String(needle)) => s.contains(needle.as_str()),
                (Value::Array(arr), _) => arr.contains(expected),
                _ => false,
            }
        }
        ComparisonOperator::Matches => {
            if let (Value::String(s), Value::String(pattern)) = (actual, expected) {
                Regex::new(pattern).map(|re| re.is_match(s)).unwrap_or(false)
            } else {
                false
            }
        }
    }
}

/// Compare numeric values.
fn compare_numeric<F>(actual: &serde_json::Value, expected: &serde_json::Value, cmp: F) -> bool
where
    F: Fn(f64, f64) -> bool,
{
    match (actual.as_f64(), expected.as_f64()) {
        (Some(a), Some(b)) => cmp(a, b),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::Duration;

    fn create_response(status: u16, body: &str, headers: HashMap<String, String>) -> ResponseSpec {
        ResponseSpec::new(status, headers, body.as_bytes().to_vec(), Duration::from_millis(50))
    }

    fn json_response(status: u16, body: &str) -> ResponseSpec {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        create_response(status, body, headers)
    }

    #[test]
    fn test_status_code_exact() {
        let runner = TestRunner::new();
        let response = create_response(200, "", HashMap::new());

        let assertion = Assertion::StatusCode {
            expected: StatusExpectation::exact(200),
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);

        let assertion = Assertion::StatusCode {
            expected: StatusExpectation::exact(201),
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(!result.passed);
    }

    #[test]
    fn test_status_code_range() {
        let runner = TestRunner::new();
        let response = create_response(201, "", HashMap::new());

        let assertion = Assertion::StatusCode {
            expected: StatusExpectation::success(),
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);
    }

    #[test]
    fn test_response_time() {
        let runner = TestRunner::new();
        let response = create_response(200, "", HashMap::new());

        let assertion = Assertion::ResponseTime { max_ms: 100 };
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);

        let assertion = Assertion::ResponseTime { max_ms: 10 };
        let result = runner.run_assertion(&assertion, &response);
        assert!(!result.passed);
    }

    #[test]
    fn test_header_exists() {
        let runner = TestRunner::new();
        let mut headers = HashMap::new();
        headers.insert("X-Custom".to_string(), "value123".to_string());
        let response = create_response(200, "", headers);

        let assertion = Assertion::HeaderExists {
            name: "X-Custom".to_string(),
            value: None,
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);

        let assertion = Assertion::HeaderExists {
            name: "X-Custom".to_string(),
            value: Some("value123".to_string()),
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);

        let assertion = Assertion::HeaderExists {
            name: "X-Missing".to_string(),
            value: None,
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(!result.passed);
    }

    #[test]
    fn test_header_matches() {
        let runner = TestRunner::new();
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer abc123".to_string());
        let response = create_response(200, "", headers);

        let assertion = Assertion::HeaderMatches {
            name: "Authorization".to_string(),
            pattern: r"Bearer \w+".to_string(),
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);
    }

    #[test]
    fn test_body_contains() {
        let runner = TestRunner::new();
        let response = create_response(200, "Hello World!", HashMap::new());

        let assertion = Assertion::BodyContains {
            text: "World".to_string(),
            ignore_case: false,
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);

        let assertion = Assertion::BodyContains {
            text: "world".to_string(),
            ignore_case: true,
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);

        let assertion = Assertion::BodyContains {
            text: "world".to_string(),
            ignore_case: false,
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(!result.passed);
    }

    #[test]
    fn test_body_matches() {
        let runner = TestRunner::new();
        let response = create_response(200, "ID: 12345", HashMap::new());

        let assertion = Assertion::BodyMatches {
            pattern: r"ID: \d+".to_string(),
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);
    }

    #[test]
    fn test_json_path() {
        let runner = TestRunner::new();
        let response = json_response(200, r#"{"user": {"id": 123, "name": "John"}}"#);

        let assertion = Assertion::JsonPath {
            path: "$.user.id".to_string(),
            expected: Some(serde_json::json!(123)),
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);

        let assertion = Assertion::JsonPath {
            path: "$.user.name".to_string(),
            expected: None,
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);

        let assertion = Assertion::JsonPath {
            path: "$.user.missing".to_string(),
            expected: None,
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(!result.passed);
    }

    #[test]
    fn test_json_path_with_array() {
        let runner = TestRunner::new();
        let response = json_response(200, r#"{"items": [{"id": 1}, {"id": 2}]}"#);

        let assertion = Assertion::JsonPath {
            path: "$.items[0].id".to_string(),
            expected: Some(serde_json::json!(1)),
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);
    }

    #[test]
    fn test_json_path_matches() {
        let runner = TestRunner::new();
        let response = json_response(200, r#"{"count": 10}"#);

        let assertion = Assertion::JsonPathMatches {
            path: "$.count".to_string(),
            operator: ComparisonOperator::GreaterThan,
            value: serde_json::json!(5),
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);

        let assertion = Assertion::JsonPathMatches {
            path: "$.count".to_string(),
            operator: ComparisonOperator::LessThan,
            value: serde_json::json!(5),
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(!result.passed);
    }

    #[test]
    fn test_is_json() {
        let runner = TestRunner::new();

        let response = create_response(200, r#"{"valid": true}"#, HashMap::new());
        let assertion = Assertion::IsJson;
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);

        let response = create_response(200, "not json", HashMap::new());
        let result = runner.run_assertion(&assertion, &response);
        assert!(!result.passed);
    }

    #[test]
    fn test_is_xml() {
        let runner = TestRunner::new();

        let response = create_response(200, "<?xml version=\"1.0\"?><root><item/></root>", HashMap::new());
        let assertion = Assertion::IsXml;
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);
    }

    #[test]
    fn test_content_type() {
        let runner = TestRunner::new();
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json; charset=utf-8".to_string());
        let response = create_response(200, "{}", headers);

        let assertion = Assertion::ContentType {
            expected: "application/json".to_string(),
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);
    }

    #[test]
    fn test_body_length() {
        let runner = TestRunner::new();
        let response = create_response(200, "Hello", HashMap::new());

        let assertion = Assertion::BodyLength {
            operator: ComparisonOperator::Equals,
            length: 5,
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);

        let assertion = Assertion::BodyLength {
            operator: ComparisonOperator::GreaterThan,
            length: 3,
        };
        let result = runner.run_assertion(&assertion, &response);
        assert!(result.passed);
    }

    #[test]
    fn test_run_suite() {
        let runner = TestRunner::new();
        let response = json_response(200, r#"{"success": true}"#);

        let suite = TestSuite::new("API Test")
            .with_assertion(Assertion::StatusCode {
                expected: StatusExpectation::success(),
            })
            .with_assertion(Assertion::IsJson)
            .with_assertion(Assertion::JsonPath {
                path: "$.success".to_string(),
                expected: Some(serde_json::json!(true)),
            });

        let results = runner.run(&suite, &response);
        assert!(results.all_passed());
        assert_eq!(results.total, 3);
        assert_eq!(results.passed, 3);
    }

    #[test]
    fn test_stop_on_failure() {
        let runner = TestRunner::new().with_stop_on_failure(true);
        let response = create_response(404, "Not Found", HashMap::new());

        let suite = TestSuite::new("Failing Test")
            .with_assertion(Assertion::StatusCode {
                expected: StatusExpectation::exact(200),
            })
            .with_assertion(Assertion::BodyContains {
                text: "success".to_string(),
                ignore_case: false,
            });

        let results = runner.run(&suite, &response);
        assert!(!results.all_passed());
        assert_eq!(results.results.len(), 1); // Stopped after first failure
    }
}
