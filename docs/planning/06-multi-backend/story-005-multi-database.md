# Historia 005: Soporte Multi-Database

## Contexto y Objetivo

Esta historia extiende el backend SQL para soportar multiples motores de base de datos: PostgreSQL, MySQL y SQLite. Utilizando feature flags de Cargo y abstracciones apropiadas, permitimos que los usuarios elijan el motor que mejor se adapte a su infraestructura.

**Casos de uso:**

- **PostgreSQL**: Produccion enterprise, features avanzadas
- **MySQL**: Compatibilidad con stacks LAMP existentes
- **SQLite**: Desarrollo local, testing, embedded deployments

El reto principal es mantener codigo compartido mientras se acomodan las diferencias entre motores SQL.

---

## Alcance

### In Scope

- Feature flags para postgres, mysql, sqlite
- Abstracciones para diferencias SQL entre motores
- Migrations especificas por motor
- Tests para cada motor
- Documentacion de diferencias

### Out of Scope

- Soporte para otros motores (Oracle, SQL Server)
- Cross-database transactions
- Automatic failover entre motores
- Schema migration tools (solo SQLx migrations)

---

## Criterios de Aceptacion

- [ ] Feature flag `postgres` habilita PostgreSQL
- [ ] Feature flag `mysql` habilita MySQL
- [ ] Feature flag `sqlite` habilita SQLite
- [ ] Compilar con cualquier combinacion de features
- [ ] `SqlConfigSource` funciona con cualquier motor habilitado
- [ ] Migrations automaticas por motor
- [ ] Tests pasan para cada motor individualmente
- [ ] Documentacion de diferencias SQL

---

## Diseno Propuesto

### Feature Flags

```toml
# Cargo.toml
[features]
default = []

# Individual database backends
postgres = ["sqlx/postgres"]
mysql = ["sqlx/mysql"]
sqlite = ["sqlx/sqlite"]

# Convenience features
sql = ["postgres"]  # Default SQL is PostgreSQL
all-sql = ["postgres", "mysql", "sqlite"]
```

### Estructura de Modulos

```
crates/vortex-backends/
├── src/
│   ├── sql/
│   │   ├── mod.rs              # Re-exports, feature gates
│   │   ├── config.rs           # SqlConfig (generic)
│   │   ├── source.rs           # SqlConfigSource trait
│   │   ├── error.rs            # Errors
│   │   ├── postgres/           # PostgreSQL specific
│   │   │   ├── mod.rs
│   │   │   ├── pool.rs
│   │   │   └── queries.rs
│   │   ├── mysql/              # MySQL specific
│   │   │   ├── mod.rs
│   │   │   ├── pool.rs
│   │   │   └── queries.rs
│   │   └── sqlite/             # SQLite specific
│   │       ├── mod.rs
│   │       ├── pool.rs
│   │       └── queries.rs
├── migrations/
│   ├── postgres/
│   ├── mysql/
│   └── sqlite/
└── tests/
    ├── postgres_test.rs
    ├── mysql_test.rs
    └── sqlite_test.rs
```

### Diagrama de Arquitectura

```
┌────────────────────────────────────────────────────────┐
│                 SqlConfigSource<DB>                     │
│           impl ConfigSource for SqlConfigSource         │
├────────────────────────────────────────────────────────┤
│  Generic over DB: sqlx::Database                        │
│  Common logic: get_config, list_apps, etc.             │
└─────────────────────────┬──────────────────────────────┘
                          │
         ┌────────────────┼────────────────┐
         │                │                │
         ▼                ▼                ▼
┌─────────────┐   ┌─────────────┐   ┌─────────────┐
│  PostgreSQL │   │    MySQL    │   │   SQLite    │
│    Pool     │   │    Pool     │   │    Pool     │
├─────────────┤   ├─────────────┤   ├─────────────┤
│ JSONB       │   │ JSON        │   │ TEXT/JSON   │
│ uuid-ossp   │   │ UUID()      │   │ randomblob  │
│ TIMESTAMPTZ │   │ TIMESTAMP   │   │ TEXT        │
└─────────────┘   └─────────────┘   └─────────────┘

#[cfg(feature = "postgres")]
#[cfg(feature = "mysql")]
#[cfg(feature = "sqlite")]
```

---

## Pasos de Implementacion

### Paso 1: Configurar Feature Flags

```toml
# Cargo.toml
[package]
name = "vortex-backends"
version = "0.1.0"
edition = "2024"

[dependencies]
# Core dependencies (always included)
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
tracing = "0.1"
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }

# SQLx with conditional features
sqlx = { version = "0.8", optional = true, default-features = false, features = [
    "runtime-tokio",
    "uuid",
    "chrono",
    "json"
]}

[features]
default = []

# PostgreSQL support
postgres = ["sqlx", "sqlx/postgres"]

# MySQL support
mysql = ["sqlx", "sqlx/mysql"]

# SQLite support
sqlite = ["sqlx", "sqlx/sqlite"]

# Convenience
sql = ["postgres"]
all-sql = ["postgres", "mysql", "sqlite"]
```

### Paso 2: Implementar Abstraccion de Database

```rust
// src/sql/database.rs
use sqlx::Database;

/// Marker trait for supported databases.
pub trait SupportedDatabase: Database {
    /// Returns the database type name.
    fn name() -> &'static str;

    /// Returns the parameter placeholder format.
    /// PostgreSQL: $1, $2
    /// MySQL: ?, ?
    /// SQLite: ?, ?
    fn placeholder(index: usize) -> String;
}

#[cfg(feature = "postgres")]
impl SupportedDatabase for sqlx::Postgres {
    fn name() -> &'static str {
        "PostgreSQL"
    }

    fn placeholder(index: usize) -> String {
        format!("${}", index)
    }
}

#[cfg(feature = "mysql")]
impl SupportedDatabase for sqlx::MySql {
    fn name() -> &'static str {
        "MySQL"
    }

    fn placeholder(_index: usize) -> String {
        "?".to_string()
    }
}

#[cfg(feature = "sqlite")]
impl SupportedDatabase for sqlx::Sqlite {
    fn name() -> &'static str {
        "SQLite"
    }

    fn placeholder(_index: usize) -> String {
        "?".to_string()
    }
}
```

### Paso 3: Implementar Pool Generico

```rust
// src/sql/pool.rs
use std::time::Duration;

/// Configuration for SQL database connection.
#[derive(Debug, Clone)]
pub struct SqlConfig {
    pub connection_string: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
}

impl SqlConfig {
    pub fn new(connection_string: impl Into<String>) -> Self {
        Self {
            connection_string: connection_string.into(),
            max_connections: 10,
            min_connections: 1,
            connect_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(600),
        }
    }
}

/// Creates a connection pool for PostgreSQL.
#[cfg(feature = "postgres")]
pub async fn create_postgres_pool(
    config: &SqlConfig,
) -> Result<sqlx::PgPool, crate::error::BackendError> {
    use sqlx::postgres::PgPoolOptions;

    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(config.connect_timeout)
        .idle_timeout(Some(config.idle_timeout))
        .connect(&config.connection_string)
        .await
        .map_err(|e| crate::error::BackendError::ConnectionError(e.to_string()))?;

    Ok(pool)
}

/// Creates a connection pool for MySQL.
#[cfg(feature = "mysql")]
pub async fn create_mysql_pool(
    config: &SqlConfig,
) -> Result<sqlx::MySqlPool, crate::error::BackendError> {
    use sqlx::mysql::MySqlPoolOptions;

    let pool = MySqlPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(config.connect_timeout)
        .idle_timeout(Some(config.idle_timeout))
        .connect(&config.connection_string)
        .await
        .map_err(|e| crate::error::BackendError::ConnectionError(e.to_string()))?;

    Ok(pool)
}

/// Creates a connection pool for SQLite.
#[cfg(feature = "sqlite")]
pub async fn create_sqlite_pool(
    config: &SqlConfig,
) -> Result<sqlx::SqlitePool, crate::error::BackendError> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    let options = SqliteConnectOptions::from_str(&config.connection_string)
        .map_err(|e| crate::error::BackendError::ConnectionError(e.to_string()))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

    let pool = SqlitePoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(config.connect_timeout)
        .idle_timeout(Some(config.idle_timeout))
        .connect_with(options)
        .await
        .map_err(|e| crate::error::BackendError::ConnectionError(e.to_string()))?;

    Ok(pool)
}
```

### Paso 4: Implementar Queries Especificas

```rust
// src/sql/postgres/queries.rs
#[cfg(feature = "postgres")]
use sqlx::PgPool;

#[cfg(feature = "postgres")]
pub async fn get_application_by_name(
    pool: &PgPool,
    name: &str,
) -> Result<Option<Application>, SqlError> {
    // PostgreSQL-specific query with JSONB support
    sqlx::query_as!(
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
    .map_err(|e| SqlError::QueryError(e.to_string()))
}

#[cfg(feature = "postgres")]
pub async fn get_active_version(
    pool: &PgPool,
    profile_id: Uuid,
) -> Result<Option<ConfigVersion>, SqlError> {
    sqlx::query_as!(
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
    .map_err(|e| SqlError::QueryError(e.to_string()))
}
```

```rust
// src/sql/mysql/queries.rs
#[cfg(feature = "mysql")]
use sqlx::MySqlPool;

#[cfg(feature = "mysql")]
pub async fn get_application_by_name(
    pool: &MySqlPool,
    name: &str,
) -> Result<Option<Application>, SqlError> {
    // MySQL uses ? placeholders
    sqlx::query_as!(
        Application,
        r#"
        SELECT id, name, description, created_at, updated_at
        FROM applications
        WHERE name = ?
        "#,
        name
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| SqlError::QueryError(e.to_string()))
}

#[cfg(feature = "mysql")]
pub async fn get_active_version(
    pool: &MySqlPool,
    profile_id: String,  // MySQL uses CHAR(36) for UUIDs
) -> Result<Option<ConfigVersion>, SqlError> {
    // MySQL JSON column
    sqlx::query_as!(
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
        WHERE profile_id = ?
        AND is_active = 1
        "#,
        profile_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| SqlError::QueryError(e.to_string()))
}
```

```rust
// src/sql/sqlite/queries.rs
#[cfg(feature = "sqlite")]
use sqlx::SqlitePool;

#[cfg(feature = "sqlite")]
pub async fn get_application_by_name(
    pool: &SqlitePool,
    name: &str,
) -> Result<Option<Application>, SqlError> {
    sqlx::query_as!(
        Application,
        r#"
        SELECT id, name, description, created_at, updated_at
        FROM applications
        WHERE name = ?
        "#,
        name
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| SqlError::QueryError(e.to_string()))
}

#[cfg(feature = "sqlite")]
pub async fn get_active_version(
    pool: &SqlitePool,
    profile_id: String,
) -> Result<Option<ConfigVersion>, SqlError> {
    // SQLite stores JSON as TEXT, parse manually
    let row = sqlx::query!(
        r#"
        SELECT
            id,
            profile_id,
            version,
            content,
            checksum,
            created_by,
            created_at,
            message,
            is_active
        FROM config_versions
        WHERE profile_id = ?
        AND is_active = 1
        "#,
        profile_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| SqlError::QueryError(e.to_string()))?;

    match row {
        Some(r) => {
            let content: serde_json::Value = serde_json::from_str(&r.content)
                .map_err(|e| SqlError::ParseError(e.to_string()))?;

            Ok(Some(ConfigVersion {
                id: Uuid::parse_str(&r.id).unwrap(),
                profile_id: Uuid::parse_str(&r.profile_id).unwrap(),
                version: r.version,
                content,
                checksum: r.checksum,
                created_by: r.created_by,
                created_at: parse_sqlite_timestamp(&r.created_at)?,
                message: r.message,
                is_active: r.is_active != 0,
            }))
        }
        None => Ok(None),
    }
}
```

### Paso 5: Implementar SqlConfigSource Multi-Database

```rust
// src/sql/source.rs
use async_trait::async_trait;
use crate::traits::ConfigSource;
use crate::types::{ConfigMap, PropertySource};
use crate::error::BackendError;

/// Enum to hold different pool types.
pub enum SqlPool {
    #[cfg(feature = "postgres")]
    Postgres(sqlx::PgPool),

    #[cfg(feature = "mysql")]
    MySql(sqlx::MySqlPool),

    #[cfg(feature = "sqlite")]
    Sqlite(sqlx::SqlitePool),
}

/// SQL-backed configuration source supporting multiple databases.
pub struct SqlConfigSource {
    pool: SqlPool,
}

impl SqlConfigSource {
    /// Creates a new SqlConfigSource from configuration.
    pub async fn new(config: SqlConfig) -> Result<Self, BackendError> {
        let pool = Self::create_pool(&config).await?;
        Ok(Self { pool })
    }

    async fn create_pool(config: &SqlConfig) -> Result<SqlPool, BackendError> {
        let conn = &config.connection_string;

        // Detect database type from connection string
        if conn.starts_with("postgres://") || conn.starts_with("postgresql://") {
            #[cfg(feature = "postgres")]
            {
                let pool = super::pool::create_postgres_pool(config).await?;
                return Ok(SqlPool::Postgres(pool));
            }
            #[cfg(not(feature = "postgres"))]
            {
                return Err(BackendError::UnsupportedDatabase(
                    "PostgreSQL support not enabled. Compile with --features postgres".into()
                ));
            }
        }

        if conn.starts_with("mysql://") {
            #[cfg(feature = "mysql")]
            {
                let pool = super::pool::create_mysql_pool(config).await?;
                return Ok(SqlPool::MySql(pool));
            }
            #[cfg(not(feature = "mysql"))]
            {
                return Err(BackendError::UnsupportedDatabase(
                    "MySQL support not enabled. Compile with --features mysql".into()
                ));
            }
        }

        if conn.starts_with("sqlite://") || conn.ends_with(".db") || conn.ends_with(".sqlite") {
            #[cfg(feature = "sqlite")]
            {
                let pool = super::pool::create_sqlite_pool(config).await?;
                return Ok(SqlPool::Sqlite(pool));
            }
            #[cfg(not(feature = "sqlite"))]
            {
                return Err(BackendError::UnsupportedDatabase(
                    "SQLite support not enabled. Compile with --features sqlite".into()
                ));
            }
        }

        Err(BackendError::UnsupportedDatabase(
            "Unknown database type. Connection string should start with postgres://, mysql://, or sqlite://".into()
        ))
    }

    /// Returns the database type name.
    pub fn database_type(&self) -> &'static str {
        match &self.pool {
            #[cfg(feature = "postgres")]
            SqlPool::Postgres(_) => "PostgreSQL",
            #[cfg(feature = "mysql")]
            SqlPool::MySql(_) => "MySQL",
            #[cfg(feature = "sqlite")]
            SqlPool::Sqlite(_) => "SQLite",
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
        match &self.pool {
            #[cfg(feature = "postgres")]
            SqlPool::Postgres(pool) => {
                self.get_config_postgres(pool, app, profiles).await
            }

            #[cfg(feature = "mysql")]
            SqlPool::MySql(pool) => {
                self.get_config_mysql(pool, app, profiles).await
            }

            #[cfg(feature = "sqlite")]
            SqlPool::Sqlite(pool) => {
                self.get_config_sqlite(pool, app, profiles).await
            }
        }
    }

    fn name(&self) -> &str {
        "sql"
    }
}

impl SqlConfigSource {
    #[cfg(feature = "postgres")]
    async fn get_config_postgres(
        &self,
        pool: &sqlx::PgPool,
        app: &str,
        profiles: &[String],
    ) -> Result<ConfigMap, BackendError> {
        use super::postgres::queries;

        let application = queries::get_application_by_name(pool, app)
            .await
            .map_err(|e| BackendError::QueryError(e.to_string()))?
            .ok_or_else(|| BackendError::NotFound(format!("App '{}' not found", app)))?;

        // ... rest of implementation
        todo!()
    }

    #[cfg(feature = "mysql")]
    async fn get_config_mysql(
        &self,
        pool: &sqlx::MySqlPool,
        app: &str,
        profiles: &[String],
    ) -> Result<ConfigMap, BackendError> {
        use super::mysql::queries;
        // Similar implementation
        todo!()
    }

    #[cfg(feature = "sqlite")]
    async fn get_config_sqlite(
        &self,
        pool: &sqlx::SqlitePool,
        app: &str,
        profiles: &[String],
    ) -> Result<ConfigMap, BackendError> {
        use super::sqlite::queries;
        // Similar implementation
        todo!()
    }
}
```

### Paso 6: Configurar Modulo con Conditional Compilation

```rust
// src/sql/mod.rs
//! SQL backend module supporting PostgreSQL, MySQL, and SQLite.
//!
//! Enable specific databases with feature flags:
//! - `postgres`: PostgreSQL support
//! - `mysql`: MySQL support
//! - `sqlite`: SQLite support

mod config;
mod error;
mod pool;
mod source;

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "mysql")]
pub mod mysql;

#[cfg(feature = "sqlite")]
pub mod sqlite;

pub use config::SqlConfig;
pub use error::SqlError;
pub use source::{SqlConfigSource, SqlPool};

// Conditional re-exports
#[cfg(feature = "postgres")]
pub use pool::create_postgres_pool;

#[cfg(feature = "mysql")]
pub use pool::create_mysql_pool;

#[cfg(feature = "sqlite")]
pub use pool::create_sqlite_pool;

/// Check which databases are available at compile time.
pub fn available_databases() -> Vec<&'static str> {
    let mut dbs = Vec::new();

    #[cfg(feature = "postgres")]
    dbs.push("PostgreSQL");

    #[cfg(feature = "mysql")]
    dbs.push("MySQL");

    #[cfg(feature = "sqlite")]
    dbs.push("SQLite");

    dbs
}
```

---

## Conceptos de Rust Aprendidos

### 1. Feature Flags y Conditional Compilation

Feature flags permiten compilacion condicional en Rust.

**Rust:**

```rust
// Cargo.toml
[features]
default = []
postgres = ["sqlx/postgres"]
mysql = ["sqlx/mysql"]

// En codigo
#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "mysql")]
pub mod mysql;

// Conditional dentro de funcion
fn create_pool(config: &Config) -> Pool {
    #[cfg(feature = "postgres")]
    {
        return create_postgres_pool(config);
    }

    #[cfg(feature = "mysql")]
    {
        return create_mysql_pool(config);
    }

    #[cfg(not(any(feature = "postgres", feature = "mysql")))]
    compile_error!("At least one database feature must be enabled");
}

// cfg_if macro para logica compleja
cfg_if::cfg_if! {
    if #[cfg(feature = "postgres")] {
        type DefaultPool = PgPool;
    } else if #[cfg(feature = "mysql")] {
        type DefaultPool = MySqlPool;
    } else if #[cfg(feature = "sqlite")] {
        type DefaultPool = SqlitePool;
    }
}
```

**Comparacion con Java (Maven Profiles):**

```xml
<!-- pom.xml -->
<profiles>
    <profile>
        <id>postgres</id>
        <dependencies>
            <dependency>
                <groupId>org.postgresql</groupId>
                <artifactId>postgresql</artifactId>
            </dependency>
        </dependencies>
    </profile>
    <profile>
        <id>mysql</id>
        <dependencies>
            <dependency>
                <groupId>mysql</groupId>
                <artifactId>mysql-connector-java</artifactId>
            </dependency>
        </dependencies>
    </profile>
</profiles>
```

```java
// En Java, runtime configuration
@Configuration
@ConditionalOnProperty(name = "db.type", havingValue = "postgres")
public class PostgresConfig {
    // ...
}

@Configuration
@ConditionalOnProperty(name = "db.type", havingValue = "mysql")
public class MySqlConfig {
    // ...
}
```

**Diferencias clave:**

| Aspecto | Rust Features | Java Profiles |
|---------|--------------|---------------|
| Cuando | Compile-time | Build-time + Runtime |
| Binario | Codigo no incluido | Todo incluido |
| Verificacion | Compiler | Runtime reflection |
| Tamano | Mas pequeno | Mas grande |

### 2. Enum para Abstraer Tipos de Pool

**Rust:**

```rust
// Enum para diferentes tipos de pool
pub enum SqlPool {
    #[cfg(feature = "postgres")]
    Postgres(sqlx::PgPool),

    #[cfg(feature = "mysql")]
    MySql(sqlx::MySqlPool),

    #[cfg(feature = "sqlite")]
    Sqlite(sqlx::SqlitePool),
}

impl SqlPool {
    pub fn database_name(&self) -> &'static str {
        match self {
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => "PostgreSQL",

            #[cfg(feature = "mysql")]
            Self::MySql(_) => "MySQL",

            #[cfg(feature = "sqlite")]
            Self::Sqlite(_) => "SQLite",
        }
    }

    pub async fn health_check(&self) -> Result<(), Error> {
        match self {
            #[cfg(feature = "postgres")]
            Self::Postgres(pool) => {
                sqlx::query("SELECT 1").execute(pool).await?;
            }

            #[cfg(feature = "mysql")]
            Self::MySql(pool) => {
                sqlx::query("SELECT 1").execute(pool).await?;
            }

            #[cfg(feature = "sqlite")]
            Self::Sqlite(pool) => {
                sqlx::query("SELECT 1").execute(pool).await?;
            }
        }
        Ok(())
    }
}
```

### 3. #[cfg] vs cfg!() Macro

```rust
// #[cfg] - Conditional compilation (codigo no existe si false)
#[cfg(feature = "postgres")]
fn postgres_only() {
    // This function doesn't exist without postgres feature
}

// cfg!() - Runtime check (codigo siempre existe)
fn runtime_check() {
    if cfg!(feature = "postgres") {
        println!("PostgreSQL is enabled");
    } else {
        println!("PostgreSQL is not enabled");
    }
}

// #[cfg_attr] - Conditional attributes
#[cfg_attr(feature = "postgres", derive(sqlx::FromRow))]
struct MyStruct {
    id: i32,
}

// Combining conditions
#[cfg(all(feature = "postgres", not(feature = "mysql")))]
fn postgres_exclusive() {}

#[cfg(any(feature = "postgres", feature = "mysql"))]
fn either_database() {}
```

### 4. compile_error! para Validacion

```rust
// Ensure at least one database is enabled
#[cfg(not(any(feature = "postgres", feature = "mysql", feature = "sqlite")))]
compile_error!(
    "At least one database feature must be enabled. \
     Use --features postgres, --features mysql, or --features sqlite"
);

// Ensure incompatible features aren't combined
#[cfg(all(feature = "postgres-only", feature = "mysql-only"))]
compile_error!("Cannot enable both postgres-only and mysql-only");
```

---

## Riesgos y Errores Comunes

### 1. Diferencias SQL Entre Motores

```sql
-- PostgreSQL: UUID nativo
SELECT uuid_generate_v4();

-- MySQL: String UUID
SELECT UUID();

-- SQLite: Custom function
SELECT lower(hex(randomblob(16)));
```

```sql
-- PostgreSQL: JSONB con operadores
SELECT * FROM configs WHERE content @> '{"env": "prod"}';

-- MySQL: JSON con funciones
SELECT * FROM configs WHERE JSON_CONTAINS(content, '"prod"', '$.env');

-- SQLite: TEXT, parse en app
SELECT * FROM configs WHERE json_extract(content, '$.env') = 'prod';
```

### 2. Placeholder Syntax

```rust
// MAL: Hardcoded placeholders
let query = "SELECT * FROM apps WHERE name = $1";  // Solo PostgreSQL!

// BIEN: Usar sqlx::query! que adapta automaticamente
sqlx::query!("SELECT * FROM apps WHERE name = $1", name)  // PostgreSQL
sqlx::query!("SELECT * FROM apps WHERE name = ?", name)   // MySQL/SQLite

// O funcion helper
fn placeholder(db: DatabaseType, index: usize) -> String {
    match db {
        DatabaseType::Postgres => format!("${}", index),
        DatabaseType::MySql | DatabaseType::Sqlite => "?".to_string(),
    }
}
```

### 3. Feature Flag Combinations

```rust
// MAL: Puede compilar sin ningun backend
pub fn create_pool() -> Pool {
    #[cfg(feature = "postgres")]
    return create_pg_pool();

    #[cfg(feature = "mysql")]
    return create_mysql_pool();

    // Si ninguno esta habilitado, error de compilacion críptico
}

// BIEN: Error explicito
pub fn create_pool() -> Pool {
    #[cfg(feature = "postgres")]
    {
        return create_pg_pool();
    }

    #[cfg(feature = "mysql")]
    {
        return create_mysql_pool();
    }

    #[cfg(not(any(feature = "postgres", feature = "mysql")))]
    {
        compile_error!("Enable at least one database feature");
    }
}
```

### 4. Testing con Multiples Features

```bash
# MAL: Solo testear con default features
cargo test

# BIEN: Testear cada combinacion
cargo test --features postgres
cargo test --features mysql
cargo test --features sqlite
cargo test --features "postgres,mysql"
cargo test --all-features
```

---

## Pruebas

### Tests por Database

```rust
// tests/postgres_test.rs
#![cfg(feature = "postgres")]

use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

#[tokio::test]
async fn postgres_get_config_works() {
    let container = Postgres::default().start().await;
    let port = container.get_host_port_ipv4(5432).await;
    let url = format!("postgres://postgres:postgres@localhost:{}/postgres", port);

    let config = SqlConfig::new(&url);
    let source = SqlConfigSource::new(config).await.unwrap();

    assert_eq!(source.database_type(), "PostgreSQL");
}
```

```rust
// tests/mysql_test.rs
#![cfg(feature = "mysql")]

use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mysql::Mysql;

#[tokio::test]
async fn mysql_get_config_works() {
    let container = Mysql::default().start().await;
    let port = container.get_host_port_ipv4(3306).await;
    let url = format!("mysql://root:mysql@localhost:{}/mysql", port);

    let config = SqlConfig::new(&url);
    let source = SqlConfigSource::new(config).await.unwrap();

    assert_eq!(source.database_type(), "MySQL");
}
```

```rust
// tests/sqlite_test.rs
#![cfg(feature = "sqlite")]

use tempfile::NamedTempFile;

#[tokio::test]
async fn sqlite_get_config_works() {
    let temp_file = NamedTempFile::new().unwrap();
    let url = format!("sqlite://{}", temp_file.path().display());

    let config = SqlConfig::new(&url);
    let source = SqlConfigSource::new(config).await.unwrap();

    assert_eq!(source.database_type(), "SQLite");
}
```

### CI Matrix Testing

```yaml
# .github/workflows/test.yml
jobs:
  test-databases:
    strategy:
      matrix:
        database: [postgres, mysql, sqlite]
        include:
          - database: postgres
            feature: postgres
          - database: mysql
            feature: mysql
          - database: sqlite
            feature: sqlite

    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Run tests for ${{ matrix.database }}
        run: cargo test --features ${{ matrix.feature }}
```

---

## Observabilidad

### Logging Database Type

```rust
impl SqlConfigSource {
    pub async fn new(config: SqlConfig) -> Result<Self, BackendError> {
        tracing::info!(
            connection_type = Self::detect_db_type(&config.connection_string),
            "Creating SQL config source"
        );

        let pool = Self::create_pool(&config).await?;

        tracing::info!(
            database = %self.database_type(),
            "SQL config source initialized"
        );

        Ok(Self { pool })
    }

    fn detect_db_type(conn: &str) -> &'static str {
        if conn.starts_with("postgres") { "PostgreSQL" }
        else if conn.starts_with("mysql") { "MySQL" }
        else if conn.contains("sqlite") { "SQLite" }
        else { "Unknown" }
    }
}
```

### Metricas por Database

```rust
// Different metrics per database
sql_query_duration_seconds{database="PostgreSQL", operation="get_config"}
sql_query_duration_seconds{database="MySQL", operation="get_config"}
sql_query_duration_seconds{database="SQLite", operation="get_config"}
```

---

## Seguridad

### Connection String Handling

```rust
// Nunca loggear connection strings completos
impl SqlConfig {
    pub fn safe_display(&self) -> String {
        // Ocultar password
        let re = regex::Regex::new(r"://[^:]+:([^@]+)@").unwrap();
        re.replace(&self.connection_string, "://<user>:<redacted>@").to_string()
    }
}

// Uso
tracing::info!(
    connection = %config.safe_display(),
    "Connecting to database"
);
```

---

## Entregable Final

### Archivos Creados/Modificados

1. `Cargo.toml` - Feature flags
2. `src/sql/mod.rs` - Module exports con cfg
3. `src/sql/database.rs` - Database abstraction
4. `src/sql/pool.rs` - Pool creation por database
5. `src/sql/source.rs` - Multi-database source
6. `src/sql/postgres/` - PostgreSQL implementation
7. `src/sql/mysql/` - MySQL implementation
8. `src/sql/sqlite/` - SQLite implementation
9. `migrations/postgres/` - PostgreSQL migrations
10. `migrations/mysql/` - MySQL migrations
11. `migrations/sqlite/` - SQLite migrations
12. `tests/postgres_test.rs`
13. `tests/mysql_test.rs`
14. `tests/sqlite_test.rs`

### Verificacion

```bash
# Verificar compilacion con cada feature
cargo check --features postgres
cargo check --features mysql
cargo check --features sqlite
cargo check --features all-sql

# Tests
cargo test --features postgres
cargo test --features mysql
cargo test --features sqlite

# Build matrix
for feat in postgres mysql sqlite; do
    echo "Testing $feat..."
    cargo test --features $feat
done
```

### Ejemplo de Uso

```rust
use vortex_backends::sql::{SqlConfig, SqlConfigSource, available_databases};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check available databases
    println!("Available databases: {:?}", available_databases());

    // Connect to PostgreSQL
    let pg_source = SqlConfigSource::new(
        SqlConfig::new("postgres://user:pass@localhost/vortex")
    ).await?;
    println!("Connected to: {}", pg_source.database_type());

    // Connect to MySQL
    let mysql_source = SqlConfigSource::new(
        SqlConfig::new("mysql://user:pass@localhost/vortex")
    ).await?;
    println!("Connected to: {}", mysql_source.database_type());

    // Connect to SQLite (embedded)
    let sqlite_source = SqlConfigSource::new(
        SqlConfig::new("sqlite://./vortex.db")
    ).await?;
    println!("Connected to: {}", sqlite_source.database_type());

    Ok(())
}
```
