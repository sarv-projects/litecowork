# Desktop UI delivery and interaction acceptance

Owners: EXPERIENCE, DESIGN-SYSTEM, MOTION, and [PRESENTATION-RUNTIME](../docs/PRESENTATION-RUNTIME.md). Implement tokens/components once, then
vertical screens backed by actual projections. E02 establishes shell; E05 workforce;
E06 knowledge; E08 full product UX; E09 responsibility; E11 devices/channels.

| Surface | User outcome | Required states and controls | Owner stories |
|---|---|---|---|
| First run / Workspace | Start useful local work | No agent/auth, choose root, neutral default Coworker (rename optional), opt-in worker assignment, local default, import failure | E02-S01/S02, E08-S01 |
| Home / composer | Ask Alex to handle work | Composer, nonzero Needs You, max4 active, max3 upcoming, max1 idea, max4 outputs; advanced lead/model/location/tools | E08-S01/S05 |
| Conversation | Chat and promote Task | Send/stream/stop/retry, UserRequest/sign-in, scope attachments, offline reconnect; chat no Task | E03-S02/S03 |
| Work / Live Desk | See work and steer | Outcome/status stays visible; no current plan is described as proof work never started; timestamps, exact pinned inputs, and persisted Plan Steps live in collapsed Work details; planning/working/waiting/verifying/failed/paused/stopping/done only from projections; accepted Steps, actual workers and outputs | E03-S03/S04, E08-S04 |
| Needs You | Resolve concrete blocker | Approval review/deny, sign-in, question, conflict, device needed; stale/resolved/expired and deep-link | E02-S04, E04-S01 |
| Agents / Main / Subagents / Installed | Choose faithful workforce | Installed vs connected vs lead eligible; disabled profiles; unsupported option; Enabled but unassigned; revision conflict | E03-S01, E05-S01 |
| Knowledge / context | Ground work in sources | File/folder/ZIP preview, upload/parse/index, partial errors, source open/citation, stale/revoked/deleting, scope | E06-S05, E08-S03 |
| Coworker | Maintain identity/context/defaults | Create/revise/pause/archive, primary guard, worker allowlist, interaction defaults not grants, execution availability | E08-S01 |
| Goals / Ideas | Track intent and useful proposals | Evidence-backed progress, pause/reopen/archive, why/source, accept/dismiss/snooze/mute/expired | E08-S02 |
| Library / Workbench | Use finished outputs | Document/table/office/PDF/image/code/dashboard preview; provenance/version, edit dirty/conflict, compare and restore-as-new-version; unsupported renderer fallback | E08-S07, E07-S04 |
| Presentation Runtime | Understand results at the right detail level | Typed safe items; truthful activity; transient stream/reconnect; plain-text fallback; context-used sources; technical detail in Inspector | E08-S06 |
| Browser / computer | Inspect or take control when needed | Hidden by default, preview/fullscreen, Agent/You control epoch, taking/returning/error, fresh observation | E07-S02/S05 |
| Automations / Runs | Delegate repeat responsibility | Define-preview-test-enable, typed inputs/schedule/timezone/authority/cost, pause/misfire/waiting/health/drift | E09-S01/S02/S03 |
| Discover | Add capabilities safely | Compatibility, permissions, installation/auth/health; no metadata as authority; MCP Apps isolated | E10-S01 |
| Devices / connections | Use local/cloud/remote | Pair/revoke/presence/capability; unavailable local dependency; sync/placement; channel assurance/auth error | E11, E12 |
| Inspector / settings | Diagnose without clutter | Attempt/session/env/fence/effect/evidence/cost events, safe diagnostics, privacy/startup/notification/accessibility | E08-S04/S05, E13 |

## Component acceptance

CoworkerCard/PresenceBadge, worker profile card/tree/status, Cost/Quota indicators,
SuggestionCard/GoalCard, RoutineHealth, control badge and Artifact viewer use design tokens
and semantic icon/text labels. No color-only state, no persistent animated avatar.
Live regions announce meaningful state changes rather than each streamed token.

Every surface has empty/loading/partial/error/offline/permission states; inspect cached
state with a timestamp and prevent authority-requiring offline optimistic actions. Only
reversible local display preferences are optimistic by default. Creation/status needs
committed response/event before it is shown as fact.

## Motion tests

Durations remain 80/120/180/280ms from MOTION. Profile enable changes toggle only; child
branch appears after child Attempt creation; Working after active session; output shelf
after ArtifactVersion; Verified after actual verifier result; handoff shows checkpoint/new
placement, not process migration. Takeover badge follows committed control epoch.
Interrupt/reverse animations retain latest projection and keyboard focus. Reduced motion
removes spatial movement/pulses; equivalent state remains readable. Fake percentages,
ETAs, active browser cursors and endless shimmer are prohibited.

## UI test matrix

Component assertions + API projection fixtures + full frontend journey + actual native
app/daemon test. Use Playwright for browser-compatible surfaces; native driver/manual
owner evidence where OS webview tooling cannot automate a path. Test keyboard-only task,
approval, profile edit and takeover; screen reader announcements; focus restore; 200/400%
zoom; large trees/long titles; light/dark/high contrast; reduced motion; connection loss;
late/out-of-order events; duplicate/reordered transient deltas; cursor resync; renderer
fallback; source revocation while open; dirty Artifact conflict; version restore as append;
context disclosure provenance; Task `Work details` starts collapsed, expands only validated
persisted state, and never fabricates execution state; user closes app while work continues. Include one
nontechnical task that never opens Inspector and one coding task that uses diff, worker,
source and evidence detail. Passing Chromium tests alone does not qualify Tauri/macOS/Windows
rendering or OS integration.
