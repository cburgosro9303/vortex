//! Configuration file reading and parsing.
//!
//! This module provides functionality for reading and parsing configuration files
//! following Spring Cloud Config conventions.

mod format;
mod parser;
mod resolver;

pub use format::ConfigFormat;
pub use parser::ConfigParser;
pub use resolver::ConfigFileResolver;
