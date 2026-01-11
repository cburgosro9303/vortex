# Historia 001: WebSocket Endpoint

## Contexto y Objetivo

Los clientes de configuracion tradicionalmente usan polling HTTP para detectar cambios: cada N segundos, hacen un request `GET /{app}/{profile}` y comparan con el estado anterior. Esto es ineficiente y agrega latencia.

WebSockets permiten una conexion persistente donde el servidor puede "pushear" cambios instantaneamente. En lugar de que el cliente pregunte "hay cambios?", el servidor dice "hubo un cambio, aqui esta".

Esta historia implementa el endpoint WebSocket basico que:
- Acepta conexiones en `/ws/{app}/{profile}`
- Maneja el upgrade HTTP a WebSocket
- Envia la configuracion inicial al conectar
- Mantiene el estado de la conexion

Para desarrolladores Java, esto es similar a `@ServerEndpoint` de Jakarta WebSocket, pero con el modelo async de Rust.

---

## Alcance

### In Scope

- Endpoint `GET /ws/{app}/{profile}` con upgrade a WebSocket
- Upgrade handler con autenticacion basica (query params)
- Envio de configuracion inicial al conectar
- Estructura de mensajes JSON
- Connection state machine basica
- Manejo de errores de conexion
- Tests del endpoint

### Out of Scope

- Broadcast de cambios (historia 002)
- Diff semantico (historia 003)
- Heartbeat/ping-pong (historia 004)
- Autenticacion avanzada (tokens JWT)
- Compresion de mensajes
- Rate limiting

---

## Criterios de Aceptacion

- [ ] `GET /ws/{app}/{profile}` acepta upgrade WebSocket
- [ ] Conexion recibe mensaje `config_snapshot` inmediatamente
- [ ] Query param `?token=xxx` permite autenticacion basica
- [ ] Errores retornan mensaje JSON estructurado
- [ ] Metricas: conexiones activas, conexiones totales
- [ ] Logs estructurados para open/close de conexiones
- [ ] Tests de integracion pasan

---

## Diseno Propuesto

### Estructura de Modulos

```
crates/vortex-server/src/ws/
├── mod.rs              # Re-exports
├── handler.rs          # WebSocket upgrade handler
├── connection.rs       # Connection state machine
└── messages.rs         # Message types
```

### Interfaces Principales

```rust
/// Handler para upgrade WebSocket
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path((app, profile)): Path<(String, String)>,
    Query(params): Query<WsQueryParams>,
    State(state): State<AppState>,
) -> impl IntoResponse;

/// Parametros de query para WebSocket
#[derive(Debug, Deserialize)]
pub struct WsQueryParams {
    /// Token de autenticacion opcional
    pub token: Option<String>,
    /// Label (branch/tag), default "main"
    pub label: Option<String>,
}

/// Mensajes enviados por el servidor
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    ConfigSnapshot {
        app: String,
        profile: String,
        label: String,
        config: serde_json::Value,
        version: String,
        timestamp: DateTime<Utc>,
    },
    Error {
        code: String,
        message: String,
        timestamp: DateTime<Utc>,
    },
}
```

---

## Pasos de Implementacion

### Paso 1: Definir Tipos de Mensajes

```rust
// src/ws/messages.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Mensajes enviados del servidor al cliente
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Snapshot completo de configuracion (enviado al conectar)
    ConfigSnapshot {
        app: String,
        profile: String,
        label: String,
        config: serde_json::Value,
        version: String,
        timestamp: DateTime<Utc>,
    },

    /// Notificacion de cambio (historia 002)
    ConfigChange {
        app: String,
        profile: String,
        diff: Vec<DiffOp>,
        old_version: String,
        new_version: String,
        timestamp: DateTime<Utc>,
    },

    /// Ping para heartbeat (historia 004)
    Ping {
        timestamp: DateTime<Utc>,
    },

    /// Error
    Error {
        code: String,
        message: String,
        timestamp: DateTime<Utc>,
    },
}

/// Mensajes enviados del cliente al servidor
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Pong en respuesta a ping
    Pong {
        timestamp: DateTime<Utc>,
    },

    /// Suscribirse a patrones adicionales
    Subscribe {
        patterns: Vec<String>,
    },

    /// Desuscribirse de patrones
    Unsubscribe {
        patterns: Vec<String>,
    },

    /// Resincronizar desde version conocida
    Resync {
        last_version: Option<String>,
    },
}

/// Operacion de diff (placeholder, detalle en historia 003)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffOp {
    pub op: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

impl ServerMessage {
    /// Crea un mensaje de snapshot
    pub fn snapshot(
        app: impl Into<String>,
        profile: impl Into<String>,
        label: impl Into<String>,
        config: serde_json::Value,
        version: impl Into<String>,
    ) -> Self {
        Self::ConfigSnapshot {
            app: app.into(),
            profile: profile.into(),
            label: label.into(),
            config,
            version: version.into(),
            timestamp: Utc::now(),
        }
    }

    /// Crea un mensaje de error
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Error {
            code: code.into(),
            message: message.into(),
            timestamp: Utc::now(),
        }
    }

    /// Serializa a JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}
```

### Paso 2: Implementar Connection State

```rust
// src/ws/connection.rs
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;
use tracing::{info, warn, instrument, Span};

/// Identificador unico de conexion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionId(Uuid);

impl ConnectionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl std::fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Estado de una conexion WebSocket
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Conexion establecida, esperando handshake
    Connecting,
    /// Conexion activa y operacional
    Connected,
    /// Conexion cerrandose gracefully
    Closing,
    /// Conexion cerrada
    Closed,
}

/// Metadata de una conexion WebSocket
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub id: ConnectionId,
    pub app: String,
    pub profile: String,
    pub label: String,
    pub state: ConnectionState,
    pub connected_at: chrono::DateTime<chrono::Utc>,
    pub last_message_at: chrono::DateTime<chrono::Utc>,
}

impl ConnectionInfo {
    pub fn new(app: String, profile: String, label: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: ConnectionId::new(),
            app,
            profile,
            label,
            state: ConnectionState::Connecting,
            connected_at: now,
            last_message_at: now,
        }
    }

    /// Actualiza el timestamp del ultimo mensaje
    pub fn touch(&mut self) {
        self.last_message_at = chrono::Utc::now();
    }

    /// Cambia el estado de la conexion
    pub fn set_state(&mut self, state: ConnectionState) {
        self.state = state;
    }

    /// Genera la cache key para esta conexion
    pub fn cache_key(&self) -> String {
        format!("{}:{}:{}", self.app, self.profile, self.label)
    }
}
```

### Paso 3: Implementar WebSocket Handler

```rust
// src/ws/handler.rs
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use tracing::{error, info, warn, instrument, Span};

use crate::server::AppState;
use super::connection::{ConnectionId, ConnectionInfo, ConnectionState};
use super::messages::{ClientMessage, ServerMessage};

/// Query parameters para conexion WebSocket
#[derive(Debug, Deserialize)]
pub struct WsQueryParams {
    /// Token de autenticacion (opcional)
    pub token: Option<String>,
    /// Label/branch, default "main"
    #[serde(default = "default_label")]
    pub label: String,
}

fn default_label() -> String {
    "main".to_string()
}

/// Handler para upgrade WebSocket
///
/// # Endpoint
/// `GET /ws/{app}/{profile}?label=main&token=xxx`
///
/// # Ejemplo
/// ```
/// ws://localhost:8080/ws/myapp/production?label=main
/// ```
#[instrument(skip(ws, state), fields(app = %app, profile = %profile))]
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path((app, profile)): Path<(String, String)>,
    Query(params): Query<WsQueryParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let label = params.label;

    info!(label = %label, "WebSocket upgrade requested");

    // Validar autenticacion si se requiere
    if let Err(e) = validate_token(&params.token, &state).await {
        warn!(error = %e, "WebSocket auth failed");
        return ws.on_upgrade(move |socket| async move {
            send_error_and_close(socket, "AUTH_FAILED", &e.to_string()).await;
        });
    }

    // Crear info de conexion
    let conn_info = ConnectionInfo::new(app.clone(), profile.clone(), label.clone());
    let conn_id = conn_info.id;

    info!(connection_id = %conn_id, "WebSocket upgrade accepted");

    // Incrementar metrica de conexiones
    // metrics::counter!("ws_connections_total").increment(1);

    ws.on_upgrade(move |socket| {
        handle_socket(socket, conn_info, state)
    })
}

/// Valida el token de autenticacion
async fn validate_token(
    token: &Option<String>,
    _state: &AppState,
) -> Result<(), &'static str> {
    // Por ahora, autenticacion opcional
    // En produccion, validar contra un servicio de auth
    if let Some(t) = token {
        if t.is_empty() {
            return Err("Empty token provided");
        }
        // TODO: Validar token real
    }
    Ok(())
}

/// Maneja una conexion WebSocket activa
#[instrument(skip(socket, state), fields(connection_id = %conn_info.id))]
async fn handle_socket(
    socket: WebSocket,
    mut conn_info: ConnectionInfo,
    state: AppState,
) {
    // Split socket into sender and receiver
    let (mut sender, mut receiver) = socket.split();

    // Marcar como conectado
    conn_info.set_state(ConnectionState::Connected);

    // Enviar configuracion inicial
    match send_initial_config(&mut sender, &conn_info, &state).await {
        Ok(_) => info!("Initial config sent"),
        Err(e) => {
            error!(error = %e, "Failed to send initial config");
            return;
        }
    }

    // Registrar conexion en el registry (para broadcast)
    // state.ws_registry.register(conn_info.id, sender.clone());

    // Loop principal de la conexion
    while let Some(msg_result) = receiver.next().await {
        match msg_result {
            Ok(msg) => {
                conn_info.touch();

                match msg {
                    Message::Text(text) => {
                        if let Err(e) = handle_text_message(&text, &mut sender, &conn_info).await {
                            warn!(error = %e, "Error handling message");
                        }
                    }
                    Message::Binary(data) => {
                        warn!(len = data.len(), "Received binary message, ignoring");
                    }
                    Message::Ping(data) => {
                        // Axum maneja pong automaticamente
                        info!("Received ping");
                    }
                    Message::Pong(_) => {
                        info!("Received pong");
                    }
                    Message::Close(reason) => {
                        info!(reason = ?reason, "Client initiated close");
                        break;
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "WebSocket error");
                break;
            }
        }
    }

    // Cleanup
    conn_info.set_state(ConnectionState::Closed);
    // state.ws_registry.unregister(conn_info.id);
    // metrics::gauge!("ws_connections_active").decrement(1.0);

    info!("WebSocket connection closed");
}

/// Envia la configuracion inicial al cliente
async fn send_initial_config(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    conn_info: &ConnectionInfo,
    state: &AppState,
) -> Result<(), axum::Error> {
    // Obtener configuracion del backend
    let config = state
        .config_source
        .get_config(&conn_info.app, &conn_info.profile, &conn_info.label)
        .await
        .map_err(|e| axum::Error::new(e))?;

    // Construir mensaje
    let message = ServerMessage::snapshot(
        &conn_info.app,
        &conn_info.profile,
        &conn_info.label,
        config.to_json_value(),
        config.version.unwrap_or_default(),
    );

    // Serializar y enviar
    let json = message.to_json()
        .map_err(|e| axum::Error::new(e))?;

    sender
        .send(Message::Text(json))
        .await
        .map_err(|e| axum::Error::new(e))
}

/// Procesa un mensaje de texto del cliente
async fn handle_text_message(
    text: &str,
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    conn_info: &ConnectionInfo,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Parsear mensaje
    let client_msg: ClientMessage = serde_json::from_str(text)?;

    match client_msg {
        ClientMessage::Pong { timestamp } => {
            info!(timestamp = %timestamp, "Received pong from client");
            Ok(())
        }
        ClientMessage::Subscribe { patterns } => {
            info!(patterns = ?patterns, "Subscribe request");
            // TODO: Implementar en historia 002
            Ok(())
        }
        ClientMessage::Unsubscribe { patterns } => {
            info!(patterns = ?patterns, "Unsubscribe request");
            // TODO: Implementar en historia 002
            Ok(())
        }
        ClientMessage::Resync { last_version } => {
            info!(last_version = ?last_version, "Resync request");
            // TODO: Implementar resync
            Ok(())
        }
    }
}

/// Envia un error y cierra la conexion
async fn send_error_and_close(mut socket: WebSocket, code: &str, message: &str) {
    let error_msg = ServerMessage::error(code, message);

    if let Ok(json) = error_msg.to_json() {
        let _ = socket.send(Message::Text(json)).await;
    }

    let _ = socket.close().await;
}
```

### Paso 4: Registrar Rutas

```rust
// src/server.rs (modificacion)
use crate::ws::handler::ws_handler;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Existing routes
        .route("/health", get(health_check))
        .route("/{app}/{profile}", get(get_config))
        .route("/{app}/{profile}/{label}", get(get_config_with_label))
        // WebSocket route
        .route("/ws/:app/:profile", get(ws_handler))
        .with_state(state)
}
```

### Paso 5: Actualizar mod.rs

```rust
// src/ws/mod.rs
//! WebSocket support for real-time configuration updates.
//!
//! This module provides WebSocket endpoints that allow clients to receive
//! configuration changes in real-time without polling.
//!
//! # Example
//!
//! ```javascript
//! const ws = new WebSocket('ws://localhost:8080/ws/myapp/production?label=main');
//!
//! ws.onmessage = (event) => {
//!     const msg = JSON.parse(event.data);
//!     if (msg.type === 'config_snapshot') {
//!         console.log('Initial config:', msg.config);
//!     } else if (msg.type === 'config_change') {
//!         console.log('Config changed:', msg.diff);
//!     }
//! };
//! ```

pub mod connection;
pub mod handler;
pub mod messages;

pub use connection::{ConnectionId, ConnectionInfo, ConnectionState};
pub use handler::{ws_handler, WsQueryParams};
pub use messages::{ClientMessage, ServerMessage, DiffOp};
```

---

## Conceptos de Rust Aprendidos

### 1. WebSocket Upgrade en Axum

Axum maneja el upgrade HTTP -> WebSocket de forma elegante con extractors.

**Rust:**
```rust
use axum::extract::ws::{WebSocket, WebSocketUpgrade, Message};

// WebSocketUpgrade es un extractor que maneja el handshake
pub async fn ws_handler(
    ws: WebSocketUpgrade,  // Extrae headers de upgrade
    Path(app): Path<String>,
) -> impl IntoResponse {
    // on_upgrade acepta un closure que recibe el WebSocket
    ws.on_upgrade(|socket| async move {
        handle_socket(socket).await
    })
}

async fn handle_socket(mut socket: WebSocket) {
    // Socket listo para usar
    while let Some(msg) = socket.recv().await {
        match msg {
            Ok(Message::Text(text)) => {
                // Procesar texto
                socket.send(Message::Text("reply".to_string())).await;
            }
            Ok(Message::Close(_)) => break,
            Err(e) => break,
            _ => {}
        }
    }
}
```

**Comparacion con Java (Jakarta WebSocket):**
```java
@ServerEndpoint("/ws/{app}/{profile}")
public class ConfigWebSocket {

    @OnOpen
    public void onOpen(Session session,
                       @PathParam("app") String app,
                       @PathParam("profile") String profile) {
        // Conexion abierta
        session.getBasicRemote().sendText("connected");
    }

    @OnMessage
    public void onMessage(String message, Session session) {
        // Mensaje recibido
        session.getBasicRemote().sendText("reply");
    }

    @OnClose
    public void onClose(Session session, CloseReason reason) {
        // Conexion cerrada
    }

    @OnError
    public void onError(Throwable t) {
        t.printStackTrace();
    }
}
```

**Diferencias clave:**

| Aspecto | Axum WebSocket | Jakarta WebSocket |
|---------|----------------|-------------------|
| Modelo | Async loop explicito | Callbacks/Annotations |
| Threading | Single task, cooperative | Thread per connection |
| Lifecycle | Manual en async loop | Annotations @OnOpen, etc |
| Backpressure | Futures naturales | Manual buffering |

### 2. Stream Split Pattern

Separar el WebSocket en sender y receiver permite operaciones concurrentes.

**Rust:**
```rust
use futures::{SinkExt, StreamExt};

async fn handle_socket(socket: WebSocket) {
    // Split into independent sender and receiver
    let (mut sender, mut receiver) = socket.split();

    // Ahora podemos enviar y recibir concurrentemente
    // Por ejemplo, en diferentes tasks:

    let send_task = tokio::spawn(async move {
        loop {
            sender.send(Message::Text("ping".to_string())).await?;
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });

    let recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            // Process messages
        }
    });

    // Esperar ambos
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}
```

**Comparacion con Java:**
```java
// Java: Una session, metodos sincronos
@OnMessage
public void onMessage(String message, Session session) {
    // Enviar respuesta (bloqueante en el mismo thread)
    session.getBasicRemote().sendText("response");

    // Para async:
    session.getAsyncRemote().sendText("response", result -> {
        if (result.isOK()) {
            // Enviado
        }
    });
}

// Para enviar desde otro thread:
public class WebSocketRegistry {
    private Map<String, Session> sessions = new ConcurrentHashMap<>();

    public void broadcast(String message) {
        sessions.values().forEach(session -> {
            session.getAsyncRemote().sendText(message);
        });
    }
}
```

### 3. Futures y SinkExt/StreamExt

Los traits de futures proveen operaciones ergonomicas sobre streams.

**Rust:**
```rust
use futures::{SinkExt, StreamExt};

// StreamExt agrega metodos como .next(), .map(), .filter()
while let Some(msg) = receiver.next().await {
    // next() es de StreamExt
}

// Tambien puedes usar operadores funcionales
let text_messages = receiver
    .filter_map(|result| async {
        match result {
            Ok(Message::Text(text)) => Some(text),
            _ => None,
        }
    });

// SinkExt agrega metodos como .send(), .send_all()
sender.send(Message::Text("hello".to_string())).await?;

// Enviar multiples mensajes
use futures::stream;
let messages = stream::iter(vec![
    Ok(Message::Text("one".to_string())),
    Ok(Message::Text("two".to_string())),
]);
sender.send_all(&mut messages).await?;
```

**Comparacion con Java (Reactive Streams):**
```java
// Java con Project Reactor
Flux.interval(Duration.ofSeconds(30))
    .map(i -> "ping-" + i)
    .subscribe(msg -> session.getAsyncRemote().sendText(msg));

// Filtrar mensajes
Flux.from(messagePublisher)
    .filter(msg -> msg.startsWith("important"))
    .subscribe(this::handleImportantMessage);
```

### 4. Tagged Enums con Serde

`#[serde(tag = "type")]` crea JSON discriminado por tipo.

**Rust:**
```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    ConfigSnapshot {
        app: String,
        config: Value,
    },
    ConfigChange {
        app: String,
        diff: Vec<DiffOp>,
    },
    Error {
        code: String,
        message: String,
    },
}

// Serializa a:
// {"type": "config_snapshot", "app": "myapp", "config": {...}}
// {"type": "config_change", "app": "myapp", "diff": [...]}
// {"type": "error", "code": "AUTH", "message": "..."}
```

**Comparacion con Java (Jackson):**
```java
@JsonTypeInfo(use = JsonTypeInfo.Id.NAME, property = "type")
@JsonSubTypes({
    @Type(value = ConfigSnapshot.class, name = "config_snapshot"),
    @Type(value = ConfigChange.class, name = "config_change"),
    @Type(value = Error.class, name = "error")
})
public abstract class ServerMessage { }

public class ConfigSnapshot extends ServerMessage {
    private String app;
    private Object config;
}
```

---

## Riesgos y Errores Comunes

### 1. Olvidar split() y bloquear

```rust
// MAL: recv() y send() en el mismo task sin split
async fn bad_handler(mut socket: WebSocket) {
    while let Some(msg) = socket.recv().await {  // Bloquea aqui
        socket.send(Message::Text("pong")).await;  // No puede enviar ping
        // No hay forma de enviar mensajes proactivamente!
    }
}

// BIEN: Split para operaciones concurrentes
async fn good_handler(socket: WebSocket) {
    let (mut tx, mut rx) = socket.split();

    tokio::select! {
        _ = send_loop(&mut tx) => {},
        _ = recv_loop(&mut rx) => {},
    }
}
```

### 2. No manejar cierre graceful

```rust
// MAL: Ignorar Message::Close
while let Some(Ok(msg)) = socket.recv().await {
    match msg {
        Message::Text(t) => handle_text(t),
        _ => {} // Close ignorado!
    }
}

// BIEN: Manejar Close explicitamente
while let Some(msg_result) = socket.recv().await {
    match msg_result {
        Ok(Message::Text(t)) => handle_text(t),
        Ok(Message::Close(reason)) => {
            info!("Client closing: {:?}", reason);
            break;  // Salir del loop
        }
        Err(e) => {
            error!("WebSocket error: {}", e);
            break;
        }
        _ => {}
    }
}
```

### 3. Memory leak por conexiones abandonadas

```rust
// MAL: Registrar conexion sin cleanup
state.connections.insert(conn_id, sender);
// Si el loop termina, la conexion queda registrada!

// BIEN: Usar Drop o finally pattern
struct ConnectionGuard<'a> {
    registry: &'a ConnectionRegistry,
    id: ConnectionId,
}

impl Drop for ConnectionGuard<'_> {
    fn drop(&mut self) {
        self.registry.unregister(self.id);
        info!(id = %self.id, "Connection unregistered");
    }
}

async fn handle_socket(...) {
    let _guard = ConnectionGuard {
        registry: &state.ws_registry,
        id: conn_info.id,
    };

    // ... loop ...

    // Guard se dropea automaticamente, limpiando la conexion
}
```

### 4. Serialization panic en send

```rust
// MAL: unwrap en serializacion
let json = serde_json::to_string(&message).unwrap();  // Puede panic!

// BIEN: Manejar error de serializacion
match serde_json::to_string(&message) {
    Ok(json) => {
        if let Err(e) = sender.send(Message::Text(json)).await {
            warn!("Failed to send: {}", e);
        }
    }
    Err(e) => {
        error!("Failed to serialize message: {}", e);
    }
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
    fn test_server_message_snapshot_serialization() {
        let msg = ServerMessage::snapshot(
            "myapp",
            "prod",
            "main",
            serde_json::json!({"key": "value"}),
            "abc123",
        );

        let json = msg.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "config_snapshot");
        assert_eq!(parsed["app"], "myapp");
        assert_eq!(parsed["profile"], "prod");
        assert_eq!(parsed["config"]["key"], "value");
    }

    #[test]
    fn test_server_message_error_serialization() {
        let msg = ServerMessage::error("AUTH_FAILED", "Invalid token");

        let json = msg.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "error");
        assert_eq!(parsed["code"], "AUTH_FAILED");
        assert_eq!(parsed["message"], "Invalid token");
    }

    #[test]
    fn test_client_message_deserialization() {
        let json = r#"{"type": "subscribe", "patterns": ["myapp:*:*"]}"#;

        let msg: ClientMessage = serde_json::from_str(json).unwrap();

        match msg {
            ClientMessage::Subscribe { patterns } => {
                assert_eq!(patterns, vec!["myapp:*:*"]);
            }
            _ => panic!("Expected Subscribe"),
        }
    }

    #[test]
    fn test_connection_info_cache_key() {
        let info = ConnectionInfo::new(
            "myapp".to_string(),
            "prod".to_string(),
            "main".to_string(),
        );

        assert_eq!(info.cache_key(), "myapp:prod:main");
    }
}
```

### Tests de Integracion

```rust
// tests/ws_connection_test.rs
use axum::body::Body;
use axum::http::{Request, StatusCode};
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use vortex_server::create_router;

#[tokio::test]
async fn test_websocket_upgrade() {
    // Start test server
    let app = create_test_app().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Connect WebSocket
    let url = format!("ws://{}/ws/myapp/prod", addr);
    let (mut ws, _) = connect_async(&url).await.expect("Failed to connect");

    // Should receive initial config
    let msg = ws.next().await.unwrap().unwrap();
    let text = msg.into_text().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();

    assert_eq!(parsed["type"], "config_snapshot");
    assert_eq!(parsed["app"], "myapp");
    assert_eq!(parsed["profile"], "prod");

    // Close cleanly
    ws.close(None).await.unwrap();
}

#[tokio::test]
async fn test_websocket_with_label() {
    let app = create_test_app().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Connect with custom label
    let url = format!("ws://{}/ws/myapp/prod?label=develop", addr);
    let (mut ws, _) = connect_async(&url).await.expect("Failed to connect");

    let msg = ws.next().await.unwrap().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&msg.into_text().unwrap()).unwrap();

    assert_eq!(parsed["label"], "develop");
}

#[tokio::test]
async fn test_websocket_client_message() {
    let app = create_test_app().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let url = format!("ws://{}/ws/myapp/prod", addr);
    let (mut ws, _) = connect_async(&url).await.unwrap();

    // Consume initial snapshot
    let _ = ws.next().await;

    // Send pong message
    let pong = r#"{"type": "pong", "timestamp": "2025-01-15T10:30:00Z"}"#;
    ws.send(Message::Text(pong.to_string())).await.unwrap();

    // Should not error
    // (No response expected for pong)
}
```

---

## Observabilidad

### Logging Estructurado

```rust
use tracing::{info, warn, error, instrument, Span};

#[instrument(skip(ws, state), fields(app = %app, profile = %profile))]
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path((app, profile)): Path<(String, String)>,
    Query(params): Query<WsQueryParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    info!(label = %params.label, "WebSocket upgrade requested");

    // ...
}

#[instrument(skip(socket, state), fields(connection_id = %conn_info.id))]
async fn handle_socket(socket: WebSocket, conn_info: ConnectionInfo, state: AppState) {
    info!(
        app = %conn_info.app,
        profile = %conn_info.profile,
        "WebSocket connection established"
    );

    // En el loop
    info!(message_type = "text", len = text.len(), "Received message");

    // Al cerrar
    info!(
        duration_secs = conn_info.connected_at.elapsed().as_secs(),
        "WebSocket connection closed"
    );
}
```

### Metricas Preparadas

```rust
// Incrementar al aceptar conexion
// metrics::counter!("ws_connections_total").increment(1);
// metrics::gauge!("ws_connections_active").increment(1.0);

// Decrementar al cerrar
// metrics::gauge!("ws_connections_active").decrement(1.0);

// Latencia de mensajes
// let start = Instant::now();
// process_message(&msg).await;
// metrics::histogram!("ws_message_duration_seconds").record(start.elapsed().as_secs_f64());
```

---

## Entregable Final

### Archivos Creados/Modificados

1. `crates/vortex-server/src/ws/mod.rs` - Modulo WebSocket
2. `crates/vortex-server/src/ws/messages.rs` - Tipos de mensajes
3. `crates/vortex-server/src/ws/connection.rs` - Estado de conexion
4. `crates/vortex-server/src/ws/handler.rs` - Handler de upgrade
5. `crates/vortex-server/src/server.rs` - Ruta WebSocket agregada
6. `crates/vortex-server/tests/ws_connection_test.rs` - Tests de integracion

### Verificacion

```bash
# Compilar
cargo build -p vortex-server

# Tests
cargo test -p vortex-server ws

# Clippy
cargo clippy -p vortex-server -- -D warnings

# Ejecutar servidor
cargo run -p vortex-server

# Test manual con websocat
websocat ws://localhost:8080/ws/myapp/prod
```

### Ejemplo de Uso (JavaScript)

```javascript
const ws = new WebSocket('ws://localhost:8080/ws/myapp/production?label=main');

ws.onopen = () => {
    console.log('Connected to Vortex Config');
};

ws.onmessage = (event) => {
    const msg = JSON.parse(event.data);

    switch (msg.type) {
        case 'config_snapshot':
            console.log('Initial config:', msg.config);
            applyConfig(msg.config);
            break;
        case 'config_change':
            console.log('Config changed:', msg.diff);
            applyDiff(msg.diff);
            break;
        case 'error':
            console.error('Error:', msg.code, msg.message);
            break;
    }
};

ws.onerror = (error) => {
    console.error('WebSocket error:', error);
};

ws.onclose = (event) => {
    console.log('Disconnected:', event.code, event.reason);
    // Implement reconnection logic
};
```

---

**Siguiente**: [Historia 002 - Broadcast de Cambios](./story-002-change-broadcast.md)
