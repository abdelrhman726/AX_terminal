# AX Terminal — Phase 2, Part 3
## Authorization, Permissions, Tenant Isolation, and Privileged Operations Audit

Repository: `abdelrhman726/AX_terminal`
Branch: `main`

---

# 1. Scope

This part inspected:

- `src/commands/mod.rs`
- `src/commands/tenant.rs`
- `src/commands/audit.rs`
- `src/commands/platform.rs`
- `src/parser.rs`
- `src/api.rs`
- `src/session.rs`
- `src/config_store.rs`

Focus:

```text
local role gating
server authorization assumptions
admin/operator separation
tenant access boundaries
tenant ID handling
destructive operations
privileged command execution
cross-tenant risks
audit authorization
```

---

# 2. Executive conclusion

The current CLI has a useful first authorization layer, but it is not yet a complete production authorization model.

The current architecture is:

```text
JWT/session role
      ↓
local role mapping
      ↓
command-level admin_only flag
      ↓
API request
      ↓
server authorization
```

This is acceptable only if the server is always the final authorization authority.

The CLI must never be treated as the security boundary.

The most important architectural risk is that the client has its own interpretation of roles and permissions while also trusting session metadata derived from the authentication response/JWT.

The correct model is:

```text
AX Terminal
    ↓
UX / early rejection
    ↓
AX-Connect API
    ↓
server-side authorization
    ↓
tenant-scoped authorization
    ↓
database enforcement
```

The local CLI should improve operator experience and prevent obvious mistakes.

It must not be the mechanism that makes an unauthorized operation secure.

---

# 3. P0 — Critical findings

## P0-1 — Local authorization is not a security boundary

The command executor decides whether a command is allowed using:

```text
current_role()
      ↓
admin_only
      ↓
allow/reject
```

The role mapping treats:

```text
admin
platform_owner
platform_manager
```

as `Admin`.

Everything else becomes:

```text
Operator
```

This is a client-side policy decision.

A malicious user can modify the CLI binary or bypass the CLI entirely.

Therefore:

```text
CLI denies command
```

does not equal:

```text
operation is secure
```

The AX-Connect server must independently enforce:

```text
identity
role
permission
tenant scope
resource ownership
```

for every privileged operation.

---

## P0-2 — Tenant authorization boundaries are not visible in the client contract

The CLI sends tenant identifiers to endpoints such as:

```text
/api/platform/restaurants/:id
/api/platform/restaurants/:id/terminate
```

The client does not visibly carry:

```text
authorized tenant scope
tenant membership
organization scope
resource ownership
```

The server must ensure:

```text
user
  ↓
organization/platform scope
  ↓
tenant authorization
  ↓
specific tenant ID
```

A valid numeric tenant ID must never be treated as proof of access.

---

## P0-3 — `tenant:use` persists a tenant ID without a local authorization model

`tenant:use` verifies that the API can return the tenant and then persists:

```text
defaultTenant = id
```

The local config stores the tenant ID in:

```text
%USERPROFILE%\.ax\config.json
```

This creates a persistent operator context.

The current design does not clearly distinguish:

```text
tenant selected locally
```

from:

```text
tenant authorized for the current user
```

The selected tenant must be revalidated against the authenticated identity and current server authorization.

A stale selected tenant must not become an implicit authorization context.

---

# 4. P1 — High-priority findings

## P1-1 — Role model is too coarse

The current model is:

```text
Admin
Operator
```

This is not sufficient for a multi-tenant SaaS control plane.

The platform already has more nuanced roles:

```text
platform_owner
platform_manager
platform_developer
restaurant_*
```

but the CLI collapses most of them into:

```text
Operator
```

This creates two problems:

### Problem A — Over-permission risk

A role that is not explicitly recognized may be placed into a broad `Operator` category.

### Problem B — Under-permission risk

Legitimate roles may be denied because the CLI does not understand their capabilities.

The target model should be capability-based:

```text
tenant.read
tenant.create
tenant.update
tenant.terminate
system.health.read
audit.read.own
audit.read.any
```

Roles can then map to permissions.

---

## P1-2 — `admin_only` is too coarse for production authorization

Current command metadata:

```text
requires_auth
admin_only
high_risk
```

This produces:

```text
admin = allowed
everyone else = denied
```

The model should instead be:

```text
required permission set
```

Example:

```text
tenant:list
    requires: tenant.read

tenant:create
    requires: tenant.create

tenant:delete
    requires:
        tenant.terminate
        elevated_authentication
```

This is more scalable than adding more boolean flags.

---

## P1-3 — Tenant delete confirmation is not cryptographically or transactionally bound

The CLI confirms:

```text
Type "DELETE"
```

and then:

```text
GET tenant details
      ↓
display tenant
      ↓
confirm
      ↓
GET/resolve tenant
      ↓
terminate
```

The confirmation is human-readable but not strongly bound to:

```text
tenant ID
tenant slug
tenant version
current server state
```

A race is possible:

```text
confirmation
      ↓
tenant changes
      ↓
termination request
```

The API does send:

```text
confirmSlug
```

which is useful, but the stronger target is:

```text
server-side revalidation
        +
resource version / precondition where appropriate
        +
explicit confirmation target
```

The server must independently verify the operation.

---

## P1-4 — Destructive operation can be triggered through aliases

The command registry includes:

```text
tenant:delete
tdel
```

The high-risk check occurs after command resolution, so the current implementation does protect aliases.

However, aliases increase the risk of:

```text
operator confusion
audit inconsistency
policy bypass in future code
```

Every canonical command should have one stable security identity for:

```text
authorization
audit
metrics
policy
```

The alias should not become the recorded action identity.

---

## P1-5 — Audit identity is client-controlled

The CLI sends audit data containing:

```text
action
status
```

The server receives the authenticated bearer token separately.

The server should derive:

```text
actor
identity
role
tenant scope
timestamp
```

from the authenticated request context.

It must not trust client-provided identity metadata.

The audit event should be server-authoritative.

---

## P1-6 — Audit logging occurs after command execution and is not guaranteed

The executor:

```text
execute operation
      ↓
append local audit
      ↓
fire-and-forget remote audit
```

The remote audit is performed in a background thread and errors are silently discarded.

This creates an accountability gap:

```text
privileged action succeeds
      ↓
audit request lost
      ↓
no server audit record
```

This is especially serious for:

```text
tenant creation
tenant termination
```

The audit system needs a durable delivery model.

---

## P1-7 — The local role can be derived from unverified session metadata

The role is stored in session state and used by local command gating.

Because the authentication lifecycle currently allows JWT payload metadata to influence the local session, the local role may be based on data that has not been independently validated.

This reinforces the Phase 2 Part 1 finding:

```text
local role metadata
≠
server-authoritative authorization
```

---

# 5. P1 — Tenant isolation risks

## P1-8 — Numeric tenant IDs are accepted as the primary resource selector

The CLI parses:

```text
u64
```

and sends the ID to the API.

This is fine as an identifier format.

It is not sufficient as an authorization control.

The server must prevent:

```text
tenant A user
      ↓
changes ID
      ↓
requests tenant B
      ↓
receives/changes tenant B
```

This is the classic object-level authorization problem.

The server must enforce authorization on every request.

---

## P1-9 — `tenant:use` creates persistent cross-session context

The selected tenant is persisted to disk.

Potential lifecycle problem:

```text
User A logs in
      ↓
selects Tenant A
      ↓
logs out
      ↓
User B logs in on same machine
      ↓
Tenant A remains in config
```

The next session can inherit stale tenant context.

The selected tenant must be:

```text
cleared on logout
```

or:

```text
bound to the authenticated user identity
```

Example:

```text
{
  "userId": "...",
  "defaultTenant": 123
}
```

Even then, the server must revalidate access.

---

# 6. P1 — Audit access control

The audit command allows:

```text
audit:log
```

for the current user's own history.

With:

```text
--user email
```

it allows admin access to another user's local audit history.

This model has problems:

1. Email is used as the identity selector.
2. Email may change.
3. The local audit store is not a trusted source of platform-wide audit truth.
4. A client-side admin check cannot replace server-side authorization.

The target should use:

```text
immutable user ID
```

and server-authoritative audit retrieval for platform audit data.

---

# 7. P2 — Important findings

## P2-1 — No explicit permission model exists

The command registry currently encodes authorization using booleans:

```text
requires_auth
admin_only
high_risk
```

This should evolve into a permission model.

---

## P2-2 — No explicit step-up authentication exists

High-risk operations such as tenant termination should eventually support:

```text
recent authentication requirement
```

or:

```text
step-up authentication
```

For example:

```text
normal session
      ↓
tenant termination
      ↓
require recent authentication
      ↓
explicit target confirmation
      ↓
server authorization
      ↓
operation
```

This is stronger than relying only on a session that may have been authenticated hours ago.

---

## P2-3 — No tenant context invalidation lifecycle exists

A selected tenant should be invalidated when:

```text
logout
identity changes
permission changes
tenant is deleted
server rejects access
```

The current architecture does not define this lifecycle.

---

## P2-4 — No explicit policy for cross-tenant operations exists

The platform needs to define whether a user can:

```text
operate one tenant
operate multiple tenants
operate all tenants
create tenants
terminate tenants
```

These should be explicit permissions/scopes.

---

# 8. Target authorization architecture

```text
┌──────────────────────┐
│ AX Terminal          │
│                      │
│ UX pre-check         │
│ permission hint      │
│ confirmation         │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ Authenticated        │
│ identity             │
│                      │
│ user_id              │
│ roles                │
│ permissions          │
│ scopes               │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ AX-Connect API       │
│                      │
│ authenticate         │
│ authorize            │
│ tenant-scope check   │
│ resource check       │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ Database / domain    │
│                      │
│ final invariant      │
│ enforcement          │
└──────────────────────┘
```

---

# 9. Recommended permission model

Instead of:

```text
Admin
Operator
```

define capabilities such as:

```text
tenant.read
tenant.create
tenant.update
tenant.terminate

system.health.read

audit.read.own
audit.read.platform

platform.user.read
platform.user.manage

billing.read
billing.manage
```

Then:

```text
platform_owner
    → all

platform_manager
    → selected platform permissions

platform_developer
    → technical/system permissions

restaurant_admin
    → own tenant scope

restaurant_operator
    → limited own tenant permissions
```

The CLI may use these permissions for early UX validation.

The API remains authoritative.

---

# 10. Required implementation sequence

## Step 1 — Define server-authoritative permission vocabulary

Before changing the CLI authorization model, define the permissions in AX-Connect.

The CLI must not invent a permission system disconnected from the server.

---

## Step 2 — Replace boolean authorization metadata

Move from:

```text
admin_only: bool
```

toward:

```text
required_permissions: &[Permission]
```

---

## Step 3 — Bind tenant context to identity

Do not store only:

```text
defaultTenant: 123
```

Use identity-aware context or clear it on logout.

---

## Step 4 — Make destructive confirmations target-bound

Confirmation should identify:

```text
canonical action
tenant ID
tenant slug/name
```

and the server should revalidate the target.

---

## Step 5 — Add step-up authentication for irreversible actions

Tenant termination should require a stronger authentication freshness policy.

---

## Step 6 — Make server authorization the final boundary

Every API operation must independently enforce:

```text
identity
permission
tenant scope
resource access
```

---

## Step 7 — Make audit records server-authoritative

The server should derive:

```text
actor ID
authenticated identity
timestamp
request ID
resource
action
result
```

The CLI may provide context, but must not define the actor identity.

---

## Step 8 — Design durable audit delivery

Do not silently discard audit failures.

Use:

```text
local durable queue
      ↓
retry
      ↓
server acknowledgement
      ↓
delivery state
```

for events that must not disappear.

---

# 11. Cumulative audit additions

The following findings are added to the AX Terminal baseline:

68. Local CLI authorization is not a security boundary.
69. Tenant authorization scope is not represented in the client contract.
70. `tenant:use` persists tenant context without an explicit identity-bound authorization lifecycle.
71. The role model is too coarse for a multi-tenant SaaS platform.
72. `admin_only` boolean authorization does not scale to capability-based permissions.
73. Destructive confirmation is not strongly bound to a server-validated resource version/state.
74. Alias/canonical command identity needs a stable security and audit identity.
75. Audit identity and authorization must be server-authoritative.
76. Remote audit delivery can silently fail after a privileged operation succeeds.
77. Numeric tenant IDs are resource selectors, not authorization controls.
78. Persisted tenant context can survive identity changes and create stale cross-session context.
79. Audit lookup by email is weaker than immutable user identity.
80. No explicit permission vocabulary is defined for the CLI/API boundary.
81. No step-up authentication/freshness policy exists for irreversible operations.
82. No tenant-context invalidation lifecycle is defined.
83. Cross-tenant operation policy is not explicitly defined.

---

# 12. Status

## Phase 2 — Part 3: COMPLETE

Phase 2 is now complete.

The cumulative security/authentication baseline contains:

```text
Phase 2 Part 1:
    Authentication and JWT lifecycle
    Findings 39–52

Phase 2 Part 2:
    Transport, TLS, and server trust
    Findings 53–67

Phase 2 Part 3:
    Authorization, permissions, tenant isolation
    Findings 68–83
```

Total cumulative findings:

# 83

The next phase should not immediately start changing random code.

The next logical phase is:

# Phase 3 — Reliability and Operational Resilience

Recommended structure:

```text
Part 1 — Error Model and Failure Boundaries
Part 2 — Timeouts, Retries, Idempotency, and Recovery
Part 3 — Audit Durability, Observability, and Operational Safety
```

This preserves the same controlled audit sequence used in Phase 1 and Phase 2.
