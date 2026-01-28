//! HTTP infrastructure utilities.
//!
//! This module provides:
//! - Body building for various content types
//! - TLS configuration (planned)

mod body_builder;

pub use body_builder::{build_body, BodyBuildError, BuiltBody};
