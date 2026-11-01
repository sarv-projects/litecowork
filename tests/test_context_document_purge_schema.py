import json
import unittest
from pathlib import Path

import jsonschema


ROOT = Path(__file__).resolve().parents[1]


def _domain_event_schema():
    return json.loads(
        (ROOT / "docs/schemas/domain-event.schema.json").read_text(encoding="utf-8")
    )


def _payload_validator(name):
    schema = _domain_event_schema()
    return jsonschema.Draft202012Validator(schema["$defs"]["payloads"][name])


def _historical_v1_payload():
    return {
        "resource_id": "resource-1",
        "replica_ref": "blob://opaque-local-store/ref-1",
        "replica_kind": "BLOB",
        "target_revision_ids": [f"revision-{index}" for index in range(20)],
        "receipt_digest": "sha256:" + "b" * 64,
        "acknowledged_at": "2026-10-15T00:00:00Z",
        "aggregate_version": 3,
    }


def _bounded_v2_payload():
    return {
        "resource_id": "resource-1",
        "purge_manifest_digest": "sha256:" + "a" * 64,
        "replica_ref": "sha256:" + "b" * 64,
        "replica_kind": "BLOB",
        "runtime_id": None,
        "runtime_incarnation_id": None,
        "target_digest": "sha256:" + "c" * 64,
        "receipt_digest": "sha256:" + "d" * 64,
        "acknowledged_at": "2026-10-15T00:00:00.000000000Z",
        "aggregate_version": 3,
    }


def _event_envelope(event_type, payload):
    return {
        "event_id": "event-1",
        "workspace_id": "workspace-1",
        "entity_type": "ContextDocument",
        "entity_id": "resource-1",
        "origin_runtime_id": "runtime-local",
        "origin_sequence": 1,
        "entity_revision": 3,
        "hlc_timestamp": "2026-10-15T00:00:00Z",
        "correlation_id": "correlation-1",
        "schema_version": 1,
        "type": event_type,
        "payload": payload,
        "aggregate_state_ref": {
            "blob": {
                "digest": "sha256:" + "e" * 64,
                "size_bytes": 10,
                "media_type": "application/json",
            },
            "entity_revision": 3,
            "record_schema_version": 1,
        },
        "recorded_at": "2026-10-15T00:00:00Z",
        "payload_digest": "sha256:" + "f" * 64,
    }


class ContextDocumentPurgeEventSchemaTests(unittest.TestCase):
    def test_historical_v1_payload_remains_readable(self):
        validator = _payload_validator("resource_context_document_purge_acknowledged")
        payload = _historical_v1_payload()

        self.assertTrue(validator.is_valid(payload))

        runtime_pair = dict(payload)
        runtime_pair["runtime_id"] = "runtime-1"
        runtime_pair["runtime_incarnation_id"] = "incarnation-1"
        self.assertTrue(validator.is_valid(runtime_pair))

    def test_v2_payload_omits_variable_membership_fields(self):
        schema = _domain_event_schema()
        payload_name = "resource_context_document_purge_acknowledged_v2"
        self.assertIn(payload_name, schema["$defs"]["payloads"])
        validator = _payload_validator(payload_name)
        payload = _bounded_v2_payload()

        self.assertTrue(validator.is_valid(payload))
        self.assertEqual(
            set(payload),
            {
                "resource_id",
                "purge_manifest_digest",
                "replica_ref",
                "replica_kind",
                "runtime_id",
                "runtime_incarnation_id",
                "target_digest",
                "receipt_digest",
                "acknowledged_at",
                "aggregate_version",
            },
        )

        for unbounded_member_field in ("target_revision_ids", "target_upload_chunk_refs"):
            with self.subTest(field=unbounded_member_field):
                with_members = dict(payload)
                with_members[unbounded_member_field] = ["member"] * 100
                self.assertFalse(validator.is_valid(with_members))

    def test_v2_local_ack_requires_an_explicit_null_runtime_pair(self):
        validator = _payload_validator("resource_context_document_purge_acknowledged_v2")

        missing_runtime_id = _bounded_v2_payload()
        del missing_runtime_id["runtime_id"]
        self.assertFalse(validator.is_valid(missing_runtime_id))

        partial_runtime_pair = _bounded_v2_payload()
        partial_runtime_pair["runtime_id"] = "runtime-1"
        self.assertFalse(validator.is_valid(partial_runtime_pair))

        complete_runtime_pair = _bounded_v2_payload()
        complete_runtime_pair["runtime_id"] = "runtime-1"
        complete_runtime_pair["runtime_incarnation_id"] = "incarnation-1"
        self.assertFalse(validator.is_valid(complete_runtime_pair))

        for field in ("purge_manifest_digest", "replica_ref", "target_digest", "receipt_digest"):
            with self.subTest(field=field):
                malformed = _bounded_v2_payload()
                malformed[field] = "/private/local/path"
                self.assertFalse(validator.is_valid(malformed))

    def test_v2_acknowledgement_fields_follow_canonical_json_bounds(self):
        validator = _payload_validator("resource_context_document_purge_acknowledged_v2")

        oversized_timestamp = _bounded_v2_payload()
        oversized_timestamp["acknowledged_at"] = "2026-10-15T00:00:00." + "1" * 100 + "Z"
        self.assertFalse(validator.is_valid(oversized_timestamp))

        oversized_version = _bounded_v2_payload()
        oversized_version["aggregate_version"] = 9_007_199_254_740_992
        self.assertFalse(validator.is_valid(oversized_version))

    def test_v2_timestamp_uses_the_canonical_fixed_width_utc_form(self):
        validator = _payload_validator("resource_context_document_purge_acknowledged_v2")

        for timestamp in (
            "2026-10-15T00:00:00Z",
            "2026-10-15T00:00:00.000000000+00:00",
            "٢٠٢٦-10-15T00:00:00.000000000Z",
        ):
            with self.subTest(timestamp=timestamp):
                payload = _bounded_v2_payload()
                payload["acknowledged_at"] = timestamp
                self.assertFalse(validator.is_valid(payload))

    def test_v1_and_v2_event_types_route_to_their_payload_contracts(self):
        schema = _domain_event_schema()
        validator = jsonschema.Draft202012Validator(schema)

        historical = _event_envelope(
            "resource.context_document.purge.acknowledged.v1", _historical_v1_payload()
        )
        bounded = _event_envelope(
            "resource.context_document.purge.acknowledged.v2", _bounded_v2_payload()
        )

        self.assertTrue(validator.is_valid(historical))
        self.assertTrue(validator.is_valid(bounded))

        wrong_v2_shape = dict(bounded)
        wrong_v2_shape["payload"] = _historical_v1_payload()
        self.assertFalse(validator.is_valid(wrong_v2_shape))


if __name__ == "__main__":
    unittest.main()
