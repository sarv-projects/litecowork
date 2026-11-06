"""Audit current V1 owner, OpenAPI, served-route and UI contract alignment.

This is a deterministic structural check. It does not prove handler semantics,
provider integration, OS security or USER qualification.
"""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs"
SCHEMA = DOCS / "schemas" / "operator-api.openapi.yaml"
INVENTORY = DOCS / "schemas" / "served-route-inventory.json"
OUTPUT = DOCS / "audit" / "v1-contract-audit.md"


def load_yaml(path: Path):
    return yaml.safe_load(path.read_text(encoding="utf-8-sig"))


def canonical(path: str) -> str:
    return re.sub(r"\{[^}]+\}", "{}", path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--strict", action="store_true")
    parser.add_argument("--write-report", action="store_true")
    args = parser.parse_args()

    errors: list[str] = []
    notes: list[str] = []
    api = load_yaml(SCHEMA)
    inventory = json.loads(INVENTORY.read_text(encoding="utf-8"))

    openapi = {
        (method.upper(), canonical(path)): operation
        for path, methods in api.get("paths", {}).items()
        for method, operation in methods.items()
        if method in {"get", "post", "put", "patch", "delete"}
    }
    served = {
        (item["method"], canonical(item["openapi_path"]))
        for item in inventory.get("served", [])
    }
    target = {
        (item["method"], canonical(item["path"]))
        for item in inventory.get("target_only", [])
    }
    outside = inventory.get("served_outside_openapi", [])

    if served & target:
        errors.append("An operation is listed as both SERVED and target-only.")
    if served | target != set(openapi):
        errors.append(
            "Served + target-only inventory does not partition OpenAPI operations: "
            f"served_only={len(set(openapi)-served-target)}, "
            f"unclassified={len(set(openapi)-(served|target))}, "
            f"unknown={len((served|target)-set(openapi))}"
        )
    counts = (len(inventory.get("served", [])), len(outside), len(inventory.get("target_only", [])))
    if counts != (173, 1, 56):
        errors.append(f"Route inventory count drift: {counts}; expected (173, 1, 56)")
    if inventory.get("version") != 3:
        errors.append(f"Unexpected route inventory version: {inventory.get('version')}")

    required_operations = {
        ("GET", "/conversations"): "listConversations",
        ("POST", "/conversations"): "createConversation",
        ("POST", "/conversations/{conversationId}/turns"): "submitConversationTurn",
        ("POST", "/conversations/{conversationId}/turns/{turnId}/cancel"): "cancelConversationTurn",
        ("POST", "/conversations/{conversationId}/turns/{turnId}/retry"): "retryConversationTurn",
        ("GET", "/needs-you"): "listNeedsYou",
        ("GET", "/tasks"): "listTasks",
        ("POST", "/tasks"): "createTask",
        ("GET", "/tasks/{taskId}/planning-readiness"): "getTaskPlanningReadiness",
    }
    for raw_key, operation_id in required_operations.items():
        key = (raw_key[0], canonical(raw_key[1]))
        if key not in openapi:
            errors.append(f"Required OpenAPI operation missing: {raw_key}")
        elif openapi[key].get("operationId") != operation_id:
            errors.append(
                f"Operation ID mismatch for {raw_key}: "
                f"{openapi[key].get('operationId')} != {operation_id}"
            )
        if key not in served:
            errors.append(f"Required user-facing operation is not registered: {raw_key}")

    ui = (DOCS / "V1-UI-SCREEN-CONTRACT.md").read_text(encoding="utf-8")
    semantic = (DOCS / "SEMANTIC-SERVED-UI-MAP.md").read_text(encoding="utf-8")
    coverage = (DOCS / "FEATURE-AND-UI-CONTRACT-COVERAGE.md").read_text(encoding="utf-8")
    for heading in (
        "## 2. Conversation and composer",
        "## 3. Work and Task detail",
        "## 4. Needs You",
        "## 5. Coworkers",
        "## 6. Automations and Routines",
        "## 7. Discover, LiteSPM and Connections",
        "## 8. Library and Artifacts",
        "## 9. Workbench",
        "## 11. Settings",
    ):
        if heading not in ui:
            errors.append(f"UI screen contract missing section: {heading}")
    for required in (
        "submit_conversation_turn",
        "create_task",
        "get_task_planning_readiness",
        "connectOrInstallCapability",
        "Simulated output",
        "Component-local useState values",
        "settleAutomationTriggers",
    ):
        if required not in semantic:
            errors.append(f"Semantic UI map missing implementation trace: {required}")
    for area in ("Universal Chat", "Bring Your Own Agent", "Persistent Coworkers",
                 "Real Computer and Workbench", "Capabilities, Plugins, MCP and Connections",
                 "Real Tasks and Multi-Agent Work", "Routines and Automations",
                 "Memory, Knowledge and Learning", "Library and Finished Work",
                 "Desktop Shell, Settings and Runtime"):
        if area not in coverage:
            errors.append(f"Feature/UI coverage missing V1 area: {area}")

    feature_register = (DOCS / "V1-FEATURE-SDD-REGISTER.md").read_text(encoding="utf-8")
    for number in range(1, 12):
        if not re.search(rf"(?m)^\|\s*{number:03d}\s*\|", feature_register):
            errors.append(f"Feature register missing SDD {number:03d}")
    for number in range(1, 12):
        packages = list((ROOT / "specs").glob(f"{number:03d}-*"))
        if len(packages) != 1:
            errors.append(f"Expected one package directory for SDD {number:03d}, found {len(packages)}")

    for item in outside:
        if item["path"] != "/v1/stream":
            notes.append(f"Served outside OpenAPI: {item['method']} {item['path']}")
    notes += [
        f"OpenAPI operations: {len(openapi)}",
        f"Composed Operator OpenAPI registrations: {counts[0]}",
        f"Authenticated local-only registrations: {counts[1]}",
        f"OpenAPI target-only operations: {counts[2]}",
        "Route registration is not handler integration or release qualification.",
    ]

    lines = [
        "# V1 contract and route/UI audit",
        "",
        "Generated by scripts/audit_v1_contracts.py. Structural consistency only; not semantic or runtime qualification.",
        "",
        f"- Blocking findings: {len(errors)}",
        "",
        "## Findings",
        "",
    ]
    lines.extend("- " + item for item in errors)
    if not errors:
        lines.append("No structural mismatch detected by this audit.")
    lines += ["", "## Source observations", ""]
    lines.extend("- " + item for item in notes)
    lines += [
        "",
        "## Route-state counts",
        "",
        "| Class | Count |",
        "|---|---:|",
        f"| Registered OpenAPI operations | {counts[0]} |",
        f"| Authenticated local-only operations | {counts[1]} |",
        f"| OpenAPI target-only operations | {counts[2]} |",
        "",
        "See docs/IMPLEMENTATION-STATUS.md and docs/SEMANTIC-SERVED-UI-MAP.md for handler and UI behavior. A mounted route may be read-only, fail-closed, pre-execution or provider-unavailable.",
        "",
    ]
    if args.write_report:
        OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        OUTPUT.write_text("\n".join(lines), encoding="utf-8")
    print(
        f"V1_CONTRACT_AUDIT errors={len(errors)} openapi={len(openapi)} "
        f"served={counts[0]} outside={counts[1]} target={counts[2]}"
    )
    for item in errors[:30]:
        print("ERROR", item)
    return 1 if errors and args.strict else 0


if __name__ == "__main__":
    raise SystemExit(main())
