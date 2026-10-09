# ADR-0022: Rich Presentation is an optional rendering enhancement

**Status:** Accepted

## Context

The Operator already has typed factual projections and transient turn streaming. Rich
response composition can make comparisons, citations, charts, and deliverables easier to
use, but it introduces an additional blob, renderer, schema, and replication dependency.
Embedding this state in ConversationMessage would make an optional UI format a dependency
of semantic history and break older clients/channels.

## Decision

ConversationMessage remains semantic truth and is committed/rendered without waiting for
rich compilation. A bounded immutable version-1 `RichPresentation` may be published as a
separate aggregate keyed to exactly one committed Agent message and its semantic digest.
It carries no authority. Trusted system blocks are resolved from authenticated Core
projections; rich-only status, citations, deliverables, actions, and facts are forbidden.
Missing, delayed, unsupported, or corrupt presentation data falls back to the complete
semantic message. No Message v2 or per-interaction DomainEvents are introduced for rich
rendering.

## Consequences

- Existing Conversation history, export, channels, and older Operators remain readable.
- Semantic message commit/turn settlement has no renderer latency dependency.
- RichPresentation storage, publication, backup, GC, and replication are independently
  versioned and integrity-checked.
- A presentation may upgrade a rendered message in place after its exact immutable blob
  arrives; historical typing and motion are not replayed.
- The compiler and Operator require closed schema/size/depth limits and exact source
  reauthorization.
