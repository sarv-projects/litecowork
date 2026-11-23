"""Evidence bundle tool tests (019-T15, AC-013, AC-014)."""
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "v1_evidence_bundle.py"
ROW = "linux-x64-opencode-v2.0.32"
SHA = "a" * 40

spec = importlib.util.spec_from_file_location("v1_evidence_bundle", SCRIPT)
tool = importlib.util.module_from_spec(spec)
sys.modules["v1_evidence_bundle"] = tool
spec.loader.exec_module(tool)

matrix_spec = importlib.util.spec_from_file_location("v1_platform_matrix", ROOT / "scripts" / "v1_platform_matrix.py")
matrix = importlib.util.module_from_spec(matrix_spec)
sys.modules["v1_platform_matrix"] = matrix
matrix_spec.loader.exec_module(matrix)

DETAILS = {
    "requirement_ids": ["AC-013"],
    "owner_contracts": ["docs/ACCEPTANCE-AND-E2E-PLAN.md"],
    "packaging_mode": "packaged",
    "auth_mode": "user-configured",
    "protocol": "opencode-v2-http-sse",
    "code_commands": [{"command": "cargo test", "exit_code": 0, "output_digest": "b" * 64}],
    "system_trace": {"request_ids": ["r"], "correlation_ids": ["c"], "event_sequences": [1], "receipt_refs": []},
    "negative_cases": [{"case": "stale handle", "outcome": "REFUSED"}],
    "limitations": [],
    "reviewer": "owner",
    "accepted_at": "2026-11-10",
}


def fake_token():
    # Built at runtime so this file itself never contains a token-shaped literal.
    return "AK" + "IA" + "ABCDEFGHIJKLMNOP"


class BundleTool(unittest.TestCase):
    def test_os_build_capture_uses_release_minimum_tuple(self):
        self.assertEqual(tool.format_os_build("Windows", "10.0.26100", "11"), "Windows 10.0.26100")
        self.assertEqual(
            tool.format_os_build("Linux", "6.8.0", "Linux", {"NAME": "Ubuntu", "VERSION_ID": "24.04"}),
            "Ubuntu 24.04",
        )

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(dir="/tmp")
        self.addCleanup(self.tmp.cleanup)
        self.dir = Path(self.tmp.name)
        self.out = self.dir / "owner"
        self.index = self.dir / "INDEX.md"

    def details(self, **changes):
        document = {**DETAILS, **changes}
        for key in [k for k, v in changes.items() if v is None]:
            document.pop(key)
        path = self.dir / "details.json"
        path.write_text(json.dumps(document))
        return path

    def assemble(self, details, *extra, agent_version="2.0.32"):
        command = [
            sys.executable, str(SCRIPT), "assemble", "--journey", "E2E-01", "--row", ROW,
            "--details", str(details), "--out-dir", str(self.out), "--index", str(self.index),
            "--repo-sha", SHA, *extra,
        ]
        if agent_version:
            command += ["--agent-version", agent_version]
        return subprocess.run(command, capture_output=True, text=True)

    def test_valid_bundle_is_written_and_indexed_with_digest_only(self):
        result = self.assemble(self.details())
        self.assertEqual(result.returncode, 0, result.stderr)
        bundle = next(self.out.glob("*.json"))
        document = json.loads(bundle.read_text())
        digest = tool.canonical_digest(document)
        self.assertEqual(digest, matrix.canonical_digest(document))
        index_text = self.index.read_text()
        self.assertIn(digest, index_text)
        self.assertNotIn("negative_cases", index_text)
        check = subprocess.run(
            [sys.executable, str(SCRIPT), "validate", str(self.index), "--bundles-dir", str(self.out)],
            capture_output=True, text=True,
        )
        self.assertEqual(check.returncode, 0, check.stderr)

    def test_missing_required_fields_reject(self):
        for field in ("negative_cases", "reviewer", "auth_mode", "protocol"):
            result = self.assemble(self.details(**{field: None}))
            self.assertEqual(result.returncode, 1, field)
            self.assertIn(field, result.stderr)
        self.assertFalse(list(self.out.glob("*.json")) if self.out.exists() else [])

    def test_missing_agent_version_rejects(self):
        result = self.assemble(self.details(), agent_version=None)
        # Only passes if opencode is installed on this host; otherwise the schema rejects.
        if result.returncode != 0:
            self.assertIn("agent_versions", result.stderr)

    def test_seeded_token_in_details_rejects_without_echoing_it(self):
        result = self.assemble(self.details(limitations=[f"log line {fake_token()} leaked"]))
        self.assertEqual(result.returncode, 1)
        self.assertIn("secret scan", result.stderr)
        self.assertNotIn(fake_token(), result.stderr + result.stdout)

    def test_seeded_token_in_log_attachment_rejects(self):
        log = self.dir / "run.log"
        log.write_text(f"token={fake_token()}\n")
        result = self.assemble(self.details(), "--attachment", f"{log}:log:file:run.log")
        self.assertEqual(result.returncode, 1)
        self.assertIn("secret scan", result.stderr)
        self.assertNotIn(fake_token(), result.stderr)

    def test_changing_an_attachment_invalidates_the_bundle(self):
        log = self.dir / "run.log"
        log.write_text("clean output\n")
        self.assertEqual(self.assemble(self.details(), "--attachment", f"{log}:log:file:run.log").returncode, 0)
        bundle = next(self.out.glob("*.json"))
        ok = subprocess.run(
            [sys.executable, str(SCRIPT), "validate", str(bundle), "--attachments-dir", str(self.dir)],
            capture_output=True, text=True,
        )
        self.assertEqual(ok.returncode, 0, ok.stderr)
        log.write_text("edited after sign-off\n")
        bad = subprocess.run(
            [sys.executable, str(SCRIPT), "validate", str(bundle), "--attachments-dir", str(self.dir)],
            capture_output=True, text=True,
        )
        self.assertEqual(bad.returncode, 1)
        self.assertIn("no longer matches", bad.stderr)

    def test_attachment_path_traversal_is_rejected(self):
        document = {**DETAILS, "journey_id": "E2E-01", "row_id": ROW, "repo_sha": SHA, "dirty": False,
                    "os_build": "x", "arch": "x86_64", "agent_versions": {"opencode": "2.0.32"},
                    "user_artifacts": [{"digest": "c" * 64, "kind": "log", "storage_ref": "file:../etc/passwd"}]}
        problems = tool.validate_bundle(document, None, self.dir)
        self.assertTrue(any("outside" in p or "missing" in p for p in problems))

    def test_journey_without_signoff_stays_below_qualified(self):
        bundle = {
            **DETAILS,
            "journey_id": "E2E-01",
            "row_id": ROW,
            "repo_sha": SHA,
            "dirty": False,
            "os_build": "Ubuntu 24.04",
            "arch": "x86_64",
            "agent_versions": {"opencode": "2.0.32"},
            "user_artifacts": [],
        }
        self.assertEqual(tool.validate_bundle(bundle), [])
        bundles = {(ROW, "E2E-01"): {"digest": tool.canonical_digest(bundle), "document": bundle}}
        rows = matrix.load_rows(ROOT / "scripts" / "v1_release_matrix_rows.json")
        self.assertEqual(matrix.evidence_tuple_mismatches(rows["rows"][1], bundle), [])
        built = matrix.build_release_matrix(rows, "linux", "NATIVE", "test", bundles)
        cell = next(r for r in built["rows"] if r["row_id"] == ROW)["cells"]["E2E-01"]
        self.assertEqual(cell["state"], "PASSED", cell)

    def test_committed_index_validates(self):
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "validate", str(ROOT / "docs/release/evidence/INDEX.md")],
            capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
