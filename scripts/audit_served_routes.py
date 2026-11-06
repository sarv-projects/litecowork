#!/usr/bin/env python3
"""Compare the composed Operator router with the published OpenAPI contract.

The scanner starts at build_operator_router, follows only the exact subrouter
functions merged by it, and recursively follows same-file helper functions that
return an Axum Router and are called from those builders. Test-only routers and
unreferenced route builders are excluded. This proves source registration only;
handler integration and product qualification are separate gates.
"""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
DAEMON = ROOT / "apps/litecoworkd/src"
OPENAPI = ROOT / "docs/schemas/operator-api.openapi.yaml"
INVENTORY = ROOT / "docs/schemas/served-route-inventory.json"
HTTP_METHODS = ("get", "post", "put", "patch", "delete")


def canonical(path: str) -> str:
    return re.sub(r"\{[^}]+\}", "{}", path)


def openapi_operations() -> dict[tuple[str, str], dict[str, str]]:
    spec = yaml.safe_load(OPENAPI.read_text(encoding="utf-8-sig"))
    operations = {}
    for path, methods in spec.get("paths", {}).items():
        for method, data in methods.items():
            if method in HTTP_METHODS and isinstance(data, dict):
                operations[(method.upper(), canonical(path))] = {
                    "path": path,
                    "operation_id": data.get("operationId", ""),
                    "summary": str(data.get("summary", ""))[:160],
                }
    return operations


def rust_block_end(source: str, open_brace: int) -> int:
    depth = 0
    index = open_brace
    state = "code"
    block_comment_depth = 0
    while index < len(source):
        char = source[index]
        nxt = source[index + 1] if index + 1 < len(source) else ""
        if state == "line_comment":
            if char == "\n":
                state = "code"
        elif state == "block_comment":
            if char == "/" and nxt == "*":
                block_comment_depth += 1
                index += 1
            elif char == "*" and nxt == "/":
                block_comment_depth -= 1
                index += 1
                if block_comment_depth == 0:
                    state = "code"
        elif state == "string":
            if char == "\\":
                index += 1
            elif char == '"':
                state = "code"
        else:
            if char == "/" and nxt == "/":
                state = "line_comment"
                index += 1
            elif char == "/" and nxt == "*":
                state = "block_comment"
                block_comment_depth = 1
                index += 1
            elif char == '"':
                state = "string"
            elif char == "{":
                depth += 1
            elif char == "}":
                depth -= 1
                if depth == 0:
                    return index + 1
        index += 1
    raise ValueError("unterminated Rust block")


def function_body(source: str, name: str) -> str:
    pattern = rf"\bfn\s+{re.escape(name)}\s*(?:<[^{{}};]*>)?\s*\("
    match = re.search(pattern, source)
    if match is None:
        raise ValueError(f"function {name} not found")
    open_brace = source.find("{", match.end())
    if open_brace < 0:
        raise ValueError(f"function {name} has no body")
    end = rust_block_end(source, open_brace)
    return source[open_brace + 1 : end - 1]


def route_expression_end(body: str, open_paren: int) -> int:
    depth = 0
    index = open_paren
    state = "code"
    block_comment_depth = 0
    while index < len(body):
        char = body[index]
        nxt = body[index + 1] if index + 1 < len(body) else ""
        if state == "line_comment":
            if char == "\n":
                state = "code"
        elif state == "block_comment":
            if char == "/" and nxt == "*":
                block_comment_depth += 1
                index += 1
            elif char == "*" and nxt == "/":
                block_comment_depth -= 1
                index += 1
                if block_comment_depth == 0:
                    state = "code"
        elif state == "string":
            if char == "\\":
                index += 1
            elif char == '"':
                state = "code"
        else:
            if char == "/" and nxt == "/":
                state = "line_comment"
                index += 1
            elif char == "/" and nxt == "*":
                state = "block_comment"
                block_comment_depth = 1
                index += 1
            elif char == '"':
                state = "string"
            elif char == "(":
                depth += 1
            elif char == ")":
                depth -= 1
                if depth == 0:
                    return index + 1
        index += 1
    raise ValueError("unterminated route expression")


def registrations(body: str, source_path: str) -> list[dict[str, str]]:
    found = []
    for match in re.finditer(r'\.route\s*\(\s*"([^"]+)"\s*,', body):
        path = match.group(1)
        open_paren = body.find("(", match.start())
        end = route_expression_end(body, open_paren)
        expression = body[match.start():end]
        for method in HTTP_METHODS:
            if re.search(rf"\b{method}\s*\(", expression):
                found.append({
                    "method": method.upper(),
                    "path": path,
                    "source": source_path,
                })
    return found


def router_function_names(source: str) -> set[str]:
    """Find same-file functions whose declared return type contains Router."""
    names = set()
    for match in re.finditer(r"\bfn\s+([A-Za-z_]\w*)\s*(?:<[^{};]*>)?\s*\(", source):
        open_brace = source.find("{", match.end())
        if open_brace < 0:
            continue
        header = source[match.start():open_brace]
        if re.search(r"->[^{};]*\bRouter\b", header, re.S):
            names.add(match.group(1))
    return names


def router_closure(path: Path, start_functions: list[str]) -> list[dict[str, str]]:
    source = path.read_text(encoding="utf-8")
    router_names = router_function_names(source)
    pending = list(start_functions)
    visited: set[str] = set()
    found: list[dict[str, str]] = []
    source_path = str(path.relative_to(ROOT)).replace("\\", "/")

    while pending:
        name = pending.pop()
        if name in visited:
            continue
        visited.add(name)
        if name not in router_names and name not in start_functions:
            raise ValueError(f"{source_path}: {name} is not a Router builder")
        body = function_body(source, name)
        found.extend(registrations(body, source_path))
        for call in re.finditer(r"\b([A-Za-z_]\w*)\s*(?:::\s*<[^>]+>)?\s*\(", body):
            helper = call.group(1)
            if helper in router_names and helper not in visited:
                pending.append(helper)
    return found


def composed_route_builders() -> list[tuple[Path, list[str]]]:
    operator_path = DAEMON / "operator.rs"
    operator_source = operator_path.read_text(encoding="utf-8")
    root_body = function_body(operator_source, "build_operator_router")
    builders: list[tuple[Path, list[str]]] = [(operator_path, ["build_operator_router"])]

    # Exact qualified subrouters merged by the production root.
    for match in re.finditer(
        r"\.merge\(\s*([A-Za-z_]\w*)::([A-Za-z_]\w*)\s*\(", root_body
    ):
        module, function = match.groups()
        builders.append((DAEMON / f"{module}.rs", [function]))

    # Same-file route builders merged by the production root.
    for match in re.finditer(r"\.merge\(\s*([A-Za-z_]\w*)\s*\(", root_body):
        function = match.group(1)
        if function not in {"routes", "router"}:
            builders.append((operator_path, [function]))

    unique = []
    seen = set()
    for path, names in builders:
        key = (str(path), tuple(names))
        if key not in seen:
            seen.add(key)
            unique.append((path, names))
    return unique


def served_routes() -> list[dict[str, str]]:
    found = []
    for path, functions in composed_route_builders():
        if not path.exists():
            raise FileNotFoundError(path)
        found.extend(router_closure(path, functions))
    unique = {}
    for entry in found:
        unique.setdefault((entry["method"], entry["path"]), entry)
    return [unique[key] for key in sorted(unique)]


def build_inventory() -> dict[str, object]:
    published = openapi_operations()
    served = served_routes()
    served_records = []
    outside = []
    served_keys = set()
    for entry in served:
        path = entry["path"]
        if not path.startswith("/v1/"):
            continue
        openapi_path = path[len("/v1"):]
        key = (entry["method"], canonical(openapi_path))
        served_keys.add(key)
        operation = published.get(key)
        if operation is None:
            outside.append({
                "method": entry["method"],
                "path": path,
                "source": entry["source"],
            })
            continue
        served_records.append({
            "method": entry["method"],
            "path": path,
            "openapi_path": operation["path"],
            "operation_id": operation["operation_id"],
            "source": entry["source"],
        })
    target_only = [
        {
            "method": method,
            "path": operation["path"],
            "operation_id": operation["operation_id"],
        }
        for (method, _), operation in sorted(published.items())
        if (method, canonical(operation["path"])) not in served_keys
    ]
    return {
        "version": 3,
        "inventory_method": "composed_router_and_called_same_file_router_helpers",
        "served": served_records,
        "served_outside_openapi": outside,
        "target_only": target_only,
    }


def summarize(inventory: dict[str, object]) -> str:
    served = inventory["served"]
    outside = inventory["served_outside_openapi"]
    target_only = inventory["target_only"]
    assert isinstance(served, list) and isinstance(outside, list) and isinstance(target_only, list)
    return (
        f"served_openapi={len(served)} "
        f"served_outside_openapi={len(outside)} "
        f"target_only={len(target_only)}"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--report", action="store_true")
    args = parser.parse_args()
    inventory = build_inventory()
    if args.write:
        INVENTORY.parent.mkdir(parents=True, exist_ok=True)
        INVENTORY.write_text(json.dumps(inventory, indent=2) + "\n", encoding="utf-8")
        print(f"Wrote {INVENTORY.relative_to(ROOT)}: {summarize(inventory)}")
        return 0
    if not INVENTORY.exists():
        print("ERROR: docs/schemas/served-route-inventory.json is missing; run --write")
        return 1
    recorded = json.loads(INVENTORY.read_text(encoding="utf-8"))
    if recorded == inventory:
        print(f"Served-route inventory matches the composed router: {summarize(inventory)}")
        return 0
    for label in ("served", "served_outside_openapi", "target_only"):
        before = {(x["method"], x["path"]) for x in recorded.get(label, [])}
        after = {(x["method"], x["path"]) for x in inventory[label]}
        for item in sorted(after - before):
            print(f"ERROR: {label} route is not recorded: {item[0]} {item[1]}")
        for item in sorted(before - after):
            print(f"ERROR: recorded {label} route no longer exists: {item[0]} {item[1]}")
    print("ERROR: docs/schemas/served-route-inventory.json is stale; run --write and review")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
