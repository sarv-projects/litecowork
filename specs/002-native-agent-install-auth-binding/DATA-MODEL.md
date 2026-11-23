# Data model - Native Agent Install, Auth and Binding

**Feature:** 002 | **Updated:** 2026-11-23 (A-120 delegated auth-observation contract)

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

Identities and lifecycles stay in the canonical owner documents. This file lists records this slice reads or commits. It does not authorize new tables by itself.

## Identities and records

- **AgentModule / trusted module manifest** — registered adapter identity, version and protocol authority. Not inferred from an ACP listing or a command name.
- **AgentRegistryEntry** — distribution metadata cache; source, license and registry freshness; not executable or authentication authority.
- **AgentInstallationObservation** — Runtime-local installation/detection/version observation, tied to exact OS/architecture/Runtime incarnation and redacted executable identity.
- **AgentProfile** — module/Runtime-specific profile/configuration and native auth observation; no raw credential material.
- **AgentControlDescriptor** — time-bounded, version-pinned, non-secret capabilities/options and native-config/credential-slot declarations.
- **AgentAuthObservation** — Runtime-local, redacted, closed-state observation scoped to exact profile, Runtime, Runtime incarnation, module, module version and auth method; includes generation plus observation/expiry timestamps. `UNKNOWN` is absence/expiry, not a stored success state.
- **AgentCredentialSubmissionReceipt** — Runtime-local cross-store recovery record scoped by Workspace, authenticated principal, AgentProfile, credential slot and digest of the Idempotency-Key. It stores only a keyed request fingerprint, SecretKind, preallocated opaque SecretRef ID, `PENDING`/`COMMITTED` state and, after reconciliation, provider reference and timestamps. It never stores credential bytes or an unkeyed credential verifier.
- **AgentBinding** — Workspace-scoped durable eligibility/default selection; disabled on create; not a session handle, permission grant or proof of session readiness.
- **AgentLifecycleOperation / receipt** — Runtime-local idempotent install/update/repair operation when a qualified module lifecycle exists; not a domain Task.
- **AgentSessionHostBinding** — opaque Runtime-local native session handle binding; never exposed as a normal UI field or replicated.

## Relations

An AgentModule may have many Runtime-specific installation observations. One installation may be associated with a module/Runtime-specific AgentProfile. A Workspace may have multiple AgentBindings pointing to eligible profiles/modules. An active native AgentSession is created by the Conversation/Task owner and pins the selected module/profile/binding version; changing Settings defaults affects future admissions only.

AgentBinding is the Workspace-scoped aggregate. Registry cache, installation observations, control descriptors, lifecycle operations, credential-submission receipts and opaque native session handles are Runtime-local. The credential receipt carries Workspace/principal scope only to prevent cross-Workspace replay; it is excluded from replicated events and ordinary Workspace backups. Runtime-local secrets, handles and private locators are excluded from replicated events and ordinary backups. Native-auth secrets remain owned by the native harness/OS credential store; LiteCowork SecretStore applies only to descriptor-declared LiteCowork-owned credential slots.

## Lifecycle and consistency

Install and update begin an idempotent lifecycle operation, perform the exact module-approved native call outside SQLite, then persist the Runtime-local observation and reconcile the receipt. An unknown result after a lost response is not a failed install; re-probe and reconcile before retry. Preserve the last known working version until replacement qualification succeeds. Uninstall is restricted to the exact installation owned by this lifecycle service and must not delete a user-managed CLI or its configuration.

Native sign-in and credential submission are separate from installation and binding. A canceled/failed sign-in preserves existing binding; credential bytes never enter frontend state after submission, SQLite, events, logs or diagnostics. A native auth observation is redacted and time-bounded. Its unique scope is `(agent_profile_id, runtime_id, runtime_incarnation_id, module_id, module_version, auth_method_id)`. Begin uses a short IMMEDIATE transaction to reserve `AUTHENTICATING` and advance the generation, releases the transaction before native launch, and later accepts an outcome only for that exact generation. An in-flight unexpired row makes concurrent begin single-flight. Expired, absent, mismatched or superseded state is never used as proof and projects as `UNKNOWN`. No Workspace event is emitted.

Credential submission validates the authenticated principal's Workspace owner scope and an exact fresh descriptor-declared `LITECOWORK_SECRET_STORE` slot. A short IMMEDIATE transaction reserves a `PENDING` receipt and stable SecretRef ID; the OS keyring call runs outside SQLite; a second short transaction commits the opaque provider reference and one audit row. Request identity is a SHA-256 digest of the non-secret Idempotency-Key. Body equality uses a Runtime-installation-keyed HMAC over descriptor digest and decoded secret bytes, so a database-only snapshot cannot test guessed credentials. Exact receipt replay is owner-scoped and occurs before mutable AgentProfile/descriptor lookup. Reuse with a different HMAC is `IDEMPOTENCY_CONFLICT`. SecretStore retries compare only within the private OS adapter and never return credential bytes. A crash leaves either no row, a retryable PENDING row, or a COMMITTED row; PENDING retries reuse the same reference and do not overwrite different content. A request that successfully reserves PENDING is the authorization linearization point; finishing that accepted operation does not hold a SQLite transaction open or require reauthorization between the OS write and receipt commit. No DomainEvent is emitted.

Binding creation, configuration, enablement and disablement use Workspace authorization, version compare-and-swap and idempotency. Create disabled; enable requires fresh compatible Runtime offer plus fresh exact AgentControlDescriptor. Disable blocks future admissions only; it does not claim a running session was interrupted. Active Task/Attempt/Conversation pins are reviewed before destructive removal.

## Persistence and events

Use existing event names in [docs/EVENTS.md](../../docs/EVENTS.md). A new event type requires a same-change edit to EVENTS.md and `docs/schemas/domain-event.schema.json`. Runtime-local observations and native install/auth operations do not become Workspace domain events unless the canonical owner explicitly promotes an event schema. Binding mutations use existing canonical event/receipt contracts.

## Retention and migration

Follow owner retention rules. Purge removes future visibility of payload bytes; receipts may remain. The `agent_control_descriptors` table already exists from migration 16 and `AgentDescriptorStore` writes validated non-secret descriptors to it with digest/version/runtime-incarnation and expiry; T04 added no migration. The three-per-minute descriptor probe budget remains an in-process sliding window and daemon restart resets it. A-120 assigns v35 to a dedicated Runtime-local `agent_auth_observations` table; the backup table map excludes it. A new Runtime incarnation never reads a prior incarnation's observation. This table records lifecycle observations only; native sign-in, authentication verification and re-probe remain separate. Under the A-67 USER-DELEGATED direction, v36 is assigned to `agent_credential_submission_receipts`; receipts have no automatic TTL so delayed exact retries cannot create another credential entry. The table is excluded from Workspace backups/events and is purged only with a separately coordinated credential/Workspace lifecycle. The older v34 reservation was consumed by 015-T22 and is immutable.

## Provider identity (002-T01)

`agent_profiles.provider_key` is stored upper case (`CODEX`, `OPENCODE`, validated in `probe_local_agent_profile`). `NativeProviderKey::from_profile_key` is implemented and the supervisor uses the closed identity for dispatch. Existing rows need no migration; tests cover canonical and legacy lowercase forms and fail closed on unknown keys. This CODE fact does not imply that each enum variant has a qualified module or dispatch path.
