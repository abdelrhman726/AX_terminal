# AX Terminal — Full Developer Documentation

**Version:** 1.0  
**Platform:** Windows (standalone `.exe`) / Node.js (dev mode)  
**Connected to:** AX-Connect REST API (`localhost:4000`)  
**Last updated:** 2026-04-13

---

## Table of Contents

1. [What AX Terminal Is](#1-what-ax-terminal-is)
2. [Project Structure](#2-project-structure)
3. [Technology Stack](#3-technology-stack)
4. [Development Setup](#4-development-setup)
5. [Environment & Credentials](#5-environment--credentials)
6. [Startup & Authentication Flow](#6-startup--authentication-flow)
7. [Command Reference](#7-command-reference)
8. [Architecture Deep Dive](#8-architecture-deep-dive)
9. [AX-Connect API Integration](#9-ax-connect-api-integration)
10. [Security Model](#10-security-model)
11. [Building the .exe](#11-building-the-exe)
12. [Deploying the .exe](#12-deploying-the-exe)
13. [Adding a New Command](#13-adding-a-new-command)
14. [Color & UI System](#14-color--ui-system)
15. [Error Handling Reference](#15-error-handling-reference)
16. [Roadmap](#16-roadmap)

---

## 1. What AX Terminal Is

AX Terminal is a **standalone Windows CLI** for managing the AX-Connect multi-tenant SaaS platform. It connects to the AX-Connect REST API running locally and gives platform administrators a fast, keyboard-driven interface for operations that would otherwise require hitting API endpoints manually.

**What you can do right now:**

- Log in with your AX-Connect platform credentials
- List all tenants
- Permanently terminate a tenant (with double-confirmation)
- Check system health

**What it is not:** a shell replacement, a web UI, or an AI assistant.

---

## 2. Project Structure

```
ax-terminal/
│
├── cli/
│   ├── index.js          Entry point. Boots the terminal, runs the auth flow,
│   │                     owns the readline REPL loop.
│   └── ui.js             All visual output: logo, colors, prompts, help text.
│
├── core/
│   ├── executor.js       Orchestrates every command execution. Enforces the
│   │                     8-step pipeline (alias → registry → session → perms
│   │                     → validate → confirm → execute).
│   ├── parser.js         Tokenizes raw input into { command, args }.
│   ├── permissions.js    Role-based access check. Throws on unauthorized.
│   └── validation.js     Input validators (tenant ID, slug format).
│
├── commands/
│   ├── platform/
│   │   └── login.js      POST /api/platform/auth/login
│   ├── tenant/
│   │   ├── list.js       GET  /api/platform/restaurants
│   │   └── delete.js     GET + POST .../terminate
│   └── system/
│       └── health.js     GET  /health
│
├── config/
│   ├── commands.js       Command registry — the source of truth for what
│   │                     commands exist, their permissions, and risk level.
│   ├── aliases.js        Short-hand aliases (e.g. "tlist" → "tenant:list").
│   └── env.js            .env file loader + exports config constants.
│
├── services/
│   ├── api.service.js    HTTP client. All AX-Connect API calls live here.
│   └── session.js        In-memory JWT store for the running session.
│
├── dist/
│   └── ax-terminal.exe   Built Windows binary (not committed to git).
│
├── .env.example          Template for the required .env file.
├── package.json
└── DOCS.md               This file.
```

---

## 3. Technology Stack

| Concern | Tool | Why |
|---|---|---|
| Runtime | Node.js ≥ 20 | Available everywhere, no compile step |
| Terminal I/O | `readline` (built-in) | No framework needed |
| Colors | `chalk ^4` | CommonJS-compatible, lightweight |
| Spinners | `ora ^5` | Pairs with chalk 4 |
| HTTP | `http` / `https` (built-in) | Zero dependencies for API calls |
| Packaging | `@yao-pkg/pkg ^6` | Produces a standalone `.exe` for Windows |

**No Commander.js, no Yargs, no Axios, no Express.**

---

## 4. Development Setup

### Prerequisites

- Node.js v20+ (check: `node --version`)
- npm v8+
- AX-Connect server running locally on port 4000

### Clone and install

```bash
cd F:\ax_terminal
npm install
```

### Create your .env file

```bash
copy .env.example .env
```

Then open `.env` and fill in the values (see [Section 5](#5-environment--credentials)).

### Run in dev mode

```bash
npm start
# or
node cli/index.js
```

The terminal starts, shows the logo, and prompts for your platform credentials.

---

## 5. Environment & Credentials

### The .env file

Place `.env` in the **project root** for dev, or **next to `ax-terminal.exe`** for production.

```env
# AX-Connect REST API base URL
AX_API_URL=http://localhost:4000

# Optional: pre-authenticated platform JWT.
# If set, the login prompt is skipped entirely.
# Leave blank to be prompted interactively.
AX_API_TOKEN=
```

### Variables explained

| Variable | Required | Default | Description |
|---|---|---|---|
| `AX_API_URL` | No | `http://localhost:4000` | Full base URL of the AX-Connect server |
| `AX_API_TOKEN` | No | _(empty)_ | A valid platform JWT. Set this to skip the interactive login prompt. Useful for scripts or shared operator setups. |

### How .env is loaded (`config/env.js`)

The loader runs **before any other module** (`cli/index.js` line 3). It searches for `.env` in two locations in order:

1. The directory containing `process.execPath` — this is the folder where `ax-terminal.exe` lives when running as a packaged binary.
2. The project root — used when running with `node cli/index.js` in dev.

The first file found wins. Variables already set in the OS environment are never overwritten.

### Platform credentials (for interactive login)

These are your AX-Connect **platform user** credentials — the same ones used to log in to the platform admin panel.

| Field | Notes |
|---|---|
| Email | Your platform user email (`platform_owner`, `platform_manager`, or `platform_developer` role) |
| Password | Your platform user password |

The terminal calls `POST /api/platform/auth/login` and stores the returned JWT in memory. The JWT lives only for the duration of the terminal session — it is never written to disk.

**Roles that can log in:**

| Role | Access in terminal |
|---|---|
| `platform_owner` | All commands including `tenant:delete` |
| `platform_manager` | All commands including `tenant:delete` |
| `platform_developer` | Read-only commands (`tenant:list`, `system:health`) |

> The terminal's internal role model currently maps all logged-in platform users to `admin`. Per-role mapping from the JWT will be implemented in Phase 2.

---

## 6. Startup & Authentication Flow

```
Double-click ax-terminal.exe
          │
          ▼
  Clear screen + print logo
  Show: API  http://localhost:4000
          │
          ├── AX_API_TOKEN set in .env?
          │         │
          │         ├── YES → session.setToken(token)
          │         │         Print: ✔ Authenticated via AX_API_TOKEN
          │         │
          │         └── NO  → Interactive login prompt
          │                    Email:    ___
          │                    Password: *** (masked)
          │                         │
          │                         ▼
          │                   POST /api/platform/auth/login
          │                         │
          │                    ┌────┴────┐
          │                  200 OK    4xx/5xx
          │                    │         │
          │             store JWT      show error
          │             in memory      retry (max 3)
          │                    │         │
          │             ✔ Logged in   3 fails → exit
          │
          ▼
    REPL loop starts
    ax (admin) > _
```

---

## 7. Command Reference

### Built-in commands (no API call)

| Command | Description |
|---|---|
| `help` or `--help` | List all available commands with roles and risk level |
| `clear` | Clear the screen and reprint the logo |
| `exit` or `quit` | End the session and close the terminal |
| Ctrl+C | Prints a reminder to use `exit`; does not quit |
| ↑ / ↓ arrow keys | Navigate command history (last 100 commands) |

---

### `platform:login`

**Alias:** `login`  
**Roles:** admin, operator  
**Risk:** low  
**API:** `POST /api/platform/auth/login`

Log in with a different account mid-session. Replaces the current JWT.

```
ax (admin) > platform:login --email=admin@example.com --password=secret
```

> The startup flow handles the initial login automatically. This command is for switching accounts without restarting.

---

### `tenant:list`

**Alias:** `tlist`  
**Roles:** admin, operator  
**Risk:** low  
**API:** `GET /api/platform/restaurants`

Fetches and displays all tenants in a table.

```
ax (admin) > tenant:list
→ Processing command...
- Fetching tenants...
✔ Tenant data retrieved

┌────┬───────────────────┬────────────────────┬──────────┬────────────┐
│ ID │ Slug              │ Name               │ Status   │ Type       │
├────┼───────────────────┼────────────────────┼──────────┼────────────┤
│ 1  │ ax-smoke          │ AX Smoke           │ active   │ restaurant │
│ 2  │ example-place     │ Example Place      │ suspended│ cafe       │
└────┴───────────────────┴────────────────────┴──────────┴────────────┘
```

**Status colors:**
- `active` — green
- `suspended` — orange
- `archived` / anything else — red

---

### `tenant:delete`

**Alias:** `tdel`  
**Roles:** admin only  
**Risk:** HIGH — requires confirmation  
**API:** `GET /api/platform/restaurants/:id` then `POST /api/platform/restaurants/:id/terminate`

Permanently terminates a tenant. This is irreversible. All tenant data is removed.

```
ax (admin) > tenant:delete --id=2
→ Processing command...
- Fetching tenant details...

⚠  HIGH RISK ACTION
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Command:    tenant:delete
Tenant ID:  2
Slug:       example-place

This action is IRREVERSIBLE.

Type "DELETE" to confirm: DELETE
- Terminating tenant...
✔ Tenant "example-place" (ID 2) has been terminated

The tenant and all associated data have been permanently removed.
```

**Required argument:**

| Flag | Type | Description |
|---|---|---|
| `--id` | positive integer | The numeric ID of the tenant to delete |

**Confirmation:** You must type the word `DELETE` exactly (case-sensitive). Anything else cancels the operation.

**What happens internally:**
1. Validates `--id` is a positive integer
2. Fetches tenant from `GET /api/platform/restaurants/:id` to retrieve the slug
3. Shows the high-risk confirmation prompt
4. On confirmation, calls `POST /api/platform/restaurants/:id/terminate` with `{ confirmSlug: "<slug>" }` in the body — this is AX-Connect's own security gate

---

### `system:health`

**Alias:** `health`  
**Roles:** admin, operator  
**Risk:** low  
**API:** `GET /health`

```
ax (admin) > system:health
→ Processing command...
- Checking system health...
✔ Health check complete

System Status: ✔ OK
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Uptime:    24h 15m
Database:  connected
Services:
  → API    healthy
  → AUTH   healthy
```

---

## 8. Architecture Deep Dive

### The 8-step execution pipeline (`core/executor.js`)

Every command — without exception — passes through all 8 steps in order:

```
1. Alias resolution
   aliases["tlist"] → "tenant:list"

2. Registry lookup
   commandRegistry["tenant:list"] → { handler, permissions, risk, ... }
   Unknown command → throw error

3. Session check
   session.isAuthenticated() must be true
   Exception: commands with skipAuth: true (platform:login)

4. Permission check
   commandDef.permissions.includes(role) must be true
   admin can run everything
   operator cannot run tenant:delete

5. Handler lookup
   HANDLERS["tenant/list"] → require("../commands/tenant/list.js")
   Static map — required for pkg binary compatibility

6. Argument validation
   handler.validate(args) — command defines its own rules
   Missing --id, invalid format → throw before any API call

7. High-risk confirmation  (only if risk: "high" && confirm: true)
   Prompt for "DELETE" — case-sensitive exact match
   Any other input → cancelled, no API call made

8. Handler execution
   handler.execute(args) → makes API call → returns result
```

### Why a static handler map?

`pkg` (the `.exe` packager) analyzes `require()` calls at bundle time. Dynamic requires like `require(path.join(__dirname, handler))` are resolved at runtime — pkg cannot include those files in the bundle. The static `HANDLERS` map in `executor.js` means all command modules are explicitly referenced and pkg can include them.

**When adding a command, you must update the HANDLERS map.**

### Parser (`core/parser.js`)

Converts raw string input into a structured object.

```
Input:  tenant:delete --id=2 --reason="Migration complete"

Output: {
  command: "tenant:delete",
  args: {
    id: "2",
    reason: "Migration complete"
  }
}
```

**Supported flag formats:**

| Format | Example | Result |
|---|---|---|
| `--key=value` | `--id=2` | `{ id: "2" }` |
| `--key="value with spaces"` | `--reason="done"` | `{ reason: "done" }` |
| `--key='value'` | `--slug='my-slug'` | `{ slug: "my-slug" }` |
| `--flag` (boolean) | `--verbose` | `{ verbose: true }` |

**Security:** argument keys are sanitized with `/^[a-zA-Z0-9-]+$/` — any key containing special characters is silently dropped. This prevents prototype pollution attacks.

### Session store (`services/session.js`)

A plain in-memory object. No disk writes, no cookies.

```javascript
session.setToken(jwt, user)   // called after successful login
session.getToken()            // returns JWT string or null
session.isAuthenticated()     // returns boolean
session.clear()               // wipes token (logout)
```

The JWT is never written to `.env`, disk, or any external store. It exists only for the current process lifetime.

---

## 9. AX-Connect API Integration

### Base URL

Configured via `AX_API_URL` in `.env`. Default: `http://localhost:4000`

### Authentication

All requests (except `POST /api/platform/auth/login`) send:

```
Authorization: Bearer <JWT>
```

The JWT is injected automatically by `api.service.js` from the session store.

### Endpoints used

| Method | Path | Used by |
|---|---|---|
| `POST` | `/api/platform/auth/login` | Startup login, `platform:login` |
| `POST` | `/api/platform/auth/logout` | `session.clear()` (future) |
| `GET` | `/api/platform/restaurants` | `tenant:list` |
| `GET` | `/api/platform/restaurants/:id` | `tenant:delete` (pre-fetch) |
| `POST` | `/api/platform/restaurants/:id/terminate` | `tenant:delete` (actual deletion) |
| `GET` | `/health` | `system:health` |
| `GET` | `/monitor/health/detailed` | Available via `apiService.getDetailedHealth()` |

### Request format

All requests send:
```
Content-Type: application/json
Accept: application/json
```

All request bodies are JSON-serialized automatically.

### Timeout

All requests time out after **10 seconds**. The socket is destroyed and an error is thrown.

### Error handling in `api.service.js`

| HTTP Status | Behavior |
|---|---|
| `200–299` | Parse JSON body and resolve |
| `401` | Reject with "session expired" message |
| `403` | Reject with "access denied" message |
| `400–499` | Reject with `message` or `error` field from response body |
| `500+` | Reject with `HTTP 5xx` |
| `ECONNREFUSED` | Reject with human-readable "cannot reach server" message |

### Tenant response shape

AX-Connect returns tenants as either a plain array `[...]` or `{ restaurants: [...] }`. The list command handles both:

```javascript
const tenants = Array.isArray(raw)
  ? raw
  : (Array.isArray(raw.restaurants) ? raw.restaurants : []);
```

Fields used from each tenant object:

| Field | Used for |
|---|---|
| `id` | Table display, delete confirmation |
| `slug` | Table display, `confirmSlug` in terminate request |
| `name` | Table display |
| `status` | Table display, color coding |
| `isActive` | Fallback if `status` is missing |
| `businessType` | Table display (Type column) |

---

## 10. Security Model

### No shell execution

The terminal never calls `exec()`, `spawn()`, or any shell command with user input. All executable code is loaded from the `commands/` directory via the static `HANDLERS` map in `executor.js`. User input can only select from pre-registered commands — it cannot inject code.

### Argument sanitization

Parser keys are filtered with `/^[a-zA-Z0-9-]+$/`. Values are stripped of surrounding quotes only. No eval, no template injection.

### Role-based access

```
admin    → all commands
operator → all commands except tenant:delete
```

Enforced in `core/permissions.js`. The check throws immediately — the handler is never reached for unauthorized roles.

### High-risk guard

Any command with `risk: "high"` and `confirm: true` in the registry requires the user to type `DELETE` exactly. The string comparison is `===` (case-sensitive, no trim). Anything else returns `{ cancelled: true }` and no API call is made.

### JWT handling

- JWT is held only in memory (`services/session.js`)
- Never written to `.env`, files, or logs
- Lost when the terminal closes
- `AX_API_TOKEN` in `.env` is a convenience for operators — treat this file like a password

### Validate before fetch

Argument validation (`step 6`) runs **before** the API call. Invalid input never reaches the network.

---

## 11. Building the .exe

### Requirements

- Node.js v20+
- `@yao-pkg/pkg` (installed as a devDependency)

### Build command

```bash
npm run build
```

This runs:

```
npx @yao-pkg/pkg . --targets node20-win-x64 --output dist/ax-terminal.exe
```

Output: `dist/ax-terminal.exe` (~42 MB — includes the full Node.js runtime)

### How the build works

`pkg` reads the `pkg` section of `package.json`:

```json
"pkg": {
  "scripts": [
    "cli/**/*.js",
    "core/**/*.js",
    "commands/**/*.js",
    "config/**/*.js",
    "services/**/*.js"
  ]
}
```

It bundles all listed JS files into a virtual snapshot filesystem embedded in the binary. The Node.js v20 runtime is also embedded. The result is a single file that runs with no dependencies on the host machine.

**Important:** The `.env` file is NOT embedded. It must be placed next to the `.exe` at runtime (see [Section 12](#12-deploying-the-exe)).

### Rebuilding after code changes

Any change to source files requires a rebuild:

```bash
npm run build
```

The binary in `dist/` is overwritten. The `.env` file next to it is unaffected.

---

## 12. Deploying the .exe

### Minimum deployment

Copy two files to the target machine (same folder):

```
anywhere\
  ax-terminal.exe
  .env
```

`.env` contents:

```env
AX_API_URL=http://localhost:4000
AX_API_TOKEN=
```

### Running

Double-click `ax-terminal.exe` — no Node.js installation required.

### If AX_API_TOKEN is set

The terminal skips the login prompt and goes straight to the REPL. Good for shared operator machines where the token is pre-configured.

### If AX_API_TOKEN is blank

The terminal prompts for email + password on every launch. Password input is masked (`***`). 3 failed attempts → exit.

### Non-interactive mode (no TTY)

If the terminal detects it is not running in an interactive terminal (e.g. piped input), it requires `AX_API_TOKEN` to be set. Without it, it prints an error and exits:

```
Non-interactive mode detected. Set AX_API_TOKEN in .env to skip login.
```

---

## 13. Adding a New Command

Follow all 4 steps. Missing any one of them will cause a runtime error.

### Step 1 — Create the command file

```
commands/<domain>/<action>.js
```

Example: `commands/billing/summary.js`

```javascript
'use strict';

const chalk      = require('chalk');
const ora        = require('ora');
const apiService = require('../../services/api.service.js');

// Throw an Error here if required args are missing or malformed.
// This runs BEFORE the API call and BEFORE any confirmation prompt.
function validate(args) {
  // example: if (!args.id) throw new Error('Missing --id');
}

async function execute(args) {
  const spinner = ora({ text: 'Fetching billing summary...', color: 'white' }).start();

  let data;
  try {
    data = await apiService.getBillingSummary(); // add this method to api.service.js
    spinner.succeed(chalk.hex('#4CAF50')('Done'));
  } catch (err) {
    spinner.fail(chalk.hex('#F44336')('Failed'));
    throw err;
  }

  console.log('');
  // render data here
  return { success: true };
}

module.exports = { validate, execute };
```

### Step 2 — Register in the command registry (`config/commands.js`)

```javascript
'billing:summary': {
  handler:     'billing/summary',
  permissions: ['admin', 'operator'],
  risk:        'low',
  description: 'View billing summary'
},
```

**Fields:**

| Field | Type | Required | Description |
|---|---|---|---|
| `handler` | string | Yes | Path relative to `commands/`, no `.js` |
| `permissions` | string[] | Yes | `["admin"]` or `["admin", "operator"]` |
| `risk` | string | Yes | `"low"` or `"high"` |
| `confirm` | boolean | No | Set `true` for high-risk commands to require "DELETE" confirmation |
| `description` | string | Yes | Shown in `help` output |
| `skipAuth` | boolean | No | Set `true` to allow before login (only for `platform:login`) |

### Step 3 — Add to the static handler map (`core/executor.js`)

```javascript
const HANDLERS = {
  'platform/login': require('../commands/platform/login.js'),
  'tenant/list':    require('../commands/tenant/list.js'),
  'tenant/delete':  require('../commands/tenant/delete.js'),
  'system/health':  require('../commands/system/health.js'),
  'billing/summary': require('../commands/billing/summary.js')  // ← add this
};
```

### Step 4 — (Optional) Add an alias (`config/aliases.js`)

```javascript
module.exports = {
  'login':   'platform:login',
  'tlist':   'tenant:list',
  'tdel':    'tenant:delete',
  'health':  'system:health',
  'billing': 'billing:summary'  // ← add this
};
```

### Step 5 — Add the API method (`services/api.service.js`)

```javascript
async getBillingSummary() {
  return request('GET', '/api/billing/summary');
}
```

### Step 6 — Rebuild the .exe

```bash
npm run build
```

---

## 14. Color & UI System

All colors are defined in `cli/ui.js`:

```javascript
const C = {
  primary:   chalk.hex('#EAEAEA'),  // soft white — normal text
  secondary: chalk.hex('#A0A0A0'),  // gray — prompts, separators, metadata
  success:   chalk.hex('#4CAF50'),  // green — ✔ success messages
  error:     chalk.hex('#F44336'),  // red — ✖ errors
  warning:   chalk.hex('#FF9800')   // orange — ⚠ high-risk warnings
};
```

**Background:** terminal default (near-black `#0D0D0D` recommended for iTerm2/Windows Terminal)

### UI functions

| Function | Output |
|---|---|
| `ui.printSuccess(msg)` | `✔ msg` in green |
| `ui.printError(msg)` | `✖ Error: msg` in red |
| `ui.printInfo(msg)` | `→ msg` in gray |
| `ui.printHelp(registry)` | Full command table |
| `ui.printLogo()` | ASCII logo + brand + API URL |
| `ui.clearScreen()` | ANSI escape to clear terminal |
| `ui.prompt(role)` | Returns `ax (admin) > ` styled string |

### Spinner usage

```javascript
const ora = require('ora');
const spinner = ora({ text: 'Loading...', color: 'white' }).start();
// ...
spinner.succeed('Done');      // green ✔
spinner.fail('Failed');       // red ✖
spinner.stop();               // silent stop
```

---

## 15. Error Handling Reference

### What each layer is responsible for

| Layer | Handles |
|---|---|
| `core/parser.js` | Malformed input, invalid flag syntax |
| `core/validation.js` | Invalid argument values (bad ID format, etc.) |
| `core/permissions.js` | Unauthorized role access |
| `core/executor.js` | Unknown command, missing handler, session not set |
| `services/api.service.js` | HTTP errors, timeouts, connection refused |
| Command `validate()` | Missing required args for that specific command |
| Command `execute()` | API-level failures for that specific command |
| `cli/index.js` | Catches all thrown errors, prints with `ui.printError()` |

### User-visible error format

All errors surface as:

```
✖ Error: <message>
```

The REPL continues after any error. The session is not invalidated.

---

## 16. Roadmap

### Phase 1 — Complete (current)

- Core terminal infrastructure
- Command registry + alias system
- Role-based permission model
- Interactive login with JWT session
- `tenant:list`, `tenant:delete`, `system:health`
- Windows `.exe` build

### Phase 2 — Next

- Derive role from JWT payload (replace hardcoded `admin`)
- `platform:logout` command (calls `/api/platform/auth/logout`)
- Command history persistence (save to `~/.ax_terminal_history`)
- Tab completion for command names
- `tenant:create` command
- `tenant:status` — change lifecycle status (active/suspended/archived)

### Phase 3 — Future

- `billing:summary` — platform-wide billing overview
- `onboarding:list` / `onboarding:approve` / `onboarding:reject`
- `analytics:export`
- Remote AX-Connect support (HTTPS + remote `AX_API_URL`)
- Multi-environment profiles (dev / staging / prod in same .env)

---

## Quick Reference Card

```
SETUP
  npm install             Install dependencies
  copy .env.example .env  Create config file
  npm start               Run in dev mode
  npm run build           Build dist/ax-terminal.exe

COMMANDS
  help                    Show all commands
  tenant:list             List tenants          (alias: tlist)
  tenant:delete --id=N    Delete tenant         (alias: tdel)
  system:health           Check health          (alias: health)
  platform:login          Re-login mid-session  (alias: login)
  clear                   Clear screen
  exit                    Quit

ENV VARIABLES
  AX_API_URL              API base URL (default: http://localhost:4000)
  AX_API_TOKEN            Pre-auth JWT (optional, skips login prompt)

ADD A COMMAND
  1. commands/<domain>/<action>.js  — implement validate() + execute()
  2. config/commands.js             — register in registry
  3. core/executor.js               — add to HANDLERS map
  4. config/aliases.js              — add alias (optional)
  5. services/api.service.js        — add API method if needed
  6. npm run build                  — rebuild exe
```
