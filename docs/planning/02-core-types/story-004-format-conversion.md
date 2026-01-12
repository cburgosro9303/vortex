# Historia 004: Conversión de Formatos (Polimorfismo con Traits)

## 🎓 Objetivo Educativo

Entender cómo Rust utiliza **Traits** para definir comportamiento compartido (Polimorfismo), implementar parsers manuales usando iteradores, y manejar errores de conversión complejos con `thiserror`.

## CONTEXTO: La Torre de Babel de la Configuración

Nuestros usuarios pueden guardar configuración en JSON, YAML o Properties. Vortex debe entenderlos todos (`Parser`) y poder escribirlos todos (`Serializer`).

En Java, usaríamos una interfaz:

```java
interface ConfigFormat {
    ConfigMap parse(String input);
    String serialize(ConfigMap config);
}
```

En Rust, definimos esto mediante **Traits**. Además, la conversión de tipos (`TryFrom`) es un ciudadano de primera clase en el lenguaje.

## 🎯 Alcance Técnico

1. Definir traits `FormatParser` y `FormatSerializer`.
2. Implementar soporte para JSON y YAML (wrappers sobre librerías existentes).
3. Implementar soporte para `.properties` (parser manual, ya que no hay crates estándar robustos que soporten nuestro modelo exacto).
4. Exponer conversiones vía `TryFrom`.

## 🧠 Conceptos Clave

### 1. Traits como Interfaces

Definiremos un trait que abstrae la lógica de parseo.

```rust
pub trait FormatParser {
    fn parse(&self, content: &str) -> VariableResult<ConfigMap>;
}
```

Esto nos permite tener un `Vec<Box<dyn FormatParser>>` o usar genéricos para aceptar cualquier formato.

### 2. Parsing Manual con Iteradores

Para el formato `.properties`, operaremos sobre `content.lines()`.
Usaremos combinadores como `filter_map`, `split_once`, y `trim` para procesar el texto de manera eficiente y legible, evitando loops `for` estilo C.

### 3. Error Handling Contextual

Si el JSON falla, queremos saber *dónde*. Si el YAML falla, igual.
Usaremos `thiserror` para envolver errores de terceros (`serde_json::Error`) en nuestro propio tipo `VortexError::ParseError`.

## 📝 Especificación

### Traits

Ubicación: `crates/vortex-core/src/format/mod.rs`

### Implementaciones

- `JsonFormat`: Usa `serde_json`.
- `YamlFormat`: Usa `serde_yaml`.
- `PropertiesFormat`: Implementación custom.
  - Debe soportar claves anidadas `server.port=8080`.
  - Debe ignorar comentarios `#`.

### `Format` Enum

Es útil tener un enum simple para selección:

```rust
pub enum Format { Json, Yaml, Properties }
```

## ✅ Criterios de Aceptación

- [ ] JSON round-trip (`Map -> JSON -> Map`) funciona y preserva tipos.
- [ ] YAML round-trip funciona.
- [ ] Properties parser maneja claves punteadas y las convierte a objetos anidados.
- [ ] Properties parser ignora líneas vacías y comentarios.
- [ ] Errores de sintaxis retornan `VortexError::ParseError` detallado.

## 🧪 Guía de Implementación

### Paso 1: Módulo format

Estructura:

```
src/format/
  mod.rs (Traits y Enum)
  json.rs
  yaml.rs
  properties.rs (Lógica compleja aquí)
```

### Paso 2: Properties Parser (El Reto)

El parser de properties debe:

1. Iterar líneas.
2. Dividir en primera ocurrencia de `=` o `:`.
3. Limpiar espacios (`trim()`).
4. Reconstruir la jerarquía. **Reto**: `props` es plano, `ConfigMap` es jerárquico. Necesitas una función `insert_path(map, key_path, value)`.

### Paso 3: Unificar errores

En `src/format/json.rs`:

```rust
impl FormatParser for JsonFormat {
    fn parse(&self, input: &str) -> Result<ConfigMap> {
        serde_json::from_str(input)
            .map_err(|e| VortexError::parse_error("json", e.to_string()))
    }
}
```

## ⚠️ Riesgos y Errores Comunes

1. **Ambigüedad en Properties**: `a.b=1` y `a=2`. ¿Qué es `a`? Un objeto o un escalar?
    - *Solución*: Último gana, o error. Para simplicidad, permitir sobrescritura (último gana).
2. **Tipos en Properties**: Todo es string en `.properties`.
    - *Decisión*: Al parsear properties, guardar todo como `ConfigValue::String`. La validación de tipos ocurrirá después (en tiempo de uso/binding), no en tiempo de parseo.

---
**Anterior**: [Historia 003 - Spring Format](./story-003-spring-format.md) | **Siguiente**: [Historia 005 - Core Testing](./story-005-core-testing.md)
