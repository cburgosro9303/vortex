# Epica 02: Core Types y Serializacion

## Objetivo

Implementar los tipos fundamentales de Vortex Config con soporte completo de serializacion para JSON, YAML y Properties. Esta epica establece la base del modelo de datos que sera utilizado por todo el sistema, siguiendo patrones idiomaticos de Rust y garantizando compatibilidad con Spring Cloud Config.

## Contexto

Vortex Config necesita representar configuraciones de aplicaciones de forma eficiente y flexible. Los tipos core deben:

- Almacenar pares clave-valor con valores anidados
- Soportar multiples formatos de serializacion
- Permitir merge de configuraciones (cascading)
- Ser compatibles con el formato de respuesta de Spring Cloud Config

## Conceptos de Rust Cubiertos

### Nivel Basico
| Concepto | Historia | Descripcion |
|----------|----------|-------------|
| Ownership | 001, 002 | Como Rust gestiona la memoria sin garbage collector |
| Borrowing (&, &mut) | 001, 002 | Referencias inmutables y mutables |
| Result<T, E> | 001-005 | Manejo explicito de errores |
| Option<T> | 002, 003 | Valores opcionales sin null |
| Derive macros | 001-004 | Generacion automatica de traits |
| HashMap | 001, 002 | Colecciones clave-valor |

### Nivel Intermedio
| Concepto | Historia | Descripcion |
|----------|----------|-------------|
| Serde | 001-004 | Framework de serializacion |
| Serde attributes | 003, 004 | Personalizacion de serializacion |
| From/Into traits | 004 | Conversiones entre tipos |
| TryFrom/TryInto | 004 | Conversiones que pueden fallar |
| Lifetimes basicos | 002, 003 | Anotaciones de tiempo de vida |
| Iterators | 002 | Procesamiento de colecciones |

## Historias de Usuario

| # | Titulo | Complejidad | Conceptos Clave |
|---|--------|-------------|-----------------|
| 001 | [ConfigMap con Serde](./story-001-configmap-serde.md) | Media | serde, derive macros, HashMap, ownership |
| 002 | [PropertySource y Merging](./story-002-property-source.md) | Media | borrowing, iterators, Option |
| 003 | [Formatos de Respuesta Spring](./story-003-spring-format.md) | Media | serde attributes, custom serialization |
| 004 | [Conversion entre Formatos](./story-004-format-conversion.md) | Alta | From/Into, TryFrom, error handling |
| 005 | [Unit Testing de Tipos Core](./story-005-core-testing.md) | Baja | #[cfg(test)], assert_eq!, test organization |

## Dependencias

### Epicas Requeridas
- **Epica 01 - Foundation**: Workspace configurado, toolchain instalado, CI basico

### Crates Externos
```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
java-properties = "2.0"
thiserror = "1.0"
indexmap = { version = "2.0", features = ["serde"] }
```

## Criterios de Aceptacion

### Funcionales
- [ ] `ConfigMap` puede almacenar valores anidados de cualquier profundidad
- [ ] Serializacion JSON produce output identico a Spring Cloud Config
- [ ] Serializacion YAML preserva estructura y comentarios
- [ ] Serializacion Properties genera formato `key.nested=value`
- [ ] `PropertySource` soporta merge con estrategia cascading
- [ ] Conversion bidireccional entre los tres formatos sin perdida de datos

### No Funcionales
- [ ] Parsing de 10,000 propiedades en < 10ms
- [ ] Memoria por ConfigMap proporcional al tamano de datos
- [ ] Zero-copy parsing donde sea posible

## Definition of Done

### Codigo
- [ ] Crate `vortex-core` compilado sin warnings
- [ ] `cargo fmt` aplicado
- [ ] `cargo clippy -- -D warnings` pasa
- [ ] Sin `unwrap()` en codigo de produccion
- [ ] Errores tipados con `thiserror`

### Tests
- [ ] Cobertura > 80% en tipos core
- [ ] Tests de serializacion para cada formato
- [ ] Tests de round-trip (serialize -> deserialize -> serialize)
- [ ] Tests de edge cases (unicode, caracteres especiales, valores vacios)

### Documentacion
- [ ] Rustdoc para todas las estructuras publicas
- [ ] Ejemplos de uso en documentation comments
- [ ] Changelog actualizado

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigacion |
|--------|--------------|---------|------------|
| Incompatibilidad con formato Spring | Media | Alto | Tests de compatibilidad con responses reales de Spring Cloud Config |
| Performance de parsing Properties | Baja | Medio | Benchmark temprano, optimizar si es necesario |
| Perdida de precision en conversiones | Media | Alto | Tests de round-trip exhaustivos |
| Orden de propiedades no preservado | Alta | Bajo | Usar IndexMap en lugar de HashMap |

## ADRs Sugeridos

1. **ADR-002: Representacion interna de valores**
   - Contexto: Elegir entre `serde_json::Value`, tipo propio, o hibrido
   - Decision sugerida: Tipo propio `ConfigValue` envolviendo `serde_json::Value`

2. **ADR-003: Estrategia de merge de configuraciones**
   - Contexto: Deep merge vs shallow merge vs override
   - Decision sugerida: Deep merge por defecto, configurable

3. **ADR-004: Manejo de tipos numericos**
   - Contexto: Properties solo tiene strings, JSON tiene tipos
   - Decision sugerida: Preservar tipos cuando el formato lo soporte

## Reglas Estrictas

1. **No usar `panic!` ni `unwrap()` en codigo de produccion**
   - Usar `Result<T, E>` para operaciones que pueden fallar
   - Usar `Option<T>` para valores opcionales
   - Usar `expect("mensaje descriptivo")` solo en tests

2. **Ownership explicito en APIs publicas**
   - Preferir `&str` sobre `String` en parametros cuando sea posible
   - Documentar cuando una funcion toma ownership vs borrow

3. **Serializacion determinista**
   - El mismo input debe producir el mismo output siempre
   - Usar IndexMap para preservar orden de claves

4. **Compatibilidad Spring Cloud Config**
   - Seguir exactamente el formato de respuesta JSON de Spring
   - Mantener tests de compatibilidad actualizados

## Estructura de Archivos Esperada

```
crates/vortex-core/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Re-exports publicos
│   ├── error.rs            # CoreError y tipos de error
│   ├── config/
│   │   ├── mod.rs
│   │   ├── map.rs          # ConfigMap
│   │   ├── value.rs        # ConfigValue
│   │   └── source.rs       # PropertySource
│   ├── format/
│   │   ├── mod.rs
│   │   ├── json.rs         # Serializacion JSON
│   │   ├── yaml.rs         # Serializacion YAML
│   │   ├── properties.rs   # Serializacion Properties
│   │   └── spring.rs       # Formato Spring Cloud Config
│   └── merge/
│       ├── mod.rs
│       └── strategy.rs     # Estrategias de merge
└── tests/
    ├── serialization_tests.rs
    ├── merge_tests.rs
    └── compatibility_tests.rs
```

## Changelog

| Fecha | Version | Cambios |
|-------|---------|---------|
| - | - | - |

---

**Siguiente**: [Historia 001 - ConfigMap con Serde](./story-001-configmap-serde.md)
