# Historia 005: Estrategia de Testing (Unitario vs Integración)

## 🎓 Objetivo Educativo

Dominar las convenciones de testing en Rust: la diferencia entre tests de unidad (inline, acceso privado) y tests de integración (`tests/` directory, acceso público), uso de fixtures, y aserciones idiomáticas.

## CONTEXTO: ¿Quién vigila a los vigilantes?

Un sistema de configuración es crítico; un error aquí rompe toda la infraestructura. Necesitamos garantías fuertes.
En Rust, el testing es ciudadano de primera clase integrado en `cargo`.

### Unit Tests (Caja Blanca)

Se escriben en el mismo archivo que el código, en un módulo auxiliar `#[cfg(test)]`.

- Tienen acceso a funciones privadas.
- Prueban la lógica interna (ej. algoritmo de merge, parseo de líneas individuales).

### Integration Tests (Caja Negra)

Se escriben en la carpeta `tests/` en la raíz del crate.

- Solo pueden usar lo que es `pub`.
- Vemos el crate "desde afuera", como lo haría un usuario.
- Prueban flujos completos (ej. Cargar config -> Merge -> Exportar a JSON).

## 🎯 Alcance Técnico

1. Crear suite de **Unit Tests** para cada módulo core (`config`, `merge`, `format`).
2. Crear **Integration Tests** que simulen casos de uso reales (Spring Cloud simulation).
3. Implementar **Test Fixtures** (datos de prueba reutilizables) en un módulo común.

## 🧠 Conceptos Clave

### 1. `#[cfg(test)]` y Compilación Condicional

Rust no compila el código de test en el binario final de producción.

```rust
#[cfg(test)]
mod tests { ... }
```

Esto significa cero overhead en release.

### 2. Test Fixtures y Módulos Comunes

En tests de integración, es común necesitar setups complejos repetidos.
Rust trata cada archivo en `tests/*.rs` como un crate separado. Para compartir código, usamos un módulo `common/mod.rs` y lo importamos en cada test.

### 3. Aserciones

Más allá de `assert_eq!`, aprenderemos a testear:

- Pánicos esperados: `#[should_panic]`
- Resultados de error: `assert!(result.is_err())`
- Matches complejos: `matches!(val, ConfigValue::String(_))`

## 📝 Especificación

### Tests Unitarios (Inline)

- `src/config/value.rs`: Testear creación y conversión de `ConfigValue`.
- `src/merge.rs`: Testear colisiones, arrays y anidamiento profundo.
- `src/format/properties.rs`: Testear casos extremos de parsing (espacios, escapes).

### Tests Integración (`tests/`)

- `tests/spring_compatibility.rs`: Cargar JSON real de Spring Cloud Config y verificar que `vortex-core` lo procesa idénticamente.
- `tests/format_roundtrip.rs`: Serializar -> Deserializar -> Serializar. El output debe ser estable.

## ✅ Criterios de Aceptación

- [ ] Cobertura de tests > 80% (medible con `cargo-tarpaulin` o similar, aunque la métrica es referencial).
- [ ] Módulo `tests/common` implementado para fixtures JSON/YAML.
- [ ] Test de "Deep Merge" cubre al menos 3 niveles de anidamiento.
- [ ] Test de "Round Trip" para todos los formatos (JSON, YAML, Properties).

## 🧪 Guía de Implementación

### Paso 1: Unit Tests

Ir archivo por archivo agregando:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn it_works() { ... }
}
```

### Paso 2: Common Fixtures

Crear `tests/common/mod.rs`:

```rust
pub fn complex_config_fixture() -> ConfigMap {
    // Retorna un objeto complejo hardcodeado o cargado de archivo
}
```

### Paso 3: Integration Suites

Crear `tests/merge_semantics.rs`:

```rust
mod common; // Importar módulo común

#[test]
fn test_complex_overrides() {
    let base = common::base_config();
    let overlay = common::prod_profile();
    let result = merge(&base, &overlay);
    // Verificar reglas de negocio
}
```

## ⚠️ Riesgos y Errores Comunes

1. **Tests Flaky**: Tests que dependen de orden de HashMaps.
    - *Solución*: `ConfigMap` usa `IndexMap`, por lo que el orden es determinista. Fundamental para tests estables.
2. **Sobre-testing de implementación**: Testear demasiados detalles privados hace el refactoring difícil.
    - *Consejo*: Preferir tests de integración (públicos) sobre unitarios (privados) cuando se prueba comportamiento, no algoritmos.

---
**Anterior**: [Historia 004 - Conversión Formatos](./story-004-format-conversion.md) | **Fin de Épica 02**
