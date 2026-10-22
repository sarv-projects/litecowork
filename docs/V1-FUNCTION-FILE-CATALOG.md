# V1 Implementation Function, File and Contract Catalog

Status NORMATIVE TARGET CROSSWALK; source modules are proposed landing zones, not an assertion files/functions exist. All canonical domain models and service semantics remain owned by linked authority docs. This catalog is implementation-oriented, intentionally enumerates function ownership and pairs every new UI contract with a backend owner, fault behavior and verification. E03/E07/E08/E09/E10 implement only after promotion to canonical schemas/routes.

## Function implementation rules

Every function must define input validation, owner/Workspace/Task scope, cancellation, idempotency, version compare, authorization and error taxonomy as appropriate. Read functions cannot have write effects. Database methods must join domain record, receipt and event/outbox in required transaction. Unknown provider result is not success. UI data is never sufficient authority. Provider adapter capability detection must be version-fenced. Each proposed module reuses current files where sensible; avoid duplicate stores/routers/service layers.

### F01 — Desktop routing and layout, proposed apps/litecowork-ui/src/shell/*

~~~ts
resolveInitialRoute(workspace, savedRoute, access): RouteResult
createNavigationIntent(route, dirtyDrafts, scopeEpoch): NavIntent
commitNavigation(intent, currentScope): NavigationState
restoreFocusedControl(previousRef, target): void
hydrateLayout(workspace, windowId, preferences): LayoutState
persistLayout(workspace, windowId, layoutVersion, values): Promise<Receipt>
setLeftRailMode(mode, width, accessibility): LayoutState
setRightWorkbenchMode(mode, ratio, currentlyOpen): LayoutState
recomputeResponsiveLayout(viewport, zoom, minWidths): LayoutState
openWorkbenchObject(ref, sourceAction, permission): WorkbenchState
closeWorkbenchObject(dirtyState, publicationState): CloseDecision
navigateWorkbenchHistory(direction, entries, permissions): WorkbenchState
preserveWorkbenchDraft(scopeChange, dirtyRevision): GuardResult
deriveNavigationCounts(inboxProjection, taskProjection): CountsOrUnknown
showScopedError(problem, focusOrigin, pendingRequest): ViewModel
restoreComposerFocusAfterOverlay(editorSelection, IME): void
~~~

Events: navigation intent/blocked/committed and pane open/close are frontend only; document commit and command accepted create durable events under existing owners. Adversarial: stale Workspace response, double click, archive while focused, dirty publish retry, 400% zoom.

### F02 — Settings and registry, proposed settings/* and agents/*

~~~ts
listAvailableAgentModules(runtime, os): ModuleRow[]
probeAgentInstallation(agentId, runtime): InstallationObservation
fetchAgentControlDescriptor(bindingId, versionFence): AgentControlDescriptor
validateNativeAgentConfiguration(module, config, schemaVersion): Validation
beginNativeAgentAuth(bindingId, mode, trustedOrigin): AuthAttempt
completeNativeAgentAuth(attemptId, callback, session): AuthReceipt
storeApiCredentialSecurely(slotId, secret, owner): CredentialReceipt
revokeCredentialSlot(slotId, expectedVersion): RevocationReceipt
createOrUpdateBinding(workspace, agentProfile, defaults, expectedVersion): Binding
setDefaultWorkspaceAgent(workspace, bindingId, expectedVersion): Receipt
resolveActiveModelCatalog(agentBinding, descriptor, account): Options
deriveReasoningControl(agentDescriptor): OptionUi[]
listDelegationProfiles(workspace, cursor): DelegationProfile[]
validateWorkerProfile(profile, workerCapabilities, budget): Validation
saveWorkerProfileRevision(profile, expectedVersion): RevisionReceipt
# LiteSPM rows are LiteCowork-side UI/API facades over the qualified LiteSPM
# client only. LiteSPM owns discovery and lifecycle; LiteCowork owns the
# authorization, preview-before-mutation and effect boundary. Never reimplement
# package discovery, install or state locally (LITESPM-LITECOWORK-INTEROP-LLD).
listLiteSpmPackages(category, cursor, capabilityFilters): PackageRow[]
previewPackageInstallation(packageId, manifestDigest): InstallPreview
installQualifiedPackage(approval, manifestDigest, scope): Receipt
assignConnectionToCoworker(connection, coworker, expectedVersion): AssignmentReceipt
revokeConnectionAssignment(assignmentId, expectedVersion): Receipt
checkConnectionHealth(connectionId, probeScope): Observation
deriveSettingsRowState(raw, authorization): ReadyRepairUnknown
~~~

Use existing AGENT-CONTROL/SERVICES/CONNECTORS-WORKFLOWS/TRUST authority, no new provider router. Passwords/API credentials only native secure input->OS keystore; status and redacted metadata only in UI. Probe != live send qualification.

### F03 — Composer, proposed conversations/*

~~~ts
loadConversationCatalog(workspace, cursor, filter): Page<Conversation>
createConversation(workspace, optionalCoworker, binding, reqId): Receipt
loadConversationSnapshot(conversationId, workspace, cursor): Snapshot
subscribeConversationStream(conversationId, afterCursor, scopeEpoch): Stream
scanNativeTrigger(text, selection, compositionState, descriptor): Trigger|null
mergeNativeCommandCatalog(nativeBuiltins, nativeCustom, precedence): Commands
filterSlashCandidates(query, descriptor): CommandOption[]
filterReferenceCandidates(query, descriptor, authorizedResources): ReferenceOption[]
insertNativeToken(text, selection, token, descriptorVersion): Draft
reduceSelectedAgent(prevDraft, nextBinding, descriptor): DraftCompatibility
deriveAgentDependentControls(binding, freshDescriptor, options): Controls
validateComposerAttachment(attachment, agentInput, scope): Issue|null
validateComposerOptions(values, nativeSchema, mutableScope): Validation
calculateSendEligibility(draft, runtime, auth, grants): Eligibility
constructConversationTurnRequest(draftSnapshot, contextRefs, requestId): ImmutableRequest
admitConversationTurn(request, exactExpectedVersion): TurnReceipt
reconcilePendingTurn(requestId, conversationId): TurnReceiptOrAmbiguous
cancelNativeTurn(turnId, owner, expectedState): StopReceipt
resumeQualifiedAgentTurn(sessionId, exactContext): ResumeResult
persistScopedDraft(draft, workspace, owner, privatePolicy): Revision
recoverScopedDraft(workspace, conversationId, digest): Draft
~~~

Definitive contract in NATIVE-COMPOSER-LLD. Turn request is never posted directly from WebView to untrusted agent API. Native dispatch, custom commands and @ reference semantics validated per harness/OS.

### F04 — Presentation and Workbench, proposed presentation/*, artifacts/*, workbench/*

~~~ts
validateNativeEvent(harnessId, version, payload): NativeEvent|Fallback
normalizeNativeEvent(nativeEvent, scope): PresentationItem
redactEventForUi(event, policy): SafePresentation
applyStreamFrame(messages, transient, orderedFrame): StreamProjection
recoverCommittedProjection(snapshot, lastCursor): Projection
resolveSafeRenderer(itemKind, schemaVersion): Renderer|Fallback
renderSafeMarkdown(text, linksPolicy): RenderTree
renderSafeTerminalOutput(bytes, ANSIAllowlist): RenderTree
renderBoundedJson(value, depthLimit, byteLimit): RenderTree
renderSafeCodeDiff(baseVersion, targetVersion): DiffView
renderVirtualTable(schema, pageCursor): TableView
compileOptionalRich(messageRef, layoutDocument): RichOrFallback
resolveArtifactVersion(workspace, artifactId, version): ExactVersion
validateArtifactDraft(base, currentHead, content): PublishPreflight
publishTextArtifactVersion(draft, expectedHead, requestId): Receipt
retryAmbiguousArtifactPublish(originalRequest): Receipt
compareExactArtifactVersions(leftRef, rightRef): Diff
saveArtifactAs(versionRef, destination, approval): SaveAsReceipt
buildNativeFallback(provider, rawEvent): AccessibleFallback
auditRendererConformance(providerFixtures, screenSnapshots): Coverage
~~~

Definitive contract in RENDERING-CONFORMANCE-LLD; no fake preview as real media renderer.

### F05 — Browser and computer, proposed browser/*, environment providers/*, UI workbench/BrowserSurface.tsx

~~~ts
discoverQualifiedBrowserProviders(os, chromeInstallations): Offers
createManagedBrowserProfile(workspace, retention, requestId): ProfileReceipt
previewChromeProfileImport(exportSource, destination): ImportPreview
commitChromeProfileImport(previewDigest, expectedProfileVersion): ImportReceipt
attachExistingChrome(userConsent, browserVersion, providerAuth): SessionReceipt
openBrowserSession(profileRef, environment, owner): BrowserSession
listBrowserTabs(sessionId, cursor): TabPage
navigateBrowser(sessionId, tabId, url, epoch, requestId): ActionReceipt
startBrowserAction(session, action, targetSnapshot, grants): Admission
requestBrowserControl(sessionId, USER, expectedEpoch): LeaseReceipt
returnBrowserControl(sessionId, AGENT, expectedEpoch): LeaseReceipt
observeBrowserFrame(sessionId, tabId, scope, redactPolicy): Frame
resolvePageActionTarget(observationRevision, selector): TargetOrStale
reconcileBrowserOperation(requestId, actionId): EffectStatus
handleBrowserDisconnect(sessionId, incarnation): RecoveryDecision
closeManagedBrowser(sessionId, retention, drain): CloseReceipt
detachExistingChrome(sessionId): DisconnectReceipt
admitBrowserDownload(sessionId, userApproval, destination): ArtifactReceipt
checkOsComputerPermission(runtime, display, operation): Eligibility
takeComputerControl(environment, epoch, user): LeaseReceipt
returnComputerControl(environment, epoch, policy): LeaseReceipt
~~~

Definitive contract in CHROME-BROWSER-LLD. Never access regular Chrome profiles via copying cookies or insecure debugging.

### F06 — Task and delegation, proposed domain-task/*, litecoworkd/src/{operator,planning,agents,delegation}*

~~~ts
createTaskSpec(request, owner, objective, pinnedSources): TaskRevision
proposeTaskPlan(specRevision, admittedLead, contextBounds): PlanCandidate
acceptPlanRevision(taskId, planCandidate, taskVersion): PlanReceipt
materializeReadySteps(planVersion, predecessorCompletion): Step[]
selectWorkerForStep(step, registryOffers, costs, policy): Candidate
validateAttemptAdmission(step, Runtime, Environment, Trust, Effect, Lease): Admission
startAttempt(admission, pinnedAgentSession, idempotency): AttemptReceipt
appendAttemptObservation(attempt, providerEvent): OrderedEvent
pauseTask(task, expectedVersion, safePointPolicy): PauseReceipt
resumeTask(task, latestGrants, recoveredEnvironment): ResumeReceipt
cancelTask(task, expectedVersion): CancellationReceipt
checkpointAttempt(attempt, portableInputs, effectLedger): Checkpoint
recoverAttemptFromCheckpoint(task, oldAttempt, providerAvailability): NewAttempt|Blocker
handoffAcceptedStep(task, worker, boundedContextEnvelope): Handoff
inspectTaskProgress(task, eventCursor): VerifiedProjection
verifyTaskOutcome(task, acceptance, independentEvidence): VerificationResult
decideTerminalStop(task, budget, verifiedResult, blockers): StopDecision
~~~

Cost estimates/quotas observational; stop/recovery objective first. Existing task runtime/domain ownership is authoritative, not this catalog.

### F07 — Connectors, approval and effects, proposed capabilities/* and litecoworkd operator adapters

~~~ts
discoverConnectionCatalog(query, category, cursor, scope): ConnectionPage
beginConnectionAuth(provider, scopes, redirectContext): AuthAttempt
handleConnectionCallback(attempt, signedState, code): ConnectionReceipt
testConnectionNonMutating(connection, method): HealthObservation
createCoworkerCapabilityAssignment(coworker, capability, policy): Assignment
resolveInvocationCapability(task, selectedAgent, requestedAction): Capability
evaluateTrustPolicy(actor, resource, action, scope): GrantOrRequest
buildUserApproval(target, effect, expiry, scope): UserRequest
answerUserRequest(requestId, expectedVersion, answer): ResolutionReceipt
admitEffect(invocation, digest, requestId): EffectReceipt
dispatchEffectOnce(effectId, currentLease): ProviderReceipt
reconcileEffect(effectId, providerObservation): FinalOrUnknown
revokeCapabilityGrant(grantId, version): RevocationReceipt
normalizeMcpTaskLifecycle(nativeTask, coreInvocation): StateMapping
~~~

Direct agent-native tool call is not Core-mediated unless qualified adapter/Gateway enforces authorization/Effect policy. LiteSPM handles package lifecycle, LiteCowork owns Task-scoped invocation.

### F08 — Coworker, memory and learning, proposed coworker/*, context/*

~~~ts
quickCreateCoworker(workspace, name, role, reqId): CoworkerReceipt
saveCoworkerRevision(coworker, changes, expectedVersion): RevisionReceipt
resolveCoworkerRecentConversation(coworker, workspace): OwnedChat|null
createCoworkerConversation(coworkerRevision, binding, reqId): ConversationReceipt
listCoworkerInstructions(coworker, revisionCursor): Instructions
evaluateLearningCandidate(source, redaction, consentPolicy): ProposalOrReject
dedupeMemoryCandidate(candidate, existingRecords): CandidateDecision
commitApprovedMemory(candidate, expectedPolicyRevision): MemoryRevision
resolveMemoryConflict(memoryId, sourceVersions): ConflictReceipt
revokeMemorySource(sourceId, expectedVersion): Revocation
projectContextActuallyUsed(turnId, receiptSources): Disclosure
assignQualifiedConnection(coworker, connection, scope): AssignmentReceipt
proposeStandingResponsibility(coworker, objective, conditions): InertProposal
~~~

Memory auto-learning is never inferred from user input alone. Privacy and source revocation must reach retrieval boundary; source sent to external provider is not retroactively erasable by local deletion.

### F09 — Automations, routines, event relay and notification, proposed automations/*, routines/*

~~~ts
createRoutineDraft(workspace, reusableInputs): Draft
publishRoutineRevision(routine, expectedVersion, requestId): Revision
previewAutomationSchedule(rule, timezone, DST, hostHealth): Preview
activateAutomationRevision(automation, trigger, grants, version): Receipt
pauseAutomation(automation, expectedVersion): Receipt
runAutomationNow(automationRevision, actor, reqId): OccurrenceReceipt
ingestAuthenticatedTrigger(providerEvent, subscription, cursor): NormalizedTrigger
dedupeTriggerOccurrence(triggerId, sourceEventOrSlot): Occurrence
admitScheduledOccurrence(occurrence, budgets, overlap, host): TaskOrBlocker
reconcileOccurrenceAfterRestart(triggerHost, cursor, policy): Outcome
continueAutomationTaskConversation(taskRef, sourceRevision, requestId): TurnReceipt
deriveNotificationEligibility(event, preferences, quietHours): Eligibility
enqueueNotification(eventRef, dedupeKey, channels): DeliveryReceipt
acknowledgeNotification(deliveryRef, providerReceipt): Receipt
~~~

No background claims if local host is asleep; scheduled actions are best effort and history must show actual missed slots.

### F10 — Runtime, storage, security, recovery, observability

~~~ts
bootstrapOperatorIpc(osPeerIdentity, RuntimeIncarnation): AuthSession
reconcileRuntimeIncarnation(previousIncarnation, processScope): Recovery
readLocalRuntimeReadiness(runtime, workspace): ExactStatus
validateWorkspaceEnrollment(principal, runtime, workspace): Authorization
prepareIsolatedEnvironment(taskPins, OSProfile): Attestation
observeProcessQuiescence(processScope, timeout): PositiveProof|Unknown
recordCommandReceiptAtomically(command, stateDiff, eventOutbox): Receipt
readCommandReceipt(requestId, actorScope): Receipt|None
rotateOsKeystoreReference(keyId, authorizedPrincipal): RotationReceipt
enforceSecretRedaction(payload, sinkType): Redacted
computeEventProjection(cursor, schemaVersion, ownerScope): Projection
replayDomainOutbox(afterOffset, consumerCheckpoint): ReplayResult
reconcileInterruptedEffect(effectId, providerTrace): UnknownOrVerified
collectDiagnosticBundle(consent, redaction, timeWindow): Bundle
previewStorageCleanup(paths, owners, sizes): CleanupPreview
cleanupGeneratedBuildOutputs(verifiedOwnedPaths, scope): Receipt
qualifyOsProviderVersion(provider, OS, kernel, securityGate): Qualification
~~~

## Physical package boundaries and cost control

React handles presentation and local pure reducers; Tauri handles privileged desktop IO/authorized bridge; litecoworkd handles orchestration, agent adapters, state transitions, provider processes, Capability/Effect/Grant; Rust domain crates enforce typed invariants; SQLite persists immutable versioning and outbox/receipts; LiteSPM handles package lifecycle. Never implement backend policy solely as disabled UI. Keep cross-module interfaces small and enforce each at actual boundary.

No duplicate React-side SQL storage, no direct browser UI -> CDP privileged token, no guessed provider APIs, no native session handles in frontend, and no separate generic provider router. More modules are not automatically better; inspect existing code and extend it before adding files.

## New contract-to-machine promotion checklist

For each target function that requires a server call, assign canonical service ownership, request/response JSON Schema, path/verb, error codes, auth scopes, idempotency and expected-version semantics, durable transaction/outbox, emitted events, affected SQLite migration, generated TypeScript type, Tauri command binding, telemetry/redaction and CODE/SYSTEM/USER gate. Promote all linked files atomically: docs/API.md, docs/EVENTS.md, docs/SCHEMAS.md, docs/STATE-MACHINES.md, docs/SERVICES.md, docs/STORAGE.md, docs/schemas/operator-api.openapi.yaml, relevant JSON schema + sqlite migration. Do not add an OpenAPI route with no handler and label it supported.

## Story readiness

E03 native chat/agent parity; E08 shell/render/Workbench; E07 browser/desktop; E05 delegation; E04 trust/connector; E09 automation; E10 package experience; E13 release. Each story in implementation/epics retains owner-specific acceptance. Exact class names here are recommended function ownership and intentionally do not override existing implemented names.


## Proposed wire promotion ledger — reviewed contract choices before code

The paths below are **candidate Operator routes** and may collide with or duplicate existing paths. Before implementation, compare docs/schemas/operator-api.openapi.yaml and existing router. If an equivalent canonical operation already exists, extend/qualify it instead of adding a duplicate. No route here is claimed served. Versioned event names and SQL table migrations must be assigned by owner before any public wire changes.

| Candidate verb/path | Typed request | Success projection | Required errors and owner |
|---|---|---|---|
| GET /v1/agent-profiles/{agentProfileId}/control-descriptor (**existing canonical route; do not create a duplicate**) | resolve selected AgentBinding -> agentProfileId; enforce fresh Runtime incarnation and descriptor digest | versioned AgentControlDescriptor with native AgentInputSurface, options/slash/reference/expiry | 401/403/404, stale or unsupported observation, provider unavailable; E03-S08/S10. Consult actual OpenAPI status codes and handler before modifying. |
| POST /v1/conversations/{id}/turns (**existing canonical route**) | `Idempotency-Key` header, `If-Match` and `X-Workspace-ID` as required by canonical API; selected binding, validated native options and typed pins | canonical committed ConversationTurn/message and receipt | Use API.md + OpenAPI schemas and exact existing domain error codes; E03-S02/S10 |
| Turn receipt reconciliation (**existing idempotency/outbox contract; NO new route assumed**) | retain original `Idempotency-Key` across transport retries; IPC frame `request_id` is correlation only | replay prior receipt through supported existing route or explicitly propose readback if provider outcome cannot be queried | API.md conventions require service/handler validation before inventing a by-request route; E03 |
| POST /v1/conversations/{id}/turns/{turn_id}/retry and POST /v1/conversations/{id}/turns/{turn_id}/cancel (**existing canonical routes**) | exact turn, `Idempotency-Key`, `If-Match`, authorized Workspace; native stop/retry mapping | actual accepted retry or cancellation request receipt | No speculative generic `/control` endpoint; steer/resume need separately justified, qualified canonical operations; E03 |
| GET /v1/browser/providers | OS/runtime/Workspace | qualified browser/provider/version/readiness/repair | 403 scope,503 runtime; E07 |
| POST /v1/browser/managed-profiles | RequestId, expected Workspace, profile retention | opaque profile ID and qualified provider | 409 duplicate/profile lock,413 quota,422 policy; E07 |
| POST /v1/browser/profiles/{id}/imports/preview | exact immutable authorized export reference | reviewed redacted plan/count/digest | 403 file,413 oversize,422 invalid file; E07 |
| POST /v1/browser/profiles/{id}/imports/commit | previewDigest, expectedProfileVersion, RequestId | applied/partial receipt and counters | 409 drift, 412 changed export, 422 invalid; E07 |
| POST /v1/browser/sessions | mode, profileRef/explicit Chrome consentRef, owner, RequestId | session ID, tab projection, control epoch, view capability | 403 missing consent,409 busy,422 unsupported Chrome,503 runtime; E07 |
| GET /v1/browser/sessions/{id} | owner + scope | safe browser projection | 403,404,409 stale incarnation; E07 |
| POST /v1/browser/sessions/{id}/controls | actor, nextOwner, expectedControlEpoch, requestId | new epoch, lease receipt, readiness | 409 stale actor/epoch, 422 unsafe pending action,503 provider; E07 |
| POST /v1/browser/sessions/{id}/actions | actor, epoch, tabGeneration, method+validated args, reqId | admitted/applied/ambiguous and effect identity | 403 unauthorized origin,409 stale,422 bad args,503 provider; E07 |
| GET /v1/browser/sessions/{id}/observation | tab ID, cursor, scope, redaction level | bounded safe DOM/a11y/frame metadata | 401/403/404/413/503; E07 |
| POST /v1/browser/sessions/{id}/detach | actor, expected version, RequestId | disconnected receipt, no user browser kill | 409 active action unknown,503 detach; E07 |
| LOCAL Tauri UI preference (no Operator route in local V1) | per-window/Workspace layout setting, bounded size, versioned non-authoritative preferences | local persistence of window/rail/Workbench ratios | authorization rechecked for content when reopening; never persist grants/leases; E08 |
| GET /v1/conversations/{id}/presentation and GET /v1/rich-presentations/{presentation_id} (**existing**) | authorized exact Conversation snapshot or rich projection | committed message and validated optional rich content | use canonical API/renderer fallback; new segmentation is a performance spike, not a predeclared V1 route; E08 |
| LOCAL schedule preview computation unless canonical API already provides the operation | exact Routine/Automation rule version, timezone/DST and Runtime host readiness | bounded list of predicted instants and explicit unknowns | prediction never enables a trigger or proves future execution; new route only if Operator-owned projection is demonstrably required; E09 |

Transport preference: reuse current authenticated local Operator IPC streaming/HTTP envelope and generated TypeScript contract. Do not make the renderer directly call privileged CDP or filesystem resources. If an operation is already in canonical API, document its existing operationId in story handoff and delete the redundant candidate here when final implementation lands.

### Candidate request/response schema fragments (pseudocode; promote atomically)

~~~ts
type ScopedMutationCommand<T> = {
  // Logical HTTP/local-IPC transport metadata, NOT an invented JSON request body.
  idempotencyKey:string; // canonical Idempotency-Key, stable across retries
  correlationId:string; // IPC request_id; different exchange correlation ID
  ifMatchVersion:number; // canonical If-Match for mutable resources
  workspaceContextId:string; // X-Workspace-ID, where scoped
  payload:T; // principal identity MUST be derived from authenticated transport
};
// Operator maps idempotencyKey to domain RequestId; do not equate it with IPC request_id.
type TurnSubmitPayload = {
  conversation_id:string; binding_id:string; descriptor_digest:string;
  draft_client_revision:number; text:string;
  native_tokens:Array<{native_id:string; kind:string; byte_start:number; byte_end:number; owner_binding_id:string}>;
  source_pins:Array<{resource_id:string; revision_id:string; digest:string}>;
  model_options:Record<string,unknown>; media_refs:string[];
};
type BrowserLeaseChangePayload = {
  session_id:string; expected_control_epoch:number;
  requested_owner:"USER"|"AGENT"|"NONE";
  observed_tab_generation:number; source:"UI"|"TASK";
};
type BrowserActionPayload = {
  session_id:string; tab_id:string; expected_control_epoch:number;
  expected_observation_revision:number; action_kind:string; bounded_arguments:unknown;
};
type BrowserProjectionSafe = {
  session_id:string; workspace_id:string; mode:string; browser_name:string;
  browser_version:string; active_tab_id:string|null;
  tabs:Array<{id:string;title:string;safe_url:string;generation:number}>;
  control_epoch:number; owner:"AGENT"|"USER"|"NONE"|"TRANSFER_PENDING";
  view_mode:"LIVE_INTERACTIVE"|"OBSERVATION_FRAMES"|"EXTERNAL_WINDOW"|"UNAVAILABLE";
  last_observation_revision:number; observed_at:string|null;
};
type OperatorMutationResult<T> = {
  request_id:string; aggregate_version:number;
  status:"ACCEPTED"|"REJECTED"|"PENDING"|"AMBIGUOUS";
  receipt:T|null; blocker_code?:string; retry_same_request_id:boolean;
};
~~~

Concrete JSON Schema promotion must add bounds/formats: string normalization, max UTF-8 sizes, URL scheme restriction, canonical UUID format where existing identity requires it, exact oneOf for input tokens/actions, enums, additionalProperties false, digest format, cursor format, numeric epochs >=0, no arbitrary browser execution string. Return sanitized domain error code/correlation and real human-readable resolution. Never return secret bearer tokens, remote debugger socket or arbitrary local profile path.

### New durable event-family implementation checklist

Proposed events only, to be scoped/versioned in EVENTS.md when served:

- agent.control_descriptor.observed / invalidated — keyed by binding/harness/version/Runtime incarnation, no secrets.
- conversation.turn.admitted / blocked / completed — exact turn/request/conversation identity, no raw user content.
- browser.session.ready / unavailable / disconnected — provider and opaque session, no Chrome profile bytes.
- browser.control.transfer_requested / committed / failed — from/to owner, expected/new epoch, Task/attempt scope.
- browser.action.admitted / observed / ambiguous — action kind, requestId, effect ID where applicable, no page secrets.
- browser.frame.observed — **transient stream only** unless separately pinned as authorized Resource, no durable domain Event per frame.
- ui.layout.updated — user preference only, may be local not domain event; never implies browser authorization.
- presentation.renderer.degraded — redacted diagnostic/telemetry, not a Task failure transition.
- automation.occurrence.admitted / skipped / missed — owner revision, dedupe key and grounded occurrence.
- coworker.connection.assignment.changed — versioned scope and provider metadata only.

New error classes to reconcile with existing shared taxonomy: STALE_DESCRIPTOR, NATIVE_FEATURE_UNSUPPORTED, DRAFT_INCOMPATIBLE, BROWSER_PROFILE_BUSY, CHROME_CONSENT_REQUIRED, BROWSER_LEASE_EPOCH_STALE, VIEW_FIDELITY_UNAVAILABLE, OBSERVATION_REVISION_STALE, EFFECT_RESULT_AMBIGUOUS, PROVIDER_AUTH_EXPIRED, RENDERER_TYPE_UNSUPPORTED, AUTOMATION_HOST_UNAVAILABLE, DRAFT_PUBLICATION_AMBIGUOUS. Use canonical codes if already present; do not fork error definitions.

### Race and transaction proof contracts

- Duplicate turn Send: two concurrent requests with identical requestId either get identical persisted outcome or one requests readback; never create two turns.
- Agent switch while descriptor load in flight: old lookup cannot swap current model/options; draft preserved, new agent controls atomic.
- Native command changed after palette selection: exact descriptor digest mismatch blocks turn and refreshes palette without executing old native operation.
- User browser takeover while agent action queued: new controlEpoch invalidates queued input, while already external Effect remains reconcilable/ambiguous.
- Browser tab close/redirect after observation: target generation and origin guard reject stale action.
- Chrome screencast frame after control transfer or route change: frame rejected when session/tab/epoch/viewport metadata mismatched.
- Browser import user changes target profile between Preview and Commit: digest/CAS blocks mutation.
- Rich response fails midway: semantic source message remains readable and committed, pending interactive controls cannot escape sandbox.
- Automation event duplicate on restart: unique source event/slot id prevents second Occurrence and Task.
- OS daemon crash during AtomicEffect receipt: recovery reads outbox, reconciles before resubmission; approval/grants rechecked.

## Function-by-function pre/post/error contract crosswalk

The current 226 catalog function signatures (F01–F14) are enumerated with owner, signature, proposed effect class, minimum precondition, postcondition, error/retry/race acceptance in [V1-FUNCTION-CONTRACT-LEDGER.md](V1-FUNCTION-CONTRACT-LEDGER.md). The generated class is a triage input for owner review, not an assertion that a side effect is actually pure or that the function exists. For cross-owner end-to-end flows see [V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md](V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md); these do not create new APIs or event enums.


## F11–F14 — Additional V1 function contracts: telemetry, memory, agent detection, terminal

These are **new proposed functions** subordinate to the four corresponding V1 LLD documents. They have not become served Operator routes, events, SQL tables, or OS handlers. Inputs/outputs are pseudotypes; preserve existing service/domain ownership and do not duplicate any current API. For every new mutation promote typed request/response, authorization, policy, idempotency and storage contract atomically before UI action becomes active. TLM-001–048 are the acceptance cases.

### F11 — Usage, cost, telemetry, health, caching; proposed litecoworkd telemetry and UI diagnostics modules

~~~ts
telemetryReceiveInferenceObservation(source, providerEvent, scope): UsageObservationReceipt
telemetryReconcileLeafIdentity(nativeReport, gatewayReport, providerReceipt): DedupDecision
telemetrySnapshotPrice(providerModelVersion, timestamp, source): PriceSnapshotOrUnknown
telemetryCalculateBillableAmount(usage, priceSnapshot, sourceConfidence): AmountOrUnknown
telemetryAggregateUsage(principalScope, dateWindow, timezone, filters): ScopedUsageSummary
telemetrySummarizeAgentSessions(scope, interval): AgentUsageBreakdown
telemetryRecordPhase(trace, phase, monotonicTick): PhaseTimingReceipt
telemetryCollectHealth(runtime, modules, dependencies): HealthProjection
telemetryClassifyFailure(error, layer, provenance): FailureKind
telemetryDeriveRecoveryAction(failure, providerOwner, effectState): RecoveryDecision
telemetryInvalidateCache(scope, policyEpoch, reason): CacheInvalidationReceipt
telemetryExportSanitizedBundle(ownerScope, previewDigest, retentionPolicy): ExportReceipt
telemetryApplyLogRetention(ownerScope, cleanupPreview, version): RetentionReceipt
~~~

Agent and gateway observations only merge under verified inference identity. Unknown usage is not zero, billable versus estimates are distinct, and parent+child rollups never double count. Persistent accounting requires a single owner that manages price snapshot, evidence and dedup identity, while debug ring logs and high-cardinality trace identifiers stay separate from immutable DomainEvents.

### F12 — Native agent-operated contextual memory, proposed Context/Learning/LiteSPM capability bridge

~~~ts
memoryResolveNativeToolSurface(binding, descriptor, scope): QualifiedToolsOrUnavailable
memoryCheckScope(principal, workspace, coworker, task, grantEpoch): AccessDecision
memorySearchScoped(query, sourceFilters, revisionFence, budget): MemorySearchReceipt
memoryFetchExact(memoryRef, resourceRevision, purpose): ScopedContentOrDenied
memoryLookupScopedCache(scopeDigest, queryDigest, revocationEpoch): CacheHitOrMiss
memoryInvalidateScopedCache(sourceRef, changedRevision, grantEpoch): InvalidationReceipt
memoryBuildContextAttachment(taskOrTurn, trustedSources, descriptor): ContextAttachmentOrBlocked
memoryProposeFromCheckpoint(committedTurnOrVerifiedTask, learningPolicy): LearningCandidateOrSkipped
memoryObserveStageLatency(trace, stage, monotonicClock): RedactedDuration
memoryRevokeAndPurgeSource(owner, source, version, purgePlan): PurgeReceipt
~~~

These calls cannot replace native harness memory or become an unrequested autonomous context injection loop. The agent controls its tool call; source owner and Revocation controls final fetch. Memory proposal is not automatically approved. Cache/policy warm path performance is separate from remote provider runtime.

### F13 — Agent auto-discovery and UI status projection, proposed AgentLifecycle/OS broker

~~~ts
agentScanModuleManifests(runtime, trustedRegistrySnapshot): ModulesAndVersions
agentEnumerateInstallCandidates(module, os, scopedRoots): CandidateSet
agentValidateInstallLocation(module, selectedPath, runtime): VerifiedCandidateOrDenied
agentProbeExecutableVersion(module, candidate, timeout): VersionObservation
agentProbeConfigStatus(module, candidate, owner): RedactedConfigObservation
agentProbeAuthStatus(module, authDescriptor, owner): AuthObservation
agentDeriveReadiness(installation, config, auth, descriptor, runtime): ReadinessAndRepair
agentRefreshBatchObservations(runtimeIncarnation, cacheEpoch, abort): CatalogProjection
agentApproveManualLocation(module, path, expectedVersion): ScopedLocationReceipt
agentPreviewPackageOperation(module, digest, distribution, diskPolicy): InstallPreview
agentAdmitPackageOperation(module, operation, reviewedDigest, owner): LifecycleReceipt
agentResolveTrustedLogoAsset(moduleManifest, assetRegistry): BundledIconOrFallback
agentRenderCatalogProjection(scope, filters, capabilityGate): CatalogPage
~~~

Executable in one Runtime is never assumed runnable in another. Probe paths must be exact/allowlisted and status axes remain independent. ACP metadata cannot spawn arbitrary commands; registry icon URL cannot execute remote SVG. Manual location is a checked fallback; no global PATH mutation.

### F14 — Integrated terminal PTY shell profiles and Workbench, proposed terminal modules

~~~ts
terminalDetectProfiles(runtime, os, installedShellInventory): QualifiedProfiles
terminalValidateProfile(profile, runtime, owner): ProfileQualification
terminalCreateSession(profileId, environment, scope, requestKey): TerminalReceipt
terminalAttachSession(sessionId, incarnation, cursor, scope): AttachedSessionOrLost
terminalReadFrames(sessionId, cursor, permission): BoundedFrameStream
terminalWriteInput(sessionId, ownershipEpoch, bytes, owner): InputReceiptOrDenied
terminalResizePty(sessionId, cols, rows, revision): ResizeReceipt
terminalChangeInputOwner(sessionId, requestedActor, expectedEpoch): ControlLeaseReceipt
terminalRestoreAgentOwnership(sessionId, owner, expectedEpoch): ControlLeaseReceipt
terminalRequestStop(sessionId, stopMode, impactApproval): StopReceiptOrPending
terminalDetachWorkbenchView(sessionId, windowId): LocalViewState
terminalSanitizeEscapes(ptyBytes, parserState, limits): SafeTerminalTokens
terminalObserveShellIntegration(sessionId, trustedProfile): IntegrationQuality
terminalExportOutput(sessionId, range, redactionPolicy, scope): ReviewedExport
terminalObserveProcessHealth(sessionId, runtimeIncarnation): ProcessHealth
~~~

Terminal UI never spawns a local shell directly. OS PTY/ConPTY process belongs to Runtime/Environment. Manual user shell is not an Agent Attempt; automated work still goes through Task/Effect. PTY output escape codes are untrusted display bytes, not security or success evidence. "Reconnect" never respawns old commands after a crashed PTY.

## F11–F14 cross-owner promotion / proof

| Module | Existing domain owner | New schema / API question | Required integration checks |
|---|---|---|---|
| F11 | OBSERVABILITY + Task/Attempt + STORAGE | usage leaf observation/provenance, separate durable measurement schema and scoped GET projections if justified | native+gateway dedup, incomplete counts, p95, pricing/FX and telemetry outage TLM-001–012 |
| F12 | CONTEXT + LEARNING + TRUST/LiteSPM | no generic Core semantic store, qualified native tool/grant and context receipt | revoke/final authorization, native autonomy, staged latency TLM-024–034 |
| F13 | AGENT-CONTROL + AgentModuleRegistry | bounded discovery observation and secure manually selected executable path | OS/WSL version/config/auth/readiness + trusted logos TLM-013–023 |
| F14 | ENVIRONMENTS + RUNTIME-LIFECYCLE + TRUST | PTY session/control lease frames and conditional persistence, not a generic raw shell endpoint | Windows ConPTY/Linux/macOS PTY, stale epoch, escape sanitization TLM-035–048 |

Do not claim TLM acceptance passed because a source file or catalog entry exists. All proposals require real provider/OS, CODE/SYSTEM/USER and release acceptance. Source comparison: V1-REFERENCE-OMNIROUTE-OMNIRUSH-TERMINAL.md.
