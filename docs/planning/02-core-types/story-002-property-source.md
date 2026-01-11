# Historia 002: PropertySource y Merging

## Contexto y Objetivo

En Spring Cloud Config, una aplicacion puede tener multiples fuentes de configuracion que se combinan con un orden de precedencia definido. Por ejemplo:

1. `application.yml` (base)
2. `application-{profile}.yml` (perfil especifico)
3. `{application}-{profile}.yml` (aplicacion + perfil)

PropertySource representa una de estas fuentes con metadata asociada (nombre, origen, prioridad). El merge combina multiples PropertySources respetando la precedencia, donde fuentes con mayor prioridad sobrescriben valores de fuentes con menor prioridad.

## Alcance

### In Scope
- Definicion de `PropertySource` con metadata
- Estrategia de merge cascading (deep merge)
- Merge de multiples PropertySources ordenados por prioridad
- Seguimiento del origen de cada propiedad (property origin tracking)

### Out of Scope
- Carga de archivos desde disco o red (epicas posteriores)
- Estrategias de merge alternativas (solo cascading por ahora)
- Resolucion de placeholders `${variable}`

## Criterios de Aceptacion

- [ ] `PropertySource` contiene ConfigMap + nombre + prioridad
- [ ] Merge de dos ConfigMaps produce deep merge correcto
- [ ] Arrays se reemplazan completamente (no se concatenan)
- [ ] Objetos anidados se fusionan recursivamente
- [ ] `PropertySourceList` ordena por prioridad y aplica merge
- [ ] Metodo para obtener origen de cada propiedad final

## Diseno Propuesto

### Modulos/Crates Implicados
- `vortex-core/src/config/source.rs` - PropertySource
- `vortex-core/src/merge/mod.rs` - Logica de merge
- `vortex-core/src/merge/strategy.rs` - MergeStrategy trait

### Interfaces

```rust
/// Fuente de configuracion con metadata
#[derive(Debug, Clone)]
pub struct PropertySource {
    /// Nombre identificador de la fuente
    pub name: String,
    /// Origen de la configuracion (archivo, URL, etc.)
    pub origin: String,
    /// Prioridad (mayor numero = mayor prioridad)
    pub priority: i32,
    /// Configuracion contenida
    pub config: ConfigMap,
}

/// Resultado del merge con informacion de origen
#[derive(Debug, Clone)]
pub struct MergedConfig {
    /// Configuracion resultante del merge
    pub config: ConfigMap,
    /// Mapa de propiedad -> nombre de fuente origen
    pub origins: HashMap<String, String>,
}

/// Lista ordenada de PropertySources
pub struct PropertySourceList {
    sources: Vec<PropertySource>,
}

impl PropertySourceList {
    /// Agrega una fuente y reordena por prioridad
    pub fn add(&mut self, source: PropertySource);

    /// Aplica merge de todas las fuentes
    pub fn merge(&self) -> MergedConfig;
}
```

### Estructura Sugerida

```
crates/vortex-core/src/
├── config/
│   ├── mod.rs
│   ├── map.rs
│   ├── value.rs
│   └── source.rs       # NUEVO
└── merge/
    ├── mod.rs          # NUEVO
    └── strategy.rs     # NUEVO
```

## Pasos de Implementacion

1. **Crear PropertySource**
   - Definir estructura con campos name, origin, priority, config
   - Implementar constructor y metodos de acceso

2. **Implementar merge de ConfigMaps**
   - Funcion `merge_configs(base: &ConfigMap, overlay: &ConfigMap) -> ConfigMap`
   - Logica recursiva para objetos anidados
   - Arrays: overlay reemplaza base completamente

3. **Implementar PropertySourceList**
   - Vector interno ordenado por prioridad
   - Metodo `merge()` que aplica merge secuencial

4. **Implementar tracking de origen**
   - Durante merge, registrar que fuente aporto cada propiedad
   - Estructura `MergedConfig` con config + origins

5. **Tests exhaustivos**

## Conceptos de Rust Aprendidos

### Borrowing y Referencias

Borrowing permite acceder a datos sin tomar ownership. Es fundamental para evitar copias innecesarias y para garantizar seguridad de memoria en tiempo de compilacion.

```rust
use crate::config::{ConfigMap, ConfigValue};
use indexmap::IndexMap;

/// Funcion que toma referencias inmutables (&) - no modifica ni consume los originales
/// Similar a pasar por referencia en Java, pero con garantias de compilacion
pub fn merge_configs(base: &ConfigMap, overlay: &ConfigMap) -> ConfigMap {
    let mut result = base.clone(); // Clonamos base para crear resultado nuevo

    // Iteramos sobre overlay tomando referencias
    for (key, overlay_value) in overlay.iter() {
        match result.get(key) {
            // Si la clave existe en base y ambos son objetos, merge recursivo
            Some(base_value) => {
                let merged_value = merge_values(base_value, overlay_value);
                result.insert(key.clone(), merged_value);
            }
            // Si no existe en base, simplemente agregar
            None => {
                result.insert(key.clone(), overlay_value.clone());
            }
        }
    }

    result
}

/// Merge de valores individuales
fn merge_values(base: &ConfigValue, overlay: &ConfigValue) -> ConfigValue {
    match (base, overlay) {
        // Ambos son objetos: merge recursivo
        (ConfigValue::Object(base_map), ConfigValue::Object(overlay_map)) => {
            let mut merged = base_map.clone();
            for (key, value) in overlay_map {
                match merged.get(key) {
                    Some(existing) => {
                        merged.insert(key.clone(), merge_values(existing, value));
                    }
                    None => {
                        merged.insert(key.clone(), value.clone());
                    }
                }
            }
            ConfigValue::Object(merged)
        }
        // Para cualquier otro caso, overlay gana
        (_, overlay) => overlay.clone(),
    }
}
```

**Reglas de Borrowing:**

| Regla | Descripcion | Ejemplo |
|-------|-------------|---------|
| Una referencia mutable O multiples inmutables | No ambas | `&mut` vs `&` |
| Referencias no pueden outlive el dato | Lifetime tracking | Compilador verifica |
| No null references | Siempre validas | Option para opcionales |

**Comparacion con Java:**

```java
// Java: todo es referencia, GC maneja memoria
public Map<String, Object> mergeConfigs(
    Map<String, Object> base,    // referencia, puede ser null
    Map<String, Object> overlay  // referencia, puede ser null
) {
    // base y overlay pueden ser modificados por otro thread
    // o ser null - Java no previene esto en compilacion
}

// Rust: ownership/borrowing explicito
pub fn merge_configs(
    base: &ConfigMap,    // referencia inmutable, garantizada no-null
    overlay: &ConfigMap  // referencia inmutable, garantizada no-null
) -> ConfigMap {
    // base y overlay NO pueden ser modificados mientras tenemos referencias
    // el compilador garantiza esto
}
```

### Iterators y Closures

Los iterators de Rust son lazy y zero-cost. Se compilan al mismo codigo que un loop manual pero con mejor expresividad.

```rust
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PropertySource {
    pub name: String,
    pub origin: String,
    pub priority: i32,
    pub config: ConfigMap,
}

pub struct PropertySourceList {
    sources: Vec<PropertySource>,
}

impl PropertySourceList {
    pub fn new() -> Self {
        PropertySourceList { sources: Vec::new() }
    }

    /// Agrega fuente y reordena por prioridad (menor a mayor)
    pub fn add(&mut self, source: PropertySource) {
        self.sources.push(source);
        // sort_by_key usa closure para extraer clave de ordenamiento
        self.sources.sort_by_key(|s| s.priority);
    }

    /// Merge de todas las fuentes
    pub fn merge(&self) -> MergedConfig {
        // fold: similar a reduce en Java Streams
        // Acumula resultado aplicando merge secuencialmente
        let (config, origins) = self.sources.iter().fold(
            (ConfigMap::new(), HashMap::new()),
            |(acc_config, mut acc_origins), source| {
                // Merge de configuraciones
                let merged = merge_configs(&acc_config, &source.config);

                // Tracking de origenes: cada clave nueva viene de esta fuente
                for key in source.config.keys() {
                    acc_origins.insert(key.clone(), source.name.clone());
                }

                (merged, acc_origins)
            },
        );

        MergedConfig { config, origins }
    }

    /// Obtener fuentes filtradas por patron de nombre
    pub fn filter_by_name(&self, pattern: &str) -> Vec<&PropertySource> {
        self.sources
            .iter()                              // Crea iterator
            .filter(|s| s.name.contains(pattern)) // Filtra (lazy)
            .collect()                           // Materializa a Vec
    }

    /// Obtener nombres de todas las fuentes
    pub fn source_names(&self) -> Vec<&str> {
        self.sources
            .iter()
            .map(|s| s.name.as_str())  // Transforma PropertySource -> &str
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct MergedConfig {
    pub config: ConfigMap,
    pub origins: HashMap<String, String>,
}

impl MergedConfig {
    /// Obtiene el origen de una propiedad
    pub fn get_origin(&self, key: &str) -> Option<&str> {
        self.origins.get(key).map(|s| s.as_str())
    }
}
```

**Metodos de Iterator comunes:**

| Metodo | Descripcion | Java Equivalent |
|--------|-------------|-----------------|
| `iter()` | Iterator inmutable | `stream()` |
| `iter_mut()` | Iterator mutable | N/A |
| `map()` | Transforma elementos | `map()` |
| `filter()` | Filtra elementos | `filter()` |
| `fold()` | Reduce a un valor | `reduce()` |
| `collect()` | Materializa | `collect()` |
| `find()` | Primer match | `findFirst()` |
| `any()` / `all()` | Predicados | `anyMatch()` / `allMatch()` |

### Option y Manejo de Valores Opcionales

Option<T> reemplaza null de Java. El compilador fuerza el manejo explicito de casos donde un valor puede no existir.

```rust
impl MergedConfig {
    /// Obtiene valor de configuracion, retorna Option
    pub fn get(&self, key: &str) -> Option<&ConfigValue> {
        self.config.get(key)
    }

    /// Obtiene valor como string con valor por defecto
    pub fn get_string_or(&self, key: &str, default: &str) -> String {
        self.config
            .get(key)                      // Option<&ConfigValue>
            .and_then(|v| v.as_str())      // Option<&str>
            .map(|s| s.to_string())        // Option<String>
            .unwrap_or_else(|| default.to_string())  // String
    }

    /// Obtiene valor requerido, retorna error si no existe
    pub fn get_required(&self, key: &str) -> Result<&ConfigValue, ConfigError> {
        self.config
            .get(key)
            .ok_or_else(|| ConfigError::MissingProperty {
                key: key.to_string(),
            })
    }

    /// Ejemplo de pattern matching con Option
    pub fn describe_value(&self, key: &str) -> String {
        match self.config.get(key) {
            Some(ConfigValue::String(s)) => format!("String: {}", s),
            Some(ConfigValue::Integer(n)) => format!("Integer: {}", n),
            Some(ConfigValue::Bool(b)) => format!("Boolean: {}", b),
            Some(ConfigValue::Null) => "Null value".to_string(),
            Some(ConfigValue::Array(arr)) => format!("Array with {} elements", arr.len()),
            Some(ConfigValue::Object(obj)) => format!("Object with {} keys", obj.len()),
            Some(ConfigValue::Float(f)) => format!("Float: {}", f),
            None => format!("Property '{}' not found", key),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required property: {key}")]
    MissingProperty { key: String },
}
```

**Comparacion con Java Optional:**

```java
// Java: Optional puede ser bypasseado, null aun es posible
Optional<String> getValue(String key) {
    return Optional.ofNullable(config.get(key))
        .map(v -> v.toString());
}
// Pero: config.get(key) puede retornar null si no usas Optional

// Rust: Option es obligatorio, no hay null
fn get_value(&self, key: &str) -> Option<&str> {
    self.config.get(key).and_then(|v| v.as_str())
}
// No existe forma de tener un valor "nulo" sin Option
```

## Riesgos y Errores Comunes

### 1. Borrow Checker Frustration

```rust
// ERROR: no puedes tener &mut y & al mismo tiempo
fn bad_example(config: &mut ConfigMap) {
    let value = config.get("key");  // Borrow inmutable
    config.insert("other".into(), ConfigValue::Null);  // Borrow mutable - ERROR!
    println!("{:?}", value);  // Uso del borrow inmutable
}

// CORRECTO: separar los scopes de borrow
fn good_example(config: &mut ConfigMap) {
    let has_key = config.get("key").is_some();  // Borrow termina aqui
    if has_key {
        config.insert("other".into(), ConfigValue::Null);  // OK
    }
}

// O usar entry API
fn better_example(config: &mut ConfigMap) {
    // entry() maneja el borrow internamente
    config.entry("key".to_string())
        .or_insert(ConfigValue::String("default".into()));
}
```

### 2. Clone Excesivo

```rust
// INEFICIENTE: clona todo siempre
fn merge_naive(sources: &[PropertySource]) -> ConfigMap {
    sources.iter().fold(ConfigMap::new(), |acc, src| {
        merge_configs(&acc, &src.config)  // Clona en cada iteracion
    })
}

// MEJOR: usar referencias donde sea posible
fn merge_efficient(sources: &[PropertySource]) -> ConfigMap {
    if sources.is_empty() {
        return ConfigMap::new();
    }

    let mut result = sources[0].config.clone();  // Solo clonar una vez
    for source in &sources[1..] {
        merge_into(&mut result, &source.config);  // Modificar in-place
    }
    result
}

fn merge_into(target: &mut ConfigMap, overlay: &ConfigMap) {
    for (key, value) in overlay.iter() {
        // Insertar o mergear segun el caso
        if let Some(existing) = target.get_mut(key) {
            *existing = merge_values(existing, value);
        } else {
            target.insert(key.clone(), value.clone());
        }
    }
}
```

### 3. Ordenamiento Incorrecto de Prioridad

```rust
// ERROR: menor prioridad aplicada ultimo = gana
sources.sort_by_key(|s| std::cmp::Reverse(s.priority));

// CORRECTO: mayor prioridad aplicada ultimo = gana
sources.sort_by_key(|s| s.priority);  // Orden ascendente
// fold aplica en orden, ultimo gana en merge
```

## Pruebas

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_source(name: &str, priority: i32, json: &str) -> PropertySource {
        PropertySource {
            name: name.to_string(),
            origin: format!("test/{}.json", name),
            priority,
            config: ConfigMap::from_json(json).unwrap(),
        }
    }

    #[test]
    fn test_simple_merge() {
        let base = ConfigMap::from_json(r#"{"a": 1, "b": 2}"#).unwrap();
        let overlay = ConfigMap::from_json(r#"{"b": 3, "c": 4}"#).unwrap();

        let result = merge_configs(&base, &overlay);

        assert_eq!(result.get("a").unwrap().as_i64(), Some(1));
        assert_eq!(result.get("b").unwrap().as_i64(), Some(3)); // overlay gana
        assert_eq!(result.get("c").unwrap().as_i64(), Some(4));
    }

    #[test]
    fn test_deep_merge() {
        let base = ConfigMap::from_json(r#"{
            "database": {"host": "localhost", "port": 5432}
        }"#).unwrap();

        let overlay = ConfigMap::from_json(r#"{
            "database": {"port": 3306, "username": "admin"}
        }"#).unwrap();

        let result = merge_configs(&base, &overlay);

        assert_eq!(
            result.get("database.host").unwrap().as_str(),
            Some("localhost")  // preserved from base
        );
        assert_eq!(
            result.get("database.port").unwrap().as_i64(),
            Some(3306)  // overwritten by overlay
        );
        assert_eq!(
            result.get("database.username").unwrap().as_str(),
            Some("admin")  // added from overlay
        );
    }

    #[test]
    fn test_array_replacement() {
        let base = ConfigMap::from_json(r#"{"items": [1, 2, 3]}"#).unwrap();
        let overlay = ConfigMap::from_json(r#"{"items": [4, 5]}"#).unwrap();

        let result = merge_configs(&base, &overlay);

        // Arrays should be replaced, not concatenated
        let items = result.get("items").unwrap();
        if let ConfigValue::Array(arr) = items {
            assert_eq!(arr.len(), 2);
            assert_eq!(arr[0].as_i64(), Some(4));
            assert_eq!(arr[1].as_i64(), Some(5));
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_priority_ordering() {
        let mut list = PropertySourceList::new();

        list.add(make_source("high", 100, r#"{"key": "high"}"#));
        list.add(make_source("low", 10, r#"{"key": "low"}"#));
        list.add(make_source("medium", 50, r#"{"key": "medium"}"#));

        let merged = list.merge();

        // Highest priority source should win
        assert_eq!(
            merged.config.get("key").unwrap().as_str(),
            Some("high")
        );
        assert_eq!(merged.get_origin("key"), Some("high"));
    }

    #[test]
    fn test_origin_tracking() {
        let mut list = PropertySourceList::new();

        list.add(make_source("base", 10, r#"{"a": 1, "b": 2}"#));
        list.add(make_source("overlay", 20, r#"{"b": 3, "c": 4}"#));

        let merged = list.merge();

        assert_eq!(merged.get_origin("a"), Some("base"));
        assert_eq!(merged.get_origin("b"), Some("overlay"));
        assert_eq!(merged.get_origin("c"), Some("overlay"));
    }
}
```

### Integration Tests

```rust
// tests/merge_tests.rs
use vortex_core::config::{PropertySource, PropertySourceList};

#[test]
fn test_spring_like_hierarchy() {
    let mut list = PropertySourceList::new();

    // Simular jerarquia Spring Cloud Config
    list.add(PropertySource {
        name: "application.yml".to_string(),
        origin: "classpath:application.yml".to_string(),
        priority: 10,
        config: ConfigMap::from_json(r#"{
            "server": {"port": 8080},
            "database": {"host": "localhost"}
        }"#).unwrap(),
    });

    list.add(PropertySource {
        name: "application-dev.yml".to_string(),
        origin: "classpath:application-dev.yml".to_string(),
        priority: 20,
        config: ConfigMap::from_json(r#"{
            "database": {"host": "dev-db.example.com"}
        }"#).unwrap(),
    });

    list.add(PropertySource {
        name: "myapp-dev.yml".to_string(),
        origin: "git:config-repo/myapp-dev.yml".to_string(),
        priority: 30,
        config: ConfigMap::from_json(r#"{
            "server": {"port": 9090},
            "feature": {"new-ui": true}
        }"#).unwrap(),
    });

    let merged = list.merge();

    // Verify cascading merge
    assert_eq!(
        merged.config.get("server.port").unwrap().as_i64(),
        Some(9090)  // from myapp-dev
    );
    assert_eq!(
        merged.config.get("database.host").unwrap().as_str(),
        Some("dev-db.example.com")  // from application-dev
    );
    assert_eq!(
        merged.config.get("feature.new-ui").unwrap().as_bool(),
        Some(true)  // from myapp-dev
    );
}
```

## Entregable Final

- PR con:
  - `crates/vortex-core/src/config/source.rs`
  - `crates/vortex-core/src/merge/mod.rs`
  - `crates/vortex-core/src/merge/strategy.rs`
  - Actualizacion de `lib.rs` con re-exports
  - Tests unitarios con cobertura > 80%
  - Tests de integracion para escenarios Spring-like
  - Rustdoc para todas las estructuras y metodos publicos

---

**Anterior**: [Historia 001 - ConfigMap con Serde](./story-001-configmap-serde.md)
**Siguiente**: [Historia 003 - Formatos de Respuesta Spring](./story-003-spring-format.md)
