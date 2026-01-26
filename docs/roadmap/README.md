# Vortex API Client — Roadmap de Implementación

> **Vortex** es un cliente de APIs de escritorio construido con Rust + Slint, diseñado para reemplazar y superar a Postman con rendimiento nativo, privacidad total y flujos Git-friendly.

Este roadmap divide la implementación en sprints independientes. Cada sprint contiene especificaciones técnicas completas con código Rust, componentes Slint y criterios de aceptación detallados, diseñados para que un agente de IA pueda implementarlos sin ambigüedad.

---

## Documentación de Producto

| Documento | Propósito |
|-----------|-----------|
| [00-product-vision.md](./00-product-vision.md) | Visión, propuesta de valor, personas, OKRs |
| [01-competitive-analysis.md](./01-competitive-analysis.md) | Análisis vs Postman, Insomnia, Bruno |
| [02-file-format-spec.md](./02-file-format-spec.md) | Especificación del formato de archivos |
| [03-ui-ux-specification.md](./03-ui-ux-specification.md) | Diseño UI/UX, layouts, shortcuts, colores |
| [milestones.md](./milestones.md) | Hitos de control evolutivo |

---

## Sprints de Implementación

| Sprint | Objetivo | Milestone |
|--------|----------|-----------|
| [Sprint 00](./sprint-00-foundations.md) | Arquitectura base y workspace multi-crate | M0 |
| [Sprint 01](./sprint-01-mvp-execution-ui.md) | MVP: ejecutar requests HTTP y ver respuestas | M1 |
| [Sprint 02](./sprint-02-persistence-collections.md) | Persistencia en archivos Git-friendly | M2 |
| [Sprint 03](./sprint-03-environments-variables.md) | Variables y entornos con resolución | M3 |
| [Sprint 04](./sprint-04-postman-import.md) | Importar colecciones Postman v2 | M4 |
| [Sprint 05](./sprint-05-auth-body-advanced.md) | OAuth2, form-data, TLS avanzado | M5 |
| [Sprint 06](./sprint-06-tests-runner-reporting.md) | Tests automatizados y runner batch | M6 |
| [Sprint 07](./sprint-07-extensibility.md) | Plugins y abstracción de protocolos | M7 |

---

## Tech Stack

| Componente | Tecnología |
|------------|------------|
| Lenguaje | Rust 1.93+ |
| UI Framework | Slint 1.9+ |
| HTTP Client | reqwest + tokio |
| Serialización | serde + serde_json |
| OAuth2 | oauth2 crate |
| JSON Path | serde_json_path |

---

## Arquitectura

```
workspace/
├── crates/
│   ├── vortex-domain        # Tipos de negocio puros
│   ├── vortex-application   # Casos de uso y ports
│   ├── vortex-infrastructure # Adapters (HTTP, FS, OAuth)
│   └── vortex-ui            # Slint components + view models
├── app/                     # Entrypoint desktop
└── plugins/                 # Plugins de ejemplo
```

---

## Para Agentes de IA

Cada sprint contiene:

1. **Structs Rust completos** con atributos serde
2. **Traits (ports)** para arquitectura hexagonal
3. **Código Slint** para componentes UI
4. **Tests unitarios** de ejemplo
5. **Orden de implementación** con dependencias
6. **Criterios de aceptación** verificables

Recomendación: implementar en orden (Sprint 00 → 07) respetando las dependencias entre tareas.

---

## Control de Versiones

Cada sprint debe completarse en una rama dedicada:

```bash
git checkout -b sprint-00-foundations
# ... implementar ...
git checkout main && git merge sprint-00-foundations
```
