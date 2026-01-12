//! Cache key generation and normalization.

use std::fmt;

/// Key unica para cache de configuraciones.
/// Normaliza app/profile/label a lowercase para consistencia.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    app: String,
    profile: String,
    label: String,
}

impl CacheKey {
    /// Crea una nueva cache key normalizando los valores a lowercase.
    ///
    /// # Examples
    ///
    /// ```
    /// use vortex_server::cache::CacheKey;
    ///
    /// let key = CacheKey::new("MyApp", "PROD", "Main");
    /// assert_eq!(key.app(), "myapp");
    /// assert_eq!(key.profile(), "prod");
    /// assert_eq!(key.label(), "main");
    /// ```
    pub fn new(app: impl Into<String>, profile: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            app: app.into().to_lowercase(),
            profile: profile.into().to_lowercase(),
            label: label.into().to_lowercase(),
        }
    }

    /// Retorna el nombre de la aplicación.
    pub fn app(&self) -> &str {
        &self.app
    }

    /// Retorna el perfil.
    pub fn profile(&self) -> &str {
        &self.profile
    }

    /// Retorna el label (branch/tag).
    pub fn label(&self) -> &str {
        &self.label
    }
}

impl fmt::Display for CacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.app, self.profile, self.label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_normalization() {
        let key1 = CacheKey::new("MyApp", "PROD", "Main");
        let key2 = CacheKey::new("myapp", "prod", "main");

        assert_eq!(key1, key2);
        assert_eq!(key1.to_string(), "myapp:prod:main");
    }

    #[test]
    fn test_cache_key_accessors() {
        let key = CacheKey::new("myapp", "production", "main");

        assert_eq!(key.app(), "myapp");
        assert_eq!(key.profile(), "production");
        assert_eq!(key.label(), "main");
    }

    #[test]
    fn test_cache_key_hash() {
        use std::collections::HashSet;

        let key1 = CacheKey::new("App", "PROD", "main");
        let key2 = CacheKey::new("app", "prod", "MAIN");

        let mut set = HashSet::new();
        set.insert(key1);

        // key2 debe ser considerada igual a key1
        assert!(set.contains(&key2));
    }
}
