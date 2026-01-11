# Epica 08: Real-time y WebSockets

## Objetivo

Implementar capacidades real-time en Vortex Config mediante WebSockets, permitiendo que los clientes reciban notificaciones instantaneas cuando las configuraciones cambian. Esta epica agrega:

1. **WebSocket Endpoint**: Conexiones persistentes para clientes que necesitan updates en tiempo real
2. **Broadcast de Cambios**: Sistema pub/sub para notificar cambios de configuracion
3. **Diff Semantico**: Envio eficiente de solo las diferencias, minimizando ancho de banda
4. **Reconexion y Heartbeat**: Manejo robusto de conexiones inestables
5. **Suite de Tests**: Testing completo para escenarios real-time

Las capacidades real-time son fundamentales para:
- **Hot reload de configuraciones**: Aplicaciones que necesitan recargar sin reinicio
- **Dashboards de monitoreo**: Visualizacion en tiempo real de estados de configuracion
- **Sistemas distribuidos**: Sincronizacion de configuraciones entre nodos

---

## Conceptos de Rust Cubiertos (Nivel Avanzado)

| Concepto | Historia | Comparacion con Java |
|----------|----------|---------------------|
| tokio::sync::broadcast | 002 | PublishProcessor (Reactor) |
| Async Streams (Stream trait) | 001, 002 | Flux (Reactor) |
| Pin y Unpin | 001, 003 | No tiene equivalente directo |
| Graceful shutdown patterns | 004 | Shutdown hooks |
| WebSocket upgrade | 001 | Jakarta WebSocket @ServerEndpoint |
| tokio::select! | 004 | CompletableFuture.anyOf() |
| Arc<RwLock<T>> patterns | 002 | ReadWriteLock |
| Async drop considerations | 004 | try-with-resources (limitado) |
| Weak references | 002 | WeakReference |
| Timeout futures | 004 | CompletableFuture.orTimeout() |

---

## Historias de Usuario

| # | Titulo | Descripcion | Puntos |
|---|--------|-------------|--------|
| 001 | [WebSocket Endpoint](./story-001-websocket-endpoint.md) | Establecer conexiones WS para clientes | 5 |
| 002 | [Broadcast de Cambios](./story-002-change-broadcast.md) | Notificar a clientes cuando config cambia | 5 |
| 003 | [Diff Semantico](./story-003-semantic-diff.md) | Calcular y enviar solo diferencias | 5 |
| 004 | [Reconexion y Heartbeat](./story-004-reconnection.md) | Manejar clientes desconectados | 5 |
| 005 | [Tests de WebSockets](./story-005-websocket-tests.md) | Suite de tests para real-time | 3 |

**Total**: 23 puntos de historia

---

## Dependencias

### Epicas Prerequisito

| Epica | Razon |
|-------|-------|
| 03 - HTTP Server | Servidor Axum funcionando con WebSocket support |
| 05 - Cache Config | Sistema de invalidacion que genera eventos de cambio |

### Dependencias de Crates

```toml
[dependencies]
# HTTP y WebSockets
axum = { version = "0.7", features = ["ws"] }
tokio = { version = "1", features = ["full", "sync"] }
tokio-stream = "0.1"
futures = "0.3"

# Serializacion
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Diff calculation
similar = "2.4"  # Para diff de texto
serde-diff = "0.4"  # Para diff estructurado

# Observabilidad
tracing = "0.1"
metrics = "0.22"

# Errores
thiserror = "1"

# Utilidades
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }

[dev-dependencies]
tokio-tungstenite = "0.21"
tokio-test = "0.4"
futures-util = "0.3"
```

---

## Criterios de Aceptacion

### Funcionales

- [ ] Endpoint `/ws/{app}/{profile}` acepta conexiones WebSocket
- [ ] Clientes reciben mensaje inicial con configuracion actual
- [ ] Cambios de configuracion se broadcast a todos los clientes suscritos
- [ ] Mensajes incluyen diff semantico (solo cambios)
- [ ] Heartbeat cada 30 segundos mantiene conexion viva
- [ ] Clientes pueden reconectarse con estado desde ultimo mensaje
- [ ] Suscripcion por patron soportada (ej: `myapp:*`)

### No Funcionales

- [ ] Latencia de broadcast p99 < 50ms desde cambio hasta cliente
- [ ] Soporte para 1000+ conexiones concurrentes por instancia
- [ ] Memory footprint < 1KB por conexion idle
- [ ] Reconexion automatica < 5 segundos
- [ ] Graceful shutdown drena conexiones en < 30 segundos

### Compatibilidad

- [ ] Protocolo compatible con clientes WebSocket estandar
- [ ] Mensajes en formato JSON
- [ ] Soporte para clientes que no soportan compression
- [ ] Headers de autenticacion via query params o subprotocol

---

## Definition of Done

- [ ] Codigo compila sin warnings (`cargo build --all-features`)
- [ ] Formateado con `cargo fmt`
- [ ] Sin errores de clippy (`cargo clippy -- -D warnings`)
- [ ] Tests unitarios pasan con cobertura > 80%
- [ ] Tests de integracion WebSocket pasan
- [ ] Rustdoc para todas las APIs publicas
- [ ] Changelog actualizado
- [ ] Sin `unwrap()` en codigo de produccion
- [ ] Metricas de conexiones expuestas
- [ ] Logs estructurados con tracing
- [ ] CI pipeline verde
- [ ] Load test con 1000 conexiones pasa

---

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigacion |
|--------|--------------|---------|------------|
| Memory leak por conexiones zombies | Media | Alto | Timeouts agresivos, cleanup periodico |
| Thundering herd en reconexion | Alta | Medio | Jitter aleatorio, backoff exponencial |
| Broadcast storm por cambios frecuentes | Media | Alto | Debouncing, rate limiting |
| Deadlock en broadcast channel | Baja | Critico | Usar try_send, bounded channels |
| Compatibilidad con proxies/LBs | Media | Medio | Documentar timeouts, ping/pong |
| Serialization overhead en diffs | Baja | Medio | Benchmarks, caching de diffs |

---

## Decisiones Arquitectonicas (ADRs)

### ADR-001: Axum WebSocket vs tungstenite Directo

**Estado**: Aceptado

**Contexto**: Necesitamos soporte WebSocket integrado con nuestro servidor HTTP existente.

**Decision**: Usar `axum::extract::ws` para WebSockets.

**Razones**:
- Integracion nativa con Axum routing
- Manejo automatico de upgrade HTTP -> WS
- Comparte middleware con endpoints HTTP
- Tokio-native async

**Alternativas consideradas**:
- `tungstenite` directo: Mas control pero mas trabajo de integracion
- `warp` WebSocket: Requiere cambiar framework HTTP
- `actix-web` WebSocket: Ecosistema diferente

### ADR-002: Broadcast Channel para Pub/Sub

**Estado**: Aceptado

**Contexto**: Necesitamos distribuir mensajes de cambio a multiples clientes.

**Decision**: Usar `tokio::sync::broadcast` para pub/sub interno.

**Razones**:
- Multi-producer, multi-consumer
- Cada subscriber recibe su copia del mensaje
- Backpressure automatica (lagged receivers)
- Integrado con Tokio runtime

**Diagrama**:
```
┌─────────────────────────────────────────────────────────────────┐
│                     ConfigChangeEmitter                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Config Update ───► broadcast::Sender ───┬──► WS Client 1       │
│                           │              ├──► WS Client 2       │
│                           │              ├──► WS Client 3       │
│                           │              └──► WS Client N       │
│                           │                                      │
│                      (cloned receivers)                          │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### ADR-003: Diff Semantico con serde-diff

**Estado**: Aceptado

**Contexto**: Enviar configuraciones completas es ineficiente; queremos enviar solo cambios.

**Decision**: Usar `serde-diff` para calcular diferencias estructuradas.

**Razones**:
- Trabaja con cualquier struct que implemente Serialize
- Diffs compactos y tipados
- Aplicable en cliente para reconstruir estado
- Mejor que diff de texto para datos estructurados

**Ejemplo de Diff**:
```json
{
  "type": "config_change",
  "app": "myapp",
  "profile": "prod",
  "diff": [
    {"op": "replace", "path": "/database/pool_size", "value": 20},
    {"op": "add", "path": "/features/new_flag", "value": true}
  ],
  "version": "abc123",
  "timestamp": "2025-01-15T10:30:00Z"
}
```

### ADR-004: Heartbeat con Ping/Pong

**Estado**: Aceptado

**Contexto**: Conexiones WebSocket pueden morir silenciosamente detras de proxies/NAT.

**Decision**: Implementar heartbeat bidireccional con ping/pong frames.

**Razones**:
- Detecta conexiones muertas rapidamente
- Mantiene conexion viva a traves de proxies
- Estandar WebSocket (RFC 6455)
- Bajo overhead (2 bytes por ping)

**Configuracion**:
- Ping del servidor cada 30 segundos
- Timeout si no hay pong en 10 segundos
- Cliente puede iniciar ping tambien

---

## Reglas Estrictas

1. **No bloquear el broadcast**: Usar `try_send` o bounded channels para evitar backpressure
2. **Timeout en todas las operaciones**: Nunca esperar indefinidamente en WebSocket ops
3. **Cleanup en drop**: Conexiones deben limpiarse automaticamente al cerrar
4. **IDs unicos por conexion**: Cada conexion tiene UUID para tracking
5. **Mensajes idempotentes**: Clientes pueden recibir mismo mensaje dos veces
6. **Versionado de mensajes**: Incluir version en schema para compatibilidad
7. **Rate limiting por cliente**: Maximo 100 mensajes/segundo por conexion
8. **Graceful shutdown**: Enviar close frame antes de terminar

---

## Estructura del Crate

```
crates/vortex-server/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── ws/
│   │   ├── mod.rs              # Re-exports
│   │   ├── handler.rs          # WebSocket upgrade handler
│   │   ├── connection.rs       # Connection state machine
│   │   ├── messages.rs         # Message types (WsMessage enum)
│   │   ├── broadcaster.rs      # Broadcast channel management
│   │   ├── diff.rs             # Semantic diff calculation
│   │   ├── heartbeat.rs        # Ping/pong logic
│   │   └── registry.rs         # Active connections registry
│   ├── handlers/
│   │   └── ...
│   └── middleware/
│       └── ...
└── tests/
    ├── ws_connection_test.rs
    ├── ws_broadcast_test.rs
    ├── ws_reconnection_test.rs
    └── helpers/
        └── ws_client.rs        # Test WebSocket client
```

---

## Diagrama de Arquitectura WebSocket

```
                    ┌─────────────────────────────────────────────┐
                    │              HTTP Request                    │
                    │         GET /ws/myapp/prod                   │
                    │         Upgrade: websocket                   │
                    └─────────────────┬───────────────────────────┘
                                      │
                    ┌─────────────────▼───────────────────────────┐
                    │           WebSocket Handler                  │
                    │         (upgrade & authenticate)             │
                    └─────────────────┬───────────────────────────┘
                                      │
                    ┌─────────────────▼───────────────────────────┐
                    │         Connection Registry                  │
                    │    ┌────────────────────────────────┐       │
                    │    │  HashMap<ConnectionId, Sender>  │       │
                    │    └────────────────────────────────┘       │
                    └─────────────────┬───────────────────────────┘
                                      │
           ┌──────────────────────────┼──────────────────────────┐
           │                          │                          │
┌──────────▼──────────┐    ┌─────────▼─────────┐    ┌──────────▼──────────┐
│   Outbound Loop     │    │  Inbound Loop     │    │   Heartbeat Loop    │
│                     │    │                   │    │                     │
│  broadcast::Rx ─►WS │    │  WS ─►commands    │    │  ticker ─►ping      │
│                     │    │                   │    │  WS ─►pong/timeout  │
└─────────────────────┘    └───────────────────┘    └─────────────────────┘
           │                          │                          │
           └──────────────────────────┼──────────────────────────┘
                                      │
                    ┌─────────────────▼───────────────────────────┐
                    │            tokio::select!                    │
                    │    (run all loops concurrently)              │
                    └─────────────────────────────────────────────┘
```

---

## Diagrama de Flujo de Mensajes

```
┌──────────────────────────────────────────────────────────────────────────┐
│                        Message Flow                                       │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  1. Config Change Detected                                                │
│     └──► ConfigSource.refresh() finds changes                            │
│                                                                           │
│  2. Diff Calculation                                                      │
│     └──► DiffCalculator.diff(old_config, new_config)                     │
│         └──► Returns Vec<DiffOp>                                         │
│                                                                           │
│  3. Event Emission                                                        │
│     └──► ConfigChangeEvent { app, profile, diff, version }               │
│         └──► broadcaster.send(event)                                     │
│                                                                           │
│  4. Broadcast to Subscribers                                              │
│     └──► For each connection subscribed to app:profile                   │
│         └──► Filter by subscription pattern                              │
│             └──► Serialize to JSON                                       │
│                 └──► ws.send(Message::Text(json))                        │
│                                                                           │
│  5. Client Processing                                                     │
│     └──► Client receives JSON                                            │
│         └──► Applies diff to local state                                 │
│             └──► Triggers application callback                           │
│                                                                           │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## Protocolo de Mensajes

### Mensajes del Servidor al Cliente

```typescript
// Initial config after connection
{
  "type": "config_snapshot",
  "app": "myapp",
  "profile": "prod",
  "label": "main",
  "config": { /* full config */ },
  "version": "abc123",
  "timestamp": "2025-01-15T10:30:00Z"
}

// Config change notification
{
  "type": "config_change",
  "app": "myapp",
  "profile": "prod",
  "diff": [
    {"op": "replace", "path": "/server/port", "value": 9090}
  ],
  "old_version": "abc123",
  "new_version": "def456",
  "timestamp": "2025-01-15T10:35:00Z"
}

// Heartbeat ping
{
  "type": "ping",
  "timestamp": "2025-01-15T10:30:30Z"
}

// Error notification
{
  "type": "error",
  "code": "SUBSCRIPTION_FAILED",
  "message": "Invalid app/profile pattern",
  "timestamp": "2025-01-15T10:30:00Z"
}
```

### Mensajes del Cliente al Servidor

```typescript
// Subscribe to additional patterns
{
  "type": "subscribe",
  "patterns": ["myapp:staging:*", "shared:*:*"]
}

// Unsubscribe
{
  "type": "unsubscribe",
  "patterns": ["myapp:staging:*"]
}

// Heartbeat pong
{
  "type": "pong",
  "timestamp": "2025-01-15T10:30:30Z"
}

// Request full config (after reconnect)
{
  "type": "resync",
  "last_version": "abc123"
}
```

---

## Changelog

| Version | Fecha | Cambios |
|---------|-------|---------|
| 0.1.0 | 2025-01-XX | Creacion inicial de la epica |

---

## Referencias

- [Axum WebSocket Documentation](https://docs.rs/axum/latest/axum/extract/ws/index.html)
- [Tokio Broadcast Channel](https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html)
- [RFC 6455 - WebSocket Protocol](https://tools.ietf.org/html/rfc6455)
- [serde-diff Documentation](https://docs.rs/serde-diff)
- [Spring Cloud Bus](https://docs.spring.io/spring-cloud-bus/docs/current/reference/html/) - Similar pattern in Spring
- [Tokio Select Macro](https://tokio.rs/tokio/tutorial/select)
