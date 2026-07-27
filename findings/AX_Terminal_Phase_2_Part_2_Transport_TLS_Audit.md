# AX Terminal — Phase 2, Part 2
## API Transport, TLS, Credential Handling, and Server Trust Audit

Repository: `abdelrhman726/AX_terminal`
Branch: `main`

---

# 1. Scope inspected

This part focused on the network trust boundary:

- `src/api.rs`
- `src/env.rs`
- `src/main.rs`
- `src/session.rs`
- `src/config_store.rs`
- `Cargo.toml`

Inspected areas:

```text
HTTP client construction
TLS support
HTTPS enforcement
certificate validation
API origin trust
Bearer token transmission
timeouts
retries
redirect behavior
proxy behavior
transport error handling
credential exposure
```

---

# 2. Executive conclusion

The current AX Terminal transport layer is **not production-ready for remote administration**.

The central architectural problem is explicit in the dependency configuration:

```text
ureq = { version = "2.9", default-features = false, features = ["json"] }
```

The project disables the TLS features and describes the client as:

```text
no TLS (localhost only)
```

At the same time, the API URL is configurable through:

```text
AX_API_URL
```

and bearer credentials are automatically attached to requests.

This creates a dangerous mismatch:

```text
configurable remote destination
        +
bearer token transmission
        +
no TLS
        =
credentials can be exposed on an untrusted network
```

For the AX-Connect architecture, especially with the Ubuntu server and external operator access, this is a **P0 transport security blocker**.

---

# 3. P0 — Critical findings

## P0-1 — TLS is disabled

The dependency configuration explicitly disables the TLS features.

The client is therefore designed for:

```text
http://
```

rather than:

```text
https://
```

The current default is also:

```text
http://localhost:4000
```

This is acceptable only for tightly controlled local development.

It is not acceptable for:

```text
Windows operator
        ↓
Internet / LAN
        ↓
AX-Connect server
```

or:

```text
AX Terminal
        ↓
public management endpoint
        ↓
AX-Connect
```

---

## P0-2 — Bearer tokens can be transmitted over plaintext HTTP

The API client automatically injects:

```text
Authorization: Bearer <token>
```

into requests.

Because the base URL is configurable, the current design can become:

```text
AX_API_URL=http://some-remote-host
        ↓
Bearer token
        ↓
plaintext HTTP
```

Anyone able to observe the network path may be able to capture the credential.

This is a direct credential confidentiality failure.

---

## P0-3 — No production HTTPS enforcement exists

There is no visible policy equivalent to:

```text
production API URL must use HTTPS
```

The client accepts the configured URL.

The target policy should be:

```text
localhost / explicit development mode
        ↓
HTTP allowed

remote / production mode
        ↓
HTTPS required
```

The client should fail closed rather than silently downgrade to HTTP.

---

# 4. P0/P1 — Server trust is not explicitly defined

The client currently trusts the configured base URL.

There is no explicit trust model for:

```text
who controls the API hostname
who controls DNS
whether the certificate is valid
whether the endpoint is the intended AX-Connect server
whether redirects can change the destination
```

For a privileged CLI, the trust chain must be:

```text
configured endpoint
        ↓
DNS resolution
        ↓
TLS certificate validation
        ↓
hostname verification
        ↓
trusted HTTPS connection
        ↓
Bearer credential transmission
```

The token must not be sent before the transport trust boundary is established.

---

# 5. P1 — HTTP client is recreated for every request

`agent()` constructs a new `ureq::Agent` for each request.

Current pattern:

```text
every request
    ↓
new Agent
    ↓
new request
```

This is inefficient and makes centralized transport policy harder.

A production client should normally own one configured transport agent:

```text
ApiClient
├── base URL
├── configured HTTP agent
├── timeout policy
├── TLS policy
└── request policy
```

This also makes future connection reuse and consistent transport configuration easier.

---

# 6. P1 — Timeout policy is incomplete

The client configures:

```text
timeout = 10 seconds
```

but the project does not define separate policies for:

```text
connect timeout
read timeout
write timeout
overall request deadline
```

A single generic timeout is a weak operational policy.

Different operations may need different budgets:

```text
health check
    → short

login
    → moderate

tenant provisioning
    → longer

audit delivery
    → background/retry policy
```

The transport layer should make these decisions explicit.

---

# 7. P1 — No retry policy exists

The client has no structured retry policy.

This means transient failures are handled the same way as permanent failures.

The system should distinguish:

```text
retryable
├── connection reset
├── temporary network failure
├── selected 5xx responses
└── possibly 429 with server guidance

non-retryable
├── 400
├── 401
├── 403
├── 404
└── validation errors
```

Retries must be:

```text
bounded
exponential backoff
jittered
idempotency-aware
```

Do not automatically retry destructive operations.

---

# 8. P1 — Redirect policy is not explicit

The transport layer does not define an explicit redirect policy.

For a privileged API client, redirects deserve special treatment.

A dangerous scenario would be:

```text
trusted API endpoint
        ↓
redirect
        ↓
different endpoint
        ↓
credential transmission
```

The client should explicitly define:

```text
redirects allowed?
redirects across hosts?
redirects from HTTPS to HTTP?
Authorization header forwarding?
```

The safest default for an administrative API is:

```text
no cross-origin redirects
no HTTPS → HTTP downgrade
do not forward bearer credentials to a different origin
```

---

# 9. P1 — API origin validation is insufficient

The current API URL is read from environment configuration.

The client should validate:

```text
scheme
host
port
path
```

before constructing requests.

At minimum:

```text
production:
    https:// required

development:
    http://localhost allowed

remote HTTP:
    rejected
```

The URL should be normalized once and stored as a validated endpoint object rather than repeatedly concatenating strings.

---

# 10. P1 — Bearer credentials are attached generically

The helper:

```text
inject_auth(...)
```

automatically attaches the current bearer token.

This is convenient but creates a risk of accidental credential transmission to endpoints that should not receive it.

The transport layer should distinguish:

```text
public request
authenticated request
```

and ideally:

```text
privileged request
```

Credential attachment should be deliberate.

---

# 11. P1 — Transport errors are string-matched

The client classifies transport failures by searching text such as:

```text
"Connection refused"
"actively refused"
"os error 10061"
"timed out"
```

This is fragile because:

- messages vary by platform
- messages vary by library version
- localization can change text
- different errors may have similar wording

The transport layer should classify structured error types where available, then convert them into user-facing messages at the UI boundary.

---

# 12. P1 — Proxy behavior is undefined

The project does not define how proxy configuration should work.

This matters because:

```text
corporate network
enterprise proxy
VPN
system proxy
```

can affect API connectivity.

The client should have an explicit policy:

```text
direct connection
configured proxy
system proxy
```

rather than relying on accidental behavior.

For AX-Connect administration, proxy support should not silently route privileged traffic through an unexpected intermediary.

---

# 13. P1 — Certificate trust policy is not defined

Once TLS is enabled, the project must explicitly define:

```text
system trust store
custom CA support?
certificate pinning?
self-signed certificates?
development certificates?
```

Recommended production default:

```text
system certificate trust store
        +
hostname verification
        +
no certificate bypass
```

Never add:

```text
accept_invalid_certs = true
```

as a convenient fix.

For local development, use a properly trusted development certificate or a clearly isolated development exception.

---

# 14. P2 — Configuration and transport are too tightly coupled

The environment loader produces:

```text
String api_url
```

The API client then accepts the raw string.

The better architecture is:

```text
raw configuration
        ↓
validation
        ↓
normalized configuration
        ↓
ApiEndpoint
        ↓
ApiClient
```

This prevents invalid transport configuration from reaching the request layer.

---

# 15. P2 — Localhost is not automatically a complete trust boundary

The current design assumes:

```text
localhost = safe
```

This is often reasonable for development, but not an absolute security guarantee.

A local malicious process could potentially interact with a local service.

The production architecture should not rely on:

```text
HTTP localhost
```

for privileged operations unless the threat model explicitly accepts it.

---

# 16. Recommended target transport architecture

```text
┌─────────────────────┐
│ AX Terminal         │
│                     │
│ Raw config          │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ Config validation   │
│                     │
│ URL scheme          │
│ Host                │
│ Port                │
│ Environment         │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ Trusted Endpoint    │
│                     │
│ HTTPS required      │
│ Host verified       │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ Shared HTTP Agent   │
│                     │
│ timeout policy      │
│ TLS policy          │
│ redirect policy     │
│ proxy policy        │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ Request Policy      │
│                     │
│ public              │
│ authenticated       │
│ privileged           │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ AX-Connect API      │
└─────────────────────┘
```

---

# 17. Required implementation sequence

## Step 1 — Define environments

Explicitly distinguish:

```text
development
staging
production
```

Then define transport policy per environment.

Example:

```text
development:
    localhost HTTP allowed

staging:
    HTTPS required

production:
    HTTPS required
```

---

## Step 2 — Enable TLS

The HTTP client must support properly validated HTTPS.

Do not disable certificate validation.

---

## Step 3 — Create validated endpoint configuration

Replace:

```text
String
```

with a validated endpoint abstraction.

Validation should occur before authentication.

---

## Step 4 — Enforce no downgrade

Reject:

```text
production → HTTP
HTTPS → HTTP redirect
```

---

## Step 5 — Create one shared transport agent

The API client should own the configured transport object.

---

## Step 6 — Define timeout policies

Separate:

```text
connect
read
write
overall
```

where supported by the HTTP library.

---

## Step 7 — Add bounded retry policy

Only retry:

```text
safe/idempotent operations
```

or operations using explicit idempotency protection.

Never blindly retry:

```text
tenant termination
tenant creation
financial operations
```

---

## Step 8 — Define redirect policy

Recommended:

```text
no cross-origin redirects
no HTTPS downgrade
no credential forwarding to changed origin
```

---

## Step 9 — Define proxy policy

Make proxy behavior explicit and observable.

---

## Step 10 — Add transport tests

At minimum:

```text
HTTP production rejection
HTTPS acceptance
invalid certificate rejection
hostname mismatch rejection
redirect downgrade rejection
cross-origin redirect rejection
timeout classification
retry behavior
401 behavior
403 behavior
```

---

# 18. Cumulative audit additions

The following findings are added to the AX Terminal baseline:

53. TLS is disabled in the HTTP client.
54. Bearer tokens can be transmitted over plaintext HTTP when configured against a remote endpoint.
55. No production HTTPS enforcement exists.
56. API server trust and origin validation are not explicitly defined.
57. HTTP client agents are recreated for every request.
58. Timeout policy is not granular or operation-aware.
59. No structured retry policy exists.
60. Redirect behavior and credential forwarding are not explicitly controlled.
61. API URL validation is insufficient.
62. Bearer credential injection is too generic.
63. Transport errors are classified using fragile string matching.
64. Proxy behavior is undefined.
65. Certificate trust policy is not defined.
66. Raw configuration is passed directly into the transport layer without a validated endpoint abstraction.
67. The architecture implicitly treats localhost as a complete trust boundary.

---

# 19. Status

## Phase 2 — Part 2: COMPLETE

This part covered:

```text
HTTP client
TLS
HTTPS
certificate trust
API origin
Bearer token transmission
timeouts
retries
redirects
proxy behavior
transport errors
```

The next audit is:

# Phase 2 — Part 3
## Authorization, Permissions, Tenant Isolation, and Privileged Operations

Scope:

```text
local role gating
server authorization assumptions
admin/operator separation
tenant access boundaries
tenant ID handling
destructive operations
privileged command execution
cross-tenant risks
```

After Phase 2 Part 3, Phase 2 will be complete and we will have a full security/authentication baseline before implementation begins.
