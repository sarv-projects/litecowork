# Contradiction and Alignment Register

**Updated:** 2026-11-01  
**Status:** living register. This is the contradiction matrix requested in [CURRENT-RUN.md](../CURRENT-RUN.md) §5. It records conflicts that were checked against owner authority. It is not a claim that every paragraph of the archive was reread in this pass.

Authority order: [docs/README.md](README.md) and root [ARCHITECTURE.md](../ARCHITECTURE.md).

## Resolved in prior reconciliation (do not reopen without new evidence)

| Topic | Conflict | Authority | Resolution |
|---|---|---|---|
| 001 B-01–B-10 | Feature 001 blocking decisions: concurrency, first harness, handshake, atomic result write, stream gaps, context reduction, process lifecycle, usage attribution | ARCHITECTURE, STATE-MACHINES, STORAGE, AGENT-CONTROL | **RESOLVED & LOCKED in Feature 001 (Rev 1.0)**: B-01 serialized turn lane (409 Conflict); B-02 Windows MSVC + Claude Code / Codex CLI with native auth pass-through; B-03 3-phase session handshake; B-04 `commit_conversation_agent_response` atomic SQLite transaction; B-05 best-effort transient streaming with REST resync; B-06 UserRequest continuation; B-07 sliding-window context reduction with audit header; B-08 dual-mode resource materialization; B-09 Job Object/cgroup with 10-min idle TTL; B-10 tiered usage attribution. Feature 001 is now ACCEPTED/LOCKED. |
| Conversation activation vs Send | Storage activation existed; Send looked available | NATIVE-COMPOSER-LLD, feature 001 | Activation is not Send. Send stays fail-closed until 001 is locked and served. |
| Native handles | Risk of putting handles in events | AGENT-CONTROL, STORAGE, EVENTS | Runtime-local binding only. Excluded from events, snapshots and public API. |
| Workspace root routes | OpenAPI describes POST/PATCH workspace-roots; handlers were not mounted | API.md comments plus OpenAPI | Target-only until feature 003 is locked and a handler is served. Feature 003 states this explicitly. |
| ZCode / DeepSeek as architecture | Desktop shells are Electron | ARCHITECTURE, DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT | Inspiration only. LiteCowork keeps Tauri plus `litecoworkd`. |
| Evidence words | Release contract uses TARGET_ONLY / CODE_IMPLEMENTED / INTEGRATION_PROVEN; AGENTS.md uses TARGET / CODE / INTEGRATED | Both are intentional vocabularies | Map them: TARGET_ONLY→TARGET, CODE_IMPLEMENTED→CODE, INTEGRATION_PROVEN→INTEGRATED, QUALIFIED→QUALIFIED. Do not invent a third scale. |

## Still open (owner must decide; SDDs name them)

| ID | Topic | Why it is not silently closed |
|---|---|---|
| 002 B-11–B-12 | Agent matrix beyond the first route; descriptor TTL | AGENT-CONTROL allows per-module policy |
| 003 B-13–B-14 | First ingestion formats; watcher vs rescan | WORLD-RESOURCES watcher is target-only |
| 004 B-15–B-16 | Portable context bound; second module | Depends on 002 |
| 005 B-17–B-18 | Environment provider; one edit class | Worktree is not a sandbox |
| 006 B-19–B-20 | Golden provider; one write operation | CONNECTORS-WORKFLOWS leaves the first account open |
| 007 B-21 | One professional format | Preview is not editing |
| 008 B-22–B-23 | Browser mode; computer-use in the same milestone | WebView is not Chrome |
| 009 B-24–B-25 | Minimal memory record; Quick Create binding | COWORKERS-TARGET schema additions are proposed |
| 010 B-26–B-27 | First trigger; local notification | Public webhooks are out of the slice |
| 011 B-28–B-29 | GA OS; which settings groups may be enabled | 187 controls are not served |

## Semantic alignment

[SEMANTIC-SERVED-UI-MAP.md](SEMANTIC-SERVED-UI-MAP.md) now has one row per settings control (187) and one row per shell control in the action ledger (36). Status is LOCAL_ONLY, NATIVE, or TARGET. **No operationId is marked SERVED**, because this checkout has no active daemon to inventory. That is the remaining BT-0.4 work, not a missing row.

## Corrected in the feature specs on 2026-11-01

These were false in the DRAFT specs and are now edited to match STATE-MACHINES, API, FLOWS, and the owner docs read in full that day.

| Spec | What was wrong | Correction now in the spec |
|---|---|---|
| 001 AC-018 | Readiness was required before creating the AgentSession | Acceptance allows a STARTING reservation. RUNNING is committed only with readiness |
| 001 §7.5 | 256 KiB treated as the adapter budget | 256 KiB is the UserRequest detail prompt cap. The package cap is 1,024 messages / 4 MiB. `CONVERSATION_CONTEXT_LIMIT_EXCEEDED` is not in the 131-code registry |
| 003 AC-003 | Every identity failure marks the root UNAVAILABLE | ACTIVE becomes UNAVAILABLE. PAUSED stays PAUSED with location UNAVAILABLE. REVOKED is not reopened |
| 004 §7 | Return to agent A could reuse a qualified session | Every admitted turn gets a fresh Conversation-scoped AgentSession |
| 005 | Omitted failover meant DISABLED | Omission copies the Coworker revision when one exists. DISABLED is only the fallback when that default is absent |
| 008 §7 | Takeover kept the same epoch | Lease stays ACTIVE. Control moves to HUMAN epoch n+1. The old epoch is rejected |
| 009 | Archive only blocked proactive work | PAUSED blocks proactive work and still allows an explicit owner Task. ARCHIVED cannot be a new Task origin |
| 011 AC-005 | Not-ready blocked every domain mutation | DEGRADED plus Operator SERVING still allows ordinary commands. Execution readiness stays closed |

OpenAPI `Connection.status` omits `REVOKED`, which CONNECTORS-WORKFLOWS includes. `runtime.availability.changed.v1` allows only PAIRING, ONLINE, DEGRADED, DRAINING, OFFLINE, and REVOKED, while the lifecycle view availability enum also includes STARTING and RECOVERING. Those promotions stay with the schema owner. Spec 001 has no "Owner facts incorporated" heading; that heading is on 002–011 only. Spec 001 §10 now uses 256 KiB only as the UserRequest detail prompt cap.

## Machine contracts previously checked (2026-10-31)

Recorded in CURRENT-RUN §13 and not re-executed as a schema mutation in this pass:

- OpenAPI 229 operationIds aligned with API.md including two explicit target-only workspace-root operations.
- 148 event types aligned between EVENTS.md and domain-event.schema.json.
- 131 error codes aligned between SCHEMAS.md and error-codes.schema.json.
- 28 SQLite migrations replayed in order on that date.

A future edit that adds an event, error or route must update the owner prose and the machine schema together, then rerun those checks.
