# ADR-0004: Runtime and Environment Are Separate

- **Status:** Accepted for vNext
- **Date:** 2026-10-01

## Context

Local, VPS, and cloud deployments should run the same headless LiteCowork Runtime, but
the place where one Attempt acts may be a separate worktree, container, VM, browser, or
desktop session.

## Decision

Runtime identifies a running `litecoworkd` and its Mesh identity/roles. Environment
identifies the execution substrate selected for an Attempt. Cloud continuation belongs
to the overall Runtime model and uses a new Attempt after handoff/recovery.

## Consequences

Placement and eligibility consider both Runtime offers and Environment compatibility.
Environment providers do not own Task truth or Runtime identity.
