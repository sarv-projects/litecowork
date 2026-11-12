# UI behavior - Native Agent Install, Auth and Binding

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

Screen ownership (A-35, accepted): the Settings -> Agents screen, its layout and its normative control table belong to [Feature 011](../../011-desktop-shell-ipc-settings-runtime/SPEC.md). This file specifies only the behavior of the 002 operations that screen and the composer invoke.

Behavior, not pixels. The Settings shell, global control authority and per-control mutation/failure classes are owned by [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md), [V1-SETTINGS-CONTRACT-LEDGER.md](../../../docs/V1-SETTINGS-CONTRACT-LEDGER.md), [DESKTOP-SHELL-LLD.md](../../../docs/DESKTOP-SHELL-LLD.md), and the [V1 Agent Discovery LLD](../../../docs/V1-AGENT-DISCOVERY-AND-CATALOG-LLD.md). This package owns agent-specific lifecycle and profile details.

## Settings > Agents screens

The category contains Catalog, Installed, Profiles, Workspace bindings and a Usage shortcut. Do not add a global Providers page or global model picker. The screen keeps the Settings category navigation persistent and a focused main panel with search and status filters.

### Catalog
Cards/list rows contain reviewed local logo or neutral fallback, display name/purpose, source/license, OS/Runtime compatibility, detection, installation/version, configuration, authentication, readiness, supported protocol/surfaces, last-used only when observed, and context-sensitive action. Search and filters are projections; they never change lifecycle state. Registry metadata must not supply executable commands or trusted install authority.

### Agent profile detail
Sections:
- Overview: module/vendor/source/license, exact version, OS/Runtime incarnation, redacted install origin, protocol/capabilities, independent status axes, last verified/used and repair reasons.
- Installation: versions, source/channel, digest/signature, disk impact, install/update/repair operation history; preview then confirm; rollback/uninstall only for lifecycle-owned binaries with qualified semantics.
- Authentication: native sign-in method kind and redacted state; descriptor-declared secure slots brokered to OS SecretStore; no credential value in WebView, storage events or logs.
- Capabilities: descriptor-driven supported/unsupported/unknown/not exposed matrix.
- Workspace bindings: disabled-by-default create, versioned enable/disable, lead eligibility/default, descriptor-specific model/effort/mode defaults. Binding is not installation/authentication/permission to invoke or proof of active session.
- Diagnostics: bounded redacted probe timeline and actionable typed repair reason.

### Independent state axes
Detection: NOT_SCANNED, SCANNING, DETECTED, NOT_DETECTED, STALE, ERROR.
Installation: MISSING, INSTALLING, INSTALLED, UPDATE_AVAILABLE, UPDATING, VERSION_UNAVAILABLE, BROKEN.
Configuration: UNCONFIGURED, PARTIAL, CONFIGURED, INVALID, UNKNOWN.
Authentication: NOT_REQUIRED, SIGN_IN_REQUIRED, AUTHENTICATING, AUTHENTICATED, EXPIRED, DENIED, UNKNOWN.
Readiness: NOT_TESTED, CHECKING, READY, DEGRADED, UNAVAILABLE, INCOMPATIBLE.

The card summary is derived. Ready requires exact module/version + Runtime incarnation + config/auth + current descriptor + qualified readiness proof. Installed is not authenticated; authenticated is not bound; bound is not granted; enabled is not session-ready.

## Actions and behavior

| Action | Preconditions | UI result |
|---|---|---|
| Search/filter | catalogue projection | local filtered projection, preserve query |
| Refresh catalog | registry read | update timestamp; retain stale cached snapshot on offline |
| Refresh detection | qualified Runtime module | per-card fresh observation; one slow probe does not block others |
| Details | profile/catalog read | full details, unknown fields explicitly unknown |
| Add existing | native picker + module exact-path validation | save only validated profile; never accept shell command |
| Preview install/update/repair | trusted module plan for OS/arch | show source, version, digest/signature, license, bytes, disk impact |
| Confirm install/update/repair | accepted preview + idempotency + lifecycle owner | operation status/receipt and post-install health probe; no implicit binding |
| Select version/rollback | lifecycle owns both versions and rollback | version history; preserve user-owned native config |
| Uninstall | explicit impact confirmation, no active claims | remove only LiteCowork-owned exact installation |
| Open native config | module-known target and fresh descriptor | open exact native target; never accept caller path |
| Sign in | supported native auth method | launch official flow; cancellation does not change prior binding |
| Add secure key | fresh descriptor declares slot + OS broker | secret submitted once, cleared from UI, never returned |
| Test auth/readiness | module defines safe probe and cost | actual probe result/source/time; disclose billable probe if applicable |
| Create binding | profile and Workspace exist | disabled binding; does not start session |
| Enable/disable binding | current version and descriptor checks | receipt; affects future admission only |
| Set default lead | enabled + lead-eligible binding | Workspace CAS; no silent alternate fallback |
| Set model/effort/mode | fresh descriptor options | future admission/new sessions; stale choice rejected, draft preserved |
| Remove binding | ownership + active Attempt impact review | binding disabled/removed; no binary uninstall |
| Usage shortcut | Usage read projection | open Usage with binding filter; unknown coverage remains explicit |

## Current implementation crosswalk (not the target)

The current desktop surface is `AgentCatalogSettings`:
- 15 curated local candidates are separate from remote ACP Registry distributions.
- Local installation/version inventory is bounded and version-only; authentication is UNKNOWN and session readiness NOT_PROBED.
- Explicit profile probe is limited to Codex and OpenCode and requires enrolled local Runtime.
- Codex App Server observations exist; OpenCode offer is CODE-compatible only for exact CLI 2.0.32 with a complete probe and observed host stop, and its descriptor advertises only observed text input.
- Observed profiles show Runtime-offer compatibility/readiness, expiry, diagnostics and Workspace binding state.
- Workspace bindings can be created disabled and enabled/disabled through versioned actions; Workspace default lead and worker-profile controls exist.
- Descriptor panel reads and refreshes through served routes and ID-only Tauri commands for the trusted OpenCode module. Module-less profiles still fail closed with 503; descriptor storage reuses migration-16 table `agent_control_descriptors`.
- No install/update action or usage projection is served here; Install and Update are shown as unavailable in V1. Sign-in and native-config routes are served but fail closed (see the trace below).

Source/UI mismatches:
- OpenCode's offer is now bindable in CODE only for exact CLI 2.0.32 with a complete probe and observed host stop. This does not establish a live native session or qualified Send path.
- Codex can be bound despite the current Conversation supervisor not dispatching Codex. A binding is not proof Send works.
- Backend enable checks the compatible Runtime offer and descriptor-revision CAS when the module declares a descriptor. The legacy module-less path bypasses AC-004; exact live auth/bind/send qualification remains open.
- Credential submission validates then returns 503 before SecretStore mutation (`NoAgentDescriptorSource`; no durable replay receipt). Auth-begin and native-config-open are served through authenticated ID-only Tauri commands but fail closed because the production launcher/opener are unqualified and OpenCode declares no method or target. Auth observations are in-memory and restart to UNKNOWN.
- ACP registry entries are metadata and do not become install or execution authority.

## Failure and accessibility contract

Empty, stale, offline, denied, unsupported OS, unsupported version, cancelled auth, auth expired, descriptor stale, profile conflict, missing executable and protocol mismatch are distinct. Preserve prior known-good profile/binding on failed refresh or auth. Use keyboard navigation, status text independent of color, screen-reader labels, high-contrast icons, reduced motion and responsive category/detail navigation. No inert clickable-looking controls.


## Provider-scoped model and effort picker

- The model picker is attached to one selected AgentProfile/AgentBinding/account; it is not global.
- Opening it shows the exact agent/provider identity, searchable descriptor-backed model list, selected option and provider-declared default marker.
- An Effort/Reasoning row appears only when the exact descriptor declares supported values for the selected model/profile. No generic effort enum is inferred.
- Provider-specific modes are separate and only appear when the adapter exposes them.
- The selected provider model identifier and descriptor revision are carried into the next admission. The adapter validates again. A change applies to future admissions/new sessions, not to an active Conversation/Attempt or provider-wide native config.
- On stale/unavailable descriptor or model removal, disable the stale choice, preserve the previous committed choice and current draft, and offer only a real refresh action.
- Task composer selection remains separate: choose Coworker/eligible AgentBinding first; model/effort selection is subordinate to that binding.
- Descriptor GET/refresh is wired through authenticated Tauri commands and works for the trusted OpenCode module; it returns fail-closed unavailable for module-less profiles. `getAgentSessionOptions` is not served, the model popover is not mounted, and provider-model choice is not revalidated by Conversation admission. Therefore the picker is a target design and must not be presented as active until the exact end-to-end path works.

## Cross-layer trace for every 002 user-visible operation (T14)

Chain: control -> Tauri command -> Operator route -> authorization -> service -> transaction/CAS -> record/event/receipt -> projection -> restart behavior -> evidence test. Current source was rechecked 2026-11-12. A step that does not exist is stated as such; it is never inferred.

| Operation | Control (`AgentCatalogSettings` / `AgentControlDescriptorPanel`) | Tauri command (`src-tauri/src/lib.rs`) | Operator route and handler | Service and transaction | Record, receipt or projection | Restart behavior | Evidence test |
|---|---|---|---|---|---|---|---|
| Probe profile | "Probe OpenCode" / "Probe Codex" | `probe_agent_profile` | `POST /v1/agent-profiles/probe`, `probe_local_agent_profile` (`operator.rs`) | bounded `probe_opencode_profile` outside any transaction, then profile/endpoint/offer write; exact OpenCode tuple is computed in `native_provider.rs` | `AgentProfile`, `AgentEndpoint`, RuntimeOffer (120 s expiry) | offer expires; probe is repeated | `opencode_profile`, `native_provider_key` (litecoworkd) |
| Create binding | "Create binding" | `create_agent_binding` | `POST /v1/agent-bindings`, `create_agent_binding` | `create_agent_binding_transaction` (fresh compatible offer gate) | disabled `AgentBinding` plus idempotency receipt | binding row is durable; offer must be fresh again to enable | `agent_binding_tests` (storage-sqlite) |
| Enable or disable binding | "Enable" / "Disable" | `enable_agent_binding`, `disable_agent_binding` | `enableAgentBinding` (`enable_agent_binding`), `disableAgentBinding` | `enable_agent_binding_transaction` with `X-Agent-Descriptor-Revision` CAS inside the same transaction | binding version, audit record, receipt | stale descriptor fails `RESOURCE_STALE` with no write | `enable_requires_the_current_fresh_descriptor_revision_when_the_module_declares_one` |
| Refresh descriptor | "Refresh controls" | `refresh_agent_control_descriptor` | `POST /v1/agent-profiles/{id}/refresh`, `refresh_control_descriptor` (`agent_control_operator.rs`); available for OpenCode module profiles | `AgentDescriptorService::refresh` (budget 3 per minute, probe outside transaction, CAS append) | `agent_control_descriptors` row from migration 16, digest | new RuntimeIncarnation never inherits a descriptor; in-process throttle resets on daemon restart | `agent_lifecycle_recovery_restart_revalidates_the_descriptor_until_reprobed`, `agent_lifecycle_recovery_failed_probe_leaves_the_prior_descriptor_intact` |
| Sign in | "Sign in: <method>" rendered only for a descriptor-declared method; OpenCode currently declares none | `begin_agent_authentication` (ID-only authenticated bridge) | `POST /v1/agent-profiles/{id}/auth/{authMethodId}/begin`, `begin_agent_authentication` (`agent_auth_operator.rs`) | `AgentAuthService::begin` behind `NativeLoginLauncher` (production `UnqualifiedLauncher`) | in-memory `AuthObservation`, resealed into descriptor | observation is Runtime-local; restart returns `UNKNOWN` | `agent_auth_*` (litecoworkd), `agent_auth_begin_*` route tests |
| Select model or effort | model popover view-model `deriveModelPicker` | none yet | `getAgentSessionOptions` is still target-only | none: no daemon admission consumes `{optionId, providerModelId, descriptorRevision}` yet | none | not applicable | `conversation-model-picker.test.ts` (view-model only) |
| Open native config | "Open native config: <target>" only for a declared target | `open_agent_native_configuration` (ID-only authenticated bridge) | `POST /v1/agent-profiles/{id}/native-config/{targetId}/open`, `open_native_configuration` (`agent_native_config_operator.rs`) | declared-target check, then `NativeConfigOpener` (V1: `UnqualifiedOpener`) | `NativeConfigOpenReceipt` | stateless | `native_config_open_rejects_undeclared_targets_and_caller_paths` |

Breaks found by this trace, left open in the owning task: (1) Sign in and Open native config have authenticated ID-only Tauri commands and descriptor-driven controls (T07, T10), but no production module declares a method/target and the launcher/opener are unqualified; (2) `getAgentSessionOptions` and `getAgentHarnessCapabilities` are target-only (T05, T09); (3) the model picker has no Tauri command or send-admission consumer (T09); (4) durable auth observations and credential receipts are absent (T07, T08); (5) live bind/send/restart evidence is absent (T15). The corresponding controls must remain unavailable for current V1 modules.
