//! Request History Domain Model
//!
//! Defines the structure for tracking executed requests.

use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::request::HttpMethod;

/// A header stored in history.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistoryHeader {
    /// Header name.
    pub key: String,
    /// Header value.
    pub value: String,
    /// Whether this header is enabled.
    pub enabled: bool,
}

/// A query parameter stored in history.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistoryParam {
    /// Parameter name.
    pub key: String,
    /// Parameter value.
    pub value: String,
    /// Whether this parameter is enabled.
    pub enabled: bool,
}

/// Authentication data stored in history.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistoryAuth {
    /// Authentication type: 0=None, 1=Bearer, 2=Basic, 3=API Key.
    pub auth_type: i32,
    /// Bearer token value.
    pub bearer_token: String,
    /// Basic auth username.
    pub basic_username: String,
    /// Basic auth password.
    pub basic_password: String,
    /// API key header/param name.
    pub api_key_name: String,
    /// API key value.
    pub api_key_value: String,
    /// API key location: 0=Header, 1=Query.
    pub api_key_location: i32,
}

/// A single entry in the request history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Unique identifier for this entry.
    pub id: String,
    /// When the request was executed.
    pub timestamp: DateTime<Utc>,
    /// HTTP method used.
    pub method: HttpMethod,
    /// The request URL.
    pub url: String,
    /// Response status code (if available).
    pub status_code: Option<u16>,
    /// Request duration in milliseconds.
    pub duration_ms: Option<u64>,
    /// Optional name of the request (from collection).
    pub request_name: Option<String>,
    /// Request body (for POST, PUT, PATCH).
    #[serde(default)]
    pub body: Option<String>,
    /// Request headers.
    #[serde(default)]
    pub headers: Vec<HistoryHeader>,
    /// Query parameters.
    #[serde(default)]
    pub params: Vec<HistoryParam>,
    /// Authentication data.
    #[serde(default)]
    pub auth: Option<HistoryAuth>,
}

impl HistoryEntry {
    /// Creates a new history entry for a successful request.
    #[must_use]
    pub fn new(
        method: HttpMethod,
        url: String,
        status_code: u16,
        duration_ms: u64,
        request_name: Option<String>,
        body: Option<String>,
        headers: Vec<HistoryHeader>,
        params: Vec<HistoryParam>,
        auth: Option<HistoryAuth>,
    ) -> Self {
        Self {
            id: crate::generate_id(),
            timestamp: Utc::now(),
            method,
            url,
            status_code: Some(status_code),
            duration_ms: Some(duration_ms),
            request_name,
            body,
            headers,
            params,
            auth,
        }
    }

    /// Creates a history entry for a failed request.
    #[must_use]
    pub fn failed(
        method: HttpMethod,
        url: String,
        request_name: Option<String>,
        body: Option<String>,
        headers: Vec<HistoryHeader>,
        params: Vec<HistoryParam>,
        auth: Option<HistoryAuth>,
    ) -> Self {
        Self {
            id: crate::generate_id(),
            timestamp: Utc::now(),
            method,
            url,
            status_code: None,
            duration_ms: None,
            request_name,
            body,
            headers,
            params,
            auth,
        }
    }

    /// Returns a human-readable "time ago" string.
    #[must_use]
    pub fn time_ago(&self) -> String {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.timestamp);

        if duration.num_seconds() < 60 {
            "just now".to_string()
        } else if duration.num_minutes() < 60 {
            let mins = duration.num_minutes();
            format!("{mins}m ago")
        } else if duration.num_hours() < 24 {
            let hours = duration.num_hours();
            format!("{hours}h ago")
        } else if duration.num_days() < 7 {
            let days = duration.num_days();
            format!("{days}d ago")
        } else {
            self.timestamp.format("%Y-%m-%d").to_string()
        }
    }

    /// Returns the duration as a display string.
    #[must_use]
    pub fn duration_display(&self) -> String {
        match self.duration_ms {
            Some(ms) if ms < 1000 => format!("{ms}ms"),
            Some(ms) => format!("{:.1}s", ms as f64 / 1000.0),
            None => "-".to_string(),
        }
    }
}

/// Request history with a maximum size limit.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RequestHistory {
    /// History entries (newest first).
    entries: VecDeque<HistoryEntry>,
    /// Maximum number of entries to keep.
    #[serde(default = "default_max_entries")]
    max_entries: usize,
}

fn default_max_entries() -> usize {
    100
}

impl RequestHistory {
    /// Creates a new empty history.
    #[must_use]
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries,
        }
    }

    /// Adds an entry to the history (at the front).
    pub fn add(&mut self, entry: HistoryEntry) {
        self.entries.push_front(entry);

        // Trim to max size
        while self.entries.len() > self.max_entries {
            self.entries.pop_back();
        }
    }

    /// Returns all entries (newest first).
    #[must_use]
    pub fn entries(&self) -> &VecDeque<HistoryEntry> {
        &self.entries
    }

    /// Returns an entry by ID.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&HistoryEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Clears all history entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns the number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if history is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Sets the maximum number of entries and trims if needed.
    pub fn set_max_entries(&mut self, max: usize) {
        self.max_entries = max;
        while self.entries.len() > max {
            self.entries.pop_back();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_entry_creation() {
        let entry = HistoryEntry::new(
            HttpMethod::Get,
            "https://api.example.com".to_string(),
            200,
            150,
            Some("Test Request".to_string()),
            None,
            vec![],
            vec![],
            None,
        );

        assert_eq!(entry.method, HttpMethod::Get);
        assert_eq!(entry.status_code, Some(200));
        assert_eq!(entry.duration_ms, Some(150));
    }

    #[test]
    fn test_history_max_entries() {
        let mut history = RequestHistory::new(3);

        for i in 0..5 {
            history.add(HistoryEntry::new(
                HttpMethod::Get,
                format!("https://example.com/{i}"),
                200,
                100,
                None,
                None,
                vec![],
                vec![],
                None,
            ));
        }

        assert_eq!(history.len(), 3);
        // Newest should be first
        assert!(history.entries()[0].url.contains("/4"));
    }

    #[test]
    fn test_duration_display() {
        let entry = HistoryEntry::new(
            HttpMethod::Get,
            "https://example.com".to_string(),
            200,
            150,
            None,
            None,
            vec![],
            vec![],
            None,
        );
        assert_eq!(entry.duration_display(), "150ms");

        let entry2 = HistoryEntry::new(
            HttpMethod::Get,
            "https://example.com".to_string(),
            200,
            1500,
            None,
            None,
            vec![],
            vec![],
            None,
        );
        assert_eq!(entry2.duration_display(), "1.5s");
    }
}
