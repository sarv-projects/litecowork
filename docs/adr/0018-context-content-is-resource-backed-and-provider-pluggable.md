# ADR-0018: Context Is Resource-Backed and Provider-Pluggable

- **Status:** Accepted for vNext
- **Date:** 2026-10-05

## Context

Private memory needs provenance, inspection, revision conflict handling, deletion, and
scope. Making embeddings or a vector index canonical would couple Task correctness to a
retrieval implementation and create an opaque second data store.

## Decision

User-authored ContextDocuments are classified, versioned Resources. A
`PersonalContextProvider` may retrieve or propose derived context, but Core owns scope,
authorization, provenance, retention, and revocation. Retrieved context is untrusted and
lower priority than current Task/user input. No first-party vector database is required.

## Consequences

Users can inspect and edit durable context using ordinary Resource revision semantics.
Provider indexes are rebuildable. A provider cannot change TaskSpec, grants, Approvals,
or SecretLeases.
