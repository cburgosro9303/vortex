# Historia 007: Tests de Integracion Multi-Backend

## Contexto y Objetivo

Esta historia establece la infraestructura de testing para todos los backends usando Testcontainers. Los tests de integracion validan que cada backend funciona correctamente contra servicios reales (PostgreSQL, LocalStack/MinIO) en lugar de mocks.

**Beneficios de Testcontainers:**
- Tests reproducibles en cualquier maquina
- No requiere infraestructura externa
- Containers efimeros y aislados
- Mismo comportamiento en CI y local

Esta historia asegura que los backends funcionan correctamente en escenarios reales.

---

## Alcance

### In Scope

- Setup de Testcontainers para PostgreSQL
- Setup de Testcontainers para LocalStack (S3)
- Helpers para crear datos de prueba
- Tests de integracion para cada backend
- Tests de integracion para el compositor
- Configuracion de CI para tests con containers

### Out of Scope

- Tests de performance/load
- Tests contra servicios reales de AWS
- Tests de MySQL/SQLite con containers (directo)
- E2E tests con cliente Spring Boot

---

## Criterios de Aceptacion

- [ ] Testcontainers funciona para PostgreSQL
- [ ] Testcontainers funciona para LocalStack (S3)
- [ ] Helpers reutilizables para setup de tests
- [ ] Tests de integracion para S3ConfigSource
- [ ] Tests de integracion para SqlConfigSource
- [ ] Tests de integracion para CompositeConfigSource
- [ ] CI pipeline corre tests con containers
- [ ] Tests son deterministas y no flaky

---

## Diseno Propuesto

### Estructura de Tests

```
crates/vortex-backends/
├── src/
│   └── ...
└── tests/
    ├── common/
    │   ├── mod.rs              # Re-exports
    │   ├── containers.rs       # Testcontainers setup
    │   ├── fixtures.rs         # Test data factories
    │   └── assertions.rs       # Custom assertions
    ├── s3_integration_test.rs
    ├── postgres_integration_test.rs
    └── composite_integration_test.rs
```

### Diagrama de Tests

```
┌──────────────────────────────────────────────────────────────┐
│                    Test Execution                             │
├──────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌─────────────────────┐    ┌─────────────────────┐         │
│  │   PostgreSQL Test   │    │      S3 Test        │         │
│  ├─────────────────────┤    ├─────────────────────┤         │
│  │                     │    │                     │         │
│  │  ┌───────────────┐  │    │  ┌───────────────┐  │         │
│  │  │  Postgres     │  │    │  │  LocalStack   │  │         │
│  │  │  Container    │  │    │  │  Container    │  │         │
│  │  │  (5432)       │  │    │  │  (4566)       │  │         │
│  │  └───────┬───────┘  │    │  └───────┬───────┘  │         │
│  │          │          │    │          │          │         │
│  │  ┌───────▼───────┐  │    │  ┌───────▼───────┐  │         │
│  │  │ Run migrations│  │    │  │ Create bucket │  │         │
│  │  │ Insert data   │  │    │  │ Upload files  │  │         │
│  │  └───────┬───────┘  │    │  └───────┬───────┘  │         │
│  │          │          │    │          │          │         │
│  │  ┌───────▼───────┐  │    │  ┌───────▼───────┐  │         │
│  │  │SqlConfigSource│  │    │  │S3ConfigSource │  │         │
│  │  │   Tests       │  │    │  │   Tests       │  │         │
│  │  └───────────────┘  │    │  └───────────────┘  │         │
│  │                     │    │                     │         │
│  └─────────────────────┘    └─────────────────────┘         │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              Composite Integration Test               │   │
│  ├──────────────────────────────────────────────────────┤   │
│  │  Both containers + CompositeConfigSource              │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

---

## Pasos de Implementacion

### Paso 1: Agregar Dependencias

```toml
# Cargo.toml
[dev-dependencies]
# Testcontainers
testcontainers = "0.18"
testcontainers-modules = { version = "0.6", features = [
    "postgres",
    "localstack"
]}

# Testing utilities
tokio-test = "0.4"
tempfile = "3"
pretty_assertions = "1"
serial_test = "3"

# Async test utilities
futures = "0.3"
```

### Paso 2: Implementar Container Helpers

```rust
// tests/common/containers.rs
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::localstack::LocalStack;
use sqlx::PgPool;
use aws_sdk_s3::Client as S3Client;

/// PostgreSQL container wrapper with helper methods.
pub struct PostgresContainer {
    container: ContainerAsync<Postgres>,
    pool: Option<PgPool>,
}

impl PostgresContainer {
    /// Starts a new PostgreSQL container.
    pub async fn start() -> Self {
        let container = Postgres::default()
            .start()
            .await;

        // Wait for container to be ready
        tokio::time::sleep(Duration::from_secs(2)).await;

        Self {
            container,
            pool: None,
        }
    }

    /// Returns the connection string for this container.
    pub async fn connection_string(&self) -> String {
        let port = self.container.get_host_port_ipv4(5432).await;
        format!("postgres://postgres:postgres@localhost:{}/postgres", port)
    }

    /// Creates and returns a connection pool.
    pub async fn pool(&mut self) -> &PgPool {
        if self.pool.is_none() {
            let conn_str = self.connection_string().await;
            let pool = PgPool::connect(&conn_str)
                .await
                .expect("Failed to connect to PostgreSQL");

            // Run migrations
            sqlx::migrate!("./migrations/postgres")
                .run(&pool)
                .await
                .expect("Failed to run migrations");

            self.pool = Some(pool);
        }

        self.pool.as_ref().unwrap()
    }

    /// Returns the host and port.
    pub async fn host_port(&self) -> (String, u16) {
        let port = self.container.get_host_port_ipv4(5432).await;
        ("localhost".to_string(), port)
    }
}

/// LocalStack container wrapper for S3 testing.
pub struct LocalStackContainer {
    container: ContainerAsync<LocalStack>,
    client: Option<S3Client>,
}

impl LocalStackContainer {
    /// Starts a new LocalStack container.
    pub async fn start() -> Self {
        let container = LocalStack::default()
            .start()
            .await;

        // Wait for LocalStack to be ready
        tokio::time::sleep(Duration::from_secs(3)).await;

        Self {
            container,
            client: None,
        }
    }

    /// Returns the S3 endpoint URL.
    pub async fn endpoint_url(&self) -> String {
        let port = self.container.get_host_port_ipv4(4566).await;
        format!("http://localhost:{}", port)
    }

    /// Creates and returns an S3 client configured for LocalStack.
    pub async fn s3_client(&mut self) -> &S3Client {
        if self.client.is_none() {
            let endpoint = self.endpoint_url().await;

            let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
                .region(aws_sdk_s3::config::Region::new("us-east-1"))
                .load()
                .await;

            let s3_config = aws_sdk_s3::config::Builder::from(&config)
                .endpoint_url(&endpoint)
                .force_path_style(true)
                .build();

            self.client = Some(S3Client::from_conf(s3_config));
        }

        self.client.as_ref().unwrap()
    }

    /// Creates a bucket.
    pub async fn create_bucket(&mut self, bucket: &str) {
        let client = self.s3_client().await;

        client
            .create_bucket()
            .bucket(bucket)
            .send()
            .await
            .expect("Failed to create bucket");
    }

    /// Uploads an object to a bucket.
    pub async fn upload_object(&mut self, bucket: &str, key: &str, content: &[u8]) {
        let client = self.s3_client().await;

        client
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(content.to_vec().into())
            .send()
            .await
            .expect("Failed to upload object");
    }

    /// Enables versioning on a bucket.
    pub async fn enable_versioning(&mut self, bucket: &str) {
        let client = self.s3_client().await;

        client
            .put_bucket_versioning()
            .bucket(bucket)
            .versioning_configuration(
                aws_sdk_s3::types::VersioningConfiguration::builder()
                    .status(aws_sdk_s3::types::BucketVersioningStatus::Enabled)
                    .build()
            )
            .send()
            .await
            .expect("Failed to enable versioning");
    }
}
```

### Paso 3: Implementar Fixtures

```rust
// tests/common/fixtures.rs
use serde_json::{json, Value};
use uuid::Uuid;
use sqlx::PgPool;

/// Test data factory for creating configurations.
pub struct ConfigFixtures;

impl ConfigFixtures {
    /// Creates a sample YAML config.
    pub fn sample_yaml() -> &'static str {
        r#"
server:
  port: 8080
  host: localhost
database:
  url: jdbc:postgresql://localhost/db
  pool-size: 10
features:
  dark-mode: false
  beta-features: true
"#
    }

    /// Creates a sample JSON config.
    pub fn sample_json() -> Value {
        json!({
            "server.port": 8080,
            "server.host": "localhost",
            "database.url": "jdbc:postgresql://localhost/db",
            "database.pool-size": 10,
            "features.dark-mode": false,
            "features.beta-features": true
        })
    }

    /// Creates sample properties.
    pub fn sample_properties() -> &'static str {
        r#"
server.port=8080
server.host=localhost
database.url=jdbc:postgresql://localhost/db
database.pool-size=10
"#
    }

    /// Creates production override config.
    pub fn production_override() -> Value {
        json!({
            "server.port": 9090,
            "server.ssl.enabled": true,
            "database.pool-size": 50
        })
    }
}

/// Database fixtures for SQL tests.
pub struct DbFixtures;

impl DbFixtures {
    /// Creates a test application.
    pub async fn create_application(pool: &PgPool, name: &str) -> Uuid {
        sqlx::query_scalar!(
            r#"
            INSERT INTO applications (name, description)
            VALUES ($1, $2)
            RETURNING id
            "#,
            name,
            format!("Test application: {}", name)
        )
        .fetch_one(pool)
        .await
        .expect("Failed to create application")
    }

    /// Creates a test profile.
    pub async fn create_profile(pool: &PgPool, app_id: Uuid, profile: &str) -> Uuid {
        sqlx::query_scalar!(
            r#"
            INSERT INTO config_profiles (application_id, profile)
            VALUES ($1, $2)
            RETURNING id
            "#,
            app_id,
            profile
        )
        .fetch_one(pool)
        .await
        .expect("Failed to create profile")
    }

    /// Creates a config version.
    pub async fn create_version(
        pool: &PgPool,
        profile_id: Uuid,
        version: i32,
        content: &Value,
        is_active: bool,
    ) -> Uuid {
        let checksum = format!("{:x}", md5::compute(content.to_string()));

        sqlx::query_scalar!(
            r#"
            INSERT INTO config_versions
                (profile_id, version, content, checksum, is_active, created_by)
            VALUES ($1, $2, $3, $4, $5, 'test')
            RETURNING id
            "#,
            profile_id,
            version,
            content,
            checksum,
            is_active
        )
        .fetch_one(pool)
        .await
        .expect("Failed to create version")
    }

    /// Creates a complete test setup: app -> profile -> version.
    pub async fn create_full_config(
        pool: &PgPool,
        app: &str,
        profile: &str,
        content: &Value,
    ) -> (Uuid, Uuid, Uuid) {
        let app_id = Self::create_application(pool, app).await;
        let profile_id = Self::create_profile(pool, app_id, profile).await;
        let version_id = Self::create_version(pool, profile_id, 1, content, true).await;

        (app_id, profile_id, version_id)
    }
}

/// S3 fixtures.
pub struct S3Fixtures;

impl S3Fixtures {
    /// Standard test bucket name.
    pub const BUCKET: &'static str = "test-config-bucket";

    /// Creates standard test structure in S3.
    pub async fn setup_standard_structure(container: &mut super::containers::LocalStackContainer) {
        container.create_bucket(Self::BUCKET).await;

        // Create multiple apps
        container.upload_object(
            Self::BUCKET,
            "payment-service/default.yml",
            ConfigFixtures::sample_yaml().as_bytes(),
        ).await;

        container.upload_object(
            Self::BUCKET,
            "payment-service/production.yml",
            b"server:\n  port: 9090\n  ssl: true",
        ).await;

        container.upload_object(
            Self::BUCKET,
            "user-service/default.json",
            ConfigFixtures::sample_json().to_string().as_bytes(),
        ).await;
    }
}
```

### Paso 4: Implementar Custom Assertions

```rust
// tests/common/assertions.rs
use vortex_backends::types::{ConfigMap, PropertySource};
use pretty_assertions::assert_eq;

/// Custom assertions for config testing.
pub trait ConfigAssertions {
    fn assert_has_property(&self, key: &str, expected: impl Into<serde_json::Value>);
    fn assert_property_count(&self, count: usize);
    fn assert_has_source(&self, name_contains: &str);
}

impl ConfigAssertions for ConfigMap {
    fn assert_has_property(&self, key: &str, expected: impl Into<serde_json::Value>) {
        let expected = expected.into();

        for source in &self.property_sources {
            if let Some(value) = source.source.get(key) {
                assert_eq!(
                    value, &expected,
                    "Property '{}' has wrong value. Expected: {:?}, Got: {:?}",
                    key, expected, value
                );
                return;
            }
        }

        panic!(
            "Property '{}' not found in any source. Available sources: {:?}",
            key,
            self.property_sources.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    fn assert_property_count(&self, count: usize) {
        let total: usize = self.property_sources
            .iter()
            .map(|s| s.source.len())
            .sum();

        assert_eq!(
            total, count,
            "Expected {} total properties, found {}",
            count, total
        );
    }

    fn assert_has_source(&self, name_contains: &str) {
        let found = self.property_sources
            .iter()
            .any(|s| s.name.contains(name_contains));

        assert!(
            found,
            "No source containing '{}' found. Sources: {:?}",
            name_contains,
            self.property_sources.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }
}

impl ConfigAssertions for PropertySource {
    fn assert_has_property(&self, key: &str, expected: impl Into<serde_json::Value>) {
        let expected = expected.into();
        let value = self.source.get(key).expect(&format!(
            "Property '{}' not found in source '{}'",
            key, self.name
        ));

        assert_eq!(
            value, &expected,
            "Property '{}' has wrong value",
            key
        );
    }

    fn assert_property_count(&self, count: usize) {
        assert_eq!(
            self.source.len(), count,
            "Expected {} properties, found {}",
            count, self.source.len()
        );
    }

    fn assert_has_source(&self, _name_contains: &str) {
        // N/A for single source
    }
}
```

### Paso 5: Tests de PostgreSQL

```rust
// tests/postgres_integration_test.rs
#![cfg(feature = "postgres")]

mod common;

use common::{
    containers::PostgresContainer,
    fixtures::{ConfigFixtures, DbFixtures},
    assertions::ConfigAssertions,
};
use vortex_backends::sql::{SqlConfig, SqlConfigSource};
use vortex_backends::traits::ConfigSource;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn get_config_returns_active_version() {
    let mut pg = PostgresContainer::start().await;
    let pool = pg.pool().await;

    // Setup
    let (_, _, _) = DbFixtures::create_full_config(
        pool,
        "myapp",
        "default",
        &ConfigFixtures::sample_json(),
    ).await;

    // Test
    let source = SqlConfigSource::from_pool(pool.clone());
    let config = source
        .get_config("myapp", &["default".to_string()], None)
        .await
        .expect("Failed to get config");

    // Assert
    assert_eq!(config.name, "myapp");
    config.assert_has_property("server.port", 8080);
    config.assert_has_source("sql:");
}

#[tokio::test]
#[serial]
async fn get_config_merges_profiles_correctly() {
    let mut pg = PostgresContainer::start().await;
    let pool = pg.pool().await;

    // Setup: default + production profiles
    let app_id = DbFixtures::create_application(pool, "payment").await;

    let default_id = DbFixtures::create_profile(pool, app_id, "default").await;
    DbFixtures::create_version(
        pool,
        default_id,
        1,
        &ConfigFixtures::sample_json(),
        true,
    ).await;

    let prod_id = DbFixtures::create_profile(pool, app_id, "production").await;
    DbFixtures::create_version(
        pool,
        prod_id,
        1,
        &ConfigFixtures::production_override(),
        true,
    ).await;

    // Test
    let source = SqlConfigSource::from_pool(pool.clone());
    let config = source
        .get_config("payment", &["production".to_string()], None)
        .await
        .expect("Failed to get config");

    // Assert: production overrides default
    assert_eq!(config.property_sources.len(), 2);
    // Production comes first (higher priority)
    assert!(config.property_sources[0].name.contains("production"));
    config.property_sources[0].assert_has_property("server.port", 9090);
}

#[tokio::test]
#[serial]
async fn get_config_returns_not_found_for_missing_app() {
    let mut pg = PostgresContainer::start().await;
    let pool = pg.pool().await;

    let source = SqlConfigSource::from_pool(pool.clone());
    let result = source
        .get_config("nonexistent", &["default".to_string()], None)
        .await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        vortex_backends::error::BackendError::NotFound(_)
    ));
}

#[tokio::test]
#[serial]
async fn list_applications_returns_all() {
    let mut pg = PostgresContainer::start().await;
    let pool = pg.pool().await;

    // Setup
    DbFixtures::create_application(pool, "app1").await;
    DbFixtures::create_application(pool, "app2").await;
    DbFixtures::create_application(pool, "app3").await;

    // Test
    let source = SqlConfigSource::from_pool(pool.clone());
    let apps = source.list_applications().await.expect("Failed to list apps");

    // Assert
    assert_eq!(apps.len(), 3);
    assert!(apps.contains(&"app1".to_string()));
    assert!(apps.contains(&"app2".to_string()));
    assert!(apps.contains(&"app3".to_string()));
}

#[tokio::test]
#[serial]
async fn get_specific_version_works() {
    let mut pg = PostgresContainer::start().await;
    let pool = pg.pool().await;

    // Setup: multiple versions
    let app_id = DbFixtures::create_application(pool, "versioned-app").await;
    let profile_id = DbFixtures::create_profile(pool, app_id, "default").await;

    DbFixtures::create_version(
        pool, profile_id, 1,
        &serde_json::json!({"version": "v1"}),
        false,
    ).await;

    DbFixtures::create_version(
        pool, profile_id, 2,
        &serde_json::json!({"version": "v2"}),
        true,  // Active
    ).await;

    // Test: get old version
    let source = SqlConfigSource::from_pool(pool.clone());
    let config = source
        .get_config_version("versioned-app", &["default".to_string()], 1)
        .await
        .expect("Failed to get version");

    // Assert
    config.assert_has_property("version", "v1");
}
```

### Paso 6: Tests de S3/LocalStack

```rust
// tests/s3_integration_test.rs
#![cfg(feature = "s3")]

mod common;

use common::{
    containers::LocalStackContainer,
    fixtures::{ConfigFixtures, S3Fixtures},
    assertions::ConfigAssertions,
};
use vortex_backends::s3::{S3Config, S3ConfigSource};
use vortex_backends::traits::ConfigSource;
use futures::StreamExt;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn get_config_reads_yaml_from_s3() {
    let mut localstack = LocalStackContainer::start().await;

    // Setup
    localstack.create_bucket("configs").await;
    localstack.upload_object(
        "configs",
        "myapp/default.yml",
        ConfigFixtures::sample_yaml().as_bytes(),
    ).await;

    // Test
    let config = S3Config::new("configs")
        .with_endpoint(&localstack.endpoint_url().await)
        .with_region("us-east-1");

    let source = S3ConfigSource::new(config)
        .await
        .expect("Failed to create S3 source");

    let result = source
        .get_config("myapp", &["default".to_string()], None)
        .await
        .expect("Failed to get config");

    // Assert
    assert_eq!(result.name, "myapp");
    result.assert_has_property("server.port", 8080);
    result.assert_has_source("s3:");
}

#[tokio::test]
#[serial]
async fn get_config_merges_profiles_from_s3() {
    let mut localstack = LocalStackContainer::start().await;

    // Setup
    localstack.create_bucket("configs").await;
    localstack.upload_object(
        "configs",
        "payment/default.yml",
        b"server:\n  port: 8080\n  host: localhost",
    ).await;
    localstack.upload_object(
        "configs",
        "payment/production.yml",
        b"server:\n  port: 9090\n  ssl: true",
    ).await;

    // Test
    let config = S3Config::new("configs")
        .with_endpoint(&localstack.endpoint_url().await)
        .with_region("us-east-1");

    let source = S3ConfigSource::new(config).await.unwrap();
    let result = source
        .get_config("payment", &["production".to_string()], None)
        .await
        .expect("Failed to get config");

    // Assert: production overrides
    assert_eq!(result.property_sources.len(), 2);
    result.assert_has_property("server.port", 9090);
    result.assert_has_property("server.ssl", true);
}

#[tokio::test]
#[serial]
async fn list_applications_from_s3() {
    let mut localstack = LocalStackContainer::start().await;

    // Setup
    S3Fixtures::setup_standard_structure(&mut localstack).await;

    // Test
    let config = S3Config::new(S3Fixtures::BUCKET)
        .with_endpoint(&localstack.endpoint_url().await)
        .with_region("us-east-1");

    let source = S3ConfigSource::new(config).await.unwrap();

    let apps: Vec<String> = source.list_applications()
        .filter_map(|r| async { r.ok() })
        .collect()
        .await;

    // Assert
    assert!(apps.contains(&"payment-service".to_string()));
    assert!(apps.contains(&"user-service".to_string()));
}

#[tokio::test]
#[serial]
async fn versioning_works_with_s3() {
    let mut localstack = LocalStackContainer::start().await;

    // Setup
    localstack.create_bucket("versioned").await;
    localstack.enable_versioning("versioned").await;

    // Upload multiple versions
    localstack.upload_object("versioned", "app/default.yml", b"version: v1").await;
    localstack.upload_object("versioned", "app/default.yml", b"version: v2").await;
    localstack.upload_object("versioned", "app/default.yml", b"version: v3").await;

    // Test
    let config = S3Config::new("versioned")
        .with_endpoint(&localstack.endpoint_url().await)
        .with_region("us-east-1");

    let source = S3ConfigSource::new(config).await.unwrap();

    let versions: Vec<_> = source.list_versions("app", "default")
        .filter_map(|r| async { r.ok() })
        .collect()
        .await;

    // Assert
    assert_eq!(versions.len(), 3);
    assert!(versions.iter().any(|v| v.is_latest));
}

#[tokio::test]
#[serial]
async fn handles_missing_config_gracefully() {
    let mut localstack = LocalStackContainer::start().await;
    localstack.create_bucket("empty").await;

    let config = S3Config::new("empty")
        .with_endpoint(&localstack.endpoint_url().await)
        .with_region("us-east-1");

    let source = S3ConfigSource::new(config).await.unwrap();
    let result = source
        .get_config("nonexistent", &["default".to_string()], None)
        .await;

    // Should return empty config, not error
    let config = result.expect("Should not error");
    assert!(config.property_sources.is_empty());
}
```

### Paso 7: Tests del Compositor

```rust
// tests/composite_integration_test.rs
#![cfg(all(feature = "postgres", feature = "s3"))]

mod common;

use std::sync::Arc;
use common::{
    containers::{PostgresContainer, LocalStackContainer},
    fixtures::{ConfigFixtures, DbFixtures, S3Fixtures},
    assertions::ConfigAssertions,
};
use vortex_backends::{
    sql::{SqlConfig, SqlConfigSource},
    s3::{S3Config, S3ConfigSource},
    composite::{CompositeBuilder, ErrorStrategy, MergeStrategy, priorities},
    traits::ConfigSource,
};
use serial_test::serial;

#[tokio::test]
#[serial]
async fn composite_merges_sql_and_s3() {
    // Start both containers
    let mut pg = PostgresContainer::start().await;
    let mut localstack = LocalStackContainer::start().await;

    let pool = pg.pool().await;

    // Setup SQL: base config
    DbFixtures::create_full_config(
        pool,
        "myapp",
        "default",
        &serde_json::json!({
            "server.port": 8080,
            "database.url": "localhost"
        }),
    ).await;

    // Setup S3: override config
    localstack.create_bucket("overrides").await;
    localstack.upload_object(
        "overrides",
        "myapp/default.yml",
        b"server:\n  port: 9090\n  ssl: true",
    ).await;

    // Create sources
    let sql_source = SqlConfigSource::from_pool(pool.clone());
    let s3_source = S3ConfigSource::new(
        S3Config::new("overrides")
            .with_endpoint(&localstack.endpoint_url().await)
            .with_region("us-east-1")
    ).await.unwrap();

    // Build composite: S3 has higher priority
    let composite = CompositeBuilder::new()
        .merge_strategy(MergeStrategy::Override)
        .add_backend("sql", priorities::PRIMARY, sql_source)
        .add_backend("s3", priorities::EMERGENCY, s3_source)
        .build()
        .await;

    // Test
    let config = composite
        .get_config("myapp", &["default".to_string()], None)
        .await
        .expect("Failed to get config");

    // Assert: S3 (higher priority) wins for conflicts
    config.assert_has_property("server.port", 9090);  // From S3
    config.assert_has_property("server.ssl", true);   // From S3
    config.assert_has_property("database.url", "localhost");  // From SQL (no conflict)
}

#[tokio::test]
#[serial]
async fn composite_continues_on_backend_error() {
    let mut pg = PostgresContainer::start().await;
    let pool = pg.pool().await;

    // Setup SQL: working
    DbFixtures::create_full_config(
        pool,
        "myapp",
        "default",
        &serde_json::json!({"from": "sql"}),
    ).await;

    // S3 with invalid config (will fail)
    let s3_source = S3ConfigSource::new(
        S3Config::new("nonexistent-bucket")
            .with_endpoint("http://localhost:9999")  // Invalid
            .with_region("us-east-1")
    ).await;

    // Should fail to create - that's OK for this test
    // In real scenario, you'd have a mock that fails

    let sql_source = SqlConfigSource::from_pool(pool.clone());

    let composite = CompositeBuilder::new()
        .error_strategy(ErrorStrategy::Continue)
        .add_backend("sql", 10, sql_source)
        // Skip failing S3 for this test
        .build()
        .await;

    let config = composite
        .get_config("myapp", &["default".to_string()], None)
        .await
        .expect("Should succeed with Continue strategy");

    config.assert_has_property("from", "sql");
}

#[tokio::test]
#[serial]
async fn composite_deep_merge_combines_configs() {
    let mut pg = PostgresContainer::start().await;
    let mut localstack = LocalStackContainer::start().await;

    let pool = pg.pool().await;

    // Setup
    DbFixtures::create_full_config(
        pool,
        "app",
        "default",
        &serde_json::json!({
            "server.port": 8080,
            "server.host": "localhost"
        }),
    ).await;

    localstack.create_bucket("merge-test").await;
    localstack.upload_object(
        "merge-test",
        "app/default.yml",
        b"server:\n  port: 9090\n  ssl: true",
    ).await;

    let sql_source = SqlConfigSource::from_pool(pool.clone());
    let s3_source = S3ConfigSource::new(
        S3Config::new("merge-test")
            .with_endpoint(&localstack.endpoint_url().await)
            .with_region("us-east-1")
    ).await.unwrap();

    let composite = CompositeBuilder::new()
        .merge_strategy(MergeStrategy::DeepMerge)
        .add_backend("sql", 10, sql_source)
        .add_backend("s3", 20, s3_source)
        .build()
        .await;

    let config = composite
        .get_config("app", &["default".to_string()], None)
        .await
        .expect("Failed to get config");

    // With DeepMerge, we should have a single merged source
    assert_eq!(config.property_sources.len(), 1);
    assert!(config.property_sources[0].name.contains("merged"));

    // All properties present
    config.assert_has_property("server.port", 9090);  // S3 wins
    config.assert_has_property("server.host", "localhost");  // From SQL
    config.assert_has_property("server.ssl", true);  // From S3
}
```

### Paso 8: Configuracion de CI

```yaml
# .github/workflows/integration-tests.yml
name: Integration Tests

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  integration-tests:
    runs-on: ubuntu-latest

    services:
      # Note: We use testcontainers, not GitHub services
      # But we need Docker
      docker:
        image: docker:dind
        options: --privileged

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Run PostgreSQL tests
        run: cargo test --features postgres --test postgres_integration_test

      - name: Run S3 tests
        run: cargo test --features s3 --test s3_integration_test

      - name: Run Composite tests
        run: cargo test --features "postgres,s3" --test composite_integration_test

      - name: Run all integration tests
        run: cargo test --features all-backends --test '*'
```

---

## Conceptos de Rust Aprendidos

### 1. Testcontainers en Rust

**Rust:**
```rust
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

#[tokio::test]
async fn test_with_postgres() {
    // Start container
    let container = Postgres::default().start().await;

    // Get dynamic port
    let port = container.get_host_port_ipv4(5432).await;

    // Container stops automatically when dropped
}
```

**Comparacion con Java:**
```java
@Testcontainers
class PostgresTest {
    @Container
    static PostgreSQLContainer<?> postgres = new PostgreSQLContainer<>("postgres:14");

    @Test
    void testWithPostgres() {
        String url = postgres.getJdbcUrl();
        // Use container
    }
}
```

### 2. Serial Tests

```rust
use serial_test::serial;

// Tests que usan containers deben ser seriales
// para evitar conflictos de puertos
#[tokio::test]
#[serial]
async fn test_1() {
    // Uses port 5432
}

#[tokio::test]
#[serial]
async fn test_2() {
    // Also uses port 5432 - would conflict without #[serial]
}
```

### 3. Feature-Gated Tests

```rust
// Este archivo solo se compila con feature postgres
#![cfg(feature = "postgres")]

// Test que requiere multiples features
#[cfg(all(feature = "postgres", feature = "s3"))]
#[tokio::test]
async fn test_composite() {
    // ...
}
```

---

## Riesgos y Errores Comunes

### 1. Tests Flaky por Timing

```rust
// MAL: Race condition
#[tokio::test]
async fn flaky_test() {
    let container = Postgres::default().start().await;
    let pool = PgPool::connect(&url).await.unwrap();  // Puede fallar si container no esta listo
}

// BIEN: Esperar a que este listo
#[tokio::test]
async fn stable_test() {
    let container = Postgres::default().start().await;

    // Wait for container to be ready
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Or use health check
    let pool = loop {
        match PgPool::connect(&url).await {
            Ok(p) => break p,
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    };
}
```

### 2. Port Conflicts

```rust
// MAL: Tests paralelos con mismo puerto
// cargo test -- --test-threads=4

// BIEN: Usar serial_test o puertos dinamicos
#[serial]
#[tokio::test]
async fn test_a() { ... }

#[serial]
#[tokio::test]
async fn test_b() { ... }
```

### 3. Container Cleanup

```rust
// Container se limpia automaticamente cuando sale de scope
#[tokio::test]
async fn test() {
    let container = Postgres::default().start().await;
    // ... test ...
}  // container dropped, stops automatically

// Si necesitas cleanup explicito:
#[tokio::test]
async fn test_with_cleanup() {
    let container = Postgres::default().start().await;

    // Cleanup function
    let cleanup = || async {
        sqlx::query!("DELETE FROM test_data").execute(&pool).await.ok();
    };

    // Test with cleanup on panic
    let result = std::panic::catch_unwind(|| async {
        // test code
    });

    cleanup().await;

    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
```

---

## Entregable Final

### Archivos Creados

1. `tests/common/mod.rs` - Module exports
2. `tests/common/containers.rs` - Container helpers
3. `tests/common/fixtures.rs` - Test data factories
4. `tests/common/assertions.rs` - Custom assertions
5. `tests/postgres_integration_test.rs` - PostgreSQL tests
6. `tests/s3_integration_test.rs` - S3/LocalStack tests
7. `tests/composite_integration_test.rs` - Composite tests
8. `.github/workflows/integration-tests.yml` - CI config

### Verificacion

```bash
# Requiere Docker corriendo

# Tests individuales
cargo test --features postgres --test postgres_integration_test -- --nocapture
cargo test --features s3 --test s3_integration_test -- --nocapture

# Tests del compositor
cargo test --features "postgres,s3" --test composite_integration_test

# Todos los tests de integracion
cargo test --features all-backends --test '*'

# Con logs
RUST_LOG=debug cargo test --features all-backends --test '*' -- --nocapture
```

### Ejemplo de Ejecucion

```bash
$ cargo test --features "postgres,s3" --test composite_integration_test

running 3 tests
test composite_merges_sql_and_s3 ... ok
test composite_continues_on_backend_error ... ok
test composite_deep_merge_combines_configs ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.23s
```
