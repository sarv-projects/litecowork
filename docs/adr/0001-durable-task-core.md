# ADR-0001: Durable Task Core Around External Agents

- **Status:** Accepted for vNext
- **Date:** 2026-10-01

## Context

LiteCowork needs durable outcomes across replaceable agents, processes, devices, and
execution substrates. A host-owned reasoning loop would make the product another agent
framework and bind Task correctness to one implementation.

## Decision

Core owns Conversation/Task truth, revisions, Steps/Attempts, authorization, Runtime
coordination, Artifacts/Effects/Evidence, verification, and projections. External agents
own reasoning, model choice, private context, and native tools. Core persists and checks
agent-proposed plans but does not invent them.

## Consequences

Tasks can outlive AgentSessions and move forward through new Attempts. Agent adapters
must report negotiated features and provenance. Core-mediated and native effects require
different assurance labels.
