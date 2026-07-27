# AX Terminal — Production Audit
## Phase 1 — Part 2: Command Execution, Parsing, Authorization, Audit, and UI

**Repository:** `abdelrhman726/AX_terminal`  
**Branch audited:** `main`  
**Audit scope:** `src/parser.rs`, `src/commands/mod.rs`, `src/commands/platform.rs`, `src/commands/tenant.rs`, `src/commands/system.rs`, `src/commands/audit.rs`, `src/audit_log.rs`, `src/ui.rs`

---

## 1. Executive conclusion

AX Terminal has a clear command pipeline:

```text
Raw terminal input
      ↓
Tokenizer / parser
      ↓
ParsedCommand
      ↓
Static command registry
      ↓
Local authentication check
      ↓
Local role check
      ↓
High-risk confirmation
      ↓
Command implementation
      ↓
AX-Connect API request
      ↓
Local JSONL audit
      ↓
Remote fire-and-forget audit
      ↓
Terminal output
```

The structure is understandable and modular.

However, the current design is **not yet production-grade for a high-privilege administrative control plane**.

The highest-priority architectural problem is that the command executor currently combines:

- command resolution,
- local authorization,
- destructive confirmation,
- command execution,
- local audit,
- remote audit dispatch

inside one central flow.

That is acceptable for the current small CLI, but it will become a serious control-plane risk as AX Terminal gains infrastructure, deployment, payment, tenant, and production operations.

---

# 2. P0 / Critical findings

## P0-1 — Local authorization is not a sufficient security boundary

The command registry marks commands as `admin_only`, and the executor blocks them based on the locally decoded session role.

Examples:

- `tenant:create` → admin only
- `tenant:delete` → admin only
- `audit:log --user` → admin only

The local role is derived from the session role and mapped to `Admin` or `Operator`.

This must remain UX-level enforcement only.

The actual security boundary must be:

```text
AX Terminal local check
        ↓
        not authoritative

AX-Connect API authorization
        ↓
        authoritative
```

A modified client binary must never be able to bypass server authorization.

### Required architectural rule

Every privileged API endpoint must independently verify:

- authenticated identity
- role/permission
- tenant/platform scope
- resource ownership or administrative scope
- action authorization

The CLI must not be trusted because it is a client application.

---

## P0-2 — Destructive confirmation is purely local and not bound strongly enough to the target

`tenant:delete`:

1. receives an ID
2. fetches the tenant
3. displays the tenant name and slug
4. asks the operator to type `DELETE`
5. sends the termination request

This is better than no confirmation, but the confirmation string is generic.

The operator is not required to type:

```text
DELETE <exact tenant slug>
```

or another target-bound confirmation.

The current flow is:

```text
tenant:delete --id 123
        ↓
fetch tenant 123
        ↓
show tenant
        ↓
type DELETE
        ↓
delete tenant
```

A stronger destructive-operation model is:

```text
resolve resource
        ↓
show exact immutable identity
        ↓
require exact target-bound confirmation
        ↓
server validates authorization
        ↓
server validates idempotency / state
        ↓
execute
```

For high-risk production operations, confirmation should be bound to the exact target.

---

## P0-3 — Remote audit events can be lost silently

The executor writes locally and then calls a remote audit method.

The remote audit method starts a thread and discards every error.

Therefore:

```text
command succeeds
      ↓
local audit succeeds
      ↓
remote audit fails
      ↓
operator sees nothing
      ↓
remote audit event is permanently lost
```

This is not acceptable for a reliable security trail.

The local JSONL file is useful as a local history, but it is not currently a durable synchronization queue.

### Required architecture

```text
command event
      ↓
durable local event record
      ↓
attempt remote delivery
      ↓
success → mark delivered
failure → retry later
```

The event should have a stable event ID.

Recommended future fields:

```text
event_id
timestamp
user_id / user_email
role
command
normalized_arguments
status
client_version
delivery_status
attempt_count
last_attempt_at
```

Do not put secrets or passwords in the event.

---

# 3. P1 / High-priority findings

## P1-1 — Parser silently ignores malformed input

The parser returns a `ParsedCommand` directly and does not return parse errors.

Malformed arguments can be silently ignored.

Examples of risks:

```text
--bad$key=value
--key="unterminated
```

or other malformed syntax.

The parser sanitizes keys, which is good, but invalid syntax is silently dropped instead of reported to the operator.

### Required improvement

Change the parser contract from:

```text
parse(input) -> ParsedCommand
```

to something equivalent to:

```text
parse(input) -> Result<ParsedCommand, ParseError>
```

The parser should distinguish:

- unknown command syntax
- malformed quoting
- invalid argument key
- duplicate argument
- unexpected positional argument
- missing value

This makes the CLI fail closed instead of silently changing the operator's intended command.

---

## P1-2 — No command schema validation layer

The current registry contains:

```text
command name
alias
command kind
requires auth
admin only
high risk
```

But argument validation is implemented inside individual command modules.

For example:

- `tenant:create` validates `--name`
- `tenant:use` validates `--id`
- `tenant:delete` validates `--id`

This creates a risk of inconsistent validation as the command count grows.

### Recommended architecture

```text
CommandDefinition
├── name
├── aliases
├── required permissions
├── risk level
├── argument schema
├── confirmation policy
└── handler
```

Then validation occurs before the handler is invoked.

---

## P1-3 — Permission model is too coarse for future AX-Connect operations

Current permission model:

```text
Admin
Operator
```

This is already too coarse for a serious platform control plane.

Future AX Terminal operations will likely include:

- tenant administration
- deployment
- infrastructure
- database operations
- payments
- billing
- security
- audit access

A two-role model will eventually create excessive privilege.

The future model should be capability/permission based, for example:

```text
tenant.read
tenant.create
tenant.delete
system.health.read
audit.read.self
audit.read.any
deployment.execute
payment.refund
infrastructure.restart
```

The server should authorize these capabilities.

The CLI may use the returned permissions for UX, but the API must enforce them.

---

## P1-4 — `tenant:use` persists a default tenant but does not yet establish a strong session scope

`tenant:use` writes a default tenant ID to persistent config.

This is useful for convenience, but it does not automatically guarantee that future commands are scoped to that tenant.

A future command could accidentally use:

- a supplied explicit ID
- the persisted default tenant
- a stale tenant ID

without a clear precedence model.

### Required rule

Define explicitly:

```text
explicit command target
        >
session target
        >
persistent default
        >
no target
```

For destructive operations, implicit targeting should generally be avoided.

---

## P1-5 — Local audit log has no integrity protection

The local audit file is append-only JSONL in normal operation, but there is no cryptographic integrity mechanism.

A local user with filesystem access can modify or delete it.

For a security tool, local audit should be treated as:

```text
local history
```

not:

```text
tamper-proof compliance evidence
```

If local audit is intended to support compliance, consider:

- hash chaining
- signed events
- remote durable ingestion
- server-side append-only audit storage

The strongest source of truth should be server-side.

---

# 4. P2 / Important findings

## P2-1 — Audit logging omits the full command arguments

The local audit records the command name and status, but not the complete normalized arguments.

This means:

```text
tenant:delete
```

is recorded, but the target tenant ID may not be.

That weakens forensic usefulness.

The audit record should include a sanitized normalized argument representation.

Never include:

```text
password
access token
secret
API key
```

---

## P2-2 — Login is intentionally excluded from audit logging

The executor skips `platform:login`.

This avoids recording credentials, which is good.

However, the system should still record a safe authentication event such as:

```text
auth.login.success
auth.login.failure
auth.logout
```

with:

- user identity if known
- timestamp
- result
- reason category
- client version
- correlation ID

No password or token.

---

## P2-3 — Audit failures are completely invisible

Both local and remote audit failures are swallowed.

The reason given is that audit must never crash the terminal.

That principle is correct, but the implementation should be:

```text
audit failure
      ↓
do not crash command
      ↓
record failure locally
      ↓
show non-secret warning when appropriate
      ↓
retry later
```

"Do not crash" must not mean "pretend the failure never happened."

---

## P2-4 — High-risk confirmation uses blocking stdin directly

The confirmation code reads directly from stdin.

This works in the current interactive REPL, but the behavior should be formalized before adding:

- non-interactive mode
- CI mode
- scripted execution
- remote terminal use

Dangerous commands should fail closed when there is no interactive confirmation mechanism.

Never automatically treat a non-interactive environment as confirmed.

---

## P2-5 — Spinner thread architecture is acceptable for current scale but should not become the transport model

The spinner uses a dedicated thread and an atomic stop flag.

The thread is joined before the result is printed.

This prevents output interleaving and is a reasonable implementation for the current blocking HTTP client.

However, the broader architecture currently has:

```text
main thread
    ↓
blocking API call

spinner thread
    ↓
terminal animation
```

This is acceptable now.

If the API client becomes async later, the UI model should be redesigned rather than adding more unmanaged threads.

---

## P2-6 — Table rendering uses byte length rather than display width

The table calculates widths using `str::len()`.

This measures bytes, not terminal display columns.

This can misalign:

- Unicode text
- Arabic
- CJK characters
- emoji

Since AX-Connect may support Arabic, this is a real future UX issue.

A production terminal UI should use Unicode display width rather than byte length.

---

# 5. P3 / Lower-priority findings

## P3-1 — Command metadata is duplicated

Command information exists in multiple locations:

- `ui.rs` help table
- `commands/mod.rs` registry
- individual command modules

This creates drift risk.

A command can be:

```text
registered one way
authorized another way
documented differently
```

The long-term solution is a single command definition source.

---

## P3-2 — `CommandKind` enum will become a bottleneck as the CLI expands

Every new command currently requires modifying central dispatch logic.

That is acceptable for seven commands.

For a large AX control plane, use a more extensible command registry/handler architecture.

The registry should remain centrally controlled for security, but implementation should be modular.

---

## P3-3 — API output is dynamically typed JSON throughout the command layer

Commands directly access fields such as:

```text
result["id"]
result["slug"]
result["status"]
```

This is flexible but weakens compile-time guarantees.

For high-risk operations, typed response models should eventually be used.

Especially for:

- tenant deletion
- deployment
- payments
- infrastructure actions

---

# 6. Current command execution security model

## Current

```text
Input
  ↓
Parser
  ↓
Command name lookup
  ↓
Local auth check
  ↓
Local role check
  ↓
Local confirmation
  ↓
Handler
  ↓
API request
  ↓
Local audit
  ↓
Remote fire-and-forget audit
```

## Target production model

```text
Raw input
  ↓
Strict parser
  ↓
Validated command schema
  ↓
Resolved command definition
  ↓
Local UX permission hint
  ↓
Target resolution
  ↓
Target-bound confirmation
  ↓
Server authorization
  ↓
Idempotent API operation
  ↓
Durable local event record
  ↓
Remote audit delivery queue
  ↓
Result
```

---

# 7. Implementation plan generated from this part

Do not implement all of this immediately.

The correct sequence is:

## Step 1 — Establish tests before refactoring

Create tests for:

- parser behavior
- malformed input
- quoting
- duplicate arguments
- command resolution
- role mapping
- destructive confirmation
- tenant ID validation
- audit record generation

## Step 2 — Separate command definition from execution

Create a structured command definition model containing:

- name
- aliases
- required authentication
- required permissions
- risk level
- argument schema
- handler

## Step 3 — Replace silent parser failure

Make parsing return structured errors.

## Step 4 — Introduce permission capabilities

Keep server authorization authoritative.

Use local permissions only for UX and early feedback.

## Step 5 — Redesign audit as durable local-first delivery

The current fire-and-forget remote audit must be replaced.

## Step 6 — Make destructive confirmations target-bound

For example:

```text
Type the exact tenant slug to confirm:
cafe-roma
```

or an equivalent exact confirmation token.

## Step 7 — Centralize command metadata

Eliminate duplicated command descriptions and permission metadata.

## Step 8 — Improve terminal rendering

Use display width for Unicode/Arabic support.

---

# 8. Findings carried forward into the cumulative AX Terminal audit

These are now part of the permanent audit baseline for future work:

### Phase 1 Part 1 baseline

1. Production TLS path is not established.
2. Remote audit events can be silently lost.
3. HTTP transport is recreated per request.
4. Configuration validation is weak.
5. JWT-derived local role must never be authoritative.
6. Local config writes are not atomic.
7. Secret memory is not explicitly zeroized.

### Phase 1 Part 2 baseline

8. Local authorization is UX-level only; server authorization is authoritative.
9. Destructive confirmation should be target-bound.
10. Remote audit must become durable and retryable.
11. Parser should return structured errors instead of silently ignoring malformed syntax.
12. Command argument validation should be schema-driven.
13. Two-role authorization is too coarse for future platform operations.
14. Persistent tenant selection needs explicit target precedence rules.
15. Local audit has no tamper-evident integrity protection.
16. Audit events should contain sanitized normalized arguments.
17. Login events should be auditable without credentials.
18. Audit failures should be visible/recoverable without crashing commands.
19. Non-interactive destructive operations must fail closed.
20. Terminal table width must use Unicode display width.
21. Command metadata is duplicated and can drift.
22. Central dispatch will become a scalability bottleneck.
23. Dynamic JSON should eventually be replaced with typed models for high-risk operations.

---

## Final status

**Phase 1 — Part 2: COMPLETE**

No production implementation should begin from memory or assumptions. This report is the source-of-truth checkpoint for this part.

