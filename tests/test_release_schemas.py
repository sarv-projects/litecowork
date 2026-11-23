"""Fixture tests for the Feature 019 release schemas (019-T02, AC-013)."""
import copy
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]
SCHEMAS = ROOT / "docs" / "schemas"


def load(name):
    return json.loads((SCHEMAS / name).read_text(encoding="utf-8"))


def errors(schema, doc):
    return list(Draft202012Validator(schema).iter_errors(doc))


BUNDLE = {
    "journey_id": "E2E-01",
    "requirement_ids": ["AC-012"],
    "owner_contracts": ["docs/ACCEPTANCE-AND-E2E-PLAN.md"],
    "row_id": "linux-x64-opencode-v2.0.32",
    "repo_sha": "a" * 40,
    "dirty": False,
    "os_build": "Ubuntu 24.04",
    "arch": "x86_64",
    "packaging_mode": "packaged",
    "agent_versions": {"opencode": "2.0.32"},
    "auth_mode": "user-configured",
    "protocol": "opencode-v2-http-sse",
    "code_commands": [{"command": "cargo test", "exit_code": 0, "output_digest": "b" * 64}],
    "system_trace": {
        "request_ids": ["r1"],
        "correlation_ids": ["c1"],
        "event_sequences": [1, 2],
        "receipt_refs": ["rcpt-1"],
    },
    "user_artifacts": [{"digest": "c" * 64, "kind": "screenshot", "storage_ref": "owner://s1"}],
    "negative_cases": [{"case": "wrong-user peer", "outcome": "REJECTED"}],
    "limitations": [],
    "reviewer": "owner",
    "accepted_at": "2026-11-10",
}

ROW = {
    "row_id": "windows-x64-opencode-v2.0.32",
    "os": "windows",
    "os_min_build": "10.0.19045",
    "arch": "x86_64",
    "target_triple": "x86_64-pc-windows-msvc",
    "desktop_runtime": "WebView2",
    "agent": "opencode",
    "agent_version": "2.0.32",
    "auth_mode": "user-configured",
    "protocol": "opencode-v2-http-sse",
    "priority": 1,
    "counted": True,
}


class EvidenceBundleSchema(unittest.TestCase):
    def setUp(self):
        self.schema = load("release-evidence-bundle.schema.json")
        Draft202012Validator.check_schema(self.schema)

    def test_positive_fixture_validates(self):
        self.assertEqual(errors(self.schema, BUNDLE), [])

    def test_missing_required_fields_are_rejected(self):
        for field in ("repo_sha", "os_build", "agent_versions", "negative_cases", "reviewer"):
            doc = copy.deepcopy(BUNDLE)
            del doc[field]
            self.assertTrue(errors(self.schema, doc), field)

    def test_empty_negative_cases_and_agent_versions_rejected(self):
        doc = copy.deepcopy(BUNDLE)
        doc["negative_cases"] = []
        self.assertTrue(errors(self.schema, doc))
        doc = copy.deepcopy(BUNDLE)
        doc["agent_versions"] = {}
        self.assertTrue(errors(self.schema, doc))

    def test_bad_sha_and_unknown_field_rejected(self):
        doc = copy.deepcopy(BUNDLE)
        doc["repo_sha"] = "xyz"
        self.assertTrue(errors(self.schema, doc))
        doc = copy.deepcopy(BUNDLE)
        doc["raw_prompt"] = "private"
        self.assertTrue(errors(self.schema, doc))

    def test_attachment_digest_must_be_sha256(self):
        doc = copy.deepcopy(BUNDLE)
        doc["user_artifacts"][0]["digest"] = "short"
        self.assertTrue(errors(self.schema, doc))


class ReleaseRowSchema(unittest.TestCase):
    def setUp(self):
        self.schema = load("release-matrix-row.schema.json")
        Draft202012Validator.check_schema(self.schema)

    def test_counted_row_validates(self):
        self.assertEqual(errors(self.schema, ROW), [])

    def test_not_counted_needs_reason(self):
        doc = copy.deepcopy(ROW)
        doc["counted"] = False
        self.assertTrue(errors(self.schema, doc))
        doc["not_counted_reason"] = "WSL2"
        self.assertEqual(errors(self.schema, doc), [])

    def test_counted_row_cannot_carry_reason(self):
        doc = copy.deepcopy(ROW)
        doc["not_counted_reason"] = "CONTAINER"
        self.assertTrue(errors(self.schema, doc))

    def test_macos_is_not_a_declarable_row(self):
        doc = copy.deepcopy(ROW)
        doc["os"] = "macos"
        self.assertTrue(errors(self.schema, doc))


if __name__ == "__main__":
    unittest.main()
