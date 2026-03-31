# ADR-001: How to Pass Active Scope in Admin Requests

**Status:** Pending decision  
**Affects:** Multi-tenant admin roles & invites feature

---

## Context

Admin users can belong to multiple organizations and projects with different roles (`owner` / `admin`).
Every protected admin endpoint needs to know *which org or project* the request is acting on behalf of,
so it can enforce the correct permissions and filter data accordingly (metrics, logs, resource creation).

This document describes the two candidate approaches and the trade-offs between them.

---

## The Decision

> **How does the server know the active org/project scope for a given admin request?**

---

## Option A — Request Header (`X-Org-Id` / `X-Project-Id`)

The JWT proves *who* the admin is. A request header selects *which scope* they are acting in.

```
Authorization: Bearer <jwt>
X-Org-Id: 019571b0-...        ← active org scope
X-Project-Id: 019571b1-...    ← active project scope (optional, more specific)
```

The middleware reads the header, queries `admin_org_memberships` or `admin_project_memberships`
to confirm membership, then injects the resolved scope into the request extensions.

**Pros**
- Admin can switch orgs/projects without re-authenticating
- Token stays small and stateless — no scope baked in
- Easy to test: change the header, get a different scope
- Natural fallback: if no header is sent, derive scope from the union of all the admin's memberships

**Cons**
- One DB query per request to validate the membership claim in the header
- Clients must remember to send the header; missing header needs a defined fallback
- Slightly more client-side coordination

---

## Option B — Scope Baked into the JWT

The token itself encodes `org_id` / `project_id` at login time, or via a dedicated
"select scope" endpoint that issues a narrower scoped token.

```json
{
  "sub": "<admin_id>",
  "org_id": "019571b0-...",
  "project_id": "019571b1-...",
  "exp": 1234567890
}
```

**Pros**
- No DB hit to verify scope — the token is self-contained and cryptographically signed
- Scope is always explicit; no "missing header" ambiguity
- Clear audit trail — the token used carries the scope it was issued for

**Cons**
- Admin must re-auth (or call a "switch scope" endpoint) to change org/project
- If membership is revoked, the old token remains valid until expiry
- Requires short TTLs or a revocation mechanism to avoid stale access
- Adds a "select scope" flow that increases API surface for no benefit in the MVP

---

## Recommendation

**Option A (headers)** is the better fit for this project's MVP because:

1. The JWT is already short-lived (5 min). A DB validation hit per request is acceptable at this scale.
2. The permission matrix requires admins to work across multiple orgs/projects; headers allow
   context switching without re-authentication.
3. Avoids a "select scope" endpoint and the revocation complexity that Option B implies.
4. The no-header fallback (union of all memberships) enables a natural "global dashboard" view
   for metrics and logs without extra client work.

---

## Where This Decision Is Used

| Layer | File | What changes |
|-------|------|-------------|
| JWT claims | `src/jwt.rs` | No change — token stays identity-only (`admin_id`, `user_type`) |
| Middleware | `src/admin.rs` | After validating the JWT, also read `X-Org-Id`/`X-Project-Id` headers and resolve membership |
| Authorization guards | `src/admin/authorization.rs` *(new)* | Reusable extractors that inject resolved `AdminScope` into handlers |
| All protected admin handlers | `src/admin/router.rs` | Accept `AdminScope` extractor; use it to filter queries and check permissions |

### `AdminScope` type (proposed)

```rust
pub enum AdminScope {
    Org { org_id: Uuid, role: Role },
    Project { project_id: Uuid, org_id: Uuid, role: Role },
    /// No header sent — union of all the admin's memberships
    Global { org_ids: Vec<Uuid>, project_ids: Vec<Uuid> },
}
```

---

## Features This Decision Unlocks

### Immediately (once the middleware is in place)
- **Scoped `POST /admin/organizations`** — auto-assigns the caller as `owner` of the new org
- **Scoped `POST /admin/projects`** — enforces that the caller is `owner|admin` of the target org
- **Scoped `POST /admin/applications`** — enforces that the caller is `owner|admin` of the target project
- **Scoped `GET /admin/metrics`** — returns metrics only for orgs/projects the caller can see
- **Scoped `GET /admin/logs`** — returns logs only for events within the caller's visible scope

### Invite flow (depends on scope enforcement)
- `POST /admin/invites` — create invite for an org or project (requires `owner|admin` of that scope)
- `POST /admin/invites/{id}/accept` — invitee accepts; membership row is inserted
- `POST /admin/invites/{id}/decline` — invitee declines; status updated
- `POST /admin/invites/{id}/revoke` — issuer revokes a pending invite

### Future (outside MVP)
- Audit log entries can be stamped with the active scope at the time of the action
- Scope-aware rate limiting (per-org quotas)
- Delegated admin: an org-level admin granting project-level access without owner involvement

---

## Further Reading

- **RFC 8693** — OAuth 2.0 Token Exchange (formalizes the "select scope → scoped token" pattern of Option B)
- **RFC 9068** — JWT Profile for OAuth 2.0 Access Tokens (what claims belong in a token)
- **"Zanzibar: Google's Consistent, Global Authorization System"** (USENIX 2019) — the paper behind relationship-based access control (ReBAC), which is what the membership model is approaching
- **"API Security in Action" — Neil Madden** (Manning, 2020) — practical chapters on token design, scope, and capability-based access
