# Historia 006: Compliance Rules Engine

## Contexto y Objetivo

Esta historia implementa un motor de reglas para validar configuraciones contra estandares de cumplimiento como PCI-DSS y SOC2. El motor permite:

- **Definir reglas declarativas**: Sin codigo, solo estructuras de datos
- **Evaluar configuraciones**: Detectar violaciones automaticamente
- **Reportar resultados**: Generar informes estructurados
- **Categorizar por severidad**: Critical, High, Medium, Low

Para desarrolladores Java, esto es similar a un rule engine como Drools, pero mas ligero y enfocado en validacion de configuracion.

El objetivo es automatizar la deteccion de problemas de seguridad como:
- Passwords en texto plano
- Credenciales en connection strings
- Endpoints HTTP inseguros
- Configuraciones de cifrado debiles

---

## Alcance

### In Scope

- `Rule` struct para definir reglas de compliance
- `ComplianceEngine` para evaluar reglas contra configuraciones
- Reglas predefinidas para PCI-DSS y SOC2
- Operadores de pattern matching: path globbing, regex, contains
- `Violation` struct para representar violaciones
- Severidades: Critical, High, Medium, Low, Info

### Out of Scope

- API REST de compliance (historia 007)
- Remediacion automatica
- Integracion con sistemas de compliance externos
- UI de administracion de reglas

---

## Criterios de Aceptacion

- [ ] Reglas definidas como YAML/JSON
- [ ] Path matching con globbing (**.password)
- [ ] Regex matching para valores
- [ ] Condition types: must_exist, must_not_exist, must_match, must_not_match
- [ ] Severidades correctamente asignadas
- [ ] Violaciones incluyen path, rule_id, severity, message
- [ ] Reglas PCI-DSS basicas implementadas
- [ ] Reglas SOC2 basicas implementadas
- [ ] Tests para cada regla predefinida

---

## Diseno Propuesto

### Arquitectura

```
┌─────────────────────────────────────────────────────────────────────┐
│                       ComplianceEngine                               │
├─────────────────────────────────────────────────────────────────────┤
│  rules: Vec<Rule>                                                    │
│  standards: HashMap<Standard, Vec<Rule>>                             │
├─────────────────────────────────────────────────────────────────────┤
│  + evaluate(config) -> ComplianceReport                              │
│  + add_rule(rule)                                                    │
│  + load_standard(standard)                                           │
│  - evaluate_rule(rule, config) -> Vec<Violation>                     │
│  - match_paths(pattern, config) -> Vec<(Path, Value)>                │
└─────────────────────────────────────────────────────────────────────┘
                    │
        ┌───────────┴───────────┐
        ▼                       ▼
┌───────────────────┐   ┌───────────────────────────────────────────┐
│       Rule        │   │            ComplianceReport               │
├───────────────────┤   ├───────────────────────────────────────────┤
│ id: String        │   │ status: Status (Passed/Failed)            │
│ name: String      │   │ violations: Vec<Violation>                │
│ description: String│   │ rules_evaluated: usize                    │
│ standard: Standard │   │ checked_at: DateTime                      │
│ severity: Severity │   │ summary: SeveritySummary                  │
│ condition: Condition│   └───────────────────────────────────────────┘
│ path_pattern: String│
│ value_pattern: Option│
└───────────────────┘
        │
        ▼
┌───────────────────────────────────────────────────────────────────┐
│                          Condition                                 │
├───────────────────────────────────────────────────────────────────┤
│ MustExist          - Path must exist                              │
│ MustNotExist       - Path must not exist                          │
│ MustMatch(regex)   - Value must match regex                       │
│ MustNotMatch(regex)- Value must not match regex                   │
│ MustBeEncrypted    - Value must be encrypted/reference            │
│ MustUseHttps       - URL must use HTTPS                           │
│ MustNotContain(str)- Value must not contain string                │
└───────────────────────────────────────────────────────────────────┘
```

### Flujo de Evaluacion

```
Configuration Input:
{
  "database": {
    "password": "plaintext123",
    "connection_string": "postgres://user:pass@host/db"
  },
  "api": {
    "endpoint": "http://api.example.com"
  }
}

┌────────────────────────────────────────────────────────────────────┐
│                    Compliance Evaluation Flow                       │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. Load Rules                                                      │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │ Rule: no-plaintext-passwords                            │    │
│     │   path: **.password                                     │    │
│     │   condition: must_be_encrypted                          │    │
│     │   severity: CRITICAL                                    │    │
│     │                                                         │    │
│     │ Rule: no-http-endpoints                                 │    │
│     │   path: **.*url*, **.*endpoint*                         │    │
│     │   condition: must_use_https                             │    │
│     │   severity: HIGH                                        │    │
│     │                                                         │    │
│     │ Rule: no-embedded-credentials                           │    │
│     │   path: **.*string*, **.*url*                           │    │
│     │   condition: must_not_match(/:\w+@/)                    │    │
│     │   severity: CRITICAL                                    │    │
│     └─────────────────────────────────────────────────────────┘    │
│                                                                     │
│  2. Match Paths                                                     │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │ **.password matches:                                    │    │
│     │   - database.password                                   │    │
│     │                                                         │    │
│     │ **.*url*, **.*endpoint* matches:                        │    │
│     │   - api.endpoint                                        │    │
│     │                                                         │    │
│     │ **.*string* matches:                                    │    │
│     │   - database.connection_string                          │    │
│     └─────────────────────────────────────────────────────────┘    │
│                                                                     │
│  3. Evaluate Conditions                                             │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │ database.password = "plaintext123"                      │    │
│     │   must_be_encrypted? NO -> VIOLATION                    │    │
│     │                                                         │    │
│     │ api.endpoint = "http://api.example.com"                 │    │
│     │   must_use_https? NO -> VIOLATION                       │    │
│     │                                                         │    │
│     │ database.connection_string = "postgres://user:pass@..."  │    │
│     │   must_not_match(/:\w+@/)? MATCHES -> VIOLATION         │    │
│     └─────────────────────────────────────────────────────────┘    │
│                                                                     │
│  4. Generate Report                                                 │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │ Status: FAILED                                          │    │
│     │ Violations: 3                                           │    │
│     │   CRITICAL: 2                                           │    │
│     │   HIGH: 1                                                │    │
│     └─────────────────────────────────────────────────────────┘    │
│                                                                     │
└────────────────────────────────────────────────────────────────────┘
```

---

## Pasos de Implementacion

### Paso 1: Definir Tipos Base

```rust
// src/compliance/types.rs
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Compliance standard identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Standard {
    PciDss,
    Soc2,
    Hipaa,
    Gdpr,
    Custom,
}

impl std::fmt::Display for Standard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PciDss => write!(f, "PCI-DSS"),
            Self::Soc2 => write!(f, "SOC2"),
            Self::Hipaa => write!(f, "HIPAA"),
            Self::Gdpr => write!(f, "GDPR"),
            Self::Custom => write!(f, "Custom"),
        }
    }
}

/// Severity levels for compliance violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    /// Returns a numeric weight for sorting.
    pub fn weight(&self) -> u8 {
        match self {
            Self::Info => 1,
            Self::Low => 2,
            Self::Medium => 3,
            Self::High => 4,
            Self::Critical => 5,
        }
    }
}

/// Overall compliance status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComplianceStatus {
    /// All rules passed.
    Passed,
    /// At least one rule failed.
    Failed,
    /// Could not complete evaluation.
    Error,
}
```

### Paso 2: Definir Condiciones de Reglas

```rust
// src/compliance/condition.rs
use serde::{Deserialize, Serialize};
use regex::Regex;

/// Condition types for compliance rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    /// Path must exist in configuration.
    MustExist,

    /// Path must not exist in configuration.
    MustNotExist,

    /// Value must match the given regex pattern.
    MustMatch {
        pattern: String,
    },

    /// Value must not match the given regex pattern.
    MustNotMatch {
        pattern: String,
    },

    /// Value must be encrypted or a reference (not plaintext).
    MustBeEncrypted,

    /// URL value must use HTTPS.
    MustUseHttps,

    /// Value must not contain the given substring.
    MustNotContain {
        substring: String,
        #[serde(default)]
        case_sensitive: bool,
    },

    /// Value must be one of the allowed values.
    MustBeOneOf {
        values: Vec<String>,
    },

    /// Numeric value must be within range.
    MustBeInRange {
        min: Option<f64>,
        max: Option<f64>,
    },

    /// All conditions must pass (AND).
    All {
        conditions: Vec<Condition>,
    },

    /// Any condition must pass (OR).
    Any {
        conditions: Vec<Condition>,
    },
}

impl Condition {
    /// Evaluates the condition against a value.
    pub fn evaluate(&self, value: &serde_json::Value) -> ConditionResult {
        match self {
            Condition::MustExist => {
                if value.is_null() {
                    ConditionResult::Failed("Value does not exist".to_string())
                } else {
                    ConditionResult::Passed
                }
            }

            Condition::MustNotExist => {
                if value.is_null() {
                    ConditionResult::Passed
                } else {
                    ConditionResult::Failed("Value should not exist".to_string())
                }
            }

            Condition::MustMatch { pattern } => {
                let str_value = value_to_string(value);
                match Regex::new(pattern) {
                    Ok(re) => {
                        if re.is_match(&str_value) {
                            ConditionResult::Passed
                        } else {
                            ConditionResult::Failed(format!(
                                "Value '{}' does not match pattern '{}'",
                                truncate(&str_value, 50),
                                pattern
                            ))
                        }
                    }
                    Err(e) => ConditionResult::Error(format!("Invalid regex: {}", e)),
                }
            }

            Condition::MustNotMatch { pattern } => {
                let str_value = value_to_string(value);
                match Regex::new(pattern) {
                    Ok(re) => {
                        if re.is_match(&str_value) {
                            ConditionResult::Failed(format!(
                                "Value matches forbidden pattern '{}'",
                                pattern
                            ))
                        } else {
                            ConditionResult::Passed
                        }
                    }
                    Err(e) => ConditionResult::Error(format!("Invalid regex: {}", e)),
                }
            }

            Condition::MustBeEncrypted => {
                let str_value = value_to_string(value);
                if is_encrypted_or_reference(&str_value) {
                    ConditionResult::Passed
                } else {
                    ConditionResult::Failed(
                        "Value appears to be plaintext (should be encrypted or a reference)".to_string()
                    )
                }
            }

            Condition::MustUseHttps => {
                let str_value = value_to_string(value);
                if str_value.starts_with("https://") || !str_value.starts_with("http://") {
                    ConditionResult::Passed
                } else {
                    ConditionResult::Failed(
                        "URL must use HTTPS, not HTTP".to_string()
                    )
                }
            }

            Condition::MustNotContain { substring, case_sensitive } => {
                let str_value = value_to_string(value);
                let contains = if *case_sensitive {
                    str_value.contains(substring)
                } else {
                    str_value.to_lowercase().contains(&substring.to_lowercase())
                };

                if contains {
                    ConditionResult::Failed(format!(
                        "Value contains forbidden substring '{}'",
                        substring
                    ))
                } else {
                    ConditionResult::Passed
                }
            }

            Condition::MustBeOneOf { values } => {
                let str_value = value_to_string(value);
                if values.contains(&str_value) {
                    ConditionResult::Passed
                } else {
                    ConditionResult::Failed(format!(
                        "Value '{}' not in allowed list: {:?}",
                        truncate(&str_value, 30),
                        values
                    ))
                }
            }

            Condition::MustBeInRange { min, max } => {
                let num = match value.as_f64() {
                    Some(n) => n,
                    None => return ConditionResult::Failed("Value is not a number".to_string()),
                };

                if let Some(min_val) = min {
                    if num < *min_val {
                        return ConditionResult::Failed(format!(
                            "Value {} is below minimum {}",
                            num, min_val
                        ));
                    }
                }

                if let Some(max_val) = max {
                    if num > *max_val {
                        return ConditionResult::Failed(format!(
                            "Value {} is above maximum {}",
                            num, max_val
                        ));
                    }
                }

                ConditionResult::Passed
            }

            Condition::All { conditions } => {
                for condition in conditions {
                    match condition.evaluate(value) {
                        ConditionResult::Passed => continue,
                        other => return other,
                    }
                }
                ConditionResult::Passed
            }

            Condition::Any { conditions } => {
                let mut last_failure = None;
                for condition in conditions {
                    match condition.evaluate(value) {
                        ConditionResult::Passed => return ConditionResult::Passed,
                        ConditionResult::Failed(msg) => last_failure = Some(msg),
                        ConditionResult::Error(e) => return ConditionResult::Error(e),
                    }
                }
                ConditionResult::Failed(
                    last_failure.unwrap_or_else(|| "No conditions matched".to_string())
                )
            }
        }
    }
}

/// Result of evaluating a condition.
#[derive(Debug, Clone)]
pub enum ConditionResult {
    Passed,
    Failed(String),
    Error(String),
}

/// Converts a JSON value to a string for pattern matching.
fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        _ => value.to_string(),
    }
}

/// Checks if a value appears to be encrypted or a reference.
fn is_encrypted_or_reference(value: &str) -> bool {
    // Common patterns for encrypted/reference values
    value.starts_with("${")      // Variable reference
        || value.starts_with("vault://")
        || value.starts_with("aws-sm://")
        || value.starts_with("ENC(")
        || value.starts_with("enc:")
        || value.starts_with("ref:")
        || value.starts_with("$ref:")
        || value.chars().all(|c| c.is_ascii_hexdigit() || c == '-')  // UUID/hex
        || is_base64_like(value)
}

fn is_base64_like(value: &str) -> bool {
    value.len() > 20
        && value.chars().all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '=')
        && value.ends_with("=")
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}
```

### Paso 3: Definir Rule y Violation

```rust
// src/compliance/rule.rs
use serde::{Deserialize, Serialize};

use super::condition::Condition;
use super::types::{Severity, Standard};

/// A compliance rule definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// Unique identifier for the rule.
    pub id: String,

    /// Human-readable name.
    pub name: String,

    /// Detailed description.
    pub description: String,

    /// Compliance standard this rule belongs to.
    pub standard: Standard,

    /// Severity of violations.
    pub severity: Severity,

    /// JSON path pattern to match (supports globbing).
    pub path_pattern: String,

    /// Condition to evaluate.
    pub condition: Condition,

    /// Whether this rule is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Tags for categorization.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Remediation guidance.
    #[serde(default)]
    pub remediation: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Rule {
    /// Creates a new rule.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        path_pattern: impl Into<String>,
        condition: Condition,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            standard: Standard::Custom,
            severity: Severity::Medium,
            path_pattern: path_pattern.into(),
            condition,
            enabled: true,
            tags: vec![],
            remediation: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn with_standard(mut self, standard: Standard) -> Self {
        self.standard = standard;
        self
    }

    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }
}

/// A compliance violation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    /// The rule that was violated.
    pub rule_id: String,

    /// Name of the violated rule.
    pub rule_name: String,

    /// JSON path where violation occurred.
    pub path: String,

    /// Severity of the violation.
    pub severity: Severity,

    /// Compliance standard.
    pub standard: Standard,

    /// Description of what went wrong.
    pub message: String,

    /// The actual value (redacted if sensitive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,

    /// Remediation guidance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

impl Violation {
    /// Creates a new violation.
    pub fn new(rule: &Rule, path: String, message: String) -> Self {
        Self {
            rule_id: rule.id.clone(),
            rule_name: rule.name.clone(),
            path,
            severity: rule.severity,
            standard: rule.standard,
            message,
            value: None,
            remediation: rule.remediation.clone(),
        }
    }

    /// Adds a redacted value preview.
    pub fn with_value(mut self, value: &serde_json::Value) -> Self {
        self.value = Some(redact_value(value));
        self
    }
}

/// Redacts potentially sensitive values.
fn redact_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => {
            if s.len() <= 4 {
                "[REDACTED]".to_string()
            } else {
                format!("{}...[REDACTED]", &s[..2])
            }
        }
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        _ => "[OBJECT]".to_string(),
    }
}
```

### Paso 4: Implementar Path Matching

```rust
// src/compliance/path_matcher.rs
use serde_json::Value;

/// Matches JSON paths against a glob pattern.
pub struct PathMatcher {
    pattern: String,
    segments: Vec<PathSegment>,
}

#[derive(Debug, Clone)]
enum PathSegment {
    /// Exact match: "database"
    Exact(String),
    /// Single wildcard: "*"
    SingleWildcard,
    /// Double wildcard: "**"
    DoubleWildcard,
    /// Contains pattern: "*password*"
    Contains(String),
}

impl PathMatcher {
    /// Creates a new path matcher from a pattern.
    pub fn new(pattern: &str) -> Self {
        let segments = pattern
            .split('.')
            .map(|s| {
                if s == "**" {
                    PathSegment::DoubleWildcard
                } else if s == "*" {
                    PathSegment::SingleWildcard
                } else if s.starts_with('*') && s.ends_with('*') && s.len() > 2 {
                    PathSegment::Contains(s[1..s.len() - 1].to_string())
                } else if s.contains('*') {
                    // Simple wildcard matching
                    PathSegment::Contains(s.replace('*', ""))
                } else {
                    PathSegment::Exact(s.to_string())
                }
            })
            .collect();

        Self {
            pattern: pattern.to_string(),
            segments,
        }
    }

    /// Finds all paths in the value that match the pattern.
    pub fn find_matches(&self, value: &Value) -> Vec<(String, Value)> {
        let mut matches = Vec::new();
        self.find_matches_recursive(value, "", &self.segments, &mut matches);
        matches
    }

    fn find_matches_recursive(
        &self,
        value: &Value,
        current_path: &str,
        remaining_segments: &[PathSegment],
        matches: &mut Vec<(String, Value)>,
    ) {
        if remaining_segments.is_empty() {
            matches.push((current_path.to_string(), value.clone()));
            return;
        }

        match value {
            Value::Object(map) => {
                let segment = &remaining_segments[0];
                let rest = &remaining_segments[1..];

                match segment {
                    PathSegment::Exact(name) => {
                        if let Some(v) = map.get(name) {
                            let new_path = if current_path.is_empty() {
                                name.clone()
                            } else {
                                format!("{}.{}", current_path, name)
                            };
                            self.find_matches_recursive(v, &new_path, rest, matches);
                        }
                    }

                    PathSegment::SingleWildcard => {
                        for (key, v) in map {
                            let new_path = if current_path.is_empty() {
                                key.clone()
                            } else {
                                format!("{}.{}", current_path, key)
                            };
                            self.find_matches_recursive(v, &new_path, rest, matches);
                        }
                    }

                    PathSegment::DoubleWildcard => {
                        // Match at current level
                        for (key, v) in map {
                            let new_path = if current_path.is_empty() {
                                key.clone()
                            } else {
                                format!("{}.{}", current_path, key)
                            };

                            // Try matching rest at this level
                            self.find_matches_recursive(v, &new_path, rest, matches);

                            // Also continue with ** at deeper levels
                            self.find_matches_recursive(v, &new_path, remaining_segments, matches);
                        }
                    }

                    PathSegment::Contains(pattern) => {
                        for (key, v) in map {
                            if key.contains(pattern) {
                                let new_path = if current_path.is_empty() {
                                    key.clone()
                                } else {
                                    format!("{}.{}", current_path, key)
                                };
                                self.find_matches_recursive(v, &new_path, rest, matches);
                            }
                        }
                    }
                }
            }

            Value::Array(arr) => {
                for (i, v) in arr.iter().enumerate() {
                    let new_path = format!("{}[{}]", current_path, i);
                    self.find_matches_recursive(v, &new_path, remaining_segments, matches);
                }
            }

            // Leaf values - only match if no more segments
            _ => {
                if remaining_segments.is_empty() {
                    matches.push((current_path.to_string(), value.clone()));
                }
            }
        }
    }
}
```

### Paso 5: Implementar ComplianceEngine

```rust
// src/compliance/engine.rs
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use tracing::{debug, info, instrument, warn};

use super::condition::ConditionResult;
use super::path_matcher::PathMatcher;
use super::rule::{Rule, Violation};
use super::types::{ComplianceStatus, Severity, Standard};

/// Report generated by compliance evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    /// Overall compliance status.
    pub status: ComplianceStatus,

    /// List of violations found.
    pub violations: Vec<Violation>,

    /// Number of rules evaluated.
    pub rules_evaluated: usize,

    /// Number of paths checked.
    pub paths_checked: usize,

    /// When the check was performed.
    pub checked_at: DateTime<Utc>,

    /// Summary by severity.
    pub severity_summary: HashMap<Severity, usize>,

    /// Summary by standard.
    pub standard_summary: HashMap<Standard, usize>,
}

impl ComplianceReport {
    /// Creates an empty passed report.
    pub fn passed(rules_evaluated: usize, paths_checked: usize) -> Self {
        Self {
            status: ComplianceStatus::Passed,
            violations: vec![],
            rules_evaluated,
            paths_checked,
            checked_at: Utc::now(),
            severity_summary: HashMap::new(),
            standard_summary: HashMap::new(),
        }
    }

    /// Creates a failed report with violations.
    pub fn failed(
        violations: Vec<Violation>,
        rules_evaluated: usize,
        paths_checked: usize,
    ) -> Self {
        let mut severity_summary = HashMap::new();
        let mut standard_summary = HashMap::new();

        for v in &violations {
            *severity_summary.entry(v.severity).or_insert(0) += 1;
            *standard_summary.entry(v.standard).or_insert(0) += 1;
        }

        Self {
            status: ComplianceStatus::Failed,
            violations,
            rules_evaluated,
            paths_checked,
            checked_at: Utc::now(),
            severity_summary,
            standard_summary,
        }
    }

    /// Returns true if there are critical violations.
    pub fn has_critical(&self) -> bool {
        self.severity_summary.get(&Severity::Critical).copied().unwrap_or(0) > 0
    }

    /// Returns true if there are high or critical violations.
    pub fn has_high_or_above(&self) -> bool {
        self.has_critical()
            || self.severity_summary.get(&Severity::High).copied().unwrap_or(0) > 0
    }
}

/// Engine for evaluating compliance rules against configurations.
pub struct ComplianceEngine {
    rules: Vec<Rule>,
}

impl ComplianceEngine {
    /// Creates a new empty engine.
    pub fn new() -> Self {
        Self { rules: vec![] }
    }

    /// Creates an engine with default rules for common standards.
    pub fn with_defaults() -> Self {
        let mut engine = Self::new();
        engine.load_pci_dss_rules();
        engine.load_soc2_rules();
        engine
    }

    /// Adds a rule to the engine.
    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// Loads rules from a vector.
    pub fn load_rules(&mut self, rules: Vec<Rule>) {
        self.rules.extend(rules);
    }

    /// Evaluates all rules against a configuration.
    #[instrument(skip(self, config))]
    pub fn evaluate(&self, config: &serde_json::Value) -> ComplianceReport {
        let mut violations = Vec::new();
        let mut paths_checked = 0;

        let enabled_rules: Vec<_> = self.rules.iter().filter(|r| r.enabled).collect();

        info!(rules = enabled_rules.len(), "Starting compliance evaluation");

        for rule in &enabled_rules {
            let rule_violations = self.evaluate_rule(rule, config, &mut paths_checked);
            violations.extend(rule_violations);
        }

        if violations.is_empty() {
            info!("Compliance check passed");
            ComplianceReport::passed(enabled_rules.len(), paths_checked)
        } else {
            warn!(violations = violations.len(), "Compliance check failed");
            ComplianceReport::failed(violations, enabled_rules.len(), paths_checked)
        }
    }

    /// Evaluates a single rule.
    fn evaluate_rule(
        &self,
        rule: &Rule,
        config: &serde_json::Value,
        paths_checked: &mut usize,
    ) -> Vec<Violation> {
        let matcher = PathMatcher::new(&rule.path_pattern);
        let matches = matcher.find_matches(config);

        debug!(
            rule_id = %rule.id,
            pattern = %rule.path_pattern,
            matches = matches.len(),
            "Evaluating rule"
        );

        let mut violations = Vec::new();

        for (path, value) in matches {
            *paths_checked += 1;

            match rule.condition.evaluate(&value) {
                ConditionResult::Passed => {
                    debug!(path = %path, "Rule passed");
                }
                ConditionResult::Failed(message) => {
                    debug!(path = %path, message = %message, "Rule failed");
                    let violation = Violation::new(rule, path, message)
                        .with_value(&value);
                    violations.push(violation);
                }
                ConditionResult::Error(err) => {
                    warn!(path = %path, error = %err, "Rule evaluation error");
                }
            }
        }

        violations
    }

    /// Loads PCI-DSS rules.
    pub fn load_pci_dss_rules(&mut self) {
        self.rules.extend(super::standards::pci_dss::rules());
    }

    /// Loads SOC2 rules.
    pub fn load_soc2_rules(&mut self) {
        self.rules.extend(super::standards::soc2::rules());
    }
}

impl Default for ComplianceEngine {
    fn default() -> Self {
        Self::new()
    }
}
```

### Paso 6: Implementar Reglas PCI-DSS

```rust
// src/compliance/standards/pci_dss.rs
use crate::compliance::{Condition, Rule, Severity, Standard};

/// Returns the default PCI-DSS compliance rules.
pub fn rules() -> Vec<Rule> {
    vec![
        // 3.4 - Render PAN unreadable
        Rule::new(
            "pci-dss-3.4-no-plaintext-pan",
            "No Plaintext Card Numbers",
            "**.card_number",
            Condition::MustBeEncrypted,
        )
        .with_standard(Standard::PciDss)
        .with_severity(Severity::Critical)
        .with_description("Card numbers (PAN) must be encrypted or tokenized")
        .with_remediation("Use tokenization or encrypt card numbers with AES-256"),

        // 3.4 - No plaintext passwords
        Rule::new(
            "pci-dss-3.4-no-plaintext-passwords",
            "No Plaintext Passwords",
            "**.*password*",
            Condition::MustBeEncrypted,
        )
        .with_standard(Standard::PciDss)
        .with_severity(Severity::Critical)
        .with_description("Passwords must not be stored in plaintext")
        .with_remediation("Use secret references (vault://, ${SECRET}) instead of plaintext"),

        // 3.4 - No credentials in connection strings
        Rule::new(
            "pci-dss-3.4-no-embedded-credentials",
            "No Embedded Credentials in Strings",
            "**.*string*",
            Condition::MustNotMatch {
                pattern: r"://[^:]+:[^@]+@".to_string(),  // user:pass@host pattern
            },
        )
        .with_standard(Standard::PciDss)
        .with_severity(Severity::Critical)
        .with_description("Connection strings must not contain embedded credentials")
        .with_remediation("Use separate credential configuration or secret references"),

        // 4.1 - Use strong cryptography
        Rule::new(
            "pci-dss-4.1-https-required",
            "HTTPS Required for Endpoints",
            "**.*endpoint*",
            Condition::MustUseHttps,
        )
        .with_standard(Standard::PciDss)
        .with_severity(Severity::High)
        .with_description("All endpoints must use HTTPS")
        .with_remediation("Change http:// URLs to https://"),

        Rule::new(
            "pci-dss-4.1-https-urls",
            "HTTPS Required for URLs",
            "**.*url*",
            Condition::MustUseHttps,
        )
        .with_standard(Standard::PciDss)
        .with_severity(Severity::High)
        .with_description("All URLs must use HTTPS")
        .with_remediation("Change http:// URLs to https://"),

        // 2.2 - Secure defaults
        Rule::new(
            "pci-dss-2.2-no-debug-mode",
            "Debug Mode Disabled",
            "**.debug",
            Condition::MustMatch {
                pattern: r"^(false|0|no|off)$".to_string(),
            },
        )
        .with_standard(Standard::PciDss)
        .with_severity(Severity::Medium)
        .with_description("Debug mode should be disabled in production")
        .with_remediation("Set debug to false"),

        // 8.2 - Strong passwords
        Rule::new(
            "pci-dss-8.2-min-password-length",
            "Minimum Password Length Configuration",
            "**.min_password_length",
            Condition::MustBeInRange {
                min: Some(12.0),
                max: None,
            },
        )
        .with_standard(Standard::PciDss)
        .with_severity(Severity::Medium)
        .with_description("Minimum password length should be at least 12")
        .with_remediation("Set min_password_length to 12 or higher"),

        // 10.7 - Audit logging
        Rule::new(
            "pci-dss-10.7-audit-logging",
            "Audit Logging Enabled",
            "**.audit.enabled",
            Condition::MustMatch {
                pattern: r"^(true|1|yes|on)$".to_string(),
            },
        )
        .with_standard(Standard::PciDss)
        .with_severity(Severity::High)
        .with_description("Audit logging must be enabled")
        .with_remediation("Set audit.enabled to true"),
    ]
}
```

### Paso 7: Implementar Reglas SOC2

```rust
// src/compliance/standards/soc2.rs
use crate::compliance::{Condition, Rule, Severity, Standard};

/// Returns the default SOC2 compliance rules.
pub fn rules() -> Vec<Rule> {
    vec![
        // CC6.1 - Logical access controls
        Rule::new(
            "soc2-cc6.1-auth-required",
            "Authentication Required",
            "**.auth.enabled",
            Condition::MustMatch {
                pattern: r"^(true|1|yes|on)$".to_string(),
            },
        )
        .with_standard(Standard::Soc2)
        .with_severity(Severity::High)
        .with_description("Authentication must be enabled")
        .with_remediation("Set auth.enabled to true"),

        // CC6.1 - No anonymous access
        Rule::new(
            "soc2-cc6.1-no-anonymous",
            "No Anonymous Access",
            "**.allow_anonymous",
            Condition::MustMatch {
                pattern: r"^(false|0|no|off)$".to_string(),
            },
        )
        .with_standard(Standard::Soc2)
        .with_severity(Severity::High)
        .with_description("Anonymous access should be disabled")
        .with_remediation("Set allow_anonymous to false"),

        // CC6.6 - Encryption in transit
        Rule::new(
            "soc2-cc6.6-tls-enabled",
            "TLS Enabled",
            "**.tls.enabled",
            Condition::MustMatch {
                pattern: r"^(true|1|yes|on)$".to_string(),
            },
        )
        .with_standard(Standard::Soc2)
        .with_severity(Severity::High)
        .with_description("TLS must be enabled for encryption in transit")
        .with_remediation("Set tls.enabled to true"),

        // CC6.6 - Strong TLS version
        Rule::new(
            "soc2-cc6.6-tls-version",
            "TLS Version 1.2 or Higher",
            "**.tls.version",
            Condition::MustMatch {
                pattern: r"^(1\.[23]|TLSv1\.[23])$".to_string(),
            },
        )
        .with_standard(Standard::Soc2)
        .with_severity(Severity::Medium)
        .with_description("TLS version must be 1.2 or higher")
        .with_remediation("Set tls.version to 1.2 or 1.3"),

        // CC6.7 - Encryption at rest
        Rule::new(
            "soc2-cc6.7-encryption-at-rest",
            "Encryption at Rest Enabled",
            "**.encryption.at_rest",
            Condition::MustMatch {
                pattern: r"^(true|1|yes|on|enabled)$".to_string(),
            },
        )
        .with_standard(Standard::Soc2)
        .with_severity(Severity::High)
        .with_description("Encryption at rest must be enabled")
        .with_remediation("Set encryption.at_rest to true"),

        // CC7.2 - Logging
        Rule::new(
            "soc2-cc7.2-logging-enabled",
            "Logging Enabled",
            "**.logging.enabled",
            Condition::MustMatch {
                pattern: r"^(true|1|yes|on)$".to_string(),
            },
        )
        .with_standard(Standard::Soc2)
        .with_severity(Severity::Medium)
        .with_description("Logging must be enabled for monitoring")
        .with_remediation("Set logging.enabled to true"),

        // CC8.1 - No hardcoded secrets
        Rule::new(
            "soc2-cc8.1-no-hardcoded-secrets",
            "No Hardcoded API Keys",
            "**.*api_key*",
            Condition::MustBeEncrypted,
        )
        .with_standard(Standard::Soc2)
        .with_severity(Severity::Critical)
        .with_description("API keys must not be hardcoded")
        .with_remediation("Use secret references instead of hardcoded values"),

        Rule::new(
            "soc2-cc8.1-no-hardcoded-tokens",
            "No Hardcoded Tokens",
            "**.*token*",
            Condition::MustBeEncrypted,
        )
        .with_standard(Standard::Soc2)
        .with_severity(Severity::Critical)
        .with_description("Tokens must not be hardcoded")
        .with_remediation("Use secret references instead of hardcoded values"),
    ]
}
```

---

## Conceptos de Rust Aprendidos

### 1. Pattern Matching Recursivo en Estructuras Anidadas

**Rust:**
```rust
fn find_matches_recursive(
    &self,
    value: &Value,
    current_path: &str,
    remaining_segments: &[PathSegment],
    matches: &mut Vec<(String, Value)>,
) {
    match value {
        Value::Object(map) => {
            // Recurse into object
            for (key, v) in map {
                let new_path = format!("{}.{}", current_path, key);
                self.find_matches_recursive(v, &new_path, rest, matches);
            }
        }
        Value::Array(arr) => {
            // Recurse into array
            for (i, v) in arr.iter().enumerate() {
                let new_path = format!("{}[{}]", current_path, i);
                self.find_matches_recursive(v, &new_path, remaining, matches);
            }
        }
        _ => {
            // Leaf value
            if remaining_segments.is_empty() {
                matches.push((current_path.to_string(), value.clone()));
            }
        }
    }
}
```

**Comparacion con Java:**
```java
void findMatchesRecursive(
    JsonNode value,
    String currentPath,
    List<PathSegment> remaining,
    List<Match> matches
) {
    if (value.isObject()) {
        value.fields().forEachRemaining(entry -> {
            String newPath = currentPath + "." + entry.getKey();
            findMatchesRecursive(entry.getValue(), newPath, rest, matches);
        });
    } else if (value.isArray()) {
        for (int i = 0; i < value.size(); i++) {
            String newPath = currentPath + "[" + i + "]";
            findMatchesRecursive(value.get(i), newPath, remaining, matches);
        }
    } else {
        if (remaining.isEmpty()) {
            matches.add(new Match(currentPath, value));
        }
    }
}
```

### 2. Enum con Datos Serializables

**Rust:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    MustExist,
    MustNotExist,
    MustMatch { pattern: String },
    MustNotMatch { pattern: String },
    MustBeInRange { min: Option<f64>, max: Option<f64> },
    All { conditions: Vec<Condition> },  // Recursivo!
}

// Serializa como:
// { "type": "must_match", "pattern": "^https://" }
// { "type": "must_be_in_range", "min": 12.0, "max": null }
// { "type": "all", "conditions": [...] }
```

**Comparacion con Java:**
```java
@JsonTypeInfo(use = JsonTypeInfo.Id.NAME, property = "type")
@JsonSubTypes({
    @Type(value = MustExist.class, name = "must_exist"),
    @Type(value = MustMatch.class, name = "must_match"),
    @Type(value = MustBeInRange.class, name = "must_be_in_range"),
    @Type(value = All.class, name = "all")
})
public abstract class Condition {
    public abstract ConditionResult evaluate(JsonNode value);
}

public class MustMatch extends Condition {
    private String pattern;
    // ...
}
```

### 3. Builder Pattern con Defaults

**Rust:**
```rust
impl Rule {
    pub fn new(id: impl Into<String>, name: impl Into<String>, ...) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            // Defaults sensatos
            standard: Standard::Custom,
            severity: Severity::Medium,
            enabled: true,
            tags: vec![],
            remediation: None,
        }
    }

    // Builder methods
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }
}

// Uso fluido
let rule = Rule::new("id", "name", "**.password", condition)
    .with_severity(Severity::Critical)
    .with_remediation("Use secret references");
```

---

## Pruebas

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_must_be_encrypted_detects_plaintext() {
        let condition = Condition::MustBeEncrypted;

        let plaintext = json!("my-password");
        let encrypted = json!("vault://secret/db");

        assert!(matches!(condition.evaluate(&plaintext), ConditionResult::Failed(_)));
        assert!(matches!(condition.evaluate(&encrypted), ConditionResult::Passed));
    }

    #[test]
    fn test_must_use_https() {
        let condition = Condition::MustUseHttps;

        assert!(matches!(
            condition.evaluate(&json!("http://example.com")),
            ConditionResult::Failed(_)
        ));
        assert!(matches!(
            condition.evaluate(&json!("https://example.com")),
            ConditionResult::Passed
        ));
    }

    #[test]
    fn test_path_matcher_double_wildcard() {
        let matcher = PathMatcher::new("**.password");

        let config = json!({
            "database": {
                "password": "secret"
            },
            "cache": {
                "redis": {
                    "password": "redis-secret"
                }
            }
        });

        let matches = matcher.find_matches(&config);

        assert_eq!(matches.len(), 2);
        assert!(matches.iter().any(|(p, _)| p == "database.password"));
        assert!(matches.iter().any(|(p, _)| p == "cache.redis.password"));
    }

    #[test]
    fn test_compliance_engine_detects_violations() {
        let engine = ComplianceEngine::with_defaults();

        let config = json!({
            "database": {
                "password": "plaintext123",
                "connection_string": "postgres://user:pass@localhost/db"
            },
            "api": {
                "endpoint": "http://api.example.com"
            }
        });

        let report = engine.evaluate(&config);

        assert_eq!(report.status, ComplianceStatus::Failed);
        assert!(report.violations.len() >= 3);  // Password, conn string, HTTP
        assert!(report.has_critical());
    }

    #[test]
    fn test_compliance_passes_secure_config() {
        let engine = ComplianceEngine::with_defaults();

        let config = json!({
            "database": {
                "password": "vault://secret/db/password",
                "connection_string": "postgres://localhost/db"
            },
            "api": {
                "endpoint": "https://api.example.com"
            }
        });

        let report = engine.evaluate(&config);

        // May have warnings but no critical violations
        assert!(!report.has_critical());
    }
}
```

---

## Entregable Final

### Archivos Creados

1. `src/compliance/mod.rs` - Module exports
2. `src/compliance/types.rs` - Base types
3. `src/compliance/condition.rs` - Condition evaluation
4. `src/compliance/rule.rs` - Rule and Violation
5. `src/compliance/path_matcher.rs` - Path matching
6. `src/compliance/engine.rs` - ComplianceEngine
7. `src/compliance/standards/mod.rs` - Standards module
8. `src/compliance/standards/pci_dss.rs` - PCI-DSS rules
9. `src/compliance/standards/soc2.rs` - SOC2 rules
10. `tests/compliance_test.rs` - Tests

### Verificacion

```bash
cargo build -p vortex-compliance
cargo test -p vortex-compliance
cargo clippy -p vortex-compliance -- -D warnings
```

---

**Anterior**: [Historia 005 - Funciones Built-in de Templates](./story-005-template-functions.md)
**Siguiente**: [Historia 007 - API de Compliance Reports](./story-007-compliance-api.md)
