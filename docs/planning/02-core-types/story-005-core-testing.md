# Historia 005: Unit Testing de Tipos Core

## Contexto y Objetivo

Una suite de tests solida es esencial para mantener la calidad del codigo y facilitar refactoring futuro. Esta historia establece la estrategia de testing para los tipos core, incluyendo tests unitarios, tests de integracion, y tests de propiedades (property-based testing).

Para un desarrollador Java, el testing en Rust es similar conceptualmente pero con diferencias importantes en organizacion y convenciones.

## Alcance

### In Scope
- Tests unitarios para ConfigMap, ConfigValue, PropertySource
- Tests de serializacion para todos los formatos
- Tests de merge y cascading
- Tests de edge cases y error handling
- Organizacion de tests (modulos, archivos)
- Fixtures y helpers de test
- Documentacion de tests como ejemplos

### Out of Scope
- Tests de integracion con sistemas externos
- Benchmarks de performance (historia futura)
- Fuzzing tests
- Tests de compatibilidad con Spring Boot client

## Criterios de Aceptacion

- [ ] Cobertura de tests > 80% en vortex-core
- [ ] Todos los tests pasan con `cargo test`
- [ ] Tests de errores verifican mensajes descriptivos
- [ ] Tests documentados como ejemplos en rustdoc
- [ ] Edge cases cubiertos: unicode, valores vacios, estructuras profundas
- [ ] Tests organizados por modulo y funcionalidad

## Diseno Propuesto

### Modulos/Crates Implicados
- `vortex-core/src/*` - Tests inline con `#[cfg(test)]`
- `vortex-core/tests/` - Tests de integracion

### Estructura de Tests

```
crates/vortex-core/
├── src/
│   ├── config/
│   │   ├── map.rs          # Tests inline para ConfigMap
│   │   ├── value.rs        # Tests inline para ConfigValue
│   │   └── source.rs       # Tests inline para PropertySource
│   └── format/
│       ├── json.rs         # Tests inline
│       ├── yaml.rs         # Tests inline
│       └── properties.rs   # Tests inline
└── tests/
    ├── common/
    │   └── mod.rs          # Fixtures y helpers compartidos
    ├── serialization_tests.rs
    ├── merge_tests.rs
    ├── roundtrip_tests.rs
    └── edge_cases_tests.rs
```

## Pasos de Implementacion

1. **Establecer helpers de test**
   - Crear modulo `tests/common/mod.rs`
   - Funciones factory para crear fixtures

2. **Tests unitarios inline**
   - Agregar modulo `#[cfg(test)]` en cada archivo
   - Cubrir casos basicos y edge cases

3. **Tests de integracion**
   - Tests que cruzan multiples modulos
   - Escenarios end-to-end de conversion

4. **Documentacion con ejemplos**
   - Agregar doc tests en rustdoc

5. **Verificar cobertura**
   - Ejecutar `cargo llvm-cov`

## Conceptos de Rust Aprendidos

### Modulo de Tests con #[cfg(test)]

En Rust, los tests unitarios se colocan junto al codigo que prueban usando el atributo `#[cfg(test)]`. Este codigo solo se compila cuando se ejecutan tests.

```rust
// src/config/value.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<ConfigValue>),
    Object(indexmap::IndexMap<String, ConfigValue>),
}

impl ConfigValue {
    pub fn is_null(&self) -> bool {
        matches!(self, ConfigValue::Null)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            ConfigValue::String(s) => Some(s),
            _ => None,
        }
    }
}

// ===== MODULO DE TESTS =====
// Solo se compila con `cargo test`
#[cfg(test)]
mod tests {
    use super::*;  // Importa todo del modulo padre

    // Cada funcion con #[test] es un test case
    #[test]
    fn test_is_null() {
        assert!(ConfigValue::Null.is_null());
        assert!(!ConfigValue::Bool(true).is_null());
        assert!(!ConfigValue::String("".into()).is_null());
    }

    #[test]
    fn test_as_str_success() {
        let value = ConfigValue::String("hello".into());
        assert_eq!(value.as_str(), Some("hello"));
    }

    #[test]
    fn test_as_str_failure() {
        let value = ConfigValue::Integer(42);
        assert_eq!(value.as_str(), None);
    }

    // Test que verifica un panic
    #[test]
    #[should_panic(expected = "index out of bounds")]
    fn test_array_out_of_bounds() {
        let arr: Vec<i32> = vec![1, 2, 3];
        let _ = arr[10];  // Panic!
    }

    // Test que retorna Result - falla si retorna Err
    #[test]
    fn test_json_parsing() -> Result<(), serde_json::Error> {
        let json = r#"{"key": "value"}"#;
        let _value: ConfigValue = serde_json::from_str(json)?;
        Ok(())
    }
}
```

**Comparacion con Java/JUnit:**

| Rust | Java/JUnit |
|------|------------|
| `#[test]` | `@Test` |
| `#[should_panic]` | `assertThrows()` |
| `#[ignore]` | `@Disabled` |
| `assert!()` | `assertTrue()` |
| `assert_eq!()` | `assertEquals()` |
| `assert_ne!()` | `assertNotEquals()` |

### Asserts y Macros de Test

Rust proporciona macros poderosas para assertions con mensajes de error descriptivos.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assert_basics() {
        // assert! para condiciones booleanas
        assert!(true);
        assert!(1 + 1 == 2);

        // assert_eq! compara igualdad con mensaje automatico
        let expected = 42;
        let actual = 40 + 2;
        assert_eq!(actual, expected);  // Muestra ambos valores si falla

        // assert_ne! verifica desigualdad
        assert_ne!(1, 2);
    }

    #[test]
    fn test_assert_with_message() {
        let config = ConfigMap::new();

        // Mensaje personalizado si falla
        assert!(
            config.is_empty(),
            "Expected empty config, but had {} items",
            config.len()
        );

        // assert_eq! con mensaje
        assert_eq!(
            config.len(),
            0,
            "Config should be empty after creation"
        );
    }

    #[test]
    fn test_debug_assertion() {
        // debug_assert! solo se ejecuta en debug builds
        // Util para checks costosos que no quieres en produccion
        debug_assert!(expensive_check());
    }

    #[test]
    fn test_option_assertions() {
        let some_value: Option<i32> = Some(42);
        let none_value: Option<i32> = None;

        // Verificar Some/None
        assert!(some_value.is_some());
        assert!(none_value.is_none());

        // Unwrap en tests es OK (panic si falla = test failure)
        assert_eq!(some_value.unwrap(), 42);
    }

    #[test]
    fn test_result_assertions() {
        let ok_result: Result<i32, &str> = Ok(42);
        let err_result: Result<i32, &str> = Err("failed");

        assert!(ok_result.is_ok());
        assert!(err_result.is_err());

        // Verificar contenido de error
        assert_eq!(err_result.unwrap_err(), "failed");
    }
}
```

### Organizacion de Tests de Integracion

Los tests en `tests/` son tests de integracion que prueban el crate como si fuera un usuario externo.

```rust
// tests/common/mod.rs - Helpers compartidos

use vortex_core::config::{ConfigMap, ConfigValue, PropertySource};

/// Crea ConfigMap desde JSON para tests
pub fn config_from_json(json: &str) -> ConfigMap {
    ConfigMap::from_json(json).expect("Invalid JSON in test fixture")
}

/// Crea PropertySource de test
pub fn make_source(name: &str, priority: i32, json: &str) -> PropertySource {
    PropertySource {
        name: name.to_string(),
        origin: format!("test:{}", name),
        priority,
        config: config_from_json(json),
    }
}

/// Fixtures de configuracion comunes
pub mod fixtures {
    use super::*;

    pub fn simple_config() -> ConfigMap {
        config_from_json(r#"{"key": "value", "number": 42}"#)
    }

    pub fn nested_config() -> ConfigMap {
        config_from_json(r#"{
            "database": {
                "host": "localhost",
                "port": 5432,
                "credentials": {
                    "username": "admin",
                    "password": "secret"
                }
            }
        }"#)
    }

    pub fn spring_like_config() -> ConfigMap {
        config_from_json(r#"{
            "spring": {
                "datasource": {
                    "url": "jdbc:postgresql://localhost/db"
                },
                "jpa": {
                    "show-sql": true
                }
            },
            "server": {
                "port": 8080
            }
        }"#)
    }
}
```

```rust
// tests/serialization_tests.rs

mod common;
use common::fixtures;
use vortex_core::config::ConfigMap;
use vortex_core::format::PropertiesSerializer;

#[test]
fn test_json_to_yaml_roundtrip() {
    let original = fixtures::nested_config();

    let yaml = original.to_yaml().expect("YAML serialization failed");
    let reparsed = ConfigMap::from_yaml(&yaml).expect("YAML parsing failed");

    assert_eq!(original, reparsed);
}

#[test]
fn test_complex_structure_to_properties() {
    let config = fixtures::spring_like_config();
    let props = PropertiesSerializer::serialize(&config);

    // Verificar que las claves estan aplanadas
    assert!(props.contains("spring.datasource.url="));
    assert!(props.contains("spring.jpa.show-sql="));
    assert!(props.contains("server.port="));
}

#[test]
fn test_all_formats_produce_equivalent_config() {
    let json = r#"{"a": 1, "b": {"c": "hello"}}"#;

    let from_json = ConfigMap::from_json(json).unwrap();
    let yaml = from_json.to_yaml().unwrap();
    let from_yaml = ConfigMap::from_yaml(&yaml).unwrap();

    assert_eq!(from_json, from_yaml, "JSON and YAML should produce equal configs");
}
```

### Doc Tests

Los ejemplos en documentacion se ejecutan como tests, garantizando que la documentacion este actualizada.

```rust
/// Representa un mapa de configuracion con valores anidados.
///
/// # Examples
///
/// ```
/// use vortex_core::config::ConfigMap;
///
/// // Crear desde JSON
/// let config = ConfigMap::from_json(r#"{"key": "value"}"#).unwrap();
/// assert_eq!(config.get("key").unwrap().as_str(), Some("value"));
/// ```
///
/// ## Acceso a valores anidados
///
/// ```
/// use vortex_core::config::ConfigMap;
///
/// let config = ConfigMap::from_json(r#"{
///     "database": {
///         "host": "localhost",
///         "port": 5432
///     }
/// }"#).unwrap();
///
/// // Dot notation para acceso anidado
/// assert_eq!(
///     config.get("database.host").unwrap().as_str(),
///     Some("localhost")
/// );
/// ```
///
/// ## Serialization
///
/// ```
/// use vortex_core::config::ConfigMap;
///
/// let mut config = ConfigMap::new();
/// config.insert("key".to_string(), "value".into());
///
/// let json = config.to_json().unwrap();
/// assert!(json.contains("key"));
/// ```
pub struct ConfigMap {
    // ...
}
```

### Tests con Datos Parametrizados

Rust no tiene anotaciones como `@ParameterizedTest` de JUnit, pero se pueden usar macros o loops.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Macro para generar tests parametrizados
    macro_rules! format_test {
        ($name:ident, $input:expr, $expected:expr) => {
            #[test]
            fn $name() {
                let config = ConfigMap::from_json($input).unwrap();
                let actual = config.get("key").unwrap().as_str();
                assert_eq!(actual, Some($expected));
            }
        };
    }

    format_test!(test_simple_string, r#"{"key": "hello"}"#, "hello");
    format_test!(test_string_with_spaces, r#"{"key": "hello world"}"#, "hello world");
    format_test!(test_empty_string, r#"{"key": ""}"#, "");

    // Alternativa: loop de test cases
    #[test]
    fn test_numeric_conversions() {
        let test_cases = vec![
            (r#"{"n": 0}"#, 0i64),
            (r#"{"n": 42}"#, 42i64),
            (r#"{"n": -1}"#, -1i64),
            (r#"{"n": 9223372036854775807}"#, i64::MAX),
        ];

        for (json, expected) in test_cases {
            let config = ConfigMap::from_json(json).unwrap();
            let actual = config.get("n").unwrap().as_i64().unwrap();
            assert_eq!(
                actual, expected,
                "Failed for input: {}",
                json
            );
        }
    }

    // Tests con matrices de datos
    #[test]
    fn test_boolean_parsing() {
        let truthy = vec![
            ("true", true),
            ("false", false),
        ];

        for (input, expected) in truthy {
            let json = format!(r#"{{"flag": {}}}"#, input);
            let config = ConfigMap::from_json(&json).unwrap();
            assert_eq!(
                config.get("flag").unwrap().as_bool(),
                Some(expected),
                "Failed for: {}",
                input
            );
        }
    }
}
```

### Tests de Edge Cases

```rust
#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_unicode_values() {
        let json = r#"{"emoji": "testing", "spanish": "Hola mundo!"}"#;
        let config = ConfigMap::from_json(json).unwrap();

        let yaml = config.to_yaml().unwrap();
        let reparsed = ConfigMap::from_yaml(&yaml).unwrap();

        assert_eq!(config, reparsed);
    }

    #[test]
    fn test_deeply_nested_structure() {
        let json = r#"{
            "a": {"b": {"c": {"d": {"e": {"f": "deep"}}}}}
        }"#;
        let config = ConfigMap::from_json(json).unwrap();

        assert_eq!(
            config.get("a.b.c.d.e.f").unwrap().as_str(),
            Some("deep")
        );
    }

    #[test]
    fn test_empty_values() {
        let json = r#"{
            "empty_string": "",
            "empty_array": [],
            "empty_object": {},
            "null_value": null
        }"#;
        let config = ConfigMap::from_json(json).unwrap();

        assert_eq!(config.get("empty_string").unwrap().as_str(), Some(""));
        assert!(config.get("null_value").unwrap().is_null());
    }

    #[test]
    fn test_special_characters_in_keys() {
        let json = r#"{
            "key-with-dashes": 1,
            "key_with_underscores": 2,
            "key.with.dots": 3
        }"#;
        let config = ConfigMap::from_json(json).unwrap();

        assert!(config.get("key-with-dashes").is_some());
        assert!(config.get("key_with_underscores").is_some());
        // Nota: "key.with.dots" es una clave, no acceso anidado
    }

    #[test]
    fn test_very_large_numbers() {
        let json = r#"{"big": 9223372036854775807, "small": -9223372036854775808}"#;
        let config = ConfigMap::from_json(json).unwrap();

        assert_eq!(config.get("big").unwrap().as_i64(), Some(i64::MAX));
        assert_eq!(config.get("small").unwrap().as_i64(), Some(i64::MIN));
    }

    #[test]
    fn test_floating_point_precision() {
        let json = r#"{"pi": 3.141592653589793}"#;
        let config = ConfigMap::from_json(json).unwrap();

        let yaml = config.to_yaml().unwrap();
        let reparsed = ConfigMap::from_yaml(&yaml).unwrap();

        // Verificar precision se mantiene
        let original = config.get("pi").unwrap();
        let after = reparsed.get("pi").unwrap();
        assert_eq!(original, after);
    }

    #[test]
    fn test_escape_sequences() {
        let json = r#"{"text": "line1\nline2\ttab"}"#;
        let config = ConfigMap::from_json(json).unwrap();

        let text = config.get("text").unwrap().as_str().unwrap();
        assert!(text.contains('\n'));
        assert!(text.contains('\t'));
    }
}
```

## Riesgos y Errores Comunes

### 1. Tests que Dependen del Orden

```rust
// MALO: tests dependen de orden de ejecucion
static mut COUNTER: i32 = 0;

#[test]
fn test_first() {
    unsafe { COUNTER = 1; }
}

#[test]
fn test_second() {
    unsafe { assert_eq!(COUNTER, 1); }  // Puede fallar!
}

// BUENO: cada test es independiente
#[test]
fn test_isolated() {
    let mut counter = 0;
    counter += 1;
    assert_eq!(counter, 1);
}
```

### 2. Unwrap en Tests sin Mensaje

```rust
// MALO: si falla, no sabes que paso
#[test]
fn test_bad() {
    let config = ConfigMap::from_json(complex_json).unwrap();
    let value = config.get("path.to.value").unwrap();
}

// BUENO: mensaje descriptivo si falla
#[test]
fn test_good() {
    let config = ConfigMap::from_json(complex_json)
        .expect("Failed to parse test JSON");
    let value = config.get("path.to.value")
        .expect("Expected path.to.value to exist");
}
```

### 3. Tests Fragiles por Orden de Claves

```rust
// MALO: depende del orden de serializacion
#[test]
fn test_fragile() {
    let config = ConfigMap::from_json(r#"{"b": 1, "a": 2}"#).unwrap();
    let json = config.to_json().unwrap();
    assert_eq!(json, r#"{"b": 1, "a": 2}"#);  // Puede fallar si orden cambia
}

// BUENO: compara estructuras, no strings
#[test]
fn test_robust() {
    let config = ConfigMap::from_json(r#"{"b": 1, "a": 2}"#).unwrap();
    let reparsed = ConfigMap::from_json(&config.to_json().unwrap()).unwrap();
    assert_eq!(config, reparsed);
}
```

## Pruebas

### Ejecutar Tests

```bash
# Todos los tests
cargo test

# Tests de un modulo especifico
cargo test config::map::tests

# Tests que matchean un patron
cargo test test_json

# Tests con output visible
cargo test -- --nocapture

# Tests en paralelo (default) o secuencial
cargo test -- --test-threads=1

# Solo doc tests
cargo test --doc
```

### Cobertura de Tests

```bash
# Instalar herramienta
cargo install cargo-llvm-cov

# Generar reporte
cargo llvm-cov --html

# Ver cobertura en terminal
cargo llvm-cov
```

## Entregable Final

- PR con:
  - Tests unitarios en todos los modulos de vortex-core
  - Tests de integracion en `tests/`
  - Modulo `tests/common/mod.rs` con helpers
  - Doc tests en estructuras publicas
  - Cobertura > 80% verificada con cargo-llvm-cov
  - CI configurado para ejecutar tests

---

**Anterior**: [Historia 004 - Conversion entre Formatos](./story-004-format-conversion.md)
**Siguiente**: [Epica 03 - HTTP Server con Axum](../03-http-server/index.md)
