//! Configuration cache using Moka.

use crate::cache::keys::CacheKey;
use crate::handlers::response::ConfigResponse;
use crate::metrics::CacheMetrics;
use moka::future::Cache;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Error del sistema de cache
#[derive(Debug, Error)]
pub enum CacheError {
    #[error("failed to fetch config: {0}")]
    FetchError(String),
}

/// Configuracion del cache.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// TTL en segundos (default: 300 = 5 minutos)
    pub ttl_seconds: u64,
    /// Maximo numero de entries (default: 10000)
    pub max_capacity: u64,
    /// Time-to-idle en segundos (opcional)
    pub tti_seconds: Option<u64>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            ttl_seconds: 300,
            max_capacity: 10_000,
            tti_seconds: None,
        }
    }
}

/// Cache de configuraciones usando Moka.
/// Thread-safe y async-friendly.
///
/// # Examples
///
/// ```no_run
/// use vortex_server::cache::{ConfigCache, CacheConfig, CacheKey};
///
/// # #[tokio::main]
/// # async fn main() {
/// let cache = ConfigCache::new(CacheConfig::default());
/// let key = CacheKey::new("myapp", "prod", "main");
///
/// // Get value if exists
/// if let Some(response) = cache.get(&key).await {
///     println!("Cache hit!");
/// }
/// # }
/// ```
#[derive(Clone)]
pub struct ConfigCache {
    inner: Cache<CacheKey, Arc<ConfigResponse>>,
    metrics: CacheMetrics,
}

impl ConfigCache {
    /// Crea un nuevo cache con la configuracion dada.
    pub fn new(config: CacheConfig) -> Self {
        let metrics = CacheMetrics::new();

        let mut builder = Cache::builder()
            .max_capacity(config.max_capacity)
            .time_to_live(Duration::from_secs(config.ttl_seconds));

        if let Some(tti) = config.tti_seconds {
            builder = builder.time_to_idle(Duration::from_secs(tti));
        }

        // Configurar listener para evictions
        let eviction_metrics = metrics.clone();
        builder = builder.eviction_listener(move |_key, _value, cause| {
            let reason = match cause {
                moka::notification::RemovalCause::Expired => "ttl",
                moka::notification::RemovalCause::Size => "capacity",
                moka::notification::RemovalCause::Explicit => "manual",
                moka::notification::RemovalCause::Replaced => "replaced",
            };
            eviction_metrics.record_eviction(reason);
        });

        Self {
            inner: builder.build(),
            metrics,
        }
    }

    /// Obtiene un valor del cache si existe.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use vortex_server::cache::{ConfigCache, CacheConfig, CacheKey};
    /// # #[tokio::main]
    /// # async fn main() {
    /// # let cache = ConfigCache::new(CacheConfig::default());
    /// # let key = CacheKey::new("app", "prod", "main");
    /// if let Some(config) = cache.get(&key).await {
    ///     println!("Found config in cache");
    /// }
    /// # }
    /// ```
    pub async fn get(&self, key: &CacheKey) -> Option<Arc<ConfigResponse>> {
        let start = Instant::now();
        let result = self.inner.get(key).await;

        if result.is_some() {
            self.metrics.record_hit();
        } else {
            self.metrics.record_miss();
        }

        self.metrics
            .record_operation_duration("get", start.elapsed());
        self.update_entry_gauge();

        result
    }

    /// Obtiene un valor o lo inserta usando la funcion proporcionada.
    /// Evita cache stampede: solo una tarea ejecuta `init` para una key dada.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use vortex_server::cache::{ConfigCache, CacheConfig, CacheKey, CacheError};
    /// # use vortex_server::handlers::response::ConfigResponse;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), CacheError> {
    /// # let cache = ConfigCache::new(CacheConfig::default());
    /// # let key = CacheKey::new("app", "prod", "main");
    /// let config = cache.get_or_insert_with(key, || async {
    ///     // Fetch from backend (solo en cache miss)
    ///     Ok(ConfigResponse::empty("myapp", vec!["prod".to_string()]))
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_or_insert_with<F, Fut>(
        &self,
        key: CacheKey,
        init: F,
    ) -> Result<Arc<ConfigResponse>, CacheError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<ConfigResponse, CacheError>>,
    {
        let start = Instant::now();

        // Verificar si existe primero
        if let Some(cached) = self.inner.get(&key).await {
            self.metrics.record_hit();
            self.metrics
                .record_operation_duration("get_or_insert_hit", start.elapsed());
            return Ok(cached);
        }

        self.metrics.record_miss();

        // Fetch desde backend
        let value = self
            .inner
            .try_get_with(key, async {
                let response = init().await?;
                Ok(Arc::new(response))
            })
            .await
            .map_err(|e: std::sync::Arc<CacheError>| CacheError::FetchError(e.to_string()))?;

        self.metrics
            .record_operation_duration("get_or_insert_miss", start.elapsed());
        self.update_entry_gauge();

        Ok(value)
    }

    /// Inserta un valor directamente en el cache.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use vortex_server::cache::{ConfigCache, CacheConfig, CacheKey};
    /// # use vortex_server::handlers::response::ConfigResponse;
    /// # #[tokio::main]
    /// # async fn main() {
    /// # let cache = ConfigCache::new(CacheConfig::default());
    /// # let key = CacheKey::new("app", "prod", "main");
    /// # let response = ConfigResponse::empty("myapp", vec!["prod".to_string()]);
    /// cache.insert(key, response).await;
    /// # }
    /// ```
    pub async fn insert(&self, key: CacheKey, value: ConfigResponse) {
        self.inner.insert(key, Arc::new(value)).await;
    }

    /// Invalida una entrada especifica.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use vortex_server::cache::{ConfigCache, CacheConfig, CacheKey};
    /// # #[tokio::main]
    /// # async fn main() {
    /// # let cache = ConfigCache::new(CacheConfig::default());
    /// # let key = CacheKey::new("app", "prod", "main");
    /// cache.invalidate(&key).await;
    /// # }
    /// ```
    pub async fn invalidate(&self, key: &CacheKey) {
        self.inner.invalidate(key).await;
    }

    /// Invalida todas las entradas.
    pub fn invalidate_all(&self) {
        self.inner.invalidate_all();
    }

    /// Retorna el numero aproximado de entries en cache.
    pub fn entry_count(&self) -> u64 {
        self.inner.entry_count()
    }

    /// Itera sobre todas las entries del cache.
    /// Nota: Esta es una snapshot, entries pueden cambiar durante iteracion.
    pub fn iter(&self) -> impl Iterator<Item = (Arc<CacheKey>, Arc<ConfigResponse>)> + '_ {
        self.inner.iter()
    }

    /// Actualiza el gauge de entry count.
    fn update_entry_gauge(&self) {
        self.metrics.update_entry_count(self.inner.entry_count());
    }

    /// Retorna las metricas para acceso externo.
    pub fn metrics(&self) -> &CacheMetrics {
        &self.metrics
    }

    /// Sincroniza el cache (para tests principalmente).
    /// Fuerza la limpieza de entries expiradas.
    #[cfg(test)]
    pub(crate) fn sync(&self) {
        self.inner.run_pending_tasks();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_cache_insert_and_get() {
        let cache = ConfigCache::new(CacheConfig::default());
        let key = CacheKey::new("myapp", "prod", "main");

        let response = ConfigResponse::empty("myapp", vec!["prod".to_string()]);

        cache.insert(key.clone(), response.clone()).await;

        let cached = cache.get(&key).await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().name, "myapp");
    }

    #[tokio::test]
    async fn test_cache_miss_returns_none() {
        let cache = ConfigCache::new(CacheConfig::default());
        let key = CacheKey::new("nonexistent", "prod", "main");

        let result = cache.get(&key).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_or_insert_with_populates_cache() {
        let cache = ConfigCache::new(CacheConfig::default());
        let key = CacheKey::new("myapp", "prod", "main");

        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = Arc::clone(&call_count);

        // Primera llamada: ejecuta init
        let result1 = cache
            .get_or_insert_with(key.clone(), || {
                let count = Arc::clone(&call_count_clone);
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Ok(ConfigResponse::empty("myapp", vec!["prod".to_string()]))
                }
            })
            .await;

        assert!(result1.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Segunda llamada: usa cache, no ejecuta init
        let result2 = cache
            .get_or_insert_with(key.clone(), || {
                let count = Arc::clone(&call_count);
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Ok(ConfigResponse::empty("myapp", vec!["prod".to_string()]))
                }
            })
            .await;

        assert!(result2.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_invalidate_removes_entry() {
        let cache = ConfigCache::new(CacheConfig::default());
        let key = CacheKey::new("myapp", "prod", "main");

        cache
            .insert(
                key.clone(),
                ConfigResponse::empty("myapp", vec!["prod".to_string()]),
            )
            .await;
        assert!(cache.get(&key).await.is_some());

        cache.invalidate(&key).await;

        // Forzar limpieza
        cache.sync();

        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn test_multiple_inserts() {
        let cache = ConfigCache::new(CacheConfig::default());

        // Insert multiple entries
        for i in 0..10 {
            let key = CacheKey::new(&format!("app{}", i), "prod", "main");
            cache
                .insert(
                    key.clone(),
                    ConfigResponse::empty("test", vec!["prod".to_string()]),
                )
                .await;
            // Verify each entry was inserted
            assert!(cache.get(&key).await.is_some());
        }

        // Verify all entries are still accessible
        for i in 0..10 {
            let key = CacheKey::new(&format!("app{}", i), "prod", "main");
            assert!(cache.get(&key).await.is_some());
        }
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let cache = Arc::new(ConfigCache::new(CacheConfig::default()));
        let call_count = Arc::new(AtomicU32::new(0));

        let key = CacheKey::new("myapp", "prod", "main");

        // Simular 100 requests concurrentes para la misma key
        let mut handles = vec![];

        for _ in 0..100 {
            let cache = Arc::clone(&cache);
            let key = key.clone();
            let count = Arc::clone(&call_count);

            handles.push(tokio::spawn(async move {
                cache
                    .get_or_insert_with(key, || {
                        let count = Arc::clone(&count);
                        async move {
                            count.fetch_add(1, Ordering::SeqCst);
                            // Simular latencia de backend
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            Ok(ConfigResponse::empty("myapp", vec!["prod".to_string()]))
                        }
                    })
                    .await
            }));
        }

        // Esperar todas las tasks
        for handle in handles {
            handle.await.unwrap().unwrap();
        }

        // Solo deberia haber llamado init UNA vez
        // (Moka previene thundering herd)
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }
}
