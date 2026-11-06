[Reading 114 lines from start (total: 114 lines, 0 remaining)]

# V1 Agent Discovery, Detection, Status Cards, Icons and Setup LLD

Status: normative target, complementary to AGENT-CONTROL and AGENT-FABRIC; no claim all agent binaries are currently probed. The native AgentModule/AgentLifecycleAdapter owns probes and authenticated capabilities. ACP Registry is distribution metadata only, never trusted executable authority. LiteSPM owns its own integration catalog. An agent able to consume a gateway is not necessarily ACP-spawnable or a supported LiteCowork native adapter.

## Requirements from screenshots and competitor comparison

The screenshots demonstrate a useful two-column registry with brand/logo, description, detected/version, configured/not-configured, last used, native/ACP support, install/configure action and details. Build the same *information hierarchy*, not a clone of styling/labels. A provider/model picker is NOT a substitute for an AgentModule registry. Uninstalled cards remain discoverable for supported OSes, but blocked ones cannot offer Run.

Categories: Installed / Ready / Needs Setup / Available / Unsupported; capability filters: Native Session, ACP, Subagent, Computer, Terminal, Model Options, Attachments, Credentials, Local Models. Per agent details retain upstream identity, attribution/license/source, executable version, protocol, signed/trusted module, native auth modes, config version, session resume support, command/@ capability, detected path identity (redacted), setup manual and repair. User never has to guess a binary path as normal setup.

## Detection HLD and ownership

~~~text
React AgentCatalog (pure presentation, no local file or PATH access)
  -> Operator authenticated AgentCatalog projection
  -> AgentRegistryService trusted manifest + cached external distribution metadata
  -> AgentLifecycleService (per module, Runtime incarnation, OS)
      -> OS discovery broker -> exact safe executable candidates
      -> executable identity + version probe through allowlisted AgentModule
      -> native config location probe (metadata only, not secret content)
      -> native auth observation (if supported)
      -> versioned AgentControlDescriptor / capability and model catalog probe
      -> direct qualified readiness handshake when user requests Test
  -> scoped AgentProfile + AgentBinding repository
~~~

Never scan the entire disk, user's complete home directory or process command line indiscriminately. Startup probes are bounded and parallel (semaphore; per-probe timeout and overall cap), cached with timestamp and invalidated when relevant configuration or executable changes. A detection task must not block the main Window startup or start/consume a model session.

## Discovery strategies and precedence

1. Runtime OS/architecture and trusted AgentModule manifest candidates; identify Windows, macOS, Linux plus WSL distributions as distinct Runtime execution environments. An executable in Windows PATH does NOT mean it can run inside Ubuntu WSL, and vice versa.
2. OS-standard PATH lookup using actual process environment plus configured login-shell path sources only when supported/safe. Do not execute arbitrary shell init scripts to find PATH; inspect known package manager install locations through bounded, sandboxed metadata. Candidate sources: recognized native installers; npm global under approved prefix/nvm or fnm; pipx/uv tool install records; Homebrew; winget/scoop; Cargo; OS app bundles and known distribution registry.
3. Exact path candidate -> canonical/real path, owner/ACL, executable permission, binary identity/hash and version. Never trust a command name from untrusted registry metadata or config file; no string-to-shell command evaluation.
4. Module-qualified version probe through subprocess argv with sanitized fixed environment, cwd and deadline; parse stdout/stderr bounded, no secrets. If candidate is a launcher script, validate module-specific launcher and interpreter path explicitly. Version probe proves runnable/version only, not auth/model/API readiness.
5. Config/auth state probe with vendor-supported official API, never read/copy private tokens or browser cookies. Auth method, expiry/needs login, available models/options only when qualified and consented. If native harness only supports interactive auth, show Sign In and avoid background credential access.
6. User-selected binary/app path is FALLBACK: native file picker -> exact path validation -> module compatibility/version handshake -> scoped owner-binding record, never a global PATH overwrite. Retain independent candidate list and let user choose detected installations/versions deliberately.
7. If no accepted candidate, display Not Detected with safe installation options (qualified supported OS distribution), Install Guide, Retry Detection, Add Existing Location. Broken and Unverified are different from Not Installed.

At startup use single-flight cache from previous verified Runtime observation; render DETECTION_PENDING/STALE gracefully. Refresh on install/update/uninstall, PATH change or OS package event where qualified, config file mtime, Runtime incarnation change, agent setup changes, manual Refresh. Avoid stale installed status when executable was removed. Detection is not authentication, and no automatic installer execution merely because metadata advertises a package.

## States: independent axes, not one boolean

~~~ts
type AgentDetection = "NOT_SCANNED"|"SCANNING"|"DETECTED"|"NOT_DETECTED"|"STALE"|"ERROR";
type AgentInstallation = "MISSING"|"INSTALLING"|"INSTALLED"|"UPDATE_AVAILABLE"|"UPDATING"|"VERSION_UNAVAILABLE"|"BROKEN";
type AgentConfiguration = "UNCONFIGURED"|"PARTIAL"|"CONFIGURED"|"INVALID"|"UNKNOWN";
type AgentAuthentication = "NOT_REQUIRED"|"SIGN_IN_REQUIRED"|"AUTHENTICATING"|"AUTHENTICATED"|"EXPIRED"|"DENIED"|"UNKNOWN";
type AgentReadiness = "NOT_TESTED"|"CHECKING"|"READY"|"DEGRADED"|"UNAVAILABLE"|"INCOMPATIBLE";
type AgentCatalogProjection = {
 registryAgentId:string; moduleId:string; logoAssetId?:string;
 displayName:string; tagline:string; upstreamHomepage?:string;
 sourceLicense?:string; vendorAttribution?:string; description?:string;
 osSupport:string[]; protocolKinds:string[]; supportedCapabilities:string[];
 installation:AgentInstallation; detection:AgentDetection;
 configuration:AgentConfiguration; auth:AgentAuthentication;
 readiness:AgentReadiness; version?:string; lastVerifiedAt?:string;
 lastUsedAt?:string; runtimeIncarnationId:string;
 source:"TRUSTED_MODULE"|"REGISTRY_METADATA"|"USER_SELECTED";
 repairReasons:string[]; availableActions:string[];
};
~~~

Mapping example: DETECTED + INSTALLED + UNCONFIGURED + SIGN_IN_REQUIRED + NOT_TESTED displays "Detected · Needs setup" (NOT "Ready"). MISSING+NOT_DETECTED displays "Not detected · Install guide". UPDATED version with stale descriptor is "Rechecking", never "Current ready". A working model catalog from gateway cannot imply a native agent binary is installed.

## Provider-specific model picker and effort selection — 2026-11-06

Reference reviewed: nightly-labs/openbot@6759fa3ee857f4f4966865e12de99ae84ab6de07 (package 0.62.0). Files: `packages/ui/src/features/conversation/AgentSettingsPanel.tsx`, `ProviderModelPicker`, provider-account helpers, `src/backend/provider-drivers.ts`, `provider-runtime.ts`, `agent/provider-models.ts`, and Tasks composer model/agent selection. The visible picker is attached to a selected Agent profile/account. It is not a global model router. The README labels this a development preview and documents danger-full-access/approvalPolicy-never behavior; package.json declares PolyForm Noncommercial 1.0.0. Study UX only; no source or permission model is copied. See [OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md](OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md).

### LiteCowork V1 picker behavior

- The Conversation composer header presents the selected Agent/AgentBinding and the currently selected model as one compact trigger; effort is a secondary compact control only when the provider exposes it.
- Opening the trigger shows a compact profile/account header (provider identity, installed version/runtime and profile/account label), a searchable list of models discovered for that exact native profile/account, a selected checkmark, and an optional provider-declared default label.
- If the catalog is long, the list is scrollable with stable grouping/search. Do not show models from another provider/account or stale profile. Provider-owned labels/icons may be used only from reviewed local assets.
- The bottom row exposes Effort/Reasoning only if the fresh descriptor defines a nonempty enumeration for that selected model or profile. Render exact supported values/labels (for example Low/Medium/High only when the descriptor returns them); do not invent a generic enum.
- Provider-specific additional options (fast mode, service tier, approval mode, context window, image setting) are separate rows only when declared by the exact descriptor and owned by the module. Advanced options remain inside a collapsed/secondary area, not mixed into the model radio list.
- Switching model or effort updates the current draft selection with a descriptor revision and applies to the next admission/new session only. It does not mutate an already-running Conversation/Attempt or silently rewrite the native CLI's account-wide config.
- A model may belong to a different provider account/profile only if that change is an explicit, separately rendered account switch supported by the module and the owner approves its impact. No cross-provider implicit switch.

### Model/effort state contract

Each option is `{optionId, displayName, providerModelId, modelFamily?, availability, supportsEffort, allowedEfforts[], defaultEffort?, capabilities[], descriptorRevision, observedAt}`. UI identity is optionId; providerModelId is preserved exactly for the adapter. The provider module validates it again at admission. `availability` is AVAILABLE, UNAVAILABLE, UNKNOWN or STALE. A stale descriptor disables selection and offers Refresh; a provider outage preserves the last committed choice but marks the list stale.

Model discovery is read-only metadata except where the exact provider requires a qualified list request. If model list is expensive, rate-limited or billable, disclose before probing and cache with source/age. Do not infer support from model names. Hidden/deprecated models and provider plan entitlements are not shown as selectable without authoritative module evidence.

Selecting a model is a versioned Workspace AgentBinding preference for future work; provider-account/model-native defaults stay provider-owned. A descriptor revision change invalidates an uncommitted stale option and requires refresh/reselection. The composer preserves typed text and attachments across refresh/conflict.

### Tasks composer agent picker

Keep Task agent selection distinct from ordinary Conversation Agent/model selection. A Task requires one eligible Coworker/lead binding under Feature 005; user selects a Coworker/AgentBinding for a new Task. If that binding supports one or more model options, model/effort is a nested secondary option after selecting the agent. Persist the choice against the exact admission snapshot. Do not store the Task agent choice only in localStorage as authority. A local convenience default may preselect, but server-side admission validates the current enabled binding, descriptor, grant and Workspace.

### UI density and interaction

Copy the reference's compact popover rather than a large modal: provider rail/selector only when the user has multiple profiles, search input, list with single-line model names, selected check, small default marker, and bottom effort row. The popover anchors to the composer trigger, has keyboard search/focus, arrow/enter/escape, scroll containment and screen-reader selected/disabled states. Avoid duplicate provider icons, verbose model descriptions in the list or showing every provider in a horizontal tab row.

### Current implementation boundary

Current Feature 002 has Codex/OpenCode profile probe observations, but the ControlDescriptor read/refresh endpoints return 503. Therefore model/effort picker and descriptor-bound options are still target-only. Current Codex binding can exist without the current supervisor dispatching Codex; OpenCode is marked incompatible by its profile probe. Do not render model/effort controls as active until a compatible exact adapter, fresh descriptor and Conversation/Task send path are qualified.

## Logo, icon and name policy

Each trusted module provides optional official logo asset identity with shipped/offline bundled vector/PNG from reviewed license and upstream attribution. Version/mode/badges are text, not color-only. Never load arbitrary remote SVG from metadata into privileged WebView or allow SVG script/external URI. Sanitize static local assets and their filenames; use initials or neutral terminal icon when official brand logo cannot be redistributed. The registry record may provide a safe link to official homepage but no inferred endorsement. Icon set and display text are reviewed on upgrade, no fetching favicons during every render. High contrast, reduced motion, accessible labels and correct icon per agent required.

## Actions and error ownership

| Action | Preconditions | Postcondition/recovery |
|---|---|---|
| Refresh detection | qualified module and Runtime | fresh signed/probed observation, cache age reset; timeout shows stale/error |
| Inspect detected path | authenticated local owner | redacted location, runtime/OS and origin, no secret CLI args |
| Browse custom path | native OS file picker and owner approval | validated candidate saved to AgentProfile, rejected path leaves previous config |
| Preview installation | trusted distribution, OS arch, license/source/bytes/disk | reviewed install plan, never download untrusted manifest command |
| Install/Update/Repair | version pin and storage constraints | background operation receipt, old working version preserved until health pass |
| Open native config | native module-qualified UI, file ownership | opens exact approved native config editor/view, without rewriting unrelated settings |
| Sign in/native auth | official supported method and OS broker | qualified auth observed, secret bytes never in React |
| Configure API key | exact AgentModule secure slot | OS key store updated via privileged broker; no plaintext in settings |
| Test agent | installed, configured, owner-approved network/compute | actual no-side-effect handshake and qualified readiness; usage must be accounted when model inference actually occurs |
| Set default agent | valid Workspace, enabled AgentBinding | versioned Workspace default CAS, not global native override |
| Set model/mode/effort | exact fresh native ControlDescriptor | bound option revision, old sessions not silently switched |
| Remove binding | owner CAS and active attempt dependency review | scoped binding disabled, never uninstalls user's native CLI |
| Uninstall AgentModule | explicit user action and no active claims | exact owned installation only, not native user config or shared agents |

No simultaneous installs/version-probes targeting same binary without module operation lock. User cannot enable an unsupported agent by typing an arbitrary shell command. Normalize native error taxonomy: NOT_FOUND, VERSION_UNAVAILABLE, PERMISSION_DENIED, BROKEN_LAUNCHER, AUTH_REQUIRED, NOT_CONFIGURED, OFFLINE, PROFILE_CONFLICT, UNSUPPORTED_OS, PROTOCOL_MISMATCH, STALE_DESCRIPTOR.

## Proposed service functions (no new served route assumed)

discoverAllSupportedAgentModules(runtime) -> manifest list / source age;
enumerateInstallCandidates(moduleId, os, approvedRoots) -> bounded candidates / provenance;
validateCandidatePath(moduleId, exactPath) -> safe canonical path or rejection;
probeAgentVersion(moduleId, candidate, timeout) -> signed/qualified observation;
probeAgentConfig(moduleId, exactInstallation) -> sanitized state / age;
probeAgentAuth(moduleId, identity) -> authorized redacted observation / UNKNOWN;
computeAgentReadyState(installation, config, auth, descriptor, runtime) -> READY/repair reason;
refreshAgentCatalog(runtime, cacheEpoch, cancel) -> single-flight projection with bounded concurrency;
previewNativeInstall(moduleId, distributionDigest) -> impact and disk budget;
admitNativeInstall(request, owner, version) -> idempotent lifecycle receipt;
registerManualAgentPath(moduleId, os, location, expectedVersion) -> validated scoped setting;
resolveAgentLogo(moduleId, publishedAssetManifest) -> reviewed local resource or neutral fallback;
listAgentCatalogPage(workspace, filters, cursor) -> access-scoped card list;
openAgentProfileDetails(agentId, runtime) -> exact version, auth and compatibility details.

## UI and test gates

Display 1–2 column responsive cards or compact list; logo+name+description+version top, independent detection/install/config/auth/ready chips, last-used time if actually observed, native/ACP status, actions Configure/Retry/Install/Details. Search/sort/filter by status, capabilities and OS; reveal why one is unavailable. Initial scan skeleton is not "Not installed". Click a card opens details even if missing; no inert buttons. Support offline registry cache but visibly label stale. Multiple installed copies and WSL+Windows copies appear as distinct qualified Runtime choices.

Tests: PATH + npm/nvm/pipx/uv/winget/brew + standalone binary detection; Windows executable cannot be launched in wrong WSL Runtime; manual path selection rejects symlink escape/untrusted executable; no arbitrary user shell commands executed during auto-scan; 5s slow provider only degrades its card; stale cache corrected on binary removal; auth expiry doesn't erase install status; version check cannot consume an expensive model call silently; two windows racing install respect owner lock; native agent session remains unchanged after default binding switch; accessibility logos+text work without color.

## External reference and use boundary

OmniRoute reference: https://github.com/ourines/omniroute/blob/main/docs/reference/CLI-TOOLS.md — its batch status includes installed/runnable, config status and per-tool error with cached multi-tool detection. See also https://github.com/ourines/omniroute/blob/main/docs/guides/CLI-INTEGRATIONS.md . Borrow the detection/state-card pattern only. A CLI consuming OmniRoute is not identical to a native agent that LiteCowork can manage/spawn through Agent Fabric. ACP Registry remains metadata; registered LiteCowork AgentModule defines safe execution authority.

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]