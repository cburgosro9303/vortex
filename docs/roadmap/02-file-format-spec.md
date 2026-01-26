# Vortex File Format Specification v1

## Principios de Diseño

1. **JSON estándar** — Compatible con cualquier herramienta
2. **Campos ordenados alfabéticamente** — Diffs determinísticos
3. **Mínimo viable** — Solo campos necesarios
4. **IDs estables** — UUIDs para referencias cruzadas
5. **Versionado de schema** — Migraciones controladas
6. **Human-readable** — Indentación de 2 espacios

---

## Estructura de Directorios

```
my-api-project/
├── vortex.json              # Manifest del workspace
├── collections/
│   ├── users-api/
│   │   ├── collection.json  # Metadata de colección
│   │   └── requests/
│   │       ├── get-users.json
│   │       ├── create-user.json
│   │       └── auth/           # Carpeta/folder
│   │           ├── folder.json
│   │           ├── login.json
│   │           └── logout.json
│   └── payments-api/
│       └── ...
├── environments/
│   ├── development.json
│   ├── staging.json
│   └── production.json
├── globals.json             # Variables globales
└── .vortex/
    ├── secrets.json         # Secretos locales (gitignored)
    └── state.json           # Estado de UI (gitignored)
```

---

## Workspace Manifest (vortex.json)

```json
{
  "name": "My API Project",
  "schema_version": 1,
  "default_environment": "development",
  "collections": [
    "collections/users-api",
    "collections/payments-api"
  ],
  "settings": {
    "timeout_ms": 30000,
    "follow_redirects": true,
    "max_redirects": 10,
    "verify_ssl": true
  }
}
```

### Campos

| Campo | Tipo | Requerido | Descripción |
|-------|------|-----------|-------------|
| `name` | string | sí | Nombre del workspace |
| `schema_version` | integer | sí | Versión del schema (1) |
| `default_environment` | string | no | Environment por defecto |
| `collections` | string[] | sí | Paths a colecciones |
| `settings` | object | no | Configuración global |

---

## Collection (collection.json)

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "Users API",
  "schema_version": 1,
  "description": "API for user management",
  "auth": null,
  "variables": {
    "base_path": "/api/v1"
  }
}
```

### Campos

| Campo | Tipo | Requerido | Descripción |
|-------|------|-----------|-------------|
| `id` | uuid | sí | Identificador único |
| `name` | string | sí | Nombre de colección |
| `schema_version` | integer | sí | Versión del schema |
| `description` | string | no | Descripción |
| `auth` | Auth | no | Auth heredable |
| `variables` | object | no | Variables de colección |

---

## Request (*.json en requests/)

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440001",
  "name": "Get Users",
  "schema_version": 1,
  "method": "GET",
  "url": "{{base_url}}{{base_path}}/users",
  "headers": {
    "Accept": "application/json",
    "X-Request-ID": "{{$uuid}}"
  },
  "query_params": {
    "page": "1",
    "limit": "{{page_size}}"
  },
  "body": null,
  "auth": {
    "type": "bearer",
    "token": "{{access_token}}"
  },
  "settings": {
    "timeout_ms": 5000
  },
  "tests": [
    {
      "name": "Status is 200",
      "type": "status",
      "expected": 200
    }
  ]
}
```

### Campos

| Campo | Tipo | Requerido | Descripción |
|-------|------|-----------|-------------|
| `id` | uuid | sí | Identificador único |
| `name` | string | sí | Nombre del request |
| `schema_version` | integer | sí | Versión del schema |
| `method` | string | sí | HTTP method |
| `url` | string | sí | URL con variables |
| `headers` | object | no | Headers key-value |
| `query_params` | object | no | Query params |
| `body` | Body | no | Request body |
| `auth` | Auth | no | Autenticación |
| `settings` | object | no | Settings específicos |
| `tests` | Test[] | no | Assertions |

### HTTP Methods Soportados
- GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS, TRACE

---

## Body Types

### Raw JSON
```json
{
  "body": {
    "type": "json",
    "content": {
      "name": "{{user_name}}",
      "email": "{{user_email}}"
    }
  }
}
```

### Raw Text
```json
{
  "body": {
    "type": "text",
    "content": "Hello {{name}}"
  }
}
```

### Form URL Encoded
```json
{
  "body": {
    "type": "form_urlencoded",
    "fields": {
      "username": "{{user}}",
      "password": "{{pass}}"
    }
  }
}
```

### Multipart Form Data
```json
{
  "body": {
    "type": "form_data",
    "fields": [
      {
        "name": "file",
        "type": "file",
        "path": "./uploads/document.pdf"
      },
      {
        "name": "description",
        "type": "text",
        "value": "My document"
      }
    ]
  }
}
```

### Binary
```json
{
  "body": {
    "type": "binary",
    "path": "./files/image.png"
  }
}
```

### GraphQL
```json
{
  "body": {
    "type": "graphql",
    "query": "query GetUser($id: ID!) { user(id: $id) { name email } }",
    "variables": {
      "id": "{{user_id}}"
    }
  }
}
```

---

## Auth Types

### None
```json
{
  "auth": null
}
```

### Bearer Token
```json
{
  "auth": {
    "type": "bearer",
    "token": "{{access_token}}"
  }
}
```

### Basic Auth
```json
{
  "auth": {
    "type": "basic",
    "username": "{{user}}",
    "password": "{{pass}}"
  }
}
```

### API Key
```json
{
  "auth": {
    "type": "api_key",
    "key": "X-API-Key",
    "value": "{{api_key}}",
    "location": "header"
  }
}
```

### OAuth2 Client Credentials
```json
{
  "auth": {
    "type": "oauth2_client_credentials",
    "token_url": "{{auth_server}}/oauth/token",
    "client_id": "{{client_id}}",
    "client_secret": "{{client_secret}}",
    "scope": "read write"
  }
}
```

### OAuth2 Authorization Code
```json
{
  "auth": {
    "type": "oauth2_auth_code",
    "auth_url": "{{auth_server}}/oauth/authorize",
    "token_url": "{{auth_server}}/oauth/token",
    "client_id": "{{client_id}}",
    "client_secret": "{{client_secret}}",
    "redirect_uri": "http://localhost:9876/callback",
    "scope": "read write"
  }
}
```

---

## Environment (*.json en environments/)

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440010",
  "name": "Development",
  "schema_version": 1,
  "variables": {
    "base_url": {
      "value": "http://localhost:3000",
      "secret": false
    },
    "api_key": {
      "value": "",
      "secret": true
    }
  }
}
```

### Campos

| Campo | Tipo | Requerido | Descripción |
|-------|------|-----------|-------------|
| `id` | uuid | sí | Identificador único |
| `name` | string | sí | Nombre del environment |
| `schema_version` | integer | sí | Versión del schema |
| `variables` | object | sí | Variables con metadata |

### Variable Object

| Campo | Tipo | Requerido | Descripción |
|-------|------|-----------|-------------|
| `value` | string | sí | Valor de la variable |
| `secret` | boolean | no | Si es secreto (default: false) |

---

## Globals (globals.json)

```json
{
  "schema_version": 1,
  "variables": {
    "app_name": {
      "value": "My App",
      "secret": false
    }
  }
}
```

---

## Secrets (.vortex/secrets.json) — GITIGNORED

```json
{
  "schema_version": 1,
  "secrets": {
    "development": {
      "api_key": "sk-dev-xxx",
      "client_secret": "secret-dev-xxx"
    },
    "production": {
      "api_key": "sk-prod-xxx",
      "client_secret": "secret-prod-xxx"
    }
  }
}
```

**Nota:** Este archivo nunca se versiona. Los valores de variables marcadas como `secret: true` se buscan aquí primero.

---

## Folder (folder.json)

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440020",
  "name": "Authentication",
  "schema_version": 1,
  "description": "Auth-related endpoints",
  "auth": null,
  "order": ["login.json", "logout.json", "refresh.json"]
}
```

---

## Test Assertions

### Status Code
```json
{
  "name": "Status is 200",
  "type": "status",
  "expected": 200
}
```

### Status Range
```json
{
  "name": "Status is 2xx",
  "type": "status_range",
  "min": 200,
  "max": 299
}
```

### Header Exists
```json
{
  "name": "Has Content-Type",
  "type": "header_exists",
  "header": "Content-Type"
}
```

### Header Equals
```json
{
  "name": "Content-Type is JSON",
  "type": "header_equals",
  "header": "Content-Type",
  "expected": "application/json"
}
```

### Body Contains
```json
{
  "name": "Body contains success",
  "type": "body_contains",
  "expected": "success"
}
```

### JSON Path Equals
```json
{
  "name": "User name is correct",
  "type": "json_path_equals",
  "path": "$.data.user.name",
  "expected": "John"
}
```

### JSON Path Exists
```json
{
  "name": "Has user ID",
  "type": "json_path_exists",
  "path": "$.data.user.id"
}
```

### Response Time
```json
{
  "name": "Response under 500ms",
  "type": "response_time",
  "max_ms": 500
}
```

---

## Built-in Variables

| Variable | Descripción | Ejemplo |
|----------|-------------|---------|
| `{{$uuid}}` | UUID v4 aleatorio | `550e8400-e29b-41d4-a716-446655440000` |
| `{{$timestamp}}` | Unix timestamp | `1706284800` |
| `{{$isoTimestamp}}` | ISO 8601 timestamp | `2024-01-26T12:00:00Z` |
| `{{$randomInt}}` | Entero aleatorio 0-1000 | `427` |
| `{{$randomString}}` | String alfanumérico 16 chars | `a1b2c3d4e5f6g7h8` |

---

## Resolución de Variables (Precedencia)

1. **Built-in** (`$uuid`, `$timestamp`, etc.)
2. **Secrets** (`.vortex/secrets.json`)
3. **Environment** (`environments/*.json`)
4. **Collection** (`collection.json` variables)
5. **Global** (`globals.json`)

---

## Validación de Schema

Cada archivo incluye `schema_version` para permitir migraciones.

### Reglas de Migración
- Schema v1 → v2: Se añade campo `migrated_from: 1`
- Campos desconocidos se preservan con prefijo `_unknown_`
- Campos removidos se mueven a `_deprecated_`

---

## Serialización Determinística

Para asegurar diffs limpios:

1. **Campos ordenados alfabéticamente**
2. **Indentación: 2 espacios**
3. **Sin trailing whitespace**
4. **Newline final**
5. **UTF-8 sin BOM**

### Ejemplo de serialización Rust

```rust
use serde::Serialize;
use serde_json::ser::{PrettyFormatter, Serializer};

fn serialize_stable<T: Serialize>(value: &T) -> String {
    let mut buf = Vec::new();
    let formatter = PrettyFormatter::with_indent(b"  ");
    let mut ser = Serializer::with_formatter(&mut buf, formatter);
    value.serialize(&mut ser).unwrap();
    let mut s = String::from_utf8(buf).unwrap();
    s.push('\n'); // trailing newline
    s
}
```

---

## Ejemplo Completo de Request

**Archivo:** `collections/users-api/requests/create-user.json`

```json
{
  "auth": {
    "token": "{{access_token}}",
    "type": "bearer"
  },
  "body": {
    "content": {
      "email": "{{user_email}}",
      "name": "{{user_name}}",
      "role": "user"
    },
    "type": "json"
  },
  "headers": {
    "Accept": "application/json",
    "Content-Type": "application/json",
    "X-Request-ID": "{{$uuid}}"
  },
  "id": "550e8400-e29b-41d4-a716-446655440002",
  "method": "POST",
  "name": "Create User",
  "query_params": {},
  "schema_version": 1,
  "settings": {
    "timeout_ms": 10000
  },
  "tests": [
    {
      "expected": 201,
      "name": "Status is 201 Created",
      "type": "status"
    },
    {
      "name": "Has user ID in response",
      "path": "$.data.id",
      "type": "json_path_exists"
    }
  ],
  "url": "{{base_url}}/api/v1/users"
}
```

Nota: campos ordenados alfabéticamente para diffs determinísticos.
