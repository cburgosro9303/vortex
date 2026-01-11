# Historia 001: ConfigMap con Serde

## Contexto y Objetivo

ConfigMap es la estructura de datos fundamental de Vortex Config. Representa un conjunto de propiedades de configuracion con soporte para valores anidados, similar a un documento JSON/YAML. Esta historia implementa ConfigMap con serializacion automatica usando Serde, el framework de serializacion estandar en Rust.

Para un desarrollador Java, ConfigMap es conceptualmente similar a `Map<String, Object>` con anidamiento, pero con tipado estatico y sin necesidad de casting.

## Alcance

### In Scope
- Definicion de `ConfigMap` y `ConfigValue`
- Derive de `Serialize` y `Deserialize`
- Operaciones basicas: get, insert, contains_key
- Acceso a propiedades anidadas con dot notation (`database.pool.size`)
- Serializacion a JSON y YAML

### Out of Scope
- Serializacion a .properties (historia 004)
- Merge de configuraciones (historia 002)
- Formato especifico de Spring Cloud Config (historia 003)
- Validacion de schemas

## Criterios de Aceptacion

- [ ] `ConfigMap` puede crearse vacio o desde un JSON string
- [ ] `ConfigValue` soporta: null, bool, number (i64/f64), string, array, object
- [ ] `get("database.pool.size")` navega estructuras anidadas
- [ ] Serializacion a JSON produce output valido y parseable
- [ ] Serializacion a YAML produce output valido y parseable
- [ ] Clone y Debug implementados para debugging
- [ ] PartialEq implementado para comparaciones en tests

## Diseno Propuesto

### Modulos/Crates Implicados
- `vortex-core/src/config/map.rs` - ConfigMap
- `vortex-core/src/config/value.rs` - ConfigValue

### Interfaces

```rust
/// Representa un mapa de configuracion con valores anidados
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigMap {
    #[serde(flatten)]
    inner: IndexMap<String, ConfigValue>,
}

/// Valor de configuracion que puede ser de varios tipos
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<ConfigValue>),
    Object(IndexMap<String, ConfigValue>),
}
```

### Estructura Sugerida

```
crates/vortex-core/src/config/
├── mod.rs          # pub mod map; pub mod value;
├── map.rs          # ConfigMap implementation
└── value.rs        # ConfigValue implementation
```

## Pasos de Implementacion

1. **Crear estructura de directorios**
   - Crear `crates/vortex-core/src/config/` directory
   - Crear archivos `mod.rs`, `map.rs`, `value.rs`

2. **Implementar ConfigValue**
   - Definir enum con variantes para cada tipo
   - Derivar Serialize, Deserialize con `#[serde(untagged)]`
   - Implementar metodos helper: `is_null()`, `as_str()`, `as_i64()`, etc.

3. **Implementar ConfigMap**
   - Definir struct con IndexMap interno
   - Implementar `new()`, `from_json()`, `from_yaml()`
   - Implementar `get()` con soporte para dot notation
   - Implementar `insert()`, `contains_key()`, `keys()`, `len()`

4. **Implementar acceso anidado**
   - Parsear path con split por '.'
   - Navegar recursivamente a traves de Objects

5. **Agregar tests unitarios**

## Conceptos de Rust Aprendidos

### Ownership y Structs

En Rust, cada valor tiene un unico dueno (owner). Cuando el dueno sale del scope, el valor se libera automaticamente. Esto reemplaza el garbage collector de Java.

```rust
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// ConfigMap posee (owns) su IndexMap interno.
/// Cuando ConfigMap se destruye, el IndexMap tambien se libera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigMap {
    #[serde(flatten)]
    inner: IndexMap<String, ConfigValue>,
}

impl ConfigMap {
    /// Crea un ConfigMap vacio
    pub fn new() -> Self {
        ConfigMap {
            inner: IndexMap::new(),
        }
    }

    /// Inserta un valor. `key` y `value` transfieren ownership a ConfigMap.
    /// Despues de llamar insert(), no puedes usar key ni value originales.
    pub fn insert(&mut self, key: String, value: ConfigValue) {
        self.inner.insert(key, value);
    }

    /// Retorna referencia al valor. No transfiere ownership.
    /// El caller puede leer pero no modificar ni tomar ownership.
    pub fn get(&self, key: &str) -> Option<&ConfigValue> {
        // Para keys simples, busqueda directa
        if !key.contains('.') {
            return self.inner.get(key);
        }

        // Para keys anidadas, navegacion recursiva
        let parts: Vec<&str> = key.splitn(2, '.').collect();
        match self.inner.get(parts[0]) {
            Some(ConfigValue::Object(map)) => {
                Self::get_nested(map, parts.get(1).copied())
            }
            _ => None,
        }
    }

    fn get_nested<'a>(
        map: &'a IndexMap<String, ConfigValue>,
        remaining: Option<&str>,
    ) -> Option<&'a ConfigValue> {
        match remaining {
            None => None,
            Some(key) if !key.contains('.') => map.get(key),
            Some(key) => {
                let parts: Vec<&str> = key.splitn(2, '.').collect();
                match map.get(parts[0]) {
                    Some(ConfigValue::Object(nested)) => {
                        Self::get_nested(nested, parts.get(1).copied())
                    }
                    other => {
                        if parts.len() == 1 {
                            other
                        } else {
                            None
                        }
                    }
                }
            }
        }
    }
}
```

**Comparacion con Java:**

| Rust | Java |
|------|------|
| `String` (owned) | `String` (reference counted by GC) |
| `&str` (borrowed) | No hay equivalente directo |
| Move semantics | Copia de referencia |
| Drop automatico | Garbage collection |

### Enums con Datos (Sum Types)

Los enums de Rust pueden contener datos, similar a sealed classes + records en Java 17+. Esto permite modelar valores que pueden ser de diferentes tipos de forma type-safe.

```rust
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// ConfigValue modela un valor que puede ser de varios tipos.
/// Similar a un "union type" pero completamente type-safe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]  // Serde infiere el tipo del JSON sin tag explicito
pub enum ConfigValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<ConfigValue>),
    Object(IndexMap<String, ConfigValue>),
}

impl ConfigValue {
    /// Verifica si el valor es null
    pub fn is_null(&self) -> bool {
        matches!(self, ConfigValue::Null)
    }

    /// Intenta obtener como string. Retorna None si no es string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ConfigValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Intenta obtener como i64
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            ConfigValue::Integer(n) => Some(*n),
            _ => None,
        }
    }

    /// Intenta obtener como bool
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ConfigValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Intenta obtener como object (mapa)
    pub fn as_object(&self) -> Option<&IndexMap<String, ConfigValue>> {
        match self {
            ConfigValue::Object(map) => Some(map),
            _ => None,
        }
    }
}
```

**Comparacion con Java:**

```java
// Java 17+ con sealed classes
public sealed interface ConfigValue permits
    NullValue, BoolValue, IntValue, FloatValue,
    StringValue, ArrayValue, ObjectValue {
}

public record StringValue(String value) implements ConfigValue {}
public record IntValue(long value) implements ConfigValue {}
// ... etc

// Uso con pattern matching (Java 21+)
String result = switch (value) {
    case StringValue(String s) -> s;
    case IntValue(long n) -> String.valueOf(n);
    default -> null;
};
```

### Serde y Derive Macros

Serde es el framework de serializacion estandar de Rust. Usando derive macros, genera automaticamente codigo de serializacion/deserializacion en tiempo de compilacion.

```rust
use serde::{Deserialize, Serialize};
use serde_json;
use serde_yaml;

// #[derive] genera implementaciones automaticas de traits.
// Similar a Lombok @Data pero en tiempo de compilacion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigMap {
    // #[serde(flatten)] "aplana" el mapa en el JSON padre
    // En lugar de {"inner": {"key": "value"}} produce {"key": "value"}
    #[serde(flatten)]
    inner: IndexMap<String, ConfigValue>,
}

impl ConfigMap {
    /// Parsea JSON string a ConfigMap
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serializa ConfigMap a JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Parsea YAML string a ConfigMap
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// Serializa ConfigMap a YAML string
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }
}

// Ejemplo de uso
fn example() -> Result<(), Box<dyn std::error::Error>> {
    let json = r#"{"database": {"host": "localhost", "port": 5432}}"#;

    let config = ConfigMap::from_json(json)?;

    // Acceso con dot notation
    if let Some(host) = config.get("database.host") {
        println!("Host: {:?}", host);
    }

    // Serializacion a YAML
    let yaml = config.to_yaml()?;
    println!("YAML:\n{}", yaml);

    Ok(())
}
```

**Comparacion con Java (Jackson):**

```java
// Java con Jackson
@JsonInclude(JsonInclude.Include.NON_NULL)
public class ConfigMap {
    private Map<String, Object> properties;

    // Jackson requiere getters/setters o @JsonProperty
    @JsonAnyGetter
    public Map<String, Object> getProperties() {
        return properties;
    }
}

// Uso
ObjectMapper mapper = new ObjectMapper();
ConfigMap config = mapper.readValue(json, ConfigMap.class);
String yaml = new YAMLMapper().writeValueAsString(config);
```

### HashMap vs IndexMap

IndexMap mantiene el orden de insercion, crucial para output determinista.

```rust
use indexmap::IndexMap;
use std::collections::HashMap;

fn compare_maps() {
    // HashMap: orden NO garantizado (similar a Java HashMap)
    let mut hash_map: HashMap<String, i32> = HashMap::new();
    hash_map.insert("z".into(), 1);
    hash_map.insert("a".into(), 2);
    hash_map.insert("m".into(), 3);
    // Iteracion puede dar cualquier orden

    // IndexMap: preserva orden de insercion (similar a Java LinkedHashMap)
    let mut index_map: IndexMap<String, i32> = IndexMap::new();
    index_map.insert("z".into(), 1);
    index_map.insert("a".into(), 2);
    index_map.insert("m".into(), 3);
    // Iteracion siempre da: z, a, m
}
```

## Riesgos y Errores Comunes

### 1. Olvidar Clone al pasar valores

```rust
// ERROR: value movido, no puedes usarlo despues
let value = ConfigValue::String("hello".to_string());
map.insert("key".to_string(), value);
// println!("{:?}", value);  // Error de compilacion!

// CORRECTO: clonar si necesitas mantener el original
let value = ConfigValue::String("hello".to_string());
map.insert("key".to_string(), value.clone());
println!("{:?}", value);  // OK
```

### 2. Deserializacion con tipos incorrectos

```rust
// El JSON tiene un numero, pero esperamos string
let json = r#"{"port": 5432}"#;
let config: ConfigMap = serde_json::from_str(json)?;

// Esto retorna None porque 5432 es Integer, no String
let port: Option<&str> = config.get("port").and_then(|v| v.as_str());
assert!(port.is_none());

// CORRECTO: usar as_i64() para numeros
let port: Option<i64> = config.get("port").and_then(|v| v.as_i64());
assert_eq!(port, Some(5432));
```

### 3. Mutabilidad explicita

```rust
// ERROR: config no es mutable
let config = ConfigMap::new();
// config.insert("key".to_string(), value);  // Error!

// CORRECTO: declarar como mutable
let mut config = ConfigMap::new();
config.insert("key".to_string(), value);  // OK
```

## Pruebas

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_configmap_new() {
        let config = ConfigMap::new();
        assert_eq!(config.len(), 0);
        assert!(config.is_empty());
    }

    #[test]
    fn test_configmap_insert_and_get() {
        let mut config = ConfigMap::new();
        config.insert(
            "name".to_string(),
            ConfigValue::String("test".to_string()),
        );

        let value = config.get("name");
        assert!(value.is_some());
        assert_eq!(value.unwrap().as_str(), Some("test"));
    }

    #[test]
    fn test_nested_access() {
        let json = r#"{"database": {"host": "localhost", "port": 5432}}"#;
        let config = ConfigMap::from_json(json).unwrap();

        assert_eq!(
            config.get("database.host").unwrap().as_str(),
            Some("localhost")
        );
        assert_eq!(
            config.get("database.port").unwrap().as_i64(),
            Some(5432)
        );
    }

    #[test]
    fn test_json_roundtrip() {
        let json = r#"{"key": "value", "nested": {"inner": true}}"#;
        let config = ConfigMap::from_json(json).unwrap();
        let serialized = config.to_json().unwrap();
        let reparsed = ConfigMap::from_json(&serialized).unwrap();

        assert_eq!(config, reparsed);
    }

    #[test]
    fn test_yaml_serialization() {
        let mut config = ConfigMap::new();
        config.insert("key".to_string(), ConfigValue::String("value".to_string()));

        let yaml = config.to_yaml().unwrap();
        assert!(yaml.contains("key:"));
        assert!(yaml.contains("value"));
    }
}
```

### Integration Tests

```rust
// tests/serialization_tests.rs
use vortex_core::config::{ConfigMap, ConfigValue};

#[test]
fn test_complex_nested_structure() {
    let json = r#"{
        "spring": {
            "datasource": {
                "url": "jdbc:postgresql://localhost/db",
                "username": "user",
                "password": "secret",
                "hikari": {
                    "maximum-pool-size": 10,
                    "minimum-idle": 5
                }
            }
        }
    }"#;

    let config = ConfigMap::from_json(json).unwrap();

    assert_eq!(
        config.get("spring.datasource.url").unwrap().as_str(),
        Some("jdbc:postgresql://localhost/db")
    );
    assert_eq!(
        config.get("spring.datasource.hikari.maximum-pool-size")
            .unwrap()
            .as_i64(),
        Some(10)
    );
}

#[test]
fn test_special_characters_in_values() {
    let json = r#"{"message": "Hello, \"World\"!\nNew line"}"#;
    let config = ConfigMap::from_json(json).unwrap();

    let yaml = config.to_yaml().unwrap();
    let reparsed = ConfigMap::from_yaml(&yaml).unwrap();

    assert_eq!(config, reparsed);
}
```

## Entregable Final

- PR con:
  - `crates/vortex-core/src/config/mod.rs`
  - `crates/vortex-core/src/config/map.rs`
  - `crates/vortex-core/src/config/value.rs`
  - Tests unitarios con cobertura > 80%
  - Rustdoc para todas las estructuras y metodos publicos
  - Ejemplo de uso en documentation comments

---

**Anterior**: [Indice de Epica 02](./index.md)
**Siguiente**: [Historia 002 - PropertySource y Merging](./story-002-property-source.md)
