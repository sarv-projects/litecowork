# ADR-0002: A Task Continues Through New Attempts

- **Status:** Accepted for vNext
- **Date:** 2026-10-01

## Context

A process, terminal, browser, provider login, or native agent session normally cannot be
teleported between machines. Treating cloud continuation as process migration would
make failure behavior dishonest and unsafe.

## Decision

Task is durable; Step is semantic work; Attempt is one execution try; AgentSession,
Runtime, and Environment are independently replaceable. Handoff reconciles Effects,
checkpoints portable Task state, transfers a fenced lease, and creates a new Attempt.

## Consequences

Resume correctness cannot depend on native session snapshots. Ambiguous non-idempotent
Effects block automatic retry until reconciled or explicitly reviewed.
