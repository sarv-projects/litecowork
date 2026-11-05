# Doc Reader A3 — run 89807517 participant 2

**Bucket docs-03** — 19/19 files read, 7,470 lines, 445 KB.

Top findings:
1. **SDD 001 conflict (hard):** `V1-FEATURE-SDD-REGISTER.md` says 001 is ACCEPTED/LOCKED Rev 1.0; `FEATURE-AND-UI-CONTRACT-COVERAGE.md` says four times that 001–011 are all DRAFT, none accepted.
2. **`Connection` has two owners:** defined in `DATA-MODEL.md` (`connector_ref: string`) and again in `CONNECTORS-WORKFLOWS.md` §4 (`ConnectorRef`, plus `AuthAttempt`). Same for `WorkflowRunBinding`, `RichPresentation`.
3. **Migration numbering only referenced:** `sqlite-v1.sql` baseline, collectors v17/v18/v19, "migration v28"; no bucket file enumerates sqlite-v2…v28.
4. **Connector kind self-contradiction:** §3 "LiteSPM-managed `ConnectorPackage`" vs §9b pinned upstream "`connector` deprecated, deferred to v2" (only mcp/skill/plugin).
5. **Memory enums disagree:** `MemoryHit.status` (ACTIVE|STALE|REVOKED|NEEDS_REVIEW) vs `MemoryRecordMetadata.status` (ACTIVE|SUPERSEDED|REVOKED|PURGE_PENDING); ADR-0005 Hub replication also drifts from V1 `LOCAL_ONLY`.

Digest: [reading/digest-docs-03.md](reading/digest-docs-03.md)