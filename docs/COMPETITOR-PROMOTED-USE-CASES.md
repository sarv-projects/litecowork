# Competitor-Promoted Use Cases — USER Test Catalog

**Status:** research-backed test intentions; not proof rivals reliably deliver them  
**Updated:** 2026-11-01  
**Purpose:** collect **exact promoted journeys** from rivals for parity USER testing on LiteCowork. Map each to a stable ID; execution evidence is [ACCEPTANCE-AND-E2E-PLAN.md](ACCEPTANCE-AND-E2E-PLAN.md) and [LITECOWORK-BUILD-TASKS.md](LITECOWORK-BUILD-TASKS.md).

Sources: [COMPETITIVE-RESEARCH.md](COMPETITIVE-RESEARCH.md), vendor pages cited there, pinned repos in [REFERENCE-IMPLEMENTATIONS.md](REFERENCE-IMPLEMENTATIONS.md) and [DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md](DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md). Re-verify promotions before locking release tests.

## How to use

| Column | Meaning |
|---|---|
| **UC-ID** | Stable id for LiteCowork USER/E2E matrix |
| **Promoted by** | Product name (marketing/docs) |
| **User journey (as promoted)** | What they tell users they can do |
| **LiteCowork test when** | Master-plan phase / SDD when we should run this on our build |
| **Viability** | See [VIABILITY-VS-COMPETITORS.md](VIABILITY-VS-COMPETITORS.md) |

## Universal chat and composer

| UC-ID | Promoted by | User journey (as promoted) | LiteCowork test when | Notes |
|---|---|---|---|---|
| UC-CHAT-01 | All | New session → ask question → streamed answer → stop | P1 / SDD 001 | Core V1 area 1 |
| UC-CHAT-02 | Claude chat/Cowork, ChatGPT | Same composer for quick Q&A and larger work | P1 | No forced Task wrapper |
| UC-CHAT-03 | OpenWork | Chat home; tool activity compact; no auto side-panel on background result | P7 Workbench | UX policy |
| UC-CHAT-04 | Grok, Dots, Muse | Named persistent assistant optional; ordinary chat without persona | P8 Coworkers | Coworker optional |
| UC-CHAT-05 | ZCode/DSH (ref) | Switch agent/model mid-draft; draft preserved; invalid options cleared | P1 | UIV1-023–036 |
| UC-CHAT-06 | Open Cowork | Rich attachments (files/images) in chat | P2 context | Agent-dependent |
| UC-CHAT-07 | Claude Cowork | Native slash/commands where harness supports | P1 BYO agent | Descriptor-driven |

## Long-running and delegated work

| UC-ID | Promoted by | User journey (as promoted) | LiteCowork test when | Notes |
|---|---|---|---|---|
| UC-WORK-01 | Claude Cowork | Hand multi-file/project; run minutes/hours; review outputs | P4 Task | Durable Task + evidence |
| UC-WORK-02 | ChatGPT Work | Work mode progress; pause on approval-required action | P4/P9 | Approval + Task state |
| UC-WORK-03 | Grok Bot | Long job with notifications, failure recovery, takeover | P7 browser/computer | Lease epochs |
| UC-WORK-04 | OpenAI dots | In progress / Scheduled / Completed activity summary | P8/P9 | Projection only |
| UC-WORK-05 | OpenDot Org | SuperDot plan → user approves → department Dots parallel → review rounds → deliverables | P8/P9 | **Research:** map to Task+Coworker, not Pi sessions |
| UC-WORK-06 | Eigent | Visual multi-agent activity timeline | P4 delegation | Timeline ≠ Core truth |
| UC-WORK-07 | Open Cowork | Trace/tool visibility for debugging | P7 Inspector | Optional technical detail |

## Files, workspace, library

| UC-ID | Promoted by | User journey (as promoted) | LiteCowork test when | Notes |
|---|---|---|---|---|
| UC-FILE-01 | Claude Cowork | Explicit folder grant; read-only / RW / no-delete modes | P2/P4 Environment | ADR Environment |
| UC-FILE-02 | OpenWork | Local project files stay local | P2 roots | WorkspaceRoot |
| UC-FILE-03 | Open Cowork | Create/edit PPTX, DOCX, XLSX, PDF deliverables | P6 Artifacts | Format-qualified |
| UC-FILE-04 | Grok Library | Shared pages/files/links per Bot | P6/P8 | Scoped Coworker library |
| UC-FILE-05 | DSH (ref) | Drag file into session; digest-verified transfer | P2/P6 | ResourceRef pattern |
| UC-FILE-06 | ZCode (ref) | Workspace identity ≠ path; remote attachment stale offline | P2/P3 | Concurrency tests |

## Browser, terminal, computer

| UC-ID | Promoted by | User journey (as promoted) | LiteCowork test when | Notes |
|---|---|---|---|---|
| UC-BRW-01 | Grok, Dots, Muse, Open Cowork | Agent uses browser; user Take over / Return | P7 | Control epoch |
| UC-BRW-02 | composio open-dot | Per-dot Chrome profile; live view; captcha handoff | P7 | **Research:** provider qualification |
| UC-BRW-03 | OpenWork | Built-in browser automation in desktop | P7 | Qualified Chrome path |
| UC-BRW-04 | Claude | Local MCP outside VM; remote MCP external | P5 capability | LiteSPM bridge |
| UC-TRM-01 | OpenWork, Eigent, AionUi | Terminal/shell work in product shell | P7 | Environment-bound PTY |
| UC-CMP-01 | Grok, Muse, Manus (vendor) | Desktop/computer use with permission | P7 | OS attestation |

## Connections, apps, MCP, packages

| UC-ID | Promoted by | User journey (as promoted) | LiteCowork test when | Notes |
|---|---|---|---|---|
| UC-CONN-01 | Grok | Connect app mid-chat; resume after sign-in | P5 | Task-time repair |
| UC-CONN-02 | Dots | Connected apps separate from local computer bridge | P5 | Consent boundaries |
| UC-CONN-03 | OpenWork | `search_capabilities` / `execute_capability` lazy discovery | P5 LiteSPM | Progressive discovery |
| UC-CONN-04 | Claude | Connectors + Skills load for work | P5 | Plan before write |
| UC-CONN-05 | OpenDot | Gmail/Calendar/Drive/M365 watchers | P9 Channels | **Research:** per-provider USER |
| UC-CONN-06 | AionUi | Multi-provider API keys / Kimi integration | P2 BYO | Native route matrix |
| UC-MCP-01 | All open desktop | Add MCP server; tool allowlist | P5 | Trust + redaction |

## Routines, schedules, automations

| UC-ID | Promoted by | User journey (as promoted) | LiteCowork test when | Notes |
|---|---|---|---|---|
| UC-AUTO-01 | Grok | Routine: schedule/event; **Test run** (explicit risk) | P9 | Test ≠ prod enable |
| UC-AUTO-02 | Grok | Pause/resume/edit/delete routine; run history | P9 | Occurrence identity |
| UC-AUTO-03 | ChatGPT | Scheduled tasks (one-time/recurring) | P9 | TZ/DST |
| UC-AUTO-04 | Kimi Work | Scheduled tasks on desktop (vendor) | P9 | Missed trigger honesty |
| UC-AUTO-05 | Claude Cowork | Scheduled / on-demand Cowork runs | P9 | |
| UC-AUTO-06 | OpenDot | Watchers + schedules + webhooks wake Dots | P9 | Sleep/wake catch-up |
| UC-AUTO-07 | composio open-dot | Cron routine posts to chat + notification | P9 | |
| UC-AUTO-08 | OpenWork | Scheduled automations in desktop | P9 | |

## Memory, knowledge, proactivity

| UC-ID | Promoted by | User journey (as promoted) | LiteCowork test when | Notes |
|---|---|---|---|---|
| UC-MEM-01 | Dots | Persistent memory; custom rules; pause/reset identity | P8 | Per-record revoke |
| UC-MEM-02 | Grok | Bot role memory; Library | P8 | Scoped |
| UC-MEM-03 | Muse | Goals + Ideas + proactive suggestions (non-authoritative) | P8 | ADR-0024 area |
| UC-MEM-04 | OpenDot | ~/.opendot plain files; PII masking before cloud | P8 SECURITY | Masking ≠ LiteCowork default yet |
| UC-MEM-05 | Grok | Primary Bot proactivity offers work | P8 | Suggestion ≠ grant |

## Skills, teach, organisation

| UC-ID | Promoted by | User journey (as promoted) | LiteCowork test when | Notes |
|---|---|---|---|---|
| UC-SKL-01 | Grok | Teach demonstrated browser workflow → draft skill | **Research gate** | Not V1 locked |
| UC-SKL-02 | OpenDot | Built-in playbooks per department; user edits copy | P8 Routines | |
| UC-SKL-03 | Goose (ref) | Recipes/headless skills | P8 | Skill vs Routine |
| UC-ORG-01 | OpenDot | Dot Links: inter-Dot messaging with rate limits + approval | P8 | **Research:** policy engine |

## Voice, mobile, cloud-only (parity excluded or V2)

| UC-ID | Promoted by | User journey (as promoted) | LiteCowork test when | Notes |
|---|---|---|---|---|
| UC-CLOUD-01 | Dots, Grok | Work continues when laptop closed (cloud computer) | **V2 / out of scope for local V1 parity** | See viability doc |
| UC-CLOUD-02 | ChatGPT Work | Continue same conversation web/mobile with local sync | **V2** | |
| UC-CLOUD-03 | Muse | WhatsApp/mobile continuation | **V2** | |
| UC-VOICE-01 | composio open-dot | Voice call; continues as chat after hangup | **Research** | Not in V1 release contract row |
| UC-VOICE-02 | CopilotKit/OpenDots | Voice + Slack channels | **Research** | Channels ADR |

## Settings and trust (cross-cutting USER cases)

| UC-ID | Promoted by | User journey (as promoted) | LiteCowork test when | Notes |
|---|---|---|---|---|
| UC-SET-01 | All | Change appearance/language without breaking auth | P11 shell | LOCAL_PREF |
| UC-SET-02 | Claude | High-level approval for consequential actions | P4/P5 Trust | Not shell spam |
| UC-SET-03 | OpenDot | Approval before send/delete/pay | P5 | Effect + Approval |
| UC-SET-04 | LiteCowork spec | Every Settings control maps to served op or honest unavailable | P11 | [SETTINGS-AND-SHELL-SURFACE-AUDIT.md](SETTINGS-AND-SHELL-SURFACE-AUDIT.md) |
| UC-SET-05 | Grok | Per-Bot notification switches | P8/P9 | |

## Reference desktop apps (engineering test prompts, not rival products)

Translate upstream E2E themes from pinned **ZCode** and **DeepSeek Harness** ([DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md](DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md)) into LiteCowork USER cases when P1/P7 qualify: conversation session resume, permission prompts, quit-with-active-work UNKNOWN, plugin/guest CSP rejection, file drag digest, update drain ordering.

---

**Total catalog:** 50+ UC-IDs. Extend this file when a rival promotion is verified; link new IDs in SDD acceptance and [LITECOWORK-BUILD-TASKS.md](LITECOWORK-BUILD-TASKS.md).
