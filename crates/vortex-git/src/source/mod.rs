//! Configuration source abstraction.
//!
//! This module defines the core trait for configuration sources and related types.

mod query;
mod result;
mod traits;

pub use query::ConfigQuery;
pub use result::ConfigResult;
pub use traits::ConfigSource;
