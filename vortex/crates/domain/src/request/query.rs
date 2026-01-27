//! Query parameter types

use serde::{Deserialize, Serialize};

/// A query parameter key-value pair.
///
/// Supports enable/disable without deletion for UI convenience.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryParam {
    /// The parameter key
    pub key: String,
    /// The parameter value
    pub value: String,
    /// Whether this parameter is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Optional description for documentation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

const fn default_enabled() -> bool {
    true
}

impl QueryParam {
    /// Creates a new enabled query parameter.
    #[must_use]
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            enabled: true,
            description: None,
        }
    }

    /// Creates a disabled query parameter.
    #[must_use]
    pub fn disabled(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            enabled: false,
            description: None,
        }
    }

    /// Adds a description to this parameter.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// A collection of query parameters.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct QueryParams {
    items: Vec<QueryParam>,
}

impl QueryParams {
    /// Creates an empty query parameter collection.
    #[must_use]
    pub const fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Adds a query parameter to the collection.
    pub fn add(&mut self, param: QueryParam) {
        self.items.push(param);
    }

    /// Returns an iterator over enabled parameters.
    pub fn enabled(&self) -> impl Iterator<Item = &QueryParam> {
        self.items.iter().filter(|p| p.enabled)
    }

    /// Returns all parameters (enabled and disabled).
    #[must_use]
    pub fn all(&self) -> &[QueryParam] {
        &self.items
    }

    /// Returns the number of parameters.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns true if there are no parameters.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl FromIterator<QueryParam> for QueryParams {
    fn from_iter<T: IntoIterator<Item = QueryParam>>(iter: T) -> Self {
        Self {
            items: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_param_creation() {
        let param = QueryParam::new("page", "1");
        assert_eq!(param.key, "page");
        assert_eq!(param.value, "1");
        assert!(param.enabled);
    }

    #[test]
    fn test_disabled_param() {
        let param = QueryParam::disabled("debug", "true");
        assert!(!param.enabled);
    }

    #[test]
    fn test_query_params_filter_enabled() {
        let mut params = QueryParams::new();
        params.add(QueryParam::new("page", "1"));
        params.add(QueryParam::disabled("debug", "true"));
        params.add(QueryParam::new("limit", "10"));

        assert_eq!(params.enabled().count(), 2);
    }
}
