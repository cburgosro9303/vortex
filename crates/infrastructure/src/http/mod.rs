//! HTTP infrastructure utilities.
//!
//! This module provides:
//! - Body building for various content types
//! - TLS configuration (planned)

mod body_builder;

pub use body_builder::{BodyBuildError, BuiltBody, build_body};
