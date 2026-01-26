# Vortex — Hitos del Producto

> Control evolutivo del desarrollo de Vortex API Client.

Los hitos permiten medir el avance y decidir si el producto está listo para avanzar a la siguiente fase. Cada hito tiene criterios de salida objetivos y verificables.

## M0 — Arquitectura base lista
**Incluye:** Sprint 00

- Workspace multi-crate funcionando
- Contratos entre capas definidos
- UI vacía con Slint renderizando
- CI local mínima (fmt + clippy + tests)

**Criterio de salida:** el proyecto compila y corre en desktop sin funcionalidad de negocio.

---

## M1 — MVP funcional (requests básicas)
**Incluye:** Sprint 01

- Editor mínimo de request
- Ejecución HTTP async
- Visualización de respuesta

**Criterio de salida:** un usuario puede hacer una request y ver status/body.

---

## M2 — Persistencia versionable
**Incluye:** Sprint 02

- Guardar y abrir colecciones en archivos planos
- Estructura estable para Git

**Criterio de salida:** el usuario guarda y reabre sin perder datos.

---

## M3 — Entornos y variables
**Incluye:** Sprint 03

- Variables con scopes
- Resolución automática en URL/headers/body

**Criterio de salida:** cambiar de entorno cambia la request sin editarla.

---

## M4 — Importación Postman
**Incluye:** Sprint 04

- Importación de colecciones y ambientes Postman v2

**Criterio de salida:** importar una colección real y ejecutarla.

---

## M5 — Auth avanzada + bodies completos
**Incluye:** Sprint 05

- OAuth2 básico
- x-www-form-urlencoded + multipart/form-data

**Criterio de salida:** ejecutar requests con OAuth y bodies complejos.

---

## M6 — Testing y runner
**Incluye:** Sprint 06

- Tests simples y runner batch
- Reporte JSON

**Criterio de salida:** ejecutar colección completa con reporte reproducible.

---

## M7 — Extensibilidad inicial
**Incluye:** Sprint 07

- APIs de plugin/protocolos definidas
- Plugin de ejemplo cargado

**Criterio de salida:** extensión mínima funcional.
