# Historia 005: Refresh y Sincronizacion

## Contexto y Objetivo

Para mantener las configuraciones actualizadas, el backend Git necesita sincronizarse periodicamente con el repositorio remoto. Esta historia implementa un mecanismo de refresh automatico que hace pull periodico, detecta cambios, y notifica cuando hay nuevas configuraciones disponibles.

Para un desarrollador Java, esto es similar a usar `ScheduledExecutorService` para ejecutar tareas periodicas, pero usando el runtime async de Tokio. La gestion de estado compartido usa primitivas de concurrencia como `RwLock` en lugar de `synchronized` o `ReentrantLock`.

## Alcance

### In Scope
- Pull periodico configurable (default: 30 segundos)
- Deteccion de cambios (comparacion de commits)
- Estado compartido thread-safe con `RwLock`
- Refresh on-demand via API
- Metricas de ultimo refresh y estado
- Graceful shutdown del background task

### Out of Scope
- Webhooks para notificacion push
- Propagacion de cambios a clientes conectados (WebSocket)
- Rollback automatico si hay errores

## Criterios de Aceptacion

- [ ] Background task hace pull cada N segundos (configurable)
- [ ] Estado compartido actualizado de forma thread-safe
- [ ] API para forzar refresh inmediato
- [ ] API para obtener estado actual (ultimo commit, ultimo refresh)
- [ ] El task se detiene gracefully cuando el servidor se apaga
- [ ] Errores de refresh no crashean el servidor
- [ ] Logs de cada refresh con resultado

## Diseno Propuesto

### Modulos/Crates Implicados
- `vortex-git/src/sync/mod.rs` - Re-exports
- `vortex-git/src/sync/state.rs` - Estado compartido
- `vortex-git/src/sync/refresh.rs` - Logica de refresh
- `vortex-git/src/sync/scheduler.rs` - Background task

### Interfaces

```rust
use std::sync::Arc;
use parking_lot::RwLock;
use tokio::sync::watch;

/// Shared state for the Git backend
#[derive(Debug)]
pub struct GitState {
    pub current_commit: String,
    pub last_refresh: Instant,
    pub last_error: Option<String>,
    pub refresh_count: u64,
}

/// Configuration for the refresh scheduler
#[derive(Debug, Clone)]
pub struct RefreshConfig {
    pub interval: Duration,
    pub retry_on_error: bool,
    pub max_retries: u32,
}

/// Handle to control the background refresh task
pub struct RefreshHandle {
    state: Arc<RwLock<GitState>>,
    shutdown_tx: watch::Sender<bool>,
    force_refresh_tx: mpsc::Sender<()>,
}

impl RefreshHandle {
    /// Force an immediate refresh
    pub async fn force_refresh(&self) -> Result<(), Error>;

    /// Get current state snapshot
    pub fn state(&self) -> GitState;

    /// Shutdown the background task
    pub async fn shutdown(self);
}
```

### Estructura Sugerida

```
crates/vortex-git/src/sync/
├── mod.rs          # pub mod state; pub mod refresh; pub mod scheduler;
├── state.rs        # GitState and shared state management
├── refresh.rs      # Refresh logic
└── scheduler.rs    # Background task scheduling
```

## Pasos de Implementacion

1. **Implementar GitState**
   - Struct con campos de estado
   - Clone para snapshots

2. **Implementar refresh logic**
   - Pull del repositorio
   - Comparacion de commits
   - Actualizacion de estado

3. **Implementar scheduler**
   - Tokio spawn para background task
   - Interval para refresh periodico
   - Channels para force refresh y shutdown

4. **Implementar RefreshHandle**
   - API publica para controlar el refresh
   - Metodos async para operaciones

5. **Tests con mock repository**

## Conceptos de Rust Aprendidos

### RwLock para Estado Compartido

`RwLock` permite multiples lectores O un escritor exclusivo. Es ideal cuando las lecturas son mas frecuentes que las escrituras.

```rust
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Instant;

/// State of the Git backend
#[derive(Debug, Clone)]
pub struct GitState {
    pub current_commit: String,
    pub current_label: String,
    pub last_refresh: Instant,
    pub last_error: Option<String>,
    pub refresh_count: u64,
}

impl GitState {
    pub fn new(commit: String, label: String) -> Self {
        Self {
            current_commit: commit,
            current_label: label,
            last_refresh: Instant::now(),
            last_error: None,
            refresh_count: 0,
        }
    }
}

/// Thread-safe wrapper for GitState
pub struct SharedState {
    inner: Arc<RwLock<GitState>>,
}

impl SharedState {
    pub fn new(initial: GitState) -> Self {
        Self {
            inner: Arc::new(RwLock::new(initial)),
        }
    }

    /// Read current state (non-blocking if no writer)
    pub fn read(&self) -> GitState {
        self.inner.read().clone()
    }

    /// Update state with exclusive lock
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut GitState),
    {
        let mut guard = self.inner.write();
        f(&mut guard);
    }

    /// Get Arc clone for sharing across tasks
    pub fn clone_arc(&self) -> Arc<RwLock<GitState>> {
        Arc::clone(&self.inner)
    }
}

// Uso
fn example() {
    let state = SharedState::new(GitState::new(
        "abc123".into(),
        "main".into(),
    ));

    // Multiples lectores pueden acceder simultaneamente
    let current = state.read();
    println!("Current commit: {}", current.current_commit);

    // Solo un escritor a la vez
    state.update(|s| {
        s.current_commit = "def456".into();
        s.refresh_count += 1;
        s.last_refresh = Instant::now();
    });
}
```

**Comparacion con Java:**

```java
// Java con ReentrantReadWriteLock
public class SharedState {
    private final ReadWriteLock lock = new ReentrantReadWriteLock();
    private GitState state;

    public GitState read() {
        lock.readLock().lock();
        try {
            return state.clone();  // Defensive copy
        } finally {
            lock.readLock().unlock();
        }
    }

    public void update(Consumer<GitState> updater) {
        lock.writeLock().lock();
        try {
            updater.accept(state);
        } finally {
            lock.writeLock().unlock();
        }
    }
}
```

| Rust (parking_lot) | Java |
|-------------------|------|
| `RwLock::read()` | `readLock().lock()` |
| `RwLock::write()` | `writeLock().lock()` |
| Guard drop = unlock | `finally { unlock() }` |
| `Arc<RwLock<T>>` | Shared reference + lock |

### Tokio Spawn e Intervals

Tokio proporciona primitivas para ejecutar tareas en background y timers.

```rust
use tokio::time::{interval, Duration, Instant};
use tokio::sync::{mpsc, watch};
use std::sync::Arc;
use parking_lot::RwLock;

/// Background refresh scheduler
pub struct RefreshScheduler {
    config: RefreshConfig,
    state: Arc<RwLock<GitState>>,
    repo_path: PathBuf,
}

impl RefreshScheduler {
    /// Start the background refresh task
    pub fn start(self) -> RefreshHandle {
        // Channels para comunicacion
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (force_tx, force_rx) = mpsc::channel(1);

        let state = Arc::clone(&self.state);

        // Spawn del background task
        let handle = tokio::spawn(async move {
            self.run_loop(shutdown_rx, force_rx).await;
        });

        RefreshHandle {
            state,
            shutdown_tx,
            force_refresh_tx: force_tx,
            task_handle: handle,
        }
    }

    async fn run_loop(
        self,
        mut shutdown_rx: watch::Receiver<bool>,
        mut force_rx: mpsc::Receiver<()>,
    ) {
        let mut ticker = interval(self.config.interval);

        loop {
            tokio::select! {
                // Check for shutdown signal
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!("Refresh scheduler shutting down");
                        break;
                    }
                }

                // Check for force refresh request
                Some(()) = force_rx.recv() => {
                    tracing::info!("Force refresh requested");
                    self.do_refresh().await;
                }

                // Regular interval tick
                _ = ticker.tick() => {
                    tracing::debug!("Scheduled refresh tick");
                    self.do_refresh().await;
                }
            }
        }
    }

    async fn do_refresh(&self) {
        let start = Instant::now();

        match self.perform_refresh().await {
            Ok(had_changes) => {
                let mut state = self.state.write();
                state.last_refresh = Instant::now();
                state.refresh_count += 1;
                state.last_error = None;

                tracing::info!(
                    duration_ms = %start.elapsed().as_millis(),
                    had_changes = %had_changes,
                    refresh_count = state.refresh_count,
                    "Refresh completed"
                );
            }
            Err(e) => {
                let mut state = self.state.write();
                state.last_error = Some(e.to_string());

                tracing::warn!(
                    duration_ms = %start.elapsed().as_millis(),
                    error = %e,
                    "Refresh failed"
                );
            }
        }
    }

    async fn perform_refresh(&self) -> Result<bool, anyhow::Error> {
        // Pull y comparar commits
        let result = pull_repository(&self.repo_path).await?;

        if result.had_changes {
            let mut state = self.state.write();
            state.current_commit = result.current_commit;
        }

        Ok(result.had_changes)
    }
}
```

**Comparacion con Java:**

```java
// Java con ScheduledExecutorService
public class RefreshScheduler {
    private final ScheduledExecutorService executor =
        Executors.newSingleThreadScheduledExecutor();

    private ScheduledFuture<?> scheduledTask;
    private final AtomicBoolean forceRefresh = new AtomicBoolean(false);

    public void start() {
        scheduledTask = executor.scheduleAtFixedRate(
            this::doRefresh,
            0,
            config.getIntervalSeconds(),
            TimeUnit.SECONDS
        );
    }

    public void forceRefresh() {
        forceRefresh.set(true);
        // Wakeup the task somehow...
    }

    public void shutdown() {
        scheduledTask.cancel(false);
        executor.shutdown();
    }

    private void doRefresh() {
        try {
            boolean hadChanges = performRefresh();
            state.update(s -> {
                s.setLastRefresh(Instant.now());
                s.incrementRefreshCount();
            });
        } catch (Exception e) {
            state.update(s -> s.setLastError(e.getMessage()));
        }
    }
}
```

### Channels para Comunicacion entre Tasks

```rust
use tokio::sync::{mpsc, oneshot, watch};

/// Handle para controlar el refresh scheduler
pub struct RefreshHandle {
    state: Arc<RwLock<GitState>>,
    shutdown_tx: watch::Sender<bool>,
    force_refresh_tx: mpsc::Sender<oneshot::Sender<RefreshResult>>,
    task_handle: tokio::task::JoinHandle<()>,
}

impl RefreshHandle {
    /// Force an immediate refresh and wait for result
    pub async fn force_refresh(&self) -> Result<RefreshResult, RefreshError> {
        // Crear channel para recibir resultado
        let (result_tx, result_rx) = oneshot::channel();

        // Enviar request al background task
        self.force_refresh_tx
            .send(result_tx)
            .await
            .map_err(|_| RefreshError::SchedulerNotRunning)?;

        // Esperar resultado
        result_rx
            .await
            .map_err(|_| RefreshError::SchedulerNotRunning)
    }

    /// Get current state snapshot (non-blocking)
    pub fn state(&self) -> GitState {
        self.state.read().clone()
    }

    /// Check if a refresh is needed based on time
    pub fn needs_refresh(&self, max_age: Duration) -> bool {
        let state = self.state.read();
        state.last_refresh.elapsed() > max_age
    }

    /// Gracefully shutdown the scheduler
    pub async fn shutdown(self) -> Result<(), RefreshError> {
        // Send shutdown signal
        let _ = self.shutdown_tx.send(true);

        // Wait for task to complete
        self.task_handle
            .await
            .map_err(|e| RefreshError::ShutdownError(e.to_string()))
    }
}

/// Result of a refresh operation
#[derive(Debug, Clone)]
pub struct RefreshResult {
    pub had_changes: bool,
    pub new_commit: Option<String>,
    pub duration: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum RefreshError {
    #[error("Refresh scheduler is not running")]
    SchedulerNotRunning,

    #[error("Shutdown error: {0}")]
    ShutdownError(String),

    #[error("Refresh failed: {0}")]
    RefreshFailed(#[from] anyhow::Error),
}
```

### Graceful Shutdown Pattern

```rust
use tokio::signal;

/// Start the Git backend with graceful shutdown
pub async fn run_git_backend(config: GitBackendConfig) -> Result<()> {
    // Inicializar el backend
    let backend = GitBackend::new(config).await?;

    // Iniciar el scheduler
    let refresh_handle = backend.start_refresh_scheduler();

    // Esperar signal de shutdown
    let shutdown = async {
        signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
        tracing::info!("Received shutdown signal");
    };

    // Ejecutar hasta shutdown
    tokio::select! {
        _ = shutdown => {
            tracing::info!("Initiating graceful shutdown");
        }
    }

    // Shutdown gracefully
    tracing::info!("Stopping refresh scheduler...");
    refresh_handle.shutdown().await?;

    tracing::info!("Shutdown complete");
    Ok(())
}

// Integracion con Axum server
pub async fn run_server_with_backend(config: ServerConfig) -> Result<()> {
    let backend = Arc::new(GitBackend::new(config.git).await?);
    let refresh_handle = backend.start_refresh_scheduler();

    // Crear router con backend como state
    let app = create_router(backend);

    // Configurar shutdown
    let shutdown_signal = async {
        signal::ctrl_c().await.ok();
    };

    // Ejecutar servidor
    let server = axum::Server::bind(&config.address)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal);

    server.await?;

    // Cleanup
    refresh_handle.shutdown().await?;

    Ok(())
}
```

## Riesgos y Errores Comunes

### 1. Deadlock por mantener lock durante I/O

```rust
// ERROR: Mantiene write lock durante operacion async
async fn bad_refresh(&self) {
    let mut state = self.state.write();  // Lock adquirido
    let result = pull_repository(&self.path).await;  // I/O async con lock!
    state.current_commit = result.commit;
}

// CORRECTO: Liberar lock antes de I/O
async fn good_refresh(&self) {
    // Primero hacer I/O sin lock
    let result = pull_repository(&self.path).await?;

    // Luego actualizar estado brevemente
    {
        let mut state = self.state.write();
        state.current_commit = result.commit;
    }  // Lock liberado aqui
}
```

### 2. No manejar errores del background task

```rust
// ERROR: Panic en background task no se detecta
tokio::spawn(async {
    loop {
        do_refresh().await.unwrap();  // Panic si falla!
    }
});

// CORRECTO: Manejar errores gracefully
tokio::spawn(async {
    loop {
        if let Err(e) = do_refresh().await {
            tracing::error!(error = %e, "Refresh failed, will retry");
            // Continuar, no panic
        }
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
});
```

### 3. Memory leak por tasks que no terminan

```rust
// ERROR: Task nunca termina, no hay forma de pararlo
let _handle = tokio::spawn(async {
    let mut interval = interval(Duration::from_secs(30));
    loop {
        interval.tick().await;
        do_refresh().await;
    }
});

// CORRECTO: Shutdown signal
let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

tokio::spawn(async move {
    let mut interval = interval(Duration::from_secs(30));
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    break;  // Exit loop
                }
            }
            _ = interval.tick() => {
                do_refresh().await;
            }
        }
    }
});

// Para parar:
// shutdown_tx.send(true).ok();
```

## Pruebas

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_state_new() {
        let state = GitState::new("abc123".into(), "main".into());

        assert_eq!(state.current_commit, "abc123");
        assert_eq!(state.current_label, "main");
        assert_eq!(state.refresh_count, 0);
        assert!(state.last_error.is_none());
    }

    #[test]
    fn test_shared_state_read() {
        let state = SharedState::new(GitState::new(
            "abc123".into(),
            "main".into(),
        ));

        let snapshot = state.read();
        assert_eq!(snapshot.current_commit, "abc123");
    }

    #[test]
    fn test_shared_state_update() {
        let state = SharedState::new(GitState::new(
            "abc123".into(),
            "main".into(),
        ));

        state.update(|s| {
            s.current_commit = "def456".into();
            s.refresh_count = 5;
        });

        let snapshot = state.read();
        assert_eq!(snapshot.current_commit, "def456");
        assert_eq!(snapshot.refresh_count, 5);
    }

    #[test]
    fn test_refresh_config_default() {
        let config = RefreshConfig::default();

        assert_eq!(config.interval, Duration::from_secs(30));
        assert!(config.retry_on_error);
        assert_eq!(config.max_retries, 3);
    }
}
```

### Async Tests

```rust
#[cfg(test)]
mod async_tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_refresh_handle_state() {
        let state = Arc::new(RwLock::new(GitState::new(
            "abc123".into(),
            "main".into(),
        )));

        let (shutdown_tx, _) = watch::channel(false);
        let (force_tx, _) = mpsc::channel(1);

        let handle = RefreshHandle {
            state: Arc::clone(&state),
            shutdown_tx,
            force_refresh_tx: force_tx,
            task_handle: tokio::spawn(async {}),
        };

        let current = handle.state();
        assert_eq!(current.current_commit, "abc123");
    }

    #[tokio::test]
    async fn test_refresh_handle_needs_refresh() {
        let mut initial = GitState::new("abc123".into(), "main".into());
        // Simular que el ultimo refresh fue hace 1 minuto
        initial.last_refresh = Instant::now() - Duration::from_secs(60);

        let state = Arc::new(RwLock::new(initial));
        let (shutdown_tx, _) = watch::channel(false);
        let (force_tx, _) = mpsc::channel(1);

        let handle = RefreshHandle {
            state,
            shutdown_tx,
            force_refresh_tx: force_tx,
            task_handle: tokio::spawn(async {}),
        };

        // Necesita refresh si max_age es 30 segundos
        assert!(handle.needs_refresh(Duration::from_secs(30)));

        // No necesita refresh si max_age es 2 minutos
        assert!(!handle.needs_refresh(Duration::from_secs(120)));
    }

    #[tokio::test]
    async fn test_scheduler_shutdown() {
        let state = Arc::new(RwLock::new(GitState::new(
            "abc123".into(),
            "main".into(),
        )));

        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let (force_tx, _force_rx) = mpsc::channel::<oneshot::Sender<RefreshResult>>(1);

        // Spawn a simple task that responds to shutdown
        let task_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                }
            }
        });

        let handle = RefreshHandle {
            state,
            shutdown_tx,
            force_refresh_tx: force_tx,
            task_handle,
        };

        // Shutdown should complete within timeout
        let result = timeout(
            Duration::from_secs(1),
            handle.shutdown()
        ).await;

        assert!(result.is_ok());
    }
}
```

### Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup_test_backend() -> (TempDir, GitBackend) {
        let temp_dir = TempDir::new().unwrap();

        // Create test repo (reuse from previous stories)
        create_test_repo(temp_dir.path()).unwrap();

        let config = GitBackendConfig::new(
            temp_dir.path().to_string_lossy(),
            temp_dir.path().join("clone"),
        )
        .with_refresh_interval(Duration::from_millis(100));

        let backend = GitBackend::new(config).await.unwrap();

        (temp_dir, backend)
    }

    #[tokio::test]
    async fn test_scheduler_runs_periodically() {
        let (_temp, backend) = setup_test_backend().await;

        let handle = backend.start_refresh_scheduler();

        // Wait for a few refresh cycles
        tokio::time::sleep(Duration::from_millis(350)).await;

        let state = handle.state();
        assert!(
            state.refresh_count >= 2,
            "Expected at least 2 refreshes, got {}",
            state.refresh_count
        );

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_force_refresh() {
        let (_temp, backend) = setup_test_backend().await;

        let handle = backend.start_refresh_scheduler();

        let initial_count = handle.state().refresh_count;

        // Force refresh
        let result = handle.force_refresh().await.unwrap();
        assert!(!result.had_changes); // No changes expected

        // Count should have increased
        assert!(handle.state().refresh_count > initial_count);

        handle.shutdown().await.unwrap();
    }
}
```

## Observabilidad

```rust
use tracing::{instrument, info, warn, debug, Span};
use std::time::Instant;

impl RefreshScheduler {
    #[instrument(skip(self), fields(
        refresh_count = tracing::field::Empty,
        had_changes = tracing::field::Empty
    ))]
    async fn do_refresh(&self) {
        let start = Instant::now();
        let span = Span::current();

        match self.perform_refresh().await {
            Ok(result) => {
                let state = self.state.read();
                span.record("refresh_count", state.refresh_count);
                span.record("had_changes", result.had_changes);

                // Metricas (si usamos prometheus)
                // REFRESH_DURATION.observe(start.elapsed().as_secs_f64());
                // REFRESH_TOTAL.inc();

                if result.had_changes {
                    info!(
                        new_commit = %result.new_commit.unwrap_or_default(),
                        "Configuration updated"
                    );
                } else {
                    debug!("No changes detected");
                }
            }
            Err(e) => {
                // REFRESH_ERRORS.inc();
                warn!(error = %e, "Refresh failed");
            }
        }
    }
}

// Metricas de estado (para endpoint de metricas)
impl RefreshHandle {
    pub fn metrics(&self) -> RefreshMetrics {
        let state = self.state.read();
        RefreshMetrics {
            refresh_count: state.refresh_count,
            seconds_since_refresh: state.last_refresh.elapsed().as_secs(),
            current_commit: state.current_commit.clone(),
            has_error: state.last_error.is_some(),
            last_error: state.last_error.clone(),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct RefreshMetrics {
    pub refresh_count: u64,
    pub seconds_since_refresh: u64,
    pub current_commit: String,
    pub has_error: bool,
    pub last_error: Option<String>,
}
```

## Entregable Final

- PR con:
  - `crates/vortex-git/src/sync/mod.rs`
  - `crates/vortex-git/src/sync/state.rs`
  - `crates/vortex-git/src/sync/refresh.rs`
  - `crates/vortex-git/src/sync/scheduler.rs`
  - Tests unitarios y de integracion
  - Ejemplo de integracion con servidor
  - Logging y metricas
  - Documentacion de configuracion

---

**Anterior**: [Historia 004 - Soporte de Labels](./story-004-labels-support.md)
**Siguiente**: [Historia 006 - Tests con Repositorio Local](./story-006-git-tests.md)
