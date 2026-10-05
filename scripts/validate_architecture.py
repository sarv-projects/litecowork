#!/usr/bin/env python3
"""Validate cross-document LiteCowork architecture contracts."""

from __future__ import annotations

import json
import hashlib
import re
import sqlite3
import sys
from pathlib import Path
from urllib.parse import unquote

import jsonschema
import openapi_spec_validator
import yaml


ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs"
ERRORS: list[str] = []
SHA256_PATTERN = r"^sha256:[a-f0-9]{64}$"


def fail(message: str) -> None:
    ERRORS.append(message)


def load_json(path: Path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:  # noqa: BLE001 - report malformed architecture inputs
        fail(f"{path.relative_to(ROOT)}: invalid JSON: {exc}")
        return {}


class UniqueKeyLoader(yaml.SafeLoader):
    pass


def unique_mapping(loader, node, deep=False):
    mapping = {}
    for key_node, value_node in node.value:
        key = loader.construct_object(key_node, deep=deep)
        if key in mapping:
            raise ValueError(f"duplicate YAML key: {key}")
        mapping[key] = loader.construct_object(value_node, deep=deep)
    return mapping


UniqueKeyLoader.add_constructor(
    yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG, unique_mapping
)


def resolve_pointer(document, pointer: str):
    current = document
    for part in pointer.lstrip("#/").split("/"):
        part = part.replace("~1", "/").replace("~0", "~")
        current = current[part]
    return current


def check_digest_encodings(document, source: str) -> None:
    """Require the documented sha256: prefix on every typed digest property."""
    def walk(node, path=""):
        if isinstance(node, dict):
            for name, schema in node.get("properties", {}).items():
                if name in {"digest", "sha256"} or name.endswith("_digest"):
                    value_type = schema.get("type") if isinstance(schema, dict) else None
                    is_string = value_type == "string" or (
                        isinstance(value_type, list) and "string" in value_type
                    )
                    if is_string and schema.get("pattern") != SHA256_PATTERN:
                        fail(f"{source}{path}/{name}: digest must use the canonical Sha256Digest pattern")
            for name, value in node.items():
                walk(value, f"{path}/{name}")
        elif isinstance(node, list):
            for index, value in enumerate(node):
                walk(value, f"{path}[{index}]")
    walk(document)


def check_json_schemas() -> None:
    for path in sorted((DOCS / "schemas").glob("*.json")):
        schema = load_json(path)
        check_digest_encodings(schema, path.relative_to(ROOT).as_posix())
        try:
            jsonschema.Draft202012Validator.check_schema(schema)
        except Exception as exc:  # noqa: BLE001
            fail(f"{path.relative_to(ROOT)}: invalid JSON Schema: {exc}")


def check_event_contract() -> None:
    text = (DOCS / "EVENTS.md").read_text(encoding="utf-8")
    registry_match = re.search(
        r"Minimum v1 registry:\s*```\s*(.*?)```", text, re.S
    )
    if not registry_match:
        fail("EVENTS.md: missing minimum v1 registry")
        return
    registry_lines = re.findall(
        r"^[a-z][a-z0-9_.-]+\.v\d+$", registry_match.group(1), re.M
    )
    registry = set(registry_lines)
    if len(registry_lines) != len(registry):
        fail("EVENTS.md: duplicate event type in registry")

    family_rows: dict[str, list[str]] = {}
    for family, cell in re.findall(r"^\| `([^`]+)` \| (.*?) \|$", text, re.M):
        fields = re.findall(r"`([^`]+)`", cell)
        if fields:
            family_rows[family] = fields

    schema = load_json(DOCS / "schemas" / "domain-event.schema.json")
    envelope_required = set(schema.get("required", []))
    for required_field in ("entity_revision", "aggregate_state_ref"):
        if required_field not in envelope_required:
            fail(f"domain-event.schema.json: event envelope must require {required_field}")
    payloads = schema.get("$defs", {}).get("payloads", {})
    branches: dict[str, str] = {}
    for branch in schema.get("allOf", []):
        event_type = (
            branch.get("if", {}).get("properties", {}).get("type", {}).get("const")
        )
        payload_ref = (
            branch.get("then", {}).get("properties", {}).get("payload", {}).get("$ref")
        )
        if event_type and payload_ref:
            branches[event_type] = payload_ref.rsplit("/", 1)[-1]

    if set(branches) != registry:
        fail(
            "EVENTS.md/schema registry mismatch: "
            f"missing branches={sorted(registry - set(branches))}, "
            f"extra branches={sorted(set(branches) - registry)}"
        )

    def event_key(name: str) -> str:
        return re.sub(r"[^a-zA-Z0-9]+", "_", re.sub(r"\.v\d+$", "", name)).strip("_")

    def row_for(event: str) -> list[str] | None:
        base = re.sub(r"\.v\d+$", "", event)
        if base in family_rows:
            return family_rows[base]
        candidates = [
            (len(family.replace("*", "")), fields)
            for family, fields in family_rows.items()
            if "*" in family and base.startswith(family.split("*", 1)[0])
        ]
        return max(candidates, key=lambda item: item[0])[1] if candidates else None

    for event in sorted(registry):
        key = event_key(event)
        payload = payloads.get(key)
        family_fields = row_for(event)
        if not payload:
            fail(f"domain-event.schema.json: missing payload definition for {event}")
            continue
        if not family_fields:
            fail(f"EVENTS.md: no payload field contract for {event}")
            continue
        def field_name(field: str) -> str:
            if field.endswith("?"):
                field = field[:-1]
            return field[:-2] if field.endswith("[]") else field

        expected_required = {
            field_name(field)
            for field in family_fields
            if not field.endswith("?")
        }
        expected_properties = {field_name(field) for field in family_fields}
        actual_required = set(payload.get("required", []))
        actual_properties = set(payload.get("properties", {}))
        if actual_required != expected_required:
            fail(
                f"{event}: payload required fields differ from EVENTS.md; "
                f"missing={sorted(expected_required-actual_required)}, "
                f"unexpected={sorted(actual_required-expected_required)}"
            )
        if actual_properties != expected_properties:
            fail(
                f"{event}: payload properties differ from EVENTS.md; "
                f"missing={sorted(expected_properties-actual_properties)}, "
                f"unexpected={sorted(actual_properties-expected_properties)}"
            )
        if payload.get("type") != "object" or payload.get("additionalProperties") is not False:
            fail(f"{event}: payload must be a closed object schema")
        for field, field_schema in payload.get("properties", {}).items():
            if not any(key in field_schema for key in ("type", "$ref", "oneOf", "anyOf", "allOf")):
                fail(f"{event}: payload field {field} has no concrete schema")

    validator = jsonschema.Draft202012Validator(
        schema, format_checker=jsonschema.FormatChecker()
    )

    def sample(value_schema, depth=0):
        if depth > 20:
            return "sample"
        if "$ref" in value_schema:
            return sample(resolve_pointer(schema, value_schema["$ref"]), depth + 1)
        if "anyOf" in value_schema:
            return sample(value_schema["anyOf"][0], depth + 1)
        if "const" in value_schema:
            return value_schema["const"]
        if "enum" in value_schema:
            return value_schema["enum"][0]
        if "oneOf" in value_schema and value_schema.get("type") != "object" and "properties" not in value_schema:
            return sample(value_schema["oneOf"][0], depth + 1)
        value_type = value_schema.get("type")
        if isinstance(value_type, list):
            value_type = next((item for item in value_type if item != "null"), "null")
        if value_type == "object" or "properties" in value_schema:
            out = {
                name: sample(value_schema.get("properties", {}).get(name, {}), depth + 1)
                for name in value_schema.get("required", [])
            }
            for branch in value_schema.get("oneOf", []):
                branch_sample = {}
                for name in branch.get("required", []):
                    property_schema = branch.get("properties", {}).get(
                        name, value_schema.get("properties", {}).get(name, {})
                    )
                    branch_sample[name] = sample(property_schema, depth + 1)
                for name, property_schema in branch.get("properties", {}).items():
                    if "const" in property_schema:
                        branch_sample[name] = property_schema["const"]
                out.update(branch_sample)
                break
            for branch in value_schema.get("allOf", []):
                condition_schema = branch.get("if", {})
                condition = condition_schema.get("properties", {})
                required_condition = set(condition_schema.get("required", []))
                matches = required_condition <= set(out) and all(
                    out.get(name) == condition_schema["const"]
                    for name, condition_schema in condition.items()
                    if "const" in condition_schema
                )
                if not matches:
                    continue
                for name, condition_schema in condition.items():
                    if "const" in condition_schema:
                        out[name] = condition_schema["const"]
                for name in branch.get("then", {}).get("required", []):
                    if name not in out:
                        out[name] = sample(
                            value_schema.get("properties", {}).get(name, {}),
                            depth + 1,
                        )
                break
            return out
        if value_type == "array":
            return []
        if value_type == "integer":
            return value_schema.get("minimum", 0)
        if value_type == "number":
            return 0
        if value_type == "boolean":
            return False
        if value_type == "string":
            if value_schema.get("format") == "date-time":
                return "2026-01-01T00:00:00Z"
            if value_schema.get("format") == "uri":
                return "https://example.invalid/skill/SKILL.md"
            if value_schema.get("pattern", "").startswith("^[0-9]+\\."):
                return "1.0.0"
            if value_schema.get("pattern", "").startswith("^sha256:"):
                return "sha256:" + "a" * 64
            return "sample"
        return {}

    for event in sorted(registry):
        payload = payloads.get(event_key(event))
        if not payload:
            continue
        payload_value = sample(payload)
        if event.startswith("agent.session."):
            if payload_value.get("scope", {}).get("kind") == "CONVERSATION":
                payload_value["scope"]["conversation_turn_id"] = "turn-sample"
                payload_value["task_spec_revision"] = None
            else:
                payload_value["task_spec_revision"] = 1
        resource_input = {
            "resource_ref": {
                "workspace_id": "workspace-input",
                "resource_id": "resource-input",
                "revision_id": "revision-input",
            },
            "observed_digest": "sha256:" + "c" * 64,
        }
        if event in ("verification.started.v1", "verification.completed.v1"):
            payload_value["inputs"] = [resource_input]
        elif event == "artifact.version.created.v1":
            payload_value["input_refs"] = [resource_input["resource_ref"]]
        elif event == "resource.created.v1":
            payload_value["provenance"]["source_inputs"] = [resource_input]
            payload_value["provenance"]["transformations"] = [
                {"operation": "sample", "inputs": [resource_input]}
            ]
        envelope = {
            "event_id": "event-sample",
            "workspace_id": "workspace-sample",
            "entity_type": "sample",
            "entity_id": "entity-sample",
            "origin_runtime_id": "runtime-sample",
            "origin_sequence": 1,
            "entity_revision": 1,
            "hlc_timestamp": "2026-01-01T00:00:00Z",
            "correlation_id": "correlation-sample",
            "schema_version": 1,
            "type": event,
            "payload": payload_value,
            "aggregate_state_ref": {
                "blob": {
                    "digest": "sha256:" + "b" * 64,
                    "size_bytes": 1,
                    "media_type": "application/vnd.litecowork.aggregate+json",
                },
                "entity_revision": 1,
                "record_schema_version": 1,
            },
            "recorded_at": "2026-01-01T00:00:00Z",
            "payload_digest": "sha256:" + "a" * 64,
        }
        errors = list(validator.iter_errors(envelope))
        if errors:
            fail(f"{event}: generated valid payload sample is rejected: {errors[0].message}")
            continue
        if event in ("verification.started.v1", "verification.completed.v1"):
            invalid_digest = json.loads(json.dumps(envelope))
            invalid_digest["payload"]["inputs"][0]["observed_digest"] = "a" * 64
            if not list(validator.iter_errors(invalid_digest)):
                fail(f"{event}: accepts an unprefixed ResourceInput observed digest")
            missing_revision = json.loads(json.dumps(envelope))
            del missing_revision["payload"]["inputs"][0]["resource_ref"]["revision_id"]
            if not list(validator.iter_errors(missing_revision)):
                fail(f"{event}: accepts a ResourceInput without a pinned revision")
        if event == "user.request.created.v1":
            missing_turn = json.loads(json.dumps(envelope))
            del missing_turn["payload"]["conversation_turn_id"]
            if not list(validator.iter_errors(missing_turn)):
                fail("user.request.created.v1: accepts a Conversation request without its exact turn")
            mixed_scope = json.loads(json.dumps(envelope))
            mixed_scope["payload"]["task_id"] = "task-sample"
            if not list(validator.iter_errors(mixed_scope)):
                fail("user.request.created.v1: accepts a mixed Conversation and Task scope")
            task_scope = json.loads(json.dumps(envelope))
            task_scope["payload"].pop("conversation_id")
            task_scope["payload"].pop("conversation_turn_id")
            task_scope["payload"]["task_id"] = "task-sample"
            if list(validator.iter_errors(task_scope)):
                fail("user.request.created.v1: rejects a valid Task-planning request scope")
            external_auth = json.loads(json.dumps(envelope))
            external_auth["payload"]["kind"] = "EXTERNAL_AUTHORIZATION"
            external_auth["payload"]["interaction_mode"] = "EXTERNAL_URL"
            if list(validator.iter_errors(external_auth)):
                fail("user.request.created.v1: rejects a URL-mode external authorization request")
            invalid_external_mode = json.loads(json.dumps(external_auth))
            invalid_external_mode["payload"]["kind"] = "QUESTION"
            if not list(validator.iter_errors(invalid_external_mode)):
                fail("user.request.created.v1: accepts EXTERNAL_URL without EXTERNAL_AUTHORIZATION")
            legacy_parallel = json.loads(json.dumps(envelope))
            legacy_parallel["payload"]["input_refs"] = []
            legacy_parallel["payload"]["input_digests"] = []
            if not list(validator.iter_errors(legacy_parallel)):
                fail(f"{event}: accepts obsolete parallel input reference/digest arrays")
        for required in payload.get("required", []):
            broken = json.loads(json.dumps(envelope))
            broken["payload"].pop(required, None)
            if not list(validator.iter_errors(broken)):
                fail(f"{event}: schema accepts payload missing required field {required}")
                break


def check_provider_circuit_counter_contract() -> None:
    """Keep the ProviderCircuit u32 counter aligned across the model, event, and SQL."""
    model = (DOCS / "DATA-MODEL.md").read_text(encoding="utf-8")
    if "consecutive_failures: u32" not in model:
        fail("DATA-MODEL.md: ProviderCircuit.consecutive_failures must remain u32")

    event = load_json(DOCS / "schemas" / "domain-event.schema.json")
    counter = (
        event.get("$defs", {})
        .get("payloads", {})
        .get("provider_circuit_changed", {})
        .get("properties", {})
        .get("consecutive_failures", {})
    )
    maximum = (1 << 32) - 1
    if (
        counter.get("type") != "integer"
        or counter.get("minimum") != 0
        or counter.get("maximum") != maximum
    ):
        fail(
            "domain-event.schema.json: provider.circuit.changed.consecutive_failures "
            "must match the unsigned 32-bit ProviderCircuit counter"
        )

    sql = (DOCS / "schemas" / "sqlite-v1.sql").read_text(encoding="utf-8")
    sql_counter = "consecutive_failures INTEGER NOT NULL DEFAULT 0 CHECK (consecutive_failures >= 0 AND consecutive_failures <= 4294967295)"
    if sql_counter not in sql:
        fail(
            "sqlite-v1.sql: provider_circuit_states.consecutive_failures must "
            "enforce the unsigned 32-bit ProviderCircuit range"
        )


def check_error_registry() -> None:
    registry = load_json(DOCS / "schemas" / "error-codes.schema.json")
    machine_codes = set(registry.get("enum", []))
    text = (DOCS / "SCHEMAS.md").read_text(encoding="utf-8")
    section = re.search(r"## Assurance and errors(.*?)The machine-readable registry", text, re.S)
    if not section:
        fail("SCHEMAS.md: missing documented ErrorCode list")
        return
    blocks = re.findall(r"```(?:text)?\s*(.*?)```", section.group(1), re.S)
    code_block = next((block for block in blocks if "NOT_FOUND" in block), "")
    documented_codes = set(re.findall(r"\b[A-Z][A-Z0-9_]+\b", code_block))
    if documented_codes != machine_codes:
        fail(
            "ErrorCode registry mismatch: "
            f"missing={sorted(machine_codes-documented_codes)}, "
            f"extra={sorted(documented_codes-machine_codes)}"
        )
    event_schema = load_json(DOCS / "schemas" / "domain-event.schema.json")
    event_codes = set(event_schema.get("$defs", {}).get("error_code", {}).get("enum", []))
    if event_codes != machine_codes:
        fail(
            "domain-event.schema.json ErrorCode mismatch: "
            f"missing={sorted(machine_codes-event_codes)}, "
            f"extra={sorted(event_codes-machine_codes)}"
        )


def check_task_status_contract() -> None:
    schema_text = (DOCS / "SCHEMAS.md").read_text(encoding="utf-8")
    task_enum = re.search(r"TaskStatus\s*=([^\n]*(?:\n  [A-Z_| ]+)+)", schema_text)
    if not task_enum:
        fail("SCHEMAS.md: missing canonical TaskStatus")
        return
    canonical = set(re.findall(r"\b[A-Z][A-Z_]+\b", task_enum.group(1)))
    try:
        api_path = DOCS / "schemas" / "operator-api.openapi.yaml"
        api = yaml.load(api_path.read_text(encoding="utf-8"), Loader=UniqueKeyLoader)
        api_task = api["components"]["schemas"]["Task"]["properties"]["status"]["enum"]
        api_resume = api["components"]["schemas"]["Task"]["properties"]["resume_status"]["enum"]
        event_schema = load_json(DOCS / "schemas" / "domain-event.schema.json")
        event_task = event_schema["$defs"]["task_status"]["enum"]
        event_resume = event_schema["$defs"]["task_pause_resume_status"]["enum"]
        sql = (DOCS / "schemas" / "sqlite-v1.sql").read_text(encoding="utf-8")
        task_sql = re.search(r"CREATE TABLE tasks \((.*?)\n\);", sql, re.S).group(1)
        sql_status = re.search(r"status TEXT NOT NULL CHECK \(status IN \(([^)]*)\)\)", task_sql).group(1)
        sql_enum = re.findall(r"'([A-Z_]+)'", sql_status)
    except Exception as exc:  # noqa: BLE001
        fail(f"TaskStatus contract comparison failed: {exc}")
        return
    for source, values in (
        ("Operator API Task.status", set(api_task)),
        ("DomainEvent TaskStatus", set(event_task)),
        ("SQLite tasks.status", set(sql_enum)),
    ):
        if values != canonical:
            fail(f"TaskStatus mismatch in {source}: missing={sorted(canonical-values)}, extra={sorted(values-canonical)}")
    nullable_api_resume = set(api_resume) - {None}
    if nullable_api_resume != set(event_resume):
        fail("Task pause resume_status differs between Operator API and DomainEvent schema")


def check_task_lifecycle_contract() -> None:
    """Keep pause/cancel semantics aligned with planner-session ownership and API behavior."""
    schema_text = (DOCS / "SCHEMAS.md").read_text(encoding="utf-8")
    task_runtime = (DOCS / "TASK-RUNTIME.md").read_text(encoding="utf-8")
    state_machines = (DOCS / "STATE-MACHINES.md").read_text(encoding="utf-8")
    api_doc = (DOCS / "API.md").read_text(encoding="utf-8")
    flows = (DOCS / "FLOWS.md").read_text(encoding="utf-8")
    data_model = (DOCS / "DATA-MODEL.md").read_text(encoding="utf-8")
    sql = (DOCS / "schemas" / "sqlite-v1.sql").read_text(encoding="utf-8")
    openapi = yaml.load(
        (DOCS / "schemas" / "operator-api.openapi.yaml").read_text(encoding="utf-8"),
        Loader=UniqueKeyLoader,
    )

    if "PAUSE_REQUESTED | PAUSED -> CANCEL_REQUESTED" not in state_machines:
        fail("STATE-MACHINES.md: cancelling a pending or paused Task must supersede pause")
    if "active TASK_PLANNING session" not in state_machines or "proposals from the old planning session are rejected" not in state_machines:
        fail("STATE-MACHINES.md: pause/cancel must settle planner sessions and fence stale proposals")
    if "PlanningAssignment` is an internal, transient dispatch envelope, not a persisted domain" not in task_runtime:
        fail("TASK-RUNTIME.md: PlanningAssignment must be explicitly classified as transient")
    if "PlanningAssignment` is an internal transient admission envelope, not a durable entity" not in data_model:
        fail("DATA-MODEL.md: transient PlanningAssignment must not be mistaken for a missing entity")
    if "uq_active_task_planning_session" not in sql:
        fail("sqlite-v1.sql: only one active TASK_PLANNING session may exist per Task")
    if "Task becomes `CANCELLED` only after the planning session" not in flows:
        fail("FLOWS.md F10: Task cancellation must await planning-session settlement")
    for route in ("/tasks/{taskId}/cancel", "/tasks/{taskId}/pause", "/tasks/{taskId}/resume"):
        if route not in openapi.get("paths", {}):
            fail(f"Operator API: missing Task lifecycle route {route}")
    if "An expired request returns `USER_REQUEST_EXPIRED`; a dismissed," not in " ".join(api_doc.split()):
        fail("API.md: UserRequest expired and closed-state response errors must be distinguished")
    if "cancel from both `PAUSED` and `PAUSE_REQUESTED`" not in (DOCS / "BENCHMARKS.md").read_text(encoding="utf-8"):
        fail("BENCHMARKS.md: pause/cancel lifecycle coverage must include PAUSED and PAUSE_REQUESTED")


def check_resource_contract() -> None:
    schema_text = (DOCS / "SCHEMAS.md").read_text(encoding="utf-8")
    expected = {
        "ResourceFreshness": {"CURRENT", "STALE", "CONFLICTED", "UNKNOWN", "UNAVAILABLE"},
        "ResourceLocationFreshness": {"CURRENT", "STALE", "UNKNOWN", "UNAVAILABLE"},
        "DependencyFreshness": {"CURRENT", "STALE", "CONFLICTED", "UNKNOWN"},
    }
    api_path = DOCS / "schemas" / "operator-api.openapi.yaml"
    try:
        api = yaml.load(api_path.read_text(encoding="utf-8"), Loader=UniqueKeyLoader)
        schemas = api["components"]["schemas"]
        for name, canonical in expected.items():
            match = re.search(rf"^{name} = ([A-Z_| ]+)$", schema_text, re.M)
            if not match:
                fail(f"SCHEMAS.md: missing {name}")
                continue
            documented = set(match.group(1).replace(" ", "").split("|"))
            api_values = set(schemas[name]["enum"])
            if documented != canonical or api_values != canonical:
                fail(f"{name} mismatch: SCHEMAS.md={sorted(documented)}, OpenAPI={sorted(api_values)}")
        paths = api["paths"]
        if "get" not in paths.get("/resources/{resourceId}/revisions", {}):
            fail("OpenAPI: missing Resource revision ancestry endpoint")
        if "ResourceRevisionPage" not in schemas:
            fail("OpenAPI: missing paged Resource revision response schema")
        if schemas.get("ResourceLocationView", {}).get("properties", {}).get("freshness", {}).get("$ref") != "#/components/schemas/ResourceLocationFreshness":
            fail("OpenAPI: ResourceLocationView must use location freshness, not Resource conflict freshness")
        if schemas.get("ResourceSearchResult", {}).get("properties", {}).get("freshness", {}).get("$ref") != "#/components/schemas/ResourceFreshness":
            fail("OpenAPI: ResourceSearchResult must expose logical Resource freshness")
        dependent_route = paths.get("/resources/{resourceId}/dependents", {}).get("get", {})
        dependent_schema = (
            dependent_route.get("responses", {})
            .get("200", {})
            .get("content", {})
            .get("application/json", {})
            .get("schema", {})
            .get("$ref")
        )
        if dependent_schema != "#/components/schemas/ResourceDependencyPage":
            fail("OpenAPI: Resource dependents must return the typed dependency/freshness projection")
        invalidation_route = paths.get("/dependency-edges/{dependencyEdgeId}/invalidations", {}).get("get", {})
        if not invalidation_route:
            fail("OpenAPI: missing paginated DependencyEdge invalidation-history route")
        pinned = schemas.get("PinnedResourceRef", {})
        if "revision_id" not in pinned.get("allOf", [{}, {}])[1].get("required", []):
            fail("OpenAPI: PinnedResourceRef must require revision_id")
        artifact = schemas.get("Artifact", {})
        version = schemas.get("ArtifactVersion", {})
        if "resource_id" not in artifact.get("required", []):
            fail("OpenAPI Artifact must expose its stable Resource identity")
        if "resource_revision_id" not in version.get("required", []):
            fail("OpenAPI ArtifactVersion must expose its mapped Resource revision")
        if version.get("properties", {}).get("input_refs", {}).get("items", {}).get("$ref") != "#/components/schemas/PinnedResourceRef":
            fail("OpenAPI ArtifactVersion inputs must pin exact Resource revisions")
        verification = schemas.get("VerificationRun", {})
        if "inputs" not in verification.get("required", []) or verification.get("properties", {}).get("inputs", {}).get("items", {}).get("$ref") != "#/components/schemas/ResourceInput":
            fail("OpenAPI VerificationRun must require paired ResourceInput records")
        api_resource_input = schemas.get("ResourceInput", {})
        observed_digest = api_resource_input.get("properties", {}).get("observed_digest", {})
        if observed_digest.get("pattern") != "^sha256:[a-f0-9]{64}$" or observed_digest.get("type") != "string":
            fail("OpenAPI ResourceInput must use a strict, non-null SHA-256 digest when present")
        api_resource_ref = schemas.get("ResourceRef", {})
        if "content_digest" in api_resource_ref.get("properties", {}):
            fail("OpenAPI ResourceRef must not duplicate the ResourceInput observed digest")
        resource_revision = schemas.get("ResourceRevision", {})
        resource_revision_digest = resource_revision.get("properties", {}).get("content_digest", {})
        if resource_revision_digest.get("pattern") != SHA256_PATTERN:
            fail("OpenAPI ResourceRevision.content_digest must use Sha256Digest")
    except Exception as exc:  # noqa: BLE001
        fail(f"Resource contract comparison failed: {exc}")

    events = load_json(DOCS / "schemas" / "domain-event.schema.json")
    payloads = events.get("$defs", {}).get("payloads", {})
    created = payloads.get("resource_created", {})
    if "identity_digest" in created.get("required", []):
        fail("resource.created: identity_digest must be optional for weak identities")
    revision = payloads.get("resource_revision_observed", {})
    parents = revision.get("properties", {}).get("parent_revision_ids", {})
    if "parent_revision_ids" not in revision.get("required", []) or parents.get("type") != "array" or not parents.get("uniqueItems"):
        fail("resource.revision.observed: unique parent_revision_ids must be required")
    invalidation = payloads.get("resource_invalidation_created", {})
    if not {"dependency_edge_id", "observed_revision_id"} <= set(invalidation.get("required", [])):
        fail("resource.invalidation.created: must link the exact dependency edge to the newly observed revision")
    artifact_inputs = payloads.get("artifact_version_created", {}).get("properties", {}).get("input_refs", {})
    if artifact_inputs.get("items", {}).get("$ref") != "#/$defs/pinned_resource_ref":
        fail("artifact_version_created: input refs must pin exact Resource revisions")
    for event_name in ("verification_started", "verification_completed"):
        inputs = payloads.get(event_name, {}).get("properties", {}).get("inputs", {})
        if inputs.get("items", {}).get("$ref") != "#/$defs/resource_input":
            fail(f"{event_name}: inputs must pair pinned Resource refs with observed digests")
    event_resource_input = events.get("$defs", {}).get("resource_input", {})
    event_observed_digest = event_resource_input.get("properties", {}).get("observed_digest", {})
    if event_observed_digest.get("pattern") != "^sha256:[a-f0-9]{64}$":
        fail("domain-event ResourceInput must use a strict SHA-256 observed digest")
    for schema_name, field in (("provenance_record", "source_inputs"), ("provenance_transformation", "inputs")):
        provenance = events.get("$defs", {}).get(schema_name, {})
        if provenance.get("properties", {}).get(field, {}).get("items", {}).get("$ref") != "#/$defs/resource_input":
            fail(f"domain-event {schema_name}.{field} must use paired ResourceInput records")
    if "content_digest" in events.get("$defs", {}).get("resource_ref", {}).get("properties", {}):
        fail("domain-event ResourceRef must not duplicate the ResourceInput observed digest")
    check_digest_encodings(events, "domain-event.schema.json")

    artifact_created = payloads.get("artifact_created", {})
    artifact_version_created = payloads.get("artifact_version_created", {})
    if "resource_id" not in artifact_created.get("required", []):
        fail("artifact.created: event must expose the Artifact Resource identity")
    for field in ("resource_id", "resource_revision_id"):
        if field not in artifact_version_created.get("required", []):
            fail(f"artifact.version.created: event must require {field}")

    model = (DOCS / "DATA-MODEL.md").read_text(encoding="utf-8")
    world = (DOCS / "WORLD-RESOURCES.md").read_text(encoding="utf-8")
    artifact_contract = (DOCS / "ARTIFACTS-EVIDENCE.md").read_text(encoding="utf-8")
    services = (DOCS / "SERVICES.md").read_text(encoding="utf-8")
    tests = (DOCS / "TESTING.md").read_text(encoding="utf-8")
    if "acyclic" not in model or "RESOURCE_CONFLICT" not in model or "multiple heads" not in world:
        fail("Resource model must specify acyclic revision ancestry and conflict behavior")
    if "provenance.source_inputs" not in model or "provenance.transformations[].inputs" not in model:
        fail("DATA-MODEL.md must bind ArtifactVersion dependency refs to all provenance inputs")
    if "input_refs` to equal" not in artifact_contract or "INTEGRITY_FAILURE" not in services:
        fail("ArtifactStore must enforce exact provenance dependencies and input digest integrity")
    if "ArtifactVersion input refs exactly match provenance" not in tests:
        fail("TESTING.md must cover ArtifactVersion/provenance dependency-set consistency")
    sql = (DOCS / "schemas" / "sqlite-v1.sql").read_text(encoding="utf-8")
    for required in (
        "CREATE TABLE resource_revision_parents",
        "CREATE UNIQUE INDEX uq_resources_identity_digest",
        "FOREIGN KEY(resource_id, current_revision_id)",
        "FOREIGN KEY(resource_id, observed_revision_id)",
    ):
        if required not in sql:
            fail(f"sqlite-v1.sql: missing Resource integrity rule {required}")
    for required in (
        "resource_id TEXT NOT NULL REFERENCES resources(resource_id)",
        "resource_revision_id TEXT NOT NULL",
        "UNIQUE(artifact_id, resource_id)",
        "FOREIGN KEY(resource_id, resource_revision_id) REFERENCES resource_revisions(resource_id, resource_revision_id)",
        "guard_artifact_current_revision",
        "guard_artifact_initial_revision",
    ):
        if required not in sql:
            fail(f"sqlite-v1.sql: missing Artifact-to-Resource integrity rule {required}")
    if "every ArtifactVersion maps to" not in model or "ResourceRevision of that Resource" not in model:
        fail("DATA-MODEL.md: missing ArtifactVersion-to-ResourceRevision rule")
    if "UNIQUE(source_revision_id, dependent_kind, dependent_ref)" not in sql:
        if "UNIQUE(dependency_edge_id, observed_revision_id)" not in sql:
            fail("sqlite-v1.sql: invalidation records must deduplicate per dependency edge and observed revision")
    if "CREATE TABLE dependency_edges" not in sql or "idx_dependency_edges_source" not in sql:
        fail("sqlite-v1.sql: missing indexed DependencyEdge reverse projection")
    if "UNIQUE(dependency_edge_id, observed_revision_id)" not in sql or "invalidation_observation_same_resource" not in sql:
        fail("sqlite-v1.sql: invalidation must be unique per edge/revision and bind to the same logical Resource")


def check_async_capability_contract() -> None:
    model = (DOCS / "DATA-MODEL.md").read_text(encoding="utf-8")
    canonical_schemas = (DOCS / "SCHEMAS.md").read_text(encoding="utf-8")
    invocation_doc = (DOCS / "CAPABILITY-INVOCATIONS.md").read_text(encoding="utf-8")
    tests = (DOCS / "TESTING.md").read_text(encoding="utf-8")
    api_doc = (DOCS / "API.md").read_text(encoding="utf-8")
    fabric = (DOCS / "CAPABILITY-FABRIC.md").read_text(encoding="utf-8")
    recovery = (DOCS / "FAILURE-RECOVERY.md").read_text(encoding="utf-8")
    events_doc = (DOCS / "EVENTS.md").read_text(encoding="utf-8")
    automation_doc = (DOCS / "AUTOMATION.md").read_text(encoding="utf-8")
    storage_doc = (DOCS / "STORAGE.md").read_text(encoding="utf-8")
    user_model = re.search(r"## UserRequest and response(.*?)(?=\n## )", model, re.S)
    if not user_model or "provider_input_key: string?" in user_model.group(1):
        fail("DATA-MODEL.md: UserRequest must not expose provider input keys")
    if "provider_input_key_ciphertext: EncryptedBytes" not in (user_model.group(1) if user_model else ""):
        fail("DATA-MODEL.md: encrypted Runtime-local ProviderInputBinding is missing")
    if "provider_task_ttl_ms: u64?" not in model or "provider_task_ttl_ms?" not in invocation_doc:
        fail("CapabilityInvocation must persist the provider's latest reported TTL")
    invocation_model = re.search(r"## CapabilityInvocation\s+```text\n(.*?)```", model, re.S)
    if not invocation_model or "provider_task_ref:" in invocation_model.group(1) or "provider_cursor:" in invocation_model.group(1):
        fail("DATA-MODEL.md: shared CapabilityInvocation must omit provider task handles/cursors")
    if "CapabilityInvocationProviderBinding" not in model or "AutomationTriggerBinding" not in model:
        fail("DATA-MODEL.md: encrypted Runtime-local provider continuation bindings are incomplete")
    for name in ("ProviderTaskStatus", "ProviderContinuationBindingStatus", "ProviderInputDeliveryStatus"):
        if not re.search(rf"^{name} = ", canonical_schemas, re.M):
            fail(f"SCHEMAS.md: missing canonical {name} enum")
    if "state: ProviderContinuationBindingStatus" not in model or "delivery_status: ProviderInputDeliveryStatus" not in model:
        fail("DATA-MODEL.md: local provider bindings must use canonical status types")
    if "UNKNOWN`, moves the Invocation to" not in invocation_doc or "`AMBIGUOUS`" not in invocation_doc:
        fail("CAPABILITY-INVOCATIONS.md: unknown provider statuses must fail closed as ambiguous")
    if "provider-native task handles and resume cursors remain runtime-private" not in " ".join(api_doc.lower().split()):
        fail("API.md: provider task handles and resume cursors must remain Runtime-private")
    if "host-managed" not in fabric or "durably recorded as a CapabilityInvocation" not in fabric:
        fail("Direct native-tool attachment must remain broker-observed and durably invoked")
    if "initial MCP task handle response lost after dispatch" not in recovery or "never blindly redispatch" not in recovery:
        fail("FAILURE-RECOVERY.md: lost initial provider task handles must not cause blind redispatch")

    events = load_json(DOCS / "schemas" / "domain-event.schema.json")
    payloads = events.get("$defs", {}).get("payloads", {})
    for event_name in ("capability_invocation_dispatched", "capability_invocation_checkpointed"):
        props = payloads.get(event_name, {}).get("properties", {})
        if props.get("provider_task_ttl_ms", {}).get("minimum") != 0:
            fail(f"domain-event {event_name}: provider_task_ttl_ms must be a nonnegative integer")
        task_status = props.get("provider_task_status", {}).get("enum", [])
        if set(task_status) != {"WORKING", "INPUT_REQUIRED", "COMPLETED", "FAILED", "CANCELLED", "UNKNOWN"}:
            fail(f"domain-event {event_name}: provider task state must be normalized")
    for event_name, forbidden in (
        ("capability_invocation_dispatched", {"provider_task_ref"}),
        ("capability_invocation_checkpointed", {"provider_cursor"}),
        ("user_request_created", {"provider_input_key"}),
    ):
        props = payloads.get(event_name, {}).get("properties", {})
        if forbidden & set(props):
            fail(f"domain-event {event_name}: Runtime-private provider data must not replicate")
    request_payload = payloads.get("user_request_created", {})
    request_kind = request_payload.get("properties", {}).get("kind", {}).get("enum", [])
    if set(request_kind) != {"QUESTION", "DECISION", "RESOURCE_SELECTION", "EXTERNAL_AUTHORIZATION"}:
        fail("user.request.created.v1 must distinguish external authorization from Approval")
    request_modes = request_payload.get("properties", {}).get("interaction_mode", {}).get("enum", [])
    if set(request_modes) != {"FORM", "EXTERNAL_URL"}:
        fail("user.request.created.v1 must identify the input interaction mode")
    request_payload_properties = set(request_payload.get("properties", {}))
    if {"url", "external_url", "provider_input_key"} & request_payload_properties:
        fail("user.request.created.v1 must not replicate provider URLs or input keys")

    api_path = DOCS / "schemas" / "operator-api.openapi.yaml"
    try:
        api = yaml.load(api_path.read_text(encoding="utf-8"), Loader=UniqueKeyLoader)
        schemas = api["components"]["schemas"]
        invocation = schemas["CapabilityInvocation"].get("properties", {})
        if invocation.get("provider_task_ttl_ms", {}).get("minimum") != 0:
            fail("OpenAPI CapabilityInvocation must expose only the sanitized provider TTL observation")
        api_task_status = invocation.get("provider_task_status", {}).get("enum", [])
        if set(api_task_status) != {"WORKING", "INPUT_REQUIRED", "COMPLETED", "FAILED", "CANCELLED", "UNKNOWN", None}:
            fail("OpenAPI CapabilityInvocation provider status must use the normalized nullable enum")
        if "provider_task_ref" in invocation or "provider_cursor" in invocation:
            fail("OpenAPI must not expose Runtime-private provider task handles or resume cursors")
        turn_event = payloads.get("conversation_turn_settled", {}).get("properties", {}).get("to", {}).get("enum", [])
        if "WAITING_DEPENDENCY" not in turn_event:
            fail("conversation.turn.settled.v1 must include WAITING_DEPENDENCY in the turn status enum")
        request = schemas["UserRequest"]
        required = set(request.get("required", []))
        if "conversation_id" in required:
            fail("OpenAPI UserRequest must allow Task-originated requests without a Conversation")
        if "provider_input_key" in request.get("properties", {}) or "external_url" in request.get("properties", {}):
            fail("OpenAPI must not expose provider input keys or URLs in ordinary UserRequest projections")
        if "interaction_mode" not in request.get("properties", {}):
            fail("OpenAPI UserRequest must expose the form/external URL interaction mode")
        handoff = api.get("paths", {}).get("/user-requests/{requestId}/external-handoff", {}).get("post", {})
        if not handoff or "no-store" not in str(handoff):
            fail("OpenAPI must define an explicit no-store external-auth handoff route")
        for turn_schema_name in ("ConversationTurn", "ConversationTurnReceipt"):
            turn_status = schemas[turn_schema_name].get("properties", {}).get("status", {}).get("enum", [])
            if "WAITING_DEPENDENCY" not in turn_status:
                fail(f"OpenAPI {turn_schema_name} must expose ConversationTurn WAITING_DEPENDENCY")
        if set(request.get("properties", {}).get("kind", {}).get("enum", [])) != {
            "QUESTION", "DECISION", "RESOURCE_SELECTION", "EXTERNAL_AUTHORIZATION"
        }:
            fail("OpenAPI UserRequest must distinguish external authorization from Approval")
        request_validator = jsonschema.Draft202012Validator(
            {"components": {"schemas": schemas}, "$ref": "#/components/schemas/UserRequest"}
        )
        conversation_request = {
            "request_id": "request", "workspace_id": "workspace", "conversation_id": "conversation",
            "conversation_turn_id": "turn", "task_id": None, "attempt_id": None,
            "agent_session_id": "session", "invocation_id": None, "kind": "QUESTION", "prompt": "Confirm",
            "interaction_mode": "FORM",
            "response_schema": None, "choices": [], "status": "PENDING", "expires_at": None,
            "created_at": "2026-01-01T00:00:00Z", "resolved_at": None, "resolved_by": None,
            "response_digest": None, "version": 1,
        }
        task_request = {
            **conversation_request, "conversation_id": None, "conversation_turn_id": None, "task_id": "task"
        }
        attempt_request = {**task_request, "attempt_id": "attempt"}
        external_auth_request = {
            **conversation_request, "kind": "EXTERNAL_AUTHORIZATION", "interaction_mode": "EXTERNAL_URL"
        }
        mismatched_external_auth_request = {
            **conversation_request, "kind": "QUESTION", "interaction_mode": "EXTERNAL_URL"
        }
        for candidate, expected in (
            (conversation_request, True),
            ({**conversation_request, "conversation_turn_id": None}, False),
            ({**conversation_request, "task_id": "task"}, False),
            (task_request, True),
            (attempt_request, True),
            (external_auth_request, True),
            (mismatched_external_auth_request, False),
            ({**external_auth_request, "response_schema": {"type": "object"}}, False),
            ({**external_auth_request, "choices": [{"choice_id": "x", "label": "x", "value": "x"}]}, False),
        ):
            accepted = not list(request_validator.iter_errors(candidate))
            if accepted != expected:
                fail("OpenAPI UserRequest must require exactly one valid scope tuple including its ConversationTurn")
    except Exception as exc:  # noqa: BLE001
        fail(f"Async invocation/UserRequest OpenAPI comparison failed: {exc}")

    sql = (DOCS / "schemas" / "sqlite-v1.sql").read_text(encoding="utf-8")
    invocation_table = re.search(r"CREATE TABLE capability_invocations \((.*?)\n\);", sql, re.S)
    request_table = re.search(r"CREATE TABLE user_requests \((.*?)\n\);", sql, re.S)
    invocation_binding = re.search(r"CREATE TABLE capability_invocation_provider_bindings \((.*?)\n\);", sql, re.S)
    input_binding = re.search(r"CREATE TABLE provider_input_bindings \((.*?)\n\);", sql, re.S)
    automation_binding = re.search(r"CREATE TABLE automation_trigger_bindings \((.*?)\n\);", sql, re.S)
    if not invocation_table or "provider_task_ttl_ms INTEGER" not in invocation_table.group(1):
        fail("sqlite-v1.sql: CapabilityInvocation must store provider_task_ttl_ms")
    elif "scope_kind = 'TASK_PLANNING'" not in invocation_table.group(1) or "attempt_id IS NULL AND effect_id IS NULL" not in invocation_table.group(1):
        fail("sqlite-v1.sql: Conversation and planning Invocations must not carry Effects")
    if not request_table or "provider_input_key" in request_table.group(1):
        fail("sqlite-v1.sql: UserRequest must not store provider input keys")
    if not invocation_table or "provider_task_ref TEXT" in invocation_table.group(1) or "provider_cursor TEXT" in invocation_table.group(1):
        fail("sqlite-v1.sql: shared CapabilityInvocation must omit opaque provider handles/cursors")
    if not invocation_binding or "binding_ciphertext BLOB NOT NULL" not in invocation_binding.group(1):
        fail("sqlite-v1.sql: missing encrypted local CapabilityInvocation provider binding")
    if not input_binding or "provider_input_key_ciphertext BLOB NOT NULL" not in input_binding.group(1) or "provider_input_payload_ciphertext BLOB NOT NULL" not in input_binding.group(1) or "UNIQUE(invocation_id, provider_input_key_tag)" not in input_binding.group(1) or "retry_safety_proof_digest TEXT" not in input_binding.group(1):
        fail("sqlite-v1.sql: encrypted provider input bindings must deduplicate within an Invocation")
    input_identity_guard = re.search(
        r"CREATE TRIGGER provider_input_binding_identity_immutable(.*?)(?=\nCREATE TABLE|\Z)", sql, re.S
    )
    if not input_identity_guard or "OLD.provider_input_payload_ciphertext <> NEW.provider_input_payload_ciphertext" not in input_identity_guard.group(1):
        fail("sqlite-v1.sql: encrypted provider input payload and URL must remain immutable")
    for required_trigger in (
        "provider_input_binding_transition_guard",
        "provider_input_binding_response_immutable",
        "provider_input_binding_dispatch_authority",
        "provider_input_binding_dispatch_proof_guard",
    ):
        if required_trigger not in sql:
            fail(f"sqlite-v1.sql: provider input outbox is missing {required_trigger}")
    for required_trigger in (
        "user_request_status_transition_guard",
        "user_request_answer_matches_response",
        "user_request_response_insert_pending",
        "user_request_external_response_shape",
        "user_request_response_parent_eligible",
        "user_request_response_immutable_update",
        "user_request_response_immutable_delete",
    ):
        if required_trigger not in sql:
            fail(f"sqlite-v1.sql: immutable UserRequest response contract is missing {required_trigger}")
    input_authority = re.search(
        r"CREATE TRIGGER provider_input_binding_dispatch_authority(.*?)(?=\nCREATE TABLE|\Z)",
        sql,
        re.S,
    )
    if not input_authority or any(
        required not in input_authority.group(1)
        for required in (
            "JOIN agent_sessions active_session",
            "ON active_session.agent_session_id = p.agent_session_id",
            "active_session.agent_session_id <> source_session.agent_session_id",
            "active_session.scope_kind = 'ATTEMPT_EXECUTION'",
            "active_session.status = 'ACTIVE'",
            "active_session.runtime_incarnation_id = p.runtime_incarnation_id",
            "source_session.status IN ('CLOSED', 'LOST')",
            "current_planner.agent_session_id <> source_session.agent_session_id",
            "current_planner.workspace_id = t.workspace_id",
            "planner_runtime.current_incarnation_id = current_planner.runtime_incarnation_id",
        )
    ):
        fail("sqlite-v1.sql: Attempt provider-input delivery requires its current fresh active AgentSession")
    cursor_table = re.search(r"CREATE TABLE automation_cursors \((.*?)\n\);", sql, re.S)
    if not cursor_table or "provider_cursor TEXT" in cursor_table.group(1):
        fail("sqlite-v1.sql: shared AutomationCursor must omit opaque provider cursors")
    if not automation_binding or "cursor_ciphertext BLOB NOT NULL" not in automation_binding.group(1):
        fail("sqlite-v1.sql: missing encrypted local AutomationTriggerBinding")
    if not automation_binding or "cursor_digest TEXT NOT NULL" not in automation_binding.group(1):
        fail("sqlite-v1.sql: encrypted AutomationTriggerBinding requires a ciphertext digest")
    schema_enums = {}
    for name in ("ProviderTaskStatus", "ProviderContinuationBindingStatus", "ProviderInputDeliveryStatus"):
        match = re.search(rf"^{name} = ([^\n]*(?:\n  [^\n]*)*)", canonical_schemas, re.M)
        if match:
            schema_enums[name] = set(re.findall(r"[A-Z][A-Z0-9_]+", match.group(1)))
    expected_binding_states = schema_enums.get("ProviderContinuationBindingStatus", set())
    expected_input_states = schema_enums.get("ProviderInputDeliveryStatus", set())
    expected_task_states = schema_enums.get("ProviderTaskStatus", set())
    def checked_values(pattern: str, label: str) -> set[str]:
        match = re.search(pattern, sql, re.S)
        if not match:
            fail(f"sqlite-v1.sql: missing {label} CHECK constraint")
            return set()
        return set(re.findall(r"'([A-Z][A-Z0-9_]+)'", match.group(1)))
    sql_binding_states = checked_values(
        r"CREATE TABLE capability_invocation_provider_bindings \(.*?state TEXT NOT NULL CHECK \(state IN \((.*?)\)\)",
        "CapabilityInvocationProviderBinding state",
    )
    sql_cursor_binding_states = checked_values(
        r"CREATE TABLE automation_trigger_bindings \(.*?state TEXT NOT NULL CHECK \(state IN \((.*?)\)\)",
        "AutomationTriggerBinding state",
    )
    sql_input_states = checked_values(
        r"CREATE TABLE provider_input_bindings \(.*?delivery_status TEXT NOT NULL CHECK \(delivery_status IN \((.*?)\)\)",
        "ProviderInputBinding delivery status",
    )
    sql_task_states = checked_values(
        r"CREATE TABLE capability_invocations \(.*?provider_task_status TEXT CHECK \(provider_task_status IS NULL OR provider_task_status IN \((.*?)\)\)",
        "CapabilityInvocation provider task status",
    )
    for label, actual in (
        ("CapabilityInvocationProviderBinding", sql_binding_states),
        ("AutomationTriggerBinding", sql_cursor_binding_states),
    ):
        if expected_binding_states and actual != expected_binding_states:
            fail(f"sqlite-v1.sql: {label} states differ from SCHEMAS.md")
    if expected_input_states and sql_input_states != expected_input_states:
        fail("sqlite-v1.sql: ProviderInputBinding delivery states differ from SCHEMAS.md")
    if expected_task_states and sql_task_states != expected_task_states:
        fail("sqlite-v1.sql: provider task statuses differ from SCHEMAS.md")
    stale_privacy_text = (
        "raw opaque provider cursors stay in the protected aggregate state",
        "provider cursor, last digest, observation reference",
        "provider task reference and provider input key by invocation",
        "keyed in shared state only by a non-reversible local tag",
        "provider task handles/cursors are indexed only inside",
    )
    privacy_corpus = "\n".join((events_doc.lower(), automation_doc.lower(), storage_doc.lower(), invocation_doc.lower()))
    for stale in stale_privacy_text:
        if stale in privacy_corpus:
            fail(f"Provider continuation privacy docs contain stale shared-state wording: {stale}")
    if "automation_trigger_binding_update_matches_current_owner" not in sql or "automation_trigger_binding_identity_immutable" not in sql:
        fail("sqlite-v1.sql: AutomationTriggerBinding must validate its owner and immutable identity")
    if "capability_invocation_provider_binding_matches_activation" not in sql or "provider_input_binding_scope_matches_request" not in sql:
        fail("sqlite-v1.sql: Runtime-local invocation/input bindings must enforce source ownership")
    for required in (
        "answered during `PAUSE_REQUESTED` is rejected",
        "resolution is one-way from PENDING",
        "originating AgentSession to be `CLOSED` or `LOST`",
        "Task-planning continuation planner on a different Runtime is eligible only when that Runtime incarnation is current",
        "replacement Attempt, Runtime, or Runtime incarnation cannot receive an old provider input key",
        "daemon restart changes RuntimeIncarnation and makes old input/task handles reconciliation-only",
        "ambiguous provider-input retry requires an authenticated observation",
        "cancelling before provider-input dispatch atomically withdraws the outbox",
        "EXTERNAL_URL UserRequests require EXTERNAL_AUTHORIZATION",
        "credential-like MCP form fields and unknown embedded input methods fail closed",
        "external handoff requires an authenticated explicit Operator action",
    ):
        if required not in tests:
            fail(f"TESTING.md: missing provider-input lifecycle conformance case: {required}")


def check_provider_host_contract() -> None:
    """Keep LiteCowork's provider-instance view distinct from LitePSM process ownership."""
    model = (DOCS / "DATA-MODEL.md").read_text(encoding="utf-8")
    fabric = (DOCS / "CAPABILITY-FABRIC.md").read_text(encoding="utf-8")
    lifecycle = (DOCS / "RUNTIME-LIFECYCLE.md").read_text(encoding="utf-8")
    services = (DOCS / "SERVICES.md").read_text(encoding="utf-8")
    tests = (DOCS / "TESTING.md").read_text(encoding="utf-8")
    storage_doc = (DOCS / "STORAGE.md").read_text(encoding="utf-8")
    events_doc = (DOCS / "EVENTS.md").read_text(encoding="utf-8")
    implementation = (DOCS / "IMPLEMENTATION.md").read_text(encoding="utf-8")
    observability = (DOCS / "OBSERVABILITY.md").read_text(encoding="utf-8")
    if "CapabilityHostInstance {" not in model or "CapabilityActivationHostBinding {" not in model or "active_activation_count: u32 # derived view value" not in model:
        fail("DATA-MODEL.md: provider host view, local Activation binding, and derived use count are required")
    if "global process reference counting" not in fabric and "global process reference count" not in lifecycle:
        fail("Capability ownership must distinguish LiteCowork use count from LitePSM global process references")
    if "CapabilityHostSupervisor" not in services or "release_activation" not in services:
        fail("SERVICES.md: CapabilityHostSupervisor readiness/use-reference contract is missing")
    if "active_activation_count" not in tests or "never shared" not in tests:
        fail("TESTING.md: shared provider use-count and isolation conformance is missing")
    if "failover creates a new local Activation" not in tests or "omits" not in storage_doc or "not replicated Task aggregates" not in events_doc:
        fail("Provider host handles must stay Runtime-local across event replication, failover, and Workspace restore")
    if "CapabilityHost activation/binding overhead" not in implementation or "CapabilityHost activation/binding overhead" not in observability or "Freeze numeric Stage 1 targets" not in implementation:
        fail("Stage 1 must benchmark CapabilityHost activation overhead and freeze measured numeric SLOs")

    sql = (DOCS / "schemas" / "sqlite-v1.sql").read_text(encoding="utf-8")
    table_match = re.search(r"CREATE TABLE capability_host_instances \((.*?)\n\);", sql, re.S)
    activation_match = re.search(r"CREATE TABLE capability_activations \((.*?)\n\);", sql, re.S)
    binding_match = re.search(r"CREATE TABLE capability_activation_host_bindings \((.*?)\n\);", sql, re.S)
    session_match = re.search(r"CREATE TABLE agent_sessions \((.*?)\n\);", sql, re.S)
    session_binding_match = re.search(r"CREATE TABLE agent_session_host_bindings \((.*?)\n\);", sql, re.S)
    agent_host_match = re.search(r"CREATE TABLE agent_host_instances \((.*?)\n\);", sql, re.S)
    if not table_match or not activation_match or not binding_match or not session_match or not session_binding_match or not agent_host_match:
        fail("sqlite-v1.sql: CapabilityHostInstance, Activation, and Runtime-local host binding storage are required")
        return
    host_table = table_match.group(1)
    activation_table = activation_match.group(1)
    binding_table = binding_match.group(1)
    session_table = session_match.group(1)
    session_binding_table = session_binding_match.group(1)
    agent_host_table = agent_host_match.group(1)
    for field in ("runtime_incarnation_id", "capability_identity_digest", "configuration_digest", "isolation_partition_digest", "sharing_policy", "state", "health", "observed_at", "expires_at"):
        if not re.search(rf"^\s*{field}\s+", host_table, re.M):
            fail(f"sqlite-v1.sql: capability_host_instances missing {field}")
    if "active_activation_count" in host_table:
        fail("sqlite-v1.sql: active LiteCowork Activation count must be derived, not stored")
    if "host_instance_id" in activation_table or "provider_handle_ref" in activation_table:
        fail("sqlite-v1.sql: replicated capability_activations must not embed Runtime-local host handles")
    if not {"workspace_id", "scope_kind", "conversation_id", "task_id", "attempt_id", "runtime_id", "runtime_incarnation_id"} <= set(re.findall(r"^\s*(\w+)\s+", activation_table, re.M)):
        fail("sqlite-v1.sql: durable CapabilityActivation must persist its scope and origin Runtime incarnation")
    if "CHECK ((scope_kind = 'CONVERSATION'" not in activation_table or "FOREIGN KEY(runtime_id, runtime_incarnation_id) REFERENCES runtime_incarnations" not in activation_table:
        fail("sqlite-v1.sql: CapabilityActivation scope variants and matching Runtime/incarnation must be constrained")
    if "host_instance_id TEXT NOT NULL REFERENCES capability_host_instances" not in binding_table or "activation_id TEXT PRIMARY KEY REFERENCES capability_activations" not in binding_table:
        fail("sqlite-v1.sql: local host binding must join a durable Activation to its Runtime host")
    if "capability_activation_host_binding_matches_origin" not in sql:
        fail("sqlite-v1.sql: CapabilityActivationHostBinding origin identity match must be enforced")
    if "capability_activation_identity_immutable" not in sql or "capability_activation_host_binding_delete_requires_settlement" not in sql:
        fail("sqlite-v1.sql: CapabilityActivation identity and host-binding release require durable settlement guards")
    if any(field in session_table for field in ("host_instance_id", "native_session_ref")):
        fail("sqlite-v1.sql: durable AgentSession must not embed Runtime-local host/native handles")
    if "FOREIGN KEY(runtime_id, runtime_incarnation_id) REFERENCES runtime_incarnations" not in session_table:
        fail("sqlite-v1.sql: AgentSession must reference a valid Runtime incarnation")
    if "agent_session_host_binding_matches_origin" not in sql or "agent_session_identity_immutable" not in sql or "agent_session_host_binding_delete_requires_settlement" not in sql:
        fail("sqlite-v1.sql: AgentSession local host-binding identity and settlement guards are required")
    if "host_instance_id TEXT NOT NULL REFERENCES agent_host_instances" not in session_binding_table or "native_session_ref TEXT" not in session_binding_table:
        fail("sqlite-v1.sql: AgentSessionHostBinding must own the Runtime-local host and native handle")
    if "active_session_count" in agent_host_table:
        fail("sqlite-v1.sql: AgentHost session-use count must be derived, not stored")
    invocation_guard = re.search(r"CREATE TRIGGER capability_invocation_admission_matches_scope(.*?)\nEND;", sql, re.S)
    live_authority = re.search(r"CREATE VIEW live_agent_session_invocation_authority AS(.*?)CREATE TABLE capability_invocations", sql, re.S)
    dispatch_guard = re.search(r"CREATE TRIGGER capability_invocation_first_dispatch_revalidates_owner(.*?)\nEND;", sql, re.S)
    if not invocation_guard or any(check not in invocation_guard.group(1) for check in ("a.scope_kind = s.scope_kind", "a.runtime_id = s.runtime_id", "a.runtime_incarnation_id = s.runtime_incarnation_id", "g.expires_at")):
        fail("sqlite-v1.sql: Invocation admission must match Activation scope, unexpired Grant, and Runtime/incarnation to its AgentSession")
    if not live_authority or any(check not in live_authority.group(1) for check in ("ct.agent_session_id = s.agent_session_id", "ct.status = 'RUNNING'", "t.current_spec_revision = s.task_spec_revision", "t.lead_agent_binding_id = s.agent_binding_id", "p.status = 'RUNNING'", "st.current_attempt_id = p.attempt_id", "l.state = 'ACTIVE'", "julianday(l.expires_at) > julianday('now')", "pr.task_spec_revision = s.task_spec_revision")):
        fail("sqlite-v1.sql: Live Invocation authority must bind the current turn, current planning spec/lead, or running Attempt/Step/lease/plan revision")
    if not dispatch_guard or "live_agent_session_invocation_authority" not in dispatch_guard.group(1):
        fail("sqlite-v1.sql: first provider dispatch must revalidate live owner authority after Invocation admission")
    session_table_columns = set(re.findall(r"^\s*(\w+)\s+", session_table, re.M))
    if not {"conversation_turn_id", "task_spec_revision"} <= session_table_columns or "uq_active_conversation_turn_session" not in sql:
        fail("sqlite-v1.sql: AgentSession must pin its ConversationTurn or TaskSpec revision and prevent concurrent sessions for one turn")
    if "runtime_incarnation_local_observations" not in sql:
        fail("sqlite-v1.sql: OS boot identifiers must be separated into local-only RuntimeIncarnation observations")

    api_path = DOCS / "schemas" / "operator-api.openapi.yaml"
    api = yaml.load(api_path.read_text(encoding="utf-8"), Loader=UniqueKeyLoader)
    host_view = api["components"]["schemas"].get("CapabilityHostInstanceView", {})
    properties = host_view.get("properties", {})
    if "provider_instance_ref" in properties or "provider_handle_ref" in properties:
        fail("CapabilityHostInstanceView must keep LitePSM/provider handles Runtime-private")
    if properties.get("active_activation_count", {}).get("minimum") != 0:
        fail("CapabilityHostInstanceView active_activation_count must be a nonnegative derived count")


def check_capability_ref_contract() -> None:
    schema_text = (DOCS / "SCHEMAS.md").read_text(encoding="utf-8")
    if "identity_kind: PACKAGE_COMPONENT | MCP_SKILL" not in schema_text:
        fail("SCHEMAS.md: CapabilityRef must distinguish package components from MCP Skills")
    if "MCP Skills require" not in schema_text or "exact advertised `SKILL.md` URI" not in schema_text:
        fail("SCHEMAS.md: MCP Skill CapabilityRef must pin origin, URI, and manifest digest")

    api_path = DOCS / "schemas" / "operator-api.openapi.yaml"
    events = load_json(DOCS / "schemas" / "domain-event.schema.json")
    try:
        api = yaml.load(api_path.read_text(encoding="utf-8"), Loader=UniqueKeyLoader)
        api_ref = api["components"]["schemas"]["CapabilityRef"]
        api_branches = api_ref.get("oneOf", [])
        event_ref = events["$defs"]["capability_ref"]
        event_branches = event_ref.get("oneOf", [])
        for name, branches in (("OpenAPI", api_branches), ("DomainEvent", event_branches)):
            by_kind = {
                branch.get("properties", {}).get("identity_kind", {}).get("const"): branch
                for branch in branches
            }
            package = by_kind.get("PACKAGE_COMPONENT", {})
            skill = by_kind.get("MCP_SKILL", {})
            if set(by_kind) != {"PACKAGE_COMPONENT", "MCP_SKILL"}:
                fail(f"{name}: CapabilityRef must define PACKAGE_COMPONENT and MCP_SKILL variants")
            if "package_version" not in package.get("required", []):
                fail(f"{name}: PACKAGE_COMPONENT CapabilityRef must require package_version")
            if "package_version" in skill.get("required", []) or "package_version" in skill.get("properties", {}):
                fail(f"{name}: MCP_SKILL CapabilityRef must not invent package_version")
            if "component" not in skill.get("required", []):
                fail(f"{name}: MCP_SKILL CapabilityRef must require exact component URI")
        ref_validator = jsonschema.Draft202012Validator(event_ref, format_checker=jsonschema.FormatChecker())
        valid_package = {
            "capability_id": "cap-package",
            "identity_kind": "PACKAGE_COMPONENT",
            "source": "litepsm-source-1",
            "package_version": "1.0.0",
            "digest": "sha256:" + "a" * 64,
        }
        valid_skill = {
            "capability_id": "cap-skill",
            "identity_kind": "MCP_SKILL",
            "source": "mcp-server-key-1",
            "digest": "sha256:" + "b" * 64,
            "component": "https://mcp.example/skills/report/SKILL.md",
        }
        for label, candidate in (("PACKAGE_COMPONENT", valid_package), ("MCP_SKILL", valid_skill)):
            if list(ref_validator.iter_errors(candidate)):
                fail(f"DomainEvent: valid {label} CapabilityRef is rejected")
        invalid_refs = (
            {**valid_package, "package_version": None},
            {key: value for key, value in valid_skill.items() if key != "component"},
            {**valid_skill, "package_version": "1.0.0"},
            {**valid_skill, "component": ""},
        )
        for candidate in invalid_refs:
            if not list(ref_validator.iter_errors(candidate)):
                fail("DomainEvent: CapabilityRef accepts an invalid/mis-scoped identity")
    except Exception as exc:  # noqa: BLE001
        fail(f"CapabilityRef schema comparison failed: {exc}")

    sql = (DOCS / "schemas" / "sqlite-v1.sql").read_text(encoding="utf-8")
    match = re.search(r"CREATE TABLE capability_locks \((.*?)\n\);", sql, re.S)
    if not match:
        fail("sqlite-v1.sql: missing capability_locks table")
    else:
        table = match.group(1)
        if "identity_kind TEXT NOT NULL" not in table or "package_version TEXT," not in table:
            fail("sqlite-v1.sql: capability lock must support a missing MCP Skill package version")
        if "capability_ref_key_digest" not in table or "PRIMARY KEY(task_id, identity_kind, source, capability_id, component)" not in table:
            fail("sqlite-v1.sql: capability lock must uniquely pin normalized CapabilityRef identity")


def check_benchmark_ids() -> None:
    text = (DOCS / "BENCHMARKS.md").read_text(encoding="utf-8")
    identifiers = re.findall(r"^### (B\d{2})\b", text, re.M)
    expected = [f"B{index:02d}" for index in range(1, len(identifiers) + 1)]
    if identifiers != expected:
        fail(f"BENCHMARKS.md: IDs must be unique and sequential; found {identifiers}")


def check_replication_scope_contract() -> None:
    data_model = (DOCS / "DATA-MODEL.md").read_text(encoding="utf-8")
    if "replication_scope_root_ids: WorkspaceRootId[]" not in data_model:
        fail("DATA-MODEL.md: selected-folder replication must use stable WorkspaceRoot IDs")
    if "revision-pinned folder ResourceRefs" in data_model:
        fail("DATA-MODEL.md: folder replication scope must follow roots across new revisions")
    api_path = DOCS / "schemas" / "operator-api.openapi.yaml"
    events = load_json(DOCS / "schemas" / "domain-event.schema.json")
    try:
        api = yaml.load(api_path.read_text(encoding="utf-8"), Loader=UniqueKeyLoader)
        schemas = api["components"]["schemas"]
        workspace = schemas["Workspace"]
        policy = schemas["UpdateWorkspacePolicyRequest"]
        create = schemas["CreateWorkspaceRequest"]
        if "replication_scope_root_ids" not in workspace.get("properties", {}):
            fail("OpenAPI Workspace must expose selected WorkspaceRoot IDs")
        if "replication_scope_root_ids" not in policy.get("properties", {}):
            fail("OpenAPI policy update must accept WorkspaceRoot IDs")
        if "SELECTED_FOLDERS" in create.get("properties", {}).get("replication_policy", {}).get("enum", []):
            fail("OpenAPI CreateWorkspaceRequest must not select roots before they exist")
        payloads = events["$defs"]["payloads"]
        for name in ("workspace_created", "workspace_replication_policy_changed"):
            payload = payloads[name]
            if "replication_scope_root_ids" not in payload.get("required", []):
                fail(f"DomainEvent {name} must carry selected WorkspaceRoot IDs")
            items = payload.get("properties", {}).get("replication_scope_root_ids", {}).get("items", {})
            if items.get("type") != "string":
                fail(f"DomainEvent {name} root IDs must be strings, not revision-pinned ResourceRefs")
    except Exception as exc:  # noqa: BLE001
        fail(f"Replication scope schema comparison failed: {exc}")

    sql = (DOCS / "schemas" / "sqlite-v1.sql").read_text(encoding="utf-8")
    if "CREATE TABLE workspace_replication_roots" not in sql:
        fail("sqlite-v1.sql: selected WorkspaceRoots must be normalized and FK-bound")
    if "replication_scope_refs_json" in sql:
        fail("sqlite-v1.sql: stale ResourceRef-based replication scope column")
    if "FOREIGN KEY(workspace_id, workspace_root_id)" not in sql:
        fail("sqlite-v1.sql: selected root membership must enforce same-Workspace ownership")


def check_backup_contract() -> None:
    model = (DOCS / "DATA-MODEL.md").read_text(encoding="utf-8")
    services = (DOCS / "SERVICES.md").read_text(encoding="utf-8")
    storage = (DOCS / "schemas" / "sqlite-v1.sql").read_text(encoding="utf-8")
    if "WorkspaceBackupManifest {" not in model or "backup_id: BackupId" not in model:
        fail("DATA-MODEL.md: missing canonical immutable WorkspaceBackupManifest")
    if "### BackupService" not in services or "restore_backup(RestoreWorkspaceBackupRequest)" not in services:
        fail("SERVICES.md: missing backup/restore service contract")
    table_match = re.search(
        r"CREATE TABLE workspace_backup_manifests\s*\((.*?)\n\);",
        storage,
        re.S,
    )
    if not table_match:
        fail("sqlite-v1.sql: missing workspace_backup_manifests table")
    else:
        table = table_match.group(1)
        for fragment in (
            "blob_manifest_ref_json TEXT NOT NULL",
            "database_snapshot_ref_json TEXT NOT NULL",
            "manifest_authentication TEXT NOT NULL",
            "verified_at TEXT NOT NULL",
        ):
            if fragment not in table:
                fail(f"sqlite-v1.sql: backup manifest missing required invariant: {fragment}")

    api_path = DOCS / "schemas" / "operator-api.openapi.yaml"
    try:
        api = yaml.load(api_path.read_text(encoding="utf-8"), Loader=UniqueKeyLoader)
        paths = api["paths"]
        required = {
            ("get", "/workspaces/{workspaceId}/backups"),
            ("post", "/workspaces/{workspaceId}/backups"),
            ("get", "/workspaces/{workspaceId}/backups/{backupId}"),
            ("post", "/recovery/backups/{backupId}/restore"),
        }
        present = {
            (method.lower(), route)
            for route, item in paths.items()
            for method in item
        }
        if not required <= present:
            fail(f"operator-api.openapi.yaml: missing backup routes: {sorted(required - present)}")
        schemas = api["components"]["schemas"]
        for name in ("WorkspaceBackupManifest", "RestoreWorkspaceBackupRequest", "RestoreReceipt"):
            if name not in schemas:
                fail(f"operator-api.openapi.yaml: missing {name} schema")
        manifest_required = set(schemas["WorkspaceBackupManifest"].get("required", []))
        if not {"database_snapshot_ref", "blob_manifest_ref", "manifest_authentication", "verified_at"} <= manifest_required:
            fail("operator-api.openapi.yaml: backup manifest omits snapshot/blob refs, authentication, or verified time")
    except Exception as exc:  # noqa: BLE001
        fail(f"Backup schema comparison failed: {exc}")


def check_openapi() -> None:
    path = DOCS / "schemas" / "operator-api.openapi.yaml"
    try:
        api = yaml.load(path.read_text(encoding="utf-8"), Loader=UniqueKeyLoader)
    except Exception as exc:  # noqa: BLE001
        fail(f"operator-api.openapi.yaml: invalid/duplicate-key YAML: {exc}")
        return
    check_digest_encodings(api, "operator-api.openapi.yaml")
    try:
        openapi_spec_validator.validate(api, base_uri=path.resolve().as_uri())
    except Exception as exc:  # noqa: BLE001 - include the schema path and exact failure
        fail(f"operator-api.openapi.yaml: OpenAPI validation failed: {exc}")
    paths = api.get("paths", {})
    components = api.get("components", {})
    api_schemas = components.get("schemas", {})
    endpoint_schema = api_schemas.get("AgentEndpoint", {})
    endpoint_properties = endpoint_schema.get("properties", {})
    if any(field in endpoint_properties for field in ("endpoint_ref", "runtime_id", "observed_at", "expires_at")):
        fail("operator-api.openapi.yaml: AgentEndpoint must not expose Runtime-local locator/readiness fields")
    endpoint_observation = api_schemas.get("AgentProfileObservation", {})
    observation_required = set(endpoint_observation.get("required", []))
    if not {"endpoint_id", "runtime_id", "runtime_incarnation_id", "compatible", "readiness", "observed_at", "offer_expires_at"} <= observation_required:
        fail("operator-api.openapi.yaml: Agent profile availability must be per endpoint and Runtime incarnation")
    secret_ref_schema = api_schemas.get("SecretRef", {})
    provider_ref_description = secret_ref_schema.get("properties", {}).get("provider_ref", {}).get("description", "")
    if "non-secret" not in provider_ref_description or "SecretLease" not in provider_ref_description:
        fail("operator-api.openapi.yaml: SecretRef provider_ref must be documented as a non-secret key requiring a lease")
    for match in re.finditer(r"\$ref:\s*['\"]([^'\"]+)['\"]|\$ref:\s*([^\s,}]+)", path.read_text()):
        ref = next(group for group in match.groups() if group is not None)
        if ref.startswith("#/"):
            try:
                resolve_pointer(api, ref)
            except (KeyError, IndexError, TypeError):
                fail(f"operator-api.openapi.yaml: unresolved local ref {ref}")
        elif ref.startswith("./"):
            ref_path = (path.parent / unquote(ref.split("#", 1)[0])).resolve()
            if not ref_path.exists():
                fail(f"operator-api.openapi.yaml: unresolved file ref {ref}")
    required = {
        ("post", "/conversations/{conversationId}/turns/{turnId}/retry"),
        ("post", "/conversations/{conversationId}/turns/{turnId}/cancel"),
        ("post", "/tasks/{taskId}/pause"),
        ("post", "/tasks/{taskId}/resume"),
        ("post", "/tasks/{taskId}/cancel"),
        ("post", "/tasks/{taskId}/steps/{stepId}/recover"),
        ("get", "/tasks/{taskId}/steps/{stepId}/execution-dependencies"),
        ("get", "/runtimes/{runtimeId}/capability-hosts"),
        ("get", "/resources/uploads/{uploadId}"),
        ("patch", "/workspaces/{workspaceId}/default-agent-binding"),
        ("post", "/workspaces/{workspaceId}/instructions/revisions"),
        ("post", "/workspaces/{workspaceId}/backups"),
        ("post", "/recovery/backups/{backupId}/restore"),
    }
    present = {(method.lower(), route) for route, item in paths.items() for method in item}
    if not required <= present:
        fail(f"operator API missing lifecycle routes: {sorted(required-present)}")
    try:
        plan = components["schemas"]["ExecutionDependencyPlan"]
        plan_required = set(plan.get("required", []))
        if not {"task_id", "step_id", "task_version", "step_version", "task_spec_revision", "plan_revision", "plan_digest", "environment_candidates", "expires_at"} <= plan_required:
            fail("ExecutionDependencyPlan must bind Task/Step/spec/plan versions, candidate set, digest, and expiry")
        digest = plan.get("properties", {}).get("plan_digest", {})
        if digest.get("pattern") != SHA256_PATTERN:
            fail("ExecutionDependencyPlan.plan_digest must use canonical Sha256Digest")
        candidates = plan.get("properties", {}).get("environment_candidates", {}).get("items", {}).get("$ref")
        if candidates != "#/components/schemas/EnvironmentPlacementCandidate":
            fail("ExecutionDependencyPlan must return typed EnvironmentPlacementCandidate values")
        candidate = components["schemas"]["EnvironmentPlacementCandidate"]
        candidate_required = set(candidate.get("required", []))
        if not {"candidate_id", "runtime_id", "runtime_incarnation_id", "environment_id", "environment_version", "eligible", "blockers"} <= candidate_required:
            fail("EnvironmentPlacementCandidate must identify the exact Runtime incarnation/Environment and eligibility")
        recovery = components["schemas"]["RecoverStepRequest"]
        recovery_required = set(recovery.get("required", []))
        if not {"expected_step_version", "recovery_reason"} <= recovery_required:
            fail("RecoverStepRequest must require the Step optimistic-concurrency version")
        override = recovery.get("properties", {}).get("placement_override", {})
        override_branches = override.get("oneOf", [])
        override_object = next((branch for branch in override_branches if branch.get("type") == "object"), {})
        override_required = set(override_object.get("required", []))
        if not {"candidate_id", "plan_digest"} <= override_required:
            fail("placement_override must atomically pair candidate_id and plan_digest")
        override_digest = override_object.get("properties", {}).get("plan_digest", {})
        if override_digest.get("pattern") != SHA256_PATTERN:
            fail("placement_override.plan_digest must use canonical Sha256Digest")
        preview_response = paths["/tasks/{taskId}/steps/{stepId}/execution-dependencies"]["get"].get("responses", {}).get("200", {}).get("content", {}).get("application/json", {}).get("schema", {}).get("$ref")
        if preview_response != "#/components/schemas/ExecutionDependencyPlan":
            fail("execution-dependencies route must return ExecutionDependencyPlan")
        host_route = paths["/runtimes/{runtimeId}/capability-hosts"]["get"]
        host_response = host_route.get("responses", {}).get("200", {}).get("content", {}).get("application/json", {}).get("schema", {}).get("$ref")
        if host_response != "#/components/schemas/CapabilityHostInstancePage":
            fail("capability-hosts route must return CapabilityHostInstancePage")
        host_view = components["schemas"]["CapabilityHostInstanceView"]
        host_required = set(host_view.get("required", []))
        if not {"host_instance_id", "runtime_incarnation_id", "capability_ref", "state", "health", "active_activation_count", "observed_at", "expires_at"} <= host_required:
            fail("CapabilityHostInstanceView must expose fresh normalized state and derived LiteCowork use count")
        if "provider_instance_ref" in host_view.get("properties", {}):
            fail("CapabilityHostInstanceView must not expose Runtime-private LitePSM references")
    except (KeyError, TypeError) as exc:
        fail(f"Operator API placement/recovery contract comparison failed: {exc}")
    if any(route.endswith("/retry") and "/tasks/" in route for route in paths):
        fail("operator API must not expose terminal Task retry")
    if not components:
        fail("operator-api.openapi.yaml: missing components")


def check_storage() -> None:
    path = DOCS / "schemas" / "sqlite-v1.sql"
    storage_sql = path.read_text(encoding="utf-8")
    for line_number, line in enumerate(storage_sql.splitlines(), start=1):
        if re.match(r"\s*(?:[A-Za-z_][A-Za-z0-9_]*digest|digest|sha256)\s+TEXT\b", line, re.I) and "CHECK" not in line:
            fail(f"sqlite-v1.sql:{line_number}: digest scalar lacks a SHA-256 shape constraint")
    verification_table = re.search(
        r"CREATE TABLE verification_runs \((.*?)\n\);", storage_sql, re.S
    )
    if not verification_table or "inputs_json TEXT NOT NULL" not in verification_table.group(1):
        fail("sqlite-v1.sql: VerificationRun must persist paired ResourceInput values")
    elif "input_refs_json" in verification_table.group(1) or "input_digests_json" in verification_table.group(1):
        fail("sqlite-v1.sql: VerificationRun must not persist parallel input ref/digest arrays")
    try:
        db = sqlite3.connect(":memory:")
        db.executescript(storage_sql)
        db.execute("PRAGMA foreign_keys = ON")
        problems = db.execute("PRAGMA foreign_key_check").fetchall()
        if problems:
            fail(f"sqlite-v1.sql: foreign_key_check failed: {problems}")
        tables = {
            row[0]
            for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")
        }
        storage_doc = (DOCS / "STORAGE.md").read_text(encoding="utf-8")
        inventory = re.search(r"Minimum tables:\s*```(?:text)?\s*(.*?)```", storage_doc, re.S)
        if not inventory:
            fail("STORAGE.md: missing minimum table inventory")
        else:
            required_tables = set(re.findall(r"^[a-z][a-z0-9_]+$", inventory.group(1), re.M))
            missing = required_tables - tables
            if missing:
                fail(f"SQLite schema missing STORAGE.md tables: {sorted(missing)}")
        db.close()
    except Exception as exc:  # noqa: BLE001
        fail(f"sqlite-v1.sql: DDL failed: {exc}")

    check_storage_constraints(path)


def check_attempt_lease_contract() -> None:
    """Keep Attempt, AgentSession, Environment and lease ownership relationally aligned."""
    model = (DOCS / "DATA-MODEL.md").read_text(encoding="utf-8")
    storage = (DOCS / "STORAGE.md").read_text(encoding="utf-8")
    mesh = (DOCS / "RUNTIME-MESH.md").read_text(encoding="utf-8")
    security = (DOCS / "NETWORK-SECURITY.md").read_text(encoding="utf-8")
    recovery = (DOCS / "FAILURE-RECOVERY.md").read_text(encoding="utf-8")
    sql = (DOCS / "schemas" / "sqlite-v1.sql").read_text(encoding="utf-8")
    attempts = re.search(r"CREATE TABLE attempts \((.*?)\n\);", sql, re.S)
    sessions = re.search(r"CREATE TABLE agent_sessions \((.*?)\n\);", sql, re.S)
    leases = re.search(r"CREATE TABLE execution_leases \((.*?)\n\);", sql, re.S)
    controls = re.search(r"CREATE TABLE environment_control_leases \((.*?)\n\);", sql, re.S)
    if not all((attempts, sessions, leases, controls)):
        fail("sqlite-v1.sql: Attempt, AgentSession and both lease tables are required")
        return
    attempt_sql = attempts.group(1)
    session_sql = sessions.group(1)
    lease_sql = leases.group(1)
    control_sql = controls.group(1)
    required_attempt_constraints = (
        "FOREIGN KEY(task_id, parent_attempt_id) REFERENCES attempts(task_id, attempt_id)",
        "FOREIGN KEY(task_id, step_id) REFERENCES steps(task_id, step_id)",
        "FOREIGN KEY(task_id, attempt_id, agent_session_id, runtime_id, runtime_incarnation_id, agent_binding_id) REFERENCES agent_sessions(task_id, attempt_id, agent_session_id, runtime_id, runtime_incarnation_id, agent_binding_id)",
        "FOREIGN KEY(environment_id, runtime_id) REFERENCES environments(environment_id, runtime_id)",
        "UNIQUE(task_id, attempt_id, environment_id, runtime_id, runtime_incarnation_id)",
    )
    for constraint in required_attempt_constraints:
        if constraint not in attempt_sql:
            fail(f"sqlite-v1.sql attempts: missing ownership constraint {constraint}")
    if "UNIQUE(task_id, attempt_id, agent_session_id, runtime_id, runtime_incarnation_id, agent_binding_id)" not in session_sql:
        fail("sqlite-v1.sql agent_sessions: missing selected-AgentBinding/incarnation composite key")
    if "attempt_environment_owner_guard" not in sql or "ATTEMPT_ENVIRONMENT_SCOPE_MISMATCH" not in sql:
        fail("sqlite-v1.sql: Attempt admission must bind Environment and AgentBinding to the Task Workspace")
    if "attempt_execution_identity_immutable" not in sql:
        fail("sqlite-v1.sql: Attempt execution identity must be immutable after admission")
    for trigger in (
        "execution_lease_current_incarnation_guard",
        "execution_lease_current_incarnation_update_guard",
        "execution_lease_identity_immutable",
        "execution_lease_no_reactivation",
        "environment_control_lease_current_incarnation_guard",
        "environment_control_lease_current_incarnation_update_guard",
        "environment_control_lease_identity_immutable",
        "environment_control_lease_owner_epoch_guard",
        "environment_control_lease_no_reactivation",
    ):
        if trigger not in sql:
            fail(f"sqlite-v1.sql: missing lease integrity trigger {trigger}")
    if "FOREIGN KEY(task_id, step_id, attempt_id, runtime_id, runtime_incarnation_id) REFERENCES attempts(task_id, step_id, attempt_id, runtime_id, runtime_incarnation_id)" not in lease_sql:
        fail("sqlite-v1.sql execution_leases: lease must reference the exact Step/Attempt/Runtime incarnation")
    if "FOREIGN KEY(task_id, attempt_id, environment_id, runtime_id, runtime_incarnation_id) REFERENCES attempts(task_id, attempt_id, environment_id, runtime_id, runtime_incarnation_id)" not in control_sql:
        fail("sqlite-v1.sql environment_control_leases: control lease must reference the Attempt's exact Environment")
    if "fencing_token_digest TEXT NOT NULL" not in lease_sql or "fencing_token_digest TEXT NOT NULL" not in control_sql:
        fail("sqlite-v1.sql: durable execution/control leases must retain the fencing credential digest")
    if re.search(r"\bfencing_token\s+TEXT\b", sql, re.I):
        fail("sqlite-v1.sql: raw fencing credentials must never be persisted")

    event_schema = load_json(DOCS / "schemas" / "domain-event.schema.json")
    payloads = event_schema.get("$defs", {}).get("payloads", {})
    expected_event_names = (
        "attempt_created",
        "lease_acquired",
        "lease_renewed",
        "lease_released",
        "lease_expired",
        "environment_control_lease_changed",
    )
    for name in expected_event_names:
        payload = payloads.get(name, {})
        required = set(payload.get("required", []))
        if "runtime_incarnation_id" not in required:
            fail(f"domain-event {name}: Runtime incarnation must be pinned")
        if "fencing_token" in payload.get("properties", {}) or "fencing_token_digest" in payload.get("properties", {}):
            fail(f"domain-event {name}: fencing credentials/digests must not be event payload fields")
    if "raw execution/control fencing credentials are runtime-private" not in " ".join((DOCS / "EVENTS.md").read_text(encoding="utf-8").split()).lower():
        fail("EVENTS.md: raw fencing credentials must be excluded from events and state blobs")
    for document, phrase in (
        (storage, "raw FencingCredentials are process-private"),
        (security, "raw value is excluded from event"),
        (recovery, "fresh Attempt with a new AgentSession"),
        (model, "fencing_token_digest: Sha256Digest"),
        (mesh, "ExecutionLeaseGrant"),
    ):
        if phrase.lower() not in " ".join(document.split()).lower():
            fail(f"Architecture docs: missing Attempt/fencing contract phrase {phrase!r}")

    api = yaml.load(
        (DOCS / "schemas" / "operator-api.openapi.yaml").read_text(encoding="utf-8"),
        Loader=UniqueKeyLoader,
    )
    schemas = api.get("components", {}).get("schemas", {})
    attempt_api = schemas.get("Attempt", {})
    if "runtime_incarnation_id" not in attempt_api.get("required", []):
        fail("OpenAPI Attempt: runtime_incarnation_id must be required")
    control_view = schemas.get("EnvironmentControlLeaseView", {})
    control_props = control_view.get("properties", {})
    if "fencing_token" in control_props or "fencing_token_digest" in control_props:
        fail("OpenAPI EnvironmentControlLeaseView must not expose fencing credential or digest")

    # Exercise the most important composite ownership rule: a control lease cannot
    # silently bind a same-Runtime but different Environment than its Attempt.
    db = sqlite3.connect(":memory:")
    try:
        db.executescript(sql)
        db.execute("PRAGMA foreign_keys = OFF")
        now = "2026-01-01T00:00:00Z"
        db.execute(
            "INSERT INTO workspaces(workspace_id,name,owner_principal_id,replication_policy,status,created_at,updated_at) VALUES(?,?,?,?,?,?,?)",
            ("ws", "Test", "principal", "LOCAL_ONLY", "ACTIVE", now, now),
        )
        db.execute(
            "INSERT INTO runtimes(runtime_id,workspace_id,device_identity_json,runtime_version,platform,architecture,roles_json,trust_zone,availability,startup_policy,current_incarnation_id,last_seen) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            ("rt", "ws", "{}", "1", "linux", "x64", "[]", "personal", "ONLINE", "LOGIN_BACKGROUND", "inc", now),
        )
        db.execute(
            "INSERT INTO runtime_incarnations(runtime_incarnation_id,runtime_id,process_started_at,litecowork_version,recovered_from_unclean_shutdown,recovery_state) VALUES(?,?,?,?,?,?)",
            ("inc", "rt", now, "1", 0, "READY"),
        )
        db.execute(
            "INSERT INTO agent_profiles(agent_profile_id,provider_key,display_name,discovered_at) VALUES(?,?,?,?)",
            ("profile", "fixture", "Fixture", now),
        )
        for binding in ("binding", "other-binding"):
            db.execute(
                "INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,endpoint_selection_policy_json,enabled,created_at) VALUES(?,?,?,?,?,?)",
                (binding, "ws", "profile", "{}", 1, now),
            )
        db.execute(
            "INSERT INTO tasks(task_id,workspace_id,current_spec_revision,status,lead_agent_binding_id,priority,created_by_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)",
            ("task", "ws", 1, "READY", "binding", "NORMAL", "{}", now, now),
        )
        for environment_id, sharing_scope, owner_attempt_id in (
            ("env-attempt", "ATTEMPT_PRIVATE", "attempt"),
            ("env-other", "TASK_SHARED", None),
        ):
            db.execute(
                "INSERT INTO environments(environment_id,runtime_id,provider_kind,class,lifetime,owner_workspace_id,owner_task_id,owner_attempt_id,sharing_scope,name,status,health,budget_enforcement_policy,budget_enforcement,resource_limits_json,network_policy_json,budget_ceiling_json,backup_policy,isolation_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                (environment_id, "rt", "fixture", "CONTAINER", "ATTEMPT", "ws", "task", owner_attempt_id, sharing_scope, environment_id, "READY", "HEALTHY", "ALLOW_HOST_MONITORED", "HOST_MONITORED", "{}", "{}", "{}", "EXCLUDED", "{}", now, now),
            )
        db.execute(
            "INSERT INTO attempts(attempt_id,task_id,step_id,agent_binding_id,runtime_id,runtime_incarnation_id,environment_id,failover_class,status,created_at) VALUES(?,?,?,?,?,?,?,?,?,?)",
            ("attempt", "task", "step", "binding", "rt", "inc", "env-attempt", "REPLAYABLE", "CREATED", now),
        )
        db.commit()
        db.execute("PRAGMA foreign_keys = ON")
        try:
            db.execute(
                "INSERT INTO environment_control_leases(control_lease_id,environment_id,task_id,attempt_id,runtime_id,runtime_incarnation_id,owner_kind,owner_ref_json,epoch,issuer_key_version,fencing_token_digest,state,acquired_at,expires_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                ("bad-control", "env-other", "task", "attempt", "rt", "inc", "AGENT", "{}", 1, 1, "sha256:" + "a" * 64, "ACTIVE", now, now),
            )
            db.commit()
            fail("sqlite-v1.sql: admitted an EnvironmentControlLease for a different Environment on the Attempt's Runtime")
        except sqlite3.IntegrityError:
            pass
        db.rollback()
        db.execute("PRAGMA foreign_keys = OFF")
        db.execute(
            "INSERT INTO agent_sessions(agent_session_id,workspace_id,scope_kind,task_id,task_spec_revision,attempt_id,agent_binding_id,endpoint_id,runtime_id,runtime_incarnation_id,status,started_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            ("wrong-session", "ws", "ATTEMPT_EXECUTION", "task", 1, "attempt", "other-binding", "endpoint", "rt", "inc", "ACTIVE", now),
        )
        db.commit()
        db.execute("PRAGMA foreign_keys = ON")
        try:
            db.execute("UPDATE attempts SET agent_session_id='wrong-session' WHERE attempt_id='attempt'")
            db.commit()
            fail("sqlite-v1.sql: admitted an Attempt AgentSession with a different AgentBinding")
        except sqlite3.IntegrityError:
            pass
        db.rollback()
    except Exception as exc:  # noqa: BLE001 - keep the architecture check diagnostic
        fail(f"sqlite-v1.sql: Attempt/lease ownership fixture failed: {exc}")
    finally:
        db.close()


def check_storage_constraints(path: Path) -> None:
    sql = path.read_text(encoding="utf-8")
    now = "2026-01-01T00:00:00Z"

    environment_table = re.search(r"CREATE TABLE environments \((.*?)\n\);", sql, re.S)
    checkpoint_table = re.search(r"CREATE TABLE environment_checkpoints \((.*?)\n\);", sql, re.S)
    environment_binding = re.search(r"CREATE TABLE environment_provider_bindings \((.*?)\n\);", sql, re.S)
    checkpoint_binding = re.search(r"CREATE TABLE environment_checkpoint_provider_bindings \((.*?)\n\);", sql, re.S)
    location_table = re.search(r"CREATE TABLE resource_locations \((.*?)\n\);", sql, re.S)
    location_binding = re.search(r"CREATE TABLE resource_location_bindings \((.*?)\n\);", sql, re.S)
    file_identity_binding = re.search(r"CREATE TABLE file_identity_bindings \((.*?)\n\);", sql, re.S)
    agent_endpoint_table = re.search(r"CREATE TABLE agent_endpoints \((.*?)\n\);", sql, re.S)
    agent_endpoint_binding = re.search(r"CREATE TABLE agent_endpoint_bindings \((.*?)\n\);", sql, re.S)
    if not all((environment_table, checkpoint_table, environment_binding, checkpoint_binding, location_table, location_binding, file_identity_binding, agent_endpoint_table, agent_endpoint_binding)):
        fail("sqlite-v1.sql: Environment, checkpoint, ResourceLocation, FileIdentity, and AgentEndpoint local-binding tables are required")
    else:
        if "locator_json" in environment_table.group(1) or "provider_ref" in checkpoint_table.group(1):
            fail("sqlite-v1.sql: durable Environment/checkpoint rows must not contain provider handles")
        if "opaque_locator_ref" in location_table.group(1) or "locator_ref_id" not in location_table.group(1):
            fail("sqlite-v1.sql: durable ResourceLocation must store only a non-secret locator key")
        if "private_locator" not in location_binding.group(1):
            fail("sqlite-v1.sql: raw ResourceLocation locators must be held in a Runtime-local binding")
        if "raw_file_id" not in file_identity_binding.group(1) or "runtime_incarnation_id" not in file_identity_binding.group(1):
            fail("sqlite-v1.sql: raw FileIdentity tuples must be Runtime/incarnation-local")
        if any(field in agent_endpoint_table.group(1) for field in ("endpoint_ref", "runtime_id", "observed_at", "expires_at")):
            fail("sqlite-v1.sql: stable AgentEndpoint identity must not store Runtime-local locators or readiness")
        if "endpoint_ref" not in agent_endpoint_binding.group(1) or "runtime_incarnation_id" not in agent_endpoint_binding.group(1):
            fail("sqlite-v1.sql: AgentEndpoint locators must be stored in a local incarnation binding")
        if not {"opaque_locator_ref", "runtime_incarnation_id"} <= set(re.findall(r"^\s*(\w+)\s+", environment_binding.group(1), re.M)):
            fail("sqlite-v1.sql: EnvironmentProviderBinding must carry a local locator and Runtime incarnation")
        if not {"provider_ref", "runtime_incarnation_id"} <= set(re.findall(r"^\s*(\w+)\s+", checkpoint_binding.group(1), re.M)):
            fail("sqlite-v1.sql: checkpoint provider handles must be incarnation-local")
        if "portable_snapshot_ref_json" not in checkpoint_table.group(1):
            fail("sqlite-v1.sql: portable checkpoint bytes need an explicit content-addressed BlobRef")
        if any(name not in sql for name in (
            "environment_provider_binding_matches_current_incarnation",
            "environment_checkpoint_binding_matches_current_incarnation",
            "resource_location_binding_matches_current_incarnation",
            "file_identity_binding_matches_current_incarnation",
            "agent_endpoint_binding_matches_current_incarnation",
            "agent_endpoint_binding_identity_immutable",
            "agent_endpoint_binding_delete_requires_drained_host",
        )):
            fail("sqlite-v1.sql: Runtime-local bindings must validate the current Runtime incarnation")

    def base_database():
        connection = sqlite3.connect(":memory:")
        connection.executescript(sql)
        connection.execute("PRAGMA foreign_keys = ON")
        connection.execute(
            "INSERT INTO workspaces(workspace_id,name,owner_principal_id,replication_policy,status,created_at,updated_at) VALUES(?,?,?,?,?,?,?)",
            ("ws", "Test", "principal", "LOCAL_ONLY", "ACTIVE", now, now),
        )
        connection.execute(
            "INSERT INTO resources(resource_id,workspace_id,kind,provider_identity_json,identity_digest,display_name,sensitivity,provenance_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?)",
            ("res", "ws", "ARTIFACT", '{"provider":"artifact-store"}', None, "Report", "PERSONAL", "{}", now, now),
        )
        connection.commit()
        return connection

    def add_revision(connection, revision_id, digest):
        connection.execute(
            "INSERT INTO resource_revisions(resource_revision_id,resource_id,content_digest,observed_at,created_by_json) VALUES(?,?,?,?,?)",
            (revision_id, "res", digest, now, "{}"),
        )

    def add_artifact(connection):
        connection.execute(
            "INSERT INTO artifacts(artifact_id,workspace_id,resource_id,kind,display_name,current_version,library_status,created_at) VALUES(?,?,?,?,?,?,?,?)",
            ("artifact", "ws", "res", "REPORT", "Report", 1, "TRANSIENT", now),
        )

    def add_artifact_version(connection, version, revision_id, input_refs_json="[]"):
        connection.execute(
            "INSERT INTO artifact_versions(artifact_id,resource_id,version,resource_revision_id,input_refs_json,content_kind,content_digest,storage_ref_json,content_media_type,content_size_bytes,provenance_json,verification_refs_json,created_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)",
            ("artifact", "res", version, revision_id, input_refs_json, "MANAGED_BLOB", "sha256:" + str(version) * 64, "{}", "application/octet-stream", 1, "{}", "[]", now),
        )

    digest_checks = base_database()
    try:
        try:
            add_revision(digest_checks, "invalid-digest-revision", "f" * 64)
            fail("sqlite-v1.sql: accepted a ResourceRevision digest without canonical sha256: encoding")
        except sqlite3.IntegrityError:
            pass
        try:
            digest_checks.execute(
                "INSERT INTO resource_upload_sessions(upload_id,workspace_id,display_name,media_type,expected_size_bytes,expected_digest,chunk_size_bytes,state,expires_at,created_at) VALUES(?,?,?,?,?,?,?,?,?,?)",
                ("upload-invalid-digest", "ws", "file.bin", "application/octet-stream", 1, "f" * 64, 1024, "OPEN", now, now),
            )
            fail("sqlite-v1.sql: accepted an upload digest without canonical sha256: encoding")
        except sqlite3.IntegrityError:
            pass
    finally:
        digest_checks.close()

    environment_bindings = sqlite3.connect(":memory:")
    try:
        environment_bindings.executescript(sql)
        environment_bindings.execute("PRAGMA foreign_keys = OFF")
        environment_bindings.execute(
            "INSERT INTO runtimes(runtime_id,workspace_id,device_identity_json,runtime_version,platform,architecture,roles_json,trust_zone,availability,startup_policy,current_incarnation_id,last_seen) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            ("runtime", "ws", "{}", "1", "linux", "x64", "[]", "personal", "ONLINE", "LOGIN_BACKGROUND", "incarnation", now),
        )
        environment_bindings.execute(
            "INSERT INTO runtime_incarnations(runtime_incarnation_id,runtime_id,process_started_at,litecowork_version,recovered_from_unclean_shutdown,recovery_state) VALUES(?,?,?,?,?,?)",
            ("incarnation", "runtime", now, "1", 0, "READY"),
        )
        environment_bindings.execute(
            "INSERT INTO agent_profiles(agent_profile_id,provider_key,display_name,discovered_at) VALUES(?,?,?,?)",
            ("profile", "test-agent", "Test Agent", now),
        )
        environment_bindings.execute(
            "INSERT INTO agent_endpoints(endpoint_id,agent_profile_id,protocol,topology,capabilities_json) VALUES(?,?,?,?,?)",
            ("endpoint", "profile", "ACP", "LOCAL_INTERACTIVE", "{}"),
        )
        environment_bindings.execute(
            "INSERT INTO agent_endpoint_bindings(endpoint_id,runtime_id,runtime_incarnation_id,endpoint_ref,observed_at) VALUES(?,?,?,?,?)",
            ("endpoint", "runtime", "incarnation", "/usr/bin/agent-adapter", now),
        )
        environment_bindings.execute(
            "INSERT INTO agent_host_instances(host_instance_id,runtime_id,runtime_incarnation_id,agent_profile_id,endpoint_id,hosting_mode,state,ownership,started_at,last_used_at) VALUES(?,?,?,?,?,?,?,?,?,?)",
            ("agent-host", "runtime", "incarnation", "profile", "endpoint", "LOCAL_PER_SESSION", "READY", "LITECOWORK", now, now),
        )
        environment_bindings.execute(
            "INSERT INTO environments(environment_id,runtime_id,provider_kind,class,lifetime,owner_workspace_id,owner_task_id,sharing_scope,name,status,health,budget_enforcement_policy,budget_enforcement,resource_limits_json,network_policy_json,budget_ceiling_json,backup_policy,isolation_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            ("environment", "runtime", "container-provider", "CONTAINER", "ATTEMPT", "ws", "task", "TASK_SHARED", "Test", "READY", "HEALTHY", "ALLOW_HOST_MONITORED", "HOST_MONITORED", "{}", "{}", "{}", "INCLUDE_CHECKPOINTS", "{}", now, now),
        )
        environment_bindings.execute(
            "INSERT INTO environment_provider_bindings(environment_id,runtime_id,runtime_incarnation_id,provider_kind,opaque_locator_ref,observed_at) VALUES(?,?,?,?,?,?)",
            ("environment", "runtime", "incarnation", "container-provider", "opaque-local-locator", now),
        )
        environment_bindings.execute(
            "INSERT INTO environment_checkpoints(checkpoint_id,environment_id,digest,portable_snapshot_ref_json,created_at) VALUES(?,?,?,?,?)",
            ("checkpoint", "environment", "sha256:" + "a" * 64, '{"digest":"sha256:' + "a" * 64 + '","size_bytes":42,"media_type":"application/octet-stream"}', now),
        )
        environment_bindings.execute(
            "INSERT INTO environment_checkpoint_provider_bindings(checkpoint_id,runtime_id,runtime_incarnation_id,provider_ref,observed_at) VALUES(?,?,?,?,?)",
            ("checkpoint", "runtime", "incarnation", "opaque-checkpoint-handle", now),
        )
        environment_bindings.execute(
            "INSERT INTO resources(resource_id,workspace_id,kind,provider_identity_json,display_name,sensitivity,provenance_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)",
            ("resource", "ws", "FILE", "{}", "Input", "PERSONAL", "{}", now, now),
        )
        environment_bindings.execute(
            "INSERT INTO resource_locations(location_id,resource_id,runtime_id,locator_ref_id,availability,writable,observed_at) VALUES(?,?,?,?,?,?,?)",
            ("location", "resource", "runtime", "locator-key", "AVAILABLE", 0, now),
        )
        environment_bindings.execute(
            "INSERT INTO resource_location_bindings(location_id,locator_ref_id,runtime_id,runtime_incarnation_id,private_locator,observed_at) VALUES(?,?,?,?,?,?)",
            ("location", "locator-key", "runtime", "incarnation", "/private/root/file.txt", now),
        )
        environment_bindings.execute(
            "INSERT INTO file_identity_bindings(location_id,runtime_id,runtime_incarnation_id,raw_filesystem_instance_id,raw_volume_id,raw_file_id,raw_generation,platform_kind,observed_at) VALUES(?,?,?,?,?,?,?,?,?)",
            ("location", "runtime", "incarnation", "raw-filesystem", "raw-volume", "raw-file-id", "generation-1", "linux", now),
        )
        try:
            environment_bindings.execute(
                "INSERT INTO environment_provider_bindings(environment_id,runtime_id,runtime_incarnation_id,provider_kind,opaque_locator_ref,observed_at) VALUES(?,?,?,?,?,?)",
                ("environment", "runtime", "stale-incarnation", "container-provider", "stale-locator", now),
            )
            fail("sqlite-v1.sql: accepted an Environment binding from a stale Runtime incarnation")
        except sqlite3.IntegrityError as exc:
            if "ENVIRONMENT_PROVIDER_BINDING_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: stale Environment binding raised unexpected error: {exc}")
        try:
            environment_bindings.execute(
                "INSERT INTO environment_checkpoint_provider_bindings(checkpoint_id,runtime_id,runtime_incarnation_id,provider_ref,observed_at) VALUES(?,?,?,?,?)",
                ("checkpoint", "other-runtime", "incarnation", "foreign-checkpoint", now),
            )
            fail("sqlite-v1.sql: accepted a checkpoint handle from a Runtime other than its Environment owner")
        except sqlite3.IntegrityError as exc:
            if "ENVIRONMENT_CHECKPOINT_BINDING_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: foreign checkpoint binding raised unexpected error: {exc}")
        try:
            environment_bindings.execute(
                "INSERT INTO resource_location_bindings(location_id,locator_ref_id,runtime_id,runtime_incarnation_id,private_locator,observed_at) VALUES(?,?,?,?,?,?)",
                ("location", "locator-key", "runtime", "stale-incarnation", "/stale/path", now),
            )
            fail("sqlite-v1.sql: accepted a ResourceLocation binding from a stale Runtime incarnation")
        except sqlite3.IntegrityError as exc:
            if "RESOURCE_LOCATION_BINDING_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: stale ResourceLocation binding raised unexpected error: {exc}")
        try:
            environment_bindings.execute(
                "INSERT INTO file_identity_bindings(location_id,runtime_id,runtime_incarnation_id,raw_filesystem_instance_id,raw_volume_id,raw_file_id,raw_generation,platform_kind,observed_at) VALUES(?,?,?,?,?,?,?,?,?)",
                ("location", "runtime", "stale-incarnation", "raw-filesystem", "raw-volume", "raw-file-id-2", "generation-1", "linux", now),
            )
            fail("sqlite-v1.sql: accepted raw FileIdentity data from a stale Runtime incarnation")
        except sqlite3.IntegrityError as exc:
            if "FILE_IDENTITY_BINDING_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: stale FileIdentity binding raised unexpected error: {exc}")
        try:
            environment_bindings.execute(
                "INSERT INTO agent_endpoint_bindings(endpoint_id,runtime_id,runtime_incarnation_id,endpoint_ref,observed_at) VALUES(?,?,?,?,?)",
                ("endpoint", "runtime", "stale-incarnation", "/stale/adapter", now),
            )
            fail("sqlite-v1.sql: accepted an AgentEndpoint locator from a stale Runtime incarnation")
        except sqlite3.IntegrityError as exc:
            if "AGENT_ENDPOINT_BINDING_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: stale AgentEndpoint binding raised unexpected error: {exc}")
        try:
            environment_bindings.execute(
                "DELETE FROM agent_endpoint_bindings WHERE endpoint_id='endpoint' AND runtime_incarnation_id='incarnation'"
            )
            fail("sqlite-v1.sql: removed an AgentEndpoint binding while its host was still ready")
        except sqlite3.IntegrityError as exc:
            if "AGENT_ENDPOINT_HOST_NOT_DRAINED" not in str(exc):
                fail(f"sqlite-v1.sql: live AgentEndpoint binding deletion raised unexpected error: {exc}")
        environment_bindings.execute("UPDATE agent_host_instances SET state='STOPPED' WHERE host_instance_id='agent-host'")
        environment_bindings.execute(
            "DELETE FROM agent_endpoint_bindings WHERE endpoint_id='endpoint' AND runtime_incarnation_id='incarnation'"
        )
        try:
            environment_bindings.execute(
                "INSERT INTO environment_checkpoints(checkpoint_id,environment_id,digest,portable_snapshot_ref_json,created_at) VALUES(?,?,?,?,?)",
                ("bad-checkpoint", "environment", "sha256:" + "b" * 64, '{"digest":"sha256:' + "a" * 64 + '","size_bytes":42,"media_type":"application/octet-stream"}', now),
            )
            fail("sqlite-v1.sql: accepted a portable checkpoint BlobRef whose digest differs from the checkpoint digest")
        except sqlite3.IntegrityError:
            pass
        try:
            environment_bindings.execute(
                "UPDATE environment_checkpoint_provider_bindings SET provider_ref='rewritten' WHERE checkpoint_id='checkpoint'"
            )
            fail("sqlite-v1.sql: accepted mutation of a checkpoint provider handle")
        except sqlite3.IntegrityError as exc:
            if "IMMUTABLE_ENVIRONMENT_CHECKPOINT_BINDING" not in str(exc):
                fail(f"sqlite-v1.sql: checkpoint binding mutation raised unexpected error: {exc}")
    except Exception as exc:  # noqa: BLE001
        fail(f"sqlite-v1.sql: Environment provider binding fixture failed: {exc}")
    finally:
        environment_bindings.close()

    invalid = base_database()
    try:
        invalid.execute("BEGIN")
        add_revision(invalid, "rev-1", "sha256:" + "1" * 64)
        add_revision(invalid, "rev-2", "sha256:" + "2" * 64)
        invalid.execute("UPDATE resources SET current_revision_id=? WHERE resource_id='res'", ("rev-1",))
        add_artifact(invalid)
        try:
            add_artifact_version(invalid, 1, "rev-2")
            fail("sqlite-v1.sql: accepted an initial ArtifactVersion that disagrees with its Resource head")
        except sqlite3.IntegrityError as exc:
            if "ARTIFACT_RESOURCE_REVISION_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: initial Artifact mismatch raised unexpected error: {exc}")
    except Exception as exc:  # noqa: BLE001
        fail(f"sqlite-v1.sql: initial Artifact constraint fixture failed: {exc}")
    finally:
        invalid.rollback()
        invalid.close()

    valid = base_database()
    try:
        valid.execute("BEGIN")
        add_revision(valid, "rev-1", "sha256:" + "1" * 64)
        valid.execute("UPDATE resources SET current_revision_id=? WHERE resource_id='res'", ("rev-1",))
        add_artifact(valid)
        add_artifact_version(valid, 1, "rev-1")
        valid.commit()

        valid.execute("BEGIN")
        add_revision(valid, "rev-2", "sha256:" + "2" * 64)
        valid.execute("UPDATE resources SET current_revision_id=? WHERE resource_id='res'", ("rev-2",))
        add_artifact_version(valid, 2, "rev-2")
        valid.execute("UPDATE artifacts SET current_version=2, version=2 WHERE artifact_id='artifact'")
        valid.commit()

        valid.execute("BEGIN")
        add_revision(valid, "rev-3", "sha256:" + "3" * 64)
        add_artifact_version(valid, 3, "rev-3")
        try:
            valid.execute("UPDATE artifacts SET current_version=3, version=3 WHERE artifact_id='artifact'")
            fail("sqlite-v1.sql: accepted an Artifact current-version change that disagrees with its Resource head")
        except sqlite3.IntegrityError as exc:
            if "ARTIFACT_RESOURCE_REVISION_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: Artifact pointer mismatch raised unexpected error: {exc}")
    except Exception as exc:  # noqa: BLE001
        fail(f"sqlite-v1.sql: Artifact version constraint fixture failed: {exc}")
    finally:
        valid.rollback()
        valid.close()

    dependencies = base_database()
    try:
        dependencies.execute("BEGIN")
        dependencies.execute(
            "INSERT INTO resources(resource_id,workspace_id,kind,provider_identity_json,display_name,sensitivity,provenance_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)",
            ("source", "ws", "FILE", '{"provider":"local"}', "Input", "PERSONAL", "{}", now, now),
        )
        add_revision(dependencies, "output-r1", "sha256:" + "1" * 64)
        dependencies.execute("UPDATE resources SET current_revision_id=? WHERE resource_id='res'", ("output-r1",))
        dependencies.execute(
            "INSERT INTO resource_revisions(resource_revision_id,resource_id,content_digest,observed_at,created_by_json) VALUES(?,?,?,?,?)",
            ("source-r1", "source", "sha256:" + "2" * 64, now, "{}"),
        )
        dependencies.execute(
            "INSERT INTO resource_revisions(resource_revision_id,resource_id,content_digest,observed_at,created_by_json) VALUES(?,?,?,?,?)",
            ("source-r2", "source", "sha256:" + "3" * 64, now, "{}"),
        )
        add_artifact(dependencies)
        input_refs = json.dumps([{"workspace_id": "ws", "resource_id": "source", "revision_id": "source-r1"}])
        add_artifact_version(dependencies, 1, "output-r1", input_refs)
        dependencies.execute(
            "INSERT INTO dependency_edges(dependency_edge_id,workspace_id,source_resource_id,source_revision_id,dependent_kind,dependent_ref,artifact_id,artifact_version,created_at) VALUES(?,?,?,?,?,?,?,?,?)",
            ("dep-1", "ws", "source", "source-r1", "ARTIFACT_VERSION", "artifact://ws/artifact@v1", "artifact", 1, now),
        )
        dependencies.execute(
            "INSERT INTO invalidation_records(invalidation_record_id,dependency_edge_id,observed_revision_id,reason_code,created_at) VALUES(?,?,?,?,?)",
            ("inv-1", "dep-1", "source-r2", "SOURCE_REVISION_CHANGED", now),
        )
        try:
            dependencies.execute(
                "INSERT INTO invalidation_records(invalidation_record_id,dependency_edge_id,observed_revision_id,reason_code,created_at) VALUES(?,?,?,?,?)",
                ("inv-duplicate", "dep-1", "source-r2", "SOURCE_REVISION_CHANGED", now),
            )
            fail("sqlite-v1.sql: accepted duplicate invalidation for a DependencyEdge and observed revision")
        except sqlite3.IntegrityError:
            pass
        try:
            dependencies.execute(
                "INSERT INTO invalidation_records(invalidation_record_id,dependency_edge_id,observed_revision_id,reason_code,created_at) VALUES(?,?,?,?,?)",
                ("inv-wrong-source", "dep-1", "output-r1", "SOURCE_REVISION_CHANGED", now),
            )
            fail("sqlite-v1.sql: accepted invalidation from a different logical Resource")
        except sqlite3.IntegrityError as exc:
            if "INVALID_INVALIDATION_SOURCE" not in str(exc):
                fail(f"sqlite-v1.sql: wrong-source invalidation raised unexpected error: {exc}")
    except Exception as exc:  # noqa: BLE001
        fail(f"sqlite-v1.sql: dependency/invalidation constraint fixture failed: {exc}")
    finally:
        dependencies.rollback()
        dependencies.close()

    refs = sqlite3.connect(":memory:")
    try:
        refs.executescript(sql)
        refs.execute("PRAGMA foreign_keys = OFF")
        refs.execute(
            "INSERT INTO capability_locks(task_id,identity_kind,source,capability_id,package_version,digest,component,capability_ref_key_digest,locked_at) VALUES(?,?,?,?,?,?,?,?,?)",
            ("task", "MCP_SKILL", "server", "skill-id", None, "sha256:" + "a" * 64, "https://mcp.example/skill/SKILL.md", "sha256:" + "b" * 64, now),
        )
        try:
            refs.execute(
                "INSERT INTO capability_locks(task_id,identity_kind,source,capability_id,package_version,digest,component,capability_ref_key_digest,locked_at) VALUES(?,?,?,?,?,?,?,?,?)",
                ("task", "PACKAGE_COMPONENT", "package", "package-id", None, "sha256:" + "c" * 64, "", "sha256:" + "d" * 64, now),
            )
            fail("sqlite-v1.sql: accepted package CapabilityRef without package_version")
        except sqlite3.IntegrityError:
            pass
    except Exception as exc:  # noqa: BLE001
        fail(f"sqlite-v1.sql: CapabilityRef constraint fixture failed: {exc}")
    finally:
        refs.close()

    invocation_db = sqlite3.connect(":memory:")
    try:
        invocation_db.executescript(sql)
        invocation_db.execute("PRAGMA foreign_keys = OFF")
        invocation_db.execute(
            "INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,endpoint_selection_policy_json,enabled,created_at) VALUES(?,?,?,?,?,?)",
            ("binding", "ws", "profile", "{}", 1, now),
        )
        invocation_db.execute(
            "INSERT INTO runtimes(runtime_id,workspace_id,device_identity_json,runtime_version,platform,architecture,roles_json,trust_zone,availability,startup_policy,current_incarnation_id,last_seen) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            ("runtime", "ws", "{}", "1", "linux", "x64", "[]", "personal", "ONLINE", "LOGIN_BACKGROUND", "incarnation", now),
        )
        invocation_db.execute(
            "INSERT INTO agent_host_instances(host_instance_id,runtime_id,runtime_incarnation_id,agent_profile_id,endpoint_id,hosting_mode,state,ownership,started_at,last_used_at) VALUES(?,?,?,?,?,?,?,?,?,?)",
            ("agent-host", "runtime", "incarnation", "profile", "endpoint", "LOCAL_PER_SESSION", "READY", "LITECOWORK", now, now),
        )
        invocation_db.execute(
            "INSERT INTO agent_sessions(agent_session_id,workspace_id,scope_kind,conversation_id,conversation_turn_id,agent_binding_id,endpoint_id,runtime_id,runtime_incarnation_id,status,started_at) VALUES(?,?,?,?,?,?,?,?,?,?,?)",
            ("session", "ws", "CONVERSATION", "conversation", "turn", "binding", "endpoint", "runtime", "incarnation", "ACTIVE", now),
        )
        invocation_db.execute(
            "INSERT INTO conversation_turns(turn_id,conversation_id,user_message_id,agent_session_id,status,created_at) VALUES(?,?,?,?,?,?)",
            ("turn", "conversation", "message", "session", "RUNNING", now),
        )
        try:
            invocation_db.execute(
                "INSERT INTO conversation_turns(turn_id,conversation_id,user_message_id,agent_session_id,status,created_at) VALUES(?,?,?,?,?,?)",
                ("wrong-session-turn", "conversation", "message", "session", "RUNNING", now),
            )
            fail("sqlite-v1.sql: bound one Conversation AgentSession to a different turn")
        except sqlite3.IntegrityError as exc:
            if "CONVERSATION_TURN_AGENT_SESSION_SCOPE_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: wrong-turn session binding raised unexpected error: {exc}")
        invocation_db.execute(
            "INSERT INTO agent_session_host_bindings(agent_session_id,host_instance_id,native_session_ref,bound_at) VALUES(?,?,?,?)",
            ("session", "agent-host", "opaque-native-handle", now),
        )
        invocation_db.execute(
            "INSERT INTO agent_sessions(agent_session_id,workspace_id,scope_kind,conversation_id,conversation_turn_id,agent_binding_id,endpoint_id,runtime_id,runtime_incarnation_id,status,started_at) VALUES(?,?,?,?,?,?,?,?,?,?,?)",
            ("mismatched-session", "ws", "CONVERSATION", "conversation", "other-turn", "binding", "endpoint", "other-runtime", "other-incarnation", "ACTIVE", now),
        )
        invocation_db.execute(
            "INSERT INTO conversation_turns(turn_id,conversation_id,user_message_id,agent_session_id,status,created_at) VALUES(?,?,?,?,?,?)",
            ("other-turn", "conversation", "message", "mismatched-session", "WAITING_USER", now),
        )
        try:
            invocation_db.execute(
                "INSERT INTO agent_session_host_bindings(agent_session_id,host_instance_id,native_session_ref,bound_at) VALUES(?,?,?,?)",
                ("mismatched-session", "agent-host", "wrong-native-handle", now),
            )
            fail("sqlite-v1.sql: accepted an AgentSession host binding from another Runtime incarnation")
        except sqlite3.IntegrityError as exc:
            if "AGENT_SESSION_HOST_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: AgentSession host mismatch raised unexpected error: {exc}")
        try:
            invocation_db.execute(
                "DELETE FROM agent_session_host_bindings WHERE agent_session_id='session'"
            )
            fail("sqlite-v1.sql: released an AgentHost reference before AgentSession settlement")
        except sqlite3.IntegrityError as exc:
            if "AGENT_SESSION_HOST_STILL_ACTIVE" not in str(exc):
                fail(f"sqlite-v1.sql: active AgentSession binding release raised unexpected error: {exc}")
        invocation_db.execute(
            "INSERT INTO agent_sessions(agent_session_id,workspace_id,scope_kind,conversation_id,conversation_turn_id,agent_binding_id,endpoint_id,runtime_id,runtime_incarnation_id,status,started_at) VALUES(?,?,?,?,?,?,?,?,?,?,?)",
            ("settled-session", "ws", "CONVERSATION", "conversation", "settled-turn", "binding", "endpoint", "runtime", "incarnation", "ACTIVE", now),
        )
        invocation_db.execute(
            "INSERT INTO agent_session_host_bindings(agent_session_id,host_instance_id,native_session_ref,bound_at) VALUES(?,?,?,?)",
            ("settled-session", "agent-host", "another-native-handle", now),
        )
        session_uses = invocation_db.execute(
            "SELECT COUNT(*) FROM agent_session_host_bindings b JOIN agent_sessions s USING(agent_session_id) WHERE b.host_instance_id=? AND s.status IN ('STARTING','ACTIVE','INTERRUPTING','CLOSING')",
            ("agent-host",),
        ).fetchone()[0]
        if session_uses != 2:
            fail("sqlite-v1.sql: AgentHost session-use count is not derived from live local bindings")
        invocation_db.execute("UPDATE agent_sessions SET status='CLOSED' WHERE agent_session_id='settled-session'")
        invocation_db.execute("DELETE FROM agent_session_host_bindings WHERE agent_session_id='settled-session'")
        remaining_session_uses = invocation_db.execute(
            "SELECT COUNT(*) FROM agent_session_host_bindings b JOIN agent_sessions s USING(agent_session_id) WHERE b.host_instance_id=? AND s.status IN ('STARTING','ACTIVE','INTERRUPTING','CLOSING')",
            ("agent-host",),
        ).fetchone()[0]
        if remaining_session_uses != 1:
            fail("sqlite-v1.sql: settled AgentSession did not release exactly one derived AgentHost use")
        invocation_db.execute(
            "INSERT INTO capability_grants(capability_grant_id,scope_kind,conversation_id,capability_ref_json,allowed_operations_json,resource_scope_json,secret_refs_json,granted_by_json,status,created_at) VALUES(?,?,?,?,?,?,?,?,?,?)",
            ("grant", "CONVERSATION", "conversation", "{}", '["read"]', "{}", "[]", "{}", "ACTIVE", now),
        )
        invocation_db.execute(
            "INSERT INTO capability_activations(activation_id,workspace_id,scope_kind,conversation_id,capability_ref_json,runtime_id,runtime_incarnation_id,mode,status,health,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            ("activation", "ws", "CONVERSATION", "conversation", "{}", "runtime", "incarnation", "NATIVE_AGENT", "ACTIVE", "HEALTHY", now, now),
        )
        host_digests = ("sha256:" + "a" * 64, "sha256:" + "b" * 64, "sha256:" + "c" * 64)
        invocation_db.execute(
            "INSERT INTO capability_host_instances(host_instance_id,runtime_id,runtime_incarnation_id,capability_ref_json,capability_identity_digest,configuration_digest,isolation_partition_digest,sharing_policy,hosting_mode,provider_instance_ref,state,health,observed_at,expires_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            ("provider-host", "runtime", "incarnation", "{}", *host_digests, "TRUST_PARTITION_SHARED", "LOCAL_MANAGED", "opaque-provider-ref", "READY", "HEALTHY", now, "2026-01-02T00:00:00Z"),
        )
        for activation_id in ("host-activation-1", "host-activation-2"):
            invocation_db.execute(
                "INSERT INTO capability_activations(activation_id,workspace_id,scope_kind,conversation_id,capability_ref_json,runtime_id,runtime_incarnation_id,mode,status,health,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
                (activation_id, "ws", "CONVERSATION", "conversation", "{}", "runtime", "incarnation", "GATEWAY_PROXY", "ACTIVE", "HEALTHY", now, now),
            )
            invocation_db.execute(
                "INSERT INTO capability_activation_host_bindings(activation_id,host_instance_id,provider_handle_ref,bound_at) VALUES(?,?,?,?)",
                (activation_id, "provider-host", "opaque-activation-handle", now),
            )
        try:
            invocation_db.execute(
                "DELETE FROM capability_activation_host_bindings WHERE activation_id='host-activation-1'"
            )
            fail("sqlite-v1.sql: released a provider-host reference before Activation settlement")
        except sqlite3.IntegrityError as exc:
            if "CAPABILITY_ACTIVATION_HOST_STILL_ACTIVE" not in str(exc):
                fail(f"sqlite-v1.sql: active Activation binding release raised unexpected error: {exc}")
        active_host_uses = invocation_db.execute(
            "SELECT COUNT(*) FROM capability_activation_host_bindings b JOIN capability_activations a USING(activation_id) WHERE b.host_instance_id=? AND a.status IN ('STARTING','ACTIVE','STOPPING')",
            ("provider-host",),
        ).fetchone()[0]
        if active_host_uses != 2:
            fail("sqlite-v1.sql: shared provider host use count is not derived from live Activations")
        invocation_db.execute(
            "UPDATE capability_activations SET status='STOPPED' WHERE activation_id='host-activation-1'"
        )
        invocation_db.execute(
            "DELETE FROM capability_activation_host_bindings WHERE activation_id='host-activation-1'"
        )
        remaining_host_uses = invocation_db.execute(
            "SELECT COUNT(*) FROM capability_activation_host_bindings b JOIN capability_activations a USING(activation_id) WHERE b.host_instance_id=? AND a.status IN ('STARTING','ACTIVE','STOPPING')",
            ("provider-host",),
        ).fetchone()[0]
        if remaining_host_uses != 1:
            fail("sqlite-v1.sql: settled Activation did not release exactly one derived provider-host use")
        try:
            invocation_db.execute(
                "UPDATE capability_activation_host_bindings SET provider_handle_ref='changed' WHERE activation_id='host-activation-2'"
            )
            fail("sqlite-v1.sql: accepted mutation of a Runtime-local provider binding")
        except sqlite3.IntegrityError as exc:
            if "IMMUTABLE_CAPABILITY_ACTIVATION_HOST_BINDING" not in str(exc):
                fail(f"sqlite-v1.sql: provider binding mutation raised unexpected error: {exc}")
        try:
            invocation_db.execute(
                "UPDATE capability_activations SET runtime_id='other-runtime' WHERE activation_id='host-activation-2'"
            )
            fail("sqlite-v1.sql: changed a durable Activation's origin Runtime while locally bound")
        except sqlite3.IntegrityError as exc:
            if "IMMUTABLE_CAPABILITY_ACTIVATION_IDENTITY" not in str(exc):
                fail(f"sqlite-v1.sql: bound Activation identity mutation raised unexpected error: {exc}")
        try:
            invocation_db.execute(
                "UPDATE capability_host_instances SET capability_identity_digest=? WHERE host_instance_id='provider-host'",
                ("sha256:" + "d" * 64,),
            )
            fail("sqlite-v1.sql: accepted mutation of immutable provider-host identity")
        except sqlite3.IntegrityError as exc:
            if "IMMUTABLE_CAPABILITY_HOST_IDENTITY" not in str(exc):
                fail(f"sqlite-v1.sql: provider-host identity mutation raised unexpected error: {exc}")
        try:
            invocation_db.execute(
                "INSERT INTO capability_activations(activation_id,workspace_id,scope_kind,conversation_id,capability_ref_json,runtime_id,runtime_incarnation_id,mode,status,health,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
                ("bad-host-activation", "ws", "CONVERSATION", "conversation", "{}", "other-runtime", "incarnation", "GATEWAY_PROXY", "ACTIVE", "HEALTHY", now, now),
            )
            invocation_db.execute(
                "INSERT INTO capability_activation_host_bindings(activation_id,host_instance_id,bound_at) VALUES(?,?,?)",
                ("bad-host-activation", "provider-host", now),
            )
            fail("sqlite-v1.sql: accepted a local host binding from another Runtime")
        except sqlite3.IntegrityError as exc:
            if "CAPABILITY_ACTIVATION_HOST_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: provider-host mismatch raised unexpected error: {exc}")
        invocation_values = (
            "invocation", "ws", "CONVERSATION", "conversation", None, None, "session", "activation", "{}", "read",
            "sha256:" + "a" * 64, "grant", "CREATED", now, now,
        )
        invocation_db.execute(
            "INSERT INTO capability_invocations(invocation_id,workspace_id,scope_kind,conversation_id,task_id,attempt_id,agent_session_id,activation_id,capability_ref_json,operation,request_digest,capability_grant_id,status,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            invocation_values,
        )
        operation_values = list(invocation_values)
        operation_values[0] = "unauthorized-operation-invocation"
        operation_values[9] = "write"
        try:
            invocation_db.execute(
                "INSERT INTO capability_invocations(invocation_id,workspace_id,scope_kind,conversation_id,task_id,attempt_id,agent_session_id,activation_id,capability_ref_json,operation,request_digest,capability_grant_id,status,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                operation_values,
            )
            fail("sqlite-v1.sql: admitted an Invocation operation absent from its grant")
        except sqlite3.IntegrityError as exc:
            if "INVOCATION_SCOPE_OR_AUTHORIZATION_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: ungranted Invocation operation raised unexpected error: {exc}")
        invocation_db.execute("UPDATE capability_grants SET expires_at='2000-01-01T00:00:00Z' WHERE capability_grant_id='grant'")
        expired_grant_values = list(invocation_values)
        expired_grant_values[0] = "expired-grant-invocation"
        try:
            invocation_db.execute(
                "INSERT INTO capability_invocations(invocation_id,workspace_id,scope_kind,conversation_id,task_id,attempt_id,agent_session_id,activation_id,capability_ref_json,operation,request_digest,capability_grant_id,status,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                expired_grant_values,
            )
            fail("sqlite-v1.sql: admitted an Invocation under an expired CapabilityGrant")
        except sqlite3.IntegrityError as exc:
            if "INVOCATION_SCOPE_OR_AUTHORIZATION_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: expired Grant Invocation raised unexpected error: {exc}")
        invocation_db.execute("UPDATE capability_grants SET expires_at=NULL WHERE capability_grant_id='grant'")
        invocation_db.execute("UPDATE conversation_turns SET status='WAITING_USER' WHERE turn_id='turn'")
        try:
            invocation_db.execute("UPDATE capability_invocations SET status='DISPATCHED' WHERE invocation_id='invocation'")
            fail("sqlite-v1.sql: dispatched an Invocation after its ConversationTurn stopped running")
        except sqlite3.IntegrityError as exc:
            if "INVOCATION_OWNER_NOT_LIVE_AT_DISPATCH" not in str(exc):
                fail(f"sqlite-v1.sql: stale-owner dispatch raised unexpected error: {exc}")
        invocation_db.execute(
            "INSERT INTO capability_activations(activation_id,workspace_id,scope_kind,task_id,capability_ref_json,runtime_id,runtime_incarnation_id,mode,status,health,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            ("wrong-scope-activation", "ws", "TASK_PLANNING", "task", "{}", "runtime", "incarnation", "GATEWAY_PROXY", "ACTIVE", "HEALTHY", now, now),
        )
        mismatched_invocation = list(invocation_values)
        mismatched_invocation[0] = "wrong-scope-invocation"
        mismatched_invocation[7] = "wrong-scope-activation"
        try:
            invocation_db.execute(
                "INSERT INTO capability_invocations(invocation_id,workspace_id,scope_kind,conversation_id,task_id,attempt_id,agent_session_id,activation_id,capability_ref_json,operation,request_digest,capability_grant_id,status,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                mismatched_invocation,
            )
            fail("sqlite-v1.sql: admitted an Invocation whose Activation scope disagrees with the AgentSession")
        except sqlite3.IntegrityError as exc:
            if "INVOCATION_SCOPE_OR_AUTHORIZATION_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: Activation scope mismatch raised unexpected error: {exc}")
        try:
            invocation_db.execute(
                "INSERT INTO capability_invocations(invocation_id,workspace_id,scope_kind,conversation_id,task_id,attempt_id,agent_session_id,activation_id,capability_ref_json,operation,request_digest,capability_grant_id,status,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                ("bad-invocation", "ws", "CONVERSATION", "other-conversation", None, None, "session", "activation", "{}", "read", "sha256:" + "b" * 64, "grant", "CREATED", now, now),
            )
            fail("sqlite-v1.sql: admitted a CapabilityInvocation whose scope differs from its AgentSession")
        except sqlite3.IntegrityError as exc:
            if "INVOCATION_SCOPE_OR_AUTHORIZATION_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: invocation scope mismatch raised unexpected error: {exc}")

        invocation_db.execute(
            "INSERT INTO tasks(task_id,workspace_id,current_spec_revision,status,lead_agent_binding_id,priority,created_by_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)",
            ("planning-task", "ws", 1, "RUNNING", "binding", "NORMAL", "{}", now, now),
        )
        invocation_db.execute(
            "INSERT INTO agent_sessions(agent_session_id,workspace_id,scope_kind,task_id,task_spec_revision,agent_binding_id,endpoint_id,runtime_id,runtime_incarnation_id,status,started_at) VALUES(?,?,?,?,?,?,?,?,?,?,?)",
            ("planning-session", "ws", "TASK_PLANNING", "planning-task", 1, "binding", "endpoint", "runtime", "incarnation", "ACTIVE", now),
        )
        try:
            invocation_db.execute("UPDATE agent_sessions SET task_spec_revision=2 WHERE agent_session_id='planning-session'")
            fail("sqlite-v1.sql: mutated an AgentSession's pinned TaskSpecRevision")
        except sqlite3.IntegrityError as exc:
            if "IMMUTABLE_AGENT_SESSION_IDENTITY" not in str(exc):
                fail(f"sqlite-v1.sql: AgentSession context mutation raised unexpected error: {exc}")
        invocation_db.execute(
            "INSERT INTO capability_grants(capability_grant_id,scope_kind,task_id,capability_ref_json,allowed_operations_json,resource_scope_json,secret_refs_json,granted_by_json,status,created_at) VALUES(?,?,?,?,?,?,?,?,?,?)",
            ("planning-grant", "TASK_PLANNING", "planning-task", "{}", '["read"]', "{}", "[]", "{}", "ACTIVE", now),
        )
        invocation_db.execute(
            "INSERT INTO capability_activations(activation_id,workspace_id,scope_kind,task_id,capability_ref_json,runtime_id,runtime_incarnation_id,mode,status,health,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            ("planning-activation", "ws", "TASK_PLANNING", "planning-task", "{}", "runtime", "incarnation", "GATEWAY_PROXY", "ACTIVE", "HEALTHY", now, now),
        )
        planning_invocation_values = (
            "planning-invocation", "ws", "TASK_PLANNING", None, "planning-task", None,
            "planning-session", "planning-activation", "{}", "read", "sha256:" + "a" * 64,
            "planning-grant", "CREATED", now, now,
        )
        invocation_insert = "INSERT INTO capability_invocations(invocation_id,workspace_id,scope_kind,conversation_id,task_id,attempt_id,agent_session_id,activation_id,capability_ref_json,operation,request_digest,capability_grant_id,status,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"
        invocation_db.execute(invocation_insert, planning_invocation_values)
        invocation_db.execute("UPDATE tasks SET current_spec_revision=2 WHERE task_id='planning-task'")
        stale_planning_values = list(planning_invocation_values)
        stale_planning_values[0] = "stale-planning-invocation"
        try:
            invocation_db.execute(invocation_insert, stale_planning_values)
            fail("sqlite-v1.sql: admitted a planning Invocation from a stale TaskSpecRevision")
        except sqlite3.IntegrityError as exc:
            if "INVOCATION_SCOPE_OR_AUTHORIZATION_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: stale planning Invocation raised unexpected error: {exc}")
        invocation_db.execute("UPDATE tasks SET current_spec_revision=1,status='PAUSE_REQUESTED' WHERE task_id='planning-task'")
        invocation_db.execute(
            "INSERT INTO user_requests(request_id,workspace_id,task_id,agent_session_id,kind,prompt,status,created_at) VALUES(?,?,?,?,?,?,?,?)",
            ("task-pause-request", "ws", "planning-task", "planning-session", "QUESTION", "Confirm", "PENDING", now),
        )
        paused_response_digest = "sha256:" + "7" * 64
        try:
            invocation_db.execute(
                "INSERT INTO user_request_responses(response_id,request_id,response_json,response_digest,responded_by_json,responded_at) VALUES(?,?,?,?,?,?)",
                ("early-task-response", "task-pause-request", '{"answer":"yes"}', paused_response_digest, '{"kind":"workspace_user"}', now),
            )
            fail("sqlite-v1.sql: accepted a Task-scoped response during PAUSE_REQUESTED")
        except sqlite3.IntegrityError as exc:
            if "USER_REQUEST_PARENT_NOT_RESPONDABLE" not in str(exc):
                fail(f"sqlite-v1.sql: PAUSE_REQUESTED response raised unexpected error: {exc}")
        invocation_db.execute("UPDATE tasks SET status='PAUSED' WHERE task_id='planning-task'")
        invocation_db.execute(
            "INSERT INTO user_request_responses(response_id,request_id,response_json,response_digest,responded_by_json,responded_at) VALUES(?,?,?,?,?,?)",
            ("paused-task-response", "task-pause-request", '{"answer":"yes"}', paused_response_digest, '{"kind":"workspace_user"}', now),
        )
        invocation_db.execute(
            "UPDATE user_requests SET status='ANSWERED',resolved_at=?,resolved_by_json=?,response_digest=? WHERE request_id='task-pause-request'",
            (now, '{"kind":"workspace_user"}', paused_response_digest),
        )
        paused_planning_values = list(planning_invocation_values)
        paused_planning_values[0] = "paused-planning-invocation"
        try:
            invocation_db.execute(invocation_insert, paused_planning_values)
            fail("sqlite-v1.sql: admitted a planning Invocation after Task pause was requested")
        except sqlite3.IntegrityError as exc:
            if "INVOCATION_SCOPE_OR_AUTHORIZATION_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: paused planning Invocation raised unexpected error: {exc}")
        invocation_db.execute("UPDATE tasks SET status='RUNNING',lead_agent_binding_id='other-binding' WHERE task_id='planning-task'")
        wrong_lead_values = list(planning_invocation_values)
        wrong_lead_values[0] = "wrong-lead-planning-invocation"
        try:
            invocation_db.execute(invocation_insert, wrong_lead_values)
            fail("sqlite-v1.sql: admitted a planning Invocation after lead AgentBinding changed")
        except sqlite3.IntegrityError as exc:
            if "INVOCATION_SCOPE_OR_AUTHORIZATION_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: stale lead planning Invocation raised unexpected error: {exc}")

        invocation_db.execute(
            "INSERT INTO tasks(task_id,workspace_id,current_spec_revision,current_plan_revision,status,lead_agent_binding_id,priority,created_by_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?)",
            ("execution-task", "ws", 1, 1, "RUNNING", "binding", "NORMAL", "{}", now, now),
        )
        invocation_db.execute(
            "INSERT INTO agent_sessions(agent_session_id,workspace_id,scope_kind,task_id,task_spec_revision,agent_binding_id,endpoint_id,runtime_id,runtime_incarnation_id,status,started_at) VALUES(?,?,?,?,?,?,?,?,?,?,?)",
            ("execution-plan-session", "ws", "TASK_PLANNING", "execution-task", 1, "binding", "endpoint", "runtime", "incarnation", "CLOSED", now),
        )
        invocation_db.execute(
            "INSERT INTO plan_revisions(task_id,revision,task_spec_revision,produced_by_agent_session_id,steps_json,created_at) VALUES(?,?,?,?,?,?)",
            ("execution-task", 1, 1, "execution-plan-session", "{}", now),
        )
        try:
            invocation_db.execute(
                "INSERT INTO plan_revisions(task_id,revision,task_spec_revision,produced_by_agent_session_id,steps_json,created_at) VALUES(?,?,?,?,?,?)",
                ("execution-task", 2, 2, "execution-plan-session", "{}", now),
            )
            fail("sqlite-v1.sql: accepted a PlanRevision whose AgentSession was pinned to another TaskSpecRevision")
        except sqlite3.IntegrityError as exc:
            if "PLAN_REVISION_SESSION_CONTEXT_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: mismatched PlanRevision context raised unexpected error: {exc}")
        invocation_db.execute(
            "INSERT INTO steps(step_id,task_id,plan_revision,title,objective,status,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?)",
            ("execution-step", "execution-task", 1, "Execute", "Execute", "READY", now, now),
        )
        invocation_db.execute(
            "INSERT INTO environments(environment_id,runtime_id,provider_kind,class,lifetime,owner_workspace_id,owner_task_id,owner_attempt_id,sharing_scope,name,status,health,budget_enforcement_policy,budget_enforcement,resource_limits_json,network_policy_json,budget_ceiling_json,backup_policy,isolation_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            ("execution-environment", "runtime", "fixture", "CONTAINER", "ATTEMPT", "ws", "execution-task", "execution-attempt", "ATTEMPT_PRIVATE", "Execution", "READY", "HEALTHY", "ALLOW_HOST_MONITORED", "HOST_MONITORED", "{}", "{}", "{}", "EXCLUDED", "{}", now, now),
        )
        invocation_db.execute(
            "INSERT INTO attempts(attempt_id,task_id,step_id,agent_binding_id,runtime_id,runtime_incarnation_id,environment_id,execution_lease_id,failover_class,status,started_at,created_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            ("execution-attempt", "execution-task", "execution-step", "binding", "runtime", "incarnation", "execution-environment", "execution-lease", "REPLAYABLE", "RUNNING", now, now),
        )
        invocation_db.execute(
            "UPDATE steps SET current_attempt_id='execution-attempt',status='RUNNING' WHERE step_id='execution-step'"
        )
        invocation_db.execute(
            "INSERT INTO agent_sessions(agent_session_id,workspace_id,scope_kind,task_id,task_spec_revision,attempt_id,agent_binding_id,endpoint_id,runtime_id,runtime_incarnation_id,status,started_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            ("execution-session", "ws", "ATTEMPT_EXECUTION", "execution-task", 1, "execution-attempt", "binding", "endpoint", "runtime", "incarnation", "ACTIVE", now),
        )
        invocation_db.execute(
            "UPDATE attempts SET agent_session_id='execution-session' WHERE attempt_id='execution-attempt'"
        )
        invocation_db.execute(
            "INSERT INTO execution_leases(lease_id,task_id,step_id,attempt_id,runtime_id,runtime_incarnation_id,epoch,issuer_key_version,fencing_token_digest,state,acquired_at,renew_by,expires_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)",
            ("execution-lease", "execution-task", "execution-step", "execution-attempt", "runtime", "incarnation", 1, 1, "sha256:" + "f" * 64, "ACTIVE", now, "2099-01-01T00:00:00Z", "2099-01-01T00:00:00Z"),
        )
        invocation_db.execute(
            "INSERT INTO capability_grants(capability_grant_id,scope_kind,task_id,attempt_id,capability_ref_json,allowed_operations_json,resource_scope_json,secret_refs_json,granted_by_json,status,created_at) VALUES(?,?,?,?,?,?,?,?,?,?,?)",
            ("execution-grant", "ATTEMPT_EXECUTION", "execution-task", "execution-attempt", "{}", '["read"]', "{}", "[]", "{}", "ACTIVE", now),
        )
        invocation_db.execute(
            "INSERT INTO capability_activations(activation_id,workspace_id,scope_kind,task_id,attempt_id,capability_ref_json,runtime_id,runtime_incarnation_id,mode,status,health,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)",
            ("execution-activation", "ws", "ATTEMPT_EXECUTION", "execution-task", "execution-attempt", "{}", "runtime", "incarnation", "GATEWAY_PROXY", "ACTIVE", "HEALTHY", now, now),
        )
        execution_invocation_values = (
            "execution-invocation", "ws", "ATTEMPT_EXECUTION", None, "execution-task", "execution-attempt",
            "execution-session", "execution-activation", "{}", "read", "sha256:" + "a" * 64,
            "execution-grant", "CREATED", now, now,
        )
        invocation_db.execute(invocation_insert, execution_invocation_values)
        invocation_db.execute("UPDATE execution_leases SET expires_at='2000-01-01T00:00:00Z' WHERE lease_id='execution-lease'")
        expired_lease_values = list(execution_invocation_values)
        expired_lease_values[0] = "expired-lease-invocation"
        try:
            invocation_db.execute(invocation_insert, expired_lease_values)
            fail("sqlite-v1.sql: admitted an execution Invocation after its lease expired")
        except sqlite3.IntegrityError as exc:
            if "INVOCATION_SCOPE_OR_AUTHORIZATION_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: expired-lease Invocation raised unexpected error: {exc}")
        invocation_db.execute("UPDATE execution_leases SET expires_at='2099-01-01T00:00:00Z' WHERE lease_id='execution-lease'")
        invocation_db.execute(
            "UPDATE capability_invocations SET status='DISPATCHED',provider_task_status='WORKING' WHERE invocation_id='execution-invocation'"
        )
        invocation_db.execute(
            "UPDATE capability_invocations SET status='INPUT_REQUIRED',provider_task_status='INPUT_REQUIRED' WHERE invocation_id='execution-invocation'"
        )
        invocation_db.execute(
            "INSERT INTO capability_invocation_provider_bindings(invocation_id,runtime_id,last_runtime_incarnation_id,binding_ciphertext,encryption_key_version,binding_digest,state,observed_at) VALUES(?,?,?,?,?,?,?,?)",
            ("execution-invocation", "runtime", "incarnation", b"encrypted-execution-provider-handle", 1, "sha256:" + "e" * 64, "AVAILABLE", now),
        )
        invocation_db.execute(
            "INSERT INTO user_requests(request_id,workspace_id,task_id,attempt_id,invocation_id,agent_session_id,kind,prompt,status,created_at) VALUES(?,?,?,?,?,?,?,?,?,?)",
            ("execution-input-request", "ws", "execution-task", "execution-attempt", "execution-invocation", "execution-session", "QUESTION", "Confirm", "PENDING", now),
        )
        invocation_db.execute(
                "INSERT INTO provider_input_bindings(request_id,invocation_id,runtime_id,provider_input_key_ciphertext,provider_input_payload_ciphertext,encryption_key_version,provider_input_key_tag,input_request_digest,delivery_status,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?)",
                ("execution-input-request", "execution-invocation", "runtime", b"encrypted-execution-input-key", b"encrypted-execution-input-payload", 1, "sha256:" + "9" * 64, "sha256:" + "8" * 64, "AWAITING_RESPONSE", now),
        )
        execution_response_digest = "sha256:" + "7" * 64
        invocation_db.execute(
            "INSERT INTO user_request_responses(response_id,request_id,response_json,response_digest,responded_by_json,responded_at) VALUES(?,?,?,?,?,?)",
            ("execution-input-response", "execution-input-request", '{"answer":"yes"}', execution_response_digest, '{"kind":"workspace_user"}', now),
        )
        invocation_db.execute(
            "UPDATE user_requests SET status='ANSWERED',resolved_at=?,resolved_by_json=?,response_digest=? WHERE request_id='execution-input-request'",
            (now, '{"kind":"workspace_user"}', execution_response_digest),
        )
        invocation_db.execute(
            "UPDATE provider_input_bindings SET response_id='execution-input-response',response_digest=?,delivery_status='PENDING',version=version+1,updated_at=? WHERE request_id='execution-input-request'",
            (execution_response_digest, now),
        )
        invocation_db.execute("UPDATE agent_sessions SET status='CLOSED',closed_at=? WHERE agent_session_id='execution-session'", (now,))
        try:
            invocation_db.execute(
                "UPDATE provider_input_bindings SET delivery_status='DISPATCHED',dispatch_count=1,last_dispatch_at=?,version=version+1,updated_at=? WHERE request_id='execution-input-request'",
                (now, now),
            )
            fail("sqlite-v1.sql: delivered Attempt provider input without a fresh current AgentSession")
        except sqlite3.IntegrityError as exc:
            if "PROVIDER_INPUT_OWNER_NOT_LIVE" not in str(exc):
                fail(f"sqlite-v1.sql: provider input without a fresh session raised unexpected error: {exc}")
        invocation_db.execute(
            "INSERT INTO agent_sessions(agent_session_id,workspace_id,scope_kind,task_id,task_spec_revision,attempt_id,agent_binding_id,endpoint_id,runtime_id,runtime_incarnation_id,status,started_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            ("execution-session-2", "ws", "ATTEMPT_EXECUTION", "execution-task", 1, "execution-attempt", "binding", "endpoint", "runtime", "incarnation", "ACTIVE", now),
        )
        invocation_db.execute(
            "UPDATE attempts SET agent_session_id='execution-session-2' WHERE attempt_id='execution-attempt'"
        )
        invocation_db.execute(
            "UPDATE provider_input_bindings SET delivery_status='DISPATCHED',dispatch_count=1,last_dispatch_at=?,version=version+1,updated_at=? WHERE request_id='execution-input-request'",
            (now, now),
        )
        try:
            invocation_db.execute("UPDATE capability_invocations SET operation='write' WHERE invocation_id='invocation'")
            fail("sqlite-v1.sql: allowed CapabilityInvocation identity mutation")
        except sqlite3.IntegrityError as exc:
            if "INVOCATION_IDENTITY_IMMUTABLE" not in str(exc):
                fail(f"sqlite-v1.sql: invocation identity mutation raised unexpected error: {exc}")
        request_values = (
            "user-request", "ws", "conversation", "turn", None, None, "invocation", "session", "QUESTION", "Confirm", "PENDING", now,
        )
        invocation_db.execute(
            "INSERT INTO user_requests(request_id,workspace_id,conversation_id,conversation_turn_id,task_id,attempt_id,invocation_id,agent_session_id,kind,prompt,status,created_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            request_values,
        )
        invocation_db.execute(
            "INSERT INTO capability_invocation_provider_bindings(invocation_id,runtime_id,last_runtime_incarnation_id,binding_ciphertext,encryption_key_version,binding_digest,state,observed_at) VALUES(?,?,?,?,?,?,?,?)",
            ("invocation", "runtime", "incarnation", b"encrypted-provider-handle", 1, "sha256:" + "e" * 64, "AVAILABLE", now),
        )
        try:
            invocation_db.execute(
                "INSERT INTO capability_invocation_provider_bindings(invocation_id,runtime_id,last_runtime_incarnation_id,binding_ciphertext,encryption_key_version,binding_digest,state,observed_at) VALUES(?,?,?,?,?,?,?,?)",
                ("invocation", "other-runtime", "other-incarnation", b"wrong-handle", 1, "sha256:" + "f" * 64, "AVAILABLE", now),
            )
            fail("sqlite-v1.sql: accepted a provider continuation binding on another Runtime")
        except sqlite3.IntegrityError as exc:
            if "INVOCATION_PROVIDER_BINDING_RUNTIME_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: provider binding Runtime mismatch raised unexpected error: {exc}")
        try:
            invocation_db.execute(
                "INSERT INTO user_requests(request_id,workspace_id,conversation_id,conversation_turn_id,task_id,attempt_id,invocation_id,agent_session_id,kind,prompt,status,created_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
                ("wrong-turn-request", "ws", "conversation", "other-turn", None, None, "invocation", "session", "QUESTION", "Confirm", "PENDING", now),
            )
            fail("sqlite-v1.sql: admitted a Conversation UserRequest linked to another turn in the same Conversation")
        except sqlite3.IntegrityError as exc:
            if "USER_REQUEST_SCOPE_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: wrong ConversationTurn linkage raised unexpected error: {exc}")
        invocation_db.execute(
            "INSERT INTO user_requests(request_id,workspace_id,conversation_id,conversation_turn_id,task_id,attempt_id,invocation_id,agent_session_id,kind,prompt,status,created_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            ("duplicate-request", "ws", "conversation", "turn", None, None, "invocation", "session", "QUESTION", "Confirm", "PENDING", now),
        )
        invocation_db.execute(
            "INSERT INTO user_requests(request_id,workspace_id,conversation_id,conversation_turn_id,task_id,attempt_id,invocation_id,agent_session_id,kind,interaction_mode,prompt,response_schema_json,choices_json,status,created_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            ("external-auth-request", "ws", "conversation", "turn", None, None, "invocation", "session", "EXTERNAL_AUTHORIZATION", "EXTERNAL_URL", "Connect to provider", None, "[]", "PENDING", now),
        )
        try:
            invocation_db.execute(
                "INSERT INTO user_request_responses(response_id,request_id,response_json,response_digest,responded_by_json,responded_at) VALUES(?,?,?,?,?,?)",
                ("external-auth-invalid-response", "external-auth-request", '{"action":"accept","content":{"token":"must-not-pass"}}', "sha256:" + "5" * 64, '{"kind":"workspace_user"}', now),
            )
            fail("sqlite-v1.sql: accepted credential/form content in an EXTERNAL_URL response")
        except sqlite3.IntegrityError as exc:
            if "INVALID_EXTERNAL_AUTH_RESPONSE" not in str(exc):
                fail(f"sqlite-v1.sql: invalid external-auth response raised unexpected error: {exc}")
        external_auth_digest = "sha256:" + "6" * 64
        invocation_db.execute(
            "INSERT INTO user_request_responses(response_id,request_id,response_json,response_digest,responded_by_json,responded_at) VALUES(?,?,?,?,?,?)",
            ("external-auth-response", "external-auth-request", '{"action":"accept"}', external_auth_digest, '{"kind":"workspace_user"}', now),
        )
        invocation_db.execute(
            "UPDATE user_requests SET status='ANSWERED',resolved_at=?,resolved_by_json=?,response_digest=? WHERE request_id='external-auth-request'",
            (now, '{"kind":"workspace_user"}', external_auth_digest),
        )
        try:
            invocation_db.execute(
                "INSERT INTO provider_input_bindings(request_id,invocation_id,runtime_id,provider_input_key_ciphertext,provider_input_payload_ciphertext,encryption_key_version,provider_input_key_tag,input_request_digest,delivery_status,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?)",
                ("user-request", "invocation", "runtime", b"encrypted-provider-input-key", b"encrypted-provider-input-payload", 1, "sha256:" + "d" * 64, "sha256:" + "c" * 64, "AWAITING_RESPONSE", now),
            )
            invocation_db.execute(
                "INSERT INTO provider_input_bindings(request_id,invocation_id,runtime_id,provider_input_key_ciphertext,provider_input_payload_ciphertext,encryption_key_version,provider_input_key_tag,input_request_digest,delivery_status,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?)",
                ("duplicate-request", "invocation", "runtime", b"duplicate-encrypted-key", b"duplicate-encrypted-payload", 1, "sha256:" + "d" * 64, "sha256:" + "c" * 64, "AWAITING_RESPONSE", now),
            )
            fail("sqlite-v1.sql: accepted a duplicate Runtime-local provider input key for one Invocation")
        except sqlite3.IntegrityError:
            pass
        # A response can be committed to the local outbox, then withdrawn only before
        # dispatch. The local state must never claim cancellation after dispatch or
        # allow a cancelled response to become deliverable again.
        response_digest = "sha256:" + "9" * 64
        invocation_db.execute(
            "INSERT INTO user_request_responses(response_id,request_id,response_json,response_digest,responded_by_json,responded_at) VALUES(?,?,?,?,?,?)",
            ("user-response", "user-request", '{"answer":"yes"}', response_digest, '{"kind":"workspace_user"}', now),
        )
        invocation_db.execute(
            "UPDATE user_requests SET status='ANSWERED',resolved_at=?,resolved_by_json=?,response_digest=? WHERE request_id='user-request'",
            (now, '{"kind":"workspace_user"}', response_digest),
        )
        try:
            invocation_db.execute(
                "UPDATE user_request_responses SET response_json='{}' WHERE response_id='user-response'"
            )
            fail("sqlite-v1.sql: accepted mutation of an immutable UserRequest response")
        except sqlite3.IntegrityError as exc:
            if "IMMUTABLE_USER_REQUEST_RESPONSE" not in str(exc):
                fail(f"sqlite-v1.sql: UserRequest response update raised unexpected error: {exc}")
        try:
            invocation_db.execute(
                "DELETE FROM user_request_responses WHERE response_id='user-response'"
            )
            fail("sqlite-v1.sql: accepted deletion of an immutable UserRequest response")
        except sqlite3.IntegrityError as exc:
            if "IMMUTABLE_USER_REQUEST_RESPONSE" not in str(exc):
                fail(f"sqlite-v1.sql: UserRequest response deletion raised unexpected error: {exc}")
        try:
            invocation_db.execute(
                "UPDATE user_requests SET response_digest=? WHERE request_id='user-request'",
                ("sha256:" + "8" * 64,),
            )
            fail("sqlite-v1.sql: allowed an answered UserRequest digest to diverge from its response")
        except sqlite3.IntegrityError as exc:
            if "USER_REQUEST_RESPONSE_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: mismatched UserRequest digest raised unexpected error: {exc}")
        try:
            invocation_db.execute(
                "UPDATE user_requests SET status='PENDING' WHERE request_id='user-request'"
            )
            fail("sqlite-v1.sql: allowed an answered UserRequest to reopen")
        except sqlite3.IntegrityError as exc:
            if "INVALID_USER_REQUEST_TRANSITION" not in str(exc):
                fail(f"sqlite-v1.sql: UserRequest reverse transition raised unexpected error: {exc}")
        invocation_db.execute(
            "UPDATE provider_input_bindings SET response_id='user-response',response_digest=?,delivery_status='PENDING',version=version+1,updated_at=? WHERE request_id='user-request'",
            (response_digest, now),
        )
        invocation_db.execute(
            "UPDATE provider_input_bindings SET delivery_status='CANCELLED',version=version+1,updated_at=? WHERE request_id='user-request'",
            (now,),
        )
        try:
            invocation_db.execute(
                "UPDATE provider_input_bindings SET delivery_status='PENDING' WHERE request_id='user-request'"
            )
            fail("sqlite-v1.sql: allowed a cancelled provider input response to become deliverable again")
        except sqlite3.IntegrityError as exc:
            if "INVALID_PROVIDER_INPUT_BINDING_TRANSITION" not in str(exc):
                fail(f"sqlite-v1.sql: cancelled provider input transition raised unexpected error: {exc}")
        try:
            invocation_db.execute(
                "UPDATE provider_input_bindings SET delivery_status='DISPATCHED' WHERE request_id='user-request'"
            )
            fail("sqlite-v1.sql: allowed a cancelled provider input response to dispatch")
        except sqlite3.IntegrityError as exc:
            if not any(code in str(exc) for code in (
                "INVALID_PROVIDER_INPUT_BINDING_TRANSITION",
                "PROVIDER_INPUT_DISPATCH_PROOF_OR_COUNT_INVALID",
            )):
                fail(f"sqlite-v1.sql: cancelled provider input dispatch raised unexpected error: {exc}")
        try:
            invocation_db.execute(
                "INSERT INTO user_requests(request_id,workspace_id,conversation_id,conversation_turn_id,task_id,attempt_id,invocation_id,agent_session_id,kind,prompt,status,created_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
                ("bad-request", "ws", "other-conversation", "wrong-turn", None, None, "invocation", "session", "QUESTION", "Confirm", "PENDING", now),
            )
            fail("sqlite-v1.sql: admitted a UserRequest whose scope differs from its Invocation/AgentSession")
        except sqlite3.IntegrityError as exc:
            if "USER_REQUEST_SCOPE_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: UserRequest scope mismatch raised unexpected error: {exc}")
    except Exception as exc:  # noqa: BLE001
        fail(f"sqlite-v1.sql: Invocation/UserRequest integrity fixture failed: {exc}")
    finally:
        invocation_db.close()

    backups = base_database()
    try:
        backup_values = (
            "backup", "ws", 1, "[]", '{"digest":"sha256:' + "a" * 64 + '","size_bytes":1,"media_type":"application/octet-stream"}',
            '{"digest":"sha256:' + "b" * 64 + '","size_bytes":1,"media_type":"application/octet-stream"}',
            "sha256:" + "c" * 64, "key-ref", "sha256:" + "d" * 64, "auth-tag", now, now,
        )
        backups.execute(
            "INSERT INTO workspace_backup_manifests(backup_id,workspace_id,schema_version,event_cursors_json,database_snapshot_ref_json,blob_manifest_ref_json,blob_manifest_digest,encryption_key_ref,integrity_digest,manifest_authentication,created_at,verified_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            backup_values,
        )
        try:
            backups.execute(
                "UPDATE workspace_backup_manifests SET integrity_digest=? WHERE backup_id='backup'",
                ("sha256:" + "e" * 64,),
            )
            fail("sqlite-v1.sql: accepted mutation of an immutable verified backup manifest")
        except sqlite3.IntegrityError as exc:
            if "IMMUTABLE_WORKSPACE_BACKUP_MANIFEST" not in str(exc):
                fail(f"sqlite-v1.sql: backup immutability raised unexpected error: {exc}")
    except Exception as exc:  # noqa: BLE001
        fail(f"sqlite-v1.sql: backup manifest constraint fixture failed: {exc}")
    finally:
        backups.close()


def check_gateway_and_names() -> None:
    path = DOCS / "CAPABILITY-FABRIC.md"
    text = path.read_text(encoding="utf-8")
    block = re.search(r"Permanent Gateway tool names:\s*```text(.*?)```", text, re.S)
    if not block:
        fail("CAPABILITY-FABRIC.md: missing permanent Gateway tool list")
    else:
        names = [line.strip() for line in block.group(1).splitlines() if line.strip()]
        wrong = [name for name in names if not name.startswith("litecowork.")]
        if wrong:
            fail(f"Gateway tool names outside frozen litecowork.* namespace: {wrong}")
        if len(names) != len(set(names)):
            fail("Gateway tool list contains duplicates")
    root_markdown = [path for path in ROOT.glob("*.md") if path.is_file()]
    for md in [*root_markdown, *DOCS.rglob("*.md")]:
        if "AgentCowork" in md.read_text(encoding="utf-8"):
            fail(f"{md.relative_to(ROOT)}: stale product name AgentCowork")
    scanned = [*root_markdown, *DOCS.rglob("*.md"), *DOCS.rglob("*.json"), *DOCS.rglob("*.yaml"), *DOCS.rglob("*.sql")]
    stale_protocol = re.compile(r"(?<![A-Za-z0-9_])litecow\.[a-z][a-z0-9_.]*")
    for path in scanned:
        for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
            match = stale_protocol.search(line)
            if match:
                fail(f"{path.relative_to(ROOT)}:{line_number}: stale protocol namespace {match.group(0)}; use litecowork.*")


def check_markdown_links() -> None:
    pattern = re.compile(r"\[[^\]]*\]\(([^)]+)\)")
    for md in [*ROOT.glob("*.md"), *DOCS.rglob("*.md")]:
        text = md.read_text(encoding="utf-8")
        fence_char = None
        fence_length = 0
        for line_number, line in enumerate(text.splitlines(), start=1):
            match = re.match(r"^ {0,3}(`{3,}|~{3,})(.*)$", line)
            if not match:
                continue
            marker, suffix = match.groups()
            if fence_char is None:
                fence_char = marker[0]
                fence_length = len(marker)
            elif marker[0] == fence_char and len(marker) >= fence_length and not suffix.strip():
                fence_char = None
                fence_length = 0
        if fence_char is not None:
            fail(f"{md.relative_to(ROOT)}: unclosed Markdown fence")
        for raw in pattern.findall(text):
            target = raw.strip().split()[0].strip("<>")
            if not target or target.startswith(("https://", "http://", "mailto:", "#")):
                continue
            target = unquote(target.split("#", 1)[0])
            if not target:
                continue
            resolved = (md.parent / target).resolve()
            if not resolved.exists():
                fail(f"{md.relative_to(ROOT)}: broken local link {raw}")


def check_runtime_routine_contract() -> None:
    """Reject known lifecycle, trigger-identity and cross-Workspace regressions."""
    automation = (DOCS / "AUTOMATION.md").read_text(encoding="utf-8")
    vectors = re.search(r"The v2 candidate vectors.*?```json\n(.*?)\n```", automation, re.S)
    if not vectors:
        fail("Automation v2 occurrence encoding lacks candidate vectors")
    else:
        for vector in json.loads(vectors.group(1)):
            encoded = b"LiteCowork/AutomationOccurrence/v2\0"
            for field in vector["fields"]:
                value = field.encode("utf-8")
                encoded += len(value).to_bytes(4, "big") + value
            if hashlib.sha256(encoded).hexdigest() != vector["occurrence_key"]:
                fail("Automation occurrence encoding vector differs from v2 contract")
    path = DOCS / "schemas" / "operator-api.openapi.yaml"
    api = yaml.load(path.read_text(encoding="utf-8"), Loader=UniqueKeyLoader)
    schemas = api["components"]["schemas"]
    if "get" not in api["paths"].get("/runtime-lifecycle/stop-preview", {}):
        fail("Runtime stop must have a read-only preview route")
    stop = schemas["StopRuntimeRequest"]
    if "expected_incarnation_id" not in stop.get("required", []):
        fail("Runtime stop must require the current incarnation")
    root = {"components": api["components"]}
    def validator(name):
        return jsonschema.Draft202012Validator({**root, "$ref": "#/components/schemas/" + name})
    trigger = validator("TriggerSpec")
    valid = {"trigger_id": "t1", "placement": "SPECIFIC_RUNTIME", "runtime_id": "r1", "trigger": {"kind": "MANUAL"}}
    if list(trigger.iter_errors(valid)):
        fail("TriggerSpec rejects a valid specific-Runtime trigger")
    for candidate in ({k: v for k, v in valid.items() if k != "runtime_id"}, {**valid, "runtime_id": None}, {**valid, "placement": "HUB"}):
        if not list(trigger.iter_errors(candidate)):
            fail("TriggerSpec accepts missing or contradictory trigger placement")
    resource = validator("ResourceEventTrigger")
    base = {"kind": "RESOURCE_EVENT", "event_kinds": ["changed"], "debounce_ms": 100}
    for candidate in (base, {**base, "workspace_root_id": "root1", "resource_id": "res1"}):
        if not list(resource.iter_errors(candidate)):
            fail("ResourceEventTrigger must select exactly one root/resource")
    if list(resource.iter_errors({**base, "resource_id": "res1"})):
        fail("ResourceEventTrigger rejects a valid single resource")
    sql = (DOCS / "schemas" / "sqlite-v1.sql").read_text(encoding="utf-8")
    for table in ("routine_revisions", "automation_revisions", "automation_occurrences", "automation_cursors"):
        body = re.search(r"CREATE TABLE " + table + r" \((.*?)\n\);", sql, re.S).group(1)
        if "workspace_id TEXT NOT NULL" not in body:
            fail(f"{table}: missing Workspace integrity key")
    cursor = re.search(r"CREATE TABLE automation_cursors \((.*?)\n\);", sql, re.S).group(1)
    if "PRIMARY KEY(automation_id, trigger_id)" not in cursor:
        fail("Automation cursor identity must survive revision edits")
    if "cursor_digest TEXT NOT NULL" not in cursor or "provider_cursor" in cursor:
        fail("AutomationCursor must replicate only a required digest, never an opaque provider cursor")
    bindings = re.search(r"CREATE TABLE automation_trigger_bindings \((.*?)\n\);", sql, re.S)
    if not bindings or "cursor_ciphertext BLOB NOT NULL" not in bindings.group(1):
        fail("Runtime-local AutomationTriggerBinding must encrypt opaque provider cursors")
    cursor_db = sqlite3.connect(":memory:")
    try:
        cursor_db.executescript(sql)
        cursor_db.execute("PRAGMA foreign_keys = OFF")
        now = "2026-10-04T00:00:00Z"
        first_digest = "sha256:" + "a" * 64
        next_digest = "sha256:" + "b" * 64
        cursor_db.execute(
            "INSERT INTO automation_cursors(workspace_id,automation_id,active_automation_revision,trigger_id,trigger_host_runtime_id,host_epoch,cursor_digest,last_checked_at) VALUES(?,?,?,?,?,?,?,?)",
            ("ws", "automation", 1, "trigger", "runtime", 1, first_digest, now),
        )
        cursor_db.execute(
            "INSERT INTO automation_trigger_bindings(automation_id,trigger_id,trigger_host_runtime_id,host_epoch,cursor_ciphertext,encryption_key_version,cursor_digest,state,updated_at) VALUES(?,?,?,?,?,?,?,?,?)",
            ("automation", "trigger", "runtime", 1, b"encrypted-cursor-1", 1, first_digest, "AVAILABLE", now),
        )
        cursor_db.execute(
            "UPDATE automation_cursors SET cursor_digest=? WHERE automation_id='automation' AND trigger_id='trigger'",
            (next_digest,),
        )
        cursor_db.execute(
            "UPDATE automation_trigger_bindings SET cursor_ciphertext=?,cursor_digest=?,version=2 WHERE automation_id='automation' AND trigger_id='trigger' AND host_epoch=1",
            (b"encrypted-cursor-2", next_digest),
        )
        try:
            cursor_db.execute(
                "UPDATE automation_trigger_bindings SET cursor_digest=? WHERE automation_id='automation' AND trigger_id='trigger' AND host_epoch=1",
                (first_digest,),
            )
            fail("sqlite-v1.sql: accepted a stale AutomationTriggerBinding cursor digest")
        except sqlite3.IntegrityError as exc:
            if "AUTOMATION_TRIGGER_BINDING_OWNER_MISMATCH" not in str(exc):
                fail(f"sqlite-v1.sql: stale automation cursor binding raised unexpected error: {exc}")
    except Exception as exc:  # noqa: BLE001
        fail(f"sqlite-v1.sql: AutomationTriggerBinding lifecycle fixture failed: {exc}")
    finally:
        cursor_db.close()
    event = load_json(DOCS / "schemas" / "domain-event.schema.json")
    payloads = event["$defs"]["payloads"]
    for name in ("automation_occurrence_created", "automation_occurrence_claimed", "automation_occurrence_settled", "automation_occurrence_status_changed"):
        required = set(payloads[name]["required"])
        if not {"routine_id", "routine_revision", "trigger_id", "trigger_host_runtime_id"} <= required:
            fail(f"{name}: missing pinned Routine/trigger provenance")


def check_channel_reply_contract() -> None:
    """Keep channel replies exact-scoped and channel host ownership fenced end to end."""
    schemas_text = (DOCS / "SCHEMAS.md").read_text(encoding="utf-8")
    channels = (DOCS / "CHANNELS.md").read_text(encoding="utf-8")
    model = (DOCS / "DATA-MODEL.md").read_text(encoding="utf-8")
    services = (DOCS / "SERVICES.md").read_text(encoding="utf-8")
    flows = (DOCS / "FLOWS.md").read_text(encoding="utf-8")
    sql = (DOCS / "schemas" / "sqlite-v1.sql").read_text(encoding="utf-8")
    api = yaml.load(
        (DOCS / "schemas" / "operator-api.openapi.yaml").read_text(encoding="utf-8"),
        Loader=UniqueKeyLoader,
    )
    api_schemas = api["components"]["schemas"]
    actions_match = re.search(r"^ChannelAction = ([A-Z_| ]+)$", schemas_text, re.M)
    if not actions_match:
        fail("SCHEMAS.md: missing canonical ChannelAction")
        return
    canonical_actions = set(actions_match.group(1).replace(" ", "").split("|"))
    expected_actions = {"VIEW", "STEER", "RESPOND", "APPROVE_SAFE", "APPROVE_SENSITIVE"}
    if canonical_actions != expected_actions:
        fail(f"ChannelAction mismatch: expected {sorted(expected_actions)}, got {sorted(canonical_actions)}")
    for name in ("ChannelBinding", "UpdateChannelBindingActionsRequest"):
        items = api_schemas.get(name, {}).get("properties", {}).get("allowed_actions", {}).get("items", {})
        if set(items.get("enum", [])) != canonical_actions:
            fail(f"OpenAPI {name}.allowed_actions differs from canonical ChannelAction")

    event_schema = load_json(DOCS / "schemas" / "domain-event.schema.json")
    event_payloads = event_schema.get("$defs", {}).get("payloads", {})
    channel_actions = set(
        event_payloads.get("channel_binding_changed", {})
        .get("properties", {})
        .get("allowed_actions", {})
        .get("items", {})
        .get("enum", [])
    )
    if channel_actions != canonical_actions:
        fail("domain-event channel.binding.changed allowed_actions differs from canonical ChannelAction")

    assignment_schema = api_schemas.get("ChannelHostAssignment", {})
    if not {"runtime_id", "host_epoch", "lease_expires_at", "status"} <= set(assignment_schema.get("properties", {})):
        fail("OpenAPI ChannelHostAssignment must expose current owner, epoch, lease expiry, and status")
    if "post" not in api.get("paths", {}).get("/channel-bindings/{channelBindingId}/host-assignment", {}):
        fail("OpenAPI must expose explicit ChannelHost reassignment")
    assignment_sql_match = re.search(r"CREATE TABLE channel_host_assignments \((.*?)\n\);", sql, re.S)
    lease_sql_match = re.search(r"CREATE TABLE channel_host_lease_records \((.*?)\n\);", sql, re.S)
    if not assignment_sql_match or not lease_sql_match:
        fail("sqlite-v1.sql: missing separate durable ChannelHostAssignment and renewable lease control record")
    else:
        assignment_sql = assignment_sql_match.group(1)
        lease_sql = lease_sql_match.group(1)
        if not {"channel_binding_id", "workspace_id", "runtime_id", "host_epoch", "status", "version"} <= set(re.findall(r"^\s{2}(\w+)\s+", assignment_sql, re.M)):
            fail("sqlite-v1.sql channel_host_assignments lacks canonical ownership fields")
        if "fencing_token_digest TEXT NOT NULL" not in lease_sql or "lease_expires_at TEXT NOT NULL" not in lease_sql:
            fail("sqlite-v1.sql channel_host_lease_records must store only the digest and bounded expiry")
        if "UNIQUE(channel_binding_id, workspace_id, runtime_id, host_epoch)" not in assignment_sql:
            fail("sqlite-v1.sql channel_host_assignments must scope lease records to exact owner epoch")

    target_sql_match = re.search(r"CREATE TABLE channel_reply_targets \((.*?)\n\);", sql, re.S)
    delivery_sql_match = re.search(r"CREATE TABLE notification_deliveries \((.*?)\n\);", sql, re.S)
    if not target_sql_match or "host_epoch INTEGER NOT NULL" not in target_sql_match.group(1):
        fail("sqlite-v1.sql ChannelReplyTarget must pin host_epoch")
    if not delivery_sql_match or not {"attempt_runtime_id", "attempt_host_epoch"} <= set(re.findall(r"^\s{2}(\w+)\s+", delivery_sql_match.group(1), re.M)):
        fail("sqlite-v1.sql NotificationDelivery must pin each channel attempt's Runtime/host epoch")
    if "d.attempt_runtime_id = NEW.runtime_id" not in sql or "d.attempt_host_epoch = NEW.host_epoch" not in sql:
        fail("sqlite-v1.sql reply target guard must bind acknowledged send to the same Runtime/host epoch")
    for payload_name, fields in (
        ("channel_host_assignment_changed", {"runtime_id", "host_epoch"}),
        ("channel_inbound_received", {"origin_runtime_id", "origin_host_epoch"}),
        ("channel_receipt_changed", {"claim_runtime_id", "claim_host_epoch"}),
        ("channel_outbound_settled", {"runtime_id", "host_epoch"}),
    ):
        required = set(event_payloads.get(payload_name, {}).get("required", []))
        if not fields <= required:
            fail(f"domain-event {payload_name} must preserve channel host provenance {sorted(fields)}")
    notification_payload = event_payloads.get("notification_delivery_changed", {})
    if notification_payload.get("dependentRequired", {}).get("attempt_runtime_id") != ["attempt_host_epoch"] or notification_payload.get("dependentRequired", {}).get("attempt_host_epoch") != ["attempt_runtime_id"]:
        fail("notification.delivery.changed must require paired attempt Runtime/host epoch provenance")

    status_match = re.search(r"^NotificationDeliveryStatus = ([A-Z_| ]+)$", schemas_text, re.M)
    if not status_match:
        fail("SCHEMAS.md: missing NotificationDeliveryStatus")
    else:
        expected_status = set(status_match.group(1).replace(" ", "").split("|"))
        if "AMBIGUOUS" not in expected_status:
            fail("NotificationDeliveryStatus must represent uncertain provider acceptance")
        for location, values in (
            ("OpenAPI Notification.status", set(api_schemas.get("Notification", {}).get("properties", {}).get("status", {}).get("enum", []))),
            ("SQLite notification_deliveries.status", set(re.findall(r"'([A-Z_]+)'", re.search(r"CREATE TABLE notification_deliveries \((.*?)\n\);", sql, re.S).group(1)))),
        ):
            if values != expected_status:
                fail(f"{location} differs from NotificationDeliveryStatus: missing={sorted(expected_status-values)}, extra={sorted(values-expected_status)}")

    for source, text in (("CHANNELS.md", channels), ("DATA-MODEL.md", model), ("SERVICES.md", services), ("FLOWS.md", flows)):
        if "ChannelReplyTarget" not in text:
            fail(f"{source}: missing durable exact reply correlation contract")
    for required in (
        "CREATE TABLE channel_reply_targets",
        "channel_reply_target_matches_delivery",
        "user_request_channel_response_authorized",
        "channel_reply_target_terminal",
        "response_channel_binding_id",
        "response_provider_event_id",
    ):
        if required not in sql:
            fail(f"sqlite-v1.sql: missing channel reply integrity rule {required}")
    channel_normalized = " ".join(channels.split())
    if "never answers “the latest” pending request" not in channel_normalized or "Plain text without" not in flows:
        fail("Channels/FLOWS: a plain message must never select an implicit pending UserRequest")
    if "Approval decisions" not in channel_normalized or "EXTERNAL_URL" not in channel_normalized:
        fail("CHANNELS.md: channel replies must exclude Approval and external sign-in paths")
    response_ref = api_schemas.get("UserRequestResponse", {}).get("properties", {}).get("response_channel_ref", {})
    if not response_ref.get("oneOf"):
        fail("OpenAPI UserRequestResponse must expose channel provenance as a paired object")

    validator = jsonschema.Draft202012Validator(event_schema)
    resolved = event_schema.get("$defs", {}).get("payloads", {}).get("user_request_resolved", {})
    if not {"channel_binding_id", "provider_event_id"} <= set(resolved.get("properties", {})):
        fail("user.request.resolved.v1 must expose optional channel response provenance")
    base_payload = {
        "request_id": "request-1", "from": "PENDING", "to": "ANSWERED",
        "resolved_by": {"kind": "USER", "principal_id": "owner-1"},
        "response_digest": "sha256:" + "a" * 64, "aggregate_version": 2,
    }
    envelope = {
        "event_id": "event-1", "workspace_id": "workspace-1", "entity_type": "USER_REQUEST",
        "entity_id": "request-1", "origin_runtime_id": "runtime-1", "origin_sequence": 1,
        "entity_revision": 2, "hlc_timestamp": "2026-10-05T00:00:00Z", "correlation_id": "corr-1",
        "schema_version": 1, "type": "user.request.resolved.v1", "payload": base_payload,
        "aggregate_state_ref": {"blob": {"digest": "sha256:" + "b" * 64, "size_bytes": 1,
          "media_type": "application/vnd.litecowork.aggregate+json"}, "entity_revision": 2,
          "record_schema_version": 1}, "recorded_at": "2026-10-05T00:00:00Z",
        "payload_digest": "sha256:" + "c" * 64,
    }
    if list(validator.iter_errors(envelope)):
        fail("user.request.resolved.v1: rejects a valid Operator response event")
    paired = json.loads(json.dumps(envelope))
    paired["payload"].update({"channel_binding_id": "channel-1", "provider_event_id": "event-42"})
    if list(validator.iter_errors(paired)):
        fail("user.request.resolved.v1: rejects a valid channel response event")
    unpaired = json.loads(json.dumps(envelope))
    unpaired["payload"]["channel_binding_id"] = "channel-1"
    if not list(validator.iter_errors(unpaired)):
        fail("user.request.resolved.v1: accepts incomplete channel response provenance")
    unanswered = json.loads(json.dumps(envelope))
    unanswered["payload"].pop("response_digest")
    if not list(validator.iter_errors(unanswered)):
        fail("user.request.resolved.v1: accepts ANSWERED without response_digest")
    dismissed = json.loads(json.dumps(envelope))
    dismissed["payload"]["to"] = "DISMISSED"
    dismissed["payload"].pop("response_digest")
    if list(validator.iter_errors(dismissed)):
        fail("user.request.resolved.v1: rejects a valid DISMISSED event")
    dismissed_with_answer = json.loads(json.dumps(dismissed))
    dismissed_with_answer["payload"]["response_digest"] = "sha256:" + "a" * 64
    if not list(validator.iter_errors(dismissed_with_answer)):
        fail("user.request.resolved.v1: accepts response provenance on a non-answer resolution")


def main() -> int:
    check_json_schemas()
    check_event_contract()
    check_provider_circuit_counter_contract()
    check_error_registry()
    check_task_status_contract()
    check_task_lifecycle_contract()
    check_resource_contract()
    check_async_capability_contract()
    check_provider_host_contract()
    check_capability_ref_contract()
    check_benchmark_ids()
    check_replication_scope_contract()
    check_backup_contract()
    check_runtime_routine_contract()
    check_channel_reply_contract()
    check_openapi()
    check_storage()
    check_attempt_lease_contract()
    check_gateway_and_names()
    check_markdown_links()
    if ERRORS:
        for error in ERRORS:
            print(f"ERROR: {error}", file=sys.stderr)
        print(f"Architecture validation failed with {len(ERRORS)} finding(s).", file=sys.stderr)
        return 1
    registry_text = (DOCS / "EVENTS.md").read_text(encoding="utf-8")
    registry_block = re.search(
        r"Minimum v1 registry:\s*```\s*(.*?)```", registry_text, re.S
    )
    event_count = len(re.findall(r"^[a-z][a-z0-9_.-]+\.v\d+$", registry_block.group(1), re.M))
    print(f"Architecture validation passed: JSON Schemas, {event_count} typed events, error codes, OpenAPI, SQLite, Gateway names, product naming, and Markdown links.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
