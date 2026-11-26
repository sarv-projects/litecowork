# Research - Safe Task Path

**Feature:** 005 | **Checked:** 2026-11-06; source crosswalk re-checked 2026-11-12; B-18 filesystem primitives and Task proposal handoff cross-checked on 2026-11-18. The older pinned rival-source review remains dated 2026-11-06.

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

## Sources

Authorities: docs/TASK-RUNTIME.md, docs/ENVIRONMENTS.md, docs/TRUST.md, docs/DELEGATION.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md §6. Reject worktree-as-sandbox. The original source/route/UI crosswalk below is historical; current provider behavior is refreshed in the 2026-11-12 note below.

## Findings

- The owner documents already decide the trust boundaries this slice must keep.
- Hosted competitors often collapse connection, permission and execution into one toggle. LiteCowork forbids that collapse.
- Reference desktop repositories are architecture comparisons, not code to vendor. Pins and paths: [REFERENCE-IMPLEMENTATIONS.md](../../docs/REFERENCE-IMPLEMENTATIONS.md), [REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md](../../docs/REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md).

## Borrowable-design sources (pinned)

| Source | Exact pinned URL | License | What informs the safe task path | Verdict |
|---|---|---|---|---|
| OpenWork `packages/automations` engine/ports doctrine (`engine.ts`, `ports.ts`, `contracts.ts`) | https://github.com/different-ai/openwork/blob/5493b18a6af64d56454658a53da00eabbc6e033d/packages/automations/src/engine.ts | MIT except `/ee` (Enterprise Edition License) | Receipts/events/results stay readable after the admitting process stops. This supports the restart-reload-receipt and independent-verification requirement (AC-007) and the reconcile-before-retry doctrine (AC-006). | Prototype the durable-readable engine/ports pattern against TASK-RUNTIME/FAILURE-RECOVERY; do not vendor the runtime or import `/ee` paths. |
| OpenCowork agent-runner loop guard (`src/main/agent/agent-runner-loop-guard.ts`) | https://github.com/opencoworkai/open-cowork/blob/e94ee2c50239e64ba0e18921e339d49b93dae31f/src/main/agent/agent-runner-loop-guard.ts | MIT at checked root LICENSE | Non-progress/loop guarding informs bounded Attempt admission and the refusal to retry blindly on an ambiguous result. | Prototype the guard logic; LiteCowork keeps TRUST/TASK-RUNTIME as the admission authority, not the loop guard. |
| OpenCowork path guard + VM bridges (`src/main/sandbox/path-guard.ts`, `lima-bridge.ts`, `wsl-bridge.ts`) | https://github.com/opencoworkai/open-cowork/blob/e94ee2c50239e64ba0e18921e339d49b93dae31f/src/main/sandbox/path-guard.ts | MIT at checked root LICENSE | Path-traversal/zip-slip guarding and VM isolation map directly to the ENFORCED filesystem-root dimension of `IsolationAttestation` and the confined-path write (AC-008/AC-009). | Prototype the path-guard checks against ENVIRONMENTS attestation; do not adopt the Electron sandbox as a second authority. |

No OpenWork or OpenCowork code is imported; all three are prototype/reference inputs that reinforce the isolation-attestation and reconcile-before-retry contracts this slice already requires.

### B-18 filesystem API cross-check (2026-11-18)

| Primary source | Exact URL | Finding used for the design |
|---|---|---|
| Linux `openat2(2)` manual | https://man7.org/linux/man-pages/man2/openat2.2.html | `RESOLVE_BENEATH`, `RESOLVE_NO_SYMLINKS`, `RESOLVE_NO_MAGICLINKS` and `RESOLVE_NO_XDEV` provide handle-relative resolution constraints; unsupported flags or kernels must fail closed rather than fall back to path canonicalization. |
| rustix 1.1 `openat2` | https://docs.rs/rustix/1.1.5/rustix/fs/fn.openat2.html | The workspace already pins rustix 1.1.5; the shared low-level crate can depend directly on the existing version for the Linux-only descriptor operations. |
| Microsoft `NtSetInformationFile` and `FILE_RENAME_INFORMATION` | https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntsetinformationfile ; https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_rename_information | The implementation uses `FileRenameInformationEx` with `FILE_RENAME_REPLACE_IF_EXISTS`, a verified parent directory in `RootDirectory`, and a relative leaf. A native Windows regression verifies replacement stays rooted and preserves the expected content/security metadata. |
| Microsoft `SetFileInformationByHandle` | https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle | Still used to copy supported file basic attributes; it is not the replacement rename path. |
| Microsoft `ReplaceFileW` | https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew | `ReplaceFileW` preserves several target attributes and ACLs, but it is path-based and `REPLACEFILE_WRITE_THROUGH` is documented as unsupported; it is therefore not a safe replacement for the handle-relative boundary. |
| Microsoft `MoveFileExW` | https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw | `MOVEFILE_WRITE_THROUGH` applies to the path-based operation; existing helper cannot be used as-is when the caller must remain bound to a held root/parent handle. |
| Microsoft `CreateFile` write-through flags | https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea?redirectedfrom=MSDN | The Windows temp is opened through `NtCreateFile` with `FILE_WRITE_THROUGH`, its contents are flushed with `sync_all`, and replacement is allowed only on NTFS. Microsoft documents write-through as requesting that metadata changes (including rename changes) be flushed on NTFS; unsupported volumes fail closed. This is a platform contract relied on by the implementation, not native Windows Task qualification. |
| Microsoft `FlushFileBuffers` | https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers | Flush requires a writable handle. The staged file is flushed before rename; Linux additionally fsyncs the parent directory. Windows relies on NTFS write-through semantics for rename metadata rather than claiming a portable directory-handle flush sequence. |

The selected direction is to retain the root capability, walk/open children without following
reparse points, preserve only explicitly supported metadata, and perform replacement through
the held parent directory. Linux copies UID/GID, mode, POSIX access ACL and `user.*` xattrs,
but rejects unknown/security xattr namespaces; the ACL copy branch remains unverified because
available WSL filesystems returned `ENOTSUP` even for a disposable ext4 loop mount. Windows
copies owner/group/DACL, DACL protection state and a closed set of DOS attributes, and fails
closed outside NTFS. A pre-rename process-crash test terminates a child after the temp is
flushed. Both paths recheck the base digest before rename, but this is not a filesystem CAS:
external editors/native tools do not participate in LiteCowork's lease fence and can race the
final rename. Ambiguous outcomes must be reconciled before retry. Windows Task provider
qualification remains blocked by B-35 and does not follow from these primitive tests; neither
platform has a served/integrated Task dispatch journey yet.

## Source, route and UI audit (2026-11-06)

Inspected `apps/litecoworkd/src/operator.rs`, `agents/planning.rs`, `task_lead_handoff_operator.rs`, `environment_provider.rs`, `environment_isolation.rs`, `attempt_process_scope.rs`, `trust_operator.rs`, `presentation_operator.rs`, `verification_runs_operator.rs`, `crates/domain-task/src/{lib.rs,planning.rs,execution.rs}`, `crates/storage-core/src/execution.rs`, `crates/storage-sqlite/src/execution.rs`, the composed route inventory, `apps/litecowork-ui/src/App.tsx`, `presentation/TaskPresentationPanel.tsx`, `artifacts/TaskArtifactOutputs.tsx`, `tasks/TaskSpecRevisionHistory.tsx`, `needs-you/TaskAttentionPage.tsx`, and `routines/RoutinesPage.tsx`.

### Served Task operations

- The composed authenticated Operator serves Task create/list/get, Task specification revision create/list, planning-readiness GET, plan-revision list GET, Step list GET and Conversation Task list GET.
- Read surfaces are also served for Task artifacts, capability invocations, presentation, progress and verification runs. These read committed records/projections; they do not admit execution.
- Suggestion acceptance can materialize a Task; the Task lead-agent owner handoff route commits a lead change. Neither action creates an Attempt or dispatches an agent.
- Task create is deliberately limited to a durable Task envelope and initial TaskSpecRevision. It uses an idempotency receipt, owner/Workspace checks and preferred lead resolution. The implementation comment explicitly says it never starts a native session or marks the Task as working.
- The Home “New task” composer creates a Task from a standalone objective, selected Coworker/default lead and pinned Resource revisions. The user can also materialize Tasks from Suggestions or Routine Manual Run. No explicit Conversation-to-Task promotion call site was found in `ConversationsPage`.

### Missing Task execution operations

- `POST /tasks/{taskId}/plan-revisions` (`submitTaskPlan`) is target-only. Only list plan revisions is served; no Operator plan-acceptance handler or UI acceptance action exists.
- Attempt list/admission, Effects list/dispatch, step recovery, Task pause/resume/cancel/steer, and Environment create/provision/suspend/resume/destroy/control operations are target-only in `served-route-inventory.json`.
- `crates/domain-task` and SQLite contain Attempt/ExecutionLease/PlanRevision admission types and transactional storage logic with tests, but repository search found no production caller of `admit_step_attempt` in `apps/litecoworkd/src`.
- `agents/planning.rs` is a read-only preflight builder. It prepares a bounded planning packet and returns dispatch blockers for unavailable isolation, unmediated native capabilities, unqualified protocol/process containment, unavailable planning context and missing session settlement. It sets `dispatch_available=false`. Its `require_dispatch()` method is explicitly unused/dead-code and always returns an error stating native Task planning dispatch is unavailable.
- `get_task_planning_readiness` is a no-side-effect diagnostic: it validates owner/Workspace and `If-Match`, calls the preflight builder, and hardcodes `dispatch_available`, `planning_started`, `agent_session_started` and `plan_created` to false. The endpoint does not start a provider or create plan/Attempt records.

### Environment, isolation, Trust and Effects (historical source snapshot, 2026-11-06)

- `probe_environment_provider()` reports provider capability observations only. On Windows it reports a LocalWorkspace class and Job Object process-tree lifetime support; on Linux it reports a LocalWorkspace class only if bubblewrap is present and explicitly notes isolation is not qualified. The probe reports `budget_enforcement=Unavailable`, no checkpoint/restore and no workspace-persistent lifetime.
- `IsolationAttestation` and fail-closed comparison code model filesystem, process-tree, network and resource-limit enforcement, along with exact Workspace/Task/Runtime incarnation and root/input digests. Repository search found no production caller connecting these attestations to Task Attempt admission.
- `attempt_process_scope.rs` declares its process containment/journal staged until production admission wires exact Runtime scope evidence (`#![allow(dead_code)]`). Its tests correctly refuse to treat parent-process exit as quiescence. This is not a live Task execution scope.
- A Windows Job Object can support process-tree lifetime/termination/resource controls, but it does not by itself attest Workspace filesystem confinement, network denial or every requested resource limit. Those must be measured and enforced by the selected Environment provider, not inferred from the Job Object badge.
- `POST /capability-grants` and revoke are served, but the create route is narrowly limited to bounded, expiring Task-planning `workspace.resource.read` grants over explicit Resource refs. This is not an execution-write grant. No Effect dispatcher/reconciler is connected to an Attempt path.

### Desktop UI crosswalk

- Home is explicitly labelled “New task”. The composer saves the objective as a Task, pins selected Resource revisions and optional Coworker/lead defaults, preserves the same request identity on retry, and navigates to Work after the Task envelope is committed.
- Work lists and filters saved Tasks, paginates, opens Task details, edits objective/specification revisions and checks read-only planning readiness. It displays pinned input Resources, read-only persisted Steps, Task presentation/progress snapshots, Task artifacts and output previews.
- `TaskPresentationPanel` loads the persisted Task presentation/progress projections and a Task stream. It labels its section “Latest saved progress”; it does not invent percentage/ETA. The panel itself is not an execution service and can only reflect records that an owning service has committed.
- Task detail explicitly says the readiness check is read-only and does not start planning or an agent. The planning readiness response states planning dispatch is unavailable. No plan acceptance, Attempt start, Effect approval, Task cancel/recover, or Environment controls are rendered.
- Task output views are read-only committed Artifact projections. The UI does not show a successful file write merely because a Task record exists. A Task status badge or READY record must not be presented as proof of a running worker.

These findings are source/route/UI inspection, not an end-to-end Task USER test. No Task plan was accepted, no Attempt was admitted, no Environment was provisioned, no file was modified by a Task provider, and no Effect was independently verified.

### Current source correction (2026-11-12)

The original crosswalk above is retained as dated research history and must not be used as current provider status. `apps/litecoworkd/src/environment_provider.rs` now reports no Windows `LocalWorkspace` class or Task lifetime; planning readiness returns `WINDOWS_ENVIRONMENT_PROVIDER_UNAVAILABLE`. The Windows Job Object code manages process-tree lifetime and is not an Environment or filesystem/network boundary. On Linux, the daemon uses `linux-process-scope::probe_local_provider`; a trusted Bubblewrap candidate enables only a capability observation, not provisioning, attestation or Task execution. Current route/UI/source evidence is recorded in `specs/005-safe-task-path/SPEC.md` §4/§7b, `TASKS.md` T02/T14 and `docs/IMPLEMENTATION-STATUS.md`.

## Alternatives

- Wait for full multi-provider parity before any slice. Rejected: the release contract asks for one qualified flow per area.
- Generate a complete schema inside this package. Rejected: machine contracts live in `docs/schemas/` and the owner doc.

## Limitations

- Rival web pages were not re-fetched on 2026-11-06 for this package; use [COMPETITIVE-RESEARCH.md](../../docs/COMPETITIVE-RESEARCH.md) only for a future market-positioning decision.
- No end-to-end Task execution was run: no plan acceptance, Attempt admission, Environment provisioning, Task-provider file modification or independent Effect verification occurred.

## Windows provider research input (B-35, PROPOSED)

- AppContainer gives per-profile filesystem and object-namespace confinement keyed by a package SID; resources must be explicitly ACL-granted to that SID.
- A restricted token removes privileges and sets user and administrator groups deny-only.
- A Job Object gives process-tree lifetime (KILL_ON_JOB_CLOSE), active-process, memory and CPU limits. It confines nothing on the filesystem or network by itself (B-09 caveat).
- Windows Filtering Platform filters can block outbound traffic for an AppContainer package SID.
- Only the combination, with each dimension observed after launch, can satisfy the four-dimension attestation. Exact Windows builds, required privileges for WFP filter install and cleanup after crash are open items for the owner decision task.

## Task candidate handoff source check (2026-11-18)

- Local source: `apps/litecoworkd/src/agents/adapter.rs` defines the ACP Client `fs_read_text_file` and `fs_write_text_file` callbacks; the write callback is currently documented as a Client-mediated filesystem write. `apps/litecoworkd/src/agents/acp_client.rs` advertises those Client filesystem capabilities to an ACP agent. Neither callback is bound to a Task Attempt-private Environment or to the managed Resource lifecycle.
- Local source: `crates/storage-core/src/lib.rs` already defines `Resource.provenance.created_by_attempt` in the canonical `ProvenanceRecord`; the Resource revision and its content digest are immutable. `docs/schemas/domain-event.schema.json` permits that provenance in the existing `resource.created.v1` payload. Therefore a Task proposal can preserve Attempt provenance without inventing a proposal table or changing the Resource event.
- Local source: `crates/storage-sqlite/src/trust.rs` checks the exact managed candidate location/digest/size and requires `resources.provenance_json.created_by_attempt = attempts.attempt_id`; native-Windows T07 focused tests pass 4/4, including absent and foreign Attempt provenance. This is grant-issue CODE with no production caller or file dispatch, as recorded in CURRENT-RUN.md.
- Decision for revision 1.1 implementation: use a Runtime-owned, protocol-neutral capture service after the exact worker process tree is observed QUIESCENT. The provider returns the one accepted-Step output as bounded bytes through a private Attempt/Environment/current-incarnation channel. Runtime computes the digest and creates the Resource; Trust verifies the Resource provenance before grant issue. ACP `fs/write_text_file` is not repurposed, since that would change its current Client-write meaning and could bypass user review.
- Limitation at the time of this source check: the Bubblewrap launch plan kept generated files on a bounded private `/workspace` tmpfs and did not expose a host output directory; no provider output-export channel or daemon Task caller existed then. The T04 source implementation check below records the lower-level channel code added afterward. Candidate capture, approval and file dispatch remain unavailable until a daemon caller and qualified provider use it.

### T04 output-channel design review (2026-11-18)

The post-exit reader idea was rejected after tracing mount lifetime: once the Bubblewrap
process tree exits, its private `/workspace` tmpfs and candidate inode are gone. Keeping a
collector alive in the same cgroup prevents the existing recursive cgroup `populated 0`
proof; binding the host output directory into the worker violates the Environment boundary.
Passing a fresh host file descriptor through the current `systemd-run --scope` → scope gate
→ Bubblewrap launch is not an existing contract, and would introduce a second launch/FD
handoff path that T04 has not qualified.

The selected revision 1.1 implementation proposal uses the current scope stderr pipe as a
Runtime-private framed channel. The packaged scope-gate binary's explicit `--attempt-runner`
mode starts the native harness with
stdin/stdout unchanged and stderr captured separately. The runner acts as a child
subreaper, hides its own `/proc` descriptors from the child, waits for all harness
descendants, reads one fixed candidate from the bounded private tmpfs with no-follow
semantics, and emits one versioned bounded frame. The Runtime can parse the frame while
the runner is alive, but only returns candidate bytes after the independent cgroup-v2
quiescence observation. This retains ordinary file semantics for the harness, avoids
exposing a socket or host path, keeps the candidate within the existing scratch cap, and
keeps the accepted Workspace target out of worker-controlled output.

The design is not yet accepted across Environment/Security/Task Runtime owners. The exact
Bubblewrap build and packaged runner hash remain release evidence. Source checks used:

- Bubblewrap v0.8.0 source/tag (4ab175f): its help includes `--ro-bind`/`--bind` and has
  no `--bind-fd`; the selected channel does not depend on a newer FD-bind option:
  [bubblewrap v0.8.0 source](https://github.com/containers/bubblewrap/blob/v0.8.0/bubblewrap.c).
- Ubuntu 24.04 Noble package metadata currently lists Bubblewrap 0.9.0-1ubuntu0.3; this
  is reference metadata, not the V1 release pin:
  [Ubuntu Noble Bubblewrap source package](https://packages.ubuntu.com/eu/source/noble/bubblewrap).
- Linux man-pages specify that `PR_SET_CHILD_SUBREAPER` reparents orphaned descendants to
  the nearest living subreaper and makes them waitable; this is a collection aid, not a
  substitute for cgroup evidence:
  [PR_SET_CHILD_SUBREAPER(2)](https://man7.org/linux/man-pages/man2/PR_SET_CHILD_SUBREAPER.2const.html).
- Linux man-pages specify that `SO_PEERCRED` returns the peer credentials at connection
  time. This approach was not selected because exposing a socket path to the worker would
  require a more complex origin/permission boundary than the existing stderr pipe:
  [unix(7)](https://man7.org/linux/man-pages/man7/unix.7.html).

The v0.8.0 and Noble references establish only command availability. They do not prove the
target Linux kernel, cgroup-v2, tmpfs, user-namespace, systemd, packaged runner or provider
behavior. Native Linux T04 and packaged Task qualification remain required.

### T04 source implementation check (2026-11-18)

The active code now includes `--cap-drop ALL` in the Bubblewrap command plan, a trusted
`--attempt-runner` mode in the packaged scope gate, descriptor-relative fixed proposal
reading with no-follow/regular-file/single-link/size checks, and the source-defined
versioned `LCOW` frame writer/parser. The format uses a 10-byte header, 1 MiB maximum
candidate, 4 KiB maximum log frame and 64 KiB cumulative log payload. WSL 24.04 CODE
checks passed for the process-scope package (36 library tests and 4 runner tests) and
staging crate (38 tests). The daemon has a private launch-plan mapper with 5 focused tests.
It checks the persisted ready Attempt-private Environment against Task, Attempt,
Runtime/incarnation and Workspace identity; compares exact staged Resource, revision and
content-digest pins; and converts only supported resource limits. A private capture session
connects the plan to `spawn_bubblewrapped`, preserves native harness stdin/stdout, drains
the candidate channel concurrently, and releases parsed bytes only after `ManagedScope`
reports recursive quiescence and launcher reap. Three fake-scope tests cover withholding
and retry, failed exchange, nonzero-exit retry denial, and cancellation. A nonzero exit
drains and discards the frame and remains terminal when the retained session is retried;
only an unknown/quiescence-timeout result may retry collection on the same scope. A separate
opt-in WSL integration test starts the locally built runner as PID 1 in direct Bubblewrap,
blocks child frame forgery, waits for a delayed reparented descendant, and parses one
candidate. It bypasses the daemon helper, `ManagedScope`, and systemd/cgroup supervision.
The helper still has no production Task Runtime caller: no candidate was correlated with
durable Attempt state, and no Resource revision or attestation was created. The frame source
remains private process transport, not a new public API or event. Seven pre-existing
Bubblewrap/systemd checks and the new direct runner test are ignored by default on this WSL
host; only the direct test was explicitly run. Native Linux Bubblewrap/systemd qualification
remains open.

### T04 Bubblewrap `/tmp` scratch mapping correction (2026-11-18)

- The production `prepare_bubblewrap_command` output initially used `--bind /workspace/tmp /tmp` to share the bounded workspace tmpfs. Running that exact command under the available WSL Bubblewrap 0.9.0 reproducibly failed before worker exec with `Can't find source path /workspace/tmp`, because `--bind` resolves its source from the host namespace rather than from the mount tree being assembled.
- The builder now uses Bubblewrap's `--symlink /workspace/tmp /tmp`, which creates the link inside the sandbox. A minimal WSL 24.04 reproduction succeeded, and the opt-in production-builder regression verifies the exact 1 MiB tmpfs readback, writes through both `/workspace/outputs` and `/tmp`, and observes `ENOSPC` against the shared quota.
- This is CODE for the command builder and tmpfs behavior under WSL only. It does not prove the released Bubblewrap version, Environment-config-to-builder mapping, systemd/cgroup supervision, native Linux behavior, attestation, Attempt admission or Task integration. Keep T04 PARTIAL and do not mark an Environment dimension ENFORCED from this probe.

### T04 live tmpfs quota observation and safe exhaustion evidence (2026-11-25)

- **Primary source:** [Linux kernel tmpfs documentation, version 6.1](https://docs.kernel.org/6.1/filesystems/tmpfs.html). It describes tmpfs data as memory-backed and notes swap behavior; this supports the resource-risk analysis, not a claim about LiteCowork's provider correctness. The exact production behavior still needs qualification on the declared Linux row.
- **Verified kernel behavior:** in WSL Ubuntu 24.04, Bubblewrap `--size 12345 --tmpfs /tmp` produced `fstatfs` block size 4096, four blocks and a 16,384-byte effective mount. This confirmed the kernel's page-granularity capacity can round upward from the raw request.
- **USER-DELEGATED DECISION:** interpret the Environment storage value as a hard maximum. Round down to the current kernel page size before passing `--size`; refuse requests below one page. The same effective value is passed to the trusted scope-gate runner, which validates `/workspace` type/capacity via `fstatfs`, writable mount identity for output/home/tmp aliases, a small removable canary in each writable location, and `EROFS` on root and staged-input create attempts before held readiness.
- **Rejected live probe:** allocating the complete production scratch cap just to observe `ENOSPC` could consume the Attempt's memory cgroup allowance or invoke OOM. Keep the full-cap exhaustion assertion out of runtime readiness. Existing direct WSL integration uses a separate 1 MiB disposable tmpfs and confirms `ENOSPC`; the production exact-cap mount is validated by readback. This is a provider-specific technical refinement of T04, not Feature 005 acceptance or Linux qualification.
- **Implemented CODE evidence:** `cargo test -p linux-process-scope --locked` passes 45 library tests and 4 scope-gate tests (3 library live-Bubblewrap tests ignored); the held runner test, child-frame-fooling/descendant test, and disposable 1 MiB `ENOSPC` test each pass 1/1 when explicitly run on WSL Ubuntu 24.04. All direct tests bypass managed systemd/cgroup supervision and are not USER qualification.

### T04 held-scope process identity capture/recheck (2026-11-25)

- Source cross-check found no production Task Runtime caller of the pre-existing `HeldProviderScopeIdentity` journal shape. Rather than duplicating the unrelated native-agent stop/recovery helper, the Linux scope owner now exposes capture/recheck on `ManagedScope` and `HeldEnvironmentScope`.
- Capture pins the host launcher PID with `pidfd_open`, reads boot ID and `/proc` start time, confirms exact unified cgroup-v2 membership, reads the effective UID and executable inode, requires the target cgroup to remain populated, verifies the cgroup directory identity, and reads exact memory/pids/cpu settings. After the mutable reads it checks liveness and the same start time again, rechecks membership, UID, executable and cgroup directory identity/population, and confirms limits still match. The digest binds those values to the unit and cgroup path/inode. Recheck captures again and requires full identity equality; missing or changed evidence fails closed.
- The focused WSL fake-proc/cgroup test covers stable observation, changed start time, cgroup movement and changed resource limits; helper tests reject zombie and reused-start observations, and parser tests cover `)` inside process command names and malformed/truncated records. These are CODE tests, not a live `ManagedScope`/systemd observation.
- No journal adapter, caller, attestation, or transaction fence was added. The primitive observes launcher process and cgroup facts only; Bubblewrap namespace/mount policy, network denial, receipt reconciliation, and startup recovery remain separate work. The existing target four-dimension attestation requirements are unchanged.

### T04 Linux preparation-journal symlink classification (2026-11-25)

- Source trace: the Unix journal reader opens entries with no-follow semantics. A symlink therefore yields `rustix::io::Errno::LOOP` before JSON parsing. The Windows reader already rejects a symlink as `InvalidRecord`; converting Unix `ELOOP` to generic I/O broke the typed fail-closed contract and made platform behavior diverge.
- The Unix open boundary now maps only `ELOOP` to `IsolationStoreError::InvalidRecord`; all other `Errno` values remain `IsolationStoreError::Io`. The diagnostic does not expose file contents or paths.
- WSL Ubuntu 24.04 CODE evidence: `cargo test -p litecoworkd --locked preparation_journal -- --nocapture` passed **7/7**, including symlink scan denial, bounded/canonical entry rejection, durable reopen and cross-process lock serialization. This does not prove a native Linux desktop or a production Task Runtime recovery path.

### T04 held-scope identity journal bridge (2026-11-25)

- The held-scope capture/recheck API and `HeldProviderScopeIdentity` journal fields already had compatible evidence shapes, but no daemon code connected them. The provider adapter now copies the exact held unit name, launcher PID, process start ticks and process-scope digest returned by `HeldEnvironmentScope` into the existing journal transition.
- Before Attempt admission, the paired adapter reloads the journal, requires `ScopeHeld`, checks that the live unit matches the recorded unit, reconstructs the recorded `ProcessScopeIdentity`, and asks the still-owned held scope to recapture and compare its current process/cgroup observation. The caller must place this call immediately before the admission transaction, with no provider or storage side effect in between; no such production caller exists yet.
- Three fake-scope tests cover persistence and successful exact recheck, phase refusal before invoking the scope, and changed process-start identity refusal. These tests validate only the daemon adapter and private journal interaction. They do not demonstrate pidfd/cgroup observation against a live managed scope, close a process exit race with the SQLite transaction, produce an attestation, or establish Task integration or qualification.

## Unresolved questions

- B-35 (Windows Environment provider) is PROPOSED and needs the owner.
- The Linux-row planning harness and version (SPEC §18).
- Whether a broker-only connector Step needs an admitted Environment (SPEC §18, shared with 006).

## DeepSeek Harness proposals (PROPOSED, owner review required)

Research note, not a decision. Source: [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159) at the pinned SHA (MIT, Copyright (c) 2026 DeepSeek). Local read-only clone `repo_take/deepseek-harness` is gitignored and never staged. Upstream paths below were checked to exist at that SHA on 2026-11-10.

Read-depth limits from the study (do not over-trust): `scripts/` about 58% read, tests about 5% read, installer UI text partly unread. See [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 0 and 15 before turning any item into a design.

- **P-38 Login-shell PATH-class capture for agents launched from a Linux desktop entry.** Class **OWNER (open; not ADOPT, ADAPT or REJECT yet)**. Upstream: [src/login-shell-environment.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/login-shell-environment.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Section 14 (row P-38 and the conflict note). Lesson: CONFLICT, stays open: capturing the login-shell environment merges ambient variables over the inherited environment, which conflicts with environment scrubbing (A-58, Section 9 of the desktop study and the Env scrubbing lesson in the plugin study) and with the T07 and T13 environment rules of package 002. Only a PATH-class allowlist could be considered, and only after an owner decision. Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React). No reuse is allowed before the owner decision. Where it applies: T07 and T13 of package 002; package 005 Environment admission. Deferral: Blocked on the owner decision. Then a Linux desktop-entry launch test. **Open conflict with environment scrubbing (A-58); owner decision required.**

Plugin-architecture lessons (see [plugin study](../../docs/DEEPSEEK-HARNESS-PLUGIN-ARCHITECTURE-DEEP-DIVE-2026-11.md) Section 8):
  - Capability seam: Definition, Provider, Consumer (ADR P-20): [docs/capability-seams.md](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/docs/capability-seams.md). Class **ADAPT**. Environments and Effects as consumers of a provider seam. Reuse: pattern only, reimplement; no upstream code imported.
  - Durable run record plus process-group liveness fence (P-27): [packages/boot/plugin-manager/src/run-tree.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/boot/plugin-manager/src/run-tree.ts). Class **ADAPT**. Attempt recovery cross-check. Reuse: pattern only, reimplement; no upstream code imported.
  - Atomic restore on a rejected mutation: [packages/boot/plugin-manager/src/operations.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/boot/plugin-manager/src/operations.ts). Class **ADAPT**. Effect rollback cross-check. Reuse: pattern only, reimplement; no upstream code imported.

