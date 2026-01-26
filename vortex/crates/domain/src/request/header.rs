//! HTTP Header types

use serde::{Deserialize, Serialize};

/// A single HTTP header with name and value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    /// The header name (e.g., "Content-Type")
    pub name: String,
    /// The header value (e.g., "application/json")
    pub value: String,
    /// Whether this header is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

const fn default_enabled() -> bool {
    true
}

impl Header {
    /// Creates a new enabled header.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            enabled: true,
        }
    }

    /// Creates a new disabled header.
    #[must_use]
    pub fn disabled(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            enabled: false,
        }
    }
}

/// A collection of HTTP headers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Headers {
    items: Vec<Header>,
}

impl Headers {
    /// Creates an empty header collection.
    #[must_use]
    pub const fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Adds a header to the collection.
    pub fn add(&mut self, header: Header) {
        self.items.push(header);
    }

    /// Returns an iterator over enabled headers.
    pub fn enabled(&self) -> impl Iterator<Item = &Header> {
        self.items.iter().filter(|h| h.enabled)
    }

    /// Returns all headers (enabled and disabled).
    #[must_use]
    pub fn all(&self) -> &[Header] {
        &self.items
    }

    /// Returns the number of headers.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::len is not const in stable
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns true if there are no headers.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::is_empty is not const in stable
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl FromIterator<Header> for Headers {
    fn from_iter<T: IntoIterator<Item = Header>>(iter: T) -> Self {
        Self {
            items: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_creation() {
        let header = Header::new("Content-Type", "application/json");
        assert_eq!(header.name, "Content-Type");
        assert_eq!(header.value, "application/json");
        assert!(header.enabled);
    }

    #[test]
    fn test_disabled_header() {
        let header = Header::disabled("X-Debug", "true");
        assert!(!header.enabled);
    }

    #[test]
    fn test_headers_filter_enabled() {
        let mut headers = Headers::new();
        headers.add(Header::new("Accept", "application/json"));
        headers.add(Header::disabled("X-Debug", "true"));
        headers.add(Header::new("User-Agent", "Vortex"));

        assert_eq!(headers.enabled().count(), 2);
    }
}
