//! Cache module for Vortex Config Server.
//!
//! This module provides a high-performance cache layer using Moka,
//! with support for TTL-based expiration, pattern-based invalidation,
//! and metrics.

pub mod config_cache;
pub mod invalidation;
pub mod keys;

// Re-exports
pub use config_cache::{CacheConfig, CacheError, ConfigCache};
pub use invalidation::InvalidationResult;
pub use keys::CacheKey;
