# Historia 004: Configuration Templating

## Contexto y Objetivo

Esta historia integra el motor de templates Tera para permitir configuraciones dinamicas. Los templates permiten:

- **Variables dinamicas**: Insertar valores del contexto en configuracion
- **Logica condicional**: Configuracion diferente segun ambiente/perfil
- **Loops y filtros**: Generar estructuras repetitivas
- **Reutilizacion**: Includes y macros para DRY

Tera es un motor de templates inspirado en Jinja2/Django, muy familiar para desarrolladores Python pero con las garantias de seguridad de Rust.

Para desarrolladores Java, Tera es conceptualmente similar a Thymeleaf o Freemarker, pero mas ligero y con sintaxis mas moderna.

---

## Alcance

### In Scope

- Integracion de Tera como motor de templates
- `TemplateEngine` wrapper con sandboxing
- Rendering de configuraciones con contexto
- Filtros basicos (upper, lower, default)
- Manejo de errores de template
- Cache de templates compilados

### Out of Scope

- Funciones custom (historia 005)
- Auto-reload de templates
- Template inheritance complejos
- Internacionalizacion

---

## Criterios de Aceptacion

- [ ] Templates Tera se renderizan correctamente
- [ ] Contexto inyectado con variables de aplicacion/ambiente
- [ ] Filtros basicos funcionan (upper, lower, trim, default)
- [ ] Errores de template retornan mensajes utiles
- [ ] Templates cacheados para performance
- [ ] Sandbox previene acceso a filesystem
- [ ] Tests de rendering pasan

---

## Diseno Propuesto

### Arquitectura

```
┌─────────────────────────────────────────────────────────────────────┐
│                        TemplateEngine                                │
├─────────────────────────────────────────────────────────────────────┤
│  tera: Tera                    (Template compiler/runtime)           │
│  context_builder: ContextBuilder  (Build Tera context)               │
├─────────────────────────────────────────────────────────────────────┤
│  + render(template_str, context) -> Result<String>                   │
│  + render_config(config, context) -> Result<ConfigMap>               │
│  + validate_template(template_str) -> Result<()>                     │
└─────────────────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       TemplateContext                                │
├─────────────────────────────────────────────────────────────────────┤
│  app: String                    Application name                     │
│  profiles: Vec<String>          Active profiles                      │
│  environment: String            Deployment environment               │
│  variables: HashMap<String, Value>  Custom variables                 │
│  properties: HashMap<String, Value> Config properties                │
└─────────────────────────────────────────────────────────────────────┘
```

### Flujo de Rendering

```
Input Template:
"{{ app.name }}-{{ environment }}.{{ format | default('yml') }}"

Context:
{
  "app": { "name": "payment" },
  "environment": "production",
  "format": null
}

┌────────────────────────────────────────────────────────────────────┐
│                     Template Rendering Flow                         │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. Parse Template                                                  │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │ Tokens: [Expr(app.name), Lit(-), Expr(environment),     │    │
│     │         Lit(.), Expr(format | default('yml'))]          │    │
│     └─────────────────────────────────────────────────────────┘    │
│                                                                     │
│  2. Build Tera Context                                              │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │ {                                                       │    │
│     │   "app": { "name": "payment" },                         │    │
│     │   "environment": "production",                          │    │
│     │   "format": null                                        │    │
│     │ }                                                       │    │
│     └─────────────────────────────────────────────────────────┘    │
│                                                                     │
│  3. Evaluate Expressions                                            │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │ app.name         -> "payment"                           │    │
│     │ environment      -> "production"                        │    │
│     │ format | default -> "yml" (null fallback)               │    │
│     └─────────────────────────────────────────────────────────┘    │
│                                                                     │
│  4. Concatenate Result                                              │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │ "payment-production.yml"                                │    │
│     └─────────────────────────────────────────────────────────┘    │
│                                                                     │
└────────────────────────────────────────────────────────────────────┘
```

---

## Pasos de Implementacion

### Paso 1: Agregar Dependencias

```toml
# crates/vortex-templating/Cargo.toml
[dependencies]
tera = "1.19"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
tracing = "0.1"
```

### Paso 2: Definir TemplateContext

```rust
// src/templating/context.rs
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tera::Context as TeraContext;

/// Context for template rendering.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemplateContext {
    /// Application name.
    #[serde(default)]
    pub app: String,

    /// Active profiles.
    #[serde(default)]
    pub profiles: Vec<String>,

    /// Deployment environment (dev, staging, prod).
    #[serde(default)]
    pub environment: String,

    /// Label (branch, tag, version).
    #[serde(default)]
    pub label: Option<String>,

    /// Custom variables for templates.
    #[serde(default)]
    pub variables: HashMap<String, Value>,

    /// Configuration properties (for self-reference).
    #[serde(default)]
    pub properties: HashMap<String, Value>,
}

impl TemplateContext {
    /// Creates a new context for the given app and environment.
    pub fn new(app: impl Into<String>, environment: impl Into<String>) -> Self {
        Self {
            app: app.into(),
            environment: environment.into(),
            ..Default::default()
        }
    }

    /// Adds profiles to the context.
    pub fn with_profiles(mut self, profiles: Vec<String>) -> Self {
        self.profiles = profiles;
        self
    }

    /// Adds a label (branch/tag).
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Adds a custom variable.
    pub fn with_variable(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.variables.insert(key.into(), value.into());
        self
    }

    /// Adds multiple variables.
    pub fn with_variables(mut self, vars: HashMap<String, Value>) -> Self {
        self.variables.extend(vars);
        self
    }

    /// Sets properties (config values for self-reference).
    pub fn with_properties(mut self, props: HashMap<String, Value>) -> Self {
        self.properties = props;
        self
    }

    /// Converts to Tera context.
    pub fn to_tera_context(&self) -> TeraContext {
        let mut ctx = TeraContext::new();

        // Add standard fields
        ctx.insert("app", &self.app);
        ctx.insert("profiles", &self.profiles);
        ctx.insert("environment", &self.environment);

        if let Some(ref label) = self.label {
            ctx.insert("label", label);
        }

        // Add variables under 'vars' namespace
        ctx.insert("vars", &self.variables);

        // Add properties under 'props' namespace
        ctx.insert("props", &self.properties);

        // Also add variables at root level for convenience
        for (key, value) in &self.variables {
            ctx.insert(key, value);
        }

        ctx
    }
}

/// Builder for TemplateContext.
pub struct TemplateContextBuilder {
    context: TemplateContext,
}

impl TemplateContextBuilder {
    pub fn new() -> Self {
        Self {
            context: TemplateContext::default(),
        }
    }

    pub fn app(mut self, app: impl Into<String>) -> Self {
        self.context.app = app.into();
        self
    }

    pub fn environment(mut self, env: impl Into<String>) -> Self {
        self.context.environment = env.into();
        self
    }

    pub fn profiles(mut self, profiles: Vec<String>) -> Self {
        self.context.profiles = profiles;
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.context.label = Some(label.into());
        self
    }

    pub fn variable(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        self.context.variables.insert(
            key.into(),
            serde_json::to_value(value).unwrap_or(Value::Null),
        );
        self
    }

    pub fn build(self) -> TemplateContext {
        self.context
    }
}

impl Default for TemplateContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}
```

### Paso 3: Definir Errores de Template

```rust
// src/templating/error.rs
use thiserror::Error;

/// Errors that can occur during template processing.
#[derive(Debug, Error)]
pub enum TemplateError {
    /// Template syntax error.
    #[error("template syntax error: {message}")]
    SyntaxError {
        message: String,
        line: Option<usize>,
        column: Option<usize>,
    },

    /// Variable not found in context.
    #[error("variable not found: {name}")]
    VariableNotFound { name: String },

    /// Filter not found.
    #[error("filter not found: {name}")]
    FilterNotFound { name: String },

    /// Rendering error.
    #[error("render error: {0}")]
    RenderError(String),

    /// Invalid template configuration.
    #[error("invalid template: {0}")]
    InvalidTemplate(String),
}

impl From<tera::Error> for TemplateError {
    fn from(err: tera::Error) -> Self {
        match err.kind {
            tera::ErrorKind::SyntaxError(msg) => TemplateError::SyntaxError {
                message: msg,
                line: None,
                column: None,
            },
            tera::ErrorKind::InvalidMacroDefinition(msg) => {
                TemplateError::SyntaxError {
                    message: format!("Invalid macro: {}", msg),
                    line: None,
                    column: None,
                }
            }
            _ => TemplateError::RenderError(err.to_string()),
        }
    }
}
```

### Paso 4: Implementar TemplateEngine

```rust
// src/templating/engine.rs
use std::sync::Arc;
use parking_lot::RwLock;
use tera::Tera;
use tracing::{debug, instrument, warn};

use super::context::TemplateContext;
use super::error::TemplateError;

/// Template engine wrapping Tera with Vortex-specific features.
pub struct TemplateEngine {
    tera: Arc<RwLock<Tera>>,
}

impl TemplateEngine {
    /// Creates a new template engine.
    pub fn new() -> Self {
        let mut tera = Tera::default();

        // Configure autoescape for security
        tera.autoescape_on(vec![]);  // No auto-escape for config files

        // Register built-in filters
        Self::register_builtin_filters(&mut tera);

        Self {
            tera: Arc::new(RwLock::new(tera)),
        }
    }

    /// Registers built-in filters.
    fn register_builtin_filters(tera: &mut Tera) {
        // Tera already has most common filters:
        // - upper, lower, capitalize
        // - trim, truncate
        // - default
        // - json_encode
        // - urlencode
        // etc.

        // We could add custom filters here if needed
    }

    /// Renders a template string with the given context.
    #[instrument(skip(self, template, context), fields(template_len = template.len()))]
    pub fn render(
        &self,
        template: &str,
        context: &TemplateContext,
    ) -> Result<String, TemplateError> {
        let tera = self.tera.read();
        let tera_ctx = context.to_tera_context();

        debug!("Rendering template with context");

        // Use one-off rendering for inline templates
        tera.render_str(template, &tera_ctx)
            .map_err(TemplateError::from)
    }

    /// Validates a template without rendering.
    pub fn validate(&self, template: &str) -> Result<(), TemplateError> {
        // Try to parse the template
        let tera = self.tera.read();

        // Create a temporary Tera instance to validate
        let mut temp_tera = Tera::default();
        temp_tera
            .add_raw_template("validation", template)
            .map_err(TemplateError::from)?;

        Ok(())
    }

    /// Renders all string values in a JSON structure.
    pub fn render_json(
        &self,
        value: &serde_json::Value,
        context: &TemplateContext,
    ) -> Result<serde_json::Value, TemplateError> {
        match value {
            serde_json::Value::String(s) => {
                // Check if string contains template syntax
                if s.contains("{{") || s.contains("{%") {
                    let rendered = self.render(s, context)?;
                    Ok(serde_json::Value::String(rendered))
                } else {
                    Ok(value.clone())
                }
            }
            serde_json::Value::Array(arr) => {
                let rendered: Result<Vec<_>, _> = arr
                    .iter()
                    .map(|v| self.render_json(v, context))
                    .collect();
                Ok(serde_json::Value::Array(rendered?))
            }
            serde_json::Value::Object(obj) => {
                let rendered: Result<serde_json::Map<String, serde_json::Value>, _> = obj
                    .iter()
                    .map(|(k, v)| {
                        // Render key if it's a template
                        let rendered_key = if k.contains("{{") {
                            self.render(k, context)?
                        } else {
                            k.clone()
                        };
                        let rendered_value = self.render_json(v, context)?;
                        Ok((rendered_key, rendered_value))
                    })
                    .collect();
                Ok(serde_json::Value::Object(rendered?))
            }
            // Other types pass through unchanged
            _ => Ok(value.clone()),
        }
    }

    /// Pre-compiles a named template for repeated use.
    pub fn add_template(
        &self,
        name: &str,
        template: &str,
    ) -> Result<(), TemplateError> {
        let mut tera = self.tera.write();
        tera.add_raw_template(name, template)
            .map_err(TemplateError::from)
    }

    /// Renders a pre-compiled template by name.
    pub fn render_named(
        &self,
        name: &str,
        context: &TemplateContext,
    ) -> Result<String, TemplateError> {
        let tera = self.tera.read();
        let tera_ctx = context.to_tera_context();

        tera.render(name, &tera_ctx)
            .map_err(TemplateError::from)
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for TemplateEngine {
    fn clone(&self) -> Self {
        Self {
            tera: Arc::clone(&self.tera),
        }
    }
}
```

### Paso 5: Integrar con ConfigMap

```rust
// src/templating/config.rs
use serde_json::{Map, Value};

use crate::core::{ConfigMap, PropertySource};
use super::context::TemplateContext;
use super::engine::TemplateEngine;
use super::error::TemplateError;

/// Extension trait for rendering ConfigMaps with templates.
pub trait ConfigMapTemplating {
    /// Renders all template expressions in the config.
    fn render_templates(
        &self,
        engine: &TemplateEngine,
        context: &TemplateContext,
    ) -> Result<ConfigMap, TemplateError>;
}

impl ConfigMapTemplating for ConfigMap {
    fn render_templates(
        &self,
        engine: &TemplateEngine,
        context: &TemplateContext,
    ) -> Result<ConfigMap, TemplateError> {
        let rendered_sources: Result<Vec<PropertySource>, TemplateError> = self
            .property_sources
            .iter()
            .map(|source| render_property_source(source, engine, context))
            .collect();

        Ok(ConfigMap {
            name: self.name.clone(),
            profiles: self.profiles.clone(),
            label: self.label.clone(),
            version: self.version.clone(),
            state: self.state.clone(),
            property_sources: rendered_sources?,
        })
    }
}

fn render_property_source(
    source: &PropertySource,
    engine: &TemplateEngine,
    context: &TemplateContext,
) -> Result<PropertySource, TemplateError> {
    let mut rendered_map = Map::new();

    for (key, value) in &source.source {
        let rendered_value = engine.render_json(value, context)?;
        rendered_map.insert(key.clone(), rendered_value);
    }

    Ok(PropertySource {
        name: source.name.clone(),
        source: rendered_map,
    })
}
```

### Paso 6: Helpers de Template

```rust
// src/templating/helpers.rs

/// Checks if a string contains template syntax.
pub fn is_template(s: &str) -> bool {
    s.contains("{{") || s.contains("{%") || s.contains("{#")
}

/// Extracts variable names from a template.
pub fn extract_variables(template: &str) -> Vec<String> {
    let mut variables = Vec::new();

    // Simple regex-free extraction for {{ var }} patterns
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' && chars.peek() == Some(&'{') {
            chars.next(); // consume second {

            // Skip whitespace
            while chars.peek().map_or(false, |c| c.is_whitespace()) {
                chars.next();
            }

            // Collect variable name
            let mut var_name = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_alphanumeric() || c == '_' || c == '.' {
                    var_name.push(chars.next().unwrap());
                } else {
                    break;
                }
            }

            if !var_name.is_empty() {
                variables.push(var_name);
            }
        }
    }

    variables
}

/// Common template patterns.
pub mod patterns {
    /// Environment variable: {{ env("VAR_NAME") }}
    pub const ENV_VAR: &str = r#"{{ env("VAR_NAME") }}"#;

    /// Default value: {{ value | default("fallback") }}
    pub const DEFAULT: &str = r#"{{ value | default("fallback") }}"#;

    /// Conditional: {% if condition %}...{% endif %}
    pub const CONDITIONAL: &str = r#"{% if condition %}value{% endif %}"#;

    /// Loop: {% for item in items %}...{% endfor %}
    pub const LOOP: &str = r#"{% for item in items %}{{ item }}{% endfor %}"#;
}
```

---

## Conceptos de Rust Aprendidos

### 1. Interior Mutability con RwLock

Tera necesita ser mutable para agregar templates, pero queremos compartirlo entre threads.

**Rust:**
```rust
use parking_lot::RwLock;
use std::sync::Arc;

pub struct TemplateEngine {
    // Arc: shared ownership entre threads
    // RwLock: multiple readers OR one writer
    tera: Arc<RwLock<Tera>>,
}

impl TemplateEngine {
    pub fn render(&self, template: &str, context: &Context) -> Result<String, Error> {
        // Read lock: multiple concurrent renders OK
        let tera = self.tera.read();
        tera.render_str(template, context)
    }

    pub fn add_template(&self, name: &str, template: &str) -> Result<(), Error> {
        // Write lock: exclusive access
        let mut tera = self.tera.write();
        tera.add_raw_template(name, template)
    }
}
```

**Comparacion con Java:**
```java
public class TemplateEngine {
    private final ReadWriteLock lock = new ReentrantReadWriteLock();
    private final Tera tera;

    public String render(String template, Context context) {
        lock.readLock().lock();
        try {
            return tera.renderStr(template, context);
        } finally {
            lock.readLock().unlock();
        }
    }

    public void addTemplate(String name, String template) {
        lock.writeLock().lock();
        try {
            tera.addRawTemplate(name, template);
        } finally {
            lock.writeLock().unlock();
        }
    }
}
```

### 2. Recursive JSON Processing

**Rust:**
```rust
pub fn render_json(
    &self,
    value: &Value,
    context: &TemplateContext,
) -> Result<Value, TemplateError> {
    match value {
        // String: potentially a template
        Value::String(s) => {
            if s.contains("{{") {
                Ok(Value::String(self.render(s, context)?))
            } else {
                Ok(value.clone())
            }
        }
        // Array: recurse into each element
        Value::Array(arr) => {
            let rendered: Result<Vec<_>, _> = arr
                .iter()
                .map(|v| self.render_json(v, context))
                .collect();
            Ok(Value::Array(rendered?))
        }
        // Object: recurse into values
        Value::Object(obj) => {
            let rendered: Result<Map<String, Value>, _> = obj
                .iter()
                .map(|(k, v)| {
                    Ok((k.clone(), self.render_json(v, context)?))
                })
                .collect();
            Ok(Value::Object(rendered?))
        }
        // Primitives: pass through
        _ => Ok(value.clone()),
    }
}
```

**Comparacion con Java:**
```java
public JsonNode renderJson(JsonNode value, TemplateContext context) {
    if (value.isTextual()) {
        String text = value.asText();
        if (text.contains("{{")) {
            return TextNode.valueOf(render(text, context));
        }
        return value;
    }
    if (value.isArray()) {
        ArrayNode result = JsonNodeFactory.instance.arrayNode();
        for (JsonNode element : value) {
            result.add(renderJson(element, context));
        }
        return result;
    }
    if (value.isObject()) {
        ObjectNode result = JsonNodeFactory.instance.objectNode();
        value.fields().forEachRemaining(entry ->
            result.set(entry.getKey(), renderJson(entry.getValue(), context))
        );
        return result;
    }
    return value;
}
```

### 3. From Trait para Conversion de Errores

**Rust:**
```rust
impl From<tera::Error> for TemplateError {
    fn from(err: tera::Error) -> Self {
        match err.kind {
            tera::ErrorKind::SyntaxError(msg) => TemplateError::SyntaxError {
                message: msg,
                line: None,
                column: None,
            },
            _ => TemplateError::RenderError(err.to_string()),
        }
    }
}

// Permite usar ? operator directamente
pub fn render(&self, template: &str, ctx: &Context) -> Result<String, TemplateError> {
    tera.render_str(template, ctx)?  // tera::Error convertido automaticamente
    // equivalente a:
    // tera.render_str(template, ctx).map_err(TemplateError::from)?
}
```

**Comparacion con Java:**
```java
// Java: constructores de excepcion o metodos estaticos
public class TemplateException extends Exception {
    public static TemplateException fromTeraError(TeraError err) {
        if (err instanceof SyntaxError) {
            return new TemplateSyntaxException(err.getMessage());
        }
        return new TemplateRenderException(err.getMessage());
    }
}

// Uso manual
try {
    return tera.renderStr(template, ctx);
} catch (TeraError e) {
    throw TemplateException.fromTeraError(e);
}
```

### 4. Extension Traits

**Rust:**
```rust
/// Extension trait para ConfigMap
pub trait ConfigMapTemplating {
    fn render_templates(
        &self,
        engine: &TemplateEngine,
        context: &TemplateContext,
    ) -> Result<ConfigMap, TemplateError>;
}

// Implementar para ConfigMap (definido en otro crate)
impl ConfigMapTemplating for ConfigMap {
    fn render_templates(&self, ...) -> Result<ConfigMap, TemplateError> {
        // ...
    }
}

// Uso
let rendered = config_map.render_templates(&engine, &context)?;
```

**Comparacion con Java:**
```java
// Java: utility class o wrapper
public class ConfigMapTemplating {
    public static ConfigMap renderTemplates(
        ConfigMap config,
        TemplateEngine engine,
        TemplateContext context
    ) {
        // ...
    }
}

// Uso
ConfigMap rendered = ConfigMapTemplating.renderTemplates(configMap, engine, context);

// O con patron decorator
public class TemplatedConfigMap extends ConfigMap {
    private final TemplateEngine engine;

    public ConfigMap render(TemplateContext context) {
        // ...
    }
}
```

---

## Riesgos y Errores Comunes

### 1. Template Injection

```rust
// MAL: Contenido de usuario como template
let user_input = get_user_input();
let rendered = engine.render(&user_input, &context)?;  // PELIGROSO!

// BIEN: Solo templates predefinidos o validados
let template = load_template_from_trusted_source()?;
let rendered = engine.render(&template, &context)?;
```

### 2. Loops Infinitos en Templates

```rust
// MAL: Sin limite de iteraciones
{% for i in range(end=1000000) %}...{% endfor %}

// BIEN: Configurar limite en Tera (no disponible directamente)
// Validar templates antes de usar
fn validate_template(template: &str) -> Result<(), Error> {
    // Check for dangerous patterns
    if template.contains("range(end=") {
        let re = regex::Regex::new(r"range\(end=(\d+)\)").unwrap();
        if let Some(caps) = re.captures(template) {
            let limit: usize = caps[1].parse().unwrap_or(0);
            if limit > 1000 {
                return Err(Error::LoopTooLarge);
            }
        }
    }
    Ok(())
}
```

### 3. Variables No Definidas

```rust
// MAL: Crash si variable no existe
{{ undefined_variable }}

// BIEN: Usar default filter
{{ undefined_variable | default("") }}

// O configurar Tera para no fallar
tera.set_fail_on_missing_include(false);
```

### 4. Escapado Incorrecto

```rust
// MAL: Auto-escape puede romper YAML/JSON
tera.autoescape_on(vec!["yml", "yaml", "json"]);

// BIEN: Sin auto-escape para config files
tera.autoescape_on(vec![]);  // Disabled for config

// Escapar manualmente cuando sea necesario
{{ user_input | escape }}
```

---

## Pruebas

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_simple_variable_substitution() {
        let engine = TemplateEngine::new();
        let context = TemplateContext::new("myapp", "production");

        let result = engine.render("App: {{ app }}, Env: {{ environment }}", &context);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "App: myapp, Env: production");
    }

    #[test]
    fn test_nested_variable() {
        let engine = TemplateEngine::new();
        let mut context = TemplateContext::new("myapp", "prod");
        context.variables.insert(
            "database".to_string(),
            json!({ "host": "localhost", "port": 5432 }),
        );

        let template = "Host: {{ database.host }}:{{ database.port }}";
        let result = engine.render(template, &context).unwrap();

        assert_eq!(result, "Host: localhost:5432");
    }

    #[test]
    fn test_default_filter() {
        let engine = TemplateEngine::new();
        let context = TemplateContext::new("myapp", "prod");

        let template = r#"{{ missing | default("fallback") }}"#;
        let result = engine.render(template, &context).unwrap();

        assert_eq!(result, "fallback");
    }

    #[test]
    fn test_conditional() {
        let engine = TemplateEngine::new();
        let mut context = TemplateContext::new("myapp", "production");
        context.variables.insert("debug".to_string(), json!(true));

        let template = r#"{% if debug %}DEBUG{% else %}RELEASE{% endif %}"#;
        let result = engine.render(template, &context).unwrap();

        assert_eq!(result, "DEBUG");
    }

    #[test]
    fn test_loop() {
        let engine = TemplateEngine::new();
        let mut context = TemplateContext::new("myapp", "prod");
        context.variables.insert(
            "servers".to_string(),
            json!(["server1", "server2", "server3"]),
        );

        let template = r#"{% for s in servers %}{{ s }},{% endfor %}"#;
        let result = engine.render(template, &context).unwrap();

        assert_eq!(result, "server1,server2,server3,");
    }

    #[test]
    fn test_filters() {
        let engine = TemplateEngine::new();
        let mut context = TemplateContext::new("myapp", "prod");
        context.variables.insert("name".to_string(), json!("hello world"));

        let tests = vec![
            ("{{ name | upper }}", "HELLO WORLD"),
            ("{{ name | lower }}", "hello world"),
            ("{{ name | capitalize }}", "Hello world"),
            ("{{ name | title }}", "Hello World"),
        ];

        for (template, expected) in tests {
            let result = engine.render(template, &context).unwrap();
            assert_eq!(result, expected, "Failed for template: {}", template);
        }
    }

    #[test]
    fn test_render_json() {
        let engine = TemplateEngine::new();
        let context = TemplateContext::new("myapp", "production");

        let input = json!({
            "static": "no templates here",
            "dynamic": "App: {{ app }}",
            "nested": {
                "env": "Environment: {{ environment }}"
            },
            "array": [
                "{{ app }}-1",
                "{{ app }}-2"
            ]
        });

        let result = engine.render_json(&input, &context).unwrap();

        assert_eq!(result["static"], "no templates here");
        assert_eq!(result["dynamic"], "App: myapp");
        assert_eq!(result["nested"]["env"], "Environment: production");
        assert_eq!(result["array"][0], "myapp-1");
        assert_eq!(result["array"][1], "myapp-2");
    }

    #[test]
    fn test_validate_valid_template() {
        let engine = TemplateEngine::new();

        let result = engine.validate("{{ app }} - {{ environment }}");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_invalid_template() {
        let engine = TemplateEngine::new();

        let result = engine.validate("{{ unclosed");
        assert!(result.is_err());
    }

    #[test]
    fn test_is_template() {
        assert!(is_template("{{ var }}"));
        assert!(is_template("{% if x %}{% endif %}"));
        assert!(is_template("{# comment #}"));
        assert!(!is_template("no templates here"));
        assert!(!is_template("{ not a template }"));
    }

    #[test]
    fn test_extract_variables() {
        let vars = extract_variables("{{ app }}-{{ database.host }}:{{ port }}");

        assert_eq!(vars, vec!["app", "database.host", "port"]);
    }
}
```

---

## Seguridad

### Consideraciones

1. **Sandbox**: Tera no tiene acceso a filesystem por defecto
2. **No eval**: No hay ejecucion de codigo arbitrario
3. **Resource limits**: Validar templates para loops grandes
4. **Escape**: Considerar XSS si output es HTML

```rust
/// Validates a template for security concerns.
pub fn validate_template_security(template: &str) -> Result<(), SecurityError> {
    // Check template length
    if template.len() > 100_000 {
        return Err(SecurityError::TemplateTooLarge);
    }

    // Check for dangerous patterns
    let dangerous_patterns = [
        ("range(end=", "Large loops not allowed"),
        ("include(", "Includes not allowed in user templates"),
    ];

    for (pattern, message) in dangerous_patterns {
        if template.contains(pattern) {
            return Err(SecurityError::DangerousPattern(message.to_string()));
        }
    }

    Ok(())
}
```

---

## Entregable Final

### Archivos Creados

1. `src/templating/mod.rs` - Module exports
2. `src/templating/context.rs` - TemplateContext
3. `src/templating/engine.rs` - TemplateEngine
4. `src/templating/error.rs` - Error types
5. `src/templating/config.rs` - ConfigMap integration
6. `src/templating/helpers.rs` - Helper functions
7. `tests/templating_test.rs` - Tests

### Verificacion

```bash
cargo build -p vortex-templating
cargo test -p vortex-templating
cargo clippy -p vortex-templating -- -D warnings
```

### Ejemplo de Uso

```rust
use vortex_templating::{TemplateEngine, TemplateContext};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = TemplateEngine::new();

    // Build context
    let context = TemplateContext::new("payment-service", "production")
        .with_profiles(vec!["cloud".to_string(), "secure".to_string()])
        .with_variable("region", "us-east-1")
        .with_variable("database", json!({
            "host": "db.example.com",
            "port": 5432
        }));

    // Render a template
    let template = r#"
spring:
  application:
    name: {{ app }}
  profiles:
    active: {% for p in profiles %}{{ p }}{% if not loop.last %},{% endif %}{% endfor %}

database:
  url: jdbc:postgresql://{{ database.host }}:{{ database.port }}/{{ app }}
  pool-size: {% if environment == "production" %}20{% else %}5{% endif %}

cloud:
  region: {{ region }}
"#;

    let rendered = engine.render(template, &context)?;
    println!("{}", rendered);

    Ok(())
}
```

**Output:**
```yaml
spring:
  application:
    name: payment-service
  profiles:
    active: cloud,secure

database:
  url: jdbc:postgresql://db.example.com:5432/payment-service
  pool-size: 20

cloud:
  region: us-east-1
```

---

**Anterior**: [Historia 003 - API de Feature Flags](./story-003-flag-api.md)
**Siguiente**: [Historia 005 - Funciones Built-in de Templates](./story-005-template-functions.md)
