# Archive Reader B2 — run 89807517 participant 10

**Bucket:** archive-02 (historical `docs/archive/legacy-implementation-c7e9c05/`)
**Read:** 40/41 files to EOF; `CURRENT-RUN.md` 520 of 10,160 lines before the deadline cancelled paging (marked explicitly in Coverage check).
**Digest:** [reading/digest-archive-02.md](reading/digest-archive-02.md)

Top findings:
1. **Zero ACCEPTED stories** in every snapshot (60-story V1 at `de73535`, 67-story at 2026-10-20); all 11 product journeys BLOCKED in `V1-PRODUCT-ACCEPTANCE-MATRIX.md`.
2. **One gap blocks the chain:** no qualified native Conversation session admission — Send unconditionally `disabled` (`ConversationsPage.tsx:583`), every `TurnCommand::Start` refused (`conversations.rs:398-400, :590-594`), route settles `FAILED`/503 `DEPENDENCY_UNAVAILABLE`.
3. **Fail-closed gates are real code:** `TASK_PLANNING_ISOLATION_UNAVAILABLE` before write, `admit_step_attempt` closed, Effect `STARTED` disabled, `WarmHoldCarriesNoAuthority`; the inventory-only admission transaction was deleted rather than left reachable.
4. **Containment is Linux-only** (cgroup v2 + Bubblewrap + `/proc` identity); Windows = DACL/reparse revalidation, macOS absent → those release cells blocked by design. SP04 proved `turn/interrupt` does **not** fence a writer (heartbeat 1→198 over 20.12 s).
5. **Recurring defect class is doc-vs-code drift:** 92 findings (2026-10-22 doc pass), 48 UI claims (18/15/15), docs claiming LIVE for unserved routes.