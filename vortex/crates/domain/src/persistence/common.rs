//! Common types shared across persistence models.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Current schema version for all Vortex file formats.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// A UUID string type for stable identifiers.
/// Using String instead of uuid::Uuid to avoid external dependency in domain.
pub type Id = String;

/// Ordered key-value map for deterministic serialization.
/// BTreeMap guarantees alphabetical key ordering.
pub type OrderedMap = BTreeMap<String, String>;

/// Settings that can be applied at various levels (workspace, collection, request).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestSettings {
    /// Request timeout in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,

    /// Whether to follow HTTP redirects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub follow_redirects: Option<bool>,

    /// Maximum number of redirects to follow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_redirects: Option<u32>,

    /// Whether to verify SSL certificates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify_ssl: Option<bool>,
}

/// HTTP methods supported by Vortex (persistence format).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum PersistenceHttpMethod {
    /// HTTP GET method
    #[default]
    Get,
    /// HTTP POST method
    Post,
    /// HTTP PUT method
    Put,
    /// HTTP PATCH method
    Patch,
    /// HTTP DELETE method
    Delete,
    /// HTTP HEAD method
    Head,
    /// HTTP OPTIONS method
    Options,
    /// HTTP TRACE method
    Trace,
}

impl std::fmt::Display for PersistenceHttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
            Self::Trace => "TRACE",
        };
        write!(f, "{s}")
    }
}

impl From<crate::request::HttpMethod> for PersistenceHttpMethod {
    fn from(method: crate::request::HttpMethod) -> Self {
        match method {
            crate::request::HttpMethod::Get => Self::Get,
            crate::request::HttpMethod::Post => Self::Post,
            crate::request::HttpMethod::Put => Self::Put,
            crate::request::HttpMethod::Patch => Self::Patch,
            crate::request::HttpMethod::Delete => Self::Delete,
            crate::request::HttpMethod::Head => Self::Head,
            crate::request::HttpMethod::Options => Self::Options,
        }
    }
}

impl From<PersistenceHttpMethod> for crate::request::HttpMethod {
    fn from(method: PersistenceHttpMethod) -> Self {
        match method {
            PersistenceHttpMethod::Get => Self::Get,
            PersistenceHttpMethod::Post => Self::Post,
            PersistenceHttpMethod::Put => Self::Put,
            PersistenceHttpMethod::Patch => Self::Patch,
            PersistenceHttpMethod::Delete => Self::Delete,
            PersistenceHttpMethod::Head => Self::Head,
            PersistenceHttpMethod::Options => Self::Options,
            PersistenceHttpMethod::Trace => Self::Options, // Map TRACE to OPTIONS as fallback
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persistence_http_method_display() {
        assert_eq!(PersistenceHttpMethod::Get.to_string(), "GET");
        assert_eq!(PersistenceHttpMethod::Post.to_string(), "POST");
        assert_eq!(PersistenceHttpMethod::Trace.to_string(), "TRACE");
    }

    #[test]
    fn test_request_settings_default() {
        let settings = RequestSettings::default();
        assert!(settings.timeout_ms.is_none());
        assert!(settings.follow_redirects.is_none());
    }
}
