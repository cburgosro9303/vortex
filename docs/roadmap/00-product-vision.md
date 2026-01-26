# Vortex API Client — Visión del Producto

## Nombre del Producto
**Vortex** — Cliente de APIs veloz, privado y versionable.

> "Vortex" evoca velocidad y potencia. El nombre es corto, memorable y disponible como dominio.

---

## Declaración de Visión

**Para** desarrolladores y equipos técnicos
**Que** necesitan probar, documentar y automatizar APIs
**Vortex** es un cliente de APIs de escritorio
**Que** ofrece rendimiento nativo, privacidad total y flujos versionables en Git
**A diferencia de** Postman, Insomnia o Bruno
**Nuestro producto** es 100% offline-first, sin cuentas obligatorias, con archivos legibles y rendimiento superior gracias a Rust y Slint.

---

## Propuesta de Valor Única (UVP)

### Por qué Vortex supera a Postman

| Aspecto | Postman | Vortex |
|---------|---------|--------|
| **Rendimiento** | Electron (300+ MB RAM) | Rust nativo (~50 MB RAM) |
| **Privacidad** | Requiere cuenta, sincroniza en nube | 100% offline, sin cuenta |
| **Versionable** | JSON complejo, diffs ilegibles | Archivos limpios, Git-friendly |
| **Costo** | Freemium agresivo, features bloqueadas | Open source, sin límites |
| **Arranque** | 5-10 segundos | <1 segundo |
| **Scripts** | JavaScript (inseguro) | WASM sandbox (seguro) |
| **Extensibilidad** | API cerrada | Plugins abiertos |

### Diferenciadores Clave

1. **Velocidad Nativa** — Rust + Slint = arranque instantáneo, UI a 60fps
2. **Privacidad por Diseño** — Sin telemetría, sin cuenta, sin nube
3. **Git-Native** — Colecciones como código, PRs para APIs
4. **Seguridad Real** — TLS estricto, secretos en keychain, scripts en sandbox
5. **Extensible** — Plugins para protocolos, auth y formatos

---

## Público Objetivo (Personas)

### Persona 1: Developer Backend (Ana)
- **Rol:** Desarrolladora backend senior
- **Frustraciones con Postman:**
  - Lento al abrir proyectos grandes
  - Colecciones no versionan bien en Git
  - Obliga a crear cuenta para features básicas
- **Necesidades:**
  - Probar endpoints rápidamente
  - Compartir colecciones vía Git
  - Variables por entorno (dev/staging/prod)
- **Quote:** "Solo quiero hacer un GET sin esperar 10 segundos"

### Persona 2: QA Engineer (Carlos)
- **Rol:** Ingeniero de QA automatizado
- **Frustraciones con Postman:**
  - Newman (CLI) es lento y consume mucha memoria
  - Tests JavaScript difíciles de mantener
  - Reportes poco útiles para CI
- **Necesidades:**
  - Runner de colecciones rápido
  - Asserts simples y legibles
  - Reportes JSON/JUnit para CI
- **Quote:** "Necesito correr 200 tests en <30 segundos"

### Persona 3: Tech Lead (María)
- **Rol:** Líder técnica de equipo
- **Frustraciones con Postman:**
  - Sincronización rompe colecciones del equipo
  - Conflictos de merge imposibles de resolver
  - Costo por usuario en equipos grandes
- **Necesidades:**
  - Colecciones como código en monorepo
  - Code review de cambios en APIs
  - Sin costos por usuario
- **Quote:** "Quiero que las colecciones vivan junto al código"

### Persona 4: Security Engineer (Diego)
- **Rol:** Ingeniero de seguridad
- **Frustraciones con Postman:**
  - Tokens se filtran en workspaces compartidos
  - Scripts JS pueden hacer cualquier cosa
  - Difícil auditar qué datos salen
- **Necesidades:**
  - Secretos en keychain del OS
  - Scripts en sandbox sin acceso a red/FS
  - Logs de auditoría local
- **Quote:** "No confío en herramientas que guardan mis tokens"

---

## Objetivos del Producto (OKRs)

### O1: Reemplazar Postman para uso individual
- KR1: MVP funcional en 8 semanas
- KR2: Importar 95% de colecciones Postman v2 sin pérdida
- KR3: Tiempo de arranque <1 segundo

### O2: Habilitar flujos colaborativos vía Git
- KR1: Archivos con diffs legibles (líneas cambiadas, no bloques)
- KR2: Merge de colecciones sin conflictos binarios
- KR3: Documentación de formato público

### O3: Superar performance de alternativas
- KR1: Uso de RAM <100 MB con 1000 requests cargadas
- KR2: Ejecución de request <50ms overhead
- KR3: UI a 60fps constante

---

## Principios de Diseño

1. **Offline-First** — Todo funciona sin conexión
2. **Git-Native** — Los archivos son el producto
3. **Keyboard-First** — Todo accesible con atajos
4. **Privacy-First** — Sin telemetría, sin tracking
5. **Performance-First** — Cada milisegundo cuenta
6. **Security-First** — Principio de mínimo privilegio

---

## Estrategia de Lanzamiento

### Fase Alpha (Sprints 0-2)
- Target: Developers individuales aventureros
- Canal: GitHub releases, dev.to, Reddit r/rust

### Fase Beta (Sprints 3-5)
- Target: Equipos pequeños migrando de Postman
- Canal: Product Hunt, Hacker News

### Fase GA (Sprints 6-7)
- Target: Adopción empresarial
- Canal: Documentación completa, workshops

---

## Métricas de Éxito

| Métrica | Target Alpha | Target GA |
|---------|--------------|-----------|
| Descargas | 1,000 | 50,000 |
| GitHub Stars | 500 | 5,000 |
| Issues resueltos | 80% <7 días | 90% <3 días |
| Colecciones importadas | 100 | 10,000 |
| Contribuidores | 5 | 50 |

---

## Riesgos Estratégicos

| Riesgo | Impacto | Mitigación |
|--------|---------|------------|
| Postman mejora offline | Alto | Diferenciarnos en velocidad y Git |
| Bruno gana tracción | Medio | Features avanzadas (GraphQL, gRPC) |
| Slint limitaciones UI | Alto | Evaluación temprana, fallback a egui |
| Adopción lenta | Medio | Importador Postman impecable |

---

## Roadmap Visual

```
        Alpha                    Beta                      GA
    ┌─────────────┐        ┌─────────────┐         ┌─────────────┐
    │ Sprint 0-2  │   →    │ Sprint 3-5  │    →    │ Sprint 6-7  │
    │             │        │             │         │             │
    │ • Fundación │        │ • Variables │         │ • Tests     │
    │ • MVP HTTP  │        │ • Import    │         │ • Runner    │
    │ • Colección │        │ • Auth      │         │ • Plugins   │
    └─────────────┘        └─────────────┘         └─────────────┘
         M0-M2                  M3-M5                   M6-M7
```
