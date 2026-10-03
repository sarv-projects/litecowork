# ADR-0007: Preserve Evidence Levels

- **Status:** Accepted for vNext
- **Date:** 2026-10-01

## Context

Agent claims, provider responses, and independent postcondition checks provide different levels of assurance.

## Decision

Use immutable REPORTED, OBSERVED, and VERIFIED Evidence records. A stronger later observation appends a new record; it never upgrades or rewrites an earlier claim. Task completion evaluates the level required by each acceptance criterion.

## Consequences

The UI and audit trail can report exactly what the system knows. A worker's completion statement is not verification.
