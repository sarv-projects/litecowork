# V1 Usage / Agent Discovery / Memory / Terminal Design-to-Test Handoff

Status: TARGET implementation/qualification tests. TLM-001 onward is a NEW, non-overlapping test namespace; it never replaces canonical U01–U20 or supplementary UIV1-001–158. Every test requires proof of CODE (exact unit/contract), SYSTEM (real OS/agent when necessary) and USER (actual visible state).

## Owning epics

| Feature | HLD/domain owner | Detailed LLD | Primary stories | Source status |
|---|---|---|---|---|
| Usage, logs, cost, latency, cache, health, resilience | OBSERVABILITY, STORAGE, EVENTS, TRUST, TASK-RUNTIME | docs/V1-TELEMETRY-USAGE-RESILIENCE-LLD.md | E04/E05/E08/E13 | Target; no API or persisted table added |
| Agent discovery, icons and setup | AGENT-CONTROL, AGENT-FABRIC | docs/V1-AGENT-DISCOVERY-AND-CATALOG-LLD.md | E03/E08 | Target; per-harness real probe pending |
| Agent memory, search, retrieval, injection | CONTEXT, LEARNING, RESPONSIBILITIES | docs/V1-AGENT-MEMORY-FAST-PATH-LLD.md | E04/E08/E10 | Target; retrieval provider must qualify |
| Terminal, shell profiles and process/session ownership | ENVIRONMENTS, RUNTIME-LIFECYCLE, TRUST | docs/V1-INTEGRATED-TERMINAL-LLD.md | E07/E08/E13 | Target; PTY/OS source qualification pending |
| Visual references and safe borrowing | REFERENCE-IMPLEMENTATIONS, DESIGN-SYSTEM | docs/V1-REFERENCE-OMNIROUTE-OMNIRUSH-TERMINAL.md | E02/E03/E08 | Research; no copied assets or code |

## Usage and observability acceptance (TLM-001 through TLM-012)

- TLM-001: A Task includes lead + 2 subagents; each actual leaf inference appears once in Task, Session and global views, with correct agent provenance.
- TLM-002: One native+gateway report describes the same provider request; dedupe requires verified ID mapping, not prompt similarity; conflicting reports show discrepancy.
- TLM-003: A native subscription without token receipt shows UNKNOWN, never 0 tokens or cost $0.00, and has coverage indicator.
- TLM-004: Cache read/write/reasoning token subsets do not inflate input/output totals; partial source omits unavailable counters.
- TLM-005: Billed cost vs estimated cost, subscription vs metered cost and price snapshot/version are visually distinct.
- TLM-006: Two currencies and model price changes across dates remain separate; half-open timezone windows with DST do not double count.
- TLM-007: Request latency phases expose control-plane admission vs native startup vs upstream time and real percentiles with sample counts.
- TLM-008: Provider service failure, account authentication failure, model-specific unavailable and LiteSPM outage yield distinct scope-specific health/repair.
- TLM-009: API/IPC stream disconnect + retry does not replay settled external Effects; old cursor/frames deduped.
- TLM-010: Logs/diagnostic/support export redact memory text, authorization headers, native stdout secrets and URLs; audit retention protected.
- TLM-011: Slow/disk-full telemetry sink does not corrupt Task admission/outbox, spans bounded and backpressure explicitly visible.
- TLM-012: Clearing local derived cache preserves immutable domain evidence and revocation prevents later stale cache reuse.

## Agent discovery / cards / native fidelity (TLM-013 through TLM-023)

- TLM-013: On Windows, macOS, Linux and WSL Runtime, PATH and recognized package installations produce correct source-qualified candidate list.
- TLM-014: WSL installed binary is not incorrectly presented as runnable in Windows Runtime; multiple copies have distinct Runtime identities.
- TLM-015: Detected + installed + not-configured + sign-in-required is rendered precisely and not labeled READY.
- TLM-016: Missing path displays install guide, Retry Detection and Browse Existing Path; file-picker path still runs permission/identity/version checks.
- TLM-017: Executable moved/removed or config/Runtime changed invalidates stale registry cache without leaking full file paths in logs.
- TLM-018: Timeout on one CLI probe does not prevent other agent cards from loading or freeze shell UI.
- TLM-019: Each supported agent renders trusted bundled logo or neutral fallback, license/attribution, keyboard labels and accurate feature description offline.
- TLM-020: Agent installed status never inferred from an OAuth provider/gateway account, ACP metadata or model catalog.
- TLM-021: Installing/updating preserves native user config and old working agent until qualified replacement; failed update has rollback path.
- TLM-022: Native auth, model/mode/effort, slash/@ and session readiness qualified separately, before Send enabled.
- TLM-023: Agent defaults and active Attempt/Session cannot be silently changed while user edits a different Workspace binding.

## Memory agent control and latency (TLM-024 through TLM-034)

- TLM-024: Agent chooses not to search memory; no unapproved forced injection or background retrieval, native memory untouched.
- TLM-025: Authorized memory search/fetch via qualified native tool returns source/provenance and scoped ContentUsed receipt.
- TLM-026: Warm policy/cache local path timing shown separately from whole IPC/provider retrieval; no false sub-ms absolute promise.
- TLM-027: Cross-Coworker or Workspace retrieval and cache leakage blocked under same query with different scopes.
- TLM-028: Source revoked while remote retrieval in-flight: final-use gate refuses hit, and old native session is fenced.
- TLM-029: Source deleted, index unavailable, provider timeout and true no result have distinct UI/agent statuses.
- TLM-030: A lower-priority memory cannot override accepted TaskSpec or current user instructions and cannot mint a grant.
- TLM-031: Temporary/private conversation does not queue extraction, persist sensitive cache or share Coworker memory.
- TLM-032: Generated candidate from untrusted web/email/tool text does not auto-commit; contradictory confirmed claims require review.
- TLM-033: Memory delete/purge saga reports pending provider cleanup honestly; historical exposed content cannot be recalled.
- TLM-034: Cache revision keys and token budgets bound provider calls, p95 under concurrency measured on target OS.

## Terminal and right Workbench (TLM-035 through TLM-048)

- TLM-035: Profile picker enumerates installed pwsh/PowerShell/cmd/Git Bash/WSL on Windows and bash/zsh/fish etc on Linux/macOS only when available.
- TLM-036: User-defined executable path requires native picker and exact qualification; cannot execute arbitrary path from registry/browser.
- TLM-037: New terminal opens scoped Runtime/Environment/CWD; authorization disallows outside-Workspace root escapes.
- TLM-038: Multiple tabs/splits, resize/maximize and Browser/Artifact pane switching preserve correct PTY and scrollback without starting duplicate shells.
- TLM-039: Manual terminal cannot silently become an Agent Attempt or produce verified Task Evidence; agent-owned terminals enforce input control lease.
- TLM-040: Two desktop windows attached to one PTY: one authorized stdin controller; other reader cannot inject.
- TLM-041: Take/Return control fences old agent command sequences and refreshes execution/authorization epoch.
- TLM-042: PowerShell/Git Bash/WSL shell integration quality NONE/BASIC/RICH reflects real command boundary capability; no fake exit code.
- TLM-043: Untrusted OSC/CSI/DCS/hyperlink/clipboard sequences and terminal output secrets cannot invoke host actions or leak to telemetry.
- TLM-044: Large/multiline paste warning, Unicode/IME, accessible text, high contrast and 400% zoom tested.
- TLM-045: Closing Workbench view does not kill terminal/Task without explicit retention policy; Stop Process shows reviewed impact and PENDING.
- TLM-046: Runtime/WSL/PTY crash reports LOST and may reopen a new terminal only with explicit user action, never fake process resume.
- TLM-047: Terminal process/PTY close, output frame gaps, resize ordering and backpressure are bounded and observable.
- TLM-048: Supported Windows, Ubuntu and macOS terminal platform conformance recorded; unsupported shell variants display reason.

## Promotion and release checklist

1. Confirm which exact canonical Operator method/services or Tauri-local actions implement the new projections; **do not assume candidate browser/Terminal/Telemetry routes exist**. Add typed Auth/Trust, OpenAPI schema, events, SQLite migration, and worker adapter in the owning vertical slice.
2. For each leaf usage field, pin provider/native/gateway provenance, token accounting math, currency pricebook, confidence and dedup identity.
3. Source-bound interop coverage: versions of AgentModule native binaries, desktop OS, LiteSPM connection, optional gateway, shell PTY and model catalog. No mock-only parity claim.
4. UI cards/controls are enabled only for served and qualified operations. Empty analytics/Memory/Terminal states report absence versus UNKNOWN/NO_ACCESS.
5. Include performance/cost baseline and security/privacy/resilience measurement from real hardware. Only after CODE+SYSTEM+USER gates may a test be marked PASS.
