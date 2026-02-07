//! HTTP proxy configuration.
//!
//! This module provides types for configuring HTTP proxies.

use serde::{Deserialize, Serialize};

/// Proxy configuration for HTTP requests.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ProxyConfig {
    /// Whether to use a proxy.
    #[serde(default)]
    pub enabled: bool,
    /// Proxy server URL (e.g., "<http://proxy.example.com:8080>").
    #[serde(default)]
    pub url: String,
    /// Proxy authentication username.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Proxy authentication password.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Hosts to bypass the proxy (comma-separated).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bypass_hosts: Vec<String>,
    /// Use system proxy settings.
    #[serde(default)]
    pub use_system_proxy: bool,
    /// Proxy type.
    #[serde(default)]
    pub proxy_type: ProxyType,
}

impl ProxyConfig {
    /// Create a new empty proxy configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a proxy configuration with URL.
    #[must_use]
    pub fn with_url(url: impl Into<String>) -> Self {
        Self {
            enabled: true,
            url: url.into(),
            ..Default::default()
        }
    }

    /// Set authentication credentials.
    #[must_use]
    pub fn with_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }

    /// Add bypass hosts.
    #[must_use]
    pub fn with_bypass(mut self, hosts: Vec<String>) -> Self {
        self.bypass_hosts = hosts;
        self
    }

    /// Check if the proxy is effectively enabled.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.enabled && (!self.url.is_empty() || self.use_system_proxy)
    }

    /// Check if the proxy has authentication.
    #[must_use]
    pub const fn has_auth(&self) -> bool {
        self.username.is_some() && self.password.is_some()
    }

    /// Get the proxy URL with authentication if present.
    #[must_use]
    pub fn url_with_auth(&self) -> Option<String> {
        if !self.is_active() || self.url.is_empty() {
            return None;
        }

        if let (Some(user), Some(pass)) = (&self.username, &self.password) {
            // Parse URL and insert credentials
            if let Some((scheme, rest)) = self.url.split_once("://") {
                return Some(format!("{scheme}://{user}:{pass}@{rest}"));
            }
        }

        Some(self.url.clone())
    }

    /// Check if a host should bypass the proxy.
    #[must_use]
    pub fn should_bypass(&self, host: &str) -> bool {
        let host_lower = host.to_lowercase();
        self.bypass_hosts.iter().any(|bypass| {
            let bypass_lower = bypass.to_lowercase().trim().to_string();
            bypass_lower.strip_prefix('*').map_or_else(|| host_lower == bypass_lower || host_lower.ends_with(&format!(".{bypass_lower}")), |suffix| host_lower.ends_with(suffix))
        })
    }

    /// Validate the proxy configuration.
    #[allow(clippy::missing_errors_doc)]
    pub fn validate(&self) -> Result<(), ProxyError> {
        if !self.enabled {
            return Ok(());
        }

        if !self.use_system_proxy && self.url.is_empty() {
            return Err(ProxyError::MissingUrl);
        }

        if !self.url.is_empty()
            && !self.url.starts_with("http://")
                && !self.url.starts_with("https://")
                && !self.url.starts_with("socks4://")
                && !self.url.starts_with("socks5://")
            {
                return Err(ProxyError::InvalidUrl(
                    "URL must start with http://, https://, socks4://, or socks5://".to_string(),
                ));
            }

        // Check for incomplete auth
        if self.username.is_some() != self.password.is_some() {
            return Err(ProxyError::IncompleteAuth);
        }

        Ok(())
    }
}

/// Type of proxy server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProxyType {
    /// HTTP proxy.
    #[default]
    Http,
    /// HTTPS proxy (HTTP CONNECT).
    Https,
    /// SOCKS4 proxy.
    Socks4,
    /// SOCKS5 proxy.
    Socks5,
}

impl ProxyType {
    /// Get the default port for this proxy type.
    #[must_use]
    pub const fn default_port(&self) -> u16 {
        match self {
            Self::Http | Self::Https => 8080,
            Self::Socks4 | Self::Socks5 => 1080,
        }
    }

    /// Get the scheme for this proxy type.
    #[must_use]
    pub const fn scheme(&self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
            Self::Socks4 => "socks4",
            Self::Socks5 => "socks5",
        }
    }

    /// Get human-readable name.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Http => "HTTP",
            Self::Https => "HTTPS",
            Self::Socks4 => "SOCKS4",
            Self::Socks5 => "SOCKS5",
        }
    }
}

/// Proxy-related errors.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ProxyError {
    /// Missing proxy URL.
    #[error("Proxy URL is required when proxy is enabled")]
    MissingUrl,
    /// Invalid proxy URL.
    #[error("Invalid proxy URL: {0}")]
    InvalidUrl(String),
    /// Incomplete authentication.
    #[error("Both username and password are required for proxy authentication")]
    IncompleteAuth,
    /// Connection failed.
    #[error("Failed to connect to proxy: {0}")]
    ConnectionFailed(String),
    /// Authentication failed.
    #[error("Proxy authentication failed")]
    AuthenticationFailed,
}

/// Global proxy settings for the application.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlobalProxySettings {
    /// HTTP proxy configuration.
    #[serde(default)]
    pub http_proxy: ProxyConfig,
    /// HTTPS proxy configuration.
    #[serde(default)]
    pub https_proxy: ProxyConfig,
    /// Whether to use the same proxy for HTTP and HTTPS.
    #[serde(default = "default_true")]
    pub use_same_proxy: bool,
    /// Global bypass hosts (applies to all proxies).
    #[serde(default)]
    pub global_bypass: Vec<String>,
}

const fn default_true() -> bool {
    true
}

impl Default for GlobalProxySettings {
    fn default() -> Self {
        Self {
            http_proxy: ProxyConfig::default(),
            https_proxy: ProxyConfig::default(),
            use_same_proxy: true,
            global_bypass: Vec::new(),
        }
    }
}

impl GlobalProxySettings {
    /// Create new global proxy settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the effective proxy for a URL scheme.
    #[must_use]
    pub const fn get_proxy(&self, is_https: bool) -> Option<&ProxyConfig> {
        if self.use_same_proxy {
            if self.http_proxy.is_active() {
                return Some(&self.http_proxy);
            }
        } else if is_https && self.https_proxy.is_active() {
            return Some(&self.https_proxy);
        } else if !is_https && self.http_proxy.is_active() {
            return Some(&self.http_proxy);
        }
        None
    }

    /// Check if a host should bypass all proxies.
    #[must_use]
    pub fn should_bypass(&self, host: &str) -> bool {
        let host_lower = host.to_lowercase();
        self.global_bypass.iter().any(|bypass| {
            let bypass_lower = bypass.to_lowercase().trim().to_string();
            host_lower == bypass_lower || host_lower.ends_with(&format!(".{bypass_lower}"))
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_config_new() {
        let config = ProxyConfig::new();
        assert!(!config.enabled);
        assert!(config.url.is_empty());
    }

    #[test]
    fn test_proxy_config_with_url() {
        let config = ProxyConfig::with_url("http://proxy.example.com:8080");
        assert!(config.enabled);
        assert_eq!(config.url, "http://proxy.example.com:8080");
    }

    #[test]
    fn test_proxy_config_with_auth() {
        let config =
            ProxyConfig::with_url("http://proxy.example.com:8080").with_auth("user", "pass");
        assert!(config.has_auth());
        assert_eq!(config.username, Some("user".to_string()));
        assert_eq!(config.password, Some("pass".to_string()));
    }

    #[test]
    fn test_proxy_url_with_auth() {
        let config =
            ProxyConfig::with_url("http://proxy.example.com:8080").with_auth("user", "pass");
        let url = config.url_with_auth().unwrap();
        assert_eq!(url, "http://user:pass@proxy.example.com:8080");
    }

    #[test]
    fn test_proxy_bypass() {
        let config = ProxyConfig::with_url("http://proxy.example.com")
            .with_bypass(vec!["localhost".to_string(), "*.internal.com".to_string()]);

        assert!(config.should_bypass("localhost"));
        assert!(config.should_bypass("api.internal.com"));
        assert!(!config.should_bypass("example.com"));
    }

    #[test]
    fn test_proxy_validate() {
        // Disabled proxy is always valid
        let config = ProxyConfig::new();
        assert!(config.validate().is_ok());

        // Enabled without URL is invalid
        let mut config = ProxyConfig::new();
        config.enabled = true;
        assert!(matches!(config.validate(), Err(ProxyError::MissingUrl)));

        // Enabled with URL is valid
        let config = ProxyConfig::with_url("http://proxy.example.com");
        assert!(config.validate().is_ok());

        // Invalid URL scheme
        let config = ProxyConfig::with_url("ftp://proxy.example.com");
        assert!(matches!(config.validate(), Err(ProxyError::InvalidUrl(_))));

        // Incomplete auth
        let mut config = ProxyConfig::with_url("http://proxy.example.com");
        config.username = Some("user".to_string());
        assert!(matches!(config.validate(), Err(ProxyError::IncompleteAuth)));
    }

    #[test]
    fn test_proxy_type() {
        assert_eq!(ProxyType::Http.default_port(), 8080);
        assert_eq!(ProxyType::Socks5.default_port(), 1080);
        assert_eq!(ProxyType::Http.scheme(), "http");
        assert_eq!(ProxyType::Socks5.scheme(), "socks5");
    }

    #[test]
    fn test_global_proxy_settings() {
        let mut settings = GlobalProxySettings::new();
        settings.http_proxy = ProxyConfig::with_url("http://proxy.example.com");

        // With use_same_proxy = true (default), both HTTP and HTTPS use the same proxy
        assert!(settings.get_proxy(false).is_some());
        assert!(settings.get_proxy(true).is_some());

        // With use_same_proxy = false
        settings.use_same_proxy = false;
        assert!(settings.get_proxy(false).is_some());
        assert!(settings.get_proxy(true).is_none()); // HTTPS proxy not configured
    }

    #[test]
    fn test_global_bypass() {
        let mut settings = GlobalProxySettings::new();
        settings.global_bypass = vec!["localhost".to_string(), "example.com".to_string()];

        assert!(settings.should_bypass("localhost"));
        assert!(settings.should_bypass("api.example.com"));
        assert!(!settings.should_bypass("other.com"));
    }
}
