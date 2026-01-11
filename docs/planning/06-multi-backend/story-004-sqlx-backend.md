# Historia 004: Backend SQL con SQLx

## Contexto y Objetivo

Esta historia implementa el backend SQL completo para PostgreSQL utilizando SQLx. El backend permite almacenar y consultar configuraciones desde una base de datos relacional, aprovechando las verificaciones en compile-time de SQLx.

**Ventajas del backend SQL:**
- Integracion con infraestructura de BD existente
- Transacciones ACID para consistencia
- Queries complejas (busqueda por propiedades, auditoria)
- Connection pooling integrado
- Verificacion de queries en compile-time

SQLx es unico en Rust porque verifica queries contra la base de datos real durante la compilacion, eliminando errores de SQL en runtime.

---

## Alcance

### In Scope

- `SqlConfigSource` implementando trait `ConfigSource`
- Connection pooling con configuracion
- Queries compile-time verified
- CRUD completo para configuraciones
- Soporte de versionado (obtener version activa)
- Transaction support para operaciones atomicas

### Out of Scope

- MySQL/SQLite (historia 005)
- Migraciones automaticas (historia 003)
- Caching (podria agregarse despues)
- Connection failover/replica
- Batch inserts

---

## Criterios de Aceptacion

- [ ] `SqlConfigSource` implementa `ConfigSource`
- [ ] Connection pool configurado con limites apropiados
- [ ] Queries verificadas en compile-time con `sqlx::query!`
- [ ] Metodo `get_config` obtiene version activa
- [ ] Metodo `list_applications` lista todas las apps
- [ ] Metodo `list_profiles` lista perfiles por app
- [ ] Transacciones para operaciones multi-paso
- [ ] Tests con PostgreSQL real (testcontainers)
- [ ] Manejo correcto de errores SQL

---

## Diseno Propuesto

### Estructura de Modulos

```
crates/vortex-backends/
├── src/
│   ├── sql/
│   │   ├── mod.rs           # Re-exports
│   │   ├── config.rs        # SqlConfig
│   │   ├── pool.rs          # Connection pool
│   │   ├── source.rs        # SqlConfigSource
│   │   ├── queries.rs       # Query functions
│   │   └── error.rs         # SQL-specific errors
│   └── ...
└── tests/
    └── postgres_test.rs
```

### Interfaces Principales

```rust
// Configuration for SQL backend
pub struct SqlConfig {
    pub connection_string: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
}

// SQL-backed configuration source
pub struct SqlConfigSource {
    pool: PgPool,
}

impl ConfigSource for SqlConfigSource {
    async fn get_config(
        &self,
        app: &str,
        profiles: &[String],
        label: Option<&str>,
    ) -> Result<ConfigMap, ConfigError>;

    fn name(&self) -> &str;
}
```

### Diagrama de Flujo

```
┌─────────────────────────────────────┐
│  get_config("payment", ["prod"])    │
└──────────────┬──────────────────────┘
               │
               ▼
┌─────────────────────────────────────┐
│  1. Get application by name         │
│     SELECT * FROM applications      │
│     WHERE name = 'payment'          │
└──────────────┬──────────────────────┘
               │
               ▼
┌─────────────────────────────────────┐
│  2. Get profiles for app            │
│     SELECT * FROM config_profiles   │
│     WHERE application_id = ?        │
│     AND profile IN ('default','prod')│
└──────────────┬──────────────────────┘
               │
               ▼
┌─────────────────────────────────────┐
│  3. Get active version for each     │
│     SELECT * FROM config_versions   │
│     WHERE profile_id = ?            │
│     AND is_active = true            │
└──────────────┬──────────────────────┘
               │
               ▼
┌─────────────────────────────────────┐
│  4. Convert to PropertySource       │
│     - Flatten JSON content          │
│     - Set source name               │
└──────────────┬──────────────────────┘
               │
               ▼
┌─────────────────────────────────────┐
│  5. Return ConfigMap                │
│     - Merge profiles (prod > default)│
└─────────────────────────────────────┘
```

---

## Pasos de Implementacion

### Paso 1: Configurar SQLx

```toml
# Cargo.toml
[dependencies]
sqlx = { version = "0.8", features = [
    "runtime-tokio",
    "postgres",
    "uuid",
    "chrono",
    "json"
], optional = true }

[features]
postgres = ["sqlx/postgres"]
```

### Paso 2: Implementar SqlConfig

```rust
// src/sql/config.rs
use std::time::Duration;

/// Configuration for SQL backend connection.
#[derive(Debug, Clone)]
pub struct SqlConfig {
    /// Database connection string.
    /// Format: postgres://user:password@host:port/database
    pub connection_string: String,

    /// Maximum number of connections in the pool.
    pub max_connections: u32,

    /// Minimum number of connections to keep open.
    pub min_connections: u32,

    /// Timeout for acquiring a connection.
    pub connect_timeout: Duration,

    /// Close connections after this idle time.
    pub idle_timeout: Duration,

    /// Maximum connection lifetime.
    pub max_lifetime: Duration,
}

impl Default for SqlConfig {
    fn default() -> Self {
        Self {
            connection_string: String::new(),
            max_connections: 10,
            min_connections: 1,
            connect_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(600),
            max_lifetime: Duration::from_secs(1800),
        }
    }
}

impl SqlConfig {
    /// Creates a new SqlConfig with the given connection string.
    pub fn new(connection_string: impl Into<String>) -> Self {
        Self {
            connection_string: connection_string.into(),
            ..Default::default()
        }
    }

    /// Sets the maximum number of connections.
    pub fn max_connections(mut self, n: u32) -> Self {
        self.max_connections = n;
        self
    }

    /// Sets the minimum number of connections.
    pub fn min_connections(mut self, n: u32) -> Self {
        self.min_connections = n;
        self
    }

    /// Sets the connection timeout.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<(), SqlError> {
        if self.connection_string.is_empty() {
            return Err(SqlError::InvalidConfig("Connection string is required".into()));
        }

        if self.max_connections < self.min_connections {
            return Err(SqlError::InvalidConfig(
                "max_connections must be >= min_connections".into()
            ));
        }

        Ok(())
    }
}
```

### Paso 3: Implementar Connection Pool

```rust
// src/sql/pool.rs
use sqlx::postgres::{PgPool, PgPoolOptions};
use super::config::SqlConfig;
use super::error::SqlError;

/// Creates a PostgreSQL connection pool from configuration.
pub async fn create_pool(config: &SqlConfig) -> Result<PgPool, SqlError> {
    config.validate()?;

    tracing::info!(
        max_connections = config.max_connections,
        min_connections = config.min_connections,
        "Creating PostgreSQL connection pool"
    );

    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(config.connect_timeout)
        .idle_timeout(Some(config.idle_timeout))
        .max_lifetime(Some(config.max_lifetime))
        .connect(&config.connection_string)
        .await
        .map_err(|e| SqlError::ConnectionError(e.to_string()))?;

    // Verify connection
    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .map_err(|e| SqlError::ConnectionError(e.to_string()))?;

    tracing::info!("PostgreSQL connection pool created successfully");

    Ok(pool)
}

/// Health check for the pool.
pub async fn health_check(pool: &PgPool) -> Result<(), SqlError> {
    sqlx::query("SELECT 1")
        .execute(pool)
        .await
        .map_err(|e| SqlError::ConnectionError(e.to_string()))?;

    Ok(())
}

/// Pool statistics.
#[derive(Debug)]
pub struct PoolStats {
    pub size: u32,
    pub idle: usize,
    pub in_use: usize,
}

pub fn pool_stats(pool: &PgPool) -> PoolStats {
    PoolStats {
        size: pool.size(),
        idle: pool.num_idle(),
        in_use: pool.size() as usize - pool.num_idle(),
    }
}
```

### Paso 4: Implementar Queries

```rust
// src/sql/queries.rs
use sqlx::PgPool;
use uuid::Uuid;
use super::schema::{Application, ConfigProfile, ConfigVersion};
use super::error::SqlError;

/// Gets an application by name.
pub async fn get_application_by_name(
    pool: &PgPool,
    name: &str,
) -> Result<Option<Application>, SqlError> {
    let result = sqlx::query_as!(
        Application,
        r#"
        SELECT id, name, description, created_at, updated_at
        FROM applications
        WHERE name = $1
        "#,
        name
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| SqlError::QueryError(e.to_string()))?;

    Ok(result)
}

/// Gets profiles for an application.
pub async fn get_profiles_by_app(
    pool: &PgPool,
    app_id: Uuid,
    profile_names: &[String],
) -> Result<Vec<ConfigProfile>, SqlError> {
    let results = sqlx::query_as!(
        ConfigProfile,
        r#"
        SELECT id, application_id, profile, created_at
        FROM config_profiles
        WHERE application_id = $1
        AND profile = ANY($2)
        ORDER BY
            CASE profile
                WHEN 'default' THEN 0
                ELSE 1
            END,
            profile
        "#,
        app_id,
        profile_names
    )
    .fetch_all(pool)
    .await
    .map_err(|e| SqlError::QueryError(e.to_string()))?;

    Ok(results)
}

/// Gets the active version for a profile.
pub async fn get_active_version(
    pool: &PgPool,
    profile_id: Uuid,
) -> Result<Option<ConfigVersion>, SqlError> {
    let result = sqlx::query_as!(
        ConfigVersion,
        r#"
        SELECT
            id,
            profile_id,
            version,
            content as "content: serde_json::Value",
            checksum,
            created_by,
            created_at,
            message,
            is_active
        FROM config_versions
        WHERE profile_id = $1
        AND is_active = true
        "#,
        profile_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| SqlError::QueryError(e.to_string()))?;

    Ok(result)
}

/// Gets a specific version.
pub async fn get_version(
    pool: &PgPool,
    profile_id: Uuid,
    version_number: i32,
) -> Result<Option<ConfigVersion>, SqlError> {
    let result = sqlx::query_as!(
        ConfigVersion,
        r#"
        SELECT
            id,
            profile_id,
            version,
            content as "content: serde_json::Value",
            checksum,
            created_by,
            created_at,
            message,
            is_active
        FROM config_versions
        WHERE profile_id = $1
        AND version = $2
        "#,
        profile_id,
        version_number
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| SqlError::QueryError(e.to_string()))?;

    Ok(result)
}

/// Lists all applications.
pub async fn list_applications(pool: &PgPool) -> Result<Vec<Application>, SqlError> {
    let results = sqlx::query_as!(
        Application,
        r#"
        SELECT id, name, description, created_at, updated_at
        FROM applications
        ORDER BY name
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| SqlError::QueryError(e.to_string()))?;

    Ok(results)
}

/// Lists all profiles for an application.
pub async fn list_profiles(
    pool: &PgPool,
    app_name: &str,
) -> Result<Vec<String>, SqlError> {
    let results = sqlx::query_scalar!(
        r#"
        SELECT cp.profile
        FROM config_profiles cp
        JOIN applications a ON cp.application_id = a.id
        WHERE a.name = $1
        ORDER BY cp.profile
        "#,
        app_name
    )
    .fetch_all(pool)
    .await
    .map_err(|e| SqlError::QueryError(e.to_string()))?;

    Ok(results)
}

/// Creates or updates an application.
pub async fn upsert_application(
    pool: &PgPool,
    name: &str,
    description: Option<&str>,
) -> Result<Application, SqlError> {
    let result = sqlx::query_as!(
        Application,
        r#"
        INSERT INTO applications (name, description)
        VALUES ($1, $2)
        ON CONFLICT (name) DO UPDATE
        SET description = EXCLUDED.description,
            updated_at = NOW()
        RETURNING id, name, description, created_at, updated_at
        "#,
        name,
        description
    )
    .fetch_one(pool)
    .await
    .map_err(|e| SqlError::QueryError(e.to_string()))?;

    Ok(result)
}

/// Creates a new config version.
pub async fn create_version(
    pool: &PgPool,
    profile_id: Uuid,
    content: &serde_json::Value,
    checksum: &str,
    created_by: Option<&str>,
    message: Option<&str>,
    activate: bool,
) -> Result<ConfigVersion, SqlError> {
    let mut tx = pool.begin().await
        .map_err(|e| SqlError::TransactionError(e.to_string()))?;

    // Get next version number
    let next_version: i32 = sqlx::query_scalar!(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM config_versions WHERE profile_id = $1",
        profile_id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| SqlError::QueryError(e.to_string()))?
    .unwrap_or(1);

    // Deactivate current active version if we're activating this one
    if activate {
        sqlx::query!(
            "UPDATE config_versions SET is_active = false WHERE profile_id = $1 AND is_active = true",
            profile_id
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| SqlError::QueryError(e.to_string()))?;
    }

    // Insert new version
    let version = sqlx::query_as!(
        ConfigVersion,
        r#"
        INSERT INTO config_versions
            (profile_id, version, content, checksum, created_by, message, is_active)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING
            id,
            profile_id,
            version,
            content as "content: serde_json::Value",
            checksum,
            created_by,
            created_at,
            message,
            is_active
        "#,
        profile_id,
        next_version,
        content,
        checksum,
        created_by,
        message,
        activate
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| SqlError::QueryError(e.to_string()))?;

    tx.commit().await
        .map_err(|e| SqlError::TransactionError(e.to_string()))?;

    Ok(version)
}
```

### Paso 5: Implementar SqlConfigSource

```rust
// src/sql/source.rs
use async_trait::async_trait;
use sqlx::PgPool;
use crate::traits::ConfigSource;
use crate::types::{ConfigMap, PropertySource};
use crate::error::BackendError;
use super::config::SqlConfig;
use super::pool::create_pool;
use super::queries;

/// SQL-backed configuration source.
pub struct SqlConfigSource {
    pool: PgPool,
}

impl SqlConfigSource {
    /// Creates a new SqlConfigSource with the given configuration.
    pub async fn new(config: SqlConfig) -> Result<Self, BackendError> {
        let pool = create_pool(&config)
            .await
            .map_err(|e| BackendError::ConnectionError(e.to_string()))?;

        Ok(Self { pool })
    }

    /// Creates a SqlConfigSource from an existing pool.
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Returns a reference to the underlying pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Flattens JSON content into dotted keys.
    fn flatten_json(value: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        let mut result = serde_json::Map::new();
        Self::flatten_recursive(value, String::new(), &mut result);
        result
    }

    fn flatten_recursive(
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
                    Self::flatten_recursive(v, new_key, result);
                }
            }
            _ => {
                if !prefix.is_empty() {
                    result.insert(prefix, value.clone());
                }
            }
        }
    }
}

#[async_trait]
impl ConfigSource for SqlConfigSource {
    async fn get_config(
        &self,
        app: &str,
        profiles: &[String],
        _label: Option<&str>,
    ) -> Result<ConfigMap, BackendError> {
        tracing::debug!(
            app = %app,
            profiles = ?profiles,
            "Fetching config from SQL"
        );

        // Get application
        let application = queries::get_application_by_name(&self.pool, app)
            .await
            .map_err(|e| BackendError::QueryError(e.to_string()))?
            .ok_or_else(|| BackendError::NotFound(format!("Application '{}' not found", app)))?;

        // Build list of profiles to fetch (always include "default")
        let mut profile_names: Vec<String> = vec!["default".to_string()];
        for p in profiles {
            if p != "default" && !profile_names.contains(p) {
                profile_names.push(p.clone());
            }
        }

        // Get profiles
        let db_profiles = queries::get_profiles_by_app(&self.pool, application.id, &profile_names)
            .await
            .map_err(|e| BackendError::QueryError(e.to_string()))?;

        // Get active version for each profile
        let mut property_sources = Vec::new();

        for profile in db_profiles {
            if let Some(version) = queries::get_active_version(&self.pool, profile.id)
                .await
                .map_err(|e| BackendError::QueryError(e.to_string()))?
            {
                let flattened = Self::flatten_json(&version.content);

                let source = PropertySource {
                    name: format!("sql:{}:{}", app, profile.profile),
                    source: flattened,
                };

                property_sources.push(source);
            }
        }

        // Reverse so later profiles (higher priority) come first
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
        "sql"
    }
}

impl SqlConfigSource {
    /// Lists all applications.
    pub async fn list_applications(&self) -> Result<Vec<String>, BackendError> {
        let apps = queries::list_applications(&self.pool)
            .await
            .map_err(|e| BackendError::QueryError(e.to_string()))?;

        Ok(apps.into_iter().map(|a| a.name).collect())
    }

    /// Lists all profiles for an application.
    pub async fn list_profiles(&self, app: &str) -> Result<Vec<String>, BackendError> {
        queries::list_profiles(&self.pool, app)
            .await
            .map_err(|e| BackendError::QueryError(e.to_string()))
    }

    /// Gets config at a specific version.
    pub async fn get_config_version(
        &self,
        app: &str,
        profiles: &[String],
        version_number: i32,
    ) -> Result<ConfigMap, BackendError> {
        let application = queries::get_application_by_name(&self.pool, app)
            .await
            .map_err(|e| BackendError::QueryError(e.to_string()))?
            .ok_or_else(|| BackendError::NotFound(format!("Application '{}' not found", app)))?;

        let mut profile_names: Vec<String> = vec!["default".to_string()];
        profile_names.extend(profiles.iter().filter(|p| *p != "default").cloned());

        let db_profiles = queries::get_profiles_by_app(&self.pool, application.id, &profile_names)
            .await
            .map_err(|e| BackendError::QueryError(e.to_string()))?;

        let mut property_sources = Vec::new();

        for profile in db_profiles {
            if let Some(version) = queries::get_version(&self.pool, profile.id, version_number)
                .await
                .map_err(|e| BackendError::QueryError(e.to_string()))?
            {
                let flattened = Self::flatten_json(&version.content);

                property_sources.push(PropertySource {
                    name: format!("sql:{}:{}:v{}", app, profile.profile, version_number),
                    source: flattened,
                });
            }
        }

        property_sources.reverse();

        Ok(ConfigMap {
            name: app.to_string(),
            profiles: profiles.to_vec(),
            label: None,
            version: Some(version_number.to_string()),
            state: None,
            property_sources,
        })
    }
}
```

### Paso 6: Implementar Errores

```rust
// src/sql/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SqlError {
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Query error: {0}")]
    QueryError(String),

    #[error("Transaction error: {0}")]
    TransactionError(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),
}

impl From<sqlx::Error> for SqlError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => SqlError::NotFound("Row not found".to_string()),
            sqlx::Error::Database(db_err) => {
                if db_err.is_unique_violation() {
                    SqlError::ConstraintViolation(db_err.message().to_string())
                } else {
                    SqlError::QueryError(db_err.message().to_string())
                }
            }
            sqlx::Error::PoolTimedOut => {
                SqlError::ConnectionError("Connection pool timeout".to_string())
            }
            _ => SqlError::QueryError(err.to_string()),
        }
    }
}
```

---

## Conceptos de Rust Aprendidos

### 1. SQLx Compile-Time Verification

SQLx verifica queries contra la BD real durante compilacion.

**Rust:**
```rust
// Requires DATABASE_URL env var at compile time
// Query is checked against actual database schema
let app = sqlx::query_as!(
    Application,
    r#"
    SELECT id, name, description, created_at, updated_at
    FROM applications
    WHERE name = $1
    "#,
    app_name  // Type-checked: must be &str
)
.fetch_one(&pool)
.await?;

// Compile error if:
// - Table doesn't exist
// - Column doesn't exist
// - Type mismatch
// - Wrong number of parameters
```

**Error de compilacion ejemplo:**
```
error: error returned from database: column "descriptin" does not exist
   --> src/sql/queries.rs:15:5
    |
15  |       let app = sqlx::query_as!(
    |  _______________^
16  | |         Application,
17  | |         "SELECT id, name, descriptin FROM applications"
    | |                           ^^^^^^^^^^^ column typo!
```

**Comparacion con Java (sin verificacion compile-time):**
```java
// Java - errores descubiertos en runtime
@Query("SELECT * FROM applications WHERE name = :name")
Optional<Application> findByName(@Param("name") String name);

// Si la query tiene un typo, falla en runtime
@Query("SELECT * FROM applicatoins WHERE name = :name")  // Typo!
Optional<Application> findByName(@Param("name") String name);
// Compila pero falla al ejecutar
```

### 2. Connection Pooling con SQLx

**Rust:**
```rust
use sqlx::postgres::PgPoolOptions;

// Create pool with configuration
let pool = PgPoolOptions::new()
    .max_connections(10)
    .min_connections(2)
    .acquire_timeout(Duration::from_secs(30))
    .idle_timeout(Some(Duration::from_secs(600)))
    .max_lifetime(Some(Duration::from_secs(1800)))
    .connect("postgres://user:pass@localhost/db")
    .await?;

// Pool manages connections automatically
// Each query acquires from pool, returns when done
sqlx::query("SELECT 1")
    .execute(&pool)  // Borrow pool, don't consume
    .await?;

// Can check pool stats
println!("Pool size: {}", pool.size());
println!("Idle connections: {}", pool.num_idle());
```

**Comparacion con HikariCP (Java):**
```java
HikariConfig config = new HikariConfig();
config.setJdbcUrl("jdbc:postgresql://localhost/db");
config.setUsername("user");
config.setPassword("pass");
config.setMaximumPoolSize(10);
config.setMinimumIdle(2);
config.setConnectionTimeout(30000);
config.setIdleTimeout(600000);
config.setMaxLifetime(1800000);

HikariDataSource ds = new HikariDataSource(config);

// Get connection from pool
try (Connection conn = ds.getConnection()) {
    // Use connection
}  // Auto-returned to pool
```

### 3. Transactions en SQLx

**Rust:**
```rust
use sqlx::Acquire;

async fn atomic_operation(pool: &PgPool) -> Result<(), SqlError> {
    // Begin transaction
    let mut tx = pool.begin().await?;

    // Execute queries within transaction
    sqlx::query!("INSERT INTO applications (name) VALUES ($1)", "app1")
        .execute(&mut *tx)  // Note: &mut *tx to get mutable reference
        .await?;

    sqlx::query!("INSERT INTO config_profiles (application_id, profile) VALUES ($1, $2)",
        app_id, "default")
        .execute(&mut *tx)
        .await?;

    // If any error occurs before commit, transaction is rolled back on drop
    tx.commit().await?;

    Ok(())
}

// Transaction with savepoints
async fn with_savepoints(pool: &PgPool) -> Result<(), SqlError> {
    let mut tx = pool.begin().await?;

    sqlx::query!("INSERT ...").execute(&mut *tx).await?;

    // Create savepoint
    let savepoint = tx.begin().await?;

    match risky_operation(&mut *savepoint).await {
        Ok(_) => savepoint.commit().await?,
        Err(_) => {
            // Rolls back to savepoint, not entire transaction
            // savepoint is dropped here
        }
    }

    tx.commit().await?;
    Ok(())
}
```

**Comparacion con Java (Spring @Transactional):**
```java
@Transactional
public void atomicOperation() {
    applicationRepository.save(new Application("app1"));
    profileRepository.save(new ConfigProfile(app.getId(), "default"));
    // Commits automatically on success
    // Rolls back on exception
}

// With savepoints
@Transactional
public void withSavepoints() {
    applicationRepository.save(app);

    TransactionStatus status = transactionManager.getTransaction(
        new DefaultTransactionDefinition(TransactionDefinition.PROPAGATION_NESTED)
    );

    try {
        riskyOperation();
        transactionManager.commit(status);
    } catch (Exception e) {
        transactionManager.rollback(status);  // Only nested transaction
    }
}
```

### 4. Type Override en query_as!

**Rust:**
```rust
// Cuando el tipo inferido no coincide, usar type override
let version = sqlx::query_as!(
    ConfigVersion,
    r#"
    SELECT
        id,
        profile_id,
        version,
        content as "content: serde_json::Value",  -- Type override!
        checksum,
        created_by,
        created_at,
        message,
        is_active
    FROM config_versions
    WHERE id = $1
    "#,
    id
)
.fetch_one(pool)
.await?;

// Formatos de override:
// column as "name: Type"     - Rename and type
// column as "name: _"        - Rename, infer type
// column as "name!"          - Non-null override
// column as "name: Type!"    - Type + non-null
// column as "name?"          - Nullable override
```

---

## Riesgos y Errores Comunes

### 1. No Usar Transacciones para Multi-Step

```rust
// MAL: Sin transaccion, puede quedar inconsistente
async fn bad_create(pool: &PgPool, app: &str) -> Result<(), Error> {
    let app = sqlx::query!("INSERT INTO applications ...")
        .execute(pool)
        .await?;  // OK

    sqlx::query!("INSERT INTO config_profiles ...")
        .execute(pool)
        .await?;  // Si falla aqui, app queda sin profile!

    Ok(())
}

// BIEN: Con transaccion
async fn good_create(pool: &PgPool, app: &str) -> Result<(), Error> {
    let mut tx = pool.begin().await?;

    let app = sqlx::query!("INSERT INTO applications ...")
        .execute(&mut *tx)
        .await?;

    sqlx::query!("INSERT INTO config_profiles ...")
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;  // Atomic

    Ok(())
}
```

### 2. N+1 Queries

```rust
// MAL: N+1 queries
async fn bad_load_all(pool: &PgPool) -> Result<Vec<ConfigWithVersions>, Error> {
    let apps = sqlx::query_as!(Application, "SELECT * FROM applications")
        .fetch_all(pool)
        .await?;

    let mut results = Vec::new();
    for app in apps {
        // Query por cada app!
        let profiles = sqlx::query_as!(Profile, "SELECT * FROM profiles WHERE app_id = $1", app.id)
            .fetch_all(pool)
            .await?;

        results.push(ConfigWithVersions { app, profiles });
    }

    Ok(results)
}

// BIEN: Single query con JOIN
async fn good_load_all(pool: &PgPool) -> Result<Vec<ConfigWithVersions>, Error> {
    let rows = sqlx::query!(
        r#"
        SELECT
            a.id as app_id,
            a.name as app_name,
            p.id as profile_id,
            p.profile
        FROM applications a
        LEFT JOIN config_profiles p ON p.application_id = a.id
        ORDER BY a.name, p.profile
        "#
    )
    .fetch_all(pool)
    .await?;

    // Group by app_id
    // ...
}
```

### 3. Pool Starvation

```rust
// MAL: Holding connection too long
async fn bad_long_operation(pool: &PgPool) -> Result<(), Error> {
    let mut tx = pool.begin().await?;  // Takes connection

    // Long external operation while holding connection
    external_api_call().await?;  // 5 seconds!

    sqlx::query!("UPDATE ...").execute(&mut *tx).await?;
    tx.commit().await?;

    Ok(())
}

// BIEN: Minimize connection time
async fn good_long_operation(pool: &PgPool) -> Result<(), Error> {
    // Do external work first
    let data = external_api_call().await?;

    // Then quick database transaction
    let mut tx = pool.begin().await?;
    sqlx::query!("UPDATE ...").execute(&mut *tx).await?;
    tx.commit().await?;

    Ok(())
}
```

### 4. Missing DATABASE_URL

```bash
# Error de compilacion si DATABASE_URL no esta set
error: `DATABASE_URL` must be set to run compile-time checks

# Solucion 1: Set env var
export DATABASE_URL=postgres://user:pass@localhost/db

# Solucion 2: Usar .env file
echo 'DATABASE_URL=postgres://user:pass@localhost/db' > .env

# Solucion 3: Offline mode (usa sqlx-data.json)
cargo sqlx prepare
cargo build --release
```

---

## Pruebas

### Tests con sqlx::test

```rust
// tests/postgres_test.rs
use sqlx::PgPool;
use vortex_backends::sql::{SqlConfig, SqlConfigSource};

#[sqlx::test]
async fn get_config_returns_active_version(pool: PgPool) {
    // Setup: create test data
    let app_id = create_test_app(&pool, "test-app").await;
    let profile_id = create_test_profile(&pool, app_id, "default").await;

    // Create two versions, activate second
    create_test_version(&pool, profile_id, 1, json!({"port": 8080}), false).await;
    create_test_version(&pool, profile_id, 2, json!({"port": 9090}), true).await;

    // Test
    let source = SqlConfigSource::from_pool(pool);
    let config = source
        .get_config("test-app", &["default".to_string()], None)
        .await
        .unwrap();

    assert_eq!(config.property_sources.len(), 1);
    assert_eq!(config.property_sources[0].source["port"], 9090);
}

#[sqlx::test]
async fn get_config_merges_profiles(pool: PgPool) {
    let app_id = create_test_app(&pool, "myapp").await;

    let default_profile = create_test_profile(&pool, app_id, "default").await;
    create_test_version(&pool, default_profile, 1, json!({
        "server.port": 8080,
        "server.host": "localhost"
    }), true).await;

    let prod_profile = create_test_profile(&pool, app_id, "prod").await;
    create_test_version(&pool, prod_profile, 1, json!({
        "server.port": 9090,  // Override
        "server.ssl": true    // New property
    }), true).await;

    let source = SqlConfigSource::from_pool(pool);
    let config = source
        .get_config("myapp", &["prod".to_string()], None)
        .await
        .unwrap();

    // prod should come first (higher priority)
    assert_eq!(config.property_sources[0].source["server.port"], 9090);
    assert_eq!(config.property_sources[0].source["server.ssl"], true);

    // default should have the base values
    assert_eq!(config.property_sources[1].source["server.host"], "localhost");
}

#[sqlx::test]
async fn list_applications_returns_all(pool: PgPool) {
    create_test_app(&pool, "app1").await;
    create_test_app(&pool, "app2").await;
    create_test_app(&pool, "app3").await;

    let source = SqlConfigSource::from_pool(pool);
    let apps = source.list_applications().await.unwrap();

    assert_eq!(apps.len(), 3);
    assert!(apps.contains(&"app1".to_string()));
}

// Helper functions
async fn create_test_app(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar!(
        "INSERT INTO applications (name) VALUES ($1) RETURNING id",
        name
    )
    .fetch_one(pool)
    .await
    .unwrap()
}
```

### Tests con Testcontainers

```rust
// tests/postgres_container_test.rs
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

#[tokio::test]
async fn integration_test_with_container() {
    // Start PostgreSQL container
    let container = Postgres::default().start().await;

    let port = container.get_host_port_ipv4(5432).await;
    let conn_string = format!(
        "postgres://postgres:postgres@localhost:{}/postgres",
        port
    );

    // Run migrations
    let pool = PgPool::connect(&conn_string).await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // Create source and test
    let config = SqlConfig::new(&conn_string);
    let source = SqlConfigSource::new(config).await.unwrap();

    // ... test operations
}
```

---

## Observabilidad

### Logging de Queries

```rust
impl SqlConfigSource {
    pub async fn get_config(&self, app: &str, ...) -> Result<...> {
        let span = tracing::info_span!(
            "sql_get_config",
            app = %app,
            profiles = ?profiles
        );

        async {
            let start = std::time::Instant::now();

            let result = self.execute_query(app, profiles).await;

            tracing::info!(
                duration_ms = start.elapsed().as_millis(),
                success = result.is_ok(),
                "SQL query completed"
            );

            result
        }
        .instrument(span)
        .await
    }
}
```

### Metricas de Pool

```rust
// Exponer metricas del pool
pub fn collect_pool_metrics(pool: &PgPool) -> PoolMetrics {
    PoolMetrics {
        size: pool.size(),
        idle: pool.num_idle(),
        in_use: pool.size() - pool.num_idle() as u32,
    }
}

// Prometheus metrics
sql_pool_connections_total{state="idle"}
sql_pool_connections_total{state="in_use"}
sql_query_duration_seconds{operation="get_config"}
sql_query_total{operation, status}
```

---

## Seguridad

### Connection String Segura

```rust
// NUNCA hardcodear credenciales
// MAL:
let config = SqlConfig::new("postgres://admin:SuperSecret123@prod-db:5432/vortex");

// BIEN: Usar environment variables
let conn_string = std::env::var("DATABASE_URL")
    .expect("DATABASE_URL must be set");
let config = SqlConfig::new(conn_string);

// MEJOR: Usar secrets manager
let password = secrets_manager.get_secret("db-password").await?;
let conn_string = format!("postgres://app:{}@db:5432/vortex", password);
```

### Evitar SQL Injection

```rust
// SQLx previene SQL injection por diseno
// Los parametros siempre son $1, $2, etc.

// BIEN: Parametros
sqlx::query!("SELECT * FROM apps WHERE name = $1", user_input)

// NUNCA hacer esto (aunque SQLx no lo permite directamente):
// format!("SELECT * FROM apps WHERE name = '{}'", user_input)
```

---

## Entregable Final

### Archivos Creados

1. `crates/vortex-backends/src/sql/mod.rs` - Modulo SQL
2. `crates/vortex-backends/src/sql/config.rs` - SqlConfig
3. `crates/vortex-backends/src/sql/pool.rs` - Connection pool
4. `crates/vortex-backends/src/sql/queries.rs` - Query functions
5. `crates/vortex-backends/src/sql/source.rs` - SqlConfigSource
6. `crates/vortex-backends/src/sql/error.rs` - SQL errors
7. `crates/vortex-backends/tests/postgres_test.rs` - Tests

### Verificacion

```bash
# Set DATABASE_URL for compile-time checks
export DATABASE_URL=postgres://user:pass@localhost/vortex

# Compilar
cargo build -p vortex-backends --features postgres

# Preparar para offline builds
cargo sqlx prepare -p vortex-backends

# Tests
cargo test -p vortex-backends --features postgres

# Con container
docker run -d -p 5432:5432 -e POSTGRES_PASSWORD=postgres postgres:14
cargo test -p vortex-backends --features postgres
```

### Ejemplo de Uso

```rust
use vortex_backends::sql::{SqlConfig, SqlConfigSource};
use vortex_backends::ConfigSource;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = SqlConfig::new("postgres://user:pass@localhost/vortex")
        .max_connections(20)
        .min_connections(5);

    let source = SqlConfigSource::new(config).await?;

    // List applications
    let apps = source.list_applications().await?;
    println!("Applications: {:?}", apps);

    // Get config
    let config = source
        .get_config("payment-service", &["production".to_string()], None)
        .await?;

    println!("Config has {} property sources", config.property_sources.len());

    for source in &config.property_sources {
        println!("  Source: {}", source.name);
        for (key, value) in &source.source {
            println!("    {} = {}", key, value);
        }
    }

    Ok(())
}
```
