# Historia 002: Configuración de Toolchain y Linting

## Contexto y Objetivo

Un proyecto Rust profesional requiere herramientas de calidad de código consistentes. Esta historia configura el toolchain de Rust y las herramientas de linting que garantizarán un código limpio, idiomático y mantenible.

Para un desarrollador Java, esto es equivalente a configurar:

- **rustfmt** = Checkstyle / Google Java Format
- **clippy** = SpotBugs / SonarLint
- **rust-analyzer** = IntelliJ IDEA / Eclipse JDT

## Alcance

### In Scope

- Instalar y configurar rustup con versión específica de Rust
- Crear `rust-toolchain.toml` para pinear versión del compilador
- Configurar `rustfmt.toml` con reglas de formateo del proyecto
- Configurar `clippy.toml` con reglas de linting
- Crear `.cargo/config.toml` con aliases y configuración local
- Documentar configuración recomendada de rust-analyzer para VS Code

### Out of Scope

- Pipeline CI (ver Historia 003)
- Pre-commit hooks (pueden añadirse después)
- Configuración de IDEs distintos a VS Code

## Criterios de Aceptación

- [ ] `rust-toolchain.toml` especifica Rust 1.92+
- [ ] `cargo fmt --check` funciona sin errores
- [ ] `cargo clippy --workspace -- -D warnings` pasa sin warnings
- [ ] Aliases útiles configurados en `.cargo/config.toml`
- [ ] Documentación de setup de rust-analyzer incluida
- [ ] Todos los archivos de configuración comentados

## Diseño Propuesto

### Archivos de Configuración

```
vortex-config/
├── rust-toolchain.toml      # Versión de Rust del proyecto
├── rustfmt.toml             # Reglas de formateo
├── clippy.toml              # Configuración de clippy
├── .cargo/
│   └── config.toml          # Aliases y configuración de cargo
└── .vscode/
    └── settings.json        # Configuración rust-analyzer (recomendada)
```

### rust-toolchain.toml

```toml
[toolchain]
channel = "1.92"
components = ["rustfmt", "clippy", "rust-analyzer"]
targets = ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]
```

### rustfmt.toml

```toml
# Vortex Config - Rust Formatting Rules
# Documentación: https://rust-lang.github.io/rustfmt/

edition = "2024"
max_width = 100
tab_spaces = 4
newline_style = "Unix"

# Imports
imports_granularity = "Module"
group_imports = "StdExternalCrate"
reorder_imports = true

# Comments
wrap_comments = true
format_code_in_doc_comments = true
doc_comment_code_block_width = 80

# Functions
fn_params_layout = "Tall"
fn_single_line = false

# Control flow
match_arm_blocks = true
match_block_trailing_comma = true

# Structs
struct_field_align_threshold = 0
```

### clippy.toml

```toml
# Vortex Config - Clippy Configuration
# Documentación: https://rust-lang.github.io/rust-clippy/

# Complejidad cognitiva máxima por función
cognitive-complexity-threshold = 15

# Máximo de argumentos en funciones
too-many-arguments-threshold = 7

# Líneas máximas por función
too-many-lines-threshold = 100

# Tipos que se permiten en unwrap (para tests principalmente)
allowed-scripts = ["Latin"]

# MSRVs (Minimum Supported Rust Version)
msrv = "1.92"
```

### .cargo/config.toml

```toml
# Vortex Config - Cargo Configuration

[alias]
# Desarrollo diario
c = "check --workspace"
b = "build --workspace"
t = "test --workspace"
r = "run -p vortex-server"

# Calidad de código
lint = "clippy --workspace --all-targets -- -D warnings"
fmt-check = "fmt --all -- --check"
audit = "audit --deny warnings"

# Release
release = "build --workspace --release"

[build]
# Más threads para linking
jobs = 8

[target.x86_64-unknown-linux-gnu]
# Usar mold linker en Linux si está disponible
# linker = "clang"
# rustflags = ["-C", "link-arg=-fuse-ld=mold"]

[target.aarch64-apple-darwin]
# Configuración para Apple Silicon
rustflags = ["-C", "target-cpu=native"]
```

## Pasos de Implementación

### Paso 1: Instalar rustup (si no está instalado)

```bash
# Verificar instalación
rustup --version

# Si no está instalado:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Paso 2: Crear rust-toolchain.toml

```bash
cd vortex-config
cat > rust-toolchain.toml << 'EOF'
[toolchain]
channel = "1.92"
components = ["rustfmt", "clippy", "rust-analyzer"]
EOF
```

### Paso 3: Crear rustfmt.toml

Crear el archivo con la configuración especificada arriba.

### Paso 4: Crear clippy.toml

Crear el archivo con la configuración especificada arriba.

### Paso 5: Crear .cargo/config.toml

```bash
mkdir -p .cargo
# Crear config.toml con la configuración especificada
```

### Paso 6: Verificar configuración

```bash
# Verificar que el toolchain se instala correctamente
rustup show

# Verificar formateo
cargo fmt --all -- --check

# Verificar linting
cargo clippy --workspace -- -D warnings
```

## Conceptos de Rust Aprendidos

### Rustup y Toolchain Management

Rustup es el gestor de versiones de Rust, similar a SDKMAN! o jEnv para Java.

```bash
# Instalar una versión específica
rustup install 1.92

# Establecer versión por defecto
rustup default 1.92

# Ver toolchains instalados
rustup show

# Actualizar todo
rustup update

# Agregar componente (clippy, rustfmt, etc.)
rustup component add clippy rustfmt rust-analyzer

# Agregar target para cross-compilation
rustup target add aarch64-unknown-linux-gnu
```

**rust-toolchain.toml** permite que el proyecto especifique su toolchain:

```toml
[toolchain]
channel = "1.92"              # Versión de Rust
components = [                # Componentes necesarios
    "rustfmt",
    "clippy",
    "rust-analyzer"
]
targets = [                   # Targets de compilación
    "x86_64-unknown-linux-gnu",
    "aarch64-apple-darwin"
]
profile = "default"           # minimal, default, complete
```

**Comparación con Java:**

| Rust (rustup) | Java |
|---------------|------|
| `rustup install 1.92` | `sdk install java 21-tem` |
| `rust-toolchain.toml` | `.sdkmanrc` o `.java-version` |
| `rustup default` | `sdk default java` |
| `rustup component add` | Instalado junto con JDK |

### Rustfmt - Formateo de Código

Rustfmt es el formateador oficial de Rust. A diferencia de Java donde hay múltiples opciones (Google Java Format, Checkstyle, etc.), Rust tiene un estándar de facto.

```toml
# rustfmt.toml - Configuración de formateo
edition = "2024"              # Edición de Rust
max_width = 100               # Ancho máximo de línea

# Control de imports
imports_granularity = "Module"    # Agrupar imports por módulo
group_imports = "StdExternalCrate" # Orden: std, externos, crate
reorder_imports = true            # Ordenar alfabéticamente

# Ejemplo de cómo formatea los imports:
# ANTES:
# use std::io::{Read, Write};
# use serde::Deserialize;
# use crate::config::ConfigMap;
# use std::collections::HashMap;

# DESPUÉS:
# use std::collections::HashMap;
# use std::io::{Read, Write};
#
# use serde::Deserialize;
#
# use crate::config::ConfigMap;
```

**Comandos útiles:**

```bash
# Formatear todo el proyecto
cargo fmt

# Solo verificar (para CI)
cargo fmt --check

# Formatear un archivo específico
rustfmt src/lib.rs

# Ver diferencias sin aplicar
cargo fmt -- --check --diff
```

**Comparación con Java:**

| rustfmt | Google Java Format |
|---------|-------------------|
| `cargo fmt` | `google-java-format *.java` |
| `cargo fmt --check` | `google-java-format --dry-run` |
| `rustfmt.toml` | `.editorconfig` / IDE settings |
| Incluido con rustup | Descarga separada |

### Clippy - Linting Avanzado

Clippy es el linter oficial de Rust con más de 600 lints organizados en categorías.

```rust
// Ejemplo de warnings que Clippy detecta:

// 1. clippy::unwrap_used - Usar unwrap sin manejo de error
let value = some_option.unwrap(); // WARNING
let value = some_option.expect("reason"); // Mejor
let value = some_option.unwrap_or_default(); // Aún mejor

// 2. clippy::needless_return - Return innecesario
fn foo() -> i32 {
    return 42; // WARNING: needless return
}
fn foo() -> i32 {
    42 // OK: expresión implícita
}

// 3. clippy::clone_on_copy - Clone innecesario en tipo Copy
let x: i32 = 5;
let y = x.clone(); // WARNING: i32 is Copy
let y = x;         // OK

// 4. clippy::redundant_closure - Closure innecesario
vec.iter().map(|x| foo(x))  // WARNING
vec.iter().map(foo)          // OK

// 5. clippy::large_enum_variant - Variante de enum muy grande
enum Event {
    Small(u32),
    Large([u8; 1024]), // WARNING: considera Box
}
enum Event {
    Small(u32),
    Large(Box<[u8; 1024]>), // OK
}
```

**Categorías de lints:**

```bash
# Lints por defecto (warn)
cargo clippy

# Denegar todos los warnings (para CI)
cargo clippy -- -D warnings

# Habilitar lints pedánticos (estrictos)
cargo clippy -- -W clippy::pedantic

# Permitir un lint específico
cargo clippy -- -A clippy::too_many_arguments
```

**Atributos en código:**

```rust
// Permitir un lint para un item específico
#[allow(clippy::too_many_arguments)]
fn complex_function(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32) {
    // ...
}

// Denegar un lint (convierte warning en error)
#![deny(clippy::unwrap_used)]

// A nivel de crate (en lib.rs)
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
```

**Comparación con Java:**

| Clippy | SpotBugs / SonarLint |
|--------|---------------------|
| `cargo clippy` | `mvn spotbugs:check` |
| `clippy.toml` | `spotbugs-exclude.xml` |
| `#[allow(...)]` | `@SuppressFBWarnings` |
| 600+ lints | 400+ bug patterns |

### Rust-Analyzer

Rust-analyzer es el language server oficial para Rust, proporcionando IDE features.

```json
// .vscode/settings.json
{
    // Habilitar rust-analyzer
    "rust-analyzer.check.command": "clippy",

    // Usar cargo check en lugar de cargo build (más rápido)
    "rust-analyzer.cargo.buildScripts.enable": true,

    // Mostrar hints de tipos
    "rust-analyzer.inlayHints.typeHints.enable": true,
    "rust-analyzer.inlayHints.parameterHints.enable": true,

    // Completado de imports
    "rust-analyzer.completion.autoimport.enable": true,

    // Formatear al guardar
    "editor.formatOnSave": true,
    "[rust]": {
        "editor.defaultFormatter": "rust-lang.rust-analyzer"
    },

    // Ejecutar clippy en lugar de check
    "rust-analyzer.check.command": "clippy",
    "rust-analyzer.check.extraArgs": ["--", "-W", "clippy::pedantic"]
}
```

**Features principales:**

```rust
// 1. Go to Definition - Navegar a definición
// 2. Find References - Encontrar usos
// 3. Rename Symbol - Refactoring seguro
// 4. Inlay Hints - Tipos inferidos visibles

let config = load_config("app.toml");
//  ^^^^^^ : Result<Config, Error>  <- Inlay hint

// 5. Code Actions - Quick fixes
vec.iter().map(|x| x.clone())  // Light bulb: usar .cloned()

// 6. Hover Documentation
// Pasa el cursor sobre cualquier item para ver docs
```

## Riesgos y Errores Comunes

### Error 1: Conflicto de versiones de rustfmt

```bash
# Si rustfmt local difiere del CI
error: couldn't read rustfmt.toml

# Solución: Asegurar mismo toolchain
rustup override set 1.92
```

### Error 2: Clippy warnings en código generado

```rust
// MAL: Clippy analiza código de macros
#[derive(Deserialize)]
struct Config {
    name: String, // WARNING: field never read
}

// BIEN: Desactivar para código generado específico
#[derive(Deserialize)]
#[allow(dead_code)] // Campos usados por deserialización
struct Config {
    name: String,
}
```

### Error 3: rust-analyzer consume mucha memoria

```json
// En settings.json, limitar recursos
{
    "rust-analyzer.cargo.features": [],  // No analizar todas las features
    "rust-analyzer.procMacro.enable": false, // Desactivar si no se necesita
}
```

### Error 4: Formateo inconsistente entre equipos

```bash
# Siempre usar la versión del proyecto
# rust-toolchain.toml lo garantiza

# Verificar en CI
cargo fmt --check || exit 1
```

## Pruebas

### Verificación de Configuración

```bash
# Test 1: Toolchain se instala correctamente
rustup show | grep "1.92"

# Test 2: Formateo funciona
cargo fmt --check
echo $?  # Debe ser 0

# Test 3: Clippy funciona
cargo clippy --workspace -- -D warnings
echo $?  # Debe ser 0

# Test 4: Aliases funcionan
cargo lint  # Debe ejecutar clippy
cargo c     # Debe ejecutar check
```

### Script de Verificación

```bash
#!/bin/bash
# scripts/verify-toolchain.sh

set -e

echo "Verificando toolchain..."
rustup show

echo "Verificando formateo..."
cargo fmt --check

echo "Verificando linting..."
cargo clippy --workspace -- -D warnings

echo "Todas las verificaciones pasaron!"
```

## Entregable Final

### PR debe incluir

1. **Archivos nuevos:**
   - `rust-toolchain.toml`
   - `rustfmt.toml`
   - `clippy.toml`
   - `.cargo/config.toml`
   - `.vscode/settings.json` (recomendado, en .gitignore o docs)

2. **Verificaciones:**
   - `cargo fmt --check` exitoso
   - `cargo clippy --workspace -- -D warnings` sin warnings
   - Screenshot de rust-analyzer funcionando

3. **Documentación:**
   - Comentarios en cada archivo de configuración
   - Instrucciones de setup en README.md

### Checklist de Revisión

- [ ] rust-toolchain.toml especifica versión 1.92+
- [ ] rustfmt.toml tiene reglas documentadas
- [ ] clippy.toml tiene thresholds razonables
- [ ] Aliases de cargo documentados en config.toml
- [ ] Configuración de VS Code incluida
- [ ] Todos los archivos tienen comentarios explicativos
- [ ] `cargo fmt --check` pasa
- [ ] `cargo clippy -- -D warnings` pasa

---

**Navegación:** [Anterior: Workspace Setup](./story-001-workspace-setup.md) | [Volver al índice](./index.md) | [Siguiente: CI Pipeline](./story-003-ci-pipeline.md)
