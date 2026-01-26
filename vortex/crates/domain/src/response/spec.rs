//! Response specification type

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::request::Headers;

/// HTTP response specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseSpec {
    /// HTTP status code
    pub status: u16,
    /// Status text (e.g., "OK", "Not Found")
    pub status_text: String,
    /// Response headers
    pub headers: Headers,
    /// Response body as string
    pub body: String,
    /// Response time
    #[serde(with = "duration_millis")]
    pub duration: Duration,
    /// Response size in bytes
    pub size: usize,
}

impl ResponseSpec {
    /// Returns true if the status code indicates success (2xx).
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    /// Returns true if the status code indicates a client error (4xx).
    #[must_use]
    pub const fn is_client_error(&self) -> bool {
        self.status >= 400 && self.status < 500
    }

    /// Returns true if the status code indicates a server error (5xx).
    #[must_use]
    pub const fn is_server_error(&self) -> bool {
        self.status >= 500 && self.status < 600
    }
}

impl Default for ResponseSpec {
    fn default() -> Self {
        Self {
            status: 0,
            status_text: String::new(),
            headers: Headers::new(),
            body: String::new(),
            duration: Duration::ZERO,
            size: 0,
        }
    }
}

mod duration_millis {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    #[allow(clippy::cast_possible_truncation)]
    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Truncation is acceptable: durations over ~584 million years are not realistic
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_checks() {
        let response_200 = ResponseSpec {
            status: 200,
            ..Default::default()
        };
        assert!(response_200.is_success());
        assert!(!response_200.is_client_error());
        assert!(!response_200.is_server_error());

        let response_404 = ResponseSpec {
            status: 404,
            ..Default::default()
        };
        assert!(!response_404.is_success());
        assert!(response_404.is_client_error());

        let response_500 = ResponseSpec {
            status: 500,
            ..Default::default()
        };
        assert!(response_500.is_server_error());
    }
}
