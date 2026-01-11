# Historia 007: API de Compliance Reports

## Contexto y Objetivo

Esta historia expone el motor de compliance (historia 006) a traves de una API REST, permitiendo:

- **Validar configuraciones on-demand**: Antes de deployment
- **Generar reportes estructurados**: JSON, CSV para auditores
- **Integrar con CI/CD**: Fallar builds si hay violaciones criticas
- **Consultar estado de compliance**: Dashboard de cumplimiento

La API permite automatizar validaciones de seguridad como parte del pipeline de desarrollo.

---

## Alcance

### In Scope

- Endpoint `POST /compliance/validate` para validacion de configuracion
- Endpoint `GET /compliance/validate/{app}/{profile}` para configs almacenadas
- Endpoint `GET /compliance/rules` para listar reglas disponibles
- Endpoint `GET /compliance/standards` para listar estandares
- Reportes en JSON y formato tabular
- Filtrado por severidad y estandar
- Response codes apropiados (200, 422, 500)

### Out of Scope

- Persistencia de reportes historicos
- Notificaciones (email, Slack)
- Remediacion automatica
- Dashboard UI

---

## Criterios de Aceptacion

- [ ] `POST /compliance/validate` acepta JSON y retorna report
- [ ] `GET /compliance/validate/{app}/{profile}` valida config almacenada
- [ ] Query params filtran por severidad y estandar
- [ ] Response 200 si compliance pasa, 422 si falla
- [ ] Reporte incluye summary, violaciones, metadata
- [ ] `GET /compliance/rules` lista todas las reglas
- [ ] `GET /compliance/standards` lista estandares soportados
- [ ] Tests de integracion pasan

---

## Diseno Propuesto

### Endpoints

```
┌──────────────────────────────────────────────────────────────────────┐
│                       Compliance API                                  │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  POST /compliance/validate                                            │
│       Validate a configuration payload                                │
│       Body: JSON configuration                                        │
│       Query: ?standards=pci-dss,soc2&min_severity=high               │
│                                                                       │
│  GET  /compliance/validate/{app}/{profile}                            │
│       Validate stored configuration for app/profile                   │
│       Query: ?standards=pci-dss&min_severity=critical                │
│                                                                       │
│  GET  /compliance/rules                                               │
│       List all available compliance rules                             │
│       Query: ?standard=pci-dss&severity=critical                     │
│                                                                       │
│  GET  /compliance/standards                                           │
│       List all supported compliance standards                         │
│                                                                       │
│  GET  /compliance/report/{report_id}                                  │
│       Get a previously generated report (if persisted)               │
│                                                                       │
└──────────────────────────────────────────────────────────────────────┘
```

### Request/Response Examples

**Validate Configuration:**
```http
POST /compliance/validate?standards=pci-dss&min_severity=high
Content-Type: application/json

{
  "database": {
    "password": "plaintext123",
    "connection_string": "postgres://user:pass@host/db"
  },
  "api": {
    "endpoint": "http://api.example.com"
  }
}

Response: 422 Unprocessable Entity
{
  "status": "FAILED",
  "summary": {
    "total_violations": 3,
    "by_severity": {
      "CRITICAL": 2,
      "HIGH": 1
    },
    "by_standard": {
      "PCI-DSS": 3
    }
  },
  "violations": [
    {
      "rule_id": "pci-dss-3.4-no-plaintext-passwords",
      "rule_name": "No Plaintext Passwords",
      "path": "database.password",
      "severity": "CRITICAL",
      "standard": "PCI-DSS",
      "message": "Value appears to be plaintext",
      "value": "pl...[REDACTED]",
      "remediation": "Use secret references instead"
    },
    ...
  ],
  "metadata": {
    "rules_evaluated": 15,
    "paths_checked": 8,
    "checked_at": "2024-01-15T10:30:00Z",
    "standards_applied": ["PCI-DSS"]
  }
}
```

**Successful Validation:**
```http
POST /compliance/validate
Content-Type: application/json

{
  "database": {
    "password": "vault://secret/db/password"
  },
  "api": {
    "endpoint": "https://api.example.com"
  }
}

Response: 200 OK
{
  "status": "PASSED",
  "summary": {
    "total_violations": 0,
    "by_severity": {},
    "by_standard": {}
  },
  "violations": [],
  "metadata": {
    "rules_evaluated": 15,
    "paths_checked": 5,
    "checked_at": "2024-01-15T10:30:00Z",
    "standards_applied": ["PCI-DSS", "SOC2"]
  }
}
```

---

## Pasos de Implementacion

### Paso 1: Definir Request/Response Types

```rust
// src/api/compliance/types.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::compliance::{
    ComplianceReport, Rule, Severity, Standard, Violation,
};

/// Query parameters for validation endpoints.
#[derive(Debug, Deserialize)]
pub struct ValidateQuery {
    /// Standards to validate against (comma-separated).
    #[serde(default)]
    pub standards: Option<String>,

    /// Minimum severity to report.
    #[serde(default)]
    pub min_severity: Option<Severity>,

    /// Output format (json, table).
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "json".to_string()
}

impl ValidateQuery {
    /// Parses standards from comma-separated string.
    pub fn get_standards(&self) -> Vec<Standard> {
        self.standards
            .as_ref()
            .map(|s| {
                s.split(',')
                    .filter_map(|s| match s.trim().to_lowercase().as_str() {
                        "pci-dss" | "pcidss" => Some(Standard::PciDss),
                        "soc2" => Some(Standard::Soc2),
                        "hipaa" => Some(Standard::Hipaa),
                        "gdpr" => Some(Standard::Gdpr),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Summary of compliance check.
#[derive(Debug, Serialize)]
pub struct ComplianceSummary {
    pub total_violations: usize,
    pub by_severity: HashMap<String, usize>,
    pub by_standard: HashMap<String, usize>,
}

impl From<&ComplianceReport> for ComplianceSummary {
    fn from(report: &ComplianceReport) -> Self {
        Self {
            total_violations: report.violations.len(),
            by_severity: report
                .severity_summary
                .iter()
                .map(|(k, v)| (format!("{:?}", k), *v))
                .collect(),
            by_standard: report
                .standard_summary
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect(),
        }
    }
}

/// Metadata about the compliance check.
#[derive(Debug, Serialize)]
pub struct ComplianceMetadata {
    pub rules_evaluated: usize,
    pub paths_checked: usize,
    pub checked_at: String,
    pub standards_applied: Vec<String>,
}

/// API response for compliance validation.
#[derive(Debug, Serialize)]
pub struct ValidateResponse {
    pub status: String,
    pub summary: ComplianceSummary,
    pub violations: Vec<ViolationResponse>,
    pub metadata: ComplianceMetadata,
}

/// Violation formatted for API response.
#[derive(Debug, Serialize)]
pub struct ViolationResponse {
    pub rule_id: String,
    pub rule_name: String,
    pub path: String,
    pub severity: String,
    pub standard: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

impl From<&Violation> for ViolationResponse {
    fn from(v: &Violation) -> Self {
        Self {
            rule_id: v.rule_id.clone(),
            rule_name: v.rule_name.clone(),
            path: v.path.clone(),
            severity: format!("{:?}", v.severity),
            standard: v.standard.to_string(),
            message: v.message.clone(),
            value: v.value.clone(),
            remediation: v.remediation.clone(),
        }
    }
}

impl ValidateResponse {
    /// Creates a response from a compliance report.
    pub fn from_report(report: ComplianceReport, standards: Vec<Standard>) -> Self {
        let status = match report.status {
            crate::compliance::ComplianceStatus::Passed => "PASSED",
            crate::compliance::ComplianceStatus::Failed => "FAILED",
            crate::compliance::ComplianceStatus::Error => "ERROR",
        };

        Self {
            status: status.to_string(),
            summary: ComplianceSummary::from(&report),
            violations: report.violations.iter().map(ViolationResponse::from).collect(),
            metadata: ComplianceMetadata {
                rules_evaluated: report.rules_evaluated,
                paths_checked: report.paths_checked,
                checked_at: report.checked_at.to_rfc3339(),
                standards_applied: standards.iter().map(|s| s.to_string()).collect(),
            },
        }
    }
}

/// Rule formatted for API response.
#[derive(Debug, Serialize)]
pub struct RuleResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub standard: String,
    pub severity: String,
    pub path_pattern: String,
    pub enabled: bool,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

impl From<&Rule> for RuleResponse {
    fn from(r: &Rule) -> Self {
        Self {
            id: r.id.clone(),
            name: r.name.clone(),
            description: r.description.clone(),
            standard: r.standard.to_string(),
            severity: format!("{:?}", r.severity),
            path_pattern: r.path_pattern.clone(),
            enabled: r.enabled,
            tags: r.tags.clone(),
            remediation: r.remediation.clone(),
        }
    }
}

/// Response for listing rules.
#[derive(Debug, Serialize)]
pub struct ListRulesResponse {
    pub rules: Vec<RuleResponse>,
    pub total: usize,
}

/// Response for listing standards.
#[derive(Debug, Serialize)]
pub struct ListStandardsResponse {
    pub standards: Vec<StandardInfo>,
}

#[derive(Debug, Serialize)]
pub struct StandardInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub rule_count: usize,
}
```

### Paso 2: Implementar Compliance Service

```rust
// src/api/compliance/service.rs
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::instrument;

use crate::compliance::{ComplianceEngine, ComplianceReport, Rule, Severity, Standard};
use crate::config_source::ConfigSource;

/// Service for compliance operations.
pub struct ComplianceService {
    engine: Arc<RwLock<ComplianceEngine>>,
    config_source: Option<Arc<dyn ConfigSource + Send + Sync>>,
}

impl ComplianceService {
    /// Creates a new compliance service with default rules.
    pub fn new() -> Self {
        Self {
            engine: Arc::new(RwLock::new(ComplianceEngine::with_defaults())),
            config_source: None,
        }
    }

    /// Creates a service with a config source for stored configurations.
    pub fn with_config_source(
        config_source: Arc<dyn ConfigSource + Send + Sync>,
    ) -> Self {
        Self {
            engine: Arc::new(RwLock::new(ComplianceEngine::with_defaults())),
            config_source: Some(config_source),
        }
    }

    /// Validates a configuration against compliance rules.
    #[instrument(skip(self, config))]
    pub async fn validate(
        &self,
        config: &serde_json::Value,
        standards: &[Standard],
        min_severity: Option<Severity>,
    ) -> ComplianceReport {
        let engine = self.engine.read().await;
        let mut report = engine.evaluate(config);

        // Filter by standards if specified
        if !standards.is_empty() {
            report.violations.retain(|v| standards.contains(&v.standard));
        }

        // Filter by minimum severity
        if let Some(min) = min_severity {
            report.violations.retain(|v| v.severity >= min);
        }

        // Recalculate summaries after filtering
        report.severity_summary.clear();
        report.standard_summary.clear();
        for v in &report.violations {
            *report.severity_summary.entry(v.severity).or_insert(0) += 1;
            *report.standard_summary.entry(v.standard).or_insert(0) += 1;
        }

        // Update status
        report.status = if report.violations.is_empty() {
            crate::compliance::ComplianceStatus::Passed
        } else {
            crate::compliance::ComplianceStatus::Failed
        };

        report
    }

    /// Validates a stored configuration.
    #[instrument(skip(self))]
    pub async fn validate_stored(
        &self,
        app: &str,
        profile: &str,
        label: Option<&str>,
        standards: &[Standard],
        min_severity: Option<Severity>,
    ) -> Result<ComplianceReport, ComplianceServiceError> {
        let source = self
            .config_source
            .as_ref()
            .ok_or(ComplianceServiceError::NoConfigSource)?;

        let config_map = source
            .get_config(app, &[profile.to_string()], label)
            .await
            .map_err(|e| ComplianceServiceError::ConfigFetchError(e.to_string()))?;

        // Convert ConfigMap to JSON for validation
        let config_json = config_map_to_json(&config_map);

        Ok(self.validate(&config_json, standards, min_severity).await)
    }

    /// Lists all rules.
    pub async fn list_rules(
        &self,
        standard: Option<Standard>,
        severity: Option<Severity>,
    ) -> Vec<Rule> {
        let engine = self.engine.read().await;

        engine
            .rules
            .iter()
            .filter(|r| standard.map_or(true, |s| r.standard == s))
            .filter(|r| severity.map_or(true, |s| r.severity == s))
            .cloned()
            .collect()
    }

    /// Lists supported standards with rule counts.
    pub async fn list_standards(&self) -> Vec<(Standard, usize)> {
        let engine = self.engine.read().await;

        let mut counts = std::collections::HashMap::new();
        for rule in &engine.rules {
            *counts.entry(rule.standard).or_insert(0) += 1;
        }

        counts.into_iter().collect()
    }

    /// Adds a custom rule.
    pub async fn add_rule(&self, rule: Rule) {
        let mut engine = self.engine.write().await;
        engine.add_rule(rule);
    }
}

impl Default for ComplianceService {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ComplianceServiceError {
    #[error("no config source configured")]
    NoConfigSource,

    #[error("failed to fetch configuration: {0}")]
    ConfigFetchError(String),
}

fn config_map_to_json(config_map: &crate::core::ConfigMap) -> serde_json::Value {
    // Merge all property sources into a single JSON object
    let mut merged = serde_json::Map::new();

    for source in config_map.property_sources.iter().rev() {
        for (key, value) in &source.source {
            merged.insert(key.clone(), value.clone());
        }
    }

    serde_json::Value::Object(merged)
}
```

### Paso 3: Implementar Handlers

```rust
// src/api/compliance/handlers.rs
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use std::sync::Arc;

use super::service::{ComplianceService, ComplianceServiceError};
use super::types::*;

/// Application state for compliance API.
pub struct ComplianceAppState {
    pub service: Arc<ComplianceService>,
}

/// POST /compliance/validate - Validate a configuration payload.
pub async fn validate_config(
    State(state): State<Arc<ComplianceAppState>>,
    Query(query): Query<ValidateQuery>,
    Json(config): Json<serde_json::Value>,
) -> impl IntoResponse {
    let standards = query.get_standards();
    let report = state
        .service
        .validate(&config, &standards, query.min_severity)
        .await;

    let response = ValidateResponse::from_report(
        report.clone(),
        if standards.is_empty() {
            vec![crate::compliance::Standard::PciDss, crate::compliance::Standard::Soc2]
        } else {
            standards
        },
    );

    let status = match report.status {
        crate::compliance::ComplianceStatus::Passed => StatusCode::OK,
        crate::compliance::ComplianceStatus::Failed => StatusCode::UNPROCESSABLE_ENTITY,
        crate::compliance::ComplianceStatus::Error => StatusCode::INTERNAL_SERVER_ERROR,
    };

    (status, Json(response))
}

/// GET /compliance/validate/{app}/{profile} - Validate stored configuration.
pub async fn validate_stored_config(
    State(state): State<Arc<ComplianceAppState>>,
    Path((app, profile)): Path<(String, String)>,
    Query(query): Query<ValidateQuery>,
) -> Result<impl IntoResponse, ComplianceApiError> {
    let standards = query.get_standards();

    let report = state
        .service
        .validate_stored(&app, &profile, None, &standards, query.min_severity)
        .await
        .map_err(ComplianceApiError::from)?;

    let response = ValidateResponse::from_report(
        report.clone(),
        if standards.is_empty() {
            vec![crate::compliance::Standard::PciDss, crate::compliance::Standard::Soc2]
        } else {
            standards
        },
    );

    let status = match report.status {
        crate::compliance::ComplianceStatus::Passed => StatusCode::OK,
        crate::compliance::ComplianceStatus::Failed => StatusCode::UNPROCESSABLE_ENTITY,
        crate::compliance::ComplianceStatus::Error => StatusCode::INTERNAL_SERVER_ERROR,
    };

    Ok((status, Json(response)))
}

/// GET /compliance/rules - List available rules.
pub async fn list_rules(
    State(state): State<Arc<ComplianceAppState>>,
    Query(query): Query<ListRulesQuery>,
) -> impl IntoResponse {
    let rules = state
        .service
        .list_rules(query.standard, query.severity)
        .await;

    let response = ListRulesResponse {
        total: rules.len(),
        rules: rules.iter().map(RuleResponse::from).collect(),
    };

    Json(response)
}

#[derive(Debug, serde::Deserialize)]
pub struct ListRulesQuery {
    pub standard: Option<crate::compliance::Standard>,
    pub severity: Option<crate::compliance::Severity>,
}

/// GET /compliance/standards - List supported standards.
pub async fn list_standards(
    State(state): State<Arc<ComplianceAppState>>,
) -> impl IntoResponse {
    let standards = state.service.list_standards().await;

    let response = ListStandardsResponse {
        standards: standards
            .into_iter()
            .map(|(standard, count)| StandardInfo {
                id: format!("{:?}", standard).to_lowercase(),
                name: standard.to_string(),
                description: get_standard_description(standard),
                rule_count: count,
            })
            .collect(),
    };

    Json(response)
}

fn get_standard_description(standard: crate::compliance::Standard) -> String {
    match standard {
        crate::compliance::Standard::PciDss => {
            "Payment Card Industry Data Security Standard".to_string()
        }
        crate::compliance::Standard::Soc2 => {
            "Service Organization Control 2".to_string()
        }
        crate::compliance::Standard::Hipaa => {
            "Health Insurance Portability and Accountability Act".to_string()
        }
        crate::compliance::Standard::Gdpr => {
            "General Data Protection Regulation".to_string()
        }
        crate::compliance::Standard::Custom => "Custom organizational rules".to_string(),
    }
}

/// API error type for compliance endpoints.
#[derive(Debug)]
pub enum ComplianceApiError {
    NotFound(String),
    ServiceError(String),
}

impl From<ComplianceServiceError> for ComplianceApiError {
    fn from(err: ComplianceServiceError) -> Self {
        match err {
            ComplianceServiceError::NoConfigSource => {
                ComplianceApiError::ServiceError("Config source not configured".to_string())
            }
            ComplianceServiceError::ConfigFetchError(msg) => {
                ComplianceApiError::NotFound(msg)
            }
        }
    }
}

impl IntoResponse for ComplianceApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ComplianceApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ComplianceApiError::ServiceError(msg) => {
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

### Paso 4: Configurar Router

```rust
// src/api/compliance/router.rs
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use super::handlers::*;

/// Creates the compliance API router.
pub fn compliance_router(state: Arc<ComplianceAppState>) -> Router {
    Router::new()
        .route("/validate", post(validate_config))
        .route("/validate/:app/:profile", get(validate_stored_config))
        .route("/rules", get(list_rules))
        .route("/standards", get(list_standards))
        .with_state(state)
}
```

### Paso 5: Implementar Formato Tabular

```rust
// src/api/compliance/format.rs
use crate::compliance::Violation;

/// Formats violations as a table for CLI/terminal output.
pub fn format_violations_table(violations: &[Violation]) -> String {
    if violations.is_empty() {
        return "No violations found. Compliance check passed.".to_string();
    }

    let mut output = String::new();

    // Header
    output.push_str(&format!(
        "{:<15} {:<10} {:<40} {}\n",
        "SEVERITY", "STANDARD", "PATH", "MESSAGE"
    ));
    output.push_str(&"-".repeat(100));
    output.push('\n');

    // Rows
    for v in violations {
        let severity = format!("{:?}", v.severity);
        let path = truncate(&v.path, 38);
        let message = truncate(&v.message, 50);

        output.push_str(&format!(
            "{:<15} {:<10} {:<40} {}\n",
            severity, v.standard, path, message
        ));
    }

    // Summary
    output.push_str(&"-".repeat(100));
    output.push_str(&format!("\nTotal: {} violations\n", violations.len()));

    output
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Formats violations as CSV.
pub fn format_violations_csv(violations: &[Violation]) -> String {
    let mut output = String::new();

    // Header
    output.push_str("rule_id,severity,standard,path,message,remediation\n");

    // Rows
    for v in violations {
        output.push_str(&format!(
            "{},{:?},{},{},{},{}\n",
            escape_csv(&v.rule_id),
            v.severity,
            v.standard,
            escape_csv(&v.path),
            escape_csv(&v.message),
            escape_csv(&v.remediation.clone().unwrap_or_default())
        ));
    }

    output
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
```

---

## Conceptos de Rust Aprendidos

### 1. Status Codes Dinamicos basados en Resultado

**Rust:**
```rust
pub async fn validate_config(...) -> impl IntoResponse {
    let report = service.validate(&config, ...).await;

    let status = match report.status {
        ComplianceStatus::Passed => StatusCode::OK,           // 200
        ComplianceStatus::Failed => StatusCode::UNPROCESSABLE_ENTITY,  // 422
        ComplianceStatus::Error => StatusCode::INTERNAL_SERVER_ERROR,  // 500
    };

    // Tupla (StatusCode, Json) implementa IntoResponse
    (status, Json(response))
}
```

**Comparacion con Java (Spring):**
```java
@PostMapping("/validate")
public ResponseEntity<ValidateResponse> validate(@RequestBody JsonNode config) {
    ComplianceReport report = service.validate(config);

    HttpStatus status = switch (report.getStatus()) {
        case PASSED -> HttpStatus.OK;
        case FAILED -> HttpStatus.UNPROCESSABLE_ENTITY;
        case ERROR -> HttpStatus.INTERNAL_SERVER_ERROR;
    };

    return ResponseEntity.status(status).body(response);
}
```

### 2. Query Parameter Parsing con Serde

**Rust:**
```rust
#[derive(Deserialize)]
pub struct ValidateQuery {
    /// Standards es un string como "pci-dss,soc2"
    #[serde(default)]
    pub standards: Option<String>,

    /// Serde puede deserializar enum directamente
    #[serde(default)]
    pub min_severity: Option<Severity>,

    /// Default via funcion
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "json".to_string()
}

// En handler
pub async fn handler(Query(query): Query<ValidateQuery>) {
    let standards = query.get_standards();  // Parsing custom
}
```

**Comparacion con Java (Spring):**
```java
@GetMapping("/validate")
public ResponseEntity<?> validate(
    @RequestParam(required = false) String standards,
    @RequestParam(required = false) Severity minSeverity,
    @RequestParam(defaultValue = "json") String format
) {
    List<Standard> standardList = parseStandards(standards);
    // ...
}
```

### 3. Error Conversion con From Trait

**Rust:**
```rust
impl From<ComplianceServiceError> for ComplianceApiError {
    fn from(err: ComplianceServiceError) -> Self {
        match err {
            ComplianceServiceError::NoConfigSource => {
                ComplianceApiError::ServiceError("Config source not configured".into())
            }
            ComplianceServiceError::ConfigFetchError(msg) => {
                ComplianceApiError::NotFound(msg)
            }
        }
    }
}

// Permite usar ? operator
pub async fn handler(...) -> Result<impl IntoResponse, ComplianceApiError> {
    let report = service
        .validate_stored(...)
        .await
        .map_err(ComplianceApiError::from)?;  // O simplemente ?

    Ok(Json(response))
}
```

**Comparacion con Java:**
```java
// Java: catch y re-throw o exception handler global
try {
    Report report = service.validateStored(...);
    return ResponseEntity.ok(report);
} catch (NoConfigSourceException e) {
    throw new ApiException(HttpStatus.INTERNAL_SERVER_ERROR, "Config source not configured");
} catch (ConfigFetchException e) {
    throw new ApiException(HttpStatus.NOT_FOUND, e.getMessage());
}
```

### 4. Multiple Extractors en Handler

**Rust:**
```rust
// Axum extrae State, Path, Query automaticamente
pub async fn validate_stored_config(
    State(state): State<Arc<ComplianceAppState>>,      // Estado compartido
    Path((app, profile)): Path<(String, String)>,       // Path params
    Query(query): Query<ValidateQuery>,                 // Query params
) -> Result<impl IntoResponse, ComplianceApiError> {
    // Todos los valores extraidos y tipados
}
```

**Comparacion con Java (Spring):**
```java
@GetMapping("/validate/{app}/{profile}")
public ResponseEntity<?> validateStored(
    // Spring injections
    @PathVariable String app,
    @PathVariable String profile,
    @RequestParam(required = false) String standards,
    @RequestParam(required = false) Severity minSeverity
) {
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
    use serde_json::json;

    async fn setup_app() -> Router {
        let service = Arc::new(ComplianceService::new());
        let state = Arc::new(ComplianceAppState { service });
        compliance_router(state)
    }

    #[tokio::test]
    async fn test_validate_passing_config() {
        let app = setup_app().await;

        let config = json!({
            "database": {
                "password": "vault://secret/db/password"
            },
            "api": {
                "endpoint": "https://api.example.com"
            }
        });

        let response = app
            .oneshot(
                Request::post("/validate")
                    .header("Content-Type", "application/json")
                    .body(Body::from(config.to_string()))
                    .unwrap()
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let json: ValidateResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(json.status, "PASSED");
        assert_eq!(json.summary.total_violations, 0);
    }

    #[tokio::test]
    async fn test_validate_failing_config() {
        let app = setup_app().await;

        let config = json!({
            "database": {
                "password": "plaintext-secret"
            },
            "api": {
                "endpoint": "http://insecure.example.com"
            }
        });

        let response = app
            .oneshot(
                Request::post("/validate")
                    .header("Content-Type", "application/json")
                    .body(Body::from(config.to_string()))
                    .unwrap()
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let json: ValidateResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(json.status, "FAILED");
        assert!(json.summary.total_violations > 0);
    }

    #[tokio::test]
    async fn test_filter_by_severity() {
        let app = setup_app().await;

        let config = json!({
            "database": {
                "password": "plaintext"  // CRITICAL
            },
            "debug": true  // MEDIUM
        });

        // Only critical
        let response = app
            .clone()
            .oneshot(
                Request::post("/validate?min_severity=CRITICAL")
                    .header("Content-Type", "application/json")
                    .body(Body::from(config.to_string()))
                    .unwrap()
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let json: ValidateResponse = serde_json::from_slice(&body).unwrap();

        // Should only have critical violations
        for v in &json.violations {
            assert_eq!(v.severity, "Critical");
        }
    }

    #[tokio::test]
    async fn test_list_rules() {
        let app = setup_app().await;

        let response = app
            .oneshot(Request::get("/rules").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let json: ListRulesResponse = serde_json::from_slice(&body).unwrap();

        assert!(json.total > 0);
        assert!(!json.rules.is_empty());
    }

    #[tokio::test]
    async fn test_list_standards() {
        let app = setup_app().await;

        let response = app
            .oneshot(Request::get("/standards").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let json: ListStandardsResponse = serde_json::from_slice(&body).unwrap();

        assert!(!json.standards.is_empty());

        let pci = json.standards.iter().find(|s| s.id == "pcidss");
        assert!(pci.is_some());
    }

    #[tokio::test]
    async fn test_filter_rules_by_standard() {
        let app = setup_app().await;

        let response = app
            .oneshot(
                Request::get("/rules?standard=pci-dss")
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let json: ListRulesResponse = serde_json::from_slice(&body).unwrap();

        for rule in &json.rules {
            assert_eq!(rule.standard, "PCI-DSS");
        }
    }
}
```

---

## Seguridad

### Consideraciones

1. **Redaccion de valores**: Nunca exponer valores sensibles completos
2. **Rate limiting**: Prevenir abuse de validacion
3. **Input size limits**: Limitar tamano de configuracion a validar
4. **No ejecutar codigo**: Solo validar estructura, no ejecutar

```rust
/// Maximum configuration size to validate (1MB).
const MAX_CONFIG_SIZE: usize = 1024 * 1024;

/// Middleware to limit request body size.
pub async fn validate_config(
    State(state): State<Arc<ComplianceAppState>>,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, ComplianceApiError> {
    if body.len() > MAX_CONFIG_SIZE {
        return Err(ComplianceApiError::ServiceError(
            "Configuration too large (max 1MB)".to_string()
        ));
    }

    let config: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| ComplianceApiError::ServiceError(format!("Invalid JSON: {}", e)))?;

    // Continue with validation...
}
```

---

## Entregable Final

### Archivos Creados

1. `src/api/compliance/mod.rs` - Module exports
2. `src/api/compliance/types.rs` - Request/Response types
3. `src/api/compliance/service.rs` - Compliance service
4. `src/api/compliance/handlers.rs` - Route handlers
5. `src/api/compliance/router.rs` - Router configuration
6. `src/api/compliance/format.rs` - Table/CSV formatting
7. `tests/api/compliance_test.rs` - Integration tests

### Verificacion

```bash
cargo build -p vortex-server
cargo test -p vortex-server api::compliance
cargo clippy -p vortex-server -- -D warnings
```

### Ejemplo de Uso

```bash
# Validate a configuration
curl -X POST http://localhost:8080/compliance/validate \
  -H "Content-Type: application/json" \
  -d '{
    "database": {
      "password": "plaintext123"
    }
  }'

# Validate with specific standards
curl -X POST "http://localhost:8080/compliance/validate?standards=pci-dss&min_severity=HIGH" \
  -H "Content-Type: application/json" \
  -d @config.json

# Validate stored configuration
curl "http://localhost:8080/compliance/validate/payment-service/production"

# List rules
curl "http://localhost:8080/compliance/rules"
curl "http://localhost:8080/compliance/rules?standard=pci-dss&severity=CRITICAL"

# List standards
curl "http://localhost:8080/compliance/standards"
```

### Integracion con CI/CD

```yaml
# .github/workflows/compliance.yml
name: Compliance Check

on: [push, pull_request]

jobs:
  compliance:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Check compliance
        run: |
          response=$(curl -s -w "%{http_code}" -o response.json \
            -X POST http://config-server/compliance/validate?min_severity=HIGH \
            -H "Content-Type: application/json" \
            -d @config/production.json)

          if [ "$response" != "200" ]; then
            echo "Compliance check failed!"
            cat response.json | jq '.violations[] | "\(.severity): \(.path) - \(.message)"'
            exit 1
          fi

          echo "Compliance check passed!"
```

---

**Anterior**: [Historia 006 - Compliance Rules Engine](./story-006-compliance-engine.md)
**Siguiente**: [Epica 10 - Enterprise](../10-enterprise/index.md)
