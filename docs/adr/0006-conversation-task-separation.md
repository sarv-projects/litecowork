# ADR-0006: Conversation and Task Are Separate

- **Status:** Accepted for vNext
- **Date:** 2026-10-01

## Context

A conversation is the user-facing continuity surface, while durable outcome-oriented work needs independent lifecycle, revisions, recovery, and verification.

## Decision

Keep Conversation and Task as separate entities. A Conversation may have no Task or multiple linked Tasks. Materialization is explicit or requested by the conversational lead under the product rules; ambiguous intent is clarified. The committed source message and initial TaskSpecRevision are linked atomically.

## Consequences

Ordinary questions do not require durable execution state. Tasks survive surface and agent changes without depending on a transcript.
