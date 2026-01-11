# Historia 002: Broadcast de Cambios

## Contexto y Objetivo

Con el endpoint WebSocket establecido en la historia 001, los clientes pueden conectarse y recibir la configuracion inicial. Ahora necesitamos notificarles cuando las configuraciones cambian.

El patron pub/sub (publish-subscribe) es ideal para esto:
- **Publisher**: El sistema de invalidacion de cache (Epica 05) emite eventos de cambio
- **Subscribers**: Las conexiones WebSocket activas reciben estos eventos

Esta historia implementa:
- Un broadcaster central que distribuye cambios a conexiones suscritas
- Un registry de conexiones activas por patron de suscripcion
- Integracion con el sistema de invalidacion existente

Para desarrolladores Java, esto es similar a usar un `EventBus` (Guava) o `ApplicationEventPublisher` (Spring), pero con el modelo async de Tokio y broadcast channels.

---

## Alcance

### In Scope

- `ConnectionRegistry`: Registro de conexiones activas
- `ConfigChangeBroadcaster`: Sistema de broadcast usando `tokio::sync::broadcast`
- Integracion con `InvalidationService` de Epica 05
- Filtrado de mensajes por patron de suscripcion
- Manejo de subscribers lentos (lagged)
- Metricas de broadcast

### Out of Scope

- Diff semantico del contenido (historia 003)
- Heartbeat y reconexion (historia 004)
- Persistencia de mensajes para reconexion
- Clustering/distribucion entre nodos

---

## Criterios de Aceptacion

- [ ] Cambios de config se broadcast a clientes suscritos
- [ ] Clientes solo reciben cambios de sus suscripciones (app:profile:label)
- [ ] Suscripcion por patron funciona (ej: `myapp:*:*`)
- [ ] Clientes lentos (lagged) reciben notificacion de mensajes perdidos
- [ ] Latencia de broadcast p99 < 50ms
- [ ] Metricas: mensajes enviados, clientes notificados, lag events

---

## Diseno Propuesto

### Estructura de Modulos

```
crates/vortex-server/src/ws/
├── mod.rs
├── handler.rs           # Existente
├── connection.rs        # Existente
├── messages.rs          # Existente
├── registry.rs          # Nuevo: ConnectionRegistry
└── broadcaster.rs       # Nuevo: ConfigChangeBroadcaster
```

### Interfaces Principales

```rust
/// Registro de conexiones activas
pub struct ConnectionRegistry {
    /// Conexiones por ID
    connections: DashMap<ConnectionId, ConnectionHandle>,
    /// Indice por patron para lookup rapido
    pattern_index: DashMap<String, HashSet<ConnectionId>>,
}

/// Handle para enviar mensajes a una conexion
pub struct ConnectionHandle {
    pub id: ConnectionId,
    pub info: ConnectionInfo,
    pub sender: mpsc::Sender<ServerMessage>,
}

/// Broadcaster de cambios de configuracion
pub struct ConfigChangeBroadcaster {
    /// Channel para recibir eventos de cambio
    change_rx: broadcast::Receiver<ConfigChangeEvent>,
    /// Registry de conexiones
    registry: Arc<ConnectionRegistry>,
}

/// Evento de cambio de configuracion
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    pub app: String,
    pub profile: String,
    pub label: String,
    pub old_config: Option<serde_json::Value>,
    pub new_config: serde_json::Value,
    pub version: String,
    pub timestamp: DateTime<Utc>,
}
```

---

## Pasos de Implementacion

### Paso 1: Implementar ConnectionRegistry

```rust
// src/ws/registry.rs
use std::collections::HashSet;
use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::mpsc;
use tracing::{info, warn, instrument};

use super::connection::{ConnectionId, ConnectionInfo};
use super::messages::ServerMessage;

/// Handle para enviar mensajes a una conexion especifica
#[derive(Debug, Clone)]
pub struct ConnectionHandle {
    pub id: ConnectionId,
    pub info: ConnectionInfo,
    /// Channel para enviar mensajes a esta conexion
    sender: mpsc::Sender<ServerMessage>,
}

impl ConnectionHandle {
    pub fn new(info: ConnectionInfo, sender: mpsc::Sender<ServerMessage>) -> Self {
        Self {
            id: info.id,
            info,
            sender,
        }
    }

    /// Envia un mensaje a esta conexion
    pub async fn send(&self, msg: ServerMessage) -> Result<(), mpsc::error::SendError<ServerMessage>> {
        self.sender.send(msg).await
    }

    /// Intenta enviar sin bloquear (para broadcast)
    pub fn try_send(&self, msg: ServerMessage) -> Result<(), mpsc::error::TrySendError<ServerMessage>> {
        self.sender.try_send(msg)
    }
}

/// Registro central de conexiones WebSocket activas.
///
/// Thread-safe usando DashMap para acceso concurrente.
#[derive(Debug, Default)]
pub struct ConnectionRegistry {
    /// Todas las conexiones por ID
    connections: DashMap<ConnectionId, ConnectionHandle>,
    /// Indice de conexiones por key (app:profile:label)
    /// Para lookup rapido al broadcast
    key_index: DashMap<String, HashSet<ConnectionId>>,
    /// Indice de conexiones por patron de suscripcion
    /// Ej: "myapp:*:*" -> [conn1, conn2]
    pattern_index: DashMap<String, HashSet<ConnectionId>>,
}

impl ConnectionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra una nueva conexion
    #[instrument(skip(self, handle), fields(id = %handle.id))]
    pub fn register(&self, handle: ConnectionHandle) {
        let id = handle.id;
        let key = handle.info.cache_key();

        // Agregar al indice por key
        self.key_index
            .entry(key.clone())
            .or_default()
            .insert(id);

        // Agregar al mapa principal
        self.connections.insert(id, handle);

        info!(key = %key, "Connection registered");
    }

    /// Desregistra una conexion
    #[instrument(skip(self), fields(id = %id))]
    pub fn unregister(&self, id: ConnectionId) -> Option<ConnectionHandle> {
        let handle = self.connections.remove(&id);

        if let Some((_, ref h)) = handle {
            let key = h.info.cache_key();

            // Remover del indice por key
            if let Some(mut set) = self.key_index.get_mut(&key) {
                set.remove(&id);
                if set.is_empty() {
                    drop(set);
                    self.key_index.remove(&key);
                }
            }

            // Remover de indices de patrones
            self.pattern_index.iter_mut().for_each(|mut entry| {
                entry.value_mut().remove(&id);
            });

            info!(key = %key, "Connection unregistered");
        }

        handle.map(|(_, h)| h)
    }

    /// Suscribe una conexion a un patron adicional
    #[instrument(skip(self))]
    pub fn subscribe_pattern(&self, id: ConnectionId, pattern: &str) {
        self.pattern_index
            .entry(pattern.to_string())
            .or_default()
            .insert(id);

        info!(pattern = %pattern, "Connection subscribed to pattern");
    }

    /// Desuscribe una conexion de un patron
    pub fn unsubscribe_pattern(&self, id: ConnectionId, pattern: &str) {
        if let Some(mut set) = self.pattern_index.get_mut(pattern) {
            set.remove(&id);
        }
    }

    /// Encuentra todas las conexiones que deben recibir un evento.
    /// Incluye matches exactos y por patron.
    pub fn find_subscribers(&self, app: &str, profile: &str, label: &str) -> Vec<ConnectionHandle> {
        let mut subscribers = Vec::new();
        let key = format!("{}:{}:{}", app, profile, label);

        // Matches exactos por key
        if let Some(ids) = self.key_index.get(&key) {
            for id in ids.iter() {
                if let Some(handle) = self.connections.get(id) {
                    subscribers.push(handle.clone());
                }
            }
        }

        // Matches por patron
        for entry in self.pattern_index.iter() {
            let pattern = entry.key();
            if self.pattern_matches(pattern, app, profile, label) {
                for id in entry.value().iter() {
                    // Evitar duplicados
                    if !subscribers.iter().any(|h| h.id == *id) {
                        if let Some(handle) = self.connections.get(id) {
                            subscribers.push(handle.clone());
                        }
                    }
                }
            }
        }

        subscribers
    }

    /// Verifica si un patron coincide con app:profile:label
    fn pattern_matches(&self, pattern: &str, app: &str, profile: &str, label: &str) -> bool {
        let parts: Vec<&str> = pattern.split(':').collect();
        if parts.len() != 3 {
            return false;
        }

        let matches_part = |pattern_part: &str, value: &str| -> bool {
            pattern_part == "*" || pattern_part == value
        };

        matches_part(parts[0], app)
            && matches_part(parts[1], profile)
            && matches_part(parts[2], label)
    }

    /// Retorna el numero de conexiones activas
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Retorna todas las conexiones (para broadcast global)
    pub fn all_connections(&self) -> Vec<ConnectionHandle> {
        self.connections
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }
}
```

### Paso 2: Implementar ConfigChangeBroadcaster

```rust
// src/ws/broadcaster.rs
use std::sync::Arc;
use chrono::{DateTime, Utc};
use tokio::sync::broadcast;
use tracing::{info, warn, error, instrument, Span};

use super::registry::ConnectionRegistry;
use super::messages::ServerMessage;

/// Evento emitido cuando una configuracion cambia
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    pub app: String,
    pub profile: String,
    pub label: String,
    /// Configuracion anterior (None si es nueva)
    pub old_config: Option<serde_json::Value>,
    /// Nueva configuracion
    pub new_config: serde_json::Value,
    /// Version/hash del nuevo config
    pub version: String,
    /// Timestamp del cambio
    pub timestamp: DateTime<Utc>,
}

impl ConfigChangeEvent {
    pub fn new(
        app: impl Into<String>,
        profile: impl Into<String>,
        label: impl Into<String>,
        old_config: Option<serde_json::Value>,
        new_config: serde_json::Value,
        version: impl Into<String>,
    ) -> Self {
        Self {
            app: app.into(),
            profile: profile.into(),
            label: label.into(),
            old_config,
            new_config,
            version: version.into(),
            timestamp: Utc::now(),
        }
    }

    /// Genera la key para routing
    pub fn key(&self) -> String {
        format!("{}:{}:{}", self.app, self.profile, self.label)
    }
}

/// Broadcaster de cambios de configuracion a clientes WebSocket.
///
/// Recibe eventos del sistema de invalidacion y los distribuye
/// a todas las conexiones suscritas.
pub struct ConfigChangeBroadcaster {
    /// Sender del channel de eventos
    event_tx: broadcast::Sender<ConfigChangeEvent>,
    /// Registry de conexiones
    registry: Arc<ConnectionRegistry>,
}

impl ConfigChangeBroadcaster {
    /// Crea un nuevo broadcaster con capacidad especificada
    pub fn new(registry: Arc<ConnectionRegistry>, capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            event_tx: tx,
            registry,
        }
    }

    /// Obtiene un receiver para suscribirse a eventos
    pub fn subscribe(&self) -> broadcast::Receiver<ConfigChangeEvent> {
        self.event_tx.subscribe()
    }

    /// Emite un evento de cambio de configuracion
    #[instrument(skip(self, event), fields(key = %event.key()))]
    pub fn emit(&self, event: ConfigChangeEvent) -> Result<usize, broadcast::error::SendError<ConfigChangeEvent>> {
        info!(version = %event.version, "Emitting config change event");
        self.event_tx.send(event)
    }

    /// Inicia el loop de broadcast en background.
    /// Consume eventos y los envia a conexiones suscritas.
    pub fn start_broadcast_loop(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let broadcaster = self.clone();
        let mut rx = self.event_tx.subscribe();

        tokio::spawn(async move {
            info!("Broadcast loop started");

            loop {
                match rx.recv().await {
                    Ok(event) => {
                        broadcaster.broadcast_event(&event).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "Broadcast receiver lagged");
                        // Continuar procesando
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("Broadcast channel closed, stopping loop");
                        break;
                    }
                }
            }
        })
    }

    /// Broadcast un evento a todas las conexiones suscritas
    #[instrument(skip(self, event), fields(key = %event.key()))]
    async fn broadcast_event(&self, event: &ConfigChangeEvent) {
        let subscribers = self.registry.find_subscribers(
            &event.app,
            &event.profile,
            &event.label,
        );

        if subscribers.is_empty() {
            info!("No subscribers for this config");
            return;
        }

        info!(subscriber_count = subscribers.len(), "Broadcasting to subscribers");

        let message = self.create_message(event);

        let mut success_count = 0;
        let mut fail_count = 0;

        for handle in subscribers {
            match handle.try_send(message.clone()) {
                Ok(_) => {
                    success_count += 1;
                }
                Err(e) => {
                    fail_count += 1;
                    warn!(
                        connection_id = %handle.id,
                        error = %e,
                        "Failed to send to subscriber"
                    );
                    // La conexion sera limpiada por su propio loop
                }
            }
        }

        info!(
            success = success_count,
            failed = fail_count,
            "Broadcast complete"
        );

        // Registrar metricas
        // metrics::counter!("ws_broadcast_messages_total").increment(1);
        // metrics::counter!("ws_broadcast_recipients_total").increment(success_count as u64);
    }

    /// Crea el mensaje a enviar basado en el evento
    fn create_message(&self, event: &ConfigChangeEvent) -> ServerMessage {
        // Por ahora enviamos snapshot completo
        // La historia 003 implementara diff semantico
        ServerMessage::snapshot(
            &event.app,
            &event.profile,
            &event.label,
            event.new_config.clone(),
            &event.version,
        )
    }
}

/// Extension de AppState para incluir broadcaster
pub struct BroadcastState {
    pub broadcaster: Arc<ConfigChangeBroadcaster>,
    pub registry: Arc<ConnectionRegistry>,
}

impl BroadcastState {
    pub fn new(capacity: usize) -> Self {
        let registry = Arc::new(ConnectionRegistry::new());
        let broadcaster = Arc::new(ConfigChangeBroadcaster::new(
            Arc::clone(&registry),
            capacity,
        ));

        Self {
            broadcaster,
            registry,
        }
    }

    /// Inicia el sistema de broadcast
    pub fn start(&self) -> tokio::task::JoinHandle<()> {
        self.broadcaster.clone().start_broadcast_loop()
    }
}
```

### Paso 3: Integrar con InvalidationService

```rust
// src/cache/invalidation.rs (modificacion)
use crate::ws::broadcaster::ConfigChangeEvent;

impl InvalidationService {
    /// Invalida y emite evento de cambio para WebSocket
    pub async fn invalidate_and_notify(
        &self,
        key: &CacheKey,
        new_config: serde_json::Value,
        version: String,
        broadcaster: &ConfigChangeBroadcaster,
    ) -> InvalidationResult {
        // Obtener config anterior antes de invalidar
        let old_config = self.cache.get(key).await.map(|c| c.to_json_value());

        // Invalidar cache
        let result = self.invalidate_key(key).await;

        // Emitir evento para WebSocket
        let event = ConfigChangeEvent::new(
            &key.app,
            &key.profile,
            &key.label,
            old_config,
            new_config,
            version,
        );

        if let Err(e) = broadcaster.emit(event) {
            warn!(error = %e, "Failed to emit change event");
        }

        result
    }
}
```

### Paso 4: Actualizar WebSocket Handler

```rust
// src/ws/handler.rs (modificacion)
use super::registry::{ConnectionHandle, ConnectionRegistry};
use tokio::sync::mpsc;

#[instrument(skip(socket, state), fields(connection_id = %conn_info.id))]
async fn handle_socket(
    socket: WebSocket,
    mut conn_info: ConnectionInfo,
    state: AppState,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Crear channel para mensajes salientes
    let (msg_tx, mut msg_rx) = mpsc::channel::<ServerMessage>(100);

    // Registrar en el registry
    let handle = ConnectionHandle::new(conn_info.clone(), msg_tx);
    state.ws_registry.register(handle);

    conn_info.set_state(ConnectionState::Connected);

    // Enviar config inicial
    if let Err(e) = send_initial_config(&mut ws_sender, &conn_info, &state).await {
        error!(error = %e, "Failed to send initial config");
        state.ws_registry.unregister(conn_info.id);
        return;
    }

    // Loop principal con tres tareas concurrentes
    loop {
        tokio::select! {
            // Mensajes entrantes del WebSocket
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        conn_info.touch();
                        if let Err(e) = handle_text_message(&text, &mut ws_sender, &conn_info, &state).await {
                            warn!(error = %e, "Error handling message");
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!("Client closed connection");
                        break;
                    }
                    Some(Err(e)) => {
                        error!(error = %e, "WebSocket error");
                        break;
                    }
                    None => {
                        info!("WebSocket stream ended");
                        break;
                    }
                    _ => {}
                }
            }

            // Mensajes del broadcaster para enviar
            msg = msg_rx.recv() => {
                match msg {
                    Some(server_msg) => {
                        match server_msg.to_json() {
                            Ok(json) => {
                                if let Err(e) = ws_sender.send(Message::Text(json)).await {
                                    error!(error = %e, "Failed to send broadcast message");
                                    break;
                                }
                            }
                            Err(e) => {
                                error!(error = %e, "Failed to serialize message");
                            }
                        }
                    }
                    None => {
                        info!("Message channel closed");
                        break;
                    }
                }
            }
        }
    }

    // Cleanup
    state.ws_registry.unregister(conn_info.id);
    info!("Connection handler terminated");
}

/// Maneja mensaje de suscripcion a patrones adicionales
async fn handle_subscribe(
    patterns: Vec<String>,
    conn_info: &ConnectionInfo,
    state: &AppState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for pattern in patterns {
        // Validar patron
        if !is_valid_pattern(&pattern) {
            return Err(format!("Invalid pattern: {}", pattern).into());
        }
        state.ws_registry.subscribe_pattern(conn_info.id, &pattern);
    }
    Ok(())
}

fn is_valid_pattern(pattern: &str) -> bool {
    let parts: Vec<&str> = pattern.split(':').collect();
    if parts.len() != 3 {
        return false;
    }
    // Cada parte debe ser "*" o un identificador valido
    parts.iter().all(|p| *p == "*" || p.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'))
}
```

---

## Conceptos de Rust Aprendidos

### 1. tokio::sync::broadcast Channel

Broadcast channels permiten multiples receivers donde cada uno obtiene su copia del mensaje.

**Rust:**
```rust
use tokio::sync::broadcast;

// Crear channel con buffer de 100 mensajes
let (tx, _) = broadcast::channel::<ConfigChangeEvent>(100);

// Cada subscriber obtiene su propio receiver
let mut rx1 = tx.subscribe();
let mut rx2 = tx.subscribe();

// Enviar mensaje (todos los receivers lo obtienen)
tx.send(ConfigChangeEvent { ... })?;

// Cada receiver lo recibe independientemente
tokio::spawn(async move {
    while let Ok(event) = rx1.recv().await {
        println!("Receiver 1: {:?}", event);
    }
});

tokio::spawn(async move {
    while let Ok(event) = rx2.recv().await {
        println!("Receiver 2: {:?}", event);
    }
});
```

**Comparacion con Java (Reactor):**
```java
// Java con Project Reactor
Sinks.Many<ConfigChangeEvent> sink = Sinks.many().multicast().onBackpressureBuffer();
Flux<ConfigChangeEvent> flux = sink.asFlux();

// Cada subscriber
flux.subscribe(event -> System.out.println("Sub 1: " + event));
flux.subscribe(event -> System.out.println("Sub 2: " + event));

// Emitir
sink.tryEmitNext(new ConfigChangeEvent(...));
```

**Diferencias clave:**

| Aspecto | broadcast (Tokio) | Reactor Sinks |
|---------|-------------------|---------------|
| Backpressure | Lagged receivers | Configurable |
| Mensaje perdido | `RecvError::Lagged(n)` | Depende de estrategia |
| Clonacion | Clone del mensaje | Referencia compartida |
| Thread safety | Built-in | Scheduler-dependent |

### 2. DashMap para Concurrencia

`DashMap` es un HashMap thread-safe optimizado para lectura concurrente.

**Rust:**
```rust
use dashmap::DashMap;
use std::collections::HashSet;

let connections: DashMap<ConnectionId, ConnectionHandle> = DashMap::new();
let pattern_index: DashMap<String, HashSet<ConnectionId>> = DashMap::new();

// Insertar (lock implicito por shard)
connections.insert(id, handle);

// Leer (lock compartido)
if let Some(conn) = connections.get(&id) {
    println!("Found: {:?}", conn.info);
}

// Modificar con entry API
pattern_index
    .entry(pattern.to_string())
    .or_default()
    .insert(conn_id);

// Iterar (snapshot, no bloquea inserciones)
for entry in connections.iter() {
    println!("{}: {:?}", entry.key(), entry.value());
}
```

**Comparacion con Java (ConcurrentHashMap):**
```java
ConcurrentHashMap<ConnectionId, ConnectionHandle> connections = new ConcurrentHashMap<>();
ConcurrentHashMap<String, Set<ConnectionId>> patternIndex = new ConcurrentHashMap<>();

// Insertar
connections.put(id, handle);

// Leer
ConnectionHandle conn = connections.get(id);

// Modificar con compute
patternIndex.compute(pattern, (k, v) -> {
    Set<ConnectionId> set = v != null ? v : ConcurrentHashMap.newKeySet();
    set.add(connId);
    return set;
});

// Iterar (vista debil)
connections.forEach((id, handle) -> {
    System.out.println(id + ": " + handle);
});
```

### 3. Arc para Estado Compartido

`Arc` (Atomic Reference Counting) permite compartir ownership entre threads.

**Rust:**
```rust
use std::sync::Arc;

// Estado compartido entre conexiones
let registry = Arc::new(ConnectionRegistry::new());
let broadcaster = Arc::new(ConfigChangeBroadcaster::new(
    Arc::clone(&registry),  // Clonar Arc, no el contenido
    100,
));

// Pasar a multiples tasks
let reg_clone = Arc::clone(&registry);
tokio::spawn(async move {
    reg_clone.register(handle);
});

// El Registry se libera cuando todos los Arc se dropean
```

**Comparacion con Java:**
```java
// Java: Los objetos son referencias por defecto
ConnectionRegistry registry = new ConnectionRegistry();
ConfigChangeBroadcaster broadcaster = new ConfigChangeBroadcaster(registry, 100);

// Pasar a threads (el GC maneja la memoria)
CompletableFuture.runAsync(() -> {
    registry.register(handle);
});
```

**Diferencias clave:**

| Aspecto | Arc (Rust) | Referencias (Java) |
|---------|------------|-------------------|
| Conteo | Explicito con clone() | Implicito (GC) |
| Costo | Atomic increment/decrement | GC overhead |
| Liberacion | Inmediata cuando count=0 | GC decides |
| Overhead | 8-16 bytes por Arc | Object header |

### 4. tokio::select! Macro

`select!` permite esperar multiples futures concurrentemente.

**Rust:**
```rust
use tokio::select;

loop {
    select! {
        // Primera rama que complete "gana"
        msg = ws_receiver.next() => {
            // Mensaje del WebSocket
            handle_ws_message(msg).await;
        }

        msg = broadcast_rx.recv() => {
            // Mensaje del broadcaster
            send_to_client(msg).await;
        }

        _ = tokio::time::sleep(Duration::from_secs(30)) => {
            // Timeout para heartbeat
            send_ping().await;
        }
    }
}

// Con biased para prioridad
select! {
    biased;  // Evalua en orden, no aleatorio

    // Alta prioridad primero
    msg = priority_rx.recv() => { ... }

    // Baja prioridad despues
    msg = normal_rx.recv() => { ... }
}
```

**Comparacion con Java:**
```java
// Java: CompletableFuture.anyOf
CompletableFuture.anyOf(
    wsReceiver.receiveAsync(),
    broadcastRx.receiveAsync(),
    CompletableFuture.delayedExecutor(30, TimeUnit.SECONDS)
        .execute(() -> null)
).thenAccept(result -> {
    if (result instanceof WsMessage) {
        handleWsMessage((WsMessage) result);
    } else if (result instanceof BroadcastMessage) {
        sendToClient((BroadcastMessage) result);
    } else {
        sendPing();
    }
});
```

---

## Riesgos y Errores Comunes

### 1. Deadlock con send() bloqueante

```rust
// MAL: send() puede bloquear si el channel esta lleno
for handle in subscribers {
    handle.sender.send(message.clone()).await;  // Bloquea!
}
// Si un subscriber es lento, bloquea broadcast a todos

// BIEN: Usar try_send() para no bloquear
for handle in subscribers {
    if let Err(e) = handle.sender.try_send(message.clone()) {
        match e {
            TrySendError::Full(_) => {
                warn!(id = %handle.id, "Client too slow, skipping");
                // Opcionalmente: marcar para desconexion
            }
            TrySendError::Closed(_) => {
                // Conexion cerrada, sera limpiada
            }
        }
    }
}
```

### 2. Memory leak en pattern_index

```rust
// MAL: Patrones vacios quedan en el indice
self.pattern_index
    .entry(pattern.to_string())
    .or_default()
    .remove(&id);
// El HashSet vacio sigue en el DashMap

// BIEN: Limpiar sets vacios
if let Some(mut set) = self.pattern_index.get_mut(pattern) {
    set.remove(&id);
    if set.is_empty() {
        drop(set);  // Liberar el lock antes de remove
        self.pattern_index.remove(pattern);
    }
}
```

### 3. Race condition en registro/broadcast

```rust
// POTENCIAL PROBLEMA:
// Thread 1: broadcaster.emit(event)  -> busca subscribers
// Thread 2: registry.register(new_conn)  -> nueva conexion
// Thread 1: broadcast a lista vieja -> nueva conexion no recibe

// MITIGACION: Aceptable para este caso de uso
// La nueva conexion recibira el proximo cambio
// Para garantias mas fuertes, usar un lock o version number
```

### 4. Clone excesivo de mensajes

```rust
// MAL: Clonar mensaje grande para cada subscriber
let large_message = ServerMessage::snapshot(..., huge_config);
for handle in subscribers {
    handle.send(large_message.clone()).await;  // Clone costoso!
}

// BIEN: Wrap en Arc para compartir
let shared_message = Arc::new(ServerMessage::snapshot(..., huge_config));
for handle in subscribers {
    handle.send_arc(Arc::clone(&shared_message)).await;
}

// O pre-serializar
let json = large_message.to_json()?;
for handle in subscribers {
    handle.send_raw(json.clone()).await;  // String clone mas barato
}
```

---

## Pruebas

### Tests Unitarios

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_register_unregister() {
        let registry = ConnectionRegistry::new();
        let (tx, _) = mpsc::channel(10);

        let info = ConnectionInfo::new("app".into(), "prod".into(), "main".into());
        let id = info.id;
        let handle = ConnectionHandle::new(info, tx);

        registry.register(handle);
        assert_eq!(registry.connection_count(), 1);

        registry.unregister(id);
        assert_eq!(registry.connection_count(), 0);
    }

    #[test]
    fn test_registry_find_subscribers_exact() {
        let registry = ConnectionRegistry::new();

        // Registrar conexiones
        for i in 0..3 {
            let (tx, _) = mpsc::channel(10);
            let info = ConnectionInfo::new("myapp".into(), "prod".into(), "main".into());
            registry.register(ConnectionHandle::new(info, tx));
        }

        // Otra app
        let (tx, _) = mpsc::channel(10);
        let info = ConnectionInfo::new("otherapp".into(), "prod".into(), "main".into());
        registry.register(ConnectionHandle::new(info, tx));

        let subscribers = registry.find_subscribers("myapp", "prod", "main");
        assert_eq!(subscribers.len(), 3);
    }

    #[test]
    fn test_registry_pattern_matching() {
        let registry = ConnectionRegistry::new();

        let (tx, _) = mpsc::channel(10);
        let info = ConnectionInfo::new("myapp".into(), "prod".into(), "main".into());
        let id = info.id;
        registry.register(ConnectionHandle::new(info, tx));

        // Suscribir a patron wildcard
        registry.subscribe_pattern(id, "*:*:main");

        // Deberia matchear cualquier app:profile con label main
        let subscribers = registry.find_subscribers("otherapp", "staging", "main");
        assert_eq!(subscribers.len(), 1);
    }

    #[tokio::test]
    async fn test_broadcaster_emit() {
        let registry = Arc::new(ConnectionRegistry::new());
        let broadcaster = ConfigChangeBroadcaster::new(Arc::clone(&registry), 100);

        let mut rx = broadcaster.subscribe();

        let event = ConfigChangeEvent::new(
            "myapp",
            "prod",
            "main",
            None,
            serde_json::json!({"key": "value"}),
            "v1",
        );

        broadcaster.emit(event.clone()).unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.app, "myapp");
        assert_eq!(received.version, "v1");
    }

    #[tokio::test]
    async fn test_broadcaster_delivers_to_subscribers() {
        let registry = Arc::new(ConnectionRegistry::new());
        let broadcaster = Arc::new(ConfigChangeBroadcaster::new(Arc::clone(&registry), 100));

        // Registrar conexion
        let (tx, mut rx) = mpsc::channel(10);
        let info = ConnectionInfo::new("myapp".into(), "prod".into(), "main".into());
        registry.register(ConnectionHandle::new(info, tx));

        // Iniciar broadcast loop
        let _handle = broadcaster.clone().start_broadcast_loop();

        // Emitir evento
        let event = ConfigChangeEvent::new(
            "myapp",
            "prod",
            "main",
            None,
            serde_json::json!({"port": 8080}),
            "v2",
        );
        broadcaster.emit(event).unwrap();

        // Esperar mensaje en la conexion
        let msg = tokio::time::timeout(
            Duration::from_millis(100),
            rx.recv()
        ).await.unwrap().unwrap();

        match msg {
            ServerMessage::ConfigSnapshot { version, .. } => {
                assert_eq!(version, "v2");
            }
            _ => panic!("Expected ConfigSnapshot"),
        }
    }
}
```

### Tests de Integracion

```rust
// tests/ws_broadcast_test.rs
#[tokio::test]
async fn test_broadcast_to_multiple_clients() {
    let app = create_test_app_with_broadcast().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let state = app.state().clone();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Conectar 3 clientes
    let mut clients = Vec::new();
    for _ in 0..3 {
        let url = format!("ws://{}/ws/myapp/prod", addr);
        let (ws, _) = connect_async(&url).await.unwrap();
        clients.push(ws);
    }

    // Consumir mensajes iniciales
    for client in &mut clients {
        let _ = client.next().await;
    }

    // Emitir cambio
    let event = ConfigChangeEvent::new(
        "myapp", "prod", "main",
        None,
        serde_json::json!({"updated": true}),
        "v3",
    );
    state.broadcaster.emit(event).unwrap();

    // Verificar que todos reciben
    for client in &mut clients {
        let msg = tokio::time::timeout(
            Duration::from_millis(100),
            client.next()
        ).await.unwrap().unwrap().unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&msg.into_text().unwrap()).unwrap();
        assert_eq!(parsed["type"], "config_snapshot");
        assert_eq!(parsed["config"]["updated"], true);
    }
}
```

---

## Observabilidad

### Logging Estructurado

```rust
#[instrument(skip(self, event), fields(key = %event.key()))]
async fn broadcast_event(&self, event: &ConfigChangeEvent) {
    let subscribers = self.registry.find_subscribers(...);

    info!(
        subscriber_count = subscribers.len(),
        version = %event.version,
        "Broadcasting config change"
    );

    for handle in subscribers {
        match handle.try_send(message.clone()) {
            Ok(_) => {
                info!(connection_id = %handle.id, "Message sent");
            }
            Err(TrySendError::Full(_)) => {
                warn!(connection_id = %handle.id, "Client buffer full");
            }
            Err(TrySendError::Closed(_)) => {
                info!(connection_id = %handle.id, "Client disconnected");
            }
        }
    }
}
```

### Metricas

```rust
// Conexiones activas
// metrics::gauge!("ws_connections_active", registry.connection_count() as f64);

// Mensajes broadcast
// metrics::counter!("ws_broadcast_total").increment(1);
// metrics::counter!("ws_broadcast_recipients").increment(subscriber_count as u64);

// Latencia de broadcast
// let start = Instant::now();
// self.broadcast_event(&event).await;
// metrics::histogram!("ws_broadcast_duration_seconds").record(start.elapsed().as_secs_f64());
```

---

## Entregable Final

### Archivos Creados/Modificados

1. `crates/vortex-server/src/ws/registry.rs` - ConnectionRegistry
2. `crates/vortex-server/src/ws/broadcaster.rs` - ConfigChangeBroadcaster
3. `crates/vortex-server/src/ws/mod.rs` - Re-exports actualizados
4. `crates/vortex-server/src/ws/handler.rs` - Integracion con registry
5. `crates/vortex-server/src/server.rs` - AppState con broadcast
6. `crates/vortex-server/src/cache/invalidation.rs` - Integracion opcional
7. `crates/vortex-server/tests/ws_broadcast_test.rs` - Tests de integracion

### Verificacion

```bash
# Compilar
cargo build -p vortex-server

# Tests
cargo test -p vortex-server broadcast

# Clippy
cargo clippy -p vortex-server -- -D warnings

# Test manual: Terminal 1
cargo run -p vortex-server

# Test manual: Terminal 2 (cliente 1)
websocat ws://localhost:8080/ws/myapp/prod

# Test manual: Terminal 3 (cliente 2)
websocat ws://localhost:8080/ws/myapp/prod

# Test manual: Terminal 4 (trigger change)
curl -X DELETE http://localhost:8080/cache/myapp/prod/main
# Ambos clientes deberian recibir update
```

---

**Anterior**: [Historia 001 - WebSocket Endpoint](./story-001-websocket-endpoint.md)
**Siguiente**: [Historia 003 - Diff Semantico](./story-003-semantic-diff.md)
