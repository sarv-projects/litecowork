# ADR-0005: Replicate Domain State, Not Database Files

- **Status:** Accepted for vNext
- **Date:** 2026-10-01

## Context

Local and remote Runtimes need a shared view of durable work without coupling storage
engines or creating split-brain execution ownership.

## Decision

Replicate versioned domain events, immutable artifacts, resource manifests, Task
revisions, capability locks, and execution ownership. Keep one authoritative Workspace
Hub initially. Do not synchronize SQLite database files or native agent private state.

## Consequences

Storage can vary by deployment while domain contracts stay stable. Event ordering,
schema evolution, artifact integrity, and fenced leases are first-class qualification
requirements.
