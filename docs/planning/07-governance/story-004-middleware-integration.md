# Historia 004: Integracion con Middleware

## Contexto y Objetivo

Con el modelo PLAC (Historia 001), el parser (Historia 002) y el motor de evaluacion (Historia 003) implementados, esta historia integra el sistema de gobernanza en el pipeline HTTP de Axum como middleware.

**El middleware de gobernanza:**
- Intercepta cada request antes de llegar al handler
- Construye el RequestContext desde los datos HTTP
- Evalua politicas usando el PolicyEngine
- Deniega requests que no cumplen politicas
- Marca requests permitidos para transformacion posterior
- Transforma respuestas segun las acciones acumuladas

Esta historia demuestra el uso avanzado de Axum middleware, layer composition, y state extraction.

---

## Alcance

### In Scope

- GovernanceLayer como Tower middleware
- Extraccion de RequestContext desde Axum Request
- Extension de Request con PolicyDecision
- Transformacion de Response segun acciones
- Integracion con el stack de middleware existente
- Headers de respuesta para warnings
- Logging de auditoria

### Out of Scope

- Implementacion detallada de acciones (Historia 006)
- Cache de decisiones
- Metricas Prometheus
- Panel de administracion

---

## Criterios de Aceptacion

- [ ] GovernanceLayer se compone con otros middleware de Tower
- [ ] RequestContext se extrae correctamente de Request HTTP
- [ ] Requests denegados retornan 403 con mensaje
- [ ] PolicyDecision se almacena en request extensions
- [ ] Warnings se agregan como headers `X-Governance-Warning`
- [ ] Respuestas se transforman segun acciones (post-processing)
- [ ] Logs de auditoria incluyen decision, policy, request_id
- [ ] Tests de integracion verifican el pipeline completo

---

## Diseno Propuesto

### Arquitectura del Middleware

```
┌─────────────────────────────────────────────────────────────────────┐
│                         HTTP Request                                 │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     RequestIdLayer                                   │
│                (Genera/propaga X-Request-Id)                        │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     LoggingLayer                                     │
│                   (Logging estructurado)                            │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                   GovernanceLayer                                    │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  PRE-PROCESSING                                              │   │
│  │                                                              │   │
│  │  1. Extract RequestContext from HTTP request                │   │
│  │     - Path params: app, profile, label                      │   │
│  │     - Headers: all relevant headers                         │   │
│  │     - Source IP: from connection info                       │   │
│  │                                                              │   │
│  │  2. Evaluate policies via PolicyEngine                      │   │
│  │                                                              │   │
│  │  3. If DENY:                                                │   │
│  │     - Log audit event                                       │   │
│  │     - Return 403 Forbidden immediately                      │   │
│  │                                                              │   │
│  │  4. If ALLOW:                                               │   │
│  │     - Store PolicyDecision in request extensions           │   │
│  │     - Continue to next layer                                │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                │                                     │
│                                ▼                                     │
│                        [Inner Service]                              │
│                                │                                     │
│                                ▼                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  POST-PROCESSING                                             │   │
│  │                                                              │   │
│  │  1. Get PolicyDecision from extensions                      │   │
│  │                                                              │   │
│  │  2. Apply response transformations:                         │   │
│  │     - Mask sensitive values                                 │   │
│  │     - Redact properties                                     │   │
│  │     - Add warning headers                                   │   │
│  │                                                              │   │
│  │  3. Return transformed response                             │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                          HTTP Response                               │
└─────────────────────────────────────────────────────────────────────┘
```

### Estructura de Archivos

```
crates/vortex-governance/src/
├── middleware/
│   ├── mod.rs              # Re-exports
│   ├── layer.rs            # GovernanceLayer
│   ├── service.rs          # GovernanceService
│   ├── extractor.rs        # RequestContext extractor
│   └── transformer.rs      # Response transformer
└── plac/
    └── ...                 # Historias anteriores
```

---

## Pasos de Implementacion

### Paso 1: Crear Extractor de RequestContext

```rust
// src/middleware/extractor.rs
use axum::{
    async_trait,
    extract::{ConnectInfo, FromRequestParts, Path},
    http::{request::Parts, HeaderMap},
};
use std::net::SocketAddr;

use crate::plac::RequestContext;

/// Params extracted from the URL path.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfigParams {
    pub app: String,
    pub profile: String,
    pub label: Option<String>,
}

/// Extractor that builds RequestContext from HTTP request parts.
pub struct GovernanceContext(pub RequestContext);

#[async_trait]
impl<S> FromRequestParts<S> for GovernanceContext
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let context = extract_context(parts);
        Ok(GovernanceContext(context))
    }
}

/// Extract RequestContext from request parts.
fn extract_context(parts: &Parts) -> RequestContext {
    // Extract path parameters
    let (app, profiles, label) = extract_path_params(parts);

    // Build context
    let mut context = RequestContext::new(&app, profiles);
    context.label = label;

    // Extract source IP
    context.source_ip = extract_source_ip(parts);

    // Extract headers
    context.headers = extract_headers(&parts.headers);

    context
}

/// Extract app, profile, and label from path.
fn extract_path_params(parts: &Parts) -> (String, Vec<String>, Option<String>) {
    let path = parts.uri.path();
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    // Expected formats:
    // /{app}/{profile}
    // /{app}/{profile}/{label}
    let app = segments.get(0).unwrap_or(&"unknown").to_string();
    let profile_str = segments.get(1).unwrap_or(&"default").to_string();
    let label = segments.get(2).map(|s| s.to_string());

    // Parse comma-separated profiles
    let profiles: Vec<String> = profile_str
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    (app, profiles, label)
}

/// Extract source IP from connection info.
fn extract_source_ip(parts: &Parts) -> Option<std::net::IpAddr> {
    // Try X-Forwarded-For first (for proxied requests)
    if let Some(forwarded) = parts.headers.get("x-forwarded-for") {
        if let Ok(value) = forwarded.to_str() {
            // Take first IP in chain
            if let Some(ip_str) = value.split(',').next() {
                if let Ok(ip) = ip_str.trim().parse() {
                    return Some(ip);
                }
            }
        }
    }

    // Try X-Real-IP
    if let Some(real_ip) = parts.headers.get("x-real-ip") {
        if let Ok(value) = real_ip.to_str() {
            if let Ok(ip) = value.parse() {
                return Some(ip);
            }
        }
    }

    // Connection info would be extracted from extensions
    // This requires ConnectInfo<SocketAddr> to be added by Axum
    parts.extensions.get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip())
}

/// Extract relevant headers.
fn extract_headers(headers: &HeaderMap) -> std::collections::HashMap<String, String> {
    let mut result = std::collections::HashMap::new();

    // Headers to extract for policy evaluation
    let relevant_headers = [
        "authorization",
        "x-api-key",
        "x-client-id",
        "x-tenant-id",
        "user-agent",
        "x-request-id",
    ];

    for name in &relevant_headers {
        if let Some(value) = headers.get(*name) {
            if let Ok(v) = value.to_str() {
                result.insert(name.to_string(), v.to_string());
            }
        }
    }

    result
}
```

### Paso 2: Implementar GovernanceLayer

```rust
// src/middleware/layer.rs
use std::sync::Arc;
use tower::Layer;

use crate::plac::CompiledPolicySet;
use super::service::GovernanceService;

/// Tower Layer that adds governance enforcement to services.
///
/// This layer wraps services to enforce PLAC policies on requests.
#[derive(Clone)]
pub struct GovernanceLayer {
    policies: Arc<CompiledPolicySet>,
    config: GovernanceConfig,
}

/// Configuration for the governance layer.
#[derive(Clone, Debug)]
pub struct GovernanceConfig {
    /// Whether to fail open (allow) or closed (deny) on errors
    pub fail_open: bool,

    /// Header name for governance warnings
    pub warning_header: String,

    /// Enable audit logging
    pub audit_logging: bool,
}

impl Default for GovernanceConfig {
    fn default() -> Self {
        Self {
            fail_open: false,  // Fail closed by default (secure)
            warning_header: "X-Governance-Warning".to_string(),
            audit_logging: true,
        }
    }
}

impl GovernanceLayer {
    /// Create a new GovernanceLayer with the given policies.
    pub fn new(policies: Arc<CompiledPolicySet>) -> Self {
        Self {
            policies,
            config: GovernanceConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(policies: Arc<CompiledPolicySet>, config: GovernanceConfig) -> Self {
        Self { policies, config }
    }

    /// Set fail-open behavior.
    pub fn fail_open(mut self, fail_open: bool) -> Self {
        self.config.fail_open = fail_open;
        self
    }
}

impl<S> Layer<S> for GovernanceLayer {
    type Service = GovernanceService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GovernanceService::new(
            inner,
            Arc::clone(&self.policies),
            self.config.clone(),
        )
    }
}
```

### Paso 3: Implementar GovernanceService

```rust
// src/middleware/service.rs
use std::sync::Arc;
use std::task::{Context, Poll};
use std::pin::Pin;
use std::future::Future;

use axum::{
    body::Body,
    http::{Request, Response, StatusCode, HeaderValue},
    response::IntoResponse,
};
use tower::Service;
use tracing::{info, warn, error, instrument, Span};

use crate::plac::{CompiledPolicySet, PolicyEngine, PolicyDecision, RequestContext};
use super::layer::GovernanceConfig;
use super::extractor::extract_context;
use super::transformer::ResponseTransformer;

/// Service that enforces governance policies.
#[derive(Clone)]
pub struct GovernanceService<S> {
    inner: S,
    policies: Arc<CompiledPolicySet>,
    config: GovernanceConfig,
}

impl<S> GovernanceService<S> {
    pub fn new(
        inner: S,
        policies: Arc<CompiledPolicySet>,
        config: GovernanceConfig,
    ) -> Self {
        Self {
            inner,
            policies,
            config,
        }
    }
}

impl<S> Service<Request<Body>> for GovernanceService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let policies = Arc::clone(&self.policies);
        let config = self.config.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Extract request context
            let (mut parts, body) = request.into_parts();
            let context = extract_context(&parts);

            // Get request ID for logging
            let request_id = parts.headers
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("unknown")
                .to_string();

            // Evaluate policies
            let engine = PolicyEngine::new(&policies);
            let decision = engine.evaluate(&context);

            // Handle decision
            match &decision {
                PolicyDecision::Deny { policy_name, message } => {
                    // Audit log for denial
                    if config.audit_logging {
                        warn!(
                            request_id = %request_id,
                            app = %context.application,
                            profile = ?context.primary_profile(),
                            source_ip = ?context.source_ip,
                            policy = %policy_name,
                            "Access denied by governance policy"
                        );
                    }

                    // Return 403 Forbidden
                    let response = (
                        StatusCode::FORBIDDEN,
                        [(
                            "Content-Type",
                            "application/json",
                        )],
                        format!(r#"{{"error": "Forbidden", "message": "{}"}}"#, message),
                    ).into_response();

                    return Ok(response);
                }

                PolicyDecision::Allow { actions } => {
                    // Log if there are actions to apply
                    if !actions.is_empty() && config.audit_logging {
                        info!(
                            request_id = %request_id,
                            app = %context.application,
                            action_count = actions.len(),
                            "Request allowed with governance actions"
                        );
                    }

                    // Store decision in request extensions for post-processing
                    parts.extensions.insert(GovernanceDecision {
                        decision: decision.clone(),
                        context: context.clone(),
                    });
                }
            }

            // Reconstruct request and call inner service
            let request = Request::from_parts(parts, body);
            let response = inner.call(request).await?;

            // Post-process response if needed
            let response = if let PolicyDecision::Allow { actions } = &decision {
                if actions.is_empty() {
                    response
                } else {
                    transform_response(response, actions, &config).await
                }
            } else {
                response
            };

            Ok(response)
        })
    }
}

/// Governance decision stored in request extensions.
#[derive(Clone)]
pub struct GovernanceDecision {
    pub decision: PolicyDecision,
    pub context: RequestContext,
}

/// Transform response based on accumulated actions.
async fn transform_response(
    response: Response<Body>,
    actions: &[crate::plac::AppliedAction],
    config: &GovernanceConfig,
) -> Response<Body> {
    let transformer = ResponseTransformer::new(actions);
    transformer.transform(response, config).await
}
```

### Paso 4: Implementar Response Transformer

```rust
// src/middleware/transformer.rs
use axum::{
    body::Body,
    http::{Response, HeaderValue},
};
use serde_json::Value;
use tracing::debug;

use crate::plac::{AppliedAction, ActionType};
use super::layer::GovernanceConfig;

/// Transforms HTTP responses based on governance actions.
pub struct ResponseTransformer<'a> {
    actions: &'a [AppliedAction],
}

impl<'a> ResponseTransformer<'a> {
    pub fn new(actions: &'a [AppliedAction]) -> Self {
        Self { actions }
    }

    /// Transform the response based on actions.
    pub async fn transform(
        &self,
        response: Response<Body>,
        config: &GovernanceConfig,
    ) -> Response<Body> {
        let (mut parts, body) = response.into_parts();

        // Collect warning messages
        let warnings: Vec<_> = self.actions
            .iter()
            .filter(|a| matches!(a.action.action_type, ActionType::Warn))
            .filter_map(|a| a.action.message.as_ref())
            .collect();

        // Add warning headers
        for warning in warnings {
            if let Ok(value) = HeaderValue::from_str(warning) {
                parts.headers.append(&config.warning_header, value);
            }
        }

        // Check if we need to transform the body
        let needs_body_transform = self.actions
            .iter()
            .any(|a| matches!(
                a.action.action_type,
                ActionType::Mask | ActionType::Redact
            ));

        if !needs_body_transform {
            return Response::from_parts(parts, body);
        }

        // Read body and transform
        let bytes = match axum::body::to_bytes(body, usize::MAX).await {
            Ok(b) => b,
            Err(_) => return Response::from_parts(parts, Body::empty()),
        };

        // Try to parse as JSON
        let transformed = match serde_json::from_slice::<Value>(&bytes) {
            Ok(json) => {
                let transformed_json = self.transform_json(json);
                serde_json::to_vec(&transformed_json).unwrap_or_else(|_| bytes.to_vec())
            }
            Err(_) => {
                // Not JSON, return as-is
                bytes.to_vec()
            }
        };

        Response::from_parts(parts, Body::from(transformed))
    }

    /// Transform JSON value based on actions.
    fn transform_json(&self, mut json: Value) -> Value {
        for action in self.actions {
            match &action.action.action_type {
                ActionType::Mask => {
                    self.apply_mask(&mut json, &action.action);
                }
                ActionType::Redact => {
                    self.apply_redact(&mut json, &action.action);
                }
                _ => {}
            }
        }
        json
    }

    /// Apply masking to JSON values.
    fn apply_mask(&self, json: &mut Value, action: &crate::plac::Action) {
        let mask_char = action.mask_char.unwrap_or('*');
        let visible_chars = action.visible_chars.unwrap_or(0);

        // This is a simplified implementation
        // Full implementation would use property_path patterns
        self.mask_recursive(json, mask_char, visible_chars);
    }

    fn mask_recursive(&self, json: &mut Value, mask_char: char, visible: usize) {
        match json {
            Value::Object(map) => {
                for (key, value) in map.iter_mut() {
                    // Mask sensitive-looking fields
                    if is_sensitive_field(key) {
                        if let Value::String(s) = value {
                            *s = mask_string(s, mask_char, visible);
                        }
                    } else {
                        self.mask_recursive(value, mask_char, visible);
                    }
                }
            }
            Value::Array(arr) => {
                for item in arr.iter_mut() {
                    self.mask_recursive(item, mask_char, visible);
                }
            }
            _ => {}
        }
    }

    /// Apply redaction to JSON values.
    fn apply_redact(&self, json: &mut Value, action: &crate::plac::Action) {
        if let Some(properties) = &action.properties {
            for prop in properties {
                self.redact_property(json, prop);
            }
        }
    }

    fn redact_property(&self, json: &mut Value, property: &str) {
        if let Value::Object(map) = json {
            // Simple implementation: remove top-level key
            // Full implementation would support nested paths like "database.password"
            map.remove(property);

            // Also check nested objects
            for value in map.values_mut() {
                self.redact_property(value, property);
            }
        }
    }
}

/// Check if a field name looks sensitive.
fn is_sensitive_field(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    name_lower.contains("password")
        || name_lower.contains("secret")
        || name_lower.contains("api_key")
        || name_lower.contains("apikey")
        || name_lower.contains("token")
        || name_lower.contains("credential")
}

/// Mask a string value.
fn mask_string(value: &str, mask_char: char, visible_chars: usize) -> String {
    let len = value.len();
    if len <= visible_chars {
        return mask_char.to_string().repeat(len);
    }

    let masked_len = len - visible_chars;
    let masked_part: String = mask_char.to_string().repeat(masked_len);
    let visible_part: String = value.chars().skip(masked_len).collect();

    format!("{}{}", masked_part, visible_part)
}
```

### Paso 5: Integrar en el Router de Axum

```rust
// src/server.rs (ejemplo de integracion)
use axum::{Router, routing::get};
use std::sync::Arc;
use tower::ServiceBuilder;

use crate::middleware::{RequestIdLayer, LoggingLayer};
use crate::governance::middleware::{GovernanceLayer, GovernanceConfig};
use crate::governance::PolicyLoader;

pub async fn create_router() -> Router {
    // Load policies
    let policies = PolicyLoader::new()
        .from_directory("./policies")
        .expect("Failed to load policies");
    let policies = Arc::new(policies);

    // Configure governance
    let governance_config = GovernanceConfig {
        fail_open: false,
        warning_header: "X-Governance-Warning".to_string(),
        audit_logging: true,
    };

    // Build middleware stack
    // Order matters: RequestId -> Logging -> Governance -> Handler
    let middleware = ServiceBuilder::new()
        .layer(RequestIdLayer)
        .layer(LoggingLayer)
        .layer(GovernanceLayer::with_config(policies, governance_config));

    Router::new()
        .route("/health", get(health_check))
        .route("/:app/:profile", get(get_config))
        .route("/:app/:profile/:label", get(get_config_with_label))
        .layer(middleware)
}

async fn health_check() -> &'static str {
    "OK"
}

async fn get_config(
    axum::extract::Path((app, profile)): axum::extract::Path<(String, String)>,
    // Can access governance decision if needed
    governance: Option<axum::Extension<crate::governance::middleware::GovernanceDecision>>,
) -> impl axum::response::IntoResponse {
    // Handler logic...
    // governance.map(|g| g.decision) gives access to the decision
    format!("Config for {}/{}", app, profile)
}
```

---

## Conceptos de Rust Aprendidos

### 1. Axum Middleware con Tower

Axum usa Tower para middleware, permitiendo composicion flexible.

**Rust:**
```rust
use tower::{Layer, Service, ServiceBuilder};
use axum::Router;

// Layer es una fabrica de Services
pub struct MyLayer;

impl<S> Layer<S> for MyLayer {
    type Service = MyService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MyService { inner }
    }
}

// Service procesa requests
pub struct MyService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for MyService<S>
where
    S: Service<Request<Body>, Response = Response<Body>>,
{
    // ... implementation
}

// Composicion de layers
let middleware = ServiceBuilder::new()
    .layer(Layer1)  // Primero
    .layer(Layer2)  // Segundo
    .layer(Layer3); // Tercero

Router::new()
    .route("/", get(handler))
    .layer(middleware)

// Flujo: Request -> Layer1 -> Layer2 -> Layer3 -> Handler -> Layer3 -> Layer2 -> Layer1 -> Response
```

**Java (Spring Filters):**
```java
@Component
@Order(1)
public class FirstFilter implements Filter {
    @Override
    public void doFilter(ServletRequest request, ServletResponse response, FilterChain chain) {
        // Pre-processing
        chain.doFilter(request, response);
        // Post-processing
    }
}

@Configuration
public class FilterConfig {
    @Bean
    public FilterRegistrationBean<FirstFilter> firstFilter() {
        FilterRegistrationBean<FirstFilter> registration = new FilterRegistrationBean<>();
        registration.setFilter(new FirstFilter());
        registration.setOrder(1);
        return registration;
    }
}
```

**Diferencias:**
| Aspecto | Tower (Rust) | Spring Filters |
|---------|--------------|----------------|
| Composicion | `ServiceBuilder::new().layer()` | `@Order` annotation |
| Tipo | Generic, type-safe | Runtime |
| Async | Nativo | Servlet blocking o WebFlux |
| Testability | Muy testeable | Requiere MockMvc |

### 2. Request Extensions

Axum permite almacenar datos en el request para uso posterior.

**Rust:**
```rust
use axum::http::Request;

// Almacenar datos en extensions
fn pre_process(mut request: Request<Body>) -> Request<Body> {
    request.extensions_mut().insert(MyData {
        value: "stored".to_string(),
    });
    request
}

// Extraer datos en handler
async fn handler(
    Extension(data): Extension<MyData>,
) -> impl IntoResponse {
    format!("Data: {}", data.value)
}

// O desde middleware
fn post_process(response: Response<Body>, request: &Request<Body>) {
    if let Some(data) = request.extensions().get::<MyData>() {
        // Usar data
    }
}
```

**Java (Request Attributes):**
```java
// Almacenar
request.setAttribute("myData", new MyData("stored"));

// Recuperar en controller
@GetMapping("/")
public String handler(HttpServletRequest request) {
    MyData data = (MyData) request.getAttribute("myData");
    return "Data: " + data.getValue();
}

// O con Spring
@RequestScope
@Component
public class RequestScopedBean {
    private String value;
}
```

### 3. Layer Composition y State Sharing

Compartir estado entre layers usando Arc.

**Rust:**
```rust
use std::sync::Arc;

// Estado compartido
pub struct SharedState {
    policies: CompiledPolicySet,
    cache: DashMap<String, CachedResult>,
}

// Layer con estado compartido
pub struct GovernanceLayer {
    state: Arc<SharedState>,
}

impl GovernanceLayer {
    pub fn new(policies: CompiledPolicySet) -> Self {
        Self {
            state: Arc::new(SharedState {
                policies,
                cache: DashMap::new(),
            }),
        }
    }
}

impl<S> Layer<S> for GovernanceLayer {
    type Service = GovernanceService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GovernanceService {
            inner,
            state: Arc::clone(&self.state),  // Clone del Arc, no del estado
        }
    }
}

// Service tiene acceso al mismo estado
impl<S> Service<Request<Body>> for GovernanceService<S> {
    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let state = Arc::clone(&self.state);

        Box::pin(async move {
            // Acceder al estado compartido
            let result = state.policies.evaluate(&context);
            // Cache result
            state.cache.insert(key, result);
            // ...
        })
    }
}
```

**Java:**
```java
@Component
public class GovernanceFilter implements Filter {
    // Singleton by default in Spring
    private final CompiledPolicySet policies;
    private final ConcurrentMap<String, CachedResult> cache = new ConcurrentHashMap<>();

    public GovernanceFilter(CompiledPolicySet policies) {
        this.policies = policies;
    }

    @Override
    public void doFilter(...) {
        // policies y cache compartidos entre requests
    }
}
```

### 4. Response Body Transformation

Transformar el body de la respuesta.

**Rust:**
```rust
use axum::body::{Body, to_bytes};

async fn transform_response(response: Response<Body>) -> Response<Body> {
    let (parts, body) = response.into_parts();

    // Leer body completo
    let bytes = to_bytes(body, usize::MAX).await?;

    // Parsear como JSON
    let mut json: Value = serde_json::from_slice(&bytes)?;

    // Transformar
    mask_sensitive_fields(&mut json);

    // Reconstruir response
    let new_body = Body::from(serde_json::to_vec(&json)?);
    Ok(Response::from_parts(parts, new_body))
}

// Alternativa: streaming transformation para bodies grandes
use futures::stream::StreamExt;

async fn stream_transform(response: Response<Body>) -> Response<Body> {
    let (parts, body) = response.into_parts();

    let transformed = body.map(|chunk| {
        chunk.map(|bytes| {
            // Transform chunk
            bytes
        })
    });

    Response::from_parts(parts, Body::from_stream(transformed))
}
```

**Java:**
```java
@ControllerAdvice
public class ResponseTransformer implements ResponseBodyAdvice<Object> {

    @Override
    public boolean supports(MethodParameter returnType, Class converterType) {
        return true;
    }

    @Override
    public Object beforeBodyWrite(Object body, MethodParameter returnType,
            MediaType mediaType, Class converterType,
            ServerHttpRequest request, ServerHttpResponse response) {

        if (body instanceof Map) {
            maskSensitiveFields((Map<String, Object>) body);
        }
        return body;
    }
}
```

---

## Riesgos y Errores Comunes

### 1. Olvidar Propagar Extensions

```rust
// MAL: Request reconstruido sin extensions
fn call(&mut self, request: Request<Body>) -> Self::Future {
    let (parts, body) = request.into_parts();

    // ... modificar parts ...

    // Extensions perdidas si no se preservan!
    let new_request = Request::builder()
        .uri(parts.uri)
        .body(body)
        .unwrap();

    self.inner.call(new_request)
}

// BIEN: Usar from_parts que preserva todo
fn call(&mut self, request: Request<Body>) -> Self::Future {
    let (mut parts, body) = request.into_parts();

    // Modificar
    parts.extensions.insert(MyData { ... });

    // Reconstruir preservando extensions
    let request = Request::from_parts(parts, body);
    self.inner.call(request)
}
```

### 2. Body Consumido Dos Veces

```rust
// MAL: Body consumido en middleware, no disponible en handler
async fn call(&mut self, request: Request<Body>) -> ... {
    let (parts, body) = request.into_parts();

    // Leer body para logging
    let bytes = to_bytes(body, usize::MAX).await?;
    println!("Body: {:?}", bytes);

    // Reconstruir con body vacio!
    let request = Request::from_parts(parts, Body::empty());
    self.inner.call(request).await
}

// BIEN: Reconstruir body despues de leer
async fn call(&mut self, request: Request<Body>) -> ... {
    let (parts, body) = request.into_parts();

    let bytes = to_bytes(body, usize::MAX).await?;
    println!("Body: {:?}", bytes);

    // Reconstruir con los mismos bytes
    let request = Request::from_parts(parts, Body::from(bytes));
    self.inner.call(request).await
}
```

### 3. Fail-Open Inseguro

```rust
// MAL: Fail-open silencioso
fn call(&mut self, request: Request<Body>) -> Self::Future {
    Box::pin(async move {
        let decision = match evaluate_policies(&request) {
            Ok(d) => d,
            Err(_) => PolicyDecision::allow(),  // Silenciosamente permite!
        };
        // ...
    })
}

// BIEN: Fail-closed con logging
fn call(&mut self, request: Request<Body>) -> Self::Future {
    let config = self.config.clone();

    Box::pin(async move {
        let decision = match evaluate_policies(&request) {
            Ok(d) => d,
            Err(e) => {
                error!(error = %e, "Policy evaluation failed");

                if config.fail_open {
                    warn!("Fail-open enabled, allowing request");
                    PolicyDecision::allow()
                } else {
                    PolicyDecision::deny("system", "Policy evaluation error")
                }
            }
        };
        // ...
    })
}
```

### 4. Headers Duplicados

```rust
// MAL: Headers duplicados
for warning in warnings {
    parts.headers.insert("X-Warning", HeaderValue::from_str(&warning)?);
    // insert reemplaza, solo queda el ultimo!
}

// BIEN: Usar append para multiples valores
for warning in warnings {
    parts.headers.append("X-Warning", HeaderValue::from_str(&warning)?);
}
```

---

## Pruebas

### Tests de Integracion del Middleware

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::{Request, StatusCode}};
    use tower::ServiceExt;

    fn create_test_router() -> Router {
        let policies = load_test_policies();
        let policies = Arc::new(policies);

        Router::new()
            .route("/:app/:profile", get(|| async { "OK" }))
            .layer(GovernanceLayer::new(policies))
    }

    #[tokio::test]
    async fn test_allowed_request() {
        let app = create_test_router();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/myapp/dev")
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_denied_request() {
        let app = create_test_router();

        // Assuming policy denies internal-* apps from external IPs
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/internal-api/prod")
                    .header("X-Forwarded-For", "192.168.1.1")
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_warning_header() {
        let app = create_test_router();

        // Assuming policy warns on legacy-app
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/legacy-app/dev")
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key("X-Governance-Warning"));
    }

    #[tokio::test]
    async fn test_response_masking() {
        let app = create_test_router_with_mock_handler(|| async {
            axum::Json(serde_json::json!({
                "database": {
                    "url": "postgres://localhost/db",
                    "password": "supersecret123"
                }
            }))
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/myapp/production")
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap();

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        // Password should be masked
        let password = json["database"]["password"].as_str().unwrap();
        assert!(password.starts_with("*"));
        assert!(!password.contains("supersecret"));
    }
}
```

### Tests del Extractor

```rust
#[cfg(test)]
mod extractor_tests {
    use super::*;

    #[test]
    fn test_extract_path_params() {
        let uri = "/payment-service/production,staging/main".parse().unwrap();
        let mut parts = http::request::Parts::default();
        parts.uri = uri;

        let context = extract_context(&parts);

        assert_eq!(context.application, "payment-service");
        assert_eq!(context.profiles, vec!["production", "staging"]);
        assert_eq!(context.label, Some("main".to_string()));
    }

    #[test]
    fn test_extract_source_ip_from_forwarded() {
        let mut parts = http::request::Parts::default();
        parts.headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("10.0.0.1, 192.168.1.1"),
        );

        let ip = extract_source_ip(&parts);

        assert_eq!(ip, Some("10.0.0.1".parse().unwrap()));
    }

    #[test]
    fn test_extract_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_static("key123"));
        headers.insert("x-tenant-id", HeaderValue::from_static("tenant1"));
        headers.insert("irrelevant-header", HeaderValue::from_static("ignored"));

        let extracted = extract_headers(&headers);

        assert_eq!(extracted.get("x-api-key"), Some(&"key123".to_string()));
        assert_eq!(extracted.get("x-tenant-id"), Some(&"tenant1".to_string()));
        assert!(!extracted.contains_key("irrelevant-header"));
    }
}
```

---

## Seguridad

- **Fail-closed por defecto**: Errores en evaluacion resultan en denegacion
- **No leak de politicas**: Mensajes de error no revelan reglas internas
- **Audit logging**: Todas las denegaciones se loguean con contexto
- **IP Spoofing**: Validar que X-Forwarded-For viene de proxy confiable
- **Header Injection**: Sanitizar valores de headers antes de copiar

---

## Entregable Final

### Archivos Creados

1. `src/middleware/mod.rs` - Re-exports del modulo
2. `src/middleware/layer.rs` - GovernanceLayer y config
3. `src/middleware/service.rs` - GovernanceService
4. `src/middleware/extractor.rs` - RequestContext extractor
5. `src/middleware/transformer.rs` - Response transformer

### Verificacion

```bash
# Compilar
cargo build -p vortex-governance

# Tests
cargo test -p vortex-governance -- middleware

# Test manual
cargo run -p vortex-server &

# Request permitido
curl -v http://localhost:8080/myapp/dev

# Request denegado (si hay politica)
curl -v http://localhost:8080/internal-api/prod \
    -H "X-Forwarded-For: 192.168.1.1"

# Expected: HTTP 403
# {
#   "error": "Forbidden",
#   "message": "Access denied: internal services require internal network"
# }

# Request con warning
curl -v http://localhost:8080/legacy-app/dev
# Expected: X-Governance-Warning: This app is deprecated
```

### Ejemplo de Stack Completo

```rust
// main.rs
use axum::Router;
use std::sync::Arc;
use tower::ServiceBuilder;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::init();

    // Load governance policies
    let policies = PolicyLoader::new()
        .from_directory("./policies")
        .expect("Failed to load policies");

    // Build router with governance
    let app = Router::new()
        .route("/health", get(health))
        .route("/:app/:profile", get(get_config))
        .route("/:app/:profile/:label", get(get_config))
        .layer(
            ServiceBuilder::new()
                .layer(RequestIdLayer)
                .layer(LoggingLayer)
                .layer(GovernanceLayer::new(Arc::new(policies)))
        );

    // Run server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```
