"""Cross-owner design invariant regression checks for active first-party docs.

This checks a defined set of adjudicated invariants. It is not semantic certification.
Derived reading digests, historical archives, third-party clones and generated reports
are excluded; current authority is checked in docs/ and specs/.
"""
from __future__ import annotations
import hashlib
import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SKIP = {
    ".git", "node_modules", "target", ".venv", "dist", "build", "coverage",
    ".next", "out", ".turbo", "vendor", ".cache", ".pytest_cache",
    ".agent-worktrees", ".worktrees", ".zwork", "archives", "archive",
    "repo_take", "reading", "staging_tar", "audit",
}
docs = {}
for folder, dirs, filenames in os.walk(ROOT):
    dirs[:] = [name for name in dirs if name not in SKIP]
    for name in filenames:
        if not name.endswith((".md", ".mdx")) or name == "V1-DEEP-SEMANTIC-SNAPSHOT.md":
            continue
        path = Path(folder) / name
        rel = path.relative_to(ROOT).as_posix()
        docs[rel] = path.read_text(encoding="utf-8-sig", errors="replace")

issues = []
def need(rel: str, fragment: str) -> None:
    if fragment not in docs.get(rel, ""):
        issues.append(f"MISSING {rel} :: {fragment[:100]}")
def forbid(rel: str, fragment: str) -> None:
    if fragment in docs.get(rel, ""):
        issues.append(f"SHADOW {rel} :: {fragment[:100]}")
def exactly(rel: str, fragment: str, count: int = 1) -> None:
    actual = docs.get(rel, "").count(fragment)
    if actual != count:
        issues.append(f"COUNT {rel} :: {fragment[:80]} expected {count} got {actual}")

cow = "docs/COWORKERS-TARGET.md"
learn = "docs/LEARNING.md"
need(cow, "**Memory policy normalization, no second memory state machine:**")
for fragment in (
    "CoworkerMemoryPolicy {",
    "MemoryCandidate { # proposal",
    "coworker.memory_policy.revised.v1",
    "coworker.memory_candidate.status_changed.v1",
    "coworker.memory_record.committed.v1",
):
    forbid(cow, fragment)
exactly(cow, "**Memory policy normalization, no second memory state machine:**")
exactly(cow, "Memory lifecycle uses only LEARNING.md target event names")
need(learn, "CoworkerLearningPolicy {")
need(learn, "LearningCandidate {")
need(learn, "MemoryRecordMetadata {")
need("docs/DATA-MODEL.md", "CoworkerLearningPolicyRevision {")
need("docs/RESPONSIBILITIES.md", "LEGACY interim field")
exactly("docs/RESPONSIBILITIES.md", "# LEGACY interim field: reviewed proposals only; new LearningPolicy controls quiet private auto-commit")
need("docs/API.md", "Accepted Coworker-owned Conversation target")
forbid("docs/API.md", "Conversation turns remain governed\nby Conversation override and Workspace default, independently")
need("docs/FLOWS.md", "For the accepted **Coworker-owned Conversation target**")
need("docs/FLOWS.md", "TaskService derives that exact origin server-side")
need("docs/SERVICES.md", "**Coworker-owned Conversation target admission (not yet served):**")
exactly("docs/SERVICES.md", "**Coworker-owned Conversation target admission (not yet served):**")
need("docs/COWORKER-FLOWS.md", "two independently recoverable owner commits")
need("docs/COWORKER-FLOWS.md", "typed proposal only through a qualified host proposal tool/input surface")
need(cow, "**F-C01 Create Quick (two recoverable owner commits):**")

need("docs/V1-AGENT-MEMORY-FAST-PATH-LLD.md", "Currently the desktop Resource upload UI supports only Workspace-owned WORKSPACE_NOTES")
need("docs/API.md", "The local desktop V1 Library currently creates only")
need("docs/V1-TELEMETRY-USAGE-RESILIENCE-LLD.md", "UNRECONCILED")
need("docs/V1-TELEMETRY-USAGE-RESILIENCE-LLD.md", "excluded from a purported exact deduplicated total")
need("docs/V1-TELEMETRY-USAGE-RESILIENCE-LLD.md", "cannot provide a guaranteed monetary hard stop")
need("docs/SERVICES.md", "Core-mediated LiteSPM capability provider invocation")
need("docs/V1-INTEGRATED-TERMINAL-LLD.md", "cannot be fenced or guaranteed observed")
need("docs/FLOWS.md", "native agent, compatible model and reasoning/mode controls")
need("docs/CONTEXT.md", "The Operator may offer a read-only")
need(cow, "No unreviewed group shared memory.")

need("README.md", "docs/IMPLEMENTATION-STATUS.md")
need("ARCHITECTURE.md", "The active executable source is in apps/ and crates/")
need("CURRENT-RUN.md", "**Release posture:** pre-release; V1 is not release-qualified")
need("docs/V1-UI-SCREEN-CONTRACT.md", "Current terminal is simulated and must not report a command exit")
need("docs/IMPLEMENTATION-STATUS.md", "Several Conversation lifecycle gaps remain release blockers.")
need("docs/IMPLEMENTATION-STATUS.md", "one-active-turn-per-Conversation rule")
need("docs/IMPLEMENTATION-STATUS.md", "| `CARGO_BUILD_JOBS=1 cargo test --workspace --locked` |")

# Reject obsolete reset-era claims in normative first-party documents.
for rel, body in docs.items():
    for stale in (
        "This docs-first checkout has no",
        "The active checkout has no implementation to mark SERVED",
        "Docs-first checkout has no active Operator handlers",
        "This checkout is documentation-first",
        "The former ARCH corpus is not present in the active docs-first checkout",
        "no current executable implementation",
    ):
        if stale in body:
            issues.append(f"STALE {rel} :: {stale}")

inventory_path = ROOT / "docs/schemas/served-route-inventory.json"
try:
    inventory = json.loads(inventory_path.read_text(encoding="utf-8"))
    if inventory.get("version") != 3:
        issues.append("SERVED INVENTORY version must be 3")
    counts = (
        len(inventory.get("served", [])),
        len(inventory.get("served_outside_openapi", [])),
        len(inventory.get("target_only", [])),
    )
    if counts != (173, 1, 56):
        issues.append(f"SERVED INVENTORY count drift: {counts}, expected (173, 1, 56)")
except (OSError, json.JSONDecodeError) as exc:
    issues.append(f"SERVED INVENTORY unreadable: {exc}")

summary = {
    "documents_read": len(docs),
    "lines_read": sum(len(value.splitlines()) for value in docs.values()),
    "sha256_document_index": hashlib.sha256(
        "\n".join(
            f"{key}\t{hashlib.sha256(value.encode()).hexdigest()}"
            for key, value in sorted(docs.items())
        ).encode()
    ).hexdigest(),
    "contract_assertions": 36,
    "issues": issues,
}
destination = ROOT / "docs/audit/v1-cross-contract-guard.json"
destination.parent.mkdir(parents=True, exist_ok=True)
destination.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
print(json.dumps(summary, indent=2))
if issues:
    raise SystemExit(1)
