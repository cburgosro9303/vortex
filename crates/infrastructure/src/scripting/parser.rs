//! Script parser for the Vortex DSL.
//!
//! The DSL supports the following commands:
//! - `set("name", "value")` - Set a variable
//! - `setHeader("name", "value")` - Set a request header
//! - `setParam("name", "value")` - Set a query parameter
//! - `log("message")` - Log a message
//! - `skip()` - Skip the request
//! - `delay(ms)` - Delay execution by milliseconds

use thiserror::Error;
use vortex_domain::scripting::ScriptCommand;

/// Error type for script parsing.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Unknown command.
    #[error("Unknown command: {0}")]
    UnknownCommand(String),
    /// Invalid syntax.
    #[error("Invalid syntax at line {line}: {message}")]
    InvalidSyntax {
        /// The line number where the error occurred.
        line: usize,
        /// The error message.
        message: String,
    },
    /// Missing argument.
    #[error("Missing argument for command {command}: expected {expected}")]
    MissingArgument {
        /// The command name.
        command: String,
        /// The expected argument description.
        expected: String,
    },
    /// Invalid argument type.
    #[error("Invalid argument type for {command}: {message}")]
    InvalidArgument {
        /// The command name.
        command: String,
        /// The error message.
        message: String,
    },
}

/// Parse a script into a list of commands.
///
/// # Errors
///
/// Returns an error if the script contains invalid syntax.
pub fn parse_script(script: &str) -> Result<Vec<ScriptCommand>, ParseError> {
    let mut commands = Vec::new();

    for (line_num, line) in script.lines().enumerate() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
            continue;
        }

        let command = parse_line(line, line_num + 1)?;
        commands.push(command);
    }

    Ok(commands)
}

fn parse_line(line: &str, line_num: usize) -> Result<ScriptCommand, ParseError> {
    // Find command name (everything before the first '(')
    let Some(paren_pos) = line.find('(') else {
        return Err(ParseError::InvalidSyntax {
            line: line_num,
            message: "Expected '(' after command name".to_string(),
        });
    };

    let command_name = line[..paren_pos].trim();
    let args_str = line[paren_pos..].trim();

    // Validate closing parenthesis
    if !args_str.ends_with(')') {
        return Err(ParseError::InvalidSyntax {
            line: line_num,
            message: "Missing closing ')'".to_string(),
        });
    }

    // Extract arguments (remove leading '(' and trailing ')')
    let args_content = &args_str[1..args_str.len() - 1];
    let args = parse_arguments(args_content)?;

    match command_name {
        "set" | "setVariable" => {
            if args.len() != 2 {
                return Err(ParseError::MissingArgument {
                    command: command_name.to_string(),
                    expected: "2 arguments (name, value)".to_string(),
                });
            }
            Ok(ScriptCommand::SetVariable {
                name: args[0].clone(),
                value: args[1].clone(),
            })
        }
        "setHeader" => {
            if args.len() != 2 {
                return Err(ParseError::MissingArgument {
                    command: command_name.to_string(),
                    expected: "2 arguments (name, value)".to_string(),
                });
            }
            Ok(ScriptCommand::SetHeader {
                name: args[0].clone(),
                value: args[1].clone(),
            })
        }
        "setParam" | "setQueryParam" => {
            if args.len() != 2 {
                return Err(ParseError::MissingArgument {
                    command: command_name.to_string(),
                    expected: "2 arguments (name, value)".to_string(),
                });
            }
            Ok(ScriptCommand::SetQueryParam {
                name: args[0].clone(),
                value: args[1].clone(),
            })
        }
        "log" | "console.log" => {
            if args.is_empty() {
                return Err(ParseError::MissingArgument {
                    command: command_name.to_string(),
                    expected: "1 argument (message)".to_string(),
                });
            }
            Ok(ScriptCommand::Log {
                message: args.join(", "),
            })
        }
        "skip" => Ok(ScriptCommand::Skip),
        "delay" | "sleep" => {
            if args.is_empty() {
                return Err(ParseError::MissingArgument {
                    command: command_name.to_string(),
                    expected: "1 argument (milliseconds)".to_string(),
                });
            }
            let millis = args[0].parse().map_err(|_| ParseError::InvalidArgument {
                command: command_name.to_string(),
                message: format!("'{}' is not a valid number", args[0]),
            })?;
            Ok(ScriptCommand::Delay { millis })
        }
        "assert" => {
            if args.is_empty() {
                return Err(ParseError::MissingArgument {
                    command: command_name.to_string(),
                    expected: "1-2 arguments (condition, optional message)".to_string(),
                });
            }
            Ok(ScriptCommand::Assert {
                condition: args[0].clone(),
                message: args.get(1).cloned(),
            })
        }
        _ => Err(ParseError::UnknownCommand(command_name.to_string())),
    }
}

#[allow(clippy::unnecessary_wraps)]
fn parse_arguments(args_str: &str) -> Result<Vec<String>, ParseError> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut string_char = '"';
    let mut escape_next = false;

    for ch in args_str.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
            continue;
        }

        match ch {
            '\\' => {
                escape_next = true;
            }
            '"' | '\'' => {
                if !in_string {
                    in_string = true;
                    string_char = ch;
                } else if ch == string_char {
                    in_string = false;
                } else {
                    current.push(ch);
                }
            }
            ',' => {
                if in_string {
                    current.push(ch);
                } else {
                    let arg = current.trim().to_string();
                    if !arg.is_empty() {
                        args.push(arg);
                    }
                    current.clear();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    // Add the last argument
    let arg = current.trim().to_string();
    if !arg.is_empty() {
        args.push(arg);
    }

    Ok(args)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_set_variable() {
        let script = r#"set("token", "abc123")"#;
        let commands = parse_script(script).expect("should parse");
        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0],
            ScriptCommand::SetVariable {
                name: "token".to_string(),
                value: "abc123".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_set_header() {
        let script = r#"setHeader("Authorization", "Bearer token")"#;
        let commands = parse_script(script).expect("should parse");
        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0],
            ScriptCommand::SetHeader {
                name: "Authorization".to_string(),
                value: "Bearer token".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_set_param() {
        let script = r#"setParam("page", "1")"#;
        let commands = parse_script(script).expect("should parse");
        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0],
            ScriptCommand::SetQueryParam {
                name: "page".to_string(),
                value: "1".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_log() {
        let script = r#"log("Hello, World!")"#;
        let commands = parse_script(script).expect("should parse");
        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0],
            ScriptCommand::Log {
                message: "Hello, World!".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_skip() {
        let script = "skip()";
        let commands = parse_script(script).expect("should parse");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], ScriptCommand::Skip);
    }

    #[test]
    fn test_parse_delay() {
        let script = "delay(1000)";
        let commands = parse_script(script).expect("should parse");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], ScriptCommand::Delay { millis: 1000 });
    }

    #[test]
    fn test_parse_multiple_commands() {
        let script = r#"
            // Set up authentication
            set("userId", "123")
            setHeader("X-User-Id", "123")
            log("Request prepared")
        "#;
        let commands = parse_script(script).expect("should parse");
        assert_eq!(commands.len(), 3);
    }

    #[test]
    fn test_skip_comments() {
        let script = r#"
            // This is a comment
            # This is also a comment
            log("Hello")
        "#;
        let commands = parse_script(script).expect("should parse");
        assert_eq!(commands.len(), 1);
    }

    #[test]
    fn test_unknown_command() {
        let script = r#"unknown("arg")"#;
        let result = parse_script(script);
        assert!(result.is_err());
        assert!(matches!(result, Err(ParseError::UnknownCommand(_))));
    }

    #[test]
    fn test_missing_parenthesis() {
        let script = "set";
        let result = parse_script(script);
        assert!(result.is_err());
    }

    #[test]
    fn test_single_quoted_strings() {
        let script = "set('key', 'value')";
        let commands = parse_script(script).expect("should parse");
        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0],
            ScriptCommand::SetVariable {
                name: "key".to_string(),
                value: "value".to_string(),
            }
        );
    }

    #[test]
    fn test_escaped_quotes() {
        let script = r#"log("He said \"hello\"")"#;
        let commands = parse_script(script).expect("should parse");
        assert_eq!(commands.len(), 1);
        if let ScriptCommand::Log { message } = &commands[0] {
            assert!(message.contains("hello"));
        } else {
            panic!("Expected Log command");
        }
    }

    #[test]
    fn test_assert_with_message() {
        let script = r#"assert("status == 200", "Expected success status")"#;
        let commands = parse_script(script).expect("should parse");
        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0],
            ScriptCommand::Assert {
                condition: "status == 200".to_string(),
                message: Some("Expected success status".to_string()),
            }
        );
    }
}
