# ADR-0010: Deduplicate Automation Occurrences Across Revisions

- **Status:** Accepted for vNext
- **Date:** 2026-10-02

## Context

An Automation may be edited after an external trigger is delivered but before it is processed or retried. Including the Automation revision in the deduplication key could create a second Task for the same source event.

## Decision

Compute an occurrence key only from stable trigger identity and enforce uniqueness on (automation_id, occurrence_key). Store the AutomationRevision separately on the occurrence so the first accepted logical delivery remains pinned to the definition that claimed it.

## Consequences

Retries and duplicate deliveries cannot replay solely because an Automation was edited. Schedule, webhook, connector, and manual triggers use explicit canonical identity fields.
