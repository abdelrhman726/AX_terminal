# AX Terminal — Developer Guide

Architecture reference, decision log, and extension guide for the Rust rewrite of AX Terminal.

---

## Overview

AX Terminal is a native Windows CLI compiled to a single PE32+ executable (~1.2 MB). It connects to the AX-Connect REST API (Node.js / Express / PostgreSQL multi-tenant SaaS) on a configurable base URL.

There is no Node runtime, no npm, no async executor, no web framework. The terminal uses:

- **Blocking I/O** everywhere — `ureq` for HTTP, `std::fs` for disk, `std::io` for stdin
- **Single thread** for all user-facing logic; background threads only for the spinner and fire-and-forget audit log
- **No `unsafe`** except for `std::env::set_var` in `env.rs`, which is called once before any threads spawn

---

## Module map

| Module | File | Responsibility |
|---|---|---|
| Entry point | `src/main.rs` | Startup, auth flow, REPL loop, JWT decoder, `PromptHelper` |
| HTTP client | `src/api.rs` | All API calls — auth, tenants, health, audit |
| Audit log | `src/audit_log.rs` | Local JSONL append/read at `%APPDATA%\ax-terminal\audit.jsonl` |
| Command registry | `src/commands/mod.rs` | `CommandKind` enum, static registry, executor, role checks |
| Audit command | `src/commands/audit.rs` | `audit:log` — reads local JSONL, filters by user |
| Platform commands | `src/commands/platform.rs` | `platform:login` |
| System commands | `src/commands/system.rs` | `system:health` |
| Tenant commands | `src/commands/tenant.rs` | `tenant:list`, `tenant:create`, `tenant:use`, `tenant:delete` |
| Config store | `src/config_store.rs` | Persistent JSON config at `%USERPROFILE%\.ax\config.json` |
| Env loader | `src/env.rs` | `.env` file parser, `Config` struct |
| Parser | `src/parser.rs` | Tokenizer: command name + `--key value` pairs |
| Session | `src/session.rs` | In-memory JWT store — token, email, role |
| UI | `src/ui.rs` | Colors, spinner, table, help text |

---

## Authentication flow

```
main()
  │
  ├─ env::Config::load()            reads .env → api_url, api_token
  │
  ├─ AX_API_TOKEN set?
  │     YES → role_from_jwt(token)  decode JWT payload (no crate, no network)
  │           session::set(token, None, role)
  │     NO  → interactive_login()
  │           prompt email + masked password (rpassword)
  │           POST /api/platform/auth/login
  │           session::set(token, email, role)
  │
  └─ repl()                         enter REPL loop
```

The JWT is **never written to disk**. It lives only in the `OnceLock<Mutex<SessionData>>` for the process lifetime.

### JWT role extraction

`role_from_jwt()` decodes the payload segment (middle part of `header.payload.signature`) without any cryptography crate:

1. Split on `.`, take index 1
2. Re-pad base64url → standard base64
3. Decode with a hand-written `base64_decode_std()` (40 lines, no allocation beyond `Vec<u8>`)
4. Parse JSON, extract `"role"` claim

The signature is **not verified** — we trust the token origin (it came from AX-Connect). The server re-validates on every request. The extracted role is used only for display and client-side command gating.

---

## Command lifecycle

```
user input
  │
  ├─ parser::parse(&input)          → ParsedCommand { name, args }
  │
  └─ commands::execute(&parsed, client)
        │
        ├─ resolve(name)            → CommandDef { kind, requires_auth, admin_only, high_risk }
        ├─ session check            → error if requires_auth and not authenticated
        ├─ role check               → error if admin_only and role != Admin
        ├─ high-risk confirmation   → prompt "Type DELETE to confirm" if high_risk
        │
        ├─ dispatch match           → call the handler fn
        │
        └─ post-dispatch (non-login, non-audit commands only)
              ├─ audit_log::append(user, role, cmd, status)   ← local JSONL
              └─ client.audit_log(cmd, status)                ← fire-and-forget API call
```

### Adding a new command

1. **Add a variant** to `CommandKind` in `src/commands/mod.rs`
2. **Add registry entries** (canonical name + alias) to `REGISTRY`
3. **Add permission flags** to the `resolve()` match arm: `(requires_auth, admin_only, high_risk)`
4. **Add dispatch arm** to the `match def.kind` block in `execute()`
5. **Implement the handler** — create `src/commands/<module>.rs` with a `pub fn <name>(parsed: &ParsedCommand, client: &ApiClient) -> Result<(), String>` signature (or without `client` if no network call is needed)
6. **Declare the module** with `pub mod <module>;` at the top of `src/commands/mod.rs`
7. **Add to help** — append a `CommandHelp` entry to `ui::COMMANDS` in `src/ui.rs`

---

## Role model

```rust
pub fn current_role() -> Role {
    match session::role().as_str() {
        "admin" | "platform_owner" | "platform_manager" => Role::Admin,
        _                                                => Role::Operator,
    }
}
```

Role is derived **live** from the session on every command — never hardcoded, never cached between commands. A `platform:login` mid-session updates the role immediately.

| AX-Connect role | Terminal role |
|---|---|
| `platform_owner` | `Admin` |
| `platform_manager` | `Admin` |
| `platform_developer` | `Operator` |
| `restaurant_*` | `Operator` |
| anything else | `Operator` (safe default) |

---

## Parser

`src/parser.rs` handles two flag forms:

```
tenant:create --name cafe-roma          space-separated (value is next token)
tenant:create --name="cafe-roma"        equals-attached
tenant:create --name 'cafe-roma'        single-quoted
tenant:create --flag                    boolean flag (no value)
```

Prototype-pollution guard: key names that match `__proto__`, `constructor`, or `prototype` are rejected silently.

---

## Prompt rendering (cursor correctness)

rustyline measures the prompt length to calculate cursor column positions. If the prompt string contains ANSI escape bytes, rustyline counts them as printable characters and every cursor operation shifts by the escape byte count — backspace eats the prompt, click-to-move lands in the wrong column.

**Fix:** `PromptHelper` implements `rustyline::Highlighter`:

```rust
impl Highlighter for PromptHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self, prompt: &'p str, _default: bool,
    ) -> Cow<'b, str> {
        // Return the colored version for rendering
    }
}
```

`rl.readline()` receives a plain string `"ax (platform_owner) > "` — rustyline measures this for cursor math. `highlight_prompt()` returns the ANSI-colored copy — rustyline uses this only for display. The two are never confused.

---

## Spinner

`ui::Spinner` runs a braille animation on a background thread via `Arc<AtomicBool>`:

```rust
let stop = Arc::new(AtomicBool::new(false));
// background thread loops until stop == true
```

`succeed()` and `fail()` call `finish()` which:
1. Sets the stop flag
2. Joins the thread
3. Erases the spinner line with `\r{spaces}\r`

The `Drop` impl handles early returns (e.g. from `?` propagation) — it only erases the line if the thread was still running, preventing a double-erase if `succeed()`/`fail()` already called `finish()`.

---

## Local audit log

**File:** `%APPDATA%\ax-terminal\audit.jsonl` (JSONL — one JSON object per line)

**Format:**
```json
{"timestamp":"2026-04-21T05:27:14Z","user":"owner@axelis.local","role":"platform_owner","command":"tenant:list","status":"success"}
```

**Timestamp:** Generated without `chrono` — `SystemTime::now()` gives a Unix epoch in seconds, which is then converted to Gregorian date/time by `audit_log::iso8601()` (~30 lines of arithmetic).

**Write strategy:** `OpenOptions::new().create(true).append(true)` — atomic append, one line per call. All I/O errors are silently swallowed. Audit must never crash or block the terminal.

**Commands not logged:**
- `platform:login` — credentials are in flight during execution; logging before auth settles would record misleading data
- `audit:log` — reading the log should not appear in the log (avoids recursive clutter)

---

## Config store

**File:** `%USERPROFILE%\.ax\config.json`

```json
{
  "defaultTenant": 5,
  "output": "table"
}
```

**Path-traversal guard:** Before writing, `config_store::save()` canonicalizes both the target directory and the write path's parent, then asserts they are identical. This prevents a malformed key from escaping `~/.ax/`.

---

## API client (`src/api.rs`)

All requests are **synchronous** — `ureq` returns a `Result` directly. No `async`/`await`, no Tokio.

`AX_API_TOKEN` is **not** TLS-protected in the default config (plain HTTP to `localhost:4000`). If the server is moved to a public address, enable TLS in `Cargo.toml`:

```toml
ureq = { version = "2.9", features = ["json", "tls"] }
```

### Error handling

| HTTP status | Terminal message |
|---|---|
| 401 | "Unauthorized — session expired." |
| 403 | "Access denied — your role cannot perform this action." |
| Other 4xx/5xx | Parsed from `error` or `message` field in JSON body |
| Connection refused (incl. Windows error 10061) | "Cannot reach AX-Connect. Is the server running?" |
| Timeout (10 s) | "Request timed out after 10 seconds." |

### `GET /api/platform/restaurants/:id` shape

AX-Connect wraps the restaurant in a nested object:

```json
{ "restaurant": { "id": 5, "slug": "cafe-roma", ... }, "settings": {...}, "branches": [...] }
```

`api.rs::get_tenant()` unwraps the `restaurant` key so callers always get a flat object. If the response is already flat (shape change), it is forwarded as-is.

---

## Known server-side limitations

| Issue | Root cause | Terminal behaviour |
|---|---|---|
| `tenant:use` returns `column "filename" does not exist` | SQL bug in AX-Connect `getRestaurant()` query | Error surfaced as-is |
| `tenant:create` returns 403 for `platform_manager` | AX-Connect restricts creation to `platform_owner` | "Access denied — your role cannot perform this action" |

---

## Build

```bash
cargo build --release
```

Output: `target/release/ax-terminal.exe`

Copy `ax-terminal.exe` and `.env` to `dist/` for distribution.

**Toolchain:** `stable-x86_64-pc-windows-gnu` (MSYS2/MinGW) or `stable-x86_64-pc-windows-msvc`.

**Kill before rebuilding** — Windows locks the `.exe` while it is running:

```powershell
Stop-Process -Name ax-terminal -Force
```
