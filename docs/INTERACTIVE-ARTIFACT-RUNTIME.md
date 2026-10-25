# Interactive Artifact Runtime

**Status:** Normative V1 target; no implementation or OS qualification is implied. This contract owns the package, local run protocol, isolation boundary, lifecycle, fallback and Workbench behavior for generated interactive applications. It does not redefine Artifact versioning, Task authority, grants or Effects. Cloud execution, mobile continuation and run-state synchronization are post‑V1.

## Product boundary

An interactive app is an immutable ArtifactVersion containing a self-contained UI bundle. It is distinct from:

| Output | User intent | Runtime |
|---|---|---|
| Semantic answer | Read an answer in the Conversation | Conversation renderer |
| Rich presentation | Inspect a chart, diagram or formatted view | Constrained presentation renderer |
| File artifact | Open, edit or export a file | Type-specific Artifact viewer/editor |
| Interactive app | Operate a generated UI with controls and local state | Isolated Artifact Runtime after an explicit Run action |

Interactive app output is optional. The semantic answer remains complete if packaging, preview, rendering or execution fails. A saved application is not evidence that the app is safe or that its results are correct.

A run is a **volatile local runtime session**, not a Task, Attempt, Effect, approval, verification result or durable ArtifactRun aggregate. It is associated in memory with the exact Workspace, ArtifactVersion, Runtime incarnation and optional source Conversation/Task. It does not increment Artifact.version or current_version. No app is run automatically when a chat, Artifact, Workspace or app restarts. V1 does not show durable run history or resume prior application state across a Runtime restart. An abrupt process exit is surfaced as interrupted/unknown when the supervisor can establish it; no completion is inferred.

## Generation and publication

Interactive app generation uses only the selected AgentBinding and the exact agent/model/options/session or admitted Task route already shown in the composer. The current AgentControlDescriptor must report `output.interactive_app=SUPPORTED`; a stale or missing value, or a state other than SUPPORTED, means no app-generation promise. This is an output-delivery capability, not a safety, quality, correctness or execution claim. It does not imply that every model on that agent route supports app output.

The selected agent returns a typed candidate package through its qualified output path. LiteCowork validates the candidate against the package schema, archive/resource limits, content policy and digest rules, then publishes a new immutable `INTERACTIVE_APP` ArtifactVersion through the canonical ArtifactStore path. Publication failure preserves the semantic answer and presents source/text fallback with a bounded reason. The renderer never consumes an unvalidated partial stream or source file as executable output. Package arrival does not focus the Workbench or execute code.

There is no hidden agent, model, account, region or provider fallback to satisfy an app request. If the selected descriptor lacks this output capability, preserve the user request and offer ordinary text/source output or explain that app packaging is unavailable for this route. A user may explicitly choose another eligible agent and retry as a new native session or Task under the normal context-transfer and admission rules. Static preview and explicit Run remain later, separate decisions.

## Package contract

The only V1 executable package is a self-contained web bundle with a versioned manifest, one declared entry document, and normalized assets. The package has no install step and cannot fetch executable code, fonts or data from the network. The manifest is the fixed root manifest.json file and declares package schema version, entrypoint (which must also appear in the declared asset set), display name, supported host protocol range, packaged resource paths/digests and requested capabilities. Each packaged asset digest and the enclosing ArtifactVersion content digest use the canonical `sha256:` plus 64 lowercase hexadecimal characters encoding. The ArtifactVersion content digest binds the complete archive, including its manifest. Unknown schema versions or undeclared fields fail closed.

The validator rejects archive traversal, absolute paths, duplicate paths after platform normalization, links, special files, undeclared files, oversized or deeply nested archives, active remote references, scripts outside the declared bundle, and unsafe media. SVG is rejected unless a separate sanitizer profile has been qualified. Source files remain downloadable as an ordinary Artifact; source inspection is a secondary action, not the default viewing experience.

Package validation, safe static preview and runtime execution are separate decisions. Static preview never executes package scripts. If no safe static preview exists, show provenance, a neutral preview-unavailable state and source download. A package that fails validation cannot be Run.

## Runtime architecture and trust boundary

The logical path is:

1. ArtifactStore verifies and publishes the immutable package bytes and exact ArtifactVersion.
2. The Workbench loads that exact version through the authenticated read path and asks the local Runtime supervisor to prepare a run.
3. The supervisor rechecks Workspace access, Artifact status/version/digest, package validation, supported Runtime profile and current grants. TRANSIENT and SAVED Artifacts may run; ARCHIVED Artifacts remain readable/source-downloadable but are preview-only and cannot start a run.
4. The supervisor starts a dedicated unprivileged renderer under an OS-specific isolation profile. It binds a unique run ID to that exact ArtifactVersion and creates an isolated origin/profile. No main-app renderer, browser profile, Tauri command surface, Node/native bridge or agent session is reused.
5. The untrusted renderer receives only the versioned message protocol in [interactive-artifact-runtime.schema.json](schemas/interactive-artifact-runtime.schema.json). The supervisor mediates every message and the host UI remains outside the sandbox.
6. Stop, revocation, Workspace switch, Artifact-version mismatch, process exit, policy failure or budget breach terminates the renderer and revokes its run-scoped broker handles.

Origin separation alone is not OS isolation. Each claimed OS needs evidence that the artifact renderer has a separate process boundary, denies ambient filesystem/process/network access, cannot reach app-private IPC, and can be killed with all child processes. If an OS cannot prove the required profile, that OS exposes source/static preview only. An implementation may use a platform WebView only if its exact version and launch configuration meet these properties.

The sandbox has no ambient access to local files, shell, clipboard, browser profiles, environment variables, OS automation, screenshots, passwords, account tokens, Workspace secrets, agent credentials, native MCP servers, extension APIs or unrestricted loopback services. No app-defined code runs in the privileged desktop renderer. Browser navigation opens an external link only after an owner click and uses the normal safe-link policy; it does not grant the app web access.

The baseline Content Security Policy is deny-by-default: no network connections, remote fonts/media/scripts, forms, top-level navigation, objects, workers, eval or dynamic code loading. Only bundled, digest-verified resources may load. Any relaxation requires a new reviewed profile and security qualification; an Artifact cannot request the relaxation itself.

## Host protocol and local API

The local interface is a closed, versioned IPC protocol, not an HTTP endpoint exposed to the package and not an Operator REST route. Each envelope has protocol version, run ID, direction-local monotonic sequence, message type and a closed payload. The supervisor rejects wrong-run, duplicate, out-of-order, over-size, unknown-version and unknown-message input. Message-size and resource ceilings are chosen in the per-OS qualification record; an unmeasured or exceeded ceiling fails closed.

V1 messages are:

| Message | Direction | Meaning |
|---|---|---|
| start | Host to renderer | Exact Workspace/Artifact/version/digest, ephemeral run ID, approved resource handles and protocol version |
| stop | Host to renderer | Idempotent request to terminate; the supervisor still kills the process if it does not exit |
| read_result | Host to renderer | Exact requested Resource bytes or a bounded denial/unavailable result for a host-issued handle |
| save_result | Host to renderer | Sanitized saved/cancelled/conflict/rejected result for the matching proposal; no credentials |
| ready | Renderer to host | Renderer handshake with the app's declared host protocol version |
| status | Renderer to host | Bounded readiness/accessibility signal; never model-authored task progress or completion |
| read_request | Renderer to host | Request to read one exact host-issued immutable ResourceRevision handle |
| save_request | Renderer to host | Proposed output name, media type, digest and bounded bytes for explicit Artifact save review |
| failure | Renderer to host | Bounded diagnostic code and message; no source contents, secrets or arbitrary paths |

The V1 capability allowlist is deliberately narrow:

- No arbitrary network, browser, shell, filesystem, MCP, plugin, agent, account, or computer-use capability.
- Read access is absent by default. An owner may select same-Workspace immutable source revisions in the host UI before Run; the broker exposes only those exact revisions through short-lived, read-only handles. No path is sent to the sandbox.
- Save is a proposal, not a write. The host validates the digest, strict UTF-8 content and a closed V1 non-executable media allowlist of text/plain and application/json (JSON must parse without duplicate object keys), then shows an owner-visible preview. HTML, SVG, JavaScript, WebAssembly, archives and executable formats are rejected. Confirmation creates a separate managed FILE Artifact through ArtifactStore.create, with the exact running ArtifactVersion pinned as provenance input, the source Artifact's Task link when present (otherwise no Task link), and an authenticated CommandContext/RequestId receipt. The app cannot create a Task or change its status. It never updates or replaces the running app package. The owner can cancel; an ambiguous result retries/reconciles the same receipt and never creates a duplicate.
- There is no V1 external Effect capability. A future mediated action must define a separate typed operation, CapabilityGrant scope, owner confirmation policy, idempotency, reconciliation, audit and OS qualification. Clicking an app control is never sufficient authorization.

The IPC protocol and all payloads are defined by the linked JSON Schema. The Workbench invokes prepare/start/get_status/stop/review_save_proposal through the local Runtime owner; it never calls renderer messages as if they were trusted Operator commands. Review-save receives a host-generated request ID bound to the exact payload and owner decision; the renderer’s request ID is correlation only. Run identity, state and sequence remain in volatile Runtime memory. DomainEvents and Task status do not represent render progress. A future durable run history or remotely controlled run requires a separate data model, API, event, migration, retention and threat-model decision.

## Lifecycle

| State | Entry condition | Allowed next state | User-visible behavior |
|---|---|---|---|
| PREVIEW_ONLY | Artifact selected; no run requested or qualification unavailable | STARTING only after explicit Run and all checks | Static preview or honest unsupported reason; no executing scripts |
| STARTING | Owner chose Run; admission checks passed; isolated process is being established | RUNNING, FAILED, STOPPING, REVOKED | Show “Starting”; Stop remains available |
| RUNNING | Renderer handshake, exact version and isolation profile verified | STOPPING, FAILED, REVOKED, INTERRUPTED | Show app title/version and active state; host controls remain visible |
| STOPPING | Owner chose Stop or a lifecycle fence is firing | STOPPED, FAILED, INTERRUPTED | Disable app input and show termination progress |
| STOPPED | Supervisor confirmed process tree quiescence | STARTING only after another explicit Run | Show stopped state; no background work |
| FAILED | Admission/runtime/protocol failure or budget exceeded | STARTING only after correction and explicit retry | Show safe failure code, recovery action and static fallback |
| REVOKED | Required Workspace/resource authority was revoked or version access lost | Terminal for this run | Clear content/cache and return to unavailable/static state |
| INTERRUPTED | Supervisor detects unclean process loss or cannot prove quiescence | Terminal for this run | Show outcome unknown; never claim app completed or auto-restart |

A successful Stop means the renderer and all descendants are quiescent, broker handles are revoked, and no run-owned process can continue work. If quiescence cannot be proved, report interruption and keep cleanup pending; do not report stopped. A new Run always creates a new run ID and rechecks the current exact ArtifactVersion and current authority.

## App state and persistence

Application state is volatile by default and is never synchronized. The V1 target may offer an explicit “Remember this app’s data on this device” choice backed by a quota-limited, deletable store partitioned by Workspace, Artifact ID and ArtifactVersion. It is opt-in, separate from Artifact source, cannot store credentials, and is erased on owner deletion or Workspace deletion according to the retention contract. Changing the ArtifactVersion starts with a fresh namespace; migration of user data requires an explicit, previewed owner action. The application cannot choose another partition or inspect another app’s data.

The isolated store is not a native browser profile and does not enable cookies, service workers, push, background timers, file downloads or offline network requests. If the OS/runtime cannot enforce isolation and deletion for this store, the preference is unavailable and state remains volatile. Persistent state never causes execution at startup.

## Workbench behavior and visual quality

The right Workbench remains the single desktop work surface. Selecting an interactive Artifact opens an inert preview with title, exact version, source/provenance, package validation, declared capabilities marked informational only, and the primary Run action. No capability is granted by opening the preview or pressing Run. The app only replaces that preview after an explicit Run. A running view has persistent host-owned title/version, Stop, status, and a visible route back to source/static preview. App chrome cannot cover or imitate these controls.

The UI distinguishes package-invalid, preview-only, starting, running, stopping, stopped, failed, interrupted, revoked, needs-auth, unavailable and inaccessible-version states. A newer Artifact head is not presented as invalidating a selected immutable historical version. Errors name the failed boundary and a useful next step without exposing internal paths or secrets. App arrival never steals focus or opens the Workbench. Save prompts show proposed filename, media type, size, destination Artifact and resulting version; an app cannot dismiss or spoof that host prompt.

Keyboard users can reach Run, Stop, source, fallback and save controls without entering the sandbox first. The sandbox has a predictable focus entry/return path; focus traps remain within real host dialogs, not inside app content. The renderer must preserve zoom, text scaling, high contrast, reduced motion, screen-reader names/roles and visible focus. Animations respect reduced motion and never delay Run/Stop feedback. Narrow desktop windows use a single Workbench panel; no phone runtime or cross-device handoff is implied.

First useful frame, time-to-interactive, cold/warm startup, CPU, peak memory, frame drops, package bytes, crash rate and termination/quiescence time are measured on each claimed OS/runtime profile. Publish raw baselines and hardware/build details; do not advertise a speed or “native app” claim before qualification. Numeric ceilings and startup targets are owned by that qualification record, not guessed here.

## Failure and recovery rules

- Invalid manifest, digest mismatch, unsupported host protocol or dangerous archive shape: refuse Run; keep the Artifact and semantic response; show static/source fallback.
- Missing OS isolation, Runtime offline or unsupported renderer build: preview-only; do not retry in the main app renderer.
- Capability denied or revoked: keep the app running only if the operation is optional and the host can prove the sandbox has no access; otherwise terminate and show the blocker.
- Malformed, oversized or replayed IPC message: reject, revoke the run and record only a bounded redacted diagnostic.
- Process hang, crash or resource ceiling breach: kill the process tree, revoke broker handles, clear volatile state and show failed/interrupted. Never retry automatically.
- Artifact head advances while preview is open: keep the selected immutable version pinned and labeled; Run may use that exact version after reauthorization if it remains accessible and the Artifact is not archived. Never substitute the new head silently.
- Workspace switch or authorization loss: stop run, revoke handles and clear cached bytes before rendering another Workspace.
- Save request reuses an app correlation ID with changed bytes: reject it as a protocol conflict. If ArtifactStore.create returns an ambiguous outcome, retain the exact host-generated RequestId and payload digest, reconcile that receipt, and never create a second FILE Artifact. A canceled owner confirmation creates no Artifact.

## Qualification and release evidence

E08-S09 owns this runtime target; E08-S07 owns general Artifact history/view/edit and Workbench foundations. Release requires CODE, SYSTEM and owner USER evidence for the same packaged desktop build, exact Runtime profile, WebView/process version and OS build. Required hostile cases include cross-Workspace references, archive bombs/traversal, path/case collisions, XSS/CSP bypass attempts, remote-code fetches, compromised/replayed IPC, broker-handle reuse, revoked authority during display, concurrent Artifact edit, process descendants, OS lock/sleep, crash during stop, storage deletion, high zoom, keyboard/screen-reader navigation and reduced motion.

No generated artifact can report its own safety, completion or verification. A runtime qualification does not qualify the user’s native agent, delegated worker, provider, package, model or Task verification route.