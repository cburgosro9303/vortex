# Epica 09: Features Avanzadas (Flags, Templating, Compliance)

## Objetivo

Implementar capacidades avanzadas que diferencian a Vortex Config de Spring Cloud Config, transformandolo de un simple servidor de configuracion a una plataforma completa de gestion de configuracion enterprise. Esta epica agrega:

- **Feature Flags**: Sistema de feature toggles con targeting y rollouts graduales
- **Templating Dinamico**: Motor Tera para templates de configuracion con funciones built-in
- **Compliance Engine**: Motor de reglas para validacion de cumplimiento (PCI-DSS, SOC2)

Estas capacidades permiten:
- Activar/desactivar features sin deployments
- Generar configuraciones dinamicas basadas en contexto
- Garantizar que las configuraciones cumplan politicas de seguridad y compliance

Esta epica es fundamental para organizaciones que requieren control granular sobre el comportamiento de sus aplicaciones y necesitan demostrar cumplimiento normativo.

---

## Conceptos de Rust Cubiertos (Nivel Enterprise)

| Concepto | Historia | Comparacion con Java |
|----------|----------|---------------------|
| Clean Architecture | Todas | Hexagonal Architecture |
| Plugin Architecture | 001-003 | SPI / ServiceLoader |
| Domain Modeling con Enums | 001 | Sealed Classes + Records |
| Serde Tagging | 001 | Jackson Polymorphism |
| Template Engines (Tera) | 004, 005 | Thymeleaf / Freemarker |
| Custom Template Functions | 005 | Custom Dialect |
| Rule Engines | 006 | Drools / Easy Rules |
| Pattern Matching Avanzado | 006 | Switch Expressions |
| Consistent Hashing | 002 | Guava Hashing |
| Report Generation | 007 | JasperReports |

---

## Historias de Usuario

| # | Titulo | Descripcion | Puntos |
|---|--------|-------------|--------|
| 001 | [Modelo de Feature Flags](./story-001-flag-model.md) | Definir tipos para flags con targeting | 5 |
| 002 | [Evaluador de Feature Flags](./story-002-flag-evaluator.md) | Motor de evaluacion con contexto | 8 |
| 003 | [API de Feature Flags](./story-003-flag-api.md) | Endpoints REST para flags | 5 |
| 004 | [Configuration Templating](./story-004-templating.md) | Integrar Tera para templates dinamicos | 5 |
| 005 | [Funciones Built-in de Templates](./story-005-template-functions.md) | env(), secrets(), base64, etc. | 5 |
| 006 | [Compliance Rules Engine](./story-006-compliance-engine.md) | Motor para reglas PCI-DSS, SOC2 | 8 |
| 007 | [API de Compliance Reports](./story-007-compliance-api.md) | Generar reportes de cumplimiento | 5 |

**Total**: 41 puntos de historia

---

## Dependencias

### Epicas Prerequisito

| Epica | Razon |
|-------|-------|
| 01 - Foundation | Workspace configurado, toolchain, CI basico |
| 02 - Core Types | ConfigMap, PropertySource, tipos base |
| 03 - HTTP Server | API REST con Axum |
| 07 - Governance | PLAC engine, esquemas de validacion, patrones de governance |

### Dependencias de Crates

```toml
[dependencies]
# Templating
tera = "1.19"

# Feature flags
siphasher = "1.0"        # Consistent hashing

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Date/Time
chrono = { version = "0.4", features = ["serde"] }

# Regex para compliance
regex = "1"

# Async
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"

# HTTP client para secrets
reqwest = { version = "0.12", features = ["json"] }

# Encoding
base64 = "0.22"
percent-encoding = "2"

# Observabilidad
tracing = "0.1"

# Errores
thiserror = "1"

[dev-dependencies]
tokio-test = "0.4"
wiremock = "0.6"
```

### Feature Flags del Crate

```toml
[features]
default = ["flags", "templating"]
flags = []
templating = ["tera"]
compliance = ["regex"]
full = ["flags", "templating", "compliance"]
```

---

## Criterios de Aceptacion

### Funcionales

- [ ] Feature flags soportan boolean, string, number, JSON variants
- [ ] Targeting por user ID, group, porcentaje, atributos custom
- [ ] Evaluacion consistente (mismo user = mismo resultado)
- [ ] Templates Tera procesan configuraciones dinamicas
- [ ] Funciones built-in: env(), secret(), base64_encode/decode, urlencode
- [ ] Compliance engine evalua reglas PCI-DSS y SOC2
- [ ] Reportes de compliance en JSON y formato legible

### No Funcionales

- [ ] Evaluacion de flags < 1ms p99
- [ ] Rendering de templates < 5ms p99
- [ ] Evaluacion de compliance < 10ms para 100 reglas
- [ ] Feature flags stateless (no requieren storage adicional)
- [ ] Templates sandboxed (sin acceso a filesystem)

### Seguridad

- [ ] Templates no pueden ejecutar codigo arbitrario
- [ ] Secrets nunca se loguean en plain text
- [ ] Compliance rules no pueden ser bypassed
- [ ] Feature flag targeting no expone datos sensibles

---

## Definition of Done

- [ ] Codigo compila sin warnings (`cargo build --all-features`)
- [ ] Compila con cada feature flag individual
- [ ] Formateado con `cargo fmt`
- [ ] Sin errores de clippy (`cargo clippy -- -D warnings`)
- [ ] Tests unitarios con cobertura > 80%
- [ ] Rustdoc para todas las APIs publicas
- [ ] Ejemplos de uso documentados
- [ ] Changelog actualizado
- [ ] Sin `unwrap()` en codigo de produccion
- [ ] Logs estructurados con tracing
- [ ] CI pipeline verde

---

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigacion |
|--------|--------------|---------|------------|
| Template injection | Media | Critico | Sandboxing estricto, no auto-escape disabled |
| Secrets en logs | Media | Critico | Redaction automatica, SecretString wrapper |
| Inconsistencia en flag evaluation | Baja | Alto | Consistent hashing con SipHash |
| Performance degradation con muchas reglas | Media | Medio | Lazy evaluation, short-circuit |
| Compliance false positives | Media | Medio | Reglas well-tested, allow-lists |
| Tera CVEs | Baja | Alto | Dependabot, version pinning |

---

## Decisiones Arquitectonicas (ADRs)

### ADR-001: Tera sobre Handlebars para Templating

**Estado**: Aceptado

**Contexto**: Necesitamos un motor de templates seguro y expresivo para configuraciones dinamicas.

**Decision**: Usar Tera como motor de templates.

**Razones**:
- Sintaxis familiar (similar a Jinja2/Django)
- Sandboxing built-in (sin acceso a filesystem)
- Soporte para funciones custom
- Mejor rendimiento que Handlebars en benchmarks
- Mantenido activamente

**Alternativas consideradas**:
- Handlebars: Menos expresivo, sin filtros custom faciles
- MiniJinja: Mas nuevo, menos features
- Liquid: Orientado a CMS

### ADR-002: Feature Flags Stateless

**Estado**: Aceptado

**Contexto**: Necesitamos feature flags que no requieran almacenamiento adicional.

**Decision**: Feature flags se almacenan como parte de la configuracion regular.

**Razones**:
- Sin dependencias adicionales (Redis, DB)
- Versionados junto con configuracion
- Auditoria via Git/backend existente
- Simples de implementar

**Trade-offs**:
- No hay UI de administracion dedicada
- Cambios requieren update de configuracion

### ADR-003: Consistent Hashing para Porcentajes

**Estado**: Aceptado

**Contexto**: Los rollouts por porcentaje deben ser deterministas y estables.

**Decision**: Usar SipHash para hashing consistente de user IDs.

**Razones**:
- Determinista: mismo user = mismo bucket siempre
- Estable: agregar/remover users no afecta a otros
- Uniforme: distribucion equitativa
- Rapido: O(1) evaluacion

**Diagrama**:
```
User ID: "user-123"
Flag ID: "new-checkout"
Salt: flag_id

hash = siphash(user_id + flag_id)
bucket = hash % 100

if bucket < percentage_rollout:
    return variant_enabled
else:
    return variant_disabled
```

### ADR-004: Compliance Rules como DSL

**Estado**: Aceptado

**Contexto**: Las reglas de compliance deben ser declarativas y auditables.

**Decision**: Definir reglas como estructuras de datos, no codigo ejecutable.

**Razones**:
- Auditables por compliance officers
- No requieren compilacion para cambios
- Faciles de serializar/versionar
- Seguras por construccion

**Formato**:
```yaml
rules:
  - id: "pci-dss-3.4"
    name: "Encrypt cardholder data"
    severity: critical
    pattern:
      path: "**.card_number"
      condition: must_not_exist_plaintext
    message: "Card numbers must be encrypted or tokenized"
```

---

## Reglas Estrictas

1. **Templates no acceden a filesystem**: Solo contexto inyectado
2. **Secrets siempre redactados en logs**: Usar SecretString wrapper
3. **Feature flags inmutables por request**: No side effects
4. **Compliance rules no modifican datos**: Solo validan
5. **Consistent hashing obligatorio**: Para rollouts por porcentaje
6. **Funciones de template registradas explicitamente**: No ejecucion arbitraria
7. **Reportes de compliance timestamped**: Para auditoria
8. **Tests para cada regla de compliance**: No deploy sin tests

---

## Estructura del Crate

```
crates/vortex-features/
├── Cargo.toml
├── src/
│   ├── lib.rs                    # Re-exports, feature gates
│   ├── error.rs                  # Tipos de error unificados
│   ├── flags/
│   │   ├── mod.rs                # Feature flags module
│   │   ├── model.rs              # Flag, Variant, Rule types
│   │   ├── evaluator.rs          # Flag evaluation engine
│   │   ├── context.rs            # EvaluationContext
│   │   ├── targeting.rs          # Targeting rules
│   │   └── hashing.rs            # Consistent hashing
│   ├── templating/
│   │   ├── mod.rs                # Templating module
│   │   ├── engine.rs             # Tera wrapper
│   │   ├── functions.rs          # Built-in functions
│   │   ├── filters.rs            # Custom filters
│   │   └── context.rs            # Template context
│   └── compliance/
│       ├── mod.rs                # Compliance module
│       ├── engine.rs             # Rules engine
│       ├── rules.rs              # Rule definitions
│       ├── report.rs             # Report generation
│       └── standards/
│           ├── mod.rs
│           ├── pci_dss.rs        # PCI-DSS rules
│           └── soc2.rs           # SOC2 rules
└── tests/
    ├── flags_test.rs
    ├── templating_test.rs
    └── compliance_test.rs
```

---

## Diagrama de Arquitectura

```
                         ┌─────────────────────────────────────────┐
                         │           Vortex Server                  │
                         │           (HTTP Layer)                   │
                         └─────────────────┬───────────────────────┘
                                           │
              ┌────────────────────────────┼────────────────────────────┐
              │                            │                            │
   ┌──────────▼──────────┐    ┌───────────▼───────────┐    ┌──────────▼──────────┐
   │   Feature Flags     │    │     Templating        │    │    Compliance       │
   │      Engine         │    │       Engine          │    │      Engine         │
   ├─────────────────────┤    ├───────────────────────┤    ├─────────────────────┤
   │ • Flag Definitions  │    │ • Tera Runtime        │    │ • Rule Definitions  │
   │ • Targeting Rules   │    │ • Built-in Functions  │    │ • PCI-DSS Rules     │
   │ • Evaluator         │    │ • Context Injection   │    │ • SOC2 Rules        │
   │ • Consistent Hash   │    │ • Sandboxing          │    │ • Report Generator  │
   └──────────┬──────────┘    └───────────┬───────────┘    └──────────┬──────────┘
              │                           │                           │
              └────────────────────────────┼───────────────────────────┘
                                           │
                         ┌─────────────────▼───────────────────────┐
                         │         Configuration Sources            │
                         │       (Git, S3, SQL, Composite)          │
                         └─────────────────────────────────────────┘
```

---

## Flujo de Feature Flag Evaluation

```
Request: GET /flags/new-checkout?user_id=user-123&environment=production

┌───────────────────────────────────────────────────────────────────────┐
│                      Flag Evaluation Flow                              │
├───────────────────────────────────────────────────────────────────────┤
│                                                                        │
│  1. Load Flag Definition                                               │
│     ┌─────────────────────────────────────────────────────────────┐   │
│     │ flag: new-checkout                                          │   │
│     │ type: boolean                                               │   │
│     │ default: false                                              │   │
│     │ rules:                                                      │   │
│     │   - if: user_id IN ["beta-tester-1", "beta-tester-2"]      │   │
│     │     then: true                                              │   │
│     │   - if: environment == "staging"                            │   │
│     │     then: true                                              │   │
│     │   - if: percentage(30)                                      │   │
│     │     then: true                                              │   │
│     └─────────────────────────────────────────────────────────────┘   │
│                                                                        │
│  2. Build Evaluation Context                                           │
│     ┌─────────────────────────────────────────────────────────────┐   │
│     │ user_id: "user-123"                                         │   │
│     │ environment: "production"                                   │   │
│     │ timestamp: 2024-01-15T10:30:00Z                             │   │
│     └─────────────────────────────────────────────────────────────┘   │
│                                                                        │
│  3. Evaluate Rules (first match wins)                                  │
│     ┌─────────────────────────────────────────────────────────────┐   │
│     │ Rule 1: user_id IN beta-testers? NO                         │   │
│     │ Rule 2: environment == staging? NO                          │   │
│     │ Rule 3: percentage(30)?                                     │   │
│     │         hash("user-123" + "new-checkout") % 100 = 47        │   │
│     │         47 < 30? NO                                         │   │
│     │ Default: false                                              │   │
│     └─────────────────────────────────────────────────────────────┘   │
│                                                                        │
│  4. Return Result                                                      │
│     ┌─────────────────────────────────────────────────────────────┐   │
│     │ { "key": "new-checkout", "value": false, "reason": "default" }  │
│     └─────────────────────────────────────────────────────────────┘   │
│                                                                        │
└───────────────────────────────────────────────────────────────────────┘
```

---

## Flujo de Templating

```
Template: "{{ app.name }}-{{ environment }}.{{ format }}"
Context: { app: { name: "payment" }, environment: "prod", format: "yml" }

┌───────────────────────────────────────────────────────────────────────┐
│                      Templating Flow                                   │
├───────────────────────────────────────────────────────────────────────┤
│                                                                        │
│  1. Parse Template                                                     │
│     ┌─────────────────────────────────────────────────────────────┐   │
│     │ AST: Expr(app.name) + Literal("-") + Expr(environment) +    │   │
│     │      Literal(".") + Expr(format)                            │   │
│     └─────────────────────────────────────────────────────────────┘   │
│                                                                        │
│  2. Build Context with Functions                                       │
│     ┌─────────────────────────────────────────────────────────────┐   │
│     │ Standard context: { app, environment, format }              │   │
│     │ Functions: env(), secret(), base64_encode(), etc.           │   │
│     └─────────────────────────────────────────────────────────────┘   │
│                                                                        │
│  3. Render Template                                                    │
│     ┌─────────────────────────────────────────────────────────────┐   │
│     │ {{ app.name }}     -> "payment"                             │   │
│     │ {{ environment }}  -> "prod"                                │   │
│     │ {{ format }}       -> "yml"                                 │   │
│     └─────────────────────────────────────────────────────────────┘   │
│                                                                        │
│  4. Result                                                             │
│     ┌─────────────────────────────────────────────────────────────┐   │
│     │ "payment-prod.yml"                                          │   │
│     └─────────────────────────────────────────────────────────────┘   │
│                                                                        │
└───────────────────────────────────────────────────────────────────────┘
```

---

## Flujo de Compliance Check

```
Config to validate:
{
  "database": {
    "password": "plaintext123",
    "connection_string": "postgres://user:pass@host/db"
  }
}

┌───────────────────────────────────────────────────────────────────────┐
│                    Compliance Check Flow                               │
├───────────────────────────────────────────────────────────────────────┤
│                                                                        │
│  1. Load Rules (PCI-DSS + SOC2)                                        │
│     ┌─────────────────────────────────────────────────────────────┐   │
│     │ Rule: no-plaintext-passwords                                │   │
│     │   pattern: **.password                                      │   │
│     │   condition: must_be_encrypted_or_reference                 │   │
│     │   severity: CRITICAL                                        │   │
│     │                                                             │   │
│     │ Rule: no-credentials-in-strings                             │   │
│     │   pattern: **.*string*                                      │   │
│     │   condition: no_embedded_credentials                        │   │
│     │   severity: HIGH                                            │   │
│     └─────────────────────────────────────────────────────────────┘   │
│                                                                        │
│  2. Evaluate Each Rule                                                 │
│     ┌─────────────────────────────────────────────────────────────┐   │
│     │ Rule 1: database.password = "plaintext123"                  │   │
│     │         Is encrypted? NO                                    │   │
│     │         Is reference? NO (not $ref or vault://)             │   │
│     │         VIOLATION: CRITICAL                                 │   │
│     │                                                             │   │
│     │ Rule 2: database.connection_string contains credentials     │   │
│     │         Pattern: user:pass@                                 │   │
│     │         VIOLATION: HIGH                                     │   │
│     └─────────────────────────────────────────────────────────────┘   │
│                                                                        │
│  3. Generate Report                                                    │
│     ┌─────────────────────────────────────────────────────────────┐   │
│     │ {                                                           │   │
│     │   "status": "FAILED",                                       │   │
│     │   "violations": [                                           │   │
│     │     { "rule": "no-plaintext-passwords",                     │   │
│     │       "path": "database.password",                          │   │
│     │       "severity": "CRITICAL" },                             │   │
│     │     { "rule": "no-credentials-in-strings",                  │   │
│     │       "path": "database.connection_string",                 │   │
│     │       "severity": "HIGH" }                                  │   │
│     │   ],                                                        │   │
│     │   "checked_at": "2024-01-15T10:30:00Z"                      │   │
│     │ }                                                           │   │
│     └─────────────────────────────────────────────────────────────┘   │
│                                                                        │
└───────────────────────────────────────────────────────────────────────┘
```

---

## Changelog

| Version | Fecha | Cambios |
|---------|-------|---------|
| 0.1.0 | 2025-01-XX | Creacion inicial de la epica |

---

## Referencias

- [Tera Template Engine](https://tera.netlify.app/)
- [LaunchDarkly Feature Flags](https://docs.launchdarkly.com/)
- [PCI-DSS Requirements](https://www.pcisecuritystandards.org/)
- [SOC2 Compliance](https://www.aicpa.org/soc2)
- [Consistent Hashing Explained](https://en.wikipedia.org/wiki/Consistent_hashing)
- [Spring Cloud Config](https://spring.io/projects/spring-cloud-config)
