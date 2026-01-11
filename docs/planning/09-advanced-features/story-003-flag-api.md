# Historia 003: API de Feature Flags

## Contexto y Objetivo

Esta historia expone los feature flags a traves de una API REST, permitiendo que aplicaciones cliente consulten el estado de flags sin necesidad de SDK dedicado. La API soporta:

- **Single flag evaluation**: Consultar un flag especifico
- **Batch evaluation**: Consultar multiples flags en una sola llamada
- **Flag listing**: Listar flags disponibles con metadata

La API esta disenada para ser eficiente, cacheable y compatible con patrones comunes de feature flag services como LaunchDarkly y Split.io.

---

## Alcance

### In Scope

- Endpoint `GET /flags/{flag_id}` para evaluacion individual
- Endpoint `POST /flags/evaluate` para evaluacion batch
- Endpoint `GET /flags` para listar flags disponibles
- Headers para pasar contexto de evaluacion
- Caching de respuestas con ETags
- Rate limiting por cliente

### Out of Scope

- Streaming de cambios (WebSocket)
- Admin API para CRUD de flags
- Flag analytics y eventos
- SDK client generation

---

## Criterios de Aceptacion

- [ ] `GET /flags/{flag_id}` retorna evaluacion para un flag
- [ ] Contexto pasado via query params o headers
- [ ] `POST /flags/evaluate` soporta batch con body JSON
- [ ] `GET /flags` lista flags con metadata
- [ ] Respuestas incluyen ETag para caching
- [ ] Rate limiting configurable (default 100 req/s)
- [ ] Errores siguen formato estandar de Vortex
- [ ] Tests de integracion pasan

---

## Diseno Propuesto

### Endpoints

```
┌──────────────────────────────────────────────────────────────────────┐
│                       Feature Flags API                               │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  GET  /flags                                                          │
│       List all available flags (metadata only)                        │
│       Query: ?app=myapp&environment=prod                              │
│                                                                       │
│  GET  /flags/{flag_id}                                                │
│       Evaluate a single flag                                          │
│       Headers: X-User-ID, X-Attributes (JSON)                         │
│       Query: ?user_id=xxx&attr.group=beta                             │
│                                                                       │
│  POST /flags/evaluate                                                 │
│       Batch evaluate multiple flags                                   │
│       Body: { context: {...}, flag_ids: [...] }                       │
│                                                                       │
│  GET  /flags/{flag_id}/details                                        │
│       Get flag definition without evaluation                          │
│                                                                       │
└──────────────────────────────────────────────────────────────────────┘
```

### Request/Response Examples

**Single Flag Evaluation:**
```http
GET /flags/new-checkout?user_id=user-123&attr.group=beta
X-Vortex-App: payment-service
X-Vortex-Environment: production

Response:
{
  "flag_id": "new-checkout",
  "variant_id": "on",
  "value": true,
  "reason": "RULE_MATCH",
  "rule_id": "beta-users",
  "evaluated_at": "2024-01-15T10:30:00Z"
}
```

**Batch Evaluation:**
```http
POST /flags/evaluate
Content-Type: application/json

{
  "context": {
    "user_id": "user-123",
    "attributes": {
      "group": "beta",
      "plan": "enterprise"
    }
  },
  "flag_ids": ["new-checkout", "dark-mode", "premium-features"]
}

Response:
{
  "results": {
    "new-checkout": { "variant_id": "on", "value": true, ... },
    "dark-mode": { "variant_id": "off", "value": false, ... },
    "premium-features": { "variant_id": "full", "value": {...}, ... }
  },
  "evaluated_at": "2024-01-15T10:30:00Z"
}
```

---

## Pasos de Implementacion

### Paso 1: Definir Request/Response Types

```rust
// src/api/flags/types.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::flags::{
    AttributeValue, EvaluationContext, EvaluationResult,
    FeatureFlag, VariantValue,
};

/// Query parameters for single flag evaluation.
#[derive(Debug, Deserialize)]
pub struct EvaluateFlagQuery {
    /// User ID for evaluation.
    pub user_id: Option<String>,

    /// Flattened attributes (attr.key=value).
    #[serde(flatten)]
    pub attributes: HashMap<String, String>,
}

impl EvaluateFlagQuery {
    /// Builds an EvaluationContext from query params.
    pub fn into_context(self) -> EvaluationContext {
        let mut ctx = match self.user_id {
            Some(id) => EvaluationContext::with_user_id(id),
            None => EvaluationContext::anonymous(),
        };

        // Extract attributes with "attr." prefix
        for (key, value) in self.attributes {
            if let Some(attr_name) = key.strip_prefix("attr.") {
                ctx.attributes.insert(
                    attr_name.to_string(),
                    AttributeValue::String(value),
                );
            }
        }

        ctx
    }
}

/// Request body for batch evaluation.
#[derive(Debug, Deserialize)]
pub struct BatchEvaluateRequest {
    /// Evaluation context.
    pub context: EvaluationContext,

    /// Flag IDs to evaluate (empty = all).
    #[serde(default)]
    pub flag_ids: Vec<String>,
}

/// Response for single flag evaluation.
#[derive(Debug, Serialize)]
pub struct FlagEvaluationResponse {
    /// Flag identifier.
    pub flag_id: String,

    /// Selected variant.
    pub variant_id: String,

    /// The value of the variant.
    pub value: VariantValue,

    /// Why this variant was selected.
    pub reason: String,

    /// Rule that matched (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,

    /// Evaluation timestamp.
    pub evaluated_at: String,
}

impl From<EvaluationResult> for FlagEvaluationResponse {
    fn from(result: EvaluationResult) -> Self {
        Self {
            flag_id: result.flag_id,
            variant_id: result.variant_id,
            value: result.value,
            reason: format!("{:?}", result.reason),
            rule_id: result.rule_id,
            evaluated_at: result.evaluated_at.to_rfc3339(),
        }
    }
}

/// Response for batch evaluation.
#[derive(Debug, Serialize)]
pub struct BatchEvaluateResponse {
    /// Results keyed by flag ID.
    pub results: HashMap<String, FlagEvaluationResponse>,

    /// Evaluation timestamp.
    pub evaluated_at: String,
}

/// Flag metadata (without rules for listing).
#[derive(Debug, Serialize)]
pub struct FlagMetadata {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub enabled: bool,
    pub variant_count: usize,
    pub rule_count: usize,
    pub tags: Vec<String>,
}

impl From<&FeatureFlag> for FlagMetadata {
    fn from(flag: &FeatureFlag) -> Self {
        Self {
            id: flag.id.clone(),
            name: flag.name.clone(),
            description: flag.description.clone(),
            enabled: flag.enabled,
            variant_count: flag.variants.len(),
            rule_count: flag.rules.len(),
            tags: flag.tags.clone(),
        }
    }
}

/// Response for listing flags.
#[derive(Debug, Serialize)]
pub struct ListFlagsResponse {
    pub flags: Vec<FlagMetadata>,
    pub total: usize,
}
```

### Paso 2: Implementar Extractors de Contexto

```rust
// src/api/flags/extractors.rs
use axum::{
    async_trait,
    extract::{FromRequestParts, Query},
    http::{request::Parts, HeaderMap, StatusCode},
};
use serde_json::Value;
use std::collections::HashMap;

use crate::flags::{AttributeValue, EvaluationContext};

/// Extractor that builds EvaluationContext from headers and query.
pub struct ContextExtractor(pub EvaluationContext);

#[async_trait]
impl<S> FromRequestParts<S> for ContextExtractor
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let mut ctx = EvaluationContext::anonymous();

        // Extract from headers
        if let Some(user_id) = parts.headers.get("X-User-ID") {
            if let Ok(id) = user_id.to_str() {
                ctx.user_id = Some(id.to_string());
            }
        }

        // X-Attributes header (JSON)
        if let Some(attrs) = parts.headers.get("X-Attributes") {
            if let Ok(json_str) = attrs.to_str() {
                if let Ok(attrs_map) = serde_json::from_str::<HashMap<String, Value>>(json_str) {
                    for (key, value) in attrs_map {
                        ctx.attributes.insert(key, value_to_attribute(value));
                    }
                }
            }
        }

        // Extract from query parameters
        let query: Query<HashMap<String, String>> =
            Query::try_from_uri(&parts.uri).unwrap_or_default();

        if let Some(user_id) = query.get("user_id") {
            ctx.user_id = Some(user_id.clone());
        }

        // Attributes with "attr." prefix
        for (key, value) in query.iter() {
            if let Some(attr_name) = key.strip_prefix("attr.") {
                ctx.attributes.insert(
                    attr_name.to_string(),
                    AttributeValue::String(value.clone()),
                );
            }
        }

        Ok(ContextExtractor(ctx))
    }
}

fn value_to_attribute(value: Value) -> AttributeValue {
    match value {
        Value::String(s) => AttributeValue::String(s),
        Value::Number(n) => AttributeValue::Number(n.as_f64().unwrap_or(0.0)),
        Value::Bool(b) => AttributeValue::Boolean(b),
        Value::Array(arr) => AttributeValue::StringList(
            arr.into_iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
        ),
        _ => AttributeValue::String(value.to_string()),
    }
}
```

### Paso 3: Implementar Flag Service

```rust
// src/api/flags/service.rs
use std::sync::Arc;
use thiserror::Error;

use crate::flags::{
    EvaluationContext, EvaluationResult, FeatureFlag,
    FeatureFlagCollection, FlagEvaluator,
};

#[derive(Debug, Error)]
pub enum FlagServiceError {
    #[error("flag not found: {0}")]
    FlagNotFound(String),

    #[error("failed to load flags: {0}")]
    LoadError(String),
}

/// Service for managing and evaluating feature flags.
pub struct FlagService {
    evaluator: FlagEvaluator,
    flags: Arc<tokio::sync::RwLock<FeatureFlagCollection>>,
}

impl FlagService {
    /// Creates a new flag service.
    pub fn new() -> Self {
        Self {
            evaluator: FlagEvaluator::new(),
            flags: Arc::new(tokio::sync::RwLock::new(FeatureFlagCollection::new())),
        }
    }

    /// Loads flags from a collection.
    pub async fn load_flags(&self, collection: FeatureFlagCollection) {
        let mut flags = self.flags.write().await;
        *flags = collection;
    }

    /// Gets a flag by ID.
    pub async fn get_flag(&self, flag_id: &str) -> Option<FeatureFlag> {
        let flags = self.flags.read().await;
        flags.get(flag_id).cloned()
    }

    /// Lists all flags.
    pub async fn list_flags(&self) -> Vec<FeatureFlag> {
        let flags = self.flags.read().await;
        flags.flags.clone()
    }

    /// Evaluates a single flag.
    pub async fn evaluate(
        &self,
        flag_id: &str,
        context: &EvaluationContext,
    ) -> Result<EvaluationResult, FlagServiceError> {
        let flags = self.flags.read().await;

        let flag = flags
            .get(flag_id)
            .ok_or_else(|| FlagServiceError::FlagNotFound(flag_id.to_string()))?;

        Ok(self.evaluator.evaluate(flag, context))
    }

    /// Evaluates multiple flags.
    pub async fn evaluate_batch(
        &self,
        flag_ids: &[String],
        context: &EvaluationContext,
    ) -> Vec<(String, EvaluationResult)> {
        let flags = self.flags.read().await;

        let flags_to_eval: Vec<_> = if flag_ids.is_empty() {
            flags.flags.iter().collect()
        } else {
            flags
                .flags
                .iter()
                .filter(|f| flag_ids.contains(&f.id))
                .collect()
        };

        flags_to_eval
            .into_iter()
            .map(|flag| {
                let result = self.evaluator.evaluate(flag, context);
                (flag.id.clone(), result)
            })
            .collect()
    }
}

impl Default for FlagService {
    fn default() -> Self {
        Self::new()
    }
}
```

### Paso 4: Implementar Handlers

```rust
// src/api/flags/handlers.rs
use axum::{
    extract::{Path, State, Json},
    http::StatusCode,
    response::IntoResponse,
};
use std::collections::HashMap;
use std::sync::Arc;

use super::extractors::ContextExtractor;
use super::service::{FlagService, FlagServiceError};
use super::types::*;

/// Application state containing flag service.
pub struct FlagAppState {
    pub flag_service: Arc<FlagService>,
}

/// GET /flags - List all flags
pub async fn list_flags(
    State(state): State<Arc<FlagAppState>>,
) -> impl IntoResponse {
    let flags = state.flag_service.list_flags().await;

    let metadata: Vec<FlagMetadata> = flags.iter().map(FlagMetadata::from).collect();
    let total = metadata.len();

    Json(ListFlagsResponse {
        flags: metadata,
        total,
    })
}

/// GET /flags/{flag_id} - Evaluate single flag
pub async fn evaluate_flag(
    State(state): State<Arc<FlagAppState>>,
    Path(flag_id): Path<String>,
    ContextExtractor(context): ContextExtractor,
) -> Result<impl IntoResponse, FlagApiError> {
    let result = state
        .flag_service
        .evaluate(&flag_id, &context)
        .await
        .map_err(|e| match e {
            FlagServiceError::FlagNotFound(_) => FlagApiError::NotFound(flag_id),
            FlagServiceError::LoadError(msg) => FlagApiError::Internal(msg),
        })?;

    Ok(Json(FlagEvaluationResponse::from(result)))
}

/// POST /flags/evaluate - Batch evaluate flags
pub async fn evaluate_batch(
    State(state): State<Arc<FlagAppState>>,
    Json(request): Json<BatchEvaluateRequest>,
) -> impl IntoResponse {
    let results = state
        .flag_service
        .evaluate_batch(&request.flag_ids, &request.context)
        .await;

    let response_results: HashMap<String, FlagEvaluationResponse> = results
        .into_iter()
        .map(|(id, result)| (id, FlagEvaluationResponse::from(result)))
        .collect();

    Json(BatchEvaluateResponse {
        results: response_results,
        evaluated_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// GET /flags/{flag_id}/details - Get flag definition
pub async fn get_flag_details(
    State(state): State<Arc<FlagAppState>>,
    Path(flag_id): Path<String>,
) -> Result<impl IntoResponse, FlagApiError> {
    let flag = state
        .flag_service
        .get_flag(&flag_id)
        .await
        .ok_or_else(|| FlagApiError::NotFound(flag_id))?;

    Ok(Json(flag))
}

/// API error type for flags endpoints.
#[derive(Debug)]
pub enum FlagApiError {
    NotFound(String),
    BadRequest(String),
    Internal(String),
}

impl IntoResponse for FlagApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            FlagApiError::NotFound(id) => {
                (StatusCode::NOT_FOUND, format!("Flag not found: {}", id))
            }
            FlagApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            FlagApiError::Internal(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
        };

        let body = serde_json::json!({
            "error": message,
            "status": status.as_u16()
        });

        (status, Json(body)).into_response()
    }
}
```

### Paso 5: Configurar Router

```rust
// src/api/flags/router.rs
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use super::handlers::*;

/// Creates the feature flags API router.
pub fn flags_router(state: Arc<FlagAppState>) -> Router {
    Router::new()
        .route("/", get(list_flags))
        .route("/:flag_id", get(evaluate_flag))
        .route("/:flag_id/details", get(get_flag_details))
        .route("/evaluate", post(evaluate_batch))
        .with_state(state)
}
```

### Paso 6: Implementar Caching con ETags

```rust
// src/api/flags/cache.rs
use axum::{
    body::Body,
    http::{header, Request, Response, StatusCode},
    middleware::Next,
};
use sha2::{Sha256, Digest};

/// Middleware that adds ETag headers and handles If-None-Match.
pub async fn etag_middleware(
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    // Get If-None-Match header
    let if_none_match = request
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // Process request
    let response = next.run(request).await;

    // Only add ETag for successful GET responses
    if response.status() != StatusCode::OK {
        return response;
    }

    // Get response body and compute ETag
    let (parts, body) = response.into_parts();

    // For streaming, we can't compute ETag without buffering
    // In production, use a hash of the flag versions instead
    let bytes = match axum::body::to_bytes(body, 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return Response::from_parts(parts, Body::empty()),
    };

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let hash = hasher.finalize();
    let etag = format!("\"{}\"", hex::encode(&hash[..8]));

    // Check If-None-Match
    if let Some(inm) = if_none_match {
        if inm == etag || inm == "*" {
            return Response::builder()
                .status(StatusCode::NOT_MODIFIED)
                .header(header::ETAG, etag)
                .body(Body::empty())
                .unwrap();
        }
    }

    // Return response with ETag
    let mut response = Response::from_parts(parts, Body::from(bytes));
    response.headers_mut().insert(
        header::ETAG,
        etag.parse().unwrap(),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "private, max-age=60".parse().unwrap());

    response
}
```

---

## Conceptos de Rust Aprendidos

### 1. Axum Extractors Custom

Axum permite crear extractors personalizados implementando `FromRequestParts`.

**Rust:**
```rust
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::Parts,
};

pub struct ContextExtractor(pub EvaluationContext);

#[async_trait]
impl<S> FromRequestParts<S> for ContextExtractor
where
    S: Send + Sync,  // Bounds requeridos
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        // Extraer de headers y query
        let ctx = build_context_from_parts(parts);
        Ok(ContextExtractor(ctx))
    }
}

// Uso automatico en handlers
async fn handler(
    ContextExtractor(ctx): ContextExtractor,  // Extraido automaticamente
) -> impl IntoResponse {
    // ctx ya contiene el contexto construido
}
```

**Comparacion con Java (Spring):**
```java
// Spring usa @RequestAttribute, HandlerMethodArgumentResolver
@Component
public class ContextResolver implements HandlerMethodArgumentResolver {
    @Override
    public boolean supportsParameter(MethodParameter parameter) {
        return parameter.getParameterType().equals(EvaluationContext.class);
    }

    @Override
    public Object resolveArgument(
        MethodParameter parameter,
        ModelAndViewContainer mavContainer,
        NativeWebRequest webRequest,
        WebDataBinderFactory binderFactory
    ) {
        return buildContextFromRequest(webRequest);
    }
}

// Uso
@GetMapping("/flags/{id}")
public Response evaluateFlag(@PathVariable String id, EvaluationContext ctx) {
    // ctx inyectado por el resolver
}
```

### 2. Error Handling con IntoResponse

**Rust:**
```rust
#[derive(Debug)]
pub enum FlagApiError {
    NotFound(String),
    BadRequest(String),
    Internal(String),
}

impl IntoResponse for FlagApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            FlagApiError::NotFound(id) => {
                (StatusCode::NOT_FOUND, format!("Flag not found: {}", id))
            }
            FlagApiError::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, msg)
            }
            FlagApiError::Internal(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
        };

        let body = serde_json::json!({
            "error": message,
            "status": status.as_u16()
        });

        (status, Json(body)).into_response()
    }
}

// Uso en handlers
async fn handler() -> Result<Json<Data>, FlagApiError> {
    let data = do_something()
        .map_err(|e| FlagApiError::Internal(e.to_string()))?;
    Ok(Json(data))
}
```

**Comparacion con Java (Spring):**
```java
// Spring usa @ControllerAdvice
@ControllerAdvice
public class FlagExceptionHandler {
    @ExceptionHandler(FlagNotFoundException.class)
    public ResponseEntity<ErrorResponse> handleNotFound(FlagNotFoundException ex) {
        return ResponseEntity
            .status(HttpStatus.NOT_FOUND)
            .body(new ErrorResponse(ex.getMessage(), 404));
    }

    @ExceptionHandler(Exception.class)
    public ResponseEntity<ErrorResponse> handleGeneral(Exception ex) {
        return ResponseEntity
            .status(HttpStatus.INTERNAL_SERVER_ERROR)
            .body(new ErrorResponse(ex.getMessage(), 500));
    }
}
```

### 3. State Sharing con Arc

**Rust:**
```rust
pub struct FlagAppState {
    pub flag_service: Arc<FlagService>,
}

// Router usa Arc<AppState> para sharing thread-safe
pub fn flags_router(state: Arc<FlagAppState>) -> Router {
    Router::new()
        .route("/", get(list_flags))
        .with_state(state)  // Clonado para cada request
}

// En handlers, State extrae Arc clonado
async fn list_flags(
    State(state): State<Arc<FlagAppState>>,  // Arc::clone, no deep copy
) -> impl IntoResponse {
    let flags = state.flag_service.list_flags().await;
    // ...
}
```

**Comparacion con Java (Spring):**
```java
// Spring beans son singletons por defecto
@Service
public class FlagService {
    // Spring maneja el lifecycle
}

@RestController
public class FlagController {
    private final FlagService flagService;  // Inyectado

    @Autowired
    public FlagController(FlagService flagService) {
        this.flagService = flagService;
    }

    @GetMapping("/flags")
    public List<Flag> listFlags() {
        return flagService.listFlags();
    }
}
```

### 4. Query Parameters Planos con Flatten

**Rust:**
```rust
#[derive(Deserialize)]
pub struct EvaluateFlagQuery {
    pub user_id: Option<String>,

    // #[serde(flatten)] captura el resto de campos
    #[serde(flatten)]
    pub attributes: HashMap<String, String>,
}

// Query: ?user_id=123&attr.group=beta&attr.plan=pro
// Resulta en:
// - user_id = Some("123")
// - attributes = {"attr.group": "beta", "attr.plan": "pro"}

// Procesamos los attr.* manualmente
impl EvaluateFlagQuery {
    pub fn into_context(self) -> EvaluationContext {
        let mut ctx = EvaluationContext::with_user_id(self.user_id.unwrap_or_default());

        for (key, value) in self.attributes {
            if let Some(attr_name) = key.strip_prefix("attr.") {
                ctx.attributes.insert(attr_name.to_string(), value.into());
            }
        }

        ctx
    }
}
```

**Comparacion con Java:**
```java
// Spring no tiene flatten directo, usamos @RequestParam
@GetMapping("/flags/{id}")
public Response evaluate(
    @PathVariable String id,
    @RequestParam(required = false) String userId,
    @RequestParam Map<String, String> allParams  // Todos los params
) {
    Map<String, String> attributes = allParams.entrySet().stream()
        .filter(e -> e.getKey().startsWith("attr."))
        .collect(toMap(
            e -> e.getKey().substring(5),
            Map.Entry::getValue
        ));
}
```

---

## Riesgos y Errores Comunes

### 1. Context Extraction Incompleto

```rust
// MAL: Solo extraer de headers, ignorar query
async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
    let user_id = parts.headers.get("X-User-ID")...;
    // Ignora query params!
}

// BIEN: Extraer de multiples fuentes con prioridad
async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
    // Headers tienen prioridad
    let mut user_id = parts.headers.get("X-User-ID")...;

    // Query params como fallback
    let query: Query<HashMap<String, String>> = Query::try_from_uri(&parts.uri)?;
    if user_id.is_none() {
        user_id = query.get("user_id").cloned();
    }
}
```

### 2. Caching Agresivo sin Invalidacion

```rust
// MAL: Cache sin considerar cambios de flags
.insert(header::CACHE_CONTROL, "public, max-age=3600".parse().unwrap());

// BIEN: Cache corto + ETag para validacion
.insert(header::CACHE_CONTROL, "private, max-age=60".parse().unwrap());
.insert(header::ETAG, compute_etag(&flags));
```

### 3. No Manejar Flag No Encontrado

```rust
// MAL: Panic si flag no existe
let result = state.flag_service.evaluate(&flag_id, &context).await.unwrap();

// BIEN: Retornar 404 apropiado
let result = state
    .flag_service
    .evaluate(&flag_id, &context)
    .await
    .map_err(|e| match e {
        FlagServiceError::FlagNotFound(_) => FlagApiError::NotFound(flag_id),
        _ => FlagApiError::Internal(e.to_string()),
    })?;
```

### 4. Batch sin Limite

```rust
// MAL: Sin limite de flags por batch
pub async fn evaluate_batch(&self, flag_ids: &[String], ...) {
    // Puede ser muy lento con miles de flags
}

// BIEN: Limitar batch size
const MAX_BATCH_SIZE: usize = 100;

pub async fn evaluate_batch(&self, flag_ids: &[String], ...) -> Result<..., Error> {
    if flag_ids.len() > MAX_BATCH_SIZE {
        return Err(Error::TooManyFlags(flag_ids.len()));
    }
    // ...
}
```

---

## Pruebas

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use axum::body::Body;
    use tower::ServiceExt;

    async fn setup_app() -> Router {
        let service = Arc::new(FlagService::new());

        // Load test flags
        let flag = FeatureFlag::boolean("test-flag", "Test", true);
        service.load_flags(FeatureFlagCollection {
            flags: vec![flag],
        }).await;

        let state = Arc::new(FlagAppState {
            flag_service: service,
        });

        flags_router(state)
    }

    #[tokio::test]
    async fn test_list_flags() {
        let app = setup_app().await;

        let response = app
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let json: ListFlagsResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(json.total, 1);
        assert_eq!(json.flags[0].id, "test-flag");
    }

    #[tokio::test]
    async fn test_evaluate_flag_with_query_params() {
        let app = setup_app().await;

        let response = app
            .oneshot(
                Request::get("/test-flag?user_id=user-123&attr.group=beta")
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let json: FlagEvaluationResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(json.flag_id, "test-flag");
    }

    #[tokio::test]
    async fn test_evaluate_flag_with_headers() {
        let app = setup_app().await;

        let response = app
            .oneshot(
                Request::get("/test-flag")
                    .header("X-User-ID", "user-456")
                    .header("X-Attributes", r#"{"group":"internal"}"#)
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_flag_not_found() {
        let app = setup_app().await;

        let response = app
            .oneshot(
                Request::get("/nonexistent-flag")
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_batch_evaluate() {
        let app = setup_app().await;

        let body = serde_json::json!({
            "context": {
                "user_id": "user-123",
                "attributes": {
                    "group": "beta"
                }
            },
            "flag_ids": ["test-flag"]
        });

        let response = app
            .oneshot(
                Request::post("/evaluate")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap()
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let json: BatchEvaluateResponse = serde_json::from_slice(&body).unwrap();

        assert!(json.results.contains_key("test-flag"));
    }

    #[tokio::test]
    async fn test_etag_caching() {
        let app = setup_app().await;

        // First request - get ETag
        let response1 = app
            .clone()
            .oneshot(Request::get("/test-flag").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let etag = response1
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        assert!(etag.is_some());

        // Second request with If-None-Match
        let response2 = app
            .oneshot(
                Request::get("/test-flag")
                    .header("If-None-Match", etag.unwrap())
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap();

        assert_eq!(response2.status(), StatusCode::NOT_MODIFIED);
    }
}
```

---

## Seguridad

### Consideraciones

1. **Rate limiting**: Prevenir abuso de API
2. **Input validation**: Sanitizar flag IDs y atributos
3. **Information disclosure**: No exponer reglas internas en errores

```rust
// Rate limiting middleware (usando tower)
use tower::limit::RateLimitLayer;
use std::time::Duration;

pub fn rate_limited_router(state: Arc<FlagAppState>) -> Router {
    flags_router(state)
        .layer(RateLimitLayer::new(100, Duration::from_secs(1)))
}

// Input validation
fn validate_flag_id(id: &str) -> Result<(), FlagApiError> {
    if id.len() > 128 {
        return Err(FlagApiError::BadRequest("Flag ID too long".to_string()));
    }
    if !id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(FlagApiError::BadRequest("Invalid flag ID characters".to_string()));
    }
    Ok(())
}
```

---

## Entregable Final

### Archivos Creados

1. `src/api/flags/mod.rs` - Module exports
2. `src/api/flags/types.rs` - Request/Response types
3. `src/api/flags/extractors.rs` - Custom extractors
4. `src/api/flags/service.rs` - Flag service
5. `src/api/flags/handlers.rs` - Route handlers
6. `src/api/flags/router.rs` - Router configuration
7. `src/api/flags/cache.rs` - Caching middleware
8. `tests/api/flags_test.rs` - Integration tests

### Verificacion

```bash
cargo build -p vortex-server
cargo test -p vortex-server api::flags
cargo clippy -p vortex-server -- -D warnings
```

### Ejemplo de Uso

```bash
# Evaluar un flag
curl "http://localhost:8080/flags/new-checkout?user_id=user-123&attr.group=beta"

# Con headers
curl -H "X-User-ID: user-123" \
     -H 'X-Attributes: {"group":"beta"}' \
     "http://localhost:8080/flags/new-checkout"

# Batch evaluation
curl -X POST "http://localhost:8080/flags/evaluate" \
     -H "Content-Type: application/json" \
     -d '{
       "context": {
         "user_id": "user-123",
         "attributes": {"group": "beta"}
       },
       "flag_ids": ["new-checkout", "dark-mode"]
     }'

# List all flags
curl "http://localhost:8080/flags"
```

---

**Anterior**: [Historia 002 - Evaluador de Feature Flags](./story-002-flag-evaluator.md)
**Siguiente**: [Historia 004 - Configuration Templating](./story-004-templating.md)
