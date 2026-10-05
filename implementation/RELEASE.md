# Production operations and release gates

Production level means tested installed behavior, recoverability and accurate capability
claims. It does not mean team/RBAC/enterprise deployment. The owner remains the sole
reviewer; automated gates supplement that review.

## Build and distribution

Reproducible locked Rust/JS/Python provider builds; supported OS CI artifacts; signatures
and update metadata; SBOM/license notices; versioned API/events/Mesh/storage migrations.
Keep OS signing and provider credentials external. Verify artifact checksums and refuse
tampered updates. Ship daemon independently of Operator lifecycle and document startup,
tray, stop, suspend and uninstall/data-retention choices.

## Cloud/remote runbook deliverables

Document persistent volumes, backup keys, least-privilege service user, TLS/auth/ingress,
egress/provider credentials, worktree/browser sandbox isolation, RAM/CPU/disk caps,
health/readiness/drain, restart policy, cost limits and provider terms. No exposed unauthenticated
control port. Use OpenClaw Docker/Gateway as operational references, not a substitute
Runtime or a copied security policy. Deploy one personal authority first; scale only from
measured requirements. SQLite isn't shared between competing cloud writers.

Remote enrollment has revocation/offline/version/host-loss procedures; record actual
capability/auth/resource availability. Losing a device does not authorize another Runtime
to continue consequential work until fencing/reconciliation/admission pass.

## Incident playbooks

| Incident | First safe action | Recovery proof |
|---|---|---|
| Ambiguous external mutation | Stop new conflicting effects; preserve journal | External reconciliation and exact Effect settlement before retry |
| Compromised/revoked Runtime | Revoke pairing/grants, fence epoch and leases | New calls denied, replacement authority freshly admitted |
| Lost/corrupt disk | Stop writes, restore verified backup to new identity | Digests/tombstones intact; no resurrected session/lease |
| Broken provider/config/quota | Open circuit/block or policy-qualified replacement | New Attempt, config/auth rechecked, bounded handoff |
| Context purge incomplete | Keep tombstone blocked and retry missing targets | Exact sealed receipt set; no restored content |
| Bad update/migration | Safe stop; refuse incompatible downgrade | Tested rollback/forward repair without mutation of pinned work |
| Browser control dispute | Human takeover fences queued input | Fresh control epoch/observation and reconciled effects |
| Cost/memory pressure | Stop new admissions/evict speculative holds under policy | Active effects and takeover dependencies preserved |

## Observability and privacy

Trace Task/Attempt/Invocation/Effect/verification and profile revisions with correlation
IDs, not secret/input bytes. Metrics include verified completion, rescue count, usage per
outcome, start latency, warm hit rate, routing reasons, retrieval/deletion state and
schedule health. Diagnostics export is owner-previewed/redacted. Retention/deletion and
cloud inference disclosure must match actual paths, including native-provider history
that LiteCowork cannot recall. Synthetic staging data is default.

## G5 checklist

- All required stories accepted; coverage rows point to executable cases and evidence.
- Full local/cloud/remote gates; supported OS/agent/provider matrix published.
- Real-use corpus passed, blockers resolved; no critical authority/isolation/data-loss
  defects. Any unavailable optional integrations visibly unavailable and owner recorded.
- Code/system/user tests and architecture/plan validation green from exact release commit.
- Numeric performance/quality baselines frozen and regression gates passed; soak report.
- Installer signature, upgrade, uninstall, restore and rollback verified on clean hosts.
- Cloud costs, health, backups, fencing, quota, channel assurance and incident drills proven.
- User documentation: first task, workers, uploads/RAG, approvals, artifacts, automation,
  cloud/remote, privacy/provider limitations, troubleshooting and diagnostics.
- Owner reviews evidence and approves release claims; no invented cost savings or parity.

Keep completed evidence in an execution record separate from this initial planned backlog.
The initial docs cannot satisfy any product release gate by themselves.
