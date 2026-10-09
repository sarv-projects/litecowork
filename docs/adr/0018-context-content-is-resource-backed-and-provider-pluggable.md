# ADR-0018: Context Is Resource-Backed and Provider-Pluggable

- **Status:** Accepted for vNext
- **Date:** 2026-10-05

## Context

Private memory needs provenance, inspection, revision conflict handling, deletion, and
scope. Making embeddings or a vector index canonical would couple Task correctness to a
retrieval implementation and create an opaque second data store.

## Decision

User-authored ContextDocuments are classified, versioned Resources. A
`PersonalContextProvider` may retrieve derived context, but Core owns scope,
authorization, provenance, retention, and revocation. Provider-generated memory proposals
are deferred until a complete owner-review lifecycle exists. Retrieved context is untrusted
and lower priority than current Task/user input. No first-party vector database is required.

## Consequences

Users can inspect and edit durable context using ordinary Resource revision semantics.
Provider indexes are rebuildable. A provider cannot change TaskSpec, grants, Approvals,
or SecretLeases.


## Relationship to ADR 0024 (2026-10-10)

This ADR's Resource-backed provenance, authorization, revision and revocation decision remains active. ADR 0024 expands the accepted TARGET beyond manual owner-authored ContextDocuments: qualified agent-backed extraction may produce bounded `MemoryCandidate` records and quiet Coworker-private memory under explicit source eligibility. That candidate/policy pipeline is not yet implemented. It does not make vector/index state canonical, create a first-party semantic planner, or permit connected sources to self-authorize durable memory.
