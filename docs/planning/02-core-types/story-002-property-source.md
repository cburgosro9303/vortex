# Historia 002: Lógica de Merge Recursivo (Deep Merge)

## 🎓 Objetivo Educativo

Entender e implementar algoritmos de **Deep Merge** en una estructura de datos recursiva, manejando referencias inmutables (`&`) y mutables (`&mut`) de manera segura con el Borrow Checker de Rust.

## CONTEXTO: Shallow vs Deep Merge

Cuando combinamos dos configuraciones, ¿qué sucede con los objetos anidados?

### Shallow Merge (Lo que hace `HashMap::extend`)

Reemplaza todo el valor de la clave.

```json
Base:    {"server": {"port": 80, "host": "localhost"}}
Overlay: {"server": {"port": 443}}
Result:  {"server": {"port": 443}}  // ❌ PERDIMOS "host"!
```

### Deep Merge (Lo que necesitamos)

Fusiona recursivamente los objetos.

```json
Base:    {"server": {"port": 80, "host": "localhost"}}
Overlay: {"server": {"port": 443}}
Result:  {"server": {"port": 443, "host": "localhost"}}  // ✅ CORRECTO
```

## 🎯 Alcance Técnico

1. Definir `PropertySource` con metadatos (prioridad, origen).
2. Implementar algoritmo de Deep Merge para `ConfigValue`.
3. Crear `PropertySourceList` para manejar múltiples fuentes ordenadas.

## 🧠 Conceptos Clave

### 1. Recursión en Estructuras

El algoritmo debe detectar si ambos valores (base y overlay) son `ConfigValue::Object`.

- Si **ambos** son objetos -> Llamada recursiva mergeando sus claves.
- Si **uno no lo es** -> El overlay reemplaza totalmente al base.
- Si es **Array** -> Generalmente se reemplaza el array completo (comportamiento Spring Cloud por defecto), aunque a veces se desea append (fuera de scope por ahora).

### 2. Ownership en el Merge

Para el merge, ¿debemos consumir los valores originales (`self`) o solo leerlos (`&self`)?

- Generalmente queremos crear una **nueva** configuración resultante sin destruir las fuentes originales (para poder inspeccionarlas después).
- Esto implica usar `clone()` frecuentemente, lo cual es aceptable en tiempo de inicio de la aplicación, pero debemos ser conscientes del costo de memoria.

### 3. Orden de Prioridad

La precedencia es crítica.
`Sistema > Argumentos CLI > Archivo de Perfil > Archivo Base > Defaults`
Implementaremos una lista ordenada donde el último elemento aplicado sobrescribe a los anteriores.

## 📝 Especificación

### `PropertySource` (Struct)

Ubicación: `crates/vortex-core/src/config/source.rs`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertySource {
    pub name: String,
    pub priority: i32, // Mayor número = gana
    pub config: ConfigMap,
    // Futuro: source_type, location, etc.
}
```

### Algoritmo de Merge

Ubicación: `crates/vortex-core/src/merge.rs` (o módulo similar)

Debe seguir la regla:

1. Iterar sobre claves del overlay.
2. Si la clave no existe en base -> Insertar (clone).
3. Si existe y ambos son Maps -> Deep Merge.
4. Si existe y no son ambos Maps -> Overlay reemplaza Base.

## ✅ Criterios de Aceptación

- [ ] `PropertySource` definido con metadatos.
- [ ] Función `merge(base: &ConfigValue, overlay: &ConfigValue) -> ConfigValue`.
- [ ] Test: Merge de configuraciones anidadas preserva claves no conflictivas.
- [ ] Test: Primitivos se sobrescriben.
- [ ] Test: Arrays se sobrescriben (no append).
- [ ] `PropertySourceList` aplica fuentes en orden correcto de prioridad.

## 🧪 Guía de Implementación

### Paso 1: Módulo Merge

Crear `src/merge/mod.rs` y `src/merge/strategy.rs`.

### Paso 2: Implementar Deep Merge

```rust
pub fn deep_merge(base: &mut ConfigMap, overlay: &ConfigMap) {
    for (key, val) in overlay.inner.iter() {
        match base.inner.get_mut(key) {
            Some(existing) => {
                // Si ambos son objetos, recursión
                if let (ConfigValue::Object(b), ConfigValue::Object(o)) = (existing, val) {
                    merge_maps(b, o);
                } else {
                    // Si no, reemplazo total
                    *existing = val.clone();
                }
            }
            None => {
                base.inner.insert(key.clone(), val.clone());
            }
        }
    }
}
```

*Nota: Este es un ejemplo simplificado. Deberás manejar los tipos concretos.*

## ⚠️ Riesgos y Errores Comunes

1. **Iteración y Mutación**: Rust no permite modificar una colección mientras la iteras. El algoritmo anterior evita esto iterando `overlay` y mutando `base` (son colecciones distintas).
2. **Ciclos**: Los JSON configuración no suelen tener ciclos, pero teóricamente un grafo podría. Asumimos estructura de árbol (DAG) sin ciclos.

---
**Anterior**: [Historia 001 - Jerarquía de Tipos](./story-001-configmap-serde.md) | **Siguiente**: [Historia 003 - Compatibilidad Spring](./story-003-spring-format.md)
