# Epica 10: Enterprise - Canary, Drift, Federation

## Objetivo

Implementar capacidades enterprise para production-readiness en Vortex Config. Esta epica transforma el servidor de configuracion en una solucion robusta para deployments a gran escala, agregando:

1. **Canary Rollout Engine**: Motor para despliegues progresivos de configuraciones con metricas de exito
2. **API de Rollouts**: Endpoints REST para gestionar el ciclo de vida de rollouts
3. **Drift Detection**: Deteccion automatica de instancias con configuraciones desactualizadas
4. **Heartbeat SDK**: Cliente ligero para que aplicaciones reporten su estado
5. **Multi-Cluster Federation**: Sincronizacion de configuraciones entre clusters via gRPC
6. **Production Readiness**: Helm charts, health checks avanzados, graceful shutdown

Las capacidades enterprise son fundamentales para:
- **Deployments seguros**: Rollouts progresivos que pueden revertirse automaticamente
- **Visibilidad operacional**: Saber que version de configuracion tiene cada instancia
- **Alta disponibilidad**: Federacion para redundancia geografica
- **Cloud-native**: Integracion nativa con Kubernetes y observabilidad moderna

---

## Conceptos de Rust Cubiertos (Nivel Enterprise)

| Concepto | Historia | Comparacion con Java |
|----------|----------|---------------------|
| Consistent hashing | 001 | Guava Hashing |
| State machines tipadas | 001, 002 | State pattern / Spring State Machine |
| gRPC con tonic | 005 | gRPC-java / grpc-spring-boot |
| Protocol Buffers | 005 | protobuf-java |
| Async bidirectional streams | 005 | Reactive gRPC |
| Tokio graceful shutdown | 006 | @PreDestroy / shutdown hooks |
| Tower middleware layers | 002 | Spring Interceptors |
| Health check patterns | 006 | Spring Actuator |
| Docker multi-stage builds | 006 | Maven/Jib |
| Metrics aggregation | 003, 004 | Micrometer |

---

## Historias de Usuario

| # | Titulo | Descripcion | Puntos |
|---|--------|-------------|--------|
| 001 | [Canary Rollout Engine](./story-001-canary-engine.md) | Motor para rollouts progresivos con metricas | 8 |
| 002 | [API de Rollouts](./story-002-rollout-api.md) | Endpoints para start/promote/rollback | 5 |
| 003 | [Drift Detection](./story-003-drift-detection.md) | Detectar instancias con config desactualizada | 5 |
| 004 | [Heartbeat SDK](./story-004-heartbeat-sdk.md) | Cliente ligero para reportar estado | 5 |
| 005 | [Multi-Cluster Federation](./story-005-federation.md) | Sincronizacion entre clusters con gRPC | 8 |
| 006 | [Production Readiness](./story-006-production-ready.md) | Helm charts, health checks, graceful shutdown | 5 |

**Total**: 36 puntos de historia

---

## Dependencias

### Epicas Prerequisito

| Epica | Razon |
|-------|-------|
| 05 - Cache Config | Sistema de cache y metricas para rollouts |
| 07 - Governance | Politicas PLAC que aplican durante rollouts |
| 08 - Realtime | WebSockets para notificar cambios de rollout |

### Dependencias de Crates

```toml
[dependencies]
# gRPC
tonic = "0.11"
prost = "0.12"
prost-types = "0.12"

# Async runtime
tokio = { version = "1", features = ["full", "sync", "signal"] }
tokio-stream = "0.1"
futures = "0.3"

# HTTP
axum = { version = "0.7", features = ["ws"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["trace", "cors"] }

# Hashing
xxhash-rust = { version = "0.8", features = ["xxh3"] }

# Serializacion
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Observabilidad
tracing = "0.1"
metrics = "0.22"
metrics-exporter-prometheus = "0.13"

# Time
chrono = { version = "0.4", features = ["serde"] }
tokio-cron-scheduler = "0.10"

# Errores
thiserror = "1"

# Utils
uuid = { version = "1", features = ["v4", "serde"] }
dashmap = "5"

[build-dependencies]
tonic-build = "0.11"

[dev-dependencies]
tokio-test = "0.4"
tempfile = "3"
mockall = "0.12"
```

---

## Criterios de Aceptacion

### Funcionales

- [ ] Rollouts canary con stages configurables (1%, 5%, 25%, 50%, 100%)
- [ ] Metricas de exito evaluan automaticamente si promover o rollback
- [ ] API REST para iniciar, promover, pausar y revertir rollouts
- [ ] Drift detection identifica instancias con versiones antiguas
- [ ] Heartbeat SDK reporta version, estado y metricas cada 30s
- [ ] Federation sincroniza configs entre clusters en < 5s
- [ ] Health checks `/health/live` y `/health/ready` conformes a Kubernetes

### No Funcionales

- [ ] Rollout promotion latency < 100ms
- [ ] Drift detection scan < 1s para 1000 instancias
- [ ] Federation sync p99 < 500ms entre regiones
- [ ] Heartbeat SDK footprint < 5MB
- [ ] Graceful shutdown drena conexiones en < 30s
- [ ] Cold start con federation < 2s

### Compatibilidad

- [ ] gRPC compatible con clients en cualquier lenguaje
- [ ] Helm chart compatible con Kubernetes 1.25+
- [ ] Docker images multi-arch (amd64, arm64)
- [ ] Prometheus metrics compatibles con Grafana dashboards

---

## Definition of Done

- [ ] Codigo compila sin warnings (`cargo build --all-features`)
- [ ] Formateado con `cargo fmt`
- [ ] Sin errores de clippy (`cargo clippy -- -D warnings`)
- [ ] Tests unitarios con cobertura > 80%
- [ ] Tests de integracion para federation
- [ ] Load tests con 1000 instancias simuladas
- [ ] Rustdoc para todas las APIs publicas
- [ ] Changelog actualizado
- [ ] Sin `unwrap()` en codigo de produccion
- [ ] Helm chart validado con `helm lint`
- [ ] Docker image < 50MB
- [ ] CI pipeline verde

---

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigacion |
|--------|--------------|---------|------------|
| Split-brain en federation | Media | Critico | Vector clocks, conflict resolution determinista |
| Rollback cascada | Media | Alto | Circuit breaker, max rollback rate |
| Heartbeat flood | Alta | Medio | Rate limiting, agregacion en servidor |
| gRPC latency entre regiones | Alta | Medio | Async replication, read-local |
| Inconsistencia en canary | Media | Alto | Consistent hashing por request |
| Drift false positives | Media | Bajo | Grace period, version tolerance |

---

## Decisiones Arquitectonicas (ADRs)

### ADR-001: Consistent Hashing para Canary Assignment

**Estado**: Aceptado

**Contexto**: Necesitamos asignar requests a grupos canary de forma determinista y consistente.

**Decision**: Usar consistent hashing con xxHash3 sobre un identificador estable (user_id, session_id, o request_id).

**Razones**:
- Mismo usuario siempre ve misma version (sticky sessions sin estado)
- Redistribucion minima al cambiar porcentajes
- xxHash3 es extremadamente rapido (>10GB/s)
- Sin necesidad de almacenar asignaciones

**Diagrama**:
```
┌─────────────────────────────────────────────────────────────────────┐
│                    Consistent Hashing Ring                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│         0%                    50%                    100%            │
│         │                      │                      │              │
│    ─────┼──────────────────────┼──────────────────────┼─────        │
│         │◄─── Canary (5%) ────►│◄──── Stable ────────►│             │
│         │                      │                      │              │
│         │   hash("user123")    │                      │              │
│         │         │            │                      │              │
│         │         ▼            │                      │              │
│         │    [lands in canary] │                      │              │
│                                                                      │
│  hash(user_id) % 100 < canary_percentage ? Canary : Stable          │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### ADR-002: State Machine para Rollout Lifecycle

**Estado**: Aceptado

**Contexto**: Los rollouts tienen un ciclo de vida complejo con multiples estados y transiciones.

**Decision**: Modelar el rollout como una state machine tipada con transiciones explicitas.

**Estados**:
```
┌─────────────────────────────────────────────────────────────────────┐
│                     Rollout State Machine                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────┐    start()    ┌──────────┐   promote()  ┌──────────┐ │
│  │ Created  │──────────────►│ Running  │─────────────►│ Running  │ │
│  └──────────┘               │ (stage 1)│              │ (stage N)│ │
│                             └─────┬────┘              └─────┬────┘ │
│                                   │                         │       │
│                     pause()       │                         │       │
│                         ┌─────────▼─────────┐               │       │
│                         │      Paused       │               │       │
│                         └─────────┬─────────┘               │       │
│                                   │ resume()                │       │
│                                   │                         │       │
│       rollback()                  ▼         complete()      │       │
│    ┌────────────────────────────────────────────────────────┘       │
│    │                              │                                  │
│    ▼                              ▼                                  │
│  ┌──────────┐              ┌──────────┐                             │
│  │ RolledBack│              │ Completed │                             │
│  └──────────┘              └──────────┘                             │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### ADR-003: gRPC para Federation

**Estado**: Aceptado

**Contexto**: Necesitamos sincronizar configuraciones entre clusters con baja latencia y alta confiabilidad.

**Decision**: Usar gRPC con tonic para comunicacion inter-cluster.

**Razones**:
- Streaming bidireccional para sync continuo
- Protobuf para serializacion eficiente
- HTTP/2 con multiplexing
- Generacion automatica de clients en cualquier lenguaje
- TLS mutual para seguridad

**Alternativas consideradas**:
- HTTP REST: Mayor latencia, no streaming
- Kafka: Overkill, agrega dependencia
- Redis Pub/Sub: No garantiza entrega

### ADR-004: Heartbeat Aggregation

**Estado**: Aceptado

**Contexto**: Miles de instancias enviando heartbeats pueden saturar el servidor.

**Decision**: Agregar heartbeats en ventanas de tiempo y usar muestreo para metricas.

**Estrategia**:
- Heartbeats se procesan en batch cada 5s
- Solo se almacena ultimo heartbeat por instancia
- Metricas se agregan por app/profile/version
- Alertas se basan en conteos agregados

---

## Reglas Estrictas

1. **Rollouts atomicos**: Un rollout completo o falla, nunca estados intermedios corruptos
2. **Idempotencia**: Todas las operaciones de rollout son idempotentes
3. **Backward compatibility**: Canary siempre puede servir version anterior
4. **Federation eventual consistency**: Tolerar divergencia temporal entre clusters
5. **Heartbeat best-effort**: Perdida de heartbeats no afecta servicio
6. **Graceful degradation**: Si federation falla, servir desde local
7. **Audit trail**: Todas las operaciones de rollout se auditan
8. **No rollout sin baseline**: Debe existir version estable antes de canary

---

## Estructura del Crate

```
crates/vortex-rollout/
├── Cargo.toml
├── src/
│   ├── lib.rs                  # Re-exports
│   ├── error.rs                # Tipos de error
│   ├── canary/
│   │   ├── mod.rs              # Modulo canary
│   │   ├── engine.rs           # CanaryEngine
│   │   ├── hasher.rs           # Consistent hashing
│   │   ├── stage.rs            # RolloutStage
│   │   └── metrics.rs          # Success metrics evaluation
│   ├── rollout/
│   │   ├── mod.rs              # Modulo rollout
│   │   ├── state.rs            # RolloutState enum
│   │   ├── manager.rs          # RolloutManager
│   │   └── repository.rs       # Rollout persistence
│   └── api/
│       ├── mod.rs              # API handlers
│       └── handlers.rs         # REST endpoints

crates/vortex-drift/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── detector.rs             # DriftDetector
│   ├── heartbeat.rs            # HeartbeatReceiver
│   └── aggregator.rs           # MetricsAggregator

crates/vortex-federation/
├── Cargo.toml
├── build.rs                    # tonic-build
├── proto/
│   └── federation.proto        # gRPC definitions
├── src/
│   ├── lib.rs
│   ├── server.rs               # gRPC server
│   ├── client.rs               # gRPC client
│   ├── sync.rs                 # Sync logic
│   └── conflict.rs             # Conflict resolution

crates/vortex-client/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── heartbeat.rs            # Heartbeat client
│   ├── config.rs               # Client config
│   └── retry.rs                # Retry logic

charts/vortex-config/
├── Chart.yaml
├── values.yaml
├── templates/
│   ├── deployment.yaml
│   ├── service.yaml
│   ├── configmap.yaml
│   ├── hpa.yaml
│   └── servicemonitor.yaml
```

---

## Diagrama de Arquitectura Enterprise

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         VORTEX CONFIG ENTERPRISE                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        CLUSTER A (Primary)                           │   │
│  │                                                                      │   │
│  │   ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐    │   │
│  │   │   Axum API  │    │  WebSocket  │    │   gRPC Federation   │    │   │
│  │   │   (REST)    │    │   Server    │    │      Server         │    │   │
│  │   └──────┬──────┘    └──────┬──────┘    └──────────┬──────────┘    │   │
│  │          │                  │                      │               │   │
│  │   ┌──────┴──────────────────┴──────────────────────┴──────┐       │   │
│  │   │                  Canary Rollout Engine                 │       │   │
│  │   │  ┌──────────┐  ┌──────────┐  ┌───────────────────┐   │       │   │
│  │   │  │Consistent│  │  Stage   │  │ Success Metrics   │   │       │   │
│  │   │  │ Hasher   │  │ Manager  │  │   Evaluator       │   │       │   │
│  │   │  └──────────┘  └──────────┘  └───────────────────┘   │       │   │
│  │   └───────────────────────────────────────────────────────┘       │   │
│  │          │                                                         │   │
│  │   ┌──────┴─────────────────────────────────────────────────┐      │   │
│  │   │                   Drift Detector                        │      │   │
│  │   │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │      │   │
│  │   │  │  Heartbeat   │  │   Instance   │  │    Alert     │ │      │   │
│  │   │  │  Receiver    │  │   Registry   │  │   Manager    │ │      │   │
│  │   │  └──────────────┘  └──────────────┘  └──────────────┘ │      │   │
│  │   └────────────────────────────────────────────────────────┘      │   │
│  │                                                                    │   │
│  └────────────────────────────────┬───────────────────────────────────┘   │
│                                   │                                        │
│                              gRPC Sync                                     │
│                                   │                                        │
│  ┌────────────────────────────────┼───────────────────────────────────┐   │
│  │                        CLUSTER B (Replica)                          │   │
│  │                                │                                    │   │
│  │   ┌────────────────────────────▼─────────────────────────────┐     │   │
│  │   │                  gRPC Federation Client                   │     │   │
│  │   │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │     │   │
│  │   │  │   Stream     │  │   Conflict   │  │    Local     │   │     │   │
│  │   │  │   Receiver   │  │   Resolver   │  │    Cache     │   │     │   │
│  │   │  └──────────────┘  └──────────────┘  └──────────────┘   │     │   │
│  │   └──────────────────────────────────────────────────────────┘     │   │
│  │                                                                    │   │
│  └────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                        ┌───────────┴───────────┐
                        ▼                       ▼
              ┌─────────────────┐     ┌─────────────────┐
              │   Application   │     │   Application   │
              │   Instance 1    │     │   Instance N    │
              │                 │     │                 │
              │ ┌─────────────┐│     │ ┌─────────────┐│
              │ │  Heartbeat  ││     │ │  Heartbeat  ││
              │ │    SDK      ││     │ │    SDK      ││
              │ └──────┬──────┘│     │ └──────┬──────┘│
              └────────┼───────┘     └────────┼───────┘
                       │                      │
                       └──────────┬───────────┘
                                  │
                            POST /heartbeat
```

---

## Diagrama de Flujo de Canary Rollout

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                         Canary Rollout Flow                                    │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│  1. Create Rollout                                                            │
│     ┌──────────────────────────────────────────────────────────────┐         │
│     │  POST /api/rollouts                                           │         │
│     │  { "app": "myapp", "from_version": "v1", "to_version": "v2" } │         │
│     └──────────────────────────────────────────────────────────────┘         │
│                           │                                                   │
│                           ▼                                                   │
│  2. Start Rollout (Stage 1: 1%)                                              │
│     ┌──────────────────────────────────────────────────────────────┐         │
│     │  Request comes in: GET /myapp/prod                            │         │
│     │                                                               │         │
│     │  user_id = "user-12345"                                       │         │
│     │  hash = xxh3(user_id) % 100 = 3                               │         │
│     │                                                               │         │
│     │  if hash < 1:  → Return v2 (canary)                           │         │
│     │  else:         → Return v1 (stable)                           │         │
│     └──────────────────────────────────────────────────────────────┘         │
│                           │                                                   │
│                           ▼                                                   │
│  3. Monitor Metrics (5 min window)                                           │
│     ┌──────────────────────────────────────────────────────────────┐         │
│     │  success_rate_canary = 99.5%                                  │         │
│     │  success_rate_stable = 99.8%                                  │         │
│     │  error_rate_canary = 0.5%                                     │         │
│     │  latency_p99_canary = 45ms                                    │         │
│     │                                                               │         │
│     │  if success_rate_canary >= threshold (99%):                   │         │
│     │      → Auto-promote to next stage                             │         │
│     │  elif error_rate_canary > max_error (5%):                     │         │
│     │      → Auto-rollback                                          │         │
│     └──────────────────────────────────────────────────────────────┘         │
│                           │                                                   │
│                           ▼                                                   │
│  4. Promote through stages                                                   │
│     ┌──────────────────────────────────────────────────────────────┐         │
│     │  Stage 1: 1%   ─────► Stage 2: 5%   ─────► Stage 3: 25%      │         │
│     │                                                               │         │
│     │  Stage 4: 50%  ─────► Stage 5: 100% ─────► COMPLETED         │         │
│     └──────────────────────────────────────────────────────────────┘         │
│                           │                                                   │
│                           ▼                                                   │
│  5. Complete or Rollback                                                     │
│     ┌──────────────────────────────────────────────────────────────┐         │
│     │  COMPLETED: v2 is now stable, v1 archived                     │         │
│     │  ROLLED_BACK: v1 restored to 100%, v2 discarded              │         │
│     └──────────────────────────────────────────────────────────────┘         │
│                                                                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## Changelog

| Version | Fecha | Cambios |
|---------|-------|---------|
| 0.1.0 | 2025-01-XX | Creacion inicial de la epica |

---

## Referencias

- [Tonic gRPC Documentation](https://docs.rs/tonic)
- [Protocol Buffers](https://protobuf.dev/)
- [Kubernetes Health Checks](https://kubernetes.io/docs/tasks/configure-pod-container/configure-liveness-readiness-startup-probes/)
- [Helm Best Practices](https://helm.sh/docs/chart_best_practices/)
- [Consistent Hashing Explained](https://www.toptal.com/big-data/consistent-hashing)
- [Canary Deployments](https://martinfowler.com/bliki/CanaryRelease.html)
- [Graceful Shutdown in Kubernetes](https://kubernetes.io/docs/concepts/workloads/pods/pod-lifecycle/)
- [Tokio Graceful Shutdown](https://tokio.rs/tokio/topics/shutdown)
