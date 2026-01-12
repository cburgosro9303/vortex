# Épica 02: Core Types y Serialización Avanzada

## 🎯 Objetivo Educativo y Técnico

Esta épica tiene un doble propósito:

1. **Técnico**: Evolucionar el modelo de dominio "plano" (`HashMap<String, String>`) creado en la Épica 01 hacia un modelo jerárquico robusto (`ConfigValue` recursivo) capaz de representar JSON/YAML complejos.
2. **Educativo**: Enseñar patrones avanzados de Rust como Enums con datos (Sum Types), Serialización custom con Serde, Ownership en estructuras recursivas y Traits de conversión.

## 🏗 Contexto Arquitectónico

En la Épica 01, creamos un `ConfigMap` básico. Ahora necesitamos que soporte la complejidad del mundo real definida en el PRD: estructuras anidadas (e.g., `datasource.hikari.max-pool-size`), arrays, y tipos mixtos.

### Evolución del Modelo

| Característica | Épica 01 (Foundation) | Épica 02 (Core Types) | Por qué el cambio |
|----------------|-----------------------|-----------------------|-------------------|
| **Estructura** | `HashMap<String, String>` | `IndexMap<String, ConfigValue>` | Necesitamos anidamiento (`json objects`) y preservar orden. |
| **Tipos** | Solo `String` | `Null`, `Bool`, `Int`, `Float`, `String`, `Array`, `Map` | Config real tiene tipos (boolean flags, puertos numéricos). |
| **Merge** | Sobrescritura simple | Deep Merge Recursivo | Cambiar una clave en un objeto anidado no debe borrar el resto del objeto. |
| **Formato** | N/A | Spring Cloud Config JSON | Compatibilidad con clientes existentes. |

## 📚 Conceptos de Rust a Aprender

Esta épica es intensiva en el sistema de tipos de Rust.

### Nivel Intermedio

| Concepto | Dónde se aplica | Explicación para Javas |
|----------|-----------------|------------------------|
| **Enums (Sum Types)** | `ConfigValue` | A diferencia de los Enums de Java, en Rust un Enum puede contener datos distintos en cada variante. Es como una `sealed interface` con `records` en Java 17+. |
| **Recursive Types** | `ConfigValue::Object` | Definir un tipo que se contiene a sí mismo (un mapa que contiene valores que pueden ser mapas). Requiere manejo cuidadoso de memoria (Indirection). |
| **Derive Macros** | `#[derive(Serialize)]` | Generación de código en compilación. Similar a Lombok, pero más poderoso y seguro. |
| **Serde Attributes** | `#[serde(flatten)]` | Control fino de cómo se mapea el JSON a structs sin escribir parsers manuales. |

### Nivel Avanzado

| Concepto | Dónde se aplica | Explicación |
|----------|-----------------|-------------|
| **Zero-cost Abstractions** | Iteradores | Usar `map`, `filter`, `fold` compila a código ensamblador tan eficiente como un loop `for` manual. |
| **Traits `From` / `TryFrom`** | Conversión de Tipos | Mecanismo estándar de Rust para convertir valores (ej. de JSON a nuestro tipo interno). |
| **IndexMap vs HashMap** | `ConfigMap` | Por qué el Hashing estándar no garantiza orden y cuándo pagar el costo extra de mantener índices. |

## 🛠 Historias de Usuario

| ID | Título | Foco de Aprendizaje |
|----|--------|---------------------|
| [001](./story-001-configmap-serde.md) | **Jerarquía de Tipos con Serde** | Creación de Enums recursivos (`ConfigValue`), `IndexMap` y uso avanzado de Serde (`untagged`, `flatten`). |
| [002](./story-002-property-source.md) | **Lógica de Merge Recursivo (Deep Merge)** | Implementación de algoritmos recursivos en Rust, manejo de Ownership (`clone` vs `borrow`) y referencias mutables. |
| [003](./story-003-spring-format.md) | **Compatibilidad Spring Cloud** | Mapeo de estructuras complejas a formatos JSON específicos usando structs intermedios (DTO pattern). |
| [004](./story-004-format-conversion.md) | **Conversión de Formatos (Properties/YAML)** | Implementación de Traits `From`/`Into` y manejo de errores de parsing. |

## ✅ Criterios de Aceptación Globales

1. **Soporte de Tipos**: Poder representar un JSON arbitrario complexo dentro de `ConfigMap`.
2. **Orden Determinista**: Serializar `ConfigMap` siempre produce el mismo JSON (mismo orden de claves).
3. **Round-trip Safety**: `deserialize(serialize(x)) == x`.
4. **Deep Merge Correcto**: Combinar dos configuraciones anidadas preserva valores no conflictivos.

## 📦 Dependencias Técnicas

```toml
[dependencies]
# Serialización
serde = { version = "1.0", features = ["derive", "rc"] }
serde_json = "1.0"
serde_yaml = "0.9"

# Estructuras de datos
indexmap = { version = "2.0", features = ["serde"] } # HashMap con orden garantizado

# Utilidades
thiserror = "1.0"
```

---
**Siguiente Paso**: Comenzar con la [Historia 001](./story-001-configmap-serde.md) para construir el motor de tipos.
