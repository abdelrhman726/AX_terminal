# AX Terminal — Phase 2, Part 1
## Authentication Lifecycle and JWT Security Audit

Repository: `abdelrhman726/AX_terminal`
Branch: `main`

## Scope

Inspected:
- `src/main.rs`
- `src/session.rs`
- `src/api.rs`
- `src/env.rs`
- `src/commands/platform.rs`

Focus:
- startup authentication
- interactive login
- `AX_API_TOKEN`
- JWT parsing
- token storage
- expiration
- 401 behavior
- logout
- password exposure
- session lifecycle

---

# Executive conclusion

The authentication design is functional but is not yet production-grade.

Current flow:

```text
Startup
  ↓
Load .env
  ↓
AX_API_TOKEN exists?
  ├─ yes → decode JWT payload → extract role → mark session authenticated
  └─ no  → email/password → login API → receive accessToken → store in memory
  ↓
REPL
  ↓
Attach Bearer token to API requests
```

The central issue is that the CLI treats the presence of an access token as sufficient to establish a local authenticated session.

The server must remain the ultimate authority, but the local authentication lifecycle is too trusting and incomplete.

---

# P0 — Critical findings

## P0-1 — AX_API_TOKEN bypasses the normal authentication lifecycle

If `AX_API_TOKEN` exists, the client decodes the JWT payload, extracts the role, and creates an authenticated session without a validation request.

A syntactically valid JWT payload is not proof of authenticity.

The client must distinguish:

```text
token structurally decodable
```

from:

```text
token cryptographically valid
```

and:

```text
token accepted by AX-Connect
```

The server remains authoritative.

---

## P0-2 — JWT expiration is not checked locally

The JWT parser only extracts `role`.

There is no handling for:

```text
exp
nbf
iat
iss
aud
sub
jti
```

Current behavior:

```text
expired token
  ↓
CLI starts
  ↓
session marked authenticated
  ↓
API request
  ↓
401
  ↓
operator is told to restart
```

The client should detect expiration where possible and clear the session.

---

## P0-3 — No reauthentication lifecycle exists

401 currently produces:

```text
Unauthorized — session expired.
Restart the terminal to log in again.
```

Production target:

```text
AUTHENTICATED
      ↓
TOKEN_EXPIRED
      ↓
REAUTH_REQUIRED
      ↓
AUTHENTICATED
```

The operator should not need to restart the process.

---

# P1 — High-priority findings

## P1-1 — Token is stored as ordinary String data

The session stores:

```text
Option<String>
```

and returns cloned token strings.

Consequences:

- token exists in heap memory
- cloning creates additional copies
- no explicit zeroization
- lifetime is not tightly controlled

Target:

```text
secure token container
  ↓
minimal lifetime
  ↓
explicit clearing
  ↓
zeroization where practical
```

---

## P1-2 — `session::clear()` is not integrated into 401 handling

A clear function exists, but the 401 path does not call it.

Therefore:

```text
401
  ↓
error returned
  ↓
old token remains in memory
```

Correct behavior:

```text
401
  ↓
clear token
  ↓
clear identity metadata
  ↓
mark unauthenticated
  ↓
reauthenticate
```

---

## P1-3 — Role extraction is duplicated

Role comes from:

```text
JWT payload
```

during token-based startup, and from:

```text
login response user.role
```

during interactive login.

The target is one canonical session identity model.

---

## P1-4 — Authentication identity is incomplete

Current session data:

```text
token
email
role
```

Missing or undefined:

```text
user_id
subject
token expiry
issuer
audience
session ID
login timestamp
last validation timestamp
```

At minimum, the production identity model should include:

```text
user_id
email
server-authoritative role/permissions
token expiration
authenticated_at
```

---

## P1-5 — Password-bearing commands can expose credentials in history

The CLI supports:

```text
platform:login --email ... --password ...
```

The REPL stores the complete input line in history.

Therefore the password can be persisted in the history file.

This is a serious credential exposure risk.

Required policy:

```text
platform:login --email user@example.com
Password: [hidden]
```

Never persist password-bearing commands.

---

# P1 — Bearer-token transmission

The API client attaches:

```text
Authorization: Bearer <token>
```

automatically.

The future client needs explicit endpoint/origin trust classification:

```text
public endpoint
authenticated endpoint
privileged endpoint
```

The configurable API destination must be validated before credentials are transmitted.

---

# P1 — API token model is undefined

The project has not formally defined:

```text
token type
token lifetime
issuer
audience
scopes
rotation
revocation
```

For automation, prefer:

```text
short-lived token
  ↓
explicit scope
  ↓
limited audience
  ↓
rotation/revocation
```

rather than an undocumented long-lived JWT beside the executable.

---

# P2 — Important findings

## P2-1 — Logout is not exposed as a complete REPL lifecycle

The API has logout functionality, but the terminal does not provide a complete:

```text
logout
  ↓
optional server revocation
  ↓
local session clear
  ↓
unauthenticated state
```

workflow.

---

## P2-2 — Three login attempts are only local UX protection

The three-attempt limit does not replace server-side:

```text
rate limiting
brute-force detection
account lockout
IP/device abuse controls
```

---

## P2-3 — No explicit authentication state machine exists

Current model:

```text
token exists = authenticated
token absent = unauthenticated
```

Target:

```text
Unauthenticated
      ↓
Authenticating
      ↓
Authenticated
      ↓
TokenExpiring
      ↓
ReauthenticationRequired
      ↓
Unauthenticated
```

---

## P2-4 — Login response is dynamically typed JSON

Authentication relies on:

```text
result["accessToken"]
result["user"]["email"]
result["user"]["role"]
```

The security boundary should eventually use typed response models.

---

# Current vs target

## Current

```text
.env
  ↓
AX_API_TOKEN
  ↓
decode payload
  ↓
extract role
  ↓
mark authenticated
  ↓
send token
```

or:

```text
email + password
  ↓
login endpoint
  ↓
accessToken
  ↓
store String
  ↓
send Bearer token
```

## Target

```text
configuration
  ↓
validate API endpoint
  ↓
authentication request
  ↓
server validates credentials
  ↓
typed authentication response
  ↓
validate token metadata
  ↓
create secure session
  ↓
track expiry
  ↓
attach token only to trusted API origin
  ↓
handle 401
  ↓
clear session
  ↓
reauthenticate
```

---

# Required implementation sequence

1. Define the authentication contract:
   - token type
   - lifetime
   - refresh policy
   - issuer
   - audience
   - scopes/permissions
   - revocation
   - logout semantics

2. Create a typed authentication/session model.

3. Add token metadata validation:
   - structure
   - `exp`
   - `nbf`
   - `iss`
   - `aud`

4. Implement the session lifecycle:
   - login
   - authenticated
   - expired
   - clear
   - reauthenticate
   - logout

5. Remove password exposure through command history.

6. Handle 401 centrally:
   - clear stale session
   - return structured authentication-expired state
   - allow reauthentication

7. Define secure automation authentication.

---

# Cumulative audit additions

These findings are added to the AX Terminal baseline:

39. `AX_API_TOKEN` can establish a local session without cryptographic/server validation.
40. JWT expiration is not checked locally.
41. 401 responses force process restart instead of supporting reauthentication.
42. Tokens are stored as ordinary `String` values and cloned.
43. `session::clear()` is not integrated into the 401 lifecycle.
44. Role extraction is duplicated across authentication paths.
45. Session identity is incomplete.
46. Password-bearing `platform:login` commands can enter persistent readline history.
47. Bearer token injection lacks explicit destination/endpoint trust classification.
48. Token lifetime, issuer, audience, scopes, rotation, and revocation are not formally defined.
49. Logout exists at the API layer but is not exposed as a complete terminal lifecycle.
50. Login attempt control is local UX protection, not server-side abuse protection.
51. No explicit authentication state machine exists.
52. Authentication responses use dynamic JSON instead of typed security-boundary models.

---

# Status

## Phase 2 — Part 1: COMPLETE

Next:

# Phase 2 — Part 2
## API Transport, TLS, Credential Handling, and Server Trust

Scope:

```text
HTTP client
HTTPS enforcement
certificate validation
API origin trust
Bearer token transmission
timeouts
retries
redirects
proxy behavior
TLS failure behavior
```
