//! Deterministic JSON serialization for Vortex file format.
//!
//! Ensures clean Git diffs by:
//! - Sorting object keys alphabetically (via `BTreeMap` in domain types)
//! - Using 2-space indentation
//! - Adding trailing newline
//! - UTF-8 encoding without BOM

mod json;

pub use json::*;
