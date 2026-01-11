# Historia 004: Conversion entre Formatos

## Contexto y Objetivo

Vortex Config debe soportar multiples formatos de configuracion: JSON, YAML y Properties. Los clientes pueden solicitar la configuracion en cualquier formato, independientemente de como este almacenada originalmente. Esto requiere conversion bidireccional entre formatos sin perdida de informacion (donde sea posible).

El formato `.properties` es particularmente importante porque es el formato nativo de aplicaciones Java/Spring, aunque tiene limitaciones (solo strings, sin anidamiento nativo).

## Alcance

### In Scope
- Conversion `ConfigMap` <-> JSON string
- Conversion `ConfigMap` <-> YAML string
- Conversion `ConfigMap` <-> Properties string
- Traits `From/Into` para conversiones infalibles
- Traits `TryFrom/TryInto` para conversiones que pueden fallar
- Manejo de caracteres especiales y escape sequences

### Out of Scope
- Preservacion de comentarios (se pierden en conversion)
- Conversion directa entre formatos sin pasar por ConfigMap
- Formatos adicionales (TOML, XML, etc.)

## Criterios de Aceptacion

- [ ] JSON -> ConfigMap -> JSON preserva estructura exacta
- [ ] YAML -> ConfigMap -> YAML produce YAML valido equivalente
- [ ] Properties -> ConfigMap parsea correctamente `key.nested=value`
- [ ] ConfigMap -> Properties produce formato aplanado correcto
- [ ] Caracteres Unicode manejados correctamente
- [ ] Escape sequences (\n, \t, etc.) preservados
- [ ] Errores de parsing retornan Result con mensaje descriptivo

## Diseno Propuesto

### Modulos/Crates Implicados
- `vortex-core/src/format/json.rs`
- `vortex-core/src/format/yaml.rs`
- `vortex-core/src/format/properties.rs`
- `vortex-core/src/error.rs`

### Interfaces

```rust
/// Formatos soportados
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Json,
    Yaml,
    Properties,
}

/// Resultado de conversion de formato
pub type FormatResult<T> = Result<T, FormatError>;

/// Errores de conversion de formato
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("YAML parse error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("Properties parse error at line {line}: {message}")]
    PropertiesError { line: usize, message: String },

    #[error("Invalid format: {0}")]
    InvalidFormat(String),
}

/// Trait para tipos que pueden convertirse a/desde ConfigMap
pub trait ConfigSerialize {
    fn to_json(&self) -> FormatResult<String>;
    fn to_yaml(&self) -> FormatResult<String>;
    fn to_properties(&self) -> FormatResult<String>;
}

pub trait ConfigDeserialize: Sized {
    fn from_json(json: &str) -> FormatResult<Self>;
    fn from_yaml(yaml: &str) -> FormatResult<Self>;
    fn from_properties(props: &str) -> FormatResult<Self>;
}
```

### Estructura Sugerida

```
crates/vortex-core/src/format/
├── mod.rs           # Re-exports, ConfigFormat enum
├── json.rs          # JSON conversion
├── yaml.rs          # YAML conversion
├── properties.rs    # Properties conversion
└── spring.rs        # Spring format (historia anterior)
```

## Pasos de Implementacion

1. **Definir tipos de error**
   - Crear `FormatError` enum con thiserror
   - Implementar `From` para errores de serde

2. **Implementar conversion JSON**
   - Ya existe basico en historia 001
   - Agregar manejo de errores mejorado

3. **Implementar conversion YAML**
   - Parsear YAML a ConfigMap
   - Serializar ConfigMap a YAML

4. **Implementar conversion Properties**
   - Parser manual para formato `key.nested=value`
   - Serializer que aplana estructura

5. **Implementar traits From/TryFrom**
   - Conversiones ergonomicas entre tipos

6. **Tests de round-trip**

## Conceptos de Rust Aprendidos

### From/Into Traits

`From` y `Into` son traits para conversiones infalibles entre tipos. Implementar `From` automaticamente da `Into` gratis.

```rust
use crate::config::ConfigValue;
use indexmap::IndexMap;

// Implementar From<T> for ConfigValue para tipos primitivos
// Permite conversiones ergonomicas como: ConfigValue::from(42)

impl From<bool> for ConfigValue {
    fn from(value: bool) -> Self {
        ConfigValue::Bool(value)
    }
}

impl From<i64> for ConfigValue {
    fn from(value: i64) -> Self {
        ConfigValue::Integer(value)
    }
}

impl From<i32> for ConfigValue {
    fn from(value: i32) -> Self {
        ConfigValue::Integer(value as i64)
    }
}

impl From<f64> for ConfigValue {
    fn from(value: f64) -> Self {
        ConfigValue::Float(value)
    }
}

impl From<String> for ConfigValue {
    fn from(value: String) -> Self {
        ConfigValue::String(value)
    }
}

// &str requiere conversion a String (owned)
impl From<&str> for ConfigValue {
    fn from(value: &str) -> Self {
        ConfigValue::String(value.to_string())
    }
}

impl<T: Into<ConfigValue>> From<Vec<T>> for ConfigValue {
    fn from(value: Vec<T>) -> Self {
        ConfigValue::Array(value.into_iter().map(Into::into).collect())
    }
}

// Ejemplo de uso
fn example() {
    // From::from explicito
    let v1: ConfigValue = ConfigValue::from(42);
    let v2: ConfigValue = ConfigValue::from("hello");

    // .into() usando Into (inferido del contexto)
    let v3: ConfigValue = 42.into();
    let v4: ConfigValue = "hello".into();

    // En funciones que esperan ConfigValue
    fn insert(value: impl Into<ConfigValue>) {
        let _v: ConfigValue = value.into();
    }

    insert(42);
    insert("string");
    insert(true);
}
```

**Comparacion con Java:**

```java
// Java: conversion explicita o constructores
ConfigValue fromInt = new ConfigValue.Integer(42);
ConfigValue fromStr = new ConfigValue.String("hello");

// O metodos factory
ConfigValue v1 = ConfigValue.of(42);
ConfigValue v2 = ConfigValue.of("hello");

// Rust: From/Into permiten conversion implicita por contexto
let v: ConfigValue = 42.into();
```

### TryFrom/TryInto para Conversiones Falibles

Cuando una conversion puede fallar, usamos `TryFrom` y `TryInto`.

```rust
use std::convert::TryFrom;
use crate::config::{ConfigMap, ConfigValue};

/// Error cuando la conversion falla
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("cannot convert {from_type} to {to_type}")]
    TypeMismatch {
        from_type: &'static str,
        to_type: &'static str,
    },

    #[error("value out of range: {0}")]
    OutOfRange(String),
}

// TryFrom ConfigValue a tipos primitivos
impl TryFrom<&ConfigValue> for String {
    type Error = ConversionError;

    fn try_from(value: &ConfigValue) -> Result<Self, Self::Error> {
        match value {
            ConfigValue::String(s) => Ok(s.clone()),
            ConfigValue::Integer(n) => Ok(n.to_string()),
            ConfigValue::Float(f) => Ok(f.to_string()),
            ConfigValue::Bool(b) => Ok(b.to_string()),
            _ => Err(ConversionError::TypeMismatch {
                from_type: "ConfigValue",
                to_type: "String",
            }),
        }
    }
}

impl TryFrom<&ConfigValue> for i64 {
    type Error = ConversionError;

    fn try_from(value: &ConfigValue) -> Result<Self, Self::Error> {
        match value {
            ConfigValue::Integer(n) => Ok(*n),
            ConfigValue::Float(f) => {
                if f.fract() == 0.0 && *f >= i64::MIN as f64 && *f <= i64::MAX as f64 {
                    Ok(*f as i64)
                } else {
                    Err(ConversionError::OutOfRange(format!("{} cannot be represented as i64", f)))
                }
            }
            ConfigValue::String(s) => s.parse().map_err(|_| ConversionError::TypeMismatch {
                from_type: "String",
                to_type: "i64",
            }),
            _ => Err(ConversionError::TypeMismatch {
                from_type: "ConfigValue",
                to_type: "i64",
            }),
        }
    }
}

impl TryFrom<&ConfigValue> for bool {
    type Error = ConversionError;

    fn try_from(value: &ConfigValue) -> Result<Self, Self::Error> {
        match value {
            ConfigValue::Bool(b) => Ok(*b),
            ConfigValue::String(s) => match s.to_lowercase().as_str() {
                "true" | "yes" | "1" | "on" => Ok(true),
                "false" | "no" | "0" | "off" => Ok(false),
                _ => Err(ConversionError::TypeMismatch {
                    from_type: "String",
                    to_type: "bool",
                }),
            },
            _ => Err(ConversionError::TypeMismatch {
                from_type: "ConfigValue",
                to_type: "bool",
            }),
        }
    }
}

// Uso con ? operator
fn get_port(config: &ConfigMap) -> Result<i64, ConversionError> {
    let value = config.get("server.port")
        .ok_or(ConversionError::TypeMismatch {
            from_type: "None",
            to_type: "i64",
        })?;

    i64::try_from(value)
}

// Uso con match
fn is_feature_enabled(config: &ConfigMap, feature: &str) -> bool {
    config.get(feature)
        .and_then(|v| bool::try_from(v).ok())
        .unwrap_or(false)
}
```

### Parsing de Properties

El formato `.properties` de Java es texto plano con lineas `key=value`. Implementamos un parser manual.

```rust
use crate::config::{ConfigMap, ConfigValue};
use indexmap::IndexMap;

/// Parser de formato .properties de Java
pub struct PropertiesParser;

impl PropertiesParser {
    /// Parsea string de properties a ConfigMap
    pub fn parse(input: &str) -> Result<ConfigMap, FormatError> {
        let mut flat_map: IndexMap<String, String> = IndexMap::new();

        for (line_num, line) in input.lines().enumerate() {
            let line = line.trim();

            // Ignorar lineas vacias y comentarios
            if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
                continue;
            }

            // Buscar separador (= o :)
            let (key, value) = Self::parse_line(line, line_num + 1)?;
            flat_map.insert(key, value);
        }

        // Convertir flat map a estructura anidada
        Ok(Self::unflatten(flat_map))
    }

    /// Parsea una linea key=value
    fn parse_line(line: &str, line_num: usize) -> Result<(String, String), FormatError> {
        // Buscar primer = o : que no este escapado
        let separator_pos = line.find(|c| c == '=' || c == ':');

        match separator_pos {
            Some(pos) => {
                let key = line[..pos].trim().to_string();
                let value = Self::unescape(&line[pos + 1..].trim_start());
                Ok((key, value))
            }
            None => Err(FormatError::PropertiesError {
                line: line_num,
                message: format!("no separator found in line: {}", line),
            }),
        }
    }

    /// Procesa escape sequences (\n, \t, \\, \=, etc.)
    fn unescape(input: &str) -> String {
        let mut result = String::with_capacity(input.len());
        let mut chars = input.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => result.push('\n'),
                    Some('t') => result.push('\t'),
                    Some('r') => result.push('\r'),
                    Some('\\') => result.push('\\'),
                    Some('=') => result.push('='),
                    Some(':') => result.push(':'),
                    Some(other) => {
                        result.push('\\');
                        result.push(other);
                    }
                    None => result.push('\\'),
                }
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Convierte mapa plano (dot notation) a estructura anidada
    fn unflatten(flat: IndexMap<String, String>) -> ConfigMap {
        let mut root: IndexMap<String, ConfigValue> = IndexMap::new();

        for (key, value) in flat {
            Self::insert_nested(&mut root, &key, value);
        }

        ConfigMap::from_inner(root)
    }

    /// Inserta valor en path anidado
    fn insert_nested(map: &mut IndexMap<String, ConfigValue>, path: &str, value: String) {
        let parts: Vec<&str> = path.splitn(2, '.').collect();

        match parts.as_slice() {
            [single_key] => {
                // Ultimo nivel: insertar valor
                map.insert(single_key.to_string(), ConfigValue::String(value));
            }
            [first, rest] => {
                // Nivel intermedio: crear/obtener objeto anidado
                let entry = map
                    .entry(first.to_string())
                    .or_insert_with(|| ConfigValue::Object(IndexMap::new()));

                if let ConfigValue::Object(inner) = entry {
                    Self::insert_nested(inner, rest, value);
                }
            }
            _ => unreachable!(),
        }
    }
}

/// Serializer a formato .properties
pub struct PropertiesSerializer;

impl PropertiesSerializer {
    /// Serializa ConfigMap a string de properties
    pub fn serialize(config: &ConfigMap) -> String {
        let mut lines = Vec::new();
        Self::flatten_to_lines(config.as_inner(), "", &mut lines);
        lines.join("\n")
    }

    fn flatten_to_lines(
        map: &IndexMap<String, ConfigValue>,
        prefix: &str,
        lines: &mut Vec<String>,
    ) {
        for (key, value) in map {
            let full_key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", prefix, key)
            };

            match value {
                ConfigValue::Object(nested) => {
                    Self::flatten_to_lines(nested, &full_key, lines);
                }
                ConfigValue::Array(arr) => {
                    // Arrays como indices: key[0], key[1], etc.
                    for (i, item) in arr.iter().enumerate() {
                        let array_key = format!("{}[{}]", full_key, i);
                        lines.push(format!(
                            "{}={}",
                            array_key,
                            Self::escape(&Self::value_to_string(item))
                        ));
                    }
                }
                _ => {
                    lines.push(format!(
                        "{}={}",
                        full_key,
                        Self::escape(&Self::value_to_string(value))
                    ));
                }
            }
        }
    }

    fn value_to_string(value: &ConfigValue) -> String {
        match value {
            ConfigValue::Null => "".to_string(),
            ConfigValue::Bool(b) => b.to_string(),
            ConfigValue::Integer(n) => n.to_string(),
            ConfigValue::Float(f) => f.to_string(),
            ConfigValue::String(s) => s.clone(),
            ConfigValue::Array(_) | ConfigValue::Object(_) => {
                // Deberia haberse manejado en flatten
                "[complex]".to_string()
            }
        }
    }

    fn escape(input: &str) -> String {
        input
            .replace('\\', "\\\\")
            .replace('\n', "\\n")
            .replace('\t', "\\t")
            .replace('\r', "\\r")
    }
}
```

### Trait Objects y Polimorfismo

Para permitir conversion generica entre formatos, usamos trait objects.

```rust
use crate::config::ConfigMap;

/// Trait para parsers de formato
pub trait FormatParser: Send + Sync {
    fn parse(&self, input: &str) -> Result<ConfigMap, FormatError>;
    fn format_name(&self) -> &'static str;
}

/// Trait para serializers de formato
pub trait FormatSerializer: Send + Sync {
    fn serialize(&self, config: &ConfigMap) -> Result<String, FormatError>;
    fn format_name(&self) -> &'static str;
}

// Implementaciones concretas
pub struct JsonFormat;
pub struct YamlFormat;
pub struct PropertiesFormat;

impl FormatParser for JsonFormat {
    fn parse(&self, input: &str) -> Result<ConfigMap, FormatError> {
        serde_json::from_str(input).map_err(FormatError::from)
    }

    fn format_name(&self) -> &'static str { "json" }
}

impl FormatSerializer for JsonFormat {
    fn serialize(&self, config: &ConfigMap) -> Result<String, FormatError> {
        serde_json::to_string_pretty(config).map_err(FormatError::from)
    }

    fn format_name(&self) -> &'static str { "json" }
}

// Funcion que acepta cualquier parser
fn parse_config(parser: &dyn FormatParser, input: &str) -> Result<ConfigMap, FormatError> {
    parser.parse(input)
}

// Funcion que retorna parser basado en extension
fn get_parser(extension: &str) -> Option<Box<dyn FormatParser>> {
    match extension {
        "json" => Some(Box::new(JsonFormat)),
        "yaml" | "yml" => Some(Box::new(YamlFormat)),
        "properties" => Some(Box::new(PropertiesFormat)),
        _ => None,
    }
}
```

## Riesgos y Errores Comunes

### 1. Perdida de Tipos en Properties

```rust
// Properties solo tiene strings, se pierde informacion de tipo
let json = r#"{"port": 8080, "enabled": true}"#;
let config = ConfigMap::from_json(json)?;
let props = PropertiesSerializer::serialize(&config);
// props = "port=8080\nenabled=true"  <- ahora son strings

let reparsed = PropertiesParser::parse(&props)?;
// port es ConfigValue::String("8080"), no Integer(8080)!

// SOLUCION: conversion explicita cuando se necesita tipo especifico
let port: i64 = i64::try_from(reparsed.get("port").unwrap())?;
```

### 2. Colisiones en Aplanamiento

```rust
// Ambiguo: es "a.b" una clave o estructura anidada?
let props1 = "a.b=1";
let props2 = "a.b.c=2\na.b=1";  // Conflicto!

// El segundo tiene a.b como objeto (con a.b.c) y como valor
// Nuestra implementacion: ultimo valor gana o error
```

### 3. Caracteres Especiales No Escapados

```rust
// ERROR: = en valor no escapado
let bad = "message=hello=world";  // Se parsea como key="message", value="hello=world"
// Puede no ser lo esperado

// CORRECTO: escape en origen
let good = "message=hello\\=world";
```

## Pruebas

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_into_primitives() {
        let v: ConfigValue = 42.into();
        assert_eq!(v, ConfigValue::Integer(42));

        let v: ConfigValue = "hello".into();
        assert_eq!(v.as_str(), Some("hello"));

        let v: ConfigValue = true.into();
        assert_eq!(v.as_bool(), Some(true));
    }

    #[test]
    fn test_try_from_valid() {
        let v = ConfigValue::Integer(42);
        let n: i64 = i64::try_from(&v).unwrap();
        assert_eq!(n, 42);

        let v = ConfigValue::String("true".to_string());
        let b: bool = bool::try_from(&v).unwrap();
        assert!(b);
    }

    #[test]
    fn test_try_from_invalid() {
        let v = ConfigValue::Array(vec![]);
        let result = i64::try_from(&v);
        assert!(result.is_err());
    }

    #[test]
    fn test_properties_parse_simple() {
        let props = "key=value\nother=data";
        let config = PropertiesParser::parse(props).unwrap();

        assert_eq!(config.get("key").unwrap().as_str(), Some("value"));
        assert_eq!(config.get("other").unwrap().as_str(), Some("data"));
    }

    #[test]
    fn test_properties_parse_nested() {
        let props = "database.host=localhost\ndatabase.port=5432";
        let config = PropertiesParser::parse(props).unwrap();

        assert_eq!(
            config.get("database.host").unwrap().as_str(),
            Some("localhost")
        );
        assert_eq!(
            config.get("database.port").unwrap().as_str(),
            Some("5432")
        );
    }

    #[test]
    fn test_properties_escape_sequences() {
        let props = r"message=hello\nworld\ttab";
        let config = PropertiesParser::parse(props).unwrap();

        assert_eq!(
            config.get("message").unwrap().as_str(),
            Some("hello\nworld\ttab")
        );
    }

    #[test]
    fn test_properties_comments() {
        let props = "# comment\nkey=value\n! another comment\nother=data";
        let config = PropertiesParser::parse(props).unwrap();

        assert_eq!(config.len(), 2);
        assert!(config.get("key").is_some());
        assert!(config.get("other").is_some());
    }

    #[test]
    fn test_properties_serialize() {
        let json = r#"{"database": {"host": "localhost", "port": 5432}}"#;
        let config = ConfigMap::from_json(json).unwrap();

        let props = PropertiesSerializer::serialize(&config);

        assert!(props.contains("database.host=localhost"));
        assert!(props.contains("database.port=5432"));
    }

    #[test]
    fn test_json_roundtrip() {
        let original = r#"{"a": 1, "b": {"c": "hello", "d": [1, 2, 3]}}"#;
        let config = ConfigMap::from_json(original).unwrap();
        let serialized = config.to_json().unwrap();
        let reparsed = ConfigMap::from_json(&serialized).unwrap();

        assert_eq!(config, reparsed);
    }

    #[test]
    fn test_yaml_roundtrip() {
        let yaml = r#"
            database:
              host: localhost
              port: 5432
            features:
              - auth
              - cache
        "#;

        let config = ConfigMap::from_yaml(yaml).unwrap();
        let serialized = config.to_yaml().unwrap();
        let reparsed = ConfigMap::from_yaml(&serialized).unwrap();

        assert_eq!(config, reparsed);
    }
}
```

### Integration Tests

```rust
// tests/format_conversion_tests.rs
use vortex_core::format::*;

#[test]
fn test_json_to_yaml_to_json() {
    let json = r#"{"server": {"port": 8080, "host": "0.0.0.0"}}"#;

    let config = ConfigMap::from_json(json).unwrap();
    let yaml = config.to_yaml().unwrap();
    let config2 = ConfigMap::from_yaml(&yaml).unwrap();
    let json2 = config2.to_json().unwrap();

    let final_config = ConfigMap::from_json(&json2).unwrap();
    assert_eq!(config, final_config);
}

#[test]
fn test_yaml_to_properties() {
    let yaml = r#"
spring:
  datasource:
    url: jdbc:postgresql://localhost/db
    username: admin
    password: secret
"#;

    let config = ConfigMap::from_yaml(yaml).unwrap();
    let props = PropertiesSerializer::serialize(&config);

    assert!(props.contains("spring.datasource.url=jdbc:postgresql://localhost/db"));
    assert!(props.contains("spring.datasource.username=admin"));
}

#[test]
fn test_unicode_preservation() {
    let json = r#"{"greeting": "Hola mundo!", "emoji": "test"}"#;
    let config = ConfigMap::from_json(json).unwrap();

    // JSON roundtrip
    let json2 = config.to_json().unwrap();
    assert!(json2.contains("Hola mundo!"));

    // YAML roundtrip
    let yaml = config.to_yaml().unwrap();
    let config2 = ConfigMap::from_yaml(&yaml).unwrap();
    assert_eq!(config, config2);
}

#[test]
fn test_special_characters_in_properties() {
    let config = ConfigMap::from_json(r#"{"path": "C:\\Users\\name"}"#).unwrap();
    let props = PropertiesSerializer::serialize(&config);

    // Backslashes deben estar escapados
    assert!(props.contains("path=C:\\\\Users\\\\name"));

    // Roundtrip
    let reparsed = PropertiesParser::parse(&props).unwrap();
    assert_eq!(
        reparsed.get("path").unwrap().as_str(),
        Some("C:\\Users\\name")
    );
}
```

## Entregable Final

- PR con:
  - `crates/vortex-core/src/format/json.rs`
  - `crates/vortex-core/src/format/yaml.rs`
  - `crates/vortex-core/src/format/properties.rs`
  - Actualizacion de `crates/vortex-core/src/format/mod.rs`
  - Implementacion de `From/Into` para ConfigValue
  - Implementacion de `TryFrom/TryInto` para conversiones falibles
  - Tests de round-trip para cada formato
  - Tests de edge cases (unicode, escapes, caracteres especiales)
  - Rustdoc completo

---

**Anterior**: [Historia 003 - Formatos de Respuesta Spring](./story-003-spring-format.md)
**Siguiente**: [Historia 005 - Unit Testing de Tipos Core](./story-005-core-testing.md)
