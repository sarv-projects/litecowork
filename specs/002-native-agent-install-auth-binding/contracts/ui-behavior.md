# UI behavior - Native Agent Install, Auth and Binding

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
- Codex App Server observations exist; OpenCode probe records incompatible offer/capabilities false.
- Observed profiles show Runtime-offer compatibility/readiness, expiry, diagnostics and Workspace binding state.
- Workspace bindings can be created disabled and enabled/disabled through versioned actions; Workspace default lead and worker-profile controls exist.
- Descriptor panel calls routes that currently return 503 DEPENDENCY_UNAVAILABLE.
- No install/update action, native sign-in flow or usage projection is served here.

Source/UI mismatches:
- OpenCode is blocked from new binding by its incompatible profile offer even though Feature 001 contains an OpenCode dispatch path.
- Codex can be bound despite the current Conversation supervisor not dispatching Codex. A binding is not proof Send works.
- Backend enable currently checks a fresh compatible Runtime offer but not the AC-004 fresh AgentControlDescriptor.
- Credential submission validates then returns 503; auth-begin and native-config-open are target-only.
- ACP registry entries are metadata and do not become install or execution authority.

## Failure and accessibility contract

Empty, stale, offline, denied, unsupported OS, unsupported version, cancelled auth, auth expired, descriptor stale, profile conflict, missing executable and protocol mismatch are distinct. Preserve prior known-good profile/binding on failed refresh or auth. Use keyboard navigation, status text independent of color, screen-reader labels, high-contrast icons, reduced motion and responsive category/detail navigation. No inert clickable-looking controls.

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

## Provider-scoped model and effort picker

- The model picker is attached to one selected AgentProfile/AgentBinding/account; it is not global.
- Opening it shows the exact agent/provider identity, searchable descriptor-backed model list, selected option and provider-declared default marker.
- An Effort/Reasoning row appears only when the exact descriptor declares supported values for the selected model/profile. No generic effort enum is inferred.
- Provider-specific modes are separate and only appear when the adapter exposes them.
- The selected provider model identifier and descriptor revision are carried into the next admission. The adapter validates again. A change applies to future admissions/new sessions, not to an active Conversation/Attempt or provider-wide native config.
- On stale/unavailable descriptor or model removal, disable the stale choice, preserve the previous committed choice and current draft, and offer only a real refresh action.
- Task composer selection remains separate: choose Coworker/eligible AgentBinding first; model/effort selection is subordinate to that binding.
- Current descriptor reads/refresh return 503 and provider send routes remain unqualified. Therefore the model picker is a target design and must not be presented as active until the exact end-to-end path works.
