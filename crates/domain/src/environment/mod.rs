//! Environment and variable domain types

mod globals;
mod resolution;
mod secrets;
mod variable;

pub use globals::Globals;
pub use resolution::ResolutionContext;
pub use secrets::SecretsStore;
pub use variable::{Environment, ResolvedVariable, Variable, VariableMap, VariableScope};
