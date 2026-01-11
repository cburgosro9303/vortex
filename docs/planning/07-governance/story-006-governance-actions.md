# Historia 006: Acciones de Gobernanza

## Contexto y Objetivo

Con el motor de evaluacion PLAC (Historia 003) y la integracion con middleware (Historia 004) implementados, esta historia completa el sistema de gobernanza implementando las acciones que transforman las respuestas: **deny**, **redact**, **mask**, y **warn**.

**Las acciones de gobernanza:**
- **Deny**: Bloquea el request completamente, retornando 403 Forbidden
- **Redact**: Elimina propiedades sensibles de la respuesta
- **Mask**: Oculta valores sensibles reemplazandolos con asteriscos
- **Warn**: Permite la respuesta pero agrega headers de advertencia

Esta historia demuestra el **Strategy Pattern** en Rust, transformacion de respuestas JSON, y patterns de composicion de acciones.

---

## Alcance

### In Scope

- Implementacion de cada tipo de accion
- Strategy Pattern para seleccion de acciones
- Transformacion de JSON para redact/mask
- Property path matching con glob patterns
- Composicion de multiples acciones
- Headers de warning configurables
- Logging de acciones aplicadas

### Out of Scope

- Acciones customizables definidas por usuario
- Plugins de acciones externas
- Acciones asincronas (webhooks, notificaciones)
- Rollback de acciones

---

## Criterios de Aceptacion

- [ ] `DenyAction` retorna 403 con mensaje configurable
- [ ] `RedactAction` elimina propiedades por path pattern
- [ ] `MaskAction` reemplaza valores con caracteres de mascara
- [ ] `WarnAction` agrega headers X-Governance-Warning
- [ ] Multiples acciones se aplican en orden de prioridad
- [ ] Property paths soportan glob patterns (e.g., `*.password`)
- [ ] Mask preserva los ultimos N caracteres si configurado
- [ ] Tests cubren todas las acciones y combinaciones

---

## Diseno Propuesto

### Strategy Pattern para Acciones

```
┌─────────────────────────────────────────────────────────────────────┐
│                      GovernanceAction Trait                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  pub trait GovernanceAction {                                       │
│      fn apply(&self, response: Response) -> ActionResult;          │
│      fn is_terminal(&self) -> bool;                                 │
│      fn priority(&self) -> u32;                                     │
│  }                                                                   │
│                                                                      │
└───────────────────────────┬─────────────────────────────────────────┘
                            │
        ┌───────────────────┼───────────────────┬───────────────────┐
        │                   │                   │                   │
        ▼                   ▼                   ▼                   ▼
┌───────────────┐   ┌───────────────┐   ┌───────────────┐   ┌───────────────┐
│  DenyAction   │   │ RedactAction  │   │  MaskAction   │   │  WarnAction   │
├───────────────┤   ├───────────────┤   ├───────────────┤   ├───────────────┤
│ message: Str  │   │ paths: [Str]  │   │ paths: [Str]  │   │ message: Str  │
│               │   │               │   │ mask_char: c  │   │ header: Str   │
│ is_terminal:  │   │ is_terminal:  │   │ visible: n    │   │               │
│   true        │   │   false       │   │               │   │ is_terminal:  │
│               │   │               │   │ is_terminal:  │   │   false       │
│ apply() ->    │   │ apply() ->    │   │   false       │   │               │
│   Deny(403)   │   │   Modified    │   │               │   │ apply() ->    │
└───────────────┘   │   Response    │   │ apply() ->    │   │   Response +  │
                    └───────────────┘   │   Modified    │   │   Header      │
                                        │   Response    │   └───────────────┘
                                        └───────────────┘
```

### Estructura de Archivos

```
crates/vortex-governance/src/
├── actions/
│   ├── mod.rs           # Trait y re-exports
│   ├── deny.rs          # DenyAction
│   ├── redact.rs        # RedactAction
│   ├── mask.rs          # MaskAction
│   ├── warn.rs          # WarnAction
│   ├── composer.rs      # ActionComposer
│   └── path_matcher.rs  # Property path matching
└── ...
```

---

## Pasos de Implementacion

### Paso 1: Definir Trait GovernanceAction

```rust
// src/actions/mod.rs
use axum::{
    body::Body,
    http::{Response, StatusCode},
};
use serde_json::Value;

pub mod deny;
pub mod redact;
pub mod mask;
pub mod warn;
pub mod composer;
pub mod path_matcher;

pub use deny::DenyAction;
pub use redact::RedactAction;
pub use mask::MaskAction;
pub use warn::WarnAction;
pub use composer::ActionComposer;

/// Result of applying a governance action.
#[derive(Debug)]
pub enum ActionResult {
    /// Request should be denied with this response.
    Deny(Response<Body>),

    /// Response should be modified with this JSON.
    Modified(Value),

    /// Response should have headers added.
    AddHeaders(Vec<(String, String)>),

    /// No modification needed.
    Pass,
}

/// Trait for governance actions.
///
/// Actions implement specific transformations or responses
/// based on policy decisions.
pub trait GovernanceAction: Send + Sync {
    /// Apply the action to a JSON configuration.
    ///
    /// For body-modifying actions (mask, redact), this receives
    /// the config JSON and returns the modified version.
    fn apply_to_config(&self, config: &mut Value);

    /// Apply the action to an HTTP response.
    ///
    /// For response-level actions (deny, warn), this receives
    /// the full response and can modify or replace it.
    fn apply_to_response(&self, response: Response<Body>) -> ActionResult {
        ActionResult::Pass
    }

    /// Check if this action terminates the request (like deny).
    fn is_terminal(&self) -> bool {
        false
    }

    /// Get the priority of this action (higher = applied first).
    fn priority(&self) -> u32 {
        100
    }

    /// Get a description of this action for logging.
    fn description(&self) -> String;
}
```

### Paso 2: Implementar DenyAction

```rust
// src/actions/deny.rs
use axum::{
    body::Body,
    http::{Response, StatusCode, header},
    response::IntoResponse,
};
use serde_json::{json, Value};

use super::{GovernanceAction, ActionResult};

/// Action that denies the request entirely.
///
/// Returns a 403 Forbidden response with a configurable message.
#[derive(Debug, Clone)]
pub struct DenyAction {
    /// Message to include in the response body.
    pub message: String,
    /// Additional details for logging (not exposed to client).
    pub reason: Option<String>,
}

impl DenyAction {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            reason: None,
        }
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl GovernanceAction for DenyAction {
    fn apply_to_config(&self, _config: &mut Value) {
        // Deny doesn't modify config - it blocks the request
    }

    fn apply_to_response(&self, _response: Response<Body>) -> ActionResult {
        let body = json!({
            "error": "Forbidden",
            "message": self.message
        });

        let response = Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        ActionResult::Deny(response)
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn priority(&self) -> u32 {
        1000 // Highest priority - deny first
    }

    fn description(&self) -> String {
        format!("Deny: {}", self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deny_is_terminal() {
        let action = DenyAction::new("Access denied");
        assert!(action.is_terminal());
    }

    #[test]
    fn test_deny_creates_403_response() {
        let action = DenyAction::new("Not allowed");
        let response = Response::new(Body::empty());

        match action.apply_to_response(response) {
            ActionResult::Deny(resp) => {
                assert_eq!(resp.status(), StatusCode::FORBIDDEN);
            }
            _ => panic!("Expected Deny result"),
        }
    }
}
```

### Paso 3: Implementar Property Path Matcher

```rust
// src/actions/path_matcher.rs
use glob_match::glob_match;

/// Matches property paths against patterns.
///
/// Supports:
/// - Exact match: "database.password"
/// - Wildcard: "*.password" matches "database.password", "cache.password"
/// - Deep wildcard: "**.password" matches any depth
/// - Multiple wildcards: "*.secrets.*"
pub struct PathMatcher {
    patterns: Vec<String>,
}

impl PathMatcher {
    pub fn new(patterns: Vec<String>) -> Self {
        Self { patterns }
    }

    /// Check if a property path matches any pattern.
    pub fn matches(&self, path: &str) -> bool {
        self.patterns.iter().any(|pattern| {
            self.match_pattern(pattern, path)
        })
    }

    fn match_pattern(&self, pattern: &str, path: &str) -> bool {
        // Handle ** for deep matching
        if pattern.contains("**") {
            let pattern = pattern.replace("**", "*");
            return glob_match(&pattern, path);
        }

        // Handle * for single segment matching
        if pattern.contains('*') {
            return glob_match(pattern, path);
        }

        // Exact match
        pattern == path
    }

    /// Get all paths from a JSON value that match the patterns.
    pub fn find_matching_paths(&self, json: &serde_json::Value) -> Vec<String> {
        let mut paths = Vec::new();
        self.collect_paths(json, String::new(), &mut paths);
        paths.into_iter().filter(|p| self.matches(p)).collect()
    }

    fn collect_paths(
        &self,
        value: &serde_json::Value,
        prefix: String,
        paths: &mut Vec<String>,
    ) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, val) in map {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", prefix, key)
                    };

                    paths.push(path.clone());
                    self.collect_paths(val, path, paths);
                }
            }
            serde_json::Value::Array(arr) => {
                for (i, val) in arr.iter().enumerate() {
                    let path = format!("{}[{}]", prefix, i);
                    self.collect_paths(val, path, paths);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_exact_match() {
        let matcher = PathMatcher::new(vec!["database.password".to_string()]);
        assert!(matcher.matches("database.password"));
        assert!(!matcher.matches("database.url"));
    }

    #[test]
    fn test_wildcard_match() {
        let matcher = PathMatcher::new(vec!["*.password".to_string()]);
        assert!(matcher.matches("database.password"));
        assert!(matcher.matches("cache.password"));
        assert!(!matcher.matches("database.url"));
    }

    #[test]
    fn test_find_matching_paths() {
        let matcher = PathMatcher::new(vec!["*.password".to_string()]);
        let json = json!({
            "database": {
                "url": "postgres://localhost",
                "password": "secret"
            },
            "cache": {
                "password": "also_secret"
            }
        });

        let matches = matcher.find_matching_paths(&json);
        assert!(matches.contains(&"database.password".to_string()));
        assert!(matches.contains(&"cache.password".to_string()));
        assert!(!matches.contains(&"database.url".to_string()));
    }
}
```

### Paso 4: Implementar RedactAction

```rust
// src/actions/redact.rs
use serde_json::Value;

use super::{GovernanceAction, ActionResult};
use super::path_matcher::PathMatcher;

/// Action that removes sensitive properties from the configuration.
///
/// Redacted properties are completely removed from the JSON response.
#[derive(Debug, Clone)]
pub struct RedactAction {
    /// Path patterns to redact.
    patterns: Vec<String>,
    /// Compiled matcher.
    matcher: PathMatcher,
}

impl RedactAction {
    pub fn new(patterns: Vec<String>) -> Self {
        let matcher = PathMatcher::new(patterns.clone());
        Self { patterns, matcher }
    }

    /// Redact a value at a specific path.
    fn redact_path(&self, json: &mut Value, path: &str) {
        let parts: Vec<&str> = path.split('.').collect();
        self.redact_recursive(json, &parts);
    }

    fn redact_recursive(&self, value: &mut Value, path: &[&str]) {
        if path.is_empty() {
            return;
        }

        if path.len() == 1 {
            // Last segment - remove the key
            if let Value::Object(map) = value {
                map.remove(path[0]);
            }
            return;
        }

        // Navigate deeper
        if let Value::Object(map) = value {
            if let Some(next) = map.get_mut(path[0]) {
                self.redact_recursive(next, &path[1..]);
            }
        }
    }
}

impl GovernanceAction for RedactAction {
    fn apply_to_config(&self, config: &mut Value) {
        let paths_to_redact = self.matcher.find_matching_paths(config);

        for path in paths_to_redact {
            self.redact_path(config, &path);
        }
    }

    fn priority(&self) -> u32 {
        500 // After deny, before warn
    }

    fn description(&self) -> String {
        format!("Redact: {:?}", self.patterns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_redact_single_property() {
        let action = RedactAction::new(vec!["database.password".to_string()]);
        let mut config = json!({
            "database": {
                "url": "postgres://localhost",
                "password": "secret123"
            }
        });

        action.apply_to_config(&mut config);

        assert!(config["database"]["url"].is_string());
        assert!(config["database"]["password"].is_null());
    }

    #[test]
    fn test_redact_wildcard() {
        let action = RedactAction::new(vec!["*.password".to_string()]);
        let mut config = json!({
            "database": {
                "password": "db_secret"
            },
            "cache": {
                "password": "cache_secret"
            },
            "app": {
                "name": "myapp"
            }
        });

        action.apply_to_config(&mut config);

        assert!(config["database"]["password"].is_null());
        assert!(config["cache"]["password"].is_null());
        assert_eq!(config["app"]["name"], "myapp");
    }

    #[test]
    fn test_redact_preserves_structure() {
        let action = RedactAction::new(vec!["secret".to_string()]);
        let mut config = json!({
            "public": "visible",
            "secret": "hidden"
        });

        action.apply_to_config(&mut config);

        assert_eq!(config["public"], "visible");
        assert!(!config.as_object().unwrap().contains_key("secret"));
    }
}
```

### Paso 5: Implementar MaskAction

```rust
// src/actions/mask.rs
use serde_json::Value;

use super::{GovernanceAction, ActionResult};
use super::path_matcher::PathMatcher;

/// Action that masks sensitive values in the configuration.
///
/// Values are replaced with mask characters, optionally preserving
/// some visible characters at the end.
#[derive(Debug, Clone)]
pub struct MaskAction {
    /// Path patterns to mask.
    patterns: Vec<String>,
    /// Compiled matcher.
    matcher: PathMatcher,
    /// Character to use for masking.
    mask_char: char,
    /// Number of characters to leave visible at the end.
    visible_chars: usize,
}

impl MaskAction {
    pub fn new(patterns: Vec<String>) -> Self {
        let matcher = PathMatcher::new(patterns.clone());
        Self {
            patterns,
            matcher,
            mask_char: '*',
            visible_chars: 0,
        }
    }

    pub fn with_mask_char(mut self, c: char) -> Self {
        self.mask_char = c;
        self
    }

    pub fn with_visible_chars(mut self, n: usize) -> Self {
        self.visible_chars = n;
        self
    }

    /// Mask a string value.
    fn mask_value(&self, value: &str) -> String {
        let len = value.len();

        if len <= self.visible_chars {
            // Everything visible if value is short
            return self.mask_char.to_string().repeat(len);
        }

        let masked_len = len.saturating_sub(self.visible_chars);
        let masked_part = self.mask_char.to_string().repeat(masked_len);

        if self.visible_chars > 0 {
            let visible_part: String = value.chars().skip(masked_len).collect();
            format!("{}{}", masked_part, visible_part)
        } else {
            masked_part
        }
    }

    /// Recursively mask matching values in JSON.
    fn mask_recursive(&self, value: &mut Value, current_path: &str) {
        match value {
            Value::Object(map) => {
                for (key, val) in map.iter_mut() {
                    let path = if current_path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", current_path, key)
                    };

                    if self.matcher.matches(&path) {
                        if let Value::String(s) = val {
                            *s = self.mask_value(s);
                        }
                    }

                    self.mask_recursive(val, &path);
                }
            }
            Value::Array(arr) => {
                for (i, val) in arr.iter_mut().enumerate() {
                    let path = format!("{}[{}]", current_path, i);
                    self.mask_recursive(val, &path);
                }
            }
            _ => {}
        }
    }
}

impl GovernanceAction for MaskAction {
    fn apply_to_config(&self, config: &mut Value) {
        self.mask_recursive(config, "");
    }

    fn priority(&self) -> u32 {
        400 // After redact
    }

    fn description(&self) -> String {
        format!(
            "Mask: {:?} (char={}, visible={})",
            self.patterns, self.mask_char, self.visible_chars
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_mask_full() {
        let action = MaskAction::new(vec!["password".to_string()]);
        let mut config = json!({
            "password": "supersecret"
        });

        action.apply_to_config(&mut config);

        assert_eq!(config["password"], "***********");
    }

    #[test]
    fn test_mask_with_visible_chars() {
        let action = MaskAction::new(vec!["password".to_string()])
            .with_visible_chars(4);

        let mut config = json!({
            "password": "supersecret123"
        });

        action.apply_to_config(&mut config);

        let masked = config["password"].as_str().unwrap();
        assert!(masked.ends_with("t123"));
        assert!(masked.starts_with("**********"));
    }

    #[test]
    fn test_mask_custom_char() {
        let action = MaskAction::new(vec!["password".to_string()])
            .with_mask_char('#');

        let mut config = json!({
            "password": "secret"
        });

        action.apply_to_config(&mut config);

        assert_eq!(config["password"], "######");
    }

    #[test]
    fn test_mask_nested_paths() {
        let action = MaskAction::new(vec!["*.password".to_string()])
            .with_visible_chars(2);

        let mut config = json!({
            "database": {
                "password": "dbpass123"
            },
            "redis": {
                "password": "redispass"
            }
        });

        action.apply_to_config(&mut config);

        assert!(config["database"]["password"].as_str().unwrap().ends_with("23"));
        assert!(config["redis"]["password"].as_str().unwrap().ends_with("ss"));
    }

    #[test]
    fn test_mask_short_value() {
        let action = MaskAction::new(vec!["pin".to_string()])
            .with_visible_chars(4);

        let mut config = json!({
            "pin": "12"  // Shorter than visible_chars
        });

        action.apply_to_config(&mut config);

        // Should mask everything since value is too short
        assert_eq!(config["pin"], "**");
    }
}
```

### Paso 6: Implementar WarnAction

```rust
// src/actions/warn.rs
use axum::{
    body::Body,
    http::Response,
};
use serde_json::Value;

use super::{GovernanceAction, ActionResult};

/// Action that adds warning headers to the response.
///
/// The request is allowed, but the client receives warnings
/// about governance policies that matched.
#[derive(Debug, Clone)]
pub struct WarnAction {
    /// Warning message to include.
    pub message: String,
    /// Header name for the warning.
    pub header_name: String,
}

impl WarnAction {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            header_name: "X-Governance-Warning".to_string(),
        }
    }

    pub fn with_header(mut self, header_name: impl Into<String>) -> Self {
        self.header_name = header_name.into();
        self
    }
}

impl GovernanceAction for WarnAction {
    fn apply_to_config(&self, _config: &mut Value) {
        // Warn doesn't modify the config
    }

    fn apply_to_response(&self, response: Response<Body>) -> ActionResult {
        ActionResult::AddHeaders(vec![
            (self.header_name.clone(), self.message.clone())
        ])
    }

    fn priority(&self) -> u32 {
        100 // Lowest priority - applied last
    }

    fn description(&self) -> String {
        format!("Warn: {}", self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_warn_not_terminal() {
        let action = WarnAction::new("This is deprecated");
        assert!(!action.is_terminal());
    }

    #[test]
    fn test_warn_adds_headers() {
        let action = WarnAction::new("Deprecated configuration");
        let response = Response::new(Body::empty());

        match action.apply_to_response(response) {
            ActionResult::AddHeaders(headers) => {
                assert_eq!(headers.len(), 1);
                assert_eq!(headers[0].0, "X-Governance-Warning");
                assert_eq!(headers[0].1, "Deprecated configuration");
            }
            _ => panic!("Expected AddHeaders result"),
        }
    }

    #[test]
    fn test_warn_custom_header() {
        let action = WarnAction::new("Warning!")
            .with_header("X-Custom-Warning");

        let response = Response::new(Body::empty());

        match action.apply_to_response(response) {
            ActionResult::AddHeaders(headers) => {
                assert_eq!(headers[0].0, "X-Custom-Warning");
            }
            _ => panic!("Expected AddHeaders result"),
        }
    }
}
```

### Paso 7: Implementar ActionComposer

```rust
// src/actions/composer.rs
use axum::{
    body::Body,
    http::{Response, HeaderValue},
};
use serde_json::Value;
use tracing::{debug, info};

use super::{GovernanceAction, ActionResult};
use crate::plac::{AppliedAction, ActionType, Action};

/// Composes and applies multiple governance actions.
///
/// The composer ensures actions are applied in the correct order
/// and handles the accumulation of non-terminal actions.
pub struct ActionComposer {
    actions: Vec<Box<dyn GovernanceAction>>,
}

impl ActionComposer {
    /// Create a new composer from applied actions.
    pub fn from_applied_actions(applied: &[AppliedAction]) -> Self {
        let mut actions: Vec<Box<dyn GovernanceAction>> = applied
            .iter()
            .map(|a| Self::create_action(&a.action))
            .collect();

        // Sort by priority (highest first)
        actions.sort_by(|a, b| b.priority().cmp(&a.priority()));

        Self { actions }
    }

    /// Create a concrete action from an Action definition.
    fn create_action(action: &Action) -> Box<dyn GovernanceAction> {
        match &action.action_type {
            ActionType::Deny => {
                let message = action.message
                    .clone()
                    .unwrap_or_else(|| "Access denied".to_string());
                Box::new(super::DenyAction::new(message))
            }

            ActionType::Redact => {
                let properties = action.properties
                    .clone()
                    .unwrap_or_default();
                Box::new(super::RedactAction::new(properties))
            }

            ActionType::Mask => {
                let mut mask = super::MaskAction::new(
                    action.properties.clone().unwrap_or_else(|| {
                        // Default patterns for masking
                        vec![
                            "*.password".to_string(),
                            "*.secret".to_string(),
                            "*.api_key".to_string(),
                            "*.token".to_string(),
                        ]
                    })
                );

                if let Some(c) = action.mask_char {
                    mask = mask.with_mask_char(c);
                }
                if let Some(n) = action.visible_chars {
                    mask = mask.with_visible_chars(n);
                }

                Box::new(mask)
            }

            ActionType::Warn => {
                let message = action.message
                    .clone()
                    .unwrap_or_else(|| "Governance warning".to_string());
                Box::new(super::WarnAction::new(message))
            }
        }
    }

    /// Apply all actions to a JSON configuration.
    pub fn apply_to_config(&self, config: &mut Value) {
        for action in &self.actions {
            debug!(action = %action.description(), "Applying action to config");
            action.apply_to_config(config);
        }
    }

    /// Apply all actions to an HTTP response.
    pub async fn apply_to_response(
        &self,
        mut response: Response<Body>,
    ) -> Response<Body> {
        let mut headers_to_add = Vec::new();

        for action in &self.actions {
            debug!(action = %action.description(), "Applying action to response");

            match action.apply_to_response(response) {
                ActionResult::Deny(deny_response) => {
                    // Terminal action - return immediately
                    info!(action = %action.description(), "Request denied");
                    return deny_response;
                }

                ActionResult::AddHeaders(headers) => {
                    headers_to_add.extend(headers);
                    // Can't continue with original response since we moved it
                    // Need to reconstruct
                }

                ActionResult::Modified(_) => {
                    // Body modification handled in apply_to_config
                }

                ActionResult::Pass => {
                    // No modification
                }
            }

            // Reconstruct response for next iteration
            // (In real implementation, avoid this by not moving)
        }

        // Add accumulated headers
        for (name, value) in headers_to_add {
            if let Ok(hv) = HeaderValue::from_str(&value) {
                response.headers_mut().append(
                    axum::http::header::HeaderName::try_from(name).unwrap(),
                    hv,
                );
            }
        }

        response
    }

    /// Transform a full response (body + headers).
    pub async fn transform_response(
        &self,
        response: Response<Body>,
    ) -> Response<Body> {
        let (mut parts, body) = response.into_parts();

        // Read and parse body
        let bytes = match axum::body::to_bytes(body, usize::MAX).await {
            Ok(b) => b,
            Err(_) => return Response::from_parts(parts, Body::empty()),
        };

        // Try to parse as JSON and apply config transformations
        let body = match serde_json::from_slice::<Value>(&bytes) {
            Ok(mut json) => {
                self.apply_to_config(&mut json);
                Body::from(serde_json::to_vec(&json).unwrap_or_else(|_| bytes.to_vec()))
            }
            Err(_) => Body::from(bytes),
        };

        // Apply response-level actions (headers)
        let mut headers_to_add = Vec::new();
        for action in &self.actions {
            if let ActionResult::AddHeaders(headers) = action.apply_to_response(
                Response::new(Body::empty())
            ) {
                headers_to_add.extend(headers);
            }
        }

        // Add headers
        for (name, value) in headers_to_add {
            if let (Ok(header_name), Ok(header_value)) = (
                axum::http::header::HeaderName::try_from(name),
                HeaderValue::from_str(&value),
            ) {
                parts.headers.append(header_name, header_value);
            }
        }

        Response::from_parts(parts, body)
    }

    /// Check if any action is terminal (deny).
    pub fn has_terminal_action(&self) -> bool {
        self.actions.iter().any(|a| a.is_terminal())
    }

    /// Get number of actions.
    pub fn action_count(&self) -> usize {
        self.actions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_applied_actions() -> Vec<AppliedAction> {
        vec![
            AppliedAction {
                policy_name: "mask-secrets".to_string(),
                priority: 100,
                action: Action {
                    action_type: ActionType::Mask,
                    properties: Some(vec!["*.password".to_string()]),
                    mask_char: Some('*'),
                    visible_chars: Some(4),
                    message: None,
                },
            },
            AppliedAction {
                policy_name: "warn-deprecated".to_string(),
                priority: 50,
                action: Action {
                    action_type: ActionType::Warn,
                    message: Some("This is deprecated".to_string()),
                    properties: None,
                    mask_char: None,
                    visible_chars: None,
                },
            },
        ]
    }

    #[test]
    fn test_composer_applies_actions_in_order() {
        let actions = create_test_applied_actions();
        let composer = ActionComposer::from_applied_actions(&actions);

        let mut config = json!({
            "database": {
                "password": "supersecret123"
            }
        });

        composer.apply_to_config(&mut config);

        // Password should be masked
        let password = config["database"]["password"].as_str().unwrap();
        assert!(password.contains("*"));
        assert!(password.ends_with("t123"));
    }

    #[test]
    fn test_composer_counts_actions() {
        let actions = create_test_applied_actions();
        let composer = ActionComposer::from_applied_actions(&actions);

        assert_eq!(composer.action_count(), 2);
    }
}
```

---

## Conceptos de Rust Aprendidos

### 1. Strategy Pattern con Traits

El Strategy Pattern permite intercambiar algoritmos en runtime.

**Rust:**
```rust
/// Trait define la estrategia
pub trait GovernanceAction: Send + Sync {
    fn apply_to_config(&self, config: &mut Value);
    fn is_terminal(&self) -> bool { false }
    fn priority(&self) -> u32 { 100 }
}

/// Diferentes implementaciones
struct DenyAction { message: String }
struct MaskAction { patterns: Vec<String> }
struct WarnAction { message: String }

impl GovernanceAction for DenyAction { /* ... */ }
impl GovernanceAction for MaskAction { /* ... */ }
impl GovernanceAction for WarnAction { /* ... */ }

/// Uso con trait objects
struct ActionComposer {
    actions: Vec<Box<dyn GovernanceAction>>,
}

impl ActionComposer {
    fn apply(&self, config: &mut Value) {
        for action in &self.actions {
            action.apply_to_config(config);  // Dispatch dinamico
        }
    }
}
```

**Java:**
```java
interface GovernanceAction {
    void applyToConfig(JsonNode config);
    default boolean isTerminal() { return false; }
    default int priority() { return 100; }
}

class DenyAction implements GovernanceAction {
    private final String message;
    // ...
}

class MaskAction implements GovernanceAction {
    private final List<String> patterns;
    // ...
}

class ActionComposer {
    private final List<GovernanceAction> actions;

    public void apply(JsonNode config) {
        for (GovernanceAction action : actions) {
            action.applyToConfig(config);
        }
    }
}
```

**Diferencias:**
| Aspecto | Rust | Java |
|---------|------|------|
| Trait/Interface | `dyn Trait` | Interface |
| Allocation | `Box<dyn T>` explicito | Implicito (heap) |
| Thread safety | `Send + Sync` bounds | No enforced |
| Default impl | En trait | `default` methods |

### 2. Transformacion Recursiva de JSON

Modificar JSON anidado recursivamente.

**Rust:**
```rust
fn mask_recursive(&self, value: &mut Value, path: &str) {
    match value {
        Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                let new_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", path, key)
                };

                // Verificar si este path debe enmascararse
                if self.should_mask(&new_path) {
                    if let Value::String(s) = val {
                        *s = self.mask_value(s);  // Mutar in-place
                    }
                }

                // Continuar recursivamente
                self.mask_recursive(val, &new_path);
            }
        }
        Value::Array(arr) => {
            for (i, val) in arr.iter_mut().enumerate() {
                let new_path = format!("{}[{}]", path, i);
                self.mask_recursive(val, &new_path);
            }
        }
        _ => {}
    }
}
```

**Java:**
```java
private void maskRecursive(JsonNode node, String path) {
    if (node.isObject()) {
        ObjectNode obj = (ObjectNode) node;
        Iterator<Map.Entry<String, JsonNode>> fields = obj.fields();
        while (fields.hasNext()) {
            var entry = fields.next();
            String newPath = path.isEmpty() ? entry.getKey()
                           : path + "." + entry.getKey();

            if (shouldMask(newPath) && entry.getValue().isTextual()) {
                obj.put(entry.getKey(), maskValue(entry.getValue().asText()));
            }

            maskRecursive(entry.getValue(), newPath);
        }
    } else if (node.isArray()) {
        ArrayNode arr = (ArrayNode) node;
        for (int i = 0; i < arr.size(); i++) {
            maskRecursive(arr.get(i), path + "[" + i + "]");
        }
    }
}
```

### 3. Builder Pattern con Self-Returning Methods

Metodos que retornan `Self` para chaining.

**Rust:**
```rust
pub struct MaskAction {
    patterns: Vec<String>,
    mask_char: char,
    visible_chars: usize,
}

impl MaskAction {
    pub fn new(patterns: Vec<String>) -> Self {
        Self {
            patterns,
            mask_char: '*',
            visible_chars: 0,
        }
    }

    // Cada metodo consume self y retorna Self modificado
    pub fn with_mask_char(mut self, c: char) -> Self {
        self.mask_char = c;
        self
    }

    pub fn with_visible_chars(mut self, n: usize) -> Self {
        self.visible_chars = n;
        self
    }
}

// Uso fluido
let action = MaskAction::new(vec!["*.password".to_string()])
    .with_mask_char('#')
    .with_visible_chars(4);
```

### 4. Factory Method con Match

Crear instancias basadas en tipo con match.

**Rust:**
```rust
fn create_action(action: &Action) -> Box<dyn GovernanceAction> {
    match &action.action_type {
        ActionType::Deny => {
            let message = action.message.clone()
                .unwrap_or_else(|| "Access denied".to_string());
            Box::new(DenyAction::new(message))
        }
        ActionType::Redact => {
            let properties = action.properties.clone().unwrap_or_default();
            Box::new(RedactAction::new(properties))
        }
        ActionType::Mask => {
            let mut mask = MaskAction::new(
                action.properties.clone().unwrap_or_default()
            );
            if let Some(c) = action.mask_char {
                mask = mask.with_mask_char(c);
            }
            Box::new(mask)
        }
        ActionType::Warn => {
            let message = action.message.clone()
                .unwrap_or_else(|| "Warning".to_string());
            Box::new(WarnAction::new(message))
        }
    }
}
```

---

## Riesgos y Errores Comunes

### 1. Orden de Acciones Incorrecto

```rust
// MAL: Warn antes de Mask (warn no tiene efecto si mask falla)
let actions = vec![warn, mask];

// BIEN: Ordenar por prioridad
actions.sort_by(|a, b| b.priority().cmp(&a.priority()));
// Resultado: [deny(1000), redact(500), mask(400), warn(100)]
```

### 2. Mutacion Parcial en Error

```rust
// MAL: Config queda parcialmente modificada si hay error
fn apply_all(&self, config: &mut Value) -> Result<(), Error> {
    for action in &self.actions {
        action.apply(config)?;  // Si falla aqui, config esta a medias
    }
    Ok(())
}

// BIEN: Clone antes de modificar
fn apply_all(&self, config: &Value) -> Result<Value, Error> {
    let mut modified = config.clone();
    for action in &self.actions {
        action.apply(&mut modified)?;
    }
    Ok(modified)  // Solo retorna si todo OK
}
```

### 3. Mascara Recuperable

```rust
// MAL: Mascara deja informacion recuperable
fn mask(&self, value: &str) -> String {
    format!("{}****", &value[..4])  // Primeros 4 chars visibles!
}

// BIEN: Ocultar principio, mostrar final
fn mask(&self, value: &str) -> String {
    let visible = self.visible_chars;
    let masked_len = value.len().saturating_sub(visible);
    let masked = "*".repeat(masked_len);
    let visible_part: String = value.chars().skip(masked_len).collect();
    format!("{}{}", masked, visible_part)
}
```

### 4. Headers Duplicados

```rust
// MAL: Sobrescribe headers existentes
response.headers_mut().insert(name, value);

// BIEN: Append para permitir multiples valores
response.headers_mut().append(name, value);
```

---

## Pruebas

### Tests de Integracion

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_full_pipeline() {
        let actions = vec![
            AppliedAction {
                policy_name: "mask".to_string(),
                priority: 100,
                action: Action {
                    action_type: ActionType::Mask,
                    properties: Some(vec!["*.password".to_string()]),
                    mask_char: Some('*'),
                    visible_chars: Some(4),
                    message: None,
                },
            },
            AppliedAction {
                policy_name: "warn".to_string(),
                priority: 50,
                action: Action {
                    action_type: ActionType::Warn,
                    message: Some("Config is deprecated".to_string()),
                    properties: None,
                    mask_char: None,
                    visible_chars: None,
                },
            },
        ];

        let composer = ActionComposer::from_applied_actions(&actions);

        let config = json!({
            "database": {
                "url": "postgres://localhost",
                "password": "supersecret123"
            }
        });

        let body = Body::from(serde_json::to_vec(&config).unwrap());
        let response = Response::builder()
            .status(200)
            .body(body)
            .unwrap();

        let result = composer.transform_response(response).await;

        // Check headers
        assert!(result.headers().contains_key("x-governance-warning"));

        // Check body
        let bytes = axum::body::to_bytes(result.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();

        let password = json["database"]["password"].as_str().unwrap();
        assert!(password.starts_with("*"));
        assert!(password.ends_with("t123"));
    }

    #[tokio::test]
    async fn test_deny_stops_pipeline() {
        let actions = vec![
            AppliedAction {
                policy_name: "deny".to_string(),
                priority: 1000,
                action: Action {
                    action_type: ActionType::Deny,
                    message: Some("Blocked".to_string()),
                    properties: None,
                    mask_char: None,
                    visible_chars: None,
                },
            },
            AppliedAction {
                policy_name: "mask".to_string(),
                priority: 100,
                action: Action {
                    action_type: ActionType::Mask,
                    properties: Some(vec!["*.password".to_string()]),
                    mask_char: None,
                    visible_chars: None,
                    message: None,
                },
            },
        ];

        let composer = ActionComposer::from_applied_actions(&actions);
        assert!(composer.has_terminal_action());
    }
}
```

### Tests de Path Matching

```rust
#[cfg(test)]
mod path_tests {
    use super::path_matcher::PathMatcher;
    use serde_json::json;

    #[test]
    fn test_complex_patterns() {
        let matcher = PathMatcher::new(vec![
            "database.*.password".to_string(),
            "services.*.credentials.api_key".to_string(),
        ]);

        let json = json!({
            "database": {
                "primary": {
                    "url": "...",
                    "password": "secret1"
                },
                "replica": {
                    "url": "...",
                    "password": "secret2"
                }
            },
            "services": {
                "payment": {
                    "credentials": {
                        "api_key": "pk_test_123"
                    }
                }
            }
        });

        let matches = matcher.find_matching_paths(&json);

        assert!(matches.contains(&"database.primary.password".to_string()));
        assert!(matches.contains(&"database.replica.password".to_string()));
        assert!(matches.contains(&"services.payment.credentials.api_key".to_string()));
    }
}
```

---

## Seguridad

- **Deny primero**: Acciones deny tienen prioridad maxima
- **Mascara irreversible**: Valores mascarados no pueden recuperarse
- **No leak en errores**: Errores no revelan valores originales
- **Audit trail**: Todas las acciones se loguean
- **Inmutabilidad**: Config original no se modifica, se crea copia

---

## Entregable Final

### Archivos Creados

1. `src/actions/mod.rs` - Trait y re-exports
2. `src/actions/deny.rs` - DenyAction
3. `src/actions/redact.rs` - RedactAction
4. `src/actions/mask.rs` - MaskAction
5. `src/actions/warn.rs` - WarnAction
6. `src/actions/composer.rs` - ActionComposer
7. `src/actions/path_matcher.rs` - PathMatcher

### Verificacion

```bash
# Compilar
cargo build -p vortex-governance

# Tests
cargo test -p vortex-governance -- actions

# Test manual con curl
curl http://localhost:8080/myapp/production

# Respuesta con mascara:
# {
#   "database": {
#     "url": "postgres://localhost",
#     "password": "**********t123"
#   }
# }
# Header: X-Governance-Warning: Production config accessed
```

### Ejemplo Completo de Politica

```yaml
policies:
  - name: mask-production-secrets
    description: Mask all secrets in production
    priority: 100
    conditions:
      - field: profile
        operator: in
        value: "production,prod"
    action:
      type: mask
      mask_char: "*"
      visible_chars: 4
      properties:
        - "*.password"
        - "*.secret"
        - "*.api_key"
        - "*.token"
        - "credentials.*"

  - name: redact-internal-only
    description: Remove internal properties from external access
    priority: 150
    conditions:
      - field: source_ip
        operator: not_in_cidr
        value: "10.0.0.0/8"
    action:
      type: redact
      properties:
        - "internal.*"
        - "debug.*"
        - "*.internal_*"

  - name: warn-deprecated-app
    description: Warn when accessing deprecated application configs
    priority: 50
    conditions:
      - field: application
        operator: matches
        value: "legacy-.*"
    action:
      type: warn
      message: "This application is deprecated and will be removed"
```
