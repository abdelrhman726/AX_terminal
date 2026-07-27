# AX Terminal — Phase 3, Part 2
## Timeouts, Retries, Idempotency, and Recovery Audit

Repository: `abdelrhman726/AX_terminal`
Branch: `main`

---

# 1. Scope

This part inspected the operational recovery model across:

- `src/api.rs`
- `src/commands/mod.rs`
- `src/commands/tenant.rs`
- `Cargo.toml`

Focus:

```text
timeouts
retry behavior
mutation safety
idempotency
unknown outcomes
duplicate operations
backoff
rate limits
connection failures
recovery after partial failure
```

---

# 2. Executive conclusion

The current AX Terminal has a single global HTTP timeout:

```text
10 seconds
```

and no actual retry or idempotency architecture.

The effective behavior is:

```text
request
   ↓
wait up to 10 seconds
   ↓
success → return
failure → return error
```

This is simple, but unsafe for a control plane that performs mutations.

The most important distinction is:

```text
safe retry
```

versus:

```text
unsafe retry
```

A read request and a tenant-creation request cannot be treated the same way.

The target architecture should be:

```text
operation
    ↓
classify: read / idempotent mutation / non-idempotent mutation
    ↓
attach request identity
    ↓
send with bounded timeout
    ↓
failure classification
    ↓
retry only when safe
    ↓
if outcome unknown:
    reconcile before retrying
```

---

# 3. P0 — Critical findings

## P0-1 — No retry policy exists

The API client performs one request and returns one result.

There is no:

```text
retry count
backoff
jitter
Retry-After handling
retry classification
```

Therefore:

```text
temporary network failure
      ↓
immediate command failure
```

This increases operator friction.

More importantly, adding naive retries later would be dangerous because mutation requests could be duplicated.

---

## P0-2 — Mutation requests have no idempotency key

Current tenant creation:

```text
POST /api/platform/restaurants
```

uses only:

```json
{
  "name": "...",
  "slug": "..."
}
```

There is no client-generated operation identity.

The sequence:

```text
POST
  ↓
server creates tenant
  ↓
response is lost
  ↓
CLI reports timeout
```

creates an unknown outcome.

A retry can create a duplicate resource.

Production mutation requests need an idempotency mechanism such as:

```text
Idempotency-Key: <unique operation ID>
```

with server-side support.

The important point is that this cannot be solved by the CLI alone.

The AX-Connect API must also understand and persist the idempotency key.

---

## P0-3 — Timeout after a mutation is treated as ordinary failure

The API client has a 10-second global timeout.

A timeout is returned as:

```text
Request timed out after 10 seconds.
```

The caller has no indication whether:

```text
the request never left the machine
```

or:

```text
the server received and completed it
```

This is a critical semantic difference.

For mutations:

```text
timeout ≠ definitely failed
```

The correct result can be:

```text
UNKNOWN_OUTCOME
```

---

## P0-4 — No reconciliation mechanism exists

After an unknown mutation outcome, the CLI has no mechanism such as:

```text
operation status
```

or:

```text
find resource by operation ID
```

or:

```text
query resource by deterministic idempotency key
```

Therefore the operator must manually guess whether to retry.

The correct recovery path should be:

```text
request timed out
      ↓
mark outcome UNKNOWN
      ↓
reconcile with server
      ↓
if operation exists:
    report success
if operation definitely did not happen:
    safe retry
if still uncertain:
    keep UNKNOWN
```

---

# 4. P1 — High-priority findings

## P1-1 — One timeout value is used for every operation

The API agent configures:

```text
10 seconds
```

for every request.

This is not ideal.

Different operations have different latency expectations:

```text
health check       → short
tenant list        → moderate
tenant create      → longer
tenant terminate   → longer
```

A global timeout makes the policy too coarse.

The target should distinguish:

```text
connect timeout
read timeout
total operation deadline
```

where supported by the HTTP stack.

---

## P1-2 — No connect timeout versus read timeout distinction

A request can fail because:

```text
server cannot be reached
```

or because:

```text
server accepted the request but is taking too long
```

These are different signals.

The current implementation exposes only a general timeout outcome.

A production control plane should preserve the distinction where possible:

```text
connect timeout
TLS handshake timeout
response/read timeout
overall deadline exceeded
```

---

## P1-3 — No exponential backoff

If retries are later added, the system must avoid:

```text
retry immediately
retry immediately
retry immediately
```

The target should use bounded exponential backoff with jitter:

```text
attempt 1 → short delay
attempt 2 → longer delay
attempt 3 → longer delay
```

with a maximum ceiling.

This is especially important during:

```text
server overload
network instability
rate limiting
```

---

## P1-4 — No `Retry-After` support

HTTP 429 and some 503 responses may communicate when the client should retry.

The current client has no dedicated handling for:

```text
Retry-After
```

Therefore the CLI cannot respect server-directed recovery timing.

The target policy should be:

```text
Retry-After present
    ↓
parse safely
    ↓
cap maximum wait
    ↓
retry only if operation is safe
```

---

## P1-5 — No distinction between retryable and non-retryable failures

The current architecture returns strings.

There is no internal policy such as:

```text
retryable = true/false
```

Examples:

```text
401 → re-authenticate, do not retry blindly
403 → do not retry
404 → usually do not retry
409 → reconcile / refresh
429 → maybe retry after delay
500 → policy-dependent
502 → often retryable
503 → often retryable
504 → potentially unknown for mutation
timeout → potentially unknown for mutation
```

This classification must exist before a retry engine is implemented.

---

## P1-6 — No operation-aware retry policy

A retry engine cannot only ask:

```text
Was the HTTP request unsuccessful?
```

It must also ask:

```text
What operation was attempted?
```

For example:

```text
GET tenant list
```

can usually be retried.

But:

```text
POST tenant create
```

cannot safely be retried unless idempotency is guaranteed.

The retry policy therefore belongs at the operation boundary, not only in the low-level HTTP client.

---

## P1-7 — Tenant termination has no client-side idempotency strategy

Termination is a destructive mutation.

The current flow:

```text
fetch tenant
      ↓
confirm DELETE
      ↓
POST terminate
```

does not attach an operation identity.

If the terminate request succeeds server-side but the response is lost, the user can be left with:

```text
tenant may be terminated
```

but:

```text
CLI reports failure
```

A retry may produce:

```text
404
```

or another state-dependent error.

The terminal needs a clear interpretation of:

```text
already terminated
```

versus:

```text
termination never occurred
```

---

# 5. P1 — Duplicate operation risks

## P1-8 — `tenant:create` can duplicate on retry

Current creation sends:

```text
name
slug
```

and relies on the server response.

If the first request succeeds but the response is lost:

```text
server state:
    tenant exists

CLI state:
    operation failed
```

A manual retry can create:

```text
duplicate tenant
```

or cause a conflict.

The correct design is:

```text
client operation ID
      ↓
server idempotency record
      ↓
same key + same request
      ↓
return original result
```

---

## P1-9 — No request ID is used as an operation identity

A correlation ID is useful for tracing.

An idempotency key is used for mutation deduplication.

These are related but not identical:

```text
X-Request-ID
    → trace this specific HTTP request

Idempotency-Key
    → deduplicate this logical operation
```

The architecture should not incorrectly use one as a substitute for the other.

A logical operation may have:

```text
one idempotency key
multiple HTTP attempts
multiple request IDs
```

---

## P1-10 — No client-side operation lifecycle

There is no model such as:

```text
created
sending
accepted
completed
failed
unknown
reconciled
```

This makes it difficult to reason about:

```text
retry
resume
reconcile
audit
```

The target should model logical operations independently from individual HTTP attempts.

---

# 6. P1 — Recovery and user experience

## P1-11 — Error output does not tell the operator whether retry is safe

The current user-facing model is effectively:

```text
❌ Request timed out after 10 seconds.
```

The operator needs more precise guidance:

```text
The request outcome is unknown.
Do not retry yet.
Attempting to reconcile with AX-Connect...
```

or:

```text
The request definitely failed before reaching the server.
Retrying is safe.
```

The recovery action should be part of the error model.

---

## P1-12 — No automatic reconciliation after uncertain mutation failure

A production CLI should not immediately abandon a mutation after:

```text
timeout
connection reset after send
gateway timeout
```

Instead:

```text
mutation uncertain
      ↓
reconciliation attempt
      ↓
server state check
```

The exact reconciliation method depends on AX-Connect API design.

Possible mechanisms:

```text
GET operation/{id}
GET resource by idempotency key
GET resource by deterministic slug
server-side idempotency record lookup
```

---

## P1-13 — Deterministic slug alone is not enough

The create operation sends:

```text
slug = name
```

A deterministic slug can help find a resource after an unknown outcome.

However, it is not a complete substitute for idempotency because:

```text
two legitimate requests may intentionally use the same slug
```

and:

```text
resource lookup by slug may be ambiguous
```

Therefore:

```text
deterministic business identifier
```

and:

```text
idempotency key
```

should be separate concepts.

---

# 7. P2 — Important findings

## P2-1 — No bounded retry budget

The system has no explicit budget such as:

```text
maximum attempts
maximum elapsed time
maximum total retry delay
```

A future retry engine must have all three.

---

## P2-2 — No jitter strategy

Even if exponential backoff is implemented, identical clients can retry simultaneously.

The target should add randomized jitter to reduce synchronized retry storms.

---

## P2-3 — No circuit breaker or temporary outage state

Repeated failures to the same AX-Connect endpoint currently produce independent failures.

A future client may benefit from a lightweight circuit-breaker concept:

```text
healthy
   ↓ failures
degraded
   ↓ repeated failures
temporarily unavailable
```

This is not necessarily required in the first implementation, but should be considered for repeated operator workflows.

---

## P2-4 — No offline queue for failed remote audit

Remote audit is fire-and-forget and silently discards failures.

This means retryable audit events are not queued.

A durable local outbox is the stronger design:

```text
operation
    ↓
local audit/outbox
    ↓
remote delivery
    ↓
success → mark delivered
failure → retry later
```

This is especially important because audit events are part of the control-plane evidence trail.

---

# 8. Recommended target architecture

```text
┌─────────────────────────────┐
│ Logical Operation           │
│                             │
│ operation_id                │
│ idempotency_key             │
│ command                     │
│ target                      │
└──────────────┬──────────────┘
               ↓
┌─────────────────────────────┐
│ Operation Policy            │
│                             │
│ read?                       │
│ mutation?                   │
│ retryable?                  │
│ reconcile strategy?         │
└──────────────┬──────────────┘
               ↓
┌─────────────────────────────┐
│ HTTP Attempt                │
│                             │
│ request_id                  │
│ timeout                     │
│ headers                     │
└──────────────┬──────────────┘
               ↓
        ┌──────┴──────┐
        │             │
      Success       Failure
        │             │
        │       classify failure
        │             │
        │       ┌─────┴─────┐
        │       │           │
        │     Retry       Unknown
        │       │           │
        │    bounded     Reconcile
        │     backoff        │
        │       │       ┌────┴────┐
        │       │       │         │
        │       │   Confirmed   Still
        │       │   success    unknown
        │       │       │         │
        └───────┴───────┴─────────┘
                    ↓
              final outcome
```

---

# 9. Recommended implementation order

## Step 1 — Define logical operation identity

For each mutation:

```text
operation_id
idempotency_key
```

must be created before the first request.

---

## Step 2 — Add request correlation

Each HTTP attempt gets:

```text
request_id
```

The relationship becomes:

```text
logical operation
      ↓
idempotency key
      ↓
HTTP attempt 1 → request ID A
HTTP attempt 2 → request ID B
```

---

## Step 3 — Add typed failure classification

Before retries:

```text
Timeout
TransportUnavailable
RateLimited
ServerError
Conflict
Unauthorized
ProtocolError
```

must be distinguishable.

---

## Step 4 — Implement retry only for safe cases

Start with:

```text
GET requests
```

Then add mutation retry only after server-side idempotency exists.

---

## Step 5 — Add bounded backoff

Use:

```text
maximum attempts
maximum total elapsed time
maximum delay
jitter
```

---

## Step 6 — Add reconciliation

For uncertain mutation outcomes:

```text
timeout
gateway timeout
connection reset after send
```

the CLI should reconcile before suggesting a retry.

---

## Step 7 — Add durable audit outbox

Remote audit delivery should become:

```text
local durable event
      ↓
delivery worker
      ↓
retry
      ↓
delivered
```

rather than:

```text
spawn thread
      ↓
send
      ↓
discard error
```

---

# 10. Cumulative audit additions

The following findings are added to the AX Terminal baseline:

105. No retry policy exists.
106. Mutation requests have no idempotency key.
107. Mutation timeouts are treated as ordinary failures instead of unknown outcomes.
108. No reconciliation mechanism exists after uncertain mutations.
109. One global 10-second timeout is used for all operations.
110. Connect and response/read timeout semantics are not distinguished.
111. No exponential backoff exists.
112. No jitter strategy exists.
113. `Retry-After` is not supported.
114. Retryable and non-retryable failures are not classified.
115. Retry policy is not operation-aware.
116. Tenant termination has no client-side idempotency strategy.
117. Request correlation ID and idempotency key are not modeled as separate concepts.
118. No logical operation lifecycle exists independently from HTTP attempts.
119. Error output does not communicate whether retry is safe.
120. No automatic reconciliation occurs after uncertain mutation failures.
121. Deterministic tenant slugs are not a substitute for idempotency.
122. No bounded retry budget exists.
123. No circuit-breaker or temporary-outage state exists.
124. Failed remote audit events have no durable retry queue/outbox.

---

# 11. Status

## Phase 3 — Part 2: COMPLETE

Current cumulative baseline:

```text
Phase 1
    Parts 1–3

Phase 2
    Parts 1–3

Phase 3
    Parts 1–2
```

Cumulative findings:

# 124

The next part should be:

# Phase 3 — Part 3
## Concurrency, State Consistency, and Process Lifecycle

That part will inspect:

```text
background threads
shared session state
concurrent commands
audit ordering
shutdown behavior
process crashes
stale state
```

and determine whether AX Terminal can remain correct when multiple operations or background tasks overlap.
