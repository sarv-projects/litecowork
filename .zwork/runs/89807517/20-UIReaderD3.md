# UI Reader D3 — run 89807517 participant 20

Digest written: [reading/digest-ui-03.md](reading/digest-ui-03.md) — **partial**, deadline hit mid-bucket.

**Bucket ui-03** — 23/59 files read fully, ~5,806 lines; `lib.rs` only to line 1900/6132; 35 files untouched (incl. `workbench-panel.css`, `ConversationsPage.tsx`, all API clients, 7 tests).

Top 5:
1. Every WebView→daemon path is a Rust command with a finite operation enum (no URL/method passthrough), `X-Workspace-ID` on all calls, `Idempotency-Key`+`If-Match` on mutations, `deny_unknown_fields` wires, static error strings.
2. Stream transport: 3 events (`conversation-presentation-stream`, `task-presentation-stream`, `runtime-inventory-stream`), single-slot frame credit, `{token}-{sequence}` ids, 5 s ack timeout fenced to (session, frame).
3. `reduceTaskStreamFrame` accepts only ready/resync/projection, enforces `max(originSequence,1)==projection_revision` and monotonic revisions; controllers use generation fencing + ack-on-discard.
4. "Unknown is not success" is coded: planning readiness rejects any true dispatch flag; ZIP intake must report UNAVAILABLE; Needs-you rejects provider URLs and requires `no-store`.
5. UI tests are `node:test` asserting on CSS/TSX **source text**, no DOM/CDP harness found in files read (existence of a `check-rendered-ui` script remains unverified).

Divergence flagged: `workspace_root_observation_bridge` uses snake_case where all other bridges use camelCase.