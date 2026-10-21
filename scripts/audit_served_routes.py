#!/usr/bin/env python3
"""Compare the composed Operator router with the published OpenAPI contract.

The OpenAPI file is the *target* contract. This gate makes the difference between the
target contract and the *served* surface machine-checkable: every published operation is
either mounted by the daemon or recorded as an explicit target-only operation, and every
mounted route is either published or recorded as served outside OpenAPI.

Usage:
  scripts/audit_served_routes.py            verify the committed inventory
  scripts/audit_served_routes.py --write    regenerate implementation/served-route-inventory.json
  scripts/audit_served_routes.py --report   print the drift summary
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
INVENTORY = ROOT / "implementation/served-route-inventory.json"
HTTP_METHODS = ("get", "post", "put", "patch", "delete")


def canonical(path: str) -> str:
    """Compare route templates by shape: parameter names are not part of identity."""
    return re.sub(r"\{[^}]+\}", "{}", path)


def openapi_operations() -> dict[tuple[str, str], dict[str, str]]:
    spec = yaml.safe_load(OPENAPI.read_text(encoding="utf-8-sig"))
    operations: dict[tuple[str, str], dict[str, str]] = {}
    for path, methods in spec.get("paths", {}).items():
        for method, data in methods.items():
            if method not in HTTP_METHODS or not isinstance(data, dict):
                continue
            operations[(method.upper(), canonical(path))] = {
                "path": path,
                "operation_id": data.get("operationId", ""),
                "summary": str(data.get("summary", ""))[:160],
            }
    return operations


def served_routes() -> list[dict[str, str]]:
    """Read every `.route("...")` registration in the daemon sources."""
    found: list[dict[str, str]] = []
    for source in sorted(DAEMON.rglob("*.rs")):
        # Files whose name marks them as a test module register fixture routes only.
        if source.stem.endswith("_tests") or source.stem.endswith("_test"):
            continue
        text = source.read_text(encoding="utf-8")
        # A `#[cfg(test)]` module registers fixture routes that are not part of the
        # production surface, so only the production part of each file is read. The
        # cut is at the first test *module declaration*, not at any test attribute.
        test_module = re.search(
            r"#\[cfg\([^]]*test[^]]*\)\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s", text
        )
        production = text if test_module is None else text[: test_module.start()]
        matches = list(re.finditer(r'\.route\(\s*"([^"]+)"', production))
        for index, match in enumerate(matches):
            end = matches[index + 1].start() if index + 1 < len(matches) else len(text)
            window = text[match.end() : min(end, match.end() + 400)]
            path = match.group(1)
            # One registration may serve several methods (`.route(p, get(x).post(y))`).
            methods = [
                candidate
                for candidate in HTTP_METHODS
                if re.search(rf"\b{candidate}\(", window)
            ]
            for candidate in methods:
                found.append(
                    {
                        "method": candidate.upper(),
                        "path": path,
                        "source": str(source.relative_to(ROOT)),
                    }
                )
    unique: dict[tuple[str, str], dict[str, str]] = {}
    for entry in found:
        unique.setdefault((entry["method"], entry["path"]), entry)
    return [unique[key] for key in sorted(unique)]


def build_inventory() -> dict[str, object]:
    published = openapi_operations()
    served = served_routes()
    served_records: list[dict[str, str]] = []
    outside: list[dict[str, str]] = []
    served_keys: set[tuple[str, str]] = set()
    for entry in served:
        path = entry["path"]
        if not path.startswith("/v1/"):
            continue
        openapi_path = path[len("/v1") :]
        key = (entry["method"], canonical(openapi_path))
        served_keys.add(key)
        operation = published.get(key)
        if operation is None:
            outside.append(
                {
                    "method": entry["method"],
                    "path": path,
                    "source": entry["source"],
                }
            )
            continue
        served_records.append(
            {
                "method": entry["method"],
                "path": path,
                "openapi_path": operation["path"],
                "operation_id": operation["operation_id"],
            }
        )
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
        "version": 1,
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
        INVENTORY.write_text(
            json.dumps(inventory, indent=2, sort_keys=False) + "\n", encoding="utf-8"
        )
        print(f"Wrote {INVENTORY.relative_to(ROOT)}: {summarize(inventory)}")
        return 0

    if not INVENTORY.exists():
        print(
            "ERROR: implementation/served-route-inventory.json is missing; "
            "run scripts/audit_served_routes.py --write",
        )
        return 1
    recorded = json.loads(INVENTORY.read_text(encoding="utf-8"))
    if recorded == inventory:
        print(f"Served-route inventory matches the daemon router: {summarize(inventory)}")
        return 0

    for label, key in (
        ("served", "served"),
        ("served_outside_openapi", "served_outside_openapi"),
        ("target_only", "target_only"),
    ):
        before = {
            (entry["method"], entry["path"]) for entry in recorded.get(key, [])  # type: ignore[union-attr]
        }
        after = {(entry["method"], entry["path"]) for entry in inventory[key]}  # type: ignore[index]
        for missing in sorted(after - before):
            print(f"ERROR: {label} route is not recorded: {missing[0]} {missing[1]}")
        for extra in sorted(before - after):
            print(f"ERROR: recorded {label} route no longer exists: {extra[0]} {extra[1]}")
    print(
        "ERROR: implementation/served-route-inventory.json is stale; "
        "run scripts/audit_served_routes.py --write and review the diff"
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
