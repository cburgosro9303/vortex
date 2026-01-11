# Historia 001: Setup del Workspace Multi-Crate

## Contexto y Objetivo

Vortex Config será un proyecto Rust con múltiples crates organizados en un workspace. Esta estructura es análoga a un proyecto Maven multi-módulo, pero con las ventajas del sistema de gestión de dependencias de Cargo.

El objetivo es crear la estructura base del proyecto con tres crates iniciales:

- **vortex-core**: Tipos y traits del dominio (librería)
- **vortex-server**: Servidor HTTP (binario, futuro)
- **vortex-sources**: Backends de configuración (librería, futuro)

## Alcance

### In Scope

- Crear estructura de directorios del workspace
- Configurar Cargo.toml raíz con workspace members
- Crear Cargo.toml para cada crate con dependencias compartidas
- Establecer relaciones de dependencia entre crates
- Configurar metadata común (version, edition, authors)

### Out of Scope

- Implementación de lógica de negocio
- Configuración de herramientas de linting (ver Historia 002)
- Pipeline CI (ver Historia 003)

## Criterios de Aceptación

- [X] Estructura de directorios creada según especificación
- [X] `cargo build --workspace` compila exitosamente
- [X] `cargo test --workspace` ejecuta sin errores (aunque no haya tests aún)
- [X] Cada crate tiene su propio Cargo.toml con metadata correcta
- [X] vortex-server depende de vortex-core
- [X] vortex-sources depende de vortex-core
- [x] Edición 2024 configurada en todos los crates
- [X] Version compartida definida en workspace

## Diseño Propuesto

### Estructura de Directorios

```
vortex-config/
├── Cargo.toml                 # Workspace manifest
├── Cargo.lock                 # Lock file (generado)
├── crates/
│   ├── vortex-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   ├── vortex-server/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs         # Será main.rs cuando tenga binario
│   └── vortex-sources/
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs
```

### Cargo.toml Raíz (Workspace)

```toml
[workspace]
resolver = "2"
members = [
    "crates/vortex-core",
    "crates/vortex-server",
    "crates/vortex-sources",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
authors = ["Vortex Team"]
license = "MIT OR Apache-2.0"
repository = "https://github.com/org/vortex-config"
rust-version = "1.92"

[workspace.dependencies]
# Dependencias compartidas se definen aquí
thiserror = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Crates internos
vortex-core = { path = "crates/vortex-core" }
```

### Cargo.toml de vortex-core

```toml
[package]
name = "vortex-core"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Core domain types and traits for Vortex Config"

[dependencies]
thiserror.workspace = true
serde.workspace = true

[dev-dependencies]
serde_json.workspace = true
```

## Pasos de Implementación

### Paso 1: Crear estructura de directorios

```bash
mkdir -p vortex-config/crates/{vortex-core,vortex-server,vortex-sources}/src
```

### Paso 2: Crear Cargo.toml del workspace

Crear el archivo `vortex-config/Cargo.toml` con la configuración del workspace.

### Paso 3: Crear Cargo.toml de cada crate

Para cada crate, crear su Cargo.toml referenciando las propiedades del workspace.

### Paso 4: Crear archivos lib.rs iniciales

Cada crate necesita un `src/lib.rs` mínimo:

```rust
//! Vortex Core - Domain types and traits
//!
//! This crate provides the foundational types for the Vortex Config server.

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_defined() {
        assert!(!version().is_empty());
    }
}
```

### Paso 5: Verificar compilación

```bash
cd vortex-config
cargo build --workspace
cargo test --workspace
```

## Conceptos de Rust Aprendidos

### Cargo Workspace

Un workspace de Cargo agrupa múltiples crates relacionados bajo un mismo `Cargo.lock`, compartiendo dependencias y configuración.

```toml
# Cargo.toml (raíz del workspace)
[workspace]
resolver = "2"  # Resolver moderno para features
members = [
    "crates/vortex-core",
    "crates/vortex-server",
    "crates/vortex-sources",
]

# Propiedades compartidas por todos los crates
[workspace.package]
version = "0.1.0"
edition = "2024"
authors = ["Vortex Team"]

# Dependencias compartidas - se referencian con .workspace = true
[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
thiserror = "1.0"
```

**Comparación con Java:**

| Cargo Workspace | Maven Multi-Module |
|-----------------|-------------------|
| `Cargo.toml` | `pom.xml` (parent) |
| `[workspace.members]` | `<modules>` |
| `[workspace.dependencies]` | `<dependencyManagement>` |
| `Cargo.lock` | Sin equivalente directo (versiones en pom) |
| `resolver = "2"` | Sin equivalente (Maven no tiene features) |

### Crates y Modules

En Rust, un **crate** es la unidad de compilación (como un JAR en Java). Un crate puede ser:

- **Library crate** (`lib.rs`): Produce una librería reutilizable
- **Binary crate** (`main.rs`): Produce un ejecutable

```rust
// crates/vortex-core/src/lib.rs

//! Documentation for the crate goes here.
//! This is the root module of vortex-core.

// Declarar submódulos (cada uno en su propio archivo)
pub mod config;      // -> src/config.rs
pub mod environment; // -> src/environment.rs
mod internal;        // privado, no exportado

// Re-exportar tipos para API más limpia
pub use config::{ConfigMap, PropertySource};
pub use environment::{Application, Profile, Label};
```

**Comparación con Java:**

| Rust | Java |
|------|------|
| Crate | JAR / Module (JPMS) |
| `mod foo` | `package foo` |
| `pub mod` | `exports` en module-info.java |
| `pub use` | Re-exportar no tiene equivalente directo |
| `lib.rs` | Punto de entrada del módulo |

### Cargo.toml vs pom.xml

```toml
# Cargo.toml de un crate
[package]
name = "vortex-core"
version.workspace = true      # Hereda del workspace
edition.workspace = true      # Hereda del workspace
description = "Core types"

[dependencies]
serde.workspace = true        # Usa versión del workspace
tokio = { version = "1.0", features = ["full"] }

[dev-dependencies]            # Solo para tests
serde_json.workspace = true

[build-dependencies]          # Para build.rs
# (vacío por ahora)

[features]                    # Compilación condicional
default = []
async = ["tokio"]
```

**Equivalencia Maven:**

```xml
<!-- pom.xml equivalente conceptual -->
<project>
    <parent>
        <groupId>com.vortex</groupId>
        <artifactId>vortex-parent</artifactId>
        <version>${revision}</version>
    </parent>

    <artifactId>vortex-core</artifactId>

    <dependencies>
        <dependency>
            <groupId>com.fasterxml.jackson.core</groupId>
            <artifactId>jackson-databind</artifactId>
            <!-- version from parent -->
        </dependency>
    </dependencies>
</project>
```

### Visibilidad y Exports

```rust
// src/lib.rs
pub mod config;        // Módulo público
mod internal;          // Módulo privado

// Re-export para API limpia (usuarios importan desde raíz)
pub use config::ConfigMap;

// src/config.rs
pub struct ConfigMap {           // Struct pública
    pub name: String,            // Campo público
    pub(crate) cache: Cache,     // Solo visible en este crate
    internal_id: u64,            // Privado al módulo
}

impl ConfigMap {
    pub fn new(name: String) -> Self { /* ... */ }     // Método público
    pub(super) fn validate(&self) { /* ... */ }        // Visible en módulo padre
    fn compute_hash(&self) -> u64 { /* ... */ }        // Privado
}
```

**Comparación con Java:**

| Rust | Java |
|------|------|
| `pub` | `public` |
| (default) | `private` al módulo |
| `pub(crate)` | `package-private` |
| `pub(super)` | Sin equivalente directo |
| `pub(in path)` | Sin equivalente |

## Riesgos y Errores Comunes

### Error 1: Olvidar `pub` en re-exports

```rust
// MAL: El tipo existe pero no se puede usar desde fuera
mod config;
use config::ConfigMap;  // ConfigMap sigue siendo privado

// BIEN: Explícitamente re-exportar como público
mod config;
pub use config::ConfigMap;
```

### Error 2: Path de dependencias internas incorrecto

```toml
# MAL: Path relativo incorrecto
[dependencies]
vortex-core = { path = "../vortex-core" }

# BIEN: Path desde la raíz del workspace
[dependencies]
vortex-core = { path = "../vortex-core" }  # OK si estás en crates/vortex-server

# MEJOR: Definir en workspace y referenciar
# En workspace Cargo.toml:
[workspace.dependencies]
vortex-core = { path = "crates/vortex-core" }

# En crate Cargo.toml:
[dependencies]
vortex-core.workspace = true
```

### Error 3: Circular dependencies

Rust **no permite** dependencias circulares entre crates. Si vortex-core depende de vortex-sources y viceversa, la compilación fallará.

**Solución:** Extraer tipos compartidos a un crate común o usar traits para inversión de dependencias.

### Error 4: Confundir workspace vs package

```toml
# MAL: Mezclar [workspace] y [package] en el mismo archivo raíz
[package]
name = "vortex-config"

[workspace]
members = ["crates/*"]

# BIEN: Si el root es solo workspace, no hay [package]
[workspace]
members = ["crates/*"]
# No hay [package] en este archivo
```

## Pruebas

### Unit Tests

Cada `lib.rs` inicial incluye un test básico:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver() {
        let v = version();
        assert!(v.split('.').count() == 3, "Version should be semver");
    }

    #[test]
    fn crate_compiles() {
        // Test implícito: si este test corre, el crate compila
        assert!(true);
    }
}
```

### Integration Tests

No aplica para esta historia (estructura inicial).

### Comandos de Verificación

```bash
# Compilar todo el workspace
cargo build --workspace

# Ejecutar todos los tests
cargo test --workspace

# Verificar que cada crate se puede publicar (dry-run)
cargo publish --dry-run -p vortex-core
```

## Entregable Final

### PR debe incluir

1. **Archivos nuevos:**
   - `Cargo.toml` (workspace root)
   - `crates/vortex-core/Cargo.toml`
   - `crates/vortex-core/src/lib.rs`
   - `crates/vortex-server/Cargo.toml`
   - `crates/vortex-server/src/lib.rs`
   - `crates/vortex-sources/Cargo.toml`
   - `crates/vortex-sources/src/lib.rs`

2. **Verificaciones:**
   - Screenshot de `cargo build --workspace` exitoso
   - Screenshot de `cargo test --workspace` con tests pasando

3. **Documentación:**
   - Breve descripción en el PR de la estructura elegida
   - Link a ADR-001 (estructura del workspace)

### Checklist de Revisión

- [X] Estructura de directorios correcta
- [x] Cargo.toml del workspace bien formateado
- [x] Cada crate tiene version.workspace = true
- [x] Dependencias entre crates correctamente definidas
- [x] lib.rs tiene doc comments básicos
- [x] Al menos un test por crate
- [x] No hay warnings de compilación

---

**Navegación:** [Volver al índice](./index.md) | [Siguiente: Toolchain Config](./story-002-toolchain-config.md)
