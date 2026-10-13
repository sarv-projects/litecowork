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
