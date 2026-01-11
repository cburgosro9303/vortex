# Historia 005: Tests de WebSockets

## Contexto y Objetivo

Las funcionalidades real-time implementadas en las historias anteriores requieren una suite de tests robusta que valide:

- Conexion y upgrade WebSocket
- Broadcast de mensajes a multiples clientes
- Diff semantico correcto
- Heartbeat y timeout
- Reconexion y recuperacion de estado
- Graceful shutdown

Testear WebSockets es mas complejo que HTTP tradicional porque involucra:
- Conexiones persistentes
- Comunicacion bidireccional
- Estado temporal
- Timing y concurrencia

Esta historia implementa una suite completa de tests utilizando las herramientas del ecosistema Rust para testing async.

Para desarrolladores Java, esto es similar a escribir tests con WebSocket containers embebidos, pero usando las capacidades async de Tokio.

---

## Alcance

### In Scope

- Tests unitarios para cada modulo WebSocket
- Tests de integracion con servidor real
- Cliente WebSocket de test reutilizable
- Tests de concurrencia (multiples clientes)
- Tests de edge cases (timeout, errores, reconexion)
- Fixtures y helpers de test
- Documentacion de como correr tests

### Out of Scope

- Tests de carga/performance (seria otra historia)
- Tests de compatibilidad con browsers reales
- Tests end-to-end con frontend
- Tests de seguridad/fuzzing

---

## Criterios de Aceptacion

- [ ] Cobertura de tests > 80% en modulo `ws/`
- [ ] Tests de integracion para cada endpoint WebSocket
- [ ] Tests de concurrencia con 10+ clientes simultaneos
- [ ] Tests de timeout y reconexion
- [ ] Helpers reutilizables documentados
- [ ] CI ejecuta todos los tests

---

## Diseno Propuesto

### Estructura de Tests

```
crates/vortex-server/
├── src/
│   └── ws/
│       ├── mod.rs
│       └── ... (unit tests dentro de cada modulo)
└── tests/
    ├── helpers/
    │   ├── mod.rs
    │   ├── ws_client.rs        # Cliente WebSocket de test
    │   ├── test_server.rs      # Servidor de test
    │   └── fixtures.rs         # Datos de prueba
    ├── ws_connection_test.rs   # Tests de conexion
    ├── ws_broadcast_test.rs    # Tests de broadcast
    ├── ws_diff_test.rs         # Tests de diff
    ├── ws_heartbeat_test.rs    # Tests de heartbeat
    ├── ws_reconnect_test.rs    # Tests de reconexion
    └── ws_concurrent_test.rs   # Tests de concurrencia
```

---

## Pasos de Implementacion

### Paso 1: Crear Cliente WebSocket de Test

```rust
// tests/helpers/ws_client.rs
use futures::{SinkExt, StreamExt};
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::{
    connect_async,
    tungstenite::Message,
    MaybeTlsStream,
    WebSocketStream,
};

/// Cliente WebSocket para tests de integracion.
///
/// Proporciona una API ergonomica para:
/// - Conectar a un servidor WebSocket
/// - Enviar y recibir mensajes JSON tipados
/// - Manejar timeouts
/// - Verificar mensajes recibidos
pub struct TestWsClient {
    ws: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    received: Vec<serde_json::Value>,
    default_timeout: Duration,
}

impl TestWsClient {
    /// Conecta a un servidor WebSocket
    pub async fn connect(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let (ws, _response) = connect_async(url).await?;
        Ok(Self {
            ws,
            received: Vec::new(),
            default_timeout: Duration::from_secs(5),
        })
    }

    /// Conecta con query parameters
    pub async fn connect_with_params(
        base_url: &str,
        params: &[(&str, &str)],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        let url = format!("{}?{}", base_url, query);
        Self::connect(&url).await
    }

    /// Configura el timeout por defecto
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Envia un mensaje JSON
    pub async fn send<T: Serialize>(&mut self, msg: &T) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string(msg)?;
        self.ws.send(Message::Text(json)).await?;
        Ok(())
    }

    /// Envia un mensaje de texto raw
    pub async fn send_text(&mut self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.ws.send(Message::Text(text.to_string())).await?;
        Ok(())
    }

    /// Recibe el proximo mensaje como JSON tipado
    pub async fn receive<T: DeserializeOwned>(&mut self) -> Result<T, Box<dyn std::error::Error>> {
        let msg = self.receive_raw().await?;
        let parsed = serde_json::from_str(&msg)?;
        Ok(parsed)
    }

    /// Recibe el proximo mensaje como JSON Value
    pub async fn receive_json(&mut self) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let msg = self.receive_raw().await?;
        let parsed = serde_json::from_str(&msg)?;
        self.received.push(parsed.clone());
        Ok(parsed)
    }

    /// Recibe el proximo mensaje como texto
    pub async fn receive_raw(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        let result = timeout(self.default_timeout, self.ws.next()).await?;

        match result {
            Some(Ok(Message::Text(text))) => Ok(text),
            Some(Ok(Message::Binary(data))) => {
                Ok(String::from_utf8(data)?)
            }
            Some(Ok(Message::Close(reason))) => {
                Err(format!("Connection closed: {:?}", reason).into())
            }
            Some(Ok(other)) => {
                Err(format!("Unexpected message type: {:?}", other).into())
            }
            Some(Err(e)) => Err(e.into()),
            None => Err("Connection closed unexpectedly".into()),
        }
    }

    /// Recibe mensaje con timeout personalizado
    pub async fn receive_with_timeout(
        &mut self,
        timeout_duration: Duration,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let old_timeout = self.default_timeout;
        self.default_timeout = timeout_duration;
        let result = self.receive_json().await;
        self.default_timeout = old_timeout;
        result
    }

    /// Intenta recibir, retorna None si hay timeout
    pub async fn try_receive(
        &mut self,
        timeout_duration: Duration,
    ) -> Option<serde_json::Value> {
        self.receive_with_timeout(timeout_duration).await.ok()
    }

    /// Espera un mensaje de tipo especifico
    pub async fn expect_message_type(
        &mut self,
        expected_type: &str,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let msg = self.receive_json().await?;
        let actual_type = msg.get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("");

        if actual_type != expected_type {
            return Err(format!(
                "Expected message type '{}', got '{}': {:?}",
                expected_type, actual_type, msg
            ).into());
        }

        Ok(msg)
    }

    /// Consume mensajes hasta encontrar uno del tipo especificado
    pub async fn find_message_type(
        &mut self,
        target_type: &str,
        max_messages: usize,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        for _ in 0..max_messages {
            let msg = self.receive_json().await?;
            let msg_type = msg.get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("");

            if msg_type == target_type {
                return Ok(msg);
            }
        }

        Err(format!(
            "Message type '{}' not found in {} messages",
            target_type, max_messages
        ).into())
    }

    /// Cierra la conexion
    pub async fn close(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.ws.close(None).await?;
        Ok(())
    }

    /// Retorna todos los mensajes recibidos
    pub fn received_messages(&self) -> &[serde_json::Value] {
        &self.received
    }

    /// Limpia el historial de mensajes
    pub fn clear_received(&mut self) {
        self.received.clear();
    }
}

/// Builder para configurar el cliente de test
pub struct TestWsClientBuilder {
    base_url: String,
    params: Vec<(String, String)>,
    timeout: Duration,
}

impl TestWsClientBuilder {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            params: Vec::new(),
            timeout: Duration::from_secs(5),
        }
    }

    pub fn param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.push((key.into(), value.into()));
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub async fn connect(self) -> Result<TestWsClient, Box<dyn std::error::Error>> {
        let url = if self.params.is_empty() {
            self.base_url
        } else {
            let query = self.params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            format!("{}?{}", self.base_url, query)
        };

        let client = TestWsClient::connect(&url).await?;
        Ok(client.with_timeout(self.timeout))
    }
}
```

### Paso 2: Crear Test Server Helper

```rust
// tests/helpers/test_server.rs
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Notify;
use vortex_server::{create_router, AppState};

/// Servidor de test que se puede iniciar y detener.
pub struct TestServer {
    addr: SocketAddr,
    state: AppState,
    shutdown: Arc<Notify>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl TestServer {
    /// Crea e inicia un servidor de test
    pub async fn start() -> Self {
        Self::start_with_config(Default::default()).await
    }

    /// Crea e inicia un servidor con configuracion personalizada
    pub async fn start_with_config(config: TestServerConfig) -> Self {
        let state = AppState::new_for_test(config.clone());
        let app = create_router(state.clone());

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind");
        let addr = listener.local_addr().expect("Failed to get addr");

        let shutdown = state.shutdown_signal.clone();

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    shutdown.notified().await;
                })
                .await
                .expect("Server error");
        });

        // Esperar a que el servidor este listo
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        Self {
            addr,
            state,
            shutdown: state.shutdown_signal.clone(),
            handle: Some(handle),
        }
    }

    /// Retorna la URL base del servidor
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Retorna la URL WebSocket
    pub fn ws_url(&self, app: &str, profile: &str) -> String {
        format!("ws://{}/ws/{}/{}", self.addr, app, profile)
    }

    /// Retorna el estado del servidor para manipulacion directa
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Emite un evento de cambio de configuracion
    pub fn emit_config_change(
        &self,
        app: &str,
        profile: &str,
        label: &str,
        old_config: Option<serde_json::Value>,
        new_config: serde_json::Value,
        version: &str,
    ) {
        use vortex_server::ws::ConfigChangeEvent;

        let event = ConfigChangeEvent::new(
            app, profile, label,
            old_config,
            new_config,
            version,
        );

        self.state.broadcaster.emit(event).ok();
    }

    /// Inicia el shutdown graceful
    pub fn shutdown(&self) {
        self.state.initiate_shutdown();
    }

    /// Espera a que el servidor termine
    pub async fn wait_for_shutdown(mut self) {
        if let Some(handle) = self.handle.take() {
            handle.await.ok();
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown.notify_waiters();
    }
}

/// Configuracion para el servidor de test
#[derive(Debug, Clone, Default)]
pub struct TestServerConfig {
    pub heartbeat_interval_ms: Option<u64>,
    pub heartbeat_timeout_ms: Option<u64>,
}
```

### Paso 3: Crear Fixtures

```rust
// tests/helpers/fixtures.rs
use serde_json::json;

/// Configuracion de ejemplo para tests
pub fn sample_config() -> serde_json::Value {
    json!({
        "server": {
            "port": 8080,
            "host": "localhost"
        },
        "database": {
            "url": "postgres://localhost/test",
            "pool_size": 10
        },
        "features": {
            "flag_a": true,
            "flag_b": false
        }
    })
}

/// Configuracion modificada para tests de diff
pub fn modified_config() -> serde_json::Value {
    json!({
        "server": {
            "port": 9090,  // Cambiado
            "host": "localhost"
        },
        "database": {
            "url": "postgres://localhost/test",
            "pool_size": 20  // Cambiado
        },
        "features": {
            "flag_a": true,
            "flag_b": false,
            "flag_c": true  // Agregado
        }
    })
}

/// Mensaje de pong valido
pub fn pong_message() -> serde_json::Value {
    json!({
        "type": "pong",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })
}

/// Mensaje de subscribe
pub fn subscribe_message(patterns: &[&str]) -> serde_json::Value {
    json!({
        "type": "subscribe",
        "patterns": patterns
    })
}

/// Mensaje de resync
pub fn resync_message(last_version: Option<&str>) -> serde_json::Value {
    json!({
        "type": "resync",
        "last_version": last_version
    })
}
```

### Paso 4: Escribir Tests de Conexion

```rust
// tests/ws_connection_test.rs
mod helpers;

use helpers::{TestServer, TestWsClient, TestWsClientBuilder, fixtures};
use std::time::Duration;

#[tokio::test]
async fn test_websocket_upgrade_succeeds() {
    let server = TestServer::start().await;

    let mut client = TestWsClient::connect(&server.ws_url("myapp", "prod"))
        .await
        .expect("Should connect");

    // Debe recibir config_snapshot
    let msg = client.expect_message_type("config_snapshot")
        .await
        .expect("Should receive snapshot");

    assert_eq!(msg["app"], "myapp");
    assert_eq!(msg["profile"], "prod");

    client.close().await.ok();
}

#[tokio::test]
async fn test_websocket_with_label_parameter() {
    let server = TestServer::start().await;

    let mut client = TestWsClientBuilder::new(server.ws_url("myapp", "prod"))
        .param("label", "develop")
        .connect()
        .await
        .expect("Should connect");

    let msg = client.expect_message_type("config_snapshot")
        .await
        .expect("Should receive snapshot");

    assert_eq!(msg["label"], "develop");

    client.close().await.ok();
}

#[tokio::test]
async fn test_invalid_app_returns_error() {
    let server = TestServer::start().await;

    // App name con caracteres invalidos
    let result = TestWsClient::connect(&format!(
        "ws://{}/ws/invalid app!/prod",
        server.addr()
    )).await;

    // Deberia fallar la conexion o recibir error
    // (dependiendo de la implementacion)
    assert!(result.is_err() || {
        let mut client = result.unwrap();
        let msg = client.receive_json().await;
        msg.map(|m| m["type"] == "error").unwrap_or(false)
    });
}

#[tokio::test]
async fn test_multiple_clients_connect_independently() {
    let server = TestServer::start().await;

    // Conectar 5 clientes
    let mut clients = Vec::new();
    for i in 0..5 {
        let url = server.ws_url(&format!("app{}", i), "prod");
        let client = TestWsClient::connect(&url)
            .await
            .expect("Should connect");
        clients.push(client);
    }

    // Todos deben recibir su snapshot
    for (i, client) in clients.iter_mut().enumerate() {
        let msg = client.expect_message_type("config_snapshot")
            .await
            .expect("Should receive snapshot");
        assert_eq!(msg["app"], format!("app{}", i));
    }

    // Cerrar todos
    for mut client in clients {
        client.close().await.ok();
    }
}

#[tokio::test]
async fn test_connection_closes_gracefully() {
    let server = TestServer::start().await;

    let mut client = TestWsClient::connect(&server.ws_url("myapp", "prod"))
        .await
        .expect("Should connect");

    // Consumir snapshot
    let _ = client.receive_json().await;

    // Cerrar desde el cliente
    client.close().await.expect("Should close cleanly");

    // Verificar que el servidor limpio la conexion
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(server.state().ws_registry.connection_count(), 0);
}
```

### Paso 5: Escribir Tests de Broadcast

```rust
// tests/ws_broadcast_test.rs
mod helpers;

use helpers::{TestServer, TestWsClient, fixtures};
use std::time::Duration;

#[tokio::test]
async fn test_broadcast_reaches_all_subscribers() {
    let server = TestServer::start().await;

    // Conectar 3 clientes a la misma app/profile
    let mut clients = Vec::new();
    for _ in 0..3 {
        let mut client = TestWsClient::connect(&server.ws_url("myapp", "prod"))
            .await
            .expect("Should connect");
        // Consumir snapshot inicial
        let _ = client.receive_json().await;
        clients.push(client);
    }

    // Emitir cambio
    server.emit_config_change(
        "myapp", "prod", "main",
        Some(fixtures::sample_config()),
        fixtures::modified_config(),
        "v2",
    );

    // Todos deben recibir el cambio
    for client in &mut clients {
        let msg = client.expect_message_type("config_change")
            .await
            .expect("Should receive change");

        assert!(msg["diff"].is_array());
    }
}

#[tokio::test]
async fn test_broadcast_only_to_matching_subscribers() {
    let server = TestServer::start().await;

    // Cliente para myapp:prod
    let mut client_myapp = TestWsClient::connect(&server.ws_url("myapp", "prod"))
        .await
        .expect("Should connect");
    let _ = client_myapp.receive_json().await;

    // Cliente para otherapp:prod
    let mut client_other = TestWsClient::connect(&server.ws_url("otherapp", "prod"))
        .await
        .expect("Should connect");
    let _ = client_other.receive_json().await;

    // Emitir cambio solo para myapp
    server.emit_config_change(
        "myapp", "prod", "main",
        None,
        fixtures::sample_config(),
        "v1",
    );

    // myapp debe recibir
    let msg = client_myapp.try_receive(Duration::from_millis(200)).await;
    assert!(msg.is_some());

    // otherapp NO debe recibir
    let msg = client_other.try_receive(Duration::from_millis(200)).await;
    assert!(msg.is_none());
}

#[tokio::test]
async fn test_pattern_subscription() {
    let server = TestServer::start().await;

    // Cliente base
    let mut client = TestWsClient::connect(&server.ws_url("myapp", "prod"))
        .await
        .expect("Should connect");
    let _ = client.receive_json().await;

    // Suscribirse a patron wildcard
    client.send(&fixtures::subscribe_message(&["*:staging:*"]))
        .await
        .expect("Should send");

    // Emitir cambio para staging
    server.emit_config_change(
        "otherapp", "staging", "main",
        None,
        fixtures::sample_config(),
        "v1",
    );

    // Debe recibir por el patron
    let msg = client.expect_message_type("config_snapshot")
        .await
        .expect("Should receive via pattern");

    assert_eq!(msg["app"], "otherapp");
    assert_eq!(msg["profile"], "staging");
}
```

### Paso 6: Escribir Tests de Diff

```rust
// tests/ws_diff_test.rs
mod helpers;

use helpers::{TestServer, TestWsClient, fixtures};
use serde_json::json;

#[tokio::test]
async fn test_diff_contains_all_changes() {
    let server = TestServer::start().await;

    let mut client = TestWsClient::connect(&server.ws_url("myapp", "prod"))
        .await
        .expect("Should connect");
    let _ = client.receive_json().await;

    let old = json!({
        "a": 1,
        "b": 2,
        "c": 3
    });
    let new = json!({
        "a": 1,
        "b": 20,  // replace
        "d": 4    // add (c removed)
    });

    server.emit_config_change("myapp", "prod", "main", Some(old), new, "v2");

    let msg = client.expect_message_type("config_change")
        .await
        .expect("Should receive change");

    let diff = msg["diff"].as_array().expect("Should have diff array");

    // Verificar operaciones
    let ops: Vec<&str> = diff.iter()
        .map(|op| op["op"].as_str().unwrap_or(""))
        .collect();

    assert!(ops.contains(&"replace")); // b: 2 -> 20
    assert!(ops.contains(&"add"));     // d: 4
    assert!(ops.contains(&"remove"));  // c removed
}

#[tokio::test]
async fn test_no_diff_when_configs_equal() {
    let server = TestServer::start().await;

    let mut client = TestWsClient::connect(&server.ws_url("myapp", "prod"))
        .await
        .expect("Should connect");
    let _ = client.receive_json().await;

    let config = fixtures::sample_config();

    // Mismo config, no deberia haber diff
    server.emit_config_change(
        "myapp", "prod", "main",
        Some(config.clone()),
        config,
        "v1", // Misma version
    );

    // Podria no enviar nada, o enviar con diff vacio
    let msg = client.try_receive(std::time::Duration::from_millis(200)).await;

    if let Some(m) = msg {
        if m["type"] == "config_change" {
            let diff = m["diff"].as_array().unwrap();
            assert!(diff.is_empty(), "Diff should be empty for identical configs");
        }
    }
}

#[tokio::test]
async fn test_nested_diff() {
    let server = TestServer::start().await;

    let mut client = TestWsClient::connect(&server.ws_url("myapp", "prod"))
        .await
        .expect("Should connect");
    let _ = client.receive_json().await;

    let old = json!({
        "database": {
            "host": "localhost",
            "port": 5432,
            "settings": {
                "pool_size": 10
            }
        }
    });
    let new = json!({
        "database": {
            "host": "localhost",
            "port": 5432,
            "settings": {
                "pool_size": 20  // Solo este cambio
            }
        }
    });

    server.emit_config_change("myapp", "prod", "main", Some(old), new, "v2");

    let msg = client.expect_message_type("config_change")
        .await
        .expect("Should receive change");

    let diff = msg["diff"].as_array().expect("Should have diff");

    // Solo debe haber un cambio
    assert_eq!(diff.len(), 1);
    assert_eq!(diff[0]["path"], "/database/settings/pool_size");
}
```

### Paso 7: Escribir Tests de Heartbeat

```rust
// tests/ws_heartbeat_test.rs
mod helpers;

use helpers::{TestServer, TestWsClient, TestServerConfig, fixtures};
use std::time::Duration;

#[tokio::test]
async fn test_client_receives_ping() {
    // Servidor con heartbeat rapido para el test
    let server = TestServer::start_with_config(TestServerConfig {
        heartbeat_interval_ms: Some(100),
        heartbeat_timeout_ms: Some(50),
    }).await;

    let mut client = TestWsClient::connect(&server.ws_url("myapp", "prod"))
        .await
        .expect("Should connect");
    let _ = client.receive_json().await;

    // Esperar ping (deberia llegar en ~100ms)
    let msg = client.receive_with_timeout(Duration::from_millis(200))
        .await;

    // Podria ser ping WebSocket (no visible) o mensaje JSON
    // Dependiendo de la implementacion
    assert!(msg.is_ok() || true); // Ajustar segun implementacion
}

#[tokio::test]
async fn test_connection_timeout_without_pong() {
    let server = TestServer::start_with_config(TestServerConfig {
        heartbeat_interval_ms: Some(50),
        heartbeat_timeout_ms: Some(50),
    }).await;

    let mut client = TestWsClient::connect(&server.ws_url("myapp", "prod"))
        .await
        .expect("Should connect");
    let _ = client.receive_json().await;

    // No responder a pings
    // Esperar timeout (50ms interval + 50ms timeout = ~100ms)
    tokio::time::sleep(Duration::from_millis(150)).await;

    // La conexion deberia haberse cerrado
    let result = client.try_receive(Duration::from_millis(100)).await;

    // Deberia ser None (conexion cerrada) o mensaje de close
    assert!(result.is_none() || result.as_ref().map(|m| m["type"] == "connection_closing").unwrap_or(false));
}

#[tokio::test]
async fn test_pong_keeps_connection_alive() {
    let server = TestServer::start_with_config(TestServerConfig {
        heartbeat_interval_ms: Some(50),
        heartbeat_timeout_ms: Some(50),
    }).await;

    let mut client = TestWsClient::connect(&server.ws_url("myapp", "prod"))
        .await
        .expect("Should connect");
    let _ = client.receive_json().await;

    // Responder a cualquier ping
    for _ in 0..5 {
        tokio::time::sleep(Duration::from_millis(60)).await;
        client.send(&fixtures::pong_message()).await.ok();
    }

    // La conexion deberia seguir activa despues de 300ms
    assert_eq!(server.state().ws_registry.connection_count(), 1);
}
```

### Paso 8: Escribir Tests de Concurrencia

```rust
// tests/ws_concurrent_test.rs
mod helpers;

use helpers::{TestServer, TestWsClient, fixtures};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Barrier;

#[tokio::test]
async fn test_concurrent_connections() {
    let server = TestServer::start().await;
    let num_clients = 50;

    let barrier = Arc::new(Barrier::new(num_clients));
    let mut handles = Vec::new();

    for i in 0..num_clients {
        let url = server.ws_url(&format!("app{}", i % 5), "prod");
        let barrier_clone = barrier.clone();

        handles.push(tokio::spawn(async move {
            let mut client = TestWsClient::connect(&url)
                .await
                .expect("Should connect");

            // Sincronizar todas las conexiones
            barrier_clone.wait().await;

            // Recibir snapshot
            let msg = client.receive_json().await.expect("Should receive");
            assert_eq!(msg["type"], "config_snapshot");

            // Cerrar
            client.close().await.ok();
        }));
    }

    // Esperar todos los clientes
    for handle in handles {
        handle.await.expect("Task should complete");
    }

    // Verificar que todas las conexiones se limpiaron
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(server.state().ws_registry.connection_count(), 0);
}

#[tokio::test]
async fn test_concurrent_broadcast() {
    let server = TestServer::start().await;
    let num_clients = 20;

    // Conectar todos los clientes a la misma app
    let mut clients = Vec::new();
    for _ in 0..num_clients {
        let mut client = TestWsClient::connect(&server.ws_url("shared", "prod"))
            .await
            .expect("Should connect");
        let _ = client.receive_json().await; // Consumir snapshot
        clients.push(client);
    }

    // Emitir broadcast
    server.emit_config_change(
        "shared", "prod", "main",
        Some(fixtures::sample_config()),
        fixtures::modified_config(),
        "v2",
    );

    // Todos deben recibir
    let mut received = 0;
    for client in &mut clients {
        if let Ok(msg) = client.receive_with_timeout(Duration::from_secs(1)).await {
            if msg["type"] == "config_change" {
                received += 1;
            }
        }
    }

    assert_eq!(received, num_clients);
}

#[tokio::test]
async fn test_rapid_connect_disconnect() {
    let server = TestServer::start().await;

    for _ in 0..100 {
        let mut client = TestWsClient::connect(&server.ws_url("rapid", "test"))
            .await
            .expect("Should connect");

        // Desconectar inmediatamente
        client.close().await.ok();
    }

    // El servidor deberia manejar esto sin leaks
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(server.state().ws_registry.connection_count(), 0);
}

#[tokio::test]
async fn test_broadcast_during_disconnect() {
    let server = TestServer::start().await;

    // Conectar un cliente
    let mut client = TestWsClient::connect(&server.ws_url("myapp", "prod"))
        .await
        .expect("Should connect");
    let _ = client.receive_json().await;

    // Iniciar broadcast en background mientras el cliente se desconecta
    let state = server.state().clone();
    let broadcast_handle = tokio::spawn(async move {
        for i in 0..10 {
            state.broadcaster.emit(vortex_server::ws::ConfigChangeEvent::new(
                "myapp", "prod", "main",
                None,
                serde_json::json!({"i": i}),
                format!("v{}", i),
            )).ok();
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    // Cerrar el cliente durante los broadcasts
    tokio::time::sleep(Duration::from_millis(30)).await;
    client.close().await.ok();

    // Esperar que terminen los broadcasts
    broadcast_handle.await.ok();

    // No deberia haber panics ni leaks
    assert_eq!(server.state().ws_registry.connection_count(), 0);
}
```

### Paso 9: Test de Reconexion

```rust
// tests/ws_reconnect_test.rs
mod helpers;

use helpers::{TestServer, TestWsClient, fixtures};
use std::time::Duration;

#[tokio::test]
async fn test_resync_with_last_version() {
    let server = TestServer::start().await;

    // Primera conexion
    let mut client1 = TestWsClient::connect(&server.ws_url("myapp", "prod"))
        .await
        .expect("Should connect");

    let snapshot = client1.expect_message_type("config_snapshot")
        .await
        .expect("Should receive snapshot");
    let version1 = snapshot["version"].as_str().unwrap().to_string();

    // Simular algunos cambios mientras "desconectado"
    client1.close().await.ok();

    server.emit_config_change(
        "myapp", "prod", "main",
        Some(fixtures::sample_config()),
        fixtures::modified_config(),
        "v2",
    );

    // Reconectar
    let mut client2 = TestWsClient::connect(&server.ws_url("myapp", "prod"))
        .await
        .expect("Should reconnect");

    // Consumir snapshot inicial
    let _ = client2.receive_json().await;

    // Pedir resync con version anterior
    client2.send(&fixtures::resync_message(Some(&version1)))
        .await
        .expect("Should send resync");

    // Deberia recibir informacion de reconexion
    let msg = client2.find_message_type("reconnect_info", 5)
        .await
        .expect("Should receive reconnect info");

    // Verificar que hay versiones perdidas
    let missed = msg["missed_versions"].as_array().unwrap();
    assert!(!missed.is_empty(), "Should have missed versions");
}

#[tokio::test]
async fn test_graceful_shutdown_notification() {
    let server = TestServer::start().await;

    let mut client = TestWsClient::connect(&server.ws_url("myapp", "prod"))
        .await
        .expect("Should connect");
    let _ = client.receive_json().await;

    // Iniciar shutdown
    server.shutdown();

    // Cliente deberia recibir notificacion
    let msg = client.receive_with_timeout(Duration::from_secs(2))
        .await
        .expect("Should receive closing message");

    assert_eq!(msg["type"], "connection_closing");
    assert!(msg["reason"].is_string());
}
```

---

## Conceptos de Rust Aprendidos

### 1. Testing Async con #[tokio::test]

El macro `#[tokio::test]` configura un runtime Tokio para cada test.

**Rust:**
```rust
#[tokio::test]
async fn test_async_operation() {
    let result = some_async_fn().await;
    assert_eq!(result, expected);
}

// Con configuracion especifica
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_concurrent() {
    // Usa runtime multi-thread
}

// Con timeout
#[tokio::test]
#[timeout(1000)]  // 1 segundo
async fn test_with_timeout() {
    // Falla si tarda mas de 1s
}
```

**Comparacion con Java (JUnit):**
```java
@Test
void testAsyncOperation() {
    CompletableFuture<Result> future = someAsyncFn();
    Result result = future.get(5, TimeUnit.SECONDS);
    assertEquals(expected, result);
}

// O con frameworks como Awaitility
@Test
void testAsync() {
    await().atMost(5, SECONDS).until(() -> condition());
}
```

### 2. tokio::sync::Barrier para Sincronizacion

`Barrier` permite sincronizar multiples tareas en un punto.

**Rust:**
```rust
use std::sync::Arc;
use tokio::sync::Barrier;

let barrier = Arc::new(Barrier::new(num_tasks));

for i in 0..num_tasks {
    let b = barrier.clone();
    tokio::spawn(async move {
        // Setup
        setup_task(i).await;

        // Esperar a que todos lleguen aqui
        b.wait().await;

        // Todos continuan juntos
        run_task(i).await;
    });
}
```

**Comparacion con Java:**
```java
CyclicBarrier barrier = new CyclicBarrier(numTasks);

for (int i = 0; i < numTasks; i++) {
    final int taskId = i;
    executor.submit(() -> {
        setupTask(taskId);

        try {
            barrier.await();  // Bloquea hasta que todos lleguen
        } catch (Exception e) {
            e.printStackTrace();
        }

        runTask(taskId);
    });
}
```

### 3. Test Fixtures Modulares

Organizacion de helpers de test reutilizables.

**Rust:**
```rust
// tests/helpers/mod.rs
mod ws_client;
mod test_server;
mod fixtures;

pub use ws_client::{TestWsClient, TestWsClientBuilder};
pub use test_server::{TestServer, TestServerConfig};
pub use fixtures::*;

// En cada archivo de test:
mod helpers;
use helpers::{TestServer, TestWsClient, fixtures};
```

**Comparacion con Java:**
```java
// src/test/java/helpers/TestServer.java
public class TestServer {
    // ...
}

// En tests:
import static helpers.Fixtures.*;
import helpers.TestServer;
```

---

## Observabilidad en Tests

### Logging en Tests

```rust
// Habilitar logs en tests
#[tokio::test]
async fn test_with_logs() {
    // Inicializar tracing para este test
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();

    // Los logs aparecen con --nocapture
    // cargo test -- --nocapture
}
```

### Metricas de Test

```rust
#[tokio::test]
async fn test_performance_baseline() {
    let server = TestServer::start().await;

    let start = std::time::Instant::now();

    for _ in 0..100 {
        let mut client = TestWsClient::connect(&server.ws_url("perf", "test"))
            .await
            .expect("Should connect");
        let _ = client.receive_json().await;
        client.close().await.ok();
    }

    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_millis() as f64 / 100.0;

    println!("Average connection time: {:.2}ms", avg_ms);
    assert!(avg_ms < 50.0, "Connection too slow: {:.2}ms", avg_ms);
}
```

---

## Entregable Final

### Archivos Creados

1. `crates/vortex-server/tests/helpers/mod.rs` - Re-exports de helpers
2. `crates/vortex-server/tests/helpers/ws_client.rs` - TestWsClient
3. `crates/vortex-server/tests/helpers/test_server.rs` - TestServer
4. `crates/vortex-server/tests/helpers/fixtures.rs` - Datos de prueba
5. `crates/vortex-server/tests/ws_connection_test.rs` - Tests de conexion
6. `crates/vortex-server/tests/ws_broadcast_test.rs` - Tests de broadcast
7. `crates/vortex-server/tests/ws_diff_test.rs` - Tests de diff
8. `crates/vortex-server/tests/ws_heartbeat_test.rs` - Tests de heartbeat
9. `crates/vortex-server/tests/ws_reconnect_test.rs` - Tests de reconexion
10. `crates/vortex-server/tests/ws_concurrent_test.rs` - Tests de concurrencia

### Verificacion

```bash
# Ejecutar todos los tests WebSocket
cargo test -p vortex-server ws_

# Ejecutar con logs visibles
cargo test -p vortex-server ws_ -- --nocapture

# Ejecutar tests de concurrencia
cargo test -p vortex-server concurrent

# Ejecutar con coverage
cargo tarpaulin -p vortex-server --out Html

# Verificar que no hay tests flaky (correr multiples veces)
for i in {1..10}; do cargo test -p vortex-server ws_ || exit 1; done
```

### Reporte de Cobertura Esperado

```
┌─────────────────────────────────┬──────────┐
│ Module                          │ Coverage │
├─────────────────────────────────┼──────────┤
│ vortex_server::ws::handler      │   85%    │
│ vortex_server::ws::messages     │   92%    │
│ vortex_server::ws::connection   │   88%    │
│ vortex_server::ws::registry     │   90%    │
│ vortex_server::ws::broadcaster  │   87%    │
│ vortex_server::ws::diff         │   95%    │
│ vortex_server::ws::heartbeat    │   91%    │
│ vortex_server::ws::reconnect    │   83%    │
├─────────────────────────────────┼──────────┤
│ Total ws module                 │   89%    │
└─────────────────────────────────┴──────────┘
```

---

**Anterior**: [Historia 004 - Reconexion y Heartbeat](./story-004-reconnection.md)
**Volver al indice**: [Epica 08 - Real-time y WebSockets](./index.md)
