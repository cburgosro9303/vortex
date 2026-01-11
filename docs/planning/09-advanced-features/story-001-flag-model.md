# Historia 001: Modelo de Feature Flags

## Contexto y Objetivo

Los feature flags permiten activar o desactivar funcionalidades en tiempo de ejecucion sin necesidad de redeployment. Esta historia define el modelo de dominio para feature flags en Vortex Config, incluyendo:

- **Flag definitions**: Estructura de un flag con variantes
- **Targeting rules**: Reglas para determinar que usuarios ven que variante
- **Variants**: Los diferentes valores que puede retornar un flag

Para desarrolladores Java, este modelo es similar a lo que ofrece LaunchDarkly o Split.io, pero integrado directamente en el servidor de configuracion.

El enfoque de Rust para modelar dominios usando enums con datos y serde tagging proporciona una forma type-safe y expresiva de representar las variantes y reglas de targeting.

---

## Alcance

### In Scope

- `FeatureFlag` struct con metadata y variantes
- `Variant` enum para boolean, string, number, JSON
- `TargetingRule` para condiciones de evaluacion
- `Condition` enum para diferentes tipos de matching
- Serialization/deserialization con serde

### Out of Scope

- Evaluacion de flags (historia 002)
- API REST (historia 003)
- Almacenamiento de flags (usa backend existente)
- UI de administracion

---

## Criterios de Aceptacion

- [ ] `FeatureFlag` con id, name, description, enabled, variants, rules
- [ ] `Variant` soporta boolean, string, number, JSON
- [ ] `TargetingRule` con conditions y variant outcome
- [ ] `Condition` soporta: equals, in_list, percentage, regex, semver
- [ ] Serializa/deserializa a YAML/JSON correctamente
- [ ] Serde tagging funciona para discriminar variantes
- [ ] Default variant especificado para fallback
- [ ] Tests de serializacion/deserializacion pasan

---

## Diseno Propuesto

### Arquitectura del Modelo

```
┌─────────────────────────────────────────────────────────────────────┐
│                         FeatureFlag                                  │
├─────────────────────────────────────────────────────────────────────┤
│  id: String                                                          │
│  name: String                                                        │
│  description: Option<String>                                         │
│  enabled: bool                                                       │
│  variants: Vec<FlagVariant>                                          │
│  rules: Vec<TargetingRule>                                           │
│  default_variant: String                                             │
│  created_at: DateTime<Utc>                                           │
│  updated_at: DateTime<Utc>                                           │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                   ┌────────────────┴────────────────┐
                   ▼                                 ▼
┌──────────────────────────────┐    ┌──────────────────────────────────┐
│        FlagVariant           │    │         TargetingRule            │
├──────────────────────────────┤    ├──────────────────────────────────┤
│  id: String                  │    │  id: String                      │
│  name: String                │    │  description: Option<String>     │
│  value: VariantValue         │    │  conditions: Vec<Condition>      │
│                              │    │  variant_id: String              │
└──────────────────────────────┘    │  priority: i32                   │
                                    └──────────────────────────────────┘
           │                                         │
           ▼                                         ▼
┌──────────────────────────────┐    ┌──────────────────────────────────┐
│       VariantValue           │    │          Condition               │
├──────────────────────────────┤    ├──────────────────────────────────┤
│  Boolean(bool)               │    │  attribute: String               │
│  String(String)              │    │  operator: Operator              │
│  Number(f64)                 │    │  values: Vec<String>             │
│  Json(serde_json::Value)     │    │                                  │
└──────────────────────────────┘    └──────────────────────────────────┘
                                                     │
                                                     ▼
                                    ┌──────────────────────────────────┐
                                    │           Operator               │
                                    ├──────────────────────────────────┤
                                    │  Equals                          │
                                    │  NotEquals                       │
                                    │  InList                          │
                                    │  NotInList                       │
                                    │  Contains                        │
                                    │  StartsWith                      │
                                    │  EndsWith                        │
                                    │  Regex                           │
                                    │  GreaterThan                     │
                                    │  LessThan                        │
                                    │  SemverGt                        │
                                    │  SemverLt                        │
                                    │  Percentage                      │
                                    └──────────────────────────────────┘
```

### Ejemplo de Flag en YAML

```yaml
feature_flags:
  - id: "new-checkout-flow"
    name: "New Checkout Flow"
    description: "Redesigned checkout experience"
    enabled: true
    default_variant: "control"
    variants:
      - id: "control"
        name: "Control Group"
        value:
          type: "boolean"
          data: false
      - id: "treatment"
        name: "Treatment Group"
        value:
          type: "boolean"
          data: true
    rules:
      - id: "beta-testers"
        description: "Enable for beta testers"
        priority: 100
        variant_id: "treatment"
        conditions:
          - attribute: "user_group"
            operator: "in_list"
            values: ["beta", "internal"]
      - id: "gradual-rollout"
        description: "30% rollout"
        priority: 50
        variant_id: "treatment"
        conditions:
          - attribute: "user_id"
            operator: "percentage"
            values: ["30"]
```

---

## Pasos de Implementacion

### Paso 1: Definir VariantValue con Serde Tagging

```rust
// src/flags/model.rs
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// The value that a flag variant returns.
/// Uses serde tagging for polymorphic serialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum VariantValue {
    /// Boolean value (true/false)
    Boolean(bool),
    /// String value
    String(String),
    /// Numeric value (f64 covers all JSON numbers)
    Number(f64),
    /// Arbitrary JSON value
    Json(JsonValue),
}

impl VariantValue {
    /// Creates a boolean variant value.
    pub fn boolean(value: bool) -> Self {
        Self::Boolean(value)
    }

    /// Creates a string variant value.
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    /// Creates a numeric variant value.
    pub fn number(value: f64) -> Self {
        Self::Number(value)
    }

    /// Creates a JSON variant value.
    pub fn json(value: JsonValue) -> Self {
        Self::Json(value)
    }

    /// Returns the value as a boolean, if it is one.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Returns the value as a string, if it is one.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }
}

impl Default for VariantValue {
    fn default() -> Self {
        Self::Boolean(false)
    }
}
```

### Paso 2: Definir FlagVariant

```rust
// src/flags/model.rs (continuacion)

/// A specific variant of a feature flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlagVariant {
    /// Unique identifier for this variant.
    pub id: String,

    /// Human-readable name.
    pub name: String,

    /// The value returned when this variant is selected.
    pub value: VariantValue,

    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Weight for random distribution (0-100).
    #[serde(default)]
    pub weight: u8,
}

impl FlagVariant {
    /// Creates a new boolean variant.
    pub fn boolean(id: impl Into<String>, name: impl Into<String>, value: bool) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            value: VariantValue::Boolean(value),
            description: None,
            weight: 0,
        }
    }

    /// Creates a new string variant.
    pub fn string(
        id: impl Into<String>,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            value: VariantValue::String(value.into()),
            description: None,
            weight: 0,
        }
    }

    /// Adds a description to the variant.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the weight for random distribution.
    pub fn with_weight(mut self, weight: u8) -> Self {
        self.weight = weight;
        self
    }
}
```

### Paso 3: Definir Operadores de Condicion

```rust
// src/flags/model.rs (continuacion)

/// Operators for targeting conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    /// Exact equality.
    Equals,
    /// Not equal.
    NotEquals,
    /// Value is in the provided list.
    InList,
    /// Value is not in the provided list.
    NotInList,
    /// String contains substring.
    Contains,
    /// String starts with prefix.
    StartsWith,
    /// String ends with suffix.
    EndsWith,
    /// Regex match.
    Regex,
    /// Numeric greater than.
    GreaterThan,
    /// Numeric greater than or equal.
    GreaterThanOrEqual,
    /// Numeric less than.
    LessThan,
    /// Numeric less than or equal.
    LessThanOrEqual,
    /// Semantic version greater than.
    SemverGreaterThan,
    /// Semantic version less than.
    SemverLessThan,
    /// Percentage-based rollout (consistent hashing).
    Percentage,
}

impl Operator {
    /// Returns true if this operator requires multiple values.
    pub fn requires_list(&self) -> bool {
        matches!(self, Self::InList | Self::NotInList)
    }

    /// Returns true if this operator works with numeric values.
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Self::GreaterThan
                | Self::GreaterThanOrEqual
                | Self::LessThan
                | Self::LessThanOrEqual
        )
    }
}
```

### Paso 4: Definir Condition

```rust
// src/flags/model.rs (continuacion)

/// A condition for targeting rules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    /// The attribute to evaluate (e.g., "user_id", "environment").
    pub attribute: String,

    /// The comparison operator.
    pub operator: Operator,

    /// Values to compare against.
    /// For percentage, this is a single value like ["30"] for 30%.
    pub values: Vec<String>,

    /// Whether to negate the condition.
    #[serde(default)]
    pub negate: bool,
}

impl Condition {
    /// Creates a new equality condition.
    pub fn equals(attribute: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            attribute: attribute.into(),
            operator: Operator::Equals,
            values: vec![value.into()],
            negate: false,
        }
    }

    /// Creates a new "in list" condition.
    pub fn in_list(attribute: impl Into<String>, values: Vec<String>) -> Self {
        Self {
            attribute: attribute.into(),
            operator: Operator::InList,
            values,
            negate: false,
        }
    }

    /// Creates a percentage rollout condition.
    pub fn percentage(attribute: impl Into<String>, percent: u8) -> Self {
        Self {
            attribute: attribute.into(),
            operator: Operator::Percentage,
            values: vec![percent.to_string()],
            negate: false,
        }
    }

    /// Negates this condition.
    pub fn negated(mut self) -> Self {
        self.negate = true;
        self
    }
}
```

### Paso 5: Definir TargetingRule

```rust
// src/flags/model.rs (continuacion)

/// A rule that determines which variant to serve based on conditions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetingRule {
    /// Unique identifier for this rule.
    pub id: String,

    /// Human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Conditions that must ALL be true for this rule to match.
    /// Empty conditions means the rule always matches.
    pub conditions: Vec<Condition>,

    /// The variant to serve when this rule matches.
    pub variant_id: String,

    /// Priority (higher = evaluated first).
    #[serde(default)]
    pub priority: i32,

    /// Whether this rule is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl TargetingRule {
    /// Creates a new targeting rule.
    pub fn new(
        id: impl Into<String>,
        variant_id: impl Into<String>,
        conditions: Vec<Condition>,
    ) -> Self {
        Self {
            id: id.into(),
            description: None,
            conditions,
            variant_id: variant_id.into(),
            priority: 0,
            enabled: true,
        }
    }

    /// Creates a rule that always matches (catch-all).
    pub fn catch_all(id: impl Into<String>, variant_id: impl Into<String>) -> Self {
        Self::new(id, variant_id, vec![])
    }

    /// Adds a description to the rule.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the priority.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}
```

### Paso 6: Definir FeatureFlag

```rust
// src/flags/model.rs (continuacion)
use chrono::{DateTime, Utc};

/// A feature flag definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureFlag {
    /// Unique identifier (e.g., "new-checkout-flow").
    pub id: String,

    /// Human-readable name.
    pub name: String,

    /// Description of what this flag controls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether the flag is globally enabled.
    /// If false, always returns the default variant.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Available variants for this flag.
    pub variants: Vec<FlagVariant>,

    /// Targeting rules (evaluated in priority order).
    #[serde(default)]
    pub rules: Vec<TargetingRule>,

    /// The variant to return when no rules match.
    pub default_variant: String,

    /// Tags for organization.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Creation timestamp.
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,

    /// Last update timestamp.
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
}

impl FeatureFlag {
    /// Creates a new boolean feature flag.
    pub fn boolean(id: impl Into<String>, name: impl Into<String>, default: bool) -> Self {
        let id = id.into();
        let default_variant = if default { "on" } else { "off" };

        Self {
            id,
            name: name.into(),
            description: None,
            enabled: true,
            variants: vec![
                FlagVariant::boolean("off", "Off", false),
                FlagVariant::boolean("on", "On", true),
            ],
            rules: vec![],
            default_variant: default_variant.to_string(),
            tags: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Gets a variant by ID.
    pub fn get_variant(&self, variant_id: &str) -> Option<&FlagVariant> {
        self.variants.iter().find(|v| v.id == variant_id)
    }

    /// Gets the default variant.
    pub fn get_default_variant(&self) -> Option<&FlagVariant> {
        self.get_variant(&self.default_variant)
    }

    /// Adds a description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self.updated_at = Utc::now();
        self
    }

    /// Adds a targeting rule.
    pub fn with_rule(mut self, rule: TargetingRule) -> Self {
        self.rules.push(rule);
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        self.updated_at = Utc::now();
        self
    }

    /// Validates the flag configuration.
    pub fn validate(&self) -> Result<(), FlagValidationError> {
        // Must have at least one variant
        if self.variants.is_empty() {
            return Err(FlagValidationError::NoVariants);
        }

        // Default variant must exist
        if self.get_default_variant().is_none() {
            return Err(FlagValidationError::InvalidDefaultVariant(
                self.default_variant.clone(),
            ));
        }

        // All rule variant_ids must exist
        for rule in &self.rules {
            if self.get_variant(&rule.variant_id).is_none() {
                return Err(FlagValidationError::InvalidRuleVariant {
                    rule_id: rule.id.clone(),
                    variant_id: rule.variant_id.clone(),
                });
            }
        }

        Ok(())
    }
}

/// Errors in flag configuration.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FlagValidationError {
    #[error("flag must have at least one variant")]
    NoVariants,

    #[error("default variant '{0}' does not exist")]
    InvalidDefaultVariant(String),

    #[error("rule '{rule_id}' references non-existent variant '{variant_id}'")]
    InvalidRuleVariant { rule_id: String, variant_id: String },
}
```

### Paso 7: Feature Flag Collection

```rust
// src/flags/model.rs (continuacion)

/// A collection of feature flags, typically stored per application/environment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeatureFlagCollection {
    /// The flags in this collection.
    #[serde(default)]
    pub flags: Vec<FeatureFlag>,
}

impl FeatureFlagCollection {
    /// Creates an empty collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets a flag by ID.
    pub fn get(&self, flag_id: &str) -> Option<&FeatureFlag> {
        self.flags.iter().find(|f| f.id == flag_id)
    }

    /// Adds a flag to the collection.
    pub fn add(&mut self, flag: FeatureFlag) {
        // Remove existing flag with same ID
        self.flags.retain(|f| f.id != flag.id);
        self.flags.push(flag);
    }

    /// Removes a flag by ID.
    pub fn remove(&mut self, flag_id: &str) -> Option<FeatureFlag> {
        let idx = self.flags.iter().position(|f| f.id == flag_id)?;
        Some(self.flags.remove(idx))
    }

    /// Validates all flags in the collection.
    pub fn validate(&self) -> Result<(), Vec<(String, FlagValidationError)>> {
        let errors: Vec<_> = self
            .flags
            .iter()
            .filter_map(|f| f.validate().err().map(|e| (f.id.clone(), e)))
            .collect();

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
```

---

## Conceptos de Rust Aprendidos

### 1. Serde Tagging para Polimorfismo

Serde permite serializar enums con datos de varias formas. El "tagging" define como se discrimina el tipo.

**Rust:**
```rust
// Tagged representation (recomendado para APIs)
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum VariantValue {
    Boolean(bool),
    String(String),
    Number(f64),
    Json(serde_json::Value),
}

// Serializa como:
// { "type": "boolean", "data": true }
// { "type": "string", "data": "hello" }
// { "type": "number", "data": 42.5 }

// Alternativas:
#[serde(tag = "type")]           // Internally tagged (data inline)
#[serde(untagged)]               // No tag (ambiguous, orden importa)
#[serde(tag = "t", content = "c")] // Custom field names
```

**Comparacion con Java (Jackson):**
```java
// Jackson usa @JsonTypeInfo para polimorfismo
@JsonTypeInfo(
    use = JsonTypeInfo.Id.NAME,
    include = JsonTypeInfo.As.PROPERTY,
    property = "type"
)
@JsonSubTypes({
    @JsonSubTypes.Type(value = BooleanValue.class, name = "boolean"),
    @JsonSubTypes.Type(value = StringValue.class, name = "string"),
    @JsonSubTypes.Type(value = NumberValue.class, name = "number"),
})
public abstract class VariantValue { }

public class BooleanValue extends VariantValue {
    private boolean data;
}

// Serializa como:
// { "type": "boolean", "data": true }
```

**Diferencias clave:**
| Aspecto | Rust (Serde) | Java (Jackson) |
|---------|--------------|----------------|
| Tipo base | enum (sum type) | abstract class |
| Variantes | Automaticas | @JsonSubTypes manual |
| Compile-time | Exhaustivo | Runtime exceptions |
| Boilerplate | Minimo | Considerable |

### 2. Enums con Datos (Sum Types)

Los enums de Rust pueden contener datos, algo similar a sealed classes en Java 17+.

**Rust:**
```rust
// Cada variante puede tener diferentes datos
pub enum VariantValue {
    Boolean(bool),                    // Un bool
    String(String),                   // Un String
    Number(f64),                      // Un f64
    Json(serde_json::Value),          // Un Value arbitrario
}

// Pattern matching exhaustivo
fn describe(value: &VariantValue) -> String {
    match value {
        VariantValue::Boolean(b) => format!("bool: {}", b),
        VariantValue::String(s) => format!("string: {}", s),
        VariantValue::Number(n) => format!("number: {}", n),
        VariantValue::Json(j) => format!("json: {}", j),
    }
    // Si agregas una variante, el compilador te obliga a manejarla
}
```

**Comparacion con Java (Sealed Classes):**
```java
// Java 17+ sealed classes
public sealed interface VariantValue
    permits BooleanValue, StringValue, NumberValue, JsonValue {
}

public record BooleanValue(boolean data) implements VariantValue {}
public record StringValue(String data) implements VariantValue {}
public record NumberValue(double data) implements VariantValue {}
public record JsonValue(JsonNode data) implements VariantValue {}

// Pattern matching con switch (Java 21+)
String describe(VariantValue value) {
    return switch (value) {
        case BooleanValue(var b) -> "bool: " + b;
        case StringValue(var s) -> "string: " + s;
        case NumberValue(var n) -> "number: " + n;
        case JsonValue(var j) -> "json: " + j;
    };
}
```

### 3. Builder Pattern con Method Chaining

Rust permite construir objetos de forma fluida usando method chaining.

**Rust:**
```rust
// Los metodos toman self y retornan Self
impl FlagVariant {
    pub fn boolean(id: impl Into<String>, name: impl Into<String>, value: bool) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            value: VariantValue::Boolean(value),
            description: None,
            weight: 0,
        }
    }

    // Method chaining: consume self, retorna Self modificado
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_weight(mut self, weight: u8) -> Self {
        self.weight = weight;
        self
    }
}

// Uso fluido
let variant = FlagVariant::boolean("on", "Enabled", true)
    .with_description("Variant for enabled state")
    .with_weight(50);
```

**Comparacion con Java Builder:**
```java
// Patron Builder tradicional
public class FlagVariant {
    public static Builder builder() {
        return new Builder();
    }

    public static class Builder {
        private String id;
        private String name;
        // ...

        public Builder id(String id) {
            this.id = id;
            return this;
        }

        public Builder description(String description) {
            this.description = description;
            return this;
        }

        public FlagVariant build() {
            return new FlagVariant(this);
        }
    }
}

// Uso
FlagVariant variant = FlagVariant.builder()
    .id("on")
    .name("Enabled")
    .description("Variant for enabled state")
    .build();
```

### 4. Validacion con Result

**Rust:**
```rust
impl FeatureFlag {
    pub fn validate(&self) -> Result<(), FlagValidationError> {
        if self.variants.is_empty() {
            return Err(FlagValidationError::NoVariants);
        }

        if self.get_default_variant().is_none() {
            return Err(FlagValidationError::InvalidDefaultVariant(
                self.default_variant.clone(),
            ));
        }

        Ok(())
    }
}

// Uso
match flag.validate() {
    Ok(()) => println!("Flag is valid"),
    Err(FlagValidationError::NoVariants) => println!("No variants defined"),
    Err(FlagValidationError::InvalidDefaultVariant(v)) => {
        println!("Invalid default: {}", v)
    }
    Err(e) => println!("Validation error: {}", e),
}
```

**Comparacion con Java:**
```java
// Java: excepciones o validation framework
public class FeatureFlag {
    public void validate() throws FlagValidationException {
        if (variants.isEmpty()) {
            throw new NoVariantsException();
        }
        if (getDefaultVariant() == null) {
            throw new InvalidDefaultVariantException(defaultVariant);
        }
    }
}

// O con JSR-380 Bean Validation
public class FeatureFlag {
    @NotEmpty(message = "Must have at least one variant")
    private List<FlagVariant> variants;
}
```

---

## Riesgos y Errores Comunes

### 1. Olvidar Validar antes de Usar

```rust
// MAL: Usar flag sin validar
let flag = load_flag_from_yaml()?;
let variant = flag.get_default_variant().unwrap();  // Panic si no existe!

// BIEN: Validar primero
let flag = load_flag_from_yaml()?;
flag.validate()?;  // Retorna error si invalido
let variant = flag.get_default_variant().expect("validated");
```

### 2. Serde Tag Inconsistente

```rust
// MAL: Cambiar el tag rompe deserializacion de datos existentes
#[serde(tag = "type")]  // Antes era "kind"
pub enum VariantValue { ... }

// BIEN: Usar rename para compatibilidad
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum VariantValue {
    #[serde(alias = "bool")]  // Acepta "bool" legacy
    Boolean(bool),
}
```

### 3. Reglas sin Ordenar por Prioridad

```rust
// MAL: Las reglas pueden estar desordenadas
impl FeatureFlag {
    pub fn with_rule(mut self, rule: TargetingRule) -> Self {
        self.rules.push(rule);  // No ordena!
        self
    }
}

// BIEN: Mantener ordenadas por prioridad
impl FeatureFlag {
    pub fn with_rule(mut self, rule: TargetingRule) -> Self {
        self.rules.push(rule);
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        self
    }
}
```

### 4. Clone Innecesario con Into

```rust
// MAL: Clone + Into
fn with_description(mut self, description: String) -> Self {
    self.description = Some(description);
    self
}
let v = variant.with_description(my_string.clone());

// BIEN: Usar impl Into para flexibilidad
fn with_description(mut self, description: impl Into<String>) -> Self {
    self.description = Some(description.into());
    self
}
let v1 = variant.with_description("static str");  // &str
let v2 = variant.with_description(my_string);     // String (moved)
```

---

## Pruebas

### Tests de Serializacion

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn variant_value_serializes_with_tag() {
        let boolean = VariantValue::Boolean(true);
        let json = serde_json::to_value(&boolean).unwrap();

        assert_eq!(json, json!({
            "type": "boolean",
            "data": true
        }));
    }

    #[test]
    fn variant_value_deserializes_from_tagged_json() {
        let json = json!({
            "type": "string",
            "data": "hello"
        });

        let value: VariantValue = serde_json::from_value(json).unwrap();
        assert_eq!(value, VariantValue::String("hello".to_string()));
    }

    #[test]
    fn feature_flag_roundtrips_through_yaml() {
        let flag = FeatureFlag::boolean("test-flag", "Test Flag", true)
            .with_description("A test flag")
            .with_rule(TargetingRule::new(
                "beta",
                "on",
                vec![Condition::in_list("user_group", vec!["beta".to_string()])],
            ));

        let yaml = serde_yaml::to_string(&flag).unwrap();
        let deserialized: FeatureFlag = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(flag.id, deserialized.id);
        assert_eq!(flag.rules.len(), deserialized.rules.len());
    }

    #[test]
    fn flag_validation_catches_no_variants() {
        let flag = FeatureFlag {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: None,
            enabled: true,
            variants: vec![],  // Empty!
            rules: vec![],
            default_variant: "off".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let result = flag.validate();
        assert!(matches!(result, Err(FlagValidationError::NoVariants)));
    }

    #[test]
    fn flag_validation_catches_invalid_default() {
        let flag = FeatureFlag::boolean("test", "Test", true);
        let flag = FeatureFlag {
            default_variant: "nonexistent".to_string(),
            ..flag
        };

        let result = flag.validate();
        assert!(matches!(
            result,
            Err(FlagValidationError::InvalidDefaultVariant(_))
        ));
    }

    #[test]
    fn condition_percentage_created_correctly() {
        let condition = Condition::percentage("user_id", 30);

        assert_eq!(condition.attribute, "user_id");
        assert_eq!(condition.operator, Operator::Percentage);
        assert_eq!(condition.values, vec!["30"]);
    }

    #[test]
    fn targeting_rule_sorts_by_priority() {
        let mut flag = FeatureFlag::boolean("test", "Test", false);

        flag = flag
            .with_rule(TargetingRule::new("low", "on", vec![]).with_priority(10))
            .with_rule(TargetingRule::new("high", "on", vec![]).with_priority(100))
            .with_rule(TargetingRule::new("medium", "on", vec![]).with_priority(50));

        assert_eq!(flag.rules[0].id, "high");
        assert_eq!(flag.rules[1].id, "medium");
        assert_eq!(flag.rules[2].id, "low");
    }
}
```

---

## Seguridad

### Consideraciones

1. **Datos sensibles en conditions**: No usar passwords/secrets como valores de condition
2. **Validacion de entrada**: Sanitizar flag IDs para evitar injection
3. **Serializacion segura**: No deserializar YAML de fuentes no confiables sin validacion

```rust
/// Validates that a flag ID is safe to use.
pub fn validate_flag_id(id: &str) -> Result<(), String> {
    // Solo alfanumericos, guiones y underscores
    let valid = id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_');

    if !valid {
        return Err(format!(
            "Flag ID '{}' contains invalid characters",
            id
        ));
    }

    if id.len() > 128 {
        return Err("Flag ID too long (max 128 chars)".to_string());
    }

    Ok(())
}
```

---

## Entregable Final

### Archivos Creados

1. `src/flags/mod.rs` - Re-exports del modulo
2. `src/flags/model.rs` - Tipos del dominio
3. `tests/flags_model_test.rs` - Tests del modelo

### Verificacion

```bash
# Compilar
cargo build -p vortex-features

# Tests
cargo test -p vortex-features model

# Doc
cargo doc -p vortex-features --open

# Clippy
cargo clippy -p vortex-features -- -D warnings
```

### Ejemplo de Uso

```rust
use vortex_features::flags::{
    FeatureFlag, FlagVariant, TargetingRule, Condition, VariantValue,
};

fn main() {
    // Crear un flag booleano simple
    let flag = FeatureFlag::boolean("dark-mode", "Dark Mode", false)
        .with_description("Enable dark mode UI")
        .with_rule(
            TargetingRule::new(
                "beta-users",
                "on",
                vec![Condition::in_list(
                    "user_group",
                    vec!["beta".to_string(), "internal".to_string()],
                )],
            )
            .with_priority(100),
        )
        .with_rule(
            TargetingRule::new(
                "gradual-rollout",
                "on",
                vec![Condition::percentage("user_id", 25)],
            )
            .with_priority(50),
        );

    // Validar
    flag.validate().expect("Flag should be valid");

    // Serializar a YAML
    let yaml = serde_yaml::to_string(&flag).unwrap();
    println!("Flag YAML:\n{}", yaml);
}
```

---

**Anterior**: [Indice de Epica 09](./index.md)
**Siguiente**: [Historia 002 - Evaluador de Feature Flags](./story-002-flag-evaluator.md)
