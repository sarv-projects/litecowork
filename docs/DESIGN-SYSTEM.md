# Design System Requirements

This document specifies semantic requirements, not brand colors. Exact visual tokens may evolve without changing architecture.

## Token groups

Required token families:
- typography: display, heading, body, label, code
- spacing: 4/8-based scale or equivalent
- radius: compact/card/panel/pill
- elevation: surface, overlay, modal
- motion: from `MOTION.md`
- semantic colors: neutral, info, active, success, warning, danger, verification, offline

## Core components

- AppShell
- SidebarNav
- Composer
- AttachmentChip
- TaskCard
- TaskStatusBadge
- WorkstreamLane
- SourceCard
- CapabilityActivityCard
- ArtifactCard
- ApprovalCard
- BlockerCard
- VerificationIndicator
- RuntimeLocationBadge
- InspectorPanel
- WorkbenchPanel
- TimelineEvent
- AutomationCard
- DiscoverItemCard

Every component defines normal, hover/focus, disabled, loading, error and reduced-motion behavior where applicable.

## Semantic status mapping

Status color/icon must not be the only signal. Text label is required for warning/error/needs-user states.

`REPORTED`, `OBSERVED`, `VERIFIED` use distinguishable labels/icons; only VERIFIED may use definitive verification styling.

## Accessibility

- keyboard reachable actions
- visible focus ring
- logical tab order
- screen-reader labels for status/verification
- WCAG contrast targets
- no color-only meaning
- reduced motion support
- live-region announcements for important Task/approval transitions, throttled to avoid noise

## Responsive behavior

Desktop: conversation + optional Live Desk/Workbench side panels.
Tablet: one secondary panel at a time.
Mobile: Conversation/Task first; Live Desk and Inspector become stacked/detail routes. Approvals remain clear and require explicit action.
