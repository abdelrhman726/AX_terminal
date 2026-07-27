# Project Report: AX Terminal CLI

This report provides a comprehensive overview of the **AX Terminal** project. The details provided are derived directly from the project's source code and historical implementation logs.

---

## 1. What is AX Terminal?
AX Terminal is a **native Windows command-line interface** (PE32+ executable) designed specifically for platform administrators and operators of the **AX-Connect** multi-tenant SaaS platform. 

It provides a secure, keyboard-driven environment for high-stakes platform operations—such as tenant management and system monitoring—that are otherwise performed via manual API calls.

### Core Philosophy
- **Performance:** Built in Rust for near-instant execution and minimal resource footprint (~1.2 MB binary).
- **Security:** Implements a strict execution model with zero shell-injection vulnerabilities; commands are registry-controlled, not shell-interpreted.
- **Portability:** Compiled to a standalone `.exe` with zero runtime dependencies (no Node.js, Python, or external DLLs required).

---

## 2. Capabilities & Features
The CLI operates on a `<domain>:<action>` command pattern and currently supports:

| Command | Description | Proof (Source) |
| :--- | :--- | :--- |
| `platform:login` | Secure JWT-based authentication with masked password input. | `src/commands/platform.rs` |
| `tenant:list` | Retrieves and displays all active tenants from AX-Connect. | `src/commands/tenant.rs` |
| `tenant:create` | Provisions new tenants with name validation. | `src/commands/tenant.rs` |
| `tenant:delete` | Permanent tenant termination requiring explicit `DELETE` confirmation. | `src/commands/tenant.rs` |
| `system:health` | Real-time monitoring of API status, database connectivity, and uptime. | `src/commands/system.rs` |
| `audit:log` | Local-first auditing of all performed actions for security compliance. | `src/commands/audit.rs` |

---

## 3. Architecture & Build
The project underwent a significant evolution, starting as a Node.js prototype before being completely rewritten in **Rust** for production reliability.

### Technology Stack
- **Language:** Rust (2021 Edition)
- **HTTP Client:** `ureq` (Blocking I/O for simplicity and speed)
- **Terminal UI:** `rustyline` (REPL, history, auto-completion) and `crossterm` (ANSI colors)
- **Serialization:** `serde_json` for robust API data handling

### Folder Structure
```text
/ax_terminal
├── src/
│   ├── main.rs          # REPL loop, Auth flow, PromptHelper
│   ├── api.rs           # Centralized AX-Connect API client
│   ├── config_store.rs  # Persistent user settings (%USERPROFILE%/.ax)
│   ├── audit_log.rs     # Local JSONL security auditing
│   └── commands/        # Modular command implementation logic
├── dist/                # Production release folder (.exe + .env)
├── Cargo.toml           # Rust dependency manifest
└── README.md            # User-facing documentation
```

---

## 4. Current State & Known Constraints
As of the latest audit:
- **Connection:** The CLI connects to the AX-Connect API (defaulting to `http://localhost:4000`).
- **Authentication:** Uses JWT Bearer tokens stored securely in memory for the duration of the session.
- **Limitation:** The current build requires an `.env` file in the same directory as the `.exe` to define the `AX_API_URL`.

---

## 5. Roadmap
Based on the implementation specifications and technical debt analysis:

1.  **Phase 1: Enhanced Telemetry**
    *   Transition `system:health` to consume the `/monitor/health/detailed` endpoint for memory and database performance metrics.
    *   Add clipboard integration (Ctrl+Y/Ctrl+V) for token and ID handling.
2.  **Phase 2: Operational Scalability**
    *   Implement `tenant:use` to "sticky" a specific tenant ID to the session, removing the need for `--id` flags on every command.
    *   Add background "fire-and-forget" audit syncing to the remote AX-Connect server.
3.  **Phase 3: Visual Polish**
    *   Implementation of a real-time animated spinner for long-running provision tasks.
    *   Table-based rendering for tenant lists using custom terminal widths.

---

**Summary for Developers:** AX Terminal is the high-performance "cockpit" for AX-Connect. It is built for speed and security, using a modular Rust architecture that makes adding new platform operations as simple as adding a new file to the `src/commands/` directory.
