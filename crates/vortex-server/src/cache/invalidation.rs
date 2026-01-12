//! Cache invalidation with pattern matching support.

use crate::cache::{CacheKey, ConfigCache};
use glob::Pattern;
use tracing::{debug, info};

/// Resultado de una operación de invalidación.
#[derive(Debug, Clone)]
pub struct InvalidationResult {
    /// Número de entries invalidadas.
    pub count: usize,
    /// Patrones aplicados.
    pub patterns: Vec<String>,
}

impl ConfigCache {
    /// Invalida todas las entradas que coincidan con el app dado.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use vortex_server::cache::{ConfigCache, CacheConfig, CacheKey};
    /// # #[tokio::main]
    /// # async fn main() {
    /// # let cache = ConfigCache::new(CacheConfig::default());
    /// let result = cache.invalidate_by_app("myapp").await;
    /// println!("Invalidated {} entries", result.count);
    /// # }
    /// ```
    pub async fn invalidate_by_app(&self, app: &str) -> InvalidationResult {
        let pattern_str = format!("{}:*:*", app.to_lowercase());
        self.invalidate_by_pattern(&pattern_str).await
    }

    /// Invalida todas las entradas que coincidan con app y profile.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use vortex_server::cache::{ConfigCache, CacheConfig, CacheKey};
    /// # #[tokio::main]
    /// # async fn main() {
    /// # let cache = ConfigCache::new(CacheConfig::default());
    /// let result = cache.invalidate_by_app_profile("myapp", "prod").await;
    /// println!("Invalidated {} entries", result.count);
    /// # }
    /// ```
    pub async fn invalidate_by_app_profile(&self, app: &str, profile: &str) -> InvalidationResult {
        let pattern_str = format!("{}:{}:*", app.to_lowercase(), profile.to_lowercase());
        self.invalidate_by_pattern(&pattern_str).await
    }

    /// Invalida una entrada específica por app, profile y label.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use vortex_server::cache::{ConfigCache, CacheConfig, CacheKey};
    /// # #[tokio::main]
    /// # async fn main() {
    /// # let cache = ConfigCache::new(CacheConfig::default());
    /// let result = cache.invalidate_by_app_profile_label("myapp", "prod", "main").await;
    /// println!("Invalidated {} entries", result.count);
    /// # }
    /// ```
    pub async fn invalidate_by_app_profile_label(
        &self,
        app: &str,
        profile: &str,
        label: &str,
    ) -> InvalidationResult {
        let key = CacheKey::new(app, profile, label);
        self.invalidate(&key).await;

        info!(
            app = %app,
            profile = %profile,
            label = %label,
            "Cache entry invalidated"
        );

        InvalidationResult {
            count: 1,
            patterns: vec![key.to_string()],
        }
    }

    /// Invalida entradas usando un patrón glob.
    ///
    /// El patrón debe seguir el formato: `app:profile:label`
    /// donde cada parte puede usar comodines:
    /// - `*`: coincide con cualquier secuencia de caracteres
    /// - `?`: coincide con un carácter
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use vortex_server::cache::{ConfigCache, CacheConfig, CacheKey};
    /// # #[tokio::main]
    /// # async fn main() {
    /// # let cache = ConfigCache::new(CacheConfig::default());
    /// // Invalida todas las configuraciones de producción
    /// let result = cache.invalidate_by_pattern("*:prod:*").await;
    ///
    /// // Invalida configuraciones de apps que empiecen con "my"
    /// let result = cache.invalidate_by_pattern("my*:*:*").await;
    /// # }
    /// ```
    pub async fn invalidate_by_pattern(&self, pattern_str: &str) -> InvalidationResult {
        let pattern = match Pattern::new(pattern_str) {
            Ok(p) => p,
            Err(e) => {
                debug!(pattern = %pattern_str, error = %e, "Invalid glob pattern");
                return InvalidationResult {
                    count: 0,
                    patterns: vec![pattern_str.to_string()],
                };
            }
        };

        let mut invalidated_keys = Vec::new();

        // Iterar sobre todas las entries y recolectar las que coincidan
        for (key, _) in self.iter() {
            let key_str = key.to_string();
            if pattern.matches(&key_str) {
                invalidated_keys.push((*key).clone());
            }
        }

        // Invalidar las keys recolectadas
        let count = invalidated_keys.len();
        for key in invalidated_keys {
            self.invalidate(&key).await;
        }

        info!(
            pattern = %pattern_str,
            count = count,
            "Cache entries invalidated by pattern"
        );

        InvalidationResult {
            count,
            patterns: vec![pattern_str.to_string()],
        }
    }

    /// Invalida múltiples patrones a la vez.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use vortex_server::cache::{ConfigCache, CacheConfig, CacheKey};
    /// # #[tokio::main]
    /// # async fn main() {
    /// # let cache = ConfigCache::new(CacheConfig::default());
    /// let patterns = vec!["myapp:*:*", "other:prod:*"];
    /// let result = cache.invalidate_by_patterns(&patterns).await;
    /// println!("Invalidated {} entries", result.count);
    /// # }
    /// ```
    pub async fn invalidate_by_patterns(&self, patterns: &[&str]) -> InvalidationResult {
        let mut total_count = 0;
        let mut all_patterns = Vec::new();

        for pattern_str in patterns {
            let result = self.invalidate_by_pattern(pattern_str).await;
            total_count += result.count;
            all_patterns.extend(result.patterns);
        }

        InvalidationResult {
            count: total_count,
            patterns: all_patterns,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::{CacheConfig, ConfigCache};
    use crate::handlers::response::ConfigResponse;

    #[tokio::test]
    async fn test_invalidate_by_app() {
        let cache = ConfigCache::new(CacheConfig::default());

        // Insert test data
        for profile in ["dev", "prod"] {
            for label in ["main", "feature"] {
                let key = CacheKey::new("myapp", profile, label);
                cache
                    .insert(key.clone(), ConfigResponse::empty("myapp", vec![profile.to_string()]))
                    .await;
                // Verificar que se insertó correctamente
                assert!(cache.get(&key).await.is_some());
            }
        }

        // Invalidate all myapp entries
        let result = cache.invalidate_by_app("myapp").await;

        assert_eq!(result.count, 4);

        // Verificar que las entries fueron invalidadas
        for profile in ["dev", "prod"] {
            for label in ["main", "feature"] {
                let key = CacheKey::new("myapp", profile, label);
                assert!(cache.get(&key).await.is_none());
            }
        }
    }

    #[tokio::test]
    async fn test_invalidate_by_app_profile() {
        let cache = ConfigCache::new(CacheConfig::default());

        // Insert test data
        for profile in ["dev", "prod"] {
            for label in ["main", "feature"] {
                let key = CacheKey::new("myapp", profile, label);
                cache
                    .insert(key.clone(), ConfigResponse::empty("myapp", vec![profile.to_string()]))
                    .await;
                assert!(cache.get(&key).await.is_some());
            }
        }

        // Invalidate only prod entries
        let result = cache.invalidate_by_app_profile("myapp", "prod").await;

        assert_eq!(result.count, 2);

        // Verificar que las prod entries fueron invalidadas
        for label in ["main", "feature"] {
            assert!(cache.get(&CacheKey::new("myapp", "prod", label)).await.is_none());
        }

        // Verificar que las dev entries siguen presentes
        for label in ["main", "feature"] {
            assert!(cache.get(&CacheKey::new("myapp", "dev", label)).await.is_some());
        }
    }

    #[tokio::test]
    async fn test_invalidate_by_pattern() {
        let cache = ConfigCache::new(CacheConfig::default());

        // Insert test data
        for app in ["myapp", "otherapp"] {
            for profile in ["dev", "prod"] {
                let key = CacheKey::new(app, profile, "main");
                cache
                    .insert(key.clone(), ConfigResponse::empty(app, vec![profile.to_string()]))
                    .await;
                assert!(cache.get(&key).await.is_some());
            }
        }

        // Invalidate all prod entries across all apps
        let result = cache.invalidate_by_pattern("*:prod:*").await;

        assert_eq!(result.count, 2);

        // Verificar que las prod entries fueron invalidadas
        assert!(cache.get(&CacheKey::new("myapp", "prod", "main")).await.is_none());
        assert!(cache.get(&CacheKey::new("otherapp", "prod", "main")).await.is_none());

        // Verificar que las dev entries siguen presentes
        assert!(cache.get(&CacheKey::new("myapp", "dev", "main")).await.is_some());
        assert!(cache.get(&CacheKey::new("otherapp", "dev", "main")).await.is_some());
    }

    #[tokio::test]
    async fn test_invalidate_by_patterns() {
        let cache = ConfigCache::new(CacheConfig::default());

        // Insert test data
        for app in ["myapp", "otherapp"] {
            for profile in ["dev", "prod"] {
                let key = CacheKey::new(app, profile, "main");
                cache
                    .insert(key.clone(), ConfigResponse::empty(app, vec![profile.to_string()]))
                    .await;
                assert!(cache.get(&key).await.is_some());
            }
        }

        // Invalidate myapp and otherapp:prod
        let patterns = vec!["myapp:*:*", "otherapp:prod:*"];
        let result = cache.invalidate_by_patterns(&patterns).await;

        // myapp:dev, myapp:prod, otherapp:prod = 3 entries
        assert_eq!(result.count, 3);

        // Verificar que las entries correctas fueron invalidadas
        assert!(cache.get(&CacheKey::new("myapp", "dev", "main")).await.is_none());
        assert!(cache.get(&CacheKey::new("myapp", "prod", "main")).await.is_none());
        assert!(cache.get(&CacheKey::new("otherapp", "prod", "main")).await.is_none());

        // Verificar que otherapp:dev sigue presente
        assert!(cache.get(&CacheKey::new("otherapp", "dev", "main")).await.is_some());
    }
}
