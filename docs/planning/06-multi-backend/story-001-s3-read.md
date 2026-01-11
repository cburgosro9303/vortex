# Historia 001: Backend S3 - Lectura

## Contexto y Objetivo

Esta historia implementa la capacidad de leer configuraciones almacenadas en Amazon S3 o servicios compatibles (MinIO, LocalStack). S3 es una opcion popular para almacenar configuraciones en entornos cloud por su durabilidad, disponibilidad y bajo costo.

**Casos de uso principales:**
- Organizaciones que ya usan S3 para almacenamiento
- Configuraciones que se generan via CI/CD y se publican a S3
- Entornos donde Git no es practico (edge computing, IoT)
- Integracion con pipelines de AWS existentes

El backend S3 implementara el trait `ConfigSource` definido en la Epica 04, permitiendo que sea intercambiable con otros backends.

---

## Alcance

### In Scope

- Cliente S3 configurado con aws-sdk-s3
- Lectura de archivos de configuracion desde S3
- Soporte para credenciales via environment/IAM
- Estructura de paths: `s3://{bucket}/{app}/{profile}.{format}`
- Formatos soportados: YAML, JSON, Properties
- Retry automatico con backoff exponencial
- Configuracion de endpoint para MinIO/LocalStack

### Out of Scope

- Escritura a S3 (read-only backend)
- Versionado de objetos S3 (historia 002)
- Listado de configuraciones (historia 002)
- Encriptacion client-side
- S3 Select queries

---

## Criterios de Aceptacion

- [ ] `S3ConfigSource` implementa trait `ConfigSource`
- [ ] Lee archivos YAML/JSON/Properties desde S3
- [ ] Soporta credenciales via `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY`
- [ ] Soporta IAM roles automaticamente
- [ ] Endpoint configurable para MinIO/LocalStack
- [ ] Retry con backoff exponencial (3 intentos)
- [ ] Timeout configurable (default 30s)
- [ ] Maneja gracefully objetos no existentes (404)
- [ ] Logs estructurados de operaciones S3

---

## Diseno Propuesto

### Estructura de Paths en S3

```
s3://my-config-bucket/
├── payment-service/
│   ├── default.yml
│   ├── dev.yml
│   ├── staging.yml
│   └── production.yml
├── user-service/
│   ├── default.json
│   └── production.json
└── shared/
    └── common.properties
```

### Interfaces Principales

```rust
// src/s3/config.rs
pub struct S3Config {
    pub bucket: String,
    pub region: Option<String>,
    pub endpoint_url: Option<String>,  // Para MinIO
    pub path_prefix: Option<String>,
    pub timeout: Duration,
    pub max_retries: u32,
}

// src/s3/source.rs
pub struct S3ConfigSource {
    client: aws_sdk_s3::Client,
    config: S3Config,
}

impl ConfigSource for S3ConfigSource {
    async fn get_config(
        &self,
        app: &str,
        profiles: &[String],
        label: Option<&str>,
    ) -> Result<ConfigMap, ConfigError>;
}
```

### Diagrama de Flujo

```
┌──────────────────┐
│  get_config()    │
│  app: "payment"  │
│  profiles: [dev] │
└────────┬─────────┘
         │
         ▼
┌──────────────────────────────────┐
│  Build S3 keys:                  │
│  - payment/default.yml           │
│  - payment/dev.yml               │
└────────┬─────────────────────────┘
         │
         ▼
┌──────────────────────────────────┐
│  For each key (parallel):        │
│  ┌─────────────────────────────┐ │
│  │ client.get_object(key)     │ │
│  │ with retry + backoff       │ │
│  └─────────────────────────────┘ │
└────────┬─────────────────────────┘
         │
         ▼
┌──────────────────────────────────┐
│  Parse response:                 │
│  - .yml/.yaml → serde_yaml       │
│  - .json → serde_json            │
│  - .properties → custom parser   │
└────────┬─────────────────────────┘
         │
         ▼
┌──────────────────────────────────┐
│  Merge configs:                  │
│  dev.yml overrides default.yml   │
└────────┬─────────────────────────┘
         │
         ▼
┌──────────────────────────────────┐
│  Return ConfigMap                │
└──────────────────────────────────┘
```

---

## Pasos de Implementacion

### Paso 1: Agregar Dependencias

```toml
# crates/vortex-backends/Cargo.toml
[dependencies]
aws-sdk-s3 = { version = "1.0", optional = true }
aws-config = { version = "1.0", optional = true }
aws-credential-types = { version = "1.0", optional = true }

[features]
s3 = ["aws-sdk-s3", "aws-config", "aws-credential-types"]
```

### Paso 2: Implementar S3Config

```rust
// src/s3/config.rs
use std::time::Duration;

/// Configuration for S3 backend.
#[derive(Debug, Clone)]
pub struct S3Config {
    /// S3 bucket name.
    pub bucket: String,

    /// AWS region (e.g., "us-east-1").
    /// If not specified, uses AWS_REGION or defaults.
    pub region: Option<String>,

    /// Custom endpoint URL for S3-compatible services.
    /// Use this for MinIO, LocalStack, etc.
    pub endpoint_url: Option<String>,

    /// Optional path prefix within the bucket.
    /// Example: "configs/" would look for "configs/app/profile.yml"
    pub path_prefix: Option<String>,

    /// Request timeout.
    pub timeout: Duration,

    /// Maximum retry attempts for failed requests.
    pub max_retries: u32,
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            bucket: String::new(),
            region: None,
            endpoint_url: None,
            path_prefix: None,
            timeout: Duration::from_secs(30),
            max_retries: 3,
        }
    }
}

impl S3Config {
    /// Creates a new S3Config with the given bucket.
    pub fn new(bucket: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            ..Default::default()
        }
    }

    /// Sets a custom endpoint for S3-compatible services.
    pub fn with_endpoint(mut self, url: impl Into<String>) -> Self {
        self.endpoint_url = Some(url.into());
        self
    }

    /// Sets the AWS region.
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Builds the S3 key for a given app and profile.
    pub fn build_key(&self, app: &str, profile: &str, format: &str) -> String {
        let base = match &self.path_prefix {
            Some(prefix) => format!("{}/{}", prefix.trim_end_matches('/'), app),
            None => app.to_string(),
        };
        format!("{}/{}.{}", base, profile, format)
    }
}
```

### Paso 3: Implementar Cliente S3

```rust
// src/s3/client.rs
use aws_sdk_s3::Client;
use aws_config::BehaviorVersion;
use crate::error::BackendError;
use super::config::S3Config;

/// Creates an S3 client from configuration.
pub async fn create_client(config: &S3Config) -> Result<Client, BackendError> {
    let mut aws_config = aws_config::defaults(BehaviorVersion::latest());

    // Set region if specified
    if let Some(region) = &config.region {
        aws_config = aws_config.region(
            aws_sdk_s3::config::Region::new(region.clone())
        );
    }

    let sdk_config = aws_config.load().await;

    let mut s3_config = aws_sdk_s3::config::Builder::from(&sdk_config);

    // Set custom endpoint for MinIO/LocalStack
    if let Some(endpoint) = &config.endpoint_url {
        s3_config = s3_config
            .endpoint_url(endpoint)
            .force_path_style(true);  // Required for MinIO
    }

    Ok(Client::from_conf(s3_config.build()))
}

/// Wrapper around S3 client with retry logic.
pub struct S3ClientWrapper {
    client: Client,
    max_retries: u32,
}

impl S3ClientWrapper {
    pub fn new(client: Client, max_retries: u32) -> Self {
        Self { client, max_retries }
    }

    /// Gets an object from S3 with automatic retry.
    pub async fn get_object(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<Option<Vec<u8>>, BackendError> {
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < self.max_retries {
            match self.try_get_object(bucket, key).await {
                Ok(data) => return Ok(data),
                Err(e) if e.is_retryable() => {
                    attempts += 1;
                    last_error = Some(e);

                    // Exponential backoff: 100ms, 200ms, 400ms...
                    let delay = Duration::from_millis(100 * 2_u64.pow(attempts - 1));
                    tracing::warn!(
                        attempt = attempts,
                        delay_ms = delay.as_millis(),
                        "S3 request failed, retrying"
                    );
                    tokio::time::sleep(delay).await;
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            BackendError::S3Error("Max retries exceeded".to_string())
        }))
    }

    async fn try_get_object(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<Option<Vec<u8>>, BackendError> {
        use aws_sdk_s3::error::SdkError;

        let result = self.client
            .get_object()
            .bucket(bucket)
            .key(key)
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
                Ok(Some(bytes))
            }
            Err(SdkError::ServiceError(err))
                if err.err().is_no_such_key() => {
                Ok(None)  // Object not found is not an error
            }
            Err(e) => Err(BackendError::S3Error(e.to_string())),
        }
    }
}
```

### Paso 4: Implementar S3ConfigSource

```rust
// src/s3/source.rs
use async_trait::async_trait;
use crate::traits::ConfigSource;
use crate::error::BackendError;
use crate::types::{ConfigMap, PropertySource};
use super::client::S3ClientWrapper;
use super::config::S3Config;

/// S3-backed configuration source.
pub struct S3ConfigSource {
    client: S3ClientWrapper,
    config: S3Config,
}

impl S3ConfigSource {
    /// Creates a new S3ConfigSource.
    pub async fn new(config: S3Config) -> Result<Self, BackendError> {
        let client = super::client::create_client(&config).await?;
        let wrapper = S3ClientWrapper::new(client, config.max_retries);

        Ok(Self {
            client: wrapper,
            config,
        })
    }

    /// Attempts to load a config file from S3.
    async fn load_config(
        &self,
        app: &str,
        profile: &str,
    ) -> Result<Option<PropertySource>, BackendError> {
        // Try formats in order: yml, yaml, json, properties
        let formats = ["yml", "yaml", "json", "properties"];

        for format in formats {
            let key = self.config.build_key(app, profile, format);

            tracing::debug!(key = %key, "Attempting to load config from S3");

            if let Some(data) = self.client
                .get_object(&self.config.bucket, &key)
                .await?
            {
                let source = self.parse_config(&key, &data, format)?;
                return Ok(Some(source));
            }
        }

        Ok(None)
    }

    /// Parses config data based on format.
    fn parse_config(
        &self,
        name: &str,
        data: &[u8],
        format: &str,
    ) -> Result<PropertySource, BackendError> {
        let content = std::str::from_utf8(data)
            .map_err(|e| BackendError::ParseError(e.to_string()))?;

        let properties = match format {
            "yml" | "yaml" => self.parse_yaml(content)?,
            "json" => self.parse_json(content)?,
            "properties" => self.parse_properties(content)?,
            _ => return Err(BackendError::ParseError(
                format!("Unknown format: {}", format)
            )),
        };

        Ok(PropertySource {
            name: format!("s3:{}/{}", self.config.bucket, name),
            source: properties,
        })
    }

    fn parse_yaml(&self, content: &str) -> Result<serde_json::Map<String, serde_json::Value>, BackendError> {
        let value: serde_json::Value = serde_yaml::from_str(content)
            .map_err(|e| BackendError::ParseError(e.to_string()))?;

        self.flatten_value(value)
    }

    fn parse_json(&self, content: &str) -> Result<serde_json::Map<String, serde_json::Value>, BackendError> {
        let value: serde_json::Value = serde_json::from_str(content)
            .map_err(|e| BackendError::ParseError(e.to_string()))?;

        self.flatten_value(value)
    }

    fn parse_properties(&self, content: &str) -> Result<serde_json::Map<String, serde_json::Value>, BackendError> {
        let mut map = serde_json::Map::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = trimmed.split_once('=') {
                map.insert(
                    key.trim().to_string(),
                    serde_json::Value::String(value.trim().to_string()),
                );
            }
        }

        Ok(map)
    }

    /// Flattens nested JSON into dotted keys.
    fn flatten_value(
        &self,
        value: serde_json::Value,
    ) -> Result<serde_json::Map<String, serde_json::Value>, BackendError> {
        let mut result = serde_json::Map::new();
        self.flatten_recursive(&value, String::new(), &mut result);
        Ok(result)
    }

    fn flatten_recursive(
        &self,
        value: &serde_json::Value,
        prefix: String,
        result: &mut serde_json::Map<String, serde_json::Value>,
    ) {
        match value {
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    let new_key = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{}.{}", prefix, k)
                    };
                    self.flatten_recursive(v, new_key, result);
                }
            }
            _ => {
                result.insert(prefix, value.clone());
            }
        }
    }
}

#[async_trait]
impl ConfigSource for S3ConfigSource {
    async fn get_config(
        &self,
        app: &str,
        profiles: &[String],
        _label: Option<&str>,  // S3 doesn't use labels (use versioning instead)
    ) -> Result<ConfigMap, BackendError> {
        let mut property_sources = Vec::new();

        // Load default profile first
        if let Some(source) = self.load_config(app, "default").await? {
            property_sources.push(source);
        }

        // Load each profile (later profiles override earlier)
        for profile in profiles {
            if profile != "default" {
                if let Some(source) = self.load_config(app, profile).await? {
                    property_sources.push(source);
                }
            }
        }

        // Reverse so higher priority (later profiles) come first
        property_sources.reverse();

        Ok(ConfigMap {
            name: app.to_string(),
            profiles: profiles.to_vec(),
            label: None,
            version: None,
            state: None,
            property_sources,
        })
    }

    fn name(&self) -> &str {
        "s3"
    }
}
```

### Paso 5: Implementar Errores

```rust
// src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BackendError {
    #[error("S3 error: {0}")]
    S3Error(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Configuration not found: {0}")]
    NotFound(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),
}

impl BackendError {
    /// Returns true if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(self,
            BackendError::S3Error(_) |
            BackendError::ConnectionError(_)
        )
    }
}
```

---

## Conceptos de Rust Aprendidos

### 1. AWS SDK para Rust

El AWS SDK para Rust es async-first y type-safe.

**Rust:**
```rust
use aws_sdk_s3::Client;
use aws_config::BehaviorVersion;

// Crear cliente con configuracion automatica
async fn create_client() -> Client {
    let config = aws_config::defaults(BehaviorVersion::latest())
        .load()
        .await;
    Client::new(&config)
}

// Operacion get_object con builder pattern
async fn get_object(client: &Client, bucket: &str, key: &str) {
    let result = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await;

    match result {
        Ok(output) => {
            let bytes = output.body.collect().await.unwrap();
            println!("Got {} bytes", bytes.len());
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

**Comparacion con Java (AWS SDK v2):**
```java
import software.amazon.awssdk.services.s3.S3Client;
import software.amazon.awssdk.services.s3.model.GetObjectRequest;

// Cliente sincrono por defecto
S3Client client = S3Client.builder()
    .region(Region.US_EAST_1)
    .build();

// Operacion get_object
GetObjectRequest request = GetObjectRequest.builder()
    .bucket(bucket)
    .key(key)
    .build();

ResponseBytes<GetObjectResponse> response =
    client.getObjectAsBytes(request);

byte[] bytes = response.asByteArray();
```

**Diferencias clave:**
| Aspecto | Rust AWS SDK | Java AWS SDK v2 |
|---------|--------------|-----------------|
| Async | Por defecto | Opcional (async client) |
| Builder | Metodos encadenados | Builder pattern clasico |
| Errores | Result<T, SdkError> | Excepciones |
| Credenciales | Chain automatica | Chain automatica |
| Memoria | Zero-copy streams | Buffering |

### 2. Async Streams y ByteStream

El SDK usa streams asincrono para bodies de respuesta.

**Rust:**
```rust
use aws_sdk_s3::primitives::ByteStream;

async fn read_body(body: ByteStream) -> Vec<u8> {
    // Collect all chunks into bytes
    let aggregated = body
        .collect()
        .await
        .expect("Failed to read body");

    // Convert to Vec<u8>
    aggregated.into_bytes().to_vec()
}

// Streaming chunk by chunk (for large files)
async fn stream_body(body: ByteStream) {
    use tokio_stream::StreamExt;

    let mut stream = body.into_async_read();
    let mut buffer = [0u8; 8192];

    loop {
        match tokio::io::AsyncReadExt::read(&mut stream, &mut buffer).await {
            Ok(0) => break,  // EOF
            Ok(n) => println!("Read {} bytes", n),
            Err(e) => panic!("Read error: {}", e),
        }
    }
}
```

**Comparacion con Java Reactive:**
```java
// Java con SDK async
S3AsyncClient asyncClient = S3AsyncClient.create();

CompletableFuture<ResponseBytes<GetObjectResponse>> future =
    asyncClient.getObject(request, AsyncResponseTransformer.toBytes());

// O con streaming
asyncClient.getObject(request, AsyncResponseTransformer.toPublisher())
    .thenAccept(publisher -> {
        Flux.from(publisher)
            .doOnNext(buffer -> System.out.println("Got chunk"))
            .blockLast();
    });
```

### 3. Error Handling con SdkError

Los errores del SDK son enums tipados.

**Rust:**
```rust
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::operation::get_object::GetObjectError;

async fn handle_s3_error(
    result: Result<GetObjectOutput, SdkError<GetObjectError>>
) -> Option<Vec<u8>> {
    match result {
        Ok(output) => {
            let bytes = output.body.collect().await.ok()?;
            Some(bytes.into_bytes().to_vec())
        }
        Err(SdkError::ServiceError(service_err)) => {
            // Service-level errors (404, 403, etc.)
            let err = service_err.err();

            if err.is_no_such_key() {
                tracing::debug!("Object not found");
                None
            } else if err.is_no_such_bucket() {
                tracing::error!("Bucket not found");
                None
            } else {
                tracing::error!("S3 service error: {:?}", err);
                None
            }
        }
        Err(SdkError::TimeoutError(_)) => {
            tracing::warn!("Request timed out");
            None
        }
        Err(e) => {
            tracing::error!("Unexpected error: {}", e);
            None
        }
    }
}
```

**Comparacion con Java:**
```java
try {
    ResponseBytes<GetObjectResponse> response =
        client.getObjectAsBytes(request);
    return response.asByteArray();
} catch (NoSuchKeyException e) {
    logger.debug("Object not found");
    return null;
} catch (NoSuchBucketException e) {
    logger.error("Bucket not found");
    return null;
} catch (S3Exception e) {
    logger.error("S3 error: " + e.getMessage());
    throw e;
}
```

### 4. Retry con Backoff Exponencial

**Rust:**
```rust
use std::time::Duration;

async fn with_retry<F, T, E>(
    mut operation: F,
    max_retries: u32,
) -> Result<T, E>
where
    F: FnMut() -> futures::future::BoxFuture<'static, Result<T, E>>,
    E: std::fmt::Debug,
{
    let mut attempts = 0;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if attempts < max_retries => {
                attempts += 1;

                // Exponential backoff: 100ms * 2^attempts
                let delay = Duration::from_millis(100 * 2_u64.pow(attempts));

                tracing::warn!(
                    attempt = attempts,
                    max = max_retries,
                    delay_ms = delay.as_millis(),
                    error = ?e,
                    "Operation failed, retrying"
                );

                tokio::time::sleep(delay).await;
            }
            Err(e) => return Err(e),
        }
    }
}
```

**Comparacion con Java (Resilience4j):**
```java
import io.github.resilience4j.retry.Retry;
import io.github.resilience4j.retry.RetryConfig;

RetryConfig config = RetryConfig.custom()
    .maxAttempts(3)
    .waitDuration(Duration.ofMillis(100))
    .retryOnException(e -> e instanceof S3Exception)
    .build();

Retry retry = Retry.of("s3-retry", config);

Supplier<byte[]> decorated = Retry.decorateSupplier(
    retry,
    () -> client.getObjectAsBytes(request).asByteArray()
);

byte[] result = decorated.get();
```

---

## Riesgos y Errores Comunes

### 1. No Usar force_path_style para MinIO

```rust
// MAL: Falla con MinIO
let s3_config = aws_sdk_s3::config::Builder::from(&sdk_config)
    .endpoint_url("http://localhost:9000")
    .build();

// BIEN: MinIO requiere path-style
let s3_config = aws_sdk_s3::config::Builder::from(&sdk_config)
    .endpoint_url("http://localhost:9000")
    .force_path_style(true)  // Importante!
    .build();
```

### 2. Olvidar Manejar NoSuchKey

```rust
// MAL: Panic si el objeto no existe
let output = client.get_object()
    .bucket("my-bucket")
    .key("missing-key")
    .send()
    .await
    .unwrap();  // Panic!

// BIEN: Manejar 404 gracefully
match client.get_object().bucket("b").key("k").send().await {
    Ok(output) => Some(output),
    Err(SdkError::ServiceError(e)) if e.err().is_no_such_key() => None,
    Err(e) => return Err(e.into()),
}
```

### 3. No Configurar Timeout

```rust
// MAL: Sin timeout puede colgarse indefinidamente
let config = aws_config::defaults(BehaviorVersion::latest())
    .load()
    .await;

// BIEN: Configurar timeout
let config = aws_config::defaults(BehaviorVersion::latest())
    .timeout_config(
        aws_config::timeout::TimeoutConfig::builder()
            .operation_timeout(Duration::from_secs(30))
            .build()
    )
    .load()
    .await;
```

### 4. Leak de Credenciales en Logs

```rust
// MAL: Puede loggear credenciales
tracing::info!("Config: {:?}", aws_config);

// BIEN: Solo loggear informacion segura
tracing::info!(
    bucket = %config.bucket,
    region = ?config.region,
    "Connecting to S3"
);
```

---

## Pruebas

### Tests Unitarios

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s3_config_builds_correct_key() {
        let config = S3Config::new("my-bucket");

        assert_eq!(
            config.build_key("myapp", "dev", "yml"),
            "myapp/dev.yml"
        );
    }

    #[test]
    fn s3_config_with_prefix_builds_correct_key() {
        let config = S3Config::new("my-bucket")
            .with_prefix("configs/");

        assert_eq!(
            config.build_key("myapp", "dev", "yml"),
            "configs/myapp/dev.yml"
        );
    }

    #[test]
    fn parse_yaml_flattens_nested_keys() {
        let source = S3ConfigSource::mock();
        let yaml = r#"
server:
  port: 8080
  host: localhost
"#;
        let result = source.parse_yaml(yaml).unwrap();

        assert_eq!(result["server.port"], 8080);
        assert_eq!(result["server.host"], "localhost");
    }

    #[test]
    fn parse_properties_ignores_comments() {
        let source = S3ConfigSource::mock();
        let props = r#"
# This is a comment
server.port=8080
# Another comment
server.host=localhost
"#;
        let result = source.parse_properties(props).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result["server.port"], "8080");
    }
}
```

### Tests de Integracion (con LocalStack)

```rust
// tests/s3_test.rs
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::localstack::LocalStack;

#[tokio::test]
async fn s3_source_reads_yaml_config() {
    // Start LocalStack container
    let container = LocalStack::default().start().await;
    let endpoint = format!(
        "http://localhost:{}",
        container.get_host_port_ipv4(4566).await
    );

    // Create bucket and upload test file
    let client = create_test_client(&endpoint).await;
    create_bucket(&client, "test-bucket").await;
    upload_object(
        &client,
        "test-bucket",
        "myapp/dev.yml",
        b"server:\n  port: 9090",
    ).await;

    // Test S3ConfigSource
    let config = S3Config::new("test-bucket")
        .with_endpoint(&endpoint)
        .with_region("us-east-1");

    let source = S3ConfigSource::new(config).await.unwrap();
    let result = source.get_config("myapp", &["dev".to_string()], None).await.unwrap();

    assert_eq!(result.name, "myapp");
    assert!(!result.property_sources.is_empty());

    let props = &result.property_sources[0].source;
    assert_eq!(props["server.port"], 9090);
}
```

---

## Observabilidad

### Logging Estructurado

```rust
impl S3ConfigSource {
    async fn load_config(&self, app: &str, profile: &str) -> Result<...> {
        let span = tracing::info_span!(
            "s3_load_config",
            app = %app,
            profile = %profile,
            bucket = %self.config.bucket
        );

        async move {
            tracing::debug!("Loading config from S3");

            // ... operation ...

            tracing::info!(
                keys_checked = formats.len(),
                found = result.is_some(),
                "S3 config lookup complete"
            );

            result
        }.instrument(span).await
    }
}
```

### Metricas Sugeridas

```rust
// Counters
s3_requests_total{bucket, operation, status}
s3_retries_total{bucket}

// Histograms
s3_request_duration_seconds{bucket, operation}
s3_object_size_bytes{bucket}
```

---

## Seguridad

### Credenciales

```bash
# Option 1: Environment variables
export AWS_ACCESS_KEY_ID=AKIA...
export AWS_SECRET_ACCESS_KEY=...
export AWS_REGION=us-east-1

# Option 2: IAM Role (recommended for AWS)
# Automatically uses EC2/ECS/Lambda role

# Option 3: Shared credentials file (~/.aws/credentials)
[default]
aws_access_key_id = AKIA...
aws_secret_access_key = ...
```

### Validacion de Input

```rust
impl S3Config {
    pub fn validate(&self) -> Result<(), BackendError> {
        // Validate bucket name
        if self.bucket.is_empty() {
            return Err(BackendError::InvalidConfig("Bucket name required"));
        }

        // Check for path traversal
        if self.bucket.contains("..") {
            return Err(BackendError::InvalidConfig("Invalid bucket name"));
        }

        Ok(())
    }
}
```

---

## Entregable Final

### Archivos Creados

1. `crates/vortex-backends/src/s3/mod.rs` - Modulo S3
2. `crates/vortex-backends/src/s3/config.rs` - Configuracion S3
3. `crates/vortex-backends/src/s3/client.rs` - Cliente S3 con retry
4. `crates/vortex-backends/src/s3/source.rs` - S3ConfigSource
5. `crates/vortex-backends/src/error.rs` - Tipos de error
6. `crates/vortex-backends/tests/s3_test.rs` - Tests de integracion

### Verificacion

```bash
# Compilar con feature s3
cargo build -p vortex-backends --features s3

# Tests unitarios
cargo test -p vortex-backends --features s3

# Tests de integracion (requiere Docker)
cargo test -p vortex-backends --features s3 --test s3_test

# Clippy
cargo clippy -p vortex-backends --features s3 -- -D warnings
```

### Ejemplo de Uso

```rust
use vortex_backends::s3::{S3Config, S3ConfigSource};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Production AWS
    let config = S3Config::new("my-config-bucket")
        .with_region("us-east-1");

    // Or MinIO/LocalStack
    let config = S3Config::new("my-config-bucket")
        .with_endpoint("http://localhost:9000")
        .with_region("us-east-1");

    let source = S3ConfigSource::new(config).await?;

    let config = source
        .get_config("payment-service", &["production".to_string()], None)
        .await?;

    println!("Loaded {} property sources", config.property_sources.len());

    Ok(())
}
```
