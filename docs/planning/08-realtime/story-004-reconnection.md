# Historia 004: Reconexion y Heartbeat

## Contexto y Objetivo

Las conexiones WebSocket pueden fallar silenciosamente por multiples razones:
- **NAT timeouts**: Routers cierran conexiones idle despues de ~60 segundos
- **Proxies/Load Balancers**: Timeouts configurables (AWS ALB default: 60s)
- **Desconexiones de red**: Cambios de WiFi, modo avion, perdida de senal
- **Servidor reiniciando**: Deploys, crashes, scaling

Sin un mecanismo de deteccion, los clientes pueden creer que estan conectados cuando en realidad la conexion esta muerta. Esto resulta en:
- Cambios de configuracion perdidos
- Estado inconsistente
- Usuarios frustrados

Esta historia implementa:
- **Heartbeat (Ping/Pong)**: Detectar conexiones muertas proactivamente
- **Graceful shutdown**: Notificar clientes antes de cerrar
- **Estado de reconexion**: Permitir a clientes retomar donde quedaron

Para desarrolladores Java, esto es similar a implementar `@OnPing`/`@OnPong` handlers en Jakarta WebSocket, combinado con patrones de circuit breaker.

---

## Alcance

### In Scope

- Ping del servidor cada 30 segundos
- Timeout si no hay pong en 10 segundos
- Graceful shutdown con notificacion a clientes
- Mensaje `reconnect_info` al conectar con last_version
- Envio de cambios perdidos al reconectar
- Metricas de latencia de ping
- Tests de timeout y reconexion

### Out of Scope

- Persistencia de mensajes en disco
- Reconexion automatica desde el servidor (es responsabilidad del cliente)
- Clustering de estado de conexion
- Backoff exponencial (es logica de cliente)
- Compresion de mensajes acumulados

---

## Criterios de Aceptacion

- [ ] Servidor envia ping cada 30 segundos
- [ ] Conexion se cierra si no hay pong en 10 segundos
- [ ] Cliente recibe `connection_closing` antes de shutdown
- [ ] Reconexion con `last_version` recibe cambios perdidos
- [ ] Latencia de ping disponible en metricas
- [ ] Tests de timeout pasan

---

## Diseno Propuesto

### Estructura de Modulos

```
crates/vortex-server/src/ws/
├── mod.rs
├── handler.rs
├── connection.rs
├── messages.rs
├── registry.rs
├── broadcaster.rs
├── diff.rs
└── heartbeat.rs        # Nuevo: HeartbeatManager
```

### Interfaces Principales

```rust
/// Configuracion de heartbeat
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// Intervalo entre pings
    pub interval: Duration,
    /// Timeout para esperar pong
    pub timeout: Duration,
    /// Jitter aleatorio (0.0-1.0) para evitar thundering herd
    pub jitter: f64,
}

/// Manager de heartbeat para una conexion
pub struct HeartbeatManager {
    config: HeartbeatConfig,
    last_pong: Instant,
    ping_sent_at: Option<Instant>,
}

/// Mensajes adicionales para lifecycle
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LifecycleMessage {
    /// Ping del servidor
    Ping { timestamp: DateTime<Utc> },

    /// Servidor va a cerrar (graceful shutdown)
    ConnectionClosing {
        reason: String,
        reconnect_after_ms: Option<u64>,
    },

    /// Informacion de reconexion
    ReconnectInfo {
        missed_versions: Vec<String>,
        current_version: String,
    },
}
```

---

## Pasos de Implementacion

### Paso 1: Implementar HeartbeatManager

```rust
// src/ws/heartbeat.rs
use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};
use rand::Rng;
use tokio::time::{interval, Interval};
use tracing::{debug, info, warn, instrument};

/// Configuracion del sistema de heartbeat
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// Intervalo entre pings (default: 30s)
    pub interval: Duration,
    /// Timeout esperando pong (default: 10s)
    pub timeout: Duration,
    /// Jitter como fraccion del intervalo (default: 0.2 = 20%)
    /// Ayuda a distribuir pings y evitar thundering herd
    pub jitter: f64,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(10),
            jitter: 0.2,
        }
    }
}

impl HeartbeatConfig {
    /// Intervalo con jitter aleatorio aplicado
    pub fn interval_with_jitter(&self) -> Duration {
        let jitter_range = (self.interval.as_secs_f64() * self.jitter) as u64;
        let jitter: i64 = rand::thread_rng().gen_range(-(jitter_range as i64)..=jitter_range as i64);
        let base_millis = self.interval.as_millis() as i64;
        Duration::from_millis((base_millis + jitter * 1000) as u64)
    }
}

/// Estado del heartbeat para una conexion
#[derive(Debug)]
pub enum HeartbeatState {
    /// Esperando proximo ping
    Idle,
    /// Ping enviado, esperando pong
    WaitingPong { sent_at: Instant },
    /// Timeout, conexion debe cerrarse
    TimedOut,
}

/// Manager de heartbeat para una conexion individual
#[derive(Debug)]
pub struct HeartbeatManager {
    config: HeartbeatConfig,
    /// Ultimo momento que recibimos actividad (pong o cualquier mensaje)
    last_activity: Instant,
    /// Estado actual
    state: HeartbeatState,
}

impl HeartbeatManager {
    pub fn new(config: HeartbeatConfig) -> Self {
        Self {
            config,
            last_activity: Instant::now(),
            state: HeartbeatState::Idle,
        }
    }

    /// Registra actividad del cliente (cualquier mensaje recibido)
    pub fn record_activity(&mut self) {
        self.last_activity = Instant::now();
        if matches!(self.state, HeartbeatState::WaitingPong { .. }) {
            self.state = HeartbeatState::Idle;
        }
    }

    /// Registra recepcion de pong
    pub fn record_pong(&mut self) -> Option<Duration> {
        let latency = match self.state {
            HeartbeatState::WaitingPong { sent_at } => {
                Some(sent_at.elapsed())
            }
            _ => None,
        };

        self.last_activity = Instant::now();
        self.state = HeartbeatState::Idle;

        latency
    }

    /// Marca que se envio un ping
    pub fn ping_sent(&mut self) {
        self.state = HeartbeatState::WaitingPong {
            sent_at: Instant::now(),
        };
    }

    /// Verifica si debemos enviar un ping
    pub fn should_ping(&self) -> bool {
        matches!(self.state, HeartbeatState::Idle)
            && self.last_activity.elapsed() >= self.config.interval
    }

    /// Verifica si la conexion ha hecho timeout
    pub fn is_timed_out(&self) -> bool {
        match self.state {
            HeartbeatState::WaitingPong { sent_at } => {
                sent_at.elapsed() >= self.config.timeout
            }
            HeartbeatState::TimedOut => true,
            _ => false,
        }
    }

    /// Retorna el tiempo hasta el proximo evento (ping o timeout)
    pub fn time_until_next_action(&self) -> Duration {
        match self.state {
            HeartbeatState::Idle => {
                let since_last = self.last_activity.elapsed();
                if since_last >= self.config.interval {
                    Duration::ZERO
                } else {
                    self.config.interval_with_jitter() - since_last
                }
            }
            HeartbeatState::WaitingPong { sent_at } => {
                let elapsed = sent_at.elapsed();
                if elapsed >= self.config.timeout {
                    Duration::ZERO
                } else {
                    self.config.timeout - elapsed
                }
            }
            HeartbeatState::TimedOut => Duration::ZERO,
        }
    }
}

/// Mensaje de ping para enviar al cliente
#[derive(Debug, Clone, serde::Serialize)]
pub struct PingMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub timestamp: DateTime<Utc>,
}

impl PingMessage {
    pub fn new() -> Self {
        Self {
            msg_type: "ping".to_string(),
            timestamp: Utc::now(),
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Mensaje de pong esperado del cliente
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PongMessage {
    pub timestamp: DateTime<Utc>,
}
```

### Paso 2: Integrar Heartbeat en Connection Handler

```rust
// src/ws/handler.rs (modificacion)
use super::heartbeat::{HeartbeatConfig, HeartbeatManager, PingMessage};
use tokio::time::{sleep, timeout};

#[instrument(skip(socket, state), fields(connection_id = %conn_info.id))]
async fn handle_socket(
    socket: WebSocket,
    mut conn_info: ConnectionInfo,
    state: AppState,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (msg_tx, mut msg_rx) = mpsc::channel::<ServerMessage>(100);

    // Registrar conexion
    let handle = ConnectionHandle::new(conn_info.clone(), msg_tx);
    state.ws_registry.register(handle);

    // Inicializar heartbeat manager
    let mut heartbeat = HeartbeatManager::new(HeartbeatConfig::default());

    conn_info.set_state(ConnectionState::Connected);

    // Enviar config inicial
    if let Err(e) = send_initial_config(&mut ws_sender, &conn_info, &state).await {
        error!(error = %e, "Failed to send initial config");
        state.ws_registry.unregister(conn_info.id);
        return;
    }

    // Loop principal con heartbeat
    loop {
        // Calcular timeout hasta la proxima accion
        let next_action = heartbeat.time_until_next_action();

        tokio::select! {
            // Mensajes entrantes del WebSocket
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        heartbeat.record_activity();
                        conn_info.touch();

                        if let Err(e) = handle_text_message(
                            &text,
                            &mut ws_sender,
                            &conn_info,
                            &state,
                            &mut heartbeat,
                        ).await {
                            warn!(error = %e, "Error handling message");
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {
                        // WebSocket-level pong (no JSON)
                        if let Some(latency) = heartbeat.record_pong() {
                            debug!(latency_ms = latency.as_millis(), "Pong received");
                            // metrics::histogram!("ws_ping_latency_seconds")
                            //     .record(latency.as_secs_f64());
                        }
                    }
                    Some(Ok(Message::Close(reason))) => {
                        info!(reason = ?reason, "Client closed connection");
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

            // Mensajes del broadcaster
            msg = msg_rx.recv() => {
                match msg {
                    Some(server_msg) => {
                        if let Ok(json) = server_msg.to_json() {
                            if let Err(e) = ws_sender.send(Message::Text(json)).await {
                                error!(error = %e, "Failed to send message");
                                break;
                            }
                        }
                    }
                    None => {
                        info!("Message channel closed");
                        break;
                    }
                }
            }

            // Timer para heartbeat
            _ = sleep(next_action) => {
                // Verificar timeout primero
                if heartbeat.is_timed_out() {
                    warn!("Connection timed out, closing");
                    // Enviar close frame
                    let _ = ws_sender.close().await;
                    break;
                }

                // Enviar ping si es necesario
                if heartbeat.should_ping() {
                    if let Err(e) = send_ping(&mut ws_sender).await {
                        warn!(error = %e, "Failed to send ping");
                        break;
                    }
                    heartbeat.ping_sent();
                    debug!("Ping sent");
                }
            }

            // Senal de shutdown
            _ = state.shutdown_signal.notified() => {
                info!("Shutdown signal received");
                send_closing_message(&mut ws_sender, "server_shutdown", Some(5000)).await;
                break;
            }
        }
    }

    // Cleanup
    state.ws_registry.unregister(conn_info.id);
    info!("Connection handler terminated");
}

/// Envia un ping WebSocket
async fn send_ping(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
) -> Result<(), axum::Error> {
    // Usar WebSocket ping frame (mas eficiente que JSON)
    sender
        .send(Message::Ping(vec![]))
        .await
        .map_err(|e| axum::Error::new(e))
}

/// Envia mensaje de cierre al cliente
async fn send_closing_message(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    reason: &str,
    reconnect_after_ms: Option<u64>,
) {
    let msg = serde_json::json!({
        "type": "connection_closing",
        "reason": reason,
        "reconnect_after_ms": reconnect_after_ms
    });

    if let Ok(json) = serde_json::to_string(&msg) {
        let _ = sender.send(Message::Text(json)).await;
    }
}

/// Procesa mensaje de pong del cliente (JSON level)
fn handle_pong_message(
    pong: &PongMessage,
    heartbeat: &mut HeartbeatManager,
) {
    if let Some(latency) = heartbeat.record_pong() {
        info!(
            latency_ms = latency.as_millis(),
            client_time = %pong.timestamp,
            "Pong received from client"
        );
    }
}
```

### Paso 3: Implementar Reconexion con Estado

```rust
// src/ws/reconnect.rs
use std::collections::VecDeque;
use std::sync::Arc;
use parking_lot::RwLock;
use chrono::{DateTime, Utc};

/// Historial de versiones recientes para reconexion
#[derive(Debug)]
pub struct VersionHistory {
    /// Cambios recientes ordenados por tiempo
    history: RwLock<VecDeque<VersionEntry>>,
    /// Maximo de entries a mantener
    max_entries: usize,
    /// Tiempo maximo de retencion
    max_age: std::time::Duration,
}

#[derive(Debug, Clone)]
pub struct VersionEntry {
    pub app: String,
    pub profile: String,
    pub label: String,
    pub version: String,
    pub config: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

impl VersionHistory {
    pub fn new(max_entries: usize, max_age: std::time::Duration) -> Self {
        Self {
            history: RwLock::new(VecDeque::with_capacity(max_entries)),
            max_entries,
            max_age,
        }
    }

    /// Registra una nueva version
    pub fn record(&self, entry: VersionEntry) {
        let mut history = self.history.write();

        // Limpiar entries viejas
        let cutoff = Utc::now() - chrono::Duration::from_std(self.max_age).unwrap_or_default();
        while history.front().map(|e| e.timestamp < cutoff).unwrap_or(false) {
            history.pop_front();
        }

        // Agregar nueva entry
        if history.len() >= self.max_entries {
            history.pop_front();
        }
        history.push_back(entry);
    }

    /// Obtiene cambios desde una version
    pub fn changes_since(
        &self,
        app: &str,
        profile: &str,
        label: &str,
        since_version: &str,
    ) -> Vec<VersionEntry> {
        let history = self.history.read();

        let mut found_start = false;
        let mut changes = Vec::new();

        for entry in history.iter() {
            if entry.app == app && entry.profile == profile && entry.label == label {
                if found_start {
                    changes.push(entry.clone());
                } else if entry.version == since_version {
                    found_start = true;
                }
            }
        }

        changes
    }

    /// Obtiene la version actual
    pub fn current_version(&self, app: &str, profile: &str, label: &str) -> Option<String> {
        let history = self.history.read();

        history
            .iter()
            .rev()
            .find(|e| e.app == app && e.profile == profile && e.label == label)
            .map(|e| e.version.clone())
    }
}

/// Mensaje de informacion de reconexion
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReconnectInfo {
    /// Versiones perdidas durante desconexion
    pub missed_versions: Vec<String>,
    /// Version actual
    pub current_version: String,
    /// Configuracion actual completa (si hubo cambios)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_config: Option<serde_json::Value>,
}

/// Handler para mensajes de resync
pub async fn handle_resync(
    app: &str,
    profile: &str,
    label: &str,
    last_version: Option<String>,
    version_history: &VersionHistory,
    config_source: &dyn ConfigSource,
) -> Result<ServerMessage, Box<dyn std::error::Error + Send + Sync>> {
    match last_version {
        Some(version) => {
            // Cliente tiene version anterior, enviar diff
            let changes = version_history.changes_since(app, profile, label, &version);

            if changes.is_empty() {
                // No hay cambios, cliente esta actualizado
                Ok(ServerMessage::ReconnectInfo {
                    missed_versions: vec![],
                    current_version: version,
                    current_config: None,
                })
            } else {
                // Hay cambios, enviar lista de versiones perdidas
                let missed: Vec<String> = changes.iter().map(|c| c.version.clone()).collect();
                let current = changes.last().unwrap();

                Ok(ServerMessage::ReconnectInfo {
                    missed_versions: missed,
                    current_version: current.version.clone(),
                    current_config: Some(current.config.clone()),
                })
            }
        }
        None => {
            // Cliente no tiene version, enviar snapshot completo
            let config = config_source.get_config(app, profile, label).await?;

            Ok(ServerMessage::snapshot(
                app,
                profile,
                label,
                config.to_json_value(),
                config.version.unwrap_or_default(),
            ))
        }
    }
}
```

### Paso 4: Agregar Shutdown Signal a AppState

```rust
// src/server.rs (modificacion)
use tokio::sync::Notify;

pub struct AppState {
    pub config_source: Arc<dyn ConfigSource>,
    pub ws_registry: Arc<ConnectionRegistry>,
    pub broadcaster: Arc<ConfigChangeBroadcaster>,
    pub version_history: Arc<VersionHistory>,
    /// Senal para shutdown graceful
    pub shutdown_signal: Arc<Notify>,
}

impl AppState {
    pub fn new(config_source: Arc<dyn ConfigSource>) -> Self {
        let registry = Arc::new(ConnectionRegistry::new());
        let broadcaster = Arc::new(ConfigChangeBroadcaster::new(
            Arc::clone(&registry),
            100,
        ));
        let version_history = Arc::new(VersionHistory::new(
            1000,  // Max 1000 entries
            std::time::Duration::from_secs(3600),  // 1 hour retention
        ));

        Self {
            config_source,
            ws_registry: registry,
            broadcaster,
            version_history,
            shutdown_signal: Arc::new(Notify::new()),
        }
    }

    /// Inicia el shutdown graceful
    pub fn initiate_shutdown(&self) {
        self.shutdown_signal.notify_waiters();
    }
}

/// Configura el servidor con graceful shutdown
pub async fn run_server_with_shutdown(addr: SocketAddr) -> Result<(), std::io::Error> {
    let state = AppState::new(/* ... */);
    let app = create_router(state.clone());

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Server listening on {}", addr);

    // Manejar SIGTERM/SIGINT
    let shutdown_signal = state.shutdown_signal.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl-c");
        info!("Shutdown signal received, draining connections...");
        shutdown_signal.notify_waiters();
    });

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            state.shutdown_signal.notified().await;
            // Dar tiempo a las conexiones para cerrar
            tokio::time::sleep(Duration::from_secs(5)).await;
        })
        .await
}
```

### Paso 5: Actualizar Mensajes

```rust
// src/ws/messages.rs (agregar)

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    // ... existing variants ...

    /// Ping para heartbeat
    Ping {
        timestamp: DateTime<Utc>,
    },

    /// Servidor va a cerrar
    ConnectionClosing {
        reason: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        reconnect_after_ms: Option<u64>,
    },

    /// Informacion de reconexion
    ReconnectInfo {
        missed_versions: Vec<String>,
        current_version: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        current_config: Option<serde_json::Value>,
    },
}
```

---

## Conceptos de Rust Aprendidos

### 1. tokio::select! con Multiples Branches

`select!` permite esperar multiples futures concurrentemente, ejecutando el primero que complete.

**Rust:**
```rust
use tokio::select;
use tokio::time::{sleep, Duration};

loop {
    select! {
        // Branch 1: Mensaje del WebSocket
        msg = ws_receiver.next() => {
            match msg {
                Some(Ok(m)) => handle_message(m).await,
                _ => break,
            }
        }

        // Branch 2: Mensaje del broadcaster
        msg = broadcast_rx.recv() => {
            send_to_client(msg).await;
        }

        // Branch 3: Timer de heartbeat
        _ = sleep(Duration::from_secs(30)) => {
            send_ping().await;
        }

        // Branch 4: Senal de shutdown
        _ = shutdown_signal.notified() => {
            info!("Shutting down");
            break;
        }
    }
}
```

**Comparacion con Java:**
```java
// Java: No hay equivalente directo, usar CompletableFuture.anyOf
while (true) {
    CompletableFuture<?> first = CompletableFuture.anyOf(
        wsReceiver.receiveAsync(),
        broadcastRx.receiveAsync(),
        CompletableFuture.delayedExecutor(30, TimeUnit.SECONDS)
            .execute(() -> "timeout"),
        shutdownFuture
    );

    Object result = first.get();
    if (result instanceof WsMessage) {
        handleMessage((WsMessage) result);
    } else if (result instanceof BroadcastMessage) {
        sendToClient((BroadcastMessage) result);
    } else if ("timeout".equals(result)) {
        sendPing();
    } else if (result instanceof ShutdownSignal) {
        break;
    }
}
```

**Diferencias clave:**

| Aspecto | tokio::select! | CompletableFuture.anyOf |
|---------|----------------|-------------------------|
| Cancelacion | Automatica de ramas no seleccionadas | Manual |
| Pattern matching | Integrado | instanceof/cast |
| Fairness | Configurable con biased | FIFO |
| Overhead | Compile-time | Runtime |

### 2. tokio::sync::Notify para Senales

`Notify` permite que una tarea notifique a otras sin enviar datos.

**Rust:**
```rust
use std::sync::Arc;
use tokio::sync::Notify;

// Crear senal compartida
let shutdown_signal = Arc::new(Notify::new());

// En el handler de SIGTERM
let signal_clone = shutdown_signal.clone();
tokio::spawn(async move {
    tokio::signal::ctrl_c().await.unwrap();
    signal_clone.notify_waiters();  // Notifica a TODOS los que esperan
});

// En cada conexion WebSocket
loop {
    select! {
        _ = shutdown_signal.notified() => {
            // Recibimos la notificacion
            send_closing_message().await;
            break;
        }
        // ... otras ramas
    }
}
```

**Comparacion con Java:**
```java
// Java: CountDownLatch o similar
CountDownLatch shutdownLatch = new CountDownLatch(1);

// En el handler de shutdown
Runtime.getRuntime().addShutdownHook(new Thread(() -> {
    shutdownLatch.countDown();
}));

// En cada conexion (bloqueante!)
if (shutdownLatch.await(0, TimeUnit.MILLISECONDS)) {
    sendClosingMessage();
    break;
}

// Mejor: usar AtomicBoolean con polling
AtomicBoolean shutdown = new AtomicBoolean(false);

// En shutdown hook
shutdown.set(true);

// En conexion
if (shutdown.get()) {
    break;
}
```

### 3. Timeouts con tokio::time

Manejar timeouts de forma async sin bloquear.

**Rust:**
```rust
use tokio::time::{timeout, Duration, Instant};

// Timeout simple
match timeout(Duration::from_secs(10), receive_pong()).await {
    Ok(pong) => {
        // Pong recibido a tiempo
        process_pong(pong);
    }
    Err(_) => {
        // Timeout!
        warn!("Pong timeout, closing connection");
        return Err(TimeoutError);
    }
}

// Calcular tiempo restante
let deadline = Instant::now() + Duration::from_secs(30);
loop {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        break; // Timeout alcanzado
    }

    select! {
        result = operation() => {
            handle_result(result);
        }
        _ = tokio::time::sleep(remaining) => {
            // Timeout
            break;
        }
    }
}
```

**Comparacion con Java:**
```java
// Java: Future.get con timeout
try {
    Pong pong = receivePong().get(10, TimeUnit.SECONDS);
    processPong(pong);
} catch (TimeoutException e) {
    logger.warn("Pong timeout");
    throw e;
}

// O con CompletableFuture
receivePong()
    .orTimeout(10, TimeUnit.SECONDS)
    .exceptionally(e -> {
        if (e instanceof TimeoutException) {
            logger.warn("Pong timeout");
        }
        return null;
    });
```

### 4. Graceful Shutdown Pattern

El patron de shutdown graceful asegura que las conexiones se cierren limpiamente.

**Rust:**
```rust
// Servidor principal
async fn run_server(state: AppState) {
    let shutdown = state.shutdown_signal.clone();

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            // Esperar senal
            shutdown.notified().await;
            info!("Starting graceful shutdown");

            // Dar tiempo a conexiones
            tokio::time::sleep(Duration::from_secs(5)).await;
        })
        .await
}

// Cada conexion escucha la senal
loop {
    select! {
        _ = state.shutdown_signal.notified() => {
            // Notificar al cliente
            send_closing_message(&mut sender, "server_shutdown", Some(5000)).await;

            // Cerrar conexion
            let _ = sender.close().await;
            break;
        }
        // ... otras ramas
    }
}
```

**Comparacion con Java (Spring):**
```java
@PreDestroy
public void onShutdown() {
    logger.info("Shutting down, closing WebSocket connections");

    // Spring maneja el cierre de sesiones
    for (WebSocketSession session : sessions.values()) {
        try {
            session.sendMessage(new TextMessage("{\"type\":\"connection_closing\"}"));
            session.close(CloseStatus.GOING_AWAY);
        } catch (IOException e) {
            logger.warn("Error closing session", e);
        }
    }
}
```

---

## Riesgos y Errores Comunes

### 1. Thundering Herd en Reconexion

```rust
// MAL: Todos los clientes reconectan exactamente al mismo tiempo
// cuando el servidor reinicia

// BIEN: Agregar jitter
impl HeartbeatConfig {
    pub fn interval_with_jitter(&self) -> Duration {
        let jitter_ms = rand::thread_rng()
            .gen_range(0..self.interval.as_millis() as u64 / 5);
        self.interval + Duration::from_millis(jitter_ms)
    }
}

// El cliente tambien deberia implementar backoff
// reconnect_delay = min(base * 2^attempt + jitter, max_delay)
```

### 2. Ping/Pong confusion (WebSocket vs JSON)

```rust
// WebSocket tiene ping/pong nativos (binary frames)
// Ademas podemos tener ping/pong como mensajes JSON

// MAL: Mezclar ambos sin claridad
Message::Ping(_) => { /* WebSocket frame */ }
Message::Text(t) if t.contains("ping") => { /* JSON message */ }

// BIEN: Usar WebSocket ping/pong para heartbeat (mas eficiente)
// Reservar JSON para logica de aplicacion

// WebSocket level (recomendado para heartbeat)
sender.send(Message::Ping(vec![])).await?;
// Pong es automatico en la mayoria de implementaciones

// JSON level (para mensajes de aplicacion)
sender.send(Message::Text(r#"{"type":"status_ping"}"#.into())).await?;
```

### 3. Olvidar cancelar el timer

```rust
// MAL: Timer sigue corriendo despues de cerrar conexion
let timer_handle = tokio::spawn(async {
    loop {
        sleep(Duration::from_secs(30)).await;
        send_ping().await;  // Error: sender ya cerrado!
    }
});

// BIEN: Usar select! que cancela automaticamente
loop {
    select! {
        _ = ws_receiver.next() => { ... }
        _ = sleep(Duration::from_secs(30)) => {
            send_ping().await;
        }
    }
}
// Al salir del loop, el sleep future se dropea y cancela
```

### 4. Race condition en shutdown

```rust
// MAL: Enviar mensaje despues de cerrar
send_closing_message(&mut sender).await;
sender.close().await;  // OK

// Pero si hay un broadcast pendiente...
// broadcast -> intenta enviar -> error porque ya cerro

// BIEN: Marcar conexion como "closing" antes
conn_info.set_state(ConnectionState::Closing);

// En el broadcaster, verificar estado
if handle.info.state == ConnectionState::Closing {
    continue;  // No enviar a conexiones cerrando
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
    fn test_heartbeat_should_ping() {
        let config = HeartbeatConfig {
            interval: Duration::from_millis(100),
            timeout: Duration::from_millis(50),
            jitter: 0.0,
        };

        let mut hb = HeartbeatManager::new(config);

        // Inicialmente no deberia hacer ping
        assert!(!hb.should_ping());

        // Simular paso de tiempo
        std::thread::sleep(Duration::from_millis(150));

        // Ahora deberia hacer ping
        assert!(hb.should_ping());
    }

    #[test]
    fn test_heartbeat_timeout() {
        let config = HeartbeatConfig {
            interval: Duration::from_millis(100),
            timeout: Duration::from_millis(50),
            jitter: 0.0,
        };

        let mut hb = HeartbeatManager::new(config);

        // Simular envio de ping
        hb.ping_sent();

        // No deberia estar en timeout aun
        assert!(!hb.is_timed_out());

        // Esperar mas que el timeout
        std::thread::sleep(Duration::from_millis(60));

        // Ahora deberia estar en timeout
        assert!(hb.is_timed_out());
    }

    #[test]
    fn test_heartbeat_pong_resets() {
        let config = HeartbeatConfig {
            interval: Duration::from_millis(100),
            timeout: Duration::from_millis(50),
            jitter: 0.0,
        };

        let mut hb = HeartbeatManager::new(config);

        hb.ping_sent();
        std::thread::sleep(Duration::from_millis(30));

        // Recibir pong
        let latency = hb.record_pong();
        assert!(latency.is_some());
        assert!(latency.unwrap() >= Duration::from_millis(30));

        // No deberia estar en timeout
        assert!(!hb.is_timed_out());
    }

    #[test]
    fn test_version_history() {
        let history = VersionHistory::new(10, Duration::from_secs(3600));

        // Agregar entries
        history.record(VersionEntry {
            app: "myapp".to_string(),
            profile: "prod".to_string(),
            label: "main".to_string(),
            version: "v1".to_string(),
            config: serde_json::json!({}),
            timestamp: Utc::now(),
        });

        history.record(VersionEntry {
            app: "myapp".to_string(),
            profile: "prod".to_string(),
            label: "main".to_string(),
            version: "v2".to_string(),
            config: serde_json::json!({}),
            timestamp: Utc::now(),
        });

        // Obtener cambios desde v1
        let changes = history.changes_since("myapp", "prod", "main", "v1");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].version, "v2");
    }
}
```

### Tests de Integracion

```rust
// tests/ws_heartbeat_test.rs
#[tokio::test]
async fn test_connection_timeout() {
    let app = create_test_app_with_config(HeartbeatConfig {
        interval: Duration::from_millis(100),
        timeout: Duration::from_millis(50),
        jitter: 0.0,
    }).await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Conectar pero no responder a pings
    let url = format!("ws://{}/ws/myapp/prod", addr);
    let (mut ws, _) = connect_async(&url).await.unwrap();

    // Consumir mensaje inicial
    let _ = ws.next().await;

    // Esperar ping (100ms)
    tokio::time::sleep(Duration::from_millis(120)).await;

    // Ignorar el ping - no enviar pong

    // Esperar timeout (50ms adicionales)
    tokio::time::sleep(Duration::from_millis(60)).await;

    // La conexion deberia cerrarse
    let result = tokio::time::timeout(
        Duration::from_millis(100),
        ws.next()
    ).await;

    match result {
        Ok(Some(Ok(Message::Close(_)))) => {
            // Esperado: servidor cerro la conexion
        }
        Ok(None) => {
            // Esperado: stream termino
        }
        other => {
            panic!("Expected connection close, got: {:?}", other);
        }
    }
}

#[tokio::test]
async fn test_graceful_shutdown() {
    let (app, state) = create_test_app_with_state().await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Conectar cliente
    let url = format!("ws://{}/ws/myapp/prod", addr);
    let (mut ws, _) = connect_async(&url).await.unwrap();

    // Consumir mensaje inicial
    let _ = ws.next().await;

    // Iniciar shutdown
    state.initiate_shutdown();

    // Cliente deberia recibir mensaje de cierre
    let msg = tokio::time::timeout(
        Duration::from_millis(100),
        ws.next()
    ).await.unwrap().unwrap().unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&msg.into_text().unwrap()).unwrap();
    assert_eq!(parsed["type"], "connection_closing");
    assert_eq!(parsed["reason"], "server_shutdown");
}
```

---

## Observabilidad

### Logging

```rust
#[instrument(skip(self), fields(state = ?self.state))]
fn check_heartbeat(&mut self) -> HeartbeatAction {
    if self.is_timed_out() {
        warn!("Heartbeat timeout");
        return HeartbeatAction::Close;
    }

    if self.should_ping() {
        debug!("Sending heartbeat ping");
        return HeartbeatAction::Ping;
    }

    HeartbeatAction::Wait(self.time_until_next_action())
}
```

### Metricas

```rust
// Latencia de ping/pong
if let Some(latency) = heartbeat.record_pong() {
    metrics::histogram!("ws_ping_latency_seconds").record(latency.as_secs_f64());
}

// Timeouts
if heartbeat.is_timed_out() {
    metrics::counter!("ws_timeout_total").increment(1);
}

// Conexiones activas
metrics::gauge!("ws_connections_active", registry.connection_count() as f64);

// Graceful shutdowns
metrics::counter!("ws_graceful_close_total").increment(1);
```

---

## Entregable Final

### Archivos Creados/Modificados

1. `crates/vortex-server/src/ws/heartbeat.rs` - HeartbeatManager
2. `crates/vortex-server/src/ws/reconnect.rs` - VersionHistory y reconexion
3. `crates/vortex-server/src/ws/handler.rs` - Integracion de heartbeat
4. `crates/vortex-server/src/ws/messages.rs` - Mensajes de lifecycle
5. `crates/vortex-server/src/server.rs` - Shutdown signal en AppState
6. `crates/vortex-server/tests/ws_heartbeat_test.rs` - Tests

### Verificacion

```bash
# Compilar
cargo build -p vortex-server

# Tests de heartbeat
cargo test -p vortex-server heartbeat

# Tests de reconexion
cargo test -p vortex-server reconnect

# Clippy
cargo clippy -p vortex-server -- -D warnings
```

### Ejemplo de Uso (Cliente JavaScript)

```javascript
class VortexConfigClient {
    constructor(url) {
        this.url = url;
        this.lastVersion = null;
        this.reconnectAttempts = 0;
        this.maxReconnectDelay = 30000;
    }

    connect() {
        this.ws = new WebSocket(this.url);

        this.ws.onopen = () => {
            console.log('Connected');
            this.reconnectAttempts = 0;

            // Si tenemos version anterior, pedir resync
            if (this.lastVersion) {
                this.ws.send(JSON.stringify({
                    type: 'resync',
                    last_version: this.lastVersion
                }));
            }
        };

        this.ws.onmessage = (event) => {
            const msg = JSON.parse(event.data);

            switch (msg.type) {
                case 'config_snapshot':
                    this.lastVersion = msg.version;
                    this.onConfig(msg.config);
                    break;

                case 'config_change':
                    this.lastVersion = msg.new_version;
                    this.onDiff(msg.diff);
                    break;

                case 'ping':
                    this.ws.send(JSON.stringify({
                        type: 'pong',
                        timestamp: new Date().toISOString()
                    }));
                    break;

                case 'connection_closing':
                    console.log('Server closing:', msg.reason);
                    if (msg.reconnect_after_ms) {
                        setTimeout(() => this.connect(), msg.reconnect_after_ms);
                    }
                    break;
            }
        };

        this.ws.onclose = (event) => {
            console.log('Disconnected:', event.code);
            this.scheduleReconnect();
        };

        this.ws.onerror = (error) => {
            console.error('WebSocket error:', error);
        };
    }

    scheduleReconnect() {
        const delay = Math.min(
            1000 * Math.pow(2, this.reconnectAttempts) + Math.random() * 1000,
            this.maxReconnectDelay
        );

        console.log(`Reconnecting in ${delay}ms...`);
        this.reconnectAttempts++;

        setTimeout(() => this.connect(), delay);
    }
}
```

---

**Anterior**: [Historia 003 - Diff Semantico](./story-003-semantic-diff.md)
**Siguiente**: [Historia 005 - Tests de WebSockets](./story-005-websocket-tests.md)
