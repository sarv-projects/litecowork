#!/usr/bin/env python3
"""Validate the implementation backlog's traceability to architecture contracts.

Run with --write to refresh the generated coverage map and inventory after a reviewed
contract change. Default mode is read-only and fails when either generated file is stale.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import re
import sqlite3
import subprocess
import sys
from collections import Counter
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
PLAN = ROOT / "implementation"
INVENTORY_PATH = PLAN / "machine-inventory.json"
COVERAGE_PATH = PLAN / "coverage.csv"
REPORT_PATH = PLAN / "ARCHITECTURE-COVERAGE.md"


def fail(message: str) -> None:
    print("ERROR:", message, file=sys.stderr)
    raise SystemExit(1)


def sha256(value: bytes | str) -> str:
    if isinstance(value, str):
        value = value.encode("utf-8")
    return hashlib.sha256(value).hexdigest()


def canonical_digest(value: object) -> str:
    return sha256(json.dumps(value, sort_keys=True, separators=(",", ":")))


def schema_story(name: str) -> str:
    low = re.sub(r"([a-z])([A-Z])", r"\1_\2", name).lower()
    if any(x in low for x in ("delegation", "worker", "lead_failover", "quota", "cost", "budget")):
        return "E05-S03" if any(x in low for x in ("quota", "cost", "budget", "performance")) else "E05-S01"
    if any(x in low for x in ("coworker", "goal", "suggestion", "context")):
        return "E08-S03" if "context" in low else "E08-S02" if any(x in low for x in ("goal", "suggestion")) else "E08-S01"
    if any(x in low for x in ("environment", "runtime", "handoff", "pairing", "replication")):
        return "E11-S03" if "handoff" in low else "E11-S02" if any(x in low for x in ("pairing", "replication")) else "E07-S01"
    if any(x in low for x in ("artifact", "evidence", "verification", "effect", "approval", "grant", "secret", "invocation", "capability")):
        return "E04-S05" if any(x in low for x in ("artifact", "evidence", "verification")) else "E04-S03" if any(x in low for x in ("effect", "invocation")) else "E04-S01" if any(x in low for x in ("approval", "grant", "secret")) else "E04-S02"
    if any(x in low for x in ("resource", "upload", "search", "document")):
        return "E06-S01" if "upload" in low else "E02-S03"
    if any(x in low for x in ("routine", "automation", "occurrence", "trigger")):
        return "E09-S02" if any(x in low for x in ("automation", "occurrence", "trigger")) else "E09-S01"
    if any(x in low for x in ("channel", "user_request", "provider_input")):
        return "E11-S04"
    if any(x in low for x in ("agent", "session", "model")):
        return "E03-S01"
    if any(x in low for x in ("task", "step", "attempt", "plan", "conversation", "turn")):
        return "E03-S02" if any(x in low for x in ("conversation", "turn")) else "E03-S04"
    return "E01-S04"


def sql_quote(identifier: str) -> str:
    return '"' + identifier.replace('"', '""') + '"'


def extract_check_expressions(body: str) -> list[str]:
    """Return balanced CHECK expressions, preserving their source for hashing."""
    result: list[str] = []
    for match in re.finditer(r"\bCHECK\s*\(", body, re.I):
        opening = body.find("(", match.start(), match.end())
        depth = 0
        quote: str | None = None
        i = opening
        while i < len(body):
            char = body[i]
            if quote:
                if char == quote:
                    if i + 1 < len(body) and body[i + 1] == quote and quote in ("'", '"'):
                        i += 2
                        continue
                    quote = None
                elif char == "\\" and quote == "'":
                    i += 2
                    continue
            elif char in ("'", '"', "`"):
                quote = char
            elif char == "[":
                quote = "]"
            elif char == "(":
                depth += 1
            elif char == ")":
                depth -= 1
                if depth == 0:
                    result.append(body[opening + 1 : i].strip())
                    break
            i += 1
        else:
            fail("unbalanced CHECK expression in sqlite-v1.sql")
    return result


def schema_docs() -> list[str]:
    tracked = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "--", "*.md"],
        cwd=ROOT,
        text=True,
    ).splitlines()
    return sorted(
        path
        for path in tracked
        if not path.startswith(
            ("implementation/", "archive_code/", "archives_docs/", "ARCHIVE/", "archives/")
        )
    )


def implementation_plan_docs() -> list[str]:
    excluded = {"implementation/ARCHITECTURE-COVERAGE.md", "implementation/CURRENT-RUN.md"}
    return sorted(
        path.relative_to(ROOT).as_posix()
        for path in PLAN.rglob("*.md")
        if path.relative_to(ROOT).as_posix() not in excluded
    )


def old_rows() -> list[dict[str, str]]:
    with COVERAGE_PATH.open(newline="", encoding="utf-8") as stream:
        return list(csv.DictReader(stream))


def current_inventory() -> dict[str, object]:
    event_schema = json.loads((ROOT / "docs/schemas/domain-event.schema.json").read_text())
    delegation_schema = json.loads((ROOT / "docs/schemas/delegation.schema.json").read_text())
    error_schema = json.loads((ROOT / "docs/schemas/error-codes.schema.json").read_text())
    api = yaml.safe_load((ROOT / "docs/schemas/operator-api.openapi.yaml").read_text())
    sql_text = (ROOT / "docs/schemas/sqlite-v1.sql").read_text()
    events_doc = (ROOT / "docs/EVENTS.md").read_text()
    registry = re.search(r"Minimum v1 registry:\s*```\s*(.*?)```", events_doc, re.S)
    event_types = (
        re.findall(r"^[a-z][a-z0-9_.-]+\.v\d+$", registry.group(1), re.M)
        if registry
        else []
    )

    operations: list[dict[str, str]] = []
    operation_digests: list[dict[str, str]] = []
    methods = {"get", "post", "put", "patch", "delete", "options", "head"}
    for path, path_items in api.get("paths", {}).items():
        for method, spec in path_items.items():
            if method.lower() in methods and isinstance(spec, dict):
                operation = {
                    "method": method.upper(),
                    "path": path,
                    "operationId": spec.get("operationId", ""),
                }
                operations.append(operation)
                operation_digests.append(
                    {**operation, "sha256": canonical_digest(spec)}
                )
    documented_routes = {
        (method.upper(), re.sub(r"\{[^}]+\}", "{}", route.split("?", 1)[0].removeprefix("/v1")))
        for method, route in re.findall(
            r"^\s*(GET|POST|PUT|PATCH|DELETE)\s+(/v1/[^\s]+)",
            (ROOT / "docs/API.md").read_text(),
            re.M,
        )
    }
    openapi_routes = {
        (item["method"], re.sub(r"\{[^}]+\}", "{}", item["path"]))
        for item in operations
    }
    if documented_routes != openapi_routes:
        fail(
            "docs/API.md and OpenAPI route inventories differ; "
            f"missing prose={sorted(openapi_routes - documented_routes)[:8]}, "
            f"missing OpenAPI={sorted(documented_routes - openapi_routes)[:8]}"
        )
    components = api.get("components", {}).get("schemas", {})
    schema_digests = [
        {"name": name, "sha256": canonical_digest(schema)}
        for name, schema in sorted(components.items())
    ]
    operation_schema_refs: dict[str, set[str]] = {name: set() for name in components}

    def find_schema_refs(value: object, visited: set[str] | None = None) -> set[str]:
        found: set[str] = set()
        visited = visited or set()
        if isinstance(value, dict):
            ref = value.get("$ref")
            if isinstance(ref, str):
                match = re.match(r"^#/components/schemas/([^/]+)(?:/|$)", ref)
                if match:
                    found.add(match.group(1))
                elif ref.startswith("#/components/") and ref not in visited:
                    visited.add(ref)
                    target: object = api.get("components", {})
                    for part in ref.removeprefix("#/components/").split("/"):
                        part = part.replace("~1", "/").replace("~0", "~")
                        target = target.get(part) if isinstance(target, dict) else None
                    if target is not None:
                        found.update(find_schema_refs(target, visited))
            for child in value.values():
                found.update(find_schema_refs(child, visited))
        elif isinstance(value, list):
            for child in value:
                found.update(find_schema_refs(child, visited))
        return found

    for path, path_items in api.get("paths", {}).items():
        for method, spec in path_items.items():
            if method.lower() in methods and isinstance(spec, dict):
                operation_key = f"{method.upper()} {path}"
                for name in find_schema_refs(spec):
                    operation_schema_refs.setdefault(name, set()).add(operation_key)
    schema_graph = {name: find_schema_refs(schema) for name, schema in components.items()}
    changed = True
    while changed:
        changed = False
        for name, refs in schema_graph.items():
            parent_operations = set(operation_schema_refs.get(name, set()))
            for ref in refs:
                before = len(operation_schema_refs.setdefault(ref, set()))
                operation_schema_refs[ref].update(parent_operations)
                changed |= len(operation_schema_refs[ref]) != before
    schema_usages = [
        {"name": name, "operations": sorted(operation_schema_refs.get(name, set()))}
        for name in sorted(components)
    ]
    operation_unreachable_schemas = sorted(
        item["name"] for item in schema_usages if not item["operations"]
    )
    if operation_unreachable_schemas:
        fail(
            "OpenAPI contains components unreachable from every operation; remove them from "
            "the public contract or connect them to an operation: "
            + ", ".join(operation_unreachable_schemas)
        )

    payloads = event_schema.get("$defs", {}).get("payloads", {})
    expected_payload_names = {
        event_type.rsplit(".v", 1)[0].replace(".", "_").replace("-", "_")
        for event_type in event_types
    }
    if expected_payload_names != set(payloads):
        fail(
            "EVENTS.md registry and domain-event.schema.json payloads differ; "
            f"missing payloads={sorted(expected_payload_names - set(payloads))[:8]}, "
            f"unregistered payloads={sorted(set(payloads) - expected_payload_names)[:8]}"
        )
    event_payloads = [
        {"name": name, "sha256": canonical_digest(payload)}
        for name, payload in sorted(payloads.items())
    ]
    event_definitions = [
        {"name": name, "sha256": canonical_digest(definition)}
        for name, definition in sorted(event_schema.get("$defs", {}).items())
    ]
    delegation_definitions = [
        {"name": name, "sha256": canonical_digest(definition)}
        for name, definition in sorted(delegation_schema.get("$defs", {}).items())
    ]

    schema_text = (ROOT / "docs/SCHEMAS.md").read_text()
    id_section = re.search(r"## Identifiers\s*```[^\n]*\n(.*?)```", schema_text, re.S)
    shared_ids = sorted(set(re.findall(r"\b[A-Z][A-Za-z0-9]*Id\b", id_section.group(1)))) if id_section else []
    shared_definitions = sorted(set(re.findall(r"^([A-Z][A-Za-z0-9_]*)\s*=", schema_text, re.M)))
    error_codes = sorted(error_schema.get("enum", []))
    if set(error_codes) != set(event_schema.get("$defs", {}).get("error_code", {}).get("enum", [])):
        fail("error-codes.schema.json and domain-event.schema.json error enums differ")
    api_error_schema = components.get("ErrorCode", {})
    if "$ref" in api_error_schema:
        referenced = (ROOT / "docs/schemas" / api_error_schema["$ref"]).resolve()
        try:
            api_error_schema = json.loads(referenced.read_text())
        except (OSError, json.JSONDecodeError) as error:
            fail(f"cannot resolve OpenAPI ErrorCode reference: {error}")
    if set(error_codes) != set(api_error_schema.get("enum", [])):
        fail("error-codes.schema.json and OpenAPI ErrorCode schema differ")

    table_matches = list(
        re.finditer(
            r"CREATE TABLE(?: IF NOT EXISTS)?\s+[`\"\[]?(\w+)[`\"\]]?\s*\((.*?)\n\);",
            sql_text,
            re.I | re.S,
        )
    )
    connection = sqlite3.connect(":memory:")
    try:
        connection.executescript(sql_text)
    except sqlite3.Error as error:
        fail(f"sqlite-v1.sql cannot be introspected: {error}")

    sqlite_tables = sorted(match.group(1) for match in table_matches)
    sqlite_columns: list[dict[str, object]] = []
    sqlite_foreign_keys: list[dict[str, object]] = []
    sqlite_unique_constraints: list[dict[str, object]] = []
    sqlite_check_constraints: list[dict[str, object]] = []
    check_counts: dict[str, int] = {}
    for match in table_matches:
        table, body = match.group(1), match.group(2)
        for col in connection.execute(f"PRAGMA table_xinfo({sql_quote(table)})"):
            # cid, name, type, notnull, default, primary-key ordinal, hidden
            if col[6] == 1:
                continue
            sqlite_columns.append(
                {
                    "table": table,
                    "name": col[1],
                    "type": col[2] or "",
                    "not_null": bool(col[3]),
                    "default": col[4],
                    "primary_key_order": int(col[5]),
                }
            )
        for fk in connection.execute(f"PRAGMA foreign_key_list({sql_quote(table)})"):
            # id, seq, target table, from, to, on_update, on_delete, match
            sqlite_foreign_keys.append(
                {
                    "table": table,
                    "id": int(fk[0]),
                    "seq": int(fk[1]),
                    "target_table": fk[2],
                    "from": fk[3],
                    "to": fk[4],
                    "on_update": fk[5],
                    "on_delete": fk[6],
                    "match": fk[7],
                }
            )
        for index in connection.execute(f"PRAGMA index_list({sql_quote(table)})"):
            # seq, name, unique, origin, partial. Explicit indexes have their own rows.
            if index[3] == "u":
                cols = [
                    row[2]
                    for row in connection.execute(
                        f"PRAGMA index_info({sql_quote(index[1])})"
                    )
                ]
                sqlite_unique_constraints.append(
                    {"table": table, "name": index[1], "columns": cols}
                )
        checks = extract_check_expressions(body)
        check_counts[table] = len(checks)
        for ordinal, expression in enumerate(checks, start=1):
            sqlite_check_constraints.append(
                {
                    "table": table,
                    "ordinal": ordinal,
                    "sha256": sha256(expression),
                }
            )

    sqlite_master = connection.execute(
        "SELECT type, name, sql FROM sqlite_master "
        "WHERE type IN ('table','view','index','trigger') "
        "AND name NOT LIKE 'sqlite_%' AND sql IS NOT NULL ORDER BY type,name"
    ).fetchall()
    master_table_names = sorted(row[1] for row in sqlite_master if row[0] == "table")
    if master_table_names != sqlite_tables:
        fail("SQLite parser table inventory differs from CREATE TABLE source inventory")
    sqlite_views = sorted(row[1] for row in sqlite_master if row[0] == "view")
    sqlite_triggers = sorted(row[1] for row in sqlite_master if row[0] == "trigger")
    sqlite_indexes = sorted(row[1] for row in sqlite_master if row[0] == "index")
    sqlite_object_digests = [
        {"kind": row[0].upper(), "name": row[1], "sha256": sha256(row[2])}
        for row in sqlite_master
    ]
    connection.close()

    table_stories = {table: table_story(table) for table in sqlite_tables}
    trigger_targets = {
        match.group(1): match.group(2)
        for match in re.finditer(
            r"CREATE TRIGGER(?: IF NOT EXISTS)?\s+[`\"\[]?(\w+)[`\"\]]?.*?\bON\s+[`\"\[]?(\w+)",
            sql_text,
            re.I | re.S,
        )
    }
    index_targets = {
        match.group(1): match.group(2)
        for match in re.finditer(
            r"CREATE (?:UNIQUE )?INDEX(?: IF NOT EXISTS)?\s+[`\"\[]?(\w+)[`\"\]]?.*?\bON\s+[`\"\[]?(\w+)",
            sql_text,
            re.I | re.S,
        )
    }

    return {
        "operator_operations": sorted(operations, key=lambda item: (item["path"], item["method"])),
        "operator_operation_digests": sorted(operation_digests, key=lambda item: (item["path"], item["method"])),
        "operator_schemas": sorted(components),
        "operator_schema_digests": schema_digests,
        "operator_schema_usages": schema_usages,
        "operator_operation_unreachable_schemas": operation_unreachable_schemas,
        "error_codes": error_codes,
        "event_schema_definitions": [item["name"] for item in event_definitions],
        "event_schema_definition_digests": event_definitions,
        "event_schema_payloads": [item["name"] for item in event_payloads],
        "event_payload_digests": event_payloads,
        "delegation_schema_definitions": [item["name"] for item in delegation_definitions],
        "delegation_schema_definition_digests": delegation_definitions,
        "event_type_registry": sorted(event_types),
        "shared_schema_identifiers": shared_ids,
        "shared_schema_definitions": shared_definitions,
        "sqlite_tables": sqlite_tables,
        "sqlite_columns": sorted(sqlite_columns, key=lambda item: (item["table"], item["name"])),
        "sqlite_foreign_keys": sorted(sqlite_foreign_keys, key=lambda item: (item["table"], item["id"], item["seq"])),
        "sqlite_unique_constraints": sorted(sqlite_unique_constraints, key=lambda item: (item["table"], item["name"])),
        "sqlite_check_constraints": sorted(sqlite_check_constraints, key=lambda item: (item["table"], item["ordinal"])),
        "sqlite_check_counts": check_counts,
        "sqlite_views": sqlite_views,
        "sqlite_triggers": sqlite_triggers,
        "sqlite_indexes": sqlite_indexes,
        "sqlite_object_digests": sqlite_object_digests,
        "sqlite_trigger_targets": trigger_targets,
        "sqlite_index_targets": index_targets,
    }


def table_story(table: str) -> str:
    name = table.lower()
    if name.startswith(("delegation_profile", "delegation_budget")):
        return "E05-S01"
    if name.startswith(("suggestion", "goal", "coworker", "context_document")):
        return "E08-S02" if name.startswith(("suggestion", "goal")) else "E08-S03" if name.startswith("context_document") else "E08-S01"
    if name.startswith(("demonstration", "skill_proposal")):
        return "E10-S02"
    if name.startswith(("routine",)):
        return "E09-S01"
    if name.startswith(("automation",)):
        return "E09-S02"
    if name.startswith(("notification",)):
        return "E09-S04"
    if name.startswith(("channel", "user_request", "provider_input")):
        return "E11-S04"
    if name.startswith(("environment_control", "environment", "attempt", "execution_lease")):
        return "E07-S01" if name.startswith("environment") else "E03-S04"
    if name.startswith(("runtime", "pairing", "handoff", "replication", "pending_replication")):
        return "E11-S02" if name.startswith(("replication", "pending_replication", "pairing")) else "E11-S03" if name.startswith("handoff") else "E07-S01"
    if name.startswith(("agent_profile", "agent_endpoint", "agent_binding", "agent_host", "agent_session")):
        return "E03-S01"
    if name.startswith(("agent_endpoint_binding",)):
        return "E03-S01"
    if name.startswith(("capability", "secret_lease", "connection", "approval", "audit", "effect", "evidence", "verification", "artifact")):
        return "E04-S03" if name.startswith(("capability_invocation", "effect")) else "E04-S05" if name.startswith(("artifact", "evidence", "verification")) else "E04-S01" if name.startswith(("approval", "secret_lease")) else "E04-S02" if name.startswith(("capability", "connection")) else "E04-S01"
    if name.startswith(("resource", "file_identity", "workspace_root", "workspace_replication_root", "dependency", "invalidation")):
        return "E02-S03" if name.startswith(("workspace_root", "file_identity")) else "E06-S03" if name.startswith(("dependency", "invalidation")) else "E06-S01"
    if name.startswith(("usage", "budget", "provider_circuit")):
        return "E05-S03"
    if name.startswith(("workspace",)):
        return "E02-S02"
    if name.startswith(("conversation",)):
        return "E03-S02"
    if name.startswith(("task_spec", "plan_revision", "steps")):
        return "E03-S03"
    if name.startswith(("tasks",)):
        return "E03-S04"
    if name.startswith(("domain_event", "request_dedup", "aggregate_snapshot", "event_archive", "workspace_backup")):
        return "E13-S01" if name.startswith("workspace_backup") else "E11-S02" if name.startswith(("domain_event", "event_archive")) else "E13-S01"
    return "E01-S02"


def operation_story(path: str, method: str, operation_id: str) -> str:
    p = path.lower()
    op = operation_id.lower()
    if "backup" in p or "restore" in p:
        return "E13-S01"
    if p.startswith("/workspaces"):
        return "E02-S02" if "instruction" not in p else "E02-S02"
    if p.startswith("/conversations"):
        return "E03-S02"
    if p.startswith("/tasks"):
        if "lead-agent" in p:
            return "E05-S04"
        if "progress" in p or "timeline" in p:
            return "E02-S04"
        if method == "POST" and p == "/tasks":
            return "E03-S03"
        if "spec-revisions" in p or "plan-revisions" in p or "execution-dependencies" in p:
            return "E03-S03"
        return "E03-S04"
    if p.startswith("/approvals"):
        return "E04-S01"
    if p.startswith(("/artifacts", "/library")):
        return "E04-S05" if p.startswith("/artifacts") else "E08-S04"
    if p.startswith("/routines"):
        return "E09-S03" if "health" in p else "E09-S01"
    if p.startswith("/automations"):
        return "E09-S03" if "test" in p or "health" in p else "E09-S02"
    if p.startswith("/notifications"):
        return "E09-S04"
    if p.startswith("/runtime-lifecycle"):
        return "E01-S03"
    if p.startswith("/runtimes"):
        return "E12-S01" if "remote" in p else "E11-S02"
    if p.startswith("/agent-bindings") or p.startswith("/agent-profiles"):
        return "E05-S03" if "quota" in p else "E03-S01"
    if p.startswith("/delegation-profiles"):
        return "E05-S03" if "performance" in p else "E05-S01"
    if p.startswith("/coworkers"):
        return "E08-S01"
    if p.startswith("/goals") or p.startswith("/suggestions"):
        return "E08-S02"
    if p.startswith(("/context", "/resources")):
        return "E08-S03" if p.startswith("/context") else "E06-S01" if "upload" in p else "E02-S03"
    if p.startswith(("/capabilities", "/discover", "/mcp")):
        return "E10-S01" if "skill" in p or "app" in p else "E04-S02"
    if p.startswith(("/effects", "/evidence", "/invocations", "/usage", "/budgets")):
        return "E05-S03" if p.startswith(("/usage", "/budgets")) else "E04-S03"
    if p.startswith("/demonstrations"):
        return "E10-S02"
    if p.startswith(("/connections", "/channel-bindings", "/channels")):
        return "E11-S04"
    if p.startswith("/needs-you") or p.startswith("/user-requests"):
        return "E02-S04"
    if p.startswith(("/search", "/workspace-roots")):
        return "E02-S03"
    return "E01-S04"


def event_story(event_type: str) -> str:
    prefix = event_type.split(".", 1)[0]
    if prefix in {"conversation", "message", "turn"}:
        return "E03-S02"
    if prefix in {"task", "step", "attempt", "plan", "execution_lease"}:
        return "E03-S04" if prefix in {"task", "step", "attempt", "execution_lease"} else "E03-S03"
    if prefix in {"agent", "agent_session", "agent_binding", "agent_profile"}:
        return "E03-S01"
    if prefix.startswith("delegation"):
        return "E05-S01"
    if prefix in {"capability", "capability_invocation", "effect", "evidence", "artifact", "approval", "secret_lease", "verification", "audit", "connection"}:
        return "E04-S05" if prefix in {"artifact", "evidence", "verification"} else "E04-S03" if prefix in {"capability_invocation", "effect"} else "E04-S01" if prefix in {"approval", "secret_lease", "audit"} else "E04-S02"
    if prefix in {"resource", "workspace_root", "file_identity", "dependency", "invalidation"}:
        return "E06-S01" if prefix == "resource" else "E02-S03"
    if prefix in {"coworker", "goal", "suggestion", "context_document"}:
        return "E08-S02" if prefix in {"goal", "suggestion"} else "E08-S03" if prefix == "context_document" else "E08-S01"
    if prefix in {"routine", "automation", "occurrence"}:
        return "E09-S02" if prefix in {"automation", "occurrence"} else "E09-S01"
    if prefix in {"demonstration", "skill_proposal"}:
        return "E10-S02"
    if prefix in {"channel", "user_request", "provider_input"}:
        return "E11-S04"
    if prefix in {"runtime", "environment", "pairing", "replication", "handoff"}:
        return "E11-S02" if prefix in {"pairing", "replication"} else "E11-S03" if prefix == "handoff" else "E07-S01"
    if prefix in {"usage", "budget", "quota"}:
        return "E05-S03"
    if prefix == "notification":
        return "E09-S04"
    if prefix == "workspace":
        return "E02-S02"
    return "E01-S02"


def contract_file_story(path: str) -> str:
    if path.endswith("delegation.schema.json"):
        return "E05-S01"
    if path.endswith("operator-api.openapi.yaml"):
        return "E01-S04"
    if path.endswith("error-codes.schema.json"):
        return "E01-S04"
    if path.endswith("sqlite-v1.sql") or path.endswith("domain-event.schema.json"):
        return "E01-S02"
    return "E01-S01"


def read_doc_mappings(previous: list[dict[str, str]]) -> dict[str, str]:
    mapping = {
        row["contract_or_scenario"]: row["primary_story"]
        for row in previous
        if row["kind"] in {"ARCH_DOC", "AUTHORITY"}
        and row["contract_or_scenario"].endswith(".md")
    }
    mapping.update(
        {
            "CONTRIBUTING.md": "E01-S01",
            "GLOSSARY.md": "E01-S01",
            "README.md": "E02-S01",
            "docs/FLOWS.md": "E13-S03",
            "docs/adr/README.md": "E01-S01",
            "docs/adr/0001-durable-task-core.md": "E03-S04",
            "docs/adr/0002-task-and-attempt-identity.md": "E03-S04",
            "docs/adr/0003-litespm-is-independent.md": "E04-S04",
            "docs/adr/0004-runtime-and-environment-are-distinct.md": "E07-S01",
            "docs/adr/0005-replicate-domain-state-not-databases.md": "E11-S02",
            "docs/adr/0006-conversation-task-separation.md": "E03-S02",
            "docs/adr/0007-reported-observed-verified.md": "E04-S05",
            "docs/adr/0008-no-generic-provider-layer.md": "E03-S01",
            "docs/adr/0009-attempt-free-lead-planning.md": "E03-S03",
            "docs/adr/0010-revision-independent-occurrences.md": "E09-S02",
            "docs/adr/0011-atomic-plan-acceptance.md": "E03-S03",
            "docs/adr/0012-native-harness-integrity.md": "E03-S05",
            "docs/adr/0013-host-delegation-uses-accepted-plan-steps.md": "E05-S02",
            "docs/adr/0014-versioned-worker-profiles-and-bounded-selection.md": "E05-S01",
            "docs/adr/0015-warmth-is-operational-and-sharing-is-separate.md": "E07-S03",
            "docs/adr/0016-coworker-goal-and-suggestion-authority-boundary.md": "E08-S02",
            "docs/adr/0017-deadline-sensitive-is-best-effort.md": "E07-S03",
            "docs/adr/0018-context-content-is-resource-backed-and-provider-pluggable.md": "E08-S03",
            "docs/adr/0019-credential-egress-and-audit-boundaries.md": "E04-S01",
        }
    )
    for path in schema_docs():
        if path not in mapping:
            fail(f"architecture document needs an explicit story owner: {path}")
    return mapping


def doc_sections(path: str, text: str) -> list[tuple[str, str, str]]:
    lines = text.splitlines(keepends=True)
    headings: list[tuple[int, int, str]] = []
    for index, line in enumerate(lines):
        match = re.match(r"^(#{1,6})\s+(.+?)\s*#*\s*$", line.rstrip("\r\n"))
        if match:
            headings.append((index, len(match.group(1)), match.group(2)))
    counts: Counter[str] = Counter()
    result: list[tuple[str, str, str]] = []
    active_case_story: str | None = None
    previous = old_rows()
    old_case_rows = {
        (row["kind"], row["contract_or_scenario"].split(" ", 1)[0]): row["primary_story"]
        for row in previous
        if row["kind"] in {"FLOW", "BENCHMARK"}
    }
    for position, (start, level, title) in enumerate(headings):
        end = len(lines)
        for next_start, next_level, _ in headings[position + 1 :]:
            if next_level <= level:
                end = next_start
                break
        label = f"{'#' * level} {title}"
        counts[label] += 1
        key = f"{path}#{label}" + (f"~{counts[label]}" if counts[label] > 1 else "")
        scenario = re.match(r"^(F\d+)\s+[—-]", title) or re.match(r"^(B\d+)\s+", title)
        if scenario:
            kind = "FLOW" if scenario.group(1).startswith("F") else "BENCHMARK"
            active_case_story = old_case_rows.get((kind, scenario.group(1)))
            if active_case_story is None:
                fail(f"new {kind.lower()} {scenario.group(1)} needs an explicit story mapping before coverage refresh")
        body = "".join(lines[start:end])
        result.append((key, sha256(body), active_case_story or ""))
    return result


def plan_doc_story(path: str, title: str, backlog: dict[str, object]) -> str:
    stories = {story["id"] for story in backlog.get("stories", [])}
    exact = re.match(r"^(E\d{2}-S\d{2})\b", title)
    if exact and exact.group(1) in stories:
        return exact.group(1)
    epic = re.search(r"/E(\d{2})\.md$", path)
    if epic:
        prefix = "E" + epic.group(1) + "-"
        return sorted(story for story in stories if story.startswith(prefix))[0]
    defaults = {
        "AUDIT.md": "E01-S01",
        "COVERAGE.md": "E01-S01",
        "PROCESS.md": "E01-S01",
        "README.md": "E01-S01",
        "RAG.md": "E06-S02",
        "RELEASE.md": "E13-S04",
        "ROADMAP.md": "E01-S01",
        "SCOPE.md": "E13-S04",
        "SOURCES.md": "E08-S02",
        "STACK.md": "E01-S01",
        "TESTING.md": "E13-S03",
        "UI.md": "E08-S04",
        "WORKFLOWS.md": "E13-S03",
    }
    for suffix, story in defaults.items():
        if path.endswith("/" + suffix):
            return story
    return "E01-S01"


def section_story(path: str, title: str, base: str) -> str:
    low = title.lower()
    if path.endswith("SCHEMAS.md"):
        if any(word in low for word in ("conversation", "message", "turn")):
            return "E03-S02"
        if any(word in low for word in ("task", "attempt", "plan", "step", "retry", "failure")):
            return "E03-S04"
        if any(word in low for word in ("resource", "world", "upload", "search")):
            return "E02-S03"
        if any(word in low for word in ("delegate", "delegation", "worker")):
            return "E05-S01"
        if any(word in low for word in ("artifact", "effect", "evidence", "verification")):
            return "E04-S05"
        if any(word in low for word in ("automation", "routine", "trigger", "occurrence")):
            return "E09-S02"
        if any(word in low for word in ("environment", "runtime", "warm")):
            return "E07-S01"
        if any(word in low for word in ("goal", "suggestion", "coworker", "context")):
            return "E08-S02"
    return base


def build_coverage(previous: list[dict[str, str]], inventory: dict[str, object]) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    doc_map = read_doc_mappings(previous)
    backlog = json.loads((PLAN / "backlog.json").read_text())

    def add(kind: str, key: str, story: str, note: str) -> None:
        rows.append(
            {
                "kind": kind,
                "contract_or_scenario": key,
                "primary_story": story,
                "coverage_note": note,
            }
        )

    for story in backlog.get("stories", []):
        story_id = story["id"]
        authorities = ", ".join(story.get("authorities", [])) or "none listed"
        tests = ", ".join(story.get("tests", []))
        add(
            "BACKLOG_STORY",
            story_id,
            story_id,
            f"{story['title']}; authorities: {authorities}; required test cases: {tests}; story sha256:{canonical_digest(story)}.",
        )

    doc_type = {
        "AGENTS.md": "Repository development rules",
        "CONTRIBUTING.md": "Contribution workflow",
        "GLOSSARY.md": "Canonical vocabulary",
        "README.md": "Product overview",
        "ARCHITECTURE.md": "Product architecture authority",
    }
    for path in schema_docs():
        text = (ROOT / path).read_text()
        kind = "Architecture decision record" if path.startswith("docs/adr/") else doc_type.get(path, "Architecture contract")
        add("ARCH_DOC", path, doc_map[path], f"{kind}; file sha256:{sha256((ROOT / path).read_bytes())}; section rows below map implementation guardians, not completion.")
        for key, digest, scenario_story in doc_sections(path, text):
            title = key.split("#", 1)[1].split("~", 1)[0].lstrip("# ")
            story = scenario_story or section_story(path, title, doc_map[path])
            add("DOC_SECTION", key, story, f"section sha256:{digest}; primary implementation guardian only; linked contracts and failure paths remain in scope.")

    for path in implementation_plan_docs():
        text = (ROOT / path).read_text()
        add("PLAN_DOC", path, plan_doc_story(path, Path(path).name, backlog), f"implementation-plan source sha256:{sha256((ROOT / path).read_bytes())}; this file guides delivery and does not override architecture authority.")
        for key, digest, _ in doc_sections(path, text):
            title = key.split("#", 1)[1].split("~", 1)[0].lstrip("# ")
            story = plan_doc_story(path, title, backlog)
            add("PLAN_SECTION", key, story, f"section sha256:{digest}; implementation guidance, subordinate to linked architecture contracts.")

    with (PLAN / "audit-inventory.csv").open(newline="", encoding="utf-8") as stream:
        audit_paths = {row["path"] for row in csv.DictReader(stream)}
    missing_audit_docs = sorted(set(schema_docs()) - audit_paths)
    if missing_audit_docs:
        fail("audit-inventory.csv omits architecture sources: " + ", ".join(missing_audit_docs))

    # Machine contracts are authority files, not Markdown architecture documents.
    contract_files = [
        "docs/schemas/delegation.schema.json",
        "docs/schemas/domain-event.schema.json",
        "docs/schemas/error-codes.schema.json",
        "docs/schemas/operator-api.openapi.yaml",
        "docs/schemas/sqlite-v1.sql",
        "scripts/validate_architecture.py",
    ]
    for path in contract_files:
        add("CONTRACT_FILE", path, contract_file_story(path), f"machine contract/tooling file sha256:{sha256((ROOT / path).read_bytes())}; exact objects are enumerated below.")

    support_files = {
        ".github/workflows/architecture-docs.yml": "E01-S01",
        "scripts/validate_implementation_plan.py": "E01-S01",
        "scripts/validate_implementation_coverage.py": "E01-S01",
        "implementation/backlog.json": "E01-S01",
        "implementation/audit-inventory.csv": "E01-S01",
    }
    for path, story in support_files.items():
        add("PLAN_TOOLING", path, story, f"delivery-plan or validation input sha256:{sha256((ROOT / path).read_bytes())}.")

    workflow_rows = []
    for line in (PLAN / "WORKFLOWS.md").read_text().splitlines():
        if not line.startswith("|"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) != 4:
            continue
        for workflow_id in re.findall(r"\bU\d{2}\b", cells[2]):
            workflow_rows.append((workflow_id, cells, line))
    for workflow_id, cells, line in workflow_rows:
        add(
            "USER_WORKFLOW",
            f"{workflow_id} — {cells[0]}",
            "E13-S03",
            f"{cells[0]}; implementation path: {cells[1]}; acceptance: {cells[2]}; provider limits: {cells[3]}; row sha256:{sha256(line)}.",
        )

    for name in inventory["shared_schema_identifiers"]:
        add("SHARED_SCHEMA", "SCHEMAS.md#" + name, schema_story(name), "shared identifier; definition digest is covered by its architecture section.")
    for name in inventory["shared_schema_definitions"]:
        add("SHARED_SCHEMA", "SCHEMAS.md#" + name, schema_story(name), "shared value/enum contract; definition digest is covered by its architecture section.")

    op_notes = { (x["method"], x["path"]): x["sha256"] for x in inventory["operator_operation_digests"] }
    op_owners = {
        f"{op['method']} {op['path']}": operation_story(op["path"], op["method"], op["operationId"])
        for op in inventory["operator_operations"]
    }
    for op in inventory["operator_operations"]:
        story = operation_story(op["path"], op["method"], op["operationId"])
        add("API_OPERATION", f"{op['method']} {op['path']}", story, f"{op['operationId']}; operation sha256:{op_notes[(op['method'], op['path'])]}.")
    schema_notes = {x["name"]: x["sha256"] for x in inventory["operator_schema_digests"]}
    schema_usages = {x["name"]: x["operations"] for x in inventory["operator_schema_usages"]}
    unreachable = set(inventory["operator_operation_unreachable_schemas"])
    for name in inventory["operator_schemas"]:
        owners = Counter(op_owners[op] for op in schema_usages[name] if op in op_owners)
        story = owners.most_common(1)[0][0] if owners else schema_story(name)
        suffix = "unreachable OpenAPI component; confirm intentionally internal or connect to an operation" if name in unreachable else "component schema; field-level validation belongs in the owning story's contract tests"
        add("OPENAPI_SCHEMA", name, story, f"schema sha256:{schema_notes[name]}; {suffix}.")
    for code in inventory["error_codes"]:
        add("ERROR_CODE", code, error_story(code), "error taxonomy member; assert at the owning operation/state boundary.")

    for name, digest in zip(inventory["event_schema_definitions"], [x["sha256"] for x in inventory["event_schema_definition_digests"]]):
        add("EVENT_DEF", "domain-event.schema.json#" + name, "E01-S02", f"shared event schema definition sha256:{digest}.")
    payload_digests = {item["name"]: item["sha256"] for item in inventory["event_payload_digests"]}
    event_to_payload = {event_type: event_type.rsplit(".v", 1)[0].replace(".", "_").replace("-", "_") for event_type in inventory["event_type_registry"]}
    for event_type, payload in event_to_payload.items():
        add("EVENT_TYPE", event_type, event_story(event_type), f"registered domain event; paired payload {payload}.")
        add("EVENT_PAYLOAD", "domain-event.schema.json#payloads/" + payload, event_story(event_type), f"payload sha256:{payload_digests[payload]}; mapped from {event_type}.")
    delegation_digests = {item["name"]: item["sha256"] for item in inventory["delegation_schema_definition_digests"]}
    for name in inventory["delegation_schema_definitions"]:
        add("SCHEMA_DEF", "delegation.schema.json#" + name, schema_story(name), f"delegation wire definition sha256:{delegation_digests[name]}.")

    sqlite_story = {table: table_story(table) for table in inventory["sqlite_tables"]}
    object_digests = {(item["kind"], item["name"]): item["sha256"] for item in inventory["sqlite_object_digests"]}
    for name in inventory["sqlite_tables"]:
        add("SQL_TABLE", name, sqlite_story[name], f"table DDL sha256:{object_digests[('TABLE', name)]}; fields/constraints below.")
    for column in inventory["sqlite_columns"]:
        key = f"{column['table']}.{column['name']}"
        details = f"type={column['type'] or 'unspecified'}; not_null={str(column['not_null']).lower()}; default={column['default']!r}; pk_order={column['primary_key_order']}"
        add("SQL_COLUMN", key, sqlite_story[column["table"]], details)
    for fk in inventory["sqlite_foreign_keys"]:
        key = f"{fk['table']}#FK{fk['id']}.{fk['seq']} {fk['from']}->{fk['target_table']}.{fk['to']}"
        details = f"on_update={fk['on_update']}; on_delete={fk['on_delete']}; match={fk['match']}"
        add("SQL_FOREIGN_KEY", key, sqlite_story[fk["table"]], details)
    for unique in inventory["sqlite_unique_constraints"]:
        key = f"{unique['table']}#UNIQUE {','.join(unique['columns'])}"
        add("SQL_UNIQUE", key, sqlite_story[unique["table"]], f"SQLite table unique constraint {unique['name']}.")
    for check in inventory["sqlite_check_constraints"]:
        key = f"{check['table']}#CHECK{check['ordinal']}"
        add("SQL_CHECK", key, sqlite_story[check["table"]], f"expression sha256:{check['sha256']}.")
    for name in inventory["sqlite_views"]:
        add("SQL_VIEW", name, "E03-S04" if "session" in name else "E01-S02", f"view DDL sha256:{object_digests[('VIEW', name)]}.")
    for name in inventory["sqlite_triggers"]:
        table = inventory["sqlite_trigger_targets"].get(name)
        story = sqlite_story.get(table, "E01-S02")
        add("SQL_TRIGGER", name, story, f"guard on {table or 'unresolved table'}; DDL sha256:{object_digests[('TRIGGER', name)]}.")
    for name in inventory["sqlite_indexes"]:
        table = inventory["sqlite_index_targets"].get(name)
        story = sqlite_story.get(table, "E01-S02")
        add("SQL_INDEX", name, story, f"index on {table or 'unresolved table'}; DDL sha256:{object_digests[('INDEX', name)]}.")

    previous_by_key = {
        (row["kind"], row["contract_or_scenario"]): row
        for row in previous
    }
    for path, pattern, kind in [
        (ROOT / "docs/FLOWS.md", r"^## (F\d+) — (.+)$", "FLOW"),
        (ROOT / "docs/BENCHMARKS.md", r"^### (B\d+) (.+)$", "BENCHMARK"),
    ]:
        text = path.read_text()
        for identifier, title in re.findall(pattern, text, re.M):
            key = identifier + " " + title
            prior = previous_by_key.get((kind, key))
            if not prior:
                fail(f"new {kind.lower()} {identifier} requires an explicit primary_story in coverage.csv")
            add(kind, key, prior["primary_story"], prior["coverage_note"])

    keys = [(row["kind"], row["contract_or_scenario"]) for row in rows]
    if len(keys) != len(set(keys)):
        fail("generated coverage contains duplicate kind/key rows")
    rows.sort(key=lambda row: (row["kind"], row["contract_or_scenario"]))
    story_ids = {story["id"] for story in backlog.get("stories", [])}
    unowned = sorted({row["primary_story"] for row in rows} - story_ids)
    if unowned:
        fail("coverage rows point to unknown backlog stories: " + ", ".join(unowned))
    mapped_stories = {
        row["contract_or_scenario"]
        for row in rows
        if row["kind"] == "BACKLOG_STORY"
    }
    if mapped_stories != story_ids:
        fail("coverage map does not contain exactly one row for every backlog story")
    return rows


def error_story(code: str) -> str:
    name = code.upper()
    if any(x in name for x in ("DELEGATION", "WORKER", "QUOTA", "BUDGET", "COST", "LEAD_FAILOVER")):
        return "E05-S04" if "QUALITY" in name or "FAILOVER" in name else "E05-S03"
    if any(x in name for x in ("COWORKER", "GOAL", "SUGGESTION", "CONTEXT")):
        return "E08-S03" if "CONTEXT" in name else "E08-S02"
    if any(x in name for x in ("RUNTIME", "ENVIRONMENT", "HANDOFF", "PAIRING", "REPLICATION")):
        return "E11-S03" if "HANDOFF" in name else "E07-S01"
    if any(x in name for x in ("ARTIFACT", "EVIDENCE", "VERIFICATION", "EFFECT")):
        return "E04-S05" if any(x in name for x in ("ARTIFACT", "EVIDENCE", "VERIFICATION")) else "E04-S03"
    if any(x in name for x in ("CAPABILITY", "APPROVAL", "GRANT", "SECRET", "CREDENTIAL", "POLICY", "AUTH")):
        return "E04-S01" if any(x in name for x in ("APPROVAL", "GRANT", "SECRET", "CREDENTIAL", "POLICY", "AUTH")) else "E04-S02"
    if any(x in name for x in ("RESOURCE", "UPLOAD", "SEARCH", "INDEX")):
        return "E06-S01"
    if any(x in name for x in ("ROUTINE", "AUTOMATION", "OCCURRENCE", "TRIGGER")):
        return "E09-S02"
    if any(x in name for x in ("CHANNEL", "USER_REQUEST", "PROVIDER_INPUT")):
        return "E11-S04"
    if any(x in name for x in ("AGENT", "SESSION", "MODEL")):
        return "E03-S01"
    if any(x in name for x in ("TASK", "STEP", "ATTEMPT", "PLAN", "LEASE")):
        return "E03-S04"
    if "NOTIFICATION" in name:
        return "E09-S04"
    return "E01-S04"


def inventory_bytes(inventory: dict[str, object]) -> bytes:
    compact = {
        key: inventory[key]
        for key in (
            "operator_operations",
            "operator_operation_digests",
            "operator_schemas",
            "operator_schema_digests",
            "operator_operation_unreachable_schemas",
            "error_codes",
            "event_schema_definitions",
            "event_schema_definition_digests",
            "event_schema_payloads",
            "event_payload_digests",
            "delegation_schema_definitions",
            "delegation_schema_definition_digests",
            "event_type_registry",
            "shared_schema_identifiers",
            "shared_schema_definitions",
            "sqlite_tables",
            "sqlite_views",
            "sqlite_triggers",
            "sqlite_indexes",
            "sqlite_object_digests",
        )
    }
    # Detailed SQLite columns and relational constraints remain one row each in
    # coverage.csv. Store totals and canonical digests here instead of repeating the
    # same thousand-plus records in JSON as well.
    compact["sqlite_detail_counts"] = {
        "columns": len(inventory["sqlite_columns"]),
        "foreign_keys": len(inventory["sqlite_foreign_keys"]),
        "unique_constraints": len(inventory["sqlite_unique_constraints"]),
        "check_constraints": len(inventory["sqlite_check_constraints"]),
    }
    compact["sqlite_detail_digests"] = {
        "columns": canonical_digest(inventory["sqlite_columns"]),
        "foreign_keys": canonical_digest(inventory["sqlite_foreign_keys"]),
        "unique_constraints": canonical_digest(inventory["sqlite_unique_constraints"]),
        "check_constraints": canonical_digest(inventory["sqlite_check_constraints"]),
    }
    return (json.dumps(compact, indent=2, sort_keys=True) + "\n").encode()


def coverage_bytes(rows: list[dict[str, str]]) -> bytes:
    import io

    out = io.StringIO(newline="")
    writer = csv.DictWriter(
        out,
        fieldnames=["kind", "contract_or_scenario", "primary_story", "coverage_note"],
        lineterminator="\n",
    )
    writer.writeheader()
    writer.writerows(rows)
    return out.getvalue().encode()


def report_text(rows: list[dict[str, str]], inventory: dict[str, object]) -> str:
    docs = [row for row in rows if row["kind"] == "ARCH_DOC"]
    sections = [row for row in rows if row["kind"] == "DOC_SECTION"]
    counts = Counter(row["kind"] for row in rows)
    lines = [
        "# Architecture and contract coverage audit",
        "",
        "This index connects the current architecture sources to implementation stories. The",
        "primary story is a planning guardian; it does not claim that a story implements every",
        "cross-cutting rule in a document. Linked flows, schemas, failure cases, and related",
        "domain owners still apply. All stories remain planned until code and evidence exist.",
        "",
        "## Coverage inventory",
        "",
        f"- {len(docs)} source Markdown documents; {len(sections)} heading-level sections are digest-pinned.",
        f"- {counts['PLAN_DOC']} implementation-plan documents and {counts['PLAN_SECTION']} plan sections are indexed; the mutable current-run handoff is intentionally excluded.",
        f"- {counts['BACKLOG_STORY']} planned backlog stories, each with CODE/SYSTEM/USER test case IDs.",
        f"- {counts['FLOW']} flows and {counts['BENCHMARK']} benchmarks.",
        f"- {counts['API_OPERATION']} OpenAPI operations and {counts['OPENAPI_SCHEMA']} component schemas; {len(inventory['operator_operation_unreachable_schemas'])} components are unreachable from every Operator operation.",
        f"- {counts['ERROR_CODE']} error codes; {counts['SHARED_SCHEMA']} shared IDs/value definitions.",
        f"- {counts['EVENT_TYPE']} event types and {counts['EVENT_PAYLOAD']} typed event payload schemas.",
        f"- {counts['SQL_TABLE']} SQLite tables, {counts['SQL_COLUMN']} columns, {counts['SQL_FOREIGN_KEY']} foreign-key columns, {counts['SQL_UNIQUE']} unique constraints, {counts['SQL_CHECK']} checks, {counts['SQL_VIEW']} view(s), {counts['SQL_TRIGGER']} triggers, and {counts['SQL_INDEX']} indexes.",
        "",
        "Machine-object names and canonical digests are compared to current source contracts by",
        "`validate_implementation_coverage.py`. This detects inventory drift; it does not prove",
        "that a proposed implementation has correct behavior. Story-level CODE, SYSTEM, and",
        "USER tests provide that evidence when executed.",
        "",
        "## Architecture document owners",
        "",
        "| Document | Planning guardian | Classification |",
        "|---|---|---|",
    ]
    for row in docs:
        note = row["coverage_note"]
        classification = note.split(";", 1)[0]
        path = row["contract_or_scenario"]
        lines.append(f"| [`{path}`](../{path}) | {row['primary_story']} | {classification} |")
    lines.extend(
        [
            "",
            "## Not covered by this plan yet",
            "",
            "1. **Product behavior is not implemented or qualified.** The backlog is planned work. No",
            "   code, provider, cloud deployment, remote Runtime, platform installer, or real-user",
            "   acceptance is claimed by this coverage audit.",
            "2. **A digest is not a semantic test.** OpenAPI component digests catch any source change,",
            "   but field-level required/optional/validation behavior must still be asserted in the",
            "   owning story's executable contract tests. The same applies to narrative requirements",
            "   and cross-domain policy interactions.",
            "3. **Cross-cutting scenarios have one primary guardian in the CSV.** A flow can exercise",
            "   several epics; the guardian is not its only implementation owner. Before coding, the",
            "   story review must identify each affected domain, transition, event, authorization",
            "   decision, failure path, projection, and test. The map does not encode a complete",
            "   many-to-many requirement-to-story relation.",
            "4. **External provider behavior remains conditional.** Claude/Codex/OpenCode/Cline, local",
            "   model servers, browsers, OS integrations, cloud credentials, and package services",
            "   require real qualification in the target environment. Mock results cannot close those",
            "   gates. LiteSPM behavior is limited to its selected documented authority; unknown",
            "   package API details are deliberately not invented.",
            "5. **Not every machine-readable field is an independent CSV row.** SQLite fields and",
            "   key constraints are enumerated; OpenAPI schema properties and nested constraints",
            "   are digest-pinned as whole component schemas. Stories must expand relevant properties",
            "   and negative cases into executable assertions rather than treating inventory as",
            "   implementation evidence.",
            "6. **Some architecture is intentionally outside v1.** Team/RBAC, native mobile/web clients,",
            "   voice, general marketplace/template sharing, and future messaging gateways remain",
            "   deferred. Their architecture docs are indexed, but they are not V1 acceptance claims.",
            "",
            "## Explicit scope boundaries",
            "",
            "The release order is desktop/local first, cloud after the desktop feature gate, then",
            "remote Runtime. The owner is the only product reviewer; coding agents do not replace",
            "owner acceptance. Messaging gateway integrations are architecture references for later",
            "work and are not required for desktop/cloud/remote v1.",
            "",
            "OpenClaw is a research reference for deployment patterns and possible future messaging",
            "gateways. LiteCowork's contracts remain authoritative for Task state, permissions,",
            "Effects, evidence, fencing, and channel identity.",
            "",
            "## Using the map",
            "",
            "Read [the plan](README.md), [the coverage CSV](coverage.csv), and the owning architecture",
            "document before changing a domain. Refresh generated inventory only after reviewing the",
            "source diff with `python3 scripts/validate_implementation_coverage.py --write`; default",
            "validator mode is read-only and must pass in CI.",
            "",
        ]
    )
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true", help="refresh generated coverage and inventory")
    args = parser.parse_args()
    previous = old_rows()
    inventory = current_inventory()
    rows = build_coverage(previous, inventory)
    outputs = {
        INVENTORY_PATH: inventory_bytes(inventory),
        COVERAGE_PATH: coverage_bytes(rows),
        REPORT_PATH: report_text(rows, inventory).encode(),
    }
    if args.write:
        for path, content in outputs.items():
            path.write_bytes(content)
        print(f"Wrote architecture coverage: {len(schema_docs())} documents, {len(rows)} rows.")
        return
    for path, expected in outputs.items():
        if not path.exists() or path.read_bytes() != expected:
            fail(f"{path.relative_to(ROOT)} is stale; review changes then run scripts/validate_implementation_coverage.py --write")
    print(
        f"Architecture coverage passed: {len(schema_docs())} documents, "
        f"{sum(row['kind'] == 'DOC_SECTION' for row in rows)} sections, {len(rows)} traceability rows."
    )


if __name__ == "__main__":
    main()
