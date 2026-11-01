# 011 - Desktop Shell IPC Settings and Runtime Health

**ID:** 011
**Status:** DRAFT
**Owners:** Runtime, Operator IPC, Desktop UI, Product
**Updated:** 2026-11-01
**Revision:** 0.1
**Target release:** V1 local desktop
**Dependencies:** root ARCHITECTURE.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md, docs/IMPLEMENTATION-MASTER-PLAN.md, and the owner documents named in RESEARCH.md. Feature 001 remains the Conversation send authority.

## 1. Problem

The shell is not the security boundary. A settings toggle without a handler must not look armed. Window close, sleep and daemon stop are different. WSL is not native Windows or Linux qualification.

Observable outcome: The owner launches the desktop, reaches a real local Runtime over authenticated IPC, changes a setting only when its handler exists, and sees truthful runtime health across restart.

## 2. Goals

- Authenticate Tauri-to-daemon IPC without a loopback bearer.
- Map each settings control to LOCAL_ONLY, NATIVE, TARGET or a served command.
- Expose Runtime incarnation, readiness and drain state honestly.
- Keep semantic chat usable when rich presentation is disabled.

## 3. Non-goals

- Replacing the daemon with a window-owned Electron host.
- Cloud Runtime enrollment.
- Claiming all 187 settings are served.
- Mobile clients.

## 4. Current evidence

This checkout is documentation-first. The behavior below is **TARGET**. Archived source and historical CODE notes are evidence of past work, not SERVED or QUALIFIED status on this tree. No owner has locked this package.

## 5. LOCKED and FLEXIBLE decisions

### LOCKED

- Operator IPC does not change API authorization (LOCAL-OPERATOR-IPC).
- Principal comes from the authenticated transport, never a client-supplied id (settings ledger).
- Display preferences must not restore grants.
- Three independent lifecycles: Runtime, AgentHost, Environment (RUNTIME-LIFECYCLE).
- V1 is local-first desktop (ARCHITECTURE).
- Untrusted presentation cannot forge host actions (master plan P10, owned here as a shell rule).

### FLEXIBLE

- Module file layout and private helper names, provided the transactions and secret boundaries stay intact.
- Presentation copy, provided state distinctions remain visible.

### Open decisions

- GA OS order.
- Which settings groups are in the first served milestone versus remain TARGET.

### Blockers

- B-28: GA OS and IPC qualification bar.
- B-29: first settings groups allowed to be enabled in the shell.

## 6. High-level design

Tauri shell renders. IPC carries Operator commands to litecoworkd. Runtime lifecycle owns incarnation and drain. Settings UI consults the semantic map: LOCAL_ONLY may change immediately; TARGET stays disabled with the ledger's failure text; served commands wait until a later package marks them SERVED.

Trust boundary: the desktop UI is untrusted; the authenticated Operator is the only command path; native credentials and OS handles stay outside public events; SQLite transactions do not perform network or process calls.

## 7. Low-level design

No new operationIds. Health reads use existing Runtime readiness contracts. Settings mutations use idempotency and If-Match only when the ledger class is OPERATOR_CAS and the operation is actually mounted. This package's implementation task is the shell gate plus IPC auth tests, not silently mounting all settings.

Ordering: authorize, compare expected version, commit the durable receipt, then perform any external call, then reconcile. Proposed payload fields are PROPOSED until the canonical owner document and machine schema change together.

## 8. Data

See [DATA-MODEL.md](DATA-MODEL.md). This feature does not create a second source of truth beside the domain owner.

## 9. Wiring

See [WIRING.yaml](WIRING.yaml). Requests enter through the Operator. External calls sit outside transactions.

## 10. Security

Peer credentials or named-pipe DACL as already specified. Reject remote pipe clients. Redact IPC frames in logs. Do not trust WebView-supplied principals.

## 11. Errors and recovery

Daemon crash shows disconnected, not an empty successful workspace. Restart creates a new incarnation and reconciles hosts per RUNTIME-LIFECYCLE. Drain stops admission before claiming stopped.

Races use compare-and-swap or a single-writer occurrence identity. Timeout after an external effect reconciles. Duplicate retries return the original receipt when the idempotency key matches and fail when the payload conflicts.

## 12. Crash and restart

Reload the last committed owner records. Do not infer success from an intent. Reconcile in-flight external work before a new side effect. Cancellation fences late results.

## 13. Configuration and compatibility

Per-OS qualification is separate. Accessibility: keyboard path for New Chat, Settings and disabled-control explanations.

OS qualification is per native desktop. WSL2 does not qualify Windows or Linux GUI behavior.

## 14. Observability

Log ids, versions, statuses and denial reasons. Do not log secret bytes, native handles, private locators or raw credentials. Audit commits include actor, workspace and receipt id.

## 15. EARS acceptance

- **AC-001:** WHEN the shell sends a command, THE SYSTEM SHALL accept it only from an authenticated local IPC peer.
- **AC-002:** IF the peer marker is missing, THEN THE SYSTEM SHALL reject the command.
- **AC-003:** WHEN a settings control is TARGET or unserved, THE SYSTEM SHALL render it unavailable and SHALL NOT send a mutation.
- **AC-004:** WHEN a control is LOCAL_ONLY, THE SYSTEM SHALL persist presentation preference without changing grants or bindings.
- **AC-005:** WHEN the Runtime incarnation is STOPPED, STARTING, or RECOVERING, THE SYSTEM SHALL show that health state and SHALL NOT present execution as ready. WHILE the incarnation is DEGRADED and GET /v1/operator/readiness is SERVING, THE SYSTEM SHALL still allow ordinary Operator commands and SHALL keep execution readiness and Task recovery closed.
- **AC-006:** WHEN the window closes and the Runtime is still running, THE SYSTEM SHALL NOT report the Runtime as stopped.
- **AC-007:** IF rich presentation fails, THEN THE SYSTEM SHALL still show semantic Conversation text.
- **AC-008:** WHEN tests run only under WSL2, THE SYSTEM SHALL NOT record them as native Windows or native Linux desktop qualification.

## 16. Executable acceptance command

```bash
python3 .specify/scripts/validate_feature_package.py specs/011-desktop-shell-ipc-settings-runtime
```

Further gates after ACCEPTED/LOCKED are the cargo filters named in TASKS.md. They are not evidence until run on a recovered source tree.

## 17. Out-of-bounds

- Do not mount every OpenAPI route in this package.
- Do not implement cloud continuation.

## 18. Assumptions and open questions

- Historical IPC CODE in the archive is not current served evidence for this checkout.
- Semantic map rows start as TARGET or LOCAL_ONLY until source returns.

Each blocker has the owners named above. Resolution gate: owner decision recorded in this SPEC before status leaves DRAFT. Implementation tasks: 011-T01, 011-T02, 011-T03, 011-T04.

## 7a. Owner facts incorporated 2026-11-01

Read from LOCAL-OPERATOR-IPC and RUNTIME-LIFECYCLE.

- IPC limits: 16 KiB header, 100 MiB request body, 10 MiB response body, 8 concurrent accepted requests, 128 MiB combined in-flight bodies, 3-second connect and header deadline, 30-second body-read deadline, handler deadline at most 60 seconds.
- Logical request headers are only X-Workspace-ID, Idempotency-Key, If-Match, Content-Type, Content-Range, and X-Chunk-SHA256. Authorization, Host, Origin, cookies, and hop-by-hop headers are forbidden. The frame request_id is correlation only and is not an Idempotency-Key.
- Unix sockets are owner-only 0600 in an owner-only 0700 directory. Symlinks are rejected. The peer UID must match the installation binding. Windows uses a local named pipe whose DACL grants only the current logon SID and SYSTEM. Remote clients are rejected. If that DACL cannot be created, startup does not open an unauthenticated pipe and does not fall back to loopback bearer authentication.
- Local incarnation recovery_state moves through STARTING, RECOVERING, READY, DEGRADED, DRAINING, STOPPING, and STOPPED. That is not the payload of runtime.availability.changed.v1, whose schema enum is only PAIRING, ONLINE, DEGRADED, DRAINING, OFFLINE, and REVOKED. The lifecycle view availability enum also includes STARTING and RECOVERING, so those three lists stay distinct. GET /v1/operator/readiness confirms Operator API serving for the reported incarnation and is not execution readiness. GET /v1/runtime-lifecycle does not probe providers or change lifecycle state.
- Graceful drain stops admission before persisting DRAINING, allows admitted handlers at most 60 seconds, and reaches STOPPING and STOPPED only after those handlers finish. A handler deadline is unsettled because work may continue after HTTP 504. The authenticated Operator stop route stays absent until Task, Effect, lease, and local-service settlement exist.
- RuntimeStartupPolicy is MANUAL, LOGIN_BACKGROUND, or ALWAYS_ON_SERVICE. LOGIN_BACKGROUND is the recommended personal-desktop mode. Sleep suspends the local Runtime. Stopping the Runtime is distinct from cancelling Tasks.
- After restart, a failed identity check makes an ACTIVE root UNAVAILABLE, leaves a PAUSED root PAUSED, and does not reactivate a revoked root.
- Cite getLocalOperatorReadiness, getLocalRuntimeLifecycle, enrollLocalRuntimeInWorkspace, previewLocalRuntimeStop, requestLocalRuntimeStop. Event: runtime.availability.changed.v1.



## 19. Requirement traceability

| Requirement | Spec | Canonical contract | Task | Acceptance |
|---|---|---|---|---|
| AC-001 | EARS acceptance | canonical owner docs cited in this SPEC | 011-T01 | AC-001 |
| AC-002 | EARS acceptance | canonical owner docs cited in this SPEC | 011-T01 | AC-002 |
| AC-003 | EARS acceptance | canonical owner docs cited in this SPEC | 011-T04 | AC-003 |
| AC-004 | EARS acceptance | canonical owner docs cited in this SPEC | 011-T04 | AC-004 |
| AC-005 | EARS acceptance | canonical owner docs cited in this SPEC | 011-T03 | AC-005 |
| AC-006 | EARS acceptance | canonical owner docs cited in this SPEC | 011-T03 | AC-006 |
| AC-007 | EARS acceptance | canonical owner docs cited in this SPEC | 011-T04 | AC-007 |
| AC-008 | EARS acceptance | canonical owner docs cited in this SPEC | 011-T04 | AC-008 |
