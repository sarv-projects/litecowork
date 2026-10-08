# ADR-0020: Runtime Identity Is Installation-Scoped

- **Status:** Accepted for vNext
- **Date:** 2026-10-07

## Context

`RuntimeId` identifies one installed `litecoworkd` Runtime and survives process
restarts. The data model also attached one `workspace_id` directly to Runtime, while the
Operator supports multiple Workspaces. That makes one installation appear to be several
Runtimes and prevents an installation from having explicit, independently revocable
Workspace authorization. It also makes Runtime readiness easy to confuse with Workspace
pairing or Mesh registration.

## Decision

Runtime and RuntimeIncarnation are installation/process scoped. Workspace access is
represented by a separate `RuntimeWorkspaceBinding`, one per Runtime and Workspace.
Bindings record whether the relationship is local enrollment or authenticated Mesh
pairing, its allowed roles, and its independent lifecycle. A local Runtime can be ready
to serve its authenticated Operator API before any Workspace exists; that readiness does
not create a Workspace binding or imply Mesh registration.

Every Workspace-scoped operation must resolve an active binding and independently
authorize the Workspace principal. Runtime use also checks the binding's role for the
operation (for example `EXECUTOR`, `CHANNEL_HOST`, `TRIGGER_HOST`, or `WORKSPACE_HUB`). A
Workspace root grant additionally requires an active binding and a native selection grant
for the exact folder. Revoking a binding blocks new operations and admissions in that
Workspace without changing the installation identity or other Workspace bindings. Runtime
incarnation registration and presence are Mesh-scoped metadata and do not substitute for
binding authorization.

## Consequences

Runtime identity is no longer duplicated per Workspace. Storage and every Workspace-bound
Runtime relation must reference an active `RuntimeWorkspaceBinding` or an equivalent
explicit scope. Additive SQLite v3 creates and backfills bindings and gates Attempt
admission on an active binding. SQLite v4 removes the legacy Runtime-to-Workspace column,
rebuilds Environment, ChannelHostAssignment, AutomationOccurrence, and AutomationCursor
tables, and adds role-scoped guards. V4 preserves the v1-v3 migration sources and checksums
and validates foreign keys before committing the rebuild. This storage migration does not
itself implement local enrollment, endpoint readiness, authenticated OS-peer IPC, or
persistent folder-root grants.

## Alternatives considered

- **One Runtime per Workspace:** rejected because it misrepresents one daemon installation,
  duplicates incarnation and process ownership, and obscures revocation scope.
- **Keep a single `workspace_id` on Runtime:** rejected because an Operator may manage
  multiple Workspaces and authorization must be independently scoped.
