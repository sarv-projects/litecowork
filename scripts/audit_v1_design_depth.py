"""Active V1 design-depth and ownership coverage inventory.

This audit inspects current docs, OpenAPI, served-route inventory, UI contracts and
feature packages. It does not claim that named functions or routes are integrated
or qualified.
"""
from __future__ import annotations

import argparse
import collections
import hashlib
import json
import os
import re
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs"
SKIP = {
    ".git", "node_modules", "target", ".venv", ".pytest_cache", "dist", "build",
    "coverage", ".cache", ".next", ".zwork", ".worktrees", ".agent-worktrees",
    "archives", "archive", "repo_take", "reading", "staging_tar", "audit",
}
OWNER = {
    "Product and release": ("docs/PRODUCT.md", "docs/V1-PRODUCT-RELEASE-CONTRACT.md", "docs/PRODUCT-STRATEGY.md"),
    "System architecture": ("ARCHITECTURE.md", "docs/DATA-MODEL.md", "docs/STATE-MACHINES.md"),
    "Native agents": ("docs/AGENT-FABRIC.md", "docs/AGENT-CONTROL.md", "docs/AGENT-ACCOUNT-ROUTE-MATRIX.md"),
    "Conversation": ("docs/EXPERIENCE.md", "docs/API.md", "docs/NATIVE-COMPOSER-LLD.md", "docs/STATE-MACHINES.md"),
    "Tasks and delegation": ("docs/TASK-RUNTIME.md", "docs/DELEGATION.md", "docs/TRUST.md", "docs/ENVIRONMENTS.md"),
    "Workspace and Resources": ("docs/WORLD-RESOURCES.md", "docs/CONTEXT.md", "docs/STORAGE.md"),
    "Capabilities and LiteSPM": ("docs/CAPABILITY-FABRIC.md", "docs/CAPABILITY-INVOCATIONS.md", "docs/LITESPM-LITECOWORK-INTEROP-LLD.md"),
    "Coworkers and learning": ("docs/COWORKERS-TARGET.md", "docs/COWORKER-FLOWS.md", "docs/LEARNING.md", "docs/RESPONSIBILITIES.md"),
    "Automations": ("docs/AUTOMATION.md", "docs/ROUTINES.md", "docs/CHANNELS.md"),
    "Artifacts and presentation": ("docs/ARTIFACTS-EVIDENCE.md", "docs/PRESENTATION-RUNTIME.md", "docs/INTERACTIVE-ARTIFACT-RUNTIME.md"),
    "Workbench and shell": ("docs/DESKTOP-SHELL-LLD.md", "docs/CHROME-BROWSER-LLD.md", "docs/V1-INTEGRATED-TERMINAL-LLD.md"),
    "Recovery and release": ("docs/FAILURE-RECOVERY.md", "docs/TESTING.md", "docs/ACCEPTANCE-AND-E2E-PLAN.md"),
}
def read(rel: str) -> str:
    return (ROOT / rel).read_text(encoding="utf-8-sig")

def active_markdown():
    files = []
    for folder, dirs, names in os.walk(ROOT):
        dirs[:] = [name for name in dirs if name not in SKIP]
        for name in names:
            path = Path(folder) / name
            if path.suffix.lower() in {".md", ".mdx"} and path.relative_to(ROOT).as_posix() != "docs/COMPLETE-ACTIVE-CORPUS-INVENTORY.md":
                files.append(path)
    return sorted(files)

def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--strict", action="store_true")
    args = parser.parse_args()

    files = active_markdown()
    records = []
    for path in files:
        text = path.read_text(encoding="utf-8-sig")
        records.append((
            path.relative_to(ROOT).as_posix(),
            len(text.splitlines()),
            sum(1 for line in text.splitlines() if re.match(r"^#{1,6} ", line)),
            hashlib.sha256(path.read_bytes()).hexdigest()[:12],
        ))

    api = yaml.safe_load(read("docs/schemas/operator-api.openapi.yaml"))
    openapi_count = sum(
        1 for methods in api.get("paths", {}).values()
        for method in methods if method in {"get", "post", "put", "patch", "delete"}
    )
    inventory = json.loads(read("docs/schemas/served-route-inventory.json"))
    served_count = len(inventory.get("served", []))
    outside_count = len(inventory.get("served_outside_openapi", []))
    target_count = len(inventory.get("target_only", []))

    catalog = read("docs/V1-FUNCTION-FILE-CATALOG.md")
    function_rows = sum(
        1 for line in catalog.splitlines()
        if re.match(r"^[A-Za-z_]\w*\(.*\)\s*:", line)
    )
    settings_ledger = read("docs/V1-SETTINGS-CONTRACT-LEDGER.md")
    settings_rows = sum(
        1 for line in settings_ledger.splitlines()
        if line.startswith("|") and not re.match(r"^\|\s*:?-+", line) and not re.match(r"^\|\s*Control\s*\|", line, re.I)
    )
    migration = read("docs/schemas/sqlite-v1.sql")
    tables = set(re.findall(
        r"(?im)^CREATE TABLE(?: IF NOT EXISTS)?\s+([A-Za-z_]\w*)", migration
    ))
    package_count = len(list((ROOT / "specs").glob("[0-9][0-9][0-9]-*")))
    ui = read("docs/V1-UI-SCREEN-CONTRACT.md")
    ui_sections = re.findall(r"(?m)^## \d+\. (.+)$", ui)
    coverage = read("docs/V1-DESIGN-INDEX.md")

    errors = []
    for area, owner_files in OWNER.items():
        for rel in owner_files:
            if not (ROOT / rel).is_file():
                errors.append(f"{area}: missing owner document {rel}")
    if package_count != 11:
        errors.append(f"Expected 11 feature packages, found {package_count}")
    if openapi_count != served_count + target_count:
        errors.append(
            f"OpenAPI/served inventory mismatch: {openapi_count} != {served_count}+{target_count}"
        )
    if (served_count, outside_count, target_count) != (173, 1, 56):
        errors.append(
            f"Composed route count drift: {(served_count, outside_count, target_count)}"
        )
    for required in ("Conversation", "Task", "Coworker", "Automation", "Discover",
                     "Library", "Workbench", "Settings"):
        if required.lower() not in ui.lower():
            errors.append(f"UI screen contract missing {required} coverage")
    if "IMPLEMENTATION-STATUS.md" not in coverage:
        errors.append("V1 design index does not link current implementation status")

    owner_rows = []
    for area, paths in OWNER.items():
        owner_rows.append((area, paths))
    report = [
        "# Active V1 design-depth and ownership audit",
        "",
        "Generated by scripts/audit_v1_design_depth.py. This is an inventory and structural check,",
        "not a sentence-level semantic review or release qualification.",
        "",
        f"- First-party Markdown files read: {len(records)}",
        f"- First-party Markdown lines read: {sum(row[1] for row in records)}",
        f"- First-party headings: {sum(row[2] for row in records)}",
        f"- Domain owner rows: {len(OWNER)}",
        f"- Function catalog signatures: {function_rows}",
        f"- Settings ledger rows (including table separators/headers excluded): {settings_rows}",
        f"- SQLite tables in baseline schema: {len(tables)}",
        f"- OpenAPI operations: {openapi_count}",
        f"- Composed Operator OpenAPI registrations: {served_count}",
        f"- Local-only registrations outside OpenAPI: {outside_count}",
        f"- OpenAPI target-only operations: {target_count}",
        f"- UI screen sections: {len(ui_sections)}",
        f"- Feature packages: {package_count}",
        f"- Structural findings: {len(errors)}",
        "",
        "## Owner crosswalk",
        "",
        "| Area | Canonical owner documents |",
        "|---|---|",
    ]
    for area, paths in owner_rows:
        report.append(f"| {area} | " + "; ".join(paths) + " |")
    report += ["", "## Findings", ""]
    report.extend("- " + item for item in errors)
    if not errors:
        report.append("No structural ownership/route-count findings.")
    report += [
        "",
        "## UI sections",
        "",
        "| Section |",
        "|---|",
    ]
    report.extend(f"| {item} |" for item in ui_sections)
    report += [
        "",
        "## Current evidence boundary",
        "",
        "A route registration does not establish handler integration. A function name in a catalog does",
        "not prove that it is called. A UI section does not prove every control is enabled or backed by",
        "a durable owner operation. See docs/IMPLEMENTATION-STATUS.md and docs/SEMANTIC-SERVED-UI-MAP.md.",
        "",
        "## Per-file Markdown inventory",
        "",
        "| Path | Lines | Headings | SHA-256 prefix |",
        "|---|---:|---:|---|",
    ]
    for rel, lines_count, headings, digest in records:
        report.append(f"| {rel} | {lines_count} | {headings} | {digest} |")

    if args.write:
        output = DOCS / "audit" / "v1-design-depth-audit.md"
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text("\n".join(report) + "\n", encoding="utf-8")
    print(
        f"DESIGN_DEPTH files={len(records)} lines={sum(row[1] for row in records)} "
        f"domains={len(OWNER)} functions={function_rows} settings_rows={settings_rows} "
        f"routes={openapi_count} served={served_count} target={target_count} "
        f"tables={len(tables)} packages={package_count} findings={len(errors)}"
    )
    for item in errors:
        print("ERROR", item)
    return int(args.strict and bool(errors))

if __name__ == "__main__":
    raise SystemExit(main())
