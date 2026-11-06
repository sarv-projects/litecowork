# Data model - Native Agent Install, Auth and Binding

**Feature:** 002 | **Status:** DRAFT | **Updated:** 2026-11-06

Identities and lifecycles stay in the canonical owner documents. This file lists records this slice reads or commits. It does not authorize new tables by itself.

## Identities and records

- **AgentModule / trusted module manifest** — registered adapter identity, version and protocol authority. Not inferred from an ACP listing or a command name.
- **AgentRegistryEntry** — distribution metadata cache; source, license and registry freshness; not executable or authentication authority.
- **AgentInstallationObservation** — Runtime-local installation/detection/version observation, tied to exact OS/architecture/Runtime incarnation and redacted executable identity.
- **AgentProfile** — module/Runtime-specific profile/configuration and native auth observation; no raw credential material.
- **AgentControlDescriptor** — time-bounded, version-pinned, non-secret capabilities/options and native-config/credential-slot declarations.
- **AgentBinding** — Workspace-scoped durable eligibility/default selection; disabled on create; not a session handle, permission grant or proof of session readiness.
- **AgentLifecycleOperation / receipt** — Runtime-local idempotent install/update/repair operation when a qualified module lifecycle exists; not a domain Task.
- **AgentSessionHostBinding** — opaque Runtime-local native session handle binding; never exposed as a normal UI field or replicated.

## Relations

An AgentModule may have many Runtime-specific installation observations. One installation may be associated with a module/Runtime-specific AgentProfile. A Workspace may have multiple AgentBindings pointing to eligible profiles/modules. An active native AgentSession is created by the Conversation/Task owner and pins the selected module/profile/binding version; changing Settings defaults affects future admissions only.

AgentBinding is the Workspace-scoped aggregate. Registry cache, installation observations, control descriptors, lifecycle operations and opaque native session handles are Runtime-local and do not carry a Workspace id unless a canonical owner document explicitly says otherwise. Runtime-local secrets, handles and private locators are excluded from replicated events and ordinary backups. Native-auth secrets remain owned by the native harness/OS credential store; LiteCowork SecretStore applies only to descriptor-declared LiteCowork-owned credential slots.

## Lifecycle and consistency

Install and update begin an idempotent lifecycle operation, perform the exact module-approved native call outside SQLite, then persist the Runtime-local observation and reconcile the receipt. An unknown result after a lost response is not a failed install; re-probe and reconcile before retry. Preserve the last known working version until replacement qualification succeeds. Uninstall is restricted to the exact installation owned by this lifecycle service and must not delete a user-managed CLI or its configuration.

Native sign-in and credential submission are separate from installation and binding. A canceled/failed sign-in preserves existing binding; credential bytes never enter frontend state after submission, SQLite, events, logs or diagnostics. An auth observation is redacted and time-bounded.

Binding creation, configuration, enablement and disablement use Workspace authorization, version compare-and-swap and idempotency. Create disabled; enable requires fresh compatible Runtime offer plus fresh exact AgentControlDescriptor. Disable blocks future admissions only; it does not claim a running session was interrupted. Active Task/Attempt/Conversation pins are reviewed before destructive removal.

## Persistence and events

Use existing event names in [docs/EVENTS.md](../../docs/EVENTS.md). A new event type requires a same-change edit to EVENTS.md and `docs/schemas/domain-event.schema.json`. Runtime-local observations and native install/auth operations do not become Workspace domain events unless the canonical owner explicitly promotes an event schema. Binding mutations use existing canonical event/receipt contracts.

## Retention and migration

Follow owner retention rules. Purge removes future visibility of payload bytes; receipts may remain. No migration is authorized in this revision. Any new table/column requires canonical model owner, schema migration, event/schema and tests in the same reviewed change.