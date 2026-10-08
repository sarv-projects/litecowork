# Current run and agent handoff

Update this file when the active story, implementation state, or handoff changes. It is
operational context; architecture and owning domain contracts remain authoritative.

## Active desktop/local status snapshot (2026-10-08)

This is the current source-level status, superseding older chronological entries below.
The owner has clarified that V1 is the complete desktop/local product; cloud continuation
and remote Runtime are post-V1 and are excluded from the V1 release gate. E11/E12 remain
planned post-V1 work. Local capabilities already committed in the architecture remain in
V1 scope.
The owner has explicitly deferred builds, tests, formatters, validators, migrations, and
real provider/OS checks until a later verification pass. None of the source work in this
snapshot should be described as compiled, tested, OS-qualified, or production-ready.

### Latest auth and desktop connection review (2026-10-08)

Re-read `AGENTS.md`, `ARCHITECTURE.md`, `docs/API.md`, `docs/LOCAL-OPERATOR-IPC.md`,
`docs/NETWORK-SECURITY.md`, and the current Unix IPC, Operator, daemon startup, and Tauri
client paths. The source review found no concrete fail-open in the Linux/macOS ingress
path: the daemon checks kernel peer UID before reading a frame, the Tauri client checks
the server UID before writing, frame methods/paths/headers and sizes are bounded, and the
private authentication marker is added only by the accepted IPC dispatcher before the
normal Operator handlers run. This does not establish per-process or per-login-session
identity: all processes with the authorized UID remain inside the documented local trust
boundary. Windows continues to fail closed until named-pipe/logon-SID support exists.

The Linux desktop service check now validates the exact generated unit and the systemd
MainPID command line against the selected `litecoworkd` executable and data directory; a
mismatch is surfaced as `DEGRADED` with `LINUX_SYSTEMD_SERVICE_UNVERIFIED`. This catches
an active service left on a stale ExecStart because `systemctl start` does not restart an
already-active unit. These are source-level findings only. No build, test, formatter,
validator, or OS command was run; IPC, daemon authentication, service-manager behavior,
and the updated Tauri status path remain unqualified.

### Linux systemd graceful-stop budget alignment (2026-10-08)

Source review found a shutdown budget mismatch: `OperatorServer` permits already-admitted
handlers to drain for up to 60 seconds, while the generated Linux user unit had
`TimeoutStopSec=10s`. That allowed systemd to SIGKILL `litecoworkd` before the documented
graceful drain could finish. The generated unit now uses 90 seconds, leaving a bounded
30-second margin for final Runtime/incarnation persistence. A source unit-rendering test
asserts the 90-second value exceeds the 60-second drain and retains control-group kill
semantics. `docs/RUNTIME-LIFECYCLE.md`, `docs/FAILURE-RECOVERY.md`, and E01 now describe the
forced-expiry path as unclean and require restart recovery. No checks or builds were run per
the owner's deferral; systemd behavior remains unqualified, and a stalled filesystem/store
can still exceed the deadline and force an unclean stop.

### Implemented in source; verification still open

- `litecoworkd` has a single-instance lifecycle, local storage/bootstrap, authenticated
  Unix IPC Operator transport, Runtime status/readiness, and Tauri on-demand launch/status.
  The Tauri startup probe now observes the spawned daemon through an authenticated
  readiness handshake. Linux status additionally checks the exact service unit and
  MainPID command; Windows IPC and packaging/service qualification remain open.
- Workspace/Library source includes workspace selection/settings, Resource metadata and
  text preview, resumable bounded multi-file/folder uploads, exact Resource revision
  selection for saved Task inputs, bounded on-demand search, encrypted deterministic
  lexical indexing for eligible plain text, and persistent WorkspaceRoot create/list/
  pause/resume/revoke source paths. Native folder selection uses the OS chooser and private
  local IPC; daemon startup revalidates saved directory identity, and fresh resume reopens
  and checks the directory before its atomic commit. These filesystem paths remain
  unbuilt/OS-unqualified, have no watcher or content reader, and Windows fails closed. A
  one-time selected-folder copy is not a persistent root. Rich-document extraction,
  semantic RAG, local inference, and safe ZIP extraction are not integrated; ZIPs remain
  opaque Resources and the UI reports extraction unavailable.
- Tasks can be created, have eligible unplanned `READY` objectives revised, and read
  immutable plan/step projections. Planner reservation/initial-plan acceptance storage
  exists, but no trusted producer-scoped planning dispatch is exposed; no Task starts an
  agent, materializes an execution Attempt/lease/Environment/Invocation, pauses/cancels a
  live worker, resumes, or performs a lead handoff. Task detail also loads exact TaskSpec
  revision history through an authenticated Workspace-scoped read bridge; it is read-only
  and does not restore or mutate history.
- A standalone Linux process-scope source primitive gates native-agent exec on same-PID
  cgroup membership and exact kernel memory/CPU/process limits. Its pre-GO cleanup now
  requires a reaped launcher plus positive unit-inactive/dead or cgroup `populated=0`
  evidence; unobservable cleanup retains a retry handle. Independent source review found no
  pre-GO success path based only on launcher reaping. This remains Linux-only, unqualified,
  unconnected to Task/Attempt/Environment/Trust, and does not prevent same-user filesystem or
  systemd-bus escape; it is not production-safe switching or a hostile-code sandbox.
- Codex installation inventory and bounded profile probing are present. OpenCode inventory
  recognizes its bare `--version` output, and an explicit authenticated local profile probe
  now reads only its provider-status/configured-catalog routes, returns bounded sanitized
  provider/model IDs and display names plus reported connected IDs, and publishes only an
  incompatible offer (`authentication=UNKNOWN`, model selection not qualified). The probe
  cannot create/enable an AgentBinding or start inference. The desktop now exposes explicit
  Codex and OpenCode probe actions and a sanitized OpenCode catalog detail panel, with a
  fresh profile/offer refresh after probing. Neither probe provides an integrated
  AgentSession supervisor, Task runner, supported inference entitlement, isolated
  Environment, mediated native tool set, process-tree containment, or production-safe
  cross-agent switching. All source remains unbuilt and unverified.
- Effect/Evidence domain and SQLite persistence source records fenced proposals, state
  transitions, and append-only Runtime-authenticated `REPORTED` Evidence. The current
  Runtime path cannot assert independent `OBSERVED`/`VERIFIED` assurance. Trust
  ApprovalUse/CapabilityInvocation admission, provider dispatch, independent verifier and
  observer authorities, Effect reconciliation, and retry admission remain unavailable.
- Coworker and Goal settings, Routine definitions, Automation definition editing/history,
  Artifact Library/Workbench, and Task Presentation snapshots are mounted in the desktop
  source. The Workbench now has a narrow authenticated text-version publication path:
  `GET /v1/artifacts/{id}/edit-head` and `POST /v1/artifacts/{id}/text-version` are wired
  through finite Tauri commands to the atomic immutable Artifact/Resource append store.
  Editing is limited to managed `text/plain` content up to 1 MiB, preserves drafts on
  conflicts, and requires explicit reload/rebase before retry. It does not edit HTML,
  external Resource content, or arbitrary Artifact types. This path is source-only and
  remains unverified. Automation creation remains PAUSED; there is no trigger host,
  occurrence-to-Task scheduler, or Automation run. A read-only, owner- and local-Runtime-
  scoped DelegationProfile catalog is now connected to Coworker Settings. Enabled profiles
  can be assigned; disabled assignments can be removed; archived/unknown assignments are
  surfaced for clearing. Profile creation, immutable revision, duplicate, and lifecycle
  status now have daemon/SQLite source paths with idempotency and version preconditions.
  The desktop editor exposes create/revise/duplicate/disable/archive; created profiles
  remain disabled, session options are empty until negotiated, and no model override is
  exposed. Profile enablement is explicitly fail-closed until adapter, Trust, Environment, and execution
  admission are integrated. Automation definition create/revise now pins an exact
  Workspace Coworker revision or explicit null; the editor preserves an existing pin
  unless the owner deliberately changes it. Automations still save PAUSED and cannot
  run or create Tasks. Workspace instruction history now displays exact source Resource
  and revision, content digest, and parent provenance while clearly distinguishing saved
  guidance from Task Context actually consumed. Generic ContextDocument edit/revocation/
  deletion and session invalidation remain unavailable. The shared SQLite Resource
  content-read admission now rejects non-ACTIVE ContextDocuments before BlobStore access;
  owner metadata remains readable and errors distinguish retained revoked content from
  deletion in progress/completed. If an admitted BlobStore read fails, the store rechecks
  current ContextDocument status; a proven transition maps to that status error, while an
  active/unproven status preserves the original failure and the Operator reports location
  unavailability separately from integrity failure. Rebuild metadata admission also
  checks status before classifying oversized/unsupported sources. A read admitted while
  ACTIVE may finish after a concurrent status transition, but the final index transaction
  CAS rejects publication unless status remains ACTIVE at the pinned Resource head. This
  is source-only and unverified; no tests, builds, formatters, validators, or provider/OS
  checks were run.
  Default lead selection is kept in the Agent Catalog
  binding controls; the duplicate Workspace-level selector was removed. This does not
  make workers runnable or safe switching available. Artifact promotion/archive remain
  unavailable. The Workbench additionally lists the ten most recent immutable Artifact
  versions using exact authenticated reads, and supports a narrow text-only
  restore-as-new-version draft for historical managed `text/plain` content up to 1 MiB.
  The owner confirms copying the selected historical text, reviews a draft based on a fresh
  current edit head, then explicitly publishes through the existing append path; immutable
  history and stale-head conflicts remain intact. That API records `user.text_edit` but
  does not persist a separate `restored_from` relation. External and non-text restore,
  general rich-document editing, and Artifact promotion/archive remain unavailable.
  Adjacent managed text versions can also be compared side by side with the same bounded
  safe renderer, exact Artifact/ResourceRevision labels, and no claimed changed-line
  detection; recent history shows recorded source count, provider, and Attempt when present.
  Presentation refresh remains a committed snapshot, not a live event stream.
- Home now displays the selected Coworker in the Task composer and offers a selector when
  the Workspace has multiple active Coworkers or a paginated roster. It supports explicit
  Workspace defaults, pins the chosen Coworker ID/version into Task creation, refreshes
  stale selections without rewriting the draft, and does not fall back from archived,
  stale, or unavailable selections. Ambiguous retry matching includes that exact selection.
  The UI source is unbuilt and unverified; this remains Task creation only and does not
  start an AgentSession or Task execution.
- Home now checks the committed Task receipt against the submitted Workspace/Coworker
  provenance, objective, and ordered Resource inputs in both the Tauri bridge and React
  before clearing the draft. A same-window retry retains its original RequestId and pinned
  identity after an ambiguous response; the retry envelope is not persisted across app
  restart, so restart recovery still requires checking Work. This is a Task-creation
  safeguard, not planner or agent execution.
- Resource batch import now reports each selected file/relative folder path and counts
  recovered committed uploads in partial-failure messages. The Task Presentation panel
  validates individual items before rendering and refreshes its saved snapshot while
  visible; ordinary Task cards display plain-language status labels. Neither change
  starts Task execution or provides a live stream.
- The Library now offers a client-side Pause upload action between resumable upload
  requests. It waits for the in-flight request, starts no later chunk/file/commit, retains
  resumable metadata, and reports a commit that wins while the request is in flight. It
  does not cancel or delete the server upload session; the same unchanged file must be
  reselected to resume. This UI change is unbuilt and untested.
- The bounded Artifact text editor now pins Artifact and backing Resource heads, publishes
  through the authenticated Operator and Tauri IPC, and reports a committed/replayed
  receipt. The desktop client preserves drafts on conflicts and does not silently rebase.
  It is restricted to managed plain text up to 1 MiB. Source wiring has been reviewed, but
  no compilation, test, or native IPC verification has been performed.
- The Library exposes metadata, bounded on-demand text, and encrypted local indexed-text
  search modes through the existing Tauri command. Indexed search remains deterministic
  lexical retrieval; PDFs, office documents, OCR, ZIP contents, embeddings, and semantic
  RAG are not integrated.
- Task Presentation now refreshes its saved snapshot every five seconds while the panel
  is in the foreground viewport. Requests are serialized, stale responses are ignored,
  and polling stops while hidden or unmounted. This does not create a live stream or
  synthesize progress. The panel now leads with the saved Task objective/status, committed
  outputs, and a short activity summary; the full activity and source IDs are collapsed.
  Stale/unknown snapshot freshness is labeled separately. Activity count says “items,”
  not “steps,” because the projection can contain more than Step records. These are
  presentation refinements only; the panel cannot show result text or blocker detail not
  present in the snapshot. A selected Task's last valid snapshot now stays visible when the
  local Runtime disconnects or a refresh fails, with an explicit possibly-out-of-date
  notice; changing Task identity clears that snapshot. No build or verification was run.
- Managed Markdown Artifacts now have a bounded structured preview for headings, paragraphs,
  lists, blockquotes, fenced code, inline emphasis/code, and explicitly confirmed HTTPS
  links. Rendering uses React text nodes without HTML injection or remote image loading;
  unsupported/malformed syntax, nested lists, and indented-list structure fall back to
  original text. Heading parsing preserves terminal hash characters (for example `# C#`)
  by falling back to the source when that syntax is outside the supported subset. This is a
  preview only, not a Markdown editor or full CommonMark renderer. No build or verification
  was run.
- The Ideas surface lists owner- and Workspace-scoped Suggestions through an authenticated
  route, with visible/snoozed/all filters, bounded filter-bound cursors, and pinned
  provenance. Dismiss, snooze, unsnooze, and acceptance now have authenticated,
  idempotent/replay-safe command paths. Acceptance atomically creates an ordinary READY
  Task with pinned inputs, criteria, optional Goal/Coworker provenance, and the Coworker's
  failover policy; it does not plan or execute the Task. The desktop preserves the original
  RequestId (and snooze target) after response loss. Expired proposals are omitted from
  the actionable view and are durably settled to EXPIRED by a bounded event-backed sweep;
  the page fails retryably if due items remain unsettled. Workspace kind preferences now
  have virtual unmuted defaults at version 0, authenticated list/update routes, and atomic
  preference plus current-proposal dismissal events/snapshots/receipt. The Ideas page
  exposes mute/unmute settings; muting clears current proposals of that kind and unmuting
  applies only to future proposals. Producers/proposal admission, Routine/Automation draft
  acceptance, and Task planning/execution remain unavailable, so no producer-backed Ideas
  are currently generated and acceptance does not start work. Source is unbuilt and
  unverified; no tests, builds, formatters, validators, migrations, or provider checks were
  run. A source audit confirmed that Task `COMPLETED` alone is not a valid producer trigger:
  Goal projection currently marks completed Tasks `UNVERIFIED`, and the verified outcome,
  producer registration/trigger, candidate admission, and atomic proposal persistence
  interfaces are not implemented. Do not add a producer until it consumes authoritative
  verified outcome evidence and SuggestionService can atomically enforce provenance,
  mute, dedupe, cooldown, expiry, and event/snapshot persistence.
- Coworker-origin standalone Task creation now resolves the Coworker's default lead
  failover policy server-side and pins that policy in the initial TaskSpec while the
  transaction rechecks the exact Coworker revision. The Coworker expected version is
  optional for direct callers, but if provided must still match at admission. Task detail
  resolves the exact immutable Coworker revision to show its historical name and revision;
  it does not substitute the current Coworker name if history cannot be loaded. After a
  lost Task-create response, the Home composer retries the original RequestId and pinned
  Coworker/lead envelope only while Workspace, objective, and ordered pinned inputs still
  match. A changed draft is blocked until the owner opens the original Workspace's Work
  list and explicitly clears the retry identity after reviewing for a possible duplicate.
  Before clearing the draft or navigating to the Task, the frontend independently checks
  the returned Workspace/Coworker identity, objective, and ordered Resource refs in
  addition to the Tauri bridge's TaskSpec identity checks. The retry envelope is still
  in-memory only; after application restart the owner must inspect Work before creating a
  potentially duplicate Task. These changes remain source-only and unverified.
- DelegationProfile writes now have a domain service and storage port, a SQLite
  idempotent immutable-revision/event transaction, and authenticated daemon routes for
  create/revise/duplicate/status. The Tauri bridge exposes supported create, revise,
  duplicate, disable, and archive operations with fixed paths, bounded JSON/response
  bodies, Workspace scoping, idempotency keys, and `If-Match` version checks. Agent
  Settings can create/revise conservative disabled profiles, duplicate into a new
  disabled profile, disable, and archive after confirmation. There is no enable action;
  enablement remains fail-closed. Domain name normalization uses NFC and Unicode case
  folding; dependency lockfile resolution remains pending because verification was
  deferred.
  Source-only review is in progress; no verification was run.
- The OpenCode Server transport now waits for a bounded listening marker from its owned
  process before sending the generated Basic-auth credential, reducing the local
  port-bind impersonation race. It remains transport-only: startup text is not OS peer
  authentication, process-tree containment, Environment isolation, or lease fencing.
- OpenCode inventory now accepts the documented bare semver output from `opencode
  --version` through the existing bounded/sanitizing parser. OpenCode profile probing is
  still unavailable; the current contract authorizes explicit probing only for Codex.
  This does not start an OpenCode session or enable Task execution. No checks were run.
- Agent Settings now lets the owner set an enabled, lead-eligible Workspace binding as
  the default lead through the existing versioned Workspace command. The update affects
  only future admissions; active Tasks remain pinned. Conflicts use the existing
  Workspace refresh/error path. No build, typecheck, or UI check was run.
- The desktop frontend toolchain is pinned to Node.js 24.21.0 (`.node-version`, constrained
  by `package.json` engines) and pnpm 12.10.1 (`packageManager`). `pnpm-workspace.yaml`
  sets `nodeVersion: 24.21.0` and `engineStrict: true`; pnpm 12 project settings belong
  in this file, while `.npmrc` is auth/registry-only. `pnpm-lock.yaml` was
  generated with `npx --yes pnpm@12.10.1 install --lockfile-only --ignore-scripts`; this
  resolved dependency metadata without installing project packages or running lifecycle
  scripts. Official Node.js release data identifies 24.21.0 as LTS; pnpm 12 supports Node
  22+. A frozen-lockfile install and actual Node 24 Tauri/Vite build remain deferred, so
  reproducible installation/build acceptance is not yet claimed. The pnpm workspace engine
  policy was added after lock generation; the lockfile was deliberately not regenerated
  because dependency declarations did not change, but frozen compatibility still needs
  verification. npm is unsupported; its `engines` enforcement requires separate npm
  configuration and LiteCowork provides only the pnpm lockfile. No tests or checks were run
  for this source/config slice.
- Graceful Runtime stop is not exposed as an Operator operation. Runtime has a
  process-supervisor shutdown path, but the Operator lacks Attempt/lease/Effect and local
  service-duty reconciliation needed to establish a safe drain. No desktop stop action
  should be added until those owners exist. This was confirmed by source and contract
  review only; no lifecycle integration was implemented.

### Current implementation ordering

1. Finish source-level integration review of the new TaskSpec-history bridge/UI and
   Artifact comparison/provenance; verification remains deferred by the owner.
2. Implement a producer-authenticated Task planning dispatch only after its process
   containment, Task Environment, native capability policy, session settlement, and
   recovery/fencing prerequisites are real; otherwise leave it visibly unavailable.
3. Implement Attempt/lease admission and bounded Attempt execution with Trust-mediated
   capabilities, durable Invocation/Effect reconciliation, and verified settlement.
4. Qualify desktop resource-root observation and safe ZIP extraction; then integrate local
   semantic retrieval behind an explicit local-model/parser provider boundary. The current
   Library's indexed mode is deterministic lexical retrieval only.
5. Add local browser/computer control through the Environment control lease. Artifact
   editing now covers only managed plain text; broader document editing/publication,
   promotion, and archive operations remain open.
6. Implement Routine/Automation occurrence hosting and Task admission, then close the
   desktop/local V1 release gate. Cloud continuation and remote Runtime are post-V1.

This snapshot is a navigation aid, not a substitute for rereading the owning contracts.
Source review does not prove correctness. The complete code/system/owner-acceptance
verification and production-readiness gates remain open.

## Artifact append storage slice (2026-10-08)

Added the `ArtifactVersionWriteStore` SQLite adapter path for user-authored managed
`text/plain` Artifact content up to 1 MiB. It verifies the content BlobRef before entering
the writer transaction, checks Workspace ownership and RequestId replay, rechecks the
Artifact aggregate/content version and exact backing Resource head, then atomically adds
the ResourceRevision ancestry, ArtifactVersion, dependency edges/invalidation rows,
Resource and Artifact events, aggregate snapshots, and idempotency receipt. Existing
versions remain immutable. The write port is re-exported from storage-core.

No authenticated Operator/API/Tauri editor or save control was added; Artifact Workbench
remains read-only. Promotion/archive and external-resource publication are unsupported.
No tests, builds, formatters, validators, migrations, or provider checks were run, so the
source transaction remains unverified and is not production-qualified.

## ZIP intake boundary correction (2026-10-08)

Static review found that the current local Operator router, Tauri invoke handler, and
Workspace Library already mount the ZIP-readiness route/bridge/notice, while
`docs/WORLD-RESOURCES.md`, `docs/SERVICES.md`, and the capability README still said those
surfaces awaited registration. Those stale statements are corrected. The notice also now
has its imported stylesheet and distinguishes a real unavailable response, a failed status
request, and the initial status check.

This does **not** integrate extraction. The Python parser remains an unactivated capability
with cooperative limits; it has no supervised isolated host, hard OS resource limits,
cancellation/crash settlement, or Core child-Resource publication/deletion path. ZIP bytes
remain opaque Resources; no members are indexed or attached. No tests, builds, formatters,
or validators were run under the deferred-verification instruction. The source and status
surface remain unverified, and ZIP extraction is not production-ready.

## Desktop Goals surface and authenticated bridge (2026-10-08)

Added a Workspace-scoped Goals page to the desktop shell with list/create/revise and
owner-requested status controls, plus a typed REST client and finite Tauri IPC bridge to
the authenticated local Operator. The view rejects cross-Workspace responses and cancels
stale requests on Workspace changes. It handles empty, loading, offline/API error and
conflict states, and clearly states that saving a Goal does not create or start a Task.
Progress is shown only when a valid Task/Evidence projection is supplied; a null/missing
projection is represented as unavailable rather than a fabricated zero or percentage.
Revision edits preserve related Task and Routine revision references. Target horizons
preserve their instant while converting between UTC API values and the local date/time
editor. Goal pages support cursor loading; stale Workspace and detail responses are
discarded. Mutations fail visibly if secure request-ID generation is unavailable.

This is a UI/transport slice only. The Goal persistence/Operator route work is concurrent
and must be reviewed for exact wire compatibility before this is considered integrated.
There is no Task creation/automation from a Goal, no explicit related-work editor in the
Goal form, and no mobile surface. No tests, builds, typechecks, formatters, or validators
were run under the current deferred-verification instruction. Add API decoder/bridge
contract tests and real desktop cases for Workspace switching, owner authorization,
create/revision replay, version conflict, lifecycle status, offline daemon, and null versus
Evidence-derived progress before release.

## Codex planning transport policy (2026-10-08)

The Runtime-local Codex App Server transport now has typed constructors for a thread
started in Codex's `read-only` sandbox and a plan turn using `type: readOnly`,
`networkAccess: false`, plus a strict structured output schema for logical Steps,
dependencies, capability requirements, and acceptance criteria. The path check is lexical
only: absolute, bounded UTF-8, no dot segments or control characters. This policy does not
restrict reads to the Task's Resource roots; the Codex read-only sandbox may read the host
filesystem. It is usable for Task planning only inside a separately enforced, Task-specific
Environment, and those path strings are not proof of Environment identity.

This does **not** start a Task planner. Native MCP/app capabilities may still be present in
the Codex configuration, and are not yet disabled or mediated for planning. No
PlanningCoordinator, trusted producer request context, Task-to-session dispatch, native
turn lifecycle integration, or descendant-writer quiescence exists. The planner must
remain unavailable until the effective native capability set and Environment identity
are qualified. Deferred test cases: assert the exact thread/turn policy and schema payload;
reject relative, control-containing, dot-segment and oversized paths plus malformed or
oversized structured results; prove the planner runs only inside an isolated Environment
and native MCP/app requests cannot cause an external effect; and exercise host/descendant
failure before and after durable session activation. No tests, builds, formatters,
validators, or provider sessions were run, per the deferred verification instruction.

## Latest agent-adapter hardening — OpenCode process environment (2026-10-08)

OpenCode Server launch now clears the daemon's inherited environment and accepts an
explicit native-user allowlist for home/profile, XDG configuration/data/cache/state,
helper `PATH`, OS support, and temporary directories. The generated per-server Basic-auth
credential is added only after environment clearing. `AGENT-FABRIC.md` now makes this
process-environment rule normative, and E03-S01 records inherited daemon variables as a
negative case. This does not wire OpenCode into Task execution or solve the documented
loopback port/server-identity race, process-tree containment, Environment isolation, or
lease fencing. No tests, builds, formatters, validators, or provider operations were run;
source remains unverified under the deferred verification instruction.

## Persistent-folder projection correction (2026-10-08)

The unexposed WorkspaceRoot creation path now emits the canonical Resource
`ProviderIdentity` shape (`provider_instance_id`, opaque `stable_object_id`, and
`identity_confidence`) and uses the canonical `PERSONAL` sensitivity value. SQLite's
commit validation checks the same exact shape. This removes the previously recorded
schema-value mismatch only. It does not prove the caller's identity digest came from a
verified open directory: `AddWorkspaceRoot` still accepts ordinary strings, no stable
platform `FileIdentity` is persisted/revalidated, and there is no safe native chooser or
root lifecycle/API/UI. Keep the root path unexposed until those are implemented. No tests,
builds, formatters, or validators were run; this source remains unverified.

## Unix directory-handle opener foundation (2026-10-08)

Added a daemon-local filesystem primitive that rejects a selected final symlink, resolves
the selection, walks every canonical Unix path component relative to an already-open
directory handle with `O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`, and retains the final open
handle while capturing device/inode identity. Its Debug output and errors do not reveal the
absolute path or raw identity. The private locator is encoded for Runtime-local storage;
the public Resource/Operator layers do not receive it. The source is currently unused and
does not make folder roots available.

This is only the handle-opening substrate. It is not yet keyed into the Runtime identity
key, does not persist `FileIdentityBinding`, does not transfer a native picker selection
through private IPC, does not create/list/revoke WorkspaceRoots, and does not provide
restart revalidation, watchers, indexing, or RAG. Unix device/inode alone is not being
advertised as a durable identity guarantee. Windows fails closed. No tests, builds,
formatters, validators, or OS/provider operations were run; source remains unverified.

## Latest desktop slice — revise a saved Task objective (2026-10-08)

Added the missing authenticated TaskSpec revision path already present in the API contract.
The local Operator exposes Task spec revision history and an owner-authenticated POST that
requires the current Task version and exact parent revision. TaskService and the SQLite
transaction permit only a `READY`, unplanned Task with no live planner; they append an
immutable TaskSpecRevision, advance the Task version/head, write its state snapshot and
`task.spec.revised.v1` event, and persist an idempotency receipt atomically. Omitted fields
inherit the parent revision. The desktop Task detail exposes an objective editor, pins
unchanged retries to the same request key, validates the receipt, and reloads Task/Plan
state. This edit does not start an agent or create a Plan, Step, Attempt, lease, Environment,
or Invocation. F103, API, Task Runtime, Experience, E03-S03 and its implementation backlog
entry record the behavior and later test cases.

This is source-only and remains unverified. No tests, builds, formatters, validators,
migrations, provider processes, or real-user scenarios were run under the current instruction
to defer verification. E03-S03 remains incomplete: trusted planner request context,
native planning dispatch, and full Task execution admission/recovery are still absent.

## Latest review correction — TaskSpec response identity (2026-10-08)

The desktop Task wire model now reads the embedded TaskSpec's `task_id` and `revision`.
Task creation checks the selected Workspace on the parent Task, requires the initial Task
and spec revision to be 1, and checks that the spec belongs to that Task and agrees with
the Task's current-spec pointer before checking the requested objective and ordered
inputs. Task loading applies the same Task identity/current-revision checks. Workspace is
not a field in the canonical TaskSpecRevision contract; its scope follows the parent Task.
This closes a response-integrity gap identified by read-only review without adding a
non-contract field. F100 records the checks and deferred cases. No tests, builds,
formatters, validators, or provider operations were run; source remains unverified.

## Latest desktop source slice — Task plan and Step details (2026-10-08)

Task details now read the current accepted PlanRevision and materialized Steps through
the existing authenticated, Workspace-scoped read routes. Tauri selects the exact Task
plan pointer, checks Task/spec/plan relationships and logical-key-to-Step identity, and
refuses incomplete or foreign rows. The UI presents persisted Step status in plain-language
labels and marks a plan stale when it pins an older TaskSpec revision. No planning provider
was started, and no Attempt, Evidence, or verification state is inferred. F102, EXPERIENCE,
and E03 record this increment and deferred cases. No tests, build, formatter, validator, or
provider operation was run; this source remains unverified.

## Latest source slice — Task input pinning and READY-state labeling (2026-10-08)

The desktop Library now lets the owner explicitly select an existing Resource revision
and carry it to the Home composer as a Task input. Selections are Workspace-specific;
choosing a newer revision for an already selected Resource replaces the pin explicitly.
The composer shows selected names and supports removal/clear. Tauri passes exact
`PinnedResourceRef`s in the existing `POST /v1/tasks` body, checks reference shape and
Workspace scope, then verifies the returned TaskSpec input list. The Operator/SQLite
transaction already checks that each Resource revision exists in the same Workspace.
Task detail reads those persisted refs and shows the exact revision, resolving a display
name from the local presentation cache when available and falling back to IDs otherwise.

These are input references only: pinning does not fetch/decrypt Resource content for an
agent, create a grant, or start planning/execution. A future ContextPlanner must still
resolve availability, policy, sensitivity and authority before content reaches an agent.
The Task remains save-only (`READY` with no Plan). The general Task list/filter says
“Ready”; Task detail uses “Ready” and “No accepted plan is currently saved for this Task”
when `current_plan_revision` is null. This does not infer that the Task has never run.

F100 and new F101, `docs/EXPERIENCE.md`, E02/E03 epic notes, and the backlog record this
flow and its deferred cases. No tests, build, format, validators, or provider operation
were run in this source slice, per owner direction. Source is unverified; exact resource
name recovery after restart is limited by the existing metadata API, and falls back to
the stable IDs/revision unless that revision's name is already in the desktop cache.

## Latest source slice — Desktop standalone Task save (2026-10-08)

The Tauri native layer now exposes `create_task`, which sends the selected Workspace,
exact default lead binding, objective and RequestId through authenticated local Operator
IPC to `POST /v1/tasks`. The Home composer enables a **Save Task** action, preserves the
draft on failure, retains the idempotency tuple for same-input retries in the current
window, and opens the committed Task detail after success. Workspace switching and
navigation are disabled while that request is in flight. A `READY` Task with no accepted
Plan is rendered as **Ready** with **No accepted plan is currently saved for this Task**;
the UI does not imply that the Task has never run.

This is durable Task creation only. It starts no agent, planning session, Plan, Step,
Attempt, lease, Environment, Invocation, Effect or Evidence. It does not complete E03-S03
and does not make the composer an execution surface. The Tauri command validates input,
checks the returned Workspace/objective scope, and uses the existing idempotent Operator
route. The same RequestId is not persisted across application restarts. No tests, build,
format, validator, or live-provider operation was run, per the owner's deferred
verification direction; this source and UI remain unverified.

Next execution work remains PlanningCoordinator/AgentSessionSupervisor and a qualified
native planning path, followed by Attempt/Environment/lease admission and Trust/Effect
execution. Do not launch a native agent against user files until writer isolation,
process identity, and recovery/fencing are established.

## Latest source slice — WorkspaceRoot atomic persistence foundation (2026-10-08)

Added a domain `WorkspaceRootService`, storage-core `WorkspaceRootStore` contract and
SQLite immediate transaction for initial folder Resource + local ResourceLocation +
incarnation-scoped private locator + WorkspaceRoot creation. The transaction checks
Workspace ownership/status, current Runtime incarnation through the existing binding
guard, event/record identity, idempotency replay, and path-free durable projections/events.
The private locator type cannot be serialized or Debug-formatted. The deferred
`implementation/licensingandpkacgingresearch.md` note also exists and remains linked from
the implementation README; it records licensing/packaging options for review only after
V1 testing and beta readiness.

This is **not a usable or production-safe persistent folder flow**. It has no native
folder chooser, handle-relative/no-follow directory opener, keyed OS file-identity probe,
root Operator/API route, root list/lifecycle operations, Tauri UI, watcher/indexer, or
restart revalidation. Nothing currently calls the service. Do not report this as
WorkspaceRoot support, filesystem authorization, indexing, or RAG. The domain/storage
source was not built, tested, formatted, or validated, at the owner's request to defer
verification. Next: qualify a cross-platform local-directory identity/opening adapter
and native selection handoff, then wire root create/list/revoke and its UI without exposing
absolute paths to the WebView or public Operator contracts. Keep test/system/user
acceptance open in E02-S03.

Read-only source review also found unresolved correctness/security gaps in this storage
foundation: the domain accepts caller-constructible path/digest strings rather than a
verified opened-directory selection; no platform FileIdentity is persisted for restart
revalidation; and the location event cannot reconstruct the committed location while
sharing a Resource revision. Do not expose or wire this service until these contracts are
corrected and a platform provider proves directory identity against a live handle. Root
create-only storage also has no list/pause/resume/revoke lifecycle yet.

## Current desktop/local progress — 2026-10-08

### Daemon integration of expired Attempt/lease recovery (source only)

The daemon now runs a bounded startup sweep after its current Runtime incarnation is
durably marked `DEGRADED` and before the Operator listener starts. It lists only expired
ExecutionLeases in ACTIVE Workspaces owned by the local principal where this exact
current Runtime incarnation has an active EXECUTOR Workspace binding. SQLite uses its
own clock and revalidates the active Workspace owner, nonrevoked Runtime Workspace
binding, Runtime trust zone/role, and exact current incarnation while listing and again
in the expiry transaction. Each candidate is expired through `StepAttemptCoordinator`; the transaction
abandons the old Attempt, blocks the Step and Task for Effect reconciliation, writes
verified aggregate-state blobs and durable events, and stores an idempotency receipt.
`docs/STATE-MACHINES.md` explicitly permits an overdue `RELEASING -> EXPIRED`
transition and states that lease expiry does not prove worker-process termination.
`docs/TASK-RUNTIME.md` now orders the partial recovery as lease expiration/Attempt
abandonment and Task blocking first, followed by required process/Effect reconciliation
before any new Attempt; the current daemon does not implement those later gates.
The old Runtime process and credential are not required. The pass is capped at 128
transitions per startup to bound startup delay.

This does **not** resume Tasks, reconcile Effects, admit a new Attempt, start a coding
agent, prove an old child process is dead, or qualify process containment/Environment/Trust.
It fences the persisted lease record only; it does not fence direct filesystem writes by
an escaped worker process. The daemon remains `DEGRADED`
with `TASK_ATTEMPT_ADMISSION_NOT_INTEGRATED` and
`TRUST_EFFECT_RECONCILIATION_NOT_INTEGRATED`; failures add
`EXPIRED_EXECUTION_LEASE_RECOVERY_FAILED`. Workspaces without an active local EXECUTOR
binding are skipped. Leases that expire after this startup pass are not swept until a
subsequent daemon startup. Explicit early Runtime-revocation fencing and periodic lease
recovery remain unimplemented. No tests, build, formatter, validator, or provider process
was run; source remains unverified.

### Plan acceptance storage slice (source only)

`storage-core` now defines immutable PlanRevision/Step records and a PlanAcceptance port.
`domain-task::TaskService::submit_initial_plan` validates a bounded initial DAG, assigns
Step IDs, resolves logical dependencies, and builds the Plan/Step event drafts. SQLite
commits the first PlanRevision, materialized Steps, Task current-plan pointer/version,
complete Task/Step aggregate-state blobs, events, and an idempotency receipt in one
immediate transaction. PlanRevision history and Step reads are available through the
TaskStore port and authenticated Operator GET routes. SQLite v5 adds immutable
PlanRevision guards, Step identity/deletion protection, and unique logical keys per plan.
The SQLite acceptance transaction also requires the producer session to remain attached to
a host on the Runtime's current `ONLINE`/`READY` incarnation, validates the endpoint and
active Workspace EXECUTOR binding, and rechecks enabled lead eligibility. Plan and Step
events must have exact payload key sets and unique event IDs. The idempotency digest now
includes the normalized typed plan and producer AgentSession, not only caller JSON. The
Runtime-to-Workspace check uses `runtime_workspace_bindings` because v4 removed
`runtimes.workspace_id`; endpoint expiry is checked against the daemon's UTC admission
clock instead of the proposal timestamp. The implementation coverage and machine inventory
have not been regenerated for this slice; reconcile them in the later verification pass.
Two read-only storage-seam reviews found and rechecked the v4 column mismatch, producer
identity omission in the dedup digest, and stale event-time expiry comparison; source edits
close those findings. Clock-skew qualification remains open.

This is only an internal durable seam. It accepts only a first plan from an active
`TASK_PLANNING` session; execution-produced replans still require Attempt/lease authority.
The machine API contract defines POST plan submission, but its Operator route is not
connected: current owner authentication does not carry a trusted AgentSession producer
assertion, and body-supplied session IDs must not grant that authority. No native agent
currently invokes the service. The composer remains disabled and this is not integrated
agent planning. No code tests, build, validators, Provider/system tests, or OS
qualification were run; all new source remains unverified as requested.

The local daemon now handles SIGINT/SIGTERM/SIGHUP through a coalesced graceful-shutdown
path, persists startup blockers before serving, and keeps the Runtime in DRAINING while
the authenticated Operator listener stops admission and its thread joins. CLI help is
explicit that this build still does not accept executable work. This is lifecycle progress,
not Task recovery/execution or cross-platform OS qualification.

The existing resumable Resource upload endpoint now creates sessions through
`ResourceUploadService`, which rechecks Workspace ownership/state and validates upload,
folder-provenance, and Context Document metadata before storage. The Workspace Library
surface now supports file drag/drop, folder selection as one-time file uploads, and ZIP
upload. Upload remains resumable and digest checked. ZIPs are stored intact; folder paths
are provenance only. Metadata search remains deterministic; there is no durable extracted
text index or semantic RAG. The Library now has an explicit `ON_DEMAND_CONTENT` mode for
current managed Resources: it scans at most 20 candidates, 1 MiB per Resource and 8 MiB
per request in memory, returns bounded snippets pinned to the rechecked Resource revision,
and persists no extracted content or search terms. ZIPs, rich documents and WorkspaceRoots
remain unsearched. The UI labels this as an on-demand scan; results are not attached to
Tasks implicitly.

Context7 and the current official OpenCode Server docs were used for a bounded native
transport in `apps/litecoworkd/src/agents/opencode_server.rs`. It launches the native
`opencode serve` harness on IPv4 loopback with generated Basic Auth and uses the stable
`/global/health`, `/session`, `/session/{id}/message`, `/session/{id}/prompt_async`,
`/session/{id}/abort`, and `/event` surface. The transport is still not integrated with
Task/Attempt admission, Environment,
Trust, leases, Effects, or event normalization. The reserved-port release before spawn
leaves a local impersonation race; process identity must be qualified before this can
carry prompts or credentials. Stopping the direct process is not proof that tool
descendants stopped writing. No model inference was invoked. Retain [R-OPENCODE](SOURCES.md)
and the [OpenCode Server docs](https://dev.opencode.ai/docs/server/) for implementation.

No build, test, architecture validator, implementation-plan validator, provider/system
test, OS qualification, commit, or push was run in this progress slice, per the owner's
deferred verification direction. Source-level test cases were added for the Resource scan
guards but have not been executed. All modified source remains unverified.

Remaining desktop/local critical path: wire Task creation to PlanningCoordinator and a
qualified native adapter; persist plan proposals/acceptance, Steps, Attempts, Environment,
ExecutionLease, and capability/effect/evidence/verification transitions; then connect
Task steering and progress to Tauri. The on-demand Resource scan is not an index: durable
protected/rebuildable indexing, ZIP extraction, roots, local semantic retrieval, and
local-model integration remain open. Heterogeneous delegation and safe switching still
require provider adapters, effect/lease reconciliation, server-process identity, and OS
process-tree containment.

## Active slice — Codex typed session transport (2026-10-08)

Two read-only subagent audits confirmed that Rust has no complete `TASK_PLANNING`
AgentSession coordinator/supervisor. `POST /v1/tasks` persists only a `READY` Task and its
initial TaskSpecRevision; the existing SQLite schema already has planner uniqueness and
authority constraints. The next product boundary is durable planner admission, not
connecting Codex RPCs directly to Task creation.

A coding subagent added bounded typed Codex App Server calls in
`apps/litecoworkd/src/agents/codex_app_server.rs` for thread start/resume, text turn start,
and turn interrupt. Current official protocol references were checked through Context7:
[Codex App Server](https://developers.openai.com/codex/app-server) and the
[Codex protocol source](https://github.com/openai/codex/blob/main/codex-rs/app-server-protocol/src/protocol/v2/thread.rs).
The transport still does not validate that a native thread belongs to the admitted Task,
does not create or activate durable AgentSessions, and cannot prove descendant-process
writer quiescence. It is not integrated into Task admission or safe switching.

This slice adds `TaskService::reserve_planning_session`, an `AgentSessionStore` port, and
the SQLite `TASK_PLANNING` reservation path. It validates the expected Task
version/spec/lead, owner, enabled lead binding, endpoint profile and current
Runtime/incarnation readiness, then atomically writes a version-1
`STARTING` row, aggregate-state blob reference, `agent.session.starting.v1`, and
RequestId receipt. A bounded query exposes stranded STARTING planner sessions across
incarnations for recovery, and a version-checked transition settles a stranded planner as
`LOST` without changing Task status. `STARTING` is not readiness and grants no Task
authority. `TaskService::activate_planning_session` now builds the `ACTIVE` event(s), and
SQLite commits session activation, a Runtime-local host binding, and first Task
`READY -> RUNNING` in one transaction. Activation rechecks that the binding remains
enabled/lead-eligible, the endpoint binding is unexpired, and the host and Runtime
incarnation are still ready/current; it also validates event schema versions. The storage
actor dispatches this transaction, and F98 now describes the activation boundary.
F98, `TASK-RUNTIME.md`, `STORAGE.md`, `SERVICES.md`, `STATE-MACHINES.md`, `EVENTS.md`,
and the machine event schema now describe this boundary.

Still missing: PlanningCoordinator/AgentSessionSupervisor wiring, adapter start using this
reservation and activation after observed readiness, general close transitions,
authenticated Task submit from Tauri, Task/Attempt lease execution,
and descendant writer fencing. Read-only subagent review found activation had to recheck
Runtime/host/binding/endpoint readiness after native startup; those checks and event
schema-version checks are now included in the activation transaction. A Runtime-local
`AgentHostStore` port and SQLite create/read/list/CAS transition operations now persist
host readiness without emitting replicated domain Events. `SERVICES.md` and F98 now
reflect the available storage operations and remaining supervisor/coordinator work. No
tests, build, validators, provider inference, OS
qualification, commit, or push were run in this slice, per the owner's deferred
verification direction.

## Active slice — local agent setup and Workspace lead selection (2026-10-08)

Source now includes an explicit selected-Workspace enrollment screen and Operator path,
then an owner-triggered bounded Codex App Server profile probe, sanitized Runtime offer,
and separate disabled-binding/create and enable actions. Workspace ownership and exact
current local Runtime/incarnation checks gate enrollment and probing; current
`ONLINE/READY` and serving `DEGRADED/DEGRADED` Runtime states are accepted for this
control-plane operation only. AgentBinding configuration is currently restricted to an
empty object until an adapter-specific non-secret allowlist exists. Enrollment replay no
longer hashes the server-generated binding ID, and stable profile identity preserves the
first durable discovery timestamp across concurrent initial probes.

Settings now also selects or clears a Workspace default lead through the versioned,
idempotent Workspace update path and the existing `workspace.default_agent_binding.changed.v1`
event. The service/storage path checks active Workspace state and binding eligibility;
the desktop selector lists only enabled, lead-eligible bindings and updates from the
committed Workspace response. This changes future admission defaults only.

This still does **not** implement Task planning/admission, AgentSession start/resume,
Task/Attempt lease integration, safe process-tree containment, Effect reconciliation, or
production-safe switching. A successful model catalog probe is not model entitlement or
inference. UI and backend source remain unbuilt, untested, and unqualified; validators,
OS qualification, provider execution beyond the explicitly requested probe flow, commit,
and push have not been performed. Setup is documented in F95; Workspace default selection
is documented in F96. The desktop uses the exact-current-local enrollment endpoint, not
the full Workspace Runtime binding inventory.

Next: implement planning-session persistence/readiness and the Task coordinator around
the durable creation path; do not expose a composer submit action until the session is
durably admitted and actually starts. Testing/system/user acceptance remains a later
explicit verification pass as requested.

### Work added after that handoff

Added `crates/domain-task` with standalone Task aggregate construction and an authenticated
`POST /v1/tasks` Operator route. The route checks owner, active Workspace, explicit/default
lead binding, and delegates atomic Task + TaskSpecRevision + event + request receipt to
`TaskStore`. The persisted Task is `READY` with no Plan, Step, Attempt, lease, Environment,
or AgentSession. Conversation-origin and Coworker-origin creation are rejected because
their atomic message/Coworker admission services are not present. Tauri has no create
command and the Home composer remains disabled, so the UI does not present this as work
being handled. This is durable storage/admission groundwork, not Task execution.

The Workspace default-lead selector is now implemented as F96. None of this source has
been compiled, tested, validated, OS-qualified, or provider-qualified; the owner explicitly
deferred those checks. The current source may contain integration errors until the later
verification pass.

## Current handoff — authenticated local IPC integration (2026-10-08)

The daemon and native Tauri client now use the shared framed Unix IPC path on Linux/macOS.
Daemon startup validates an installation-scoped OS-principal binding before publishing the
endpoint; Tauri authenticates the peer UID before sending a request; the daemon dispatches
through the existing Axum routes using a private process-local authentication marker. The
old loopback HTTP/bearer descriptor path is removed. Windows fails closed pending named
pipe/logon-SID support. This source is not built, tested, or OS-qualified.

This slice also adds pre-reserved bounded response memory, a 10 MiB current Operator
response limit, a 60-second handler deadline, stop-admission acknowledgment before Runtime
drain state transitions, and structured API error code/retryability/correlation reference
in Tauri error strings. Runtime first-install binding now persists the generated Runtime ID
before keyring creation and records whether the binding was established, so a crash during
bootstrap can retry against the same ID without reopening an established binding.

Read-only review identified and this slice addressed response-memory accounting,
shutdown-admission ordering, handler deadlines, and loss of actionable API error metadata.
A scoped re-review found no remaining Critical/High issue in those IPC areas. A separate
Codex transport review identified terminal-event draining, malformed JSON-RPC error
validation, and retained-event count bounds; source corrections are now present. These
changes remain unbuilt and untested. No validators, OS qualification, commit, or push have
been run. Do not claim safe agent switching or production-ready IPC: product
Task/Attempt/lease/Effect reconciliation and OS qualification remain unimplemented.

Historical entries below record the state as it was at their timestamp; this handoff block
is the current source-state summary.

## Historical implementation slice — Codex App Server transport (2026-10-07)

E03-S01 is the next product implementation seam after local agent inventory: current
Codex/OpenCode rows only report executable versions and do not start sessions. A focused
read-only audit selected Codex App Server first because the user already chose Codex with
GPT-6-Luna/medium for the handover work. [Official OpenAI App Server documentation](https://learn.chatgpt.com/docs/app-server)
confirms the native JSONL lifecycle (`initialize`/`initialized`, thread start/resume,
turn start/steer/interrupt, and streamed events). A coding subagent added a bounded
stdio process/RPC layer in `apps/litecoworkd/src/agents/codex_app_server.rs`; it is not
being called against a model and does not alter native config. This layer will not create
durable Task/Attempt records, grant Effects, or prove child-process quiescence. Those remain
required integration work before production-safe switching. No build or test is being run
in this slice, per the owner's deferred verification direction.

### Source work completed in this slice

Added a bounded stdio JSONL transport implementation at
`apps/litecoworkd/src/agents/codex_app_server.rs`. It exposes explicit spawn,
initialize, thread start/resume, turn start/steer/interrupt, event receive, and explicit
server-request reply/rejection operations. It keeps native payloads private, bounds lines
and queues, drains stderr without retaining it, and always reports writer quiescence as
unproven. It remains unintegrated with Task/Attempt, AgentSession storage, TrustService,
Environment containment, or the daemon Operator. A source review found that the initial
synchronous RPC API cannot service a server request that blocks the same RPC response,
that the daemon environment was inherited, host timeout/cleanup and aggregate byte bounds
needed strengthening, and malformed request handling needed to fail closed. The author is
reworking the transport/event dispatch boundary. Do not treat the module as usable or
production-safe until that work and later qualification pass.

The shared `operator-ipc` crate now also has asynchronous request/response framing built
on the existing validators. Frames retain body-budget reservations, constructed frames
require the shared budget, response IDs are checked before body allocation, and failed or
cancelled I/O requires discarding the one-exchange stream. This remains a framing library;
there is still no authenticated IPC listener/client or daemon/UI integration. A review
found and corrected the earlier unbudgeted-frame ownership gap. No compilation, tests, or
validators have been run; both source slices remain unverified.

### Durable Task storage foundation

Added `TaskRecord`, `TaskSpecRevisionRecord`, aggregate snapshot/view types, the
`TaskStore` port, and SQLite create/get/list operations using the existing v1 tables.
Creation persists Task + TaskSpecRevision(1) + `task.created.v1` + aggregate-state blob
reference + workspace/origin sequence + idempotency receipt in one SQLite transaction.
The storage boundary validates Workspace/Coworker/source Resource and message scope,
lead-binding precedence, static `enabled && lead_eligible`, and the initial aggregate shape.
It creates no Plan, Step, Attempt, lease, Environment, or AgentSession. No migration was
needed. This is only persistence: there is no TaskService, Operator route, binding/auth/
Runtime readiness admission, atomic ConversationMessage-to-Task command, plan coordinator,
or recovery path yet. The current composer therefore remains correctly unavailable for
durable execution. This slice is unbuilt and untested.

### Codex transport source review

A second read-only review found the initial transport blockers resolved in the revised
source: RPC/event dispatch is split, child environment inheritance is cleared, direct-child
reaping is owned, transport faults request host termination, wire-byte and queue-count
bounds are enforced, request replies are type/method constrained, malformed IDs poison the
transport, and initialize refreshes its watchdog. Remaining qualification blockers are
OS process-tree containment/quiescence, typed outbound request constructors, approval
request expiry/user-wait lifecycle, and worker cleanup when descendants inherit pipes.
The adapter stays unintegrated; direct-child exit is not safe-switch evidence. No native
agent was launched and no model was called.

## Latest desktop transport slice — shared frame protocol (2026-10-07)

The active desktop/local slice is still the authenticated loopback bearer bootstrap; it
does not authenticate the OS peer. A focused subagent audited all Tauri call sites and the
Windows transport surface. Context7 was used for current `interprocess` and Tokio IPC
documentation. The result is recorded normatively in `docs/LOCAL-OPERATOR-IPC.md` and
cross-linked from Network Security, API, Runtime Lifecycle, Coverage, F94, and E01-S04.
Added a shared `operator-ipc` crate with versioned request/response headers, a 4-byte
length-prefixed JSON header plus raw body, allowlisted methods/headers, relative-path
validation, and bounded allocations. It is only a framing primitive: it does not create
an endpoint, authenticate peers, adapt Axum, or replace the Tauri HTTP client.

The audit found that safe cross-platform IPC requires more than replacing TCP with a Unix
socket: Linux/macOS must check the accepted peer UID; Windows must reject remote named-pipe
clients and restrict access to the current logon SID plus SYSTEM. The current Runtime's
random `local_principal_id` is not an OS identity, and the workspace-wide
`unsafe_code = "forbid"` blocks a narrowly isolated Win32 token adapter. The selected
implementation sequence is: complete/integrate the shared framed transport; add a persistent
Runtime OS-principal binding; add Unix peer checks and an isolated reviewed Windows token/
DACL adapter; route requests through the existing Operator handlers; migrate Tauri to IPC;
remove desktop bearer/TCP fallback; then qualify Linux, macOS, and Windows. Do not claim
production-safe switching or persistent folder grants before these gates pass.

No IPC listener/client integration, build, tests, validators, OS qualification, commit, or
push has been performed for this slice. The preexisting loopback bootstrap remains in
place. The framing code is unverified and is not a secure transport or production-ready
feature.

## Local IPC integration history (2026-10-08)

The local IPC design was reviewed with focused subagents and Context7 documentation for
Axum 0.8 `Router::oneshot`; integration kept the existing Operator handlers as the one
authorization/application path. The sections above capture the resulting source changes
and remaining qualification gates. Earlier in this log, the 2026-10-07 and initial
2026-10-08 snapshots correctly describe the then-active bearer bootstrap; they are historical
and superseded by the current handoff above.

## Current cross-cutting finding — Runtime/Workspace binding (2026-10-07)

The owner asked why parallel agents and MCP sources were not being used consistently. The
repository says to avoid parallel agents unless the owner assigns them; the owner's earlier
explicit instruction to use subagents is authorization. Two focused read-only audits were
therefore run for Runtime/Workspace contract consistency and SQLite migration impact. Both
confirmed that the current prose treats Runtime as installation-scoped, while `Runtime`
and four composite SQLite foreign keys still bind it to a single Workspace. Context7 was
used for local IPC/library qualification in this workstream; available MCP sources also
include current vendor/library docs, repository research, and browser inspection where
those apply.

ADR-0020 and coordinated RuntimeWorkspaceBinding updates now span Architecture, Data Model,
shared schemas, lifecycle, Runtime Mesh, security, API/OpenAPI, service ownership, state
machine, flow, storage, coverage, and SP03. SQLite v3 remains the immutable additive
backfill. A v4 migration now removes `runtimes.workspace_id`, rebuilds Environment,
ChannelHostAssignment, AutomationOccurrence, and AutomationCursor, adds role-scoped
RuntimeWorkspaceBinding guards, validates trigger-enforced row scope and foreign keys
inside the migration transaction, and restores FK enforcement on success or rollback. Mesh
hub assignment requires an active MESH_PAIRING binding; revocation requires clearing that
pointer and draining channel/trigger hosts. The migrator now carries fresh/v1/v2/v3
databases to v4 without changing v1-v3 sources/checksums. This source has not been built or
tested yet. The daemon still does not register Runtime/incarnation rows or enroll/query
bindings; endpoint offers, AgentBindings, task execution admission, and qualified OS-peer
identity remain unimplemented. Persistent Workspace roots therefore remain blocked. No
tests, builds, validators, or OS qualification were run.

## Latest desktop UX correction — one-time folder import (2026-10-07)

The Library now labels folder selection as “Import folder files” and explains that it
copies the selected files into encrypted Workspace storage without continuing to watch
the source folder. Search copy now says it does not cover continuously watched folders.
This avoids presenting one-time `webkitdirectory` uploads as persistent `WorkspaceRoot`
grants. Persistent roots remain unimplemented: a delegated contract review confirmed that
they require authenticated OS-peer IPC and a defined Runtime-to-Workspace association
before the daemon can safely retain local filesystem bindings. Context7 was used to check
Tokio's Unix peer-credential and named-pipe security surfaces. No build, tests, validators,
commit, or push were run, per owner direction.

## Latest review corrections — upload recovery and Operator errors (2026-10-07)

A read-only delegated review found three gaps. Committed uploads are now recovered against
their durable commit receipt without applying the 24-hour expiry used for unfinished
sessions; the selected Workspace, display name, folder path, media type, size, and digest
must still match, and a missing committed Resource mapping now stops for review instead of
starting another upload. Upload session reuse now includes media type
in both local resume metadata and server-session identity. The local Operator now serializes
the documented top-level `ApiError` fields and repeats its correlation ID in
`X-Correlation-ID`; the Tauri bridge reads that shape. E02-S03's later verification cases
now cover post-expiry commit recovery and media-type changes. These remain source-only,
unbuilt and untested, per owner direction. No commit or push was made.

A separate read-only WorkspaceRoot seam review found that the specified root CRUD API has
no daemon route or native picker flow. Existing folder selection is only a one-time upload;
it creates encrypted-blob Resources, not a FOLDER Resource or filesystem location. Durable
private location/file-identity bindings require a registered Workspace Runtime incarnation,
but startup currently keeps only bootstrap IDs outside the database; the current local
Operator also lacks qualified authenticated OS-peer identity. The next persistent-root
slice must first resolve these identity and native-IPC boundaries, then atomically create
the FOLDER Resource/location/bindings/root and stop observation before successful revocation.
This is a contract review finding, not an implemented root feature. Official Tauri docs
were checked through Context7 for [native directory selection](https://v2.tauri.app/plugin/dialog/)
and [dynamically granted filesystem scopes](https://v2.tauri.app/plugin/file-system/); a
picker alone does not implement Runtime-owned durable root authority.

## Active implementation slice — structured folder-import provenance (2026-10-07)

E02-S03 now pins typed `folder_import.relative_path` metadata in one-time folder uploads.
The daemon validates canonical relative segments and requires the path to equal the current
compatibility `display_name`; it never resolves the path or treats it as a persistent
WorkspaceRoot grant. Tauri sends the metadata, validates the returned session, and the UI
includes it in upload resume identity. SQLite persists it in `resource_upload_sessions`
and session snapshots; commit copies only the pinned session value to Resource provenance.
The idempotency payload includes the path only when present, preserving the prior digest for
non-folder requests. Folder-derived Resources use `resource.created.v2`, preserving the
closed `resource.created.v1` contract. SQLite v1 is immutable; additive v2 includes upload
lifecycle/progress and folder-provenance fields, chunk-request/blob-GC recovery tables, and
guards. It recovers committed Resource IDs from status events where possible. Historical
digest-less sessions remain readable but cannot be resumed or committed; old folder paths
are never inferred from display names. Updated OpenAPI, event schema, storage migration,
Data Model, API, Events, World Resources, F85, E02-S03 and coverage map. Still deferred:
persistent native WorkspaceRoot grant, ZIP inspection/extraction, content extraction and
protected indexing/RAG. No tests, build, validators, commit, or push were run by owner
direction; this source slice remains unverified and not production-qualified.

## Latest implementation slice — authenticated local Operator readiness (2026-10-07)

Added `GET /v1/operator/readiness` to the local authenticated Operator. It reports
`operator_state=SERVING`, the bootstrap Runtime and local-incarnation IDs, and API contract
version 1; the IDs are returned only to the authenticated same-installation client and
remain non-Mesh/non-replicated. Tauri now validates the private descriptor's owner-only
file mode and shape, performs the bounded bearer-authenticated handshake, and requires
exact identity equality before issuing Workspace/resource API calls. The fixed-loopback
clients bypass environment proxies so the bearer cannot be forwarded, and the readiness
JSON uses the OpenAPI snake_case field names. Runtime status now
reports `processRunning` and `operatorReady` separately, and start waits for both rather
than treating the process lock as API readiness. The UI distinguishes a process that is
starting from a Runtime whose Operator is serving while Task execution remains blocked.
OpenAPI, API, Runtime Lifecycle, Network Security, F91, E01, and this handoff record were
updated. This is an unbuilt, untested source slice; it does not qualify OS peer
authentication, implement durable Mesh registration, or make local Runtime execution
ready. Safe F42 stop preview/drain remains blocked on Task/Effect/lease and dependency
inventory; no empty list will be returned for unknown dependencies. No tests, build,
validators, commit, or push were run.

## Latest implementation slice — scoped Task reads (2026-10-07)

Added authenticated `GET /v1/tasks`, `GET /v1/tasks/{task_id}`, and
`GET /v1/conversations/{conversation_id}/tasks` to the local Operator.
The list route scopes by the selected Workspace and owner, validates status and optional
Conversation filters, uses a filter-bound versioned cursor with descending
`(created_at, task_id)` keyset pagination, defaults to 50 items, and caps pages at 200.
The detail route returns the Task plus its current immutable TaskSpecRevision and uses
the same not-found response for absent or foreign Task IDs. SQLite calls run on Tokio's
blocking pool, and archived Workspaces remain readable. The daemon manifest now directly
declares its existing `storage-core` API use and enables the `time` parsing feature used
to validate cursors. Conversation-scoped Task pages share the same bounded pagination
implementation and bind cursors to the Conversation filter. OpenAPI now restricts global
Task status filters to the exact states accepted by the daemon.

These are read-only Task views. Task creation is still unavailable: the daemon has no
persisted AgentProfile/Endpoint/Binding admission service or protocol/auth readiness
check, and it still has no Planner, Plan/Step/Attempt creation, ExecutionLease,
Environment, AgentSession, Task recovery, or Effect reconciliation integration. The
Codex App Server transport remains an unintegrated source module; version discovery,
initialize, account/model catalog, or process launch alone must not be represented as
inference/model entitlement or safe switching. No build, tests, validators, provider
launch, OS qualification, real-user acceptance, commit, or push has been performed, per
the owner's deferred verification direction. This slice remains unverified.

## Latest adapter slice — bounded Codex discovery RPCs (2026-10-08)

The Codex App Server transport now has explicit `account_read()` and `model_list()`
constructors. Both send an empty params object, and `begin()` rejects caller-supplied
parameters for these methods. This keeps the discovery path from enabling token refresh,
pagination, or hidden-model options. The account response may contain identifiers or an
email and must remain transient/private; model-list output is an option catalog only and
does not prove model entitlement. Current protocol details were checked against the
[OpenAI App Server guidance](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server)
and upstream [account protocol](https://github.com/openai/codex/blob/main/codex-rs/app-server-protocol/src/protocol/v2/account.rs)
and [model protocol](https://github.com/openai/codex/blob/main/codex-rs/app-server-protocol/src/protocol/v2/model.rs).

These are transport methods only: no Codex process, native credentials, or inference was
accessed. There is still no sanitized account projection, AgentProfile/RuntimeOffer
persistence, AgentBinding API/service, auth/Runtime admission, AgentSession, Task
planning/execution, or safe-switch integration. No build, tests, validators, or provider
qualification ran; source remains unverified.

The transport now launches the explicitly documented
`codex app-server --listen stdio://` mode and exposes a typed initialize constructor that
requires stable LiteCowork `clientInfo` fields rather than accepting arbitrary initialize
JSON. Official OpenAI documentation also warns that `model/list` may return a bundled or
cached catalog, not proof of account entitlement; the transport preserves that distinction.
This remains source-only and unqualified.

## Latest desktop slice — read-only Work and recent Tasks (2026-10-08)

The Tauri shell now calls the authenticated Task list/detail APIs through the existing
Operator connection and selected Workspace header. Home shows up to five recently
persisted Tasks. Work supports exact-status filtering, 50-row cursor paging, manual
refresh, and a read-only Task detail with the current spec/plan revision information.
Workspace/filter request generations prevent late responses from replacing the current
view. Loading, empty, request-error, and offline/stale states are represented; no Task
creation, execution, or control actions were added. The bridge bounds response bodies and
checks that detail identity matches the selected Workspace and Task.

This is a UI consumer for storage-backed Task reads only. There is no supported path to
create a Task yet, so the screens are empty until a Task is created through a future
admitted TaskService. The slice was source-reviewed but not built or tested; no validators,
provider calls, real-user acceptance, commit, or push were performed.

## Latest implementation slice — whole-file identity for resumable intake (2026-10-07)

Closed a resumable-upload identity gap found during the E02-S03 contract review. The
desktop computes a SHA-256 digest for the selected file before opening an upload session;
the authenticated Operator requires and pins it; session reads and the creation event
carry it; SQLite requires it; commit verifies the reconstructed bytes against it. Resume
now also requires the reselected file digest to match, so same-name/size/mtime files cannot
continue one another's unfinished upload. This is source-level only: no build, tests,
validators, or end-to-end run were performed. The 100 MiB file limit bounds the current
whole-file WebCrypto buffer, but peak memory and responsiveness remain unmeasured. Add
coverage for digest-required creation, restart persistence, replaced-file resume, mixed
chunks, and integrity failure before release. See E02-S03, `docs/API.md`, `docs/EVENTS.md`,
and the OpenAPI/SQLite contracts. No commit or push was made.

## Latest implementation slice — truthful local agent installation inventory (2026-10-07)

Added an authenticated, runtime-local `GET /v1/agent-installations` projection for Codex
and OpenCode. Discovery invokes only each known executable's `--version`, waits up to two
seconds before terminating an overdue direct child, limits captured output to 257 bytes,
validates a three-component numeric version with an optional bounded suffix, and discards
stderr/arbitrary stdout. A five-second cache coalesces concurrent refreshes. The response
distinguishes missing/installed/version
unavailable while deliberately reporting authentication `UNKNOWN` and session readiness
`NOT_PROBED`; it does not create an AgentProfile/Binding or launch a coding session. The
desktop Settings page displays this inventory and its protocol candidate. Contracts were
updated in AGENT-FABRIC, API, OpenAPI, SCHEMAS, and E03. Native protocol handshake,
credential/provider readiness, session support, Task execution and safe switching remain
unimplemented. This is an unbuilt, untested source slice; tests and OS/provider
qualification remain deferred. No tests, build, validator, commit, or push was run.

## Latest source correction — resumable chunk replay ordering (2026-10-07)

Moved durable chunk-request receipt lookup ahead of the new-reservation requirement in the
SQLite finalization transaction. This lets an exact retry return the current session after
the original request consumed its reservation, including when the final chunk already
transitioned the upload to CONTENT_RECEIVED. Reservation admission now recognizes an exact
prior receipt or an identical accepted chunk before requiring an OPEN session, while new
chunks still require the OPEN state, fixed chunk size, exact expected size, unexpired
session, GC-fence clearance, and a live reservation. Added the required-size guard before
replay matching. Exact upload/chunk geometry is now validated before reservation/blob
write, while receipt replay can still proceed against a terminal accepted session. Added
Exact accepted-chunk replay is resolved before the progress-version CAS for same- and
different-RequestId retries. If a concurrent duplicate created its own reservation, that
reservation is removed atomically with the replay receipt, preventing a referenced digest
from leaving a permanently stranded reservation. Added later verification cases for both
concurrent retry forms, final-chunk replay after CONTENT_RECEIVED/COMMITTED, wrong-sized
replay, and malformed ranges leaving no reservation or blob.

This is an unverified source correction. No tests, build, validator, or OS qualification was
run. The path still needs the later transaction/concurrency/restart verification pass.

## Latest implementation update — encrypted orphan upload-chunk cleanup (2026-10-07)

Added a BlobStore exact-object removal contract and a FileBlobStore implementation that
rejects symlink/non-regular targets and syncs the parent directory after deletion. SQLite
records a durable per-chunk reservation before the encrypted blob write; receipt commit
consumes it atomically. The bounded collector selects expired reservations only when no
accepted chunk references the Workspace/digest and no live reservation exists, records a
GC fence, removes the object, then clears operational rows. Failed deletion leaves the
fence for retry. `litecoworkd` invokes the sweep after upload expiry, with a 100-object
bound per 30-second tick. No build, test, validator, security review, OS qualification, or
real-user acceptance was run; this is source-level implementation, not production-safe
qualification. See `docs/STORAGE.md`, `docs/SERVICES.md`, F90, and E02-S03 for contract and
later verification cases. No commit or push was made.

## Active owner direction — desktop/local implementation

The owner has directed implementation across the finalized V1 feature set, with desktop /
local first. Cloud continuation and remote Runtime follow. Verification and testing are
explicitly deferred to a later pass; code written during this run is therefore not yet
validated or production-qualified. Do not interpret implementation presence as feature
completion.

### Latest change — Typed folder provenance and additive upload migration (2026-10-07)

Folder-selected uploads now carry normalized relative-path provenance through the initial
upload request, idempotency record, resumable session, Resource provenance, and the
versioned `resource.created.v2` payload. The path remains display/provenance metadata and
never becomes a filesystem locator or grant. The desktop preserves it across resume
restarts. SQLite v1 is restored byte-for-byte as the immutable baseline; additive v2 now
adds lifecycle/progress and folder-provenance columns, chunk idempotency and blob-recovery
tables, and guards. V2 recovers committed Resource IDs from status events when possible.
Legacy sessions without a whole-file digest remain readable as historical records but
cannot be safely resumed or committed. Non-folder upload idempotency payloads retain the
previous canonical shape. A read-only review found and the source was updated for the
folder-path resume rewrite and stored chunk-size load. No build, test, validator, or
real-user acceptance has been run; the migration and implementation remain unverified.

### Previous change — Resource upload lifecycle transitions (2026-10-07)

Separated upload lifecycle `version` from `progress_version`: new accepted chunks advance
only transfer progress; lifecycle transitions advance the aggregate revision used by events.
The final contiguous chunk now atomically stores its receipt, advances both versions, saves
the CONTENT_RECEIVED aggregate-state blob, and appends OPEN -> CONTENT_RECEIVED. Commit now
constructs the committed session snapshot and atomically writes the Resource/revision/location,
Resource-created event, committed upload projection, COMMITTED aggregate-state blob/event,
and idempotency receipt. TTL expiry uses progress-version fencing and advances lifecycle
version only. Definite stored-content integrity failure now atomically changes the session
to FAILED, advances only lifecycle `version`, and appends its aggregate snapshot/event before
returning an integrity error; transient storage/database errors remain retryable. The OpenAPI,
SQLite DDL transition guard, data model, state machine, service, API, event, and F90 flow
contracts were updated. Empty uploads begin at CONTENT_RECEIVED revision 1.

This covers upload creation, final-chunk receipt, expiry, successful commit, and definite
content-integrity failure events. Transient storage/database errors remain retryable. This is
source-level implementation only. No build, test, validator, OS qualification, or end-to-end
acceptance was run per owner direction. The later orphan-cleanup section above supersedes the
cleanup status recorded when this change was first made. See the working tree; no commit or push was made.

### Latest change — resumable desktop Resource intake (2026-10-07)

Implemented an unverified resumable upload slice across the local SQLite store, encrypted
BlobStore boundary, authenticated Operator, Tauri native commands, and Library UI. Limits
are 100 MiB per file, 100 files/100 MiB per selection, fixed 4 MiB chunks, and 24-hour
session expiry. Chunk metadata is durable; bytes use the separate encrypted
`RESOURCE_UPLOAD_CHUNK` purpose. Tauri persists only upload/request/file metadata and
re-hashes previously accepted chunks against a reselected file before resuming. Empty files
commit without chunk rows. Resource, initial revision/location, Resource-created event,
commit receipt, and session COMMITTED state share the final SQLite transaction. No build,
test, validator, OS qualification, or provider/system acceptance was run. The later
lifecycle-event slice below supersedes the event-status note here; the later cleanup update
above describes the current collector implementation.

The desktop UI changes were completed by an already-authorized isolated helper in
`apps/litecowork-ui/`; the daemon/storage integration remains under the primary agent.

Current implementation slice:

- Added a first local Workspace-instructions API slice: immutable instructions revisions
  reference a pinned same-Workspace Resource revision, accept only verified UTF-8 text up
  to 64 KiB, require the Workspace version via `If-Match` and an idempotency key, and
  atomically commit the instruction record, Workspace current-revision/version projection,
  domain event, aggregate-state blob reference, and replay receipt. The list endpoint now
  uses an opaque Workspace-bound cursor over the indexed immutable revision table. The
  Tauri shell loads all pages with a repeated-cursor guard. The shell now exposes an
  instruction editor, loads the latest revision's Resource content, saves by first
  importing an immutable text Resource and then committing the instruction revision, and
  displays revision history. The UI keeps a stable request key across retries of an
  unchanged draft. Resource import and instruction commit are separate transactions, so a
  failed second step may leave an unreferenced Resource; there is no cleanup/garbage
  collection path yet. This has no TaskSpec pinning or Hub authority, and has not been
  built or tested; it is a local slice, not complete Workspace instruction support.

- Added an initial desktop Resource quick-import path: the Library accepts multiple files,
  folders (as one-time file attachments with safe relative paths retained as display names),
  and ZIP files; per-file size is capped at 10 MiB
  and one selection at 100 files/100 MiB. The authenticated Operator stores file bytes via
  `BlobPurpose::Resource`, writes Resource/ResourceRevision/ResourceLocation metadata and
  a `resource.created.v1` event with an idempotency receipt, and returns a bounded catalog
  list. The content is encrypted locally. The follow-up read path is Workspace-scoped,
  resolves only the current revision, verifies decrypted bytes against revision digest/size,
  returns bytes as `application/octet-stream` with no-store/nosniff, and lets Tauri preview
  only UTF-8 text media up to 1 MiB. Preview requests are invalidated on Workspace switches
  to avoid showing a late result in another Workspace. These edits are **unbuilt and
  untested** and do not satisfy resumable upload, ZIP extraction, folder-root, indexing, or
  search contracts.

- Added `storage-sqlite::OsWorkspaceBlobKeyProvider`, using the OS credential store for
  versioned, Workspace/purpose-scoped encrypted-blob keys. The provider fails closed and
  has no file/plaintext fallback. Rotation, restart behavior, Linux/macOS/Windows key-store
  access, dependency build, cryptographic review, and key-loss recovery remain unverified.
- Added `LocalWorkspaceStorage::open` as the daemon-facing composition for SQLite plus
  encrypted file blobs under one private state root; `litecoworkd` now opens it on startup.
- Added `keyring` 3.6.3 with target-specific Linux, macOS, and Windows backends to preserve
  the workspace's declared Rust 1.85 MSRV. Cargo.lock was updated by dependency resolution;
  axum/Tokio/time Operator dependencies have now also been resolved into Cargo.lock using
  the Rust 1.85 compatibility resolver. No build or test was run in this slice.
- Added `litecoworkd run/status` with a local single-instance lock and persisted bootstrap
  Runtime identity. `status` now probes lock ownership so stale state after a crash is
  reported as `NOT_RUNNING`, not mistaken for a live daemon. The lifecycle remains
  `DEGRADED`; Task/Effect/lease recovery and execution remain absent.
- Added a Tauri 2 + React desktop shell. Native commands can discover/start the daemon and
  render its actual lifecycle state and blockers. The daemon is a separate process and
  survives closing the window; the UI can create/list Workspaces and import/list local
  Resource metadata and preview small text Resources. It still cannot accept a Task. Daemon packaging into signed desktop installers is not
  wired or qualified. Development executable lookup supports `LITECOWORKD_PATH`/`PATH`;
  release lookup is limited to app resources or the executable directory.
- Added an authenticated local Operator slice: loopback-only ephemeral endpoint,
  per-incarnation 256-bit bearer credential, private connection descriptor, exact Host
  matching, and browser-Origin rejection. GET Workspace list/read enforce local owner and
  selected Workspace scope. POST Workspace create is routed through `WorkspaceService`
  and stores the idempotency receipt in the same SQLite transaction as Workspace state and
  its domain event. The command honors supported initial replication policies; it rejects
  `SELECTED_FOLDERS` until Workspace roots exist. The native Tauri layer lists/creates
  Workspaces without exposing the token to frontend JavaScript. Resource import/read and
  replication-policy update are now present as unverified slices; remaining policy/root
  operations, persistent roots, task mutations, event streaming and OS qualification are
  open. Resource catalog pagination has since been added but is unverified.
- The Operator's standard listener is converted to Tokio inside its active API runtime,
  avoiding listener conversion before a reactor exists. This source review is not a
  runtime verification result.
- Added a desktop Settings flow to create Workspaces and list the resulting local records.
  The normal-work composer remains disabled because durable Task admission/execution is
  absent. Library now uses resumable file upload, small text-preview, keyset-paginated
  catalog loading, and local in-memory name/type filtering; indexed and full-text Resource
  search remain absent.
- The active Workspace selector now restores the last selected local Workspace and falls
  back to the first available Workspace if that record was removed. This selection is
  local Operator presentation state; Workspace-scoped Resource and policy requests use the
  authenticated Operator, while most domain requests remain unwired.
- No test/build/validator was run for these edits, per the owner's instruction to defer
  verification. The shell and lifecycle changes are unvalidated on every OS. Run the
  requested verification pass before relying on them.
- Added the E02-S03 increment: authenticated current-revision Resource byte retrieval,
  encrypted BlobStore read plus digest/size verification, `application/octet-stream` with
  `no-store`/`nosniff`, and a Tauri preview limited to UTF-8 text-like content at 1 MiB.
  Preview requests are invalidated on Workspace changes and content is shown as escaped
  plain text. Added F86, API/OpenAPI docs, later verification cases, and regenerated the
  implementation coverage inventory. Code is still unbuilt/untested; binary preview,
  downloads, ZIP extraction, roots, indexing, and search are still absent. Resumable upload
  is now implemented as an unverified slice described above.
- Added an initial E02-S02 Workspace policy slice: authenticated `PATCH /workspaces/{id}`
  requires matching `X-Workspace-ID`, owner identity, `If-Match`, and an idempotency key;
  policy projection, event, and replay receipt use the SQLite transaction path. Desktop
  Settings exposes local-only, metadata, active-input, and full-Workspace policy choices
  and clearly says cloud transfer is not connected yet. Selected-folder policy remains
  unavailable until persistent WorkspaceRoot creation and authorization exist. This code
  and its dependent UI are unbuilt and untested.
- Aligned quick-import empty-file handling with the Resource schema: zero-byte files are
  now accepted by the UI and Operator, and the encrypted content-addressed BlobStore
  supports their empty-content digest. This slice is also unbuilt and untested; its
  acceptance case is recorded under E02-S03.
- Added a bounded client-side Library filter over the currently loaded Resource catalog,
  matching case-insensitive display name and media type only. It clears when the selected
  Workspace changes and explicitly states that Resource content is not indexed/searched.
  This is a UI convenience, not the contractual deterministic search/index feature; no
  build or UI test has been run.
- Replaced the fixed 500-row Resource catalog read with stable keyset pagination. The
  local Operator accepts `limit` (1–100) and a Workspace-bound opaque cursor, and SQLite
  pages by descending `(created_at, resource_id)`. Tauri requests one page at a time; the
  Library exposes “Load older files”, appends unseen Resource IDs, and rejects empty or
  repeated cursors. Name/type filtering explicitly covers loaded rows only. This removes
  catalog truncation without fetching the entire catalog at once; content search, indexing,
  root scope remained absent at that point in the chronological log; resumable uploads were
  implemented later in the latest-change section above. These edits are unbuilt and
  untested; the OpenAPI inventory/coverage artifacts need refresh in the later contract
  verification pass.
- Next implementation slice: finish E02-S02 WorkspaceRoot identity/grants and policy/root
  consistency, followed by ZIP validation/extraction, deterministic indexing/search, and
  file identity/freshness; then proceed into real native-agent discovery/bindings and durable
  Task admission. Encrypted upload orphan cleanup is now implemented in source but remains
  unverified. Keep the bearer credential native-only and transactional RequestId
  deduplication on mutations.

Feature coverage remains largely unimplemented: complete daemon lifecycle, authenticated
Operator API, Workspace/resource UI and Resource ingestion/indexing, Task/Attempt runtime, native coding-agent
adapters, Trust/capability/Effect/Evidence execution, heterogeneous production switching,
RAG/local-model integration, browser/computer execution, Coworker/Goals/automations,
Artifact Workbench, and Presentation Runtime. Keep status labels honest as each story is
implemented.

## Previous spike — SP04 handover PoC (deferred)

The owner redirected the active work to the Codex (`gpt-6-luna`, medium) → OpenCode
handover feasibility question. This is **not a coding-speed benchmark**. The current local
prototype materializes manifest-listed checkpoint files into a digest-addressed, read-only
snapshot before sender settlement and re-verifies that snapshot at receiver admission. The
Linux bubblewrap supervisor passes manifest-checked, sealed `memfd` file contents as
read-only receiver mounts while leaving the receiver project directory writable. Forty-two
focused PoC tests pass. Mutating the original
sender workspace after settlement leaves the pinned snapshot unchanged; tampering with the
stored snapshot blocks admission. An unchanged snapshot is admitted after ledger restart at
lease epoch 2. An integrated Linux fixture now runs sender stop/quiescence → snapshot → ledger
settlement → replacement admission → receiver mount; the receiver reads the pinned bytes,
cannot write the mounted snapshot, updates the working project file, and writes in its project
directory. Host changes after sandbox launch do not change the receiver's sealed bytes, and a
pre-launch digest mismatch prevents worker start. Replacement state distinguishes `ADMITTED`,
`RUNNING`, and `START_FAILED`; a failed launch releases the Task's active slot while consuming
its lease epoch, and retry starts at a higher epoch. Effect reconciliation and lease release
remain mocked, and this is not product Task/Attempt integration. Packaging is capped at 4,096
files and 512 MiB total; oversized checkpoints fail before worker launch.

A Linux restart test crashes the Runtime after launching the contained heartbeat writer but
before committing `RUNNING`. Recovery blocks while the launcher identity is live; after it exits,
the admission becomes `START_FAILED`, and retry uses lease epoch 3. A second test crashes the
Runtime after `RUNNING` is committed. The ledger pins both the Runtime launcher identity and the
contained bubblewrap process identity; recovery waits for both to be proven gone, marks the old
Attempt `ABANDONED`, records mocked Effect/lease reconciliation, and retries at epoch 3. Both
tests require a stable heartbeat quiet interval. PID reuse and old-boot identities are rejected,
unreadable identity data fails closed, and an unresolved Attempt still blocks duplicate writers.
The two Runtime-crash recovery tests were each repeated five additional times; all 10 repeat runs
passed. This is repeatability evidence for the Linux fixture transitions only.
This remains a Linux PoC; it is not integrated with product RuntimeLifecycle, TaskService, real
lease fencing, or Effect reconciliation.

The 2026-10-07 review pass is recorded in
[`reviews/SP04-HANDOVER-REVIEW.md`](reviews/SP04-HANDOVER-REVIEW.md). HO-01 through HO-04
scratch diffs were unavailable; HO-05 was source-reviewed and corrected. This host is WSL2,
so native Linux/macOS/Windows containment remains unqualified. Product Task/Attempt/Lease/
Effect switching is not implemented; see the dependency sequence in the review packet. Do not
report switch speed as a pass criterion. See [`spikes/SP04.md`](spikes/SP04.md).

## Implementation backlog status (separate from the active SP04 PoC)

**E01-S02 — Transactional persistence** (`IN_PROGRESS`, owner review pending).

E01-S01 remains `IN_REVIEW`: owner clean-clone acceptance on the reference machine and
hosted GitHub Actions are still pending. This S02 work continues at the owner's direction;
it does not waive the dependency or claim either story accepted.

## Repository state

- Repository: `litecowork`
- Branch: `main`
- Current HEAD: `fe11ea7c0eaf4e2d5867e28549cc6d2d645a206b`.
- Work since HEAD is uncommitted in the working tree; no push was performed.
- No push is authorized or performed in this run.
- V1 delivery order remains desktop/local, cloud continuation, then remote Runtime.
- Coding agents implement and report evidence; the owner reviews and accepts product
  behavior. Local tests do not establish hosted CI, owner-machine acceptance, provider
  qualification, or production readiness.

## Latest verification — 2026-10-07

Continuation from `fe11ea7` split the E08 product UX work into reviewable implementation
stories. Those Presentation Runtime and Workbench items remain planned; this does not claim
that UI behavior has been implemented.

- `python3 -m pytest -q implementation/spikes/agent_switching_poc/tests`: **42 passed**.
- HO-05 disposable fixture `python3 -m unittest -q`: **20 passed** after direct source review
  and regression fixes for integer conversion limits and not-before rounding.
- The two Runtime-crash recovery tests were each repeated five additional times: **10/10 passed**.
- `cargo test --locked -p storage-sqlite sqlite_full_error_maps_safely_and_rolls_back_workspace_commit -- --nocapture`:
  **1 passed**. SQLite's page limit generated `SQLITE_FULL` inside the transaction function;
  aggregate/event/origin sequence remained unchanged, and retry after restoring capacity
  committed at the next contiguous sequence. This does not simulate physical filesystem,
  WAL/SHM, or encrypted blob-volume exhaustion.
- `bash scripts/check.sh`: **passed**, including Rust workspace tests (26 total), six
  repository Python tests, architecture validation, 65-document/4,860-row coverage
  validation, and implementation-plan validation (57 stories).
- `git diff --check`: passed. No commit or push was made.

The Agile plan assigns E08-S04 to Work/activity projections, E08-S06 to typed presentation
and reconnect-safe streaming, E08-S07 to the Artifact Workbench/version history, and E08-S05
to integrated accessibility plus nontechnical/technical acceptance. E06-S05 owns resource
readiness and Context-used source presentation; E08-S03 owns editable context lifecycle.

## Source-only continuation — 2026-10-08

E08-S03 received a narrow desktop provenance refinement in Settings → Workspace instructions:
revision history now shows each immutable instruction revision's source Resource ID and
revision, content digest, and parent instruction revisions. The UI explicitly says this is
saved Workspace guidance provenance, not evidence of what a Task or agent actually used.
This reuses the existing authenticated Workspace-instruction history response; it adds no
Operator route and does not expose historical Resource bytes.

The full ContextDocument lifecycle remains blocked at the product API boundary: although
architecture/OpenAPI describe Resource metadata, revision uploads, status changes, and
deletion receipts, the local daemon router currently mounts none of those ContextDocument
or Resource detail/revision lifecycle routes. The desktop therefore does not classify
ordinary Library files as ContextDocuments, edit ContextDocument revisions, revoke/delete
context, or claim purge completion. Safe native-session invalidation is also not integrated.
No tests, build, formatter, validator, migration, or runtime checks were run for this
source-only change; verification is deferred by the current run instruction.

## Story status — 2026-10-07

| Area | Status | Evidence / remaining gate |
|---|---|---|
| E01-S01 toolchain and CLI foundation | `IN_REVIEW` | Local gate and recorded clean-clone run pass. Owner reference-machine acceptance and hosted GitHub Actions remain pending. |
| E01-S02 Workspace persistence | `IN_PROGRESS` | Workspace SQLite/blob slice and earlier storage checks are implemented; an OS-keystore provider and local storage composition were added after the last full check and are unbuilt/untested. Physical disk/WAL/blob exhaustion, owner review, supported-hardware capacity/memory/shutdown checks, key rotation/recovery qualification, cryptographic review, and product-domain expansion remain open. |
| Runtime lifecycle, authenticated Operator API, desktop | `IN_PROGRESS — IPC source integrated; unverified` | Daemon opens storage, holds a process lock, persists local incarnation state, validates the Linux/macOS OS-principal binding, and serves owner-scoped Workspace/Resource/upload routes through framed Unix IPC. Tauri authenticates the peer and uses the same route semantics. Windows fails closed. Source is unbuilt/untested; Workspace roots, ZIP extraction, indexing/search, Task recovery/execution, event streams, packaged sidecar/service-manager integration, and all OS qualification remain open. |
| Task execution and safe agent switching | `NOT_IMPLEMENTED` | No durable Task/Attempt runtime, agent adapter, handoff coordinator, fencing path, or end-to-end switching test exists yet. |
| Handover PoC | `IN_PROGRESS — five disposable handovers reached task acceptance, with two independent-review corrections in HO-05; Linux fixture covers quiescence, sealed checkpoints, and recovery before and after RUNNING` | Cases 1–3: sequential Codex GPT-6 Luna/medium → OpenCode handovers; combined suites passed 9, 6, and 10 tests. Case 4: Codex partially implemented `normalize_slug` with `is_valid_slug` unfinished; native App Server turn interruption returned `interrupted` but the command kept writing for at least 20 seconds. After sender-host shutdown and a verified quiet interval, OpenCode Ling 3.1 Flash Free completed the task; all 6 tests passed and were independently rerun. MiMo free returned HTTP 429 for cases 2 and 4; Ling was used as fallback. The 42-test disposable suite materializes/revalidates snapshots, seals verified file bytes into memfds, rejects changed snapshots before launch, exercises sender quiescence and ledger settlement, distinguishes admitted/running/abandoned/start-failed replacement states, recovers Runtime crashes before and after RUNNING only after process identities are proven gone, fails closed on missing/unreadable identities or unresolved Effect/lease gates, retries at a higher epoch, tests Linux receiver mounting with a writable project directory, and confirms fail-closed duplicate prevention after ledger restart. It does not establish product Task/Attempt switching: Effect/lease reconciliation are mocked, product Runtime recovery is unimplemented, and other desktop OS containment remains unqualified. These cases establish preliminary checkpoint-continuation feasibility, not broad reliability. HO-01 through HO-04 source-level review is limited because their scratch trees are gone; HO-05 adds a sequential handover with independent HTTP grammar and input-boundary corrections; see [`spikes/SP04.md`](spikes/SP04.md). |

Five disposable coding handovers show the selected agents continued from bounded packets and
project state across three completed-subtask checkpoints, one partial implementation with
failing tests, and a fifth digest-pinned retry-policy checkpoint. The fourth receiver restated
the task, completed work, remaining requirements, and acceptance criteria before editing; its
manifest digest was checked, and its combined six-test suite passed independently. The fifth
receiver also restated its handoff correctly and completed the task; independent review found
and corrected one HTTP grammar edge. In that live case, Codex `turn/completed(interrupted)` did
not stop the spawned sandbox writer: its heartbeat advanced for 20.12 seconds while the App
Server remained alive. The sender host was stopped, then the heartbeat remained unchanged for
three seconds before OpenCode admission. Therefore an interrupted turn status is not a safe
switch fence; sender-host/environment shutdown and observed quiescence are required. This does
not establish broad reliability, product Task/Attempt persistence, actual lease fencing, Effect
reconciliation, provider-independent cancellation, or cross-platform containment. See
[`spikes/SP04.md`](spikes/SP04.md) for evidence and remaining gates. The attempt to run Codex
inside the fixture's read-only-root sandbox failed during native App Server initialization
before task work; it remains an adapter/Environment boundary finding, not a handover case.

## Completed implementation scope

- Added Rust `storage-core` ports for `StateStore`, `EventStore`, `BlobStore`, and their
  Workspace persistence composition.
- Added `domain-workspace::WorkspaceService` create and replication-policy update commands
  with expected-version checks and closed, versioned Workspace events.
- Added `storage-sqlite`: bounded writer actor; FK/WAL/FULL durability configuration;
  canonical v1 DDL migration; source checksum and resulting-schema fingerprint; bounded
  busy timeout; immediate transactions; atomic Workspace projection/event/origin-sequence
  commits; replay from immutable state blobs; and error mapping.
- Added encrypted content-addressed `FileBlobStore` with Workspace/purpose-scoped paths,
  XChaCha20-Poly1305 authenticated data, versioned injected keys, digest/size validation,
  atomic no-clobber writes, fsync, and fail-closed permissions/key errors. Existing tests
  use a fixed test-only key. A separate OS credential-store provider was just added and
  has not yet been compiled, exercised, or platform-qualified.
- Added adversarial tests for JCS ordering/number formatting, unsafe integer rejection,
  migration receipts/rollback, schema drift, concurrent stale writes, transaction rollback,
  busy timeout, replay, corrupt/wrong-scope blobs, unavailable keys, broad directory
  permissions, private SQLite state-directory/file permissions, symlink database paths,
  and failed BlobStore commits.
- Updated storage/event/schema docs, the E01-S02 story/backlog, research references and
  generated implementation coverage.

## Active SP02 qualification update

- Separate SQLx 0.9.0 and rusqlite 0.40.2 executables use the same `WorkspaceService`,
  production encrypted `FileBlobStore`, eight concurrent producers, 32-slot bounded writer,
  WAL/FULL settings, and host SQLite 3.45.1. SQLx is configured with `sqlite-unbundled`
  alone; the SQLx `sqlite` convenience feature also enables bundled SQLite. Product
  `storage-sqlite` remains bundled by default; only the rusqlite spike disables that default.
- Both executables close/reopen the database, verify the Workspace projection, decrypt
  aggregate-state blobs, replay events, and compare the replayed state. The rusqlite side
  uses the product adapter. SQLx is a prototype and lacks the product migration receipts,
  schema-drift checks, full path hardening/error mapping, and root-scope reads.
- Three sequential optimized 200-update runs per driver were alternated on Ubuntu 24.04.4
  LTS under WSL2, x86_64, Linux 6.18.40.1, Rust/Cargo 1.98.1, system SQLite 3.45.1.
  Median throughput was 327.4 updates/s for rusqlite and 322.9 for SQLx; observed ranges
  overlap, so this experiment does not select a driver. Exact samples and method are in
  `implementation/spikes/SP02.md`.
- `storage-sqlite` retains bundled SQLite and rusqlite's default cache/wasm-compatible
  features for production. The benchmark disables only SQLite bundling while explicitly
  preserving those rusqlite defaults. The storage runtime test wording now applies to both
  bundled and system SQLite builds.
- Added a rusqlite product-adapter mixed workload: 40 Workspace writers, four readers, a
  32-slot writer queue, and 1,200 updates per measured run. Readers check monotonic,
  policy-consistent snapshots; close/reopen verifies replay for all 40 Workspaces. Three
  sequential runs passed integrity checks but showed highly variable elapsed and tail
  latency. Exact queue occupancy is not exposed; longer capacity qualification remains open.
- Added process-local `SqliteWriterMetricsSnapshot` observations for outstanding command
  submissions and bounded-channel send wait. A held external writer lock test confirms
  blocked callers raise the high-water mark and settle without a partial Workspace write.
  This is runtime telemetry only; it is not durable state, exact queue occupancy, or an
  Operator metrics export.

## Verification status

Focused checks passed during this run:

- `uv run --locked python -m unittest discover -s implementation/spikes/agent_switching_poc/tests -v`:
  42 PoC tests passed, including Linux PID-namespace containment of a `setsid()` descendant,
  packet/checkpoint integrity rejection, malformed handoff, crash recovery, restart before
  replacement admission, immutable snapshot materialization, source-workspace isolation,
  tampered-snapshot admission rejection, sealed-byte handling under host mutation, launch-time
  digest rejection, size bounds, read-only receiver mounts with writable project workspaces,
  replacement startup failure cleanup, Runtime crash during worker launch, verified owner
  identity recovery before and after the replacement reaches `RUNNING`, PID-reuse/boot
  fencing, fail-closed missing-worker-identity handling, duplicate prevention, and same-lead
  retry at a fenced higher lease epoch.
- SP04-HO-04 in the disposable receiver fixture: `python3 -m unittest` passed all six tests
  in OpenCode and in an independent rerun; independent boundary checks covered six accepted
  values, ten rejected values, four non-string `TypeError` cases, and the sender's
  `normalize_slug` example. The sender heartbeat remained at 198 for three seconds after
  receiver completion. This correctness evidence is not a speed measurement.
- The disposable Codex → OpenCode coding task's combined suite: 9 tests passed after both
  agents completed; source and tests remain only under `/tmp/litecowork-sp04-coding-jsvc7ifu`.
- `cargo test --no-default-features --features sqlite-rusqlite-defaults -p storage-sqlite --offline`:
  19 passed using system SQLite while retaining rusqlite's non-bundle default features.
- `cargo test --manifest-path implementation/spikes/sp02-driver-compare/Cargo.toml --offline`:
  6 passed, including the encrypted Workspace restart/replay run and production blob tests.
- `cargo test --manifest-path implementation/spikes/sp02-driver-compare/rusqlite/Cargo.toml --offline`:
  2 passed using the product adapter, including mixed-load snapshot/replay correctness.
- Three release samples per driver completed; every run reported SQLite 3.45.1, eight
  replayed Workspaces and `replay_matches=true`.

The full `scripts/check.sh` passed with exit code 0 after refreshing generated coverage. It
ran Rust formatting, Clippy, build and 25 workspace Rust tests; six repository Python tests;
architecture validation; plan and coverage validation (64 documents, 870 sections, 4,827
traceability rows); and `git diff --check`. Locked SQLx and rusqlite spike test commands also
passed after the feature correction. Default production bundled SQLite remains SQLite 3.53.2
as previously recorded. Owner-machine acceptance and hosted GitHub Actions remain pending.

Exact review instructions and remaining product limitations are in
[`reviews/E01-S02.md`](reviews/E01-S02.md) and [`spikes/SP02.md`](spikes/SP02.md). SP02 and
E01-S02 remain partial/in progress. The mixed-load correctness and initial pressure sample
is complete, but longer supported-hardware capacity qualification, memory,
cancellation/shutdown, disk-full injection, key-service qualification, key rotation and
cryptographic review are still open.
The storage library is not yet wired into
`litecoworkd`, the desktop, Operator transport, or a real external provider.

## Next implementation scope

Current owner direction supersedes the earlier SP04-review sequencing: prioritize the
desktop/local product end to end; cloud continuation and remote Runtime follow. The active
sequence is OS-keystore qualification/build, daemon lifecycle, authenticated local Operator
boundary, Tauri shell and Workspace/resource intake, durable Tasks/Attempts, Trust-mediated
capabilities and Evidence, native adapters/delegation, then remaining local product surfaces.
Keep each slice tied to its current Epic and authority docs. Do not mark the Linux handover
PoC as production switching. Verification/testing is deferred by the owner; label code from
this run unvalidated until the dedicated pass. Do not push without explicit instruction.

## Active Runtime identity/migration slice — 2026-10-08

The v4 source review found two admission gaps and one documentation split. Migration
preflight now requires an ACTIVE EXECUTOR binding for reusable/nonterminal Environments;
only FAILED/DESTROYED historical Environments may retain an inactive binding. Nonterminal
AutomationOccurrence updates now require an ACTIVE TRIGGER_HOST binding, and revocation
waits for all pinned nonterminal occurrences as well as enabled cursors. Channel assignment
activation now rechecks ACTIVE binding, and migration preflight rejects revoked ChannelHost
rows with unexpired leases. Occurrence identity and terminal status are immutable; legal
status edges are enforced, and the materialized Task pointer is one-time and cross-checked
against the Task's origin occurrence. Duplicate Tasks per occurrence are rejected, direct
occurrence insertion must begin PENDING/unmaterialized, and migration preflight cross-checks
both sides of existing occurrence/Task links. DATA-MODEL and
STORAGE now distinguish installation-local durable Runtime registration from authenticated
Mesh publication. SERVICES, STATE-MACHINES, and F93 now specify that disabling an
Automation prevents new occurrences but does not settle existing work. A read-only review
of these corrections is in progress.

Independent reviews found and drove closure of several v4 boundary gaps. Occurrence claim
triggers explicitly reject unparsable old/new expiry values (SQLite date comparisons
otherwise return NULL), and migration preflight rejects malformed or impossible legacy
claim/status/epoch/expiry/Task tuples. F93 records these required later verification cases.
ChannelHost receipt settlement is now allowed during DRAINING only for an unchanged,
unexpired source claim; new claims/reclaims remain blocked. Lease records pin a minimum
30-second expiry-plus-clock-skew margin, quiescent drain proofs are immutable and tied to
the source lease/control version, and lease deletion writes an immutable release record.
Reassignment requires that record rather than mere lease-row absence. Continuity provenance
is immutable within a host epoch; new epochs require replay evidence or an owner-decision
AuditRecord for `GAP_ACCEPTED`. The assignment event contract preserves v1 and adds v2 for
release/continuity provenance. The migration fails closed for legacy host epochs whose
provenance cannot be reconstructed. These changes are unverified; tests/builds/validators
remain deferred.

The daemon startup identity prerequisite is now implemented in source: ADR-0021 specifies
an OS-keystore-backed Ed25519 installation identity before local Runtime/incarnation
registration. There is no placeholder-key path; missing or mismatched credentials fail
closed before Operator startup. Registration still creates no Workspace binding and emits
no Mesh state. The migration and startup sources remain unverified; no test/build/validator
has been run for this active slice, and no production readiness is claimed.

### Runtime startup registration source changes

Added ADR-0021 to fix the previously unspecified device-signature contract: Ed25519,
`ed25519:<lowercase-hex>` public-key encoding, and a `DeviceId` derived from the public
key digest. The private seed is generated with the OS CSPRNG and stored in a dedicated
OS credential-store entry, independently scoped from Workspace blob keys. This is
OS-protected software key material; no hardware-backed/non-exportable guarantee is made.
Missing/corrupt credentials or a bootstrap RuntimeId mismatch fail closed.

Added `RuntimeLifecycleStore` with a SQLite single-writer transaction that inserts or
validates the installation Runtime identity, commits a new `RECOVERING` incarnation and
its local-only observation atomically, and performs version-fenced lifecycle transitions.
`litecoworkd run` now registers after `LocalWorkspaceStorage::open` and before starting the
Operator API; it updates durable state to `DEGRADED` because Task recovery remains absent.
Graceful shutdown persists `DRAINING`, `STOPPING`, then `STOPPED` / Runtime `OFFLINE` after
the Operator listener stops. This records daemon lifecycle only; it does not claim Mesh
publication, Workspace enrollment, child-process reconciliation, or Effect settlement.

The seed decoded from the OS credential entry is held in `zeroize::Zeroizing<[u8; 32]>`;
the short-lived `ed25519_dalek::SigningKey` is dropped immediately after public-key
derivation. This relies on the enabled `ed25519-dalek/zeroize` Cargo feature, whose
`SigningKey` Drop implementation zeroizes its private scalar. This feature assumption is
part of ADR-0021 and must remain enabled or be replaced by an equivalently reviewed
implementation.

No test, build, architecture validator, migration execution, OS-keyring qualification, or
crypto review was run, per owner direction. Source behavior remains unverified. The
data-directory-bound credential scope means moving the state directory needs a future
explicit identity-recovery flow. No Workspace binding or Task execution was added.

### ChannelHost safe-switch review follow-up (2026-10-08)

Three independent read-only agent reviews were used to check SQLite/Rust migration safety,
event/API/schema alignment, and receipt/lease lifecycle races. They found and documented
seams missed in the initial solo pass: operational proof refs were modeled as Resources,
drain proofs could bypass the skew wait after lease expiry, continuity refs were not
verified records, legacy provenance preflight had a write race, receipt ingress needed a
serialized drain boundary, and the no-target Runtime-revocation path lacked a complete API
projection. The event/OpenAPI contracts and lifecycle docs have been revised accordingly.

Root edits now require a live matching source lease when recording quiescent drain proof,
add an immutable continuity-proof record referenced by assignment, check owner identity for
ingress-gap AuditRecords, enforce assignment-to-current-lease cardinality with deferred
SQLite foreign keys, gate assignment deletion on a safe release record, and run legacy
provenance/lease preflight under `BEGIN IMMEDIATE`. Migration preflight rejects unrecoverable
legacy assignments without matching leases. The v1-v3 schemas contain no persisted skew
margin; v4 documents the 30-second contract minimum as the compatibility backfill.

These changes remain unverified: no SQL execution, tests, builds, validators, OS/provider
qualification, or production-safe switching claim has been made. Next review must reconcile
the final SQL with F46/F93, confirm event projections and nullable assignment API shape, and
then schedule the deferred race/migration tests and per-OS qualification. No files were
staged or committed.

### Safe-switch contract closure follow-up (2026-10-08)

The next source pass fixed a malformed target-binding query in the continuity-proof
admission trigger. Runtime Mesh, Services, and State Machines now distinguish a persisted
expired lease row on a legacy DRAINING assignment from an unexpired ACTIVE authority: v1→v4
rejects ACTIVE rows without a matching unexpired lease and DRAINING rows without any
matching lease row; an expired DRAINING row is provenance only and receives the v4 skew
boundary backfill. The docs now make the trust boundary explicit: only authenticated
RuntimeMesh commands derive drain/continuity proof values from Hub receipt, Effect, lease,
and replication state; valid JSON/zero counters in SQLite are not proof or authentication.

This closes contract wording and the inspected query shape only. It does not prove the SQL
parses, the migration succeeds, service serialization is implemented, or switching is safe.
No tests/builds/validators/SQL execution were run, as verification is deferred. Next local
product slice remains blocked on durable AgentBinding/session admission before exposing
Task creation; storage-only Task persistence must not be presented as usable execution.

### Codex native initialization and desktop dependency audit (2026-10-08)

Official OpenAI App Server guidance confirms the stdio launch mode, stable `clientInfo`
initialization, and initialize/initialized ordering. The Codex transport now launches
`codex app-server --listen stdio://`, uses a typed client-info constructor, and rejects an
initialize result unless bounded `codexHome`, `platformFamily`, `platformOs`, and
`userAgent` fields are present. Machine-local values are validated but not exposed,
persisted, or logged. This remains an unintegrated protocol transport; no Codex process,
account, model, or inference was accessed.

Three read-only audits established the next local dependencies: Task storage is atomic,
but Task creation cannot be exposed until AgentProfile/Endpoint/Binding and execution
readiness admission exist; persistent WorkspaceRoot grants must wait for OS-authenticated
local IPC and cannot reuse one-time folder upload; ZIP extraction and content RAG need
bounded parsing and protected index storage, not plaintext SQLite FTS. The current loopback
bearer API and IPC framing crate still lack authenticated listener/client integration. No
tests/builds/validators/SQL migration or protocol launch were run. The next desktop slice is
OS-authenticated IPC and Operator dispatch, then WorkspaceRoot grants/observation and Agent
binding admission; the Task composer remains disabled until planning/session admission is
real.

### Required later verification cases

- First startup creates one device identity and one `RECOVERING` incarnation after SQLite
  opens; repeated lock-holder startup keeps RuntimeId/device identity stable and adds one
  new incarnation.
- Missing, locked, corrupt, or mismatched OS credentials fail closed before Operator API
  startup; private seed bytes do not appear in SQLite, bootstrap JSON, logs, or output.
- A bootstrap RuntimeId mismatch against the keyring identity or an existing Runtime row
  is rejected; a revoked Runtime is not revived by local startup.
- Runtime + incarnation + local observation commit atomically; injected failure leaves no
  partial registration. A stale incarnation/version cannot update the current Runtime.
- Startup remains `RECOVERING` then `DEGRADED`; no Workspace or RuntimeWorkspaceBinding is
  created and no Mesh presence is emitted.
- Graceful shutdown records `DRAINING` → `STOPPING` → incarnation `STOPPED` / Runtime
  `OFFLINE` while preserving the unresolved Task-recovery blocker; this is not evidence
  that Tasks, child processes, or Effects have been reconciled.
- Qualify credential-store behavior on Linux, macOS, and Windows, including locked/unavailable
  backends and application reinstall/data-directory move behavior; separately review the
  Ed25519 implementation and future signing/rotation protocol.

## Active slice — bounded Resource content search (2026-10-08)

Library now has an explicit `ON_DEMAND_CONTENT` mode in addition to default metadata
search. The authenticated Operator considers up to 20 current managed Resource candidates
per request, reads at most 1 MiB per Resource and 8 MiB total, and searches only allowlisted
valid UTF-8 plain text. It decrypts managed blob bytes, checks their digest through the
existing bounded read, then rechecks exact revision/digest before returning a pinned
ResourceRef and optional bounded excerpt. Extracted text, terms, and snippets are transient;
no database migration or plaintext SQLite index was added. The UI displays the per-request
limits and labels matches as an on-demand scan.

Explicit exclusions: ZIP member extraction, PDF/Office parsing, persistent WorkspaceRoots,
durable/background indexing, embeddings, semantic RAG, and local model inference. This is
not a durable index or RAG implementation. Source and contracts are unbuilt and untested;
the owner deferred verification. Later verification must cover query mode/cursor binding,
Workspace isolation, text allowlist and invalid UTF-8/control rejection, current revision
recheck, snippet limits/escaping, file/candidate/aggregate budgets, pagination under budget
exhaustion, ZIP/rich-document exclusion, no content/term/snippet persistence, and the real
Library workflow.

## Active slice — native persistent folder registration and revocation (2026-10-08)

The desktop Library now distinguishes **Add folder** (one-time copy through the existing
upload flow) from **Add persistent folder** (native Tauri directory chooser). The path is
obtained in the native command, encoded as raw Unix path bytes only for the reserved
authenticated local IPC request, opened in the daemon using retained directory handles
and no-follow flags, and excluded from WebView state, public Operator/OpenAPI payloads,
events, aggregate Resource state and replay responses. The daemon derives an HMAC-based
FileIdentity pseudonym from the installation OS-keyring identity and atomically stores the
folder Resource, location, current-incarnation locator and raw local identity bindings,
WorkspaceRoot, three creation events and the idempotency receipt. Owner-scoped root listing
is keyset-paginated and the Library reloads persisted records.

The owner can pause, resume, or revoke a listed root. Pause/resume use versioned status
events and idempotency receipts while preserving identity bindings and selected-root
replication preference. Resume checks that location and identity bindings are available
for the current Runtime incarnation; startup revalidation establishes that state. Revoke
marks the ResourceLocation revoked, deletes local locator and raw identity bindings,
removes the selected-root replication relation, and persists the idempotency receipt
atomically. Already transferred remote copies are retained. The Library calls these
**saved scopes**, not indexed folders, and explicitly states that watchers/indexing are not
active. The local IPC proves the OS principal, not the Tauri executable; another process
running as that principal can call the endpoint. This is not an app-identity attestation.

Pause/resume/revoke and restart identity revalidation are source-only, unbuilt, untested,
and OS-unqualified. A folder replaced after startup revalidation is not detected by the
current resume route; it checks current-incarnation database bindings rather than reopening
the directory. Do not treat this as production-safe identity switching. Watcher, safe
traversal for indexing, change observation, ZIP extraction, durable text extraction/indexing,
RAG and local model integration remain missing. Windows remains fail-closed; other Unix
platforms are not OS-qualified. No tests, build, format, validator, migration execution,
or provider operation was run. Persisted `ACTIVE` state does not prove ongoing root
availability or current observation. Production readiness is not established.

## Active slice — Runtime-start WorkspaceRoot identity revalidation (2026-10-08)

After Runtime incarnation registration and before Operator IPC startup, `litecoworkd` now
pages persisted non-revoked WorkspaceRoots belonging to its Runtime. Linux/macOS source
reopens each saved private locator without following symlink path components, compares the
current directory identity to the prior raw `FileIdentityBinding`, and checks the existing
Resource's stable keyed `identity_digest` and `provider_identity.file_identity` projection.
On exact match, one SQLite immediate transaction binds both private records to the current
incarnation and marks the ResourceLocation AVAILABLE; a previously UNAVAILABLE root returns
to ACTIVE only after that match, while a PAUSED root stays PAUSED. For a root-specific
missing, mismatched, unsupported, or changed identity, the same transaction removes any
partial current-incarnation bindings, records safe reason-coded events, and marks the
location UNAVAILABLE; an ACTIVE root becomes UNAVAILABLE while a PAUSED root remains
PAUSED. Revoked roots are excluded. These
per-root identity failures do not block unrelated Runtime/Operator startup; a storage or
atomic-commit integrity error prevents Operator startup to avoid exposing stale trusted root
state. The SQLite V6 migration adds ResourceLocation UNAVAILABLE and is wired for fresh and
forward schema migration.

This is source-only and remains unbuilt, untested, migration-unexecuted, and OS-unqualified;
Linux/macOS platform qualification is open, and Windows/other platforms fail closed. A
missing or invalid OS-principal/keyring binding is checked before SQLite opens, so that
key-loss case still aborts startup before root revalidation and does not atomically persist
root UNAVAILABLE transitions. Do not claim that case is handled or that root switching is
production-safe. Deferred cases are listed in E02, including same-path replacement,
symlink/missing-root and malformed-binding failures, revoked-root exclusion, migration
rollback/FK restoration, lost-response idempotency, per-root continuation, full transaction
rollback, current-incarnation pair privacy, and owner-operated desktop restart verification.
No tests, builds, formatters, validators, SQL execution, or providers were run.

## Review correction — persistent-root recovery intent and degraded Runtime record (2026-10-08)

Read-only review found that a failed identity check could turn a user-paused root into
`UNAVAILABLE`, after which a later successful Runtime restart would return it to `ACTIVE`.
Startup recovery now keeps `PAUSED` as the root's explicit owner intent while marking its
ResourceLocation `UNAVAILABLE`; exact identity recovery restores location availability but
does not resume the root. Active roots still become `UNAVAILABLE` on identity failure, and
only an exact subsequent identity match returns such a root to `ACTIVE`.

The same review found that a fatal root-query/transaction error after incarnation
registration set only `runtime-state.json` to `DEGRADED`. Startup now also attempts the
versioned durable RuntimeIncarnation/Runtime transition to `DEGRADED` before exiting. If
storage rejects that update, startup still records the local blocker and returns the durable
transition error; it does not start Operator IPC.

Contracts in `STATE-MACHINES.md`, `RUNTIME-LIFECYCLE.md`, and E02 now cover these cases.
This remains source-only and unverified. No tests, builds, formatters, validators, SQL
execution, or OS/provider qualification were run.

## Active slice — persistent WorkspaceRoot pause/resume (2026-10-08)

The authenticated local Operator now exposes `POST /workspace-roots/{id}/pause` and
`/resume`; the Tauri command bridge sends Workspace scope, `If-Match`, and an
`Idempotency-Key`, and the Library updates a root row only after validating the committed
WorkspaceRoot response. Pause accepts only `ACTIVE -> PAUSED` and preserves local identity
bindings plus the selected-root replication preference. Resume accepts only `PAUSED ->
ACTIVE` and the SQLite transaction requires an AVAILABLE location and both local identity
bindings for the current Runtime incarnation. The Runtime startup path establishes these
bindings after no-follow folder identity comparison. Paused intent survives restart and is
not auto-resumed.

Owner-intent idempotency payloads contain the action, WorkspaceRoot, Workspace, and
expected version. Runtime ID/incarnation are commit preconditions, not part of the resume
request digest, so the same request can replay after a daemon restart. Existing revoke
payload shape is preserved for local databases created by the preceding source slice.
Archived-Workspace failures map to `WORKSPACE_ARCHIVED` while exact prior receipts are
checked before rejecting new archived-Workspace mutations.

At the time of this earlier review, resume trusted only current-incarnation database
bindings. That gap is superseded by the later `resume_workspace_root_live` path, which
reopens the saved directory and retains the checked handle through the resume commit. The
remaining limit is that this is a point-in-time admission check: no watcher/content reader
uses the root, and filesystem consumers must perform their own qualified handle-based
validation. The root-list projection still exposes WorkspaceRoot status without the
ResourceLocation availability detail. Runtime watcher/indexer, freshness projection,
Windows named-pipe authentication, and OS qualification remain open.

Source-only changes span domain, storage-core, SQLite, daemon routes, OpenAPI/service/API/
state/flow docs, E02, and the Tauri Library. No tests, builds, formatters, validators,
migration execution, or OS/provider qualification were run per the deferred verification
sequence. The detailed pause/resume contract and test checklist is F104 in `docs/FLOWS.md`.

## Parallel product implementation batch (2026-10-08)

Owner explicitly authorized parallel coding agents to accelerate the desktop/local product.
Three non-overlapping streams were started in this batch:

- Durable Task-to-agent planning/session integration. It must honor the contract gate that
  forbids starting a native agent against user files until Task-specific Environment
  isolation, effective native-capability control, and recovery are enforceable. Safe
  switching/delegation shares the Task/Attempt/lease lifecycle and stays coupled to this
  stream until the durable admission primitives exist.
- Bounded ZIP intake/extraction parser in a separate source module, not a supervised
  isolated process. This is not upload extraction, indexing, RAG, or local-model
  integration.
- Artifact read/Workbench foundation against existing Artifact contracts. No fake data or
  invented write/restore API is allowed; the persistent Artifact publishing path remains a
  separate integration gate.

The local tree currently contains unverified source changes attempting to reopen and
recheck a saved folder identity during PAUSED-to-ACTIVE resume and commit the location/root
updates atomically. This supersedes the earlier statement above that resume only used
startup-era database bindings, but it is not yet reviewed or contract-synchronized. Treat
the folder-resume path as unverified and not production-safe until review, tests, database
transaction checks, and OS qualification are complete.

No tests, builds, formatters, validators, SQL migration execution, or provider sessions
are being run in this batch; the owner deferred verification. No changes are staged or
committed. Reconcile each agent's changed-file report, review for cross-boundary issues,
then schedule the deferred CODE/SYSTEM/USER verification before claiming any story complete.

## Parallel batch status update (2026-10-08)

The slices have moved forward:

- Task planning now has bounded, revision-pinned model input and strict structured-plan
  validation. A second slice is defining typed Attempt/ExecutionLease admission and
  fail-closed renewal/release decisions. No SQLite lifecycle or actual Task execution is
  integrated yet.
- ZIP intake now parses bounded central-directory metadata, verifies local-header and
  `ZipInfo` agreement, rejects embedded-NUL/ambiguous names, and hashes raw name bytes.
  It remains an isolated provider, not wired to Resource upload or RAG.
- Artifact Library/Workbench reads committed SQLite Artifacts through authenticated
  Operator IPC. The desktop Library mounts the real read-only UI for the selected
  Workspace, and Task details now expose a lazy read-only Outputs panel that opens those
  exact committed versions. Artifact publication/edit/restore remain unimplemented.
- Coworker/Automation definitions now have typed immutable revisions and lifecycle
  decisions in `domain-responsibility`; its Coworker SQLite persistence slice is underway.
  Trigger hosting, Operator routes, desktop Coworker/Automation surfaces, and end-to-end
  Task-origin wiring remain unimplemented.

These are source changes only. Tests/checks/builds remain deferred, so no story is marked
verified or complete. Safe agent switching, local-model/RAG execution, and production
containment are still release blockers.

## Desktop implementation continuation — typed Presentation Runtime foundation (2026-10-08)

Added a UI-only typed Presentation Runtime foundation under
`apps/litecowork-ui/src/presentation/`: bounded validation for untrusted presentation
items, stable source ordering, safe built-in renderers and visible unsupported-item
fallbacks, source/provenance disclosure, responsive/reduced-motion styles, and a pure
stream reducer for ready/snapshot/upsert/resync and transient turn frames. The reducer
requires an initial snapshot before applying deltas, ignores duplicates/out-of-order
frames, clears partial text on sequence gaps, and does not turn transient output into a
durable message.

This is not connected to authenticated daemon streams or the main UI yet. There is no
presentation projector endpoint in the local Runtime, and the frontend has no stream
transport integration; it must not be described as an end-to-end Conversation renderer.
Artifact/Task callbacks are navigation-only, Approval/UserRequest cards cannot mutate
domain state, and richer renderers (editable Office files, live browser/terminal control,
MCP Apps) remain unimplemented. All files are source-only; no tests, builds, type checks,
formatters, validators, or OS/provider qualification were run, per the deferred
verification phase.

Parallel execution/store integration and revision-scoped local Resource indexing are
still in progress. Resource indexing is being designed as encrypted derived data using
workspace-scoped OS-protected keys; ZIP/rich-document extraction, semantic embeddings,
and local-model RAG remain explicitly outside that slice.

## Automation persistence increment (2026-10-08)

The SQLite responsibility adapter now has source changes for loading/listing Automation
heads and exact revisions, persisting immutable Automation revisions, journaling typed
Automation events and aggregate snapshots, idempotent replay, same-Workspace Routine and
Coworker revision checks, and status CAS. Coworker archival facts now count ENABLED
Automations and unresolved occurrences rather than treating PAUSED Automations as active.
Source tests cover creation/replay/revision and verify that enabling remains rejected
until the missing trigger reconciliation/materialization service exists. No routes or UI
for Automations are wired yet, and no test/build/validator has been run. This partial
store does not implement Routine persistence, TriggerCoordinator, occurrences, Task
materialization, or scheduler hosting; enabled automation execution is intentionally
unavailable.

## Passive Goal domain boundary — 2026-10-08

Added a domain-only Goal service in `crates/domain-responsibility/src/goals.rs`: typed
Goal/immutable-revision/status values, bounded definition validation, unique same-revision
link validation, expected-version commands, explicit ACTIVE/PAUSED/COMPLETED/ARCHIVED
transitions, authored domain-event drafts, canonical idempotency fingerprinting, and a
transaction port that requires same-Workspace Task/Routine/Coworker reference checks.
Goal mutations cannot start or schedule work. Added source-level unit cases for invalid
links/text, archive-terminal status transitions, reopening, and CAS/overflow behavior;
they have not been run.

The local SQLite adapter and authenticated local Operator routes are now source-wired.
Create/revise/status commands use the GoalService transaction port, compare current
Workspace ownership and write eligibility, validate same-Workspace Task/Routine/Coworker
references, and atomically persist the head, immutable revision and link rows, aggregate
snapshot blob, domain event, and idempotency receipt. Owner reads and stable paging load
the exact immutable current revision. The Tauri Goals surface is being implemented by a
separate slice against these routes.

At this point Goal reads returned `progress: null`; there was no integrated accepted
Task/Evidence projector, so no linked-Task counts or completion claims were synthesized.
This read state is superseded by the partial projection recorded below. Goal status
changes remain explicit owner commands and never create Tasks or start Automations. Source
tests cover persistence/restart, revision immutability, idempotent replay/conflict,
current-owner reauthorization, archived-Workspace read-only behavior, and event snapshots;
they have not been run. No tests, build, formatter, validators, migration checks, or
provider/OS operations were run under the current deferred-verification direction.

## Goal UI completion and read-only Task presentation snapshot — 2026-10-08

The Workspace Goals page and finite authenticated Tauri bridge are now source-integrated
with Goal list/detail/create/revision/status routes. It preserves linked Task/Routine refs
when revising, converts date-time inputs safely, uses optimistic versions and idempotency,
handles pagination/loading/offline/conflict states, and shows progress only when the
server provides a Task/Evidence projection. At this point in the run the server still
returned `progress: null`; Goal status changes remained owner-authored and did not start
work.

The Operator also now has a finite `GET /v1/tasks/{id}/presentation` projection for the
committed Task head, current Steps, and resolvable Artifact versions. A selected Task
loads that snapshot through authenticated local Operator IPC and renders its validated
items through the existing built-in Presentation Runtime. Freshness is `UNKNOWN` because
the sources are read using separate store calls. The source remains read-only: there is no
assistant turn stream, presentation resync cursor, or live activity projection, and the
endpoint cannot create/resolve domain state. The snapshot route and desktop panel are
source-integrated but not tested or built.

No tests, builds, type checks, formatters, validators, migration execution, or provider/OS
qualification were run. Authored tests are not verification evidence. Automation
definition routes/UI and Routine persistence remain in progress; trigger execution is
not integrated.

## Partial Goal progress projection — 2026-10-08

Goal list/get and mutation responses now include a read-only projection loaded from the
authenticated selected Workspace. It reports current linked Task statuses and up to 200
committed Evidence IDs per Task. `INCOMPLETE`, `FAILED`, and `CANCELLED` Task statuses map
to `INCOMPLETE`; every other Task status, including `COMPLETED`, remains `UNVERIFIED`.
The projection is marked `PARTIAL` when verification or source-freshness readers are not
available and returns null for verified/stale/conflicted counts it cannot prove. Typed
limitations are shown in the Goal UI; an Evidence list over the bound is explicitly
truncated. No percentages or Goal completion are inferred, and reads do not mutate Goal
version/status.

The projection also returns Evidence references from each exact Goal-pinned Artifact
version only when each reference resolves to committed Evidence in the selected Workspace.
Unresolved references are omitted with a typed limitation; output is capped at 1,000
Evidence references per projection and truncation is reported. The current implementation
has no integrated VerificationRun read model or Task/Artifact dependency-freshness
projector. These limitations prevent VERIFIED, STALE, and CONFLICTED outcomes from being
established in this source increment. The projection is source-only:
no tests, build, type-check, validator, formatter, or system/owner acceptance was run.

## Desktop Automation definitions surface — 2026-10-08

Added a standalone desktop Automations page and finite authenticated Tauri bridge for
listing, reading, pausing, and permanently disabling Workspace-scoped Automation records.
The UI uses the current Automation head fields, sends idempotency keys and quoted
`If-Match` versions for status mutations, guards stale async responses by selected
Workspace/record, and asks for confirmation before permanent disable. It explicitly says
stored definitions do not run triggers or create Tasks in this build. Create/revise is
intentionally unavailable until a saved Routine list/revision API can supply and validate
real immutable Routine references; no arbitrary Routine IDs are accepted. Resume, manual
run, scheduler execution, trigger hosting, and occurrence materialization remain absent.

The page is exported as `AutomationsPage({ api, workspaceId })` from
`apps/litecowork-ui/src/automations/AutomationsPage.tsx`; Tauri exports
`automation_bridge::automation_request`. The root App/Tauri command registry are left for
the integration owner to mount alongside concurrent UI bridge changes. These are source
changes only; no test, typecheck, build, formatter, or validator was run.

## Automation editor and pinned Routine revision picker — 2026-10-08

The desktop Automation editor now loads Workspace-scoped saved Routine heads and exact
immutable Routine revisions before allowing a save. It creates Automations in the domain's
PAUSED initial state, and revises only after fetching the exact current AutomationRevision
from the descending history page. The editor submits a full definition body, bounds
schedule/one-shot trigger inputs and execution-policy values, preserves the Coworker pin
(the PATCH contract does not accept a new one), and keeps an existing trigger set exactly
unless the owner explicitly chooses to replace it. An exact Routine revision is
revalidated immediately before mutation; the domain/store transaction also validates the
same-Workspace reference.

The local Operator now exposes the already-contracted AutomationRevision history route;
SQLite reads a bounded page in descending order so current revision is available first.
The finite Tauri bridge now supports only Workspace-scoped Automation create/revise,
read/history/safe-stop plus Routine list/detail/revision reads. It still has no enable,
resume, manual-run, trigger-hosting, occurrence, or Task-materialization operation. All
source remains unbuilt and untested; no typecheck, build, formatter, validator, database
execution, or provider test was run.

## Desktop integration continuation — Automation definitions and Task presentation (2026-10-08)

The saved Automation definition router is now mounted under the authenticated local
Operator, and its desktop page/Tauri bridge is mounted in the app shell. The page supports
Workspace-scoped list/detail and versioned pause/permanent-disable. Definition creation and
revision stay unavailable until a real saved Routine selector is available. Trigger
hosting, occurrence materialization, and Automation-to-Task execution are still absent;
`ENABLED` is displayed as stored state, not proof that a trigger is running.

The read-only Task presentation panel now exposes an explicit accessible name for Task
outcome/activity items instead of inheriting the Conversation renderer label. It continues
to display only the finite authenticated snapshot and does not start or mutate Tasks.

This is source integration only. No tests, builds, formatters, validators, SQL migration
execution, provider sessions, or OS qualification were run. The Automation UI/API wiring
and Presentation accessibility label remain unverified. Review for route composition,
request/response shape, Workspace scoping, and source-contract alignment is pending.

## Saved Routine domain and SQLite boundary — 2026-10-08

Added `domain-responsibility::RoutineService` and typed Routine/immutable
RoutineRevision values, with create, revise, and one-way archive commands. The domain
port checks owner scope, optimistic aggregate versions, revision sequencing, definition
shape, and enabled-Automation references before archiving. Revision-owned schemas remain
opaque JSON objects until their owning contracts are integrated. This slice does not
schedule work, run triggers, or create Tasks.

Added `SqliteRoutineStore` using the bounded SQLite writer, Workspace-owner rechecks,
active-Workspace mutation checks, idempotency replay/fingerprint, aggregate snapshots,
domain events, and immutable revision rows. SQLite v8 adds revision append/update/delete
guards and ACTIVE-to-ARCHIVED-only Routine head protection; older migration SQL remains
unchanged. Routine reads and stable pagination are available at the storage boundary, but
no Operator/API or Automation route has been connected to it yet. Automation creation
still needs a same-Workspace Routine read port that pins an exact active revision.

Authored source tests cover Routine create/revise/archive persistence, revision reads,
idempotent create replay, Workspace scoping, and SQL revision immutability. They have not
been run. No build, formatter, type check, migration execution, or validator was run; the
new Routine slice and migration remain unverified. No git commit was created.

## Routine Operator routes — 2026-10-08

Added the separate `apps/litecoworkd/src/routine_operator.rs` route module, not mounted
in `operator.rs`. It implements the documented owner-authenticated Workspace list/create,
get, paged current-and-historical revision listing, revise, and archive endpoints with
idempotency keys, aggregate-version checks, no-store responses, bounded bodies, current
Workspace ownership checks, and active local Runtime Workspace binding checks. Storage
now exposes stable ascending revision pagination for this route. The module does not
expose run/health operations or start trigger execution. Creating a Routine yields its
contractual `ACTIVE` saved-definition status; Automations remain separate and are created
`PAUSED` by the existing domain service.

Root still needs to add `mod routine_operator;` and merge `routine_operator::routes()` into
the Operator router in `operator.rs`. No API/OpenAPI contract change was needed. No tests,
build, formatter, validators, or migration execution were run; these routes are source-only
and unverified.

## Desktop feature integration continuation — responsibility and capability catalog (2026-10-08)

Source integration has advanced beyond the preceding entries:

- Authenticated local Operator now mounts Routine storage routes for Workspace-scoped list,
  detail, immutable revision history/create, revise, and archive. This is saved definition
  management only; there is no `/run`, Task materialization, or trigger scheduler.
- Authenticated local Operator now mounts Automation definition list/detail/create/revise
  and pause/permanent-disable routes. Creation remains PAUSED. The desktop page and finite
  Tauri bridge are mounted; a follow-on slice is adding create/revise UX with a verified
  Routine revision picker. ENABLED records remain inert because trigger hosting and
  occurrence-to-Task execution are not implemented.
- ZIP intake has an owner-scoped read-only readiness route, Tauri command, and Library
  notice. It reports `UNAVAILABLE / ISOLATED_WORKER_NOT_QUALIFIED`; ZIP bytes still enter
  storage intact and never reach the in-process Python parser.
- The isolated agent catalog panel is mounted in Settings in place of the prior duplicate
  agent-setup block. It separates local installation inventory, expiring adapter profiles,
  Workspace bindings, lead eligibility, and default lead selection. It does not expose
  unsupported session model controls, start Tasks, or claim switching/delegation support.
- Automation API construction is memoized by selected Workspace so a new client object does
  not retrigger the Automation page's list effect on every parent render. Task presentation
  items now have a Task-specific accessible name.

Routine desktop editing and Automation create/revise UX are still in progress. No tests,
builds, type checks, formatters, validators, migrations, or provider/OS qualification were
run. These are source changes and remain unverified. Production-safe Task execution,
process/Environment containment, Trust-mediated capabilities, Effect reconciliation,
semantic RAG/local inference, browser/computer execution, and scheduler execution remain
incomplete; this update does not mark the overall product or these stories complete.

## Desktop Routine and adapter continuation — 2026-10-08

The saved Routine definition editor and narrow desktop bridge have now been mounted in
the Tauri shell. The page supports list/detail, immutable revision history, create/revise,
and archive. The composer is versioned and retry-stable. Routine definitions remain inert:
there is no manual run, scheduler, occurrence materialization, or Task creation path.
The Tauri bridge exposes only the authenticated local Operator route.

An explicit OpenCode profile/readiness probe is being implemented separately from
OpenCode task/session execution. It is constrained to bounded read-only native Server
observations; no prompt, tool, Task, Attempt, Environment write, or Effect dispatch is
authorized by this probe.

No tests, builds, type checks, formatters, validators, migrations, provider sessions, or
OS qualification have been run during this implementation pass, per the owner's
instruction to defer verification. Source changes remain unverified. Operational Task
execution and safe switching still require admission, isolation, process containment,
Trust-mediated capability execution, Effect settlement/reconciliation, and platform
qualification; no implementation slice here establishes those guarantees.

## Effect/Evidence persistence foundation — 2026-10-08

Added the `domain-effects` crate with typed Effect states/methods, value validation,
append-only Evidence validation, explicit legal transitions, and a retry path that requires
reconciliation authorization. Added a storage-core port and a SQLite adapter that proposes
Effects only against a live matching Attempt/Invocation/Grant/ExecutionLease and current
Runtime incarnation; it commits the PROPOSED row, immutable snapshot, event, Invocation link,
and idempotency receipt before any possible later provider dispatch. Evidence append checks
Task Workspace and pinned Resource scope and commits its row, snapshot, event, and receipt.
SQLite v9 adds direct-SQL guards for initial PROPOSED state, documented Effect transitions,
and append-only Evidence.

This is persistence groundwork only. No external provider dispatch, Trust policy/Approval
admission, Capability Gateway, Invocation creation, Effect reconciliation, full Effect
recovery, or verification-run producer is integrated. A future dispatcher must call Trust
before dispatch and must not treat the proposal commit as authorization. Verification and
retry evidence still depend on producers not implemented here. No tests, builds, formatting,
validators, migration execution, or provider/OS qualification were run; source changes remain
unverified.

## Routine and Automation contract correction — 2026-10-08

Applied a source-only correction from the read-only review:

- Routine revision editing now disables the name field because the current command contract
  versions definition content but has no rename command. The desktop editor rejects blank
  instruction text, and the OpenAPI Routine revision schemas now express the existing
  domain requirement.
- The responsibility domain now rejects Automation definition revisions unless the
  aggregate is `PAUSED`, returning typed `AUTOMATION_NOT_PAUSED` (409) for enabled
  definitions. The error registry, event-schema mirror, OpenAPI operation response, API
  prose, state-machine contract, and desktop error copy were aligned.
- Automation detail GET now returns `Cache-Control: no-store`, consistent with other
  Workspace-scoped definition reads.

No tests, builds, formatters, validators, or migrations were run per the current owner
instruction. These changes are source-only and unverified; Automation trigger hosting,
occurrence-to-Task execution, and Routine execution remain unavailable.

## Desktop Runtime lifecycle source refinement — 2026-10-08

The Tauri `get_runtime_status` command now runs daemon status and authenticated readiness
probes on Tauri's blocking pool. On-demand startup retains the spawned daemon child handle
until authenticated Operator readiness succeeds or the child exits; when the child exits,
the shell reports failure if no process owns the Runtime lock, or continues readiness
observation if another daemon won the single-instance race. It releases the handle after
readiness so `litecoworkd` keeps its independent lifecycle. A startup readiness timeout intentionally does not kill a
possibly recovering daemon; later status/start calls must observe the process lock and
authenticated handshake again. The source behavior is documented in
[`docs/RUNTIME-LIFECYCLE.md`](../docs/RUNTIME-LIFECYCLE.md).

Deferred qualification: startup/status latency and process-exit races; daemon crash during
readiness; UI responsiveness under a hung status executable/IPC peer; graceful daemon
shutdown and stale endpoint cleanup; packaged sidecar discovery; Linux/macOS native runs;
Windows named-pipe/DACL support; and install/login-service ownership. No tests, builds,
formatters, validators, migrations, providers, or OS integration were run per owner
direction. The daemon remains a storage/Operator foundation with Task admission and real
execution unavailable; this change does not establish production readiness or safe agent
switching.

## Effect/Evidence persistence review closure — 2026-10-08

Source changes close the reviewed Effect/Evidence persistence seams: `effect.proposed.v1`
now requires the initial `dispatch_ordinal: 0`; idempotent Effect/Evidence receipts return
`replayed: true`; `OBSERVED` transitions require exact same-Task Evidence and pin its ID in
the immutable state/event; retry, failure, and ambiguity provenance is retained in event
payloads (including failure retryability); and ResourceRefs are checked against Workspace
ownership and the exact Resource revision. Runtime-authenticated Evidence admission is limited to `REPORTED` records whose
producer matches the authenticated Runtime principal. SQLite v9 rejects higher-assurance
Evidence until independent observer/verifier admission exists. Migration 9 preflights and
fails closed when Effects or Evidence already exist, because prior append-only and producer
assurance cannot be reconstructed safely.

This is a persistence/domain foundation only. Provider dispatch, Trust ApprovalUse + exact
CapabilityInvocation atomic admission, independent OBSERVED/VERIFIED producer assurance,
verification-run admission, Effect reconciliation and production-safe switching remain
unavailable. No tests, builds, formatters, validators, or migrations were run; authored
tests remain unexecuted, so all code is unverified source work.

## Desktop IPC request and readiness bounds — 2026-10-08

The Tauri Operator client now serializes JSON through a bounded writer capped by the IPC
request-body limit instead of building an unbounded serialized `Vec` before frame
validation. Coworker/Goal/Automation JSON preflight uses the same bounded serialization
path; Task creation caps pinned inputs at 256 and workspace instruction revisions cap
merge parents at 256. Added a source unit case for exact-limit and over-limit serialization.

Runtime status now bounds the `litecoworkd status` subprocess/output and authenticated
readiness exchange. Ordinary status has one absolute one-second probe budget; subprocess
and readiness phases each have a 400 ms cap. Startup uses one absolute five-second deadline
across the initial status read and repeated readiness checks. Readiness uses a bounded
caller-side timeout; the IPC frame protocol and peer authorization are unchanged.

No tests, builds, formatters, validators, or OS runs were performed per instruction. These
are unverified source changes. They do not qualify the Unix socket, keyring behavior,
packaged daemon discovery, or Windows named-pipe/DACL support; production-safe Task
execution and agent switching remain unavailable.

## Artifact version comparison presentation — 2026-10-08

The desktop Artifact Workbench now renders both sides of its adjacent-version comparison
through the same bounded safe text renderer. Each pane identifies the exact immutable
Artifact version and its pinned ResourceRevision; the bounded history rows also show
recorded source count, provider, and creating Attempt when available. The comparison is
explicitly read-only and does not claim changed-line detection or publication. It reuses
the existing authenticated exact-version metadata/content routes and adds no API or
storage behavior. No tests, builds, formatters, validators, or provider/OS checks were
run; this source increment remains unverified.

## Task specification history presentation — 2026-10-08

Task detail now has a read-only, lazily loaded history disclosure backed by the existing
authenticated Workspace-scoped `GET /v1/tasks/{id}/spec-revisions` Operator route. The
Tauri bridge validates returned Workspace/Task identity, revision ordering, parent pins,
author identity, and required timestamps. The UI shows exact revision numbers, objectives,
timestamps, author and parent-revision provenance; it has no restore or revision action.
No new daemon route or storage behavior was added. This is unbuilt and unverified source;
the owner-deferred test/build/validation pass remains pending.

## Source review follow-ups for F112/F113 — 2026-10-08

A review found three UI state defects and the source has been adjusted: Artifact selected
version reads and prior-version comparison reads now have separate error boundaries, so a
missing/unauthorized predecessor preserves the selected authorized preview/history, while
selected-version authorization failure clears the selected metadata/content view. Artifact
refresh is disabled during editing, and closing an unpublished changed draft asks for
explicit discard confirmation. TaskSpec history now compares its observed head to the Task
detail revision; it labels a newer record “Latest saved,” discloses an older/incomplete
response, and offers a Task reload rather than incorrectly marking a stale row “Current.”
That reload action is disabled while the Task objective editor has a draft or unresolved
request, preserving that separate edit state. F112/F113 and the E03 backlog description
record these states and their deferred test cases.
No tests, builds, formatters, validators, or provider/OS checks were run; these edits remain
source-only and unverified.

## Desktop Artifact Save As — 2026-10-08

The desktop Workbench now calls the registered `artifact_save_as` Tauri command for the
exact selected managed ArtifactVersion, capped at 10 MiB. Rust and TypeScript agree on the
command name, camel-case request arguments, and a minimal `{status: SAVED|CANCELLED}`
response. The bridge compares the selected ResourceRevision/digest/media type/size against
authenticated exact-version metadata, validates the backing ResourceRevision through the
existing bounded local Operator route, and uses the exact-version content route; SQLite
storage verifies the managed blob digest and size before serving it. Tauri checks returned
media type and byte length and writes through a same-directory temporary file and rename.
The dialog suggestion includes the selected immutable version number. Cancel reads no
content; bytes and destination paths remain native; external and oversized content are
not fetched. F114, Artifact/Workbench experience/design/motion contracts, E08-S07, and this
handoff now describe the actual ordering and integrity boundary.

This is source-only review, not verified desktop behavior. No tests, builds, formatters,
validators, migrations, or platform runs were performed. Native dialog behavior, replacement
semantics, IPC availability, and filesystem failure paths still need owner-run OS checks.
Cloud continuation and remote Runtime remain post-V1; local Task execution and production-
safe switching remain blocked on contained agent sessions, lease admission/fencing, Trust
mediation, Effect reconciliation, and verifier integration.

## V1 scope and next execution gate — 2026-10-08

Cloud continuation and remote Runtime are explicitly post-V1. Work in this run stays on the
desktop/local path.

A focused review of `SqliteStepAttemptStore::admit_step_attempt` confirmed that it should
remain fail-closed for now: the authoritative admission snapshot has no integrated producers
for Environment availability, Task input resolution, Trust/grant and budget admission, Effect
reconciliation, mutation fencing, process containment, or private credential delivery. The
proposed atomic Attempt + ExecutionLease transaction cannot be made reachable without
inventing those proofs. No storage changes were made in this review.

The next implementation dependency is a real local Environment/process-containment provider,
followed by authoritative proof producers and only then atomic Attempt admission. A standalone
`linux-process-scope` source primitive now uses `systemd-run --user --scope` plus a trusted
gate: the parent verifies the gate PID's exact cgroup-v2 membership and kernel
`memory.max`, `cpu.max`, and `pids.max` values before sending GO. The gate confirms GO
consumption before the launcher exposes worker stdin; this does not prove the target `exec`
succeeded. Cleanup failures retain a retryable `PendingCleanup` handle. This is process/resource
ownership only. It does not restrict same-user systemd-bus or filesystem access, so a separate
Environment boundary remains necessary before running untrusted workers. Cgroup quiescence
does not settle external Effects, leases, or provider-side work. macOS and Windows still need
independently qualified providers. ZIP extraction remains unavailable until a supervised
isolated worker and durable manifest/batch-Resource flow exist.

Ruling: accept a Linux-only, launch-time refreshed systemd user unit as the first E01-S03
service-manager increment; do not enable it at login, and do not stop/restart a running
daemon just to refresh `ExecStart`. This is a prerequisite only: it does not make Attempt
admission or containment ready. Package-uninstall cleanup remains a release gate. Cost if
wrong: a stale unenabled user unit may remain after uninstall or a daemon update may take
effect only after its next natural restart; it must never silently terminate active work.

No tests, builds, formatters, validators, migrations, provider runs, or OS qualification were
performed. This entry records source review and sequencing only; no production-safe local
Task execution or agent switching is established.

## Linux local service lifecycle increment — 2026-10-08

The Tauri shell now has a Linux-only systemd user-service path for `litecoworkd`: it writes
or refreshes an owner-only marked unit with escaped absolute executable/data paths,
`Delegate=yes`, `KillMode=control-group`, bounded restart/stop settings, and invokes
`systemctl --user` under the existing startup/status deadlines. Linux startup has no direct
daemon fallback when the user manager is unavailable. It does not enable the service at
login or restart an already-active Runtime merely to apply a refreshed executable path.
Status checks the exact generated unit body, systemd's active/delegation/kill properties,
and the service MainPID executable and arguments against the selected daemon/data paths.
A mismatch marks the displayed Runtime `DEGRADED` while leaving authenticated API
readiness as a separate fact. The install path uses the Tauri configuration directory,
which may follow the user's XDG configuration location rather than literal `~/.config`.

This is daemon lifecycle configuration only. The official systemd delegation guidance says
`Delegate=yes` grants a delegated subtree but does not itself enable controllers; actual
cgroup-v2 control must be inspected and configured by the delegated owner. No Attempt
containment proof, worker-before-exec placement, filesystem Environment, Effect settlement,
lease fencing, or Task admission is provided by this increment. The Linux process-containment
audit found no existing EnvironmentManager/Provider or lifecycle seam to safely wire a
provider yet; a transient systemd scope launch is being investigated as a narrower option.
See [systemd cgroup delegation](https://systemd.io/CGROUP_DELEGATION/).

Package uninstall cleanup, user-manager/path compatibility, timeout and concurrent-start
behavior, active-service update behavior, and Linux OS qualification remain open. The
service is not enabled at login; uninstall must later remove only the marked unit after a
safe Runtime drain and must not terminate active work. Source has not been built or tested
by owner instruction. Cloud continuation and remote Runtime remain post-V1.

## Linux transient process-scope primitive — 2026-10-08

Added the standalone `crates/linux-process-scope` source crate and packaged gate binary.
The gate waits in the transient scope while the parent checks the same PID's cgroup-v2
membership and requested `memory.max`, `cpu.max`, and `pids.max`; only then does the parent
send GO. A bounded GO-consumed marker is removed from stdout before native-agent stdio is
exposed. Failed cleanup returns a must-use `PendingCleanup`: pre-GO cleanup retains the unit,
commands, and child handle; post-GO cleanup also retains the resolved cgroup path and requires
recursive proof. Non-Linux builds expose a matching API that returns `Unsupported`.

This is not wired to AgentHostSupervisor, Environment creation, Attempt/lease admission,
Trust, Effects, Evidence, or verification. It is not a hostile-code sandbox: same-user
systemd-bus and filesystem access can escape the scope. Recursive quiescence can be
unobservable if systemd removes an empty transient cgroup before it is read; the source fails
closed in that case. Linux systemd/cgroup behavior, package delivery of the gate, normal-exit
quiescence, and OS compatibility remain unqualified. No tests, builds, formatters, validators,
migrations, provider runs, or OS checks were performed.

## Owner-triggered local lexical Resource index rebuild — 2026-10-08

The Library now exposes an explicit **Rebuild local text index** action for the exact
Resource revision/digest in the catalog. The Tauri bridge sends a bounded
Workspace-scoped command with a request ID and validates that the response is for the same
Workspace, Resource, revision, digest, and request. It retains the request ID after an
ambiguous error so a user retry can replay the durable result; a stale-head conflict offers
an explicit Library reload. The daemon checks authenticated Workspace ownership, and the
SQLite store checks the durable receipt before source access, verifies the active owner and
exact current revision/digest, prepares from at most 1 MiB of verified source bytes, then
commits projection replacement/removal and the typed `request_dedup` result together.
Shared Resource content-read admission rejects `REVOKED`, `DELETION_PENDING`, and `DELETED`
ContextDocuments before BlobStore access; an already-admitted active read may finish, but
the final transaction rechecks status and will not publish its index after revocation.
Responses expose only `INDEXED` or typed `NOT_INDEXABLE` reasons; they contain no file
content or terms. This is deterministic encrypted lexical indexing only, not semantic
RAG, embedding, ZIP extraction, expanded parsing, or background work. It creates no
Resource/Task history or domain event. The Library keeps the retry RequestId in app memory
across Library navigation, not across a full app restart; after restart a new explicit
rebuild uses a new key but remains pinned to the selected Resource revision and digest.

Source/contracts updated: `crates/storage-core/src/lib.rs`,
`crates/storage-sqlite/src/lib.rs`, `apps/litecoworkd/src/operator.rs`,
`apps/litecowork-ui/src-tauri/src/lib.rs`, `apps/litecowork-ui/src/App.tsx`,
`docs/API.md`, `docs/schemas/operator-api.openapi.yaml`, `docs/STORAGE.md`,
`docs/WORLD-RESOURCES.md`, `docs/DATA-MODEL.md`, `docs/SCHEMAS.md`,
`docs/SERVICES.md`, `docs/EXPERIENCE.md`, and `implementation/epics/E06.md`.
No tests, builds, formatters, validators, migrations, provider runs, or OS checks were
run by instruction. This is source-only work and remains unverified; E06 acceptance is
still open.

## Task Presentation read snapshot — 2026-10-08

The Task Presentation read now collects the Task/current spec, current-plan Steps, Task
Artifacts, and resolvable current ArtifactVersions in one bounded SQLite read transaction.
The response marks only this persisted snapshot `CURRENT`, rejects more than 100 Steps or
200 Task Artifacts without returning a partial view, and omits unresolved ArtifactVersions.
This does not establish live worker state, progress, verification, external/provider
freshness, or event streaming. A focused storage regression test was added, but no tests,
builds, formatters, validators, migrations, provider checks, or OS commands were run, so
the source change remains unverified.

## Desktop/local dependency audit — 2026-10-08

An independent source audit confirms that durable Task-to-agent execution has no safe
adapter-wiring-only increment: planning dispatch is explicitly rejected, Attempt admission
fails closed while Environment/inputs/Trust/budget/Effect/process-containment proofs are
missing, and lease renew/release is not qualified. Codex read-only does not constrain host
filesystem reads; OpenCode/native tools are not mediated; neither transport proves
descendant quiescence. A real planning slice needs an enforced Task-specific filesystem
Environment, a disabled/mediated native capability set, qualified containment/session
settlement, and producer-scoped plan admission. Step execution additionally needs Trust,
Invocation/Effect admission, reconciliation, Evidence/verification, and fenced release.
Do not present the current PoC, probes, or planning-readiness screen as integrated switching.

The ContextDocument purge contracts already define immutable target plans, tombstones,
exact receipts, and completion only after every registered replica is acknowledged, but
the runtime Resource status/purge transaction does not exist. Resource and encrypted
`RESOURCE_INDEX` blobs are written before their SQLite reference commit, and content-addressed
objects may be shared. Existing temporary-upload GC reservations do not fence those writes;
purge must first coordinate writer reservations/fences across Resource creation, revisions,
index rebuilds and recovery, then safely remove only unreferenced objects. No purge-side
partial implementation was made. These are source-audit findings; no tests/builds/validators
or provider/OS checks were run.

## Desktop Library Resource Save As — 2026-10-08

Added a bounded native **Save original…** action for current local managed file Resources
up to the existing 10 MiB Operator response cap. The Tauri command re-reads Workspace-
scoped Resource detail and revision history, pins the current head plus exact digest/size/
media type, requires the managed local-upload provider, and refuses non-active
ContextDocuments before presenting a save dialog. After owner destination selection, it
requests only the exact pinned current revision, relies on the daemon's authorization,
status/head and blob-integrity checks, validates response media type and size, and uses the
existing same-directory atomic writer. The WebView receives only `SAVED`/`CANCELLED`; file
bytes and destination paths stay native. ZIPs remain opaque and this flow copies original
bytes without extraction. No new API route, background work, generic file write, or domain
mutation was added.

Source/contracts updated: `apps/litecowork-ui/src-tauri/src/lib.rs`,
`apps/litecowork-ui/src-tauri/src/artifact_bridge.rs`,
`apps/litecowork-ui/src-tauri/src/resource_save_bridge.rs`,
`apps/litecowork-ui/src/App.tsx`, `docs/API.md`, `docs/EXPERIENCE.md`, and `docs/FLOWS.md`.
No tests, builds, typechecks, formatters, validators, migrations, providers, or OS checks
were run by instruction. The source slice is unverified.
