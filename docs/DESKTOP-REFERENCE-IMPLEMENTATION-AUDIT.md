# Desktop Reference Implementation Audit: ZCode and DeepSeek Harness

**Review date:** 2026-10-31  
**Status:** Non-normative research; adaptation candidates only.  
**Purpose:** inspect the actual desktop apps, their host/renderer/IPC/plugin/runtime/update boundaries and tests. Complements the screenshot-focused [ZCode UI/UX Inspiration Audit](ZCODE-UI-INSPIRATION-AUDIT.md).  
**Authority:** this document does not amend LiteCowork architecture, domain ownership, schemas, security policy, V1 scope, implementation approval or qualification status.

## 1. Scope, pins and confidence

Both repositories were shallow-cloned into the local Git-ignored `repo_take/` folder. They are research material, not submodules, product dependencies or deliverables. Both checkouts were clean after clone. Neither upstream was built, executed or tested in this review.

| Ref | Repository and actual desktop app | Checked revision | Version | Root license observed |
|---|---|---|---|---|
| ZCODE-DESKTOP | [zai-org/ZCode](https://github.com/zai-org/ZCode), `packages/desktop` | [a60ba6a7e5a06742270ca605f5d2cc2766afa7e1](https://github.com/zai-org/ZCode/tree/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1), 2026-10-30 | `@zcode/desktop`, monorepo 3.17.1 | Apache-2.0; root `NOTICE.md` present |
| DSH-DESKTOP | [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness), `apps/desktop` | [ce7073d51254c26cd63b22a60835b8f948deb447](https://github.com/deepseek-ai/deepseek-harness/tree/ce7073d51254c26cd63b22a60835b8f948deb447), 2026-10-30 | `@deepseek-ai/dsh-desktop` 0.2.2-alpha.2 | MIT; root `LICENSE` present |

The paths matter: ZCode's desktop app is `packages/desktop`, not `apps/zcode-cli` (the latter is its agent CLI/runtime). DeepSeek's desktop app is `apps/desktop`. Both use Electron; LiteCowork uses Tauri plus its Rust daemon. These are not drop-in shell replacements.

### Review limits

- The manifests, app READMEs, relevant Host/IPC/preload/plugin/update sources, architecture references and targeted tests were inspected. This is a targeted component audit, not a claim that every source file, dependency, test or commit was read line by line.
- Upstream tests were inventoried but **not run**. The clone alone proves neither build success nor OS/user qualification.
- Root licenses were inspected, not every file header, asset, generated source, dependency or transitive license. LiteCowork has no root `LICENSE` file at the checked path; this is a code-reuse gate, not a conclusion about intended ownership.
- Review and preserve ZCode's `NOTICE.md` where applicable. DeepSeek's root MIT license does not automatically relicense vendor or third-party material. Review each candidate file and dependency.
- DeepSeek's desktop README is internally inconsistent: Known limitations says sign-in is not connected, while later sections describe browser PKCE sign-in and expiry/sign-out. Verify exact code and a real account rather than assuming the README proves readiness.

## 2. Architecture comparison

| Concern | ZCode desktop | DeepSeek Harness desktop | LiteCowork decision |
|---|---|---|---|
| Shell | Electron 44 | Electron 44 | Keep Tauri/Rust; do not transplant Electron Main/preload wholesale |
| UI | Shared React UI in `packages/ui`, composed by Desktop | Packaged DSH Web app loaded by Electron | LiteCowork screen contracts remain authoritative; selectively adapt UI after dependency/owner review |
| Host | One window-scoped utility Host per BrowserWindow | One desktop-owned Node-mode Host for shared DSH Web app | Neither equals durable per-user `litecoworkd` |
| Conversation/runtime | CLI owns conversation/session state; Host routes workspace scopes | Shared Harness Host/profile backs Web app | Harnesses retain private transcript/model/tools; Core owns canonical Conversation, Task, Trust, Effect, Artifact and Evidence |
| Renderer access | Service/protocol abstractions; Main mediates native actions | Narrow typed `contextBridge` API; no raw IPC/shell/filesystem access | Keep authenticated Operator API and typed Tauri commands; WebView is untrusted |
| Remote scopes | Host-owned connection registry and scoped attachments | Primarily local Desktop Host/Web backend | V1 remains local-first; no remote Runtime expansion from this comparison |
| Plugins | Main-owned plugin/MCP-app guest sandbox | Plugins and Host execute in same Electron Node-mode process | Do not copy same-process execution for untrusted extensions; LiteSPM owns package lifecycle, Trust owns use grants/effects |
| Update/quit | Main supervises Host and desktop lifecycle | Admission lock, drain, recheck, positive graceful Host ack; unknown fails closed | Adapt ordering only after Task/Attempt/Effect/lease/service owners specify drain |
| Persistence | ZCode CLI/service stores | DSH profile under Harness home | Do not substitute either for LiteCowork SQLite/Workspace/Runtime identities |
| Tests | Large Desktop and conversation/permission E2E corpus | Desktop IPC/preload/Host/update/quit/runtime/installer tests | Use as test prompts; only LiteCowork evidence can qualify LiteCowork |

## 3. ZCode: architecture and reusable patterns

### 3.1 Actual process model

ZCode's architecture overview describes multiple workspace windows. Each BrowserWindow has exactly one window Host; each `workspaceKey` has at most one chat CLI. Renderer submits intent via service/protocol boundaries. CLI holds authoritative conversation/session runtime. Host aggregates local services, remote connection management, task-list projection and window attachments.

- **Main:** native windows/dialogs, external URLs/OAuth, Host process supervision and thin control/message forwarding. It does not own conversation rows, streams, queues, snapshots, goals or interaction state.
- **Renderer:** React layout and transient draft/scroll/focus state; it calls service proxies and renders snapshots/deltas, not Node/CLI/repositories as business APIs.
- **Window Host:** constructs local services, routes remote connections/sessions and owns attachment registries; it is not the conversation reducer.
- **CLI:** session/core runtime, command inbox, idempotency/CAS, projection reducer, conversation topics, queues, goals, interactions, tools and model runtime.

This is a strong reference for a thin shell and explicit authority owners. It does **not** justify making LiteCowork's UI window own durable Runtime. LiteCowork must retain the Operator/daemon split: closing the window can leave authorised background work running under the Runtime policy.

### 3.2 Exact code paths

| Pinned source | Observed design | Candidate use |
|---|---|---|
| [Desktop Main entry](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/packages/desktop/src/main/index.ts) | Native composition root, window config, Host wiring, policies and lifecycle | Study decomposition; do not transplant Electron entry |
| [Desktop Host adapter](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/packages/desktop/src/main/desktopHostProcess.ts) | Electron UtilityProcess, typed response schema, Host IDs, per-window map, callbacks/ports | Adapt structured process supervision and message validation |
| [Host initialization](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/packages/desktop/src/host/hostInitialization.ts) | Disposes unowned network transport under bounded phase when owner creation fails | Resource ownership and bounded partial-start rollback |
| [Host shutdown phases](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/packages/desktop/src/host/hostShutdownPhases.ts) | Named, bounded teardown phases | Cleanup inventory and per-phase diagnostics |
| [Architecture overview](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/docs/architecture/zcode-code-architecture-overview.md) | Current source-level identity and data-flow model | Compare Main/Renderer/Host/CLI against LiteCowork owners |

ZCode's main window sets `contextIsolation: true`, `nodeIntegration: false` and `webviewTag: false`. The plugin sandbox is a separate guest boundary; a secure parent window is not itself plugin isolation.

Its architecture distinguishes workspace identity from filesystem path: `workspaceKey = workspaceIdentity?.trim() || workspacePath`; the path is used for file IO. Remote connections can be pooled without merging workspace identities. Each attachment independently owns trusted delivery mode, subscription, sequence/backpressure and disposal. Disconnect preserves the last trusted projection as offline while mutations fail closed; reconnect replaces it from a fresh source snapshot. These details provide useful stale-generation and concurrency test cases, not new LiteCowork identity rules.

### 3.3 Plugin and interactive-content sandbox

The inspected `packages/desktop/src/main/pluginSandbox/CONTRACT.md` specifies a Main-owned plugin sandbox bound to WebContents, window/workspace/session/server/scope plus generated sandbox/init identifiers. It includes:

- Persistent Chromium partition derived from application identity.
- Scheme/origin and owner checks before guest attachment; owner frame and partition must match.
- Guest context isolation, sandbox and web security, with no Node integration or guest preload.
- Restrictive CSP and origin restrictions; ungranted camera, microphone, geolocation and clipboard-write access are blocked.
- Host-mediated JSON-RPC/MessagePort instead of broad Electron API exposure.
- Explicit activity and cleanup lifecycle tied to owner/session state.

This maps to LiteCowork's distinction between MCP Apps and generated interactive Artifacts. It is a reference for isolating untrusted UI and mediating capability calls, not permission for generated content to inherit the main renderer's privileges. Keep `TRUST.md`, `INTERACTIVE-ARTIFACT-RUNTIME.md`, `PRESENTATION-RUNTIME.md`, `ARTIFACTS-EVIDENCE.md`, Environment and Effect as owners. A Chromium partition alone is not OS process isolation, filesystem confinement, network egress control or Effect authorization.

### 3.4 Testing corpus

ZCode has many targeted tests under `packages/desktop/test`, including `e2e/conversation-session`, browser permission prompts, settings, remote workspaces, session resume, background work, file operations, model selection and renderer interaction. Use these to find missing user journeys. Translate relevant cases into LiteCowork's own Conversation, Task, Attempt, Effect, Grant, ExecutionLease, Environment, RuntimeIncarnation and receipt semantics. Upstream tests are not LiteCowork passes.

### 3.5 Can we reuse the React UI code directly?

The preserved LiteCowork UI package manifest is `@litecowork/desktop` 0.1.0 and uses React/React DOM `^19.1.0`, Vite `^6.3.5`, TypeScript `~5.8.3` and Tauri API `^2.8.0`. ZCode Desktop declares React/React DOM `^19.2.4`, Tailwind CSS `^4.2.2`, Vite `^8.0.2` and TypeScript `^6.0.2`; its shared `@zcode/ui` package also depends on internal workspace packages `@zcode/provider`, `@zcode/rpc`, `@zcode/services` and `@zcode/shared`, plus Radix, Motion, Zustand and other UI dependencies.

This gives us **framework-level compatibility, not drop-in component compatibility**. React 19 is common, but ZCode's UI is coupled to its own provider/service/protocol models, workspace packages and build versions. The archived LiteCowork UI does not declare those ZCode workspace packages. Importing the entire `@zcode/ui` package would pull in a second service/state model and substantial build/dependency changes. The practical reuse strategy is to port selected presentational components and styling into LiteCowork's own screen contracts, replace all data/service calls with existing typed Operator API/domain projections, and retain LiteCowork's Tauri boundary. Each candidate still needs file-level license/NOTICE review and an isolated comparison build before adoption.

DeepSeek's Desktop package manifest is an Electron shell around the bundled DSH Web application and Host; the desktop manifest itself does not expose a standalone React component library to add to LiteCowork. Treat its renderer/host integrations as architecture references unless a separately reviewed, genuinely framework-agnostic component is identified.
## 4. DeepSeek Harness: desktop integration patterns

### 4.1 Topology

The `apps/desktop` manifest describes an Electron shell for a bundled DSH runtime and external plugins. It loads the packaged Web application under `dsh-app://app/`. A separate Node-mode Host child starts the shared profile runner and provides the authenticated Web backend. Node IPC carries boot/readiness/failure/shutdown; Electron forwards application HTTP and WebSocket requests to the owned Host.

This differs from ZCode's per-window Utility Host and LiteCowork's durable daemon. It is a concrete shell/Host adapter example, not the source of LiteCowork Runtime identity or lifecycle.

### 4.2 Typed preload and sender-owned IPC

| Pinned source | Observed design | Candidate use |
|---|---|---|
| [Desktop Main](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/apps/desktop/src/main.ts) | Window config, lifecycle, sender checks and Host composition | Explicit native composition/API boundary |
| [IPC contract](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/apps/desktop/src/ipc.ts) | Named channels, typed API and `assertDesktopSender` checking scheme/hostname | Typed allowlist and caller ownership, not Electron IPC itself |
| [App preload](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/apps/desktop/src/preload-app.ts) | Narrow `contextBridge` calls/listeners for the owned app main frame | Least-authority renderer API and listener cleanup |
| [Host process](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/apps/desktop/src/host-process.ts) | Typed lifecycle messages, request IDs, control deadlines, task inspection and bounded stop | Update/quit sequence reference |
| [Quit confirmation](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/apps/desktop/src/quit-confirmation.ts) | Fail-safe prompt for active/scheduled work or unknown inspection; one pending prompt | UX and race-test reference |

The main window explicitly enables `contextIsolation`, `sandbox` and `webSecurity`, disables `nodeIntegration`, and enables `webviewTag` only for the primary window. New-window requests are denied (ordinary HTTP(S) opens externally); navigation is limited to the app scheme and narrow same-origin cases. Preload exposes individual methods, not raw IPC; Main validates the sender frame is an owned `dsh-app://` document.

LiteCowork does not use Electron, so `assertDesktopSender` is not a Tauri authorization mechanism. Transfer only the principle: explicit command allowlists, exact caller/window ownership, argument/schema validation, no generic shell/execute API, secrets outside WebView JS and owner-side authorization for every consequential operation. `LOCAL-OPERATOR-IPC.md` and `API.md` remain normative.

### 4.3 Update and quit handoff

The inspected Host/Main/quit code uses a conservative sequence:

1. Inspect whether active work would be interrupted.
2. Lock new Host/API request admission for install.
3. Drain already-admitted requests with a deadline.
4. Reinspect tasks so an earlier admitted write cannot silently create new active work.
5. Request Host shutdown and require positive completion acknowledgement.
6. Wait for process exit, escalating under bounded SIGTERM/SIGKILL deadlines.
7. Refuse installation on unclean or unacknowledged shutdown; retain retry/recovery path.
8. Treat failed/timed-out quit inspection as unknown and warn instead of assuming no work exists.

The checked source uses 10 seconds for graceful exit, then 5 seconds for SIGTERM and 5 seconds for SIGKILL; update-task control deadline is 10 seconds. These are **upstream values, not LiteCowork defaults**.

#### Reconciliation against LiteCowork

`RUNTIME-LIFECYCLE.md` already separates Operator shutdown from Runtime/worker lifecycle. The current documented stop path stops Operator admission and drains bounded in-flight handlers, but explicitly says Task/Attempt drain, Effect reconciliation, lease fencing/release and local service-reference settlement are **not integrated**; authenticated stop/stop-preview remain unimplemented until those owners are composed. A signal path is not proof that external work or Effects settled.

This is the material gap confirmed by the comparison. DeepSeek's ordering is a useful sequence pattern, but LiteCowork cannot adopt its simple task count. A future LiteCowork stop/update protocol must be designed by owners and reconcile admitted Tasks/Attempts, external Effects, ExecutionLeases/EnvironmentControlLeases, CapabilityActivation/service references, provider process quiescence and pending cleanup handles. An ambiguous external Effect remains UNKNOWN and blocks unsafe cleanup; it cannot be relabeled cancelled because a desktop process exited.

**No new lifecycle contract is approved here.** Keep `RUNTIME-LIFECYCLE`, `STATE-MACHINES`, `TRUST`, `TASK-RUNTIME`, `ENVIRONMENTS`, `EVENTS`, `STORAGE`, `API` and `FAILURE-RECOVERY` authoritative. Stop-preview/update changes require owner-specific state transitions, transactions, schemas, errors, tests and evidence before implementation.

### 4.4 Plugin management and file transfer

DeepSeek's desktop README describes a profile-owned plugin arrangement and CLI management while the app is quit. The Main/Renderer surface does not expose arbitrary pnpm arguments or direct plugin-management IPC; management goes through the authenticated Web app. This separation is worth studying, but LiteCowork has already selected **LiteSPM** as package lifecycle owner. LiteCowork owns scoped authorization to invoke a package. Neither UI nor a copied DSH plugin manager should create a second package database or lifecycle controller.

Critical caveat: DSH docs say its Host and plugins execute in the same Electron Node-mode process. That is not evidence of strong isolation for arbitrary community extensions. Do not copy it for untrusted skills, MCP servers, generated apps or connectors. Signed/pinned package trust, OS Environment isolation, process/network boundaries, Trust grants and Effect mediation remain separate.

DSH file drag is another candidate: a Session-scoped resource address is resolved by Host, local mapping verified, source checked by digest, and Main-owned transfer uses bounded temporary storage, cancellation and cleanup. Borrowed original files are not deleted. Map this to pinned ResourceRef/Artifact and Environment boundaries; do not expose ambient absolute paths or arbitrary filesystem methods to Agents.

## 5. Reuse/adaptation register

| Priority | Candidate pattern | Reference | LiteCowork owners to review | Decision now |
|---|---|---|---|---|
| P0 | Typed native command surface; caller ownership; strict input schemas | DSH IPC/preload | `LOCAL-OPERATOR-IPC`, `API`, `TRUST` | Adapt principle to Tauri; no raw transplant |
| P0 | Stop admission, drain, re-evaluate, prove quiescence, fail closed on UNKNOWN | DSH Host/update | `RUNTIME-LIFECYCLE`, `TASK-RUNTIME`, `STATE-MACHINES`, `TRUST`, `ENVIRONMENTS`, `FAILURE-RECOVERY` | Review checklist; owner contract still required |
| P0 | Guest/plugin ownership, identity partition, CSP, mediated bridge | ZCode plugin sandbox | `TRUST`, `INTERACTIVE-ARTIFACT-RUNTIME`, `PRESENTATION-RUNTIME`, `ARTIFACTS-EVIDENCE` | Adapt isolation ideas; OS sandbox separately qualified |
| P1 | Typed Host messages, request correlation, bounded cleanup phases | ZCode and DSH Host | `RUNTIME-LIFECYCLE`, `LOCAL-OPERATOR-IPC`, `FAILURE-RECOVERY` | Compare to existing owner contracts |
| P1 | Session-scoped file transfer, digest, byte budgets, abort and source-preserving cleanup | DSH file-drag | `WORLD-RESOURCES`, `ARTIFACTS-EVIDENCE`, `ENVIRONMENTS`, `STORAGE` | Candidate for ResourceRef/Artifact transfer |
| P1 | Scoped generations, independent subscriptions/backpressure and stale-owner rejection | ZCode Host attachments | `PROTOCOLS`, `API`, `EVENTS`, `RUNTIME-MESH` | Concurrency test inspiration; no V1 remote scope expansion |
| P1 | Conversation and permission regression/E2E case breadth | ZCode Desktop tests | `TESTING`, `ACCEPTANCE-AND-E2E-PLAN` | Port scenarios into LiteCowork semantics |
| P2 | Visual hierarchy, loading/empty/error, permission and action states | ZCode UI + DSH desktop | `DESKTOP-SHELL-LLD`, `V1-UI-SCREEN-CONTRACT`, ZCode UI audit | Reuse visual patterns after API/dependency/UX review |
| P2 | Profile-local plugin management | DSH profile model | `LITESPM-LITECOWORK-INTEROP-LLD`, `CAPABILITY-FABRIC` | Comparison only; LiteSPM remains sole owner |

### Do not copy

- Do not replace Tauri, local Operator API/IPC, Rust daemon, SQLite or Task/Attempt/Effect state machines with Electron-specific code.
- Do not make the UI window own the durable Runtime; authorised background work can outlive the Operator window.
- Do not replace LiteCowork identity/evidence with a ZCode workspace key, DSH profile or upstream conversation model.
- Do not run untrusted plugins in the same Node process as trusted Host.
- Do not make arbitrary WebView JS, browser guests, generated HTML, file paths or shell strings privileged by rendering.
- Do not treat an iframe/Chromium partition, Git worktree, app profile or origin allowlist as OS isolation.
- Do not copy permission defaults, blanket full-access/YOLO behavior, credentials, TLS exceptions or update timeouts without owner review.
- Do not add a second package manager. LiteSPM remains authoritative.

## 6. LiteCowork cross-document reconciliation

| Existing authority | Connection to findings | Required care |
|---|---|---|
| [ARCHITECTURE](../ARCHITECTURE.md), [PRODUCT](PRODUCT.md), [V1 release contract](V1-PRODUCT-RELEASE-CONTRACT.md) | Preserve local-first desktop and distinct product roles | Repos do not redefine V1 scope |
| [Runtime lifecycle](RUNTIME-LIFECYCLE.md), [Task runtime](TASK-RUNTIME.md), [State machines](STATE-MACHINES.md) | Host drain/exit is a useful comparison | Task/Effect/lease drain gap remains open |
| [Local Operator IPC](LOCAL-OPERATOR-IPC.md), [API](API.md) | Narrow commands, caller identity and input validation | Tauri OS peer auth differs from Electron sender-frame checks |
| [Trust](TRUST.md), [Security](SECURITY.md), [Environments](ENVIRONMENTS.md) | Guest ownership, scope, CSP and OS isolation | UI prompt is not authorization authority |
| [Interactive Artifact Runtime](INTERACTIVE-ARTIFACT-RUNTIME.md), [Presentation Runtime](PRESENTATION-RUNTIME.md), [Artifacts/Evidence](ARTIFACTS-EVIDENCE.md) | ZCode sandbox applies to untrusted interactive content | MCP Apps and generated Artifacts retain distinct owners |
| [LiteSPM interop](LITESPM-LITECOWORK-INTEROP-LLD.md), [LiteSPM plan](LITESPM-INTEGRATION-PLAN.md), [Capability Fabric](CAPABILITY-FABRIC.md) | DSH profile is a package-management comparison | LiteSPM owns install/update/remove/provider process; LiteCowork authorizes use |
| [World Resources](WORLD-RESOURCES.md), [Storage](STORAGE.md), [Artifacts](ARTIFACTS-EVIDENCE.md) | Digest-verified, scoped file handoff | Pinned identities and local Environment enforcement required |
| [Testing](TESTING.md), [Acceptance/E2E](ACCEPTANCE-AND-E2E-PLAN.md) | Upstream regression suites suggest scenarios | Only LiteCowork CODE/SYSTEM/USER evidence qualifies LiteCowork |

## 7. License and code-reuse gate

The cloned roots show ZCode Apache-2.0 with a root `NOTICE.md`, and DeepSeek Harness MIT. The active LiteCowork checkout has no root `LICENSE` file at the reviewed path. That absence does not by itself prohibit private/internal use of permissively licensed code, but it leaves the repository's intended outbound terms unclear and makes redistribution obligations especially important. Before selecting source, review each file's header, root/containing license, dependencies, generated code, fonts/icons/assets, notices and compatibility; preserve ZCode's Apache-2.0 NOTICE/attribution requirements and DeepSeek's MIT notice. Confirm the project's intended distribution/licensing posture with its owner before shipping adapted files. This is not legal advice and does not establish that any particular file is clean to copy.

**No upstream implementation file has been copied into the active LiteCowork source tree.** The clones stay local and ignored under `repo_take/` until an explicit, license-reviewed task selects exact files and an adapter plan. Reuse should be selective and provenance-preserving, not a wholesale repository transplant.

## 8. Validation and next gates

- Both clones were shallow and clean at the pins in §1; root `.gitignore` excludes `repo_take/`, and `git check-ignore` confirmed both roots.
- Desktop source/test paths and manifests were inspected; upstream test suites were **not executed**.
- No LiteCowork source, schema, migration or package manifest was changed by this research.
- Before reuse: select one owner/domain slice; map exact files and transitive dependencies; review licenses/notices; write a port/adapter SDD and acceptance tests; require owner approval; implement in an isolated clean worktree. Confirm the intended distribution posture before shipping adapted files.
- Before desktop shutdown/update is called working: integrate Task/Attempt, Effect, lease and capability-host settlement with Runtime owner, then run failure-injected SYSTEM and USER journeys on each declared OS. Process exit alone is not quiescence evidence.
