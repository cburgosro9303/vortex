# Historia 003: Diff Semantico

## Contexto y Objetivo

Hasta ahora, cuando una configuracion cambia, enviamos el snapshot completo al cliente. Esto funciona pero es ineficiente:

- Una configuracion grande de 100KB enviada por un cambio de 1 linea
- Multiples clientes multiplicando el overhead
- Clientes deben comparar localmente para saber que cambio

El **diff semantico** resuelve esto enviando solo las diferencias estructuradas. En lugar de enviar toda la configuracion, enviamos:

```json
{
  "type": "config_change",
  "diff": [
    {"op": "replace", "path": "/database/pool_size", "value": 20},
    {"op": "add", "path": "/features/new_flag", "value": true}
  ]
}
```

Esta historia implementa:
- Calculo de diferencias entre configuraciones JSON
- Formato de diff compatible con JSON Patch (RFC 6902)
- Optimizacion de mensajes WebSocket
- Reconstruccion de estado en el cliente

Para desarrolladores Java, esto es similar a librerias como `json-patch` de zjsonpatch, pero integrado con el ecosistema serde de Rust.

---

## Alcance

### In Scope

- `DiffCalculator`: Servicio para calcular diferencias entre JSON Values
- Formato de diff compatible con JSON Patch (RFC 6902)
- Operaciones: add, remove, replace, move, copy
- Integracion con `ConfigChangeBroadcaster`
- Envio de diff en lugar de snapshot completo
- Tests de diff calculation

### Out of Scope

- Diff de archivos de texto plano (YAML, properties)
- Compresion de diffs
- Merge de diffs conflictivos
- Diff bidireccional (three-way merge)
- Persistencia de historial de diffs

---

## Criterios de Aceptacion

- [ ] Diff calcula correctamente add, remove, replace
- [ ] Path usa notacion JSON Pointer (RFC 6901)
- [ ] Diffs vacios no se envian (config sin cambios)
- [ ] Mensaje de cambio incluye old_version y new_version
- [ ] Cliente puede aplicar diff para reconstruir estado
- [ ] Tamano de mensaje reducido > 50% para cambios pequenos

---

## Diseno Propuesto

### Estructura de Modulos

```
crates/vortex-server/src/ws/
├── mod.rs
├── handler.rs
├── connection.rs
├── messages.rs
├── registry.rs
├── broadcaster.rs
└── diff.rs           # Nuevo: DiffCalculator
```

### Interfaces Principales

```rust
/// Operacion de diff (JSON Patch)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffOperation {
    pub op: DiffOpType,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,  // Para move/copy
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffOpType {
    Add,
    Remove,
    Replace,
    Move,
    Copy,
    Test,  // Para validacion
}

/// Calculador de diferencias semanticas
pub struct DiffCalculator;

impl DiffCalculator {
    /// Calcula el diff entre dos JSON values
    pub fn diff(
        old: &serde_json::Value,
        new: &serde_json::Value,
    ) -> Vec<DiffOperation>;

    /// Aplica un diff a un JSON value
    pub fn apply(
        target: &mut serde_json::Value,
        ops: &[DiffOperation],
    ) -> Result<(), DiffError>;
}
```

---

## Pasos de Implementacion

### Paso 1: Definir Tipos de Diff

```rust
// src/ws/diff.rs
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Tipo de operacion de diff (JSON Patch RFC 6902)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffOpType {
    /// Agregar valor en path
    Add,
    /// Remover valor en path
    Remove,
    /// Reemplazar valor en path
    Replace,
    /// Mover valor de from a path
    Move,
    /// Copiar valor de from a path
    Copy,
    /// Verificar que path tiene value (para validacion)
    Test,
}

/// Operacion individual de diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffOperation {
    /// Tipo de operacion
    pub op: DiffOpType,
    /// Path en formato JSON Pointer (ej: "/database/host")
    pub path: String,
    /// Valor para add/replace/test
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// Path origen para move/copy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

impl DiffOperation {
    /// Crea operacion add
    pub fn add(path: impl Into<String>, value: Value) -> Self {
        Self {
            op: DiffOpType::Add,
            path: path.into(),
            value: Some(value),
            from: None,
        }
    }

    /// Crea operacion remove
    pub fn remove(path: impl Into<String>) -> Self {
        Self {
            op: DiffOpType::Remove,
            path: path.into(),
            value: None,
            from: None,
        }
    }

    /// Crea operacion replace
    pub fn replace(path: impl Into<String>, value: Value) -> Self {
        Self {
            op: DiffOpType::Replace,
            path: path.into(),
            value: Some(value),
            from: None,
        }
    }
}

/// Errores de diff
#[derive(Debug, Error)]
pub enum DiffError {
    #[error("Path not found: {0}")]
    PathNotFound(String),

    #[error("Invalid path format: {0}")]
    InvalidPath(String),

    #[error("Type mismatch at path {path}: expected {expected}, got {actual}")]
    TypeMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    #[error("Test operation failed at {path}")]
    TestFailed { path: String },

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}
```

### Paso 2: Implementar DiffCalculator

```rust
// src/ws/diff.rs (continuacion)

/// Calculador de diferencias semanticas entre JSON values.
///
/// Genera un conjunto de operaciones JSON Patch (RFC 6902) que,
/// cuando se aplican a `old`, producen `new`.
pub struct DiffCalculator;

impl DiffCalculator {
    /// Calcula las diferencias entre dos JSON values.
    ///
    /// # Ejemplo
    /// ```rust
    /// let old = json!({"a": 1, "b": 2});
    /// let new = json!({"a": 1, "b": 3, "c": 4});
    ///
    /// let diff = DiffCalculator::diff(&old, &new);
    /// // diff = [
    /// //   { "op": "replace", "path": "/b", "value": 3 },
    /// //   { "op": "add", "path": "/c", "value": 4 }
    /// // ]
    /// ```
    pub fn diff(old: &Value, new: &Value) -> Vec<DiffOperation> {
        let mut ops = Vec::new();
        Self::diff_values(old, new, String::new(), &mut ops);
        ops
    }

    /// Aplica operaciones de diff a un JSON value.
    ///
    /// # Errores
    /// Retorna error si alguna operacion no puede aplicarse.
    pub fn apply(target: &mut Value, ops: &[DiffOperation]) -> Result<(), DiffError> {
        for op in ops {
            Self::apply_operation(target, op)?;
        }
        Ok(())
    }

    /// Verifica si dos values son semanticamente iguales
    pub fn is_equal(a: &Value, b: &Value) -> bool {
        Self::diff(a, b).is_empty()
    }

    // --- Implementacion interna ---

    fn diff_values(old: &Value, new: &Value, path: String, ops: &mut Vec<DiffOperation>) {
        match (old, new) {
            // Ambos son objetos: comparar keys
            (Value::Object(old_map), Value::Object(new_map)) => {
                // Keys removidas
                for key in old_map.keys() {
                    if !new_map.contains_key(key) {
                        let key_path = Self::append_path(&path, key);
                        ops.push(DiffOperation::remove(key_path));
                    }
                }

                // Keys agregadas o modificadas
                for (key, new_val) in new_map {
                    let key_path = Self::append_path(&path, key);
                    match old_map.get(key) {
                        Some(old_val) => {
                            // Key existe, comparar valores recursivamente
                            Self::diff_values(old_val, new_val, key_path, ops);
                        }
                        None => {
                            // Key nueva
                            ops.push(DiffOperation::add(key_path, new_val.clone()));
                        }
                    }
                }
            }

            // Ambos son arrays: comparar elementos
            (Value::Array(old_arr), Value::Array(new_arr)) => {
                Self::diff_arrays(old_arr, new_arr, &path, ops);
            }

            // Tipos diferentes o valores primitivos diferentes
            _ => {
                if old != new {
                    if path.is_empty() {
                        // Root replacement
                        ops.push(DiffOperation::replace("/".to_string(), new.clone()));
                    } else {
                        ops.push(DiffOperation::replace(path, new.clone()));
                    }
                }
            }
        }
    }

    fn diff_arrays(old: &[Value], new: &[Value], path: &str, ops: &mut Vec<DiffOperation>) {
        // Estrategia simple: comparar por indice
        // Para arrays con muchos cambios, esto genera mas ops de las necesarias
        // Una implementacion mas sofisticada usaria LCS (Longest Common Subsequence)

        let max_len = old.len().max(new.len());

        for i in 0..max_len {
            let idx_path = format!("{}/{}", path, i);

            match (old.get(i), new.get(i)) {
                (Some(old_val), Some(new_val)) => {
                    // Ambos existen, comparar
                    Self::diff_values(old_val, new_val, idx_path, ops);
                }
                (Some(_), None) => {
                    // Elemento removido (nota: remover del final primero)
                    // Por ahora, lo agregamos; el orden se ajusta despues
                    ops.push(DiffOperation::remove(idx_path));
                }
                (None, Some(new_val)) => {
                    // Elemento agregado
                    ops.push(DiffOperation::add(idx_path, new_val.clone()));
                }
                (None, None) => unreachable!(),
            }
        }

        // Reordenar removes para que se apliquen del final al inicio
        Self::reorder_array_removes(ops, path);
    }

    fn reorder_array_removes(ops: &mut Vec<DiffOperation>, array_path: &str) {
        // Los removes en arrays deben aplicarse del indice mayor al menor
        // para que los indices sigan siendo validos
        let prefix = format!("{}/", array_path);

        // Separar removes de este array
        let (mut removes, others): (Vec<_>, Vec<_>) = ops.drain(..).partition(|op| {
            op.op == DiffOpType::Remove && op.path.starts_with(&prefix)
        });

        // Ordenar removes por indice descendente
        removes.sort_by(|a, b| {
            let idx_a = Self::extract_array_index(&a.path);
            let idx_b = Self::extract_array_index(&b.path);
            idx_b.cmp(&idx_a)
        });

        // Reconstruir ops: otros primero, removes al final
        ops.extend(others);
        ops.extend(removes);
    }

    fn extract_array_index(path: &str) -> usize {
        path.rsplit('/')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }

    fn append_path(base: &str, key: &str) -> String {
        // Escapar caracteres especiales en key segun RFC 6901
        let escaped = key.replace('~', "~0").replace('/', "~1");
        format!("{}/{}", base, escaped)
    }

    fn apply_operation(target: &mut Value, op: &DiffOperation) -> Result<(), DiffError> {
        match op.op {
            DiffOpType::Add => Self::apply_add(target, &op.path, op.value.clone()),
            DiffOpType::Remove => Self::apply_remove(target, &op.path),
            DiffOpType::Replace => Self::apply_replace(target, &op.path, op.value.clone()),
            DiffOpType::Move => {
                let from = op.from.as_ref().ok_or_else(|| {
                    DiffError::InvalidOperation("Move requires 'from' field".to_string())
                })?;
                Self::apply_move(target, from, &op.path)
            }
            DiffOpType::Copy => {
                let from = op.from.as_ref().ok_or_else(|| {
                    DiffError::InvalidOperation("Copy requires 'from' field".to_string())
                })?;
                Self::apply_copy(target, from, &op.path)
            }
            DiffOpType::Test => {
                let expected = op.value.as_ref().ok_or_else(|| {
                    DiffError::InvalidOperation("Test requires 'value' field".to_string())
                })?;
                Self::apply_test(target, &op.path, expected)
            }
        }
    }

    fn apply_add(target: &mut Value, path: &str, value: Option<Value>) -> Result<(), DiffError> {
        let value = value.ok_or_else(|| {
            DiffError::InvalidOperation("Add requires 'value' field".to_string())
        })?;

        let (parent_path, key) = Self::split_path(path)?;
        let parent = Self::get_mut_at_path(target, parent_path)?;

        match parent {
            Value::Object(map) => {
                map.insert(key.to_string(), value);
                Ok(())
            }
            Value::Array(arr) => {
                if key == "-" {
                    // "-" significa append
                    arr.push(value);
                } else {
                    let idx: usize = key.parse().map_err(|_| {
                        DiffError::InvalidPath(format!("Invalid array index: {}", key))
                    })?;
                    if idx > arr.len() {
                        return Err(DiffError::PathNotFound(path.to_string()));
                    }
                    arr.insert(idx, value);
                }
                Ok(())
            }
            _ => Err(DiffError::TypeMismatch {
                path: parent_path.to_string(),
                expected: "object or array".to_string(),
                actual: Self::type_name(parent).to_string(),
            }),
        }
    }

    fn apply_remove(target: &mut Value, path: &str) -> Result<(), DiffError> {
        let (parent_path, key) = Self::split_path(path)?;
        let parent = Self::get_mut_at_path(target, parent_path)?;

        match parent {
            Value::Object(map) => {
                map.remove(key).ok_or_else(|| DiffError::PathNotFound(path.to_string()))?;
                Ok(())
            }
            Value::Array(arr) => {
                let idx: usize = key.parse().map_err(|_| {
                    DiffError::InvalidPath(format!("Invalid array index: {}", key))
                })?;
                if idx >= arr.len() {
                    return Err(DiffError::PathNotFound(path.to_string()));
                }
                arr.remove(idx);
                Ok(())
            }
            _ => Err(DiffError::TypeMismatch {
                path: parent_path.to_string(),
                expected: "object or array".to_string(),
                actual: Self::type_name(parent).to_string(),
            }),
        }
    }

    fn apply_replace(target: &mut Value, path: &str, value: Option<Value>) -> Result<(), DiffError> {
        let value = value.ok_or_else(|| {
            DiffError::InvalidOperation("Replace requires 'value' field".to_string())
        })?;

        if path == "/" || path.is_empty() {
            *target = value;
            return Ok(());
        }

        let (parent_path, key) = Self::split_path(path)?;
        let parent = Self::get_mut_at_path(target, parent_path)?;

        match parent {
            Value::Object(map) => {
                if !map.contains_key(key) {
                    return Err(DiffError::PathNotFound(path.to_string()));
                }
                map.insert(key.to_string(), value);
                Ok(())
            }
            Value::Array(arr) => {
                let idx: usize = key.parse().map_err(|_| {
                    DiffError::InvalidPath(format!("Invalid array index: {}", key))
                })?;
                if idx >= arr.len() {
                    return Err(DiffError::PathNotFound(path.to_string()));
                }
                arr[idx] = value;
                Ok(())
            }
            _ => Err(DiffError::TypeMismatch {
                path: parent_path.to_string(),
                expected: "object or array".to_string(),
                actual: Self::type_name(parent).to_string(),
            }),
        }
    }

    fn apply_move(target: &mut Value, from: &str, path: &str) -> Result<(), DiffError> {
        // Move = Remove from 'from' + Add to 'path'
        let value = Self::get_at_path(target, from)?.clone();
        Self::apply_remove(target, from)?;
        Self::apply_add(target, path, Some(value))
    }

    fn apply_copy(target: &mut Value, from: &str, path: &str) -> Result<(), DiffError> {
        // Copy = Get from 'from' + Add to 'path'
        let value = Self::get_at_path(target, from)?.clone();
        Self::apply_add(target, path, Some(value))
    }

    fn apply_test(target: &Value, path: &str, expected: &Value) -> Result<(), DiffError> {
        let actual = Self::get_at_path(target, path)?;
        if actual == expected {
            Ok(())
        } else {
            Err(DiffError::TestFailed {
                path: path.to_string(),
            })
        }
    }

    fn split_path(path: &str) -> Result<(&str, &str), DiffError> {
        if !path.starts_with('/') {
            return Err(DiffError::InvalidPath(format!(
                "Path must start with '/': {}",
                path
            )));
        }

        let path = &path[1..]; // Remove leading '/'
        match path.rfind('/') {
            Some(pos) => Ok((&path[..pos], &path[pos + 1..])),
            None => Ok(("", path)),
        }
    }

    fn get_at_path<'a>(value: &'a Value, path: &str) -> Result<&'a Value, DiffError> {
        if path.is_empty() || path == "/" {
            return Ok(value);
        }

        let path = path.strip_prefix('/').unwrap_or(path);
        let mut current = value;

        for part in path.split('/') {
            let key = Self::unescape_key(part);
            current = match current {
                Value::Object(map) => map.get(&key).ok_or_else(|| {
                    DiffError::PathNotFound(path.to_string())
                })?,
                Value::Array(arr) => {
                    let idx: usize = key.parse().map_err(|_| {
                        DiffError::InvalidPath(format!("Invalid array index: {}", key))
                    })?;
                    arr.get(idx).ok_or_else(|| {
                        DiffError::PathNotFound(path.to_string())
                    })?
                }
                _ => return Err(DiffError::PathNotFound(path.to_string())),
            };
        }

        Ok(current)
    }

    fn get_mut_at_path<'a>(value: &'a mut Value, path: &str) -> Result<&'a mut Value, DiffError> {
        if path.is_empty() {
            return Ok(value);
        }

        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut current = value;

        for part in parts {
            let key = Self::unescape_key(part);
            current = match current {
                Value::Object(map) => map.get_mut(&key).ok_or_else(|| {
                    DiffError::PathNotFound(path.to_string())
                })?,
                Value::Array(arr) => {
                    let idx: usize = key.parse().map_err(|_| {
                        DiffError::InvalidPath(format!("Invalid array index: {}", key))
                    })?;
                    arr.get_mut(idx).ok_or_else(|| {
                        DiffError::PathNotFound(path.to_string())
                    })?
                }
                _ => return Err(DiffError::PathNotFound(path.to_string())),
            };
        }

        Ok(current)
    }

    fn unescape_key(key: &str) -> String {
        // RFC 6901: ~1 -> /, ~0 -> ~
        key.replace("~1", "/").replace("~0", "~")
    }

    fn type_name(value: &Value) -> &'static str {
        match value {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }
}
```

### Paso 3: Integrar con Broadcaster

```rust
// src/ws/broadcaster.rs (modificacion)
use super::diff::{DiffCalculator, DiffOperation};

impl ConfigChangeBroadcaster {
    /// Crea el mensaje a enviar basado en el evento
    fn create_message(&self, event: &ConfigChangeEvent) -> ServerMessage {
        // Calcular diff si tenemos config anterior
        match &event.old_config {
            Some(old) => {
                let diff = DiffCalculator::diff(old, &event.new_config);

                if diff.is_empty() {
                    // Sin cambios reales, no enviar nada
                    // (Esto no deberia pasar, pero por seguridad)
                    return ServerMessage::ConfigChange {
                        app: event.app.clone(),
                        profile: event.profile.clone(),
                        diff: vec![],
                        old_version: event.version.clone(), // Mismo version
                        new_version: event.version.clone(),
                        timestamp: event.timestamp,
                    };
                }

                // Convertir DiffOperation a DiffOp para serialization
                let diff_ops: Vec<DiffOp> = diff.into_iter().map(|op| DiffOp {
                    op: format!("{:?}", op.op).to_lowercase(),
                    path: op.path,
                    value: op.value,
                }).collect();

                ServerMessage::ConfigChange {
                    app: event.app.clone(),
                    profile: event.profile.clone(),
                    diff: diff_ops,
                    old_version: event.old_config
                        .as_ref()
                        .map(|_| "prev".to_string())
                        .unwrap_or_default(),
                    new_version: event.version.clone(),
                    timestamp: event.timestamp,
                }
            }
            None => {
                // No hay config anterior, enviar snapshot completo
                ServerMessage::snapshot(
                    &event.app,
                    &event.profile,
                    &event.label,
                    event.new_config.clone(),
                    &event.version,
                )
            }
        }
    }
}
```

### Paso 4: Actualizar Tipos de Mensajes

```rust
// src/ws/messages.rs (modificacion)

/// Mensajes enviados del servidor al cliente
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Snapshot completo de configuracion
    ConfigSnapshot {
        app: String,
        profile: String,
        label: String,
        config: serde_json::Value,
        version: String,
        timestamp: DateTime<Utc>,
    },

    /// Cambio incremental con diff
    ConfigChange {
        app: String,
        profile: String,
        diff: Vec<DiffOp>,
        old_version: String,
        new_version: String,
        timestamp: DateTime<Utc>,
    },

    // ... otros tipos
}

/// Operacion de diff para serializacion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffOp {
    pub op: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}
```

---

## Conceptos de Rust Aprendidos

### 1. Recursive Pattern Matching

El diff calculator usa pattern matching recursivo para comparar estructuras anidadas.

**Rust:**
```rust
fn diff_values(old: &Value, new: &Value, path: String, ops: &mut Vec<DiffOperation>) {
    match (old, new) {
        // Ambos objetos
        (Value::Object(old_map), Value::Object(new_map)) => {
            // Comparar maps recursivamente
            for (key, new_val) in new_map {
                match old_map.get(key) {
                    Some(old_val) => {
                        // Recurse
                        diff_values(old_val, new_val, new_path, ops);
                    }
                    None => {
                        ops.push(DiffOperation::add(new_path, new_val.clone()));
                    }
                }
            }
        }

        // Ambos arrays
        (Value::Array(old_arr), Value::Array(new_arr)) => {
            // Comparar arrays
        }

        // Catch-all: valores primitivos diferentes
        _ if old != new => {
            ops.push(DiffOperation::replace(path, new.clone()));
        }

        // Iguales, no hacer nada
        _ => {}
    }
}
```

**Comparacion con Java:**
```java
void diffValues(JsonNode old, JsonNode newNode, String path, List<DiffOp> ops) {
    if (old.isObject() && newNode.isObject()) {
        ObjectNode oldObj = (ObjectNode) old;
        ObjectNode newObj = (ObjectNode) newNode;

        Iterator<String> fieldNames = newObj.fieldNames();
        while (fieldNames.hasNext()) {
            String key = fieldNames.next();
            if (oldObj.has(key)) {
                diffValues(oldObj.get(key), newObj.get(key), path + "/" + key, ops);
            } else {
                ops.add(DiffOp.add(path + "/" + key, newObj.get(key)));
            }
        }
    } else if (old.isArray() && newNode.isArray()) {
        // Compare arrays
    } else if (!old.equals(newNode)) {
        ops.add(DiffOp.replace(path, newNode));
    }
}
```

**Ventajas de Rust:**
- Exhaustive matching garantiza manejar todos los casos
- El compilador avisa si olvidas un caso
- Pattern guards (`_ if old != new`) son expresivos

### 2. Mutable Borrows y Recursion

Pasar referencias mutables a funciones recursivas requiere cuidado.

**Rust:**
```rust
// El borrow checker garantiza que solo hay un &mut a la vez
fn get_mut_at_path<'a>(
    value: &'a mut Value,
    path: &str
) -> Result<&'a mut Value, DiffError> {
    let parts: Vec<&str> = path.split('/').collect();
    let mut current = value;

    for part in parts {
        // current se "re-borrow" en cada iteracion
        // Esto funciona porque el borrow anterior ya no se usa
        current = match current {
            Value::Object(map) => map.get_mut(part).ok_or(...)?,
            Value::Array(arr) => arr.get_mut(idx).ok_or(...)?,
            _ => return Err(...),
        };
    }

    Ok(current)
}

// Lifetime 'a garantiza que el resultado vive tanto como el input
```

**Comparacion con Java:**
```java
// Java: Sin lifetimes, el GC maneja la memoria
JsonNode getAtPath(JsonNode node, String path) {
    JsonNode current = node;
    for (String part : path.split("/")) {
        if (current.isObject()) {
            current = current.get(part);
        } else if (current.isArray()) {
            current = current.get(Integer.parseInt(part));
        }
        if (current == null) {
            throw new PathNotFoundException(path);
        }
    }
    return current;
}
```

### 3. String Escaping con Replace Chains

RFC 6901 requiere escapar caracteres especiales en paths.

**Rust:**
```rust
// Escapar: / -> ~1, ~ -> ~0 (~ primero para evitar doble escape)
fn escape_key(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

// Desescapar: ~1 -> /, ~0 -> ~ (inverso del escape)
fn unescape_key(key: &str) -> String {
    key.replace("~1", "/").replace("~0", "~")
}

// Ejemplo:
let key = "config/database";
let escaped = escape_key(key);  // "config~1database"
let back = unescape_key(&escaped);  // "config/database"
```

**Comparacion con Java:**
```java
String escapeKey(String key) {
    return key.replace("~", "~0").replace("/", "~1");
}

String unescapeKey(String key) {
    return key.replace("~1", "/").replace("~0", "~");
}
```

### 4. Builder-style con Metodos Asociados

`DiffOperation` usa metodos asociados para crear instancias de forma ergonomica.

**Rust:**
```rust
impl DiffOperation {
    pub fn add(path: impl Into<String>, value: Value) -> Self {
        Self {
            op: DiffOpType::Add,
            path: path.into(),
            value: Some(value),
            from: None,
        }
    }

    pub fn remove(path: impl Into<String>) -> Self {
        Self {
            op: DiffOpType::Remove,
            path: path.into(),
            value: None,
            from: None,
        }
    }
}

// Uso ergonomico
let op = DiffOperation::add("/new/key", json!(42));
let op = DiffOperation::remove("/old/key");
```

**Comparacion con Java (Builder pattern):**
```java
DiffOperation op = DiffOperation.builder()
    .op(OpType.ADD)
    .path("/new/key")
    .value(new IntNode(42))
    .build();

// O con factory methods
DiffOperation op = DiffOperation.add("/new/key", new IntNode(42));
```

---

## Riesgos y Errores Comunes

### 1. Escape incorrecto de paths

```rust
// MAL: Olvidar escapar caracteres especiales
let path = format!("/{}", key);  // key podria contener "/"

// BIEN: Escapar siempre
let path = format!("/{}", escape_key(key));

// Ejemplo problematico:
let key = "database/host";
// Sin escape: "/database/host" -> busca nested object!
// Con escape: "/database~1host" -> busca key literal "database/host"
```

### 2. Orden de operaciones en arrays

```rust
// MAL: Remover en orden ascendente
// Si removemos indice 2, el indice 3 se convierte en 2
ops.push(DiffOperation::remove("/arr/2"));
ops.push(DiffOperation::remove("/arr/3"));  // Error! Ahora es 2

// BIEN: Remover en orden descendente
ops.push(DiffOperation::remove("/arr/3"));
ops.push(DiffOperation::remove("/arr/2"));
```

### 3. Diff grande vs snapshot

```rust
// Si el diff es mas grande que el snapshot, enviar snapshot
fn create_message(&self, event: &ConfigChangeEvent) -> ServerMessage {
    let diff = DiffCalculator::diff(old, new);
    let diff_size = serde_json::to_string(&diff)?.len();
    let snapshot_size = serde_json::to_string(&event.new_config)?.len();

    if diff_size > snapshot_size * 0.8 {
        // Diff no es eficiente, enviar snapshot
        return ServerMessage::snapshot(...);
    }

    ServerMessage::ConfigChange { diff, ... }
}
```

### 4. Aplicar diff en orden incorrecto

```rust
// Las operaciones deben aplicarse en orden
// Especialmente importante para arrays

// MAL: Aplicar en paralelo
ops.par_iter().for_each(|op| apply(target, op));  // Race conditions!

// BIEN: Aplicar secuencialmente
for op in ops {
    DiffCalculator::apply(target, &[op.clone()])?;
}
```

---

## Pruebas

### Tests Unitarios

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_diff_add_key() {
        let old = json!({"a": 1});
        let new = json!({"a": 1, "b": 2});

        let diff = DiffCalculator::diff(&old, &new);

        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].op, DiffOpType::Add);
        assert_eq!(diff[0].path, "/b");
        assert_eq!(diff[0].value, Some(json!(2)));
    }

    #[test]
    fn test_diff_remove_key() {
        let old = json!({"a": 1, "b": 2});
        let new = json!({"a": 1});

        let diff = DiffCalculator::diff(&old, &new);

        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].op, DiffOpType::Remove);
        assert_eq!(diff[0].path, "/b");
    }

    #[test]
    fn test_diff_replace_value() {
        let old = json!({"a": 1});
        let new = json!({"a": 2});

        let diff = DiffCalculator::diff(&old, &new);

        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].op, DiffOpType::Replace);
        assert_eq!(diff[0].path, "/a");
        assert_eq!(diff[0].value, Some(json!(2)));
    }

    #[test]
    fn test_diff_nested_object() {
        let old = json!({"db": {"host": "localhost", "port": 5432}});
        let new = json!({"db": {"host": "localhost", "port": 5433}});

        let diff = DiffCalculator::diff(&old, &new);

        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].path, "/db/port");
        assert_eq!(diff[0].value, Some(json!(5433)));
    }

    #[test]
    fn test_diff_array_add() {
        let old = json!({"tags": ["a", "b"]});
        let new = json!({"tags": ["a", "b", "c"]});

        let diff = DiffCalculator::diff(&old, &new);

        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].op, DiffOpType::Add);
        assert_eq!(diff[0].path, "/tags/2");
    }

    #[test]
    fn test_diff_no_changes() {
        let old = json!({"a": 1, "b": {"c": 2}});
        let new = json!({"a": 1, "b": {"c": 2}});

        let diff = DiffCalculator::diff(&old, &new);

        assert!(diff.is_empty());
    }

    #[test]
    fn test_apply_add() {
        let mut target = json!({"a": 1});
        let ops = vec![DiffOperation::add("/b", json!(2))];

        DiffCalculator::apply(&mut target, &ops).unwrap();

        assert_eq!(target, json!({"a": 1, "b": 2}));
    }

    #[test]
    fn test_apply_remove() {
        let mut target = json!({"a": 1, "b": 2});
        let ops = vec![DiffOperation::remove("/b")];

        DiffCalculator::apply(&mut target, &ops).unwrap();

        assert_eq!(target, json!({"a": 1}));
    }

    #[test]
    fn test_apply_replace() {
        let mut target = json!({"a": 1});
        let ops = vec![DiffOperation::replace("/a", json!(2))];

        DiffCalculator::apply(&mut target, &ops).unwrap();

        assert_eq!(target, json!({"a": 2}));
    }

    #[test]
    fn test_roundtrip() {
        let old = json!({
            "database": {
                "host": "localhost",
                "port": 5432,
                "options": ["a", "b"]
            },
            "cache": true
        });

        let new = json!({
            "database": {
                "host": "newhost",
                "port": 5432,
                "options": ["a", "b", "c"]
            },
            "debug": true
        });

        let diff = DiffCalculator::diff(&old, &new);
        let mut result = old.clone();
        DiffCalculator::apply(&mut result, &diff).unwrap();

        assert_eq!(result, new);
    }

    #[test]
    fn test_escape_special_chars() {
        let old = json!({});
        let new = json!({"a/b": 1, "c~d": 2});

        let diff = DiffCalculator::diff(&old, &new);

        // Path debe estar escapado
        let paths: Vec<&str> = diff.iter().map(|op| op.path.as_str()).collect();
        assert!(paths.contains(&"/a~1b"));  // / -> ~1
        assert!(paths.contains(&"/c~0d"));  // ~ -> ~0
    }
}
```

### Tests de Integracion

```rust
// tests/ws_diff_test.rs
#[tokio::test]
async fn test_broadcast_sends_diff() {
    let app = create_test_app_with_broadcast().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = app.state().clone();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Conectar cliente
    let url = format!("ws://{}/ws/myapp/prod", addr);
    let (mut ws, _) = connect_async(&url).await.unwrap();

    // Consumir snapshot inicial
    let _ = ws.next().await;

    // Emitir cambio con old_config
    let old_config = json!({"port": 8080});
    let new_config = json!({"port": 9090, "debug": true});

    state.broadcaster.emit(ConfigChangeEvent::new(
        "myapp", "prod", "main",
        Some(old_config),
        new_config,
        "v2",
    )).unwrap();

    // Verificar que recibimos diff
    let msg = tokio::time::timeout(Duration::from_millis(100), ws.next())
        .await.unwrap().unwrap().unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&msg.into_text().unwrap()).unwrap();

    assert_eq!(parsed["type"], "config_change");
    assert!(parsed["diff"].is_array());

    let diff = parsed["diff"].as_array().unwrap();
    // Deberia haber replace de port y add de debug
    assert_eq!(diff.len(), 2);
}
```

---

## Observabilidad

### Logging

```rust
#[instrument(skip(old, new), fields(old_size, new_size, diff_count))]
pub fn diff_with_metrics(old: &Value, new: &Value) -> Vec<DiffOperation> {
    let old_size = serde_json::to_string(old).map(|s| s.len()).unwrap_or(0);
    let new_size = serde_json::to_string(new).map(|s| s.len()).unwrap_or(0);

    Span::current().record("old_size", old_size);
    Span::current().record("new_size", new_size);

    let ops = DiffCalculator::diff(old, new);

    Span::current().record("diff_count", ops.len());

    info!(
        reduction_pct = 100.0 - (ops.len() as f64 * 50.0 / new_size as f64),
        "Diff calculated"
    );

    ops
}
```

### Metricas

```rust
// Tamano de diffs
// metrics::histogram!("ws_diff_ops_count").record(ops.len() as f64);
// metrics::histogram!("ws_diff_size_bytes").record(diff_json.len() as f64);

// Eficiencia del diff
// let efficiency = 1.0 - (diff_size as f64 / snapshot_size as f64);
// metrics::histogram!("ws_diff_efficiency").record(efficiency);
```

---

## Entregable Final

### Archivos Creados/Modificados

1. `crates/vortex-server/src/ws/diff.rs` - DiffCalculator completo
2. `crates/vortex-server/src/ws/messages.rs` - Tipo ConfigChange con diff
3. `crates/vortex-server/src/ws/broadcaster.rs` - Integracion con diff
4. `crates/vortex-server/src/ws/mod.rs` - Re-exports
5. `crates/vortex-server/tests/ws_diff_test.rs` - Tests de integracion

### Verificacion

```bash
# Compilar
cargo build -p vortex-server

# Tests de diff
cargo test -p vortex-server diff

# Todos los tests
cargo test -p vortex-server

# Clippy
cargo clippy -p vortex-server -- -D warnings
```

### Ejemplo de Mensaje con Diff

```json
{
  "type": "config_change",
  "app": "myapp",
  "profile": "production",
  "diff": [
    {"op": "replace", "path": "/database/pool_size", "value": 20},
    {"op": "add", "path": "/features/new_flag", "value": true},
    {"op": "remove", "path": "/deprecated/old_setting"}
  ],
  "old_version": "abc123",
  "new_version": "def456",
  "timestamp": "2025-01-15T10:35:00Z"
}
```

---

**Anterior**: [Historia 002 - Broadcast de Cambios](./story-002-change-broadcast.md)
**Siguiente**: [Historia 004 - Reconexion y Heartbeat](./story-004-reconnection.md)
