//! Script executor for running parsed commands.

use std::collections::HashMap;

use vortex_domain::scripting::{Script, ScriptCommand, ScriptResult};

use super::parser::parse_script;

/// Context for script execution.
#[derive(Debug, Clone, Default)]
pub struct ScriptContext {
    /// Current environment variables.
    pub variables: HashMap<String, String>,
    /// Response status (for post-response scripts).
    pub status: Option<u16>,
    /// Response body (for post-response scripts).
    pub body: Option<String>,
    /// Response headers (for post-response scripts).
    pub response_headers: HashMap<String, String>,
}

impl ScriptContext {
    /// Create a new empty context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a context with variables.
    #[must_use]
    pub fn with_variables(variables: HashMap<String, String>) -> Self {
        Self {
            variables,
            ..Default::default()
        }
    }

    /// Set response data for post-response scripts.
    #[must_use]
    pub fn with_response(
        mut self,
        status: u16,
        body: String,
        headers: HashMap<String, String>,
    ) -> Self {
        self.status = Some(status);
        self.body = Some(body);
        self.response_headers = headers;
        self
    }
}

/// Script executor that runs scripts and produces results.
#[derive(Debug, Default)]
pub struct ScriptExecutor;

impl ScriptExecutor {
    /// Create a new script executor.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Execute a script with the given context.
    ///
    /// # Errors
    ///
    /// Returns an error if the script fails to parse or execute.
    #[must_use] 
    pub fn execute(&self, script: &Script, context: &ScriptContext) -> ScriptResult {
        if !script.should_run() {
            return ScriptResult::success();
        }

        // Parse the script
        let commands = match parse_script(&script.content) {
            Ok(cmds) => cmds,
            Err(e) => return ScriptResult::error(e.to_string()),
        };

        // Execute commands
        self.execute_commands(&commands, context)
    }

    /// Execute a list of commands.
    #[must_use] 
    pub fn execute_commands(
        &self,
        commands: &[ScriptCommand],
        context: &ScriptContext,
    ) -> ScriptResult {
        let mut result = ScriptResult::success();

        for command in commands {
            match self.execute_command(command, context, &result) {
                Ok(cmd_result) => {
                    // Merge command result into overall result
                    result.variables.extend(cmd_result.variables);
                    result.headers.extend(cmd_result.headers);
                    result.query_params.extend(cmd_result.query_params);
                    result.logs.extend(cmd_result.logs);
                    if cmd_result.skip_request {
                        result.skip_request = true;
                    }
                    if cmd_result.delay_millis > 0 {
                        result.delay_millis =
                            result.delay_millis.saturating_add(cmd_result.delay_millis);
                    }
                }
                Err(e) => {
                    return ScriptResult::error(e);
                }
            }
        }

        result
    }

    fn execute_command(
        &self,
        command: &ScriptCommand,
        context: &ScriptContext,
        _current_result: &ScriptResult,
    ) -> Result<ScriptResult, String> {
        let mut result = ScriptResult::success();

        match command {
            ScriptCommand::SetVariable { name, value } => {
                let resolved_value = self.resolve_value(value, context);
                result.add_variable(name, resolved_value);
            }
            ScriptCommand::SetHeader { name, value } => {
                let resolved_value = self.resolve_value(value, context);
                result.add_header(name, resolved_value);
            }
            ScriptCommand::SetQueryParam { name, value } => {
                let resolved_value = self.resolve_value(value, context);
                result.add_query_param(name, resolved_value);
            }
            ScriptCommand::Log { message } => {
                let resolved_message = self.resolve_value(message, context);
                result.add_log(resolved_message);
            }
            ScriptCommand::Skip => {
                result.skip_request = true;
            }
            ScriptCommand::Delay { millis } => {
                result.delay_millis = *millis;
            }
            ScriptCommand::Assert { condition, message } => {
                // Simple assertion evaluation
                if !self.evaluate_condition(condition, context) {
                    let error_msg = message
                        .clone()
                        .unwrap_or_else(|| format!("Assertion failed: {condition}"));
                    return Err(error_msg);
                }
            }
        }

        Ok(result)
    }

    #[allow(clippy::unused_self)]
    fn resolve_value(&self, value: &str, context: &ScriptContext) -> String {
        let mut result = value.to_string();

        // Replace {{variable}} patterns
        #[allow(clippy::expect_used)]
        let var_pattern = regex::Regex::new(r"\{\{(\w+)\}\}").expect("valid regex");
        for cap in var_pattern.captures_iter(value) {
            let var_name = &cap[1];
            if let Some(var_value) = context.variables.get(var_name) {
                result = result.replace(&cap[0], var_value);
            }
        }

        // Replace special values
        result = result.replace(
            "{{$status}}",
            &context.status.map_or(String::new(), |s| s.to_string()),
        );
        if let Some(body) = &context.body {
            result = result.replace("{{$body}}", body);
        }

        result
    }

    fn evaluate_condition(&self, condition: &str, context: &ScriptContext) -> bool {
        let condition = condition.trim();

        // Handle simple comparisons
        if let Some((left, right)) = condition.split_once("==") {
            let left_val = self.resolve_value(left.trim(), context);
            let right_val = self.resolve_value(right.trim(), context);
            return left_val == right_val;
        }

        if let Some((left, right)) = condition.split_once("!=") {
            let left_val = self.resolve_value(left.trim(), context);
            let right_val = self.resolve_value(right.trim(), context);
            return left_val != right_val;
        }

        if let Some((left, right)) = condition.split_once(">=") {
            return compare_numeric(left.trim(), right.trim(), context, |a, b| a >= b);
        }

        if let Some((left, right)) = condition.split_once("<=") {
            return compare_numeric(left.trim(), right.trim(), context, |a, b| a <= b);
        }

        if let Some((left, right)) = condition.split_once('>') {
            return compare_numeric(left.trim(), right.trim(), context, |a, b| a > b);
        }

        if let Some((left, right)) = condition.split_once('<') {
            return compare_numeric(left.trim(), right.trim(), context, |a, b| a < b);
        }

        // Handle truthy check
        let resolved = self.resolve_value(condition, context);
        !resolved.is_empty() && resolved != "false" && resolved != "0"
    }
}

fn compare_numeric<F>(left: &str, right: &str, context: &ScriptContext, cmp: F) -> bool
where
    F: Fn(f64, f64) -> bool,
{
    let executor = ScriptExecutor::new();
    let left_val = executor.resolve_value(left, context);
    let right_val = executor.resolve_value(right, context);

    match (left_val.parse::<f64>(), right_val.parse::<f64>()) {
        (Ok(l), Ok(r)) => cmp(l, r),
        _ => false,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_set_variable() {
        let executor = ScriptExecutor::new();
        let script = Script::with_content(r#"set("token", "abc123")"#);
        let context = ScriptContext::new();

        let result = executor.execute(&script, &context);
        assert!(result.success);
        assert_eq!(
            result.variables,
            vec![("token".to_string(), "abc123".to_string())]
        );
    }

    #[test]
    fn test_execute_set_header() {
        let executor = ScriptExecutor::new();
        let script = Script::with_content(r#"setHeader("X-Custom", "value")"#);
        let context = ScriptContext::new();

        let result = executor.execute(&script, &context);
        assert!(result.success);
        assert_eq!(
            result.headers,
            vec![("X-Custom".to_string(), "value".to_string())]
        );
    }

    #[test]
    fn test_execute_skip() {
        let executor = ScriptExecutor::new();
        let script = Script::with_content("skip()");
        let context = ScriptContext::new();

        let result = executor.execute(&script, &context);
        assert!(result.success);
        assert!(result.skip_request);
    }

    #[test]
    fn test_execute_delay() {
        let executor = ScriptExecutor::new();
        let script = Script::with_content("delay(500)");
        let context = ScriptContext::new();

        let result = executor.execute(&script, &context);
        assert!(result.success);
        assert_eq!(result.delay_millis, 500);
    }

    #[test]
    fn test_execute_log() {
        let executor = ScriptExecutor::new();
        let script = Script::with_content(r#"log("Hello, World!")"#);
        let context = ScriptContext::new();

        let result = executor.execute(&script, &context);
        assert!(result.success);
        assert_eq!(result.logs, vec!["Hello, World!".to_string()]);
    }

    #[test]
    fn test_variable_interpolation() {
        let executor = ScriptExecutor::new();
        let script = Script::with_content(r#"setHeader("Authorization", "Bearer {{token}}")"#);
        let mut variables = HashMap::new();
        variables.insert("token".to_string(), "secret123".to_string());
        let context = ScriptContext::with_variables(variables);

        let result = executor.execute(&script, &context);
        assert!(result.success);
        assert_eq!(
            result.headers,
            vec![("Authorization".to_string(), "Bearer secret123".to_string())]
        );
    }

    #[test]
    fn test_assert_success() {
        let executor = ScriptExecutor::new();
        let script = Script::with_content(r#"assert("200 == 200")"#);
        let context = ScriptContext::new();

        let result = executor.execute(&script, &context);
        assert!(result.success);
    }

    #[test]
    fn test_assert_failure() {
        let executor = ScriptExecutor::new();
        let script = Script::with_content(r#"assert("200 == 404", "Status mismatch")"#);
        let context = ScriptContext::new();

        let result = executor.execute(&script, &context);
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("Status mismatch"));
    }

    #[test]
    fn test_assert_with_context_status() {
        let executor = ScriptExecutor::new();
        let script = Script::with_content(r#"assert("{{$status}} == 200")"#);
        let context = ScriptContext::new().with_response(200, String::new(), HashMap::new());

        let result = executor.execute(&script, &context);
        assert!(result.success);
    }

    #[test]
    fn test_multiple_commands() {
        let executor = ScriptExecutor::new();
        let script = Script::with_content(
            r#"
            set("userId", "123")
            setHeader("X-User-Id", "{{userId}}")
            log("Setup complete")
        "#,
        );
        let context = ScriptContext::new();

        let result = executor.execute(&script, &context);
        assert!(result.success);
        assert_eq!(result.variables.len(), 1);
        assert_eq!(result.headers.len(), 1);
        assert_eq!(result.logs.len(), 1);
    }

    #[test]
    fn test_disabled_script() {
        let executor = ScriptExecutor::new();
        let mut script = Script::with_content("skip()");
        script.enabled = false;
        let context = ScriptContext::new();

        let result = executor.execute(&script, &context);
        assert!(result.success);
        assert!(!result.skip_request); // Should not skip because script is disabled
    }

    #[test]
    fn test_numeric_comparison() {
        let executor = ScriptExecutor::new();
        let script = Script::with_content(r#"assert("{{$status}} >= 200")"#);
        let context = ScriptContext::new().with_response(201, String::new(), HashMap::new());

        let result = executor.execute(&script, &context);
        assert!(result.success);
    }
}
