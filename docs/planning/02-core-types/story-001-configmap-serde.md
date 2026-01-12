# Historia 001: Jerarquía de Tipos Dinámicos con Serde

## 🎓 Objetivo Educativo

Aprender a modelar datos dinámicos (como JSON) en un lenguaje estáticamente tipado como Rust, utilizando **Enums con Datos** y el framework de serialización **Serde**.

## CONTEXTO: El Problema del Tipado Estático vs. Datos Dinámicos

En Java, a menudo usamos `Map<String, Object>` para config genérica. `Object` puede ser cualquier cosa.
En Rust, **no existe `Object`**. Rust necesita saber el tamaño y tipo de todo en tiempo de compilación.

¿Cómo representamos entonces un JSON que puede tener strings, números, booleanos o arrays mezclados?
**Solución**: Usamos un **Enum** (Sum Type).

```rust
// Java: Cualquier cosa es un Object
Object val = "hello";

// Rust: Debemos enumerar explícitamente qué puede ser
enum ConfigValue {
    String(String),
    Integer(i64),
    Boolean(bool)
}
```

## 🎯 Alcance Técnico

1. Actualizar `ConfigMap` para usar `IndexMap` en lugar de `HashMap`.
2. Crear el enum `ConfigValue` para soportar tipos JSON completos.
3. Implementar `Serialize` y `Deserialize` para soportar conversión automática.

## 🧠 Conceptos Clave

### 1. IndexMap vs HashMap

En configuración, **el orden importa**. Si un usuario escribe un archivo YAML, espera que al guardarlo se mantenga el orden de las claves.

- `HashMap`: No garantiza orden (depende del hash).
- `IndexMap`: Mantiene orden de inserción (como `LinkedHashMap` en Java) pero con performance cercana a HashMap.

### 2. Serde `untagged`

Por defecto, Serde serializa un enum en Rust así: `{"String": "valor"}` (External tagging).
Para configuración, queremos que sea transparente: `"valor"`.
Usamos `#[serde(untagged)]` para decirle a Serde: "Intenta encajar el valor en alguna de las variantes, no uses el nombre del enum".

### 3. Tipos Recursivos

Un `ConfigValue` puede ser un `Object`, y ese `Object` contiene `ConfigValue`s.

```rust
enum ConfigValue {
    // ...
    Object(IndexMap<String, ConfigValue>) // Recursión!
}
```

Rust permite esto porque `IndexMap` (al igual que `Vec` o `Box`) almacena los datos en el **Heap**, por lo que el tamaño del enum en el **Stack** es conocido y fijo (el tamaño del puntero).

## 📝 Especificación de Tipos

### `ConfigValue` (Enum)

Ubicación: `crates/vortex-core/src/config/value.rs`

Debe soportar:

- `Null`
- `Bool(bool)`
- `Integer(i64)`
- `Float(f64)`
- `String(String)`
- `Array(Vec<ConfigValue>)`
- `Object(IndexMap<String, ConfigValue>)`

### `ConfigMap` (Struct)

Ubicación: `crates/vortex-core/src/config/map.rs` (Refactorizar existente)

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigMap {
    #[serde(flatten)]
    inner: IndexMap<String, ConfigValue>,
}
```

> **Nota sobre `#[serde(flatten)]`**: Esto hace que el JSON de `ConfigMap` sea directamente el objeto JSON inner, eliminando el nivel de anidamiento del struct wrapper.

## ✅ Criterios de Aceptación

- [ ] `ConfigValue` implementado con todas las variantes JSON.
- [ ] `ConfigMap` refactorizado para usar `IndexMap<String, ConfigValue>`.
- [ ] Parsear JSON: `{"a": 1, "b": "text", "c": [true, false]}` funciona correctamente.
- [ ] Serializar `ConfigMap` preserva el orden de inserción de claves.
- [ ] Métodos helper implementados: `as_str()`, `as_i64()`, `is_null()`.
- [ ] Acceso por path (`get("server.port")`) funciona para estructuras anidadas.

## 🧪 Guía de Implementación (Paso a Paso)

### Paso 1: Dependencias

Agregar a `crates/vortex-core/Cargo.toml`:

```toml
[dependencies]
indexmap = { version = "2.0", features = ["serde"] }
ordered-float = "4.0" # Opcional, útil para Hash de floats
```

### Paso 2: Implementar `ConfigValue`

Crear `src/config/value.rs`.
Implementar `From` traits para facilitar la creación (ej. `From<String> for ConfigValue`).

### Paso 3: Refactorizar `ConfigMap`

Modificar `src/config.rs` (probablemente renombrar a `map.rs` y mover a carpeta `config/`).
Implementar navegación recursiva para `get(path)`.

```rust
// Pista para navegación recursiva
pub fn get_at_path(&self, path: &str) -> Option<&ConfigValue> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = /* referencia al root map */;
    
    // Iterar parts y profundizar...
}
```

## ⚠️ Riesgos y Errores Comunes

1. **Ambigüedad Numérica**: `serde_json` puede parsear números como `u64`, `i64` o `f64`. Asegurar que `ConfigValue` capture correctamente la distinción o normalizar.
2. **Recursión Infinita en Display**: Si implementas `Display` manualmente, cuidado con la recursión.
3. **Comparación de Floats**: `PartialEq` para floats es tricky (`NaN != NaN`). Considerar usar wrappers si se necesita total ordering.

---
**Siguiente**: [Historia 002 - Merge Recursivo](./story-002-property-source.md)
