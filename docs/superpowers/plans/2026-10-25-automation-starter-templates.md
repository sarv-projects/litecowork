# Automation Starter Templates Implementation Plan

> **For agentic workers:** This plan is executed inline in the existing LiteCowork workspace.

**Goal:** Add Coding and Cowork Automation starter flows that create a reviewable Routine draft and then a paused Automation draft.

**Architecture:** Curated, typed template data feeds the Automations template gallery. An explicit selection carries the template through the existing Routine editor and idempotent create API; after the Routine receipt is confirmed, the app opens the Automation editor with that exact Routine revision and template schedule. Routine save and Automation save are separate owner actions; neither enables, runs, or starts an agent.

**Tech Stack:** React, TypeScript, Node built-in test runner, existing Routine and Automation APIs.

**Spec:** docs/V1-UI-SCREEN-CONTRACT.md and docs/AUTOMATION.md.

## Global Constraints

- Workspace ownership and exact immutable Routine revisions are checked by existing API and editor paths.
- Routine and Automation drafts are never auto-saved, enabled, run, or externally shared.
- An Automation definition is saved paused; enabling and manual Run remain explicit separate actions.
- A template never installs a connector, grants capability, authenticates a provider, or launches an agent.
- Canceling or changing Workspace clears any in-flight template continuation.

## Review Focus

- Cancel before saving a Routine leaves no Routine or Automation.
- An ambiguous Routine create reuses the existing exact request identity and does not create duplicates.
- A Workspace switch cannot carry the template or Routine reference into another Workspace.
- The Automation editor validates the created Routine revision before saving a paused definition.
- The selected schedule/time zone and output description are visible before the owner saves.

---

### Task 1: Typed automation starter definitions

**Files:**
- Create: apps/litecowork-ui/src/automations/automation-starters.ts
- Create: apps/litecowork-ui/tests/automation-starters.test.ts

**Interfaces:**
- Produces two curated starter definitions, one Coding and one Cowork, each with preview copy, requirements, expected Task output, Routine draft fields, and a supported recurring schedule.
- A pure mapper returns the exact Routine editor draft for a selected starter.

- [ ] Write tests for the two categories, bounded draft fields, and exact routine draft mapping.
- [ ] Run the focused test and confirm it fails because the starter module is missing.
- [ ] Implement the typed data and pure mapper.
- [ ] Run the focused test and the UI test suite.

### Task 2: Template-to-Routine-to-Automation flow

**Files:**
- Modify: apps/litecowork-ui/src/App.tsx
- Modify: apps/litecowork-ui/src/automations/AutomationsPage.tsx
- Modify: apps/litecowork-ui/src/automations/AutomationEditor.tsx
- Modify: apps/litecowork-ui/src/routines/RoutinesPage.tsx

**Interfaces:**
- AutomationsPage starts a selected starter flow through App-owned Workspace-scoped state.
- RoutinesPage receives a draft starter and reports a confirmed Routine create receipt with the exact Routine ID and current revision.
- AutomationEditor receives the selected starter and exact newly created Routine reference; it validates the revision using its existing read path before save.

- [ ] Add template gallery cards with previewed requirements and READY Task output.
- [ ] Route the owner to the Routine editor with prefilled, editable objective and instructions.
- [ ] Continue to Automation editor only after a confirmed Routine create receipt; prefill name, schedule, and exact Routine reference.
- [ ] Keep Automation paused and require explicit review before save.
- [ ] Clear flow state on cancel, successful Automation save, and Workspace change.
- [ ] Run the full UI Node test suite and production build.

### Task 3: Contract and implementation record

**Files:**
- Modify: docs/V1-UI-SCREEN-CONTRACT.md
- Modify: implementation/CURRENT-RUN.md

- [ ] Record the template flow and exact implementation boundary.
- [ ] Record verification output and any deferred gaps.

No commit or external publish is included.
