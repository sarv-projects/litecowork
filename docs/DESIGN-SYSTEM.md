# Design System

This is the v1 visual and component contract for LiteCowork. It defines a calm neutral
operator workspace: readable surfaces, one cobalt action color, status colors with
accessible text labels, and no decorative activity that could be mistaken for work.
Framework and component-library choice remain implementation details; app components
must implement the anatomy, tokens, states, and accessibility behavior defined here.

## Design principles

- Keep the conversation and requested outcome visually primary; protocol and provider
  details stay in Inspector/Settings.
- Show the actual Workspace, AgentBinding, Runtime, Environment, resource location, and
  control owner wherever they affect a decision.
- Use surface, typography, icon, and label together to distinguish status; never depend
  on color alone.
- Prefer stable layout and short state transitions over continuous motion.
- Use bundled/system fonts and local assets. The desktop shell must not fetch fonts or
  component code from a third-party CDN at runtime.

## Color tokens

The v1 default is light, with system/light/dark selection. The dark theme preserves the
same semantic assignments. Raw palette values are primitive tokens; product meaning is
assigned in semantic tokens; components consume only semantic/component tokens.

```css
/* Primitive palette */
:root {
  color-scheme: light;
  --lc-white: #FFFFFF;
  --lc-ink: #171D27;
  --lc-neutral-50: #F6F8FB;
  --lc-neutral-100: #EFF2F6;
  --lc-neutral-200: #DCE2EA;
  --lc-neutral-300: #C5CDD8;
  --lc-neutral-400: #8A95A5;
  --lc-neutral-500: #687385;
  --lc-neutral-600: #515C6C;
  --lc-neutral-800: #262E3A;
  --lc-neutral-900: #171D27;
  --lc-blue-100: #EAF0FF;
  --lc-blue-600: #2457D6;
  --lc-blue-700: #1C45AD;
  --lc-green-100: #E6F5EB;
  --lc-green-700: #145C3B;
  --lc-amber-100: #FFF4CE;
  --lc-amber-800: #694900;
  --lc-red-100: #FDECEC;
  --lc-red-700: #A12D2D;
  --lc-info-100: #E8F2FF;
  --lc-info-700: #1556A2;
}

/* Semantic light theme */
:root {
  --color-background: var(--lc-neutral-50);
  --color-surface: var(--lc-white);
  --color-surface-raised: var(--lc-white);
  --color-text: var(--lc-ink);
  --color-text-muted: var(--lc-neutral-600);
  --color-text-subtle: var(--lc-neutral-500);
  --color-border: var(--lc-neutral-200);
  --color-border-strong: var(--lc-neutral-300);
  --color-primary: var(--lc-blue-600);
  --color-primary-hover: var(--lc-blue-700);
  --color-primary-soft: var(--lc-blue-100);
  --color-focus: var(--lc-blue-600);
  --color-info: var(--lc-info-700);
  --color-info-soft: var(--lc-info-100);
  --color-success: var(--lc-green-700);
  --color-success-soft: var(--lc-green-100);
  --color-warning: var(--lc-amber-800);
  --color-warning-soft: var(--lc-amber-100);
  --color-danger: var(--lc-red-700);
  --color-danger-soft: var(--lc-red-100);
}

.theme-dark {
  color-scheme: dark;
  --color-background: #11151C;
  --color-surface: #1A2029;
  --color-surface-raised: #222A35;
  --color-text: #EEF2F7;
  --color-text-muted: #A9B3C2;
  --color-text-subtle: #8995A5;
  --color-border: #394453;
  --color-border-strong: #526072;
  --color-primary: #ADC2FF;
  --color-primary-hover: #C4D2FF;
  --color-primary-soft: #202F51;
  --color-focus: #ADC2FF;
  --color-info: #9EC8FF;
  --color-info-soft: #1A314A;
  --color-success: #8AD1A4;
  --color-success-soft: #173724;
  --color-warning: #FFD980;
  --color-warning-soft: #423516;
  --color-danger: #FFB1AC;
  --color-danger-soft: #482423;
}
```

Text contrast for the selected foreground/background pairs is at least 6.1:1 in the
light palette and 7.3:1 in the dark status palette; body text must meet WCAG 2.2 AA
(4.5:1), large text and meaningful UI boundaries must meet 3:1. A theme change must
re-run contrast checks for every semantic pair. Do not use opacity to create muted text
without rechecking contrast.

Status assignments:

| Meaning | Semantic tokens | Required non-color cue |
|---|---|---|
| Active/selected | `color-primary`, `color-primary-soft` | Selected label/icon or control state |
| Informational | `color-info`, `color-info-soft` | Info icon and concise text |
| Success/verified | `color-success`, `color-success-soft` | “Completed” or “Verified” plus check icon |
| Warning/needs user | `color-warning`, `color-warning-soft` | “Needs you”/“Waiting” label and action |
| Failure/destructive | `color-danger`, `color-danger-soft` | Error label and recovery explanation |
| Offline/unknown | `color-text-muted`, `color-border` | “Offline”/“Unknown” label; never imply failure or absence |

`REPORTED`, `OBSERVED`, and `VERIFIED` use distinct labels and icons. Only VERIFIED uses
the success/check treatment. An agent's completion claim cannot render as verified.

## Type, space, shape, and elevation

```css
:root {
  --font-ui: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  --font-code: ui-monospace, "SFMono-Regular", Consolas, monospace;
  --text-xs: 0.75rem;    /* 12px */
  --text-sm: 0.875rem;  /* 14px */
  --text-md: 1rem;      /* 16px */
  --text-lg: 1.25rem;   /* 20px */
  --text-xl: 1.5rem;    /* 24px */
  --text-display: 2rem; /* 32px */
  --space-1: 0.25rem;
  --space-2: 0.5rem;
  --space-3: 0.75rem;
  --space-4: 1rem;
  --space-5: 1.25rem;
  --space-6: 1.5rem;
  --space-8: 2rem;
  --space-10: 2.5rem;
  --space-12: 3rem;
  --radius-control: 0.5rem;
  --radius-card: 0.75rem;
  --radius-panel: 1rem;
  --radius-pill: 999px;
  --shadow-card: 0 1px 2px rgb(18 28 45 / 0.06);
  --shadow-overlay: 0 12px 32px rgb(18 28 45 / 0.16);
  --ring-width: 2px;
  --ring-offset: 2px;
}
```

Body text is 16px/1.5; secondary labels may use 14px/1.4; metadata may use 12px/1.4
only when nonessential information remains legible. Use 20–24px for section headings and
32px for the Home prompt/title. Use sentence case, short labels, and tabular numerals for
timestamps, byte sizes, usage, and version identifiers. Code and opaque identifiers use
the code font and can wrap or copy; they must not force horizontal page scrolling.

Spacing uses a 4px base. Component internals use 4/8/12/16px; cards and sections use
20/24/32px. Use 8px control radius, 12px cards, 16px major panels, and pill radius only
for compact status badges. Shadows separate overlays from the page; ordinary cards use
border + subtle shadow, not elevation alone.

## Component contract

All component colors reference semantic tokens. Component-specific tokens may alias
semantic tokens but may not introduce hard-coded hex values in component styles.

| Component | V1 anatomy/size | Required states and behavior |
|---|---|---|
| Primary button | 40px high desktop, 44px touch; 16px horizontal padding | Default, hover, pressed, visible focus, disabled, loading; loading keeps label and announces busy |
| Secondary/outline button | Same hit target, neutral surface/border | Same states; never visually outranks the primary action |
| Destructive button | Danger semantic colors | Confirmation names the consequence; no destructive action on hover alone |
| Icon button | 40px desktop, 44px touch; 18px icon | Accessible name/tooltip; visible focus; disabled/loading behavior as applicable |
| Text field | 40px desktop, 44px touch; label + control + hint/error | Default, hover, focus, invalid, disabled, loading; error is text and `aria-describedby` |
| Composer | Multiline, grows to 6 lines then scrolls; send button stays visible | Draft remains through admission errors; no eligible AgentBinding shows setup action without clearing text |
| Task/Artifact/Resource card | 16px padding; title, status, one-line context, actions | Focusable only when interactive; freshness/location and status remain textual |
| Blocker/UserRequest card | Status icon, safe message, resolution hint, explicit action/expiry | UserRequest answer is never styled as Approval; stale/expired request disables submission |
| Approval card | Exact operation, target, scope, assurance, expiry, approve/deny | Dangerous action is never preselected; replay/stale approval asks for a fresh decision |
| Status badge | 12px label + icon; 20–24px high | Text always names status; no color-only badge |
| Dialog/sheet | Title, short consequence, scrollable body, explicit footer | Focus trap, Escape behavior appropriate to action, initial focus, return focus, destructive confirm |
| Timeline row | Timestamp, actor/source, event text, optional linked resource | History order follows projection cursor; old events do not replay animations |
| Coworker card | Identity/name, derived presence, active work count, one next useful action | Presence comes from real state; paused/offline are explicit; no avatar activity theater |
| Worker profile card | Profile name, parent AgentBinding, short routing description, readiness, cost/quality freshness | Enabled means eligible for future selection only; current Attempt status is separate |
| Delegation tree | Nested accessible list of lead, host children, reported native children, and capabilities | Child exists only after Attempt creation; each row opens pinned profile/Attempt details |
| Cost indicator | Value + unit/currency + source + confidence + observed time | Exact/estimated/unknown states; never render unknown as zero |
| Quota indicator | NORMAL/LOW/EXHAUSTED/UNKNOWN label and observation age | No percent gauge unless provider supplies a valid quantified observation |
| Goal card | Objective, owner-authored status, linked verified outcomes, stale/conflict count | Progress links to Task/Evidence; worker summaries cannot complete Goal |
| Suggestion card | Reason, source refs, intended result, authority preview, expiry, actions | Accept is an explicit proposal transition and never grants authority |
| Control owner badge | Agent/Human owner, Environment name, input-control epoch status | Separate from ExecutionLease; stale owner actions are disabled with reason |
| Profile editor group | Routing, native options, instructions, enforced limits, budget, environment | Guidance and enforceable policy have separate headings and validation states |
| Presentation item | Typed content, source identity, freshness, status, accessible label | Stable source ordering; safe text/download fallback; no arbitrary HTML or authority in payload |
| Context-used disclosure | Source groups, scope, exact revision/freshness, retrieval limitation | Read-only; never claims all history/resources were read; opens authorized source only |
| Pinned Task Resource preview | Native disclosure, exact Resource revision, escaped text preview or typed unavailable reason | Lazy read only after open; text-only and capped at 1 MiB; never substitutes the current head or grants agent access |
| Resource revision comparison | Two explicit same-Resource revision selectors and labeled side-by-side escaped text panes | Only committed `text/plain`/Markdown revisions up to 1 MiB each; exact pins; unsupported/unavailable comparison preserves the surrounding history/editor; no HTML/SVG renderer and no changed-line claim |
| Artifact version panel | Current version, version list, source Task/author, verification, compare/restore/Save As actions | Save As is a native owner action for exact managed versions up to 10 MiB; cancel writes nothing; restore appends a new version; stale publish preserves draft and requires explicit resolution |
| Activity summary | Plain-language current action, observed time, expandable details | Shows actual projection/observation only; stale/offline state textual; no fake percentage/ETA |
| Task outcome/activity panel | Saved objective/status, committed output records, short activity preview, collapsed sources | Snapshot freshness is labeled; output is not a verified outcome unless a VerificationRun says so; count reflects presentation items, not presumed Steps |
| Task specification history | Lazy read-only revision list with objective, author, time, and parent revisions | Exact Task/Workspace identity only; stale cached history is labeled offline; no restore control |
| Markdown preview | Heading/body/list/quote/code hierarchy; explicit external-link affordance | Bounded supported syntax only; unsupported/malformed input becomes escaped source text; no active HTML or implicit remote assets |

The current Markdown preview is a subset, not a full renderer. External HTTPS links display
their destination in a confirmation before leaving the Workbench. Raw content remains
available; tables and other unsupported syntax use the plain-text fallback rather than a
partially interpreted view.

Button state priority is disabled, loading, pressed, focus, hover, default. Hover and
pressed change surface shade/border only; do not move important content. Focus uses a
2px semantic focus ring with 2px surface offset. Disabled controls remain readable and
explain a blocking prerequisite nearby; do not rely on opacity alone. Every async control
announces `aria-busy`; invalid inputs use `aria-invalid` and a linked error message.

## Layout and responsive behavior

- Desktop app shell: 240px expanded navigation rail, 64px collapsed rail; content min-width
  560px; Inspector 320px; Workbench 420–720px. Users can close secondary panels.
- At 900–1279px: navigation may collapse; only one secondary panel is open at once.
- Below 900px: navigation becomes a labeled drawer; Inspector/Workbench become detail
  routes or stacked panels. Conversation and Task actions stay primary.
- Below 600px: single-column layout, full-width composer, bottom-safe-area padding, dialogs
  become full-screen sheets, and all touch targets are at least 44px.
- At 200% zoom, the interface reflows without losing actions or requiring two-dimensional
  scrolling except for inherently tabular/diagram content, which gets an accessible list.

Live Desk workstreams are aligned to outcomes and use neutral lane surfaces; do not assign
permanent colors to Agents or model vendors. Provider/source identities use text/icon.
Workbench preserves the source Artifact title/version and unsaved-edit state at every
viewport.

## Accessibility and motion

- Keyboard order follows visual order; all actions are keyboard operable.
- Focus never disappears behind a sticky header, panel, or modal.
- Status/verification changes use a polite live region; interruptive prompts use an
  appropriate alert/dialog announcement and are throttled to avoid repetitive updates.
- Screen readers receive the full state and action label, not an unexplained icon or color.
- Reduced motion keeps state text, icons, and focus indicators; it removes nonessential
  transforms, pulses, and continuous progress effects.
- Timing and semantic transition rules are owned by [`MOTION.md`](MOTION.md).

## Workspace policy and domain-specific components

`ReplicationPolicySelector` states which content may transfer, which Runtime may receive
it, and that changes apply prospectively. `SELECTED_FOLDERS` requires selected active
WorkspaceRoot IDs and explains that new revisions under those roots remain in scope.
`ArchivedWorkspaceBanner` persists and names available read/download actions;
archive confirmation lists blocking Tasks/Automations.

`PlanningStatus` identifies active lead planning without a fake work lane, Environment,
or progress meter. `AutomationRevisionHistory` distinguishes the current definition from
the immutable revision pinned by each occurrence. Resource cards separate Resource
identity from provider/path location and show freshness per location. Notification rows
distinguish delivery acknowledgement from Task state. `ControlOwnerBadge` names Agent or
Human control and the current input epoch without implying ExecutionLease ownership.

## Background-work components

`ArtifactLibraryActions` names Save to Library and Archive explicitly, confirms the named
Artifact, and keeps status textual. Pending commands show response confirmation separately
from a committed result. Conflicts require refresh/review; unconfirmed responses retain an
unchanged retry action. Archive explains retained history and has no unarchive affordance.

`RecentConversationList` retains stable Conversation identity and current Workspace scope;
keyboard selection reopens the existing exchange. `NeedsYouBadge` exposes a textual pending
count and stale marker; linked blocker/request/approval entries are deduplicated as defined
in EXPERIENCE. UserRequest answers and sensitive approvals remain distinct controls.

`RoutineCard` shows revision, typed input requirements and execution dependencies without
claiming a run is active. `AutomationTriggerList` separates TriggerHost from execution
placement and presents each enabled trigger's timezone/misfire behavior. `OccurrenceRow`
shows due/waiting/started/settled status alongside the actual linked Task outcome.
`RoutineHealthIndicator` shows the current RoutineRevision sample count, terminal outcome
summary, observation-qualified dependencies, and `HEALTHY`/`WARNING`/`DRIFTED`/`UNKNOWN`
with text and icon. Fewer than three eligible runs is “Not enough runs yet”; no colored
success percentage is shown without that minimum sample.

`RuntimeStopDialog` renders a read-only dependency preview and explicit choices; after
confirmation, display accepted drain separately from observed process stop. Changed
incarnation/dependencies invalidate stale assumptions and show updated blockers. An
installed/cold agent or startable provider uses neutral availability labels without busy
animation. Quick Entry follows normal composer draft, attachment preview and focus rules.

`PersistentEnvironmentCard` shows provider health separately from current use, names
resource/network limits and retention, and displays estimate confidence and budget
enforcement policy separately from actual enforcement (`provider-enforced`,
`host-monitored`, or unavailable) in text. It shows observed cumulative usage, currency,
confidence and observation time; missing/stale usage uses the explicit Unknown state and
never a zero value. Provision
confirmation blocks on missing required budgets; a monitored cap is labeled best-effort.
Suspend/resume/destroy controls expose active-use blockers and never suggest the data is
gone until provider destruction is confirmed.

## Coworker and delegation components

`CoworkerCard` leads with name/role and derived presence, then one-line active work and a
single useful action. Avatar is optional and never communicates status alone. `WorkerProfileCard`
is nested under its installed AgentBinding so multiple configurations for one harness do
not look like separate products. It shows Enabled/Disabled for new work separately from
current active Attempt count and observed availability.

`DelegationTree` is collapsed to a short summary in normal Task detail (“2 workers
helping”). Expanded rows identify Lead, LiteCowork worker, native-reported worker,
Capability, and Verifier distinctly. Selecting a row opens a side panel with profile and
revision, harness descriptor digest, AgentSession/Attempt, Runtime/Environment, current
usage confidence, Artifacts, Evidence, and blockers. Private native handles, raw prompts,
secrets, and process IDs never appear. On narrow screens it is a nested list route, not a
scaled desktop tree.

`CostIndicator` always carries a unit and source; currency is shown only for monetary
values. An unknown native-plan charge is rendered as “Provider usage; exact cost
unavailable”. `QuotaIndicator` becomes stale after its observation expiry and then reads
“Usage unavailable”, not the previous LOW/EXHAUSTED state as current truth. These controls
use existing semantic status tokens and textual labels; they introduce no vendor color.

`GoalCard` distinguishes owner-set status from derived contribution summary and gives a
direct link to each Task/Evidence source. `SuggestionCard` exposes Why this?, source list,
expected result, authority preview, expiry, and clear Prepare/Accept and Dismiss actions.
Its overflow menu offers Remind me (Later today, Tomorrow, Next week) and “Don't suggest
this type.” Muted-kind settings are editable in Settings; snoozed and expired dates are
textual. The acceptance button does not use approval styling because a Suggestion is not
an Approval.
`EnvironmentControlBadge`/`ControlOwnerBadge` shows only the active input controller and
does not share the Task lease visual treatment.

The global visual hierarchy remains outcome → workstreams → machinery. Home emphasizes
Needs You, work being handled, upcoming work, one Idea, and recent Artifacts. Agent
configuration and Inspector use the same component tokens at a denser layout; no new
palette, font family, or decorative animation is introduced for the delegation feature.

## Rich response composition

Rich responses use the existing semantic and component tokens. The response root is a
document flow, not one giant card: prose remains on the conversation surface and individual
cards/tables/charts use existing card and panel components. Initial content measures are
`--rich-readable-max: 48rem` and `--rich-wide-max: 72rem`; section gap uses `--space-6`,
block gap `--space-4`, inline gap `--space-2`. A block declares READABLE, WIDE, or
FULL_AVAILABLE; ordinary paragraphs remain readable width even next to a wide chart.

The registered model-safe component set is RichText, Layout (Stack/Row/Grid), RichCard,
Callout, MediaFrame/Gallery, CodePanel, DataTable, ChartFrame, DiagramFrame, Timeline,
Checklist, DeliverableGroup, and ArtifactCollection. Host-bound Task/Attempt, UserRequest,
Approval, ArtifactViewer, Verification, Effect, Runtime, Cost/Quota, capability activity,
and MCP App blocks are rendered by trusted adapters from authorized projections. A
model-safe renderer cannot impersonate those components through labels, icons, or color.

| Component | Required anatomy and fallback |
|---|---|
| RichCard | Existing 12px card radius, semantic border/surface, title and body; no nested card wall |
| DeliverableRow | Icon hint, title, exact version/provenance where useful, permitted actions, minimum 44px action target |
| CitationChip | Accessible source label, keyboard-open action, exact pinned revision on source detail |
| MediaFrame | Pinned Resource only, required alt text, optional caption; no implicit remote fetch/autoplay |
| ChartFrame | Title, plot, legend when needed, accessible summary and textual/table fallback |
| DiagramFrame | Diagram plus accessible node/edge or text representation and user-controlled playback |
| CodePanel | Language/name, highlighted inert text, copy control; never executes code |
| Checklist | Ordinary checkbox semantics, visually distinct from Task and Verification status |
| ArtifactCollection | Count, paged/virtualized rows, show-all and ZIP only for exact existing versions |
| MCP App frame | Provider/app provenance and sandbox framing; permission path remains host-owned |

At desktop widths, ROW may lay out side-by-side at 900 CSS px and above; below that it
stacks, and below 600 CSS px it becomes one column. These are layout defaults; usability
and focus order must be checked at narrow windows, 200% zoom, and 400% zoom. A chart and
diagram always have a screen-reader-accessible text equivalent. Unknown/future block kinds
fall back to a safe explanation and semantic message; arbitrary HTML/JS/renderer packages
never enter the Operator DOM.

## Chat-first Coworker components, tokens and accessibility target

Coworker roster is a collapsible navigation group with a dedicated New Coworker action; selecting a Coworker opens its latest eligible chat and preserves global ordinary conversations. Distinguish header identity, live Activity, proactive status and Runtime readiness as separate text labels; no synthetic presence. Default Coworker surface is generous conversation plus a resizable right Workbench, not a dashboard. Quick Create uses two primary fields (name/purpose); all additional sections optional and collapsed. The Add Connections catalog has category tabs, scalable search and a single Connect action per card, with provider-owned secure login; advanced Manage access stays behind disclosure. Responsibilities show a compact What/When/Where/Needs You confirmation card, followed by a secondary list of enabled/paused/draft responsibilities. Memory updates are quiet; Memory & Knowledge affords version history, source, supersession and revocation. Buttons disabled for unsupported backends include an explicit reason and never silently create stub state. For every interactive control, specify focus restoration, Escape/Cancel, loading, empty, error, offline, stale response, duplicate submission, keyboard ordering, reduced motion, 200% zoom, screen reader label and destructive consequences in [UI acceptance](../implementation/UI.md). Existing semantic palette/status constraints remain unchanged.
