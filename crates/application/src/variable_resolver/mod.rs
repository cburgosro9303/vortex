//! Variable resolution module
//!
//! Provides parsing and resolution of `{{variable}}` syntax in strings.
//!
//! # Usage
//!
//! ```
//! use vortex_application::variable_resolver::{VariableResolver, parser};
//! use vortex_domain::environment::{ResolutionContext, Environment, Globals, SecretsStore};
//!
//! // Create a resolution context
//! let mut env = Environment::new("development");
//! env.add_variable("host", "localhost");
//!
//! let ctx = ResolutionContext::from_environment(&env, &SecretsStore::new());
//! let mut resolver = VariableResolver::new(ctx);
//!
//! // Resolve variables
//! let result = resolver.resolve("http://{{host}}/api");
//! assert_eq!(result.resolved, "http://localhost/api");
//! ```

pub mod builtins;
pub mod engine;
pub mod parser;

pub use builtins::{BuiltinInfo, BuiltinVariables};
pub use engine::{ResolutionResult, VariableResolver};
pub use parser::{
    VariableReference, extract_variable_names, has_variables, is_valid_variable_name,
    parse_variables,
};
