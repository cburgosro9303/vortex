# Historia 001: Integracion de Moka Cache

## Contexto y Objetivo

El cache es fundamental para cumplir los objetivos de latencia de Vortex Config (p99 < 10ms). Sin cache, cada request requeriria leer del backend (Git, S3, SQL), resultando en latencias inaceptables para configuraciones frecuentemente accedidas.

Moka es una libreria de cache async-native para Rust, inspirada en Caffeine de Java. Ofrece:
- Cache thread-safe sin locks explicitos
- TTL (time-to-live) y TTI (time-to-idle) configurables
- Eviction policy TinyLFU (mejor que LRU tradicional)
- API async-friendly perfecta para Tokio

Para desarrolladores Java, Moka es conceptualmente similar a Caffeine + CompletableFuture, pero integrado nativamente con el runtime async de Rust.

---

## Alcance

### In Scope

- Wrapper de Moka cache para configuraciones
- TTL configurable por tipo de configuracion
- Cache key generation consistente
- Integracion con `ConfigSource` trait
- Tests unitarios de cache behavior

### Out of Scope

- Invalidacion avanzada (historia 002)
- Metricas de cache (historia 004)
- Persistencia de cache
- Cache distribuido

---

## Criterios de Aceptacion

- [ ] `ConfigCache` wrappea Moka con API type-safe
- [ ] TTL configurable (default 5 minutos)
- [ ] Max capacity configurable (default 10,000 entries)
- [ ] Cache keys normalizadas (lowercase, deterministic)
- [ ] `get_or_insert` pattern para evitar cache stampede
- [ ] Thread-safe para uso concurrente desde multiples handlers
- [ ] Tests pasan: `cargo test -p vortex-server cache`

---

## Diseno Propuesto

### Estructura de Modulos

```
crates/vortex-server/src/cache/
├── mod.rs              # pub mod config_cache, keys;
├── config_cache.rs     # ConfigCache struct
└── keys.rs             # CacheKey generation
```

### Interfaces Principales

```rust
// Cache key para configuraciones
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    app: String,
    profile: String,
    label: String,
}

// Wrapper type-safe sobre Moka
pub struct ConfigCache {
    inner: moka::future::Cache<CacheKey, Arc<ConfigResponse>>,
}

impl ConfigCache {
    pub fn new(config: CacheConfig) -> Self;

    pub async fn get(&self, key: &CacheKey) -> Option<Arc<ConfigResponse>>;

    pub async fn get_or_insert_with<F, Fut>(
        &self,
        key: CacheKey,
        init: F,
    ) -> Result<Arc<ConfigResponse>, Error>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<ConfigResponse, Error>>;

    pub async fn insert(&self, key: CacheKey, value: ConfigResponse);

    pub fn invalidate(&self, key: &CacheKey);
}
```

---

## Pasos de Implementacion

### Paso 1: Agregar Dependencias

```toml
# crates/vortex-server/Cargo.toml
[dependencies]
moka = { version = "0.12", features = ["future"] }
```

### Paso 2: Implementar CacheKey

```rust
// src/cache/keys.rs
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
    /// Crea una nueva cache key normalizando los valores.
    pub fn new(app: impl Into<String>, profile: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            app: app.into().to_lowercase(),
            profile: profile.into().to_lowercase(),
            label: label.into().to_lowercase(),
        }
    }

    pub fn app(&self) -> &str {
        &self.app
    }

    pub fn profile(&self) -> &str {
        &self.profile
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

impl fmt::Display for CacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.app, self.profile, self.label)
    }
}
```

### Paso 3: Implementar CacheConfig

```rust
// src/cache/config_cache.rs
use std::sync::Arc;
use std::time::Duration;
use moka::future::Cache;

/// Configuracion del cache
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
```

### Paso 4: Implementar ConfigCache

```rust
// src/cache/config_cache.rs (continuacion)
use crate::cache::keys::CacheKey;
use crate::handlers::response::ConfigResponse;
use std::future::Future;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("failed to fetch config: {0}")]
    FetchError(String),
}

/// Cache de configuraciones usando Moka.
/// Thread-safe y async-friendly.
#[derive(Clone)]
pub struct ConfigCache {
    inner: Cache<CacheKey, Arc<ConfigResponse>>,
}

impl ConfigCache {
    /// Crea un nuevo cache con la configuracion dada.
    pub fn new(config: CacheConfig) -> Self {
        let mut builder = Cache::builder()
            .max_capacity(config.max_capacity)
            .time_to_live(Duration::from_secs(config.ttl_seconds));

        if let Some(tti) = config.tti_seconds {
            builder = builder.time_to_idle(Duration::from_secs(tti));
        }

        Self {
            inner: builder.build(),
        }
    }

    /// Obtiene un valor del cache si existe.
    pub async fn get(&self, key: &CacheKey) -> Option<Arc<ConfigResponse>> {
        self.inner.get(key).await
    }

    /// Obtiene un valor o lo inserta usando la funcion proporcionada.
    /// Evita cache stampede: solo una tarea ejecuta `init` para una key dada.
    pub async fn get_or_insert_with<F, Fut>(
        &self,
        key: CacheKey,
        init: F,
    ) -> Result<Arc<ConfigResponse>, CacheError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<ConfigResponse, CacheError>>,
    {
        // Moka maneja internamente el "thundering herd" problem
        let value = self.inner
            .try_get_with(key, async {
                let response = init().await?;
                Ok(Arc::new(response))
            })
            .await
            .map_err(|e| CacheError::FetchError(e.to_string()))?;

        Ok(value)
    }

    /// Inserta un valor directamente en el cache.
    pub async fn insert(&self, key: CacheKey, value: ConfigResponse) {
        self.inner.insert(key, Arc::new(value)).await;
    }

    /// Invalida una entrada especifica.
    pub fn invalidate(&self, key: &CacheKey) {
        self.inner.invalidate(key);
    }

    /// Invalida todas las entradas.
    pub fn invalidate_all(&self) {
        self.inner.invalidate_all();
    }

    /// Retorna el numero aproximado de entries en cache.
    pub fn entry_count(&self) -> u64 {
        self.inner.entry_count()
    }
}
```

### Paso 5: Integrar con Estado del Servidor

```rust
// src/server.rs (modificacion)
use crate::cache::{ConfigCache, CacheConfig};
use std::sync::Arc;

/// Estado compartido del servidor
#[derive(Clone)]
pub struct AppState {
    pub cache: ConfigCache,
    pub config_source: Arc<dyn ConfigSource + Send + Sync>,
}

impl AppState {
    pub fn new(
        cache_config: CacheConfig,
        config_source: Arc<dyn ConfigSource + Send + Sync>,
    ) -> Self {
        Self {
            cache: ConfigCache::new(cache_config),
            config_source,
        }
    }
}
```

### Paso 6: Uso en Handlers

```rust
// src/handlers/config.rs (ejemplo de uso)
use axum::extract::State;
use crate::cache::CacheKey;
use crate::server::AppState;

pub async fn get_config(
    State(state): State<AppState>,
    path: ConfigPath,
) -> Result<Json<ConfigResponse>, AppError> {
    let key = CacheKey::new(&path.app, &path.profile, &path.label);

    let response = state.cache
        .get_or_insert_with(key, || async {
            // Solo se ejecuta en cache miss
            state.config_source
                .fetch(&path.app, &path.profile, &path.label)
                .await
                .map_err(|e| CacheError::FetchError(e.to_string()))
        })
        .await?;

    Ok(Json((*response).clone()))
}
```

---

## Conceptos de Rust Aprendidos

### 1. Arc (Atomic Reference Counting)

`Arc<T>` permite compartir datos inmutables entre multiples owners de forma thread-safe. Es esencial en aplicaciones async donde multiples tasks pueden necesitar acceso al mismo dato.

**Rust:**
```rust
use std::sync::Arc;

// Arc permite compartir ConfigResponse entre multiples requests
// sin copiar los datos
pub struct ConfigCache {
    // El valor en cache es Arc<ConfigResponse>
    // Multiples handlers pueden tener referencias al mismo dato
    inner: Cache<CacheKey, Arc<ConfigResponse>>,
}

impl ConfigCache {
    pub async fn get(&self, key: &CacheKey) -> Option<Arc<ConfigResponse>> {
        // Retornar Arc incrementa el reference count (atomico)
        // No hay copia de datos, solo incremento de contador
        self.inner.get(key).await
    }
}

// Uso en handler
async fn handler(cache: ConfigCache) {
    let response = cache.get(&key).await;

    // response es Arc<ConfigResponse>
    // Podemos clonar Arc (solo incrementa contador, O(1))
    let response_clone = response.clone();

    // Cuando todos los Arc salen de scope, el dato se libera
}
```

**Comparacion con Java:**
```java
// Java: Todo es referencia por defecto, GC maneja liberacion
// No necesitas Arc explicitamente

public class ConfigCache {
    private final Cache<CacheKey, ConfigResponse> cache;

    public ConfigResponse get(CacheKey key) {
        // Java retorna referencia, GC trackea cuando liberarla
        return cache.get(key);
    }
}

// AtomicReference para casos especiales de concurrencia
AtomicReference<ConfigResponse> ref = new AtomicReference<>(response);
```

**Diferencias clave:**

| Aspecto | Arc (Rust) | Referencias Java |
|---------|------------|------------------|
| Conteo | Explicito, atomico | Implicito (GC) |
| Overhead | Minimo (contador atomico) | GC pauses |
| Liberacion | Determinista (cuando count=0) | No determinista |
| Thread-safety | Built-in | Requiere sincronizacion |

### 2. Clone vs Copy y Arc::clone

En Rust, `Clone` y `Copy` son traits que definen como se duplican valores.

**Rust:**
```rust
// Copy: duplicacion bit-a-bit, implicita (tipos primitivos)
let x: i32 = 5;
let y = x;  // x se copia, ambos son validos
println!("{} {}", x, y);  // OK

// Clone: duplicacion explicita, puede ser costosa
let s1 = String::from("hello");
let s2 = s1.clone();  // Copia los datos del heap
// s1 todavia es valido porque usamos clone()

// Arc::clone: barato! Solo incrementa contador
let arc1 = Arc::new(expensive_data);
let arc2 = Arc::clone(&arc1);  // O(1), no copia datos
// Convencion: usar Arc::clone() en lugar de arc1.clone()
// para hacer explicito que es barato
```

**Comparacion con Java:**
```java
// Java: clone() puede ser superficial o profunda
public class ConfigResponse implements Cloneable {
    @Override
    protected Object clone() {
        // Puede ser shallow o deep copy
        return super.clone();
    }
}

// Referencias se copian implicitamente (como Arc)
ConfigResponse ref1 = new ConfigResponse();
ConfigResponse ref2 = ref1;  // Misma referencia, como Arc
```

### 3. Async Cache Patterns

Moka proporciona patrones async-safe para evitar problemas comunes de cache.

**Rust (Moka):**
```rust
use moka::future::Cache;

// Cache stampede prevention con try_get_with
impl ConfigCache {
    /// get_or_insert_with evita "thundering herd"
    /// Si 100 requests llegan para la misma key:
    /// - Solo 1 ejecuta la funcion init
    /// - Las otras 99 esperan el resultado
    pub async fn get_or_insert_with<F, Fut>(
        &self,
        key: CacheKey,
        init: F,
    ) -> Result<Arc<ConfigResponse>, CacheError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<ConfigResponse, CacheError>>,
    {
        self.inner
            .try_get_with(key, async {
                // Esta closure solo se ejecuta una vez por key
                let response = init().await?;
                Ok(Arc::new(response))
            })
            .await
            .map_err(|e| CacheError::FetchError(e.to_string()))
    }
}
```

**Comparacion con Java (Caffeine):**
```java
// Java con Caffeine
Cache<CacheKey, ConfigResponse> cache = Caffeine.newBuilder()
    .maximumSize(10_000)
    .expireAfterWrite(5, TimeUnit.MINUTES)
    .build();

// get() con loader - similar a get_or_insert_with
public CompletableFuture<ConfigResponse> getConfig(CacheKey key) {
    return cache.get(key, k -> {
        // Esta lambda solo se ejecuta una vez por key
        // Caffeine maneja concurrencia internamente
        return fetchFromBackend(k);
    });
}
```

### 4. Trait Bounds en Funciones Async

Rust requiere bounds explicitos para closures async.

**Rust:**
```rust
// F: closure que retorna Fut
// Fut: Future que produce Result<T, E>
pub async fn get_or_insert_with<F, Fut>(
    &self,
    key: CacheKey,
    init: F,  // La closure
) -> Result<Arc<ConfigResponse>, CacheError>
where
    F: FnOnce() -> Fut,                              // F es una closure
    Fut: Future<Output = Result<ConfigResponse, CacheError>>,  // que retorna Future
{
    // ...
}

// Uso:
cache.get_or_insert_with(key, || async {
    // async block se convierte en Future automaticamente
    fetch_config().await
}).await;
```

**Comparacion con Java:**
```java
// Java usa interfaces funcionales
public <T> CompletableFuture<T> getOrInsert(
    Key key,
    Supplier<CompletableFuture<T>> loader  // Equivalente a F
) {
    return cache.get(key, k -> loader.get().join());
}

// Uso:
cache.getOrInsert(key, () -> fetchConfigAsync());
```

---

## Riesgos y Errores Comunes

### 1. Olvidar await en operaciones de cache

```rust
// MAL: El Future nunca se ejecuta
async fn bad_cache_usage(cache: &ConfigCache) {
    cache.get(&key);  // Retorna Future, pero no lo esperamos!
}

// BIEN: Usar await
async fn good_cache_usage(cache: &ConfigCache) {
    let value = cache.get(&key).await;  // Ahora si se ejecuta
}
```

### 2. Cache key inconsistente

```rust
// MAL: Keys no normalizadas pueden causar duplicados
let key1 = CacheKey::new("MyApp", "Prod", "Main");
let key2 = CacheKey::new("myapp", "prod", "main");
// key1 != key2, tendriamos dos entries para la misma config!

// BIEN: Normalizar en el constructor
impl CacheKey {
    pub fn new(app: impl Into<String>, ...) -> Self {
        Self {
            app: app.into().to_lowercase(),  // Siempre lowercase
            // ...
        }
    }
}
```

### 3. No manejar errores en init closure

```rust
// MAL: unwrap puede causar panic
cache.get_or_insert_with(key, || async {
    fetch_config().await.unwrap()  // Panic si falla!
}).await;

// BIEN: Propagar errores
cache.get_or_insert_with(key, || async {
    fetch_config().await
        .map_err(|e| CacheError::FetchError(e.to_string()))
}).await?;  // Propagar con ?
```

### 4. Arc sin necesidad

```rust
// MAL: Arc innecesario para datos pequenos
let cache: Cache<Key, Arc<bool>> = ...;  // bool es Copy!

// BIEN: Usar Arc solo para datos grandes o que requieren sharing
let cache: Cache<Key, bool> = ...;  // Para tipos Copy
let cache: Cache<Key, Arc<LargeStruct>> = ...;  // Para structs grandes
```

---

## Pruebas

### Tests Unitarios

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_cache_insert_and_get() {
        let cache = ConfigCache::new(CacheConfig::default());
        let key = CacheKey::new("myapp", "prod", "main");

        let response = ConfigResponse {
            name: "myapp".to_string(),
            profiles: vec!["prod".to_string()],
            label: "main".to_string(),
            property_sources: vec![],
        };

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

        let call_count = std::sync::atomic::AtomicU32::new(0);

        // Primera llamada: ejecuta init
        let result1 = cache.get_or_insert_with(key.clone(), || {
            call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async {
                Ok(ConfigResponse {
                    name: "myapp".to_string(),
                    profiles: vec!["prod".to_string()],
                    label: "main".to_string(),
                    property_sources: vec![],
                })
            }
        }).await;

        assert!(result1.is_ok());
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);

        // Segunda llamada: usa cache, no ejecuta init
        let result2 = cache.get_or_insert_with(key.clone(), || {
            call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async {
                Ok(ConfigResponse::default())
            }
        }).await;

        assert!(result2.is_ok());
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_cache_key_normalization() {
        let key1 = CacheKey::new("MyApp", "PROD", "Main");
        let key2 = CacheKey::new("myapp", "prod", "main");

        assert_eq!(key1, key2);
        assert_eq!(key1.to_string(), "myapp:prod:main");
    }

    #[tokio::test]
    async fn test_invalidate_removes_entry() {
        let cache = ConfigCache::new(CacheConfig::default());
        let key = CacheKey::new("myapp", "prod", "main");

        cache.insert(key.clone(), ConfigResponse::default()).await;
        assert!(cache.get(&key).await.is_some());

        cache.invalidate(&key);

        // Moka invalidation es async, puede requerir sync
        tokio::time::sleep(Duration::from_millis(10)).await;
        cache.inner.sync();

        assert!(cache.get(&key).await.is_none());
    }
}
```

### Tests de Concurrencia

```rust
#[tokio::test]
async fn test_concurrent_access() {
    let cache = Arc::new(ConfigCache::new(CacheConfig::default()));
    let call_count = Arc::new(std::sync::atomic::AtomicU32::new(0));

    let key = CacheKey::new("myapp", "prod", "main");

    // Simular 100 requests concurrentes para la misma key
    let mut handles = vec![];

    for _ in 0..100 {
        let cache = Arc::clone(&cache);
        let key = key.clone();
        let count = Arc::clone(&call_count);

        handles.push(tokio::spawn(async move {
            cache.get_or_insert_with(key, || {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                async {
                    // Simular latencia de backend
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    Ok(ConfigResponse::default())
                }
            }).await
        }));
    }

    // Esperar todas las tasks
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    // Solo deberia haber llamado init UNA vez
    // (Moka previene thundering herd)
    assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
}
```

---

## Observabilidad

### Logging

```rust
use tracing::{info, debug, instrument};

impl ConfigCache {
    #[instrument(skip(self, init), fields(key = %key))]
    pub async fn get_or_insert_with<F, Fut>(
        &self,
        key: CacheKey,
        init: F,
    ) -> Result<Arc<ConfigResponse>, CacheError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<ConfigResponse, CacheError>>,
    {
        if let Some(cached) = self.get(&key).await {
            debug!("cache hit");
            return Ok(cached);
        }

        debug!("cache miss, fetching from backend");
        // ... resto de implementacion
    }
}
```

### Preparacion para Metricas (Historia 004)

```rust
// Estructura preparada para metricas
impl ConfigCache {
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entry_count: self.entry_count(),
            // Metricas adicionales en historia 004
        }
    }
}

#[derive(Debug)]
pub struct CacheStats {
    pub entry_count: u64,
}
```

---

## Entregable Final

### Archivos Creados/Modificados

1. `crates/vortex-server/src/cache/mod.rs` - Re-exports del modulo
2. `crates/vortex-server/src/cache/keys.rs` - CacheKey implementation
3. `crates/vortex-server/src/cache/config_cache.rs` - ConfigCache con Moka
4. `crates/vortex-server/src/server.rs` - AppState con cache
5. `crates/vortex-server/tests/cache_test.rs` - Tests de cache

### Verificacion

```bash
# Compilar
cargo build -p vortex-server

# Tests
cargo test -p vortex-server cache

# Clippy
cargo clippy -p vortex-server -- -D warnings

# Documentacion
cargo doc -p vortex-server --no-deps --open
```

### Ejemplo de Uso Completo

```rust
use vortex_server::cache::{ConfigCache, CacheConfig, CacheKey};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Configurar cache
    let cache = ConfigCache::new(CacheConfig {
        ttl_seconds: 300,      // 5 minutos
        max_capacity: 10_000,  // 10k entries max
        tti_seconds: Some(60), // Expira si no se accede en 1 min
    });

    let key = CacheKey::new("myapp", "production", "main");

    // Obtener o insertar
    let response = cache.get_or_insert_with(key.clone(), || async {
        // Fetch from backend (solo en cache miss)
        fetch_from_git().await
    }).await?;

    println!("Config: {:?}", response);

    // Stats
    println!("Cache entries: {}", cache.entry_count());
}
```

---

**Anterior**: [Indice de Epica 05](./index.md)
**Siguiente**: [Historia 002 - Invalidacion de Cache](./story-002-invalidation.md)
