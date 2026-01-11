# Análisis y Complemento del PRD — Vortex Config

## Evaluación Ejecutiva del PRD Actual

El documento presenta una base sólida con enfoque en los pilares correctos: compatibilidad Spring, gobernanza, persistencia híbrida y operabilidad. Sin embargo, identifico oportunidades significativas para posicionar Vortex Config no solo como un reemplazo de Spring Cloud Config, sino como una plataforma de configuración de siguiente generación que aborde gaps críticos del ecosistema actual.

---

## 1. Features Diferenciales de Alto Impacto

### 1.1 Configuration Inheritance & Composition Engine

**Problema que resuelve:** Spring Cloud Config maneja perfiles de forma lineal. En arquitecturas complejas, la configuración heredada entre múltiples niveles (organización → equipo → servicio → instancia) genera duplicación y deriva.

**Propuesta:**

```yaml
# vortex-hierarchy.yaml
inheritance:
  strategy: cascading  # cascading | override | merge-deep
  levels:
    - organization    # configs/org/{org}/
    - team           # configs/org/{org}/team/{team}/
    - service        # configs/org/{org}/team/{team}/svc/{app}/
    - instance       # configs/org/{org}/team/{team}/svc/{app}/instance/{id}/
  
composition:
  mixins:
    - name: "security-baseline"
      source: "shared/security-hardened.yml"
      priority: 100
    - name: "observability-stack"
      source: "shared/observability.yml"
      priority: 90
```

**Endpoint adicional:**

```
GET /{app}/{profile}/{label}?resolve=full&show-origin=true
```

Respuesta enriquecida con `propertyOrigin` indicando de qué nivel proviene cada propiedad, habilitando debugging y trazabilidad de herencia.

**Trait Rust:**

```rust
#[async_trait::async_trait]
pub trait InheritanceResolver: Send + Sync {
    async fn resolve_hierarchy(
        &self,
        context: &ResolutionContext,
    ) -> anyhow::Result<ResolvedConfigTree>;
    
    fn compute_effective_config(
        &self,
        tree: &ResolvedConfigTree,
        strategy: MergeStrategy,
    ) -> anyhow::Result<ConfigMap>;
}

pub struct ResolutionContext {
    pub organization: Option<String>,
    pub team: Option<String>,
    pub application: String,
    pub profile: String,
    pub label: String,
    pub instance_id: Option<String>,
}
```

---

### 1.2 Feature Flags Nativos con Targeting

**Problema que resuelve:** Equipos usan LaunchDarkly, Unleash o flags ad-hoc separados de la configuración. Vortex puede unificar configuración y feature flags bajo el mismo paradigma de gobernanza.

**Modelo:**

```yaml
# features/{app}/flags.yml
flags:
  new-checkout-flow:
    type: boolean
    default: false
    variants:
      - value: true
        weight: 20  # 20% rollout
    targeting:
      - match:
          context.user.tier: "premium"
        variant: true
      - match:
          context.region: "us-east-1"
        variant: true
        percentage: 50
    
  rate-limit-threshold:
    type: number
    default: 1000
    variants:
      - match:
          context.environment: "staging"
        value: 100
```

**Endpoints:**

```
GET /flags/{app}/{profile}?context={base64-encoded-json}
GET /flags/{app}/{profile}/{flag-key}?context=...
POST /flags/{app}/{profile}/evaluate  # batch evaluation
```

**Trait:**

```rust
#[async_trait::async_trait]
pub trait FeatureFlagEvaluator: Send + Sync {
    async fn evaluate(
        &self,
        app: &str,
        profile: &str,
        flag_key: &str,
        context: &EvaluationContext,
    ) -> anyhow::Result<FlagValue>;
    
    async fn evaluate_batch(
        &self,
        app: &str,
        profile: &str,
        flags: &[String],
        context: &EvaluationContext,
    ) -> anyhow::Result<HashMap<String, FlagValue>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationContext {
    pub user: Option<UserContext>,
    pub session: Option<SessionContext>,
    pub device: Option<DeviceContext>,
    pub custom: HashMap<String, serde_json::Value>,
}
```

---

### 1.3 Configuration Canary & Progressive Rollout

**Problema que resuelve:** Cambios de configuración en producción son "big bang". Un valor incorrecto afecta al 100% del tráfico instantáneamente.

**Propuesta:**

```yaml
# vortex-rollout.yaml
rollouts:
  payment-service:
    strategy: canary
    stages:
      - name: "canary"
        percentage: 5
        duration: "15m"
        success_criteria:
          error_rate: "< 0.1%"
          latency_p99: "< 200ms"
      - name: "early-adopters"
        percentage: 25
        duration: "30m"
      - name: "general-availability"
        percentage: 100
    
    rollback:
      automatic: true
      triggers:
        - metric: "error_rate"
          threshold: "> 1%"
          window: "5m"
```

**Endpoints:**

```
POST /rollout/{app}/{profile}/start
GET /rollout/{app}/{profile}/status
POST /rollout/{app}/{profile}/promote
POST /rollout/{app}/{profile}/rollback
```

**Integración con el resolver:**

```rust
pub struct RolloutAwareResolver {
    inner: Arc<dyn ConfigSource>,
    rollout_manager: Arc<RolloutManager>,
}

impl RolloutAwareResolver {
    pub async fn fetch_for_instance(
        &self,
        app: &str,
        profile: &str,
        label: &str,
        instance_hash: &str,  // Hash del instance-id para distribución determinista
    ) -> anyhow::Result<ConfigMap> {
        let rollout = self.rollout_manager.get_active_rollout(app, profile).await?;
        
        match rollout {
            Some(r) => {
                let stage = r.compute_stage_for_instance(instance_hash);
                self.inner.fetch(app, profile, &stage.config_label).await
            }
            None => self.inner.fetch(app, profile, label).await,
        }
    }
}
```

---

### 1.4 Configuration Drift Detection & Remediation

**Problema que resuelve:** En entornos con múltiples réplicas o despliegues, es posible que algunas instancias operen con configuración desactualizada sin detección.

**Propuesta:**

```yaml
# vortex-drift.yaml
drift_detection:
  enabled: true
  check_interval: "30s"
  
  sources:
    - type: "instance-heartbeat"  # Instancias reportan su config hash
    - type: "prometheus-query"     # Query a métricas de instancias
      query: 'vortex_config_version{app="$app"}'
  
  remediation:
    strategy: "alert-and-refresh"  # alert-only | force-refresh | alert-and-refresh
    refresh_endpoint: "/actuator/refresh"  # Para clientes Spring
    
  alerts:
    - channel: "slack"
      webhook: "${SLACK_DRIFT_WEBHOOK}"
    - channel: "pagerduty"
      severity: "warning"
```

**Endpoints:**

```
GET /drift/{app}/{profile}/status
GET /drift/{app}/{profile}/instances
POST /drift/{app}/{profile}/remediate
```

**Heartbeat para clientes (SDK ligero):**

```rust
// Cliente Rust embebible en aplicaciones
pub struct VortexHeartbeat {
    config_hash: AtomicU64,
    server_url: String,
    app: String,
    profile: String,
    instance_id: String,
}

impl VortexHeartbeat {
    pub async fn report(&self) -> anyhow::Result<HeartbeatResponse> {
        let response = self.client
            .post(format!("{}/heartbeat/{}/{}", self.server_url, self.app, self.profile))
            .json(&HeartbeatPayload {
                instance_id: &self.instance_id,
                config_hash: self.config_hash.load(Ordering::SeqCst),
                timestamp: Utc::now(),
            })
            .send()
            .await?;
        
        // Respuesta puede incluir instrucción de refresh
        response.json().await
    }
}
```

---

### 1.5 Secrets Rotation Orchestration

**Problema que resuelve:** Vortex no reemplaza Vault, pero puede orquestar la rotación de secretos coordinando con el ciclo de vida de configuración.

**Propuesta:**

```yaml
# vortex-secrets.yaml
secrets_lifecycle:
  providers:
    vault:
      address: "${VAULT_ADDR}"
      auth: "kubernetes"
      role: "vortex-config"
    
    aws_secrets_manager:
      region: "us-east-1"
  
  rotations:
    - secret_path: "database/creds/payment-db"
      provider: "vault"
      schedule: "0 0 * * 0"  # Weekly
      pre_rotation_hook:
        notify:
          - channel: "slack"
            message: "Initiating DB credential rotation for payment-service"
      post_rotation_hook:
        trigger_refresh:
          apps: ["payment-service", "billing-service"]
        verify:
          endpoint: "/health/db"
          timeout: "30s"
          retries: 3
```

**Endpoints:**

```
GET /secrets/rotation/schedule
POST /secrets/rotation/{rotation-id}/trigger
GET /secrets/rotation/{rotation-id}/status
```

**Trait:**

```rust
#[async_trait::async_trait]
pub trait SecretsProvider: Send + Sync {
    async fn get_secret(&self, path: &str) -> anyhow::Result<SecretValue>;
    async fn rotate(&self, path: &str) -> anyhow::Result<RotationResult>;
    async fn get_rotation_status(&self, path: &str) -> anyhow::Result<RotationStatus>;
}

pub struct RotationOrchestrator {
    providers: HashMap<String, Arc<dyn SecretsProvider>>,
    scheduler: Arc<Scheduler>,
    notifier: Arc<dyn Notifier>,
}
```

---

### 1.6 Configuration Compliance Engine

**Problema que resuelve:** Regulaciones (PCI-DSS, SOC2, GDPR) requieren que ciertos valores de configuración cumplan estándares específicos. Hoy esto se verifica manualmente o con scripts externos.

**Propuesta:**

```yaml
# compliance-rules/{standard}/rules.yml
compliance:
  standard: "PCI-DSS-4.0"
  version: "1.0.0"
  
  rules:
    - id: "PCI-2.2.1"
      name: "Strong Cryptography"
      description: "Encryption must use approved algorithms"
      severity: "critical"
      match:
        pattern: "*.encryption.algorithm"
      check:
        type: "enum"
        allowed: ["AES-256-GCM", "ChaCha20-Poly1305"]
      
    - id: "PCI-8.2.3"
      name: "Password Complexity"
      description: "Minimum password requirements"
      match:
        pattern: "*.security.password.min-length"
      check:
        type: "range"
        min: 12
      
    - id: "PCI-10.1"
      name: "Audit Logging"
      description: "Audit logging must be enabled"
      match:
        pattern: "*.audit.enabled"
      check:
        type: "equals"
        value: true
```

**Endpoints:**

```
GET /compliance/{app}/{profile}/report
GET /compliance/{app}/{profile}/violations
POST /compliance/validate  # Validación pre-commit (CI/CD hook)
```

**Respuesta de compliance:**

```json
{
  "application": "payment-service",
  "profile": "production",
  "compliance_status": "non_compliant",
  "standards_checked": ["PCI-DSS-4.0", "SOC2-Type2"],
  "violations": [
    {
      "rule_id": "PCI-2.2.1",
      "property": "database.encryption.algorithm",
      "current_value": "AES-128-CBC",
      "expected": "AES-256-GCM or ChaCha20-Poly1305",
      "severity": "critical",
      "remediation": "Update encryption algorithm to AES-256-GCM"
    }
  ],
  "passed_checks": 47,
  "failed_checks": 1,
  "report_generated_at": "2025-01-11T15:30:00Z"
}
```

---

### 1.7 Configuration Dependencies & Impact Analysis

**Problema que resuelve:** Cambiar una propiedad compartida puede afectar múltiples servicios de forma no evidente. No existe visibilidad del grafo de dependencias.

**Propuesta:**

```yaml
# vortex-dependencies.yaml
dependency_tracking:
  enabled: true
  
  shared_configs:
    - path: "shared/database-pools.yml"
      consumers:
        - "payment-service"
        - "inventory-service"
        - "order-service"
    
    - path: "shared/kafka-clusters.yml"
      consumers:
        - "event-processor"
        - "notification-service"
```

**Endpoints:**

```
GET /impact/{app}/{profile}?property={key}
GET /dependencies/graph?format=mermaid|json|dot
GET /dependencies/{app}/upstream
GET /dependencies/{app}/downstream
```

**Respuesta de impact analysis:**

```json
{
  "property": "shared.kafka.bootstrap-servers",
  "direct_consumers": [
    {
      "app": "event-processor",
      "profiles": ["production", "staging"],
      "usage_count": 3
    }
  ],
  "transitive_impact": [
    {
      "app": "notification-service",
      "reason": "Depends on event-processor output topic",
      "impact_level": "indirect"
    }
  ],
  "risk_score": "high",
  "recommended_actions": [
    "Coordinate change window with event-processor team",
    "Verify Kafka connectivity in staging before production"
  ]
}
```

---

### 1.8 Environment Promotion Workflows

**Problema que resuelve:** Promover configuración de dev → staging → production es manual, propenso a errores, y sin trazabilidad de aprobaciones.

**Propuesta:**

```yaml
# vortex-promotions.yaml
promotion_pipelines:
  default:
    stages:
      - name: "development"
        label: "develop"
        auto_promote: true
        
      - name: "staging"
        label: "staging"
        gates:
          - type: "approval"
            approvers: ["team-lead", "qa-lead"]
            min_approvals: 1
          - type: "test"
            suite: "integration-tests"
            
      - name: "production"
        label: "main"
        gates:
          - type: "approval"
            approvers: ["platform-team", "security-team"]
            min_approvals: 2
          - type: "compliance"
            standards: ["PCI-DSS-4.0"]
          - type: "diff-review"
            require_explicit_ack: true
```

**Endpoints:**

```
POST /promotion/{app}/initiate
GET /promotion/{app}/pending
POST /promotion/{app}/{promotion-id}/approve
POST /promotion/{app}/{promotion-id}/reject
GET /promotion/{app}/history
```

---

### 1.9 Configuration Templating con Contexto Dinámico

**Problema que resuelve:** Spring Cloud Config soporta placeholders básicos. En escenarios complejos, se necesita lógica condicional y funciones de transformación.

**Propuesta (sintaxis inspirada en Tera/Jinja2):**

```yaml
# Archivo de configuración con templating
database:
  host: "{{ env.DB_HOST | default('localhost') }}"
  port: "{{ env.DB_PORT | default(5432) | int }}"
  pool:
    max_size: "{{ if profile == 'production' }}100{{ else }}10{{ endif }}"
    min_idle: "{{ (pool.max_size * 0.1) | round }}"
  
  connection_string: >-
    postgresql://{{ secrets.db_user }}:{{ secrets.db_password | urlencode }}
    @{{ database.host }}:{{ database.port }}/{{ app }}_{{ profile }}

rate_limiting:
  requests_per_second: "{{ tier_config[context.user.tier].rps | default(100) }}"
  
observability:
  service_name: "{{ app }}-{{ profile }}-{{ env.REGION | default('unknown') }}"
```

**Funciones built-in:**

- `env(VAR)` - Variables de entorno
- `secrets(path)` - Integración con secrets providers
- `base64_encode/decode`
- `hash(algorithm)`
- `urlencode`
- `json_encode/decode`
- `yaml_encode`
- `default(value)`
- `upper/lower/capitalize`
- `split/join`
- Operadores aritméticos y lógicos

**Endpoint con contexto:**

```
GET /{app}/{profile}/{label}?context={base64-json}
```

---

### 1.10 Configuration Analytics & Insights

**Problema que resuelve:** Sin visibilidad de patrones de uso, es difícil optimizar, identificar configuración obsoleta, o entender el comportamiento real del sistema.

**Propuesta:**

**Métricas adicionales (Prometheus):**

```
# Frecuencia de acceso por propiedad
vortex_property_access_total{app, profile, property, client_id}

# Propiedades nunca accedidas (posible dead config)
vortex_property_last_access_timestamp{app, profile, property}

# Cambios de configuración
vortex_config_changes_total{app, profile, change_type}

# Tiempo de propagación de cambios
vortex_config_propagation_duration_seconds{app, profile}

# Feature flag evaluations
vortex_flag_evaluation_total{app, flag, variant, context_segment}
```

**Endpoints de analytics:**

```
GET /analytics/{app}/usage?window=7d
GET /analytics/{app}/unused-properties?threshold=30d
GET /analytics/{app}/change-frequency
GET /analytics/flags/{app}/effectiveness
```

**Respuesta de usage analytics:**

```json
{
  "application": "payment-service",
  "analysis_window": "7d",
  "total_config_fetches": 145230,
  "unique_clients": 12,
  "property_usage": [
    {
      "property": "database.connection-string",
      "access_count": 145230,
      "access_pattern": "every-request"
    },
    {
      "property": "feature.legacy-checkout",
      "access_count": 0,
      "last_accessed": "2024-11-15T10:00:00Z",
      "recommendation": "Consider removal - unused for 57 days"
    }
  ],
  "insights": [
    {
      "type": "optimization",
      "message": "Property 'cache.ttl' accessed frequently - consider client-side caching",
      "impact": "Reduce server load by ~40%"
    }
  ]
}
```

---

## 2. Mejoras Arquitectónicas

### 2.1 Event Sourcing para Audit Trail

En lugar de tablas de auditoría tradicionales, implementar event sourcing para máxima trazabilidad:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigEvent {
    ConfigFetched {
        app: String,
        profile: String,
        label: String,
        client_id: String,
        version: String,
        properties_accessed: Vec<String>,
        plac_decisions: Vec<PlacDecision>,
        timestamp: DateTime<Utc>,
    },
    ConfigChanged {
        app: String,
        profile: String,
        from_version: String,
        to_version: String,
        diff: ConfigDiff,
        author: String,
        commit_message: Option<String>,
        timestamp: DateTime<Utc>,
    },
    PolicyEvaluated {
        app: String,
        profile: String,
        client_id: String,
        policy_version: String,
        decisions: Vec<PlacDecision>,
        timestamp: DateTime<Utc>,
    },
    SchemaValidationFailed {
        app: String,
        profile: String,
        version: String,
        violations: Vec<SchemaViolation>,
        action_taken: GovernanceAction,
        timestamp: DateTime<Utc>,
    },
}

#[async_trait::async_trait]
pub trait EventStore: Send + Sync {
    async fn append(&self, event: ConfigEvent) -> anyhow::Result<EventId>;
    async fn read_stream(&self, stream_id: &str, from: Option<EventId>) -> anyhow::Result<Vec<ConfigEvent>>;
    async fn read_all(&self, from: Option<EventId>, limit: usize) -> anyhow::Result<Vec<ConfigEvent>>;
}
```

### 2.2 Sidecar Mode para Service Mesh Integration

Para entornos con Istio/Linkerd, ofrecer modo sidecar que inyecte configuración como archivos o variables de entorno:

```yaml
# vortex-sidecar.yaml
sidecar:
  mode: "file-sync"  # file-sync | env-injection | grpc-stream
  
  file_sync:
    target_path: "/etc/config"
    format: "yaml"  # yaml | json | properties | env
    watch_interval: "10s"
    
  env_injection:
    prefix: "VORTEX_"
    flatten: true
    
  grpc_stream:
    port: 50051
    reflection: true
```

### 2.3 Multi-Cluster Federation

Para organizaciones con múltiples clusters Kubernetes:

```rust
#[async_trait::async_trait]
pub trait ClusterFederation: Send + Sync {
    async fn sync_config(&self, source_cluster: &str, target_clusters: &[String]) -> anyhow::Result<SyncResult>;
    async fn get_cluster_status(&self) -> anyhow::Result<Vec<ClusterStatus>>;
    async fn resolve_conflicts(&self, conflict: ConfigConflict, strategy: ConflictResolution) -> anyhow::Result<()>;
}

pub enum ConflictResolution {
    SourceWins,
    TargetWins,
    MergeWithPrecedence(Vec<String>),  // Lista ordenada de clusters por precedencia
    Manual,
}
```

---

## 3. Consideraciones de Seguridad Avanzadas

### 3.1 Configuration Signing & Verification

Garantizar integridad criptográfica de la configuración:

```rust
pub struct SignedConfig {
    pub config: ConfigMap,
    pub signature: Vec<u8>,
    pub signing_key_id: String,
    pub algorithm: SigningAlgorithm,
    pub timestamp: DateTime<Utc>,
}

#[async_trait::async_trait]
pub trait ConfigSigner: Send + Sync {
    async fn sign(&self, config: &ConfigMap) -> anyhow::Result<SignedConfig>;
    async fn verify(&self, signed: &SignedConfig) -> anyhow::Result<VerificationResult>;
}

pub enum SigningAlgorithm {
    Ed25519,
    RSA_PSS_SHA256,
    ECDSA_P256_SHA256,
}
```

**Header de respuesta:**

```
X-Vortex-Signature: <base64-signature>
X-Vortex-Signing-Key-Id: <key-id>
X-Vortex-Signature-Algorithm: Ed25519
```

### 3.2 Zero-Trust Policy Enforcement

Integración con SPIFFE/SPIRE para identidad workload-aware:

```rust
pub struct SpiffeIdentity {
    pub spiffe_id: String,  // spiffe://trust-domain/path
    pub trust_domain: String,
    pub path: Vec<String>,
}

impl PlacContext {
    pub fn from_spiffe(identity: &SpiffeIdentity) -> Self {
        PlacContext {
            principal: identity.spiffe_id.clone(),
            attributes: HashMap::from([
                ("spiffe.trust_domain".into(), identity.trust_domain.clone()),
                ("spiffe.path".into(), identity.path.join("/")),
            ]),
        }
    }
}
```

### 3.3 Anomaly Detection

Detectar patrones de acceso anómalos que podrían indicar exfiltración:

```rust
pub struct AnomalyDetector {
    baseline: AccessPatternBaseline,
    threshold_config: AnomalyThresholds,
}

impl AnomalyDetector {
    pub fn evaluate(&self, access: &ConfigAccess) -> Option<AnomalyAlert> {
        // Detectar:
        // - Acceso a propiedades inusuales para el cliente
        // - Volumen anormalmente alto de requests
        // - Acceso fuera de horario habitual
        // - Patrones de scanning (acceso secuencial a muchas propiedades)
    }
}
```

---

## 4. Especificación Técnica Ampliada

### 4.1 Crates Adicionales Recomendados

```toml
[dependencies]
# Core (ya en PRD)
axum = "0.7"
tokio = { version = "1", features = ["full"] }
gix = "0.58"
sqlx = { version = "0.7", features = ["runtime-tokio", "postgres", "mysql", "sqlite"] }
moka = { version = "0.12", features = ["future"] }
serde = { version = "1", features = ["derive"] }

# Templating
tera = "1.19"  # Motor de templates

# Feature Flags
murmur3 = "0.5"  # Hash para consistent hashing de rollouts

# Compliance
jsonschema = "0.18"
semver = "1"

# Crypto/Signing
ed25519-dalek = "2"
ring = "0.17"

# Analytics
hdrhistogram = "7"  # Histogramas de latencia

# Federation/Clustering
raft = "0.7"  # Si se implementa consenso distribuido
etcd-client = "0.12"  # Alternativa a raft propio

# Observability adicional
metrics = "0.22"
metrics-exporter-prometheus = "0.13"

# gRPC (para sidecar mode)
tonic = "0.11"
prost = "0.12"

# Scheduling
cron = "0.12"  # Para rotación programada de secretos
```

### 4.2 Estructura de Proyecto Sugerida

```
vortex-config/
├── crates/
│   ├── vortex-core/           # Tipos compartidos, traits, ConfigMap
│   ├── vortex-server/         # Axum API, routing, middleware
│   ├── vortex-sources/        # Implementaciones de ConfigSource
│   │   ├── git/
│   │   ├── s3/
│   │   └── sql/
│   ├── vortex-governance/     # PLAC, Schema, Compliance
│   ├── vortex-features/       # Feature flags engine
│   ├── vortex-rollout/        # Canary/progressive deployment
│   ├── vortex-audit/          # Event sourcing, analytics
│   ├── vortex-secrets/        # Integración Vault/AWS SM/rotation
│   ├── vortex-templating/     # Tera integration, funciones custom
│   ├── vortex-federation/     # Multi-cluster sync
│   └── vortex-client/         # SDK para aplicaciones Rust
├── proto/                     # gRPC definitions
├── charts/                    # Helm charts
├── docker/
│   ├── Dockerfile.minimal     # Git-only
│   ├── Dockerfile.s3          # Con Litestream
│   └── Dockerfile.full        # Enterprise
└── tests/
    ├── integration/
    ├── compatibility/         # Tests contra Spring Boot client
    └── load/
```

---

## 5. Roadmap Revisado

### Fase 1 — MVP Drop-in (Semana 1-4)

*Sin cambios respecto al PRD original*

### Fase 2 — Persistencia & Cloud (Semana 5-8)

*Sin cambios respecto al PRD original*

### Fase 3 — Governance Core (Semana 9-12)

- PLAC completo
- Schema validation
- Diff semántico
- WebSockets push
- **Nuevo:** Configuration signing

### Fase 4 — Advanced Features (Semana 13-18)

- Configuration inheritance & composition
- Feature flags nativos
- Configuration templating
- Compliance engine (PCI-DSS, SOC2)
- Environment promotion workflows

### Fase 5 — Enterprise & Operations (Semana 19-24)

- Canary/progressive rollout
- Drift detection & remediation
- Secrets rotation orchestration
- Configuration analytics
- Multi-cluster federation
- Sidecar mode

---

## 6. KPIs Adicionales

| Métrica | Objetivo |
|---------|----------|
| Time-to-detection de drift | < 60s |
| Feature flag evaluation p99 | < 5ms |
| Configuration propagation time | < 30s (99th percentile) |
| Compliance scan time | < 10s por aplicación |
| Rollout decision latency | < 1ms |

---

## 7. Ventajas Competitivas vs Spring Cloud Config

| Capacidad | Spring Cloud Config | Vortex Config |
|-----------|---------------------|---------------|
| Footprint | ~200MB heap mínimo | < 30MB total |
| Cold start | 5-15s | < 500ms |
| Property-level ACL | No | PLAC nativo |
| Feature flags | Requiere integración externa | Nativo |
| Canary config | No | Nativo |
| Drift detection | No | Nativo |
| Compliance engine | No | Nativo |
| Configuration signing | No | Nativo |
| Templating avanzado | Placeholders básicos | Tera completo |
| Analytics | Métricas básicas | Analytics completos |
