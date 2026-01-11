# Historia 002: Invalidacion de Cache

## Contexto y Objetivo

Un cache es tan util como su capacidad de invalidarse correctamente. Datos stale pueden causar problemas graves en produccion: una aplicacion podria usar credenciales expiradas o configuraciones incorrectas.

Esta historia implementa tres estrategias de invalidacion complementarias:

1. **TTL-based**: Expiracion automatica despues de un tiempo configurable
2. **On-demand**: Invalidacion explicita de entries individuales via API
3. **Pattern-based**: Invalidacion de multiples entries usando patrones glob

Para desarrolladores Java, esto es similar a como Caffeine + Spring Cache maneja invalidacion, pero con patrones async-native y sin necesidad de AOP.

---

## Alcance

### In Scope

- Invalidacion por TTL (ya parcialmente en historia 001)
- Endpoint `DELETE /cache/{key}` para invalidacion individual
- Endpoint `DELETE /cache?pattern=myapp:*` para invalidacion por patron
- Servicio `InvalidationService` centralizado
- Notificaciones async de invalidacion usando channels
- Tests de invalidacion

### Out of Scope

- Invalidacion distribuida (multi-nodo)
- Webhooks de invalidacion
- Invalidacion basada en eventos de backend (Git hooks)
- UI de administracion de cache

---

## Criterios de Aceptacion

- [ ] `DELETE /cache/{app}/{profile}/{label}` invalida entry especifica
- [ ] `DELETE /cache?pattern=myapp:*` invalida por patron glob
- [ ] Invalidacion retorna numero de entries afectadas
- [ ] Channels notifican a subscribers sobre invalidaciones
- [ ] Invalidacion de 1000 entries < 100ms
- [ ] Tests de integracion pasan
- [ ] Logs estructurados para cada invalidacion

---

## Diseno Propuesto

### Estructura de Modulos

```
crates/vortex-server/src/cache/
├── mod.rs
├── config_cache.rs      # Existente
├── keys.rs              # Existente
├── invalidation.rs      # Nuevo: InvalidationService
└── patterns.rs          # Nuevo: Pattern matching
```

### Interfaces Principales

```rust
/// Servicio de invalidacion de cache
pub struct InvalidationService {
    cache: ConfigCache,
    /// Channel para notificar invalidaciones
    invalidation_tx: broadcast::Sender<InvalidationEvent>,
}

/// Evento de invalidacion
#[derive(Debug, Clone)]
pub struct InvalidationEvent {
    pub keys: Vec<CacheKey>,
    pub reason: InvalidationReason,
    pub timestamp: Instant,
}

#[derive(Debug, Clone)]
pub enum InvalidationReason {
    Manual,
    Pattern(String),
    Ttl,
    Refresh,
}

/// Patron para invalidacion masiva
pub struct GlobPattern {
    pattern: glob::Pattern,
}
```

---

## Pasos de Implementacion

### Paso 1: Implementar Pattern Matching

```rust
// src/cache/patterns.rs
use glob::Pattern;
use crate::cache::CacheKey;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PatternError {
    #[error("invalid glob pattern: {0}")]
    InvalidPattern(#[from] glob::PatternError),
}

/// Patron glob para matching de cache keys.
/// Soporta: * (cualquier secuencia), ? (un caracter)
#[derive(Debug, Clone)]
pub struct GlobPattern {
    pattern: Pattern,
    raw: String,
}

impl GlobPattern {
    /// Crea un nuevo patron desde string.
    /// Ejemplos validos: "myapp:*:*", "myapp:prod:*", "*:*:main"
    pub fn new(pattern: &str) -> Result<Self, PatternError> {
        Ok(Self {
            pattern: Pattern::new(pattern)?,
            raw: pattern.to_string(),
        })
    }

    /// Verifica si una cache key matchea el patron.
    pub fn matches(&self, key: &CacheKey) -> bool {
        let key_str = key.to_string();
        self.pattern.matches(&key_str)
    }

    /// Retorna el patron original como string.
    pub fn as_str(&self) -> &str {
        &self.raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wildcard_matching() {
        let pattern = GlobPattern::new("myapp:*:*").unwrap();

        assert!(pattern.matches(&CacheKey::new("myapp", "prod", "main")));
        assert!(pattern.matches(&CacheKey::new("myapp", "dev", "feature")));
        assert!(!pattern.matches(&CacheKey::new("otherapp", "prod", "main")));
    }

    #[test]
    fn test_partial_wildcard() {
        let pattern = GlobPattern::new("*:prod:*").unwrap();

        assert!(pattern.matches(&CacheKey::new("app1", "prod", "main")));
        assert!(pattern.matches(&CacheKey::new("app2", "prod", "v2")));
        assert!(!pattern.matches(&CacheKey::new("app1", "dev", "main")));
    }
}
```

### Paso 2: Implementar InvalidationService

```rust
// src/cache/invalidation.rs
use crate::cache::{ConfigCache, CacheKey, GlobPattern, PatternError};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;
use tracing::{info, warn, instrument};

/// Evento emitido cuando entries son invalidadas
#[derive(Debug, Clone)]
pub struct InvalidationEvent {
    /// Keys que fueron invalidadas
    pub keys: Vec<CacheKey>,
    /// Razon de la invalidacion
    pub reason: InvalidationReason,
    /// Timestamp de la invalidacion
    pub timestamp: Instant,
}

#[derive(Debug, Clone)]
pub enum InvalidationReason {
    /// Invalidacion manual via API
    Manual,
    /// Invalidacion por patron glob
    Pattern(String),
    /// Expiracion por TTL
    Ttl,
    /// Refresh forzado
    Refresh,
}

/// Resultado de una operacion de invalidacion
#[derive(Debug)]
pub struct InvalidationResult {
    /// Numero de entries invalidadas
    pub count: usize,
    /// Keys que fueron invalidadas
    pub keys: Vec<CacheKey>,
}

/// Servicio centralizado de invalidacion de cache.
/// Maneja invalidacion individual, por patron, y notificaciones.
#[derive(Clone)]
pub struct InvalidationService {
    cache: ConfigCache,
    /// Channel para notificar invalidaciones a subscribers
    invalidation_tx: broadcast::Sender<InvalidationEvent>,
}

impl InvalidationService {
    /// Crea un nuevo servicio de invalidacion.
    /// Retorna el servicio y un receiver para subscribirse a eventos.
    pub fn new(cache: ConfigCache) -> (Self, broadcast::Receiver<InvalidationEvent>) {
        let (tx, rx) = broadcast::channel(100);
        (
            Self {
                cache,
                invalidation_tx: tx,
            },
            rx,
        )
    }

    /// Obtiene un nuevo receiver para subscribirse a eventos de invalidacion.
    pub fn subscribe(&self) -> broadcast::Receiver<InvalidationEvent> {
        self.invalidation_tx.subscribe()
    }

    /// Invalida una entry especifica por key.
    #[instrument(skip(self), fields(key = %key))]
    pub async fn invalidate_key(&self, key: &CacheKey) -> InvalidationResult {
        info!("invalidating cache entry");

        self.cache.invalidate(key);

        let event = InvalidationEvent {
            keys: vec![key.clone()],
            reason: InvalidationReason::Manual,
            timestamp: Instant::now(),
        };

        // Notificar subscribers (ignorar error si no hay receivers)
        let _ = self.invalidation_tx.send(event);

        InvalidationResult {
            count: 1,
            keys: vec![key.clone()],
        }
    }

    /// Invalida todas las entries que matchean un patron glob.
    #[instrument(skip(self), fields(pattern = %pattern.as_str()))]
    pub async fn invalidate_pattern(
        &self,
        pattern: &GlobPattern,
    ) -> InvalidationResult {
        info!("invalidating cache entries by pattern");

        // Obtener todas las keys que matchean
        let matching_keys = self.find_matching_keys(pattern).await;
        let count = matching_keys.len();

        if count == 0 {
            info!("no matching entries found");
            return InvalidationResult {
                count: 0,
                keys: vec![],
            };
        }

        // Invalidar cada key
        for key in &matching_keys {
            self.cache.invalidate(key);
        }

        info!(count = count, "invalidated entries");

        let event = InvalidationEvent {
            keys: matching_keys.clone(),
            reason: InvalidationReason::Pattern(pattern.as_str().to_string()),
            timestamp: Instant::now(),
        };

        let _ = self.invalidation_tx.send(event);

        InvalidationResult {
            count,
            keys: matching_keys,
        }
    }

    /// Invalida todas las entries del cache.
    #[instrument(skip(self))]
    pub async fn invalidate_all(&self) -> InvalidationResult {
        warn!("invalidating ALL cache entries");

        let count = self.cache.entry_count() as usize;
        self.cache.invalidate_all();

        InvalidationResult {
            count,
            keys: vec![], // No listamos todas las keys por performance
        }
    }

    /// Busca todas las keys que matchean un patron.
    /// Nota: Moka no expone iteracion sobre keys directamente,
    /// necesitamos mantener un indice separado o usar cache.iter()
    async fn find_matching_keys(&self, pattern: &GlobPattern) -> Vec<CacheKey> {
        // Iteramos sobre el cache para encontrar matches
        self.cache
            .iter()
            .filter(|(key, _)| pattern.matches(key))
            .map(|(key, _)| key.clone())
            .collect()
    }
}
```

### Paso 3: Agregar iter() a ConfigCache

```rust
// src/cache/config_cache.rs (agregar metodo)
impl ConfigCache {
    /// Itera sobre todas las entries del cache.
    /// Nota: Esta es una snapshot, entries pueden cambiar durante iteracion.
    pub fn iter(&self) -> impl Iterator<Item = (CacheKey, Arc<ConfigResponse>)> + '_ {
        self.inner.iter()
    }
}
```

### Paso 4: Implementar Endpoints HTTP

```rust
// src/handlers/cache.rs
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::cache::{CacheKey, GlobPattern, InvalidationService};
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct InvalidationQuery {
    pattern: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InvalidationResponse {
    pub invalidated_count: usize,
    pub keys: Vec<String>,
}

/// DELETE /cache/{app}/{profile}/{label}
/// Invalida una entry especifica
#[instrument(skip(state))]
pub async fn invalidate_key(
    State(state): State<AppState>,
    Path((app, profile, label)): Path<(String, String, String)>,
) -> Result<Json<InvalidationResponse>, StatusCode> {
    let key = CacheKey::new(&app, &profile, &label);

    let result = state.invalidation_service
        .invalidate_key(&key)
        .await;

    Ok(Json(InvalidationResponse {
        invalidated_count: result.count,
        keys: result.keys.iter().map(|k| k.to_string()).collect(),
    }))
}

/// DELETE /cache?pattern=myapp:*
/// Invalida entries por patron
#[instrument(skip(state))]
pub async fn invalidate_pattern(
    State(state): State<AppState>,
    Query(query): Query<InvalidationQuery>,
) -> Result<Json<InvalidationResponse>, StatusCode> {
    let pattern_str = query.pattern
        .ok_or(StatusCode::BAD_REQUEST)?;

    let pattern = GlobPattern::new(&pattern_str)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let result = state.invalidation_service
        .invalidate_pattern(&pattern)
        .await;

    Ok(Json(InvalidationResponse {
        invalidated_count: result.count,
        keys: result.keys.iter().map(|k| k.to_string()).collect(),
    }))
}

/// DELETE /cache (sin parametros)
/// Invalida todo el cache
#[instrument(skip(state))]
pub async fn invalidate_all(
    State(state): State<AppState>,
) -> Json<InvalidationResponse> {
    let result = state.invalidation_service
        .invalidate_all()
        .await;

    Json(InvalidationResponse {
        invalidated_count: result.count,
        keys: vec![], // No listamos todas por performance
    })
}
```

### Paso 5: Registrar Rutas

```rust
// src/server.rs (modificacion)
use crate::handlers::cache::{invalidate_key, invalidate_pattern, invalidate_all};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/{app}/{profile}", get(get_config))
        .route("/{app}/{profile}/{label}", get(get_config_with_label))
        // Rutas de invalidacion
        .route("/cache/:app/:profile/:label", delete(invalidate_key))
        .route("/cache", delete(invalidate_all))
        .route("/cache", delete(invalidate_pattern))  // Con query param
        .with_state(state)
}
```

---

## Conceptos de Rust Aprendidos

### 1. Broadcast Channels (tokio::sync::broadcast)

Los broadcast channels permiten enviar un mensaje a multiples receivers. Cada receiver obtiene una copia del mensaje.

**Rust:**
```rust
use tokio::sync::broadcast;

// Crear channel con capacidad de 100 mensajes
let (tx, mut rx1) = broadcast::channel::<InvalidationEvent>(100);
let mut rx2 = tx.subscribe();  // Segundo subscriber

// Enviar evento (todos los subscribers lo reciben)
tx.send(InvalidationEvent {
    keys: vec![key],
    reason: InvalidationReason::Manual,
    timestamp: Instant::now(),
})?;

// Cada receiver obtiene su propia copia
tokio::spawn(async move {
    while let Ok(event) = rx1.recv().await {
        println!("Receiver 1: {:?}", event);
    }
});

tokio::spawn(async move {
    while let Ok(event) = rx2.recv().await {
        println!("Receiver 2: {:?}", event);
    }
});
```

**Comparacion con Java:**
```java
// Java con Reactor/RxJava
PublishProcessor<InvalidationEvent> processor = PublishProcessor.create();

// Subscribers
processor.subscribe(event -> System.out.println("Sub 1: " + event));
processor.subscribe(event -> System.out.println("Sub 2: " + event));

// Publicar evento
processor.onNext(new InvalidationEvent(keys, reason));

// Java BlockingQueue (no es broadcast, un consumer toma el mensaje)
BlockingQueue<Event> queue = new LinkedBlockingQueue<>();
queue.put(event);  // Solo un consumer lo recibe
```

**Diferencias clave:**

| Aspecto | broadcast (Tokio) | PublishProcessor (RxJava) |
|---------|-------------------|---------------------------|
| Backpressure | Mensajes se pierden si buffer lleno | Configurable |
| Clonacion | Clona mensaje para cada receiver | Referencia compartida |
| Async | Nativo | Schedulers |
| Lifecycle | Sender dropped = channel closed | onComplete/onError |

### 2. tokio::sync::Notify

`Notify` permite que una tarea espere hasta ser notificada por otra. Similar a `CountDownLatch` o `Condition` en Java.

**Rust:**
```rust
use tokio::sync::Notify;
use std::sync::Arc;

let notify = Arc::new(Notify::new());
let notify_clone = notify.clone();

// Tarea que espera
tokio::spawn(async move {
    // Espera hasta que alguien llame notify()
    notify_clone.notified().await;
    println!("Received notification!");
});

// Mas tarde, notificar
notify.notify_one();  // Despierta una tarea esperando
// o
notify.notify_waiters();  // Despierta todas las tareas esperando
```

**Comparacion con Java:**
```java
// Java con CountDownLatch
CountDownLatch latch = new CountDownLatch(1);

// Thread que espera
new Thread(() -> {
    latch.await();  // Bloquea hasta countDown()
    System.out.println("Received notification!");
}).start();

// Notificar
latch.countDown();

// Java con Condition
Lock lock = new ReentrantLock();
Condition condition = lock.newCondition();

// Thread que espera
lock.lock();
try {
    condition.await();  // Espera signal()
} finally {
    lock.unlock();
}

// Notificar
lock.lock();
try {
    condition.signal();  // o signalAll()
} finally {
    lock.unlock();
}
```

### 3. Pattern Matching con glob

El crate `glob` permite matching de patrones estilo shell.

**Rust:**
```rust
use glob::Pattern;

// Crear patron
let pattern = Pattern::new("myapp:*:main")?;

// Matching
assert!(pattern.matches("myapp:prod:main"));
assert!(pattern.matches("myapp:dev:main"));
assert!(!pattern.matches("myapp:prod:feature"));

// Wildcards soportados:
// *  - cualquier secuencia de caracteres
// ?  - un solo caracter
// [abc] - cualquiera de a, b, o c
// [!abc] - cualquier caracter excepto a, b, c
```

**Comparacion con Java:**
```java
// Java con PathMatcher (para paths)
PathMatcher matcher = FileSystems.getDefault()
    .getPathMatcher("glob:myapp:*:main");

// Java con regex (mas comun para strings)
Pattern pattern = Pattern.compile("myapp:.*:main");
Matcher m = pattern.matcher("myapp:prod:main");
boolean matches = m.matches();
```

### 4. Manejo de Errores en Channels

Los channels de Tokio pueden fallar; es importante manejar estos casos.

**Rust:**
```rust
use tokio::sync::broadcast;

// Enviar puede fallar si no hay receivers
let result = tx.send(event);
match result {
    Ok(receiver_count) => println!("Sent to {} receivers", receiver_count),
    Err(SendError(event)) => {
        // No hay receivers, el evento se devuelve
        println!("No receivers, event: {:?}", event);
    }
}

// Patron comun: ignorar error si no hay subscribers
let _ = tx.send(event);  // Usa let _ para silenciar warning

// Recibir puede fallar si el sender fue dropped
match rx.recv().await {
    Ok(event) => handle_event(event),
    Err(RecvError::Closed) => {
        // Sender dropped, channel cerrado
        break;
    }
    Err(RecvError::Lagged(count)) => {
        // Perdimos 'count' mensajes por buffer lleno
        warn!("Lagged behind by {} messages", count);
    }
}
```

---

## Riesgos y Errores Comunes

### 1. Race condition en invalidacion + lookup

```rust
// POTENCIAL PROBLEMA:
// Thread 1: lookup key -> cache miss -> fetch from backend
// Thread 2: invalidate key
// Thread 1: insert fetched value into cache
// Resultado: valor stale en cache despues de invalidacion

// MITIGACION: Usar get_or_insert_with que es atomico
let value = cache.get_or_insert_with(key, || async {
    // Moka garantiza atomicidad de esta operacion
    fetch_from_backend().await
}).await;
```

### 2. Iteracion sobre cache mientras se modifica

```rust
// PRECAUCION: iter() es una snapshot, pero puede tener inconsistencias
async fn find_matching_keys(&self, pattern: &GlobPattern) -> Vec<CacheKey> {
    // Esta iteracion puede no ver keys recien insertadas
    // o puede ver keys que fueron invalidadas
    self.cache
        .iter()
        .filter(|(key, _)| pattern.matches(key))
        .collect()
}

// Es aceptable para invalidacion (peor caso: no invalida una key nueva)
// Para casos criticos, considerar un indice separado
```

### 3. Broadcast channel lagging

```rust
// Si un receiver es lento, puede perder mensajes
let mut rx = invalidation_service.subscribe();

loop {
    match rx.recv().await {
        Ok(event) => {
            // Procesar evento
            slow_operation(&event).await;  // Si esto es lento...
        }
        Err(broadcast::error::RecvError::Lagged(n)) => {
            // Perdimos n mensajes!
            warn!("Lost {} invalidation events", n);
            // Considerar invalidar todo el cache local
        }
        Err(broadcast::error::RecvError::Closed) => break,
    }
}
```

### 4. Olvidar sync() despues de invalidate

```rust
// Moka: invalidation es lazy por defecto
cache.invalidate(&key);

// El valor puede seguir visible brevemente
let value = cache.get(&key).await;  // Podria retornar Some!

// Para invalidacion inmediata, llamar sync()
cache.invalidate(&key);
cache.sync();  // Fuerza limpieza
let value = cache.get(&key).await;  // Ahora es None
```

---

## Pruebas

### Tests Unitarios

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_invalidate_single_key() {
        let cache = ConfigCache::new(CacheConfig::default());
        let (service, _rx) = InvalidationService::new(cache.clone());

        // Insertar entry
        let key = CacheKey::new("myapp", "prod", "main");
        cache.insert(key.clone(), ConfigResponse::default()).await;

        // Invalidar
        let result = service.invalidate_key(&key).await;

        assert_eq!(result.count, 1);
        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn test_invalidate_by_pattern() {
        let cache = ConfigCache::new(CacheConfig::default());
        let (service, _rx) = InvalidationService::new(cache.clone());

        // Insertar varias entries
        for profile in &["prod", "dev", "staging"] {
            let key = CacheKey::new("myapp", profile, "main");
            cache.insert(key, ConfigResponse::default()).await;
        }

        // Invalidar todas las de myapp
        let pattern = GlobPattern::new("myapp:*:*").unwrap();
        let result = service.invalidate_pattern(&pattern).await;

        assert_eq!(result.count, 3);
    }

    #[tokio::test]
    async fn test_invalidation_events_broadcast() {
        let cache = ConfigCache::new(CacheConfig::default());
        let (service, mut rx) = InvalidationService::new(cache.clone());

        let key = CacheKey::new("myapp", "prod", "main");
        cache.insert(key.clone(), ConfigResponse::default()).await;

        // Invalidar
        service.invalidate_key(&key).await;

        // Verificar que recibimos el evento
        let event = rx.recv().await.unwrap();
        assert_eq!(event.keys.len(), 1);
        assert!(matches!(event.reason, InvalidationReason::Manual));
    }

    #[tokio::test]
    async fn test_pattern_matching() {
        let pattern = GlobPattern::new("*:prod:*").unwrap();

        assert!(pattern.matches(&CacheKey::new("app1", "prod", "main")));
        assert!(pattern.matches(&CacheKey::new("app2", "prod", "v2")));
        assert!(!pattern.matches(&CacheKey::new("app1", "dev", "main")));
    }
}
```

### Tests de Integracion HTTP

```rust
// tests/cache_invalidation_test.rs
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

#[tokio::test]
async fn test_invalidate_endpoint() {
    let app = create_test_app().await;

    // Primero, popular el cache
    let _ = app
        .clone()
        .oneshot(Request::get("/myapp/prod/main").body(Body::empty()).unwrap())
        .await;

    // Invalidar
    let response = app
        .oneshot(
            Request::delete("/cache/myapp/prod/main")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: InvalidationResponse = parse_body(response).await;
    assert_eq!(body.invalidated_count, 1);
}

#[tokio::test]
async fn test_invalidate_pattern_endpoint() {
    let app = create_test_app().await;

    // Popular cache con varias apps
    for app_name in &["app1", "app2", "app3"] {
        let _ = app
            .clone()
            .oneshot(
                Request::get(&format!("/{}/prod/main", app_name))
                    .body(Body::empty())
                    .unwrap()
            )
            .await;
    }

    // Invalidar por patron
    let response = app
        .oneshot(
            Request::delete("/cache?pattern=app*:prod:*")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    let body: InvalidationResponse = parse_body(response).await;
    assert_eq!(body.invalidated_count, 3);
}
```

---

## Observabilidad

### Logging Estructurado

```rust
use tracing::{info, warn, instrument, Span};

#[instrument(skip(self), fields(key = %key))]
pub async fn invalidate_key(&self, key: &CacheKey) -> InvalidationResult {
    info!("invalidating cache entry");

    self.cache.invalidate(key);

    // Agregar campo al span actual
    Span::current().record("success", true);

    info!(count = 1, "cache entry invalidated");

    // ...
}

#[instrument(skip(self), fields(pattern = %pattern.as_str()))]
pub async fn invalidate_pattern(&self, pattern: &GlobPattern) -> InvalidationResult {
    let matching_keys = self.find_matching_keys(pattern).await;
    let count = matching_keys.len();

    if count == 0 {
        info!("no matching entries found for pattern");
    } else {
        info!(count, "invalidating matching entries");
    }

    // ...
}
```

### Preparacion para Metricas

```rust
// Contadores para metricas (implementacion completa en historia 004)
impl InvalidationService {
    fn record_invalidation(&self, count: usize, reason: &InvalidationReason) {
        // metrics::counter!("cache_invalidations_total", count as u64, "reason" => reason.as_str());
    }
}

impl InvalidationReason {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Pattern(_) => "pattern",
            Self::Ttl => "ttl",
            Self::Refresh => "refresh",
        }
    }
}
```

---

## Entregable Final

### Archivos Creados/Modificados

1. `crates/vortex-server/src/cache/patterns.rs` - GlobPattern para matching
2. `crates/vortex-server/src/cache/invalidation.rs` - InvalidationService
3. `crates/vortex-server/src/cache/mod.rs` - Re-exports actualizados
4. `crates/vortex-server/src/handlers/cache.rs` - Endpoints de invalidacion
5. `crates/vortex-server/src/server.rs` - Rutas y AppState actualizados
6. `crates/vortex-server/tests/cache_invalidation_test.rs` - Tests de integracion

### Verificacion

```bash
# Compilar
cargo build -p vortex-server

# Tests
cargo test -p vortex-server invalidation

# Tests de integracion
cargo test -p vortex-server --test cache_invalidation_test

# Clippy
cargo clippy -p vortex-server -- -D warnings
```

### Ejemplo de Uso

```bash
# Invalidar entry especifica
curl -X DELETE http://localhost:8080/cache/myapp/prod/main
# {"invalidated_count":1,"keys":["myapp:prod:main"]}

# Invalidar por patron
curl -X DELETE "http://localhost:8080/cache?pattern=myapp:*:*"
# {"invalidated_count":3,"keys":["myapp:prod:main","myapp:dev:main","myapp:staging:main"]}

# Invalidar todo
curl -X DELETE http://localhost:8080/cache
# {"invalidated_count":150,"keys":[]}
```

---

**Anterior**: [Historia 001 - Integracion de Moka Cache](./story-001-moka-integration.md)
**Siguiente**: [Historia 003 - Configuracion del Servidor](./story-003-server-config.md)
