# AX Terminal

Native command-line interface for managing the **AX-Connect** multi-tenant SaaS platform.

Compiled as a standalone Windows PE32+ binary — no Node, no Python, no runtime dependency.

---

## Requirements

| | |
|---|---|
| OS | Windows 10 / 11 x86-64 |
| AX-Connect | running and reachable on the configured port |
| .env | `dist/.env` must contain `AX_API_URL` (see Configuration) |

---

## Installation

1. Copy the contents of `dist/` to any directory on the target machine:
   ```
   dist/
   ├── ax-terminal.exe
   └── .env
   ```

2. Edit `.env` — set `AX_API_URL` to your AX-Connect server address:
   ```env
   AX_API_URL=http://localhost:4000
   ```

3. Run:
   ```
   ax-terminal.exe
   ```

No installation wizard, no `npm install`, no PATH change required.

---

## Configuration

The terminal reads a `.env` file on startup. It searches two locations in order:

1. The directory that contains `ax-terminal.exe` *(used in production)*
2. The current working directory *(used during development)*

### Supported variables

| Variable | Required | Description |
|---|---|---|
| `AX_API_URL` | Yes | Base URL of the AX-Connect API, e.g. `http://localhost:4000` |
| `AX_API_TOKEN` | No | JWT for **CI / automation only** — skips interactive login. Never set this for interactive use. |

> **Note:** `AX_API_TOKEN` is intended for automated pipelines (e.g., GitHub Actions, cron scripts) where interactive login is impossible. Running the terminal interactively with a pre-set token means any command will execute under that token's identity without asking for credentials. Remove it from `.env` after testing.

---

## Authentication

When `AX_API_TOKEN` is **not set**, the terminal always prompts for credentials:

```
  Platform login required.

  Email:    owner@axelis.local
  Password: ••••••••••••

  ✔  Logged in as owner@axelis.local  (role: platform_owner)
```

Up to **3 attempts** are allowed. The session exists only in memory for the duration of the process — no credentials are ever written to disk.

To log in again from inside the REPL:

```
ax (platform_owner) > platform:login --email owner@axelis.local --password ••••••••
```

---

## Commands

### Platform

| Command | Alias | Description | Roles | Risk |
|---|---|---|---|---|
| `platform:login --email <e> --password <p>` | `login` | Authenticate a platform account | all | low |

### Tenant

| Command | Alias | Description | Roles | Risk |
|---|---|---|---|---|
| `tenant:list` | `tlist` | List all tenants | admin, operator | low |
| `tenant:list --output json` | | Same, raw JSON output | admin, operator | low |
| `tenant:create --name <slug>` | `tcreate` | Create a new tenant | **admin only** | low |
| `tenant:use --id <id>` | `tuse` | Set the default tenant for this session | admin, operator | low |
| `tenant:delete --id <id>` | `tdel` | Permanently terminate a tenant | **admin only** | **HIGH** |

### System

| Command | Alias | Description | Roles | Risk |
|---|---|---|---|---|
| `system:health` | `health` | Check API and database health | admin, operator | low |

### Audit

| Command | Alias | Description | Roles | Risk |
|---|---|---|---|---|
| `audit:log` | `alog` | Show your own command history | all | low |
| `audit:log --user <email>` | | Show another user's history | **admin only** | low |
| `audit:log --output json` | | Raw JSON output | all | low |

### Built-in

| Command | Description |
|---|---|
| `help` | Show the full command reference |
| `clear` | Clear the screen |
| `exit` / `quit` | Exit the terminal |

---

## Permission Model

| Role | Mapped from AX-Connect roles | Access |
|---|---|---|
| **admin** | `platform_owner`, `platform_manager` | All commands, including destructive operations |
| **operator** | `platform_developer`, restaurant roles | Read and safe operations only |

Role is resolved live from the JWT on every command — no hardcoding.

---

## Tenant name validation

`tenant:create` enforces slug rules before sending the API request:

- Lowercase ASCII letters and digits only
- Hyphens allowed in the middle
- No leading or trailing hyphens
- No spaces, no uppercase

Valid: `cafe-roma`, `tenant-01`, `axelis`
Invalid: `Cafe Roma`, `-cafe`, `cafe-`, `café`

---

## High-risk confirmation flow

`tenant:delete` triggers a manual confirmation prompt before any API call is made:

```
ax (platform_owner) > tenant:delete --id 3

  ⚠  HIGH RISK ACTION
  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Command:   tenant:delete
  Tenant ID: 3

  ✖  This action is IRREVERSIBLE.

  Type "DELETE" to confirm: DELETE

  ✔  Tenant "cafe-roma" (ID 3) has been terminated
```

Anything other than the exact string `DELETE` aborts silently.

---

## Local audit log

Every command you run is appended locally to:

```
%APPDATA%\ax-terminal\audit.jsonl
```

Each line is a JSON object:

```json
{"timestamp":"2026-04-21T05:27:14Z","user":"owner@axelis.local","role":"platform_owner","command":"tenant:list","status":"success"}
```

The log is written to disk — it survives across sessions and does not require a network connection to read.

### Reading the log

```
ax (platform_owner) > audit:log
```
Shows your own history as a table.

```
ax (platform_owner) > audit:log --user manager@axelis.local
```
Shows another user's history. Requires admin role.

```
ax (platform_owner) > audit:log --output json
```
Outputs the full history as a JSON array — useful for piping to `jq` or scripts.

---

## Persistent session config

`tenant:use` saves the selected tenant ID to:

```
%USERPROFILE%\.ax\config.json
```

```json
{
  "defaultTenant": 5,
  "output": "table"
}
```

This config survives restarts. It is never sent to the server.

---

## Keyboard shortcuts

| Key | Action |
|---|---|
| `↑` / `↓` | Scroll command history |
| `Ctrl+R` | Search history (incremental) |
| `Ctrl+A` / `Home` | Jump to start of line |
| `Ctrl+E` / `End` | Jump to end of line |
| `Ctrl+W` | Delete word back |
| `Ctrl+K` | Delete to end of line |
| `Ctrl+U` | Clear entire line |
| `Ctrl+Y` | Paste last killed text |
| `Ctrl+C` | Cancel current input (does not exit) |
| `Ctrl+D` | Exit (EOF) |

Command history is persisted across sessions at `%APPDATA%\ax-terminal\history` (up to 500 entries, duplicates suppressed).

---

## Project structure

```
ax_terminal/
├── src/
│   ├── main.rs          Entry point, REPL loop, JWT decoder, PromptHelper
│   ├── api.rs           Blocking HTTP client (ureq) — all API calls
│   ├── audit_log.rs     Local JSONL audit log — append and read
│   ├── commands/
│   │   ├── mod.rs       Command registry, executor, role checks, audit dispatch
│   │   ├── audit.rs     audit:log command
│   │   ├── platform.rs  platform:login command
│   │   ├── system.rs    system:health command
│   │   └── tenant.rs    tenant:list / create / use / delete
│   ├── config_store.rs  Persistent config at %USERPROFILE%\.ax\config.json
│   ├── env.rs           .env loader
│   ├── parser.rs        Command tokenizer — name + --key value pairs
│   ├── session.rs       In-memory JWT session (token, email, role)
│   └── ui.rs            Colors, spinner, table renderer, help output
├── dist/
│   ├── ax-terminal.exe  Compiled binary (Windows x86-64)
│   └── .env             Runtime configuration
├── Cargo.toml
└── README.md
```

---

## Building from source

**Prerequisites:**
- Rust stable toolchain (`rustup`)
- `x86_64-pc-windows-gnu` target for cross-compilation, or MSVC on Windows

```bash
# Release build (optimised, LTO, stripped)
cargo build --release

# Output: target/release/ax-terminal.exe
```

The release profile is configured for a small, fast binary:

```toml
[profile.release]
opt-level     = 3
lto           = true
codegen-units = 1
panic         = "abort"
strip         = true
```

**Dependencies** (all pure Rust, no system libraries):

| Crate | Purpose |
|---|---|
| `ureq 2.9` | Blocking HTTP client — no async, no TLS (internal network) |
| `serde` / `serde_json` | JSON serialisation |
| `crossterm 0.27` | Cross-platform ANSI colors and terminal control |
| `rustyline 14` | Readline-style line editing, history, Emacs keybindings |
| `rpassword 7` | Masked password input |
