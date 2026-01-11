# Historia 002: Parser de Politicas YAML

## Contexto y Objetivo

Con el modelo de datos de PLAC definido en la Historia 001, esta historia implementa el parser que carga politicas desde archivos YAML. El parser debe ser robusto, validar semanticamente las politicas, y proporcionar errores descriptivos cuando algo falla.

**Responsabilidades del Parser:**
- Cargar archivos YAML individuales o directorios de politicas
- Deserializar YAML a estructuras Rust usando serde
- Validar sintaxis y semantica de las politicas
- Compilar patrones regex anticipadamente
- Cachear politicas para recargas eficientes
- Soportar hot-reload cuando cambian los archivos

Esta historia demuestra el uso avanzado de serde para deserializacion custom y manejo robusto de errores de parsing.

---

## Alcance

### In Scope

- Carga de politicas desde archivo YAML individual
- Carga de directorio de politicas (*.yaml, *.yml)
- Validacion semantica post-deserializacion
- Compilacion y cache de patrones regex
- Hot-reload de politicas (watch filesystem)
- Errores descriptivos con linea y columna
- Tests con fixtures YAML

### Out of Scope

- Evaluacion de politicas (Historia 003)
- Carga desde URLs remotas
- Encriptacion de archivos de politicas
- UI para edicion de politicas

---

## Criterios de Aceptacion

- [ ] `PolicyLoader::from_file(path)` carga un archivo YAML
- [ ] `PolicyLoader::from_directory(path)` carga todos los *.yaml/*.yml
- [ ] Errores de YAML incluyen linea y columna
- [ ] Validacion semantica detecta operadores invalidos para campos
- [ ] Patrones regex se compilan y cachean
- [ ] Hot-reload detecta cambios en archivos y recarga
- [ ] Tests cubren casos validos e invalidos
- [ ] Errores son descriptivos y accionables

---

## Diseno Propuesto

### Arquitectura del Parser

```
┌────────────────────────────────────────────────────────────────┐
│                      PolicyLoader                               │
├────────────────────────────────────────────────────────────────┤
│                                                                 │
│  from_file(path) ──────┐                                       │
│                        │                                       │
│  from_directory(path) ─┼──▶ ┌─────────────────────────────┐   │
│                        │    │     YAML Deserializer       │   │
│  from_string(yaml) ────┘    │     (serde_yaml)            │   │
│                             └──────────────┬──────────────┘   │
│                                            │                   │
│                                            ▼                   │
│                             ┌─────────────────────────────┐   │
│                             │   Semantic Validator        │   │
│                             │   - Field/Operator compat   │   │
│                             │   - Required fields         │   │
│                             │   - Value formats           │   │
│                             └──────────────┬──────────────┘   │
│                                            │                   │
│                                            ▼                   │
│                             ┌─────────────────────────────┐   │
│                             │   Regex Compiler            │   │
│                             │   - Compile patterns        │   │
│                             │   - Validate syntax         │   │
│                             │   - Cache compiled regex    │   │
│                             └──────────────┬──────────────┘   │
│                                            │                   │
│                                            ▼                   │
│                             ┌─────────────────────────────┐   │
│                             │   CompiledPolicySet         │   │
│                             │   (Ready for evaluation)    │   │
│                             └─────────────────────────────┘   │
│                                                                 │
└────────────────────────────────────────────────────────────────┘
```

### Estructura de Archivos

```
crates/vortex-governance/src/plac/
├── mod.rs
├── model.rs          # Historia 001
├── builder.rs        # Historia 001
├── parser.rs         # NUEVO: Este archivo
├── loader.rs         # NUEVO: Carga de archivos
├── validator.rs      # NUEVO: Validacion semantica
└── compiled.rs       # NUEVO: Regex compilados
```

---

## Pasos de Implementacion

### Paso 1: Definir Errores de Parsing

```rust
// src/plac/error.rs
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur when loading or parsing policies.
#[derive(Debug, Error)]
pub enum PolicyParseError {
    #[error("Failed to read policy file {path}: {source}")]
    IoError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("YAML parse error in {path} at line {line}, column {column}: {message}")]
    YamlError {
        path: PathBuf,
        line: Option<usize>,
        column: Option<usize>,
        message: String,
    },

    #[error("Validation error in policy '{policy_name}': {message}")]
    ValidationError {
        policy_name: String,
        message: String,
    },

    #[error("Invalid regex pattern in policy '{policy_name}': {pattern}")]
    InvalidRegex {
        policy_name: String,
        pattern: String,
        #[source]
        source: regex::Error,
    },

    #[error("Invalid CIDR notation in policy '{policy_name}': {cidr}")]
    InvalidCidr {
        policy_name: String,
        cidr: String,
    },

    #[error("Incompatible operator {operator:?} for field {field:?} in policy '{policy_name}'")]
    IncompatibleOperator {
        policy_name: String,
        field: ConditionField,
        operator: Operator,
    },

    #[error("Duplicate policy name: '{name}'")]
    DuplicatePolicyName { name: String },

    #[error("No policy files found in directory: {path}")]
    NoPoliciesFound { path: PathBuf },
}

impl PolicyParseError {
    /// Create a YAML error from serde_yaml::Error.
    pub fn from_yaml_error(path: PathBuf, err: serde_yaml::Error) -> Self {
        let location = err.location();
        Self::YamlError {
            path,
            line: location.map(|l| l.line()),
            column: location.map(|l| l.column()),
            message: err.to_string(),
        }
    }
}
```

### Paso 2: Implementar PolicyLoader

```rust
// src/plac/loader.rs
use std::path::{Path, PathBuf};
use std::fs;
use tracing::{info, warn, instrument};

use super::model::{Policy, PolicySet};
use super::error::PolicyParseError;
use super::validator::PolicyValidator;
use super::compiled::CompiledPolicySet;

/// Loads and parses policy files.
///
/// PolicyLoader handles reading YAML files from disk and converting
/// them into validated, ready-to-use policy sets.
pub struct PolicyLoader {
    validator: PolicyValidator,
}

impl PolicyLoader {
    /// Create a new PolicyLoader.
    pub fn new() -> Self {
        Self {
            validator: PolicyValidator::new(),
        }
    }

    /// Load policies from a single YAML file.
    #[instrument(skip(self), fields(path = %path.as_ref().display()))]
    pub fn from_file<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<CompiledPolicySet, PolicyParseError> {
        let path = path.as_ref();
        info!("Loading policies from file");

        let content = fs::read_to_string(path).map_err(|e| {
            PolicyParseError::IoError {
                path: path.to_path_buf(),
                source: e,
            }
        })?;

        self.from_string(&content, path.to_path_buf())
    }

    /// Load policies from a YAML string.
    pub fn from_string(
        &self,
        yaml: &str,
        source_path: PathBuf,
    ) -> Result<CompiledPolicySet, PolicyParseError> {
        // Deserialize YAML
        let policy_set: PolicySet = serde_yaml::from_str(yaml)
            .map_err(|e| PolicyParseError::from_yaml_error(source_path.clone(), e))?;

        // Validate all policies
        for policy in &policy_set.policies {
            self.validator.validate(policy)?;
        }

        // Check for duplicate names
        self.check_duplicates(&policy_set)?;

        // Compile regex patterns and return
        CompiledPolicySet::compile(policy_set)
    }

    /// Load all policies from a directory.
    #[instrument(skip(self), fields(dir = %dir.as_ref().display()))]
    pub fn from_directory<P: AsRef<Path>>(
        &self,
        dir: P,
    ) -> Result<CompiledPolicySet, PolicyParseError> {
        let dir = dir.as_ref();
        info!("Loading policies from directory");

        let mut all_policies = PolicySet::new();
        let mut found_any = false;

        // Read directory entries
        let entries = fs::read_dir(dir).map_err(|e| PolicyParseError::IoError {
            path: dir.to_path_buf(),
            source: e,
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| PolicyParseError::IoError {
                path: dir.to_path_buf(),
                source: e,
            })?;

            let path = entry.path();

            // Skip non-YAML files
            if !is_yaml_file(&path) {
                continue;
            }

            found_any = true;
            info!(file = %path.display(), "Loading policy file");

            let content = fs::read_to_string(&path).map_err(|e| {
                PolicyParseError::IoError {
                    path: path.clone(),
                    source: e,
                }
            })?;

            let policy_set: PolicySet = serde_yaml::from_str(&content)
                .map_err(|e| PolicyParseError::from_yaml_error(path.clone(), e))?;

            // Validate and add policies
            for policy in policy_set.policies {
                self.validator.validate(&policy)?;
                all_policies.add_policy(policy);
            }
        }

        if !found_any {
            return Err(PolicyParseError::NoPoliciesFound {
                path: dir.to_path_buf(),
            });
        }

        // Check for duplicates across all files
        self.check_duplicates(&all_policies)?;

        // Compile and return
        CompiledPolicySet::compile(all_policies)
    }

    /// Check for duplicate policy names.
    fn check_duplicates(&self, policy_set: &PolicySet) -> Result<(), PolicyParseError> {
        let mut seen = std::collections::HashSet::new();
        for policy in &policy_set.policies {
            if !seen.insert(&policy.name) {
                return Err(PolicyParseError::DuplicatePolicyName {
                    name: policy.name.clone(),
                });
            }
        }
        Ok(())
    }
}

impl Default for PolicyLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a path is a YAML file.
fn is_yaml_file(path: &Path) -> bool {
    path.extension()
        .map(|ext| ext == "yaml" || ext == "yml")
        .unwrap_or(false)
}
```

### Paso 3: Implementar Validador Semantico

```rust
// src/plac/validator.rs
use super::model::*;
use super::error::PolicyParseError;

/// Validates policies for semantic correctness.
///
/// Semantic validation goes beyond YAML parsing to ensure:
/// - Operators are valid for their field types
/// - Required values are present
/// - Value formats are correct (CIDR, regex, etc.)
pub struct PolicyValidator {
    // Can hold configuration if needed
}

impl PolicyValidator {
    pub fn new() -> Self {
        Self {}
    }

    /// Validate a single policy.
    pub fn validate(&self, policy: &Policy) -> Result<(), PolicyParseError> {
        // Validate each condition
        for condition in &policy.conditions {
            self.validate_condition(policy, condition)?;
        }

        // Validate action
        self.validate_action(policy)?;

        Ok(())
    }

    /// Validate a condition.
    fn validate_condition(
        &self,
        policy: &Policy,
        condition: &Condition,
    ) -> Result<(), PolicyParseError> {
        // Check operator/field compatibility
        self.check_operator_compatibility(policy, condition)?;

        // Validate value format based on operator
        self.validate_value_format(policy, condition)?;

        Ok(())
    }

    /// Check that the operator is valid for the field type.
    fn check_operator_compatibility(
        &self,
        policy: &Policy,
        condition: &Condition,
    ) -> Result<(), PolicyParseError> {
        let valid = match (&condition.field, &condition.operator) {
            // CIDR operators only valid for IP fields
            (ConditionField::SourceIp, Operator::InCidr | Operator::NotInCidr) => true,
            (_, Operator::InCidr | Operator::NotInCidr) => false,

            // All other operators valid for all fields
            _ => true,
        };

        if !valid {
            return Err(PolicyParseError::IncompatibleOperator {
                policy_name: policy.name.clone(),
                field: condition.field.clone(),
                operator: condition.operator.clone(),
            });
        }

        Ok(())
    }

    /// Validate the value format based on operator.
    fn validate_value_format(
        &self,
        policy: &Policy,
        condition: &Condition,
    ) -> Result<(), PolicyParseError> {
        match &condition.operator {
            // Validate regex patterns
            Operator::Matches | Operator::NotMatches => {
                regex::Regex::new(&condition.value).map_err(|e| {
                    PolicyParseError::InvalidRegex {
                        policy_name: policy.name.clone(),
                        pattern: condition.value.clone(),
                        source: e,
                    }
                })?;
            }

            // Validate CIDR notation
            Operator::InCidr | Operator::NotInCidr => {
                self.validate_cidr(policy, &condition.value)?;
            }

            // No special validation for other operators
            _ => {}
        }

        Ok(())
    }

    /// Validate CIDR notation.
    fn validate_cidr(&self, policy: &Policy, cidr: &str) -> Result<(), PolicyParseError> {
        // Simple validation: must contain '/'
        if !cidr.contains('/') {
            return Err(PolicyParseError::InvalidCidr {
                policy_name: policy.name.clone(),
                cidr: cidr.to_string(),
            });
        }

        // Parse network address and prefix
        let parts: Vec<&str> = cidr.split('/').collect();
        if parts.len() != 2 {
            return Err(PolicyParseError::InvalidCidr {
                policy_name: policy.name.clone(),
                cidr: cidr.to_string(),
            });
        }

        // Validate IP address
        if parts[0].parse::<std::net::IpAddr>().is_err() {
            return Err(PolicyParseError::InvalidCidr {
                policy_name: policy.name.clone(),
                cidr: cidr.to_string(),
            });
        }

        // Validate prefix length
        let prefix: u8 = parts[1].parse().map_err(|_| PolicyParseError::InvalidCidr {
            policy_name: policy.name.clone(),
            cidr: cidr.to_string(),
        })?;

        // Check prefix range based on IP version
        let max_prefix = if parts[0].contains(':') { 128 } else { 32 };
        if prefix > max_prefix {
            return Err(PolicyParseError::InvalidCidr {
                policy_name: policy.name.clone(),
                cidr: cidr.to_string(),
            });
        }

        Ok(())
    }

    /// Validate action configuration.
    fn validate_action(&self, policy: &Policy) -> Result<(), PolicyParseError> {
        match &policy.action.action_type {
            ActionType::Deny => {
                // Deny should have a message
                if policy.action.message.is_none() {
                    // Warning, not error - default message can be used
                }
            }
            ActionType::Redact => {
                // Redact should specify properties
                if policy.action.properties.is_none() {
                    return Err(PolicyParseError::ValidationError {
                        policy_name: policy.name.clone(),
                        message: "Redact action requires 'properties' field".to_string(),
                    });
                }
            }
            ActionType::Mask => {
                // Mask should have visible_chars >= 0
                if let Some(visible) = policy.action.visible_chars {
                    if visible > 100 {
                        return Err(PolicyParseError::ValidationError {
                            policy_name: policy.name.clone(),
                            message: "visible_chars should be <= 100".to_string(),
                        });
                    }
                }
            }
            ActionType::Warn => {
                // Warn should have a message
                if policy.action.message.is_none() {
                    return Err(PolicyParseError::ValidationError {
                        policy_name: policy.name.clone(),
                        message: "Warn action requires 'message' field".to_string(),
                    });
                }
            }
        }

        Ok(())
    }
}

impl Default for PolicyValidator {
    fn default() -> Self {
        Self::new()
    }
}
```

### Paso 4: Implementar CompiledPolicySet

```rust
// src/plac/compiled.rs
use std::collections::HashMap;
use regex::Regex;

use super::model::*;
use super::error::PolicyParseError;

/// A policy set with pre-compiled regex patterns.
///
/// Compiling regex patterns once at load time is much more efficient
/// than compiling them on every evaluation.
#[derive(Debug)]
pub struct CompiledPolicySet {
    /// Original policy set
    pub policy_set: PolicySet,

    /// Compiled regex patterns, keyed by pattern string
    pub(crate) compiled_patterns: HashMap<String, Regex>,
}

impl CompiledPolicySet {
    /// Compile a PolicySet, pre-compiling all regex patterns.
    pub fn compile(policy_set: PolicySet) -> Result<Self, PolicyParseError> {
        let mut compiled_patterns = HashMap::new();

        for policy in &policy_set.policies {
            for condition in &policy.conditions {
                // Compile regex for Matches/NotMatches operators
                if matches!(
                    condition.operator,
                    Operator::Matches | Operator::NotMatches
                ) {
                    if !compiled_patterns.contains_key(&condition.value) {
                        let regex = Regex::new(&condition.value).map_err(|e| {
                            PolicyParseError::InvalidRegex {
                                policy_name: policy.name.clone(),
                                pattern: condition.value.clone(),
                                source: e,
                            }
                        })?;
                        compiled_patterns.insert(condition.value.clone(), regex);
                    }
                }
            }
        }

        Ok(Self {
            policy_set,
            compiled_patterns,
        })
    }

    /// Get compiled regex for a pattern.
    pub fn get_regex(&self, pattern: &str) -> Option<&Regex> {
        self.compiled_patterns.get(pattern)
    }

    /// Get policies sorted by priority (highest first).
    pub fn sorted_policies(&self) -> Vec<&Policy> {
        self.policy_set.sorted_by_priority()
    }

    /// Get only active policies, sorted by priority.
    pub fn active_policies(&self) -> Vec<&Policy> {
        self.policy_set.active_policies()
    }

    /// Get the number of policies.
    pub fn len(&self) -> usize {
        self.policy_set.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.policy_set.is_empty()
    }

    /// Get the number of compiled patterns.
    pub fn pattern_count(&self) -> usize {
        self.compiled_patterns.len()
    }
}
```

### Paso 5: Implementar Hot-Reload con Watcher

```rust
// src/plac/watcher.rs
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, error, warn};

use super::loader::PolicyLoader;
use super::compiled::CompiledPolicySet;
use super::error::PolicyParseError;

/// Watch for policy file changes and reload automatically.
///
/// PolicyWatcher monitors a directory for changes and triggers
/// reloads when policy files are modified.
pub struct PolicyWatcher {
    /// Current compiled policies
    policies: Arc<RwLock<CompiledPolicySet>>,
    /// Path being watched
    watch_path: PathBuf,
    /// Loader instance
    loader: PolicyLoader,
}

impl PolicyWatcher {
    /// Create a new watcher for a directory.
    pub fn new<P: AsRef<Path>>(
        path: P,
    ) -> Result<Self, PolicyParseError> {
        let path = path.as_ref().to_path_buf();
        let loader = PolicyLoader::new();

        // Initial load
        let policies = loader.from_directory(&path)?;

        Ok(Self {
            policies: Arc::new(RwLock::new(policies)),
            watch_path: path,
            loader,
        })
    }

    /// Get a reference to the current policies.
    pub fn policies(&self) -> Arc<RwLock<CompiledPolicySet>> {
        Arc::clone(&self.policies)
    }

    /// Manually trigger a reload.
    pub fn reload(&self) -> Result<(), PolicyParseError> {
        info!(path = %self.watch_path.display(), "Reloading policies");

        let new_policies = self.loader.from_directory(&self.watch_path)?;

        let mut policies = self.policies.write().map_err(|_| {
            PolicyParseError::ValidationError {
                policy_name: "".to_string(),
                message: "Failed to acquire write lock".to_string(),
            }
        })?;

        *policies = new_policies;

        info!("Policies reloaded successfully");
        Ok(())
    }

    /// Start watching for changes (returns a channel for reload events).
    ///
    /// This spawns a background task that monitors the directory.
    pub fn start_watching(
        &self,
        debounce: Duration,
    ) -> mpsc::Receiver<Result<(), PolicyParseError>> {
        let (tx, rx) = mpsc::channel(16);
        let policies = Arc::clone(&self.policies);
        let watch_path = self.watch_path.clone();
        let loader = PolicyLoader::new();

        tokio::spawn(async move {
            // Simple polling implementation
            // In production, use notify crate for filesystem events
            let mut last_modified = get_dir_modified_time(&watch_path);

            loop {
                tokio::time::sleep(debounce).await;

                let current_modified = get_dir_modified_time(&watch_path);

                if current_modified != last_modified {
                    info!("Policy files changed, reloading");
                    last_modified = current_modified;

                    let result = match loader.from_directory(&watch_path) {
                        Ok(new_policies) => {
                            if let Ok(mut guard) = policies.write() {
                                *guard = new_policies;
                                Ok(())
                            } else {
                                Err(PolicyParseError::ValidationError {
                                    policy_name: "".to_string(),
                                    message: "Lock poisoned".to_string(),
                                })
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "Failed to reload policies");
                            Err(e)
                        }
                    };

                    if tx.send(result).await.is_err() {
                        // Receiver dropped, stop watching
                        break;
                    }
                }
            }
        });

        rx
    }
}

/// Get the most recent modification time for files in a directory.
fn get_dir_modified_time(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::read_dir(path)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "yaml" || ext == "yml")
                .unwrap_or(false)
        })
        .filter_map(|e| e.metadata().ok())
        .filter_map(|m| m.modified().ok())
        .max()
}
```

---

## Conceptos de Rust Aprendidos

### 1. Serde Custom Deserialization

Serde permite personalizar como se deserializan los datos.

**Rust:**
```rust
use serde::{Deserialize, Deserializer};

/// Custom deserializer for handling flexible input formats.
#[derive(Debug, Clone)]
pub struct FlexibleCidr(pub String);

impl<'de> Deserialize<'de> for FlexibleCidr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Accept either string or object format
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum CidrInput {
            Simple(String),
            Complex { network: String, prefix: u8 },
        }

        let input = CidrInput::deserialize(deserializer)?;

        let cidr = match input {
            CidrInput::Simple(s) => s,
            CidrInput::Complex { network, prefix } => {
                format!("{}/{}", network, prefix)
            }
        };

        Ok(FlexibleCidr(cidr))
    }
}

// Acepta ambos formatos:
// cidr: "10.0.0.0/8"
// cidr:
//   network: "10.0.0.0"
//   prefix: 8
```

**Java con Jackson:**
```java
public class FlexibleCidrDeserializer extends StdDeserializer<String> {

    @Override
    public String deserialize(JsonParser p, DeserializationContext ctx)
            throws IOException {
        JsonNode node = p.getCodec().readTree(p);

        if (node.isTextual()) {
            return node.asText();
        } else if (node.isObject()) {
            String network = node.get("network").asText();
            int prefix = node.get("prefix").asInt();
            return String.format("%s/%d", network, prefix);
        }

        throw new JsonMappingException(p, "Invalid CIDR format");
    }
}

@JsonDeserialize(using = FlexibleCidrDeserializer.class)
private String cidr;
```

### 2. Error Handling con Location Information

Extraer informacion de ubicacion de errores de parsing.

**Rust:**
```rust
impl PolicyParseError {
    /// Extract line/column from serde_yaml error.
    pub fn from_yaml_error(path: PathBuf, err: serde_yaml::Error) -> Self {
        // serde_yaml::Error provides location() method
        let location = err.location();

        Self::YamlError {
            path,
            line: location.map(|l| l.line()),
            column: location.map(|l| l.column()),
            message: err.to_string(),
        }
    }
}

// Uso
match serde_yaml::from_str::<PolicySet>(yaml) {
    Ok(set) => Ok(set),
    Err(e) => {
        // Error con ubicacion precisa
        // "YAML parse error in policies.yaml at line 15, column 3: ..."
        Err(PolicyParseError::from_yaml_error(path, e))
    }
}
```

**Java:**
```java
try {
    return mapper.readValue(yaml, PolicySet.class);
} catch (JsonProcessingException e) {
    JsonLocation loc = e.getLocation();
    throw new PolicyParseException(
        String.format("Parse error at line %d, column %d: %s",
            loc.getLineNr(), loc.getColumnNr(), e.getMessage())
    );
}
```

### 3. Filesystem Operations con Error Context

Agregar contexto a errores de I/O.

**Rust:**
```rust
use std::fs;
use std::path::Path;

fn load_file<P: AsRef<Path>>(path: P) -> Result<String, PolicyParseError> {
    let path = path.as_ref();

    // map_err agrega contexto al error
    fs::read_to_string(path).map_err(|e| PolicyParseError::IoError {
        path: path.to_path_buf(),
        source: e,  // Preserva el error original
    })
}

// El error resultante contiene:
// - Path del archivo que fallo
// - Error original de I/O
// - Display: "Failed to read policy file /path/to/file: Permission denied"
```

**Java:**
```java
public String loadFile(Path path) throws PolicyParseException {
    try {
        return Files.readString(path);
    } catch (IOException e) {
        throw new PolicyParseException(
            "Failed to read policy file " + path,
            e  // Causa original
        );
    }
}
```

### 4. Pre-compiled Regex Cache

Compilar regex una vez y reutilizar.

**Rust:**
```rust
use std::collections::HashMap;
use regex::Regex;

pub struct CompiledPolicySet {
    policies: Vec<Policy>,
    // Cache de regex compilados
    patterns: HashMap<String, Regex>,
}

impl CompiledPolicySet {
    pub fn compile(policies: Vec<Policy>) -> Result<Self, Error> {
        let mut patterns = HashMap::new();

        for policy in &policies {
            for condition in &policy.conditions {
                if matches!(condition.operator, Operator::Matches) {
                    // Solo compilar si no existe
                    if !patterns.contains_key(&condition.value) {
                        let regex = Regex::new(&condition.value)?;
                        patterns.insert(condition.value.clone(), regex);
                    }
                }
            }
        }

        Ok(Self { policies, patterns })
    }

    // Usar regex pre-compilado
    pub fn matches(&self, pattern: &str, value: &str) -> bool {
        self.patterns
            .get(pattern)
            .map(|r| r.is_match(value))
            .unwrap_or(false)
    }
}
```

**Java:**
```java
public class CompiledPolicySet {
    private final List<Policy> policies;
    private final Map<String, Pattern> patterns = new HashMap<>();

    public CompiledPolicySet(List<Policy> policies) {
        this.policies = policies;
        for (Policy policy : policies) {
            for (Condition cond : policy.getConditions()) {
                if (cond.getOperator() == Operator.MATCHES) {
                    patterns.computeIfAbsent(
                        cond.getValue(),
                        Pattern::compile
                    );
                }
            }
        }
    }

    public boolean matches(String pattern, String value) {
        Pattern p = patterns.get(pattern);
        return p != null && p.matcher(value).matches();
    }
}
```

---

## Riesgos y Errores Comunes

### 1. No Validar Regex Antes de Compilar

```rust
// MAL: Crash en runtime si regex invalido
fn evaluate(pattern: &str, value: &str) -> bool {
    Regex::new(pattern).unwrap().is_match(value)  // panic!
}

// BIEN: Validar en carga
fn load_policy(yaml: &str) -> Result<Policy, Error> {
    let policy: Policy = serde_yaml::from_str(yaml)?;

    // Validar regex
    for condition in &policy.conditions {
        if matches!(condition.operator, Operator::Matches) {
            Regex::new(&condition.value)?;  // Error propagado
        }
    }

    Ok(policy)
}
```

### 2. ReDoS (Regex Denial of Service)

```rust
// MAL: Regex vulnerable a ReDoS
let pattern = "(a+)+$";  // Exponencial con "aaaaaaaaaaaaaaaaX"

// MEJOR: Limitar complejidad o usar timeout
use regex::RegexBuilder;

let regex = RegexBuilder::new(pattern)
    .size_limit(10_000)  // Limitar tamano del automata
    .build()?;

// O validar patrones en carga
fn validate_pattern(pattern: &str) -> Result<(), Error> {
    // Rechazar patrones con anidamiento excesivo
    let nested_quantifiers = Regex::new(r"\([^)]*[+*]\)[+*]")?;
    if nested_quantifiers.is_match(pattern) {
        return Err(Error::UnsafePattern(pattern.to_string()));
    }
    Ok(())
}
```

### 3. Path Traversal en Carga de Archivos

```rust
// MAL: Usuario puede cargar archivos arbitrarios
fn load_policy(user_path: &str) -> Result<Policy, Error> {
    let content = fs::read_to_string(user_path)?;  // /etc/passwd!
    // ...
}

// BIEN: Validar y canonicalizar paths
fn load_policy(base_dir: &Path, filename: &str) -> Result<Policy, Error> {
    // Rechazar path traversal
    if filename.contains("..") || filename.starts_with('/') {
        return Err(Error::InvalidPath(filename.to_string()));
    }

    let full_path = base_dir.join(filename).canonicalize()?;

    // Verificar que este dentro del directorio base
    if !full_path.starts_with(base_dir.canonicalize()?) {
        return Err(Error::InvalidPath(filename.to_string()));
    }

    let content = fs::read_to_string(full_path)?;
    // ...
}
```

### 4. Lock Poisoning en Hot-Reload

```rust
// MAL: Panic si lock esta envenenado
fn reload(&self) -> Result<(), Error> {
    let mut guard = self.policies.write().unwrap();  // panic si poisoned!
    *guard = new_policies;
    Ok(())
}

// BIEN: Manejar lock poisoning
fn reload(&self) -> Result<(), Error> {
    let mut guard = self.policies.write().map_err(|e| {
        // El lock esta envenenado (otro thread hizo panic mientras lo tenia)
        Error::LockPoisoned(e.to_string())
    })?;

    *guard = new_policies;
    Ok(())
}

// O recuperar el lock poisonado
fn reload(&self) -> Result<(), Error> {
    let mut guard = self.policies.write().unwrap_or_else(|e| {
        // Recuperar el guard aunque este poisoned
        e.into_inner()
    });
    *guard = new_policies;
    Ok(())
}
```

---

## Pruebas

### Tests de Carga de Archivos

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs::File;
    use std::io::Write;

    fn create_temp_policy_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        let mut file = File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn test_load_single_file() {
        let dir = TempDir::new().unwrap();
        let yaml = r#"
version: "1.0"
policies:
  - name: test-policy
    conditions:
      - field: application
        operator: equals
        value: myapp
    action:
      type: deny
      message: Denied
"#;
        let path = create_temp_policy_file(&dir, "policy.yaml", yaml);

        let loader = PolicyLoader::new();
        let result = loader.from_file(&path);

        assert!(result.is_ok());
        let policies = result.unwrap();
        assert_eq!(policies.len(), 1);
    }

    #[test]
    fn test_load_directory() {
        let dir = TempDir::new().unwrap();

        let yaml1 = r#"
policies:
  - name: policy-1
    conditions:
      - field: profile
        operator: equals
        value: prod
    action:
      type: warn
      message: Production access
"#;
        create_temp_policy_file(&dir, "policies1.yaml", yaml1);

        let yaml2 = r#"
policies:
  - name: policy-2
    conditions:
      - field: application
        operator: matches
        value: "internal-.*"
    action:
      type: deny
      message: Internal only
"#;
        create_temp_policy_file(&dir, "policies2.yml", yaml2);

        let loader = PolicyLoader::new();
        let result = loader.from_directory(dir.path());

        assert!(result.is_ok());
        let policies = result.unwrap();
        assert_eq!(policies.len(), 2);
    }

    #[test]
    fn test_yaml_error_with_location() {
        let dir = TempDir::new().unwrap();
        let invalid_yaml = r#"
policies:
  - name: test
    conditions:
      - field: application
        operator: invalid_operator  # Error!
        value: test
"#;
        let path = create_temp_policy_file(&dir, "invalid.yaml", invalid_yaml);

        let loader = PolicyLoader::new();
        let result = loader.from_file(&path);

        assert!(result.is_err());
        let error = result.unwrap_err();
        // Error should mention line number
        assert!(error.to_string().contains("line") || error.to_string().contains("invalid"));
    }
}
```

### Tests de Validacion Semantica

```rust
#[cfg(test)]
mod validation_tests {
    use super::*;

    #[test]
    fn test_invalid_cidr_operator_for_non_ip_field() {
        let yaml = r#"
policies:
  - name: invalid-operator
    conditions:
      - field: application
        operator: in_cidr
        value: "10.0.0.0/8"
    action:
      type: deny
      message: Test
"#;
        let loader = PolicyLoader::new();
        let result = loader.from_string(yaml, PathBuf::from("test.yaml"));

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, PolicyParseError::IncompatibleOperator { .. }));
    }

    #[test]
    fn test_invalid_regex_pattern() {
        let yaml = r#"
policies:
  - name: invalid-regex
    conditions:
      - field: application
        operator: matches
        value: "[invalid(regex"
    action:
      type: deny
      message: Test
"#;
        let loader = PolicyLoader::new();
        let result = loader.from_string(yaml, PathBuf::from("test.yaml"));

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, PolicyParseError::InvalidRegex { .. }));
    }

    #[test]
    fn test_invalid_cidr_format() {
        let yaml = r#"
policies:
  - name: invalid-cidr
    conditions:
      - field: source_ip
        operator: in_cidr
        value: "not-a-cidr"
    action:
      type: deny
      message: Test
"#;
        let loader = PolicyLoader::new();
        let result = loader.from_string(yaml, PathBuf::from("test.yaml"));

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, PolicyParseError::InvalidCidr { .. }));
    }

    #[test]
    fn test_redact_without_properties() {
        let yaml = r#"
policies:
  - name: redact-missing-props
    conditions:
      - field: profile
        operator: equals
        value: prod
    action:
      type: redact
"#;
        let loader = PolicyLoader::new();
        let result = loader.from_string(yaml, PathBuf::from("test.yaml"));

        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_policy_names() {
        let yaml = r#"
policies:
  - name: duplicate
    conditions:
      - field: application
        operator: equals
        value: app1
    action:
      type: deny
      message: First
  - name: duplicate
    conditions:
      - field: application
        operator: equals
        value: app2
    action:
      type: deny
      message: Second
"#;
        let loader = PolicyLoader::new();
        let result = loader.from_string(yaml, PathBuf::from("test.yaml"));

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, PolicyParseError::DuplicatePolicyName { .. }));
    }
}
```

### Tests de Regex Pre-compilado

```rust
#[cfg(test)]
mod compiled_tests {
    use super::*;

    #[test]
    fn test_regex_compilation() {
        let yaml = r#"
policies:
  - name: regex-policy
    conditions:
      - field: application
        operator: matches
        value: "service-[a-z]+"
    action:
      type: warn
      message: Test
"#;
        let loader = PolicyLoader::new();
        let compiled = loader.from_string(yaml, PathBuf::from("test.yaml")).unwrap();

        // Should have compiled the regex
        assert_eq!(compiled.pattern_count(), 1);
        assert!(compiled.get_regex("service-[a-z]+").is_some());
    }

    #[test]
    fn test_shared_regex_patterns() {
        let yaml = r#"
policies:
  - name: policy-1
    conditions:
      - field: application
        operator: matches
        value: "api-.*"
    action:
      type: warn
      message: Test
  - name: policy-2
    conditions:
      - field: application
        operator: matches
        value: "api-.*"
    action:
      type: deny
      message: Test
"#;
        let loader = PolicyLoader::new();
        let compiled = loader.from_string(yaml, PathBuf::from("test.yaml")).unwrap();

        // Same pattern should be compiled only once
        assert_eq!(compiled.pattern_count(), 1);
    }
}
```

---

## Seguridad

- **Validacion de paths**: No permitir path traversal en nombres de archivo
- **Limite de tamano**: Rechazar archivos YAML mayores a un limite
- **Timeout en regex**: Usar regex con limite de tiempo/complejidad
- **No ejecutar codigo**: YAML deserializado no puede ejecutar codigo
- **Permisos de archivo**: Verificar que solo root/owner pueden modificar politicas

---

## Entregable Final

### Archivos Creados/Modificados

1. `src/plac/error.rs` - Tipos de error de parsing
2. `src/plac/loader.rs` - PolicyLoader para carga de archivos
3. `src/plac/validator.rs` - Validador semantico
4. `src/plac/compiled.rs` - CompiledPolicySet con regex
5. `src/plac/watcher.rs` - Hot-reload de politicas
6. `src/plac/mod.rs` - Re-exports actualizados

### Verificacion

```bash
# Compilar
cargo build -p vortex-governance

# Tests
cargo test -p vortex-governance -- policy

# Test con archivo real
cat > /tmp/test-policy.yaml << 'EOF'
version: "1.0"
policies:
  - name: mask-passwords
    description: Mask all password fields in production
    priority: 100
    conditions:
      - field: profile
        operator: equals
        value: production
      - field: property_path
        operator: matches
        value: ".*password.*"
    action:
      type: mask
      mask_char: "*"
      visible_chars: 4
EOF

# Validar sintaxis (herramienta CLI)
cargo run -p vortex-governance -- validate /tmp/test-policy.yaml
```

### Ejemplo de YAML Valido

```yaml
version: "1.0"

policies:
  # Deny access to internal services from external networks
  - name: deny-external-to-internal
    description: Block external access to internal-* applications
    priority: 200
    enabled: true
    conditions:
      - field: application
        operator: matches
        value: "internal-.*"
      - field: source_ip
        operator: not_in_cidr
        value: "10.0.0.0/8"
    action:
      type: deny
      message: "Access denied: internal services require internal network"

  # Mask secrets in production
  - name: mask-production-secrets
    description: Mask sensitive fields in production configs
    priority: 100
    conditions:
      - field: profile
        operator: in
        value: "production,prod,prd"
      - field: property_path
        operator: matches
        value: ".*(password|secret|api[_-]?key|token).*"
        case_sensitive: false
    action:
      type: mask
      mask_char: "*"
      visible_chars: 4

  # Warn on deprecated configs
  - name: warn-deprecated
    description: Warn when accessing deprecated configurations
    priority: 50
    conditions:
      - field: application
        operator: matches
        value: "legacy-.*"
    action:
      type: warn
      message: "This application uses deprecated configuration format"
```
