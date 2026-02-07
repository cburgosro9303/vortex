//! Cookie management types.
//!
//! This module provides types for managing HTTP cookies.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single HTTP cookie.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cookie {
    /// Cookie name.
    pub name: String,
    /// Cookie value.
    pub value: String,
    /// Domain the cookie belongs to.
    pub domain: String,
    /// Path the cookie applies to.
    #[serde(default = "default_path")]
    pub path: String,
    /// Expiration time (None for session cookies).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<DateTime<Utc>>,
    /// HttpOnly flag.
    #[serde(default)]
    pub http_only: bool,
    /// Secure flag.
    #[serde(default)]
    pub secure: bool,
    /// SameSite attribute.
    #[serde(default)]
    pub same_site: SameSite,
    /// When the cookie was created/received.
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
}

fn default_path() -> String {
    "/".to_string()
}

impl Cookie {
    /// Create a new cookie.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        value: impl Into<String>,
        domain: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            domain: domain.into(),
            path: default_path(),
            expires: None,
            http_only: false,
            secure: false,
            same_site: SameSite::default(),
            created_at: Utc::now(),
        }
    }

    /// Set the path.
    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    /// Set the expiration.
    #[must_use]
    pub fn with_expires(mut self, expires: DateTime<Utc>) -> Self {
        self.expires = Some(expires);
        self
    }

    /// Set HttpOnly flag.
    #[must_use]
    pub const fn with_http_only(mut self, http_only: bool) -> Self {
        self.http_only = http_only;
        self
    }

    /// Set Secure flag.
    #[must_use]
    pub const fn with_secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self
    }

    /// Set SameSite attribute.
    #[must_use]
    pub const fn with_same_site(mut self, same_site: SameSite) -> Self {
        self.same_site = same_site;
        self
    }

    /// Check if the cookie is expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expires.is_some_and(|exp| exp < Utc::now())
    }

    /// Check if this is a session cookie (no expiration).
    #[must_use]
    pub fn is_session(&self) -> bool {
        self.expires.is_none()
    }

    /// Check if the cookie applies to a given URL.
    #[must_use]
    pub fn applies_to(&self, url: &str, is_secure: bool) -> bool {
        // Check secure flag
        if self.secure && !is_secure {
            return false;
        }

        // Check domain (simplified matching)
        if let Some(host) = extract_host(url) {
            if !domain_matches(&self.domain, &host) {
                return false;
            }
        }

        // Check path
        if let Some(path) = extract_path(url) {
            if !path.starts_with(&self.path) {
                return false;
            }
        }

        true
    }

    /// Format for Cookie header.
    #[must_use]
    pub fn to_cookie_header(&self) -> String {
        format!("{}={}", self.name, self.value)
    }

    /// Parse from Set-Cookie header.
    pub fn from_set_cookie(header: &str, request_domain: &str) -> Option<Self> {
        let parts: Vec<&str> = header.split(';').collect();
        if parts.is_empty() {
            return None;
        }

        // First part is name=value
        let (name, value) = parts[0].split_once('=')?;
        let mut cookie = Cookie::new(name.trim(), value.trim(), request_domain);

        // Parse attributes
        for part in parts.iter().skip(1) {
            let part = part.trim();
            if let Some((attr, val)) = part.split_once('=') {
                let attr = attr.trim().to_lowercase();
                let val = val.trim();
                match attr.as_str() {
                    "domain" => cookie.domain = val.trim_start_matches('.').to_string(),
                    "path" => cookie.path = val.to_string(),
                    "expires" => {
                        if let Ok(exp) = DateTime::parse_from_rfc2822(val) {
                            cookie.expires = Some(exp.with_timezone(&Utc));
                        }
                    }
                    "max-age" => {
                        if let Ok(secs) = val.parse::<i64>() {
                            cookie.expires = Some(Utc::now() + chrono::Duration::seconds(secs));
                        }
                    }
                    "samesite" => {
                        cookie.same_site = match val.to_lowercase().as_str() {
                            "strict" => SameSite::Strict,
                            "lax" => SameSite::Lax,
                            "none" => SameSite::None,
                            _ => SameSite::default(),
                        };
                    }
                    _ => {}
                }
            } else {
                let attr = part.to_lowercase();
                match attr.as_str() {
                    "httponly" => cookie.http_only = true,
                    "secure" => cookie.secure = true,
                    _ => {}
                }
            }
        }

        Some(cookie)
    }
}

/// SameSite attribute values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SameSite {
    /// Cookies are sent with all requests.
    #[default]
    None,
    /// Cookies are sent with top-level navigations and GET from third-party sites.
    Lax,
    /// Cookies are only sent in first-party context.
    Strict,
}

impl SameSite {
    /// Get human-readable name.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Lax => "Lax",
            Self::Strict => "Strict",
        }
    }
}

/// Cookie jar for storing cookies.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CookieJar {
    /// Stored cookies by domain.
    #[serde(default)]
    cookies: HashMap<String, Vec<Cookie>>,
    /// Whether the jar is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl CookieJar {
    /// Create a new empty cookie jar.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cookies: HashMap::new(),
            enabled: true,
        }
    }

    /// Add a cookie to the jar.
    pub fn add(&mut self, cookie: Cookie) {
        if cookie.is_expired() {
            return;
        }

        let domain = cookie.domain.clone();
        let cookies = self.cookies.entry(domain).or_default();

        // Remove existing cookie with same name and path
        cookies.retain(|c| c.name != cookie.name || c.path != cookie.path);

        cookies.push(cookie);
    }

    /// Remove a cookie by name and domain.
    pub fn remove(&mut self, name: &str, domain: &str) {
        if let Some(cookies) = self.cookies.get_mut(domain) {
            cookies.retain(|c| c.name != name);
        }
    }

    /// Get all cookies for a URL.
    #[must_use]
    pub fn get_for_url(&self, url: &str) -> Vec<&Cookie> {
        let is_secure = url.starts_with("https://");
        let host = extract_host(url).unwrap_or_default();

        self.cookies
            .iter()
            .flat_map(|(_, cookies)| cookies.iter())
            .filter(|c| !c.is_expired() && c.applies_to(url, is_secure))
            .filter(|c| domain_matches(&c.domain, &host))
            .collect()
    }

    /// Get all cookies.
    #[must_use]
    pub fn all(&self) -> Vec<&Cookie> {
        self.cookies.values().flatten().collect()
    }

    /// Get all non-expired cookies.
    #[must_use]
    pub fn all_valid(&self) -> Vec<&Cookie> {
        self.cookies
            .values()
            .flatten()
            .filter(|c| !c.is_expired())
            .collect()
    }

    /// Clear all cookies.
    pub fn clear(&mut self) {
        self.cookies.clear();
    }

    /// Clear cookies for a specific domain.
    pub fn clear_domain(&mut self, domain: &str) {
        self.cookies.remove(domain);
    }

    /// Remove expired cookies.
    pub fn cleanup_expired(&mut self) {
        for cookies in self.cookies.values_mut() {
            cookies.retain(|c| !c.is_expired());
        }
        self.cookies.retain(|_, cookies| !cookies.is_empty());
    }

    /// Get the total number of cookies.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cookies.values().map(Vec::len).sum()
    }

    /// Check if the jar is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }

    /// Build the Cookie header value for a URL.
    #[must_use]
    pub fn cookie_header(&self, url: &str) -> Option<String> {
        if !self.enabled {
            return None;
        }

        let cookies = self.get_for_url(url);
        if cookies.is_empty() {
            return None;
        }

        let header_value: Vec<String> = cookies.iter().map(|c| c.to_cookie_header()).collect();
        Some(header_value.join("; "))
    }

    /// Process Set-Cookie headers from a response.
    pub fn process_set_cookies(&mut self, headers: &[(String, String)], request_domain: &str) {
        if !self.enabled {
            return;
        }

        for (name, value) in headers {
            if name.eq_ignore_ascii_case("set-cookie") {
                if let Some(cookie) = Cookie::from_set_cookie(value, request_domain) {
                    self.add(cookie);
                }
            }
        }
    }
}

/// Extract host from URL.
fn extract_host(url: &str) -> Option<String> {
    let url = url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let host = url.split('/').next()?;
    let host = host.split(':').next()?; // Remove port
    Some(host.to_lowercase())
}

/// Extract path from URL.
fn extract_path(url: &str) -> Option<String> {
    let url = url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let path = url.find('/').map(|i| &url[i..]).unwrap_or("/");
    let path = path.split('?').next()?; // Remove query
    Some(path.to_string())
}

/// Check if a cookie domain matches a request host.
fn domain_matches(cookie_domain: &str, request_host: &str) -> bool {
    let cookie_domain = cookie_domain.to_lowercase();
    let request_host = request_host.to_lowercase();

    if cookie_domain == request_host {
        return true;
    }

    // Domain matching: cookie domain can be a suffix of request host
    if request_host.ends_with(&format!(".{}", cookie_domain)) {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cookie_new() {
        let cookie = Cookie::new("session", "abc123", "example.com");
        assert_eq!(cookie.name, "session");
        assert_eq!(cookie.value, "abc123");
        assert_eq!(cookie.domain, "example.com");
        assert_eq!(cookie.path, "/");
    }

    #[test]
    fn test_cookie_expired() {
        let cookie = Cookie::new("test", "value", "example.com")
            .with_expires(Utc::now() - chrono::Duration::hours(1));
        assert!(cookie.is_expired());

        let cookie = Cookie::new("test", "value", "example.com")
            .with_expires(Utc::now() + chrono::Duration::hours(1));
        assert!(!cookie.is_expired());
    }

    #[test]
    fn test_cookie_session() {
        let cookie = Cookie::new("test", "value", "example.com");
        assert!(cookie.is_session());

        let cookie = cookie.with_expires(Utc::now() + chrono::Duration::hours(1));
        assert!(!cookie.is_session());
    }

    #[test]
    fn test_cookie_applies_to() {
        let cookie = Cookie::new("test", "value", "example.com").with_path("/api");

        assert!(cookie.applies_to("https://example.com/api/users", true));
        assert!(cookie.applies_to("http://example.com/api/users", false));
        assert!(!cookie.applies_to("https://example.com/other", true));
    }

    #[test]
    fn test_cookie_secure() {
        let cookie = Cookie::new("test", "value", "example.com").with_secure(true);

        assert!(cookie.applies_to("https://example.com/", true));
        assert!(!cookie.applies_to("http://example.com/", false));
    }

    #[test]
    fn test_cookie_header() {
        let cookie = Cookie::new("session", "abc123", "example.com");
        assert_eq!(cookie.to_cookie_header(), "session=abc123");
    }

    #[test]
    fn test_cookie_from_set_cookie() {
        let header = "session=abc123; Path=/; HttpOnly; Secure; SameSite=Strict";
        let cookie = Cookie::from_set_cookie(header, "example.com").unwrap();

        assert_eq!(cookie.name, "session");
        assert_eq!(cookie.value, "abc123");
        assert_eq!(cookie.path, "/");
        assert!(cookie.http_only);
        assert!(cookie.secure);
        assert_eq!(cookie.same_site, SameSite::Strict);
    }

    #[test]
    fn test_cookie_jar_add_get() {
        let mut jar = CookieJar::new();

        jar.add(Cookie::new("session", "abc", "example.com"));
        jar.add(Cookie::new("token", "xyz", "api.example.com"));

        // Only session cookie applies to root domain
        let cookies = jar.get_for_url("https://example.com/");
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].name, "session");

        // Both cookies apply to subdomain (parent domain cookies inherit)
        let cookies = jar.get_for_url("https://api.example.com/");
        assert_eq!(cookies.len(), 2);
        assert!(cookies.iter().any(|c| c.name == "session"));
        assert!(cookies.iter().any(|c| c.name == "token"));
    }

    #[test]
    fn test_cookie_jar_header() {
        let mut jar = CookieJar::new();

        jar.add(Cookie::new("a", "1", "example.com"));
        jar.add(Cookie::new("b", "2", "example.com"));

        let header = jar.cookie_header("https://example.com/").unwrap();
        assert!(header.contains("a=1"));
        assert!(header.contains("b=2"));
    }

    #[test]
    fn test_cookie_jar_remove() {
        let mut jar = CookieJar::new();

        jar.add(Cookie::new("session", "abc", "example.com"));
        assert_eq!(jar.len(), 1);

        jar.remove("session", "example.com");
        assert_eq!(jar.len(), 0);
    }

    #[test]
    fn test_cookie_jar_clear() {
        let mut jar = CookieJar::new();

        jar.add(Cookie::new("a", "1", "example.com"));
        jar.add(Cookie::new("b", "2", "other.com"));

        jar.clear_domain("example.com");
        assert_eq!(jar.len(), 1);

        jar.clear();
        assert!(jar.is_empty());
    }

    #[test]
    fn test_domain_matching() {
        assert!(domain_matches("example.com", "example.com"));
        assert!(domain_matches("example.com", "api.example.com"));
        assert!(domain_matches("example.com", "sub.api.example.com"));
        assert!(!domain_matches("example.com", "otherexample.com"));
    }
}
