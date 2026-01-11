# Historia 001: Canary Rollout Engine

## Contexto y Objetivo

Los deployments de configuracion en produccion son riesgosos. Un cambio mal configurado puede afectar a todos los usuarios simultaneamente. Los canary rollouts mitigan este riesgo desplegando cambios gradualmente a un porcentaje pequeno de usuarios, monitoreando metricas de exito, y solo expandiendo si todo va bien.

Esta historia implementa el motor central de canary rollouts que:
- Asigna requests a grupos canary o stable usando consistent hashing
- Gestiona stages de rollout (1%, 5%, 25%, 50%, 100%)
- Evalua metricas de exito para decidir promocion o rollback automatico
- Mantiene consistencia (mismo usuario siempre ve misma version)

Para desarrolladores Java, esto es similar a lo que ofrecen herramientas como LaunchDarkly para feature flags, pero integrado directamente en el servidor de configuracion con consistent hashing al estilo de Guava.

---

## Alcance

### In Scope

- `CanaryEngine` trait y implementacion
- `ConsistentHasher` con xxHash3
- `RolloutStage` con porcentajes configurables
- `SuccessMetrics` para evaluar salud del canary
- Asignacion determinista de requests a grupos
- Tests unitarios exhaustivos

### Out of Scope

- API REST para rollouts (historia 002)
- Persistencia de estado de rollouts (historia 002)
- Drift detection (historia 003)
- Integracion con metricas externas (Prometheus, Datadog)
- UI de administracion

---

## Criterios de Aceptacion

- [ ] `ConsistentHasher` asigna requests deterministicamente basado en key
- [ ] Stages configurables con porcentajes arbitrarios
- [ ] `CanaryEngine.resolve()` retorna version correcta para cada request
- [ ] `SuccessMetrics` evalua error rate, latency p99, success rate
- [ ] Threshold de metricas configurable por rollout
- [ ] Tests demuestran distribucion uniforme del hasher
- [ ] Performance: resolve() < 1us por request

---

## Diseno Propuesto

### Arquitectura del Motor

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          CanaryEngine                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────────┐   │
│  │ ConsistentHasher│     │  RolloutState   │     │  SuccessMetrics     │   │
│  │                 │     │                 │     │                     │   │
│  │ - xxh3 hash     │     │ - stages        │     │ - error_rate        │   │
│  │ - deterministic │     │ - current_stage │     │ - latency_p99       │   │
│  │ - uniform dist  │     │ - percentage    │     │ - success_rate      │   │
│  └────────┬────────┘     └────────┬────────┘     │ - request_count     │   │
│           │                       │              └──────────┬──────────┘   │
│           │                       │                         │               │
│           └───────────────────────┼─────────────────────────┘               │
│                                   │                                          │
│                         ┌─────────▼─────────┐                               │
│                         │   resolve(key)    │                               │
│                         │                   │                               │
│                         │ 1. hash = xxh3(key)                               │
│                         │ 2. bucket = hash % 100                            │
│                         │ 3. if bucket < stage.percentage:                  │
│                         │       return canary_version                       │
│                         │    else:                                          │
│                         │       return stable_version                       │
│                         └───────────────────┘                               │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Modelo de Datos

```rust
/// Stage de un rollout con porcentaje de trafico
pub struct RolloutStage {
    pub name: String,
    pub percentage: u8,      // 0-100
    pub min_duration: Duration,
    pub success_threshold: SuccessThreshold,
}

/// Thresholds para considerar el canary exitoso
pub struct SuccessThreshold {
    pub min_success_rate: f64,     // 0.0-1.0 (e.g., 0.99 = 99%)
    pub max_error_rate: f64,       // 0.0-1.0 (e.g., 0.01 = 1%)
    pub max_latency_p99_ms: u64,   // e.g., 100ms
    pub min_request_count: u64,    // Minimum requests before evaluating
}

/// Configuracion de un rollout
pub struct RolloutConfig {
    pub id: Uuid,
    pub app: String,
    pub profile: String,
    pub stable_version: String,
    pub canary_version: String,
    pub stages: Vec<RolloutStage>,
    pub auto_promote: bool,
    pub auto_rollback: bool,
}
```

---

## Pasos de Implementacion

### Paso 1: Implementar ConsistentHasher

```rust
// src/canary/hasher.rs
use xxhash_rust::xxh3::xxh3_64;

/// Hasher for consistent canary assignment.
///
/// Uses xxHash3 for deterministic hashing, ensuring the same key
/// always maps to the same bucket.
#[derive(Debug, Clone)]
pub struct ConsistentHasher {
    /// Salt to differentiate between rollouts
    seed: u64,
}

impl ConsistentHasher {
    /// Creates a new hasher with the given seed.
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Creates a hasher from a rollout ID.
    pub fn from_rollout_id(rollout_id: &uuid::Uuid) -> Self {
        // Use first 8 bytes of UUID as seed
        let bytes = rollout_id.as_bytes();
        let seed = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        Self::new(seed)
    }

    /// Computes a bucket (0-99) for the given key.
    ///
    /// The same key always returns the same bucket.
    pub fn bucket(&self, key: &str) -> u8 {
        // Combine seed with key for unique hash per rollout
        let mut data = self.seed.to_le_bytes().to_vec();
        data.extend_from_slice(key.as_bytes());

        let hash = xxh3_64(&data);
        (hash % 100) as u8
    }

    /// Determines if a key falls within the canary percentage.
    ///
    /// # Example
    ///
    /// ```rust
    /// let hasher = ConsistentHasher::new(42);
    ///
    /// // With 5% canary, ~5% of keys will return true
    /// let is_canary = hasher.is_canary("user-123", 5);
    ///
    /// // Same key always returns same result
    /// assert_eq!(hasher.is_canary("user-123", 5), is_canary);
    /// ```
    pub fn is_canary(&self, key: &str, canary_percentage: u8) -> bool {
        self.bucket(key) < canary_percentage
    }
}

impl Default for ConsistentHasher {
    fn default() -> Self {
        Self::new(0)
    }
}
```

### Paso 2: Definir RolloutStage y Thresholds

```rust
// src/canary/stage.rs
use std::time::Duration;
use serde::{Deserialize, Serialize};

/// Threshold configuration for canary success evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessThreshold {
    /// Minimum success rate (0.0-1.0). Default: 0.99 (99%)
    #[serde(default = "default_success_rate")]
    pub min_success_rate: f64,

    /// Maximum error rate (0.0-1.0). Default: 0.01 (1%)
    #[serde(default = "default_error_rate")]
    pub max_error_rate: f64,

    /// Maximum p99 latency in milliseconds. Default: 100ms
    #[serde(default = "default_latency")]
    pub max_latency_p99_ms: u64,

    /// Minimum requests before evaluation. Default: 100
    #[serde(default = "default_min_requests")]
    pub min_request_count: u64,
}

fn default_success_rate() -> f64 { 0.99 }
fn default_error_rate() -> f64 { 0.01 }
fn default_latency() -> u64 { 100 }
fn default_min_requests() -> u64 { 100 }

impl Default for SuccessThreshold {
    fn default() -> Self {
        Self {
            min_success_rate: default_success_rate(),
            max_error_rate: default_error_rate(),
            max_latency_p99_ms: default_latency(),
            min_request_count: default_min_requests(),
        }
    }
}

/// A stage in a canary rollout progression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolloutStage {
    /// Stage name (e.g., "initial", "expand", "full")
    pub name: String,

    /// Percentage of traffic to route to canary (0-100)
    pub percentage: u8,

    /// Minimum time to stay in this stage before promotion
    #[serde(with = "humantime_serde")]
    pub min_duration: Duration,

    /// Success criteria for this stage
    #[serde(default)]
    pub threshold: SuccessThreshold,
}

impl RolloutStage {
    /// Creates a new rollout stage.
    pub fn new(
        name: impl Into<String>,
        percentage: u8,
        min_duration: Duration,
    ) -> Self {
        Self {
            name: name.into(),
            percentage: percentage.min(100),
            min_duration,
            threshold: SuccessThreshold::default(),
        }
    }

    /// Sets custom thresholds for this stage.
    pub fn with_threshold(mut self, threshold: SuccessThreshold) -> Self {
        self.threshold = threshold;
        self
    }
}

/// Predefined stage progressions for common rollout strategies.
pub struct StagePresets;

impl StagePresets {
    /// Conservative rollout: 1% -> 5% -> 25% -> 50% -> 100%
    pub fn conservative() -> Vec<RolloutStage> {
        vec![
            RolloutStage::new("initial", 1, Duration::from_secs(300)),
            RolloutStage::new("early", 5, Duration::from_secs(300)),
            RolloutStage::new("expand", 25, Duration::from_secs(600)),
            RolloutStage::new("majority", 50, Duration::from_secs(600)),
            RolloutStage::new("full", 100, Duration::from_secs(0)),
        ]
    }

    /// Aggressive rollout: 10% -> 50% -> 100%
    pub fn aggressive() -> Vec<RolloutStage> {
        vec![
            RolloutStage::new("initial", 10, Duration::from_secs(60)),
            RolloutStage::new("expand", 50, Duration::from_secs(120)),
            RolloutStage::new("full", 100, Duration::from_secs(0)),
        ]
    }

    /// Single stage: direct 100% rollout (use with caution)
    pub fn immediate() -> Vec<RolloutStage> {
        vec![
            RolloutStage::new("full", 100, Duration::from_secs(0)),
        ]
    }
}
```

### Paso 3: Implementar SuccessMetrics

```rust
// src/canary/metrics.rs
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use parking_lot::RwLock;

/// Metrics sample for a single request.
#[derive(Debug, Clone)]
pub struct RequestSample {
    pub timestamp: Instant,
    pub latency: Duration,
    pub success: bool,
    pub is_canary: bool,
}

/// Aggregated metrics for canary evaluation.
#[derive(Debug, Clone, Default)]
pub struct AggregatedMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub latency_samples: Vec<Duration>,
}

impl AggregatedMetrics {
    /// Calculates the success rate (0.0-1.0).
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 1.0;
        }
        self.successful_requests as f64 / self.total_requests as f64
    }

    /// Calculates the error rate (0.0-1.0).
    pub fn error_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        self.failed_requests as f64 / self.total_requests as f64
    }

    /// Calculates the p99 latency.
    pub fn latency_p99(&self) -> Duration {
        if self.latency_samples.is_empty() {
            return Duration::ZERO;
        }

        let mut sorted = self.latency_samples.clone();
        sorted.sort();

        let idx = ((sorted.len() as f64) * 0.99) as usize;
        sorted.get(idx.min(sorted.len() - 1))
            .copied()
            .unwrap_or(Duration::ZERO)
    }
}

/// Collector for canary metrics with time-windowed aggregation.
pub struct SuccessMetricsCollector {
    /// Window duration for metrics aggregation
    window: Duration,
    /// Samples within the current window
    samples: RwLock<VecDeque<RequestSample>>,
}

impl SuccessMetricsCollector {
    /// Creates a new collector with the specified window.
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            samples: RwLock::new(VecDeque::new()),
        }
    }

    /// Records a request sample.
    pub fn record(&self, latency: Duration, success: bool, is_canary: bool) {
        let sample = RequestSample {
            timestamp: Instant::now(),
            latency,
            success,
            is_canary,
        };

        let mut samples = self.samples.write();
        samples.push_back(sample);

        // Prune old samples
        self.prune_old_samples(&mut samples);
    }

    /// Gets aggregated metrics for canary requests only.
    pub fn canary_metrics(&self) -> AggregatedMetrics {
        self.aggregate(true)
    }

    /// Gets aggregated metrics for stable requests only.
    pub fn stable_metrics(&self) -> AggregatedMetrics {
        self.aggregate(false)
    }

    fn aggregate(&self, canary: bool) -> AggregatedMetrics {
        let samples = self.samples.read();
        let cutoff = Instant::now() - self.window;

        let relevant: Vec<_> = samples
            .iter()
            .filter(|s| s.timestamp > cutoff && s.is_canary == canary)
            .collect();

        AggregatedMetrics {
            total_requests: relevant.len() as u64,
            successful_requests: relevant.iter().filter(|s| s.success).count() as u64,
            failed_requests: relevant.iter().filter(|s| !s.success).count() as u64,
            latency_samples: relevant.iter().map(|s| s.latency).collect(),
        }
    }

    fn prune_old_samples(&self, samples: &mut VecDeque<RequestSample>) {
        let cutoff = Instant::now() - self.window;
        while let Some(front) = samples.front() {
            if front.timestamp < cutoff {
                samples.pop_front();
            } else {
                break;
            }
        }
    }
}

/// Evaluates if canary metrics meet success thresholds.
pub struct MetricsEvaluator;

impl MetricsEvaluator {
    /// Evaluates canary health against thresholds.
    pub fn evaluate(
        canary: &AggregatedMetrics,
        threshold: &super::stage::SuccessThreshold,
    ) -> EvaluationResult {
        // Check minimum request count
        if canary.total_requests < threshold.min_request_count {
            return EvaluationResult::InsufficientData {
                current: canary.total_requests,
                required: threshold.min_request_count,
            };
        }

        // Check error rate
        if canary.error_rate() > threshold.max_error_rate {
            return EvaluationResult::FailedErrorRate {
                current: canary.error_rate(),
                threshold: threshold.max_error_rate,
            };
        }

        // Check success rate
        if canary.success_rate() < threshold.min_success_rate {
            return EvaluationResult::FailedSuccessRate {
                current: canary.success_rate(),
                threshold: threshold.min_success_rate,
            };
        }

        // Check latency
        let p99_ms = canary.latency_p99().as_millis() as u64;
        if p99_ms > threshold.max_latency_p99_ms {
            return EvaluationResult::FailedLatency {
                current_ms: p99_ms,
                threshold_ms: threshold.max_latency_p99_ms,
            };
        }

        EvaluationResult::Healthy
    }
}

/// Result of metrics evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum EvaluationResult {
    /// Canary is healthy, can promote
    Healthy,
    /// Not enough data to evaluate
    InsufficientData { current: u64, required: u64 },
    /// Error rate too high
    FailedErrorRate { current: f64, threshold: f64 },
    /// Success rate too low
    FailedSuccessRate { current: f64, threshold: f64 },
    /// Latency too high
    FailedLatency { current_ms: u64, threshold_ms: u64 },
}

impl EvaluationResult {
    /// Returns true if the canary should be rolled back.
    pub fn should_rollback(&self) -> bool {
        matches!(
            self,
            Self::FailedErrorRate { .. }
                | Self::FailedSuccessRate { .. }
                | Self::FailedLatency { .. }
        )
    }

    /// Returns true if the canary can be promoted.
    pub fn can_promote(&self) -> bool {
        matches!(self, Self::Healthy)
    }
}
```

### Paso 4: Implementar CanaryEngine

```rust
// src/canary/engine.rs
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{info, warn, instrument};
use uuid::Uuid;

use super::hasher::ConsistentHasher;
use super::stage::{RolloutStage, SuccessThreshold};
use super::metrics::{SuccessMetricsCollector, MetricsEvaluator, EvaluationResult};

/// Configuration for a canary rollout.
#[derive(Debug, Clone)]
pub struct RolloutConfig {
    /// Unique rollout identifier
    pub id: Uuid,
    /// Application name
    pub app: String,
    /// Profile (e.g., "production")
    pub profile: String,
    /// Current stable version
    pub stable_version: String,
    /// New canary version being rolled out
    pub canary_version: String,
    /// Rollout stages
    pub stages: Vec<RolloutStage>,
    /// Enable automatic promotion based on metrics
    pub auto_promote: bool,
    /// Enable automatic rollback on failure
    pub auto_rollback: bool,
}

impl RolloutConfig {
    /// Creates a new rollout configuration.
    pub fn new(
        app: impl Into<String>,
        profile: impl Into<String>,
        stable_version: impl Into<String>,
        canary_version: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            app: app.into(),
            profile: profile.into(),
            stable_version: stable_version.into(),
            canary_version: canary_version.into(),
            stages: super::stage::StagePresets::conservative(),
            auto_promote: true,
            auto_rollback: true,
        }
    }

    /// Sets custom stages.
    pub fn with_stages(mut self, stages: Vec<RolloutStage>) -> Self {
        self.stages = stages;
        self
    }
}

/// Result of resolving which version to serve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedVersion {
    /// The version to serve
    pub version: String,
    /// Whether this is the canary version
    pub is_canary: bool,
    /// Current stage name (if rollout active)
    pub stage: Option<String>,
}

/// State of an active rollout.
struct ActiveRollout {
    config: RolloutConfig,
    current_stage_idx: usize,
    stage_started_at: std::time::Instant,
    hasher: ConsistentHasher,
    metrics: Arc<SuccessMetricsCollector>,
}

impl ActiveRollout {
    fn current_stage(&self) -> Option<&RolloutStage> {
        self.config.stages.get(self.current_stage_idx)
    }

    fn current_percentage(&self) -> u8 {
        self.current_stage()
            .map(|s| s.percentage)
            .unwrap_or(0)
    }
}

/// The canary rollout engine.
///
/// Manages active rollouts and determines which version to serve for each request.
pub struct CanaryEngine {
    /// Active rollouts by (app, profile)
    rollouts: RwLock<std::collections::HashMap<(String, String), ActiveRollout>>,
    /// Default metrics window
    metrics_window: std::time::Duration,
}

impl CanaryEngine {
    /// Creates a new canary engine.
    pub fn new() -> Self {
        Self {
            rollouts: RwLock::new(std::collections::HashMap::new()),
            metrics_window: std::time::Duration::from_secs(300), // 5 minutes
        }
    }

    /// Starts a new rollout.
    #[instrument(skip(self), fields(rollout_id = %config.id))]
    pub fn start_rollout(&self, config: RolloutConfig) -> Result<Uuid, CanaryError> {
        let key = (config.app.clone(), config.profile.clone());
        let id = config.id;

        let mut rollouts = self.rollouts.write();

        // Check for existing rollout
        if rollouts.contains_key(&key) {
            return Err(CanaryError::RolloutAlreadyActive {
                app: config.app,
                profile: config.profile,
            });
        }

        // Validate stages
        if config.stages.is_empty() {
            return Err(CanaryError::InvalidConfig("No stages defined".to_string()));
        }

        let rollout = ActiveRollout {
            hasher: ConsistentHasher::from_rollout_id(&config.id),
            config,
            current_stage_idx: 0,
            stage_started_at: std::time::Instant::now(),
            metrics: Arc::new(SuccessMetricsCollector::new(self.metrics_window)),
        };

        rollouts.insert(key, rollout);
        info!("Started rollout");

        Ok(id)
    }

    /// Resolves which version to serve for a request.
    ///
    /// # Arguments
    ///
    /// * `app` - Application name
    /// * `profile` - Profile name
    /// * `key` - Consistent hashing key (e.g., user_id, session_id)
    /// * `default_version` - Version to return if no rollout is active
    #[instrument(skip(self))]
    pub fn resolve(
        &self,
        app: &str,
        profile: &str,
        key: &str,
        default_version: &str,
    ) -> ResolvedVersion {
        let rollouts = self.rollouts.read();
        let lookup_key = (app.to_string(), profile.to_string());

        match rollouts.get(&lookup_key) {
            Some(rollout) => {
                let percentage = rollout.current_percentage();
                let is_canary = rollout.hasher.is_canary(key, percentage);

                ResolvedVersion {
                    version: if is_canary {
                        rollout.config.canary_version.clone()
                    } else {
                        rollout.config.stable_version.clone()
                    },
                    is_canary,
                    stage: rollout.current_stage().map(|s| s.name.clone()),
                }
            }
            None => ResolvedVersion {
                version: default_version.to_string(),
                is_canary: false,
                stage: None,
            },
        }
    }

    /// Records a request result for metrics.
    pub fn record_request(
        &self,
        app: &str,
        profile: &str,
        latency: std::time::Duration,
        success: bool,
        is_canary: bool,
    ) {
        let rollouts = self.rollouts.read();
        let key = (app.to_string(), profile.to_string());

        if let Some(rollout) = rollouts.get(&key) {
            rollout.metrics.record(latency, success, is_canary);
        }
    }

    /// Evaluates if the current stage should be promoted or rolled back.
    pub fn evaluate(&self, app: &str, profile: &str) -> Option<EvaluationResult> {
        let rollouts = self.rollouts.read();
        let key = (app.to_string(), profile.to_string());

        rollouts.get(&key).and_then(|rollout| {
            let stage = rollout.current_stage()?;
            let canary_metrics = rollout.metrics.canary_metrics();
            Some(MetricsEvaluator::evaluate(&canary_metrics, &stage.threshold))
        })
    }

    /// Promotes the rollout to the next stage.
    #[instrument(skip(self))]
    pub fn promote(&self, app: &str, profile: &str) -> Result<RolloutStage, CanaryError> {
        let mut rollouts = self.rollouts.write();
        let key = (app.to_string(), profile.to_string());

        let rollout = rollouts.get_mut(&key)
            .ok_or_else(|| CanaryError::RolloutNotFound {
                app: app.to_string(),
                profile: profile.to_string(),
            })?;

        // Check if we can promote
        let next_idx = rollout.current_stage_idx + 1;
        if next_idx >= rollout.config.stages.len() {
            return Err(CanaryError::AlreadyAtFinalStage);
        }

        rollout.current_stage_idx = next_idx;
        rollout.stage_started_at = std::time::Instant::now();

        let stage = rollout.config.stages[next_idx].clone();
        info!(stage = %stage.name, percentage = stage.percentage, "Promoted to next stage");

        Ok(stage)
    }

    /// Rolls back the rollout (removes it, stable version serves 100%).
    #[instrument(skip(self))]
    pub fn rollback(&self, app: &str, profile: &str) -> Result<(), CanaryError> {
        let mut rollouts = self.rollouts.write();
        let key = (app.to_string(), profile.to_string());

        if rollouts.remove(&key).is_none() {
            return Err(CanaryError::RolloutNotFound {
                app: app.to_string(),
                profile: profile.to_string(),
            });
        }

        warn!("Rollout rolled back");
        Ok(())
    }

    /// Completes the rollout (canary becomes new stable).
    #[instrument(skip(self))]
    pub fn complete(&self, app: &str, profile: &str) -> Result<String, CanaryError> {
        let mut rollouts = self.rollouts.write();
        let key = (app.to_string(), profile.to_string());

        let rollout = rollouts.remove(&key)
            .ok_or_else(|| CanaryError::RolloutNotFound {
                app: app.to_string(),
                profile: profile.to_string(),
            })?;

        let new_stable = rollout.config.canary_version;
        info!(new_stable = %new_stable, "Rollout completed");

        Ok(new_stable)
    }

    /// Gets the current status of a rollout.
    pub fn status(&self, app: &str, profile: &str) -> Option<RolloutStatus> {
        let rollouts = self.rollouts.read();
        let key = (app.to_string(), profile.to_string());

        rollouts.get(&key).map(|r| RolloutStatus {
            id: r.config.id,
            app: r.config.app.clone(),
            profile: r.config.profile.clone(),
            stable_version: r.config.stable_version.clone(),
            canary_version: r.config.canary_version.clone(),
            current_stage: r.current_stage().map(|s| s.name.clone()),
            current_percentage: r.current_percentage(),
            canary_metrics: r.metrics.canary_metrics(),
            stable_metrics: r.metrics.stable_metrics(),
        })
    }
}

impl Default for CanaryEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Status of an active rollout.
#[derive(Debug, Clone)]
pub struct RolloutStatus {
    pub id: Uuid,
    pub app: String,
    pub profile: String,
    pub stable_version: String,
    pub canary_version: String,
    pub current_stage: Option<String>,
    pub current_percentage: u8,
    pub canary_metrics: super::metrics::AggregatedMetrics,
    pub stable_metrics: super::metrics::AggregatedMetrics,
}

/// Errors that can occur in canary operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CanaryError {
    #[error("rollout already active for {app}/{profile}")]
    RolloutAlreadyActive { app: String, profile: String },

    #[error("rollout not found for {app}/{profile}")]
    RolloutNotFound { app: String, profile: String },

    #[error("already at final stage")]
    AlreadyAtFinalStage,

    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
}
```

### Paso 5: Modulo y Re-exports

```rust
// src/canary/mod.rs
//! Canary rollout engine for progressive configuration deployments.
//!
//! This module provides the core functionality for canary rollouts:
//! - Consistent hashing for deterministic traffic splitting
//! - Stage-based progression with configurable percentages
//! - Metrics collection and evaluation for automatic promotion/rollback
//!
//! # Example
//!
//! ```rust
//! use vortex_rollout::canary::{CanaryEngine, RolloutConfig};
//!
//! let engine = CanaryEngine::new();
//!
//! // Start a rollout
//! let config = RolloutConfig::new("myapp", "prod", "v1.0", "v1.1");
//! engine.start_rollout(config)?;
//!
//! // Resolve version for a request
//! let resolved = engine.resolve("myapp", "prod", "user-123", "v1.0");
//! println!("Serving version: {}", resolved.version);
//! ```

pub mod engine;
pub mod hasher;
pub mod metrics;
pub mod stage;

pub use engine::{CanaryEngine, CanaryError, RolloutConfig, RolloutStatus, ResolvedVersion};
pub use hasher::ConsistentHasher;
pub use metrics::{AggregatedMetrics, EvaluationResult, MetricsEvaluator, SuccessMetricsCollector};
pub use stage::{RolloutStage, StagePresets, SuccessThreshold};
```

---

## Conceptos de Rust Aprendidos

### 1. Consistent Hashing con xxHash

xxHash es uno de los algoritmos de hashing mas rapidos disponibles, perfecto para hot paths.

**Rust:**
```rust
use xxhash_rust::xxh3::xxh3_64;

pub struct ConsistentHasher {
    seed: u64,
}

impl ConsistentHasher {
    pub fn bucket(&self, key: &str) -> u8 {
        // Combine seed with key
        let mut data = self.seed.to_le_bytes().to_vec();
        data.extend_from_slice(key.as_bytes());

        // Hash and modulo to get bucket 0-99
        let hash = xxh3_64(&data);
        (hash % 100) as u8
    }

    pub fn is_canary(&self, key: &str, percentage: u8) -> bool {
        self.bucket(key) < percentage
    }
}

// Usage: deterministic assignment
let hasher = ConsistentHasher::new(42);
assert_eq!(hasher.is_canary("user-123", 5), hasher.is_canary("user-123", 5));
```

**Comparacion con Java (Guava Hashing):**
```java
import com.google.common.hash.Hashing;

public class ConsistentHasher {
    private final long seed;

    public int bucket(String key) {
        // Guava's consistent hashing
        return Hashing.consistentHash(
            Hashing.murmur3_128(seed).hashString(key, StandardCharsets.UTF_8),
            100
        );
    }

    public boolean isCanary(String key, int percentage) {
        return bucket(key) < percentage;
    }
}
```

**Diferencias clave:**
| Aspecto | Rust (xxHash) | Java (Guava) |
|---------|---------------|--------------|
| Performance | ~10GB/s | ~2GB/s |
| Zero-copy | slice de bytes | String allocation |
| Crate size | ~50KB | Guava es ~3MB |
| API | Funcional | OOP |

### 2. Interior Mutability con parking_lot

`parking_lot` proporciona primitivas de sincronizacion mas rapidas que `std::sync`.

**Rust:**
```rust
use parking_lot::RwLock;
use std::collections::HashMap;

pub struct CanaryEngine {
    // RwLock permite multiples readers O un writer
    rollouts: RwLock<HashMap<(String, String), ActiveRollout>>,
}

impl CanaryEngine {
    pub fn resolve(&self, app: &str, profile: &str, key: &str) -> ResolvedVersion {
        // Read lock - multiple concurrent readers allowed
        let rollouts = self.rollouts.read();
        // ...
    }

    pub fn start_rollout(&self, config: RolloutConfig) -> Result<Uuid, Error> {
        // Write lock - exclusive access
        let mut rollouts = self.rollouts.write();
        rollouts.insert(key, rollout);
        // ...
    }
}
```

**Comparacion con Java:**
```java
import java.util.concurrent.locks.ReadWriteLock;
import java.util.concurrent.locks.ReentrantReadWriteLock;

public class CanaryEngine {
    private final Map<String, ActiveRollout> rollouts = new HashMap<>();
    private final ReadWriteLock lock = new ReentrantReadWriteLock();

    public ResolvedVersion resolve(String app, String profile, String key) {
        lock.readLock().lock();
        try {
            // Read operation
        } finally {
            lock.readLock().unlock();
        }
    }

    public void startRollout(RolloutConfig config) {
        lock.writeLock().lock();
        try {
            rollouts.put(key, rollout);
        } finally {
            lock.writeLock().unlock();
        }
    }
}
```

**Ventajas de parking_lot:**
- 2-3x mas rapido que std::sync
- No hay poisoning (panic no corrompe lock)
- Read guards mas pequenos en memoria
- fair locking opcional

### 3. Time-windowed Metrics Collection

Colectar metricas en ventanas de tiempo es comun para evaluar salud.

**Rust:**
```rust
use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub struct MetricsCollector {
    window: Duration,
    samples: RwLock<VecDeque<Sample>>,
}

impl MetricsCollector {
    pub fn record(&self, sample: Sample) {
        let mut samples = self.samples.write();
        samples.push_back(sample);

        // Prune old samples (sliding window)
        let cutoff = Instant::now() - self.window;
        while let Some(front) = samples.front() {
            if front.timestamp < cutoff {
                samples.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn aggregate(&self) -> AggregatedMetrics {
        let samples = self.samples.read();
        let cutoff = Instant::now() - self.window;

        samples
            .iter()
            .filter(|s| s.timestamp > cutoff)
            .fold(AggregatedMetrics::default(), |acc, s| {
                // Aggregate...
            })
    }
}
```

**Comparacion con Java (Micrometer):**
```java
import io.micrometer.core.instrument.MeterRegistry;
import io.micrometer.core.instrument.Timer;

public class MetricsCollector {
    private final Timer requestTimer;
    private final Counter errorCounter;

    public MetricsCollector(MeterRegistry registry) {
        this.requestTimer = Timer.builder("requests")
            .publishPercentiles(0.99)
            .register(registry);
        this.errorCounter = registry.counter("errors");
    }

    public void record(Duration latency, boolean success) {
        requestTimer.record(latency);
        if (!success) {
            errorCounter.increment();
        }
    }
}
```

### 4. Builder Pattern con Defaults

Combinar builder pattern con `Default` trait para configuracion flexible.

**Rust:**
```rust
#[derive(Debug, Clone)]
pub struct SuccessThreshold {
    pub min_success_rate: f64,
    pub max_error_rate: f64,
    pub max_latency_p99_ms: u64,
}

impl Default for SuccessThreshold {
    fn default() -> Self {
        Self {
            min_success_rate: 0.99,
            max_error_rate: 0.01,
            max_latency_p99_ms: 100,
        }
    }
}

// Con serde, defaults se aplican automaticamente
#[derive(Deserialize)]
pub struct RolloutStage {
    pub name: String,
    pub percentage: u8,
    #[serde(default)]  // Usa Default::default()
    pub threshold: SuccessThreshold,
}
```

**Comparacion con Java:**
```java
public class SuccessThreshold {
    private double minSuccessRate = 0.99;
    private double maxErrorRate = 0.01;
    private long maxLatencyP99Ms = 100;

    public static Builder builder() {
        return new Builder();
    }

    public static class Builder {
        private SuccessThreshold threshold = new SuccessThreshold();

        public Builder minSuccessRate(double rate) {
            threshold.minSuccessRate = rate;
            return this;
        }
        // ... more setters

        public SuccessThreshold build() {
            return threshold;
        }
    }
}
```

---

## Riesgos y Errores Comunes

### 1. Hash Collision en Keys Cortas

```rust
// MAL: Keys muy cortas pueden tener colisiones
let bucket = hasher.bucket("1");  // Solo 1 byte

// BIEN: Usar keys con suficiente entropia
let bucket = hasher.bucket("user-1-session-abc123");
```

### 2. Race Condition en Promote

```rust
// MAL: Check-then-act sin lock
if self.can_promote(app, profile) {  // Read lock released
    self.promote(app, profile)?;      // Write lock - state could have changed!
}

// BIEN: Atomic operation
pub fn try_promote(&self, app: &str, profile: &str) -> Result<RolloutStage, Error> {
    let mut rollouts = self.rollouts.write();  // Hold lock for entire operation
    let rollout = rollouts.get_mut(&key)?;

    // Validation
    let next_idx = rollout.current_stage_idx + 1;
    if next_idx >= rollout.config.stages.len() {
        return Err(Error::AlreadyAtFinalStage);
    }

    // Mutation
    rollout.current_stage_idx = next_idx;
    Ok(rollout.config.stages[next_idx].clone())
}
```

### 3. Metrics Window Drift

```rust
// MAL: Pruning solo en record puede causar datos stale
pub fn record(&self, sample: Sample) {
    self.samples.push(sample);
    self.prune_old();  // Solo se llama cuando hay writes
}

// BIEN: Prune en read tambien, o usar background task
pub fn aggregate(&self) -> AggregatedMetrics {
    let cutoff = Instant::now() - self.window;

    // Filter during aggregation
    self.samples
        .iter()
        .filter(|s| s.timestamp > cutoff)
        .fold(...)
}
```

### 4. Percentage Edge Cases

```rust
// MAL: No validar percentage
pub fn new(percentage: u8) -> Self {
    Self { percentage }  // Could be > 100!
}

// BIEN: Clamp or validate
pub fn new(percentage: u8) -> Self {
    Self {
        percentage: percentage.min(100)
    }
}

// O con validacion explicita
pub fn new(percentage: u8) -> Result<Self, Error> {
    if percentage > 100 {
        return Err(Error::InvalidPercentage(percentage));
    }
    Ok(Self { percentage })
}
```

---

## Pruebas

### Tests de Distribucion del Hasher

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hasher_is_deterministic() {
        let hasher = ConsistentHasher::new(42);
        let key = "user-12345";

        let bucket1 = hasher.bucket(key);
        let bucket2 = hasher.bucket(key);
        let bucket3 = hasher.bucket(key);

        assert_eq!(bucket1, bucket2);
        assert_eq!(bucket2, bucket3);
    }

    #[test]
    fn hasher_distribution_is_uniform() {
        let hasher = ConsistentHasher::new(12345);
        let mut buckets = [0u32; 100];

        // Hash 100,000 different keys
        for i in 0..100_000 {
            let key = format!("user-{}", i);
            let bucket = hasher.bucket(&key) as usize;
            buckets[bucket] += 1;
        }

        // Each bucket should have ~1000 keys
        // Allow 20% deviation
        for (i, &count) in buckets.iter().enumerate() {
            assert!(
                count >= 800 && count <= 1200,
                "Bucket {} has {} keys (expected ~1000)",
                i, count
            );
        }
    }

    #[test]
    fn different_seeds_produce_different_assignments() {
        let hasher1 = ConsistentHasher::new(1);
        let hasher2 = ConsistentHasher::new(2);

        let key = "user-123";

        // Different seeds should (usually) produce different buckets
        // This test might occasionally fail by chance, but very unlikely
        let different_count = (0..100)
            .filter(|i| {
                let k = format!("user-{}", i);
                hasher1.bucket(&k) != hasher2.bucket(&k)
            })
            .count();

        assert!(different_count > 50, "Seeds should produce different distributions");
    }

    #[test]
    fn canary_percentage_respected() {
        let hasher = ConsistentHasher::new(42);

        let canary_count = (0..10_000)
            .filter(|i| {
                let key = format!("user-{}", i);
                hasher.is_canary(&key, 5)  // 5% canary
            })
            .count();

        // Should be approximately 5% (500 out of 10,000)
        // Allow 1% deviation
        assert!(
            canary_count >= 400 && canary_count <= 600,
            "Canary count {} not within expected range",
            canary_count
        );
    }
}
```

### Tests del Engine

```rust
#[cfg(test)]
mod engine_tests {
    use super::*;

    #[test]
    fn start_and_resolve_rollout() {
        let engine = CanaryEngine::new();

        let config = RolloutConfig::new("myapp", "prod", "v1.0", "v1.1")
            .with_stages(vec![
                RolloutStage::new("initial", 10, Duration::from_secs(60)),
            ]);

        engine.start_rollout(config).unwrap();

        // Some users should get canary, others stable
        let resolved1 = engine.resolve("myapp", "prod", "user-abc", "v1.0");
        let resolved2 = engine.resolve("myapp", "prod", "user-xyz", "v1.0");

        // At least verify consistency
        let resolved1_again = engine.resolve("myapp", "prod", "user-abc", "v1.0");
        assert_eq!(resolved1.version, resolved1_again.version);
    }

    #[test]
    fn cannot_start_duplicate_rollout() {
        let engine = CanaryEngine::new();

        let config = RolloutConfig::new("myapp", "prod", "v1.0", "v1.1");
        engine.start_rollout(config.clone()).unwrap();

        let result = engine.start_rollout(config);
        assert!(matches!(result, Err(CanaryError::RolloutAlreadyActive { .. })));
    }

    #[test]
    fn promote_advances_stage() {
        let engine = CanaryEngine::new();

        let config = RolloutConfig::new("myapp", "prod", "v1.0", "v1.1")
            .with_stages(vec![
                RolloutStage::new("s1", 5, Duration::from_secs(0)),
                RolloutStage::new("s2", 25, Duration::from_secs(0)),
                RolloutStage::new("s3", 100, Duration::from_secs(0)),
            ]);

        engine.start_rollout(config).unwrap();

        let status = engine.status("myapp", "prod").unwrap();
        assert_eq!(status.current_percentage, 5);

        engine.promote("myapp", "prod").unwrap();

        let status = engine.status("myapp", "prod").unwrap();
        assert_eq!(status.current_percentage, 25);
    }

    #[test]
    fn rollback_removes_rollout() {
        let engine = CanaryEngine::new();

        let config = RolloutConfig::new("myapp", "prod", "v1.0", "v1.1");
        engine.start_rollout(config).unwrap();

        assert!(engine.status("myapp", "prod").is_some());

        engine.rollback("myapp", "prod").unwrap();

        assert!(engine.status("myapp", "prod").is_none());

        // Resolve should return default version
        let resolved = engine.resolve("myapp", "prod", "user-123", "v1.0");
        assert_eq!(resolved.version, "v1.0");
        assert!(!resolved.is_canary);
    }
}
```

---

## Observabilidad

### Metricas

```rust
use metrics::{counter, gauge, histogram};

impl CanaryEngine {
    pub fn resolve(&self, app: &str, profile: &str, key: &str, default: &str) -> ResolvedVersion {
        let result = self.resolve_internal(app, profile, key, default);

        // Record metrics
        counter!("canary_resolutions_total",
            "app" => app.to_string(),
            "is_canary" => result.is_canary.to_string()
        ).increment(1);

        if let Some(ref stage) = result.stage {
            gauge!("canary_stage_percentage",
                "app" => app.to_string(),
                "stage" => stage.clone()
            ).set(result.percentage as f64);
        }

        result
    }
}
```

### Logging

```rust
use tracing::{info, warn, instrument};

#[instrument(skip(self), fields(rollout_id = %config.id))]
pub fn start_rollout(&self, config: RolloutConfig) -> Result<Uuid, Error> {
    info!(
        app = %config.app,
        profile = %config.profile,
        stable = %config.stable_version,
        canary = %config.canary_version,
        stages = config.stages.len(),
        "Starting canary rollout"
    );
    // ...
}

#[instrument(skip(self))]
pub fn promote(&self, app: &str, profile: &str) -> Result<RolloutStage, Error> {
    // ...
    info!(
        stage = %new_stage.name,
        percentage = new_stage.percentage,
        "Promoted to next stage"
    );
}
```

---

## Seguridad

### Consideraciones

1. **Hashing determinista**: No usar para datos sensibles (el patron es predecible)
2. **Rate limiting**: Limitar frecuencia de operaciones de rollout
3. **Audit log**: Registrar quien inicia/promueve/rollback rollouts

```rust
/// Audit event for rollout operations.
#[derive(Debug, Serialize)]
pub struct RolloutAuditEvent {
    pub timestamp: DateTime<Utc>,
    pub rollout_id: Uuid,
    pub action: RolloutAction,
    pub actor: String,
    pub app: String,
    pub profile: String,
}

#[derive(Debug, Serialize)]
pub enum RolloutAction {
    Started { canary_version: String },
    Promoted { from_stage: String, to_stage: String },
    RolledBack { reason: String },
    Completed { new_stable: String },
}
```

---

## Entregable Final

### Archivos Creados

1. `crates/vortex-rollout/src/canary/mod.rs` - Modulo y re-exports
2. `crates/vortex-rollout/src/canary/hasher.rs` - ConsistentHasher
3. `crates/vortex-rollout/src/canary/stage.rs` - RolloutStage, SuccessThreshold
4. `crates/vortex-rollout/src/canary/metrics.rs` - SuccessMetricsCollector
5. `crates/vortex-rollout/src/canary/engine.rs` - CanaryEngine
6. `crates/vortex-rollout/tests/canary_tests.rs` - Tests de integracion

### Verificacion

```bash
# Compilar
cargo build -p vortex-rollout

# Tests
cargo test -p vortex-rollout canary

# Benchmarks
cargo bench -p vortex-rollout hasher

# Clippy
cargo clippy -p vortex-rollout -- -D warnings

# Doc
cargo doc -p vortex-rollout --open
```

### Ejemplo de Uso

```rust
use vortex_rollout::canary::{CanaryEngine, RolloutConfig, StagePresets};
use std::time::Duration;

fn main() {
    let engine = CanaryEngine::new();

    // Create rollout with conservative stages
    let config = RolloutConfig::new("payment-service", "production", "v2.3.0", "v2.4.0")
        .with_stages(StagePresets::conservative());

    // Start rollout
    let rollout_id = engine.start_rollout(config).expect("Failed to start rollout");
    println!("Started rollout: {}", rollout_id);

    // Simulate request resolution
    let user_id = "user-12345";
    let resolved = engine.resolve("payment-service", "production", user_id, "v2.3.0");

    println!("User {} gets version {} (canary: {})",
        user_id, resolved.version, resolved.is_canary);

    // Record request result
    engine.record_request(
        "payment-service",
        "production",
        Duration::from_millis(45),
        true,  // success
        resolved.is_canary,
    );

    // Evaluate metrics
    if let Some(eval) = engine.evaluate("payment-service", "production") {
        if eval.can_promote() {
            engine.promote("payment-service", "production").unwrap();
            println!("Promoted to next stage!");
        } else if eval.should_rollback() {
            engine.rollback("payment-service", "production").unwrap();
            println!("Rolled back due to: {:?}", eval);
        }
    }
}
```

---

**Siguiente**: [Historia 002 - API de Rollouts](./story-002-rollout-api.md)
