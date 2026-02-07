//! Script execution infrastructure.
//!
//! This module provides script parsing and execution capabilities.

mod executor;
mod parser;

pub use executor::ScriptExecutor;
pub use parser::{parse_script, ParseError};
