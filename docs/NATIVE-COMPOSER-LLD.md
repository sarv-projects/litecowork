# Native Chat, Composer, and Agent-Parity LLD (V1)

> **Canonical V1 entry:** ordinary Conversation Send must run through a
> qualified native AgentSession **without** mandatory Task/Coworker creation.
> Agent selection determines valid native model/effort/commands, not a
> duplicated LiteCowork-wide model router. Current source deliberately
> disables Send until real agent-session admission; this is TARGET, not live.
> See [final screen contract](V1-UI-SCREEN-CONTRACT.md).

Status: NORMATIVE TARGET, not evidence of implemented Operator endpoints. Owners: E03-S01/S02/S06–S11, E08-S01/S04/S06 and E04-S06. Authority: AGENT-CONTROL, AGENT-FABRIC, TASK-RUNTIME, API, SCHEMAS, STATE-MACHINES, TRUST, EXPERIENCE. This document defines UI/local orchestration algorithms; machine endpoints/events must be promoted atomically to API/OpenAPI/Events/schema/storage and tested before serving.

## Current implementation boundary (2026-10-25)

The served Conversation submit route persists the USER message, settles its turn
FAILED, and returns 503 DEPENDENCY_UNAVAILABLE while no qualified native session
supervisor is connected. Runtime inventory (ONLINE, incarnation READY, and Workspace
EXECUTOR) is not a native SESSION_READY handshake. No caller may create a STARTING
session and mark the turn RUNNING or report 202 from those inventory facts alone. The
unsafe low-level SQLite admission operation has been removed; the storage interface has
no Conversation STARTING-session reservation or readiness activation operation yet. A
future supervisor must reserve durably, invoke the native adapter outside the transaction,
then atomically activate the exact session and move the turn to RUNNING only after its
readiness handshake is observed and the host/incarnation is revalidated.

For an exact idempotent submit replay, the route checks the authenticated principal's
payload-digest receipt before consulting the current AgentBinding selection. It reuses the
binding recorded in the committed turn-created event, so a later disable/change cannot
turn a lost-response retry into `AGENT_UNAVAILABLE`. If the original request persisted an
OPEN turn but was interrupted before failure settlement, replay can continue that same
settlement; it cannot create another message or turn. A different payload under the same
key remains a conflict. This receipt path is implemented and does not enable native Send.

## Outcome and non-negotiable invariants

1. Every supported harness keeps its qualified native experience: slash commands, custom commands, @-file/@-agent references, model/provider selection, reasoning/effort/mode, images, files, session operations, approval/permission prompts, diffs, output events, cancellation and resume where upstream exposes them.
2. The selected AgentBinding owns available models and options. Never synthesize another provider's configuration, claims of auth status, or custom command meanings.
3. Plain text, resource pins and attachments survive Agent selection changes. Native tokens and options are revalidated transactionally; no silent reinterpretation; invalid turns never dispatch.
4. Host actions (+ menu, Task/Schedule/Save Routine) and harness-native / commands are separate authority namespaces. Untrusted command metadata is display data, not executable code.
5. A ConversationTurn is distinct from Task/Attempt. No durable Task emerges from a simple chat without a documented user action/policy; provider calls cannot bypass Task Trust/Effect fences.
6. Every request is user-scoped, Workspace-scoped, version-fenced and idempotent. Optimistic turn, draft and attachment identities prevent double sends and stale completions.
7. Exact agent version/adapter capability is qualified by real-harness conformance tests, not a registry entry or a demo. Unsupported native affordances remain explicit and nonfunctional rather than fake.
8. No direct provider secrets, untrusted execution results or private session handles in frontend/events/logs.

## Component owner tree

~~~text
ConversationRoute(workspaceId, conversationId?)
  LeftConversationRail(search, history, new, coworkers, unread/NeedsYou)
  ConversationViewport
    CommittedMessageList(virtualized)
    TransientTurnOverlay(reconnect-safe)
    ProviderInputRequest/ApprovalCard
    TaskCard/ArtifactCard/OpenInWorkbench
  StickyComposer
    RichPlainTextEditor(text/selection/composition, accessible)
    AttachmentTray(file, image, folder, resource, screenshot)
    SemanticNativeTokenOverlays
    AutocompleteRoot
      NativeSlashPalette
      NativeReferencePalette
      FileResourcePicker
      AgentMentionPicker
      HostActionPalette(SEPARATE)
    ComposerControls(+, agent, model, effort/reasoning, mode, overflow)
    Send / RunTask / Schedule / Stop / Retry
    EligibilityAndCompatibilityFeedback
  ContextualWorkbenchHost (see DESKTOP-SHELL-LLD)
~~~

Composer and options are shared for ordinary and Coworker-owned conversations; Coworker is not mandatory. UI controller data is transient except explicit draft persistence. Streaming is presentation, not authority.

## State and types (target client contracts)

~~~ts
type ComposerState = {
  workspaceId: string; conversationId: string | null;
  selectedBindingId: string | null; selectionEpoch: number;
  draftRevision: number; text: string; caret: number; composingIME: boolean;
  attachments: PinnedAttachment[]; nativeTokens: NativeToken[];
  optionSelections: Record<string, unknown>; optionDescriptorDigest: string | null;
  palette: null | {kind:"SLASH"|"REFERENCE"|"HOST"; range:[number,number]; query:string; epoch:number};
  descriptor: AgentControlDescriptor | null;
  eligibility: "LOADING"|"ELIGIBLE"|"INVALID_DRAFT"|"NEEDS_INSTALL"|"NEEDS_AUTH"|"STALE"|"OFFLINE"|"BLOCKED";
  inFlight: null | {clientTurnId:string; requestId:string; draftRevision:number; bindingId:string; status:"SENDING"|"AMBIGUOUS"|"ACCEPTED"};
};
type NativeToken = {
  tokenId:string; kind:"NATIVE_COMMAND"|"NATIVE_REFERENCE"|"NATIVE_AGENT_MENTION"|"HOST_RESOURCE";
  range:[number,number]; ownerBindingId:string; descriptorDigest:string;
  nativeId:string; display:string; rawLiteral:string; payload:unknown;
  validity:"VALID"|"NEEDS_REFRESH"|"INCOMPATIBLE"|"UNSUPPORTED";
};
type QualifiedCommand = {
  id:string; label:string; description?:string; source:"HARNESS_BUILTIN"|"HARNESS_CUSTOM";
  argsSchema?:unknown; execution:"AGENT_NATIVE"|"SESSION_OPERATION";
  effectClass:"READ"|"WRITE"|"UNKNOWN"; available:boolean;
};
type AgentUiDescriptor = {
  bindingId:string; digest:string; observedAt:string; expiry:string; harnessVersion:string;
  supports:{text:boolean;image:boolean;file:boolean;folder:boolean;resources:boolean};
  commands:QualifiedCommand[]; references:NativeReferenceKindDescriptor[];
  options:AgentSessionOptionDescriptor[]; nativeSessionActions:SessionActionDescriptor[];
};
~~~

Actual schema names normalize existing AgentControlDescriptor/AgentInputSurfaceDescriptor; do not introduce duplicate domain aggregates. Type aliases and projections adapt the canonical machine descriptor when promoted.

## Required pure functions and logic

| Function | Inputs -> outputs | Required edge behavior |
|---|---|---|
| resolveActiveBinding | Workspace/Coworker default, explicit selection, Runtime offer -> binding/repair | Never silently switch to a different provider after failure |
| loadFreshAgentDescriptor | bindingId, Runtime incarnation, digest, abort signal -> descriptor | Refuse expired offer; single-flight cache by exact version; cancel stale refresh |
| mergeNativeCommands | adapter reports, custom workspace config -> qualified list | Upstream precedence including custom overriding built-in; collisions use actual harness semantics |
| scanNativeTrigger | text, caret, selection, composingIME, escape/quotes -> trigger/range or null | No palette mid-IME, inside code block/link/path when rule excludes it |
| filterNativePalette | descriptor entries, query, locale -> virtual rows | Stable id, keyboard rank, sanitized display, no arbitrary executable metadata |
| resolveReferenceKind | trigger, descriptor, project context -> kinds/source | @subagent and @file are distinct; only provider-supported types |
| pickNativeToken | palette row, digest, draft -> updated text/semantic token | Replace only marked range; native id survives UI label changes |
| parseNativeInvocation | token/text, selected binding, descriptor -> provider request or error | Do not turn unknown slash into host action or blindly execute local shell |
| validateOptionSelections | selected options, descriptor schema -> valid/map/errors | No cross-agent guessed mapping; respect LIVE_SESSION versus NEW_SESSION |
| compareDraftCompatibility | files/images/resources/tokens, input surface -> field errors | Show retained incompatible input; never drop files silently |
| reduceAgentSelection | prior state, nextBindingId, descriptor -> atomic state | Increment epoch; retain text/files; invalidate owner-specific tokens; recompute dependent controls |
| deriveSendEligibility | Runtime, auth, binding, descriptor, draft, turn state -> status | Distinguish known blocked, unknown/stale and confirmed ready |
| buildTurnRequest | qualified state, context pins, requested options -> canonical immutable request | Validate workspace/version/bytes/permission; idempotency key and hash |
| reconcileTurnReceipt | requestId, persisted turn/message/readback -> accepted/ambiguous/error | Lost HTTP receipt must not duplicate side effect; query authoritative state |
| deriveTurnActions | active provider state -> stop/retry/resume/steer capability | Never show success on cancel request without confirmation |
| chooseSessionTransition | old/new binding, session health -> same/live change/new session/explicit handoff | A different harness never inherits a private provider session |
| restoreComposerDraft | scoped storage key and schema version -> valid draft/conflict | Never read a different Workspace/Coworker; redact sensitive metadata |
| trackDraftPublication | accepted turn receipt and draftVersion -> clear/not clear | Clear only original committed draft; preserve newer user typing |

## Agent switch algorithm (authoritative UI sequencing)

~~~text
selectAgent(nextBinding):
  save existing user text/attachments/selection, increment selectionEpoch
  close native palette; freeze dependent controls as one group
  load descriptor for nextBinding with abort token and Runtime-incarnation fence
  if stale result or Workspace changed: discard result, keep draft
  retain only option IDs/values validated in next descriptor AND visibly retained
  for each token from previous binding: mark incompatible until reselected
  compare draft attachment capabilities; keep incompatible items visibly
  atomically update binding/model/effort/mode/commands/reference catalog
  disable send until same digest & Workspace validation has completed
  do not auto-create session; do not dispatch turn; do not mutate an existing Attempt
~~~

When an active conversation changes native harness, display "Continue with new agent" and make provenance explicit. For a provider that accepts compatible live model changes, use its documented native session option call and await its acknowledgment. Otherwise allocate a new AgentSession for the next turn; prior committed messages remain historical. Large context transfer must use explicit supported Conversation context construction, pinned sources and bounds rather than provider-private session migration.

## Slash, @, and editor semantics

- Slash command display derives from live versioned harness descriptor, including custom project/global commands, exact argument schema, native dispatch action and reserved collisions. Never infer native commands from global names. On provider command table changes, refresh and preserve typed literal without dispatch.
- Support leading and in-text slash only where native syntax permits; distinguish literal slash in URLs, Markdown/code fences, pasted text, escaped slash and shell paths. IME composition must not trigger send, insert or auto-select.
- For @, qualify agent mentions, files and resources separately. Provider-native files use native path semantics only after permitted Workspace-root resolution. LiteCowork Resource pins stay typed, never silently become host filesystem paths.
- Enter commits selected palette item when open; Escape closes it; Up/Down/Home/End navigate; Tab accepts only according to explicitly displayed hint; Enter versus Shift+Enter configurable; Ctrl/Cmd+Enter sends when eligible; screen readers get finite live announcement.
- The entire draft text is not parsed as host instructions. For a native command that expands to prompt text, ask the adapter to resolve per upstream semantics; never run string templating, embedded shell substitution or path expansion in the UI.
- Compose image/file constraints from selected agent native input, upload policy and Task policy. Folder attachments are manifest/pinned resource selections, not ambient recursive permission.
- Native command may be disallowed inside existing task or conversation state; provide status and explicit repair, not a silent fallback to chat text.

## Turn admission and stream (target)

~~~text
onSend:
  snapshot editor state + selected descriptor digest + Workspace version + draft revision
  validate native token ownership and eligible Runtime/auth/option scope
  persist/restore attachment pins with immutable revision and digest
  create deterministic requestId/clientTurnId; persist pending-send local receipt
  send request to Operator (not direct provider from WebView)
  Operator authorizes, dedupes and commits ConversationTurn + input message
  runtime/session adapter starts native invocation when admitted
  frontend reads ordered committed stream + transient events from last cursor
  on accepted commit clear only exactly submitted draft version
  on ambiguous reply query requestId and pinned state; never resubmit as new turn
  on reconnect merge by event/message identity and persisted sequence
  on complete persist semantic answer before optional rich presentation
~~~

If invocation is a consequential external Effect, effect fencing and approval belong in canonical Core service regardless of native adapter. Provider session events are not themselves proof of external side effects or Task completion. Native turn requests do not carry secret bearer credentials to the WebView.

## Required native parity fixture matrix (E03-S11)

Per exact (OS, harness name, harness version, adapter version, auth mode), run directly native and via LiteCowork: onboarding/install/update/uninstall; auth cancel/expiry/repair; provider/model/effort/mode inventories; native built-in and custom slash collisions; @file/@agent; copy/paste Unicode and IME; files/images size/type; permission prompts; tool invocation; message streaming/reconnect; stop; resume; session history; switching across active runs; context compaction; long output; native extensions/skills/subagents; errors; local paths; interrupted Runtime. Record upstream-supported, bridged, degraded-with-disclosure, unsupported and blocked states. Support claims require no undisclosed regression on mandatory native affordances.

## Implementation file ownership (proposed; inspect current before adding)

- UI: apps/litecowork-ui/src/conversations/Composer.tsx, ComposerController.ts, composer-reducer.ts, native-input.ts, NativePalette.tsx, AgentDependentControls.tsx, conversation-send.ts.
- UI API: agents/agent-control-client.ts, conversations/conversation-stream.ts; use existing desktop-* bridge conventions and generated OpenAPI types.
- Tauri: src-tauri/src/conversation_stream_bridge.rs and native agent descriptor bridge only for desktop transport.
- Daemon: apps/litecoworkd/src/conversation_operator.rs, agents/{codex_app_server,opencode_server}.rs, orchestration adapters.
- Domain: existing Conversation/AgentSession/Task types in domain crates; storage-sqlite transaction/outbox.
- Tests: reducer property cases; native descriptor fixtures; two real harness end-to-end suites; crash/replay; accessibility.

These are target file responsibilities, not evidence that these files exist. No unnecessary generic provider layer; adapter remains protocol/harness-specific.

## Upstream documentation to pin at implementation time

- OpenCode commands: https://docs.opencode.ai/docs/commands/
- OpenCode agents and @ mentions: https://docs.opencode.ai/docs/agents/
- OpenCode server, sessions and events: https://docs.opencode.ai/docs/server/
- OpenCode permissions: https://docs.opencode.ai/docs/permissions/
- OpenCode rules/config: https://docs.opencode.ai/docs/rules/
- Codex documentation entry: https://developers.openai.com/codex/
- ACP protocol: https://agentclientprotocol.com/
Version pin exact upstream API/CLI source snapshots in implementation/SOURCES.md for conformance; docs URLs are references, not guarantees across future versions.

## Canonical mutation transport clarification (2026-10-12 audit)

The earlier client pseudocode uses `requestId` as a durable logical submission identifier. The actual Operator transport MUST follow docs/API.md: the stable `Idempotency-Key` header is preserved across retries and maps to the domain RequestId; local IPC `request_id` is only a per-exchange correlation ID. Mutations use `If-Match` for versioned resources and `X-Workspace-ID` for scoped commands. Do not add duplicate routes: canonical ConversationTurn creation/retry/cancel and agent-profile control-descriptor reads already exist in API.md/OpenAPI. Client-side token validation does not replace Operator authorization or qualified native adapter checks.


## Implementation gap found — Conversation agent selection (2026-10-25)

The accepted Composer contract allows an explicit Conversation AgentBinding override before falling back to the Workspace default. Current source cannot set that override: Conversation creation always stores null, the served router has no set/clear mutation, and storage validation rejects a non-null binding. The submit handler's override-first resolution therefore describes a target branch that the supported UI/API cannot reach. Do not present an in-chat Agent picker as working until the override mutation has an authenticated Workspace-scoped, version-fenced, idempotent storage/event contract and the UI reads the committed selection. Even after that, Send remains gated on a qualified native session adapter, descriptor/model controls, dispatch, streaming, settlement, cancel/retry and recovery.
