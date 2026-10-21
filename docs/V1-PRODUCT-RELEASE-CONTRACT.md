# LiteCowork V1 — Product and Release Contract

**Status:** accepted V1 TARGET; **not a claim of implementation or release qualification**.  
**Authority:** subordinate to [Architecture](../ARCHITECTURE.md) for Core safety/ownership, and the companion to [Product Contract](PRODUCT.md) for complete desktop scope.  
**Supersedes:** ambiguous Task-first product framing and transitional Home as the *default* UX; no Task identity, safety, evidence, or accepted ADR is revoked.  
**Implementation truth:** [Release scope](../implementation/SCOPE.md), [qualification matrix](../implementation/QUALIFICATION-MATRIX.md), [served-route inventory](../implementation/V1-SERVED-CAPABILITY-INVENTORY.md), actual source, and CODE/SYSTEM/USER evidence.

## 1. Single product promise

LiteCowork is a **local-first, chat-first AI workspace** in which a person works with a selected **native agent harness**, can use their existing agent subscriptions and configurations, can optionally create persistent Coworkers, and can complete real local work with reviewable results. User interaction starts with **New Chat**, not Task/Coworker setup. Tasks are durable execution records *when execution is admitted*, not mandatory wrappers around ordinary questions.

A user may ask and receive a native response without creating a Coworker or a Task. When the user requests durable work, an authorized Task captures accepted intent, revisions, Steps, Attempts, Effects, Evidence, artifacts and recovery. A Coworker is a reusable identity/context/responsibility layer; neither Coworker nor agent owns Core Task truth.

Core local functionality requires **no mandatory LiteCowork subscription or hosted LiteCowork account**. This does not promise that the user's chosen native agent, external app, provider, model or hosted capability is free. Preserve native sign-in and provider billing boundaries. This is a **product requirement**; the licensing/packaging implementation decision must be separately recorded before release.

## 2. Product family: three different roles

| Product | Owns | Must not own |
|---|---|---|
| **LiteCowork** | Desktop workspace; user conversations; Task/Attempt/Effect/Evidence truth; Coworkers; context/memory; assignments and Trust grants; work and artifact history; automations; notifications; Workbench and OS activity admission | Duplicate cross-agent package-manager lifecycle; native agent's private reasoning, account, model-router or subagent internals |
| **LiteSPM** | Cross-agent capability catalog, inventory and provenance; package resolution, imports, installation, adoption, updates, unmanage and uninstall; per-host adapters; package health and lifecycle operation receipts | Coworker and Task decisions; approval to execute a consequential action in LiteCowork; native agent credentials unless the explicitly qualified custody contract permits it |
| **HorizonCode** | Specialized long-horizon coding agent with native session, tools and worker execution features | LiteCowork Core scheduler, authoritative Effect/Trust grants, universal package-manager database or privileged bypass |
| **Other native agents** | Their actual native sessions, authentication, models/options, native features, extensions and internal worker implementations | LiteCowork's durable Task truth or another harness's private configuration |

LiteSPM may **observe** external capabilities without modifying them, **adopt** only eligible existing entries with explicit user authorization, or **install** new ones. These are not synonyms. A supported package installed in three agents may represent three host registrations rather than three globally equivalent independent package identities. See [LiteSPM interop LLD](LITESPM-LITECOWORK-INTEROP-LLD.md).

HorizonCode is an AgentModule/qualified worker candidate, not the default invisible task planner or prerequisite for conversations. Native agent and provider support is qualified by exact agent build, transport, feature and operating system. An agent appearing in a catalog is never a supported claim.

## 3. Ten required V1 desktop product areas

| # | Area | Minimum working V1 acceptance; qualify each claimed format/provider |
|---|---|---|
| 1 | Universal Chat | New Chat; real native agent Send; streaming with stop/recovery; semantic committed history; search, rename/archive/delete; supported attachments; agent-dependent model, effort, commands and references. No Coworker/Task required. |
| 2 | Bring Your Own Agent | Real detection, installation/configuration observation, exact version support, native account sign-in, session start/resume/stop, supported models/options, native tools/subagent honesty, update/repair when qualified. No universal model-router substitution. |
| 3 | Persistent Coworkers | Quick Create or Customize; usable first chat; multiple scoped chats; stable identity; editable context/memory and selected chief; optional bounded worker allowlist; separately approved grants/responsibilities; pause/archive and recovery. |
| 4 | Real Computer and Workbench | Contextual right Workbench with truthful Activity, Files, Browser, Computer, Terminal and qualified artifact views; local file/API/site operations; real provider/session/controller identity; human takeover and return when actually supported. |
| 5 | Capabilities, Plugins, MCP, Connections | LiteSPM discovery, live installed/external inventory, source and compatibility, plan-before-write install/adopt/copy/update/repair/unmanage/remove; separate Connection, Coworker assignment and Task/Conversation grant; permitted invocation and evidence. |
| 6 | Real Tasks and Multi-Agent Work | Plain-language outcome to reviewed durable work; planning by native lead; accepted versioned plan; authorized Attempts/Effects; eligible native or heterogeneous delegation; control, steering, cancellation, recovery, independent verification and handoff. |
| 7 | Routines, Automations and Proactivity | Explicit saved Routine/Trigger preview; schedule timezone/DST/overlap/misfire; local host dependency; exact occurrence and Task identity; durable settlement, notification and history; approved triggers, bounded proactive suggestions. |
| 8 | Memory, Knowledge and Learning | Workspace and Coworker scopes; local documents/folders and grounded retrieval; transparent eligibility/confidence; quiet eligible learning only within accepted policy; inspect, correct, revoke, purge and avoid cross-scope leakage. |
| 9 | Library and Finished Work | Real file/revision ownership and provenance; format-specific preview/edit/save/export/compare or clearly read-only state; reopen across app restart; association to Conversation and Task and source revisions. |
| 10 | Desktop Shell, Settings, Background Runtime | Windows/Linux/macOS ambitions qualified individually; clean left navigation/chat/right Workbench; accessible keyboard and responsive UI; local-first data and permission controls, usage/diagnostics; backups/recovery, install/update, start/stop and machine-awake background runtime. |

**Completeness means one qualified real flow in every area, not that all agents and all formats are identical.** For each supported row publish supported exact combinations; unsupported optional integrations remain visibly unavailable and do not masquerade as passes. V1 cannot claim the whole area complete based only on mocks, enum values, schema fixtures or a catalog listing.

## 4. Canonical UX contract

### First use and primary navigation

First use: launch → choose Workspace → detect an eligible native agent → select native sign-in/setup if needed → blank Conversation with optional attachment and agent/model/effort controls. Never force Coworker creation, Task plan, capability wizard or workflow-builder.

Persistent sidebar: **New Chat**, conversation search/history, **Coworkers**, **Needs You**, **Work**, **Automations**, **Library**, **Discover**; Settings/Agent management and Inspector are progressive disclosure. The right Workbench is closed by default unless the user explicitly preserved a preference; it opens for meaningful active work or upon request. New Chat and Coworker chats use one composer and Conversation model. No separate Code/Cowork worlds.

Composer: attach files; choose **native agent first** → show that agent's valid native models/modes/effort only; send; optional actions (Run as Task, Schedule, Save as Routine) through explicit review, not permanent forced pills. Sending a message creates a Conversation turn; it does not create a Task by inference alone. A Task admitted later links its origin message and carries independent durable state.

### User controls

Never style absent data as successful. Show blocked / needs setup / offline / unqualified / permission required with a specific repair path. Separate **reported**, **observed** and **verified** outcomes. Agent selection is not permission, package installation is not permission, and a native tool report is not a LiteCowork-mediated receipt.

### Workbench and deliverables

Workbench tabs are views over real authorized objects, never simulated process or browser state. Browser WebView presentation is not Chrome automation. Rich presentation is an optional projection; semantic text survives rendering failure. Artifact UI must declare which formats are truly viewable, editable, exportable, comparable, or unsupported. Do not present a PDF preview as full DOCX editing.

## 5. V1 vs V2 hard boundary

**V1:** complete production-qualified local desktop feature set above, using a local Runtime where local machine/network availability allows. A provider-owned hosted agent may be accessed by the local desktop if separately qualified; it is **not** equivalent to hosting LiteCowork Runtime in cloud.

**V2:** LiteCowork-controlled cloud Runtime and cross-device continuation; remote Runtime enrollment and replication/fencing; multi-device native clients; multi-user organizations, team sharing/RBAC, managed cloud ecosystem. A native session or local process cannot teleport between devices. Future design entities may exist but do not block local chat.

## 6. Required cross-cutting invariants

1. Trust/Effect/Evidence authority applies only to mediated operations. Native-side actions are labelled with the actual assurance level; never pretend Core enforced them.
2. A Task effect with an unknown external result is reconciled authoritatively before a retry; no duplicate consequential actions.
3. One accepted Task spec/plan pins revisions; handoff starts an authorized new Attempt and fences late results.
4. A Coworker memory change cannot silently grant permissions, add a new external Connection, install a package, or enable a schedule.
5. LiteSPM observes before writing. An adopted external installation may be un-managed without uninstalling native resources; user edits win by default.
6. Data scope, user ownership, revocation and delete fences apply to retrieval, context, memory, logs and artifacts.
7. Local background work may continue after closing the *window* only while the qualified Runtime, machine, account and required dependencies remain available. Sleep/offline/shutdown are explicit blockers.
8. Accessible actions must have real handlers and visible error/empty/loading states. No mock provider execution under shipped labels.

## 7. Release evidence and status vocabulary

Use **TARGET_ONLY**, **CODE_IMPLEMENTED**, **SERVED**, **INTEGRATION_PROVEN**, **QUALIFIED** as distinct axes rather than an ambiguous green `implemented` badge. A real user journey requires all relevant owners and CODE + SYSTEM + owner USER evidence. For each row record exact build/OS/agent/provider version, method, observed result, failure/restart cases and evidence ref.

Gate order inside V1: (A) real native Conversation; (B) one verified durable Task; (C) LiteSPM observe/adopt/install + scoped use; (D) Coworker/Workbench/files/artifacts; (E) heterogeneous delegation and learning; (F) local automation and recovery; (G) platform/security/performance/backup and real-user release matrix. Independent qualifying tracks may overlap; no later gate makes an unproven earlier operation true.

**No amount of documentation or compile-only tests replaces production proof.** See [acceptance crosswalk](../implementation/V1-PRODUCT-ACCEPTANCE-MATRIX.md).
