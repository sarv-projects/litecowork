# ADR-0014: Version Worker Profiles and Bound Selection

- **Status:** Accepted for vNext
- **Date:** 2026-10-05

## Context

One installed AgentBinding may need multiple worker roles or model options. Treating each
as a fake installation loses configuration provenance. Ranking from an installed catalog
would also expose workers that the owner never enabled. Cost-only routing can reduce
quality or conceal unknown provider usage.

## Decision

Worker roles are immutable `DelegationProfile` revisions attached to an AgentBinding.
Discovery, Workspace binding authorization, lead eligibility, and worker-profile
enablement remain distinct. Selection first filters all hard constraints, then ranks the
eligible set under an explicit policy. Unknown cost is not zero. Verification and bounded
new-Attempt escalation determine whether a cheaper worker is adequate. Explicit REQUIRE
selection never substitutes.

## Consequences

The same installation can be a lead and one or more distinct workers. Model/session
options remain adapter-owned. Performance projections can guide ranking but never grant
authority or overrule an explicit required profile.
