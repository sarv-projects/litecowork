# 002 - Native Agent Install, Auth and Binding

**ID:** 002
**Status:** DRAFT
**Owners:** Agent Runtime, Security/Trust, Desktop UI, Product
**Updated:** 2026-11-01
**Revision:** 0.1
**Target release:** V1 local desktop
**Dependencies:** root ARCHITECTURE.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md, docs/IMPLEMENTATION-MASTER-PLAN.md, and the owner documents named in RESEARCH.md. Feature 001 remains the Conversation send authority.

## 1. Problem

Universal Chat cannot start until one native agent is observed, authenticated and bound without collapsing those states into a single connected flag, and without a LiteCowork model router substituting for the harness.

Observable outcome: The owner detects a real native agent, completes that agent's own sign-in, and gets an enabled AgentBinding whose install, auth, descriptor freshness and session readiness stay separate.

## 2. Goals

- Observe installation separately from authentication, descriptor freshness and session readiness.
- Persist an enabled Workspace AgentBinding only after the selected AgentModule reports a fresh non-secret control descriptor.
- Fail new admission when a selected model or session option is stale, and keep the composer draft.
- Show Settings > Agent Registry controls only for adapter-reported affordances.

## 3. Non-goals

- A universal Providers screen or cost router.
- Silent rewrite of native CLAUDE.md, AGENTS.md, Codex config, hooks, skills or MCP settings.
- LiteSPM package install as a substitute for native account sign-in.
- Qualifying every agent in one release; V1 qualifies one exact route first, then a second for relay.

## 4. Current evidence

This checkout is documentation-first. The behavior below is **TARGET**. Archived source and historical CODE notes are evidence of past work, not SERVED or QUALIFIED status on this tree. No owner has locked this package.

## 5. LOCKED and FLEXIBLE decisions

### LOCKED

- No global provider/model catalog (AGENT-CONTROL §2.1).
- Registry listing is not readiness (§2.3–2.4).
- Secret bytes never enter ordinary configuration, events, bindings or logs (§2.6).
- No silent option substitution (§2.8).
- LiteSPM owns package lifecycle; LiteCowork owns scoped use (§2.10).
- Installed != authenticated != bound != granted != session-ready (ARCHITECTURE).

### FLEXIBLE

- Module file layout and private helper names, provided the transactions and secret boundaries stay intact.
- Presentation copy, provided state distinctions remain visible.

### Open decisions

- Which exact OS/harness/version/auth tuple is the first GA route (P0 exit, shared with 001 B-02).
- Which credential slots on the first qualified module use NATIVE_AGENT versus LITECOWORK_SECRET_STORE. The closed rules are already locked: a NATIVE_AGENT slot is never a SecretRef; a LITECOWORK_SECRET_STORE slot stores bytes only in SecretStore.

### Blockers

- B-02 shared with 001: first provider handshake version.
- B-11: which additional AgentModules are in the V1 matrix after the first route.
- B-12: descriptor TTL and probe budget per module.

## 6. High-level design

Desktop UI talks only to the authenticated Operator. AgentRegistryService caches distribution metadata. AgentLifecycleService records Runtime-local installation observations. AgentBindingService commits Workspace-scoped bindings. Native sign-in stays outside SQLite. Session start is feature 001, not this package.

Trust boundary: the desktop UI is untrusted; the authenticated Operator is the only command path; native credentials and OS handles stay outside public events; SQLite transactions do not perform network or process calls.

## 7. Low-level design

Operator routes in docs/AGENT-CONTROL.md §12 and docs/API.md are the authority. This package does not invent operationIds. Proposed work is limited to mounting already specified registry/lifecycle/binding operations and projecting separate state chips. Any new field is PROPOSED until promoted into AGENT-CONTROL and the OpenAPI document in the same reviewed change.

Ordering: authorize, compare expected version, commit the durable receipt, then perform any external call, then reconcile. Proposed payload fields are PROPOSED until the canonical owner document and machine schema change together.

## 8. Data

See [DATA-MODEL.md](DATA-MODEL.md). This feature does not create a second source of truth beside the domain owner.

## 9. Wiring

See [WIRING.yaml](WIRING.yaml). Requests enter through the Operator. External calls sit outside transactions.

## 10. Security

Principal is the authenticated local IPC peer. Binding changes are Workspace-scoped compare-and-swap. Native credential material stays in the harness or SecretStore. Descriptors store normalized non-secret digests only. Unsupported OS or missing module returns unavailable, not a fake binding.

## 11. Errors and recovery

Stale descriptor, expired auth, missing binary, unsupported version and cancelled native login are distinct typed failures. A failed probe does not delete a prior binding. Retry re-probes; it does not replay secret bytes. Crash mid-probe leaves the last committed binding and a failed observation.

Races use compare-and-swap or a single-writer occurrence identity. Timeout after an external effect reconciles. Duplicate retries return the original receipt when the idempotency key matches and fail when the payload conflicts.

## 12. Crash and restart

Reload the last committed owner records. Do not infer success from an intent. Reconcile in-flight external work before a new side effect. Cancellation fences late results.

## 13. Configuration and compatibility

Per-AgentModule conformance fixture pins protocol version, auth method enum and descriptor expiry. Unsupported platforms fail closed. No silent fallback agent.

OS qualification is per native desktop. WSL2 does not qualify Windows or Linux GUI behavior.

## 14. Observability

Log ids, versions, statuses and denial reasons. Do not log secret bytes, native handles, private locators or raw credentials. Audit commits include actor, workspace and receipt id.

## 15. EARS acceptance

- **AC-001:** WHEN the owner requests an agent registry search, THE SYSTEM SHALL return discoverable distributions without marking them installed, authenticated or session-ready.
- **AC-002:** WHEN a lifecycle probe observes an installed binary, THE SYSTEM SHALL record a Runtime-local installation observation that is not by itself an AgentBinding.
- **AC-003:** IF native sign-in is cancelled or fails, THEN THE SYSTEM SHALL leave any existing binding unchanged and SHALL NOT persist secret bytes.
- **AC-004:** WHEN the owner enables a binding, THE SYSTEM SHALL require a fresh non-secret AgentControlDescriptor for that exact module version.
- **AC-005:** IF a previously selected model or session option is no longer in the fresh descriptor, THEN THE SYSTEM SHALL reject new admission and preserve the composer draft.
- **AC-006:** WHEN the desktop restarts, THE SYSTEM SHALL revalidate install, auth and descriptor freshness independently before showing the binding as usable.
- **AC-007:** IF the module reports native-only MCP or hooks outside LiteSPM, THEN THE SYSTEM SHALL label them as native harness state and SHALL NOT claim a Core Effect.

## 16. Executable acceptance command

```bash
python3 .specify/scripts/validate_feature_package.py specs/002-native-agent-install-auth-binding
```

Further gates after ACCEPTED/LOCKED are the cargo filters named in TASKS.md. They are not evidence until run on a recovered source tree.

## 17. Out-of-bounds

- Do not start a Conversation turn (feature 001).
- Do not install LiteSPM packages as part of binding enablement.
- Do not copy native transcripts or credentials into Workspace backup.

## 18. Assumptions and open questions

- First GA agent route remains an owner decision shared with feature 001.
- Historical source may contain partial registry reads; the active checkout has no served handlers, so evidence is TARGET.

Each blocker has the owners named above. Resolution gate: owner decision recorded in this SPEC before status leaves DRAFT. Implementation tasks: 002-T01, 002-T02, 002-T03, 002-T04.

## 7a. Owner facts incorporated 2026-11-01

Read from AGENT-CONTROL, AGENT-FABRIC, AGENT-ACCOUNT-ROUTE-MATRIX, V1-AGENT-DISCOVERY-AND-CATALOG-LLD, and NATIVE-COMPOSER-LLD.

- Core never branches on agent brand. Every configuration field owner is exactly NATIVE_AGENT, LITECOWORK_NON_SECRET, LITECOWORK_SECRET_REF, or LITESPM_CAPABILITY.
- ACP Registry metadata cannot invent install commands, auth callbacks, or secret slots. LiteCowork does not execute a shell command from that metadata.
- Lifecycle install, update, auth, descriptor probe, and native-config open do not start a Conversation, Task, Attempt, or native session, and install does not create an AgentProfile or AgentBinding.
- AgentInstallationObservation is Runtime-local. It is not an AgentProfile, AgentEndpoint, or AgentBinding. Inventory readiness stays UNKNOWN or NOT_PROBED until probed.
- AgentAuthDescriptor.state is only UNKNOWN, NEEDS_AUTH, AUTHENTICATING, AUTHENTICATED, EXPIRED, DEGRADED, or ERROR. credential_slots is required and may be empty. Auth is not AgentAdapter.authenticate().
- New bindings are created disabled. Enable is a separate owner step and does not grant Workspace or Task authority. lead_eligible=false returns AGENT_NOT_LEAD_ELIGIBLE. No enabled binding returns AGENT_UNAVAILABLE and keeps the draft. A disabled explicit override does not fall back to the Workspace default.
- Enable and disable commit one ALLOW AuditRecord with the binding projection, domain event, and idempotency receipt, excluding configuration values and SecretRefs. Exact replay returns that receipt and writes no second audit. Disable blocks new admissions only. A committed STARTING reservation may still activate on its pinned host.
- Installation, update, and auth observations are not Workspace domain events. Non-secret binding configuration emits agent.binding.configuration.changed.v1 with credential slot ids only.
- Credential submit stays unavailable until an exact Idempotency-Key replay (1–200 printable ASCII) returns the original SecretRef. The same key with a changed request conflicts. Secret bytes are not echoed.
- open_native_config accepts only a module-known target id. It accepts no caller path or shell command.
- Only a QUALIFIED account route is offered as ready. CONDITIONAL, STALE, and REVOKED are not launched as ready. Subscription-cookie reuse, credential copying, account pooling, and hidden proxying fail closed.
- Catalog cards keep detection, installation, configuration, authentication, and readiness independent. Removing a binding does not uninstall the native CLI.
- A fresh descriptor and an enabled binding do not enable Send. With no qualified supervisor, the turn settles OPEN to FAILED with CONVERSATION_SESSION_ADMISSION_UNAVAILABLE and the route returns 503 DEPENDENCY_UNAVAILABLE.
- The opaque native handle stays in Runtime-local AgentSessionHostBinding.
- Install and update run the module call outside SQLite, then persist the Runtime-local observation. Binding enable commits inside the transaction. Neither receipt means authentication success or session readiness.
- Registry cache, installation observations, control descriptors, and lifecycle operations are Runtime-local. AgentBinding is the Workspace-scoped aggregate.
- Operator routes to cite: GET /agent-registry, POST /agent-registry/refresh, GET /agent-installations, install and update, profile refresh, control-descriptor, begin auth, credential submit, native-config open, binding list, create, configuration, enable, disable. Several remain target-only until served.



## 19. Requirement traceability

| Requirement | Spec | Canonical contract | Task | Acceptance |
|---|---|---|---|---|
| AC-001 | EARS acceptance | canonical owner docs cited in this SPEC | 002-T01 | AC-001 |
| AC-002 | EARS acceptance | canonical owner docs cited in this SPEC | 002-T01 | AC-002 |
| AC-003 | EARS acceptance | canonical owner docs cited in this SPEC | 002-T03 | AC-003 |
| AC-004 | EARS acceptance | canonical owner docs cited in this SPEC | 002-T03 | AC-004 |
| AC-005 | EARS acceptance | canonical owner docs cited in this SPEC | 002-T04 | AC-005 |
| AC-006 | EARS acceptance | canonical owner docs cited in this SPEC | 002-T03 | AC-006 |
| AC-007 | EARS acceptance | canonical owner docs cited in this SPEC | 002-T04 | AC-007 |
