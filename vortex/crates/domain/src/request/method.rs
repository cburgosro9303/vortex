//! HTTP Method enumeration

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::error::{DomainError, DomainResult};

/// Supported HTTP methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
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
}

impl HttpMethod {
    /// Returns all available HTTP methods.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Get,
            Self::Post,
            Self::Put,
            Self::Patch,
            Self::Delete,
            Self::Head,
            Self::Options,
        ]
    }

    /// Returns whether this method typically has a request body.
    #[must_use]
    pub const fn has_body(self) -> bool {
        matches!(self, Self::Post | Self::Put | Self::Patch)
    }

    /// Returns the method as a static string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for HttpMethod {
    type Err = DomainError;

    fn from_str(s: &str) -> DomainResult<Self> {
        match s.to_uppercase().as_str() {
            "GET" => Ok(Self::Get),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "PATCH" => Ok(Self::Patch),
            "DELETE" => Ok(Self::Delete),
            "HEAD" => Ok(Self::Head),
            "OPTIONS" => Ok(Self::Options),
            other => Err(DomainError::UnsupportedMethod(other.to_string())),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_method_from_str() {
        assert_eq!("get".parse::<HttpMethod>().unwrap(), HttpMethod::Get);
        assert_eq!("POST".parse::<HttpMethod>().unwrap(), HttpMethod::Post);
        assert_eq!("Put".parse::<HttpMethod>().unwrap(), HttpMethod::Put);
    }

    #[test]
    fn test_method_display() {
        assert_eq!(HttpMethod::Get.to_string(), "GET");
        assert_eq!(HttpMethod::Post.to_string(), "POST");
    }

    #[test]
    fn test_invalid_method() {
        let result = "INVALID".parse::<HttpMethod>();
        assert!(result.is_err());
    }

    #[test]
    fn test_has_body() {
        assert!(!HttpMethod::Get.has_body());
        assert!(HttpMethod::Post.has_body());
        assert!(HttpMethod::Put.has_body());
        assert!(HttpMethod::Patch.has_body());
        assert!(!HttpMethod::Delete.has_body());
    }
}
