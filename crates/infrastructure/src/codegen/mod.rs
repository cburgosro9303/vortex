//! Code generation infrastructure.
//!
//! This module provides code generators for various programming languages.

mod generator;

pub use generator::{CodeGenerator, generate_code};
