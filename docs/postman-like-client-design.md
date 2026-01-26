# Vortex API Client — Documento de Diseño Principal

> **Vortex** es un cliente de APIs de escritorio construido con **Rust 1.93+ y Slint**, diseñado para **reemplazar y superar a Postman** con rendimiento nativo, privacidad total y flujos Git-friendly.

## Propuesta de Valor

| Aspecto | Postman | **Vortex** |
|---------|---------|------------|
| Rendimiento | Electron (300+ MB RAM) | **Rust nativo (~50 MB)** |
| Privacidad | Requiere cuenta, nube | **100% offline, sin cuenta** |
| Versionable | JSON complejo | **Archivos Git-friendly** |
| Arranque | 5-10 segundos | **<1 segundo** |
| Costo | Freemium agresivo | **Open source** |

---

## Documentación Detallada

Para especificaciones completas, ver el directorio `roadmap/`:

- **[roadmap/README.md](roadmap/README.md)** — Índice del roadmap
- **[roadmap/00-product-vision.md](roadmap/00-product-vision.md)** — Visión, personas, OKRs
- **[roadmap/01-competitive-analysis.md](roadmap/01-competitive-analysis.md)** — Análisis competitivo
- **[roadmap/02-file-format-spec.md](roadmap/02-file-format-spec.md)** — Formato de archivos
- **[roadmap/03-ui-ux-specification.md](roadmap/03-ui-ux-specification.md)** — Especificación UI/UX

---

## Objetivo

Construir una aplicación de escritorio multiplataforma enfocada en **ejecución, prueba y depuración de APIs** (REST inicialmente, con base para gRPC/WebSockets), **sin sincronización en nube ni cuentas de usuario**. El almacenamiento será **offline-first** en **archivos planos** versionables (Git). La interfaz debe ser **intuitiva, profesional y no sobresaturada**.

---

## 0) Supuestos y límites

- Sin cuentas, sin sincronización en nube, sin colaboración multiusuario.
- Persistencia en archivos planos; idealmente **differences Git limpios**.
- Primer enfoque: REST HTTP/HTTPS. Dejar base para gRPC/WebSockets.
- UI en Slint; lógica en Rust con separación estricta.
- Seguridad: priorizar **seguridad local** (secretos y TLS) y **seguridad de ejecución** (scripts/plugins).

---

## 1) Alcance funcional por etapas

### 1.1 MVP (núcleo utilitario)
**Meta:** ejecutar requests HTTP y visualizar resultados de forma clara.

- Crear requests HTTP: método, URL, headers, query params, body (raw JSON/plain text)
- Ejecutar request (async) con feedback de loading
- Ver respuesta: status, headers, body, tiempo
- Historial local simple (por sesión)
- Guardar y abrir colecciones **en archivo plano**
- UI mínima usable (lista de requests + editor + panel de respuesta)

### 1.2 V1 (productividad real)
**Meta:** flujo completo para trabajo diario.

- Colecciones con carpetas, duplicar requests, renombrar, mover
- Entornos y variables ({{var}}) con resolución en request
- Persistencia robusta en disco (versionable en Git)
- Importar colecciones y ambientes desde Postman (JSON)
- Exportar a formato propio versionable
- Autenticación básica: API key, Bearer, Basic
- Mejor UX: tabs, atajos, estados visuales claros

### 1.3 V2 (paridad útil con Postman para requests)
**Meta:** cobertura funcional para la mayoría de casos de uso.

- Body: form-data, x-www-form-urlencoded
- Auth: OAuth 2.0 (client credentials / auth code)
- Pre-request / Post-response scripts (limitado/sandboxed)
- Tests automatizados por request (asserts básicos)
- Variables por scope: global / collection / environment
- Soporte de certificados, TLS settings, timeouts, retries

### 1.4 V3 (avanzado y extensible)
**Meta:** plataforma extensible y completa para request testing.

- gRPC / WebSockets (base arquitectónica)
- Plugin system (protocolos, auth, importers)
- Runner de colecciones con reportes
- Mock servers básicos (opcional)
- CLI complementaria para ejecutar colecciones

---

## 2) Principios de diseño

- **Separación estricta**: UI ↔ dominio ↔ infraestructura
- **Asincronía segura**: nada bloquea la UI
- **Archivos planos versionables**: sin DB central por ahora
- **Tipos fuertes** para request/response/estado
- **Extensibilidad**: protocolos y plugins futuros
- **UX sin saturación**: menos controles visibles, más acciones contextuales

---

## 3) Arquitectura propuesta (workspace multi-crate)

```
workspace
├─ crates
│  ├─ domain           # tipos de negocio: request, response, env, collection
│  ├─ application      # casos de uso: ejecutar, guardar, importar
│  ├─ infrastructure   # http, filesystem, parsers, importers
│  ├─ ui               # Slint, view models, bindings
│  └─ cli (opcional)   # runner de colecciones / utilidades
└─ app (bin)           # entrypoint desktop
```

### 3.1 domain (tipos expresivos)
- `RequestSpec`
  - `Method`, `Url`, `Headers`, `QueryParams`, `Body`
- `ResponseSpec`
  - `Status`, `Headers`, `Body`, `Duration`, `Size`
- `Auth`
  - `None | ApiKey | Bearer | Basic | OAuth2`
- `Environment`
  - Vars (key/value), secrets
- `Collection`
  - Tree (folders, requests), metadata

### 3.2 application (casos de uso)
- `ExecuteRequest`
- `SaveCollection` / `LoadCollection`
- `ImportFromPostman`
- `ResolveVariables`
- `RunCollection` (batch)
- `ValidateRequest` / `Normalize`

### 3.3 infrastructure
- HTTP: `reqwest` / `hyper`
- FS: almacenamiento en archivos planos (JSON/YAML/TOML)
- Importers: Postman v2 JSON
- Test runner: asserts simples

### 3.4 UI (Slint)
- Shell principal con layout 3 columnas
- Panel central: editor de request
- Panel inferior: response viewer
- Tabs para múltiples requests abiertos

---

## 4) Persistencia en archivos planos

**Objetivo:** versionar colecciones en Git.

### 4.1 Formato de almacenamiento
- `collection.json` + `environment.json`
- Estructura simple, estable, con versionado de schema

Ejemplo básico:
```json
{
  "schema": 1,
  "name": "My Collection",
  "items": [
    { "type": "request", "id": "r1", "name": "List Users", "request": { ... } }
  ]
}
```

### 4.2 Organización sugerida
```
collections/
  MyProject/
    collection.json
    environments/
      dev.json
      prod.json
```

### 4.3 Consideraciones
- Archivos legibles para humanos
- Fields ordenados para diffs limpios
- IDs estables (UUID)
- Separar secretos del contenido versionado (ver sección de seguridad)

---

## 5) Importación de Postman

- Parser de colecciones Postman v2
- Mapeo:
  - `item` → folder/request
  - `request.url`, `header`, `body`
  - `auth` → Auth
  - `variable` → Environment
- Conversión a formato local
- **Validación** de inputs malformados

---

## 6) UX/UI (visión de un especialista UI/UX)

### 6.1 Principios visuales
- Jerarquía clara: **acción primaria** (Send), **contexto** (colección), **resultado** (response)
- Densidad controlada: mostrar detalles solo cuando se necesitan
- Escalabilidad: listas y panels preparados para grandes colecciones

### 6.2 Layout propuesto
- **Sidebar**: colecciones, búsqueda, entornos
- **Área central**: editor de request
- **Área inferior**: respuesta (tabs: Body, Headers, Meta)
- **Tabs superiores**: múltiples requests abiertos
- **Panel contextual**: Auth, Params, Headers, Body (switch tabs)

### 6.3 Flujos clave
- Crear request rápido: `Ctrl+N` → nombre → URL → Send
- Duplicar request: `Ctrl+D`
- Ejecutar: `Ctrl+Enter`
- Cambiar entorno: selector superior con preview de variables

### 6.4 Accesibilidad y productividad
- Navegación por teclado completa
- Focus states visibles
- Atajos configurables
- Soporte para temas (claro/oscuro) sin saturar

### 6.5 Feedback y estados
- Loading animado no intrusivo
- Error humanizado con hints
- Response truncation con “ver más” para grandes payloads

---

## 7) Seguridad (visión de un especialista en ciberseguridad)

### 7.1 Riesgos principales
- Exposición de secretos en archivos o Git
- Requests a destinos inseguros (TLS débil, MITM)
- Importaciones maliciosas (Postman con payloads extremos)
- Scripts con acceso peligroso (si se habilitan)

### 7.2 Controles obligatorios (MVP+)
- **Redacción de secretos** en logs y UI
- Variables sensibles en **archivo separado** no versionado (`.gitignore` recomendado)
- TLS estricto por defecto (bloquear certs inválidos)
- Timeout y límites de tamaño de respuesta
- Validación de importadores (limitar tamaño, profundidad y campos)

### 7.3 Controles avanzados (V2+)
- Soporte para **mTLS** (certificados cliente)
- Proxies configurables
- Opción explícita para “insecure requests” con warning
- Sandboxing para scripts (sin FS ni red salvo la request actual)

### 7.4 Prácticas recomendadas
- Secretos en **keychain local** opcional (OS)
- Cifrado de archivos sensibles
- Política de error segura (no mostrar tokens en errores)
- Auditoría local de accesos a tokens

---

## 8) Plan técnico detallado (epics → historias → tareas)

### Fase 0: Fundaciones técnicas
**Epic:** Base del workspace y arquitectura limpia

- Historia: estructura multi-crate y contratos
  - Tarea: crear workspace y crates
  - Tarea: configurar CI local (fmt, clippy, tests)
  - Tarea: definir módulos y boundaries

**Criterio de aceptación:** compila y ejecuta una app vacía con Slint y crates desacoplados.

---

### Fase 1: MVP funcional
**Epic:** ejecución y visualización básica

- Historia: RequestSpec + ResponseSpec
  - Tarea: definir tipos base y serialización
  - Tarea: implementar normalización de URL
- Historia: ejecutar HTTP
  - Tarea: `ExecuteRequest` async con `reqwest`
  - Tarea: timeouts y métricas
- Historia: UI básica
  - Tarea: layout 3 columnas
  - Tarea: editor minimal de request
  - Tarea: panel de respuesta

**Criterio de aceptación:** puedo crear una request, ejecutarla y ver status + body.

---

### Fase 2: Persistencia y colecciones
**Epic:** almacenamiento en archivos planos versionables

- Historia: formato de colección
  - Tarea: schema v1
  - Tarea: serialización estable (orden de campos)
- Historia: guardar / cargar
  - Tarea: `SaveCollection` / `LoadCollection`
  - Tarea: selección de carpeta local

**Criterio de aceptación:** guardar colección en disco y reabrirla sin pérdida de datos.

---

### Fase 3: Entornos y variables
**Epic:** productividad real

- Historia: Environment
  - Tarea: definir scopes
  - Tarea: resolver {{var}} en URL/headers/body
- Historia: UI variables
  - Tarea: panel de entorno
  - Tarea: preview de resolución

**Criterio de aceptación:** cambiar de entorno modifica la request sin edición manual.

---

### Fase 4: Importación Postman
**Epic:** compatibilidad

- Historia: parser Postman v2
  - Tarea: mapear estructura item → request
  - Tarea: mapear auth
  - Tarea: mapear variables

**Criterio de aceptación:** importar una colección real y ejecutarla.

---

### Fase 5: Autenticación avanzada + tests
**Epic:** utilidad completa

- Historia: OAuth2
  - Tarea: flujo client credentials
  - Tarea: flujo auth code (opcional)
- Historia: asserts
  - Tarea: framework básico de tests
  - Tarea: UI de resultados

**Criterio de aceptación:** ejecutar requests con OAuth y validar status/body con asserts.

---

### Fase 6: Extensibilidad
**Epic:** diseño a largo plazo

- Historia: arquitectura de plugins
  - Tarea: definir manifest + APIs
  - Tarea: sandbox de plugins

**Criterio de aceptación:** cargar un plugin de prueba.

---

## 9) Riesgos y mitigaciones

- **Complejidad del formato**: mantener schema versionado
- **UI saturada**: aplicar diseño minimal y contextual
- **Import Postman**: validar casos edge
- **Performance**: requests async + streaming de respuesta
- **Seguridad**: secretos fuera de Git + TLS estricto

---

## 10) Preguntas abiertas

1. ¿Prefieres formato de almacenamiento JSON, YAML o TOML?
2. ¿Quieres compatibilidad bidireccional con Postman (exportar en su formato)?
3. ¿Qué nivel de scripting deseas (JS completo o asserts básicos)?
4. ¿Se requiere soporte HTTPS avanzado (certs custom, proxy, mTLS)?
5. ¿Deseas CLI en paralelo desde el inicio o después del MVP?

---

Si quieres, puedo convertir este plan en un **roadmap ejecutable** con tareas para cada sprint o generar el **workspace inicial con crates y estructura base**.
