//! Variable parser for {{variable}} syntax
//!
//! Parses strings to extract variable references with their positions.

use std::ops::Range;

/// Represents a parsed variable reference in a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableReference {
    /// The variable name (without {{ }}).
    pub name: String,

    /// Whether this is a built-in variable (starts with $).
    pub is_builtin: bool,

    /// Byte range in the original string where this reference appears.
    pub span: Range<usize>,
}

impl VariableReference {
    /// Creates a new variable reference.
    #[must_use]
    pub fn new(name: impl Into<String>, span: Range<usize>) -> Self {
        let name = name.into();
        let is_builtin = name.starts_with('$');
        Self {
            name,
            is_builtin,
            span,
        }
    }
}

/// Parses a string and extracts all variable references.
///
/// Supports:
/// - `{{variable_name}}` - user-defined variables
/// - `{{$uuid}}` - built-in dynamic variables
///
/// # Examples
///
/// ```
/// use vortex_application::variable_resolver::parser::parse_variables;
///
/// let refs = parse_variables("Hello {{name}}, your ID is {{$uuid}}");
/// assert_eq!(refs.len(), 2);
/// assert_eq!(refs[0].name, "name");
/// assert_eq!(refs[1].name, "$uuid");
/// assert!(refs[1].is_builtin);
/// ```
#[must_use]
pub fn parse_variables(input: &str) -> Vec<VariableReference> {
    let mut references = Vec::new();
    let mut chars = input.char_indices().peekable();

    while let Some((i, ch)) = chars.next() {
        if ch == '{' {
            // Check for {{
            if let Some((_, next_ch)) = chars.peek() {
                if *next_ch == '{' {
                    chars.next(); // consume second {
                    let start = i;
                    let mut name = String::new();
                    let mut found_end = false;

                    // Read until }}
                    while let Some((_, ch)) = chars.next() {
                        if ch == '}' {
                            if let Some((end_idx, '}')) = chars.peek() {
                                let end = *end_idx + 1;
                                chars.next(); // consume second }

                                let trimmed_name = name.trim().to_string();
                                if !trimmed_name.is_empty() {
                                    references.push(VariableReference::new(trimmed_name, start..end));
                                }
                                found_end = true;
                                break;
                            }
                        }
                        name.push(ch);
                    }

                    // If we didn't find the closing }}, skip to avoid infinite loop
                    if !found_end {
                        break;
                    }
                }
            }
        }
    }

    references
}

/// Validates a variable name.
/// Valid names: alphanumeric, underscore, hyphen, and optionally starting with $ for built-ins.
#[must_use]
pub fn is_valid_variable_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let name = if let Some(stripped) = name.strip_prefix('$') {
        stripped
    } else {
        name
    };

    if name.is_empty() {
        return false;
    }

    // First character must be letter or underscore
    let mut chars = name.chars();
    if let Some(first) = chars.next() {
        if !first.is_alphabetic() && first != '_' {
            return false;
        }
    }

    // Remaining characters must be alphanumeric, underscore, or hyphen
    chars.all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// Returns true if the input string contains any variable references.
#[must_use]
pub fn has_variables(input: &str) -> bool {
    input.contains("{{") && input.contains("}}")
}

/// Extracts just the variable names from the input without full parsing info.
#[must_use]
pub fn extract_variable_names(input: &str) -> Vec<String> {
    parse_variables(input)
        .into_iter()
        .map(|r| r.name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_variable() {
        let refs = parse_variables("{{name}}");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "name");
        assert!(!refs[0].is_builtin);
        assert_eq!(refs[0].span, 0..8);
    }

    #[test]
    fn test_parse_builtin_variable() {
        let refs = parse_variables("{{$uuid}}");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "$uuid");
        assert!(refs[0].is_builtin);
    }

    #[test]
    fn test_parse_multiple_variables() {
        let refs = parse_variables("{{base_url}}/api/{{version}}/users/{{$uuid}}");
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0].name, "base_url");
        assert_eq!(refs[1].name, "version");
        assert_eq!(refs[2].name, "$uuid");
    }

    #[test]
    fn test_parse_with_whitespace() {
        let refs = parse_variables("{{ name }}");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "name");
    }

    #[test]
    fn test_no_variables() {
        let refs = parse_variables("Hello, World!");
        assert!(refs.is_empty());
    }

    #[test]
    fn test_unclosed_variable() {
        let refs = parse_variables("{{name");
        assert!(refs.is_empty());
    }

    #[test]
    fn test_empty_variable() {
        let refs = parse_variables("{{}}");
        assert!(refs.is_empty());
    }

    #[test]
    fn test_whitespace_only_variable() {
        let refs = parse_variables("{{   }}");
        assert!(refs.is_empty());
    }

    #[test]
    fn test_variable_with_underscores() {
        let refs = parse_variables("{{my_variable_name}}");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "my_variable_name");
    }

    #[test]
    fn test_variable_with_numbers() {
        let refs = parse_variables("{{var123}}");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "var123");
    }

    #[test]
    fn test_variable_in_url() {
        let refs = parse_variables("https://{{host}}:{{port}}/{{path}}?key={{api_key}}");
        assert_eq!(refs.len(), 4);
        assert_eq!(refs[0].name, "host");
        assert_eq!(refs[1].name, "port");
        assert_eq!(refs[2].name, "path");
        assert_eq!(refs[3].name, "api_key");
    }

    #[test]
    fn test_variable_in_json() {
        let refs = parse_variables(r#"{"name": "{{user_name}}", "id": "{{$uuid}}"}"#);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].name, "user_name");
        assert_eq!(refs[1].name, "$uuid");
    }

    #[test]
    fn test_adjacent_variables() {
        let refs = parse_variables("{{a}}{{b}}{{c}}");
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0].name, "a");
        assert_eq!(refs[1].name, "b");
        assert_eq!(refs[2].name, "c");
    }

    #[test]
    fn test_single_brace() {
        let refs = parse_variables("{name}");
        assert!(refs.is_empty());
    }

    #[test]
    fn test_valid_variable_names() {
        assert!(is_valid_variable_name("name"));
        assert!(is_valid_variable_name("my_var"));
        assert!(is_valid_variable_name("myVar123"));
        assert!(is_valid_variable_name("_private"));
        assert!(is_valid_variable_name("var-name"));
        assert!(is_valid_variable_name("$uuid"));
        assert!(is_valid_variable_name("$timestamp"));
    }

    #[test]
    fn test_invalid_variable_names() {
        assert!(!is_valid_variable_name(""));
        assert!(!is_valid_variable_name("$"));
        assert!(!is_valid_variable_name("123var"));
        assert!(!is_valid_variable_name("-start"));
    }

    #[test]
    fn test_has_variables() {
        assert!(has_variables("{{name}}"));
        assert!(has_variables("Hello {{name}}!"));
        assert!(!has_variables("Hello World!"));
        assert!(!has_variables("{{incomplete"));
        assert!(!has_variables("incomplete}}"));
    }

    #[test]
    fn test_extract_variable_names() {
        let names = extract_variable_names("{{a}} and {{b}} and {{c}}");
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_span_positions() {
        let input = "Hello {{name}}, welcome!";
        let refs = parse_variables(input);
        assert_eq!(refs.len(), 1);
        assert_eq!(&input[refs[0].span.clone()], "{{name}}");
    }
}
