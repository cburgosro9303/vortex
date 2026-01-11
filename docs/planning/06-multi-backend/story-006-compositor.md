# Historia 006: Backend Compositor

## Contexto y Objetivo

Esta historia implementa un backend compositor que combina multiples fuentes de configuracion con prioridades configurables. Esto permite escenarios como:

- **Override local**: Git base + SQL overrides por ambiente
- **Fallback**: Intentar S3 primero, caer a SQL si falla
- **Multi-tenancy**: Diferentes backends por aplicacion

El compositor implementa el patron Strategy, permitiendo agregar y remover backends dinamicamente mientras mantiene una interfaz unificada `ConfigSource`.

---

## Alcance

### In Scope

- `CompositeConfigSource` que implementa `ConfigSource`
- Configuracion de prioridades (mayor numero = mayor prioridad)
- Estrategias de merge: override, merge-deep, first-wins
- Manejo de errores por backend (fail-fast vs continue)
- Queries paralelas a todos los backends

### Out of Scope

- Hot-reload de configuracion de backends
- Balanceo de carga entre backends
- Circuit breaker por backend
- Persistencia de configuracion del compositor

---

## Criterios de Aceptacion

- [ ] `CompositeConfigSource` implementa `ConfigSource`
- [ ] Soporta agregar/remover backends con prioridades
- [ ] Merge correcto: prioridad mayor sobrescribe menor
- [ ] Queries paralelas a todos los backends
- [ ] Estrategia configurable para errores (fail-fast/continue)
- [ ] Logging de cada backend consultado
- [ ] Tests con multiples backends mockeados

---

## Diseno Propuesto

### Arquitectura

```
┌─────────────────────────────────────────────────────────────┐
│                  CompositeConfigSource                       │
├─────────────────────────────────────────────────────────────┤
│  backends: Vec<PrioritizedBackend>                          │
│  error_strategy: ErrorStrategy                              │
│  merge_strategy: MergeStrategy                              │
├─────────────────────────────────────────────────────────────┤
│  + get_config() -> queries all, merges by priority          │
│  + add_backend(priority, source)                            │
│  + remove_backend(name)                                     │
│  + list_backends() -> Vec<BackendInfo>                      │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ contains
                          ▼
┌─────────────────────────────────────────────────────────────┐
│              PrioritizedBackend                              │
├─────────────────────────────────────────────────────────────┤
│  name: String                                               │
│  priority: i32                                              │
│  source: Arc<dyn ConfigSource>                              │
│  enabled: bool                                              │
└─────────────────────────────────────────────────────────────┘
```

### Flujo de Merge

```
Request: get_config("myapp", ["prod"])

Backend Priorities:
  - Git:  priority 10 (base config)
  - SQL:  priority 20 (environment overrides)
  - S3:   priority 30 (emergency overrides)

┌────────────────────────────────────────────────────────────┐
│  1. Query all backends in parallel                          │
├────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────┐    ┌─────────┐    ┌─────────┐                │
│  │   Git   │    │   SQL   │    │   S3    │                │
│  │ pri: 10 │    │ pri: 20 │    │ pri: 30 │                │
│  └────┬────┘    └────┬────┘    └────┬────┘                │
│       │              │              │                      │
│       ▼              ▼              ▼                      │
│  ┌─────────┐    ┌─────────┐    ┌─────────┐                │
│  │ port:   │    │ port:   │    │ port:   │                │
│  │   8080  │    │   9090  │    │ (empty) │                │
│  │ timeout:│    │ timeout:│    │         │                │
│  │   30    │    │   60    │    │         │                │
│  │ feature:│    │         │    │ feature:│                │
│  │  false  │    │         │    │  true   │                │
│  └─────────┘    └─────────┘    └─────────┘                │
│                                                             │
├────────────────────────────────────────────────────────────┤
│  2. Sort by priority (ascending for merge)                  │
├────────────────────────────────────────────────────────────┤
│                                                             │
│  Git (10) → SQL (20) → S3 (30)                             │
│                                                             │
├────────────────────────────────────────────────────────────┤
│  3. Merge: later (higher priority) wins                     │
├────────────────────────────────────────────────────────────┤
│                                                             │
│  Result:                                                    │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ port: 9090      (SQL overrides Git)                  │  │
│  │ timeout: 60     (SQL overrides Git)                  │  │
│  │ feature: true   (S3 overrides Git)                   │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                             │
└────────────────────────────────────────────────────────────┘
```

### Interfaces

```rust
/// Priority for a backend (higher = more important)
pub type Priority = i32;

/// Strategy for handling backend errors
#[derive(Debug, Clone, Copy)]
pub enum ErrorStrategy {
    /// Fail immediately if any backend errors
    FailFast,
    /// Continue with other backends, log error
    Continue,
    /// Continue only for specific error types
    ContinueOnNotFound,
}

/// Strategy for merging configurations
#[derive(Debug, Clone, Copy)]
pub enum MergeStrategy {
    /// Higher priority values completely override lower
    Override,
    /// Deep merge objects, higher priority wins on conflicts
    DeepMerge,
    /// Use first successful result only
    FirstWins,
}

/// A backend with its priority
pub struct PrioritizedBackend {
    pub name: String,
    pub priority: Priority,
    pub source: Arc<dyn ConfigSource + Send + Sync>,
    pub enabled: bool,
}

/// Composite configuration source
pub struct CompositeConfigSource {
    backends: RwLock<Vec<PrioritizedBackend>>,
    error_strategy: ErrorStrategy,
    merge_strategy: MergeStrategy,
}
```

---

## Pasos de Implementacion

### Paso 1: Definir Tipos Base

```rust
// src/composite/types.rs
use std::sync::Arc;
use crate::traits::ConfigSource;

/// Priority level for a backend.
/// Higher values = higher priority (will override lower priorities).
pub type Priority = i32;

/// Common priority constants.
pub mod priorities {
    use super::Priority;

    /// Lowest priority - base defaults
    pub const BASE: Priority = 0;
    /// Standard priority for primary storage
    pub const PRIMARY: Priority = 10;
    /// Environment-specific overrides
    pub const ENVIRONMENT: Priority = 20;
    /// Emergency/runtime overrides
    pub const EMERGENCY: Priority = 100;
}

/// Strategy for handling backend errors.
#[derive(Debug, Clone, Copy, Default)]
pub enum ErrorStrategy {
    /// Fail immediately if any backend returns an error.
    FailFast,
    /// Log error and continue with remaining backends.
    #[default]
    Continue,
    /// Continue only if the error is "not found".
    ContinueOnNotFound,
}

/// Strategy for merging configurations from multiple backends.
#[derive(Debug, Clone, Copy, Default)]
pub enum MergeStrategy {
    /// Higher priority values completely replace lower priority values.
    #[default]
    Override,
    /// Deep merge objects; higher priority wins on key conflicts.
    DeepMerge,
    /// Use the first successful result; ignore other backends.
    FirstWins,
}

/// Information about a registered backend.
#[derive(Debug, Clone)]
pub struct BackendInfo {
    pub name: String,
    pub priority: Priority,
    pub enabled: bool,
    pub backend_type: String,
}
```

### Paso 2: Implementar PrioritizedBackend

```rust
// src/composite/backend.rs
use std::sync::Arc;
use crate::traits::ConfigSource;
use super::types::Priority;

/// A backend with its priority configuration.
pub struct PrioritizedBackend {
    /// Unique name for this backend instance.
    pub name: String,

    /// Priority level (higher = more important).
    pub priority: Priority,

    /// The actual configuration source.
    pub source: Arc<dyn ConfigSource + Send + Sync>,

    /// Whether this backend is currently enabled.
    pub enabled: bool,
}

impl PrioritizedBackend {
    /// Creates a new prioritized backend.
    pub fn new(
        name: impl Into<String>,
        priority: Priority,
        source: Arc<dyn ConfigSource + Send + Sync>,
    ) -> Self {
        Self {
            name: name.into(),
            priority,
            source,
            enabled: true,
        }
    }

    /// Disables this backend.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Enables this backend.
    pub fn enable(&mut self) {
        self.enabled = true;
    }
}

impl std::fmt::Debug for PrioritizedBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrioritizedBackend")
            .field("name", &self.name)
            .field("priority", &self.priority)
            .field("enabled", &self.enabled)
            .field("source_type", &self.source.name())
            .finish()
    }
}
```

### Paso 3: Implementar Merge Logic

```rust
// src/composite/merge.rs
use serde_json::{Map, Value};
use crate::types::PropertySource;
use super::types::MergeStrategy;

/// Merges property sources according to the given strategy.
pub fn merge_property_sources(
    sources: Vec<PropertySource>,
    strategy: MergeStrategy,
) -> Vec<PropertySource> {
    match strategy {
        MergeStrategy::Override => merge_override(sources),
        MergeStrategy::DeepMerge => merge_deep(sources),
        MergeStrategy::FirstWins => first_wins(sources),
    }
}

/// Override merge: each source is kept separate, ordered by priority.
/// Higher priority sources come first in the list (Spring Cloud Config style).
fn merge_override(sources: Vec<PropertySource>) -> Vec<PropertySource> {
    // Sources should already be sorted by priority (high to low)
    sources
}

/// Deep merge: combine all sources into a single merged source.
fn merge_deep(sources: Vec<PropertySource>) -> Vec<PropertySource> {
    if sources.is_empty() {
        return Vec::new();
    }

    let mut merged = Map::new();
    let mut names = Vec::new();

    // Merge in order (low priority first, so high priority overwrites)
    for source in sources.iter().rev() {
        names.push(source.name.clone());
        deep_merge_maps(&mut merged, &source.source);
    }

    vec![PropertySource {
        name: format!("merged[{}]", names.join(", ")),
        source: merged,
    }]
}

/// First wins: return only the first non-empty source.
fn first_wins(sources: Vec<PropertySource>) -> Vec<PropertySource> {
    sources
        .into_iter()
        .find(|s| !s.source.is_empty())
        .map(|s| vec![s])
        .unwrap_or_default()
}

/// Deep merges src into dst, with src values winning on conflicts.
fn deep_merge_maps(dst: &mut Map<String, Value>, src: &Map<String, Value>) {
    for (key, src_value) in src {
        match dst.get_mut(key) {
            Some(Value::Object(dst_map)) if src_value.is_object() => {
                // Both are objects, merge recursively
                deep_merge_maps(dst_map, src_value.as_object().unwrap());
            }
            _ => {
                // Replace value (src wins)
                dst.insert(key.clone(), src_value.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deep_merge_combines_objects() {
        let mut dst: Map<String, Value> = serde_json::from_value(json!({
            "server": {
                "port": 8080,
                "host": "localhost"
            }
        })).unwrap();

        let src: Map<String, Value> = serde_json::from_value(json!({
            "server": {
                "port": 9090,
                "ssl": true
            }
        })).unwrap();

        deep_merge_maps(&mut dst, &src);

        assert_eq!(dst["server"]["port"], 9090);  // src wins
        assert_eq!(dst["server"]["host"], "localhost");  // kept from dst
        assert_eq!(dst["server"]["ssl"], true);  // added from src
    }
}
```

### Paso 4: Implementar CompositeConfigSource

```rust
// src/composite/source.rs
use std::sync::Arc;
use async_trait::async_trait;
use futures::future::join_all;
use tokio::sync::RwLock;
use tracing::{info, warn, debug, instrument};

use crate::traits::ConfigSource;
use crate::types::{ConfigMap, PropertySource};
use crate::error::BackendError;
use super::backend::PrioritizedBackend;
use super::types::{Priority, ErrorStrategy, MergeStrategy, BackendInfo};
use super::merge::merge_property_sources;

/// A composite configuration source that combines multiple backends.
pub struct CompositeConfigSource {
    backends: RwLock<Vec<PrioritizedBackend>>,
    error_strategy: ErrorStrategy,
    merge_strategy: MergeStrategy,
}

impl CompositeConfigSource {
    /// Creates a new empty composite source.
    pub fn new() -> Self {
        Self {
            backends: RwLock::new(Vec::new()),
            error_strategy: ErrorStrategy::default(),
            merge_strategy: MergeStrategy::default(),
        }
    }

    /// Creates a composite source with the given strategies.
    pub fn with_strategies(
        error_strategy: ErrorStrategy,
        merge_strategy: MergeStrategy,
    ) -> Self {
        Self {
            backends: RwLock::new(Vec::new()),
            error_strategy,
            merge_strategy,
        }
    }

    /// Adds a backend with the given priority.
    pub async fn add_backend(
        &self,
        name: impl Into<String>,
        priority: Priority,
        source: Arc<dyn ConfigSource + Send + Sync>,
    ) {
        let backend = PrioritizedBackend::new(name, priority, source);

        let mut backends = self.backends.write().await;

        // Remove existing backend with same name
        backends.retain(|b| b.name != backend.name);

        backends.push(backend);

        // Sort by priority (ascending, so we can process low-to-high)
        backends.sort_by_key(|b| b.priority);

        info!(
            backend = %backends.last().unwrap().name,
            priority = priority,
            total_backends = backends.len(),
            "Added backend to composite source"
        );
    }

    /// Removes a backend by name.
    pub async fn remove_backend(&self, name: &str) -> bool {
        let mut backends = self.backends.write().await;
        let len_before = backends.len();
        backends.retain(|b| b.name != name);
        let removed = backends.len() < len_before;

        if removed {
            info!(backend = %name, "Removed backend from composite source");
        }

        removed
    }

    /// Enables or disables a backend.
    pub async fn set_backend_enabled(&self, name: &str, enabled: bool) -> bool {
        let mut backends = self.backends.write().await;

        if let Some(backend) = backends.iter_mut().find(|b| b.name == name) {
            backend.enabled = enabled;
            info!(backend = %name, enabled = enabled, "Backend status changed");
            true
        } else {
            false
        }
    }

    /// Lists all registered backends.
    pub async fn list_backends(&self) -> Vec<BackendInfo> {
        let backends = self.backends.read().await;

        backends
            .iter()
            .map(|b| BackendInfo {
                name: b.name.clone(),
                priority: b.priority,
                enabled: b.enabled,
                backend_type: b.source.name().to_string(),
            })
            .collect()
    }

    /// Queries all enabled backends in parallel.
    async fn query_all_backends(
        &self,
        app: &str,
        profiles: &[String],
        label: Option<&str>,
    ) -> Vec<(String, Priority, Result<ConfigMap, BackendError>)> {
        let backends = self.backends.read().await;

        let enabled_backends: Vec<_> = backends
            .iter()
            .filter(|b| b.enabled)
            .collect();

        if enabled_backends.is_empty() {
            warn!("No enabled backends in composite source");
            return Vec::new();
        }

        debug!(
            app = %app,
            profiles = ?profiles,
            backends = enabled_backends.len(),
            "Querying backends"
        );

        // Query all backends in parallel
        let futures: Vec<_> = enabled_backends
            .iter()
            .map(|backend| {
                let name = backend.name.clone();
                let priority = backend.priority;
                let source = Arc::clone(&backend.source);
                let app = app.to_string();
                let profiles = profiles.to_vec();
                let label = label.map(String::from);

                async move {
                    let result = source
                        .get_config(&app, &profiles, label.as_deref())
                        .await;

                    (name, priority, result)
                }
            })
            .collect();

        join_all(futures).await
    }

    /// Processes results according to error strategy.
    fn process_results(
        &self,
        results: Vec<(String, Priority, Result<ConfigMap, BackendError>)>,
    ) -> Result<Vec<(Priority, Vec<PropertySource>)>, BackendError> {
        let mut successful = Vec::new();

        for (name, priority, result) in results {
            match result {
                Ok(config) => {
                    debug!(
                        backend = %name,
                        priority = priority,
                        sources = config.property_sources.len(),
                        "Backend returned config"
                    );
                    successful.push((priority, config.property_sources));
                }
                Err(e) => {
                    match self.error_strategy {
                        ErrorStrategy::FailFast => {
                            return Err(BackendError::CompositeError(format!(
                                "Backend '{}' failed: {}", name, e
                            )));
                        }
                        ErrorStrategy::Continue => {
                            warn!(
                                backend = %name,
                                error = %e,
                                "Backend error (continuing)"
                            );
                        }
                        ErrorStrategy::ContinueOnNotFound => {
                            if matches!(e, BackendError::NotFound(_)) {
                                debug!(backend = %name, "Backend not found (continuing)");
                            } else {
                                return Err(BackendError::CompositeError(format!(
                                    "Backend '{}' failed: {}", name, e
                                )));
                            }
                        }
                    }
                }
            }
        }

        Ok(successful)
    }
}

impl Default for CompositeConfigSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConfigSource for CompositeConfigSource {
    #[instrument(skip(self), fields(backends = tracing::field::Empty))]
    async fn get_config(
        &self,
        app: &str,
        profiles: &[String],
        label: Option<&str>,
    ) -> Result<ConfigMap, BackendError> {
        // Query all backends
        let results = self.query_all_backends(app, profiles, label).await;

        if results.is_empty() {
            return Err(BackendError::NotFound(
                "No backends configured".to_string()
            ));
        }

        // Process results according to error strategy
        let successful = self.process_results(results)?;

        if successful.is_empty() {
            return Err(BackendError::NotFound(format!(
                "No backend returned config for app '{}'", app
            )));
        }

        // Flatten and sort by priority
        let mut all_sources: Vec<(Priority, PropertySource)> = successful
            .into_iter()
            .flat_map(|(priority, sources)| {
                sources.into_iter().map(move |s| (priority, s))
            })
            .collect();

        // Sort by priority descending (highest first)
        all_sources.sort_by(|a, b| b.0.cmp(&a.0));

        let sources: Vec<PropertySource> = all_sources
            .into_iter()
            .map(|(_, s)| s)
            .collect();

        // Apply merge strategy
        let merged = merge_property_sources(sources, self.merge_strategy);

        Ok(ConfigMap {
            name: app.to_string(),
            profiles: profiles.to_vec(),
            label: label.map(String::from),
            version: None,
            state: None,
            property_sources: merged,
        })
    }

    fn name(&self) -> &str {
        "composite"
    }
}
```

### Paso 5: Builder Pattern

```rust
// src/composite/builder.rs
use std::sync::Arc;
use crate::traits::ConfigSource;
use super::source::CompositeConfigSource;
use super::types::{Priority, ErrorStrategy, MergeStrategy};

/// Builder for CompositeConfigSource.
pub struct CompositeBuilder {
    error_strategy: ErrorStrategy,
    merge_strategy: MergeStrategy,
    backends: Vec<(String, Priority, Arc<dyn ConfigSource + Send + Sync>)>,
}

impl CompositeBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self {
            error_strategy: ErrorStrategy::default(),
            merge_strategy: MergeStrategy::default(),
            backends: Vec::new(),
        }
    }

    /// Sets the error handling strategy.
    pub fn error_strategy(mut self, strategy: ErrorStrategy) -> Self {
        self.error_strategy = strategy;
        self
    }

    /// Sets the merge strategy.
    pub fn merge_strategy(mut self, strategy: MergeStrategy) -> Self {
        self.merge_strategy = strategy;
        self
    }

    /// Adds a backend.
    pub fn add_backend(
        mut self,
        name: impl Into<String>,
        priority: Priority,
        source: impl ConfigSource + Send + Sync + 'static,
    ) -> Self {
        self.backends.push((name.into(), priority, Arc::new(source)));
        self
    }

    /// Adds a backend from an Arc.
    pub fn add_backend_arc(
        mut self,
        name: impl Into<String>,
        priority: Priority,
        source: Arc<dyn ConfigSource + Send + Sync>,
    ) -> Self {
        self.backends.push((name.into(), priority, source));
        self
    }

    /// Builds the composite source.
    pub async fn build(self) -> CompositeConfigSource {
        let source = CompositeConfigSource::with_strategies(
            self.error_strategy,
            self.merge_strategy,
        );

        for (name, priority, backend) in self.backends {
            source.add_backend(name, priority, backend).await;
        }

        source
    }
}

impl Default for CompositeBuilder {
    fn default() -> Self {
        Self::new()
    }
}
```

---

## Conceptos de Rust Aprendidos

### 1. Trait Objects con dyn

Para almacenar diferentes tipos de backends, usamos trait objects.

**Rust:**
```rust
use std::sync::Arc;

// Trait object: dynamic dispatch
let backends: Vec<Arc<dyn ConfigSource + Send + Sync>> = vec![
    Arc::new(GitConfigSource::new()),
    Arc::new(S3ConfigSource::new()),
    Arc::new(SqlConfigSource::new()),
];

// Llamar metodos via dynamic dispatch
for backend in &backends {
    let config = backend.get_config("app", &profiles, None).await?;
}

// Send + Sync son necesarios para uso entre threads
// Arc permite ownership compartido thread-safe
```

**Comparacion con Java:**
```java
// Java usa interfaces naturalmente
List<ConfigSource> backends = List.of(
    new GitConfigSource(),
    new S3ConfigSource(),
    new SqlConfigSource()
);

// Polimorfismo via interface
for (ConfigSource backend : backends) {
    ConfigMap config = backend.getConfig("app", profiles, null);
}
```

**Diferencias:**
| Aspecto | Rust (dyn Trait) | Java (Interface) |
|---------|------------------|------------------|
| Dispatch | Runtime (vtable) | Runtime (vtable) |
| Size | Fat pointer (2 usize) | Reference |
| Thread safety | Explicit (Send+Sync) | Implicit |
| Ownership | Arc/Box required | GC managed |

### 2. RwLock para Concurrencia

**Rust:**
```rust
use tokio::sync::RwLock;

pub struct CompositeConfigSource {
    // RwLock permite multiple readers O un writer
    backends: RwLock<Vec<PrioritizedBackend>>,
}

impl CompositeConfigSource {
    pub async fn list_backends(&self) -> Vec<BackendInfo> {
        // Read lock: multiple concurrent readers allowed
        let backends = self.backends.read().await;
        backends.iter().map(|b| b.info()).collect()
    }

    pub async fn add_backend(&self, backend: PrioritizedBackend) {
        // Write lock: exclusive access
        let mut backends = self.backends.write().await;
        backends.push(backend);
    }
}
```

**Comparacion con Java:**
```java
public class CompositeConfigSource {
    private final ReadWriteLock lock = new ReentrantReadWriteLock();
    private final List<Backend> backends = new ArrayList<>();

    public List<BackendInfo> listBackends() {
        lock.readLock().lock();
        try {
            return backends.stream()
                .map(Backend::info)
                .collect(toList());
        } finally {
            lock.readLock().unlock();
        }
    }

    public void addBackend(Backend backend) {
        lock.writeLock().lock();
        try {
            backends.add(backend);
        } finally {
            lock.writeLock().unlock();
        }
    }
}
```

### 3. Strategy Pattern con Enums

**Rust:**
```rust
#[derive(Debug, Clone, Copy)]
pub enum MergeStrategy {
    Override,
    DeepMerge,
    FirstWins,
}

impl MergeStrategy {
    pub fn merge(&self, sources: Vec<PropertySource>) -> Vec<PropertySource> {
        match self {
            Self::Override => merge_override(sources),
            Self::DeepMerge => merge_deep(sources),
            Self::FirstWins => first_wins(sources),
        }
    }
}

// Uso
let merged = config.merge_strategy.merge(sources);
```

**Comparacion con Java Strategy:**
```java
public interface MergeStrategy {
    List<PropertySource> merge(List<PropertySource> sources);
}

public class OverrideStrategy implements MergeStrategy {
    @Override
    public List<PropertySource> merge(List<PropertySource> sources) {
        return mergeOverride(sources);
    }
}

public class DeepMergeStrategy implements MergeStrategy {
    @Override
    public List<PropertySource> merge(List<PropertySource> sources) {
        return mergeDeep(sources);
    }
}

// Uso
List<PropertySource> merged = config.getMergeStrategy().merge(sources);
```

### 4. Parallel Futures con join_all

**Rust:**
```rust
use futures::future::join_all;

async fn query_all_backends(&self) -> Vec<Result<ConfigMap, Error>> {
    let backends = self.backends.read().await;

    // Crear futures para cada backend
    let futures: Vec<_> = backends
        .iter()
        .map(|b| {
            let source = Arc::clone(&b.source);
            async move {
                source.get_config("app", &[], None).await
            }
        })
        .collect();

    // Ejecutar todos en paralelo
    join_all(futures).await
}
```

**Comparacion con Java CompletableFuture:**
```java
public List<ConfigMap> queryAllBackends() {
    List<CompletableFuture<ConfigMap>> futures = backends.stream()
        .map(backend -> CompletableFuture.supplyAsync(() ->
            backend.getConfig("app", List.of(), null)
        ))
        .collect(toList());

    // Wait for all
    return futures.stream()
        .map(CompletableFuture::join)
        .collect(toList());

    // Or with allOf
    CompletableFuture.allOf(futures.toArray(new CompletableFuture[0]))
        .thenApply(v -> futures.stream()
            .map(CompletableFuture::join)
            .collect(toList()));
}
```

---

## Riesgos y Errores Comunes

### 1. Deadlock con RwLock

```rust
// MAL: Deadlock potential
async fn bad_nested_lock(&self) {
    let backends = self.backends.read().await;
    for backend in backends.iter() {
        // Intenta adquirir otro read lock mientras tiene uno
        self.some_method_that_locks().await;  // DEADLOCK si usa write!
    }
}

// BIEN: Liberar lock antes de operaciones que puedan bloquear
async fn good_nested(&self) {
    let backend_list: Vec<_> = {
        let backends = self.backends.read().await;
        backends.iter().map(|b| b.clone()).collect()
    };  // Lock liberado aqui

    for backend in backend_list {
        self.some_method_that_locks().await;  // OK
    }
}
```

### 2. Arc Clone en Loops

```rust
// Cuidado con clones innecesarios
async fn query(&self) {
    let backends = self.backends.read().await;

    // MAL: Clone innecesario cada iteracion
    for backend in backends.iter() {
        let source = backend.source.clone();  // Arc::clone es barato pero...
    }

    // BIEN: Clone solo cuando necesario
    let futures: Vec<_> = backends
        .iter()
        .map(|b| {
            let source = Arc::clone(&b.source);  // Necesario para mover a async block
            async move { source.get_config(...).await }
        })
        .collect();
}
```

### 3. Error Handling en Paralelo

```rust
// MAL: Ignora errores silenciosamente
let results = join_all(futures).await;
let configs: Vec<_> = results
    .into_iter()
    .filter_map(|r| r.ok())  // Silenciosamente ignora errores!
    .collect();

// BIEN: Manejar errores explicitamente
let results = join_all(futures).await;
for (name, result) in names.iter().zip(results) {
    match result {
        Ok(config) => successful.push(config),
        Err(e) => {
            match self.error_strategy {
                ErrorStrategy::FailFast => return Err(e),
                ErrorStrategy::Continue => {
                    tracing::warn!(backend = %name, error = %e, "Backend failed");
                }
            }
        }
    }
}
```

---

## Pruebas

### Tests con Mock Backends

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use async_trait::async_trait;

    /// Mock backend for testing
    struct MockBackend {
        name: String,
        config: ConfigMap,
        should_fail: bool,
    }

    #[async_trait]
    impl ConfigSource for MockBackend {
        async fn get_config(&self, app: &str, profiles: &[String], _label: Option<&str>)
            -> Result<ConfigMap, BackendError>
        {
            if self.should_fail {
                Err(BackendError::ConnectionError("Mock error".into()))
            } else {
                Ok(self.config.clone())
            }
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    fn mock_backend(name: &str, props: serde_json::Map<String, serde_json::Value>) -> MockBackend {
        MockBackend {
            name: name.to_string(),
            config: ConfigMap {
                name: "test".to_string(),
                profiles: vec!["default".to_string()],
                label: None,
                version: None,
                state: None,
                property_sources: vec![PropertySource {
                    name: name.to_string(),
                    source: props,
                }],
            },
            should_fail: false,
        }
    }

    #[tokio::test]
    async fn higher_priority_overrides_lower() {
        let composite = CompositeBuilder::new()
            .add_backend("low", 10, mock_backend("low", serde_json::json!({
                "port": 8080,
                "host": "localhost"
            }).as_object().unwrap().clone()))
            .add_backend("high", 20, mock_backend("high", serde_json::json!({
                "port": 9090
            }).as_object().unwrap().clone()))
            .build()
            .await;

        let config = composite
            .get_config("test", &["default".to_string()], None)
            .await
            .unwrap();

        // High priority should come first
        assert_eq!(config.property_sources[0].name, "high");
        assert_eq!(config.property_sources[1].name, "low");
    }

    #[tokio::test]
    async fn fail_fast_stops_on_error() {
        let mut failing = mock_backend("failing", serde_json::Map::new());
        failing.should_fail = true;

        let composite = CompositeBuilder::new()
            .error_strategy(ErrorStrategy::FailFast)
            .add_backend("failing", 10, failing)
            .add_backend("working", 20, mock_backend("working", serde_json::Map::new()))
            .build()
            .await;

        let result = composite
            .get_config("test", &["default".to_string()], None)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn continue_on_error_returns_successful() {
        let mut failing = mock_backend("failing", serde_json::Map::new());
        failing.should_fail = true;

        let composite = CompositeBuilder::new()
            .error_strategy(ErrorStrategy::Continue)
            .add_backend("failing", 10, failing)
            .add_backend("working", 20, mock_backend("working", serde_json::json!({
                "key": "value"
            }).as_object().unwrap().clone()))
            .build()
            .await;

        let config = composite
            .get_config("test", &["default".to_string()], None)
            .await
            .unwrap();

        assert_eq!(config.property_sources.len(), 1);
        assert_eq!(config.property_sources[0].name, "working");
    }
}
```

---

## Observabilidad

### Logging

```rust
impl CompositeConfigSource {
    async fn get_config(&self, ...) -> Result<ConfigMap, BackendError> {
        let span = tracing::info_span!(
            "composite_get_config",
            app = %app,
            profiles = ?profiles
        );

        async {
            let backends = self.list_backends().await;
            tracing::Span::current().record("backends", backends.len());

            // ... query logic ...

            tracing::info!(
                successful = successful_count,
                failed = failed_count,
                "Composite query complete"
            );

            result
        }
        .instrument(span)
        .await
    }
}
```

### Metricas

```rust
// Suggested metrics
composite_backends_total{status="enabled|disabled"}
composite_query_duration_seconds{app}
composite_backend_query_duration_seconds{backend}
composite_backend_errors_total{backend, error_type}
composite_merge_duration_seconds{strategy}
```

---

## Entregable Final

### Archivos Creados

1. `src/composite/mod.rs` - Module exports
2. `src/composite/types.rs` - Type definitions
3. `src/composite/backend.rs` - PrioritizedBackend
4. `src/composite/merge.rs` - Merge strategies
5. `src/composite/source.rs` - CompositeConfigSource
6. `src/composite/builder.rs` - Builder pattern
7. `tests/composite_test.rs` - Tests

### Verificacion

```bash
# Compilar
cargo build -p vortex-backends

# Tests
cargo test -p vortex-backends composite

# Doc
cargo doc -p vortex-backends --open
```

### Ejemplo de Uso

```rust
use vortex_backends::{
    composite::{CompositeBuilder, ErrorStrategy, MergeStrategy, priorities},
    git::GitConfigSource,
    s3::S3ConfigSource,
    sql::SqlConfigSource,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create individual backends
    let git = GitConfigSource::new("/path/to/repo")?;
    let s3 = S3ConfigSource::new(S3Config::new("bucket")).await?;
    let sql = SqlConfigSource::new(SqlConfig::new("postgres://...")).await?;

    // Build composite with priorities
    let composite = CompositeBuilder::new()
        .error_strategy(ErrorStrategy::Continue)
        .merge_strategy(MergeStrategy::Override)
        .add_backend("git", priorities::BASE, git)
        .add_backend("sql", priorities::ENVIRONMENT, sql)
        .add_backend("s3", priorities::EMERGENCY, s3)
        .build()
        .await;

    // Query - will check all backends, merge by priority
    let config = composite
        .get_config("payment-service", &["production".to_string()], None)
        .await?;

    println!("Merged config from {} sources", config.property_sources.len());

    // List backends
    for backend in composite.list_backends().await {
        println!("  {} (priority: {}, type: {})",
            backend.name, backend.priority, backend.backend_type);
    }

    Ok(())
}
```
