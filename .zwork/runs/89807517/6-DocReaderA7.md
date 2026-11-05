# Doc Reader A7 — run 89807517 participant 6

**Bucket docs-07** — 18/18 files, 6,576 lines, 455,100 bytes (444 KiB).

**Top 5 findings**
1. **ADR decisions:** 0014 (Accepted vNext) — worker roles are immutable `DelegationProfile` revisions; hard-constraint filter *then* rank, "unknown cost is not zero". 0017 — `DEADLINE_SENSITIVE` = prioritized placement only, hard realtime explicitly not a contract. 0024 — "**Accepted target**", implementation/schema/owner acceptance *pending*; Coworkers optional, Conversation primary. 0025 — one versioned `AgentModule` per agent; no global provider/model catalog.
2. **Audit (2026-10-31):** active tree is docs-only; 44 links rewritten, ARCHITECTURE renumbered 1–31; reconciliation 203 paths/229 ops/229 operationIds, 148 events, 131 error codes, 25 ADRs, 28 migrations → 135 tables. Still open: no served Send, no supervisor caller, no watcher, blockers B-01–05/B-07–10.
3. **Audit internal drift:** §4.1 says 143 files/863 links; §6–8 say 137 files/877 links.
4. **Defect:** ENVIRONMENTS.md has `provider archive/legacy-implementation-c7e9c05/version` — a stray archive path inside the `IsolationAttestation` sentence.
5. **Evidence dates:** COMPETITIVE-RESEARCH is titled 2026-10-25 but contains a 2026-11-01 pass; pinned `ec75891b…` (opendot) and `11cd2a9a…` (AionUi); LiteSPM 69,376/26,554 listings.

Digest: [reading/digest-docs-07.md](reading/digest-docs-07.md)