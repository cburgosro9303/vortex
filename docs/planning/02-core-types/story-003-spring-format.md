# Historia 003: Compatibilidad Spring Cloud Config (DTOs y Adaptadores)

## 🎓 Objetivo Educativo

Aprender a desacoplar el **Modelo de Dominio** del **Modelo de Representación** utilizando structs específicos para serialización ("DTOs") y adaptadores con Traits `From`, manteniendo el núcleo de la aplicación limpio de detalles de frameworks externos.

## CONTEXTO: Adaptándose al Mundo Exterior

El modelo de dominio de Vortex (`ConfigMap`) es ideal para lógica interna, pero el mundo exterior (clientes Spring Boot) espera un formato JSON muy específico, con convenciones de nombres distintas (`camelCase` vs `snake_case`) y metadatos adicionales.

En Java con Jackson, solíamos llenar nuestras clases de dominio con anotaciones `@JsonProperty`, `@JsonIgnore`, mezclando responsabilidades.
En Rust, preferimos crear **DTOs dedicados** (Data Transfer Objects) que "adaptan" el dominio justo antes de la salida.

## 🎯 Alcance Técnico

1. Definir structs `SpringConfigResponse` y `SpringPropertySource` que imiten exactamente la respuesta JSON de Spring Cloud Config.
2. Implementar la lógica de "aplanamiento" (Flattening): convertir objetos anidados (`{"a": {"b": 1}}`) en claves planas (`"a.b": 1`).
3. Implementar `From` traits para convertir limpiamente del dominio a los DTOs.

## 🧠 Conceptos Clave

### 1. DTO Pattern en Rust

En lugar de ensuciar `ConfigMap` con anotaciones para que parezca una respuesta Spring, creamos un struct separado:

```rust
// Dominio (Limpio)
struct PropertySource { ... }

// DTO (Solo para salida)
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SpringPropertySource { ... }
```

Esto permite cambiar el formato de salida sin tocar el dominio.

### 2. Aplanamiento de Mapas (Recursive Flattening)

Spring Config espera:

```json
{
  "propertySources": [
    {
      "name": "...",
      "source": { 
          "server.port": 8080,         // <-- Clave plana
          "database.host": "localhost" // <-- Clave plana
      }
    }
  ]
}
```

Pero nuestro modelo interno es jerárquico. Necesitamos un algoritmo recursivo que transforme el árbol en una lista plana de claves punteadas.

### 3. Serde Renaming

Rust usa `snake_case` por convención. JSON/Java usa `camelCase`.
Serde maneja esto automáticamente con `#[serde(rename_all = "camelCase")]` a nivel de struct, evitando renombrar campo por campo.

## 📝 Especificación

### `SpringConfigResponse` (Struct)

Ubicación: `crates/vortex-core/src/format/spring.rs`

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpringConfigResponse {
    pub name: String,
    pub profiles: Vec<String>,
    pub label: Option<String>,
    pub version: Option<String>,
    pub state: Option<String>,
    #[serde(rename = "propertySources")] // Excepción explícita si necesario
    pub property_sources: Vec<SpringPropertySource>,
}
```

### Algoritmo Flatten

Debe recorrer el `IndexMap<String, ConfigValue>`, acumulando el prefijo de las claves.

- Input: `{"server": {"port": 8080}}`
- Output: `{"server.port": 8080}`

## ✅ Criterios de Aceptación

- [ ] Structs de respuesta definidos con `camelCase`.
- [ ] Función `flatten` implementada correctamente (recursiva).
- [ ] Conversión `From<MergedConfig> for SpringConfigResponse`.
- [ ] JSON generado es idéntico al de un servidor Spring Cloud Config real (verificado con tests).
- [ ] `version` y `state` se omiten del JSON si son `None` (`skip_serializing_if`).

## 🧪 Guía de Implementación

### Paso 1: Definir los DTOs

Crear `src/format/spring.rs`. Usar los atributos de Serde para ajustar la salida.

### Paso 2: Implementar Flattening

```rust
fn flatten_value(prefix: &str, value: &ConfigValue, target: &mut IndexMap<String, ConfigValue>) {
    match value {
        ConfigValue::Object(map) => {
            for (k, v) in map {
                let new_key = if prefix.is_empty() { k.clone() } else { format!("{}.{}", prefix, k) };
                flatten_value(&new_key, v, target);
            }
        }
        _ => {
            target.insert(prefix.to_string(), value.clone());
        }
    }
}
```

### Paso 3: Implementar Traits `From`

```rust
impl From<&PropertySource> for SpringPropertySource {
    fn from(src: &PropertySource) -> Self {
        SpringPropertySource {
            name: src.name.clone(),
            source: flatten(&src.config), // Usar la función helper
        }
    }
}
```

## ⚠️ Riesgos y Errores Comunes

1. **Arrays**: Spring generalmente no aplana arrays (ej. `arr[0]=val`), sino que los deja como valores JSON si el cliente lo soporta, O usa notación de índices. **Decisión**: Por ahora, dejar arrays como valores JSON en el mapa aplanado.
2. **Valores Nulos**: `skip_serializing_if = "Option::is_none"` es crucial para no enviar `null` explícitos que podrían confundir a clientes antiguos.

---
**Anterior**: [Historia 002 - Merge Recursivo](./story-002-property-source.md) | **Siguiente**: [Historia 004 - Conversión Formatos](./story-004-format-conversion.md)
