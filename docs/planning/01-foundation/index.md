# Épica 01: Foundation - Proyecto Base y Toolchain Rust

## Objetivo

Establecer la base sólida del proyecto Vortex Config: un servidor de configuración cloud-native inspirado en Spring Cloud Config, implementado en Rust. Esta épica crea el workspace multi-crate, configura el toolchain de desarrollo, implementa el pipeline CI, y define el modelo de dominio core junto con el sistema de manejo de errores.

## Alcance

Esta épica cubre exclusivamente la **infraestructura de desarrollo** y el **modelo de dominio base**. No implementa funcionalidad de runtime ni integración con backends de configuración.

### Incluido

- Estructura de workspace Cargo multi-crate
- Configuración completa del toolchain (rustfmt, clippy, rust-analyzer)
- Pipeline CI con GitHub Actions
- Tipos del dominio core (ConfigMap, PropertySource, etc.)
- Sistema de errores tipado con thiserror

### Excluido

- Servidor HTTP
- Backends de configuración (Git, Vault, S3)
- Autenticación/Autorización
- Métricas y observabilidad

## Conceptos de Rust que se Enseñan

Esta épica introduce los conceptos fundamentales de Rust necesarios para un desarrollador Java:

| Concepto | Equivalente Java | Historia |
|----------|------------------|----------|
| Cargo Workspace | Maven multi-module | 001 |
| Crates y Modules | Packages y Classes | 001 |
| Cargo.toml | pom.xml / build.gradle | 001 |
| rustfmt / clippy | Checkstyle / SpotBugs | 002 |
| cargo test | JUnit | 003 |
| Structs | Classes (data) | 004 |
| Enums | Enums + sealed classes | 004 |
| Derive macros | Lombok annotations | 004 |
| pub visibility | public/private modifiers | 004 |
| Result / Option | Optional / Exceptions | 005 |
| match expressions | switch expressions | 005 |
| thiserror | Exception hierarchies | 005 |

## Historias de Usuario

| ID | Título | Puntos | Prioridad |
|----|--------|--------|-----------|
| [001](./story-001-workspace-setup.md) | Setup del Workspace Multi-Crate | 3 | Alta |
| [002](./story-002-toolchain-config.md) | Configuración de Toolchain y Linting | 2 | Alta |
| [003](./story-003-ci-pipeline.md) | Pipeline CI Básico | 3 | Alta |
| [004](./story-004-domain-model.md) | Modelo de Dominio Core | 5 | Alta |
| [005](./story-005-error-handling.md) | Sistema de Errores con thiserror | 3 | Alta |

**Total estimado: 16 puntos**

## Dependencias

### Épicas Previas

- Ninguna (esta es la épica inicial)

### Dependencias Externas

- Rust toolchain 1.92+ (edición 2024)
- GitHub Actions runners
- Conexión a crates.io

## Criterios de Aceptación de la Épica

- [ ] Workspace compila sin warnings (`cargo build --workspace`)
- [ ] `cargo fmt --check` pasa sin errores
- [ ] `cargo clippy --workspace -- -D warnings` pasa sin errores
- [ ] `cargo test --workspace` ejecuta al menos 10 tests unitarios
- [ ] `cargo audit` no reporta vulnerabilidades críticas
- [ ] CI pipeline ejecuta en < 5 minutos
- [ ] Documentación inline (`cargo doc --workspace`) genera sin errores
- [ ] Todos los tipos públicos tienen doc comments

## Definition of Done

### Para cada Historia

- [ ] Código implementado y compila sin warnings
- [ ] Tests unitarios escritos y pasando (cobertura > 80% para lógica)
- [ ] Documentación inline completa (doc comments en items públicos)
- [ ] PR revisado y aprobado
- [ ] CI pipeline verde
- [ ] Changelog actualizado

### Para la Épica Completa

- [ ] Todas las historias completadas
- [ ] README.md del proyecto actualizado con instrucciones de setup
- [ ] ADRs documentados y aprobados
- [ ] Demo del build pipeline funcionando
- [ ] Retrospectiva realizada

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|--------------|---------|------------|
| Curva de aprendizaje Rust empinada | Alta | Medio | Documentación detallada, comparaciones con Java |
| Conflictos de dependencias en workspace | Baja | Alto | Definir versions en workspace Cargo.toml |
| CI lento por compilación Rust | Media | Bajo | Caché de cargo en GitHub Actions |
| Cambios breaking en modelo de dominio | Media | Alto | Diseño cuidadoso, ADR para decisiones |

## ADRs Sugeridos

### ADR-001: Estructura del Workspace

**Decisión**: Usar workspace multi-crate con separación por responsabilidad.

```
vortex-config/
├── Cargo.toml (workspace)
├── crates/
│   ├── vortex-core/      # Tipos y traits del dominio
│   ├── vortex-server/    # Servidor HTTP (futuro)
│   └── vortex-sources/   # Backends de configuración (futuro)
```

**Justificación**:

- Compilación incremental más rápida
- Boundaries claros entre responsabilidades
- Facilita testing aislado
- Similar a módulos Maven pero con mejor gestión de dependencias

### ADR-002: Estrategia de Versionado

**Decisión**: Versionado unificado para todos los crates del workspace.

**Justificación**:

- Simplifica releases
- Evita incompatibilidades entre crates internos
- Patrón común en proyectos Rust multi-crate

### ADR-003: Error Handling Pattern

**Decisión**: Usar `thiserror` para errores de librería, tipos Result custom.

**Justificación**:

- `thiserror` genera implementaciones de `std::error::Error`
- Permite errores tipados sin boilerplate
- Patrón idiomático en el ecosistema Rust

## Reglas Estrictas para Cambios

### Modificación de Archivos de esta Épica

1. **Nunca modificar** archivos de historias completadas sin crear un ADR de cambio
2. **Cualquier cambio** al modelo de dominio requiere actualizar todas las historias afectadas
3. **Cambios en CI** deben probarse en branch antes de merge a main
4. **Nuevas dependencias** requieren justificación en el PR

### Proceso de Cambio

```
1. Crear issue describiendo el cambio necesario
2. Si afecta modelo de dominio → crear ADR
3. Actualizar historia(s) afectada(s)
4. Actualizar index.md (esta página)
5. Actualizar Changelog
6. PR con revisión obligatoria
```

### Versionado de Documentación

- Cambios menores (typos, clarificaciones): incrementar patch en Changelog
- Cambios en scope/criterios: incrementar minor en Changelog
- Cambios estructurales: incrementar major en Changelog

## Estructura de Archivos Generados

Al completar esta épica, el repositorio tendrá:

```
vortex-config/
├── Cargo.toml                    # Workspace manifest
├── rust-toolchain.toml           # Versión de Rust pinned
├── rustfmt.toml                  # Configuración de formateo
├── clippy.toml                   # Configuración de linting
├── .cargo/
│   └── config.toml               # Configuración de cargo
├── .github/
│   └── workflows/
│       └── ci.yml                # Pipeline CI
├── crates/
│   ├── vortex-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── config.rs         # ConfigMap, PropertySource
│   │       ├── environment.rs    # Application, Profile, Label
│   │       └── error.rs          # Jerarquía de errores
│   ├── vortex-server/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   └── vortex-sources/
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs
└── docs/
    └── planning/
        └── 01-foundation/
            ├── index.md
            ├── story-001-workspace-setup.md
            ├── story-002-toolchain-config.md
            ├── story-003-ci-pipeline.md
            ├── story-004-domain-model.md
            └── story-005-error-handling.md
```

## Métricas de Éxito

| Métrica | Objetivo | Cómo Medir |
|---------|----------|------------|
| Tiempo de CI | < 5 min | GitHub Actions |
| Warnings de compilación | 0 | `cargo build 2>&1 \| grep warning` |
| Cobertura de tests | > 80% | cargo-tarpaulin |
| Doc coverage | 100% public items | `cargo doc --document-private-items` |

## Changelog

### [Unreleased]

#### Added

- Documento inicial de la épica
- 5 historias de usuario definidas

#### Changed

- (ninguno)

#### Deprecated

- (ninguno)

#### Removed

- (ninguno)

#### Fixed

- (ninguno)

---

**Navegación**: [Volver al índice de épicas](../README.md) | [Siguiente épica: Storage Backends](../02-storage/index.md)
