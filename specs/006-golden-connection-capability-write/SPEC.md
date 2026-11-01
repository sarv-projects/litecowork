# 006 - Golden Connection and One Capability Write

**ID:** 006
**Status:** DRAFT
**Owners:** Connectors, Capability Fabric, Security, LiteSPM boundary, Desktop UI
**Updated:** 2026-11-01
**Revision:** 0.1
**Target release:** V1 local desktop
**Dependencies:** root ARCHITECTURE.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md, docs/IMPLEMENTATION-MASTER-PLAN.md, and the owner documents named in RESEARCH.md. Feature 001 remains the Conversation send authority.

## 1. Problem

Capabilities are not installed merely by appearing in a catalog. Token bytes must not reach the agent. A write with an unknown provider result must not be retried blindly.

Observable outcome: The owner completes one real account Connection, performs one authorized read, then one consequential write with approval, idempotency and reconciliation, and revoke fences later use.

## 2. Goals

- Finish one first-party Connection with SecretStore custody.
- Execute one scoped read and one scoped write through the capability broker.
- Require explicit approval on the write and reconcile UNKNOWN before retry.
- Revoke so later invocations fail closed.

## 3. Non-goals

- Embedding Activepieces, Nango or n8n runtimes.
- Generating operations from OpenAPI without review.
- Multiple providers in this slice.
- Coworker assignment as a substitute for a grant.

## 4. Current evidence

This checkout is documentation-first. The behavior below is **TARGET**. Archived source and historical CODE notes are evidence of past work, not SERVED or QUALIFIED status on this tree. No owner has locked this package.

## 5. LOCKED and FLEXIBLE decisions

### LOCKED

- LiteCowork remains the control plane (CONNECTORS-WORKFLOWS §1).
- Installed, connected, assigned, granted, activated and invoked are distinct.
- Tokens stay in SecretStore; the model never sees them.
- TLS verification stays on; SSRF, redirects, timeouts and byte limits apply (AGENTS.md).
- Package approval is bound to an exact plan digest; agents cannot self-approve.

### FLEXIBLE

- Module file layout and private helper names, provided the transactions and secret boundaries stay intact.
- Presentation copy, provided state distinctions remain visible.

### Open decisions

- Which single provider is golden: GitHub, Google Workspace or Microsoft 365.

### Blockers

- B-19: golden provider and exact OAuth/PKCE or token recipe.
- B-20: the one write operation id and its idempotency key.

## 6. High-level design

UI starts connect. ConnectionService holds the OAuth dance outside SQLite. SecretStore writes tokens. CapabilityBroker loads only a grant-scoped invocation. The provider HTTP call is outside the invocation transaction. Effect settlement records the reconciled receipt.

Trust boundary: the desktop UI is untrusted; the authenticated Operator is the only command path; native credentials and OS handles stay outside public events; SQLite transactions do not perform network or process calls.

## 7. Low-level design

Follow CONNECTORS-WORKFLOWS Connection record and ConnectionService. One read operation and one write operation are named only after B-20. Approval is a Trust decision, not a UI boolean. LiteSPM may supply the package; it does not grant use.

Ordering: authorize, compare expected version, commit the durable receipt, then perform any external call, then reconcile. Proposed payload fields are PROPOSED until the canonical owner document and machine schema change together.

## 8. Data

See [DATA-MODEL.md](DATA-MODEL.md). This feature does not create a second source of truth beside the domain owner.

## 9. Wiring

See [WIRING.yaml](WIRING.yaml). Requests enter through the Operator. External calls sit outside transactions.

## 10. Security

Opaque Connection id only in agent-visible context. No token bytes in events, logs or prompts. Redirect and host allowlists. Revocation fences refresh and invoke.

## 11. Errors and recovery

Cancelled OAuth leaves no Connection. Refresh serialization avoids stampeding the provider. UNKNOWN write blocks retry until reconciliation. Restart reloads Connection status without printing tokens.

Races use compare-and-swap or a single-writer occurrence identity. Timeout after an external effect reconciles. Duplicate retries return the original receipt when the idempotency key matches and fail when the payload conflicts.

## 12. Crash and restart

Reload the last committed owner records. Do not infer success from an intent. Reconcile in-flight external work before a new side effect. Cancellation fences late results.

## 13. Configuration and compatibility

One provider. Public desktop OAuth with loopback and PKCE where the provider allows it; otherwise the slice stays blocked rather than embedding a confidential cloud gateway.

OS qualification is per native desktop. WSL2 does not qualify Windows or Linux GUI behavior.

## 14. Observability

Log ids, versions, statuses and denial reasons. Do not log secret bytes, native handles, private locators or raw credentials. Audit commits include actor, workspace and receipt id.

## 15. EARS acceptance

- **AC-001:** WHEN the owner starts connect, THE SYSTEM SHALL keep provider token bytes out of Conversation, events and logs.
- **AC-002:** IF the owner cancels auth, THEN THE SYSTEM SHALL persist no usable Connection.
- **AC-003:** WHEN a read runs, THE SYSTEM SHALL require an explicit grant distinct from the Connection record.
- **AC-004:** WHEN a write is requested, THE SYSTEM SHALL require a fresh approval bound to the exact plan digest before invocation.
- **AC-005:** IF the provider result is UNKNOWN, THEN THE SYSTEM SHALL reconcile before retry and SHALL NOT emit a second consequential call meanwhile.
- **AC-006:** WHEN the owner revokes the Connection, THE SYSTEM SHALL fail later reads and writes for that Connection id.
- **AC-007:** IF an agent requests install or approval of its own write, THEN THE SYSTEM SHALL deny self-approval.

## 16. Executable acceptance command

```bash
python3 .specify/scripts/validate_feature_package.py specs/006-golden-connection-capability-write
```

Further gates after ACCEPTED/LOCKED are the cargo filters named in TASKS.md. They are not evidence until run on a recovered source tree.

## 17. Out-of-bounds

- Do not enable arbitrary URLs from the model.
- Do not ship a workflow-builder canvas in this slice.

## 18. Assumptions and open questions

- Provider choice is an owner decision.
- Historical connector code in the archive is not served evidence.

Each blocker has the owners named above. Resolution gate: owner decision recorded in this SPEC before status leaves DRAFT. Implementation tasks: 006-T01, 006-T02, 006-T03, 006-T04.

## 7a. Owner facts incorporated 2026-11-01

Read from CONNECTORS-WORKFLOWS, CAPABILITY-INVOCATIONS, CAPABILITY-FABRIC, and NETWORK-SECURITY.

- Connection.status in CONNECTORS-WORKFLOWS is CONNECTING, CONNECTED, DEGRADED, REAUTH_REQUIRED, REVOKED, or DISCONNECTED. The OpenAPI Connection status enum omits REVOKED. Promote that value into the schema and API owner together before implementation. account_display_hint is not authenticated identity.
- The loopback listener binds only 127.0.0.1 on an OS-chosen port. state is single-use. The callback body contains no token. Secrets are stored before status becomes CONNECTED. A failed Connection commit purges the new secrets. Plaintext fallback is forbidden.
- Cancelled OAuth records AuthAttempt CANCELLED and persists no Connection. The first terminal callback wins.
- Refresh holds a per-connection lock. invalid_grant or provider revocation becomes REAUTH_REQUIRED. Other refresh failures become DEGRADED. Refresh does not delete historical Effects and does not start a consequential retry.
- ConnectorActionDescriptor.operation_class is READ, WRITE, SEND, DELETE, PURCHASE_LIKE, or ADMIN. A package READ label cannot override Trust classification. Idempotency is PROVIDER_KEY, CLIENT_KEY, READ_ONLY, or UNKNOWN.
- CapabilityInvocation.execution_method is STRUCTURED_API, STRUCTURED_BROWSER, ACCESSIBILITY_BROWSER, SCREEN_COMPUTER_USE, DETERMINISTIC_LOCAL, NATIVE_AGENT_TOOL, or UNKNOWN. Status includes AMBIGUOUS, which is non-terminal. A timeout is not completion. CONVERSATION and TASK_PLANNING invocations are read-only.
- Credential-like elicitation fails with SENSITIVE_INPUT_UNSUPPORTED. Unknown MCP methods, including sampling, fail with PROVIDER_INPUT_UNSUPPORTED and do not become a UserRequest.
- Worker egress rejects loopback, private, link-local, multicast, and cloud-metadata destinations. A Task grant cannot exempt those classes. Every redirect is revalidated. TLS downgrade is forbidden.
- External dispatch waits until one transaction rechecks owner, lease, grant, activation, Trust decision, and exact ApprovalUse. A PROPOSED Effect is not a dispatch permit.
- Cite listConnections, getConnection, disconnectConnection, createCapabilityGrant, revokeCapabilityGrant, resolveApproval. Event: connection.state.changed.v1. No golden-provider write operationId exists until B-20.



## 19. Requirement traceability

| Requirement | Spec | Canonical contract | Task | Acceptance |
|---|---|---|---|---|
| AC-001 | EARS acceptance | canonical owner docs cited in this SPEC | 006-T01 | AC-001 |
| AC-002 | EARS acceptance | canonical owner docs cited in this SPEC | 006-T01 | AC-002 |
| AC-003 | EARS acceptance | canonical owner docs cited in this SPEC | 006-T03 | AC-003 |
| AC-004 | EARS acceptance | canonical owner docs cited in this SPEC | 006-T03 | AC-004 |
| AC-005 | EARS acceptance | canonical owner docs cited in this SPEC | 006-T03 | AC-005 |
| AC-006 | EARS acceptance | canonical owner docs cited in this SPEC | 006-T02 | AC-006 |
| AC-007 | EARS acceptance | canonical owner docs cited in this SPEC | 006-T03 | AC-007 |
