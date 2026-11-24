# Superset Design Package — Line-by-Line Disposition

**Initial review:** 2026-11-23; **implementation/evidence refresh:** 2026-11-24

**Design package:** `litecowork_complete_superset_design_package.md` and
`litecowork-design-package-2026-11-23.zip`

**Source fingerprints:** standalone Markdown SHA-256
`D83A670B5AAA5F856C36A9988632AF64CEE34FB030F54CAC67D6C0D72327202C`; ZIP SHA-256
`C1ACACCF89FDBD78FA5A4C1AAA473008361091DD2DAF3A0F652C421702B8D797`. The standalone file is
byte-identical to the main Markdown member in the ZIP. The ZIP has 15 directory/file entries and
contains the main package, three audit/index files and all nine Track 1–9 files.

**Package source baseline:** commit `54b69d1` / SQLite v35 (as stated in the package)

**Initial source audit:** `bb2c53acc7459d62eac57ae097dec43fe8d37522` on `main`
**Latest checkout recheck:** `6145c2a77144053f5fac3316d82efa5f6718b66d` on `main`, equal to `origin/main` before this documentation/UI slice
**Disposition:** design review and USER-DELEGATED product direction; not formal domain-owner acceptance,
INTEGRATED evidence, native-platform qualification, or release approval.

## 1. Review method and evidence limits

The standalone Markdown document is byte-identical to the ZIP member
`LITECOWORK-COMPLETE-SUPERSET-DESIGN-PACKAGE.md`; it was reviewed once, followed by every
manifest, claim-ledger, OpenBot-study and Track 1–9 file in the ZIP. The package was checked
against the current repository instructions, root architecture, product and release boundary,
master plan, migration allocator, alignment register, owning feature packages, active UI and
daemon source, schema, tests, pinned OpenBot source references, and local `repo_take/` material.

Every package statement is treated as a claim until source, contract or primary reference
evidence supports it. The package's commit, schema counts, source line numbers, task IDs,
migration slots and test totals are historical where they differ from this checkout. The
read-only track audit reports are summarized here; no external repository code has been
copied.

Evidence labels remain the repository vocabulary: **TARGET** is a design; **CODE** is source
or test evidence at a revision; **SERVED** means an authenticated route and owning path are
mounted; **INTEGRATED** requires a real provider and persisted product path; **QUALIFIED**
requires the exact declared OS/build/provider/auth route with CODE, SYSTEM and owner USER
evidence. A design proposal, ACP connection, mock, UI tab or unit test cannot advance those
labels by itself.

## 2. Baseline corrections and audit-claim dispositions

| Claim in supplied summary/package | Current disposition at `bb2c53a` | Correct interpretation |
|---|---|---|
| SQLite v35 / 143 tables | **STALE** | v36 is wired after Feature 002 T08 storage work. The current schema must be described from the live migration chain; the earlier v35 count is retained only as the package's baseline. |
| ZIP parent JSON parser leaks `RecursionError` | **SOURCE-CONFIRMED** | `ZipWorker._run` decodes bounded stdout with `json.loads` and catches `UnicodeDecodeError`, `JSONDecodeError` and `ValueError`, but not `RecursionError`. The existing regression is skipped outside Linux, so native Windows test output cannot establish the Linux failure or its fix. |
| Only OpenCode dispatch is wired | **SOURCE-CONFIRMED** | `AgentTurnSupervisor` owns `AgentHostSupervisor<OpenCodeServer>`. ACP transport code has no production dispatch caller; Codex App Server is exercised by its profile probe. These are adapter foundations, not additional qualified providers. |
| Task/effect execution is unwired | **SOURCE-CONFIRMED, scope nuance** | Planning/readiness reports `dispatch_available: false`; storage has no production Attempt-admission caller through the managed execution path. Treat the Task vertical slice as a release blocker. Do not imply no related lower-level code exists. |
| Two Python contract tests fail | **REPRODUCED** | At the current checkout, the two named tests fail: the Operator stream snapshot fixture lacks the required `turn_status`; the release evidence fixture lacks `auth_mode`. The containing two-module run was 13 passed / 2 failed. |
| Credential submission is “unwired” | **PARTLY CONFIRMED; previous wording too broad** | Credential routes are mounted and validate/authenticate requests, but submission returns `503 DEPENDENCY_UNAVAILABLE`; the v36 receipt and keyring primitives are not called by the production route. The lower-level write helper appears only in tests and does not provide the full replay-safe path. |
| Notification delivery has no sender | **SOURCE-CONFIRMED** | Automation can commit a `PENDING` notification decision and read it back; no delivery claim/send/reconcile owner is wired. PENDING is not delivery. |
| Coworker PAUSED is not enforced by Automation | **PARTLY CONFIRMED; framing too broad** | There is no active trigger-host execution path to qualify. Coworker inactive-state validation exists in related enable logic, but the reported Automation status fence has no production execution path. Recheck it when implementing that owner path. |
| macOS local Operator IPC is missing | **REFUTED** | The local IPC support declaration includes macOS and the active Unix implementation is compiled for macOS. This does not establish packaged macOS qualification, which is outside the declared V1 matrix. |
| Backup KDF has no upper bound | **SOURCE-CONFIRMED; CODE FIX WITH FOCUSED WINDOWS TESTS** | Creation accepts 600,000–1,000,000 iterations; header reads preserve the old 100,000 floor but cap at 1,000,000 before PBKDF2. The focused round-trip verifies/restores a legacy 100,000-iteration archive. The exact-source optimized Rust rerun measured 53 ms at 100,000, 190 ms at 600,000 and 308 ms at 1,000,000 on the current Windows host. Archives above 1,000,000 were previously accepted but are now outside the bounded read policy. T14 full-operation/peak-memory baseline and slowest-row evidence remain open; see A-122 and 017-T19/T14. |
| C-01 v33 deferred foreign-key deadlock | **REFUTED** | Current schema uses nullable `attempts.agent_session_id` with a deferred FK; the package's cited migration claim does not override that contract/source. |
| 638/9/5 UI suite result is current | **STALE / NOT RE-RUN** | That is the last reported full UI result, not a result produced in this review. Keep the latest report dated and red until a full suite is rerun on the pinned toolchain. |
| ZIP suite is green on every OS | **NOT ESTABLISHED** | The initial Windows-host discovery produced 3 failures, 32 errors and 35 skips because POSIX descriptor-relative materialization checks ran without required OS support. ZIP-SUP-02 gated both materialization test classes to Linux; the rerun passed 81 tests, 0 failures and 52 skips. Seventeen extraction/materialization cases and the Linux/Bubblewrap integration cases skip on Windows. This corrects test classification only; it does not implement Windows extraction or establish Linux containment qualification. |

The current migration authority is `docs/LITECOWORK-BUILD-TASKS.md`, not the supplied
v35→v50 manifest. The current chain is v36; v37–v47 already have reservations or provisional
owners. No migration number from the supplied package is carried over. OpenBot was inspected
at the pinned commit named in the package; its license is PolyForm Noncommercial 1.0.0 and no
code was imported.

## 3. Product and architecture decisions

These are repository-level **USER-DELEGATED DIRECTION RECORDED** decisions in this change.
They do not mark a feature package ACCEPTED/LOCKED or waive protocol, security-owner, native
platform, provider-account or release evidence.

1. **One local-first intelligent workspace.** Keep the product chat-first and BYOA-first.
   Conversation, Task, Coworker, AgentSession, Runtime, Environment, CapabilityInvocation,
   Effect, Artifact, Evidence and presentation state remain distinct owners. LiteCowork owns
   the coherent experience, durable Core truth, Trust and recovery; a native harness still
   owns its own reasoning, model catalog, authentication, private transcript and native tools.
2. **Compact navigation and contextual object workspace.** Keep New Chat/Recents, Projects,
   Needs You, Work, Coworkers, Library, Automations, More and Settings within the accepted
   shell contract/review. Do not turn each tool into a permanent left-tab destination. A
   contextual right pane uses one internal object registry for tabs, the `+` chooser and close
   fallback. The registry is not a plugin API. Persist only layout/object references; reopening
   reauthorizes and never auto-starts a provider, process, capture or agent.
3. **Takeover and stop mean different things.** `Take over` serializes through the Computer
   input owner, revokes queued agent input and advances its control epoch before any human
   input is admitted. It leaves unrelated Conversation reasoning and Task work alone. `Stop
   run` requests cancellation through the Conversation or Task owner and settles only after
   observed quiescence. Manual takeover does not create a false Task/Effect receipt.
4. **Computer preview is local and separately authorized.** Keep local observation, disclosure
   of a frame to one exact AgentSession, and computer input as distinct permissions. The local
   preview is not a network screen share; it is excluded from chat, logs and transcripts by
   default. A selected right-pane live preview is a V1 target only after the owner contract,
   capture provider, permission UX, input arbiter, denial and recovery tests exist. Frame rate,
   size, staleness and latency limits require measurement on declared release hardware; the
   package's proposed numeric limits are not adopted as facts.
5. **Terminal stays idle until opened.** A user explicitly opens a terminal in an admitted,
   qualified Environment. No startup shell script runs during discovery. Show provider and
   Environment blockers honestly; do not represent sample output or a terminal tab as a real
   PTY/ConPTY journey.
6. **Native + ACP adapters are complementary.** Build a provider capability matrix keyed by
   exact harness/version, transport, OS, auth route, feature and evidence. Use a native adapter
   where it preserves provider-owned capability and control fidelity; use ACP as a separate
   supported route for features it actually advertises. ACP is not a universal UI schema,
   subagent feed, permission system, model picker or guarantee of resume, steering, forms,
   filesystem and terminal support. Keep OpenCode v2.0.32 as the current first release row;
   newer OpenCode versions and additional harnesses need their own conformance and qualification
   rows. Do not claim “12+ supported” from a generic protocol client.
7. **Do not add the loopback MCP relay by analogy alone.** The pinned OpenBot demonstrates a
   session-scoped stdio-to-Streamable-HTTP relay with loopback binding and bearer authentication.
   Its source and upstream issues establish an implementation pattern and specific lifecycle
   failures, not a universal OpenCode/ACP defect. Add a relay only for a reproduced provider
   need, after matching that provider's supported transport, token custody, port binding,
   cancellation, cleanup, process incarnation and test contracts. Never trust model-supplied
   session identity or correlation.
8. **Trust remains deterministic and owner-bound.** Adopt exact provenance, egress and typed
   custom-rule visibility only through existing Trust/Connection owners. Rules may narrow
   existing authority, never widen a grant, turn DENY into ALLOW or consume approval. Reject
   model-risk-classifier automatic approval in V1. Show execution path separately from evidence:
   mediated/native and reported/observed/committed/verified/ambiguous are independent facts.
9. **Browser profiles and import remain privacy-limited.** Prefer a managed isolated profile
   for automation. Existing Chrome access needs its own boundary; consent UI alone is not an
   OS security boundary. Keep passwords, cookies, authenticated state, history and extensions
   out of import. Adopt only explicitly reviewed non-secret preferences/bookmarks that the
   current Feature 008 contract permits. Never decrypt or transfer a native browser secret.
   Site settings, full-URL display, downloads, browsing-data removal and site-permission review
   belong in Browser Settings under the existing 008 owner contract.
10. **Rich presentation has one declarative source of truth.** Continue the canonical
    RichPresentation contract and registered host components; render validated data, preserve
    the semantic message, and degrade safely. No generated React, HTML, JavaScript, hidden
    second-model pass, unrestricted MCP app or untrusted action execution. The current schema,
    Rust/TypeScript validators, accepted component profile and actual renderer must be reconciled
    together. A schema kind is not implemented merely because it appears in JSON Schema.
11. **Saved Views are in V1, changed inputs are separate.** Keep the user-selected Saved View
    direction in Library, backed by an immutable Artifact version and explicit save/update/open
    behavior. Preserve B-40: values changed only in a view are transient. Do not silently save
    those values. B-46 remembered inputs/input sharing require explicit, separate consent,
    identity, retention, deletion, backup and recovery contracts; current input-snapshot fields
    must be closed enums/bounded declared values, never arbitrary strings or secrets.
12. **Artifacts expand by qualified format capability.** V1 must provide a genuine Markdown
    author/publish path, exact provenance, reopen/compare and a durable export receipt. DOCX,
    XLSX, PPTX and PDF editing/export are a full product target, but each format needs a separate
    engine/license, macro/external-link policy, fidelity statement, deterministic behavior,
    resource limits, recovery and platform qualification. Do not ship a generic “editable Office”
    promise before those journeys exist. Resolve A-81 export/write/receipt ordering first.
13. **Voice and visual work stay in the product roadmap.** Add complete Feature 021 and 022 SDDs
    before implementation. Voice begins as explicit push-to-talk, local/transient transcription
    through the normal composer with a visible recording state and editable text; it cannot
    authorize sensitive actions. Visual annotation anchors to immutable Artifact/render versions
    and is explicitly added to context. Teach-by-Demo requires explicit observation consent,
    reviewable redaction of pixels/DOM/accessibility data and a draft-only action model; it cannot
    silently create a package, grant, Routine or Automation. These are V1 roadmap requirements,
    staged after their V1 dependencies and not a claim of shipped functionality.
14. **Remote Runtime and mobile continuity stay V2.** The current V1 release contract promises
    local desktop. Remote Runtime, cloud continuation, cross-device approval/replay and mobile
    clients require a formal release-boundary change; this package does not silently change it.

## 4. Track-by-track disposition

### Track 1 — Unified chat-first shell

**Adopt:** compact navigation; a contextual object panel; object-aware tabs and `+` chooser;
deterministic close fallback; quiet, progressive details; exact Needs You projection; persistent
separation between Take over and Stop run.

**Correct:** `011-T19` currently owns shell reachability/truthfulness, not the full compact IA or
persisted object layout. `011-T23` is already the verifier. Define new/extended 011 tasks after
recording their exact UI, Tauri, persistence and accessibility paths. Do not rely on the
proposal's generic `/v1/{resources|artifacts|tasks|sessions}/{id}` routes: no generic sessions
route exists. Map each object to its exact owning route, or label the target view unavailable.
The current `/v1/needs-you` route covers approval, user-request and Task-blocker items only.

### Track 2 — Workbench, Computer, Browser and Terminal

**Adopt:** idle-by-default, contextual work surfaces; local live Computer view as a target;
separate observation/disclosure/input permissions; epoch-fenced single input owner; global
Take over distinct from Stop run; managed Browser profile; real version/VCS diffs; explicit
terminal launch only within an admitted Environment; provenance in Activity.

**Correct:** 008 already has T22–T27; its T22 is review, T23 contract promotion, T24 observation,
T25 input/takeover, T26 shell/Canvas/terminal integration and T27 qualification. Do not map
Browser settings or import into T22. Browser UI needs an implementation task after promotion.
The supplied proposal broadens import to history/extensions, which conflicts with the current
non-secret-only T22 boundary; do not adopt that expansion. Migration v42 is not a blank slot:
it already reserves Browser/Terminal session/lease/receipt data; any extra profile/site/import/
computer data needs a reviewed same-owner schema delta and allocator update.

### Track 3 — Provider compatibility

**Adopt:** a capability-oriented adapter boundary and per-route conformance evidence; keep the
native provider UX and auth semantics; add a native provider where it gives reliable control;
use generic ACP only where its advertised capability covers the requested operation.

**Correct:** target Codex App Server, Claude and ACP routes as separate adapters with separate
tests and exact auth/OS/version rows. Existing task IDs collide: 001-T33 is unused but must be
connected to the supervisor owner task; 002-T19 is Package VERIFIED and T17 already defines
ACP foundations. Add/extend provider tasks without making package verification obsolete. Do
not upgrade the release OpenCode pin or add loopback relay until its provider-specific evidence
and resource/security contracts pass.

### Track 4 — Trust, credentials and egress

**Adopt:** explicit credential custody; request-scoped auth recovery; egress/SSRF checks at the
actual broker boundary; UI distinction of native, observed and mediated operations; user-visible
scoped controls and deterministic review rules.

**Reject for V1:** model-classifier-driven `AUTO_REVIEW`, implicit `SCOPED_ALLOW`, automatic
email/3DS/OTP retrieval, broad browser control, or a second Trust engine. The native OS/browser
and agent own their credentials; LiteCowork secrets serve only LiteCowork-owned brokered
Connections. Windows AppContainer/Restricted Token/WFP is a candidate B-35 design, not a
qualified Windows Environment. A Job Object alone is process-tree cleanup, not a filesystem or
network sandbox. Update the existing Feature 005/006 owners; do not create duplicate security
authority.

### Track 5 — RichPresentation

**Adopt:** semantic-first selection, validated registered components, deterministic calculations,
static accessible chart with table fallback, strict formula evaluation, explicit citations and
provenance, and robust per-block Markdown degradation.

**Correct:** the contract is not yet internally executable as one profile: the checked-in schema
contains kinds the active Rust/TypeScript validator/render path does not support. Keep the schema
as TARGET until Feature 016 T01/T02 decide and promote one profile; update the schema, shared Rust
validator, TypeScript parity corpus, renderer and UI contract in one change. Resolve A-85 before
model-visible presentation proposal input. KaTeX and Mermaid are candidates only after strict
input grammar, renderer security, bundle/license, complexity bounds and output sanitization are
specified; they may not render arbitrary HTML/SVG from provider content. Keep Interactive Artifact
Runtime PREVIEW_ONLY and inert.

### Track 6 — Saved Views and local state

**Adopt:** Save an eligible committed presentation explicitly as a non-executable Library
Artifact version; reopen the exact version and preserve the source/fallback. A new response
creates a new version/source lineage rather than mutating the old answer. Keep view-local changes
transient by default.

**Hold separately:** B-46's Remember/Forget and explicit input-sharing flow. It requires its own
closed storage contract, scoped identity, encryption, quotas, backup policy, delete/reset and
crash recovery. Do not add arbitrary input strings to Saved View JSON or implicitly snapshot live
session values. The product direction for Saved Views does not equal Feature 007/016 package
acceptance; existing T25/T01/T02 sequencing remains to be reconciled with this user direction.

### Track 7 — Professional Artifacts

**Adopt:** a truthful format/capability matrix; Markdown authoring first; immutable Artifact
versions and lineage; actual save/reopen/compare; export receipt with idempotency and UNKNOWN
reconciliation; format-specific editability and fidelity copy.

**Design before implementation:** format-specific DOCX/XLSX/PPTX/PDF providers. Their contract
must settle import/export ownership, formula calculation (no silent external workbook
recalculation), macro/external links, unsupported-feature warnings, binary/parser limits,
license/notices, platform behavior and crash recovery. Current Feature 007 T11–T13 already have
compare, import/Task-source and recovery meanings; never reuse their IDs. Resolve A-81 before
success receipts or export authority are implemented.

### Track 8 — Voice, annotation and Teach-by-Demo

**Add as full owner packages first:** Feature 021 Voice and Feature 022 Visual Annotation and
Teach-by-Demo. Write each complete SDD (scope, HLD, LLD, APIs, state, UI, errors, recovery,
privacy/security, dependencies, tests, acceptance and sequence) and research exact local
libraries/platform privileges before creating implementation tasks. The 2026-11-23 decision is
to include these capabilities in the V1 roadmap, after native Conversation, Computer/Browser
permission and Artifact dependencies. Audio is transient unless a separate consented artifact
path is accepted. An annotation is tied to an immutable ArtifactVersion or render digest and
only enters a prompt after a user review. Teach-by-Demo never accepts arbitrary provider text as
an approved action or permission.

### Track 9 — Remote Runtime and mobile

**Keep V2:** remote Runtime registration, cloud continuation, multi-device clients and offline
action replay remain outside current V1. Preserve the design direction in the V2 architecture
without a v1 migration, route, or UI implication. Any move into V1 first changes the product
release contract and the security/replication authority model.

## 5. Open design work and implementation order

The order below adds the supplied capabilities to the product roadmap without mislabeling them
as available. Existing owner task IDs are retained and extended only through the owner packages.

1. **Safety and evidence hygiene:** ZIP-SUP-01 now rejects oversized or deeply nested worker
   responses before JSON parsing and catches parser recursion. ZIP-SUP-02 classifies the
   descriptor-relative materialization tests as Linux-only; portable archive/provider tests still
   run on Windows, and skips do not imply Windows extraction support. A-122 now directs bounded
   backup creation/header work and 017-T19 implements it; T14 still owns the slowest-row,
   full-operation and peak-memory evidence. 019-T25 repairs the two root Python contract
   fixtures. Update schema counts, migration allocation and evidence without importing the stale
   package baseline.
2. **Conversation and providers:** complete OpenCode send/permission/continuation/stop/retry/
   restart/recovery; add conformance-led Codex App Server, ACP and Claude adapters; preserve
   harness-native authentication and exact capability gates; qualify at least the declared
   primary route and required heterogeneous pair.
3. **Task/Trust/Connections:** complete Attempt admission, Environment attestation, mediated
   bounded write, atomic Effect/approval/fence, receipt reconciliation, independent verification,
   cancellation/timeout/restart; finish scoped credential submission and Connection read/write;
   then wire automation/notification sends to real owners.
4. **Context and Workbench:** finish root ingestion/index/retrieval and durable Context Used;
   deliver compact shell and contextual object registry; qualified Computer preview/takeover,
   managed Browser settings/site access/non-secret import, and idle terminal in qualified
   Environment; no imitation or simulated provider success.
5. **Presentation and saved work:** reconcile the RichPresentation profile; publish after
   semantic commit; implement accessible chart/table fallback and deterministic calculator;
   implement explicit Saved View create/open/update/archive in Library; finish Markdown
   authoring/export receipt before broad Office engines; add office formats after per-format
   capability and fidelity evidence.
6. **Voice and visual authoring:** finish Feature 021/022 SDDs, approved local/transient voice
   path, immutable visual annotation, then carefully consented/redacted Teach-by-Demo. These are
   required V1 roadmap scope and cannot be marked shipped before their own gates.
7. **Release:** native Windows x64/OpenCode v2.0.32 first and native Linux x64/OpenCode v2.0.32
   second, packaged E2E, failure/restart/reconnect, usability/performance/accessibility/security
   and exact USER evidence. No current documentation, code test, WSL/container run or competitor
   comparison qualifies a release row.

`docs/IMPLEMENTATION-MASTER-PLAN.md` orders these workstreams. The owning `specs/` packages,
canonical contracts and `docs/LITECOWORK-BUILD-TASKS.md` remain the places where their API,
event, schema, task paths, acceptance and migration behavior are defined.

## 6. Primary and source references checked

- Repository authority: `AGENTS.md`, `ARCHITECTURE.md`, `docs/PRODUCT.md`,
  `docs/V1-PRODUCT-RELEASE-CONTRACT.md`, `docs/IMPLEMENTATION-MASTER-PLAN.md`,
  `docs/IMPLEMENTATION-STATUS.md`, `docs/LITECOWORK-BUILD-TASKS.md`.
- Feature owners: Features 001, 002, 005, 006, 007, 008, 011, 016 and 017, with their
  current TASKS, DATA-MODEL, WIRING, UI/API/error contracts and linked LLDs.
- Primary ACP overview: <https://agentclientprotocol.com/protocol/v1/overview> — optional
  capabilities must be negotiated; this does not define a universal presentation tree.
- OpenCode official ACP guide: <https://opencode.ai/docs/cli/acp> — ACP subprocess launch and
  advertised capability behavior are provider-specific.
- Pinned OpenBot source: `repo_take/t3code` is not the reference package; the supplied OpenBot
  study's pinned external source is `nightly-labs/openbot@5464e5743b411ab076ad36225adcd72e2d61d4fd`.
  Its PolyForm Noncommercial license is a no-code-copy boundary. Its pane registry, relay and
  browser capture-lock details are observations of that version only.
- Chromium CDP reference: <https://chromedevtools.github.io/devtools-protocol/> — local remote
  debugging exposure is a privileged browser-control surface; it is not a safe way to attach to
  an arbitrary user's default profile.
- OpenCode issue #57204 is a concrete session-resume startup race:
  <https://github.com/anomalyco/opencode/issues/57204>. OpenBot issue #9254 and linked #9630
  concern its reported OpenCode MCP-child lifecycle case; neither establishes a universal
  defect across ACP agents.

External references support the stated protocol and source-specific observations only. They do
not establish LiteCowork integration or qualification.

## 7. Latest source and contract recheck — 2026-11-23 client date

The earlier claim table and implementation evidence above remain anchored to their explicitly
named historical checkouts. The design package, Track 1–9 files, owner contracts, current shell,
provider adapters, Workbench source and migration allocator were rechecked against
`6145c2a77144053f5fac3316d82efa5f6718b66d`. The current schema is v36 with 144 application
tables; v37–v47 remain reserved or provisional. The full UI suite's latest result is 658 passed,
9 failed and 5 skipped, exit 1; do not repeat the earlier 638/9/5 number as current. See
`CURRENT-RUN.md` for the exact run environment and failure evidence. The current Task count is
the owner ledger count, not a completion percentage; no task is VERIFIED and no release journey
is QUALIFIED.

### Track-by-track corrections from the current checkout

| Track | Keep | Correct before implementation |
|---|---|---|
| 1 — Chat-first shell | Fewer permanent destinations, unified-workspace intent, one contextual object pane, object registry owned by Workbench, and reauthorization on reopen. | Feature 011 already has T19–T23 and Feature 008 owns object tabs/chooser/Canvas/terminal semantics in T26. `Projects` maps to existing Workspace records for navigation only; no Project entity or second boundary. Recent Chats remains Conversation-only until cross-owner ordering/pagination/partial freshness/recovery exist. A composite Needs You count must not merge persisted items with Runtime-local permission prompts without exact dedupe/snapshot behavior. The generic session route in the package does not exist; use each owner's exact read path. |
| 2 — Workbench and local computer | Real Browser/Computer/Terminal providers; separate local observation, agent disclosure and mediated input; idle terminal; Take over fences new agent input while Stop run cancels the owning run. | Feature 008 tasks T22–T27 already own review, promotion, observation, input/takeover, object tabs and qualification. T22's Browser import boundary is non-secret; history/extensions require an explicit owner amendment. DOM scrubbing or automatic reload is not a privacy boundary; pause capture before sensitive entry and require a fresh authorized frame. Computer Task/Attempt input grants do not yet resolve ordinary Conversation input authority. A shortcut/tray design is not a qualified out-of-app escape; keep input unavailable until it works on the exact OS/provider row. v42 remains reserved; no supplied schema fields are adopted. |
| 3 — Native plus ACP adapters | Capability-specific native and ACP paths with exact version, transport, OS, auth, event and recovery conformance. | `AgentHostSupervisor<T>` owns retained process handles; the existing `AgentAdapter` is the provider boundary. Do not trait-objectify the process supervisor as proposed. Keep OpenCode v2.0.32. Codex App Server is probe-only; ACP v1 and Claude launch entries are foundations with no qualified Conversation dispatch. Claude T12 and same-incarnation recovery remain gated. No fixed resume retry or loopback MCP relay is adopted without a reproduced provider need and token/process lifecycle contract. ACP does not promise component trees, subagent feeds or permissions. |
| 4 — Trust, credentials, egress and execution | Provenance, least authority, bounded Effects and explicit takeover are sound principles. | Keep mediation and outcome evidence as separate axes. Artifact/Delegation actions stay with their owners; a generic Gateway must not absorb them. Model-classifier `AUTO_REVIEW`/automatic approval conflicts with deterministic default-deny Trust and is rejected. Custom rules need Trust-owned revisions and may narrow only existing grants. Windows AppContainer remains blocked by B-35; a proposal is not containment. ZIP recursion and KDF maximum findings are fixed in source at the current checkout; slowest-row performance and native Linux extraction qualification remain open. |
| 5 — RichPresentation | Semantic answer first; one canonical typed schema; bounded registered renderers; deterministic calculations; safe text fallback and explicit provenance. | Current schema kinds exceed parts of the Rust/TypeScript/render pipeline. Do not add a competing v2 schema or tool contract. A-85 and 016-T20 remain blocked pending provider correlation/replay evidence. `DIAGRAM`/`MEDIA` proposed labels conflict with the canonical `DIAGRAM_BLOCK`/`MEDIA_CARD`; KaTeX/Mermaid require grammar, sanitization, complexity, accessibility and license review. SVG is not inherently safe; render only host-validated values. No arbitrary HTML, JavaScript, or hidden second model pass. |
| 6 — Saved Views and input memory | Explicit Saved View in Library with immutable provenance, source fallback, reopen/update/archive; transient calculator input under B-40. | Keep Saved Views in V1 as user-directed but not yet cross-owner accepted. Existing Feature 007 T11–T13 and Feature 016 T22–T25 already own adjacent work; supplied IDs collide. Reject the proposed input snapshot field and keep B-46 Remember/Forget separate until its own scoped retention, deletion, backup and recovery contract is resolved. No Saved View route is served. |
| 7 — Professional file authoring/export | Per-format capability and fidelity disclosure, immutable versions, source lineage and recoverable export are required. | Accepted Feature 007 v1 starts with Markdown and leaves DOCX/XLSX/PPTX/PDF read-only or unsupported until owner amendment. The proposal's `/versions/{v}/exports` conflicts with `/artifacts/{artifact_id}/exports`. A-81 still blocks choosing receipt-before-write versus write-before-receipt; use operation identity, pinned source digest, UNKNOWN after ambiguity and explicit reconciliation. `openpyxl` does not calculate formulas and may lose unsupported parts; `python-docx` support has limitations; HyperFormula licensing needs review. Subprocess isolation alone is not a security boundary. Do not add redundant format/fidelity columns or reuse 007 task IDs/migration v38. |
| 8 — Voice, annotations and Teach-by-Demo | The product owner keeps full Voice and visual authoring capabilities on the V1 roadmap; local/transient capture, exact visual anchors and reviewed proposals are appropriate. | Features 021 and 022 do not exist yet; author complete SDDs before implementation. Voice should create an editable local draft for normal Conversation Send rather than a new durable utterance endpoint; audio is transient by default and voice cannot approve. Annotations bind to immutable ArtifactVersion/render digest and become stale on target change. Teach-by-Demo should produce a reviewed RoutineRevision under Feature 010; do not invent a Skill ownership path or persist raw recordings by default. No v49/v50 migration allocation is valid. |
| 9 — Remote Runtime and mobile | Remote/mobile continuity can be a future extension. | Keep it in V2 per `V1-PRODUCT-RELEASE-CONTRACT.md` and `RUNTIME-MESH.md`. Local Operator IPC is not a remote authenticated API; mobile approval must not queue offline and replay authority later. Do not add V1 schema/routes or claim target flows as served. |

### Implementation decision and immediate slice

The product owner explicitly delegates internal implementation choices in A-121. A-123 records
the compact-navigation mapping: New Chat, Recent Chats, Projects, Needs You, More and footer
Settings; Projects is a directory over the existing Workspace list; secondary domains remain
reachable; no composite Needs You count is shown without exact source/deduplication semantics.
Feature 011 T19 is the owner task and its allowed UI paths cover this work. The implementation
plan now places this shell slice before the next dependency-ready release task. The change adds
no route, domain entity, event, schema or migration. The currently accepted four-destination
V1 contract and Feature 011 package lifecycle remain formally unchanged; the user-directed
implementation does not establish owner acceptance, provider integration or qualification.

**T19 source update:** the compact rail, Workspace-backed Projects component, Recent Chats state
component, local guarded `ConversationNavigationIntent`, and New Chat auto-selection fence are
present in the working tree. Focused shell component/source tests pass 19/19, TypeScript passes,
and the rendered browser group passed all four checks in Chrome 158.0.8127.38 against the
production-built harness. The separate Conversation rendered group passed 15 checks, including
blank-chat draft entry and preservation without issuing a create/send request. The rendered-test
launcher now discovers common Windows Chrome/Edge locations when no explicit executable is set.
The broader focused shell/Conversation/navigation/source suite passes 56/56. These are CODE/UI-render results only; T19
remains PARTIAL, with packaged Tauri, provider integration, formal package acceptance and
qualification still open. No UI action was converted into an API, entity, grant or provider dispatch.
See CURRENT-RUN.md for exact attempts.

The follow-up API check also exposed an unrelated Feature 002-T08 OpenAPI defect: its credential
conflict response referenced nonexistent `components/schemas/Error` instead of the canonical
`ApiError`. The reference is corrected and the generated Operator types now declare exact replay
(`200`), committed write (`201`) and idempotency conflict (`409`). This does not change route
behavior: credential submission still fails closed with `503`, and T08 remains IN PROGRESS.

All other tracks stay in their owning dependency order. In particular, saved answers, Computer
capture/input, additional adapters, browser import, Office authoring, voice and annotations are
not implemented merely by adding this disposition. Their owners must complete any required
contract revision, API/schema/event promotion, UI trace, recovery design and focused evidence
before their implementation tasks begin.

## 8. Live checkout correction and next implementation slice (2026-11-24)

This section supersedes historical statements above that still say Feature 002-T08 credential
submission returns `503`: current source at the live checkout has a mounted authenticated route
backed by Runtime-local v36 receipt reservation/settlement and idempotent SecretStore mutation
(CODE/SERVED). It still does not provide a provider sign-in journey because the current native
module descriptors declare no LiteCowork-owned credential slot; daemon-process restart,
platform keyring and provider bind/send evidence remain open. The current main checkout is
`a2f366c` at the start of the follow-up implementation, and the working tree was clean before
this A-124 documentation update. Recheck HEAD and worktree before further edits.

The Conversation plan's earlier “claim ambiguity” next-step text is stale: A-116 exact
same-incarnation dispatch-claim reconciliation is CODE in T20. The next authorized gap is the
volatile-feed loss after `StopUnconfirmed`. Pinned OpenCode v2.0.32 protocol sources define a
durable session log and replay-to-live watermark; its execution source also confirms that
`shutdown` interruption deliberately retains the native claim. A-124 records the bounded
same-incarnation recovery contract in the Feature 001 SPEC, Recovery contract, RESEARCH, PLAN,
TASKS and WIRING, and the alignment register. It requires exact prompt receipt identity and
forbids prompt replay. This can improve same-process Stop recovery, but cannot replace a live
provider capture, packaged Stop/restart journey, or T09/T23 Windows/Linux qualification.
The next-gap statement in this section was superseded when the A-124 implementation began;
Section 9 records the resulting CODE evidence and remaining integration gates.

The full superset roadmap remains intact: compact chat-first shell and contextual object pane,
Computer preview/takeover, idle Terminal, capability-specific native/ACP adapter families, safe
RichPresentation, V1 Saved Views, professional Artifacts, Voice and Visual Annotation/Teach by
Demo remain in their owner packages; remote Runtime and mobile remain V2. No broad track is
claimed implemented by the selected Conversation slice. The latest full UI suite remains red at
658 passed / 9 failed / 5 skipped; provider/package/platform/release qualification remains open.

## 9. A-124 live code update (2026-11-24)

This update follows the bounded T03/T04 implementation recorded in the current checkout. The OpenCode adapter now validates the prompt receipt's exact user message/session identity and, only after `StopUnconfirmed`, reads the pinned v2.0.32 same-incarnation durable session log for at most five seconds. A bounded reducer checks replay watermark, event identity/order, exact inbox delivery/execution lineage and one terminal event before mapping to the existing outcome path. It excludes deltas, reasoning, tool data and provider error text, never resends, and leaves ambiguous evidence unresolved. No LiteCowork schema, event, route or migration was added.

Focused native-Windows CODE verification passed: `cargo test -j 1 -p litecoworkd --locked agents::opencode_ -- --nocapture` reported 35 passed, 0 failed and 1 intentionally ignored. This is code-level implementation only. The reducer and fake-loopback tests do not prove an authenticated provider event stream, SQLite settlement of the recovered outcome, retained-host retirement, restart/reconnect, packaged desktop behavior or Windows/Linux USER qualification. T03/T04 remain PARTIAL; T09/T23 retain packaged provider qualification. The completed work closes the documentation/design gap for A-124 but does not implement the remaining superset tracks or claim a release-ready product.
