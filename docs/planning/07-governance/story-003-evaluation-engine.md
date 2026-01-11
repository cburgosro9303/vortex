# Historia 003: Motor de Evaluacion PLAC

## Contexto y Objetivo

El Motor de Evaluacion PLAC es el cerebro del sistema de gobernanza. Toma las politicas cargadas (Historia 002) y las evalua contra el contexto de cada request para determinar que acciones aplicar.

**Responsabilidades del Motor:**
- Evaluar condiciones contra el contexto del request
- Ordenar politicas por prioridad
- Determinar si una politica aplica (todas las condiciones deben coincidir)
- Recopilar acciones a ejecutar (deny, mask, redact, warn)
- Cortocircuitar evaluacion en acciones terminales (deny)

Esta historia introduce conceptos avanzados de Rust como pattern matching exhaustivo, el patron Visitor para evaluacion de condiciones, y trait objects para polimorfismo.

---

## Alcance

### In Scope

- RequestContext struct con toda la informacion del request
- ConditionEvaluator trait para evaluar condiciones
- PolicyEngine que orquesta la evaluacion
- PolicyDecision enum con el resultado de evaluacion
- Soporte para todos los operadores definidos
- Evaluacion de CIDR para IPs
- Cache de evaluacion por request

### Out of Scope

- Integracion con middleware (Historia 004)
- Ejecucion de acciones (Historia 006)
- Persistencia de decisiones
- Metricas de evaluacion

---

## Criterios de Aceptacion

- [ ] RequestContext contiene: app, profile, label, source_ip, headers, property_paths
- [ ] ConditionEvaluator evalua cada tipo de operador correctamente
- [ ] PolicyEngine evalua politicas en orden de prioridad
- [ ] Acciones terminales (deny) detienen evaluacion inmediatamente
- [ ] Acciones no terminales se acumulan
- [ ] Pattern matching funciona con regex
- [ ] CIDR matching funciona con IPv4 e IPv6
- [ ] Tests cubren todos los operadores y combinaciones

---

## Diseno Propuesto

### Arquitectura del Motor

```
┌─────────────────────────────────────────────────────────────────┐
│                       PolicyEngine                               │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Input: RequestContext + CompiledPolicySet                      │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                 Evaluation Pipeline                      │   │
│  │                                                          │   │
│  │  1. Get active policies (enabled = true)                │   │
│  │                                                          │   │
│  │  2. Sort by priority (descending)                       │   │
│  │                                                          │   │
│  │  3. For each policy:                                    │   │
│  │     ┌─────────────────────────────────────────────┐    │   │
│  │     │  Evaluate all conditions (AND logic)         │    │   │
│  │     │                                              │    │   │
│  │     │  ┌─────────────────────────────────────┐   │    │   │
│  │     │  │  ConditionEvaluator                  │   │    │   │
│  │     │  │  - Equals/NotEquals                  │   │    │   │
│  │     │  │  - Matches/NotMatches (regex)        │   │    │   │
│  │     │  │  - In/NotIn (list)                   │   │    │   │
│  │     │  │  - Contains                          │   │    │   │
│  │     │  │  - InCidr/NotInCidr                  │   │    │   │
│  │     │  └─────────────────────────────────────┘   │    │   │
│  │     │                                              │    │   │
│  │     │  If ALL conditions match:                   │    │   │
│  │     │    - If action is terminal (deny): RETURN   │    │   │
│  │     │    - Else: accumulate action                │    │   │
│  │     └─────────────────────────────────────────────┘    │   │
│  │                                                          │   │
│  │  4. Return PolicyDecision with accumulated actions      │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  Output: PolicyDecision { Allow(actions) | Deny(message) }     │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Estructura de Archivos

```
crates/vortex-governance/src/plac/
├── mod.rs
├── model.rs          # Historia 001
├── builder.rs        # Historia 001
├── parser.rs         # Historia 002
├── loader.rs         # Historia 002
├── context.rs        # NUEVO: RequestContext
├── evaluator.rs      # NUEVO: ConditionEvaluator
├── engine.rs         # NUEVO: PolicyEngine
└── decision.rs       # NUEVO: PolicyDecision
```

---

## Pasos de Implementacion

### Paso 1: Definir RequestContext

```rust
// src/plac/context.rs
use std::collections::HashMap;
use std::net::IpAddr;

/// Context of an incoming request for policy evaluation.
///
/// Contains all the information needed to evaluate policy conditions.
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Application name being requested
    pub application: String,

    /// Profile(s) being requested
    pub profiles: Vec<String>,

    /// Label (branch/tag) if specified
    pub label: Option<String>,

    /// Source IP address of the request
    pub source_ip: Option<IpAddr>,

    /// HTTP headers from the request
    pub headers: HashMap<String, String>,

    /// Property paths being accessed (for property-level policies)
    /// This is populated after config is retrieved
    pub property_paths: Vec<String>,

    /// Additional custom attributes for extensibility
    pub attributes: HashMap<String, String>,
}

impl RequestContext {
    /// Create a new RequestContext with required fields.
    pub fn new(application: impl Into<String>, profiles: Vec<String>) -> Self {
        Self {
            application: application.into(),
            profiles,
            label: None,
            source_ip: None,
            headers: HashMap::new(),
            property_paths: Vec::new(),
            attributes: HashMap::new(),
        }
    }

    /// Get the primary profile (first in list).
    pub fn primary_profile(&self) -> Option<&str> {
        self.profiles.first().map(|s| s.as_str())
    }

    /// Check if a profile is in the requested profiles.
    pub fn has_profile(&self, profile: &str) -> bool {
        self.profiles.iter().any(|p| p == profile)
    }

    /// Get a header value.
    pub fn header(&self, name: &str) -> Option<&str> {
        // Headers are case-insensitive
        let name_lower = name.to_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == name_lower)
            .map(|(_, v)| v.as_str())
    }

    /// Get an attribute value.
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(|s| s.as_str())
    }
}

/// Builder for RequestContext.
#[derive(Debug, Default)]
pub struct RequestContextBuilder {
    application: Option<String>,
    profiles: Vec<String>,
    label: Option<String>,
    source_ip: Option<IpAddr>,
    headers: HashMap<String, String>,
    property_paths: Vec<String>,
    attributes: HashMap<String, String>,
}

impl RequestContextBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn application(mut self, app: impl Into<String>) -> Self {
        self.application = Some(app.into());
        self
    }

    pub fn profile(mut self, profile: impl Into<String>) -> Self {
        self.profiles.push(profile.into());
        self
    }

    pub fn profiles(mut self, profiles: Vec<String>) -> Self {
        self.profiles = profiles;
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn source_ip(mut self, ip: IpAddr) -> Self {
        self.source_ip = Some(ip);
        self
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    pub fn property_path(mut self, path: impl Into<String>) -> Self {
        self.property_paths.push(path.into());
        self
    }

    pub fn attribute(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(name.into(), value.into());
        self
    }

    pub fn build(self) -> Result<RequestContext, &'static str> {
        let application = self.application.ok_or("Application is required")?;

        if self.profiles.is_empty() {
            return Err("At least one profile is required");
        }

        Ok(RequestContext {
            application,
            profiles: self.profiles,
            label: self.label,
            source_ip: self.source_ip,
            headers: self.headers,
            property_paths: self.property_paths,
            attributes: self.attributes,
        })
    }
}
```

### Paso 2: Implementar ConditionEvaluator

```rust
// src/plac/evaluator.rs
use std::net::IpAddr;
use regex::Regex;
use tracing::{debug, trace};

use super::model::{Condition, ConditionField, Operator};
use super::context::RequestContext;
use super::compiled::CompiledPolicySet;

/// Evaluates a single condition against a request context.
pub struct ConditionEvaluator<'a> {
    /// Compiled policies (for regex access)
    policies: &'a CompiledPolicySet,
}

impl<'a> ConditionEvaluator<'a> {
    pub fn new(policies: &'a CompiledPolicySet) -> Self {
        Self { policies }
    }

    /// Evaluate a condition against the request context.
    pub fn evaluate(
        &self,
        condition: &Condition,
        context: &RequestContext,
    ) -> bool {
        // Get the value to evaluate from the context
        let context_value = self.get_context_value(&condition.field, context);

        trace!(
            field = ?condition.field,
            operator = ?condition.operator,
            expected = %condition.value,
            actual = ?context_value,
            "Evaluating condition"
        );

        // Handle special case: PropertyPath may have multiple values
        if matches!(condition.field, ConditionField::PropertyPath) {
            return self.evaluate_property_paths(condition, context);
        }

        // Get single value or return false if missing
        let value = match context_value {
            Some(v) => v,
            None => return false,
        };

        // Apply case sensitivity
        let (value, pattern) = if condition.case_sensitive {
            (value.to_string(), condition.value.clone())
        } else {
            (value.to_lowercase(), condition.value.to_lowercase())
        };

        // Evaluate based on operator
        self.evaluate_operator(&condition.operator, &value, &pattern, context)
    }

    /// Get the value from context for a given field.
    fn get_context_value(
        &self,
        field: &ConditionField,
        context: &RequestContext,
    ) -> Option<String> {
        match field {
            ConditionField::Application => Some(context.application.clone()),
            ConditionField::Profile => context.primary_profile().map(|s| s.to_string()),
            ConditionField::Label => context.label.clone(),
            ConditionField::SourceIp => context.source_ip.map(|ip| ip.to_string()),
            ConditionField::Header(name) => context.header(name).map(|s| s.to_string()),
            ConditionField::PropertyPath => None, // Handled specially
        }
    }

    /// Evaluate condition against all property paths.
    fn evaluate_property_paths(
        &self,
        condition: &Condition,
        context: &RequestContext,
    ) -> bool {
        // For PropertyPath, ANY match is sufficient
        context.property_paths.iter().any(|path| {
            let (value, pattern) = if condition.case_sensitive {
                (path.clone(), condition.value.clone())
            } else {
                (path.to_lowercase(), condition.value.to_lowercase())
            };

            self.evaluate_operator(&condition.operator, &value, &pattern, context)
        })
    }

    /// Evaluate the operator.
    fn evaluate_operator(
        &self,
        operator: &Operator,
        value: &str,
        pattern: &str,
        context: &RequestContext,
    ) -> bool {
        match operator {
            Operator::Equals => value == pattern,

            Operator::NotEquals => value != pattern,

            Operator::Matches => {
                self.policies
                    .get_regex(pattern)
                    .map(|r| r.is_match(value))
                    .unwrap_or(false)
            }

            Operator::NotMatches => {
                self.policies
                    .get_regex(pattern)
                    .map(|r| !r.is_match(value))
                    .unwrap_or(true)
            }

            Operator::In => {
                pattern.split(',')
                    .map(|s| s.trim())
                    .any(|s| s == value)
            }

            Operator::NotIn => {
                !pattern.split(',')
                    .map(|s| s.trim())
                    .any(|s| s == value)
            }

            Operator::Contains => value.contains(pattern),

            Operator::InCidr => {
                context.source_ip
                    .map(|ip| self.ip_in_cidr(ip, pattern))
                    .unwrap_or(false)
            }

            Operator::NotInCidr => {
                context.source_ip
                    .map(|ip| !self.ip_in_cidr(ip, pattern))
                    .unwrap_or(true)
            }
        }
    }

    /// Check if an IP is within a CIDR range.
    fn ip_in_cidr(&self, ip: IpAddr, cidr: &str) -> bool {
        // Parse CIDR
        let parts: Vec<&str> = cidr.split('/').collect();
        if parts.len() != 2 {
            return false;
        }

        let network: IpAddr = match parts[0].parse() {
            Ok(ip) => ip,
            Err(_) => return false,
        };

        let prefix: u8 = match parts[1].parse() {
            Ok(p) => p,
            Err(_) => return false,
        };

        // Check IP version match
        match (ip, network) {
            (IpAddr::V4(ip), IpAddr::V4(net)) => {
                self.ipv4_in_cidr(ip, net, prefix)
            }
            (IpAddr::V6(ip), IpAddr::V6(net)) => {
                self.ipv6_in_cidr(ip, net, prefix)
            }
            _ => false, // IPv4/IPv6 mismatch
        }
    }

    fn ipv4_in_cidr(
        &self,
        ip: std::net::Ipv4Addr,
        network: std::net::Ipv4Addr,
        prefix: u8,
    ) -> bool {
        if prefix > 32 {
            return false;
        }

        let ip_bits = u32::from(ip);
        let net_bits = u32::from(network);
        let mask = if prefix == 0 {
            0
        } else {
            !0u32 << (32 - prefix)
        };

        (ip_bits & mask) == (net_bits & mask)
    }

    fn ipv6_in_cidr(
        &self,
        ip: std::net::Ipv6Addr,
        network: std::net::Ipv6Addr,
        prefix: u8,
    ) -> bool {
        if prefix > 128 {
            return false;
        }

        let ip_bits = u128::from(ip);
        let net_bits = u128::from(network);
        let mask = if prefix == 0 {
            0
        } else {
            !0u128 << (128 - prefix)
        };

        (ip_bits & mask) == (net_bits & mask)
    }
}
```

### Paso 3: Definir PolicyDecision

```rust
// src/plac/decision.rs
use super::model::{Action, Policy};

/// The result of evaluating policies against a request.
#[derive(Debug, Clone)]
pub enum PolicyDecision {
    /// Request is allowed, possibly with transformations.
    Allow {
        /// Actions to apply to the response (mask, redact, warn).
        actions: Vec<AppliedAction>,
    },

    /// Request is denied.
    Deny {
        /// The policy that caused the denial.
        policy_name: String,
        /// Message to return to the client.
        message: String,
    },
}

impl PolicyDecision {
    /// Create an Allow decision with no actions.
    pub fn allow() -> Self {
        Self::Allow {
            actions: Vec::new(),
        }
    }

    /// Create an Allow decision with actions.
    pub fn allow_with_actions(actions: Vec<AppliedAction>) -> Self {
        Self::Allow { actions }
    }

    /// Create a Deny decision.
    pub fn deny(policy_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Deny {
            policy_name: policy_name.into(),
            message: message.into(),
        }
    }

    /// Check if the decision is Allow.
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow { .. })
    }

    /// Check if the decision is Deny.
    pub fn is_denied(&self) -> bool {
        matches!(self, Self::Deny { .. })
    }

    /// Get actions if allowed.
    pub fn actions(&self) -> Option<&[AppliedAction]> {
        match self {
            Self::Allow { actions } => Some(actions),
            Self::Deny { .. } => None,
        }
    }

    /// Get deny message if denied.
    pub fn deny_message(&self) -> Option<&str> {
        match self {
            Self::Allow { .. } => None,
            Self::Deny { message, .. } => Some(message),
        }
    }
}

/// An action to apply, along with the policy that triggered it.
#[derive(Debug, Clone)]
pub struct AppliedAction {
    /// Name of the policy that triggered this action.
    pub policy_name: String,

    /// The action to apply.
    pub action: Action,

    /// Priority of the policy (for ordering).
    pub priority: u32,
}

impl AppliedAction {
    pub fn new(policy: &Policy) -> Self {
        Self {
            policy_name: policy.name.clone(),
            action: policy.action.clone(),
            priority: policy.priority,
        }
    }
}
```

### Paso 4: Implementar PolicyEngine

```rust
// src/plac/engine.rs
use tracing::{debug, info, instrument, warn};

use super::model::Policy;
use super::context::RequestContext;
use super::compiled::CompiledPolicySet;
use super::evaluator::ConditionEvaluator;
use super::decision::{PolicyDecision, AppliedAction};

/// Engine for evaluating policies against requests.
///
/// The PolicyEngine is the main entry point for policy evaluation.
/// It orchestrates the evaluation of all policies and returns a decision.
pub struct PolicyEngine<'a> {
    /// Compiled policies to evaluate against.
    policies: &'a CompiledPolicySet,
}

impl<'a> PolicyEngine<'a> {
    /// Create a new PolicyEngine.
    pub fn new(policies: &'a CompiledPolicySet) -> Self {
        Self { policies }
    }

    /// Evaluate policies against a request context.
    ///
    /// Returns a PolicyDecision indicating whether the request is
    /// allowed (possibly with transformations) or denied.
    #[instrument(skip(self, context), fields(
        app = %context.application,
        profile = ?context.primary_profile(),
    ))]
    pub fn evaluate(&self, context: &RequestContext) -> PolicyDecision {
        let evaluator = ConditionEvaluator::new(self.policies);
        let mut accumulated_actions: Vec<AppliedAction> = Vec::new();

        // Get active policies sorted by priority (highest first)
        let policies = self.policies.active_policies();

        debug!(policy_count = policies.len(), "Evaluating policies");

        for policy in policies {
            let matches = self.evaluate_policy(policy, context, &evaluator);

            if matches {
                info!(
                    policy = %policy.name,
                    action = ?policy.action.action_type,
                    "Policy matched"
                );

                // Check if action is terminal (deny)
                if policy.has_terminal_action() {
                    let message = policy.action.message
                        .clone()
                        .unwrap_or_else(|| "Access denied".to_string());

                    return PolicyDecision::deny(&policy.name, message);
                }

                // Accumulate non-terminal action
                accumulated_actions.push(AppliedAction::new(policy));
            }
        }

        // All policies evaluated, no denial
        if accumulated_actions.is_empty() {
            debug!("No policies matched, allowing request");
            PolicyDecision::allow()
        } else {
            debug!(
                action_count = accumulated_actions.len(),
                "Allowing with actions"
            );
            PolicyDecision::allow_with_actions(accumulated_actions)
        }
    }

    /// Evaluate a single policy's conditions.
    fn evaluate_policy(
        &self,
        policy: &Policy,
        context: &RequestContext,
        evaluator: &ConditionEvaluator,
    ) -> bool {
        // All conditions must match (AND logic)
        for condition in &policy.conditions {
            if !evaluator.evaluate(condition, context) {
                return false;
            }
        }

        true
    }

    /// Evaluate policies and return detailed results for debugging.
    pub fn evaluate_with_trace(
        &self,
        context: &RequestContext,
    ) -> EvaluationTrace {
        let evaluator = ConditionEvaluator::new(self.policies);
        let mut trace = EvaluationTrace::new();

        let policies = self.policies.active_policies();

        for policy in policies {
            let mut condition_results = Vec::new();

            for condition in &policy.conditions {
                let result = evaluator.evaluate(condition, context);
                condition_results.push(ConditionResult {
                    condition: format!("{:?} {:?} {}",
                        condition.field,
                        condition.operator,
                        condition.value
                    ),
                    matched: result,
                });
            }

            let all_matched = condition_results.iter().all(|r| r.matched);

            trace.policy_results.push(PolicyResult {
                policy_name: policy.name.clone(),
                priority: policy.priority,
                matched: all_matched,
                conditions: condition_results,
            });

            if all_matched && policy.has_terminal_action() {
                trace.decision = Some(PolicyDecision::deny(
                    &policy.name,
                    policy.action.message.clone().unwrap_or_default(),
                ));
                break;
            }
        }

        if trace.decision.is_none() {
            let actions: Vec<_> = trace.policy_results
                .iter()
                .filter(|r| r.matched)
                .filter_map(|r| {
                    self.policies.policy_set.get_by_name(&r.policy_name)
                })
                .map(AppliedAction::new)
                .collect();

            trace.decision = Some(if actions.is_empty() {
                PolicyDecision::allow()
            } else {
                PolicyDecision::allow_with_actions(actions)
            });
        }

        trace
    }
}

/// Detailed trace of policy evaluation for debugging.
#[derive(Debug)]
pub struct EvaluationTrace {
    pub policy_results: Vec<PolicyResult>,
    pub decision: Option<PolicyDecision>,
}

impl EvaluationTrace {
    fn new() -> Self {
        Self {
            policy_results: Vec::new(),
            decision: None,
        }
    }
}

/// Result of evaluating a single policy.
#[derive(Debug)]
pub struct PolicyResult {
    pub policy_name: String,
    pub priority: u32,
    pub matched: bool,
    pub conditions: Vec<ConditionResult>,
}

/// Result of evaluating a single condition.
#[derive(Debug)]
pub struct ConditionResult {
    pub condition: String,
    pub matched: bool,
}
```

### Paso 5: Crear Visitor Pattern para Condiciones (Opcional, Avanzado)

```rust
// src/plac/visitor.rs
use super::model::{Condition, ConditionField, Operator};
use super::context::RequestContext;

/// Visitor trait for condition evaluation.
///
/// This pattern allows extending condition evaluation without
/// modifying the core evaluator.
pub trait ConditionVisitor {
    /// Visit a condition and return whether it matches.
    fn visit_condition(
        &self,
        condition: &Condition,
        context: &RequestContext,
    ) -> bool;
}

/// Default visitor implementation using pattern matching.
pub struct DefaultConditionVisitor;

impl ConditionVisitor for DefaultConditionVisitor {
    fn visit_condition(
        &self,
        condition: &Condition,
        context: &RequestContext,
    ) -> bool {
        // Dispatch based on field type
        match &condition.field {
            ConditionField::Application => {
                self.visit_string_field(&context.application, condition)
            }
            ConditionField::Profile => {
                context.primary_profile()
                    .map(|p| self.visit_string_field(p, condition))
                    .unwrap_or(false)
            }
            ConditionField::Label => {
                context.label.as_ref()
                    .map(|l| self.visit_string_field(l, condition))
                    .unwrap_or(false)
            }
            ConditionField::SourceIp => {
                self.visit_ip_field(context, condition)
            }
            ConditionField::Header(name) => {
                context.header(name)
                    .map(|h| self.visit_string_field(h, condition))
                    .unwrap_or(false)
            }
            ConditionField::PropertyPath => {
                self.visit_property_paths(context, condition)
            }
        }
    }
}

impl DefaultConditionVisitor {
    fn visit_string_field(&self, value: &str, condition: &Condition) -> bool {
        let (value, pattern) = if condition.case_sensitive {
            (value.to_string(), condition.value.clone())
        } else {
            (value.to_lowercase(), condition.value.to_lowercase())
        };

        match condition.operator {
            Operator::Equals => value == pattern,
            Operator::NotEquals => value != pattern,
            Operator::Contains => value.contains(&pattern),
            Operator::In => pattern.split(',').any(|s| s.trim() == value),
            Operator::NotIn => !pattern.split(',').any(|s| s.trim() == value),
            Operator::Matches | Operator::NotMatches => {
                // Regex handling delegated to evaluator
                false
            }
            Operator::InCidr | Operator::NotInCidr => false,
        }
    }

    fn visit_ip_field(&self, context: &RequestContext, condition: &Condition) -> bool {
        // IP-specific operators
        matches!(condition.operator, Operator::InCidr | Operator::NotInCidr)
    }

    fn visit_property_paths(&self, context: &RequestContext, condition: &Condition) -> bool {
        context.property_paths.iter().any(|path| {
            self.visit_string_field(path, condition)
        })
    }
}
```

---

## Conceptos de Rust Aprendidos

### 1. Pattern Matching Avanzado

El pattern matching de Rust es exhaustivo y el compilador verifica que todos los casos esten cubiertos.

**Rust:**
```rust
// El compilador fuerza manejar TODOS los casos
fn evaluate_operator(operator: &Operator, value: &str, pattern: &str) -> bool {
    match operator {
        Operator::Equals => value == pattern,
        Operator::NotEquals => value != pattern,
        Operator::Matches => regex_match(pattern, value),
        Operator::NotMatches => !regex_match(pattern, value),
        Operator::In => pattern.split(',').any(|s| s.trim() == value),
        Operator::NotIn => !pattern.split(',').any(|s| s.trim() == value),
        Operator::Contains => value.contains(pattern),
        Operator::InCidr => ip_in_cidr(value, pattern),
        Operator::NotInCidr => !ip_in_cidr(value, pattern),
        // Si agregamos Operator::StartsWith, DEBEMOS manejarlo aqui
        // o el codigo no compila
    }
}

// Pattern matching con guards
fn get_value(field: &ConditionField, ctx: &RequestContext) -> Option<String> {
    match field {
        ConditionField::Application => Some(ctx.application.clone()),
        ConditionField::Profile if !ctx.profiles.is_empty() => {
            Some(ctx.profiles[0].clone())
        }
        ConditionField::Profile => None,
        ConditionField::Header(name) if ctx.headers.contains_key(name) => {
            ctx.headers.get(name).cloned()
        }
        ConditionField::Header(_) => None,
        // ... resto de casos
    }
}
```

**Java (switch expressions, Java 14+):**
```java
public boolean evaluateOperator(Operator op, String value, String pattern) {
    return switch (op) {
        case EQUALS -> value.equals(pattern);
        case NOT_EQUALS -> !value.equals(pattern);
        case MATCHES -> Pattern.matches(pattern, value);
        case NOT_MATCHES -> !Pattern.matches(pattern, value);
        case IN -> Arrays.stream(pattern.split(","))
                         .map(String::trim)
                         .anyMatch(s -> s.equals(value));
        case NOT_IN -> Arrays.stream(pattern.split(","))
                            .map(String::trim)
                            .noneMatch(s -> s.equals(value));
        case CONTAINS -> value.contains(pattern);
        case IN_CIDR -> ipInCidr(value, pattern);
        case NOT_IN_CIDR -> !ipInCidr(value, pattern);
        // Java no fuerza exhaustividad sin sealed classes
    };
}
```

**Diferencias:**
| Aspecto | Rust match | Java switch |
|---------|------------|-------------|
| Exhaustividad | Compile-time enforced | Solo con sealed (Java 17+) |
| Guards | `if condition` en arm | Limitado |
| Destructuring | Completo | Records (Java 16+) |
| Returns value | Siempre | switch expression |

### 2. Trait Objects (dyn Trait)

Polimorfismo en runtime usando trait objects.

**Rust:**
```rust
/// Trait para evaluadores de condiciones
pub trait ConditionEvaluator {
    fn evaluate(&self, condition: &Condition, ctx: &RequestContext) -> bool;
}

/// Implementacion por defecto
struct DefaultEvaluator;

impl ConditionEvaluator for DefaultEvaluator {
    fn evaluate(&self, condition: &Condition, ctx: &RequestContext) -> bool {
        // Logica de evaluacion
        true
    }
}

/// Evaluador custom para testing
struct MockEvaluator {
    should_match: bool,
}

impl ConditionEvaluator for MockEvaluator {
    fn evaluate(&self, _: &Condition, _: &RequestContext) -> bool {
        self.should_match
    }
}

/// Motor que usa trait objects
struct PolicyEngine {
    // Box<dyn Trait> permite diferentes implementaciones
    evaluator: Box<dyn ConditionEvaluator>,
}

impl PolicyEngine {
    fn new(evaluator: Box<dyn ConditionEvaluator>) -> Self {
        Self { evaluator }
    }

    fn evaluate(&self, condition: &Condition, ctx: &RequestContext) -> bool {
        // Dispatch dinamico (vtable)
        self.evaluator.evaluate(condition, ctx)
    }
}

// Uso
let engine = PolicyEngine::new(Box::new(DefaultEvaluator));
let mock_engine = PolicyEngine::new(Box::new(MockEvaluator { should_match: true }));
```

**Java equivalente:**
```java
interface ConditionEvaluator {
    boolean evaluate(Condition condition, RequestContext ctx);
}

class DefaultEvaluator implements ConditionEvaluator {
    @Override
    public boolean evaluate(Condition condition, RequestContext ctx) {
        return true;
    }
}

class PolicyEngine {
    private final ConditionEvaluator evaluator;

    public PolicyEngine(ConditionEvaluator evaluator) {
        this.evaluator = evaluator;
    }

    public boolean evaluate(Condition condition, RequestContext ctx) {
        return evaluator.evaluate(condition, ctx);
    }
}
```

**Diferencias:**
| Aspecto | Rust dyn Trait | Java Interface |
|---------|----------------|----------------|
| Allocation | `Box<dyn T>` explicito | Implicito (heap) |
| Dispatch | vtable | vtable |
| Object safety | Reglas estrictas | Cualquier interface |
| Multiple traits | `dyn A + B` | Solo uno |

### 3. Lifetimes en Structs

Cuando un struct contiene referencias, necesita lifetime annotations.

**Rust:**
```rust
/// Evaluador con referencia a politicas compiladas
pub struct ConditionEvaluator<'a> {
    // 'a indica que policies vive al menos tanto como el evaluador
    policies: &'a CompiledPolicySet,
}

impl<'a> ConditionEvaluator<'a> {
    pub fn new(policies: &'a CompiledPolicySet) -> Self {
        Self { policies }
    }

    pub fn get_regex(&self, pattern: &str) -> Option<&Regex> {
        // Retorna referencia con lifetime de policies
        self.policies.get_regex(pattern)
    }
}

// Uso - el evaluador no puede outlive las politicas
fn evaluate(policies: &CompiledPolicySet, ctx: &RequestContext) -> bool {
    let evaluator = ConditionEvaluator::new(policies);
    // evaluator valido mientras policies exista
    evaluator.evaluate(/* ... */)
} // evaluator dropped, policies puede seguir existiendo

// Error: policies no vive suficiente
fn bad_usage() -> ConditionEvaluator<'static> {
    let policies = CompiledPolicySet::new();
    ConditionEvaluator::new(&policies)  // Error! policies dropped al final
}
```

**Java no tiene equivalente directo:**
```java
// Java usa referencias manejadas por GC
class ConditionEvaluator {
    private final CompiledPolicySet policies;

    public ConditionEvaluator(CompiledPolicySet policies) {
        this.policies = policies;  // Referencia, no ownership
    }
    // GC maneja lifetime automaticamente
}
```

### 4. CIDR Matching con Bitwise Operations

**Rust:**
```rust
fn ipv4_in_cidr(
    ip: std::net::Ipv4Addr,
    network: std::net::Ipv4Addr,
    prefix: u8,
) -> bool {
    // Convertir a u32 para operaciones bit a bit
    let ip_bits: u32 = ip.into();
    let net_bits: u32 = network.into();

    // Crear mascara: prefix=24 -> 0xFFFFFF00
    let mask = if prefix == 0 {
        0u32
    } else {
        !0u32 << (32 - prefix)  // Shift izquierda, luego NOT
    };

    // Comparar bits de red
    (ip_bits & mask) == (net_bits & mask)
}

// Ejemplo:
// IP:      192.168.1.100  = 0xC0A80164
// Network: 192.168.1.0/24 = 0xC0A80100
// Mask:    255.255.255.0  = 0xFFFFFF00
// IP & Mask:     0xC0A80100
// Net & Mask:    0xC0A80100
// Match!
```

**Java:**
```java
public boolean ipv4InCidr(InetAddress ip, InetAddress network, int prefix) {
    byte[] ipBytes = ip.getAddress();
    byte[] netBytes = network.getAddress();

    int ipInt = ByteBuffer.wrap(ipBytes).getInt();
    int netInt = ByteBuffer.wrap(netBytes).getInt();

    int mask = prefix == 0 ? 0 : -1 << (32 - prefix);

    return (ipInt & mask) == (netInt & mask);
}
```

---

## Riesgos y Errores Comunes

### 1. Evaluar Condiciones en Orden Incorrecto

```rust
// MAL: No respetar prioridad
fn evaluate(&self, context: &RequestContext) -> PolicyDecision {
    for policy in &self.policies.policies {  // Orden aleatorio!
        // ...
    }
}

// BIEN: Ordenar por prioridad
fn evaluate(&self, context: &RequestContext) -> PolicyDecision {
    let policies = self.policies.active_policies();  // Ya ordenado
    for policy in policies {
        // Highest priority first
    }
}
```

### 2. Short-circuit Incorrecto

```rust
// MAL: No detener en deny
fn evaluate(&self, context: &RequestContext) -> PolicyDecision {
    let mut actions = Vec::new();
    for policy in policies {
        if self.matches(policy, context) {
            actions.push(policy.action.clone());
            // Sigue evaluando despues de deny!
        }
    }
    PolicyDecision::allow_with_actions(actions)
}

// BIEN: Retornar inmediatamente en deny
fn evaluate(&self, context: &RequestContext) -> PolicyDecision {
    let mut actions = Vec::new();
    for policy in policies {
        if self.matches(policy, context) {
            if policy.has_terminal_action() {
                return PolicyDecision::deny(/* ... */);  // Termina!
            }
            actions.push(AppliedAction::new(policy));
        }
    }
    PolicyDecision::allow_with_actions(actions)
}
```

### 3. Case Sensitivity Inconsistente

```rust
// MAL: Ignorar case_sensitive flag
fn evaluate_equals(&self, value: &str, pattern: &str) -> bool {
    value == pattern  // Siempre case-sensitive!
}

// BIEN: Respetar el flag
fn evaluate_equals(&self, condition: &Condition, value: &str) -> bool {
    let (v, p) = if condition.case_sensitive {
        (value.to_string(), condition.value.clone())
    } else {
        (value.to_lowercase(), condition.value.to_lowercase())
    };
    v == p
}
```

### 4. CIDR con IPv4/IPv6 Mezclados

```rust
// MAL: Asumir siempre IPv4
fn ip_in_cidr(&self, ip: IpAddr, cidr: &str) -> bool {
    let ip_v4 = match ip {
        IpAddr::V4(v4) => v4,
        IpAddr::V6(_) => return false,  // IPv6 siempre falla!
    };
    // ...
}

// BIEN: Manejar ambos
fn ip_in_cidr(&self, ip: IpAddr, cidr: &str) -> bool {
    let network: IpAddr = parts[0].parse()?;

    match (ip, network) {
        (IpAddr::V4(ip), IpAddr::V4(net)) => self.ipv4_in_cidr(ip, net, prefix),
        (IpAddr::V6(ip), IpAddr::V6(net)) => self.ipv6_in_cidr(ip, net, prefix),
        _ => false,  // Version mismatch
    }
}
```

---

## Pruebas

### Tests de Operadores

```rust
#[cfg(test)]
mod operator_tests {
    use super::*;

    fn create_condition(
        field: ConditionField,
        operator: Operator,
        value: &str,
    ) -> Condition {
        Condition {
            field,
            operator,
            value: value.to_string(),
            case_sensitive: true,
        }
    }

    fn create_context(app: &str, profile: &str) -> RequestContext {
        RequestContext::new(app, vec![profile.to_string()])
    }

    #[test]
    fn test_equals_operator() {
        let policies = load_empty_policies();
        let evaluator = ConditionEvaluator::new(&policies);

        let condition = create_condition(
            ConditionField::Application,
            Operator::Equals,
            "myapp",
        );

        let ctx_match = create_context("myapp", "dev");
        let ctx_no_match = create_context("other", "dev");

        assert!(evaluator.evaluate(&condition, &ctx_match));
        assert!(!evaluator.evaluate(&condition, &ctx_no_match));
    }

    #[test]
    fn test_in_operator() {
        let policies = load_empty_policies();
        let evaluator = ConditionEvaluator::new(&policies);

        let condition = create_condition(
            ConditionField::Profile,
            Operator::In,
            "dev, staging, prod",
        );

        let ctx_dev = create_context("app", "dev");
        let ctx_prod = create_context("app", "prod");
        let ctx_test = create_context("app", "test");

        assert!(evaluator.evaluate(&condition, &ctx_dev));
        assert!(evaluator.evaluate(&condition, &ctx_prod));
        assert!(!evaluator.evaluate(&condition, &ctx_test));
    }

    #[test]
    fn test_case_insensitive() {
        let policies = load_empty_policies();
        let evaluator = ConditionEvaluator::new(&policies);

        let mut condition = create_condition(
            ConditionField::Application,
            Operator::Equals,
            "MyApp",
        );
        condition.case_sensitive = false;

        let ctx = create_context("myapp", "dev");
        assert!(evaluator.evaluate(&condition, &ctx));
    }
}
```

### Tests de CIDR

```rust
#[cfg(test)]
mod cidr_tests {
    use super::*;
    use std::net::IpAddr;

    fn create_ip_context(ip: &str) -> RequestContext {
        let mut ctx = RequestContext::new("app", vec!["dev".to_string()]);
        ctx.source_ip = Some(ip.parse().unwrap());
        ctx
    }

    #[test]
    fn test_ipv4_cidr_match() {
        let policies = load_empty_policies();
        let evaluator = ConditionEvaluator::new(&policies);

        let condition = Condition {
            field: ConditionField::SourceIp,
            operator: Operator::InCidr,
            value: "10.0.0.0/8".to_string(),
            case_sensitive: true,
        };

        let ctx_match = create_ip_context("10.1.2.3");
        let ctx_no_match = create_ip_context("192.168.1.1");

        assert!(evaluator.evaluate(&condition, &ctx_match));
        assert!(!evaluator.evaluate(&condition, &ctx_no_match));
    }

    #[test]
    fn test_ipv4_cidr_edge_cases() {
        let policies = load_empty_policies();
        let evaluator = ConditionEvaluator::new(&policies);

        // /32 - single IP
        let condition_32 = Condition {
            field: ConditionField::SourceIp,
            operator: Operator::InCidr,
            value: "192.168.1.100/32".to_string(),
            case_sensitive: true,
        };

        let ctx_exact = create_ip_context("192.168.1.100");
        let ctx_off_by_one = create_ip_context("192.168.1.101");

        assert!(evaluator.evaluate(&condition_32, &ctx_exact));
        assert!(!evaluator.evaluate(&condition_32, &ctx_off_by_one));

        // /0 - all IPs
        let condition_0 = Condition {
            field: ConditionField::SourceIp,
            operator: Operator::InCidr,
            value: "0.0.0.0/0".to_string(),
            case_sensitive: true,
        };

        assert!(evaluator.evaluate(&condition_0, &ctx_exact));
    }

    #[test]
    fn test_ipv6_cidr() {
        let policies = load_empty_policies();
        let evaluator = ConditionEvaluator::new(&policies);

        let condition = Condition {
            field: ConditionField::SourceIp,
            operator: Operator::InCidr,
            value: "2001:db8::/32".to_string(),
            case_sensitive: true,
        };

        let ctx_match = create_ip_context("2001:db8::1");
        let ctx_no_match = create_ip_context("2001:db9::1");

        assert!(evaluator.evaluate(&condition, &ctx_match));
        assert!(!evaluator.evaluate(&condition, &ctx_no_match));
    }
}
```

### Tests del PolicyEngine

```rust
#[cfg(test)]
mod engine_tests {
    use super::*;

    fn create_test_policies() -> CompiledPolicySet {
        let yaml = r#"
policies:
  - name: deny-external
    priority: 200
    conditions:
      - field: application
        operator: matches
        value: "internal-.*"
      - field: source_ip
        operator: not_in_cidr
        value: "10.0.0.0/8"
    action:
      type: deny
      message: Internal only

  - name: mask-secrets
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

  - name: warn-deprecated
    priority: 50
    conditions:
      - field: application
        operator: equals
        value: legacy-app
    action:
      type: warn
      message: This app is deprecated
"#;
        PolicyLoader::new()
            .from_string(yaml, PathBuf::from("test.yaml"))
            .unwrap()
    }

    #[test]
    fn test_deny_policy() {
        let policies = create_test_policies();
        let engine = PolicyEngine::new(&policies);

        let mut ctx = RequestContext::new("internal-api", vec!["dev".to_string()]);
        ctx.source_ip = Some("192.168.1.1".parse().unwrap());  // External

        let decision = engine.evaluate(&ctx);

        assert!(decision.is_denied());
        assert_eq!(decision.deny_message(), Some("Internal only"));
    }

    #[test]
    fn test_allow_internal() {
        let policies = create_test_policies();
        let engine = PolicyEngine::new(&policies);

        let mut ctx = RequestContext::new("internal-api", vec!["dev".to_string()]);
        ctx.source_ip = Some("10.1.2.3".parse().unwrap());  // Internal

        let decision = engine.evaluate(&ctx);

        assert!(decision.is_allowed());
    }

    #[test]
    fn test_accumulated_actions() {
        let policies = create_test_policies();
        let engine = PolicyEngine::new(&policies);

        let mut ctx = RequestContext::new("legacy-app", vec!["production".to_string()]);
        ctx.property_paths = vec!["database.password".to_string()];

        let decision = engine.evaluate(&ctx);

        assert!(decision.is_allowed());
        let actions = decision.actions().unwrap();
        // Should have both mask and warn actions
        assert_eq!(actions.len(), 2);
    }

    #[test]
    fn test_priority_order() {
        let policies = create_test_policies();
        let engine = PolicyEngine::new(&policies);

        // trace muestra orden de evaluacion
        let ctx = RequestContext::new("some-app", vec!["dev".to_string()]);
        let trace = engine.evaluate_with_trace(&ctx);

        // Policies should be evaluated in priority order
        assert_eq!(trace.policy_results[0].policy_name, "deny-external");
        assert_eq!(trace.policy_results[0].priority, 200);
        assert_eq!(trace.policy_results[1].policy_name, "mask-secrets");
        assert_eq!(trace.policy_results[1].priority, 100);
    }
}
```

---

## Seguridad

- **Fail-closed**: Si hay error evaluando, denegar por defecto
- **No leak de politicas**: Los errores no revelan nombres o condiciones de politicas
- **Audit logging**: Loguear todas las decisiones deny con contexto
- **Rate limiting**: Considerar limite de evaluaciones por segundo
- **Regex timeout**: Las evaluaciones de regex tienen limite de tiempo implicito

---

## Entregable Final

### Archivos Creados

1. `src/plac/context.rs` - RequestContext y builder
2. `src/plac/evaluator.rs` - ConditionEvaluator
3. `src/plac/decision.rs` - PolicyDecision y AppliedAction
4. `src/plac/engine.rs` - PolicyEngine
5. `src/plac/visitor.rs` - (Opcional) Visitor pattern

### Verificacion

```bash
# Compilar
cargo build -p vortex-governance

# Tests
cargo test -p vortex-governance -- engine

# Test de evaluacion manual
cargo run -p vortex-governance -- evaluate \
    --policies /path/to/policies.yaml \
    --app myapp \
    --profile production \
    --source-ip 10.0.0.1

# Output esperado:
# Evaluating 3 policies...
# Policy 'deny-external': NO MATCH (source_ip in internal range)
# Policy 'mask-secrets': MATCH
# Policy 'warn-deprecated': NO MATCH (app != legacy-app)
# Decision: ALLOW with 1 action(s)
#   - mask (priority: 100)
```
