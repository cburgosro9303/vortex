# Vortex UI/UX Specification

## Design Principles

1. **Keyboard-First** — Todo accesible sin mouse
2. **Information Density** — Mostrar lo necesario, ocultar lo opcional
3. **Consistent** — Mismos patrones en toda la app
4. **Fast** — UI nunca bloquea, feedback inmediato
5. **Accessible** — WCAG 2.1 AA compliance

---

## Layout Principal

```
┌─────────────────────────────────────────────────────────────────────────┐
│  [≡] Vortex    │ Collection ▼ │ Environment ▼ │        [⚙] [?] [—][□][×] │
├────────────────┼───────────────────────────────────────────────────────┤
│                │ [+ New] [Tab 1: Get Users ×] [Tab 2: Create User ×]   │
│  COLLECTIONS   ├───────────────────────────────────────────────────────┤
│                │ ┌─────────────────────────────────────────────────┐   │
│  ▼ Users API   │ │ [GET ▼] [{{base_url}}/api/users        ] [Send] │   │
│    • Get Users │ └─────────────────────────────────────────────────┘   │
│    • Create    │                                                       │
│    ▶ Auth      │ [Params] [Headers] [Body] [Auth] [Tests] [Settings]   │
│                │ ┌─────────────────────────────────────────────────┐   │
│  ▶ Payments    │ │ Key              │ Value                        │   │
│                │ │──────────────────┼──────────────────────────────│   │
│  ENVIRONMENTS  │ │ page             │ 1                            │   │
│  ○ Development │ │ limit            │ {{page_size}}                │   │
│  ● Staging     │ │ [+ Add param]                                   │   │
│  ○ Production  │ └─────────────────────────────────────────────────┘   │
│                ├───────────────────────────────────────────────────────┤
│  HISTORY       │ Response    [Body] [Headers] [Tests] [Meta]           │
│  • GET /users  │ ┌─────────────────────────────────────────────────┐   │
│  • POST /login │ │ Status: 200 OK    Time: 124ms    Size: 1.2 KB   │   │
│                │ ├─────────────────────────────────────────────────┤   │
│ [Import] [+]   │ │ {                                               │   │
│                │ │   "data": [                                     │   │
│                │ │     { "id": 1, "name": "John" },                │   │
│                │ │     { "id": 2, "name": "Jane" }                 │   │
│                │ │   ],                                            │   │
│                │ │   "total": 42                                   │   │
│                │ │ }                                               │   │
│                │ │ [Copy] [Save] [Format] [Wrap]                   │   │
│                │ └─────────────────────────────────────────────────┘   │
└────────────────┴───────────────────────────────────────────────────────┘
```

---

## Componentes de UI

### 1. Sidebar (250px width, resizable)

#### Collections Tree
```
▼ Users API                    ← Click to expand/collapse
  ├─ • Get Users              ← Request item
  ├─ • Create User
  └─ ▶ Auth                   ← Folder (collapsed)
       ├─ • Login
       └─ • Logout
```

**Estados visuales:**
- Normal: texto blanco/negro
- Hover: background sutil
- Selected: background accent, bold
- Modified (unsaved): bullet naranja
- Error: bullet rojo

#### Environments Section
```
ENVIRONMENTS
  ○ Development               ← Radio button, inactive
  ● Staging                   ← Radio button, active
  ○ Production
```

#### History Section
```
HISTORY
  12:34 GET /api/users → 200    ← Timestamp, method, path, status
  12:32 POST /api/login → 401   ← Error status in red
  12:30 GET /api/health → 200
```

---

### 2. Request Editor (Panel Central)

#### URL Bar
```
┌───────┬──────────────────────────────────────────────┬────────┐
│ GET ▼ │ {{base_url}}/api/users?page={{page}}         │ [Send] │
└───────┴──────────────────────────────────────────────┴────────┘
```

- Method dropdown: GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS
- URL input: monospace, syntax highlight para `{{variables}}`
- Send button: primary action, shortcut `Ctrl+Enter`

#### Tabs de Configuración
```
[Params] [Headers] [Body] [Auth] [Tests] [Settings]
   ↑                  ↑
 Badge (2)        Badge "JSON"
```

- Badge numérico cuando hay items configurados
- Badge de tipo para Body (JSON, Form, Text, etc.)

#### Params Tab
```
┌──────────────────┬────────────────────────────┬───────┬───┐
│ Key              │ Value                      │ Desc  │ × │
├──────────────────┼────────────────────────────┼───────┼───┤
│ ☑ page           │ 1                          │       │ × │
│ ☑ limit          │ {{page_size}}              │       │ × │
│ ☐ filter         │ active                     │       │ × │
└──────────────────┴────────────────────────────┴───────┴───┘
[+ Add parameter]
```

- Checkbox para enable/disable sin borrar
- Highlight de variables `{{...}}`
- Botón × para eliminar
- Campo descripción opcional (collapsed por default)

#### Headers Tab
```
┌──────────────────┬────────────────────────────┬───┐
│ Key              │ Value                      │ × │
├──────────────────┼────────────────────────────┼───┤
│ ☑ Accept         │ application/json           │ × │
│ ☑ Authorization  │ Bearer {{token}}           │ × │
│ ☑ X-Request-ID   │ {{$uuid}}                  │ × │
└──────────────────┴────────────────────────────┴───┘
[+ Add header]  [Presets ▼]
```

- Presets: JSON, Form, XML, etc. (añade headers comunes)
- Autocomplete para headers conocidos

#### Body Tab
```
[none] [json] [text] [form-urlencoded] [form-data] [binary] [graphql]
       ↑ selected

┌─────────────────────────────────────────────────────────────────┐
│ {                                                              1│
│   "name": "{{user_name}}",                                     2│
│   "email": "{{user_email}}",                                   3│
│   "role": "user"                                               4│
│ }                                                              5│
│                                                                 │
│ [Format] [Collapse]                                             │
└─────────────────────────────────────────────────────────────────┘
```

- Editor con syntax highlighting
- Line numbers
- Variable highlighting
- Format button para JSON
- Validación inline de JSON

#### Auth Tab
```
Type: [No Auth ▼]
      ├─ No Auth
      ├─ Bearer Token
      ├─ Basic Auth
      ├─ API Key
      ├─ OAuth 2.0 (Client Credentials)
      └─ OAuth 2.0 (Authorization Code)

─────────────────────────────────────────
Bearer Token selected:

Token: [{{access_token}}                 ]
       Variables allowed

Prefix: [Bearer                          ]
        Default: "Bearer"
```

#### Tests Tab
```
┌───────────────────────────────────────────────────────────────┐
│ ✓ Status is 200          [status = 200]                     × │
│ ✓ Response time < 500ms  [response_time < 500]              × │
│ ○ Has user ID            [json_path $.data.id exists]       × │
└───────────────────────────────────────────────────────────────┘
[+ Add test]

Test Builder:
┌────────────────────────────────────────────────────────────────┐
│ Test type: [Status Code ▼]                                     │
│                                                                │
│ Expected: [200                    ]                            │
│                                                                │
│ Name: [Status is 200             ]    [Add Test]               │
└────────────────────────────────────────────────────────────────┘
```

---

### 3. Response Panel (Panel Inferior)

#### Response Header
```
┌─────────────────────────────────────────────────────────────────┐
│ ● 200 OK          Time: 124ms          Size: 1.2 KB            │
└─────────────────────────────────────────────────────────────────┘
  ↑ Green dot       Timing                Size
```

**Status Colors:**
- 2xx: Verde
- 3xx: Azul
- 4xx: Naranja
- 5xx: Rojo

#### Response Tabs
```
[Body] [Headers (8)] [Tests (2/3)] [Cookies] [Meta]
        ↑ Badge        ↑ Pass/Total
```

#### Body View
```
┌─────────────────────────────────────────────────────────────────┐
│ View: [Pretty ▼] [JSON ▼]   Search: [          ] [↑] [↓]       │
│       ├─ Pretty                                                 │
│       ├─ Raw                                                    │
│       └─ Preview (HTML)                                         │
├─────────────────────────────────────────────────────────────────┤
│ {                                                              │
│   "data": [                                                    │
│     { "id": 1, "name": "John", "email": "john@example.com" },  │
│     { "id": 2, "name": "Jane", "email": "jane@example.com" }   │
│   ],                                                           │
│   "pagination": {                                              │
│     "total": 42,                                               │
│     "page": 1,                                                 │
│     "per_page": 10                                             │
│   }                                                            │
│ }                                                              │
├─────────────────────────────────────────────────────────────────┤
│ [Copy] [Copy Path] [Save to File] [Word Wrap]                  │
└─────────────────────────────────────────────────────────────────┘
```

#### Tests Results
```
┌─────────────────────────────────────────────────────────────────┐
│ ✓ Status is 200              PASSED           0ms              │
│ ✓ Response time < 500ms      PASSED           -                │
│ ✗ Has field "admin"          FAILED                            │
│   └─ Expected: $.admin exists                                  │
│      Actual: path not found                                    │
└─────────────────────────────────────────────────────────────────┘
  Pass: 2 │ Fail: 1 │ Total: 3
```

---

### 4. Estados de UI

#### Loading State
```
┌─────────────────────────────────────────────────────────────────┐
│ [=======>                    ] Sending request...              │
│                                                                 │
│ [Cancel]                                                       │
└─────────────────────────────────────────────────────────────────┘
```

#### Error State
```
┌─────────────────────────────────────────────────────────────────┐
│ ⚠ Connection Error                                             │
│                                                                 │
│ Could not connect to server: Connection refused                │
│                                                                 │
│ Suggestions:                                                   │
│ • Check if the server is running                               │
│ • Verify the URL is correct                                    │
│ • Check your network connection                                │
│                                                                 │
│ [Retry] [Copy Error]                                           │
└─────────────────────────────────────────────────────────────────┘
```

#### Empty State (No Request Selected)
```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│                         [Icon: Request]                        │
│                                                                 │
│                    Select or create a request                  │
│                                                                 │
│              [+ New Request]    [Import Collection]            │
│                                                                 │
│                        Ctrl+N to create new                    │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Keyboard Shortcuts

### Global
| Shortcut | Action |
|----------|--------|
| `Ctrl+N` | New request |
| `Ctrl+O` | Open collection |
| `Ctrl+S` | Save current request |
| `Ctrl+Shift+S` | Save all |
| `Ctrl+W` | Close current tab |
| `Ctrl+Tab` | Next tab |
| `Ctrl+Shift+Tab` | Previous tab |
| `Ctrl+1-9` | Go to tab N |
| `Ctrl+,` | Settings |
| `Ctrl+P` | Command palette |
| `Ctrl+Shift+P` | Search in collection |
| `F11` | Toggle fullscreen |
| `Escape` | Cancel request / Close dialog |

### Request Editor
| Shortcut | Action |
|----------|--------|
| `Ctrl+Enter` | Send request |
| `Ctrl+D` | Duplicate request |
| `Ctrl+E` | Edit request name |
| `Ctrl+Shift+C` | Copy as cURL |
| `Alt+1` | Params tab |
| `Alt+2` | Headers tab |
| `Alt+3` | Body tab |
| `Alt+4` | Auth tab |
| `Alt+5` | Tests tab |

### Response Panel
| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+B` | Copy response body |
| `Ctrl+Shift+H` | Copy response headers |
| `Ctrl+F` | Search in response |
| `Ctrl+G` | Go to line |

### Sidebar
| Shortcut | Action |
|----------|--------|
| `Ctrl+B` | Toggle sidebar |
| `↑/↓` | Navigate items |
| `Enter` | Open selected |
| `F2` | Rename |
| `Delete` | Delete (with confirmation) |

---

## Color Palette (Dark Theme)

```
Background:
  --bg-primary:    #1e1e1e    Base background
  --bg-secondary:  #252526    Panels, sidebars
  --bg-tertiary:   #2d2d2d    Inputs, hover states
  --bg-accent:     #094771    Selected items

Text:
  --text-primary:   #cccccc   Main text
  --text-secondary: #858585   Subtle text
  --text-accent:    #4fc1ff   Links, variables

Status:
  --status-success: #4ec9b0   2xx, pass
  --status-info:    #569cd6   3xx
  --status-warning: #ce9178   4xx
  --status-error:   #f14c4c   5xx, fail

Syntax (JSON):
  --syntax-key:     #9cdcfe   Object keys
  --syntax-string:  #ce9178   String values
  --syntax-number:  #b5cea8   Numbers
  --syntax-boolean: #569cd6   true/false/null
  --syntax-variable:#dcdcaa   {{variables}}

Methods:
  --method-get:     #61affe   GET
  --method-post:    #49cc90   POST
  --method-put:     #fca130   PUT
  --method-patch:   #50e3c2   PATCH
  --method-delete:  #f93e3e   DELETE
  --method-head:    #9012fe   HEAD
  --method-options: #0d5aa7   OPTIONS
```

---

## Color Palette (Light Theme)

```
Background:
  --bg-primary:    #ffffff
  --bg-secondary:  #f3f3f3
  --bg-tertiary:   #e8e8e8
  --bg-accent:     #cce5ff

Text:
  --text-primary:   #333333
  --text-secondary: #666666
  --text-accent:    #0066cc

(Status y Methods mantienen colores similares)
```

---

## Typography

```
Font Stack:
  UI:   "Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif
  Code: "JetBrains Mono", "Fira Code", "Consolas", monospace

Sizes:
  --font-xs:    11px   Badges, hints
  --font-sm:    12px   Secondary text
  --font-base:  13px   Body text
  --font-lg:    14px   Headers
  --font-xl:    16px   Titles

Weights:
  --font-normal:  400
  --font-medium:  500
  --font-bold:    600
```

---

## Responsive Behavior

### Minimum Window Size
- Width: 800px
- Height: 600px

### Panel Resizing
```
Sidebar:    150px - 400px (default 250px)
Response:   100px - 70% of height (default 40%)
```

### Collapse Behavior
- < 1000px: Auto-collapse sidebar
- < 900px: Stack response below (no split view option)

---

## Animations

### Durations
```
--duration-fast:    100ms   Hover states, toggles
--duration-normal:  200ms   Panels, tabs
--duration-slow:    300ms   Modals, overlays
```

### Transitions
```css
/* Button hover */
transition: background-color var(--duration-fast) ease;

/* Panel resize */
transition: width var(--duration-normal) ease-out;

/* Modal appear */
transition: opacity var(--duration-slow) ease,
            transform var(--duration-slow) ease;
```

---

## Dialogs & Modals

### Import Collection Dialog
```
┌──────────────────────────────────────────────────────────────┐
│ Import Collection                                        [×] │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Source: [Postman v2 ▼]                                      │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │                                                        │  │
│  │         Drag and drop collection file here            │  │
│  │                   or click to browse                   │  │
│  │                                                        │  │
│  │                  Supports: .json                       │  │
│  │                                                        │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  Preview:                                                    │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ Collection: "Users API"                                │  │
│  │ Requests: 12                                           │  │
│  │ Folders: 3                                             │  │
│  │ Variables: 5                                           │  │
│  │ ⚠ 2 warnings (unsupported features)                    │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│                              [Cancel]  [Import Collection]   │
└──────────────────────────────────────────────────────────────┘
```

### Settings Dialog
```
┌──────────────────────────────────────────────────────────────┐
│ Settings                                                 [×] │
├──────────────────────────────────────────────────────────────┤
│ [General] [Editor] [Proxy] [Certificates] [Shortcuts]        │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│ GENERAL                                                      │
│                                                              │
│ Theme              [Dark ▼]                                  │
│ Language           [English ▼]                               │
│ Auto-save          [☑] Enabled                               │
│ Auto-save interval [30 seconds ▼]                            │
│                                                              │
│ ─────────────────────────────────────────────                │
│                                                              │
│ REQUEST DEFAULTS                                             │
│                                                              │
│ Timeout            [30000    ] ms                            │
│ Follow redirects   [☑] Enabled                               │
│ Max redirects      [10       ]                               │
│ Verify SSL         [☑] Enabled                               │
│                                                              │
│                                   [Reset to Defaults] [Save] │
└──────────────────────────────────────────────────────────────┘
```

### Environment Variables Dialog
```
┌──────────────────────────────────────────────────────────────┐
│ Manage Environments                                      [×] │
├──────────────────────────────────────────────────────────────┤
│ [+ New Environment]                                          │
│                                                              │
│ ┌──────────────┐ ┌──────────────────────────────────────────┐│
│ │ Development  │ │ Variable         Value            🔒    ││
│ │ Staging    ● │ │ ───────────────────────────────────────── ││
│ │ Production   │ │ base_url         http://localhost:3000   ││
│ │              │ │ api_key          ••••••••••••••    🔒    ││
│ │              │ │ page_size        20                      ││
│ │              │ │ [+ Add variable]                         ││
│ │              │ │                                          ││
│ │              │ │ 🔒 = Secret (stored locally, not in Git) ││
│ └──────────────┘ └──────────────────────────────────────────┘│
│                                                              │
│                                          [Close]             │
└──────────────────────────────────────────────────────────────┘
```

---

## Slint Component Mapping

```
UI Element          → Slint Component
─────────────────────────────────────
Window              → Window
Sidebar             → VerticalBox + ListView
Request Editor      → VerticalBox + TabWidget
Response Panel      → VerticalBox + TabWidget
URL Bar             → HorizontalBox + ComboBox + LineEdit + Button
Key-Value Table     → ListView with custom delegate
Code Editor         → TextEdit (custom syntax highlighting)
Tabs                → TabWidget
Tree View           → ListView with indent logic
Dropdown            → ComboBox
Button              → Button
Toggle              → Switch
Checkbox            → CheckBox
Dialog              → PopupWindow / Dialog
Toast               → Rectangle with Timer
```

---

## Accessibility Requirements

### WCAG 2.1 AA Compliance

1. **Color Contrast**
   - Text: minimum 4.5:1 ratio
   - Large text: minimum 3:1 ratio
   - UI components: minimum 3:1 ratio

2. **Keyboard Navigation**
   - All interactive elements focusable
   - Visible focus indicators
   - Logical tab order
   - No keyboard traps

3. **Screen Reader Support**
   - ARIA labels on all controls
   - Status announcements for async operations
   - Error messages associated with inputs

4. **Motion**
   - Respect `prefers-reduced-motion`
   - No essential animations
