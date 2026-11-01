# 008 - Workbench Browser Terminal and Computer

**ID:** 008
**Status:** DRAFT
**Owners:** Desktop shell, Environments, Browser provider, Trust, Desktop UI
**Updated:** 2026-11-01
**Revision:** 0.1
**Target release:** V1 local desktop
**Dependencies:** root ARCHITECTURE.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md, docs/IMPLEMENTATION-MASTER-PLAN.md, and the owner documents named in RESEARCH.md. Feature 001 remains the Conversation send authority.

## 1. Problem

A WebView is not Chrome. A terminal tab is not an Environment. Computer use without consent and epoch revocation is unsafe. The Workbench must show real objects only.

Observable outcome: The owner uses the right Workbench to finish real browser and terminal work, takes over and returns control, and a stale handle cannot act.

## 2. Goals

- Ship one context-aware Workbench bound to Conversation or Task.
- Qualify browser session epochs with takeover and return.
- Bind terminal PTY lifetime to an admitted Environment process tree.
- Revoke stale browser and terminal handles on epoch change or Runtime restart rules.

## 3. Non-goals

- Claiming Tauri WebView is Google Chrome.
- Headless browser pretending to be the same visible session.
- Terminal that executes shell startup scripts during discovery.
- Computer use without an explicit consent epoch.

## 4. Current evidence

This checkout is documentation-first. The behavior below is **TARGET**. Archived source and historical CODE notes are evidence of past work, not SERVED or QUALIFIED status on this tree. No owner has locked this package.

## 5. LOCKED and FLEXIBLE decisions

### LOCKED

- Browser provider and presentation adapter are separate (CHROME-BROWSER-LLD).
- UNAVAILABLE is not a browser mode.
- Terminal UI does not own a privileged shell (V1-INTEGRATED-TERMINAL-LLD).
- Workbench is views over authorized objects (release contract).
- Stale handles cannot act (master plan P7).

### FLEXIBLE

- Module file layout and private helper names, provided the transactions and secret boundaries stay intact.
- Presentation copy, provided state distinctions remain visible.

### Open decisions

- First qualified browser distribution and terminal shell profile on the GA OS.

### Blockers

- B-22: browser provider mode for GA.
- B-23: default shell profile and whether computer-use is in the same milestone or strictly after browser and terminal.

## 6. High-level design

Right Workbench hosts views. Browser control goes through a lease gateway outside SQLite. Terminal opens a PTY only after Environment admission. Computer actions use the same epoch and consent record. UI disablement follows provider availability.

Trust boundary: the desktop UI is untrusted; the authenticated Operator is the only command path; native credentials and OS handles stay outside public events; SQLite transactions do not perform network or process calls.

## 7. Low-level design

Takeover keeps the EnvironmentControlLease ACTIVE, moves control from AGENT epoch n to HUMAN epoch n+1, and rejects the old epoch. The browser process may stay up. Return requires explicit confirmation and another epoch increment. Handle presentation to the model is a capability grant, not the raw CDP or PTY handle in events.

Ordering: authorize, compare expected version, commit the durable receipt, then perform any external call, then reconcile. Proposed payload fields are PROPOSED until the canonical owner document and machine schema change together.

## 8. Data

See [DATA-MODEL.md](DATA-MODEL.md). This feature does not create a second source of truth beside the domain owner.

## 9. Wiring

See [WIRING.yaml](WIRING.yaml). Requests enter through the Operator. External calls sit outside transactions.

## 10. Security

Consent is explicit per epoch. Network and filesystem remain Environment policy. Screenshots and page text are untrusted input. Handles are redacted from logs.

## 11. Errors and recovery

Provider crash marks the epoch lost. Restart does not replay a half-applied browser action without reconciliation. Closing the window does not by itself kill the Runtime, but sleep and shutdown follow RUNTIME-LIFECYCLE.

Races use compare-and-swap or a single-writer occurrence identity. Timeout after an external effect reconciles. Duplicate retries return the original receipt when the idempotency key matches and fail when the payload conflicts.

## 12. Crash and restart

Reload the last committed owner records. Do not infer success from an intent. Reconcile in-flight external work before a new side effect. Cancellation fences late results.

## 13. Configuration and compatibility

Modes that are not qualified stay hidden with a repair path. Computer use can remain unavailable if B-23 defers it.

OS qualification is per native desktop. WSL2 does not qualify Windows or Linux GUI behavior.

## 14. Observability

Log ids, versions, statuses and denial reasons. Do not log secret bytes, native handles, private locators or raw credentials. Audit commits include actor, workspace and receipt id.

## 15. EARS acceptance

- **AC-001:** WHEN no qualified browser provider exists, THE SYSTEM SHALL show UNAVAILABLE and SHALL NOT label a WebView as Chrome.
- **AC-002:** WHEN the owner starts a managed browser epoch, THE SYSTEM SHALL record consent and a session epoch before any agent browser action.
- **AC-003:** WHEN the owner takes over, THE SYSTEM SHALL stop agent actions until return or close.
- **AC-004:** IF a stale epoch handle is presented, THEN THE SYSTEM SHALL reject it.
- **AC-005:** WHEN a terminal opens, THE SYSTEM SHALL bind it to one Environment process tree and SHALL NOT merge tabs across Environments.
- **AC-006:** WHEN shell profiles are discovered, THE SYSTEM SHALL NOT execute profile startup scripts or scrape secrets.
- **AC-007:** WHEN the Runtime loses the provider, THE SYSTEM SHALL mark the epoch lost and reconcile in-flight actions before retry.

## 16. Executable acceptance command

```bash
python3 .specify/scripts/validate_feature_package.py specs/008-workbench-browser-terminal-computer
```

Further gates after ACCEPTED/LOCKED are the cargo filters named in TASKS.md. They are not evidence until run on a recovered source tree.

## 17. Out-of-bounds

- Do not automate the user's everyday Chrome profile by default.
- Do not treat WSL as proof of native Windows terminal qualification.

## 18. Assumptions and open questions

- Computer use may be deferred by B-23.
- Archive UI is not served evidence.

Each blocker has the owners named above. Resolution gate: owner decision recorded in this SPEC before status leaves DRAFT. Implementation tasks: 008-T01, 008-T02, 008-T03, 008-T04.

## 7a. Owner facts incorporated 2026-11-01

Read from CHROME-BROWSER-LLD, V1-INTEGRATED-TERMINAL-LLD, and STATE-MACHINES.

- AvailabilityStatus is READY, NEEDS_SETUP, UNAVAILABLE, OFFLINE, or UNKNOWN. BrowserMode is MANAGED_CHROMIUM, CONNECT_EXISTING_CHROME, MANUAL_ONLY, or HEADLESS_TASK. HEADLESS_TASK handoff to a visible browser is a new surface.
- BrowserControlOwner is AGENT, USER, NONE, or TRANSFER_PENDING. Session health is STARTING, READY, DEGRADED, DISCONNECTED, RECOVERING, or CLOSED. viewMode is LIVE_INTERACTIVE, OBSERVATION_FRAMES, EXTERNAL_WINDOW, or UNAVAILABLE. BrowserActionReceipt.state is REJECTED, ADMITTED, IN_PROGRESS, APPLIED, AMBIGUOUS, or BLOCKED. The command actor field is caller intent. Authority comes from the authenticated principal plus the control lease.
- Take control is an atomic epoch transfer. Return requires explicit confirmation, increments the epoch, and the agent replans from a fresh observation. Closing the side pane does not close the browser or cancel the Task. A matching URL after restart is not proof that a pre-crash write succeeded.
- Password fields, payments, CAPTCHA, permission prompts, and MFA require a human. Copying encrypted cookies or importing history, passwords, or login state is forbidden. Chrome remote debugging against the default user directory and Playwright automation of the default Chrome profile are forbidden.
- reconcileBrowserAction leaves UNKNOWN ambiguous and does not submit the same form twice. detachBrowserSession does not kill a user-owned Chrome profile.
- shellProcessState is STARTING, ACTIVE, EXITED, RECONNECTING, LOST, DETACHED, or STOPPING. ownerKind is USER_MANUAL, TASK_ATTEMPT, or PROVIDER_PRIVATE. An unauthorized root is rejected. Silent fallback to home is forbidden. Closing the terminal view is not Stop Task. A manual terminal history does not become Task Evidence automatically.
- Cite takeEnvironmentControl and returnEnvironmentControl. Event: environment.control.lease.changed.v1. No browser-session or terminal-PTY operationId exists yet.



## 19. Requirement traceability

| Requirement | Spec | Canonical contract | Task | Acceptance |
|---|---|---|---|---|
| AC-001 | EARS acceptance | canonical owner docs cited in this SPEC | 008-T01 | AC-001 |
| AC-002 | EARS acceptance | canonical owner docs cited in this SPEC | 008-T01 | AC-002 |
| AC-003 | EARS acceptance | canonical owner docs cited in this SPEC | 008-T02 | AC-003 |
| AC-004 | EARS acceptance | canonical owner docs cited in this SPEC | 008-T02 | AC-004 |
| AC-005 | EARS acceptance | canonical owner docs cited in this SPEC | 008-T03 | AC-005 |
| AC-006 | EARS acceptance | canonical owner docs cited in this SPEC | 008-T03 | AC-006 |
| AC-007 | EARS acceptance | canonical owner docs cited in this SPEC | 008-T02 | AC-007 |
