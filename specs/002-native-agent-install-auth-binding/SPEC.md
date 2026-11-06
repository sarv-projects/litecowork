[Reading 278 lines from start (total: 278 lines, 0 remaining)]

[Reading 186 lines from start (total: 186 lines, 0 remaining)]

# 002 - Native Agent Install, Auth and Binding

**ID:** 002
**Status:** DRAFT
**Owners:** Agent Runtime, Security/Trust, Desktop UI, Product
**Updated:** 2026-11-06
**Revision:** 0.3
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

The active checkout has partial, source-backed Agent Registry, installation-inventory, profile-probe and AgentBinding surfaces. It does not implement the complete install/auth/control-descriptor lifecycle promised by this target. The matrix below is a code/route/UI audit, not owner acceptance or release qualification.

| Capability | Current source evidence | Limitation / release meaning |
|---|---|---|
| ACP Registry discovery | CODE/SERVED: `GET /v1/agent-registry` and `POST /v1/agent-registry/refresh`; remote snapshot is bounded, cached and stale-labelled | Read-only metadata. ACP entries are always projected as `NO_ADAPTER`; registry presence is not installation or support. |
| Local installation inventory | CODE/SERVED: `GET /v1/agent-installations`; bounded version-only inventory probes and Cline package metadata cover the curated candidate set | Auth is always UNKNOWN and session readiness NOT_PROBED. There is no install/update action. |
| Workspace profile discovery | CODE/SERVED: `GET /v1/agent-profiles`; explicit `POST /v1/agent-profiles/probe` supports CODEX and OPENCODE after local Runtime enrollment | Probe creates/updates profile, endpoint binding and Runtime offer observations; it does not prove inference or a usable Conversation session. Other curated candidates have no profile probe. |
| Codex probe | CODE for App Server initialize/account/model-list observations; compatibility and readiness are separate | Conversation safety eligibility is separately reported. Feature 001's active Conversation supervisor does not dispatch Codex, so a Codex binding is not a working Conversation route. |
| OpenCode probe | CODE for bounded provider/model catalogue observation and process-stop observation | It always records `compatible=false`; endpoint capability flags are false. UI correctly blocks creation of a new Workspace binding. Feature 001 has an OpenCode dispatch code path, but the current UI cannot create the binding needed to reach it. |
| AgentControlDescriptor | GET descriptor and POST refresh routes are mounted | Both deliberately return `503 DEPENDENCY_UNAVAILABLE`; no qualified AgentModule descriptor producer is wired. |
| Credential submission | POST credential-slot route is mounted with no-store, bounded body and request validation | It checks for a fresh descriptor and then deliberately returns `503`; SecretStore is not mutated because durable exact replay is not implemented. Native auth-begin remains target-only. |
| AgentBinding | List/get/create/enable/disable are CODE/SERVED; creation is disabled by default; mutations use version checks, idempotency receipts and audit events | Storage checks a fresh compatible Runtime endpoint offer. Enable does not currently require a fresh AgentControlDescriptor, contrary to AC-004. `auth_ref` is rejected on create. |
| Install/update and native config | OpenAPI target operations exist | `POST /agent-registry/{id}/install`, `/update`, native-config open and auth-begin have no served handlers in the active composed router. |
| Desktop UI | `AgentCatalogSettings` renders 15 curated candidates, ACP Registry results, local inventory, Codex/OpenCode probes, observed profiles, Workspace bindings/default lead and worker-profile controls | No install/update or native sign-in flow. Auth remains explicitly unknown; Inspect native controls reaches the unavailable descriptor route. This is a catalogue/probe/binding surface, not a complete agent lifecycle UI. |

Code or a route alone does not qualify the feature. This package remains DRAFT until its named owners accept the decisions and supporting contracts.

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

Operator routes in docs/AGENT-CONTROL.md §12 and docs/API.md are the authority. This package does not invent operationIds. The active checkout already mounts partial registry, inventory, profile-probe and binding routes. Proposed work must close the missing install/update/auth-begin/native-config operations and the unavailable descriptor/credential producers without duplicating existing routes. Any new field is PROPOSED until promoted into AGENT-CONTROL and the OpenAPI document in the same reviewed change.

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
- The active checkout has partial served registry/inventory/profile/binding handlers. Install/update, native auth-begin and native-config open remain target-only; descriptor refresh and credential submission routes are mounted but fail closed with 503. See §4 and docs/IMPLEMENTATION-STATUS.md.

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



## 7b. Current source and visible UI boundary — 2026-11-06

- The mounted Settings Agents surface is `AgentCatalogSettings`, not the target Catalog/Installed/Profiles/Workspace bindings/Usage information architecture. It shows 15 curated candidates and ACP Registry distributions; registry entries are read-only metadata and map to `NO_ADAPTER`. Local inventory is bounded version/path observation, with authentication `UNKNOWN` and readiness `NOT_PROBED`.
- Profile probe is limited to Codex and OpenCode and requires an enrolled local Runtime. Codex App Server initialize/account/model-list observations exist. OpenCode is recorded incompatible with its current offer/capability probe. Neither profile probe proves an end-to-end Conversation session.
- No native AgentModule install/update/repair action exists in the active UI or a qualified installer route. Native auth-begin and native-config-open are target-only. Control-descriptor read/refresh and credential submission routes fail closed with `503 DEPENDENCY_UNAVAILABLE`; credential submission does not write a SecretStore value.
- Workspace binding list/create/enable/disable/default-lead operations are served. New bindings are disabled by default. Backend enable checks a fresh compatible Runtime offer but does not enforce the Feature 002 AC-004 fresh `AgentControlDescriptor` gate. The UI therefore exposes a binding lifecycle that is not equivalent to authenticated/descriptor-ready execution.
- The current Conversation supervisor has an OpenCode dispatch branch only. The current OpenCode profile offer is incompatible, so the visible profile cannot be newly bound through the current gate. Codex can be bound, but there is no Codex Conversation dispatch branch. The exact first GA agent/OS/version/auth route remains unqualified.
- The composer model/effort picker is target-only: descriptor reads are unavailable and no exact provider dispatch path qualifies it. Do not render a provider model selector or claim a model selection can run merely because a catalogue or native profile exposes model metadata.
- No Usage projection is served. No Install/Update, native sign-in, credential-slot form, descriptor-driven capabilities matrix or durable profile diagnostics timeline is mounted in the active Settings UI.

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

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

## V1 model and effort picker details — 2026-11-06

The composer model picker is scoped to the exact selected AgentProfile/AgentBinding and native account. It is a compact anchored popover, not a global Provider settings control. It shows profile/provider identity, version/runtime, a searchable model list, one selected check, provider-declared default marker, and a bottom Effort row only when the exact descriptor exposes effort options.

Model and effort choices are descriptor-bound `{optionId, providerModelId, descriptorRevision}` values and are revalidated at admission. A stale descriptor blocks selection and offers refresh while preserving the draft. Switching changes future admission/new sessions only; it cannot mutate a running Conversation/Attempt or rewrite provider-native account settings. Optional provider-specific modes are shown as distinct secondary settings only when module-declared.

For Task creation, user chooses the eligible Coworker/AgentBinding first; any supported model/effort option is nested under that selected binding. A locally remembered default may preselect but is never admission authority. Server validates Workspace, binding status, descriptor freshness, grants and eligible runtime before creating the Task.

Current route remains target-only: ControlDescriptor endpoints are 503, Codex is not currently dispatched by the Conversation supervisor despite binding support, and OpenCode's current profile probe is incompatible. No active model/effort picker may appear until one exact send path and descriptor is qualified.

## V1 UI and lifecycle contract — 2026-11-06

This section makes the existing lifecycle distinction visible in the Settings experience. It does not claim that the currently missing installer, auth broker or descriptor handlers have become implemented.

### Information architecture

Settings > Agents is a two-level experience:

1. **Agent catalog** — search and discover supported modules and distributions, then inspect each candidate's source, supported OS/Runtime, capabilities and installability.
2. **Agent profile** — one detail view for one selected agent installation/profile, with Overview, Installation, Authentication, Capabilities, Workspace bindings and Diagnostics sections. The exact sections shown depend on module-reported capabilities; do not show empty fake settings pages for unsupported surfaces.

The Catalog has compact filters: All, Installed, Ready, Needs setup, Available and Unsupported. Filters are projections over independent state axes and must not mutate them. Add capability filters only when backed by the trusted module manifest: Native session, ACP, Subagent, Computer, Terminal, Model options, Attachments, Credentials and Local models. Search covers display name, vendor and declared capability metadata, not executable arguments or secret configuration.

The card/list row presents: reviewed local logo or neutral fallback; display name and concise purpose; vendor/source/license; detection and installation/version; configuration; authentication; readiness; supported execution surface/protocol; Runtime/OS target; last-used time only when observed; and one context-sensitive primary action plus Details. Never load untrusted remote SVG into the privileged WebView. Do not use brand color alone to convey status.

### Agent lifecycle as independent, visible state

The visible card has separate status labels for:
- Detection: Not scanned, Scanning, Detected, Not detected, Stale, Error.
- Installation: Missing, Installing, Installed, Update available, Updating, Version unavailable, Broken.
- Configuration: Unconfigured, Partial, Configured, Invalid, Unknown.
- Authentication: Not required, Sign-in required, Authenticating, Authenticated, Expired, Denied, Unknown.
- Readiness: Not tested, Checking, Ready, Degraded, Unavailable, Incompatible.

The combined summary text is derived, never persisted as a second state machine. Examples: Detected + Installed + Sign-in required + Not tested = "Detected · Needs setup"; Installed + Authenticated + descriptor stale = "Rechecking"; Missing + Not detected = "Available to install" only when a trusted install plan exists, otherwise "Install guide". "Ready" requires the exact module/version, Runtime incarnation, auth/config state, fresh descriptor and a direct qualified readiness handshake where that module requires one. A compatible model endpoint alone is not native-agent readiness.

### Agent profile: information and actions

**Overview** shows vendor, homepage, source/license, exact module/version, Runtime and OS/architecture, detected executable origin (redacted path by default), protocol, supported capabilities, configuration/auth/readiness, last verified and last used timestamps, and known repair reasons. "Open official site" is an external link, not an install action.

**Installation** shows installed versions and provenance, trusted distribution/channel, version, digest/signature verification, disk estimate, update availability, and operation history. Available actions:
- Refresh detection — bounded read/probe.
- Add existing installation — native picker; validate exact candidate and module compatibility. Never accept a shell command or arbitrary config path.
- Preview install/update/repair — display exact trusted plan, source, version, digest, license, platform, bytes/disk impact and native configuration impact.
- Install/update/repair — enabled only after preview and owner confirmation, durable idempotency and qualified native installer; preserve previous working version until the replacement passes its health gate.
- Select version / rollback — only when installer stores owned version artifacts and rollback semantics are qualified. Never delete user-owned native configuration.
- Uninstall — separate destructive confirmation, exact LiteCowork-owned installation only, active-claim check; it never uninstalls a user-managed native CLI or removes its native configuration.

No arbitrary package-manager command or registry-provided shell string is executed. Distribution metadata is not executable authority. LiteSPM remains owner of MCP/skills/plugins/capability package lifecycle; Feature 002's installer is only for AgentModules/native harness distributions that its trusted module lifecycle owns.

**Authentication** shows method kind and redacted state only. Prefer the agent's official native sign-in flow. Secure key entry is shown only for exact descriptor-declared credential slots and is brokered into OS SecretStore; the UI must never persist secret bytes, reveal them after submission or put them in events/logs. Test auth is a bounded read-only provider probe and must disclose when it could consume a billable request. Reauthenticate and sign out affect the exact native profile/account and must state which bindings/sessions are impacted. Cancellation or failure preserves existing binding and prior working native auth state.

**Capabilities** is a read-only descriptor-driven matrix: native conversation/session, ACP client/server, resume, models/options, effort/mode, attachments, tools, native subagents, terminal/browser/computer and any adapter-specific affordances. Each row shows supported/unsupported/unknown/not exposed, scope and descriptor timestamp. An unexposed capability is not inferred from a brand, executable name, gateway or ACP directory entry.

**Workspace bindings** lists Workspace-scoped AgentBindings separately from installed profile identity. Create binding disabled, then enable as a separate CAS action after fresh descriptor validation. Configure Workspace default lead only from enabled, lead-eligible binding. Model/effort/mode defaults are options declared by that binding's current descriptor and affect future admissions/new sessions only. Disable/remove a binding blocks new admissions; it does not uninstall the binary, revoke a separate account, or claim to kill an active session. Changing the default does not switch a running Conversation or Attempt.

**Diagnostics** shows bounded redacted probe history, source and age, supported version/platform, exact repair reason and retry. It never includes auth material, raw command arguments, prompts or native transcripts. "Copy diagnostics" is explicitly redacted.

### Global Settings and the Agents section

The Agents category contains these panels in order:
- Catalog: discovery, detection, installation state, capabilities and readiness.
- Installed: local host installations and versions, with Runtime selector when a machine has distinct Windows/macOS/Linux/WSL Runtime identities.
- Profiles: detected and registered agent profiles, auth/config/descriptor state.
- Workspace bindings: enabled/disabled bindings, lead eligibility and Workspace default.
- Usage shortcut: opens the Usage page with agent filter preselected; never shows fabricated per-agent spend.

Do not create a generic Providers page or a global model router. Native agent profile configuration belongs to the module/harness. LiteCowork stores only scoped binding/default choices and non-secret module observations.

### Control/action contract

| Control | V1 behavior | Authority and enabled condition | Failure/empty behavior |
|---|---|---|---|
| Search and filters | Local filter over fresh/cached catalogue projection | AgentRegistry read | Preserve query; show registry stale age/offline |
| Refresh catalog | Re-fetch registry metadata | Registry service | Show prior snapshot as stale; no false empty |
| Refresh detection | Re-probe local installations | qualified AgentLifecycle module + Runtime | Per-agent timeout/error; other cards remain usable |
| Details | Open profile detail view | existing catalog/profile read | Show missing fields as unknown |
| Add existing | OS picker then validate exact executable | native picker + module validation | Reject mismatch/path issue; retain prior profile |
| Install/Update/Repair | two-step Preview then Confirm | trusted installer plan, supported OS, idempotent operation | No action when target-only; show exact missing prerequisite |
| Open native config | module-known target only | fresh descriptor declares target; owner consent | Disabled when route/target unavailable; never accept path string |
| Sign in | launch native official auth flow | module auth descriptor and native auth owner | Cancelled/expired/denied distinct; no secret persistence |
| Add secure key | descriptor-declared slot with OS broker | supported slot and secure storage | Input cleared; failure does not alter prior slot |
| Test connection | no-side-effect handshake where available | module defines probe type and cost disclosure | Unknown if not exposed; no generic "Connected" claim |
| Create binding | create disabled Workspace binding | Workspace owner, profile identity, version/receipt | Conflict and duplicate handled by original request id |
| Enable/Disable | versioned binding mutation | fresh descriptor, valid offer, Workspace auth | New admissions only; explicit active-session caveat |
| Set default lead | Workspace default CAS | enabled + lead eligible | No fallback to a different provider silently |
| Model/mode/effort defaults | descriptor-specific option selection | fresh exact descriptor | Stale selection rejected, composer draft preserved |
| Remove binding | disable/remove scoped reference | impact and active Attempt review | Does not uninstall native binary |
| Usage | navigate to Usage filtered by binding | Usage projection if served | Empty/partial/unknown stated, never $0 for hidden usage |

### Settings page shell and visual behavior

Use the screenshots as information-hierarchy reference, not exact styling. On desktop, retain the persistent Settings navigation/category column and focused main panel, with a compact search field. The main panel should use grouped, low-noise sections and list rows rather than large dashboard cards for every setting. Keep the active category visually obvious and preserve selection when a detail panel opens. On narrower widths, category navigation becomes a focus-managed drawer; back returns to the same list and search/filter state. All rows are keyboard navigable and status text remains screen-reader accessible. Loading placeholders are per section; one provider's slow probe must not block the entire page.

### V1 release gating

For V1, only expose primary actions whose exact implementation is qualified on the declared native OS/Runtime. Unserved install/update/auth/descriptor operations remain absent or visibly unavailable with the dependency. ACP registry metadata never generates a shell install command. The current state described in §4 remains the implementation truth until code, OpenAPI, schemas, storage, UI and tests are promoted together.

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]