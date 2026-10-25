# V1 Observability, Token/Cost Accounting, Cache, Health and Connection Resilience LLD

Status: **normative desktop-V1 design target**, not a report of implemented handlers. This refines OBSERVABILITY, AGENT-FABRIC, TASK-RUNTIME, TRUST, STORAGE, EVENTS, RUNTIME-LIFECYCLE and DESKTOP-SHELL-LLD. Existing canonical domain models and event schemas remain authoritative. New proposal events/routes/tables MUST be promoted through existing schema migrations/service owners in one cohesive story before exposure.

## Mission and HLD boundaries

One LiteCowork Operator collects **observations** from its own action admission, native AgentModules, Task/Attempt/Effect journal, Runtime/process observation, gateway-provided usage, LiteSPM capability receipts, and UI timeline. Telemetry does not replace their authority and cannot rewrite an agent's private sessions or hallucinate its model/API usage.

~~~text
Native AgentModules / qualified Gateway usage / LiteSPM invocation / Tauri+OS probes
    -> bounded raw adapter observations (redacted; explicit provenance)
    -> UsageIngestor + HealthObserver + TraceCollector
         -> WAL/outbox for authorized accounting facts; ring buffer for volatile debug logs
         -> UsageDedup + PriceBook snapshot + Aggregation projections
         -> Trace/process/transport diagnostics + retention controller
    -> Operator scoped projections/stream
    -> Usage & Health pages; Session/Task inspector; diagnostics/support bundle
~~~

LiteSPM still owns install/auth/process lifecycle of connectors/capability hosts. External OmniRoute, if configured, is **one optional gateway data source**, not a new mandatory provider abstraction. Agent native data collection MUST be opt-in per its qualified protocol, never by parsing secret auth state or inventing an upstream billing contract.

## Telemetry classes and privacy boundaries

| Class | Owner and retention semantics | Examples | Explicitly prohibited |
|---|---|---|---|
| Immutable domain facts | Existing StateStore/EventStore/outbox | Task status, Attempt result, approved Effect | Fake domain events from UI animations |
| Security audit | TrustService; longer reviewable retention | grants/revocation, sensitive effect admission, credential-slot metadata | token material, full command secrets |
| Structured operational log | bounded local ring + configured rotation; severity/correlation | provider launch failure, scoped API error, source/age | prompt text, memory contents, environment tokens, private URLs |
| Traces/spans | sampled and bounded by owner | exact timing phases, no unbounded metric labels | native transcript/raw prompt/reasoning |
| Measurements | verified vendor/agent receipts or labeled estimates | tokens, billable currency, cache-hit, latency | guessed $0 or total tokens when unknown |
| Derived metrics | aggregations over eligible deduplicated observations | p50/p95/p99, per agent/session/workspace | IDs / model strings as uncontrolled metric labels |

Debug logs have configurable size cap, daily rotation, redaction, delete/export; default minimal diagnostics. User may inspect per-source privacy and clear operational logs without erasing immutable domain/audit records or historical Task Evidence. A support bundle requires preview, visible redaction categories and explicit user action.

## The canonical unit of accounting: one inference observation, not one Task

~~~ts
type UsageSource = "NATIVE_AGENT"|"GATEWAY"|"PROVIDER_RECEIPT"|"BILLING_IMPORT"|"MANUAL_ESTIMATE";
type UsageConfidence = "PROVIDER_BILLED"|"NATIVE_REPORTED"|"GATEWAY_REPORTED"|"TOKENIZER_ESTIMATE"|"UNKNOWN";
type UsageCompleteness = "COMPLETE"|"PARTIAL"|"UNKNOWN";
type InferenceUsageObservation = {
  observationId:string; runtimeId:string; runtimeIncarnationId:string;
  workspaceId:string; taskId?:string; planStepId?:string; attemptId?:string;
  conversationId?:string; turnId?:string; agentBindingId:string; agentSessionRef?:string;
  parentDelegationId?:string; providerRequestId?:string; externalGatewayRequestId?:string;
  modelId?:string; modelVersion?:string; providerId?:string; priceBookVersion?:string;
  observationSource:UsageSource; confidence:UsageConfidence; completeness:UsageCompleteness;
  inputTokens?:number; outputTokens?:number; reasoningOutputTokens?:number;
  cacheReadInputTokens?:number; cacheWriteInputTokens?:number;
  unitPrice?:{currency:string;inputPerMillion?:string;outputPerMillion?:string;
    cacheReadPerMillion?:string;cacheWritePerMillion?:string};
  providerBilledAmount?:{currency:string;decimalAmount:string};
  estimatedAmount?:{currency:string;decimalAmount:string};
  subscriptionKind?:"METERED_API"|"FIXED_PLAN"|"FREE_TIER"|"UNKNOWN";
  startedAt:string; observedAt:string; finishedAt?:string; sourceEventDigest:string;
  rootCorrelationId:string; observationVersion:number;
};
~~~

A UsageObservation is **not** itself a domain Event until accepted into a versioned accounting projection with canonical schema. A model/agent may conceal individual subcalls, cache activity or a flat-subscription quota; in that case only the outer observed session duration/request count may be recorded and token/cost coverage is PARTIAL/UNKNOWN. No proxy-level accounting is promised for private, unmediated native inference. Only the minimum safe references cross from native adapters; secret access/account identifiers remain access-controlled and never high-cardinality metrics. All integer token counters must be finite, nonnegative, bounded; cache-read/cache-write and reasoning token counters are **subsets** of input/output totals where the source convention says so. Do **not** add subset values to the input/output totals again. For an unsupported source, field absent + status UNKNOWN—not zero.

**Dedupe and provenance precedence:** prefer provider-billed receipt over an exact gateway record over native reported use over tokenizer estimate. The same underlying inference seen from agent and gateway is **one unit**, joined only by verified identity (provider request ID and trusted adapter source mapping); near-same timestamps/prompt fingerprints alone cannot safely dedupe. Conflicting or potentially overlapping reports **without proven join identity** remain UNRECONCILED; they are individually visible with provenance but excluded from a purported exact deduplicated total until either disjointness or sameness is proven. Surface a partial/uncertain range or coverage warning, not a misleading additive billed total. Distinct known provider-request IDs from separate actual calls remain independent. No two providers' costs are silently merged. Replayed telemetry increments observation version, not spend. A follow-up and a retry that made real upstream calls each get independent usage rows; a deduplicated transport replay does not. Nested lead/subagent/Task rollups use additive *leaf inference IDs*, not both leaf and already aggregated parent totals.

**Pricing:** immutable price snapshot per measured request (provider, model, effective date, currency, input/output/cache tier, applicable plan); monetary decimals, never float for persisted money. Provider billed amount, calculated estimate and fixed-plan subscription allocation are separate views. A subscription plan's already-paid access is not a $0 metered cost claim. If usage is hidden by native subscription or an upstream omits cache counters, show **Unknown/not reported**, with coverage percent and source rather than zeros. Never invent real USD value or FX conversion. Support multi-currency grouping; no sum across currencies unless explicit rate/source/time.

## Usage scopes and aggregation contract

Dimension filters: exact Workspace, Coworker if applicable, Conversation/session, Task, PlanStep, Attempt, lead/subagent, provider, model, Runtime, day interval, source/confidence, request outcome. Owner-scoped query access only. Time filters: 1D/7D/30D/90D/YTD/All/Custom, UTC storage timestamps and user-local **display** timezone including DST windows; half-open [start,end) interval; daily bucket by selected reporting zone with stable cursor, not arbitrary Date.now string grouping.

Summaries: actual inference requests (dedup ID count), successful/failed/aborted calls, reported input/output/cached tokens with source coverage, estimated and billed costs separately, average tokens per eligible request, cost per eligible verified Task, cache read ratio, per agent share, quota/remaining status UNKNOWN vs observed, time-to-first-token and p50/p95/p99 per operation. Request attempts and upstream inferences are **different counters**; retries increase invocation attempts, only accepted inference IDs increase actual model use. Per-session, per-Task, all-workspaces, and multi-day views derive from the same immutable leaves and never sum rollups into rollups.

Charts must not show "0 tokens / $0.00" as fact when nothing was observed from native agents. The initial empty state says "No measured usage yet; some agents do not expose token counts". Show coverage tags/source, missing data, date and pricing confidence; allow drill-down without leaking other Workspaces. Price budget alerts must distinguish measured versus estimated and reserve conservatively when usage is unknown. A native fixed-subscription or opaque model without authoritative cost/remaining-quota cannot provide a guaranteed monetary hard stop: enforce only supported operation/time/attempt budgets at admission, label any cost ceiling as an estimate, and require a provider-enforced/billed guarantee for a true hard currency cap.

## Timing, logs and diagnostic schema

Every supported operation starts a monotonic local timer and one trace/span. Clock correlation is only across wall timestamps; latency measured using monotonic elapsed time. Phases: UI admission, authorization/CAS, IPC queue/wire, agent host cold start/attach, native descriptor refresh, memory policy gate, memory local hit, retrieval provider wait, context merge, upstream handshake, time-to-first-token, streaming, external Effect/reconciliation, verification, artifact render, UI first paint. Record p50/p95/p99 and denominator; avoid percentile claims on sparse samples and distinguish control-plane time from upstream inference.

~~~ts
type OperationDiagnostic = {
  traceId:string; spanId:string; parentSpanId?:string; correlationId:string;
  scopeKind:"TURN"|"TASK"|"ATTEMPT"|"INFERENCE"|"INVOCATION"|"TERMINAL"|"BROWSER"|"MEMORY"|"AUTOMATION";
  logicalRef:string; sourceClass:string; operationClass:string; startUtc:string;
  elapsedMs:number; result:"SUCCESS"|"FAILED"|"CANCELLED"|"TIMEOUT"|"AMBIGUOUS"|"UNKNOWN";
  reasonCode?:string; retryCount:number; transportReconnectCount?:number;
  phases:Array<{phase:string;durationMs:number}>; redactionProfile:string;
};
~~~

Logs/metrics/traces are not append-only User/Task status. Diagnostic correlated child spans retain parent trace but do not inherit grants. Sensitive source content may be displayed only by an authorized owner action against original Resources, never by telemetry export. Search diagnostics by error class, day/time, agent, Task, component, root trace. File export is subject to size/privacy quota.

## System health, performance and reliability

Independent probes: Operator serving, IPC handshake, SQLite writer queue/WAL/disk, event projection lag, Task/Attempt supervisor, AgentModule discovery, active AgentSession handshake, LiteSPM reachability/qualified active capability, browser/terminal Environment health, index freshness, scheduled trigger host, logs and telemetry backpressure. Probe outcomes are READY, DEGRADED, BLOCKED, UNKNOWN, OFFLINE with observation age and stale cutoffs. A binary present is DETECTED—not usable or authorized; daemon running is SERVING—not execution ready.

Health cards show uptime based on real process incarnation; CPU/RAM/disk of LiteCowork vs child agents separately; cold-start duration; IPC reconnect rate; event outbox lag; active failed/ambiguous Effects; terminal disconnections; data indexed; recent critical errors; resource pressure; crash/recovery time; completion/reconciliation confidence. Profiling baseline is required before setting numeric thresholds; don't hardcode SLO achievements. Poll slow-changing probes with single-flight cache and invalidation; prefer push authoritative state when available.

### Connection resilience layers (never conflate)

| Layer | Breaker/cooldown scope | Repair/retry policy |
|---|---|---|
| Desktop IPC connection | exact Operator incarnation + authenticated peer | exponential jitter + bounded attempts, then readable offline state; cursor-based stream resume |
| Native AgentSession transport | AgentBinding + session/incarnation | adapter-specific reconnect/resume after new qualified descriptor; no silent switch to another harness |
| Provider gateway | provider connection/account/model (only if a gateway actually reports it) | observe gateway-owned breaker/cooldown, do not invent a second router or alter native agent backend |
| LiteSPM connection | exact installation/activation/connection | per-connection health, auth expiry and revocation; register retries through LiteSPM contract |
| Browser/Terminal Environment | Runtime/Environment/lease epoch | stale epoch fences all new actions; prefer attach only to confirmed live process |
| Durable Task/Effect | Step/Attempt/Effect identity | canonical reconciliation; never auto-reissue uncertain writes with a new idempotency key |

Failures: classify auth expired, permission/revoked, quota exhausted, provider down, rate limit with Retry-After, unsupported model, 404 resource, network offline, process crash, handshake/protocol mismatch, split-brain, local storage pressure, unknown side effect. Auth 401/403 does not automatically mean provider outage; model unavailable does not take down every agent; circuit half-open probes are owned by the corresponding provider/gateway, not invented by LiteCowork. Circuit states CLOSED/DEGRADED/OPEN/HALF_OPEN apply only to scopes under direct LiteCowork control; otherwise display the provider's reported state with age.

Streaming resume: read authoritative committed transcript/events from durable cursor, dedupe frames using (session/incarnation, sequence/event ID), bound out-of-order buffer, resync after gap, keep local draft. Restart replays outbox receipt, not native tool calls. Backpressure drops/coalesces **debug sampling frames**, not approved effect/event/audit facts. Graceful stop drains and reports UNKNOWN side effects, never "everything failed" when disconnected.

## Caching: four independent purposes, never one global cache switch

1. **Native/provider prompt cache:** vendor-owned; LiteCowork observes cache-read/write tokens and effective cost only if exposed. Never override native provider prompt prefix/cache rules without explicit qualified agent contract.
2. **Metadata/discovery cache:** per Runtime/incarnation/AgentModule version; short TTL and invalidate on install, PATH/config/descriptor change, auth state or user refresh; stale can display with timestamp, cannot authorize action.
3. **Context/memory retrieval cache:** only for exact Workspace/Coworker/principal, policy/grant epoch, source revisions, query/filter digest, provider/index version, purpose and redaction level. Bound byte capacity/TTL; sensitive payload in approved encrypted or process-memory store; tombstone/revoke invalidates synchronously before new use. Negative permission decisions are never cached beyond the gate.
4. **Read-only deterministic output cache:** only deterministic side-effect-free transforms with pinned sources, schema/algorithm version, visibility scope and verified digest. No reuse of LLM completions, browser actions, terminal commands, untrusted tool outputs or non-idempotent Effects as a "hit".

Always measure cache hit/miss/eviction/age/p95 and correctness. Cold miss must not become an uncapped synchronous model-memory lookup. Have an explicit disable/clear per cache class; clearing derived caches cannot delete immutable audit/event evidence.

## Operations and candidate modules

~~~text
apps/litecoworkd/src/telemetry/usage_ingest.rs (candidate)
apps/litecoworkd/src/telemetry/aggregation.rs
apps/litecoworkd/src/telemetry/pricing.rs
apps/litecoworkd/src/telemetry/health.rs
apps/litecoworkd/src/telemetry/resilience_observer.rs
apps/litecowork-ui/src/diagnostics/ (Usage, Performance, Health, Logs, Caches)
~~~

Functions with required outcomes:
- observeInferenceUsage(nativeEvent, sourceProof, scope) -> ACCEPTED/RECONCILED/DUPLICATE/UNKNOWN with stable leaf identity; never charge twice.
- reconcileUsageProvenance(leaf, gatewayReceipt, providerReceipt) -> merged provenance / flagged mismatch; cannot use prompt-content matching.
- snapshotEffectivePrice(providerModelVersion, timestamp, rateSource) -> immutable typed quote or UNKNOWN.
- aggregateUsage(scope, dateWindow, timezone, confidence) -> breakdown with explicit known/unknown coverage; exact half-open date filtering.
- recordLatencyPhase(traceId, phase, monotonicNow) -> bounded elapsedMs; never put raw User IDs in metric labels.
- observeHealth(runtime, module, connection, env) -> fresh status, age and exact repair reason without causing a write.
- deriveConnectionRecovery(failureKind, ownership, retryAfter, effectStatus) -> RETRY_TRANSPORT/WAIT_FOR_USER/RECONCILE_EFFECT/BLOCK; no blind non-idempotent retry.
- invalidateScopedCache(scope, cause, revision) -> tombstone/new epoch; revocation check at final authorization gate.
- exportSanitizedSupportBundle(selection, redactionPolicy) -> preview, manifest, bounded archive; no credentials/private chats.
- applyTelemetryRetention(policy, activeTraceRefs) -> rotated logs and protected immutable evidence, not task history deletion.

**No served API assumed:** proposed GET UsageSummary/UsageItems/Health/Trace/Logs/CacheStats projections require explicit authenticated Operator OpenAPI + Command/Schema/SQLite owner promotion. Settings like log retention/metrics opt-in must use canonical Trust/Runtime settings service; clear-cache is a scoped versioned mutation, not an ad-hoc frontend action. V1 therefore exposes no active `setTelemetryRetention` or `setDiagnosticSampling` control; bounded retention/sampling remain service policy until a durable local settings contract is promoted.

## UI layout acceptance

Usage dashboard adapted from reference screenshots but not cloned: global filter bar [1D 7D 30D 90D YTD All Custom]; 4 headline cards (measured total tokens, input, output, provider-billed versus estimated cost); session/Task/agent/model/provider breakdown; requests and latency/failure chart; cache hit/read/write and true quota observation; sparklines with data provenance; drill-down inspector. Dashboard tabs Overview / Sessions / Agents / Models / Cost / Reliability / Cache / Logs, keyboard/screenreader accessible. Empty state cannot falsely advertise "$0.00" for unobservable agent use. Health and logs have read-only default, redaction, scoped export and repair actions.

## Cross-agent usage interface and the detailed table — 2026-10-22

The owner's requirement: usage must be measurable **per session, per Coworker, per Task and
per agent**, must **add up across every harness the user runs**, and must open into a
**detailed table** rather than a single headline number. This section specifies that
interface. It extends, and never weakens, the observation model, dedupe precedence and
honesty rules above.

### Why one ledger is not enough

Competitors converge on the same problem: a subscription agent has no per-token ground
truth. The accepted resolution (Cursor's own event API is the cleanest precedent) is to
**meter everything and charge only some rows** — Cursor emits both `requestsCosts` (quota
units) and `chargedCents` (money) with a `kind` of "Included in Business" vs "Usage-based".
LiteCowork therefore keeps **two joinable ledgers**:

~~~ts
// Ledger 1 — extends InferenceUsageObservation above; one immutable row per model response.
type UsageCostBasis =
  | "PROVIDER_API"            // real billed cents from a provider receipt
  | "LIST_PRICE_ESTIMATE"     // computed from an immutable price snapshot; an estimate
  | "SUBSCRIPTION_INCLUDED"   // included in a flat plan the user already pays for
  | "UNKNOWN";                // the harness exposed no cost-bearing signal at all

// Ledger 2 — NEW. A flat plan's remaining entitlement, never money.
type QuotaSnapshot = {
  snapshotId:string; workspaceId:string; agentBindingId:string; agentHarness:string;
  planLabel?:string;
  windowKind?:"HOURLY"|"FIVE_HOUR"|"WEEKLY"|"MONTHLY"|"UNKNOWN";
  usedAmount?:number; limitAmount?:number; unit?:"REQUESTS"|"TOKENS"|"CREDITS"|"UNKNOWN";
  remainingPercent?:number; resetAt?:string;
  source:"HARNESS_REPORTED"|"PROVIDER_API"|"USER_REPORTED"|"UNKNOWN";
  confidence:UsageConfidence; observedAt:string; version:number;
};
~~~

A subscription's already-paid access is **never** a $0 metered cost, and a quota snapshot is
**never** converted into money. They are displayed on separate axes and never summed.

### What a local app can actually read, per harness

`HARNESS_REPORTED` rows are obtained by a **usage-only reader** over the user's own local
session files. This is a new `UsageSource` value and it is deliberately narrow:

| Harness | Local source | What it yields | Traps the reader must handle |
|---|---|---|---|
| Claude Code | `~/.claude/projects/**/*.jsonl` | per-response `input_tokens`, `output_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, model | one API response spans several lines sharing `message.id`/`requestId`; per-line `output_tokens` is a placeholder, so take the max per id; subagent rows live under `subagents/`; `costUSD` is usually absent; **plan-limit utilization is not in local files at all** |
| Codex CLI | `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` | `total_token_usage`, `last_token_usage`, `rate_limits`, model, cwd | `total_token_usage` is **cumulative — take the last value, never sum**; `cached_input_tokens` is **inside** `input_tokens` (unlike Claude); duplicate `token_usage_record` rows carry `response_id` for dedupe; older interactive TUIs did not persist `token_count` |
| OpenCode | `opencode stats` plus its server SQLite store | per-session cost and `tokens_input/output/reasoning/cache_read/write` | subscription models are already labelled "Pricing unavailable" upstream — preserve that label, do not infer a price |
| Cursor, Copilot, and any subscription-only path | **nothing local is authoritative** | server dashboard or Admin API only, when the user supplies it | never reconstruct token counts from screenshots or estimates presented as fact |

**Hard rule for these readers.** A session log contains prompt and response text. The reader
extracts **only** numeric usage counters, model id, response id and timestamps. It never
ingests message content into LiteCowork storage, events, metrics or logs, and it fails
closed (row absent, coverage `PARTIAL`) rather than storing content to "understand" a
format it cannot parse.

### Aggregation arithmetic

Roll-ups are **leaf-additive only**, computed from immutable leaf rows, never from another
roll-up:

~~~text
day → model → agent harness → AgentBinding → Coworker → Task/Attempt → Conversation/session
~~~

Each roll-up row reports: `requests`, tokens by type (uncached input, cached input, cache
write, output, reasoning), `equivalent_list_price` (estimate lane, labelled), `billed`
(money lane, only where `cost_basis = PROVIDER_API`), `coverage` (percent of requests with
a known basis), and — for subscription agents — the joined `QuotaSnapshot`. Two rules make
"add up all" safe:

1. **No double counting across tag dimensions.** LiteLLM shipped a multi-tag explode view
   that overstated spend by 67%; when a row carries several values for one dimension the
   cost is **split proportionally across the group**, never copied into each group.
2. **No rollup-of-rollup, no cross-currency sum.** Leaf inference IDs are the only additive
   unit; a lead plus its two subagents is three leaves, and the lead's own row is never also
   added as a parent total. Amounts in different currencies stay in separate totals with an
   explicit rate/source before any conversion.

### The detailed table the user opens

The dashboard headline cards stay summaries; the table below is the contract for what a
user sees when they open any number.

~~~text
Usage table (virtualized, sortable, keyboard/screen-reader navigable)
  Grouping selector: By session | By Coworker | By Task | By agent | By model | By day
  Columns:
    Label            the grouping key's real name (session title, Coworker name, Task
                      objective, harness name, model id, or date)
    Requests         accepted leaf inference count (retries are counted separately and
                      never inflate this)
    In / Out / Cached tokens, split by type, each cell showing its coverage tag
    Equivalent cost  estimate lane, always suffixed (est.)
    Billed           money lane, only when cost_basis = PROVIDER_API; blank otherwise
    Quota            for subscription agents: used/limit + reset time from QuotaSnapshot,
                     or "Not reported by this agent"
    Coverage         COMPLETE | PARTIAL | UNKNOWN for that row, with the missing source named
    Source           HARNESS_REPORTED | PROVIDER_API | GATEWAY | ESTIMATE | MIXED
  Drill-down: any row opens its leaf events; any leaf opens its provenance (adapter, response id,
              price snapshot, confidence) — never prompt or response content.
  Export: owner-initiated, scoped to the filtered window, redacted, size/privacy bounded.
~~~

The table **opens on the grouping the user last chose, per Workspace**, and a "Total" row
appears only when the visible rows' currencies and coverage make a total meaningful; the
footer always shows the coverage percentage and the count of rows whose basis is UNKNOWN,
so a total can never silently imply completeness.

### Interface promotion status

These are **target projections**, not served routes. Promotion requires the canonical chain:
OpenAPI operation, authenticated Operator handler, SQLite owner for both ledgers, TS types,
Tauri command, served-route inventory regeneration and contract tests. Until then the UI
must not render a usage table that claims measured totals; the honest empty state from the
dashboard contract above still applies.

## Acceptance and adversarial tests

- One lead calling two subagents and a gateway: 3 distinct inference IDs, deduped even if gateway+native observation both arrive; rollup never counts parent twice.
- Fixed-price native subscription with no usage reporting: UNKNOWN tokens/cost, not zero; cost-per-Task labeled unavailable.
- Reconnect/replay/delayed usage report: update provenance/revision without second charge. Out-of-order and late receipts cannot change old price snapshots.
- Cached tokens are subsets; never double count; no observed cache field => not 0 cache usage.
- Two currencies and pricing rate revision: separate totals, immutable source and effective time, no silent conversion.
- Provider service 503 versus account 401 vs model 404: different qualified repair; authorization not retried as provider outage.
- IPC drop after external Effect: stream resumes from committed cursor, no side-effect replay.
- Traces don't leak prompt text, JWT/cookies/secrets, terminal keystrokes or Resource URL strings; metric label cardinality bounded.
- 30-day filter across DST and year boundary: correct half-open buckets, no missed/duplicated leaf rows.
- Control-plane p95 measured independently of model response latency with monotonic clock.
- Cache revoke race: external-source tombstone stops future context injection even if old cache entry is still present.
- Stress: telemetry sink unavailable/slow; Task/event admission remains correct, logging backpressure bounded; disk pressure reports degraded, not silent deletion.
- Support-bundle preview/redaction and log retention cannot erase required audit/Evidence.

## Source research and non-adoption decisions

- OmniRoute architecture/resilience: https://github.com/ourines/omniroute/blob/main/docs/architecture/ARCHITECTURE.md and https://github.com/ourines/omniroute/blob/main/docs/architecture/RESILIENCE_GUIDE.md
- Specific source: https://github.com/ourines/omniroute/blob/main/src/shared/utils/requestTelemetry.ts ; https://github.com/ourines/omniroute/blob/main/open-sse/utils/usageTracking.ts ; https://github.com/ourines/omniroute/blob/main/src/domain/costRules.ts ; https://github.com/ourines/omniroute/blob/main/src/shared/utils/circuitBreaker.ts
- OpenTelemetry GenAI conventions: https://github.com/open-telemetry/semantic-conventions-genai/blob/main/docs/gen-ai/client-inference.md

Borrow the source-proven separation of request phases, billable usage, account/model outage classes, and dashboard drilldown. Do **not** adopt OmniRoute's replacement provider gateway, invasive native session extraction, web-login proxy techniques, generic provider auth store, or prompt replay as LiteCowork Core features. Its code/docs versions and route counts may diverge; pin a revision before copying an algorithm, and review license/security independently.
