# Historia 003: Formatos de Respuesta Spring

## Contexto y Objetivo

Spring Cloud Config Server retorna configuraciones en un formato JSON especifico que incluye metadata ademas de las propiedades. Los clientes Spring Boot esperan este formato exacto para poder consumir la configuracion correctamente.

Esta historia implementa la serializacion al formato de respuesta de Spring Cloud Config, garantizando compatibilidad completa con clientes existentes.

## Alcance

### In Scope
- Estructura `SpringConfigResponse` compatible con Spring Cloud Config
- Estructura `PropertySource` (formato Spring, diferente al nuestro interno)
- Serializacion JSON con campo naming correcto (camelCase)
- Soporte para multiples PropertySources en respuesta
- Campo `version` y `state` para tracking

### Out of Scope
- Endpoints HTTP (epica 03)
- Serializacion a otros formatos (YAML plano, .properties)
- Encriptacion de valores `{cipher}...`
- Labels y branches de Git

## Criterios de Aceptacion

- [ ] `SpringConfigResponse` serializa a JSON identico a Spring Cloud Config
- [ ] Nombres de campos en camelCase como espera Spring
- [ ] `propertySources` es array ordenado por precedencia
- [ ] Campo `version` incluye hash/label del commit
- [ ] Campo `state` es opcional
- [ ] Tests de compatibilidad con responses reales de Spring Cloud Config

## Diseno Propuesto

### Formato de Respuesta Spring Cloud Config

```json
{
  "name": "myapp",
  "profiles": ["production"],
  "label": "main",
  "version": "abc123def",
  "state": null,
  "propertySources": [
    {
      "name": "git@github.com:config-repo/myapp-production.yml",
      "source": {
        "database.host": "prod-db.example.com",
        "database.port": 5432,
        "feature.enabled": true
      }
    },
    {
      "name": "git@github.com:config-repo/application.yml",
      "source": {
        "server.port": 8080,
        "database.host": "localhost"
      }
    }
  ]
}
```

### Interfaces

```rust
/// Respuesta compatible con Spring Cloud Config
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpringConfigResponse {
    /// Nombre de la aplicacion
    pub name: String,
    /// Perfiles activos
    pub profiles: Vec<String>,
    /// Label (branch/tag de Git)
    pub label: String,
    /// Version (commit hash)
    pub version: Option<String>,
    /// Estado adicional (opcional)
    pub state: Option<String>,
    /// Lista de fuentes de propiedades
    pub property_sources: Vec<SpringPropertySource>,
}

/// Fuente de propiedades en formato Spring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpringPropertySource {
    /// Nombre/identificador de la fuente
    pub name: String,
    /// Propiedades aplanadas (flat keys)
    pub source: IndexMap<String, serde_json::Value>,
}
```

### Estructura Sugerida

```
crates/vortex-core/src/format/
├── mod.rs
├── json.rs
├── yaml.rs
└── spring.rs       # NUEVO
```

## Pasos de Implementacion

1. **Crear estructuras de respuesta Spring**
   - `SpringConfigResponse` con atributos serde
   - `SpringPropertySource` con source aplanado

2. **Implementar conversion desde tipos internos**
   - `From<MergedConfig>` para `SpringConfigResponse`
   - `From<PropertySource>` para `SpringPropertySource`

3. **Implementar aplanamiento de propiedades**
   - Funcion para convertir objetos anidados a flat keys
   - `{"database": {"host": "x"}}` -> `{"database.host": "x"}`

4. **Implementar builder para SpringConfigResponse**
   - Metodos fluidos para agregar metadata

5. **Tests de compatibilidad**
   - Comparar output con responses reales de Spring Cloud Config

## Conceptos de Rust Aprendidos

### Serde Attributes y Customizacion

Serde ofrece atributos para personalizar la serializacion/deserializacion. Estos atributos se procesan en tiempo de compilacion y generan codigo optimo.

```rust
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Respuesta compatible con Spring Cloud Config
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]  // Convierte snake_case -> camelCase
pub struct SpringConfigResponse {
    /// Nombre de la aplicacion
    pub name: String,

    /// Perfiles activos
    pub profiles: Vec<String>,

    /// Label (branch/tag)
    #[serde(default)]  // Si no existe en JSON, usa Default::default()
    pub label: String,

    /// Version del commit
    #[serde(skip_serializing_if = "Option::is_none")]  // No incluir si es None
    pub version: Option<String>,

    /// Estado adicional
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,

    /// Lista de fuentes - campo se renombra a "propertySources" en JSON
    #[serde(rename = "propertySources")]
    pub property_sources: Vec<SpringPropertySource>,
}

/// Fuente de propiedades formato Spring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpringPropertySource {
    pub name: String,

    /// IndexMap preserva orden de insercion
    /// Value permite cualquier tipo JSON (string, number, bool, etc.)
    pub source: IndexMap<String, Value>,
}

impl SpringConfigResponse {
    /// Constructor con valores requeridos
    pub fn new(name: impl Into<String>, profiles: Vec<String>, label: impl Into<String>) -> Self {
        SpringConfigResponse {
            name: name.into(),
            profiles,
            label: label.into(),
            version: None,
            state: None,
            property_sources: Vec::new(),
        }
    }

    /// Builder pattern - establece version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Builder pattern - agrega PropertySource
    pub fn add_source(mut self, source: SpringPropertySource) -> Self {
        self.property_sources.push(source);
        self
    }
}
```

**Atributos Serde comunes:**

| Atributo | Uso | Ejemplo |
|----------|-----|---------|
| `#[serde(rename = "x")]` | Renombra campo | `#[serde(rename = "firstName")]` |
| `#[serde(rename_all = "camelCase")]` | Renombra todos los campos | En struct |
| `#[serde(skip)]` | Ignora campo | Campos internos |
| `#[serde(skip_serializing_if = "...")]` | Omite condicionalmente | `Option::is_none` |
| `#[serde(default)]` | Valor por defecto si falta | Campos opcionales |
| `#[serde(flatten)]` | Aplana struct anidado | Embedding |
| `#[serde(with = "module")]` | Serializacion custom | Fechas, enums |

**Comparacion con Jackson:**

```java
// Jackson en Java
@JsonNaming(PropertyNamingStrategies.LowerCamelCaseStrategy.class)
public class SpringConfigResponse {
    private String name;
    private List<String> profiles;

    @JsonProperty("propertySources")
    private List<PropertySource> propertySources;

    @JsonInclude(JsonInclude.Include.NON_NULL)
    private String version;
}

// Rust con Serde - equivalente
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpringConfigResponse {
    pub name: String,
    pub profiles: Vec<String>,
    #[serde(rename = "propertySources")]
    pub property_sources: Vec<SpringPropertySource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}
```

### Aplanamiento de Estructuras Anidadas

Convertir estructuras anidadas a formato flat key es necesario para compatibilidad con Spring que usa `source` como mapa plano.

```rust
use crate::config::{ConfigMap, ConfigValue};
use indexmap::IndexMap;
use serde_json::Value;

/// Convierte ConfigMap anidado a mapa plano con dot notation
pub fn flatten_config(config: &ConfigMap) -> IndexMap<String, Value> {
    let mut result = IndexMap::new();
    flatten_value(&Value::Object(config_to_json_map(config)), "", &mut result);
    result
}

/// Recursivamente aplana un Value JSON
fn flatten_value(value: &Value, prefix: &str, result: &mut IndexMap<String, Value>) {
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                let new_key = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };

                // Recursion para objetos anidados
                if val.is_object() {
                    flatten_value(val, &new_key, result);
                } else {
                    // Valor primitivo o array: insertar directamente
                    result.insert(new_key, val.clone());
                }
            }
        }
        // Valor no-objeto en raiz (raro pero posible)
        _ => {
            if !prefix.is_empty() {
                result.insert(prefix.to_string(), value.clone());
            }
        }
    }
}

/// Convierte ConfigMap a serde_json::Map
fn config_to_json_map(config: &ConfigMap) -> serde_json::Map<String, Value> {
    // Serializar a JSON y parsear como Map
    let json_str = serde_json::to_string(config).unwrap_or_default();
    serde_json::from_str(&json_str).unwrap_or_default()
}

// Ejemplo de uso
fn example() {
    let json = r#"{
        "database": {
            "host": "localhost",
            "port": 5432,
            "pool": {
                "maxSize": 10,
                "minIdle": 2
            }
        },
        "enabled": true
    }"#;

    let config = ConfigMap::from_json(json).unwrap();
    let flat = flatten_config(&config);

    // Resultado:
    // {
    //   "database.host": "localhost",
    //   "database.port": 5432,
    //   "database.pool.maxSize": 10,
    //   "database.pool.minIdle": 2,
    //   "enabled": true
    // }

    assert_eq!(flat.get("database.host").unwrap(), "localhost");
    assert_eq!(flat.get("database.pool.maxSize").unwrap(), 10);
}
```

### Conversion de Tipos Internos a Spring Format

Implementar traits de conversion para transformar nuestros tipos internos al formato Spring.

```rust
use crate::config::{ConfigMap, PropertySource, MergedConfig};

impl SpringPropertySource {
    /// Crea desde nuestro PropertySource interno
    pub fn from_internal(source: &PropertySource) -> Self {
        SpringPropertySource {
            name: source.origin.clone(),
            source: flatten_config(&source.config),
        }
    }
}

impl SpringConfigResponse {
    /// Crea respuesta desde configuracion merged
    pub fn from_merged(
        app: &str,
        profiles: Vec<String>,
        label: &str,
        merged: &MergedConfig,
        sources: &[PropertySource],
        version: Option<&str>,
    ) -> Self {
        let property_sources: Vec<SpringPropertySource> = sources
            .iter()
            .rev()  // Spring espera mayor prioridad primero
            .map(SpringPropertySource::from_internal)
            .collect();

        SpringConfigResponse {
            name: app.to_string(),
            profiles,
            label: label.to_string(),
            version: version.map(|v| v.to_string()),
            state: None,
            property_sources,
        }
    }

    /// Serializa a JSON string formateado
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Serializa a JSON compacto (sin espacios)
    pub fn to_json_compact(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}
```

### Lifetimes Basicos en Funciones

Los lifetimes aseguran que las referencias no sobrevivan a los datos que referencian. En la mayoria de casos, el compilador los infiere automaticamente.

```rust
/// El lifetime 'a indica que la referencia retornada vive tanto
/// como la referencia de entrada `sources`
pub fn find_source_by_name<'a>(
    sources: &'a [PropertySource],
    name: &str,  // Este &str no necesita lifetime explicito
) -> Option<&'a PropertySource> {
    sources.iter().find(|s| s.name == name)
}

// Rust infiere los lifetimes en casos simples.
// Estas dos firmas son equivalentes:
fn get_name(response: &SpringConfigResponse) -> &str {
    &response.name
}

fn get_name_explicit<'a>(response: &'a SpringConfigResponse) -> &'a str {
    &response.name
}

// Los lifetimes explicitos son necesarios cuando hay multiples referencias
// y el compilador no puede inferir cual sobrevive
fn choose_label<'a>(primary: &'a str, fallback: &'a str, use_primary: bool) -> &'a str {
    if use_primary { primary } else { fallback }
}
```

**Cuando especificar lifetimes:**

| Caso | Necesita lifetime explicito |
|------|---------------------------|
| Una referencia de entrada, una de salida | No (inferido) |
| Multiples referencias de entrada, una de salida | Si |
| Referencia en struct | Si |
| `&self` que retorna referencia | No (elision rule) |

## Riesgos y Errores Comunes

### 1. Orden Incorrecto de PropertySources

```rust
// ERROR: Spring espera mayor prioridad primero
let sources: Vec<SpringPropertySource> = internal_sources
    .iter()
    .map(SpringPropertySource::from_internal)
    .collect();  // Orden incorrecto si internal_sources esta en orden ascendente

// CORRECTO: invertir para que mayor prioridad este primero
let sources: Vec<SpringPropertySource> = internal_sources
    .iter()
    .rev()  // Invertir orden
    .map(SpringPropertySource::from_internal)
    .collect();
```

### 2. Tipos Numericos en JSON

```rust
// Spring puede retornar numeros como int o como string
// Hay que manejar ambos casos en deserializacion

#[derive(Deserialize)]
#[serde(untagged)]
enum NumberOrString {
    Number(i64),
    String(String),
}

impl NumberOrString {
    fn as_i64(&self) -> Option<i64> {
        match self {
            NumberOrString::Number(n) => Some(*n),
            NumberOrString::String(s) => s.parse().ok(),
        }
    }
}
```

### 3. Campos Faltantes vs Null

```rust
// En JSON: { "version": null } vs campo ausente
// Son diferentes y Serde los maneja diferente

// Esto distingue "presente pero null" de "ausente"
#[derive(Serialize, Deserialize)]
struct Response {
    // Si falta en JSON: None. Si es null en JSON: Some(None)
    // Complejo! Mejor evitar esta distincion si posible
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version: Option<Option<String>>,
}

// Mejor: tratar null y ausente como equivalentes
#[derive(Serialize, Deserialize)]
struct Response {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version: Option<String>,  // None para null O ausente
}
```

## Pruebas

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spring_response_serialization() {
        let response = SpringConfigResponse::new("myapp", vec!["prod".into()], "main")
            .with_version("abc123");

        let json = response.to_json().unwrap();

        assert!(json.contains(r#""name": "myapp""#));
        assert!(json.contains(r#""profiles": ["prod"]"#));
        assert!(json.contains(r#""label": "main""#));
        assert!(json.contains(r#""version": "abc123""#));
        assert!(json.contains(r#""propertySources""#));  // camelCase
    }

    #[test]
    fn test_flatten_simple() {
        let json = r#"{"key": "value"}"#;
        let config = ConfigMap::from_json(json).unwrap();
        let flat = flatten_config(&config);

        assert_eq!(flat.len(), 1);
        assert_eq!(flat.get("key").unwrap(), "value");
    }

    #[test]
    fn test_flatten_nested() {
        let json = r#"{
            "database": {
                "host": "localhost",
                "credentials": {
                    "username": "admin"
                }
            }
        }"#;
        let config = ConfigMap::from_json(json).unwrap();
        let flat = flatten_config(&config);

        assert!(flat.contains_key("database.host"));
        assert!(flat.contains_key("database.credentials.username"));
        assert!(!flat.contains_key("database"));  // Objetos no aparecen
    }

    #[test]
    fn test_skip_none_version() {
        let response = SpringConfigResponse::new("app", vec![], "main");
        // version es None

        let json = response.to_json().unwrap();

        assert!(!json.contains("version"));  // No debe aparecer
    }

    #[test]
    fn test_camel_case_fields() {
        let mut response = SpringConfigResponse::new("app", vec![], "main");
        response.property_sources.push(SpringPropertySource {
            name: "test".into(),
            source: IndexMap::new(),
        });

        let json = response.to_json().unwrap();

        // Debe ser camelCase, no snake_case
        assert!(json.contains("propertySources"));
        assert!(!json.contains("property_sources"));
    }
}
```

### Integration Tests - Compatibilidad con Spring

```rust
// tests/compatibility_tests.rs
use vortex_core::format::spring::{SpringConfigResponse, SpringPropertySource};

/// Response real de Spring Cloud Config Server para comparacion
const SPRING_RESPONSE: &str = r#"{
  "name": "myapp",
  "profiles": ["production"],
  "label": "main",
  "version": "a1b2c3d4e5f6",
  "state": null,
  "propertySources": [
    {
      "name": "git@github.com:org/config-repo.git/myapp-production.yml",
      "source": {
        "server.port": 8080,
        "database.url": "jdbc:postgresql://prod-db:5432/myapp",
        "feature.new-checkout": true
      }
    },
    {
      "name": "git@github.com:org/config-repo.git/application.yml",
      "source": {
        "server.port": 8000,
        "logging.level.root": "INFO"
      }
    }
  ]
}"#;

#[test]
fn test_deserialize_spring_response() {
    let response: SpringConfigResponse = serde_json::from_str(SPRING_RESPONSE).unwrap();

    assert_eq!(response.name, "myapp");
    assert_eq!(response.profiles, vec!["production"]);
    assert_eq!(response.label, "main");
    assert_eq!(response.version, Some("a1b2c3d4e5f6".to_string()));
    assert!(response.state.is_none());
    assert_eq!(response.property_sources.len(), 2);
}

#[test]
fn test_roundtrip_spring_response() {
    // Deserializar respuesta Spring
    let original: SpringConfigResponse = serde_json::from_str(SPRING_RESPONSE).unwrap();

    // Serializar con nuestro codigo
    let serialized = original.to_json().unwrap();

    // Deserializar de nuevo
    let reparsed: SpringConfigResponse = serde_json::from_str(&serialized).unwrap();

    // Debe ser equivalente
    assert_eq!(original.name, reparsed.name);
    assert_eq!(original.profiles, reparsed.profiles);
    assert_eq!(original.property_sources.len(), reparsed.property_sources.len());
}

#[test]
fn test_spring_client_compatibility() {
    // Crear respuesta como lo haria Vortex
    let mut source = IndexMap::new();
    source.insert("server.port".to_string(), serde_json::json!(8080));
    source.insert("database.url".to_string(), serde_json::json!("jdbc:postgresql://localhost/db"));

    let response = SpringConfigResponse::new("testapp", vec!["dev".into()], "develop")
        .with_version("abc123")
        .add_source(SpringPropertySource {
            name: "file:config/testapp-dev.yml".into(),
            source,
        });

    let json = response.to_json().unwrap();

    // Verificar estructura esperada por Spring Boot client
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert!(parsed["name"].is_string());
    assert!(parsed["profiles"].is_array());
    assert!(parsed["propertySources"].is_array());
    assert!(parsed["propertySources"][0]["source"].is_object());
}
```

## Entregable Final

- PR con:
  - `crates/vortex-core/src/format/spring.rs`
  - Actualizacion de `crates/vortex-core/src/format/mod.rs`
  - Tests de compatibilidad con responses reales de Spring Cloud Config
  - Rustdoc para todas las estructuras publicas
  - Ejemplos de uso en documentation comments

---

**Anterior**: [Historia 002 - PropertySource y Merging](./story-002-property-source.md)
**Siguiente**: [Historia 004 - Conversion entre Formatos](./story-004-format-conversion.md)
