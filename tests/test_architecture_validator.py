import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import yaml

from scripts import validate_architecture


class JsonSchemaValidationTests(unittest.TestCase):
    def test_malformed_schema_is_reported(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            schemas = root / "docs" / "schemas"
            schemas.mkdir(parents=True)
            (schemas / "malformed.json").write_text("{not json", encoding="utf-8")

            with (
                patch.object(validate_architecture, "ROOT", root),
                patch.object(validate_architecture, "DOCS", root / "docs"),
            ):
                validate_architecture.ERRORS.clear()
                validate_architecture.check_json_schemas()
                errors = list(validate_architecture.ERRORS)
                validate_architecture.ERRORS.clear()

        self.assertTrue(any("invalid JSON" in error for error in errors), errors)

    def test_valid_schema_is_accepted(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            schemas = root / "docs" / "schemas"
            schemas.mkdir(parents=True)
            (schemas / "valid.json").write_text(
                '{"$schema":"https://json-schema.org/draft/2020-12/schema",'
                '"type":"object","additionalProperties":false}',
                encoding="utf-8",
            )

            with (
                patch.object(validate_architecture, "ROOT", root),
                patch.object(validate_architecture, "DOCS", root / "docs"),
            ):
                validate_architecture.ERRORS.clear()
                validate_architecture.check_json_schemas()
                errors = list(validate_architecture.ERRORS)
                validate_architecture.ERRORS.clear()

        self.assertEqual(errors, [])


class CheckScriptToolchainTests(unittest.TestCase):
    def run_with_stub(self, executable_name, content):
        with tempfile.TemporaryDirectory() as temporary_directory:
            bin_directory = Path(temporary_directory)
            executable = bin_directory / executable_name
            executable.write_text(f"#!/bin/sh\n{content}\n", encoding="utf-8")
            executable.chmod(0o755)
            environment = os.environ.copy()
            environment["PATH"] = f"{bin_directory}:{environment['PATH']}"
            return subprocess.run(
                ["bash", "scripts/check.sh"],
                cwd=Path(__file__).resolve().parents[1],
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )

    def test_wrong_uv_version_fails_before_build(self):
        result = self.run_with_stub("uv", 'printf "uv 0.0.0\\n"')

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Expected uv 0.12.23, found 0.0.0", result.stderr)

    def test_wrong_rust_version_fails_before_build(self):
        result = self.run_with_stub("rustc", 'printf "rustc 0.0.0 (stub)\\n"')

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Expected Rust 1.98.1, found 0.0.0", result.stderr)

    def test_wrong_python_version_fails_before_checks(self):
        result = self.run_with_stub(
            "uv",
            'if [ "$1" = "--version" ]; then echo "uv 0.12.23"; '
            'elif [ "$1" = "sync" ]; then exit 0; '
            'elif [ "$1" = "run" ]; then echo "Python 3.12.0"; '
            'else exit 1; fi',
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Expected Python 3.13.12, found 3.12.0", result.stderr)

    def test_ci_uses_the_local_check_and_pinned_uv_version(self):
        root = Path(__file__).resolve().parents[1]
        workflow = yaml.safe_load(
            (root / ".github/workflows/architecture-docs.yml").read_text(
                encoding="utf-8"
            )
        )
        steps = workflow["jobs"]["check"]["steps"]
        uv_step = next(
            step for step in steps if step.get("uses", "").startswith("astral-sh/setup-uv@")
        )

        self.assertEqual(
            uv_step["with"]["version"],
            (root / ".uv-version").read_text(encoding="utf-8").strip(),
        )
        self.assertTrue(any(step.get("run") == "scripts/check.sh" for step in steps))
        self.assertTrue(
            any(
                step.get("uses", "").startswith(
                    "actions-rust-lang/setup-rust-toolchain@"
                )
                for step in steps
            )
        )


if __name__ == "__main__":
    unittest.main()
