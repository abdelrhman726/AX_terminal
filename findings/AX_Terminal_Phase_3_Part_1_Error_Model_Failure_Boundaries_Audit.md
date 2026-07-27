# AX Terminal — Phase 3, Part 1
## Error Model and Failure Boundaries Audit

Repository: `abdelrhman726/AX_terminal`
Branch: `main`

---

# 1. Scope

This part inspected the error and failure path across:

- `src/api.rs`
- `src/main.rs`
- `src/commands/mod.rs`
- `src/commands/tenant.rs`
- `src/audit_log.rs`
- `src/session.rs`

Focus:

```text
network failures
HTTP failures
JSON/schema failures
command failures
authentication failures
panic/unwrap boundaries
audit failures
partial-success states
user-visible error semantics
```

---

# 2. Executive conclusion

AX Terminal currently has a simple and understandable error model:

```text
Result<T, String>
        ↓
command returns Err(String)
        ↓
executor audits error
        ↓
REPL prints error
        ↓
terminal continues
```

This is acceptable for an early prototype.

It is not sufficient for a production control-plane CLI.

The main architectural problem is that fundamentally different failures are collapsed into plain strings:

```text
authentication failure
authorization failure
validation failure
network failure
timeout
TLS failure
server error
malformed server response
local filesystem failure
configuration failure
```

all eventually become:

```text
String
```

This destroys machine-readable semantics and makes safe recovery difficult.

The target architecture should be:

```text
typed error
    ↓
classified failure
    ↓
recovery policy
    ↓
user-facing rendering
    ↓
structured audit/telemetry
```

---

# 3. P0 — Critical findings

## P0-1 — All API failures are flattened into `String`

`ApiClient` exposes methods such as:

```rust
Result<Value, String>
```

and converts all failures into human-readable strings.

This means callers cannot reliably distinguish:

```text
401 Unauthorized
403 Forbidden
404 Not Found
409 Conflict
422 Validation Error
429 Rate Limited
500 Server Error
502/503/504 Upstream Failure
timeout
connection refused
TLS failure
invalid JSON
```

The consequence is severe:

```text
failure
  ↓
String
  ↓
caller guesses from text
```

Production code needs:

```text
ApiError::Unauthorized
ApiError::Forbidden
ApiError::RateLimited
ApiError::Timeout
ApiError::Transport(...)
ApiError::InvalidResponse(...)
```

---

## P0-2 — Error recovery cannot be correct while error identity is lost

Because errors are strings, the terminal cannot safely implement:

```text
401 → clear session / re-authenticate
403 → do not retry
429 → respect Retry-After
500 → maybe retry
timeout → bounded retry
connection refused → server unavailable
malformed JSON → protocol failure
```

Instead, the current code produces text such as:

```text
"Request timed out after 10 seconds."
```

or:

```text
"Access denied — your role cannot perform this action."
```

The text is suitable for display, but not as an internal control signal.

---

## P0-3 — HTTP success does not guarantee semantic success

The API client treats a successful HTTP response as JSON and returns it.

A response with:

```text
HTTP 200
```

and an unexpected body can still be returned as success.

Examples:

```text
{}
```

or:

```json
{"message":"operation queued"}
```

when the caller expects:

```json
{"id":123,"slug":"..."}
```

The caller then continues and may:

```text
display incorrect success
persist incorrect state
perform a subsequent operation using missing data
```

The client needs typed response validation at the boundary.

---

## P0-4 — Partial-success states are not modeled

Several operations have multiple externally visible steps.

Example:

```text
tenant:delete
    ↓
fetch tenant
    ↓
confirm
    ↓
terminate
    ↓
audit locally
    ↓
send remote audit
```

The business operation may succeed while:

```text
local audit fails
remote audit fails
terminal crashes after server success
```

The system currently does not model:

```text
operation succeeded
audit delivery pending
```

as a distinct state.

This matters for production observability.

---

# 4. P1 — High-priority findings

## P1-1 — Transport error classification relies on string matching

The transport layer detects connection failures by searching text:

```text
"Connection refused"
"Os {"
"actively refused"
"os error 10061"
```

and timeout by searching:

```text
"timed out"
```

This is fragile because:

```text
OS wording changes
library wording changes
platform wording differs
nested error formatting changes
```

The correct implementation should classify underlying error kinds where the library exposes them.

String matching can remain only as a last-resort fallback.

---

## P1-2 — The API response parser accepts arbitrary JSON

`handle_response` parses any successful body into:

```text
serde_json::Value
```

This provides no schema guarantee.

The result is a weak boundary:

```text
server response
      ↓
arbitrary JSON
      ↓
business code
```

The target should use typed DTOs for important operations:

```text
LoginResponse
Tenant
TenantListResponse
HealthResponse
TerminateTenantResponse
```

For truly flexible endpoints, validation should still be explicit.

---

## P1-3 — JSON parse failures are not classified as protocol failures

Malformed JSON currently becomes a plain string error.

The caller cannot distinguish:

```text
server returned invalid JSON
```

from:

```text
network failed
```

This should be a dedicated protocol/schema error.

It may indicate:

```text
wrong server
proxy/intermediary
HTML error page
backend regression
version mismatch
```

These should not be treated as ordinary user mistakes.

---

## P1-4 — Error messages may expose server-controlled content without normalization

For non-401/403 HTTP errors, the client extracts:

```text
error
message
```

from the server response and displays it directly.

This can cause:

```text
unbounded error output
terminal formatting problems
misleading server-controlled messages
sensitive backend details
```

The API boundary should:

```text
limit error size
normalize error codes
sanitize rendering
preserve structured metadata internally
```

---

## P1-5 — 429 rate limiting has no dedicated behavior

HTTP 429 is currently handled as a generic status error:

```text
HTTP 429
```

The client does not appear to model:

```text
Retry-After
```

or a rate-limit error category.

For a production control plane:

```text
429
  ↓
no blind retry loop
  ↓
respect server guidance
  ↓
bounded backoff
  ↓
clear operator message
```

---

## P1-6 — 5xx failures have no bounded recovery policy

Server errors are surfaced as generic errors.

There is no explicit policy for:

```text
retryable 502
retryable 503
retryable 504
non-retryable 500
```

This is dangerous because blindly retrying mutation operations can create duplicates.

The retry policy must be operation-aware.

---

## P1-7 — API calls have no request identity visible to the CLI

There is no clear request/correlation ID generated by the client and propagated through:

```text
request
      ↓
server
      ↓
error
      ↓
audit
```

When an operator reports:

```text
"tenant creation failed"
```

the system should be able to connect:

```text
CLI event
API request
server log
audit event
```

using a correlation identifier.

---

# 5. P1 — Panic and process-boundary findings

## P1-8 — Several internal failures use `unwrap()` or `expect()`

Examples include:

```text
Mutex lock
rustyline initialization
configuration setup
```

The current code uses:

```rust
.lock().unwrap()
```

and:

```rust
.expect(...)
```

These are not necessarily immediately exploitable.

However, a production CLI should define whether an internal failure:

```text
terminates safely
```

or:

```text
panics with a controlled diagnostic
```

The current model is inconsistent.

---

## P1-9 — REPL-level input errors terminate the entire terminal

A `ReadlineError` other than:

```text
Interrupted
Eof
```

causes:

```text
eprintln(...)
break
```

Therefore a single terminal input subsystem error ends the process.

This may be acceptable for unrecoverable terminal I/O failures, but it should be an explicit policy rather than an accidental default.

---

## P1-10 — Login attempts are bounded, but the failure model is not typed

The interactive login allows three attempts.

That is a good boundary.

However:

```text
wrong password
network timeout
server unavailable
rate limited
malformed response
```

all consume the same login attempt loop.

A temporary network outage should not necessarily be treated the same as invalid credentials.

---

# 6. P1 — Local persistence failure boundaries

## P1-11 — Audit storage silently loses data

The local audit writer ignores:

```text
directory creation failure
file open failure
write failure
serialization failure
```

This creates:

```text
privileged operation
      ↓
local audit write fails
      ↓
operation still completes
      ↓
no local evidence
```

The design deliberately avoids crashing the terminal, but silently losing audit data is not acceptable for high-risk operations.

The correct model is:

```text
operation succeeds
      ↓
audit write fails
      ↓
audit delivery state = failed/pending
      ↓
operator notified
```

---

## P1-12 — Local audit reads silently treat missing/corrupt files as empty history

If the audit file cannot be opened, `read()` returns:

```text
Vec::new()
```

If individual lines are malformed, they are silently discarded.

This makes:

```text
corrupted log
```

look identical to:

```text
no audit history
```

That is a serious observability ambiguity.

---

# 7. P2 — Important findings

## P2-1 — No unified error code taxonomy exists

The project should define stable error categories:

```text
AUTH_REQUIRED
AUTH_EXPIRED
AUTH_INVALID
FORBIDDEN
NOT_FOUND
CONFLICT
VALIDATION
RATE_LIMITED
SERVER_ERROR
NETWORK_UNAVAILABLE
TIMEOUT
TLS_FAILURE
PROTOCOL_ERROR
LOCAL_STORAGE_ERROR
USER_CANCELLED
```

The human message can change.

The internal code should remain stable.

---

## P2-2 — No explicit user-facing recovery actions

Errors should tell the operator what to do next.

Example:

```text
AUTH_EXPIRED
→ Run login

RATE_LIMITED
→ Wait 30 seconds

NETWORK_UNAVAILABLE
→ Check server/network

FORBIDDEN
→ Use an authorized account

CONFLICT
→ Refresh resource state
```

The current system mainly prints a message.

---

## P2-3 — No error severity model exists

A useful classification:

```text
INFO
WARNING
ERROR
CRITICAL
```

For example:

```text
user input invalid → WARNING
API 404 → ERROR
TLS trust failure → CRITICAL
audit durability failure after destructive action → CRITICAL
```

---

## P2-4 — No operation outcome model exists

The terminal currently mostly uses:

```text
Ok(())
Err(...)
```

The target should distinguish:

```text
Succeeded
Failed
Cancelled
SucceededWithWarning
UnknownOutcome
```

`UnknownOutcome` is particularly important when:

```text
request may have reached the server
but the client timed out before receiving the response
```

For mutation operations, this prevents unsafe automatic retry.

---

# 8. The most dangerous reliability case

Consider:

```text
tenant:create
      ↓
POST request sent
      ↓
server creates tenant
      ↓
network connection breaks
      ↓
client receives timeout
```

The CLI sees:

```text
failure
```

But the server state is:

```text
success
```

If the operator retries:

```text
tenant:create again
```

the result may be:

```text
duplicate tenant
```

Therefore:

```text
timeout after mutation
```

must not automatically mean:

```text
safe to retry
```

The operation outcome may be:

```text
UNKNOWN
```

This is the foundation for the next Phase 3 parts:

```text
idempotency
request IDs
operation status
reconciliation
```

---

# 9. Target error architecture

```text
┌────────────────────────────┐
│ Transport                  │
│                            │
│ timeout                    │
│ TLS                        │
│ DNS                        │
│ connection refused         │
└──────────────┬─────────────┘
               ↓
┌────────────────────────────┐
│ HTTP                       │
│                            │
│ 401                        │
│ 403                        │
│ 404                        │
│ 409                        │
│ 429                        │
│ 5xx                        │
└──────────────┬─────────────┘
               ↓
┌────────────────────────────┐
│ Protocol / Schema          │
│                            │
│ invalid JSON               │
│ missing required fields    │
│ unexpected response        │
└──────────────┬─────────────┘
               ↓
┌────────────────────────────┐
│ Domain / Operation         │
│                            │
│ validation                │
│ conflict                  │
│ unauthorized              │
│ unknown outcome            │
└──────────────┬─────────────┘
               ↓
┌────────────────────────────┐
│ Presentation               │
│                            │
│ human message              │
│ recovery action            │
│ severity                   │
└────────────────────────────┘
```

---

# 10. Recommended implementation sequence

## Step 1 — Create a typed internal error model

Example conceptual structure:

```text
AxError
├── Authentication
├── Authorization
├── Validation
├── NotFound
├── Conflict
├── RateLimited
├── Server
├── Network
├── Timeout
├── Tls
├── Protocol
├── LocalStorage
├── UserCancelled
└── UnknownOutcome
```

---

## Step 2 — Preserve machine-readable HTTP status

Do not immediately convert:

```text
HTTP 409
```

into:

```text
"Conflict"
```

Keep:

```text
status_code = 409
server_code = ...
message = ...
request_id = ...
```

---

## Step 3 — Add structured server error parsing

Prefer:

```json
{
  "error": {
    "code": "TENANT_SLUG_EXISTS",
    "message": "Tenant already exists",
    "requestId": "..."
  }
}
```

over relying only on:

```json
{
  "message": "..."
}
```

---

## Step 4 — Define retryability explicitly

Each error should answer:

```text
retryable?
```

Examples:

```text
401 → no, re-authenticate
403 → no
409 → no, reconcile
429 → maybe, after delay
500 → maybe
502/503/504 → maybe
timeout after mutation → unknown outcome
```

---

## Step 5 — Define operation outcome semantics

Especially for mutations:

```text
success
failure-before-send
failure-after-send
unknown
```

---

## Step 6 — Make audit failures visible

Do not crash the terminal.

Do not silently erase the failure.

Use:

```text
operation succeeded
audit persistence warning
```

and durable retry for important events.

---

## Step 7 — Add request/correlation IDs

Every request should eventually carry:

```text
X-Request-ID
```

or an equivalent correlation identifier.

The ID should appear in:

```text
CLI errors
local audit
server audit
server logs
```

---

# 11. Cumulative audit additions

The following findings are added to the AX Terminal baseline:

84. API failures are flattened into `String`, destroying machine-readable error identity.
85. Error recovery cannot be correct while failure categories are lost.
86. HTTP success does not guarantee semantic response success.
87. Partial-success states are not modeled.
88. Transport classification relies heavily on fragile error-string matching.
89. API responses are largely untyped arbitrary JSON.
90. Malformed JSON is not classified as a protocol/schema failure.
91. Server-controlled error messages are not normalized or size-bounded.
92. HTTP 429 has no dedicated rate-limit behavior.
93. 5xx errors have no explicit bounded recovery policy.
94. API requests lack a visible correlation/request identity model.
95. Internal `unwrap`/`expect` boundaries are not consistently classified.
96. Some REPL input failures terminate the process without a defined recovery model.
97. Login attempt accounting does not distinguish credential failure from transient infrastructure failure.
98. Local audit write failures are silently lost.
99. Corrupt/missing audit files can appear identical to empty audit history.
100. No stable internal error-code taxonomy exists.
101. Errors lack explicit recovery actions.
102. No error severity model exists.
103. Operation outcomes are not modeled beyond `Ok`/`Err`.
104. Mutation timeout can produce an unknown outcome that is currently treated as ordinary failure.

---

# 12. Status

## Phase 3 — Part 1: COMPLETE

The audit now contains:

```text
Phase 1
    Parts 1–3

Phase 2
    Parts 1–3

Phase 3
    Part 1
```

Cumulative findings:

# 104

The next part should be:

# Phase 3 — Part 2
## Timeouts, Retries, Idempotency, and Recovery

This part will focus on the operational question:

> What happens when the CLI sends an operation, the network fails, and we do not know whether AX-Connect completed the operation?

That is the next major reliability boundary.
