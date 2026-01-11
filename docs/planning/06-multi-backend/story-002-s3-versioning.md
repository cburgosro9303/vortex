# Historia 002: Backend S3 - Listing y Versionado

## Contexto y Objetivo

Esta historia extiende el backend S3 para soportar listado de configuraciones disponibles y versionado de objetos S3. El versionado permite mantener historico de cambios y hacer rollback a versiones anteriores, similar a como Git maneja branches y commits.

**Capacidades agregadas:**
- Listar todas las aplicaciones/perfiles disponibles en el bucket
- Acceder a versiones especificas de configuraciones
- Consultar historico de versiones
- Pagination para buckets con muchos objetos

S3 Versioning es una feature nativa de AWS S3 que mantiene multiples versiones de cada objeto, ideal para configuraciones criticas que requieren auditoria.

---

## Alcance

### In Scope

- Implementar metodo `list_applications()` en S3ConfigSource
- Implementar metodo `list_profiles()` para una aplicacion
- Soporte para S3 Object Versioning
- Metodo `get_config_version()` para obtener version especifica
- Metodo `list_versions()` para ver historico
- Pagination con continuation tokens
- Async iterators para resultados grandes

### Out of Scope

- Lifecycle policies para versiones antiguas
- Delete markers handling
- Cross-region replication
- S3 Batch Operations
- Escritura/upload de configuraciones

---

## Criterios de Aceptacion

- [ ] `list_applications()` retorna lista de apps en el bucket
- [ ] `list_profiles(app)` retorna perfiles disponibles para una app
- [ ] `get_config_version(app, profile, version_id)` obtiene version especifica
- [ ] `list_versions(app, profile)` retorna historico de versiones
- [ ] Pagination funciona para buckets con >1000 objetos
- [ ] Async iterators para resultados grandes
- [ ] Maneja buckets sin versioning habilitado
- [ ] Tests con LocalStack

---

## Diseno Propuesto

### Estructura de Versiones en S3

```
s3://my-config-bucket/payment-service/production.yml
├── Version: abc123 (current)
│   └── Content: server.port=9090
├── Version: def456
│   └── Content: server.port=8080
└── Version: ghi789
    └── Content: server.port=8000
```

### Interfaces Principales

```rust
// Extender S3ConfigSource
impl S3ConfigSource {
    /// Lists all applications in the bucket.
    pub fn list_applications(&self) -> impl Stream<Item = Result<String, BackendError>>;

    /// Lists all profiles for an application.
    pub fn list_profiles(&self, app: &str) -> impl Stream<Item = Result<String, BackendError>>;

    /// Gets a specific version of a config.
    pub async fn get_config_version(
        &self,
        app: &str,
        profiles: &[String],
        version_id: &str,
    ) -> Result<ConfigMap, BackendError>;

    /// Lists all versions of a config file.
    pub fn list_versions(
        &self,
        app: &str,
        profile: &str,
    ) -> impl Stream<Item = Result<ConfigVersion, BackendError>>;
}

/// Metadata about a config version.
#[derive(Debug, Clone)]
pub struct ConfigVersion {
    pub version_id: String,
    pub last_modified: DateTime<Utc>,
    pub size: i64,
    pub is_latest: bool,
    pub etag: String,
}
```

### Diagrama de Flujo - Listing

```
┌──────────────────────┐
│  list_applications() │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────────────────────┐
│  S3 ListObjectsV2                    │
│  Prefix: "" (root)                   │
│  Delimiter: "/"                      │
└──────────┬───────────────────────────┘
           │
           ▼
┌──────────────────────────────────────┐
│  Response contains:                  │
│  CommonPrefixes: [                   │
│    "payment-service/",               │
│    "user-service/",                  │
│    "shared/"                         │
│  ]                                   │
└──────────┬───────────────────────────┘
           │
           ▼
┌──────────────────────────────────────┐
│  If truncated:                       │
│  - Use ContinuationToken             │
│  - Yield more results                │
└──────────┬───────────────────────────┘
           │
           ▼
┌──────────────────────────────────────┐
│  Stream yields:                      │
│  "payment-service"                   │
│  "user-service"                      │
│  "shared"                            │
└──────────────────────────────────────┘
```

---

## Pasos de Implementacion

### Paso 1: Implementar Listing de Aplicaciones

```rust
// src/s3/listing.rs
use aws_sdk_s3::Client;
use futures::stream::{self, Stream, StreamExt};
use crate::error::BackendError;

impl S3ConfigSource {
    /// Lists all applications (top-level directories) in the bucket.
    ///
    /// Returns an async stream that handles pagination automatically.
    pub fn list_applications(&self) -> impl Stream<Item = Result<String, BackendError>> + '_ {
        let prefix = self.config.path_prefix.clone().unwrap_or_default();

        stream::unfold(
            ListState::Initial,
            move |state| {
                let prefix = prefix.clone();
                async move {
                    self.list_applications_page(&prefix, state).await
                }
            }
        )
        .flat_map(|result| {
            match result {
                Ok(apps) => stream::iter(apps.into_iter().map(Ok)).boxed(),
                Err(e) => stream::once(async move { Err(e) }).boxed(),
            }
        })
    }

    async fn list_applications_page(
        &self,
        prefix: &str,
        state: ListState,
    ) -> Option<(Result<Vec<String>, BackendError>, ListState)> {
        let continuation_token = match state {
            ListState::Initial => None,
            ListState::Continue(token) => Some(token),
            ListState::Done => return None,
        };

        let mut request = self.client
            .list_objects_v2()
            .bucket(&self.config.bucket)
            .delimiter("/");

        if !prefix.is_empty() {
            request = request.prefix(prefix);
        }

        if let Some(token) = continuation_token {
            request = request.continuation_token(token);
        }

        let result = request.send().await;

        match result {
            Ok(output) => {
                let apps: Vec<String> = output
                    .common_prefixes()
                    .iter()
                    .filter_map(|p| p.prefix())
                    .map(|p| {
                        // Remove prefix and trailing slash
                        let name = p.strip_prefix(prefix).unwrap_or(p);
                        name.trim_end_matches('/').to_string()
                    })
                    .filter(|s| !s.is_empty())
                    .collect();

                let next_state = if output.is_truncated() == Some(true) {
                    output
                        .next_continuation_token()
                        .map(|t| ListState::Continue(t.to_string()))
                        .unwrap_or(ListState::Done)
                } else {
                    ListState::Done
                };

                Some((Ok(apps), next_state))
            }
            Err(e) => {
                Some((Err(BackendError::S3Error(e.to_string())), ListState::Done))
            }
        }
    }
}

/// Internal state for pagination.
enum ListState {
    Initial,
    Continue(String),
    Done,
}
```

### Paso 2: Implementar Listing de Profiles

```rust
// src/s3/listing.rs (continued)
impl S3ConfigSource {
    /// Lists all profiles available for an application.
    pub fn list_profiles(
        &self,
        app: &str,
    ) -> impl Stream<Item = Result<String, BackendError>> + '_ {
        let prefix = match &self.config.path_prefix {
            Some(p) => format!("{}/{}/", p.trim_end_matches('/'), app),
            None => format!("{}/", app),
        };

        stream::unfold(
            ListState::Initial,
            move |state| {
                let prefix = prefix.clone();
                async move {
                    self.list_profiles_page(&prefix, state).await
                }
            }
        )
        .flat_map(|result| {
            match result {
                Ok(profiles) => stream::iter(profiles.into_iter().map(Ok)).boxed(),
                Err(e) => stream::once(async move { Err(e) }).boxed(),
            }
        })
    }

    async fn list_profiles_page(
        &self,
        prefix: &str,
        state: ListState,
    ) -> Option<(Result<Vec<String>, BackendError>, ListState)> {
        let continuation_token = match state {
            ListState::Initial => None,
            ListState::Continue(token) => Some(token),
            ListState::Done => return None,
        };

        let mut request = self.client
            .list_objects_v2()
            .bucket(&self.config.bucket)
            .prefix(prefix);

        if let Some(token) = continuation_token {
            request = request.continuation_token(token);
        }

        let result = request.send().await;

        match result {
            Ok(output) => {
                let profiles: Vec<String> = output
                    .contents()
                    .iter()
                    .filter_map(|obj| obj.key())
                    .filter_map(|key| {
                        // Extract profile name from key like "app/profile.yml"
                        let filename = key.strip_prefix(prefix)?;
                        let (name, _ext) = filename.rsplit_once('.')?;
                        Some(name.to_string())
                    })
                    .collect();

                let next_state = if output.is_truncated() == Some(true) {
                    output
                        .next_continuation_token()
                        .map(|t| ListState::Continue(t.to_string()))
                        .unwrap_or(ListState::Done)
                } else {
                    ListState::Done
                };

                Some((Ok(profiles), next_state))
            }
            Err(e) => {
                Some((Err(BackendError::S3Error(e.to_string())), ListState::Done))
            }
        }
    }
}
```

### Paso 3: Implementar Versionado

```rust
// src/s3/versioning.rs
use chrono::{DateTime, Utc};

/// Metadata about a configuration version.
#[derive(Debug, Clone)]
pub struct ConfigVersion {
    /// S3 version ID.
    pub version_id: String,

    /// When this version was created.
    pub last_modified: DateTime<Utc>,

    /// Size in bytes.
    pub size: i64,

    /// Whether this is the current version.
    pub is_latest: bool,

    /// ETag (MD5 hash) of the content.
    pub etag: String,
}

impl S3ConfigSource {
    /// Gets a specific version of a configuration.
    pub async fn get_config_version(
        &self,
        app: &str,
        profiles: &[String],
        version_id: &str,
    ) -> Result<ConfigMap, BackendError> {
        let mut property_sources = Vec::new();

        // Load default profile version
        if let Some(source) = self
            .load_config_version(app, "default", version_id)
            .await?
        {
            property_sources.push(source);
        }

        // Load each profile with same version
        for profile in profiles {
            if profile != "default" {
                if let Some(source) = self
                    .load_config_version(app, profile, version_id)
                    .await?
                {
                    property_sources.push(source);
                }
            }
        }

        property_sources.reverse();

        Ok(ConfigMap {
            name: app.to_string(),
            profiles: profiles.to_vec(),
            label: None,
            version: Some(version_id.to_string()),
            state: None,
            property_sources,
        })
    }

    async fn load_config_version(
        &self,
        app: &str,
        profile: &str,
        version_id: &str,
    ) -> Result<Option<PropertySource>, BackendError> {
        let formats = ["yml", "yaml", "json", "properties"];

        for format in formats {
            let key = self.config.build_key(app, profile, format);

            let result = self.client
                .get_object()
                .bucket(&self.config.bucket)
                .key(&key)
                .version_id(version_id)
                .send()
                .await;

            match result {
                Ok(output) => {
                    let bytes = output.body
                        .collect()
                        .await
                        .map_err(|e| BackendError::S3Error(e.to_string()))?
                        .into_bytes()
                        .to_vec();

                    let source = self.parse_config(&key, &bytes, format)?;
                    return Ok(Some(source));
                }
                Err(e) if is_not_found(&e) => continue,
                Err(e) => return Err(BackendError::S3Error(e.to_string())),
            }
        }

        Ok(None)
    }

    /// Lists all versions of a configuration file.
    pub fn list_versions(
        &self,
        app: &str,
        profile: &str,
    ) -> impl Stream<Item = Result<ConfigVersion, BackendError>> + '_ {
        // Find the key first (we need to know the format)
        let app = app.to_string();
        let profile = profile.to_string();

        stream::once(async move {
            self.find_config_key(&app, &profile).await
        })
        .flat_map(move |key_result| {
            match key_result {
                Ok(Some(key)) => {
                    self.list_versions_for_key(&key).boxed()
                }
                Ok(None) => {
                    stream::empty().boxed()
                }
                Err(e) => {
                    stream::once(async move { Err(e) }).boxed()
                }
            }
        })
    }

    fn list_versions_for_key(
        &self,
        key: &str,
    ) -> impl Stream<Item = Result<ConfigVersion, BackendError>> + '_ {
        let key = key.to_string();

        stream::unfold(
            VersionListState::Initial,
            move |state| {
                let key = key.clone();
                async move {
                    self.list_versions_page(&key, state).await
                }
            }
        )
        .flat_map(|result| {
            match result {
                Ok(versions) => stream::iter(versions.into_iter().map(Ok)).boxed(),
                Err(e) => stream::once(async move { Err(e) }).boxed(),
            }
        })
    }

    async fn list_versions_page(
        &self,
        key: &str,
        state: VersionListState,
    ) -> Option<(Result<Vec<ConfigVersion>, BackendError>, VersionListState)> {
        let (key_marker, version_marker) = match state {
            VersionListState::Initial => (None, None),
            VersionListState::Continue { key_marker, version_marker } => {
                (Some(key_marker), Some(version_marker))
            }
            VersionListState::Done => return None,
        };

        let mut request = self.client
            .list_object_versions()
            .bucket(&self.config.bucket)
            .prefix(key);

        if let Some(km) = key_marker {
            request = request.key_marker(km);
        }
        if let Some(vm) = version_marker {
            request = request.version_id_marker(vm);
        }

        let result = request.send().await;

        match result {
            Ok(output) => {
                let versions: Vec<ConfigVersion> = output
                    .versions()
                    .iter()
                    .filter(|v| v.key() == Some(key))
                    .filter_map(|v| {
                        Some(ConfigVersion {
                            version_id: v.version_id()?.to_string(),
                            last_modified: DateTime::from_timestamp(
                                v.last_modified()?.secs(),
                                0,
                            )?,
                            size: v.size().unwrap_or(0),
                            is_latest: v.is_latest(),
                            etag: v.e_tag()
                                .unwrap_or_default()
                                .trim_matches('"')
                                .to_string(),
                        })
                    })
                    .collect();

                let next_state = if output.is_truncated() == Some(true) {
                    match (output.next_key_marker(), output.next_version_id_marker()) {
                        (Some(km), Some(vm)) => VersionListState::Continue {
                            key_marker: km.to_string(),
                            version_marker: vm.to_string(),
                        },
                        _ => VersionListState::Done,
                    }
                } else {
                    VersionListState::Done
                };

                Some((Ok(versions), next_state))
            }
            Err(e) => {
                Some((Err(BackendError::S3Error(e.to_string())), VersionListState::Done))
            }
        }
    }

    async fn find_config_key(&self, app: &str, profile: &str) -> Result<Option<String>, BackendError> {
        let formats = ["yml", "yaml", "json", "properties"];

        for format in formats {
            let key = self.config.build_key(app, profile, format);

            let result = self.client
                .head_object()
                .bucket(&self.config.bucket)
                .key(&key)
                .send()
                .await;

            if result.is_ok() {
                return Ok(Some(key));
            }
        }

        Ok(None)
    }
}

enum VersionListState {
    Initial,
    Continue { key_marker: String, version_marker: String },
    Done,
}
```

### Paso 4: Implementar Helper para Pagination

```rust
// src/s3/pagination.rs
use futures::stream::Stream;
use pin_project::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};

/// A paginated stream that collects all results.
#[pin_project]
pub struct CollectAll<S> {
    #[pin]
    stream: S,
    results: Vec<S::Item>,
}

impl<S: Stream> CollectAll<S> {
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            results: Vec::new(),
        }
    }
}

/// Extension trait for paginated streams.
pub trait PaginatedStreamExt: Stream + Sized {
    /// Collects all items from a paginated stream.
    fn collect_all(self) -> CollectAll<Self> {
        CollectAll::new(self)
    }

    /// Takes at most `n` items from the stream.
    fn take_n(self, n: usize) -> impl Stream<Item = Self::Item> {
        futures::stream::StreamExt::take(self, n)
    }
}

impl<S: Stream> PaginatedStreamExt for S {}
```

---

## Conceptos de Rust Aprendidos

### 1. Async Iterators (Streams)

Rust usa `Stream` trait de futures para iteradores asincronos.

**Rust:**
```rust
use futures::stream::{self, Stream, StreamExt};

// Crear un stream desde un unfold (lazy generation)
fn paginated_list() -> impl Stream<Item = Result<String, Error>> {
    stream::unfold(
        PaginationState::Initial,
        |state| async move {
            // Return None to end the stream
            // Return Some((items, next_state)) to continue
            match fetch_page(state).await {
                Ok((items, next)) => Some((Ok(items), next)),
                Err(e) => Some((Err(e), PaginationState::Done)),
            }
        }
    )
    .flat_map(|result| {
        // Flatten Vec<Item> into individual items
        match result {
            Ok(items) => stream::iter(items.into_iter().map(Ok)),
            Err(e) => stream::once(async { Err(e) }),
        }
    })
}

// Consumir el stream
async fn consume() {
    let mut stream = paginated_list();

    // Option 1: Manual iteration
    while let Some(result) = stream.next().await {
        match result {
            Ok(item) => println!("Got: {}", item),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    // Option 2: Collect all
    let all: Vec<_> = paginated_list().collect().await;

    // Option 3: Take first N
    let first_10: Vec<_> = paginated_list().take(10).collect().await;
}
```

**Comparacion con Java Reactive (Flux):**
```java
import reactor.core.publisher.Flux;

// Crear un Flux paginado
Flux<String> paginatedList() {
    return Flux.generate(
        () -> new PaginationState(),
        (state, sink) -> {
            if (state.isDone()) {
                sink.complete();
                return state;
            }

            Page page = fetchPage(state);
            page.getItems().forEach(sink::next);

            return page.getNextState();
        }
    );
}

// Consumir el Flux
paginatedList()
    .take(10)
    .collectList()
    .subscribe(items -> System.out.println("Got " + items.size()));
```

**Diferencias clave:**
| Aspecto | Rust Stream | Java Flux |
|---------|-------------|-----------|
| Tipo | Trait `Stream<Item = T>` | Class `Flux<T>` |
| Error handling | `Item = Result<T, E>` | onError callback |
| Backpressure | Implicit (pull-based) | Explicit (request-n) |
| Operators | StreamExt trait | Fluent API |
| Memory | Zero alloc generators | Object allocations |

### 2. Pagination con Unfold

`stream::unfold` crea streams lazy desde una funcion generadora.

**Rust:**
```rust
use futures::stream::{self, Stream};

enum State {
    Initial,
    Continue(String),  // Continuation token
    Done,
}

fn paginate<'a>(
    client: &'a Client,
) -> impl Stream<Item = Vec<String>> + 'a {
    stream::unfold(State::Initial, move |state| async move {
        match state {
            State::Done => None,  // End of stream
            State::Initial => {
                let (items, token) = client.list_first_page().await;
                let next = match token {
                    Some(t) => State::Continue(t),
                    None => State::Done,
                };
                Some((items, next))
            }
            State::Continue(token) => {
                let (items, next_token) = client.list_next_page(&token).await;
                let next = match next_token {
                    Some(t) => State::Continue(t),
                    None => State::Done,
                };
                Some((items, next))
            }
        }
    })
}
```

**Comparacion conceptual con Java:**
```java
// Java no tiene equivalente directo, se usa iterador personalizado
public class PaginatedIterator implements Iterator<Page> {
    private String continuationToken = null;
    private boolean done = false;

    @Override
    public boolean hasNext() {
        return !done;
    }

    @Override
    public Page next() {
        Page page = continuationToken == null
            ? client.listFirstPage()
            : client.listNextPage(continuationToken);

        continuationToken = page.getNextToken();
        done = continuationToken == null;

        return page;
    }
}
```

### 3. Stream Combinators

**Rust:**
```rust
use futures::stream::StreamExt;

async fn process_stream() {
    let stream = list_applications();

    // flat_map: transform and flatten
    let profiles = stream.flat_map(|app| {
        list_profiles(&app)
    });

    // filter: keep only matching items
    let production = profiles.filter(|p| {
        futures::future::ready(p.contains("prod"))
    });

    // map: transform items
    let upper = production.map(|p| p.to_uppercase());

    // buffer_unordered: concurrent processing
    let results = upper.buffer_unordered(10);

    // collect: gather all results
    let all: Vec<String> = results.collect().await;
}
```

### 4. Lifetime Annotations en Streams

**Rust:**
```rust
impl S3ConfigSource {
    // El lifetime 'a indica que el stream no puede vivir
    // mas que &self
    pub fn list_applications(&self) -> impl Stream<Item = Result<String, Error>> + '_ {
        //                                                                          ^^
        // '_ es azucar sintactico para el lifetime de &self

        let bucket = self.config.bucket.clone();  // Clone para evitar lifetime issues

        stream::unfold(State::Initial, move |state| {
            // 'move' captura bucket por valor
            let bucket = bucket.clone();
            async move {
                // ...
            }
        })
    }
}

// Alternativa: retornar Box<dyn Stream>
pub fn list_boxed(&self) -> Pin<Box<dyn Stream<Item = String> + Send + '_>> {
    Box::pin(stream::iter(vec!["a", "b", "c"].into_iter().map(String::from)))
}
```

---

## Riesgos y Errores Comunes

### 1. No Manejar Truncation

```rust
// MAL: Solo obtiene primera pagina
async fn list_all_bad(client: &Client, bucket: &str) -> Vec<String> {
    let output = client.list_objects_v2()
        .bucket(bucket)
        .send()
        .await
        .unwrap();

    output.contents()
        .iter()
        .filter_map(|o| o.key().map(String::from))
        .collect()
    // Si hay mas de 1000 objetos, solo obtiene los primeros!
}

// BIEN: Usar pagination
fn list_all_good(client: &Client, bucket: &str) -> impl Stream<Item = String> + '_ {
    stream::unfold(None, move |token| async move {
        let mut req = client.list_objects_v2().bucket(bucket);
        if let Some(t) = token {
            req = req.continuation_token(t);
        }

        let output = req.send().await.ok()?;

        let items: Vec<String> = output.contents()
            .iter()
            .filter_map(|o| o.key().map(String::from))
            .collect();

        let next = if output.is_truncated() == Some(true) {
            output.next_continuation_token().map(String::from)
        } else {
            None
        };

        if items.is_empty() && next.is_none() {
            None
        } else {
            Some((stream::iter(items), next))
        }
    })
    .flatten()
}
```

### 2. Versioning No Habilitado

```rust
// MAL: Asume que versioning esta habilitado
async fn get_version_bad(client: &Client, key: &str, version: &str) {
    let _ = client.get_object()
        .bucket("bucket")
        .key(key)
        .version_id(version)  // Falla si versioning no esta habilitado
        .send()
        .await;
}

// BIEN: Verificar primero
async fn get_version_good(client: &Client, bucket: &str, key: &str, version: &str)
    -> Result<Option<Vec<u8>>, Error>
{
    // Check if versioning is enabled
    let versioning = client.get_bucket_versioning()
        .bucket(bucket)
        .send()
        .await?;

    if versioning.status() != Some(&BucketVersioningStatus::Enabled) {
        return Err(Error::VersioningNotEnabled);
    }

    // Now safe to use version_id
    let output = client.get_object()
        .bucket(bucket)
        .key(key)
        .version_id(version)
        .send()
        .await?;

    Ok(Some(output.body.collect().await?.to_vec()))
}
```

### 3. Memory Explosion con collect()

```rust
// MAL: Puede consumir toda la memoria
async fn dangerous() {
    let all: Vec<_> = huge_stream().collect().await;  // Millones de items en memoria
}

// BIEN: Procesar en chunks o usar take()
async fn safe() {
    // Option 1: Process in batches
    let mut stream = huge_stream();
    while let Some(batch) = stream.next().await {
        process_batch(batch);
    }

    // Option 2: Limit results
    let first_1000: Vec<_> = huge_stream().take(1000).collect().await;

    // Option 3: Use for_each
    huge_stream()
        .for_each(|item| async {
            process_item(item);
        })
        .await;
}
```

---

## Pruebas

### Tests Unitarios

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_version_from_s3_object_version() {
        let version = ConfigVersion {
            version_id: "abc123".to_string(),
            last_modified: Utc::now(),
            size: 1024,
            is_latest: true,
            etag: "d41d8cd98f00b204e9800998ecf8427e".to_string(),
        };

        assert!(version.is_latest);
        assert_eq!(version.size, 1024);
    }

    #[tokio::test]
    async fn list_state_transitions() {
        let state = ListState::Initial;

        // Simulate pagination
        let state = ListState::Continue("token1".to_string());
        match state {
            ListState::Continue(token) => assert_eq!(token, "token1"),
            _ => panic!("Expected Continue state"),
        }
    }
}
```

### Tests de Integracion

```rust
// tests/s3_versioning_test.rs
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::localstack::LocalStack;
use futures::StreamExt;

#[tokio::test]
async fn list_applications_returns_all_apps() {
    let container = LocalStack::default().start().await;
    let endpoint = get_endpoint(&container).await;

    let client = create_test_client(&endpoint).await;

    // Setup: create bucket with multiple apps
    create_bucket(&client, "test-bucket").await;
    upload_object(&client, "test-bucket", "app1/default.yml", b"key: value1").await;
    upload_object(&client, "test-bucket", "app2/default.yml", b"key: value2").await;
    upload_object(&client, "test-bucket", "app3/default.yml", b"key: value3").await;

    // Test
    let config = S3Config::new("test-bucket").with_endpoint(&endpoint);
    let source = S3ConfigSource::new(config).await.unwrap();

    let apps: Vec<String> = source.list_applications()
        .filter_map(|r| async { r.ok() })
        .collect()
        .await;

    assert_eq!(apps.len(), 3);
    assert!(apps.contains(&"app1".to_string()));
    assert!(apps.contains(&"app2".to_string()));
    assert!(apps.contains(&"app3".to_string()));
}

#[tokio::test]
async fn list_versions_returns_version_history() {
    let container = LocalStack::default().start().await;
    let endpoint = get_endpoint(&container).await;

    let client = create_test_client(&endpoint).await;

    // Setup: enable versioning
    create_bucket(&client, "versioned-bucket").await;
    enable_versioning(&client, "versioned-bucket").await;

    // Upload multiple versions
    upload_object(&client, "versioned-bucket", "app/prod.yml", b"v1").await;
    upload_object(&client, "versioned-bucket", "app/prod.yml", b"v2").await;
    upload_object(&client, "versioned-bucket", "app/prod.yml", b"v3").await;

    // Test
    let config = S3Config::new("versioned-bucket").with_endpoint(&endpoint);
    let source = S3ConfigSource::new(config).await.unwrap();

    let versions: Vec<ConfigVersion> = source.list_versions("app", "prod")
        .filter_map(|r| async { r.ok() })
        .collect()
        .await;

    assert_eq!(versions.len(), 3);
    assert!(versions.iter().any(|v| v.is_latest));
}

#[tokio::test]
async fn pagination_handles_many_objects() {
    let container = LocalStack::default().start().await;
    let endpoint = get_endpoint(&container).await;

    let client = create_test_client(&endpoint).await;
    create_bucket(&client, "large-bucket").await;

    // Create 1500 objects (more than one page of 1000)
    for i in 0..1500 {
        let key = format!("app{}/default.yml", i);
        upload_object(&client, "large-bucket", &key, b"key: value").await;
    }

    let config = S3Config::new("large-bucket").with_endpoint(&endpoint);
    let source = S3ConfigSource::new(config).await.unwrap();

    let count = source.list_applications()
        .filter_map(|r| async { r.ok() })
        .count()
        .await;

    assert_eq!(count, 1500);
}
```

---

## Observabilidad

### Logging

```rust
impl S3ConfigSource {
    pub fn list_applications(&self) -> impl Stream<...> + '_ {
        tracing::info!(
            bucket = %self.config.bucket,
            "Starting to list applications"
        );

        // ... stream implementation ...
    }

    async fn list_applications_page(&self, state: ListState) -> ... {
        let span = tracing::debug_span!(
            "s3_list_page",
            bucket = %self.config.bucket,
            has_continuation = state.has_token()
        );

        async {
            tracing::debug!("Fetching page");
            // ...
            tracing::debug!(items = items.len(), "Page fetched");
        }
        .instrument(span)
        .await
    }
}
```

### Metricas

```rust
// Suggested metrics
s3_list_requests_total{bucket, operation}
s3_list_pages_total{bucket}
s3_list_items_total{bucket}
s3_versions_fetched_total{bucket}
```

---

## Entregable Final

### Archivos Creados/Modificados

1. `crates/vortex-backends/src/s3/listing.rs` - Listing de apps y profiles
2. `crates/vortex-backends/src/s3/versioning.rs` - Soporte de versiones
3. `crates/vortex-backends/src/s3/pagination.rs` - Helpers de pagination
4. `crates/vortex-backends/tests/s3_versioning_test.rs` - Tests

### Verificacion

```bash
# Compilar
cargo build -p vortex-backends --features s3

# Tests
cargo test -p vortex-backends --features s3

# Test especifico de versioning
cargo test -p vortex-backends --features s3 versioning

# Con Docker para LocalStack
docker run -d -p 4566:4566 localstack/localstack
cargo test -p vortex-backends --features s3 --test s3_versioning_test
```

### Ejemplo de Uso

```rust
use futures::StreamExt;
use vortex_backends::s3::{S3Config, S3ConfigSource};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = S3Config::new("my-config-bucket")
        .with_region("us-east-1");

    let source = S3ConfigSource::new(config).await?;

    // List all applications
    println!("Applications:");
    let mut apps = source.list_applications();
    while let Some(app) = apps.next().await {
        println!("  - {}", app?);
    }

    // List profiles for an app
    println!("\nProfiles for payment-service:");
    let mut profiles = source.list_profiles("payment-service");
    while let Some(profile) = profiles.next().await {
        println!("  - {}", profile?);
    }

    // List versions of a config
    println!("\nVersions of payment-service/production:");
    let mut versions = source.list_versions("payment-service", "production");
    while let Some(version) = versions.next().await {
        let v = version?;
        println!("  - {} ({})", v.version_id,
            if v.is_latest { "current" } else { "old" });
    }

    // Get specific version
    let old_config = source
        .get_config_version("payment-service", &["production".into()], "abc123")
        .await?;

    println!("\nLoaded version {} with {} sources",
        old_config.version.unwrap_or_default(),
        old_config.property_sources.len());

    Ok(())
}
```
