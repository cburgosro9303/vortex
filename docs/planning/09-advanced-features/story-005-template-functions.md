# Historia 005: Funciones Built-in de Templates

## Contexto y Objetivo

Esta historia extiende el motor de templates con funciones custom que proporcionan capacidades especificas para configuracion:

- **env()**: Leer variables de entorno
- **secret()**: Resolver secretos de Vault/AWS Secrets Manager
- **base64_encode/decode()**: Codificacion base64
- **urlencode()**: Codificacion URL
- **now()**: Timestamp actual
- **uuid()**: Generar UUIDs

Estas funciones permiten crear configuraciones que se adaptan al entorno de ejecucion sin hardcodear valores sensibles.

Para desarrolladores Java, esto es similar a los custom dialects de Thymeleaf o las funciones de Freemarker.

---

## Alcance

### In Scope

- Funcion `env("VAR_NAME", default)` para variables de entorno
- Funcion `secret("path/to/secret")` con mock para desarrollo
- Funciones `base64_encode()` y `base64_decode()`
- Funcion `urlencode()`
- Funciones `now()` y `uuid()`
- Filtros adicionales: `to_json`, `from_json`, `sha256`
- Sandboxing de funciones peligrosas

### Out of Scope

- Integracion real con Vault (epica de secrets)
- Funciones de filesystem
- Funciones de red (HTTP calls)
- Funciones async

---

## Criterios de Aceptacion

- [ ] `env("VAR")` retorna variable de entorno
- [ ] `env("VAR", "default")` retorna default si no existe
- [ ] `secret("path")` simula lectura de secret
- [ ] `base64_encode/decode` funcionan correctamente
- [ ] `urlencode` escapa caracteres especiales
- [ ] `now()` retorna timestamp ISO
- [ ] `uuid()` genera UUID v4 valido
- [ ] Funciones documentadas con ejemplos
- [ ] Tests pasan para todas las funciones

---

## Diseno Propuesto

### Funciones Disponibles

```
┌──────────────────────────────────────────────────────────────────────┐
│                    Built-in Template Functions                        │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  ENVIRONMENT                                                          │
│  ├── env(name)              Read environment variable                 │
│  ├── env(name, default)     Read with fallback                        │
│  └── required_env(name)     Read, fail if missing                     │
│                                                                       │
│  SECRETS                                                              │
│  ├── secret(path)           Read secret (mock/Vault)                  │
│  └── secret(path, key)      Read specific key from secret             │
│                                                                       │
│  ENCODING                                                             │
│  ├── base64_encode(str)     Encode to base64                          │
│  ├── base64_decode(str)     Decode from base64                        │
│  ├── urlencode(str)         URL encode                                │
│  └── urldecode(str)         URL decode                                │
│                                                                       │
│  HASHING                                                              │
│  ├── sha256(str)            SHA-256 hash (hex)                        │
│  ├── md5(str)               MD5 hash (hex) - legacy only              │
│  └── hmac_sha256(str, key)  HMAC-SHA256                               │
│                                                                       │
│  DATETIME                                                             │
│  ├── now()                  Current ISO timestamp                     │
│  ├── now(format)            Current time with format                  │
│  └── timestamp()            Unix timestamp (seconds)                  │
│                                                                       │
│  GENERATORS                                                           │
│  ├── uuid()                 Generate UUID v4                          │
│  └── random_string(len)     Generate random alphanumeric              │
│                                                                       │
│  JSON                                                                 │
│  ├── to_json(value)         Serialize to JSON string                  │
│  └── from_json(str)         Parse JSON string                         │
│                                                                       │
└──────────────────────────────────────────────────────────────────────┘
```

### Ejemplo de Template

```yaml
# config.yml.tera
database:
  host: {{ env("DB_HOST", "localhost") }}
  port: {{ env("DB_PORT", "5432") | int }}
  username: {{ env("DB_USER") }}
  password: {{ secret("database/credentials", "password") }}

cache:
  redis:
    url: {{ env("REDIS_URL") | urlencode }}

api:
  key: {{ base64_encode(env("API_KEY")) }}

metadata:
  generated_at: {{ now() }}
  config_id: {{ uuid() }}
  checksum: {{ props | to_json | sha256 }}
```

---

## Pasos de Implementacion

### Paso 1: Definir Configuracion de Funciones

```rust
// src/templating/functions/config.rs
use std::collections::HashMap;

/// Configuration for template functions.
#[derive(Debug, Clone)]
pub struct FunctionConfig {
    /// Whether to allow env() function.
    pub allow_env: bool,

    /// Allowed environment variable prefixes (empty = all allowed).
    pub env_prefixes: Vec<String>,

    /// Whether to allow secret() function.
    pub allow_secrets: bool,

    /// Mock secrets for development/testing.
    pub mock_secrets: HashMap<String, String>,

    /// Secret backend URL (for real implementation).
    pub secret_backend_url: Option<String>,

    /// Whether to allow uuid() function.
    pub allow_uuid: bool,

    /// Whether to allow now() function.
    pub allow_now: bool,
}

impl Default for FunctionConfig {
    fn default() -> Self {
        Self {
            allow_env: true,
            env_prefixes: vec![],  // All allowed
            allow_secrets: true,
            mock_secrets: HashMap::new(),
            secret_backend_url: None,
            allow_uuid: true,
            allow_now: true,
        }
    }
}

impl FunctionConfig {
    /// Creates a restrictive config for untrusted templates.
    pub fn restricted() -> Self {
        Self {
            allow_env: false,
            env_prefixes: vec![],
            allow_secrets: false,
            mock_secrets: HashMap::new(),
            secret_backend_url: None,
            allow_uuid: true,
            allow_now: true,
        }
    }

    /// Creates a development config with mock secrets.
    pub fn development() -> Self {
        let mut mock_secrets = HashMap::new();
        mock_secrets.insert("database/credentials".to_string(), r#"{"password": "dev-password"}"#.to_string());
        mock_secrets.insert("api/keys".to_string(), r#"{"key": "dev-api-key"}"#.to_string());

        Self {
            mock_secrets,
            ..Default::default()
        }
    }
}
```

### Paso 2: Implementar Funciones de Entorno

```rust
// src/templating/functions/env.rs
use std::collections::HashMap;
use std::env;
use tera::{Function, Result, Value};

use super::config::FunctionConfig;

/// Creates the env() function.
pub fn make_env_function(config: FunctionConfig) -> impl Function {
    Box::new(move |args: &HashMap<String, Value>| -> Result<Value> {
        if !config.allow_env {
            return Err("env() function is not allowed".into());
        }

        let name = args
            .get("name")
            .or_else(|| args.get("__tera_positional_0"))
            .ok_or("env() requires a variable name")?
            .as_str()
            .ok_or("Variable name must be a string")?;

        // Check allowed prefixes
        if !config.env_prefixes.is_empty() {
            let allowed = config.env_prefixes.iter().any(|p| name.starts_with(p));
            if !allowed {
                return Err(format!(
                    "Environment variable '{}' not in allowed prefixes",
                    name
                ).into());
            }
        }

        let default = args
            .get("default")
            .or_else(|| args.get("__tera_positional_1"));

        match env::var(name) {
            Ok(value) => Ok(Value::String(value)),
            Err(_) => match default {
                Some(v) => Ok(v.clone()),
                None => Ok(Value::Null),
            }
        }
    })
}

/// Creates the required_env() function that errors if var is missing.
pub fn make_required_env_function(config: FunctionConfig) -> impl Function {
    Box::new(move |args: &HashMap<String, Value>| -> Result<Value> {
        if !config.allow_env {
            return Err("required_env() function is not allowed".into());
        }

        let name = args
            .get("name")
            .or_else(|| args.get("__tera_positional_0"))
            .ok_or("required_env() requires a variable name")?
            .as_str()
            .ok_or("Variable name must be a string")?;

        // Check allowed prefixes
        if !config.env_prefixes.is_empty() {
            let allowed = config.env_prefixes.iter().any(|p| name.starts_with(p));
            if !allowed {
                return Err(format!(
                    "Environment variable '{}' not in allowed prefixes",
                    name
                ).into());
            }
        }

        match env::var(name) {
            Ok(value) => Ok(Value::String(value)),
            Err(_) => Err(format!(
                "Required environment variable '{}' is not set",
                name
            ).into()),
        }
    })
}
```

### Paso 3: Implementar Funciones de Secrets

```rust
// src/templating/functions/secrets.rs
use std::collections::HashMap;
use tera::{Function, Result, Value};

use super::config::FunctionConfig;

/// Creates the secret() function.
pub fn make_secret_function(config: FunctionConfig) -> impl Function {
    Box::new(move |args: &HashMap<String, Value>| -> Result<Value> {
        if !config.allow_secrets {
            return Err("secret() function is not allowed".into());
        }

        let path = args
            .get("path")
            .or_else(|| args.get("__tera_positional_0"))
            .ok_or("secret() requires a path")?
            .as_str()
            .ok_or("Path must be a string")?;

        let key = args
            .get("key")
            .or_else(|| args.get("__tera_positional_1"))
            .and_then(|v| v.as_str());

        // Check mock secrets first
        if let Some(mock_value) = config.mock_secrets.get(path) {
            if let Some(key) = key {
                // Parse as JSON and extract key
                let json: serde_json::Value = serde_json::from_str(mock_value)
                    .map_err(|e| format!("Failed to parse secret as JSON: {}", e))?;

                return json.get(key)
                    .map(|v| match v {
                        serde_json::Value::String(s) => Value::String(s.clone()),
                        _ => Value::String(v.to_string()),
                    })
                    .ok_or_else(|| format!("Key '{}' not found in secret", key).into());
            }
            return Ok(Value::String(mock_value.clone()));
        }

        // In production, would call secret backend here
        // For now, return a placeholder
        if config.secret_backend_url.is_some() {
            // TODO: Implement actual secret fetching
            return Err(format!(
                "Secret backend not implemented. Path: {}",
                path
            ).into());
        }

        Err(format!("Secret not found: {}", path).into())
    })
}

/// A SecretString that redacts itself in logs.
#[derive(Clone)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecretString([REDACTED])")
    }
}

impl std::fmt::Display for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED]")
    }
}
```

### Paso 4: Implementar Funciones de Encoding

```rust
// src/templating/functions/encoding.rs
use std::collections::HashMap;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use percent_encoding::{utf8_percent_encode, percent_decode_str, NON_ALPHANUMERIC};
use tera::{Function, Result, Value};

/// Creates the base64_encode() function.
pub fn make_base64_encode_function() -> impl Function {
    Box::new(|args: &HashMap<String, Value>| -> Result<Value> {
        let input = args
            .get("value")
            .or_else(|| args.get("__tera_positional_0"))
            .ok_or("base64_encode() requires a value")?
            .as_str()
            .ok_or("Value must be a string")?;

        let encoded = BASE64.encode(input.as_bytes());
        Ok(Value::String(encoded))
    })
}

/// Creates the base64_decode() function.
pub fn make_base64_decode_function() -> impl Function {
    Box::new(|args: &HashMap<String, Value>| -> Result<Value> {
        let input = args
            .get("value")
            .or_else(|| args.get("__tera_positional_0"))
            .ok_or("base64_decode() requires a value")?
            .as_str()
            .ok_or("Value must be a string")?;

        let decoded_bytes = BASE64
            .decode(input)
            .map_err(|e| format!("Invalid base64: {}", e))?;

        let decoded_str = String::from_utf8(decoded_bytes)
            .map_err(|e| format!("Decoded bytes are not valid UTF-8: {}", e))?;

        Ok(Value::String(decoded_str))
    })
}

/// Creates the urlencode() function.
pub fn make_urlencode_function() -> impl Function {
    Box::new(|args: &HashMap<String, Value>| -> Result<Value> {
        let input = args
            .get("value")
            .or_else(|| args.get("__tera_positional_0"))
            .ok_or("urlencode() requires a value")?
            .as_str()
            .ok_or("Value must be a string")?;

        let encoded = utf8_percent_encode(input, NON_ALPHANUMERIC).to_string();
        Ok(Value::String(encoded))
    })
}

/// Creates the urldecode() function.
pub fn make_urldecode_function() -> impl Function {
    Box::new(|args: &HashMap<String, Value>| -> Result<Value> {
        let input = args
            .get("value")
            .or_else(|| args.get("__tera_positional_0"))
            .ok_or("urldecode() requires a value")?
            .as_str()
            .ok_or("Value must be a string")?;

        let decoded = percent_decode_str(input)
            .decode_utf8()
            .map_err(|e| format!("Invalid URL encoding: {}", e))?
            .into_owned();

        Ok(Value::String(decoded))
    })
}
```

### Paso 5: Implementar Funciones de Hashing

```rust
// src/templating/functions/hashing.rs
use std::collections::HashMap;
use sha2::{Sha256, Digest};
use hmac::{Hmac, Mac};
use tera::{Function, Result, Value};

type HmacSha256 = Hmac<Sha256>;

/// Creates the sha256() function.
pub fn make_sha256_function() -> impl Function {
    Box::new(|args: &HashMap<String, Value>| -> Result<Value> {
        let input = args
            .get("value")
            .or_else(|| args.get("__tera_positional_0"))
            .ok_or("sha256() requires a value")?
            .as_str()
            .ok_or("Value must be a string")?;

        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        let result = hasher.finalize();

        Ok(Value::String(hex::encode(result)))
    })
}

/// Creates the hmac_sha256() function.
pub fn make_hmac_sha256_function() -> impl Function {
    Box::new(|args: &HashMap<String, Value>| -> Result<Value> {
        let input = args
            .get("value")
            .or_else(|| args.get("__tera_positional_0"))
            .ok_or("hmac_sha256() requires a value")?
            .as_str()
            .ok_or("Value must be a string")?;

        let key = args
            .get("key")
            .or_else(|| args.get("__tera_positional_1"))
            .ok_or("hmac_sha256() requires a key")?
            .as_str()
            .ok_or("Key must be a string")?;

        let mut mac = HmacSha256::new_from_slice(key.as_bytes())
            .map_err(|e| format!("Invalid HMAC key: {}", e))?;

        mac.update(input.as_bytes());
        let result = mac.finalize();

        Ok(Value::String(hex::encode(result.into_bytes())))
    })
}
```

### Paso 6: Implementar Funciones de DateTime y Generators

```rust
// src/templating/functions/datetime.rs
use std::collections::HashMap;
use chrono::{Utc, Local};
use tera::{Function, Result, Value};

/// Creates the now() function.
pub fn make_now_function() -> impl Function {
    Box::new(|args: &HashMap<String, Value>| -> Result<Value> {
        let format = args
            .get("format")
            .or_else(|| args.get("__tera_positional_0"))
            .and_then(|v| v.as_str());

        let now = Utc::now();

        let formatted = match format {
            Some(fmt) => now.format(fmt).to_string(),
            None => now.to_rfc3339(),
        };

        Ok(Value::String(formatted))
    })
}

/// Creates the timestamp() function (Unix timestamp).
pub fn make_timestamp_function() -> impl Function {
    Box::new(|_args: &HashMap<String, Value>| -> Result<Value> {
        let timestamp = Utc::now().timestamp();
        Ok(Value::Number(timestamp.into()))
    })
}

// src/templating/functions/generators.rs
use std::collections::HashMap;
use tera::{Function, Result, Value};
use uuid::Uuid;
use rand::Rng;

/// Creates the uuid() function.
pub fn make_uuid_function() -> impl Function {
    Box::new(|_args: &HashMap<String, Value>| -> Result<Value> {
        let uuid = Uuid::new_v4();
        Ok(Value::String(uuid.to_string()))
    })
}

/// Creates the random_string() function.
pub fn make_random_string_function() -> impl Function {
    Box::new(|args: &HashMap<String, Value>| -> Result<Value> {
        let length = args
            .get("length")
            .or_else(|| args.get("__tera_positional_0"))
            .and_then(|v| v.as_u64())
            .unwrap_or(32) as usize;

        if length > 1024 {
            return Err("random_string() length cannot exceed 1024".into());
        }

        let charset: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let mut rng = rand::thread_rng();

        let random: String = (0..length)
            .map(|_| {
                let idx = rng.gen_range(0..charset.len());
                charset[idx] as char
            })
            .collect();

        Ok(Value::String(random))
    })
}
```

### Paso 7: Implementar Filtros JSON

```rust
// src/templating/filters/json.rs
use std::collections::HashMap;
use tera::{Filter, Result, Value};

/// Filter to convert value to JSON string.
pub fn to_json_filter(value: &Value, _args: &HashMap<String, Value>) -> Result<Value> {
    let json = serde_json::to_string(value)
        .map_err(|e| format!("Failed to serialize to JSON: {}", e))?;
    Ok(Value::String(json))
}

/// Filter to convert value to pretty-printed JSON string.
pub fn to_json_pretty_filter(value: &Value, _args: &HashMap<String, Value>) -> Result<Value> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Failed to serialize to JSON: {}", e))?;
    Ok(Value::String(json))
}

/// Filter to parse JSON string to value.
pub fn from_json_filter(value: &Value, _args: &HashMap<String, Value>) -> Result<Value> {
    let json_str = value
        .as_str()
        .ok_or("from_json filter expects a string")?;

    let parsed: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;

    // Convert serde_json::Value to tera::Value
    Ok(serde_json::from_value(parsed).unwrap_or(Value::Null))
}
```

### Paso 8: Registrar Funciones en TemplateEngine

```rust
// src/templating/engine.rs (actualizado)
use super::functions::{
    config::FunctionConfig,
    env::{make_env_function, make_required_env_function},
    secrets::make_secret_function,
    encoding::*,
    hashing::*,
    datetime::*,
    generators::*,
};
use super::filters::json::*;

impl TemplateEngine {
    /// Creates a new template engine with all functions.
    pub fn new() -> Self {
        Self::with_config(FunctionConfig::default())
    }

    /// Creates a template engine with custom configuration.
    pub fn with_config(config: FunctionConfig) -> Self {
        let mut tera = Tera::default();

        // Disable auto-escape for config files
        tera.autoescape_on(vec![]);

        // Register functions
        Self::register_functions(&mut tera, config);

        // Register filters
        Self::register_filters(&mut tera);

        Self {
            tera: Arc::new(RwLock::new(tera)),
        }
    }

    fn register_functions(tera: &mut Tera, config: FunctionConfig) {
        // Environment functions
        if config.allow_env {
            tera.register_function("env", make_env_function(config.clone()));
            tera.register_function("required_env", make_required_env_function(config.clone()));
        }

        // Secret function
        if config.allow_secrets {
            tera.register_function("secret", make_secret_function(config.clone()));
        }

        // Encoding functions (always available)
        tera.register_function("base64_encode", make_base64_encode_function());
        tera.register_function("base64_decode", make_base64_decode_function());
        tera.register_function("urlencode", make_urlencode_function());
        tera.register_function("urldecode", make_urldecode_function());

        // Hashing functions
        tera.register_function("sha256", make_sha256_function());
        tera.register_function("hmac_sha256", make_hmac_sha256_function());

        // DateTime functions
        if config.allow_now {
            tera.register_function("now", make_now_function());
            tera.register_function("timestamp", make_timestamp_function());
        }

        // Generator functions
        if config.allow_uuid {
            tera.register_function("uuid", make_uuid_function());
            tera.register_function("random_string", make_random_string_function());
        }
    }

    fn register_filters(tera: &mut Tera) {
        tera.register_filter("to_json", to_json_filter);
        tera.register_filter("to_json_pretty", to_json_pretty_filter);
        tera.register_filter("from_json", from_json_filter);
    }
}
```

---

## Conceptos de Rust Aprendidos

### 1. Closures como Funciones de Tera

Tera usa closures que implementan el trait `Function` para funciones custom.

**Rust:**
```rust
use tera::{Function, Result, Value};
use std::collections::HashMap;

// Tera Function trait requiere Fn (closure inmutable)
pub fn make_env_function() -> impl Function {
    // Box para trait object
    Box::new(|args: &HashMap<String, Value>| -> Result<Value> {
        // Extraer argumentos posicionales
        let name = args
            .get("__tera_positional_0")
            .and_then(|v| v.as_str())
            .ok_or("Missing argument")?;

        Ok(Value::String(std::env::var(name).unwrap_or_default()))
    })
}

// Con estado capturado
pub fn make_env_function_with_config(config: FunctionConfig) -> impl Function {
    // config es capturado por el closure (move)
    Box::new(move |args: &HashMap<String, Value>| -> Result<Value> {
        if !config.allow_env {  // Accede a config capturado
            return Err("Not allowed".into());
        }
        // ...
    })
}
```

**Comparacion con Java (Thymeleaf):**
```java
// Thymeleaf custom dialect
public class EnvDialect extends AbstractDialect implements IExpressionObjectDialect {
    @Override
    public IExpressionObjectFactory getExpressionObjectFactory() {
        return new EnvExpressionFactory();
    }
}

public class EnvExpressionFactory implements IExpressionObjectFactory {
    @Override
    public Object buildObject(IExpressionContext context, String name) {
        return new EnvFunctions();
    }
}

public class EnvFunctions {
    public String env(String name) {
        return System.getenv(name);
    }

    public String env(String name, String defaultValue) {
        return Optional.ofNullable(System.getenv(name)).orElse(defaultValue);
    }
}
```

### 2. Newtype Pattern para Secrets

**Rust:**
```rust
/// Newtype que oculta el valor en logs
#[derive(Clone)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    // Metodo explicito para obtener el valor
    pub fn expose(&self) -> &str {
        &self.0
    }
}

// Debug muestra [REDACTED]
impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecretString([REDACTED])")
    }
}

// Display tambien redacta
impl std::fmt::Display for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED]")
    }
}

// Uso seguro
let secret = SecretString::new("my-password");
println!("{:?}", secret);  // SecretString([REDACTED])

// Solo expose() revela el valor
use_password(secret.expose());
```

**Comparacion con Java:**
```java
// Java: clase wrapper similar
public final class SecretString {
    private final String value;

    public SecretString(String value) {
        this.value = value;
    }

    public String expose() {
        return value;
    }

    @Override
    public String toString() {
        return "[REDACTED]";
    }
}
```

### 3. Trait Objects con Box<dyn>

**Rust:**
```rust
// Tera espera Box<dyn Function>
pub fn make_env_function() -> impl Function {
    // impl Function = el tipo exacto es inferido por el compilador
    Box::new(|args: &HashMap<String, Value>| -> Result<Value> {
        // ...
    })
}

// Alternativamente, retorno explicito
pub fn make_env_function_explicit() -> Box<dyn Function> {
    Box::new(|args: &HashMap<String, Value>| -> Result<Value> {
        // ...
    })
}

// Para closures con estado, necesitamos move
pub fn make_function_with_state(config: Config) -> Box<dyn Function> {
    // move captura config por valor
    Box::new(move |args: &HashMap<String, Value>| -> Result<Value> {
        config.validate()?;  // Usa config capturado
        // ...
    })
}
```

**Comparacion con Java:**
```java
// Java: interfaces funcionales
@FunctionalInterface
public interface TemplateFunction {
    Value apply(Map<String, Value> args) throws TemplateException;
}

// Factory method
public static TemplateFunction makeEnvFunction() {
    return args -> {
        // ...
    };
}

// Con estado (efectivamente final)
public static TemplateFunction makeFunctionWithState(Config config) {
    return args -> {
        config.validate();
        // ...
    };
}
```

### 4. Generic Return Type con impl Trait

**Rust:**
```rust
// impl Function significa "algun tipo que implementa Function"
pub fn make_function() -> impl Function {
    // El compilador infiere el tipo concreto
    Box::new(|args: &HashMap<String, Value>| -> Result<Value> {
        Ok(Value::Null)
    })
}

// Mas flexible que especificar Box<dyn Function>
// porque el caller no necesita saber el tipo exacto

// Comparar con:
pub fn make_function_boxed() -> Box<dyn Function> {
    // Aqui el tipo es Box<dyn Function> explicitamente
    // Dynamic dispatch obligatorio
}

// impl Trait en posicion de retorno permite
// que el compilador optimice (monomorphization)
```

---

## Riesgos y Errores Comunes

### 1. Leaking Secrets en Logs

```rust
// MAL: Secret aparece en logs
let password = secret("db/password")?;
tracing::info!("Using password: {}", password);  // LEAK!

// BIEN: Usar SecretString
let password = SecretString::new(secret("db/password")?);
tracing::info!("Using password: {}", password);  // "[REDACTED]"

// Solo exponer cuando sea necesario
connect_db(password.expose());
```

### 2. env() sin Default en Produccion

```rust
// MAL: Puede ser null/error en runtime
database:
  host: {{ env("DB_HOST") }}

// BIEN: Siempre tener default o usar required_env
database:
  host: {{ env("DB_HOST", "localhost") }}
  # O forzar que exista
  password: {{ required_env("DB_PASSWORD") }}
```

### 3. base64_decode con Input Invalido

```rust
// MAL: Error críptico si input no es base64 valido
let decoded = base64_decode(user_input)?;

// BIEN: Validar y dar mensaje claro
let decoded_bytes = BASE64
    .decode(input)
    .map_err(|e| format!(
        "Invalid base64 input '{}': {}",
        &input[..input.len().min(20)],  // Solo primeros 20 chars
        e
    ))?;
```

### 4. Funciones No Deterministicas

```rust
// CUIDADO: uuid() y now() dan valores diferentes cada vez
config:
  id: {{ uuid() }}         # Diferente cada render!
  generated: {{ now() }}   # Diferente cada render!

// Esto puede causar problemas si se espera idempotencia
// Considerar pasar valores pre-generados en context

let context = TemplateContext::new("app", "prod")
    .with_variable("config_id", Uuid::new_v4().to_string())
    .with_variable("generated_at", Utc::now().to_rfc3339());
```

---

## Pruebas

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_env_function() {
        env::set_var("TEST_VAR", "test_value");

        let engine = TemplateEngine::new();
        let context = TemplateContext::default();

        let result = engine.render(r#"{{ env("TEST_VAR") }}"#, &context).unwrap();
        assert_eq!(result, "test_value");

        env::remove_var("TEST_VAR");
    }

    #[test]
    fn test_env_with_default() {
        let engine = TemplateEngine::new();
        let context = TemplateContext::default();

        let result = engine.render(
            r#"{{ env("NONEXISTENT_VAR", "default_value") }}"#,
            &context
        ).unwrap();

        assert_eq!(result, "default_value");
    }

    #[test]
    fn test_secret_function_with_mock() {
        let config = FunctionConfig::development();
        let engine = TemplateEngine::with_config(config);
        let context = TemplateContext::default();

        let result = engine.render(
            r#"{{ secret("database/credentials", "password") }}"#,
            &context
        ).unwrap();

        assert_eq!(result, "dev-password");
    }

    #[test]
    fn test_base64_encode() {
        let engine = TemplateEngine::new();
        let context = TemplateContext::default();

        let result = engine.render(
            r#"{{ base64_encode("hello world") }}"#,
            &context
        ).unwrap();

        assert_eq!(result, "aGVsbG8gd29ybGQ=");
    }

    #[test]
    fn test_base64_decode() {
        let engine = TemplateEngine::new();
        let context = TemplateContext::default();

        let result = engine.render(
            r#"{{ base64_decode("aGVsbG8gd29ybGQ=") }}"#,
            &context
        ).unwrap();

        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_urlencode() {
        let engine = TemplateEngine::new();
        let context = TemplateContext::default();

        let result = engine.render(
            r#"{{ urlencode("hello world&foo=bar") }}"#,
            &context
        ).unwrap();

        assert_eq!(result, "hello%20world%26foo%3Dbar");
    }

    #[test]
    fn test_sha256() {
        let engine = TemplateEngine::new();
        let context = TemplateContext::default();

        let result = engine.render(
            r#"{{ sha256("hello") }}"#,
            &context
        ).unwrap();

        // SHA256 of "hello"
        assert_eq!(
            result,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_now_function() {
        let engine = TemplateEngine::new();
        let context = TemplateContext::default();

        let result = engine.render(r#"{{ now() }}"#, &context).unwrap();

        // Should be ISO format
        assert!(result.contains("T"));
        assert!(result.contains("Z") || result.contains("+"));
    }

    #[test]
    fn test_uuid_function() {
        let engine = TemplateEngine::new();
        let context = TemplateContext::default();

        let result = engine.render(r#"{{ uuid() }}"#, &context).unwrap();

        // UUID v4 format
        assert_eq!(result.len(), 36);
        assert!(result.contains("-"));
    }

    #[test]
    fn test_to_json_filter() {
        let engine = TemplateEngine::new();
        let mut context = TemplateContext::default();
        context.variables.insert(
            "data".to_string(),
            serde_json::json!({"key": "value"})
        );

        let result = engine.render(
            r#"{{ data | to_json }}"#,
            &context
        ).unwrap();

        assert_eq!(result, r#"{"key":"value"}"#);
    }

    #[test]
    fn test_restricted_config_blocks_env() {
        let config = FunctionConfig::restricted();
        let engine = TemplateEngine::with_config(config);
        let context = TemplateContext::default();

        let result = engine.render(r#"{{ env("PATH") }}"#, &context);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not allowed"));
    }

    #[test]
    fn test_env_prefix_restriction() {
        let config = FunctionConfig {
            allow_env: true,
            env_prefixes: vec!["APP_".to_string(), "VORTEX_".to_string()],
            ..Default::default()
        };
        let engine = TemplateEngine::with_config(config);
        let context = TemplateContext::default();

        // PATH not in allowed prefixes
        let result = engine.render(r#"{{ env("PATH") }}"#, &context);
        assert!(result.is_err());

        // Set and test allowed prefix
        env::set_var("APP_NAME", "test");
        let result = engine.render(r#"{{ env("APP_NAME") }}"#, &context);
        assert!(result.is_ok());
        env::remove_var("APP_NAME");
    }
}
```

---

## Seguridad

### Consideraciones

1. **env() exposure**: Limitar variables accesibles con prefijos
2. **secret() mock**: No usar mocks en produccion
3. **Input validation**: Validar inputs a funciones
4. **Resource limits**: Limitar longitud de random_string

```rust
// Configuracion segura para produccion
let config = FunctionConfig {
    allow_env: true,
    env_prefixes: vec![
        "VORTEX_".to_string(),
        "APP_".to_string(),
    ],
    allow_secrets: true,
    mock_secrets: HashMap::new(),  // Sin mocks
    secret_backend_url: Some("https://vault.internal:8200".to_string()),
    allow_uuid: true,
    allow_now: true,
};
```

---

## Entregable Final

### Archivos Creados

1. `src/templating/functions/mod.rs` - Module exports
2. `src/templating/functions/config.rs` - FunctionConfig
3. `src/templating/functions/env.rs` - Environment functions
4. `src/templating/functions/secrets.rs` - Secret functions
5. `src/templating/functions/encoding.rs` - Encoding functions
6. `src/templating/functions/hashing.rs` - Hashing functions
7. `src/templating/functions/datetime.rs` - DateTime functions
8. `src/templating/functions/generators.rs` - Generator functions
9. `src/templating/filters/json.rs` - JSON filters
10. `tests/functions_test.rs` - Tests

### Verificacion

```bash
cargo build -p vortex-templating
cargo test -p vortex-templating functions
cargo clippy -p vortex-templating -- -D warnings
```

### Ejemplo de Uso

```rust
use vortex_templating::{TemplateEngine, TemplateContext, FunctionConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Development config with mock secrets
    let config = FunctionConfig::development();
    let engine = TemplateEngine::with_config(config);

    let context = TemplateContext::new("payment-service", "production")
        .with_variable("region", "us-east-1");

    let template = r#"
spring:
  datasource:
    url: jdbc:postgresql://{{ env("DB_HOST", "localhost") }}:5432/payments
    username: {{ env("DB_USER", "postgres") }}
    password: {{ secret("database/credentials", "password") }}

security:
  api-key: {{ base64_encode(env("API_KEY", "dev-key")) }}
  signature: {{ hmac_sha256(app, env("SIGNING_KEY", "secret")) }}

metadata:
  build-id: {{ uuid() }}
  generated-at: {{ now() }}
  checksum: {{ sha256(app ~ environment ~ region) }}

endpoints:
  callback: {{ urlencode("https://api.example.com/callback?app=" ~ app) }}
"#;

    let rendered = engine.render(template, &context)?;
    println!("{}", rendered);

    Ok(())
}
```

---

**Anterior**: [Historia 004 - Configuration Templating](./story-004-templating.md)
**Siguiente**: [Historia 006 - Compliance Rules Engine](./story-006-compliance-engine.md)
