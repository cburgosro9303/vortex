//! Code generation types for generating code snippets from requests.
//!
//! This module provides types for generating code in various programming
//! languages from HTTP request specifications.

use serde::{Deserialize, Serialize};

/// Supported programming languages for code generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CodeLanguage {
    /// cURL command line
    #[default]
    Curl,
    /// Python with requests library
    Python,
    /// JavaScript with fetch API
    JavaScript,
    /// JavaScript with Axios library
    JavaScriptAxios,
    /// TypeScript with fetch API
    TypeScript,
    /// Rust with reqwest library
    Rust,
    /// Go with net/http
    Go,
    /// Java with HttpClient
    Java,
    /// C# with HttpClient
    CSharp,
    /// PHP with cURL
    Php,
    /// Ruby with Net::HTTP
    Ruby,
    /// Swift with URLSession
    Swift,
    /// Kotlin with OkHttp
    Kotlin,
}

impl CodeLanguage {
    /// Get display name for the language.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Curl => "cURL",
            Self::Python => "Python (requests)",
            Self::JavaScript => "JavaScript (fetch)",
            Self::JavaScriptAxios => "JavaScript (axios)",
            Self::TypeScript => "TypeScript (fetch)",
            Self::Rust => "Rust (reqwest)",
            Self::Go => "Go (net/http)",
            Self::Java => "Java (HttpClient)",
            Self::CSharp => "C# (HttpClient)",
            Self::Php => "PHP (cURL)",
            Self::Ruby => "Ruby (Net::HTTP)",
            Self::Swift => "Swift (URLSession)",
            Self::Kotlin => "Kotlin (OkHttp)",
        }
    }

    /// Get file extension for the language.
    #[must_use]
    pub const fn file_extension(&self) -> &'static str {
        match self {
            Self::Curl => "sh",
            Self::Python => "py",
            Self::JavaScript | Self::JavaScriptAxios => "js",
            Self::TypeScript => "ts",
            Self::Rust => "rs",
            Self::Go => "go",
            Self::Java => "java",
            Self::CSharp => "cs",
            Self::Php => "php",
            Self::Ruby => "rb",
            Self::Swift => "swift",
            Self::Kotlin => "kt",
        }
    }

    /// Get all available languages.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Curl,
            Self::Python,
            Self::JavaScript,
            Self::JavaScriptAxios,
            Self::TypeScript,
            Self::Rust,
            Self::Go,
            Self::Java,
            Self::CSharp,
            Self::Php,
            Self::Ruby,
            Self::Swift,
            Self::Kotlin,
        ]
    }
}

impl std::fmt::Display for CodeLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Options for code generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGenOptions {
    /// Target programming language.
    pub language: CodeLanguage,
    /// Include comments in generated code.
    #[serde(default = "default_true")]
    pub include_comments: bool,
    /// Use pretty formatting (indentation, newlines).
    #[serde(default = "default_true")]
    pub pretty_format: bool,
    /// Include error handling code.
    #[serde(default)]
    pub include_error_handling: bool,
    /// Use async/await syntax where applicable.
    #[serde(default = "default_true")]
    pub use_async: bool,
    /// Indent size (spaces).
    #[serde(default = "default_indent")]
    pub indent_size: usize,
}

const fn default_true() -> bool {
    true
}

const fn default_indent() -> usize {
    4
}

impl Default for CodeGenOptions {
    fn default() -> Self {
        Self {
            language: CodeLanguage::default(),
            include_comments: true,
            pretty_format: true,
            include_error_handling: false,
            use_async: true,
            indent_size: 4,
        }
    }
}

impl CodeGenOptions {
    /// Create options for a specific language with defaults.
    #[must_use]
    pub fn for_language(language: CodeLanguage) -> Self {
        Self {
            language,
            ..Default::default()
        }
    }

    /// Get indentation string.
    #[must_use]
    pub fn indent(&self) -> String {
        " ".repeat(self.indent_size)
    }

    /// Get double indentation string.
    #[must_use]
    pub fn indent2(&self) -> String {
        " ".repeat(self.indent_size * 2)
    }
}

/// Generated code snippet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSnippet {
    /// The generated code.
    pub code: String,
    /// Language of the generated code.
    pub language: CodeLanguage,
    /// Optional imports/dependencies needed.
    pub imports: Vec<String>,
    /// Optional setup code (e.g., client initialization).
    pub setup: Option<String>,
}

impl CodeSnippet {
    /// Create a new code snippet.
    #[must_use]
    pub fn new(code: impl Into<String>, language: CodeLanguage) -> Self {
        Self {
            code: code.into(),
            language,
            imports: Vec::new(),
            setup: None,
        }
    }

    /// Add an import.
    pub fn with_import(mut self, import: impl Into<String>) -> Self {
        self.imports.push(import.into());
        self
    }

    /// Add multiple imports.
    pub fn with_imports(mut self, imports: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.imports.extend(imports.into_iter().map(Into::into));
        self
    }

    /// Add setup code.
    pub fn with_setup(mut self, setup: impl Into<String>) -> Self {
        self.setup = Some(setup.into());
        self
    }

    /// Get the complete code including imports and setup.
    #[must_use]
    pub fn full_code(&self) -> String {
        let mut parts = Vec::new();

        if !self.imports.is_empty() {
            parts.push(self.imports.join("\n"));
            parts.push(String::new());
        }

        if let Some(setup) = &self.setup {
            parts.push(setup.clone());
            parts.push(String::new());
        }

        parts.push(self.code.clone());

        parts.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_language_display() {
        assert_eq!(CodeLanguage::Curl.display_name(), "cURL");
        assert_eq!(CodeLanguage::Python.display_name(), "Python (requests)");
        assert_eq!(CodeLanguage::Rust.display_name(), "Rust (reqwest)");
    }

    #[test]
    fn test_code_language_extension() {
        assert_eq!(CodeLanguage::Curl.file_extension(), "sh");
        assert_eq!(CodeLanguage::Python.file_extension(), "py");
        assert_eq!(CodeLanguage::Rust.file_extension(), "rs");
    }

    #[test]
    fn test_code_gen_options_default() {
        let options = CodeGenOptions::default();
        assert!(options.include_comments);
        assert!(options.pretty_format);
        assert!(!options.include_error_handling);
        assert!(options.use_async);
        assert_eq!(options.indent_size, 4);
    }

    #[test]
    fn test_code_snippet_full_code() {
        let snippet = CodeSnippet::new("print('hello')", CodeLanguage::Python)
            .with_import("import requests")
            .with_setup("# Setup");

        let full = snippet.full_code();
        assert!(full.contains("import requests"));
        assert!(full.contains("# Setup"));
        assert!(full.contains("print('hello')"));
    }

    #[test]
    fn test_all_languages() {
        let all = CodeLanguage::all();
        assert!(all.len() >= 10);
        assert!(all.contains(&CodeLanguage::Curl));
        assert!(all.contains(&CodeLanguage::Python));
    }
}
