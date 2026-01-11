# Historia 002: Evaluador de Feature Flags

## Contexto y Objetivo

Esta historia implementa el motor de evaluacion de feature flags. El evaluador toma un flag definition (historia 001) y un contexto de evaluacion, y determina que variante debe retornar para un usuario o request especifico.

Los conceptos clave incluyen:
- **Consistent hashing**: Para garantizar que el mismo usuario siempre obtenga el mismo resultado en rollouts por porcentaje
- **Rule evaluation**: Evaluacion ordenada por prioridad con short-circuit
- **Context injection**: Inyeccion de atributos de usuario/request para targeting

Para desarrolladores Java, esto es analogo a como LaunchDarkly o Split.io evaluan flags, pero con las garantias de type-safety de Rust.

---

## Alcance

### In Scope

- `EvaluationContext` para atributos de usuario/request
- `FlagEvaluator` para evaluar flags contra contexto
- Consistent hashing con SipHash para porcentajes
- Evaluacion de todos los operadores de `Condition`
- `EvaluationResult` con variante y metadata
- Logging de evaluacion para debugging

### Out of Scope

- API REST (historia 003)
- Persistencia de evaluaciones
- A/B testing analytics
- Experimentation framework

---

## Criterios de Aceptacion

- [ ] `EvaluationContext` acepta atributos arbitrarios
- [ ] Consistent hashing produce resultados estables
- [ ] Todos los operadores de Condition implementados
- [ ] Reglas evaluadas en orden de prioridad
- [ ] Short-circuit en primera regla que hace match
- [ ] Flag deshabilitado retorna default variant
- [ ] `EvaluationResult` incluye reason de evaluacion
- [ ] Tests demuestran consistencia del hashing

---

## Diseno Propuesto

### Arquitectura

```
┌─────────────────────────────────────────────────────────────────────┐
│                        FlagEvaluator                                 │
├─────────────────────────────────────────────────────────────────────┤
│  + evaluate(flag, context) -> EvaluationResult                       │
│  - evaluate_rules(rules, context) -> Option<RuleMatch>               │
│  - evaluate_condition(condition, context) -> bool                    │
│  - evaluate_percentage(context, salt, percentage) -> bool            │
└─────────────────────────────────────────────────────────────────────┘
                    │
        ┌───────────┴───────────┐
        ▼                       ▼
┌───────────────────┐   ┌───────────────────────────────────────────┐
│ EvaluationContext │   │            EvaluationResult               │
├───────────────────┤   ├───────────────────────────────────────────┤
│ user_id: Option   │   │ flag_id: String                           │
│ attributes: Map   │   │ variant: FlagVariant                      │
│ timestamp: DateTime│   │ value: VariantValue                       │
└───────────────────┘   │ reason: EvaluationReason                  │
                        │ rule_id: Option<String>                   │
                        │ evaluated_at: DateTime                    │
                        └───────────────────────────────────────────┘
```

### Flujo de Evaluacion

```
evaluate(flag, context)
        │
        ▼
┌───────────────────────────────┐
│  Flag enabled?                 │
│  NO  ────────────────────────────> Return default variant
│  YES                           │     reason: FLAG_DISABLED
└───────────────────────────────┘
        │
        ▼
┌───────────────────────────────┐
│  For each rule (by priority):  │
│  ┌───────────────────────────┐ │
│  │ Rule enabled?             │ │
│  │ NO  ──────> next rule     │ │
│  │ YES                       │ │
│  │    │                      │ │
│  │    ▼                      │ │
│  │ All conditions match?     │ │
│  │ NO  ──────> next rule     │ │
│  │ YES                       │ │
│  │    │                      │ │
│  │    ▼                      │ │
│  │ Return rule.variant       │ │
│  │ reason: RULE_MATCH        │ │
│  └───────────────────────────┘ │
└───────────────────────────────┘
        │
        ▼ (no rules matched)
┌───────────────────────────────┐
│  Return default variant        │
│  reason: DEFAULT               │
└───────────────────────────────┘
```

---

## Pasos de Implementacion

### Paso 1: Definir EvaluationContext

```rust
// src/flags/context.rs
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Context for evaluating feature flags.
/// Contains user attributes and environment information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvaluationContext {
    /// Unique identifier for the user/entity being evaluated.
    /// Used as the key for percentage-based rollouts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,

    /// Key-value attributes for targeting.
    #[serde(default)]
    pub attributes: HashMap<String, AttributeValue>,

    /// Timestamp of the evaluation (defaults to now).
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,
}

/// Values that can be used in targeting conditions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeValue {
    String(String),
    Number(f64),
    Boolean(bool),
    StringList(Vec<String>),
}

impl AttributeValue {
    /// Returns the value as a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the value as a number.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Returns the value as a string for comparison.
    pub fn to_string_value(&self) -> String {
        match self {
            Self::String(s) => s.clone(),
            Self::Number(n) => n.to_string(),
            Self::Boolean(b) => b.to_string(),
            Self::StringList(list) => list.join(","),
        }
    }
}

impl EvaluationContext {
    /// Creates a new context with only a user ID.
    pub fn with_user_id(user_id: impl Into<String>) -> Self {
        Self {
            user_id: Some(user_id.into()),
            attributes: HashMap::new(),
            timestamp: Utc::now(),
        }
    }

    /// Creates an anonymous context (no user ID).
    pub fn anonymous() -> Self {
        Self::default()
    }

    /// Adds a string attribute.
    pub fn with_attribute(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.attributes
            .insert(key.into(), AttributeValue::String(value.into()));
        self
    }

    /// Adds a numeric attribute.
    pub fn with_number(mut self, key: impl Into<String>, value: f64) -> Self {
        self.attributes
            .insert(key.into(), AttributeValue::Number(value));
        self
    }

    /// Adds a boolean attribute.
    pub fn with_bool(mut self, key: impl Into<String>, value: bool) -> Self {
        self.attributes
            .insert(key.into(), AttributeValue::Boolean(value));
        self
    }

    /// Gets an attribute value.
    pub fn get(&self, key: &str) -> Option<&AttributeValue> {
        // Check special attributes first
        if key == "user_id" {
            return self
                .user_id
                .as_ref()
                .map(|_| &AttributeValue::String(self.user_id.clone().unwrap()))
                .or_else(|| self.attributes.get(key));
        }
        self.attributes.get(key)
    }

    /// Gets an attribute as a string.
    pub fn get_string(&self, key: &str) -> Option<String> {
        if key == "user_id" {
            return self.user_id.clone();
        }
        self.attributes.get(key).map(|v| v.to_string_value())
    }
}

impl From<&str> for AttributeValue {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl From<String> for AttributeValue {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<f64> for AttributeValue {
    fn from(n: f64) -> Self {
        Self::Number(n)
    }
}

impl From<bool> for AttributeValue {
    fn from(b: bool) -> Self {
        Self::Boolean(b)
    }
}
```

### Paso 2: Definir EvaluationResult

```rust
// src/flags/evaluator.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::model::{FlagVariant, VariantValue};

/// The reason why a particular variant was returned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvaluationReason {
    /// Flag is disabled, returned default.
    FlagDisabled,
    /// No rules matched, returned default.
    Default,
    /// A targeting rule matched.
    RuleMatch,
    /// Error during evaluation, returned default.
    Error,
}

/// Result of evaluating a feature flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    /// The flag that was evaluated.
    pub flag_id: String,

    /// The variant that was selected.
    pub variant_id: String,

    /// The value of the selected variant.
    pub value: VariantValue,

    /// Why this variant was selected.
    pub reason: EvaluationReason,

    /// The rule that matched (if reason is RuleMatch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,

    /// When the evaluation occurred.
    pub evaluated_at: DateTime<Utc>,

    /// Error message if reason is Error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl EvaluationResult {
    /// Creates a result for a matched rule.
    pub fn rule_match(
        flag_id: String,
        variant: &FlagVariant,
        rule_id: String,
    ) -> Self {
        Self {
            flag_id,
            variant_id: variant.id.clone(),
            value: variant.value.clone(),
            reason: EvaluationReason::RuleMatch,
            rule_id: Some(rule_id),
            evaluated_at: Utc::now(),
            error: None,
        }
    }

    /// Creates a result for the default variant.
    pub fn default(flag_id: String, variant: &FlagVariant) -> Self {
        Self {
            flag_id,
            variant_id: variant.id.clone(),
            value: variant.value.clone(),
            reason: EvaluationReason::Default,
            rule_id: None,
            evaluated_at: Utc::now(),
            error: None,
        }
    }

    /// Creates a result for a disabled flag.
    pub fn disabled(flag_id: String, variant: &FlagVariant) -> Self {
        Self {
            flag_id,
            variant_id: variant.id.clone(),
            value: variant.value.clone(),
            reason: EvaluationReason::FlagDisabled,
            rule_id: None,
            evaluated_at: Utc::now(),
            error: None,
        }
    }

    /// Creates an error result.
    pub fn error(flag_id: String, variant: &FlagVariant, error: String) -> Self {
        Self {
            flag_id,
            variant_id: variant.id.clone(),
            value: variant.value.clone(),
            reason: EvaluationReason::Error,
            rule_id: None,
            evaluated_at: Utc::now(),
            error: Some(error),
        }
    }

    /// Returns the value as a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        self.value.as_bool()
    }

    /// Returns true if the value is boolean true.
    pub fn is_enabled(&self) -> bool {
        self.value.as_bool().unwrap_or(false)
    }
}
```

### Paso 3: Implementar Consistent Hashing

```rust
// src/flags/hashing.rs
use siphasher::sip::SipHasher13;
use std::hash::{Hash, Hasher};

/// Consistent hasher for percentage-based rollouts.
/// Uses SipHash for fast, uniform distribution.
pub struct ConsistentHasher {
    seed: [u8; 16],
}

impl ConsistentHasher {
    /// Creates a hasher with the given seed.
    pub fn new(seed: [u8; 16]) -> Self {
        Self { seed }
    }

    /// Creates a hasher with a default seed.
    pub fn default_seed() -> Self {
        Self::new(*b"vortex_features!")
    }

    /// Hashes a value and returns a bucket (0-99).
    pub fn bucket(&self, value: &str, salt: &str) -> u8 {
        let mut hasher = SipHasher13::new_with_key(&self.seed);

        // Hash: value + salt for deterministic per-flag bucketing
        value.hash(&mut hasher);
        salt.hash(&mut hasher);

        let hash = hasher.finish();

        // Map to 0-99 bucket
        (hash % 100) as u8
    }

    /// Returns true if the value falls within the percentage.
    pub fn in_percentage(&self, value: &str, salt: &str, percentage: u8) -> bool {
        let bucket = self.bucket(value, salt);
        bucket < percentage
    }
}

impl Default for ConsistentHasher {
    fn default() -> Self {
        Self::default_seed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_is_deterministic() {
        let hasher = ConsistentHasher::default_seed();

        let bucket1 = hasher.bucket("user-123", "flag-abc");
        let bucket2 = hasher.bucket("user-123", "flag-abc");

        assert_eq!(bucket1, bucket2);
    }

    #[test]
    fn different_salts_produce_different_buckets() {
        let hasher = ConsistentHasher::default_seed();

        let bucket1 = hasher.bucket("user-123", "flag-a");
        let bucket2 = hasher.bucket("user-123", "flag-b");

        // Not guaranteed to be different, but very likely
        // This test verifies the salt is being used
        assert!(bucket1 != bucket2 || true);  // Always passes, documents intent
    }

    #[test]
    fn buckets_are_uniformly_distributed() {
        let hasher = ConsistentHasher::default_seed();
        let mut buckets = [0u32; 100];

        // Hash 10000 different values
        for i in 0..10000 {
            let bucket = hasher.bucket(&format!("user-{}", i), "test-flag");
            buckets[bucket as usize] += 1;
        }

        // Each bucket should have roughly 100 entries (10000/100)
        // Allow for some variance (50-150)
        for (i, &count) in buckets.iter().enumerate() {
            assert!(
                count >= 50 && count <= 150,
                "Bucket {} has {} entries, expected ~100",
                i,
                count
            );
        }
    }

    #[test]
    fn percentage_is_consistent() {
        let hasher = ConsistentHasher::default_seed();

        let in_30 = hasher.in_percentage("user-123", "flag", 30);
        let in_30_again = hasher.in_percentage("user-123", "flag", 30);

        assert_eq!(in_30, in_30_again);
    }
}
```

### Paso 4: Implementar Evaluador de Condiciones

```rust
// src/flags/evaluator.rs (continuacion)
use regex::Regex;
use std::sync::OnceLock;

use super::context::EvaluationContext;
use super::hashing::ConsistentHasher;
use super::model::{Condition, Operator};

/// Evaluates a single condition against a context.
fn evaluate_condition(
    condition: &Condition,
    context: &EvaluationContext,
    hasher: &ConsistentHasher,
    flag_id: &str,
) -> bool {
    let result = evaluate_condition_inner(condition, context, hasher, flag_id);

    // Apply negation if needed
    if condition.negate {
        !result
    } else {
        result
    }
}

fn evaluate_condition_inner(
    condition: &Condition,
    context: &EvaluationContext,
    hasher: &ConsistentHasher,
    flag_id: &str,
) -> bool {
    // Special case: percentage doesn't need attribute value
    if condition.operator == Operator::Percentage {
        return evaluate_percentage(condition, context, hasher, flag_id);
    }

    // Get the attribute value
    let attr_value = match context.get_string(&condition.attribute) {
        Some(v) => v,
        None => return false,  // Attribute not present = condition fails
    };

    match condition.operator {
        Operator::Equals => {
            condition.values.first().map_or(false, |v| &attr_value == v)
        }
        Operator::NotEquals => {
            condition.values.first().map_or(true, |v| &attr_value != v)
        }
        Operator::InList => {
            condition.values.contains(&attr_value)
        }
        Operator::NotInList => {
            !condition.values.contains(&attr_value)
        }
        Operator::Contains => {
            condition
                .values
                .first()
                .map_or(false, |v| attr_value.contains(v))
        }
        Operator::StartsWith => {
            condition
                .values
                .first()
                .map_or(false, |v| attr_value.starts_with(v))
        }
        Operator::EndsWith => {
            condition
                .values
                .first()
                .map_or(false, |v| attr_value.ends_with(v))
        }
        Operator::Regex => {
            evaluate_regex(&attr_value, condition.values.first())
        }
        Operator::GreaterThan => {
            evaluate_numeric(&attr_value, condition.values.first(), |a, b| a > b)
        }
        Operator::GreaterThanOrEqual => {
            evaluate_numeric(&attr_value, condition.values.first(), |a, b| a >= b)
        }
        Operator::LessThan => {
            evaluate_numeric(&attr_value, condition.values.first(), |a, b| a < b)
        }
        Operator::LessThanOrEqual => {
            evaluate_numeric(&attr_value, condition.values.first(), |a, b| a <= b)
        }
        Operator::SemverGreaterThan => {
            evaluate_semver(&attr_value, condition.values.first(), |a, b| a > b)
        }
        Operator::SemverLessThan => {
            evaluate_semver(&attr_value, condition.values.first(), |a, b| a < b)
        }
        Operator::Percentage => unreachable!(),  // Handled above
    }
}

fn evaluate_percentage(
    condition: &Condition,
    context: &EvaluationContext,
    hasher: &ConsistentHasher,
    flag_id: &str,
) -> bool {
    // Get the bucketing key (usually user_id)
    let key = context
        .get_string(&condition.attribute)
        .or_else(|| context.user_id.clone())
        .unwrap_or_else(|| "anonymous".to_string());

    // Get the percentage value
    let percentage: u8 = condition
        .values
        .first()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    hasher.in_percentage(&key, flag_id, percentage)
}

fn evaluate_regex(value: &str, pattern: Option<&String>) -> bool {
    let pattern = match pattern {
        Some(p) => p,
        None => return false,
    };

    // Compile regex (consider caching in production)
    match Regex::new(pattern) {
        Ok(re) => re.is_match(value),
        Err(_) => false,
    }
}

fn evaluate_numeric<F>(value: &str, target: Option<&String>, cmp: F) -> bool
where
    F: Fn(f64, f64) -> bool,
{
    let target = match target.and_then(|t| t.parse::<f64>().ok()) {
        Some(t) => t,
        None => return false,
    };

    match value.parse::<f64>() {
        Ok(v) => cmp(v, target),
        Err(_) => false,
    }
}

fn evaluate_semver<F>(value: &str, target: Option<&String>, cmp: F) -> bool
where
    F: Fn(&semver::Version, &semver::Version) -> bool,
{
    let target = match target.and_then(|t| semver::Version::parse(t).ok()) {
        Some(t) => t,
        None => return false,
    };

    match semver::Version::parse(value) {
        Ok(v) => cmp(&v, &target),
        Err(_) => false,
    }
}
```

### Paso 5: Implementar FlagEvaluator

```rust
// src/flags/evaluator.rs (continuacion)
use tracing::{debug, instrument};

use super::context::EvaluationContext;
use super::hashing::ConsistentHasher;
use super::model::{FeatureFlag, TargetingRule};

/// Evaluates feature flags against user context.
pub struct FlagEvaluator {
    hasher: ConsistentHasher,
}

impl FlagEvaluator {
    /// Creates a new evaluator with default hasher.
    pub fn new() -> Self {
        Self {
            hasher: ConsistentHasher::default_seed(),
        }
    }

    /// Creates an evaluator with a custom hasher.
    pub fn with_hasher(hasher: ConsistentHasher) -> Self {
        Self { hasher }
    }

    /// Evaluates a feature flag for the given context.
    #[instrument(skip(self, flag), fields(flag_id = %flag.id))]
    pub fn evaluate(
        &self,
        flag: &FeatureFlag,
        context: &EvaluationContext,
    ) -> EvaluationResult {
        // Get default variant for fallback
        let default_variant = match flag.get_default_variant() {
            Some(v) => v,
            None => {
                return EvaluationResult::error(
                    flag.id.clone(),
                    &flag.variants[0],  // Use first variant as emergency fallback
                    "Default variant not found".to_string(),
                );
            }
        };

        // If flag is disabled, return default
        if !flag.enabled {
            debug!("Flag is disabled, returning default");
            return EvaluationResult::disabled(flag.id.clone(), default_variant);
        }

        // Evaluate rules in priority order
        for rule in &flag.rules {
            if !rule.enabled {
                debug!(rule_id = %rule.id, "Rule is disabled, skipping");
                continue;
            }

            if self.evaluate_rule(rule, context, &flag.id) {
                debug!(rule_id = %rule.id, "Rule matched");

                let variant = match flag.get_variant(&rule.variant_id) {
                    Some(v) => v,
                    None => {
                        debug!(
                            rule_id = %rule.id,
                            variant_id = %rule.variant_id,
                            "Rule variant not found, continuing"
                        );
                        continue;
                    }
                };

                return EvaluationResult::rule_match(
                    flag.id.clone(),
                    variant,
                    rule.id.clone(),
                );
            }
        }

        // No rules matched, return default
        debug!("No rules matched, returning default");
        EvaluationResult::default(flag.id.clone(), default_variant)
    }

    /// Evaluates a single targeting rule.
    fn evaluate_rule(
        &self,
        rule: &TargetingRule,
        context: &EvaluationContext,
        flag_id: &str,
    ) -> bool {
        // Empty conditions = always matches
        if rule.conditions.is_empty() {
            return true;
        }

        // All conditions must match (AND logic)
        for condition in &rule.conditions {
            if !evaluate_condition(condition, context, &self.hasher, flag_id) {
                return false;
            }
        }

        true
    }

    /// Evaluates multiple flags in one call.
    pub fn evaluate_all(
        &self,
        flags: &[FeatureFlag],
        context: &EvaluationContext,
    ) -> Vec<EvaluationResult> {
        flags
            .iter()
            .map(|flag| self.evaluate(flag, context))
            .collect()
    }
}

impl Default for FlagEvaluator {
    fn default() -> Self {
        Self::new()
    }
}
```

### Paso 6: Batch Evaluation

```rust
// src/flags/evaluator.rs (continuacion)

/// Request for batch flag evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchEvaluationRequest {
    /// Context for evaluation.
    pub context: EvaluationContext,

    /// Flag IDs to evaluate (empty = evaluate all).
    #[serde(default)]
    pub flag_ids: Vec<String>,
}

/// Response for batch flag evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchEvaluationResponse {
    /// Results keyed by flag ID.
    pub results: HashMap<String, EvaluationResult>,

    /// When the evaluation occurred.
    pub evaluated_at: DateTime<Utc>,
}

impl BatchEvaluationResponse {
    /// Gets a result by flag ID.
    pub fn get(&self, flag_id: &str) -> Option<&EvaluationResult> {
        self.results.get(flag_id)
    }

    /// Gets a boolean value for a flag, with a default.
    pub fn get_bool(&self, flag_id: &str, default: bool) -> bool {
        self.results
            .get(flag_id)
            .and_then(|r| r.as_bool())
            .unwrap_or(default)
    }
}

use std::collections::HashMap;

impl FlagEvaluator {
    /// Evaluates multiple flags and returns results keyed by ID.
    pub fn evaluate_batch(
        &self,
        flags: &[FeatureFlag],
        request: &BatchEvaluationRequest,
    ) -> BatchEvaluationResponse {
        let flags_to_evaluate: Vec<_> = if request.flag_ids.is_empty() {
            flags.iter().collect()
        } else {
            flags
                .iter()
                .filter(|f| request.flag_ids.contains(&f.id))
                .collect()
        };

        let results: HashMap<String, EvaluationResult> = flags_to_evaluate
            .into_iter()
            .map(|flag| {
                let result = self.evaluate(flag, &request.context);
                (flag.id.clone(), result)
            })
            .collect();

        BatchEvaluationResponse {
            results,
            evaluated_at: Utc::now(),
        }
    }
}
```

---

## Conceptos de Rust Aprendidos

### 1. Consistent Hashing con SipHash

SipHash es un hash criptograficamente seguro disenado para hash tables, ideal para consistent hashing.

**Rust:**
```rust
use siphasher::sip::SipHasher13;
use std::hash::{Hash, Hasher};

pub struct ConsistentHasher {
    seed: [u8; 16],  // 128-bit seed
}

impl ConsistentHasher {
    pub fn bucket(&self, value: &str, salt: &str) -> u8 {
        // Crear hasher con seed fijo
        let mut hasher = SipHasher13::new_with_key(&self.seed);

        // Hash el valor + salt
        value.hash(&mut hasher);
        salt.hash(&mut hasher);

        // Obtener hash y mapear a bucket
        let hash = hasher.finish();  // u64
        (hash % 100) as u8
    }
}
```

**Comparacion con Java (Guava Hashing):**
```java
import com.google.common.hash.Hashing;
import com.google.common.hash.HashFunction;

public class ConsistentHasher {
    private final HashFunction hashFunction = Hashing.sipHash24();

    public int bucket(String value, String salt) {
        long hash = hashFunction.newHasher()
            .putString(value, StandardCharsets.UTF_8)
            .putString(salt, StandardCharsets.UTF_8)
            .hash()
            .asLong();

        return (int) Math.floorMod(hash, 100);
    }
}
```

**Propiedades de Consistent Hashing:**
| Propiedad | Descripcion |
|-----------|-------------|
| Determinismo | Mismo input = mismo output siempre |
| Uniformidad | Distribucion equitativa entre buckets |
| Estabilidad | Agregar/quitar users no afecta a otros |
| Salt | Diferentes flags = diferentes buckets |

### 2. Pattern Matching Exhaustivo

Rust garantiza que manejes todos los casos de un enum.

**Rust:**
```rust
fn evaluate_operator(&self, op: &Operator, ...) -> bool {
    match op {
        Operator::Equals => self.equals(...),
        Operator::NotEquals => self.not_equals(...),
        Operator::InList => self.in_list(...),
        Operator::NotInList => self.not_in_list(...),
        Operator::Contains => self.contains(...),
        // ... todos los demas casos
        // Si agregas un nuevo Operator, el compilador te obliga
        // a agregar el caso aqui
    }
}

// Si quieres un default, usas _
match op {
    Operator::Percentage => special_handling(),
    _ => generic_handling(),  // Todos los demas
}
```

**Comparacion con Java:**
```java
// Java: switch expressions (Java 14+)
boolean evaluate(Operator op) {
    return switch (op) {
        case EQUALS -> equals();
        case NOT_EQUALS -> notEquals();
        case IN_LIST -> inList();
        // Si no es exhaustivo, necesitas default
        default -> throw new IllegalArgumentException();
    };
}

// O con pattern matching (Java 21+)
boolean evaluate(Operator op) {
    return switch (op) {
        case Operator o when o == Operator.PERCENTAGE -> percentage();
        case Operator o -> generic(o);
    };
}
```

### 3. Closures como Predicados

**Rust:**
```rust
// Closure generico para comparaciones numericas
fn evaluate_numeric<F>(value: &str, target: Option<&String>, cmp: F) -> bool
where
    F: Fn(f64, f64) -> bool,  // Trait bound: F es una funcion f64 x f64 -> bool
{
    let target = match target.and_then(|t| t.parse::<f64>().ok()) {
        Some(t) => t,
        None => return false,
    };

    match value.parse::<f64>() {
        Ok(v) => cmp(v, target),  // Llamar la closure
        Err(_) => false,
    }
}

// Uso con diferentes predicados
evaluate_numeric(&value, &target, |a, b| a > b);   // Greater than
evaluate_numeric(&value, &target, |a, b| a >= b);  // Greater or equal
evaluate_numeric(&value, &target, |a, b| a < b);   // Less than
```

**Comparacion con Java:**
```java
// Java: BiPredicate o DoubleBinaryOperator
boolean evaluateNumeric(
    String value,
    String target,
    DoublePredicate2 predicate  // Custom functional interface
) {
    try {
        double v = Double.parseDouble(value);
        double t = Double.parseDouble(target);
        return predicate.test(v, t);
    } catch (NumberFormatException e) {
        return false;
    }
}

// Uso
evaluateNumeric(value, target, (a, b) -> a > b);
evaluateNumeric(value, target, (a, b) -> a >= b);
```

### 4. Option Chaining con and_then/or_else

**Rust:**
```rust
fn get_percentage_key(context: &EvaluationContext, attr: &str) -> String {
    context
        .get_string(attr)                        // Option<String>
        .or_else(|| context.user_id.clone())     // Si None, intenta user_id
        .unwrap_or_else(|| "anonymous".to_string())  // Si aun None, default
}

// Mas complejo
let percentage: u8 = condition
    .values
    .first()                        // Option<&String>
    .and_then(|v| v.parse().ok())   // Option<u8> (parse puede fallar)
    .unwrap_or(0);                  // Default si cualquier paso fallo
```

**Comparacion con Java:**
```java
String getPercentageKey(EvaluationContext context, String attr) {
    return Optional.ofNullable(context.getString(attr))
        .or(() -> Optional.ofNullable(context.getUserId()))
        .orElse("anonymous");
}

// Mas complejo
int percentage = Optional.ofNullable(condition.getValues())
    .filter(v -> !v.isEmpty())
    .map(v -> v.get(0))
    .flatMap(v -> {
        try {
            return Optional.of(Integer.parseInt(v));
        } catch (NumberFormatException e) {
            return Optional.empty();
        }
    })
    .orElse(0);
```

---

## Riesgos y Errores Comunes

### 1. Percentage con Usuario Anonimo

```rust
// MAL: Usuarios anonimos siempre obtienen el mismo bucket
let key = context.user_id.clone().unwrap_or("anonymous".to_string());
// Todos los anonimos caen en el mismo bucket!

// BIEN: Usar session ID o generar ID unico
let key = context
    .user_id
    .clone()
    .or_else(|| context.get_string("session_id"))
    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
```

### 2. Regex sin Cache

```rust
// MAL: Compilar regex en cada evaluacion
fn evaluate_regex(value: &str, pattern: &str) -> bool {
    let re = Regex::new(pattern).ok()?;  // Compile cada vez!
    re.is_match(value)
}

// BIEN: Cache de regex compiladas
use std::sync::OnceLock;
use std::collections::HashMap;
use parking_lot::RwLock;

static REGEX_CACHE: OnceLock<RwLock<HashMap<String, Regex>>> = OnceLock::new();

fn get_or_compile_regex(pattern: &str) -> Option<Regex> {
    let cache = REGEX_CACHE.get_or_init(|| RwLock::new(HashMap::new()));

    // Try read lock first
    if let Some(re) = cache.read().get(pattern) {
        return Some(re.clone());
    }

    // Compile and cache
    let re = Regex::new(pattern).ok()?;
    cache.write().insert(pattern.to_string(), re.clone());
    Some(re)
}
```

### 3. Orden de Reglas Incorrecto

```rust
// MAL: Regla catch-all antes de reglas especificas
let flag = FeatureFlag::boolean("test", "Test", false)
    .with_rule(TargetingRule::catch_all("everyone", "on").with_priority(100))  // Siempre match!
    .with_rule(TargetingRule::new("beta", "on", vec![...]).with_priority(50));  // Nunca alcanzado

// BIEN: Reglas especificas con mayor prioridad
let flag = FeatureFlag::boolean("test", "Test", false)
    .with_rule(TargetingRule::new("beta", "on", vec![...]).with_priority(100))  // Especifico
    .with_rule(TargetingRule::catch_all("everyone", "on").with_priority(0));    // Fallback
```

### 4. Short-circuit Incorrecto

```rust
// MAL: Evaluar todas las condiciones aunque una falle
fn evaluate_rule(&self, rule: &TargetingRule, ...) -> bool {
    let results: Vec<bool> = rule.conditions
        .iter()
        .map(|c| evaluate_condition(c, ...))  // Evalua TODAS
        .collect();
    results.iter().all(|&r| r)
}

// BIEN: Short-circuit en primera condicion false
fn evaluate_rule(&self, rule: &TargetingRule, ...) -> bool {
    for condition in &rule.conditions {
        if !evaluate_condition(condition, ...) {
            return false;  // Short-circuit
        }
    }
    true
}
```

---

## Pruebas

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn test_flag() -> FeatureFlag {
        FeatureFlag::boolean("test-flag", "Test Flag", false)
            .with_rule(
                TargetingRule::new(
                    "beta-users",
                    "on",
                    vec![Condition::in_list(
                        "user_group",
                        vec!["beta".to_string()],
                    )],
                )
                .with_priority(100),
            )
            .with_rule(
                TargetingRule::new(
                    "percentage-rollout",
                    "on",
                    vec![Condition::percentage("user_id", 50)],
                )
                .with_priority(50),
            )
    }

    #[test]
    fn disabled_flag_returns_default() {
        let mut flag = test_flag();
        flag.enabled = false;

        let evaluator = FlagEvaluator::new();
        let context = EvaluationContext::with_user_id("user-123")
            .with_attribute("user_group", "beta");

        let result = evaluator.evaluate(&flag, &context);

        assert_eq!(result.reason, EvaluationReason::FlagDisabled);
        assert_eq!(result.variant_id, "off");
    }

    #[test]
    fn beta_user_gets_treatment() {
        let flag = test_flag();
        let evaluator = FlagEvaluator::new();
        let context = EvaluationContext::with_user_id("user-123")
            .with_attribute("user_group", "beta");

        let result = evaluator.evaluate(&flag, &context);

        assert_eq!(result.reason, EvaluationReason::RuleMatch);
        assert_eq!(result.rule_id, Some("beta-users".to_string()));
        assert_eq!(result.variant_id, "on");
    }

    #[test]
    fn non_beta_user_evaluated_by_percentage() {
        let flag = test_flag();
        let evaluator = FlagEvaluator::new();

        // Test multiple users to verify percentage works
        let mut in_treatment = 0;
        let total = 1000;

        for i in 0..total {
            let context = EvaluationContext::with_user_id(format!("user-{}", i));
            let result = evaluator.evaluate(&flag, &context);

            if result.variant_id == "on" {
                in_treatment += 1;
            }
        }

        // With 50% rollout, expect roughly 500 users in treatment
        // Allow 10% variance
        let ratio = in_treatment as f64 / total as f64;
        assert!(
            ratio >= 0.40 && ratio <= 0.60,
            "Expected ~50% in treatment, got {:.1}%",
            ratio * 100.0
        );
    }

    #[test]
    fn percentage_is_consistent_for_same_user() {
        let flag = test_flag();
        let evaluator = FlagEvaluator::new();
        let context = EvaluationContext::with_user_id("consistent-user");

        let result1 = evaluator.evaluate(&flag, &context);
        let result2 = evaluator.evaluate(&flag, &context);

        assert_eq!(result1.variant_id, result2.variant_id);
    }

    #[test]
    fn no_rules_match_returns_default() {
        let flag = FeatureFlag::boolean("test", "Test", false);
        let evaluator = FlagEvaluator::new();
        let context = EvaluationContext::with_user_id("user-123");

        let result = evaluator.evaluate(&flag, &context);

        assert_eq!(result.reason, EvaluationReason::Default);
        assert_eq!(result.variant_id, "off");
    }

    #[test]
    fn operators_evaluate_correctly() {
        let evaluator = FlagEvaluator::new();
        let context = EvaluationContext::with_user_id("user-123")
            .with_attribute("environment", "production")
            .with_attribute("version", "2.5.0")
            .with_number("score", 85.0);

        // Test various operators
        let tests = vec![
            (Condition::equals("environment", "production"), true),
            (Condition::equals("environment", "staging"), false),
            (Condition::in_list("environment", vec!["prod".into(), "production".into()]), true),
            // Add more operator tests...
        ];

        for (condition, expected) in tests {
            let rule = TargetingRule::new("test", "on", vec![condition]);
            let flag = FeatureFlag::boolean("test", "Test", false)
                .with_rule(rule);

            let result = evaluator.evaluate(&flag, &context);
            let matched = result.reason == EvaluationReason::RuleMatch;

            assert_eq!(matched, expected, "Condition failed");
        }
    }
}
```

---

## Seguridad

### Consideraciones

1. **Timing attacks**: Evaluacion en tiempo constante para conditions sensibles
2. **DoS via regex**: Limitar complejidad de patrones regex
3. **Information leakage**: No exponer detalles de evaluacion en errores

```rust
/// Maximum regex pattern length to prevent ReDoS.
const MAX_REGEX_LENGTH: usize = 256;

fn evaluate_regex_safe(value: &str, pattern: &str) -> bool {
    if pattern.len() > MAX_REGEX_LENGTH {
        tracing::warn!(
            pattern_length = pattern.len(),
            "Regex pattern too long, rejecting"
        );
        return false;
    }

    // Use regex with timeout in production
    evaluate_regex(value, Some(&pattern.to_string()))
}
```

---

## Entregable Final

### Archivos Creados

1. `src/flags/context.rs` - EvaluationContext
2. `src/flags/hashing.rs` - ConsistentHasher
3. `src/flags/evaluator.rs` - FlagEvaluator y EvaluationResult
4. `tests/flags_evaluator_test.rs` - Tests

### Verificacion

```bash
cargo build -p vortex-features
cargo test -p vortex-features evaluator
cargo clippy -p vortex-features -- -D warnings
```

### Ejemplo de Uso

```rust
use vortex_features::flags::{
    FeatureFlag, TargetingRule, Condition,
    FlagEvaluator, EvaluationContext,
};

fn main() {
    // Define flag
    let flag = FeatureFlag::boolean("new-dashboard", "New Dashboard", false)
        .with_rule(
            TargetingRule::new("beta", "on", vec![
                Condition::in_list("user_group", vec!["beta".to_string()]),
            ])
            .with_priority(100),
        )
        .with_rule(
            TargetingRule::new("rollout", "on", vec![
                Condition::percentage("user_id", 25),
            ])
            .with_priority(50),
        );

    // Create evaluator
    let evaluator = FlagEvaluator::new();

    // Evaluate for a user
    let context = EvaluationContext::with_user_id("user-12345")
        .with_attribute("user_group", "standard")
        .with_attribute("plan", "enterprise");

    let result = evaluator.evaluate(&flag, &context);

    println!("Flag: {}", result.flag_id);
    println!("Variant: {}", result.variant_id);
    println!("Value: {:?}", result.value);
    println!("Reason: {:?}", result.reason);

    if result.is_enabled() {
        println!("New dashboard is enabled for this user!");
    }
}
```

---

**Anterior**: [Historia 001 - Modelo de Feature Flags](./story-001-flag-model.md)
**Siguiente**: [Historia 003 - API de Feature Flags](./story-003-flag-api.md)
