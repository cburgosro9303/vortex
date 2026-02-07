//! WebSocket connection types.
//!
//! This module provides types for WebSocket connections, messages, and state.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// WebSocket connection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    /// The WebSocket URL (ws:// or wss://).
    pub url: String,
    /// Additional headers to send with the handshake.
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    /// Ping interval in seconds (0 to disable).
    #[serde(default)]
    pub ping_interval_secs: u64,
    /// Connection timeout in seconds.
    #[serde(default = "default_timeout")]
    pub connect_timeout_secs: u64,
    /// Enable automatic reconnection.
    #[serde(default)]
    pub auto_reconnect: bool,
    /// Maximum reconnection attempts (0 for unlimited).
    #[serde(default = "default_max_reconnects")]
    pub max_reconnect_attempts: u32,
    /// Subprotocols to request.
    #[serde(default)]
    pub subprotocols: Vec<String>,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            headers: Vec::new(),
            ping_interval_secs: 0,
            connect_timeout_secs: default_timeout(),
            auto_reconnect: false,
            max_reconnect_attempts: default_max_reconnects(),
            subprotocols: Vec::new(),
        }
    }
}

const fn default_timeout() -> u64 {
    30
}

const fn default_max_reconnects() -> u32 {
    5
}

impl WebSocketConfig {
    /// Create a new WebSocket configuration.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            ..Default::default()
        }
    }

    /// Set additional headers.
    #[must_use]
    pub fn with_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.headers = headers;
        self
    }

    /// Set ping interval.
    #[must_use]
    pub const fn with_ping_interval(mut self, secs: u64) -> Self {
        self.ping_interval_secs = secs;
        self
    }

    /// Enable auto-reconnect.
    #[must_use]
    pub const fn with_auto_reconnect(mut self, enabled: bool) -> Self {
        self.auto_reconnect = enabled;
        self
    }

    /// Get the connect timeout as Duration.
    #[must_use]
    pub const fn connect_timeout(&self) -> Duration {
        Duration::from_secs(self.connect_timeout_secs)
    }

    /// Get the ping interval as Duration, if enabled.
    #[must_use]
    pub const fn ping_interval(&self) -> Option<Duration> {
        if self.ping_interval_secs > 0 {
            Some(Duration::from_secs(self.ping_interval_secs))
        } else {
            None
        }
    }

    /// Validate the configuration.
    #[allow(clippy::missing_errors_doc)]
    pub fn validate(&self) -> Result<(), WebSocketError> {
        if self.url.is_empty() {
            return Err(WebSocketError::InvalidUrl(
                "URL cannot be empty".to_string(),
            ));
        }

        if !self.url.starts_with("ws://") && !self.url.starts_with("wss://") {
            return Err(WebSocketError::InvalidUrl(
                "URL must start with ws:// or wss://".to_string(),
            ));
        }

        Ok(())
    }
}

/// WebSocket connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    /// Not connected.
    #[default]
    Disconnected,
    /// Connection in progress.
    Connecting,
    /// Connected and ready.
    Connected,
    /// Reconnecting after disconnect.
    Reconnecting,
    /// Connection failed.
    Failed,
    /// Closing connection.
    Closing,
}

impl ConnectionState {
    /// Check if the connection is active.
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }

    /// Check if connection is in progress.
    #[must_use]
    pub const fn is_connecting(&self) -> bool {
        matches!(self, Self::Connecting | Self::Reconnecting)
    }

    /// Get a human-readable status string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Disconnected => "Disconnected",
            Self::Connecting => "Connecting...",
            Self::Connected => "Connected",
            Self::Reconnecting => "Reconnecting...",
            Self::Failed => "Connection Failed",
            Self::Closing => "Closing...",
        }
    }
}

/// A WebSocket message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage {
    /// Unique message ID.
    pub id: Uuid,
    /// Message direction.
    pub direction: MessageDirection,
    /// Message type.
    pub message_type: MessageType,
    /// Message content.
    pub content: String,
    /// Binary content (if binary message).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub binary: Vec<u8>,
    /// Timestamp when the message was sent/received.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Size in bytes.
    pub size: usize,
}

impl WebSocketMessage {
    /// Create a new outgoing text message.
    #[must_use]
    pub fn text(content: impl Into<String>) -> Self {
        let content = content.into();
        let size = content.len();
        Self {
            id: Uuid::now_v7(),
            direction: MessageDirection::Sent,
            message_type: MessageType::Text,
            content,
            binary: Vec::new(),
            timestamp: chrono::Utc::now(),
            size,
        }
    }

    /// Create a new outgoing binary message.
    #[must_use]
    pub fn binary(data: Vec<u8>) -> Self {
        let size = data.len();
        Self {
            id: Uuid::now_v7(),
            direction: MessageDirection::Sent,
            message_type: MessageType::Binary,
            content: String::new(),
            binary: data,
            timestamp: chrono::Utc::now(),
            size,
        }
    }

    /// Create a received text message.
    #[must_use]
    pub fn received_text(content: impl Into<String>) -> Self {
        let content = content.into();
        let size = content.len();
        Self {
            id: Uuid::now_v7(),
            direction: MessageDirection::Received,
            message_type: MessageType::Text,
            content,
            binary: Vec::new(),
            timestamp: chrono::Utc::now(),
            size,
        }
    }

    /// Create a received binary message.
    #[must_use]
    pub fn received_binary(data: Vec<u8>) -> Self {
        let size = data.len();
        Self {
            id: Uuid::now_v7(),
            direction: MessageDirection::Received,
            message_type: MessageType::Binary,
            content: String::new(),
            binary: data,
            timestamp: chrono::Utc::now(),
            size,
        }
    }

    /// Create a system/control message.
    #[must_use]
    pub fn system(content: impl Into<String>, msg_type: MessageType) -> Self {
        let content = content.into();
        let size = content.len();
        Self {
            id: Uuid::now_v7(),
            direction: MessageDirection::System,
            message_type: msg_type,
            content,
            binary: Vec::new(),
            timestamp: chrono::Utc::now(),
            size,
        }
    }

    /// Format the size for display.
    #[must_use]
    pub fn size_display(&self) -> String {
        #[allow(clippy::cast_precision_loss)]
        if self.size >= 1024 * 1024 {
            format!("{:.2} MB", self.size as f64 / (1024.0 * 1024.0))
        } else if self.size >= 1024 {
            format!("{:.2} KB", self.size as f64 / 1024.0)
        } else {
            format!("{} B", self.size)
        }
    }
}

/// Direction of a WebSocket message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageDirection {
    /// Message sent by the client.
    Sent,
    /// Message received from the server.
    Received,
    /// System/control message.
    System,
}

/// Type of WebSocket message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    /// Text message.
    Text,
    /// Binary message.
    Binary,
    /// Ping message.
    Ping,
    /// Pong message.
    Pong,
    /// Close message.
    Close,
}

/// WebSocket-related errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum WebSocketError {
    /// Invalid URL.
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    /// Connection failed.
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    /// Connection closed.
    #[error("Connection closed: {0}")]
    ConnectionClosed(String),
    /// Send failed.
    #[error("Failed to send message: {0}")]
    SendFailed(String),
    /// Receive failed.
    #[error("Failed to receive message: {0}")]
    ReceiveFailed(String),
    /// Timeout.
    #[error("Connection timeout")]
    Timeout,
    /// Not connected.
    #[error("Not connected")]
    NotConnected,
}

/// WebSocket connection info.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectionInfo {
    /// The connection ID.
    pub id: Uuid,
    /// Connection state.
    pub state: ConnectionState,
    /// Connected URL.
    pub url: String,
    /// Connected at timestamp.
    pub connected_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Number of messages sent.
    pub messages_sent: u64,
    /// Number of messages received.
    pub messages_received: u64,
    /// Total bytes sent.
    pub bytes_sent: u64,
    /// Total bytes received.
    pub bytes_received: u64,
    /// Reconnection attempt count.
    pub reconnect_attempts: u32,
    /// Negotiated subprotocol.
    pub subprotocol: Option<String>,
}

impl ConnectionInfo {
    /// Create new connection info.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            url: url.into(),
            ..Default::default()
        }
    }

    /// Mark as connected.
    pub fn connected(&mut self, subprotocol: Option<String>) {
        self.state = ConnectionState::Connected;
        self.connected_at = Some(chrono::Utc::now());
        self.subprotocol = subprotocol;
        self.reconnect_attempts = 0;
    }

    /// Mark as disconnected.
    pub const fn disconnected(&mut self) {
        self.state = ConnectionState::Disconnected;
    }

    /// Record a sent message.
    pub const fn record_sent(&mut self, size: usize) {
        self.messages_sent = self.messages_sent.saturating_add(1);
        self.bytes_sent = self.bytes_sent.saturating_add(size as u64);
    }

    /// Record a received message.
    pub const fn record_received(&mut self, size: usize) {
        self.messages_received = self.messages_received.saturating_add(1);
        self.bytes_received = self.bytes_received.saturating_add(size as u64);
    }

    /// Get the connection duration.
    #[must_use]
    pub fn duration(&self) -> Option<chrono::Duration> {
        self.connected_at.map(|t| chrono::Utc::now() - t)
    }

    /// Format duration for display.
    #[must_use]
    pub fn duration_display(&self) -> String {
        self.duration().map_or_else(|| "-".to_string(), |d| {
                let secs = d.num_seconds();
                if secs >= 3600 {
                    format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
                } else if secs >= 60 {
                    format!("{}m {}s", secs / 60, secs % 60)
                } else {
                    format!("{secs}s")
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_config_new() {
        let config = WebSocketConfig::new("wss://example.com/ws");
        assert_eq!(config.url, "wss://example.com/ws");
        assert_eq!(config.connect_timeout_secs, 30);
    }

    #[test]
    fn test_websocket_config_validate() {
        let config = WebSocketConfig::new("wss://example.com/ws");
        assert!(config.validate().is_ok());

        let config = WebSocketConfig::new("");
        assert!(config.validate().is_err());

        let config = WebSocketConfig::new("http://example.com");
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_connection_state() {
        assert!(!ConnectionState::Disconnected.is_connected());
        assert!(ConnectionState::Connected.is_connected());
        assert!(ConnectionState::Connecting.is_connecting());
        assert!(ConnectionState::Reconnecting.is_connecting());
    }

    #[test]
    fn test_websocket_message_text() {
        let msg = WebSocketMessage::text("Hello");
        assert_eq!(msg.content, "Hello");
        assert_eq!(msg.direction, MessageDirection::Sent);
        assert_eq!(msg.message_type, MessageType::Text);
        assert_eq!(msg.size, 5);
    }

    #[test]
    fn test_websocket_message_binary() {
        let data = vec![1, 2, 3, 4, 5];
        let msg = WebSocketMessage::binary(data.clone());
        assert_eq!(msg.binary, data);
        assert_eq!(msg.direction, MessageDirection::Sent);
        assert_eq!(msg.message_type, MessageType::Binary);
        assert_eq!(msg.size, 5);
    }

    #[test]
    fn test_message_size_display() {
        let msg = WebSocketMessage::text("Hello");
        assert_eq!(msg.size_display(), "5 B");

        let mut msg = WebSocketMessage::text("");
        msg.size = 1536;
        assert_eq!(msg.size_display(), "1.50 KB");
    }

    #[test]
    fn test_connection_info() {
        let mut info = ConnectionInfo::new("wss://example.com");
        assert_eq!(info.state, ConnectionState::Disconnected);

        info.connected(Some("graphql-ws".to_string()));
        assert_eq!(info.state, ConnectionState::Connected);
        assert_eq!(info.subprotocol, Some("graphql-ws".to_string()));

        info.record_sent(100);
        assert_eq!(info.messages_sent, 1);
        assert_eq!(info.bytes_sent, 100);

        info.record_received(200);
        assert_eq!(info.messages_received, 1);
        assert_eq!(info.bytes_received, 200);
    }
}
