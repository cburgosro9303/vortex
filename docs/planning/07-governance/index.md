# Epica 07: Governance - PLAC y Schema Validation

## Objetivo

Implementar un sistema de gobernanza robusto para Vortex Config que proporcione control de acceso granular mediante PLAC (Policy Language for Access Control) y validacion de configuraciones contra JSON Schemas.

Esta epica introduce:
- **PLAC**: Un lenguaje declarativo para definir politicas de acceso a configuraciones
- **Schema Validation**: Validacion de configuraciones contra JSON Schemas antes de servirlas
- **Acciones de Gobernanza**: Deny, redact, mask y warn para control fino de respuestas

La gobernanza es fundamental para organizaciones que necesitan:
- Control de acceso basado en roles/contexto a configuraciones sensibles
- Validacion automatica de que las configuraciones cumplen esquemas definidos
- Enmascaramiento de datos sensibles (API keys, passwords) segun el consumidor
- Auditoria y trazabilidad de accesos a configuraciones

---

## Conceptos de Rust Cubiertos (Nivel Avanzado)

| Concepto | Historia | Comparacion con Java |
|----------|----------|---------------------|
| Macros declarativas (`macro_rules!`) | 001, 003 | No hay equivalente directo |
| DSL (Domain Specific Language) | 001-003 | Builder patterns, fluent APIs |
| Parsing con nom/pest | 002 | ANTLR, JavaCC |
| Builder Pattern | 001 | Builder pattern clasico |
| Enums con datos (tagged unions) | 001, 006 | Sealed classes (Java 17+) |
| Pattern Matching avanzado | 003 | switch expressions (Java 14+) |
| Visitor Pattern | 003 | Visitor pattern clasico |
| Serde custom serialization | 002 | Jackson custom deserializers |
| Strategy Pattern | 006 | Strategy pattern clasico |
| Trait objects (dyn Trait) | 003, 004 | Interface references |
| Axum middleware layers | 004 | Spring Filters/Interceptors |
| Response transformation | 006 | ResponseBodyAdvice |

---

## Historias de Usuario

| # | Titulo | Descripcion | Puntos |
|---|--------|-------------|--------|
| 001 | [Modelo de Politicas PLAC](./story-001-plac-model.md) | Definir structs y enums para el modelo de politicas | 5 |
| 002 | [Parser de Politicas YAML](./story-002-policy-parser.md) | Cargar y validar politicas desde archivos YAML | 5 |
| 003 | [Motor de Evaluacion PLAC](./story-003-evaluation-engine.md) | Evaluar politicas contra contexto de request | 8 |
| 004 | [Integracion con Middleware](./story-004-middleware-integration.md) | Aplicar PLAC en el pipeline HTTP de Axum | 5 |
| 005 | [JSON Schema Validation](./story-005-schema-validation.md) | Validar configuraciones contra JSON Schemas | 5 |
| 006 | [Acciones de Gobernanza](./story-006-governance-actions.md) | Implementar deny, redact, mask, warn | 5 |

**Total**: 33 puntos de historia

---

## Dependencias

### Epicas Prerequisito

| Epica | Razon |
|-------|-------|
| 01 - Foundation | Workspace configurado, toolchain, CI basico |
| 02 - Core Types | ConfigMap, PropertySource, serializacion con serde |
| 03 - HTTP Server | Axum server, middleware stack, extractors |

### Dependencias de Crates

```toml
[dependencies]
# Serializacion y parsing
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
serde_json = "1"

# JSON Schema validation
jsonschema = "0.18"

# Framework HTTP
axum = "0.7"
tower = "0.4"

# Async
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"

# Pattern matching y regex
regex = "1"
glob-match = "0.2"

# Observabilidad
tracing = "0.1"

# Errores
thiserror = "1"

[dev-dependencies]
tokio-test = "0.4"
tempfile = "3"
```

---

## Criterios de Aceptacion

### Funcionales

- [ ] Politicas PLAC se definen en archivos YAML con sintaxis clara
- [ ] El parser valida la sintaxis y semantica de las politicas
- [ ] El motor evalua politicas contra contexto (app, profile, label, headers, IP)
- [ ] Middleware intercepta requests y aplica politicas antes de responder
- [ ] JSON Schemas se cargan desde archivos o URL
- [ ] Configuraciones invalidas se rechazan con errores descriptivos
- [ ] Accion `deny` rechaza el request con 403
- [ ] Accion `redact` elimina propiedades sensibles de la respuesta
- [ ] Accion `mask` reemplaza valores con asteriscos
- [ ] Accion `warn` incluye warnings en headers de respuesta

### No Funcionales

- [ ] Evaluacion de politicas < 1ms por request
- [ ] Politicas se cachean y hot-reload en cambios
- [ ] Validacion de schema < 5ms para configuraciones tipicas
- [ ] Memory footprint < 10MB para cache de politicas

### Seguridad

- [ ] Las politicas no se exponen via API
- [ ] Errores de evaluacion no revelan detalles internos
- [ ] Logs de auditoria para accesos denegados
- [ ] Valores enmascarados no son recuperables

---

## Definition of Done

- [ ] Codigo compila sin warnings (`cargo build --all-features`)
- [ ] Formateado con `cargo fmt`
- [ ] Sin errores de clippy (`cargo clippy -- -D warnings`)
- [ ] Tests unitarios con cobertura > 80%
- [ ] Tests de integracion para pipeline completo
- [ ] Rustdoc para todas las APIs publicas
- [ ] Ejemplos de politicas documentados
- [ ] Changelog actualizado
- [ ] Sin `unwrap()` en codigo de produccion
- [ ] Logs estructurados con tracing
- [ ] CI pipeline verde

---

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigacion |
|--------|--------------|---------|------------|
| Politicas complejas afectan performance | Media | Alto | Cache de evaluacion, optimizacion de matching |
| Errores en politicas causan denegaciones incorrectas | Media | Alto | Modo dry-run, validacion estricta, tests exhaustivos |
| Schema validation muy estricta rompe compatibilidad | Media | Medio | Modo warn vs strict, validacion progresiva |
| Bypass de politicas por error de implementacion | Baja | Critico | Code review, tests de seguridad, fail-closed |
| Complejidad de DSL para usuarios | Media | Medio | Documentacion extensa, ejemplos, validador online |

---

## Decisiones Arquitectonicas (ADRs)

### ADR-001: PLAC como DSL Declarativo en YAML

**Estado**: Aceptado

**Contexto**: Necesitamos un lenguaje para expresar politicas de acceso que sea legible, versionable y facil de auditar.

**Decision**: Usar YAML como sintaxis base para PLAC con estructura predefinida.

**Razones**:
- YAML es familiar para equipos DevOps/SRE
- Versionable en Git junto con configuraciones
- Parseable con serde_yaml existente
- Legible para auditores no tecnicos
- Facil de generar programaticamente

**Alternativas consideradas**:
- Rego (OPA): Muy potente pero curva de aprendizaje alta
- JSON: Menos legible para humanos
- DSL custom: Mayor esfuerzo de implementacion

**Ejemplo de Politica PLAC**:
```yaml
policies:
  - name: protect-production-secrets
    description: Mask secrets in production configs
    priority: 100
    conditions:
      - field: profile
        operator: equals
        value: production
      - field: property_path
        operator: matches
        value: "*.password|*.secret|*.api_key"
    action:
      type: mask
      mask_char: "*"
      visible_chars: 4

  - name: deny-external-access-to-internal
    description: Block external IPs from internal configs
    priority: 200
    conditions:
      - field: application
        operator: matches
        value: "internal-*"
      - field: source_ip
        operator: not_in_cidr
        value: "10.0.0.0/8"
    action:
      type: deny
      message: "Access denied: internal configs require internal network"
```

### ADR-002: Evaluacion Basada en Prioridades

**Estado**: Aceptado

**Contexto**: Multiples politicas pueden aplicar a un mismo request. Necesitamos resolver conflictos.

**Decision**: Evaluar politicas por prioridad (mayor numero = mayor prioridad). Primera politica que matchea con accion terminal (deny) gana. Acciones no terminales se acumulan.

**Razones**:
- Modelo mental simple
- Predecible y auditable
- Similar a firewalls y ACLs
- Permite override con alta prioridad

**Orden de evaluacion**:
1. Ordenar politicas por prioridad descendente
2. Evaluar condiciones de cada politica
3. Si match y accion es `deny`: terminar con denegacion
4. Si match y accion es `redact`/`mask`/`warn`: acumular
5. Aplicar todas las acciones acumuladas a la respuesta

### ADR-003: JSON Schema para Validacion Estructural

**Estado**: Aceptado

**Contexto**: Necesitamos validar que las configuraciones cumplan estructuras esperadas.

**Decision**: Usar JSON Schema Draft 2020-12 via crate `jsonschema`.

**Razones**:
- Estandar ampliamente adoptado
- Tooling existente (editores, validadores online)
- Documentacion de schemas como contrato
- Soporte en crate maduro

**Ejemplo**:
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "server.port": {
      "type": "integer",
      "minimum": 1024,
      "maximum": 65535
    },
    "database.url": {
      "type": "string",
      "format": "uri"
    }
  },
  "required": ["server.port"]
}
```

### ADR-004: Middleware como Punto de Enforcement

**Estado**: Aceptado

**Contexto**: Las politicas deben aplicarse consistentemente a todos los requests.

**Decision**: Implementar governance como Axum middleware layer.

**Razones**:
- Punto unico de enforcement
- No bypasseable por handlers
- Composable con otros middleware
- Acceso a request completo y response

**Flujo**:
```
Request
   │
   ▼
┌──────────────────┐
│ GovernanceLayer  │ ← Evalua politicas
└────────┬─────────┘
         │
         ▼ (si permitido)
┌──────────────────┐
│ Handler          │ ← Obtiene config
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ GovernanceLayer  │ ← Transforma respuesta (redact/mask)
└────────┬─────────┘
         │
         ▼
Response
```

---

## Reglas Estrictas

1. **Fail-closed por defecto**: Si hay error evaluando politicas, denegar
2. **No bypass de governance**: Todo request pasa por middleware
3. **Politicas inmutables en runtime**: Cambios requieren reload explicito
4. **Auditoria obligatoria**: Todo deny se logea con contexto completo
5. **Schemas versionados**: Schemas incluyen version para compatibilidad
6. **Tests de regresion**: Cada politica tiene tests asociados
7. **Separacion de concerns**: Modelo, parser, evaluador, acciones son modulos separados
8. **No secrets en politicas**: Politicas no contienen valores sensibles

---

## Estructura del Crate

```
crates/vortex-governance/
├── Cargo.toml
├── src/
│   ├── lib.rs                  # Re-exports publicos
│   ├── error.rs                # Tipos de error
│   ├── plac/
│   │   ├── mod.rs              # Modulo PLAC
│   │   ├── model.rs            # Structs: Policy, Condition, Action
│   │   ├── parser.rs           # YAML parser
│   │   ├── engine.rs           # Motor de evaluacion
│   │   └── context.rs          # RequestContext para evaluacion
│   ├── schema/
│   │   ├── mod.rs              # Modulo Schema
│   │   ├── loader.rs           # Carga schemas desde archivos/URL
│   │   ├── validator.rs        # Validador JSON Schema
│   │   └── registry.rs         # Cache de schemas compilados
│   ├── actions/
│   │   ├── mod.rs              # Modulo Acciones
│   │   ├── deny.rs             # Accion deny
│   │   ├── redact.rs           # Accion redact
│   │   ├── mask.rs             # Accion mask
│   │   └── warn.rs             # Accion warn
│   └── middleware/
│       ├── mod.rs              # Modulo Middleware
│       ├── layer.rs            # GovernanceLayer
│       └── extractor.rs        # Extractors para contexto
├── schemas/
│   └── policy-schema.json      # Schema para validar politicas
└── tests/
    ├── plac_model_test.rs
    ├── parser_test.rs
    ├── engine_test.rs
    ├── middleware_test.rs
    ├── schema_test.rs
    ├── actions_test.rs
    └── fixtures/
        ├── policies/
        │   ├── basic.yaml
        │   ├── complex.yaml
        │   └── invalid.yaml
        └── schemas/
            ├── app-config.json
            └── db-config.json
```

---

## Diagrama de Arquitectura

```
                         ┌─────────────────────┐
                         │   HTTP Request      │
                         │ GET /myapp/prod     │
                         └──────────┬──────────┘
                                    │
                         ┌──────────▼──────────┐
                         │   RequestContext    │
                         │ app: myapp          │
                         │ profile: prod       │
                         │ ip: 10.0.0.5        │
                         │ headers: {...}      │
                         └──────────┬──────────┘
                                    │
                         ┌──────────▼──────────┐
                         │  GovernanceLayer    │
                         │  (Pre-handler)      │
                         └──────────┬──────────┘
                                    │
              ┌─────────────────────┼─────────────────────┐
              │                     │                     │
    ┌─────────▼─────────┐ ┌────────▼────────┐ ┌─────────▼─────────┐
    │  Policy Engine    │ │ Schema Registry │ │  Action Registry  │
    │                   │ │                 │ │                   │
    │ - Load policies   │ │ - Load schemas  │ │ - Deny            │
    │ - Evaluate rules  │ │ - Validate JSON │ │ - Redact          │
    │ - Match context   │ │                 │ │ - Mask            │
    └─────────┬─────────┘ └────────┬────────┘ │ - Warn            │
              │                    │          └─────────┬─────────┘
              │                    │                    │
              └────────────────────┼────────────────────┘
                                   │
                        ┌──────────▼──────────┐
                        │   Policy Decision   │
                        │ allow / deny /      │
                        │ transform           │
                        └──────────┬──────────┘
                                   │
                    ┌──────────────┴──────────────┐
                    │                             │
           ┌────────▼────────┐          ┌────────▼────────┐
           │     DENY        │          │     ALLOW       │
           │                 │          │                 │
           │ Return 403      │          │ Continue to     │
           │ Log audit       │          │ handler         │
           └─────────────────┘          └────────┬────────┘
                                                 │
                                      ┌──────────▼──────────┐
                                      │   Config Handler    │
                                      │   (Get config)      │
                                      └──────────┬──────────┘
                                                 │
                                      ┌──────────▼──────────┐
                                      │   Schema Validator  │
                                      │   (Validate config) │
                                      └──────────┬──────────┘
                                                 │
                                      ┌──────────▼──────────┐
                                      │  Response Transform │
                                      │  (Redact/Mask)      │
                                      └──────────┬──────────┘
                                                 │
                                      ┌──────────▼──────────┐
                                      │   HTTP Response     │
                                      │   (Transformed)     │
                                      └─────────────────────┘
```

---

## Flujo de Evaluacion de Politicas

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Policy Evaluation Flow                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Input: RequestContext { app, profile, label, ip, headers, ... }    │
│                                                                      │
│  1. Load policies (cached)                                          │
│     policies: [P1(pri:100), P2(pri:200), P3(pri:50)]               │
│                                                                      │
│  2. Sort by priority descending                                     │
│     sorted: [P2(200), P1(100), P3(50)]                             │
│                                                                      │
│  3. Evaluate each policy:                                           │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │  P2: conditions = [app matches "admin-*"]                │    │
│     │      context.app = "myapp"                               │    │
│     │      Result: NO MATCH                                    │    │
│     └─────────────────────────────────────────────────────────┘    │
│                          │                                          │
│                          ▼                                          │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │  P1: conditions = [profile == "prod",                    │    │
│     │                    property matches "*.password"]        │    │
│     │      context.profile = "prod"                            │    │
│     │      Result: MATCH                                       │    │
│     │      Action: mask { char: "*", visible: 4 }              │    │
│     │      Terminal: NO -> accumulate action, continue         │    │
│     └─────────────────────────────────────────────────────────┘    │
│                          │                                          │
│                          ▼                                          │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │  P3: conditions = [source_ip not_in "10.0.0.0/8"]       │    │
│     │      context.ip = "10.0.0.5"                             │    │
│     │      Result: NO MATCH                                    │    │
│     └─────────────────────────────────────────────────────────┘    │
│                                                                      │
│  4. Accumulated actions: [mask { char: "*", visible: 4 }]          │
│                                                                      │
│  5. Return PolicyDecision::Allow { actions: [...] }                 │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Changelog

| Version | Fecha | Cambios |
|---------|-------|---------|
| 0.1.0 | 2025-01-XX | Creacion inicial de la epica |

---

## Referencias

- [JSON Schema Specification](https://json-schema.org/specification.html)
- [Open Policy Agent](https://www.openpolicyagent.org/) (inspiracion para PLAC)
- [AWS IAM Policies](https://docs.aws.amazon.com/IAM/latest/UserGuide/access_policies.html)
- [Spring Security](https://spring.io/projects/spring-security) (patrones de autorizacion)
- [Tower Middleware](https://docs.rs/tower/latest/tower/)
- [Axum Middleware](https://docs.rs/axum/latest/axum/middleware/index.html)
