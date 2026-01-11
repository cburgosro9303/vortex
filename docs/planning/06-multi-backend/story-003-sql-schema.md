# Historia 003: Schema SQL para Configuraciones

## Contexto y Objetivo

Esta historia define el schema de base de datos para almacenar configuraciones en sistemas SQL (PostgreSQL, MySQL, SQLite). Un schema bien disenado permite almacenamiento eficiente, versionado, y consultas rapidas de configuraciones.

**Requerimientos del schema:**
- Almacenar configuraciones por aplicacion/perfil
- Soportar versionado con historico
- Metadata para auditoria (quien, cuando, por que)
- Eficiencia en consultas por app/profile
- Compatible con multiples engines SQL

SQLx proporciona un sistema de migraciones type-safe que verificara nuestras queries en compile-time.

---

## Alcance

### In Scope

- Diseno del schema de tablas
- Migraciones SQLx para crear tablas
- Indices para queries eficientes
- Soporte de versionado con timestamps
- Constraints de integridad referencial
- Schema compatible con PostgreSQL, MySQL, SQLite

### Out of Scope

- Implementacion del backend SQL (historia 004)
- Replication o sharding
- Stored procedures o triggers
- Full-text search en valores
- Partitioning de tablas

---

## Criterios de Aceptacion

- [ ] Schema soporta configuraciones por app/profile
- [ ] Versionado automatico con timestamps
- [ ] Indices en columnas de busqueda frecuente
- [ ] Constraints para integridad de datos
- [ ] Migraciones reversibles (up/down)
- [ ] Compatible con PostgreSQL 14+
- [ ] Compatible con MySQL 8+
- [ ] Compatible con SQLite 3.35+
- [ ] Documentacion de cada tabla y columna

---

## Diseno Propuesto

### Diagrama Entidad-Relacion

```
┌─────────────────────────────────────────────────────────────┐
│                        applications                          │
├─────────────────────────────────────────────────────────────┤
│ id: UUID (PK)                                               │
│ name: VARCHAR(255) UNIQUE                                   │
│ description: TEXT                                           │
│ created_at: TIMESTAMP                                       │
│ updated_at: TIMESTAMP                                       │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           │ 1:N
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                    config_profiles                           │
├─────────────────────────────────────────────────────────────┤
│ id: UUID (PK)                                               │
│ application_id: UUID (FK)                                   │
│ profile: VARCHAR(100)                                       │
│ created_at: TIMESTAMP                                       │
│ UNIQUE(application_id, profile)                             │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           │ 1:N
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                   config_versions                            │
├─────────────────────────────────────────────────────────────┤
│ id: UUID (PK)                                               │
│ profile_id: UUID (FK)                                       │
│ version: INTEGER                                            │
│ content: JSONB / JSON                                       │
│ checksum: VARCHAR(64)                                       │
│ created_by: VARCHAR(255)                                    │
│ created_at: TIMESTAMP                                       │
│ message: TEXT                                               │
│ is_active: BOOLEAN                                          │
│ UNIQUE(profile_id, version)                                 │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           │ 1:N
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                   config_properties                          │
├─────────────────────────────────────────────────────────────┤
│ id: UUID (PK)                                               │
│ version_id: UUID (FK)                                       │
│ key: VARCHAR(500)                                           │
│ value: TEXT                                                 │
│ value_type: VARCHAR(50)                                     │
│ UNIQUE(version_id, key)                                     │
└─────────────────────────────────────────────────────────────┘
```

### Tablas Detalladas

#### applications

Almacena las aplicaciones registradas en el sistema.

| Columna | Tipo | Constraints | Descripcion |
|---------|------|-------------|-------------|
| id | UUID | PK | Identificador unico |
| name | VARCHAR(255) | UNIQUE, NOT NULL | Nombre de la aplicacion |
| description | TEXT | NULL | Descripcion opcional |
| created_at | TIMESTAMP | NOT NULL, DEFAULT NOW | Fecha de creacion |
| updated_at | TIMESTAMP | NOT NULL, DEFAULT NOW | Ultima modificacion |

#### config_profiles

Almacena los perfiles de cada aplicacion (dev, staging, prod, etc).

| Columna | Tipo | Constraints | Descripcion |
|---------|------|-------------|-------------|
| id | UUID | PK | Identificador unico |
| application_id | UUID | FK, NOT NULL | Referencia a aplicacion |
| profile | VARCHAR(100) | NOT NULL | Nombre del perfil |
| created_at | TIMESTAMP | NOT NULL, DEFAULT NOW | Fecha de creacion |

**Indices:**
- `idx_profiles_app_id` en `application_id`
- `uniq_app_profile` UNIQUE en `(application_id, profile)`

#### config_versions

Almacena versiones de configuracion con contenido JSON.

| Columna | Tipo | Constraints | Descripcion |
|---------|------|-------------|-------------|
| id | UUID | PK | Identificador unico |
| profile_id | UUID | FK, NOT NULL | Referencia a perfil |
| version | INTEGER | NOT NULL | Numero de version (auto-increment) |
| content | JSONB/JSON | NOT NULL | Configuracion completa en JSON |
| checksum | VARCHAR(64) | NOT NULL | SHA-256 del contenido |
| created_by | VARCHAR(255) | NULL | Usuario que creo la version |
| created_at | TIMESTAMP | NOT NULL, DEFAULT NOW | Fecha de creacion |
| message | TEXT | NULL | Mensaje descriptivo del cambio |
| is_active | BOOLEAN | NOT NULL, DEFAULT FALSE | Version actualmente activa |

**Indices:**
- `idx_versions_profile_id` en `profile_id`
- `idx_versions_active` en `(profile_id, is_active)` WHERE `is_active = true`
- `uniq_profile_version` UNIQUE en `(profile_id, version)`

#### config_properties (opcional - para queries por propiedad)

Almacena propiedades individuales para busquedas eficientes.

| Columna | Tipo | Constraints | Descripcion |
|---------|------|-------------|-------------|
| id | UUID | PK | Identificador unico |
| version_id | UUID | FK, NOT NULL | Referencia a version |
| key | VARCHAR(500) | NOT NULL | Clave de la propiedad |
| value | TEXT | NOT NULL | Valor de la propiedad |
| value_type | VARCHAR(50) | NOT NULL | Tipo: string, number, boolean, array, object |

**Indices:**
- `idx_properties_version_id` en `version_id`
- `idx_properties_key` en `key`
- `uniq_version_key` UNIQUE en `(version_id, key)`

---

## Pasos de Implementacion

### Paso 1: Configurar SQLx CLI

```bash
# Instalar SQLx CLI
cargo install sqlx-cli --features postgres,mysql,sqlite

# Crear base de datos
sqlx database create

# Verificar conexion
sqlx database setup
```

### Paso 2: Crear Migracion Inicial (PostgreSQL)

```sql
-- migrations/20240101000000_initial_schema.up.sql

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Applications table
CREATE TABLE applications (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uniq_app_name UNIQUE (name)
);

-- Index for name lookups
CREATE INDEX idx_applications_name ON applications(name);

-- Comment on table
COMMENT ON TABLE applications IS 'Stores registered applications';
COMMENT ON COLUMN applications.name IS 'Unique application identifier';

-- Config profiles table
CREATE TABLE config_profiles (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    application_id UUID NOT NULL REFERENCES applications(id) ON DELETE CASCADE,
    profile VARCHAR(100) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uniq_app_profile UNIQUE (application_id, profile)
);

-- Index for application lookups
CREATE INDEX idx_profiles_app_id ON config_profiles(application_id);

COMMENT ON TABLE config_profiles IS 'Stores configuration profiles per application';
COMMENT ON COLUMN config_profiles.profile IS 'Profile name: default, dev, staging, production, etc';

-- Config versions table
CREATE TABLE config_versions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    profile_id UUID NOT NULL REFERENCES config_profiles(id) ON DELETE CASCADE,
    version INTEGER NOT NULL,
    content JSONB NOT NULL,
    checksum VARCHAR(64) NOT NULL,
    created_by VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    message TEXT,
    is_active BOOLEAN NOT NULL DEFAULT FALSE,
    CONSTRAINT uniq_profile_version UNIQUE (profile_id, version)
);

-- Indices for common queries
CREATE INDEX idx_versions_profile_id ON config_versions(profile_id);
CREATE INDEX idx_versions_active ON config_versions(profile_id, is_active)
    WHERE is_active = TRUE;
CREATE INDEX idx_versions_created_at ON config_versions(created_at DESC);

-- GIN index for JSONB content queries
CREATE INDEX idx_versions_content ON config_versions USING GIN (content);

COMMENT ON TABLE config_versions IS 'Stores versioned configuration content';
COMMENT ON COLUMN config_versions.content IS 'Full configuration as flattened JSON object';
COMMENT ON COLUMN config_versions.is_active IS 'Only one version per profile should be active';

-- Config properties table (denormalized for efficient queries)
CREATE TABLE config_properties (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    version_id UUID NOT NULL REFERENCES config_versions(id) ON DELETE CASCADE,
    key VARCHAR(500) NOT NULL,
    value TEXT NOT NULL,
    value_type VARCHAR(50) NOT NULL DEFAULT 'string',
    CONSTRAINT uniq_version_key UNIQUE (version_id, key),
    CONSTRAINT valid_value_type CHECK (
        value_type IN ('string', 'number', 'boolean', 'array', 'object', 'null')
    )
);

-- Indices for property lookups
CREATE INDEX idx_properties_version_id ON config_properties(version_id);
CREATE INDEX idx_properties_key ON config_properties(key);
CREATE INDEX idx_properties_key_pattern ON config_properties(key varchar_pattern_ops);

COMMENT ON TABLE config_properties IS 'Denormalized properties for efficient key-based queries';

-- Trigger to ensure only one active version per profile
CREATE OR REPLACE FUNCTION ensure_single_active_version()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.is_active = TRUE THEN
        UPDATE config_versions
        SET is_active = FALSE
        WHERE profile_id = NEW.profile_id
          AND id != NEW.id
          AND is_active = TRUE;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_single_active_version
    BEFORE INSERT OR UPDATE ON config_versions
    FOR EACH ROW
    EXECUTE FUNCTION ensure_single_active_version();

-- Function to auto-increment version
CREATE OR REPLACE FUNCTION next_version_number(p_profile_id UUID)
RETURNS INTEGER AS $$
    SELECT COALESCE(MAX(version), 0) + 1
    FROM config_versions
    WHERE profile_id = p_profile_id;
$$ LANGUAGE SQL;

-- Updated_at trigger for applications
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_applications_updated_at
    BEFORE UPDATE ON applications
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
```

### Paso 3: Migracion Down (Rollback)

```sql
-- migrations/20240101000000_initial_schema.down.sql

-- Drop triggers first
DROP TRIGGER IF EXISTS trg_single_active_version ON config_versions;
DROP TRIGGER IF EXISTS trg_applications_updated_at ON applications;

-- Drop functions
DROP FUNCTION IF EXISTS ensure_single_active_version();
DROP FUNCTION IF EXISTS next_version_number(UUID);
DROP FUNCTION IF EXISTS update_updated_at_column();

-- Drop tables in reverse dependency order
DROP TABLE IF EXISTS config_properties;
DROP TABLE IF EXISTS config_versions;
DROP TABLE IF EXISTS config_profiles;
DROP TABLE IF EXISTS applications;

-- Drop extension
DROP EXTENSION IF EXISTS "uuid-ossp";
```

### Paso 4: Schema MySQL (Alternativo)

```sql
-- migrations/mysql/20240101000000_initial_schema.up.sql

-- Applications table
CREATE TABLE applications (
    id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY uniq_app_name (name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE INDEX idx_applications_name ON applications(name);

-- Config profiles table
CREATE TABLE config_profiles (
    id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
    application_id CHAR(36) NOT NULL,
    profile VARCHAR(100) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uniq_app_profile (application_id, profile),
    FOREIGN KEY (application_id) REFERENCES applications(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE INDEX idx_profiles_app_id ON config_profiles(application_id);

-- Config versions table
CREATE TABLE config_versions (
    id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
    profile_id CHAR(36) NOT NULL,
    version INT NOT NULL,
    content JSON NOT NULL,
    checksum VARCHAR(64) NOT NULL,
    created_by VARCHAR(255),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    message TEXT,
    is_active BOOLEAN NOT NULL DEFAULT FALSE,
    UNIQUE KEY uniq_profile_version (profile_id, version),
    FOREIGN KEY (profile_id) REFERENCES config_profiles(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE INDEX idx_versions_profile_id ON config_versions(profile_id);
CREATE INDEX idx_versions_active ON config_versions(profile_id, is_active);
CREATE INDEX idx_versions_created_at ON config_versions(created_at DESC);

-- Config properties table
CREATE TABLE config_properties (
    id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
    version_id CHAR(36) NOT NULL,
    `key` VARCHAR(500) NOT NULL,
    value TEXT NOT NULL,
    value_type VARCHAR(50) NOT NULL DEFAULT 'string',
    UNIQUE KEY uniq_version_key (version_id, `key`(255)),
    FOREIGN KEY (version_id) REFERENCES config_versions(id) ON DELETE CASCADE,
    CHECK (value_type IN ('string', 'number', 'boolean', 'array', 'object', 'null'))
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE INDEX idx_properties_version_id ON config_properties(version_id);
CREATE INDEX idx_properties_key ON config_properties(`key`(255));
```

### Paso 5: Schema SQLite (Alternativo)

```sql
-- migrations/sqlite/20240101000000_initial_schema.up.sql

-- Applications table
CREATE TABLE applications (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(4))) || '-' ||
        lower(hex(randomblob(2))) || '-4' ||
        substr(lower(hex(randomblob(2))),2) || '-' ||
        substr('89ab', abs(random()) % 4 + 1, 1) ||
        substr(lower(hex(randomblob(2))),2) || '-' ||
        lower(hex(randomblob(6)))),
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_applications_name ON applications(name);

-- Config profiles table
CREATE TABLE config_profiles (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(4))) || '-' ||
        lower(hex(randomblob(2))) || '-4' ||
        substr(lower(hex(randomblob(2))),2) || '-' ||
        substr('89ab', abs(random()) % 4 + 1, 1) ||
        substr(lower(hex(randomblob(2))),2) || '-' ||
        lower(hex(randomblob(6)))),
    application_id TEXT NOT NULL REFERENCES applications(id) ON DELETE CASCADE,
    profile TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(application_id, profile)
);

CREATE INDEX idx_profiles_app_id ON config_profiles(application_id);

-- Config versions table
CREATE TABLE config_versions (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(4))) || '-' ||
        lower(hex(randomblob(2))) || '-4' ||
        substr(lower(hex(randomblob(2))),2) || '-' ||
        substr('89ab', abs(random()) % 4 + 1, 1) ||
        substr(lower(hex(randomblob(2))),2) || '-' ||
        lower(hex(randomblob(6)))),
    profile_id TEXT NOT NULL REFERENCES config_profiles(id) ON DELETE CASCADE,
    version INTEGER NOT NULL,
    content TEXT NOT NULL,  -- JSON stored as TEXT
    checksum TEXT NOT NULL,
    created_by TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    message TEXT,
    is_active INTEGER NOT NULL DEFAULT 0,
    UNIQUE(profile_id, version)
);

CREATE INDEX idx_versions_profile_id ON config_versions(profile_id);
CREATE INDEX idx_versions_active ON config_versions(profile_id, is_active) WHERE is_active = 1;
CREATE INDEX idx_versions_created_at ON config_versions(created_at DESC);

-- Config properties table
CREATE TABLE config_properties (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(4))) || '-' ||
        lower(hex(randomblob(2))) || '-4' ||
        substr(lower(hex(randomblob(2))),2) || '-' ||
        substr('89ab', abs(random()) % 4 + 1, 1) ||
        substr(lower(hex(randomblob(2))),2) || '-' ||
        lower(hex(randomblob(6)))),
    version_id TEXT NOT NULL REFERENCES config_versions(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    value_type TEXT NOT NULL DEFAULT 'string' CHECK (
        value_type IN ('string', 'number', 'boolean', 'array', 'object', 'null')
    ),
    UNIQUE(version_id, key)
);

CREATE INDEX idx_properties_version_id ON config_properties(version_id);
CREATE INDEX idx_properties_key ON config_properties(key);

-- Enable foreign keys
PRAGMA foreign_keys = ON;
```

### Paso 6: Configurar SQLx en Rust

```rust
// src/sql/schema.rs
use sqlx::FromRow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Application entity.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Application {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Configuration profile entity.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ConfigProfile {
    pub id: Uuid,
    pub application_id: Uuid,
    pub profile: String,
    pub created_at: DateTime<Utc>,
}

/// Configuration version entity.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ConfigVersion {
    pub id: Uuid,
    pub profile_id: Uuid,
    pub version: i32,
    pub content: serde_json::Value,
    pub checksum: String,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub message: Option<String>,
    pub is_active: bool,
}

/// Configuration property entity.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ConfigProperty {
    pub id: Uuid,
    pub version_id: Uuid,
    pub key: String,
    pub value: String,
    pub value_type: String,
}

/// Value types for properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValueType {
    String,
    Number,
    Boolean,
    Array,
    Object,
    Null,
}

impl ValueType {
    pub fn from_json_value(value: &serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(_) => Self::Boolean,
            serde_json::Value::Number(_) => Self::Number,
            serde_json::Value::String(_) => Self::String,
            serde_json::Value::Array(_) => Self::Array,
            serde_json::Value::Object(_) => Self::Object,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Array => "array",
            Self::Object => "object",
            Self::Null => "null",
        }
    }
}
```

---

## Conceptos de Rust Aprendidos

### 1. SQLx Migrations

SQLx proporciona un sistema de migraciones integrado.

**Estructura de proyecto:**
```
project/
├── migrations/
│   ├── 20240101000000_initial_schema.up.sql
│   ├── 20240101000000_initial_schema.down.sql
│   ├── 20240102000000_add_labels.up.sql
│   └── 20240102000000_add_labels.down.sql
├── src/
│   └── main.rs
└── Cargo.toml
```

**Rust - Ejecutar migraciones:**
```rust
use sqlx::postgres::PgPoolOptions;
use sqlx::migrate::Migrator;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

async fn run_migrations() -> Result<(), sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://user:pass@localhost/db")
        .await?;

    // Run all pending migrations
    MIGRATOR.run(&pool).await?;

    Ok(())
}
```

**Comparacion con Java (Flyway):**
```java
// Flyway configuration
Flyway flyway = Flyway.configure()
    .dataSource("jdbc:postgresql://localhost/db", "user", "pass")
    .locations("classpath:db/migration")
    .load();

// Run migrations
flyway.migrate();

// Migration files: V1__Initial_schema.sql, V2__Add_labels.sql
```

**Diferencias clave:**
| Aspecto | SQLx | Flyway |
|---------|------|--------|
| Verificacion | Compile-time | Runtime |
| Embebido | `sqlx::migrate!()` | Archivos separados |
| Reversible | up.sql / down.sql | Opcional con undo |
| Checksum | Automatico | Automatico |

### 2. FromRow Derive

`FromRow` mapea filas de resultado a structs automaticamente.

**Rust:**
```rust
use sqlx::FromRow;

#[derive(FromRow)]
pub struct Application {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,  // Option para columnas NULL
    pub created_at: DateTime<Utc>,
}

// Uso con query_as
async fn get_app(pool: &PgPool, id: Uuid) -> Result<Application, sqlx::Error> {
    sqlx::query_as!(
        Application,
        "SELECT id, name, description, created_at FROM applications WHERE id = $1",
        id
    )
    .fetch_one(pool)
    .await
}

// O sin macro (menos type-safe)
async fn get_app_dynamic(pool: &PgPool, id: Uuid) -> Result<Application, sqlx::Error> {
    sqlx::query_as::<_, Application>(
        "SELECT id, name, description, created_at FROM applications WHERE id = $1"
    )
    .bind(id)
    .fetch_one(pool)
    .await
}
```

**Comparacion con Java (JPA/Hibernate):**
```java
@Entity
@Table(name = "applications")
public class Application {
    @Id
    private UUID id;

    @Column(nullable = false)
    private String name;

    private String description;

    @Column(name = "created_at")
    private Instant createdAt;

    // Getters, setters...
}

// Uso con EntityManager
Application app = entityManager.find(Application.class, id);

// O con Spring Data JPA
interface ApplicationRepository extends JpaRepository<Application, UUID> {
    Optional<Application> findByName(String name);
}
```

### 3. Type Mapping SQL a Rust

**PostgreSQL a Rust:**
```rust
// Type mappings comunes
// PostgreSQL          Rust
// -----------------   --------------------
// UUID                Uuid (from uuid crate)
// VARCHAR, TEXT       String
// INTEGER, INT4       i32
// BIGINT, INT8        i64
// BOOLEAN             bool
// TIMESTAMPTZ         DateTime<Utc>
// TIMESTAMP           NaiveDateTime
// JSONB, JSON         serde_json::Value
// BYTEA               Vec<u8>
// NUMERIC             BigDecimal (with feature)
// ARRAY               Vec<T>

use sqlx::types::Json;

#[derive(FromRow)]
pub struct ConfigVersion {
    pub id: Uuid,
    pub content: Json<serde_json::Value>,  // JSONB column
    pub tags: Vec<String>,                  // TEXT[] array
}
```

### 4. Checksum para Integridad

```rust
use sha2::{Sha256, Digest};

/// Calculates SHA-256 checksum of config content.
pub fn calculate_checksum(content: &serde_json::Value) -> String {
    let json = serde_json::to_string(content)
        .expect("Failed to serialize JSON");

    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    let result = hasher.finalize();

    // Convert to hex string
    result.iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

// Uso
let content = serde_json::json!({
    "server.port": 8080,
    "database.url": "postgres://localhost/db"
});

let checksum = calculate_checksum(&content);
// "a1b2c3d4..."
```

---

## Riesgos y Errores Comunes

### 1. No Usar Indices Apropiados

```sql
-- MAL: Query lento sin indice
SELECT * FROM config_versions
WHERE profile_id = ? AND is_active = true;
-- Full table scan!

-- BIEN: Crear indice parcial
CREATE INDEX idx_versions_active
ON config_versions(profile_id, is_active)
WHERE is_active = TRUE;
-- Index scan!
```

### 2. UUID Generation Inconsistente

```sql
-- PostgreSQL: usar uuid-ossp
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
-- DEFAULT uuid_generate_v4()

-- MySQL: UUID nativo
-- DEFAULT (UUID())

-- SQLite: funcion custom
-- DEFAULT (lower(hex(randomblob(16))))
```

### 3. JSON vs JSONB (PostgreSQL)

```sql
-- MAL: JSON almacena texto, sin indices
CREATE TABLE bad_table (
    content JSON  -- Stored as text, no indexing
);

-- BIEN: JSONB almacena binario, permite indices
CREATE TABLE good_table (
    content JSONB  -- Binary format, can index
);

-- Indice GIN para queries JSON
CREATE INDEX idx_content ON good_table USING GIN (content);

-- Query con indice
SELECT * FROM good_table WHERE content @> '{"env": "prod"}';
```

### 4. Foreign Keys en SQLite

```sql
-- SQLite requiere habilitar foreign keys explicitamente
PRAGMA foreign_keys = ON;

-- Sin esto, ON DELETE CASCADE no funciona!
```

---

## Pruebas

### Tests de Migracion

```rust
#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    #[sqlx::test]
    async fn migrations_run_successfully(pool: PgPool) {
        // sqlx::test automatically runs migrations
        // and rolls back after test

        let result = sqlx::query("SELECT 1")
            .fetch_one(&pool)
            .await;

        assert!(result.is_ok());
    }

    #[sqlx::test]
    async fn can_insert_application(pool: PgPool) {
        let result = sqlx::query!(
            r#"
            INSERT INTO applications (name, description)
            VALUES ($1, $2)
            RETURNING id
            "#,
            "test-app",
            "Test application"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(!result.id.is_nil());
    }

    #[sqlx::test]
    async fn unique_constraint_prevents_duplicate_apps(pool: PgPool) {
        // Insert first app
        sqlx::query!("INSERT INTO applications (name) VALUES ($1)", "myapp")
            .execute(&pool)
            .await
            .unwrap();

        // Try to insert duplicate
        let result = sqlx::query!("INSERT INTO applications (name) VALUES ($1)", "myapp")
            .execute(&pool)
            .await;

        assert!(result.is_err());
    }

    #[sqlx::test]
    async fn cascade_delete_removes_profiles(pool: PgPool) {
        // Create app
        let app = sqlx::query!("INSERT INTO applications (name) VALUES ('app') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

        // Create profile
        sqlx::query!(
            "INSERT INTO config_profiles (application_id, profile) VALUES ($1, 'dev')",
            app.id
        )
        .execute(&pool)
        .await
        .unwrap();

        // Delete app
        sqlx::query!("DELETE FROM applications WHERE id = $1", app.id)
            .execute(&pool)
            .await
            .unwrap();

        // Profile should be gone
        let count: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM config_profiles WHERE application_id = $1",
            app.id
        )
        .fetch_one(&pool)
        .await
        .unwrap()
        .unwrap_or(0);

        assert_eq!(count, 0);
    }
}
```

### Test de Schema con Tipos

```rust
#[sqlx::test]
async fn config_version_types_match(pool: PgPool) {
    // Insert test data
    let app_id = create_test_app(&pool).await;
    let profile_id = create_test_profile(&pool, app_id).await;

    let content = serde_json::json!({
        "server.port": 8080,
        "enabled": true
    });

    let checksum = calculate_checksum(&content);

    // Insert version
    let version = sqlx::query_as!(
        ConfigVersion,
        r#"
        INSERT INTO config_versions (profile_id, version, content, checksum, is_active)
        VALUES ($1, 1, $2, $3, true)
        RETURNING id, profile_id, version, content, checksum, created_by, created_at, message, is_active
        "#,
        profile_id,
        content,
        checksum
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(version.version, 1);
    assert!(version.is_active);
    assert_eq!(version.content["server.port"], 8080);
}
```

---

## Observabilidad

### Logging de Migraciones

```rust
use tracing::{info, warn};

async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::Error> {
    info!("Running database migrations");

    let migrator = sqlx::migrate!("./migrations");

    for migration in migrator.migrations.iter() {
        info!(
            version = migration.version,
            description = %migration.description,
            "Found migration"
        );
    }

    migrator.run(pool).await?;

    info!("Migrations completed successfully");
    Ok(())
}
```

### Health Check de Schema

```rust
pub async fn check_schema_health(pool: &PgPool) -> Result<SchemaHealth, Error> {
    let tables = vec!["applications", "config_profiles", "config_versions", "config_properties"];

    let mut missing = Vec::new();

    for table in tables {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = $1)"
        )
        .bind(table)
        .fetch_one(pool)
        .await?;

        if !exists {
            missing.push(table.to_string());
        }
    }

    Ok(SchemaHealth {
        healthy: missing.is_empty(),
        missing_tables: missing,
    })
}
```

---

## Entregable Final

### Archivos Creados

1. `migrations/20240101000000_initial_schema.up.sql` - Schema PostgreSQL
2. `migrations/20240101000000_initial_schema.down.sql` - Rollback
3. `migrations/mysql/20240101000000_initial_schema.up.sql` - Schema MySQL
4. `migrations/sqlite/20240101000000_initial_schema.up.sql` - Schema SQLite
5. `crates/vortex-backends/src/sql/schema.rs` - Entidades Rust

### Verificacion

```bash
# Verificar sintaxis SQL
sqlx migrate info

# Ejecutar migraciones en PostgreSQL
DATABASE_URL=postgres://user:pass@localhost/vortex sqlx migrate run

# Verificar que compila con SQLx
DATABASE_URL=postgres://user:pass@localhost/vortex cargo sqlx prepare

# Rollback
DATABASE_URL=postgres://user:pass@localhost/vortex sqlx migrate revert

# Tests
cargo test -p vortex-backends --features postgres
```

### Queries de Verificacion

```sql
-- Verificar tablas creadas
SELECT table_name FROM information_schema.tables
WHERE table_schema = 'public';

-- Verificar indices
SELECT indexname, indexdef FROM pg_indexes
WHERE tablename IN ('applications', 'config_profiles', 'config_versions');

-- Verificar constraints
SELECT constraint_name, constraint_type
FROM information_schema.table_constraints
WHERE table_name = 'config_versions';

-- Test insert flow
INSERT INTO applications (name) VALUES ('test-app');
INSERT INTO config_profiles (application_id, profile)
SELECT id, 'dev' FROM applications WHERE name = 'test-app';
INSERT INTO config_versions (profile_id, version, content, checksum, is_active)
SELECT id, 1, '{"key": "value"}'::jsonb, 'abc123', true
FROM config_profiles WHERE profile = 'dev';
```
