#!/usr/bin/env python3
"""Validate structural completeness of LiteCowork feature SDD packages.

This documentation-only validator uses the Python standard library. It does not
approve a specification, validate runtime behavior, or replace owner review.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPECS = ROOT / "specs"

REQUIRED = (
    "README.md",
    "SPEC.md",
    "PLAN.md",
    "TASKS.md",
    "DATA-MODEL.md",
    "RESEARCH.md",
    "WIRING.yaml",
    "contracts/README.md",
)

SECTIONS = {
    "problem": r"Problem",
    "goals": r"Goals",
    "non-goals": r"Non-goals",
    "HLD": r"High-level design",
    "LLD": r"Low-level design",
    "security": r"Security",
    "errors/recovery": r"Errors|edge cases|recovery",
    "configuration": r"Configuration|compatibility",
    "EARS acceptance": r"EARS acceptance",
    "executable command": r"Executable acceptance command",
    "out-of-bounds": r"Out-of-bounds",
    "assumptions": r"Assumptions",
    "traceability": r"Traceability",
}

MD_LINK = re.compile(r"(?<!)\[[^\]]+\]\(([^)]+)\)")
TASK_ID = re.compile(r"\b(\d{3}-T\d{2})\b")
AC_ID = re.compile(r"\b(AC-\d{3})\b")
TASK_HEADING = re.compile(r"(?m)^##\s+(\d{3}-T\d{2})\b")
NODE_ID = re.compile(r'^\s+- id:\s*"([^"]+)"\s*$', re.M)
EDGE_ID = re.compile(r'^\s+- id:\s*"([^"]+)"\s*$', re.M)
TRANSACTION_ID = re.compile(r'^  - id:\s*"([^"]+)"\s*$', re.M)
EDGE = re.compile(
    r'^\s+- id:\s*"[^"]+"\s*[\r\n]+\s+from:\s*"([^"]+)"\s*[\r\n]+\s+to:\s*"([^"]+)"',
    re.M,
)
DEPENDENCIES = re.compile(r"(?m)^-\s*\*\*Depends on:\*\*\s*(.+?)\s*$")


def check_links(folder: Path) -> list[str]:
    errors: list[str] = []
    for path in sorted(folder.rglob("*.md")):
        if not path.is_file():
            continue
        rel = path.relative_to(folder).as_posix()
        body = path.read_text(encoding="utf-8")
        for target in MD_LINK.findall(body):
            target = target.strip().split("#", 1)[0].split("?", 1)[0]
            if not target or target.startswith(("https://", "http://", "mailto:", "tel:")):
                continue
            if not (path.parent / target).resolve().exists():
                errors.append(f"{folder.name}: broken link in {rel}: {target}")
    return errors


def check_task_graph(folder: Path, tasks: str) -> list[str]:
    errors: list[str] = []
    feature_match = re.match(r"(\d{3})-", folder.name)
    if not feature_match:
        return [f"{folder.name}: feature folder must begin with a three-digit ID"]
    prefix = feature_match.group(1)

    matches = list(TASK_HEADING.finditer(tasks))
    if not matches:
        return [f"{folder.name}: TASKS has no stable task headings"]

    graph: dict[str, set[str]] = {}
    for index, match in enumerate(matches):
        task_id = match.group(1)
        start = match.start()
        end = matches[index + 1].start() if index + 1 < len(matches) else len(tasks)
        block = tasks[start:end]
        required_fields = (
            "Status:",
            "Depends on:",
            "Owner",
            "Allowed paths:",
            "Contract:",
            "Change intent:",
            "Acceptance:",
            "Command:",
        )
        for field in required_fields:
            if field not in block:
                errors.append(f"{folder.name}: {task_id} missing {field}")

        dep_match = DEPENDENCIES.search(block)
        if not dep_match:
            graph[task_id] = set()
            continue
        dep_text = dep_match.group(1).strip().rstrip('.')
        deps: set[str] = set()
        if dep_text.upper() != "NONE":
            tokens = re.findall(r"\bT(\d{2})(?:-T(\d{2}))?\b", dep_text)
            if not tokens:
                errors.append(f"{folder.name}: {task_id} has unparseable dependencies: {dep_text}")
            for first, last in tokens:
                begin = int(first)
                finish = int(last or first)
                if finish < begin:
                    errors.append(f"{folder.name}: {task_id} has descending dependency range {dep_text}")
                    continue
                for number in range(begin, finish + 1):
                    deps.add(f"{prefix}-T{number:02d}")
        graph[task_id] = deps

    known = set(graph)
    for task_id, deps in graph.items():
        if task_id in deps:
            errors.append(f"{folder.name}: {task_id} depends on itself")
        for dep in sorted(deps):
            if dep not in known:
                errors.append(f"{folder.name}: {task_id} depends on missing task {dep}")

    visited: set[str] = set()
    visiting: set[str] = set()

    def visit(task_id: str) -> None:
        if task_id in visited or task_id not in graph:
            return
        if task_id in visiting:
            errors.append(f"{folder.name}: task dependency cycle includes {task_id}")
            return
        visiting.add(task_id)
        for dep in graph[task_id]:
            visit(dep)
        visiting.remove(task_id)
        visited.add(task_id)

    for task_id in sorted(graph):
        visit(task_id)
    return errors


def check_feature(folder: Path) -> list[str]:
    errors: list[str] = []
    for rel in REQUIRED:
        if not (folder / rel).is_file():
            errors.append(f"{folder.name}: missing {rel}")

    spec_path = folder / "SPEC.md"
    tasks_path = folder / "TASKS.md"
    wiring_path = folder / "WIRING.yaml"
    if any(not path.is_file() for path in (spec_path, tasks_path, wiring_path)):
        return errors

    spec = spec_path.read_text(encoding="utf-8")
    tasks = tasks_path.read_text(encoding="utf-8")
    wiring = wiring_path.read_text(encoding="utf-8")

    if not re.search(
        r"\*\*Status:\*\*\s*(DRAFT|IN_REVIEW|ACCEPTED/LOCKED|IMPLEMENTING|VERIFIED|BLOCKED|SUPERSEDED)",
        spec,
    ):
        errors.append(f"{folder.name}: SPEC has no recognized status")

    for label, pattern in SECTIONS.items():
        if not re.search(pattern, spec, re.I):
            errors.append(f"{folder.name}: SPEC missing section {label}")

    acceptance_ids = set(AC_ID.findall(spec))
    if len(acceptance_ids) < 3:
        errors.append(f"{folder.name}: fewer than 3 EARS acceptance IDs")

    bash_fence = chr(96) * 3 + "bash"
    if spec.count(bash_fence) != 1:
        errors.append(f"{folder.name}: SPEC must identify exactly one canonical fenced bash acceptance command")

    task_ids = TASK_ID.findall(tasks)
    if not task_ids:
        errors.append(f"{folder.name}: TASKS has no stable task IDs")
    if len(task_ids) != len(set(task_ids)):
        errors.append(f"{folder.name}: duplicate task IDs")

    spec_task_ids = set(TASK_ID.findall(spec))
    for task_id in sorted(spec_task_ids - set(task_ids)):
        errors.append(f"{folder.name}: SPEC references missing task {task_id}")

    task_acceptance_ids = set(AC_ID.findall(tasks))
    for ac_id in sorted(task_acceptance_ids - acceptance_ids):
        errors.append(f"{folder.name}: TASKS references unknown acceptance ID {ac_id}")

    trace_match = re.search(r"(?ms)^##\s+\d+\.\s+Requirement traceability\s*\n(.*)$", spec)
    if not trace_match:
        errors.append(f"{folder.name}: SPEC has no requirement traceability table")
    else:
        trace_ids = set(AC_ID.findall(trace_match.group(1)))
        for ac_id in sorted(acceptance_ids - trace_ids):
            errors.append(f"{folder.name}: acceptance criterion {ac_id} is missing from requirement traceability")

    errors.extend(check_links(folder))
    errors.extend(check_task_graph(folder, tasks))

    node_section = wiring.split("edges:", 1)[0]
    node_ids = NODE_ID.findall(node_section)
    if not node_ids:
        errors.append(f"{folder.name}: WIRING has no nodes")
    if len(node_ids) != len(set(node_ids)):
        errors.append(f"{folder.name}: duplicate WIRING node IDs")

    if "edges:" not in wiring or "transactions:" not in wiring:
        errors.append(f"{folder.name}: WIRING must declare edges and transactions")
    else:
        edge_section = wiring.split("edges:", 1)[1].split("transactions:", 1)[0]
        edge_ids = EDGE_ID.findall(edge_section)
        if len(edge_ids) != len(set(edge_ids)):
            errors.append(f"{folder.name}: duplicate WIRING edge IDs")
        for source, target in EDGE.findall(edge_section):
            if source not in node_ids:
                errors.append(f"{folder.name}: WIRING edge source is undefined: {source}")
            if target not in node_ids:
                errors.append(f"{folder.name}: WIRING edge target is undefined: {target}")

        transaction_section = wiring.split("transactions:", 1)[1].split("external_boundaries:", 1)[0]
        transaction_matches = list(TRANSACTION_ID.finditer(transaction_section))
        transaction_ids = [match.group(1) for match in transaction_matches]
        if not transaction_matches:
            errors.append(f"{folder.name}: WIRING has no transaction declarations")
        if len(transaction_ids) != len(set(transaction_ids)):
            errors.append(f"{folder.name}: duplicate WIRING transaction IDs")
        for index, match in enumerate(transaction_matches):
            end = transaction_matches[index + 1].start() if index + 1 < len(transaction_matches) else len(transaction_section)
            block = transaction_section[match.end():end]
            for field in ("owner:", "atomic_records:", "external_calls_inside:"):
                if field not in block:
                    errors.append(f"{folder.name}: WIRING transaction {match.group(1)} missing {field}")
            if not re.search(r"(?m)^    atomic_records:\s*\[[^]]+\]\s*$", block):
                errors.append(f"{folder.name}: WIRING transaction {match.group(1)} has no non-empty atomic_records list")
            if not re.search(r"(?m)^    external_calls_inside:\s*false\s*$", block):
                errors.append(f"{folder.name}: WIRING transaction {match.group(1)} must set external_calls_inside: false")

    if re.search(r"(?m)^\s*external_calls_inside:\s*true\s*$", wiring):
        errors.append(f"{folder.name}: external call placed inside transaction")
    if not re.search(r'status:\s*"DRAFT"', wiring):
        errors.append(f"{folder.name}: WIRING status must remain DRAFT until acceptance")

    return errors


def main() -> int:
    if not SPECS.is_dir():
        print("ERROR: specs/ directory missing")
        return 2
    folders = sorted(path for path in SPECS.iterdir() if path.is_dir() and not path.name.startswith("."))
    errors: list[str] = []
    for folder in folders:
        errors.extend(check_feature(folder))
    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        print(f"FAILED: {len(errors)} issue(s) across {len(folders)} feature package(s)")
        return 1
    print(
        f"PASS: {len(folders)} feature package(s); required files, SPEC sections, "
        "one canonical bash command, all-package Markdown links, task fields/dependencies, "
        "acceptance traceability, WIRING edges and transaction boundaries validated"
    )
    print("NOTE: this is structural validation only; it is not owner approval or runtime qualification.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
