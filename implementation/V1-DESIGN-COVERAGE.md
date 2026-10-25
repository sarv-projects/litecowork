# V1 Implementation-Ready Design Coverage and Qualification Matrix

Status: documentation target / proposed implementations. This is a crosswalk, not proof that code exists. Canonical V1 scope is implementation/SCOPE.md and epics E01–E10/E13; post-V1 cloud/remote are E11/E12. No acceptance gate is DONE merely because detailed design exists.

**ID namespace rule:** This supplemental UI and integration catalog uses `UIV1-001` through `UIV1-158`. Existing `U01` through `U20` in implementation/TESTING.md and implementation/WORKFLOWS.md describe different canonical workflow acceptance scenarios and remain unchanged. Never abbreviate `UIV1-001` as `U01`; trace each supplementary case back to an owning epic and existing canonical scenario before implementation.

## Ownership and traceability

| UI / workflow | Normative LLD | Current code entry | Required target ownership | Epic | Operational proof |
|---|---|---|---|---|---|
| Shell/left rail/Workspace selector | docs/DESKTOP-SHELL-LLD.md | src/App.tsx | scoped navigation/layout guards and persisted prefs | E02/E08 | UIV1-001–UIV1-009 |
| First-run onboarding | UI.md, DESKTOP-SHELL-LLD | src/App.tsx and onboarding components | Workspace + native agent setup | E02/E03/E08 | UIV1-001,UIV1-010 |
| Chat create/history/search | docs/NATIVE-COMPOSER-LLD | conversations/ConversationsPage.tsx | Conversation operator and owner bindings | E03/E08 | UIV1-011–UIV1-015 |
| Native Send/Stop/Retry | docs/NATIVE-COMPOSER-LLD | currently disabled | durable turns/agent sessions, native bridge | E03 | UIV1-016–UIV1-022 |
| Model/effort/mode dependent control | docs/NATIVE-COMPOSER-LLD | agents/AgentCatalogSettings.tsx | AgentControlDescriptor, SessionSpec | E03 | UIV1-023–UIV1-027 |
| Slash/custom/@/attachments | docs/NATIVE-COMPOSER-LLD | no qualified composer control | native adapter and text/token reducer | E03 | UIV1-028–UIV1-036 |
| Agent Registry/install/auth/config | docs/AGENT-CONTROL.md and shell LLD | agents/AgentCatalogSettings.tsx | AgentModuleRegistry, secure auth | E03 | UIV1-037–UIV1-043 |
| Coworkers/roster/recent chat | docs/COWORKERS-TARGET.md and shell LLD | coworkers/CoworkerSettings.tsx | Coworker-owner Conversation API | E08 | UIV1-044–UIV1-048 |
| Left/rich central chat viewer | docs/RENDERING-CONFORMANCE-LLD | presentation/* | event decoder + typed renderers | E08/E03 | UIV1-049–UIV1-057 |
| Right Workbench/split | docs/DESKTOP-SHELL-LLD | ArtifactWorkbench embedded | unified shell/surface host | E08 | UIV1-058–UIV1-064 |
| File/code/diff/office/media | docs/RENDERING-CONFORMANCE-LLD | artifacts/* | qualified artifact viewer/provider | E07/E08 | UIV1-065–UIV1-071 |
| Managed Chrome/profile import | docs/CHROME-BROWSER-LLD | none qualified | browser Environment provider | E07 | UIV1-072–UIV1-081 |
| Existing Chrome/Takeover/Computer | docs/CHROME-BROWSER-LLD | no finished UI | OS-specific control lease / provider | E07 | UIV1-082–UIV1-090 |
| Task planning/steps/attempts | docs/V1-WORKFLOW-EVENT-LLD | task operator/storage foundations | durable task + verified effect admission | E03/E04 | UIV1-091–UIV1-100 |
| Delegation/failover/cost and Settings → Subagents | docs/DELEGATION.md, DATA-MODEL.md, API.md, schemas/operator-api.openapi.yaml, schemas/sqlite-v28.sql, EVENTS.md | native probes only | WorkspaceDelegationSettingsService, Coworker inherit/narrow override, native descriptor, host worker scheduler, Task policy pin | E05 | UIV1-101–UIV1-109 |
| MCP/packages/connection auth | docs/CONNECTORS-WORKFLOWS.md, function catalog | partial capability/runtime | LiteSPM bridge + Trust | E04/E10 | UIV1-110–UIV1-119 |
| Goals/suggestions/memory | docs/LEARNING.md, COWORKERS-TARGET | partial pages | source-pinned context + memory policy | E08 | UIV1-120–UIV1-127 |
| Routines/automations | docs/AUTOMATION.md, V1-WORKFLOW-EVENT-LLD | manual occurrence slice | trigger host and recovery | E09 | UIV1-128–UIV1-139 |
| Notifications/Needs You | docs/EXPERIENCE.md, V1-WORKFLOW-EVENT-LLD | partial inbox | delivery/dedup/quiet preference | E09 | UIV1-140–UIV1-147 |
| Security/Runtime/cleanup | docs/TRUST.md, ENVIRONMENTS.md, RUNTIME-LIFECYCLE.md | partial OS primitives | isolation/IPC/policy/provider proof | E01/E04/E07/E13 | UIV1-148–UIV1-158 |

## User journey acceptance cases (perform CODE + SYSTEM + USER where story demands)

### Shell and first run
- UIV1-001 Cold app startup with no Workspace: clear create Workspace route and absent-agent repair, no fake send.
- UIV1-002 Existing local Workspace with stale persisted window route: restore only authorized destination.
- UIV1-003 Switch Workspace mid-resource read: previous results and drafts never cross owner boundary.
- UIV1-004 Left rail collapse/expand and 50-item roster; active route and focus remain visible.
- UIV1-005 Keyboard navigate entire sidebar including Coworker menus, Needs You and Settings.
- UIV1-006 Resize right Workbench by mouse and keyboard; close/max/restore preserves ratio and chat input.
- UIV1-007 View at 1440/900/600/400 CSS px, 200/400% zoom, light/dark/high contrast and reduced motion.
- UIV1-008 Editing an Artifact then navigate away; Save/Discard/Stay and ambiguous publish lock work.
- UIV1-009 Restart app; layout preference survives, unauthorized object reopens as Start instead of leaking data.
- UIV1-010 Start with installed vs missing vs auth-expired agent; each correct readiness/repair.

### Conversation, model, commands
- UIV1-011 Create ordinary chat without Coworker; select and persist Conversation exactly once.
- UIV1-012 Create Coworker-owned chat with exact owner revision; owner conversation history only.
- UIV1-013 Search many conversations; permission filter and partial/error/empty distinguish.
- UIV1-014 Switch between two Coworkers while draft nonempty; preserve or confirm.
- UIV1-015 Reconnect reading history; no duplicated or reordered committed messages.
- UIV1-016 Send to qualified Codex session; actual native response, not fixture echo.
- UIV1-017 Send to qualified OpenCode; native event sequence and committed output.
- UIV1-018 Double-click Send and timeout after server commit -> one turn, draft not lost.
- UIV1-019 Native stop while tool running -> truthful stopping/unknown external send.
- UIV1-020 Retry accepted failed provider turn with exact safe request identity and authorization.
- UIV1-021 Same conversation switches harness -> clear session provenance; no fake session transfer.
- UIV1-022 Runtime restart mid-stream -> canonical turn state with no typing replay.
- UIV1-023 Select Codex -> native model/effort options; no OpenCode model leak.
- UIV1-024 Select OpenCode -> actual provider/model and mode options, no global guess.
- UIV1-025 Stale descriptor refresh during switch -> old response dropped, controls atomic.
- UIV1-026 Unsupported model/effort value -> invalid preview and blocked send.
- UIV1-027 NEW_SESSION-only option -> effective boundary clearly displayed.
- UIV1-028 OpenCode built-in slash command works with original semantics.
- UIV1-029 OpenCode custom command of same name uses actual native precedence.
- UIV1-030 Slash with args, quotes, IME composition and pasted URL path doesn't corrupt draft.
- UIV1-031 Native @ file mention resolves authorized path/resource per harness.
- UIV1-032 Native @ subagent mention chooses correct native subagent, not host capability.
- UIV1-033 Type unknown slash: literal or clear unsupported, no accidental LiteCowork action.
- UIV1-034 Switch binding with native token: incompatible token remains visible until resolved.
- UIV1-035 Image/folder/resource attachment typed validity and exact pinned revision.
- UIV1-036 Clipboard, paste multiline, accessibility announcement, Enter/Shift+Enter and Ctrl+Enter.

### Registry and coworker
- UIV1-037 Install/repair agent using verified source/version; no native configuration loss.
- UIV1-038 Native browser sign-in canceled/retried; auth state never falsely READY.
- UIV1-039 API-key slot stored only in OS keystore; no value in logs/WebView/runtime event.
- UIV1-040 Missing/expired/unsupported native session option blocked with specific repair.
- UIV1-041 Per-agent install+version+OS parity suite matches direct harness.
- UIV1-042 Switch Workspace default lead without changing active Attempt.
- UIV1-043 Agent module crash/recovery doesn't leave duplicate sessions/unsafe workers.
- UIV1-044 Coworker Quick Create without mandatory connectors and correct default.
- UIV1-045 Coworker paused/archived/new chat/revision changes; existing pinned Tasks preserved.
- UIV1-046 Coworker last-active chat missing API -> unavailable, never unrelated conversation.
- UIV1-047 Worker delegation settings versioning/limits and exact source.
- UIV1-048 Coworker connection assignment removed without global account loss.

### Presentation and Workbench
- UIV1-049 Long Markdown/code/math/table message stays responsive and accessible.
- UIV1-050 Native tool call and tool result statuses distinct from actual Effect verification.
- UIV1-051 Terminal logs sanitize escape sequences and show exit status, no fake active shell.
- UIV1-052 Code patch/diff render rename/binary/large hunks without losing provenance.
- UIV1-053 Optional RichPresentation rejected -> full semantic response still present.
- UIV1-054 Unknown native output event -> safe diagnostic fallback, not disappear/crash.
- UIV1-055 Stream reconnect merges committed+transient without duplication or focus steal.
- UIV1-056 10k row table or huge output pagination doesn't freeze chat.
- UIV1-057 XSS/CSS/script/unsafe hyperlink attempts render inert.
- UIV1-058 Workbench opens on user action, not background Artifact arrival.
- UIV1-059 Open Artifact exact immutable version and stale revision detection.
- UIV1-060 Compare text versions, make draft, publish conflict and retry ambiguous.
- UIV1-061 Open PDF/media with proper qualified viewer or honest fallback.
- UIV1-062 Open an MCP App only through its capability sandbox; open an Interactive Artifact through its inert exact-version preview and explicit Run/Stop only under a qualified isolated Artifact Runtime; deny forged, replayed, wrong-version and unauthorized host requests; Save As is host-confirmed, version-provenanced and never overwrites the app.
- UIV1-063 Open Terminal only when provider/Workspace grant qualified.
- UIV1-064 Inspector shows actual worker/Effect/evidence without inventing score/progress.
- Range UIV1-065–UIV1-071 (summary only; individual definitions below): Media/office providers: valid/invalid XLSX formula, PPTX rendering, PDF font, invalid image, protected document, large CSV, missing linked source.

### Browser and computer
- UIV1-072 Managed Chromium/Chrome isolated persistent profile with manual login and revisits.
- UIV1-073 Existing ordinary Chrome default profile cannot be used via unsupported automation flags.
- UIV1-074 Profile import previews bookmark-only allowlist and requires approval.
- UIV1-075 Broken/duplicate export and interrupted import recover without copying secrets.
- UIV1-076 Existing Chrome attach only on version+user approved supported provider.
- UIV1-077 Existing Chrome revoke/detach leaves user-owned browser intact.
- UIV1-078 Browser tabs, navigation, back/reload, downloads, popups, redirects handled.
- UIV1-079 Workbench browser shows real live or explicitly labeled observation/external mode.
- UIV1-080 Browser frame resize/DPI/scroll/focus mappings verified on each supported OS.
- UIV1-081 Browser context/profile contention and crash recovery preserve private sessions safely.
- UIV1-082 Agent triggers Take control; exactly one input actor after epoch fence.
- UIV1-083 User edits form; Return control captures fresh state; old selector blocked.
- UIV1-084 Agent interrupted after externally submitted form -> ambiguous and reconciled; no double post.
- UIV1-085 Password/MFA/CAPTCHA/payment/browser warnings require legitimate user takeover, no bypass.
- UIV1-086 OS locked/asleep -> browser action blocks and doesn't claim action completed.
- UIV1-087 Browser tab navigates disallowed origin -> reauthorization before next action.
- UIV1-088 Computer provider revoked permission/wrong active window -> no action.
- UIV1-089 Camera/screen sensitive area opt-in is scoped, revocable and redacted.
- UIV1-090 Close browser view versus stop Task versus detach profile are distinct operations.

### Task, delegation, connector, automation, reliability
- Range UIV1-091–UIV1-100 (summary only): Task immutable plan, ready Step, lease conflict, Effect external send, pause, resume, crash/restart, verified outcome, incomplete outcome, cancellation races.
- Range UIV1-101–UIV1-109 (summary only): Workspace default native-only/balanced/cost-aware/host-only strategy; Coworker inherit versus narrower profile/strategy override; unsupported native-delegation controls; native child model default versus exact parent inheritance; host worker profile on a second AgentBinding with independent auth; profile/route/model/OS mismatch; stale or disabled profile; quota UNKNOWN/exhaustion; exact TaskSpec policy revision pin; stop/cancel/handoff without duplicate Effects. Ordinary Send remains native and never starts a host worker; Run as Task/Task-plan acceptance is explicit.
- Range UIV1-110–UIV1-119 (summary only): LiteSPM catalog and inventory freshness; installable vs discovery-only; MCP/Skill/Plugin package type and plugin component preview; per-host/OS/scope compatibility; structural plan and human approval (agent cannot self-approve); exact receipt and UNKNOWN reconciliation; managed vs observed/adopted/drifted ownership and safe removal; install separate from Connection/auth/assignment/Grant/activation/invocation; OAuth cancellation, missing Task-time auth, service errors, stale scopes, provider idempotency limits, hostile package metadata and permission denial.
- Range UIV1-120–UIV1-127 (summary only): Scoped memory candidate, redact, duplicate/conflict, approval, source citation, temporary chat disabled learning, revoke source and pending purge.
- Range UIV1-128–UIV1-139 (summary only): Schedule DST gaps/folds, manual Run now, duplicate trigger, provider event, missed offline slot, overlap, no-change watch, pause new work, Pause all safely, Task follow-up, notification quiet hours, actual event history.
- Range UIV1-140–UIV1-147 (summary only): Needs You approval, expired grant, wrong principal, multi-window dedup, notification suppression, action from notification, reopen exact Task, offline state.
- Range UIV1-148–UIV1-158 (summary only): OS sandbox staged source revisions, immutable inputs, network restrictions, process descendant cleanup, Windows/macOS OS qualification, compromised IPC peer denial, secret redaction, restore backups, worktree cleanup, disk budget, clean install/update/uninstall.

Ranges denote families of distinct tests: implementation must turn each item into a named executable or owner UAT scenario, not mark an entire range as one superficial test. This document is an initial completeness crosswalk; canonical numbered flow and benchmark inventories retain separate authoritative numbering.

## Readiness and no-guessing rule

A design row means SPECIFIED, source code presence means PARTIAL, real integration means IMPLEMENTED, adversarial code/system/owner gates mean QUALIFIED. The current read-only ConversationsPage has disabled Send; native composer cannot be marked complete. The right Workbench is not yet a unified shell. Browser/takeover not qualified. Do not display user-facing marketing claims for unqualified rows.

## Contract freeze / implementation handoff

For every story, owner picks the exact LLD -> code files (V1-FUNCTION-FILE-CATALOG), promotes machine objects in docs/schemas and migrations only when a real implementation is being landed, adds test fixtures and observable receipts, checks all UAT IDs above, and returns a code/system/user gate matrix. Consumer/provider boundaries must be supported by source and real versioned upstream docs; no guessed API.


## Expanded individual cases for grouped UAT ranges

The grouped ranges above are shorthand ONLY. Each identifier below is an independently recorded acceptance outcome, with owner scope and real source evidence.

- UIV1-065: XLSX formula calculation and independent formula-result confirmation on qualified office provider (E07).
- UIV1-066: DOCX write and actual rendered layout/font check (E07).
- UIV1-067: PPTX generation, missing font fallback and rendered slide clipping check (E07).
- UIV1-068: PDF preview/export with pagination and safe embedded links (E07/E08).
- UIV1-069: Invalid/untrusted image or SVG cannot execute script or poison viewer (E08).
- UIV1-070: Truncated CSV/incorrect encoding yields explicit partial/error, never invented rows (E07).
- UIV1-071: Missing/revoked external document resource gives unavailable version, not substituted latest (E08).
- UIV1-091: User Run as Task creates exactly one immutable TaskSpec revision (E03).
- UIV1-092: Planner output rejected if TaskSpec changed before acceptance (E03).
- UIV1-093: Accepted PlanRevision atomically materializes Steps with valid dependencies (E03).
- UIV1-094: Step dispatch cannot start without Task/Runtime/Environment/Trust/Lease checks (E03/E04).
- UIV1-095: Concurrent duplicate Attempt reservation allows only one current execution authority (E03).
- UIV1-096: External Effect response lost: reconcile, never repeat with new request ID (E04).
- UIV1-097: Pause requested during running external Effect remains pending until safe point (E03/E04).
- UIV1-098: Resume after provider crash fences old lease and creates new Attempt if eligible (E03/E07).
- UIV1-099: Completion verdict based on independent accepted Evidence, not only model sentence (E04).
- UIV1-100: Cancel/restart race preserves outcome UNKNOWN where external action may have committed (E03/E04).
- UIV1-101: Agent A delegates accepted Step to qualified Agent B under bounded worker scope (E05).
- UIV1-102: Native harness subagent distinguished from LiteCowork host worker (E05).
- UIV1-103: Worker with missing capability rejected without creating fake Attempt (E05).
- UIV1-104: Running Task experiences quota exhaustion and returns actionable choice (E05).
- UIV1-105: Attempt retry budget and no-progress threshold stop repeated work (E05).
- UIV1-106: Handoff source envelope preserves pinned plan/source/Evidence but not native session secrets (E05).
- UIV1-107: Worker Runtime failure leads to safe recovery with new incarnation and lease (E05).
- UIV1-108: Worker profile changed mid-task does not mutate pinned Attempt identity (E05).
- UIV1-109: Stop criteria distinguish verified completed, incomplete and needs user (E05).
- UIV1-110: Search/discover LiteSPM packages with bounded catalog and honest trust status (E10).
- UIV1-111: Install package version and verify publisher/digest before activation (E10).
- UIV1-112: Connection install and provider auth are separately observable (E04/E10).
- UIV1-113: Connection assignment to one Coworker does not implicitly grant another (E04).
- UIV1-114: User cancels OAuth and no account is marked ready (E04).
- UIV1-115: Task blocked by missing connection resumes only after repaired authorization (E04).
- UIV1-116: Provider write with lost response gets Effect reconciliation before retry (E04).
- UIV1-117: Revoked connector scope during dispatch prevents further tool use (E04).
- UIV1-118: Malicious manifest/prompt-injection cannot silently install/activate extra capabilities (E10).
- UIV1-119: High-cardinality connector list remains searchable without activating everything (E10).
- UIV1-120: Candidate memory extraction excludes disallowed sources and private secrets (E08).
- UIV1-121: Duplicate memory with same provenance is merged/deduped per policy (E08).
- UIV1-122: Conflicting memory is marked unresolved until reviewed, never silently overwritten (E08).
- UIV1-123: Coworker-specific memory defaults private and requires explicit share authority (E08).
- UIV1-124: Context used accurately lists transmitted source pins; unavailable provenance labeled unknown (E08).
- UIV1-125: Temporary chat/disabled learning creates no new learning candidate (E08).
- UIV1-126: Revocation fences future retrieval even while external deletion remains pending (E08).
- UIV1-127: Memory correction creates immutable new revision and retains source history (E08).
- UIV1-128: Schedule preview uses IANA timezone and renders next real occurrences (E09).
- UIV1-129: DST skipped local wall time follows explicit configured gap policy (E09).
- UIV1-130: DST repeated local wall time does not duplicate unapproved occurrence (E09).
- UIV1-131: Run Now records distinct manual occurrence, does not enable schedule (E09).
- UIV1-132: Duplicate provider webhook produces one idempotent occurrence (E09).
- UIV1-133: Provider event without valid subscription/auth is rejected with no Task (E09).
- UIV1-134: Sleeping/offline local host records missed/reconciliation state honestly (E09).
- UIV1-135: Overlap/concurrency policy rejects, queues or merges per selected rule (E09).
- UIV1-136: Monitor/no-change watch does not send empty alerts or infinite model polling (E09).
- UIV1-137: Pause new work blocks trigger admission while existing Task semantics remain separate (E09).
- UIV1-138: Pause all safely surfaces pending running Effects, not instant success (E09).
- UIV1-139: Automation Continue reopens exact Task-linked follow-up chat and source revision (E09).
- UIV1-140: Needs You inbox links to actual unresolved request and correct Workspace (E02/E04).
- UIV1-141: Expired approval cannot be used to authorize new Effect (E04).
- UIV1-142: Wrong principal/Coworker cannot answer another owner’s request (E04).
- UIV1-143: Duplicate notification across windows/channels dedupes by canonical event key (E09).
- UIV1-144: Quiet hours suppress delivery appropriately without losing pending Needs You entry (E09).
- UIV1-145: Notification action revalidates scope/authorization before opening (E09).
- UIV1-146: Completion notification opens exact committed Task/ArtifactVersion, not guessed latest (E09).
- UIV1-147: Offline notification delivery shows pending/unknown, never false sent status (E09).
- UIV1-148: Staged exact pinned Resource inputs cannot change after preparation (E07).
- UIV1-149: Symlink/path traversal cannot escape isolated filesystem/output boundary (E07).
- UIV1-150: Worker network restriction actually enforced on claimed OS (E07).
- UIV1-151: Background descendant writer canceled and positive quiescence observed before cleanup (E07).
- UIV1-152: Windows security provider qualified with real process/ACL tests, not Linux inference (E13).
- UIV1-153: macOS environment/security provider qualified with real system tests (E13).
- UIV1-154: Unauthorized local IPC client rejected even if transport reachable (E01/E13).
- UIV1-155: Secret redaction verified in logs/events/support bundle/native messages (E04/E13).
- UIV1-156: Backup manifest restored across supported schema versions and collision policies (E13).
- UIV1-157: Agent-created worktree/build cleanup shows before/after disk delta and leaves user data intact (E13).
- UIV1-158: Clean install, update, uninstall and interrupted shutdown preserve intended durable Tasks/Artifacts (E13).
