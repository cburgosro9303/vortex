# Historia 001: Modelo de Politicas PLAC

## Contexto y Objetivo

PLAC (Policy Language for Access Control) es el corazon del sistema de gobernanza de Vortex Config. Antes de poder parsear o evaluar politicas, necesitamos un modelo de datos solido que represente todos los conceptos del lenguaje.

Esta historia define las estructuras de datos fundamentales:
- **Policy**: Una politica completa con nombre, condiciones y accion
- **Condition**: Una condicion individual que debe cumplirse
- **Action**: La accion a ejecutar cuando las condiciones se cumplen
- **Operator**: Los operadores de comparacion soportados

El modelo sigue el **Builder Pattern** para construccion ergonomica y usa **enums con datos** (tagged unions) para representar las variantes de condiciones y acciones.

---

## Alcance

### In Scope

- Structs para Policy, Condition, Action, Operator
- Enums con datos para ActionType y ConditionField
- Builder pattern para construccion de politicas
- Traits Serialize/Deserialize con serde
- Validacion basica de estructuras
- Documentation con rustdoc

### Out of Scope

- Parsing desde YAML (Historia 002)
- Evaluacion de politicas (Historia 003)
- Integracion con middleware (Historia 004)
- Implementacion de acciones (Historia 006)

---

## Criterios de Aceptacion

- [ ] Struct `Policy` con campos: name, description, priority, conditions, action, enabled
- [ ] Enum `ActionType` con variantes: Deny, Redact, Mask, Warn
- [ ] Enum `Operator` con variantes: Equals, NotEquals, Matches, NotMatches, In, NotIn, Contains, InCidr, NotInCidr
- [ ] Enum `ConditionField` para campos evaluables: Application, Profile, Label, PropertyPath, SourceIp, Header
- [ ] Builder para `Policy` con metodos encadenables
- [ ] Derive Serialize/Deserialize para todas las estructuras
- [ ] Tests unitarios para builders y serialization
- [ ] Documentacion rustdoc completa

---

## Diseno Propuesto

### Modelo de Datos

```
┌─────────────────────────────────────────────────────────────────┐
│                          Policy                                  │
├─────────────────────────────────────────────────────────────────┤
│ name: String           │ Identificador unico de la politica     │
│ description: String    │ Descripcion legible                    │
│ priority: u32          │ Orden de evaluacion (mayor = primero)  │
│ enabled: bool          │ Si la politica esta activa             │
│ conditions: Vec<Cond>  │ Condiciones que deben cumplirse        │
│ action: Action         │ Accion a ejecutar si match             │
└─────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    │                               │
         ┌──────────▼──────────┐        ┌─────────▼─────────┐
         │     Condition       │        │      Action       │
         ├─────────────────────┤        ├───────────────────┤
         │ field: CondField    │        │ action_type: Type │
         │ operator: Operator  │        │ message: Option   │
         │ value: String       │        │ mask_char: Option │
         │ case_sensitive: bool│        │ visible: Option   │
         └──────────▲──────────┘        │ properties: Option│
                    │                   └───────────────────┘
         ┌──────────┴──────────┐
         │                     │
┌────────▼────────┐  ┌────────▼────────┐
│  ConditionField │  │    Operator     │
├─────────────────┤  ├─────────────────┤
│ Application     │  │ Equals          │
│ Profile         │  │ NotEquals       │
│ Label           │  │ Matches (regex) │
│ PropertyPath    │  │ In (list)       │
│ SourceIp        │  │ InCidr          │
│ Header(String)  │  │ Contains        │
└─────────────────┘  └─────────────────┘
```

### Estructura de Archivos

```
crates/vortex-governance/src/plac/
├── mod.rs          # Re-exports
├── model.rs        # Este archivo: definiciones de tipos
├── builder.rs      # Builders para Policy y Condition
└── validation.rs   # Validacion de estructuras
```

---

## Pasos de Implementacion

### Paso 1: Crear Modulo PLAC y Definir Enums Base

```rust
// src/plac/model.rs
use serde::{Deserialize, Serialize};

/// Field types that can be evaluated in conditions.
///
/// Represents the different aspects of a request that can be
/// used in policy conditions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionField {
    /// Application name (e.g., "payment-service")
    Application,
    /// Profile name (e.g., "production", "development")
    Profile,
    /// Label/branch name (e.g., "main", "v1.0.0")
    Label,
    /// Property path within config (e.g., "database.password")
    PropertyPath,
    /// Source IP address of the request
    SourceIp,
    /// HTTP header value (header name as parameter)
    #[serde(rename = "header")]
    Header(String),
}

/// Comparison operators for conditions.
///
/// These operators determine how the condition value is compared
/// against the actual request context value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    /// Exact equality comparison
    Equals,
    /// Negated equality comparison
    NotEquals,
    /// Regex pattern matching
    Matches,
    /// Negated regex pattern matching
    NotMatches,
    /// Value is in a list (comma-separated in YAML)
    In,
    /// Value is not in a list
    NotIn,
    /// String contains substring
    Contains,
    /// IP is within CIDR range
    InCidr,
    /// IP is not within CIDR range
    NotInCidr,
}
```

### Paso 2: Definir ActionType y Action

```rust
// src/plac/model.rs (continuacion)

/// Type of action to perform when policy matches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    /// Deny the request entirely (returns 403)
    Deny,
    /// Remove specified properties from response
    Redact,
    /// Mask sensitive values with asterisks
    Mask,
    /// Allow but add warning header to response
    Warn,
}

impl ActionType {
    /// Returns true if this action terminates evaluation.
    ///
    /// Terminal actions (like Deny) stop policy evaluation
    /// immediately and return a response.
    pub fn is_terminal(&self) -> bool {
        matches!(self, ActionType::Deny)
    }
}

/// Action to perform when a policy matches.
///
/// Contains the action type and optional configuration
/// specific to each action type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Action {
    /// Type of action to perform
    #[serde(rename = "type")]
    pub action_type: ActionType,

    /// Message for Deny/Warn actions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Character to use for masking (default: '*')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask_char: Option<char>,

    /// Number of characters to leave visible at end
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible_chars: Option<usize>,

    /// List of property paths to redact
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Vec<String>>,
}

impl Default for Action {
    fn default() -> Self {
        Self {
            action_type: ActionType::Deny,
            message: None,
            mask_char: None,
            visible_chars: None,
            properties: None,
        }
    }
}
```

### Paso 3: Definir Condition y Policy

```rust
// src/plac/model.rs (continuacion)

/// A single condition that must be satisfied for a policy to match.
///
/// Conditions are evaluated as AND - all conditions in a policy
/// must match for the policy to apply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    /// Field to evaluate
    pub field: ConditionField,

    /// Operator for comparison
    pub operator: Operator,

    /// Value to compare against
    pub value: String,

    /// Whether comparison is case-sensitive (default: true)
    #[serde(default = "default_case_sensitive")]
    pub case_sensitive: bool,
}

fn default_case_sensitive() -> bool {
    true
}

/// A complete policy definition.
///
/// Policies are the core unit of governance in PLAC. Each policy
/// consists of conditions that must be met and an action to perform.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    /// Unique identifier for the policy
    pub name: String,

    /// Human-readable description
    #[serde(default)]
    pub description: String,

    /// Evaluation priority (higher = evaluated first)
    #[serde(default = "default_priority")]
    pub priority: u32,

    /// Whether the policy is active
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Conditions that must ALL match (AND logic)
    pub conditions: Vec<Condition>,

    /// Action to perform when all conditions match
    pub action: Action,
}

fn default_priority() -> u32 {
    100
}

fn default_enabled() -> bool {
    true
}
```

### Paso 4: Implementar Builder Pattern para Policy

```rust
// src/plac/builder.rs
use super::model::*;

/// Builder for creating Policy instances fluently.
///
/// # Example
/// ```
/// use vortex_governance::plac::{PolicyBuilder, ActionType, ConditionField, Operator};
///
/// let policy = PolicyBuilder::new("mask-passwords")
///     .description("Mask all password fields in production")
///     .priority(100)
///     .condition(ConditionField::Profile, Operator::Equals, "production")
///     .condition(ConditionField::PropertyPath, Operator::Matches, ".*password.*")
///     .action_mask('*', 4)
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Default)]
pub struct PolicyBuilder {
    name: Option<String>,
    description: String,
    priority: u32,
    enabled: bool,
    conditions: Vec<Condition>,
    action: Option<Action>,
}

impl PolicyBuilder {
    /// Create a new PolicyBuilder with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            description: String::new(),
            priority: 100,
            enabled: true,
            conditions: Vec::new(),
            action: None,
        }
    }

    /// Set the policy description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set the policy priority.
    pub fn priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Set whether the policy is enabled.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Add a condition to the policy.
    pub fn condition(
        mut self,
        field: ConditionField,
        operator: Operator,
        value: impl Into<String>,
    ) -> Self {
        self.conditions.push(Condition {
            field,
            operator,
            value: value.into(),
            case_sensitive: true,
        });
        self
    }

    /// Add a case-insensitive condition.
    pub fn condition_case_insensitive(
        mut self,
        field: ConditionField,
        operator: Operator,
        value: impl Into<String>,
    ) -> Self {
        self.conditions.push(Condition {
            field,
            operator,
            value: value.into(),
            case_sensitive: false,
        });
        self
    }

    /// Set a deny action.
    pub fn action_deny(mut self, message: impl Into<String>) -> Self {
        self.action = Some(Action {
            action_type: ActionType::Deny,
            message: Some(message.into()),
            ..Default::default()
        });
        self
    }

    /// Set a mask action.
    pub fn action_mask(mut self, mask_char: char, visible_chars: usize) -> Self {
        self.action = Some(Action {
            action_type: ActionType::Mask,
            mask_char: Some(mask_char),
            visible_chars: Some(visible_chars),
            ..Default::default()
        });
        self
    }

    /// Set a redact action.
    pub fn action_redact(mut self, properties: Vec<String>) -> Self {
        self.action = Some(Action {
            action_type: ActionType::Redact,
            properties: Some(properties),
            ..Default::default()
        });
        self
    }

    /// Set a warn action.
    pub fn action_warn(mut self, message: impl Into<String>) -> Self {
        self.action = Some(Action {
            action_type: ActionType::Warn,
            message: Some(message.into()),
            ..Default::default()
        });
        self
    }

    /// Build the Policy, returning an error if required fields are missing.
    pub fn build(self) -> Result<Policy, PolicyBuildError> {
        let name = self.name.ok_or(PolicyBuildError::MissingName)?;
        let action = self.action.ok_or(PolicyBuildError::MissingAction)?;

        if self.conditions.is_empty() {
            return Err(PolicyBuildError::NoConditions);
        }

        Ok(Policy {
            name,
            description: self.description,
            priority: self.priority,
            enabled: self.enabled,
            conditions: self.conditions,
            action,
        })
    }
}

/// Errors that can occur when building a Policy.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PolicyBuildError {
    #[error("Policy name is required")]
    MissingName,

    #[error("Policy action is required")]
    MissingAction,

    #[error("Policy must have at least one condition")]
    NoConditions,
}
```

### Paso 5: Agregar Metodos de Conveniencia a Policy

```rust
// src/plac/model.rs (metodos impl)

impl Policy {
    /// Check if this policy is currently active.
    pub fn is_active(&self) -> bool {
        self.enabled
    }

    /// Check if the action is terminal (stops evaluation).
    pub fn has_terminal_action(&self) -> bool {
        self.action.action_type.is_terminal()
    }

    /// Get the number of conditions.
    pub fn condition_count(&self) -> usize {
        self.conditions.len()
    }
}

impl Condition {
    /// Create a new condition.
    pub fn new(
        field: ConditionField,
        operator: Operator,
        value: impl Into<String>,
    ) -> Self {
        Self {
            field,
            operator,
            value: value.into(),
            case_sensitive: true,
        }
    }

    /// Create a case-insensitive condition.
    pub fn new_case_insensitive(
        field: ConditionField,
        operator: Operator,
        value: impl Into<String>,
    ) -> Self {
        Self {
            field,
            operator,
            value: value.into(),
            case_sensitive: false,
        }
    }
}
```

### Paso 6: Crear PolicySet para Coleccion de Politicas

```rust
// src/plac/model.rs (PolicySet)

/// A collection of policies, typically loaded from a file.
///
/// PolicySet handles loading, sorting, and iterating over policies.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PolicySet {
    /// Version of the policy file format
    #[serde(default = "default_version")]
    pub version: String,

    /// List of policies
    pub policies: Vec<Policy>,
}

fn default_version() -> String {
    "1.0".to_string()
}

impl PolicySet {
    /// Create a new empty PolicySet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a policy to the set.
    pub fn add_policy(&mut self, policy: Policy) {
        self.policies.push(policy);
    }

    /// Get policies sorted by priority (highest first).
    pub fn sorted_by_priority(&self) -> Vec<&Policy> {
        let mut sorted: Vec<_> = self.policies.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    /// Get only enabled policies, sorted by priority.
    pub fn active_policies(&self) -> Vec<&Policy> {
        let mut active: Vec<_> = self.policies
            .iter()
            .filter(|p| p.enabled)
            .collect();
        active.sort_by(|a, b| b.priority.cmp(&a.priority));
        active
    }

    /// Get policy by name.
    pub fn get_by_name(&self, name: &str) -> Option<&Policy> {
        self.policies.iter().find(|p| p.name == name)
    }

    /// Get the number of policies.
    pub fn len(&self) -> usize {
        self.policies.len()
    }

    /// Check if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.policies.is_empty()
    }
}
```

---

## Conceptos de Rust Aprendidos

### 1. Builder Pattern

El Builder Pattern en Rust es mas ergonomico que en Java gracias al ownership y method chaining.

**Rust:**
```rust
// Builder consume self y retorna Self para chaining
pub struct PolicyBuilder {
    name: Option<String>,
    priority: u32,
    // ...
}

impl PolicyBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            priority: 100,
            // ...
        }
    }

    // Cada metodo consume self y retorna Self
    pub fn priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    // build() consume el builder y retorna Result
    pub fn build(self) -> Result<Policy, PolicyBuildError> {
        // Validacion y construccion
        Ok(Policy { /* ... */ })
    }
}

// Uso fluido
let policy = PolicyBuilder::new("my-policy")
    .priority(200)
    .description("A policy")
    .condition(/* ... */)
    .action_deny("Access denied")
    .build()?;
```

**Java equivalente:**
```java
public class PolicyBuilder {
    private String name;
    private int priority = 100;

    public PolicyBuilder(String name) {
        this.name = name;
    }

    // Metodos retornan this
    public PolicyBuilder priority(int priority) {
        this.priority = priority;
        return this;
    }

    public Policy build() {
        if (name == null) throw new IllegalStateException("Name required");
        return new Policy(name, priority, /* ... */);
    }
}

// Uso similar
Policy policy = new PolicyBuilder("my-policy")
    .priority(200)
    .description("A policy")
    .addCondition(/* ... */)
    .actionDeny("Access denied")
    .build();
```

**Diferencias clave:**
| Aspecto | Rust | Java |
|---------|------|------|
| Ownership | Builder consumido en `build()` | Builder reutilizable |
| Tipo retorno | `Result<T, E>` | Throws exception |
| Genericidad | `impl Into<String>` | Overloads o Object |
| Mutabilidad | `mut self` explicito | Implicita |

### 2. Enums con Datos (Tagged Unions)

Los enums de Rust pueden contener datos, algo que Java logro recientemente con sealed classes.

**Rust:**
```rust
/// Enum con variantes que contienen datos diferentes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionField {
    // Variantes simples (unit variants)
    Application,
    Profile,
    Label,
    PropertyPath,
    SourceIp,
    // Variante con dato asociado (tuple variant)
    Header(String),
}

// Uso con pattern matching
fn get_field_value(field: &ConditionField, ctx: &Context) -> Option<String> {
    match field {
        ConditionField::Application => Some(ctx.app.clone()),
        ConditionField::Profile => Some(ctx.profile.clone()),
        ConditionField::Label => ctx.label.clone(),
        ConditionField::PropertyPath => None, // Manejado diferente
        ConditionField::SourceIp => ctx.source_ip.clone(),
        // Extraer el dato del Header
        ConditionField::Header(name) => ctx.headers.get(name).cloned(),
    }
}
```

**Java (sealed classes, Java 17+):**
```java
public sealed interface ConditionField
    permits Application, Profile, Label, PropertyPath, SourceIp, Header {
}

public record Application() implements ConditionField {}
public record Profile() implements ConditionField {}
public record Label() implements ConditionField {}
public record PropertyPath() implements ConditionField {}
public record SourceIp() implements ConditionField {}
public record Header(String name) implements ConditionField {}

// Uso con pattern matching (Java 21+)
String getFieldValue(ConditionField field, Context ctx) {
    return switch (field) {
        case Application a -> ctx.getApp();
        case Profile p -> ctx.getProfile();
        case Label l -> ctx.getLabel();
        case PropertyPath pp -> null;
        case SourceIp ip -> ctx.getSourceIp();
        case Header(String name) -> ctx.getHeaders().get(name);
    };
}
```

**Ventajas de Rust:**
- Syntax mas compacta
- Match exhaustivo forzado por compilador
- Serde serializa automaticamente
- Mejor optimizacion de memoria

### 3. Serde Derive y Atributos

Serde automatiza la serializacion con derives y atributos.

**Rust:**
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]  // JSON: "action_type" en vez de "actionType"
pub struct Action {
    #[serde(rename = "type")]  // JSON: "type" en vez de "action_type"
    pub action_type: ActionType,

    #[serde(skip_serializing_if = "Option::is_none")]  // Omitir si None
    pub message: Option<String>,

    #[serde(default = "default_mask_char")]  // Valor por defecto al deserializar
    pub mask_char: char,
}

fn default_mask_char() -> char {
    '*'
}

// Serializa a:
// { "type": "mask", "mask_char": "*" }
// (message omitido porque es None)
```

**Java con Jackson:**
```java
public class Action {
    @JsonProperty("type")
    private ActionType actionType;

    @JsonInclude(JsonInclude.Include.NON_NULL)
    private String message;

    @JsonProperty(defaultValue = "*")
    private char maskChar = '*';

    // Getters, setters, constructors...
}
```

**Diferencias:**
| Aspecto | Serde (Rust) | Jackson (Java) |
|---------|--------------|----------------|
| Configuracion | Atributos en struct | Anotaciones en clase |
| Defaults | Funciones custom | Valores literales |
| Code generation | Compile-time (proc macro) | Runtime (reflection) |
| Performance | Cero overhead | Overhead de reflection |

### 4. thiserror para Errores

`thiserror` genera implementaciones de Error automaticamente.

**Rust:**
```rust
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PolicyBuildError {
    #[error("Policy name is required")]
    MissingName,

    #[error("Policy action is required")]
    MissingAction,

    #[error("Policy must have at least one condition")]
    NoConditions,

    #[error("Invalid operator {0} for field {1}")]
    InvalidOperator(Operator, ConditionField),
}

// Uso
fn example() -> Result<Policy, PolicyBuildError> {
    Err(PolicyBuildError::MissingName)
}
```

**Java equivalente:**
```java
public class PolicyBuildException extends Exception {
    public static PolicyBuildException missingName() {
        return new PolicyBuildException("Policy name is required");
    }

    public static PolicyBuildException missingAction() {
        return new PolicyBuildException("Policy action is required");
    }

    public static PolicyBuildException invalidOperator(Operator op, ConditionField field) {
        return new PolicyBuildException(
            String.format("Invalid operator %s for field %s", op, field)
        );
    }

    private PolicyBuildException(String message) {
        super(message);
    }
}
```

---

## Riesgos y Errores Comunes

### 1. Olvidar Derive de Clone/Debug

```rust
// MAL: No se puede clonar ni debuggear
#[derive(Serialize, Deserialize)]
pub struct Policy {
    // ...
}

// BIEN: Siempre incluir Clone y Debug
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    // ...
}
```

### 2. Serde rename_all Inconsistente

```rust
// MAL: Mezcla de naming conventions
#[derive(Serialize, Deserialize)]
pub enum ActionType {
    Deny,           // Serializa como "Deny"
    redact,         // Error de compilacion!
}

// BIEN: Usar rename_all consistente
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Deny,           // Serializa como "deny"
    Redact,         // Serializa como "redact"
}
```

### 3. Builder que No Consume Self

```rust
// MAL: Builder con &mut self permite estados invalidos
impl PolicyBuilder {
    pub fn priority(&mut self, p: u32) -> &mut Self {
        self.priority = p;
        self
    }

    pub fn build(&self) -> Policy {
        // Builder sigue disponible despues de build!
        // Puede causar bugs
    }
}

// BIEN: Consumir self
impl PolicyBuilder {
    pub fn priority(mut self, p: u32) -> Self {
        self.priority = p;
        self
    }

    pub fn build(self) -> Result<Policy, Error> {
        // Builder consumido, no se puede reusar
    }
}
```

### 4. Option vs Default

```rust
// MAL: Option cuando hay default logico
pub struct Action {
    pub mask_char: Option<char>,  // None significa...?
}

// MEJOR: Default explicito con serde
pub struct Action {
    #[serde(default = "default_mask_char")]
    pub mask_char: char,  // Siempre tiene valor
}

fn default_mask_char() -> char { '*' }

// O usando Option con skip_serializing_if
pub struct Action {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask_char: Option<char>,  // None = usar default en runtime
}
```

---

## Pruebas

### Tests de Serializacion

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_type_serialization() {
        let action_type = ActionType::Deny;
        let json = serde_json::to_string(&action_type).unwrap();
        assert_eq!(json, "\"deny\"");

        let deserialized: ActionType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ActionType::Deny);
    }

    #[test]
    fn test_condition_field_with_header() {
        let field = ConditionField::Header("X-Api-Key".to_string());
        let yaml = serde_yaml::to_string(&field).unwrap();
        assert!(yaml.contains("header"));
        assert!(yaml.contains("X-Api-Key"));
    }

    #[test]
    fn test_policy_yaml_round_trip() {
        let yaml = r#"
name: test-policy
description: A test policy
priority: 100
enabled: true
conditions:
  - field: application
    operator: equals
    value: myapp
action:
  type: deny
  message: Access denied
"#;
        let policy: Policy = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(policy.name, "test-policy");
        assert_eq!(policy.priority, 100);
        assert!(policy.enabled);
        assert_eq!(policy.conditions.len(), 1);

        let serialized = serde_yaml::to_string(&policy).unwrap();
        let reparsed: Policy = serde_yaml::from_str(&serialized).unwrap();
        assert_eq!(policy, reparsed);
    }
}
```

### Tests del Builder

```rust
#[cfg(test)]
mod builder_tests {
    use super::*;

    #[test]
    fn test_policy_builder_success() {
        let policy = PolicyBuilder::new("test")
            .description("Test policy")
            .priority(200)
            .condition(ConditionField::Application, Operator::Equals, "myapp")
            .action_deny("Denied")
            .build()
            .unwrap();

        assert_eq!(policy.name, "test");
        assert_eq!(policy.description, "Test policy");
        assert_eq!(policy.priority, 200);
        assert!(policy.enabled);
        assert_eq!(policy.conditions.len(), 1);
    }

    #[test]
    fn test_policy_builder_missing_action() {
        let result = PolicyBuilder::new("test")
            .condition(ConditionField::Profile, Operator::Equals, "prod")
            .build();

        assert!(matches!(result, Err(PolicyBuildError::MissingAction)));
    }

    #[test]
    fn test_policy_builder_no_conditions() {
        let result = PolicyBuilder::new("test")
            .action_deny("Denied")
            .build();

        assert!(matches!(result, Err(PolicyBuildError::NoConditions)));
    }

    #[test]
    fn test_multiple_conditions() {
        let policy = PolicyBuilder::new("multi")
            .condition(ConditionField::Profile, Operator::Equals, "prod")
            .condition(ConditionField::Application, Operator::Matches, "internal-.*")
            .condition(ConditionField::SourceIp, Operator::InCidr, "10.0.0.0/8")
            .action_mask('*', 4)
            .build()
            .unwrap();

        assert_eq!(policy.conditions.len(), 3);
    }
}
```

### Tests de PolicySet

```rust
#[cfg(test)]
mod policy_set_tests {
    use super::*;

    fn create_test_policy(name: &str, priority: u32, enabled: bool) -> Policy {
        PolicyBuilder::new(name)
            .priority(priority)
            .enabled(enabled)
            .condition(ConditionField::Application, Operator::Equals, "test")
            .action_deny("test")
            .build()
            .unwrap()
    }

    #[test]
    fn test_sorted_by_priority() {
        let mut set = PolicySet::new();
        set.add_policy(create_test_policy("low", 10, true));
        set.add_policy(create_test_policy("high", 100, true));
        set.add_policy(create_test_policy("medium", 50, true));

        let sorted = set.sorted_by_priority();
        assert_eq!(sorted[0].name, "high");
        assert_eq!(sorted[1].name, "medium");
        assert_eq!(sorted[2].name, "low");
    }

    #[test]
    fn test_active_policies_filters_disabled() {
        let mut set = PolicySet::new();
        set.add_policy(create_test_policy("enabled", 100, true));
        set.add_policy(create_test_policy("disabled", 200, false));

        let active = set.active_policies();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "enabled");
    }

    #[test]
    fn test_get_by_name() {
        let mut set = PolicySet::new();
        set.add_policy(create_test_policy("find-me", 100, true));

        assert!(set.get_by_name("find-me").is_some());
        assert!(set.get_by_name("not-found").is_none());
    }
}
```

---

## Seguridad

- **No almacenar secretos en politicas**: Las politicas definen reglas, no contienen valores sensibles
- **Validacion de regex**: Los patrones regex en condiciones deben validarse para evitar ReDoS
- **Inmutabilidad**: Policy y Action son inmutables despues de construccion
- **Clone explicito**: Clonar politicas es explicito (no hay copias implicitas)

---

## Entregable Final

### Archivos Creados

1. `crates/vortex-governance/Cargo.toml` - Dependencias del crate
2. `crates/vortex-governance/src/lib.rs` - Re-exports publicos
3. `crates/vortex-governance/src/plac/mod.rs` - Modulo PLAC
4. `crates/vortex-governance/src/plac/model.rs` - Definiciones de tipos
5. `crates/vortex-governance/src/plac/builder.rs` - Builder para Policy
6. `crates/vortex-governance/src/plac/error.rs` - Tipos de error

### Verificacion

```bash
# Compilar el crate
cargo build -p vortex-governance

# Ejecutar tests
cargo test -p vortex-governance

# Verificar documentacion
cargo doc -p vortex-governance --open

# Clippy
cargo clippy -p vortex-governance -- -D warnings
```

### API Publica

```rust
// src/lib.rs
pub mod plac;

// Re-exports principales
pub use plac::{
    Policy, PolicySet, PolicyBuilder, PolicyBuildError,
    Condition, ConditionField, Operator,
    Action, ActionType,
};
```
