//! Code generation infrastructure.
//!
//! This module provides code generators for various programming languages.

mod generator;

pub use generator::{generate_code, CodeGenerator};
