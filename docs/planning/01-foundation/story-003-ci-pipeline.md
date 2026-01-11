# Historia 003: Pipeline CI Basico

## Contexto y Objetivo

Un pipeline de Integracion Continua (CI) es esencial para mantener la calidad del codigo en cualquier proyecto profesional. Para Vortex Config, implementaremos un pipeline en GitHub Actions que valide formateo, linting, tests y seguridad en cada push y pull request.

Este pipeline es equivalente a lo que un desarrollador Java configuraria con GitHub Actions + Maven/Gradle, pero adaptado al ecosistema Rust.

## Alcance

### In Scope

- Workflow de GitHub Actions para CI
- Jobs: format check, clippy, tests, security audit
- Cache de dependencias Cargo para builds rapidos
- Matriz de pruebas en multiples OS (Linux, macOS)
- Badge de estado en README

### Out of Scope

- CD (Continuous Deployment)
- Publicacion a crates.io
- Builds de release
- Pruebas de integracion con servicios externos

## Criterios de Aceptacion

- [ ] Workflow `.github/workflows/ci.yml` creado
- [ ] CI ejecuta en push a main y en PRs
- [ ] Job de formato verifica `cargo fmt --check`
- [ ] Job de lint ejecuta `cargo clippy -- -D warnings`
- [ ] Job de tests ejecuta `cargo test --workspace`
- [ ] Job de audit ejecuta `cargo audit`
- [ ] Cache de Cargo configurado correctamente
- [ ] Pipeline completo ejecuta en < 5 minutos
- [ ] Badge de CI en README.md

## Diseno Propuesto

### Estructura del Workflow

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  fmt:
    # Verificar formateo
  clippy:
    # Linting
  test:
    # Tests en matriz de OS
  audit:
    # Seguridad de dependencias
```

### Diagrama de Jobs

```
          +-------+
          |  fmt  |
          +-------+
               |
          +--------+
          | clippy |
          +--------+
               |
    +----------+----------+
    |                     |
+-------+            +--------+
| test  |            | audit  |
| linux |            +--------+
+-------+
    |
+-------+
| test  |
| macos |
+-------+
```

## Pasos de Implementacion

### Paso 1: Crear directorio de workflows

```bash
mkdir -p .github/workflows
```

### Paso 2: Crear ci.yml

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]
  # Permitir ejecucion manual
  workflow_dispatch:

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1
  # Versión mínima de Rust soportada
  MSRV: "1.92"

jobs:
  # ============================================
  # Job 1: Verificar formateo de código
  # ============================================
  fmt:
    name: Format Check
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt

      - name: Check formatting
        run: cargo fmt --all -- --check

  # ============================================
  # Job 2: Linting con Clippy
  # ============================================
  clippy:
    name: Clippy Lints
    runs-on: ubuntu-latest
    needs: fmt  # Solo ejecutar si fmt pasa
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy

      - name: Cache cargo registry
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-clippy-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-clippy-

      - name: Run Clippy
        run: cargo clippy --workspace --all-targets --all-features -- -D warnings

  # ============================================
  # Job 3: Tests en múltiples OS
  # ============================================
  test:
    name: Test (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    needs: clippy  # Solo ejecutar si clippy pasa
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest]
        # Opcional: probar múltiples versiones de Rust
        # rust: [stable, beta]
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo registry
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-test-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-test-

      - name: Run tests
        run: cargo test --workspace --all-features

      - name: Run doc tests
        run: cargo test --workspace --doc

  # ============================================
  # Job 4: Auditoría de seguridad
  # ============================================
  audit:
    name: Security Audit
    runs-on: ubuntu-latest
    needs: clippy  # Ejecutar en paralelo con tests
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Install cargo-audit
        run: cargo install cargo-audit --locked

      - name: Cache advisory database
        uses: actions/cache@v4
        with:
          path: ~/.cargo/advisory-db
          key: cargo-audit-db

      - name: Run security audit
        run: cargo audit

  # ============================================
  # Job 5: Verificar MSRV (Minimum Supported Rust Version)
  # ============================================
  msrv:
    name: MSRV Check
    runs-on: ubuntu-latest
    needs: fmt
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Install Rust ${{ env.MSRV }}
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: ${{ env.MSRV }}

      - name: Cache cargo registry
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-msrv-${{ hashFiles('**/Cargo.lock') }}

      - name: Check MSRV compatibility
        run: cargo check --workspace --all-features

  # ============================================
  # Job 6: Generar documentación
  # ============================================
  docs:
    name: Documentation
    runs-on: ubuntu-latest
    needs: clippy
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo registry
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-docs-${{ hashFiles('**/Cargo.lock') }}

      - name: Build documentation
        run: cargo doc --workspace --no-deps --all-features
        env:
          RUSTDOCFLAGS: "-D warnings"
```

### Paso 3: Agregar badge al README

```markdown
# Vortex Config

[![CI](https://github.com/ORG/vortex-config/actions/workflows/ci.yml/badge.svg)](https://github.com/ORG/vortex-config/actions/workflows/ci.yml)
```

### Paso 4: Crear Cargo.lock

```bash
# Generar Cargo.lock (necesario para cache efectivo)
cargo generate-lockfile
git add Cargo.lock
```

## Conceptos de Rust Aprendidos

### Cargo Test

`cargo test` es el test runner integrado de Rust. A diferencia de Java donde necesitas JUnit/TestNG, Rust tiene testing incorporado en el lenguaje.

```rust
// src/lib.rs o cualquier módulo

// Tests unitarios van en el mismo archivo
#[cfg(test)]  // Solo compila en modo test
mod tests {
    use super::*;  // Importa todo del módulo padre

    #[test]
    fn test_basic_functionality() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }

    #[test]
    fn test_with_custom_message() {
        let config = ConfigMap::new("app");
        assert!(
            config.name().len() > 0,
            "Config name should not be empty, got: {}",
            config.name()
        );
    }

    #[test]
    #[should_panic(expected = "empty name")]
    fn test_panics_on_empty_name() {
        ConfigMap::new("");  // Debería hacer panic
    }

    #[test]
    #[ignore]  // Ignorar por defecto (CI lento, requiere setup)
    fn test_expensive_operation() {
        // Test que tarda mucho
    }
}
```

**Comandos de cargo test:**

```bash
# Ejecutar todos los tests
cargo test

# Tests de un crate específico
cargo test -p vortex-core

# Tests que coinciden con un patrón
cargo test config::  # Tests en módulo config

# Ejecutar tests ignorados
cargo test -- --ignored

# Ejecutar un test específico
cargo test test_basic_functionality

# Ver output de println! en tests
cargo test -- --nocapture

# Tests en paralelo (por defecto) o secuencial
cargo test -- --test-threads=1
```

**Comparación con Java:**

| Rust | JUnit 5 |
|------|---------|
| `#[test]` | `@Test` |
| `#[should_panic]` | `assertThrows()` |
| `#[ignore]` | `@Disabled` |
| `assert_eq!(a, b)` | `assertEquals(a, b)` |
| `assert!(cond)` | `assertTrue(cond)` |
| `#[cfg(test)]` | `src/test/java/` |

### Integration Tests

En Rust, los tests de integración van en un directorio separado:

```
vortex-config/
├── crates/
│   └── vortex-core/
│       ├── src/
│       │   └── lib.rs
│       └── tests/           # Tests de integración
│           ├── common/
│           │   └── mod.rs   # Código compartido
│           ├── config_tests.rs
│           └── source_tests.rs
```

```rust
// tests/config_tests.rs
// Los integration tests ven el crate como un usuario externo

use vortex_core::{ConfigMap, PropertySource};

mod common;  // Importa tests/common/mod.rs

#[test]
fn test_config_map_creation() {
    let config = ConfigMap::builder()
        .name("myapp")
        .profile("production")
        .build()
        .unwrap();

    assert_eq!(config.name(), "myapp");
}
```

### Cargo Audit

`cargo audit` verifica que tus dependencias no tengan vulnerabilidades conocidas usando la RustSec Advisory Database.

```bash
# Instalar cargo-audit
cargo install cargo-audit

# Ejecutar auditoría
cargo audit

# Output ejemplo:
# Crate:     smallvec
# Version:   0.6.10
# Warning:   unsound
# Title:     Buffer overflow in SmallVec::insert_many
# Solution:  Upgrade to >=0.6.14 OR >=1.6.1
```

**Archivo de configuración opcional:**

```toml
# .cargo/audit.toml

[advisories]
# Ignorar advisories específicos (con justificación)
ignore = [
    # "RUSTSEC-2020-0071",  # Ejemplo: ya mitigado
]

# Fallar en advisories de tipo "unmaintained"
informational_warnings = ["unmaintained"]

[database]
# Ruta a base de datos local (opcional)
# path = "~/.cargo/advisory-db"

[output]
# Formato de salida
format = "terminal"
```

**Comparación con Java:**

| Rust | Java |
|------|------|
| `cargo audit` | OWASP Dependency-Check |
| RustSec Advisory DB | NVD / OSS Index |
| `Cargo.lock` | `pom.xml` versions |

### GitHub Actions Cache para Cargo

El cache es crítico para builds Rust que pueden ser lentos:

```yaml
- name: Cache cargo registry
  uses: actions/cache@v4
  with:
    path: |
      ~/.cargo/registry     # Índice de crates.io
      ~/.cargo/git          # Dependencias git
      target                # Artefactos compilados
    key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
    restore-keys: |
      ${{ runner.os }}-cargo-
```

**Qué cachear:**

| Path | Contenido | Tamaño típico |
|------|-----------|---------------|
| `~/.cargo/registry` | Índice y sources de crates.io | 100-500 MB |
| `~/.cargo/git` | Dependencias desde Git | Variable |
| `target` | Artefactos compilados | 500 MB - 2 GB |

**Tips de optimización:**

```yaml
# Separar cache por job para evitar conflictos
key: ${{ runner.os }}-cargo-${{ github.job }}-${{ hashFiles('**/Cargo.lock') }}

# Usar sccache para cache distribuido
- name: Install sccache
  run: cargo install sccache
- name: Configure sccache
  run: echo "RUSTC_WRAPPER=sccache" >> $GITHUB_ENV
```

## Riesgos y Errores Comunes

### Error 1: Cache invalidado frecuentemente

```yaml
# MAL: Solo usa Cargo.lock
key: cargo-${{ hashFiles('**/Cargo.lock') }}

# BIEN: Incluye OS y permite fallback
key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
restore-keys: |
  ${{ runner.os }}-cargo-
```

### Error 2: Tests flaky por paralelismo

```rust
// MAL: Tests que dependen de estado global
static mut COUNTER: i32 = 0;

#[test]
fn test_increment() {
    unsafe { COUNTER += 1; }
    assert_eq!(unsafe { COUNTER }, 1);  // Puede fallar!
}

// BIEN: Cada test tiene su propio estado
#[test]
fn test_increment() {
    let mut counter = Counter::new();
    counter.increment();
    assert_eq!(counter.value(), 1);
}
```

### Error 3: cargo audit falla por advisory ignorable

```bash
# En lugar de ignorar en código, documentar en audit.toml
# .cargo/audit.toml
[advisories]
ignore = [
    "RUSTSEC-2023-0001",  # Justificación: solo afecta feature X que no usamos
]
```

### Error 4: CI muy lento

```yaml
# Estrategias de optimización:

# 1. Fail fast
strategy:
  fail-fast: true  # Cancelar otros jobs si uno falla

# 2. Solo tests críticos en PRs
jobs:
  test:
    if: github.event_name == 'pull_request'
    # Solo tests rápidos

  test-full:
    if: github.ref == 'refs/heads/main'
    # Suite completa

# 3. Cargo incremental builds
env:
  CARGO_INCREMENTAL: 1
```

## Pruebas

### Verificación Local del Workflow

```bash
# Instalar act (ejecutor local de GitHub Actions)
brew install act  # macOS
# o
curl -s https://raw.githubusercontent.com/nektos/act/master/install.sh | bash

# Ejecutar workflow localmente
act -j fmt
act -j clippy
act -j test
```

### Checklist de Verificación

```bash
# 1. Verificar sintaxis del workflow
# (GitHub valida al hacer push)

# 2. Ejecutar los mismos comandos que CI
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo audit

# 3. Verificar que pasan en tiempo razonable
time cargo test --workspace  # Debe ser < 2 min
```

### Métricas de CI

Después de algunos runs, verificar:

- Tiempo promedio de pipeline: < 5 minutos
- Cache hit rate: > 80%
- Tasa de fallos flaky: < 1%

## Entregable Final

### PR debe incluir

1. **Archivos nuevos:**
   - `.github/workflows/ci.yml`
   - `Cargo.lock` (si no existía)
   - Actualización de README.md con badge

2. **Verificaciones:**
   - Screenshot de workflow ejecutando exitosamente
   - Link al run de GitHub Actions

3. **Documentación:**
   - Comentarios en ci.yml explicando cada job
   - Instrucciones en README para ejecutar checks localmente

### Checklist de Revisión

- [ ] Workflow ejecuta en push a main y PRs
- [ ] Job de fmt usa `cargo fmt --check`
- [ ] Job de clippy usa `-D warnings`
- [ ] Job de test ejecuta en Linux y macOS
- [ ] Job de audit está configurado
- [ ] Cache de Cargo correctamente configurado
- [ ] Dependencias entre jobs definidas (needs)
- [ ] Badge agregado al README
- [ ] Tiempo total < 5 minutos
- [ ] Cargo.lock commiteado

---

**Navegación:** [Anterior: Toolchain Config](./story-002-toolchain-config.md) | [Volver al índice](./index.md) | [Siguiente: Domain Model](./story-004-domain-model.md)
