"""Regression tests for the pinned-toolchain behaviour of ``scripts/check.sh``.

The existing toolchain guard tests live in ``tests/test_architecture_validator.py``.
This module covers the behaviour that guard was extended with: a missing pinned
tool is reported clearly, a floating (non-isolated) Python interpreter is
rejected, the fast preflight mode never starts a Rust build, and the gate
reports the pinned tool versions it actually used.

Every external command is replaced by a stub so the tests never depend on a
network, a credential service, or a compiled workspace.
"""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# A uv stub that satisfies the pinned toolchain contract without doing any work:
# it reports the pinned versions, reports an isolated interpreter, and treats
# every repository script invocation as a successful no-op.
UV_STUB = """
if [ "$1" = "--version" ]; then
  printf 'uv 0.12.23\\n'
elif [ "$1" = "sync" ]; then
  exit 0
elif [ "$1" = "run" ]; then
  case "$*" in
    *--version*) printf 'Python 3.13.12\\n' ;;
    *-c*) printf '/repo/.venv\\n/usr\\n' ;;
    *) exit 0 ;;
  esac
else
  exit 0
fi
"""

# A uv stub whose interpreter is a floating system install: the reported prefix
# and base prefix are identical, so the interpreter is not an isolated
# environment.
UV_FLOATING_PYTHON_STUB = """
if [ "$1" = "--version" ]; then
  printf 'uv 0.12.23\\n'
elif [ "$1" = "sync" ]; then
  exit 0
elif [ "$1" = "run" ]; then
  case "$*" in
    *--version*) printf 'Python 3.13.12\\n' ;;
    *-c*) printf '/usr\\n/usr\\n' ;;
    *) exit 0 ;;
  esac
else
  exit 0
fi
"""

# A cargo stub that records any real build/test invocation and fails it.
CARGO_MARKING_STUB = """
if [ "$1" = "--version" ]; then
  printf 'cargo 1.98.1 (stub)\\n'
  exit 0
fi
printf 'cargo invoked\\n' >> "$LITECOWORK_TEST_MARKER"
exit 1
"""

GIT_STUB = "exit 0\n"


class CheckScriptBehaviourTests(unittest.TestCase):
    def run_check(self, stubs, arguments=(), path=None, environment=None):
        with tempfile.TemporaryDirectory() as temporary_directory:
            bin_directory = Path(temporary_directory) / "bin"
            bin_directory.mkdir()
            for name, content in stubs.items():
                executable = bin_directory / name
                executable.write_text(f"#!/bin/sh\n{content}\n", encoding="utf-8")
                executable.chmod(0o755)
            marker = Path(temporary_directory) / "marker"
            environment_variables = os.environ.copy()
            environment_variables["LITECOWORK_TEST_MARKER"] = str(marker)
            if path is None:
                environment_variables["PATH"] = (
                    f"{bin_directory}:{environment_variables['PATH']}"
                )
            else:
                environment_variables["PATH"] = path
            if environment:
                environment_variables.update(environment)
            result = subprocess.run(
                ["bash", "scripts/check.sh", *arguments],
                cwd=ROOT,
                env=environment_variables,
                capture_output=True,
                text=True,
                check=False,
            )
            return result, marker.exists()

    def test_missing_pinned_tool_is_reported_before_any_work(self):
        result, _ = self.run_check(
            {},
            path="/usr/bin:/bin",
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Required pinned tool uv was not found on PATH.", result.stderr)
        self.assertIn("Install uv 0.12.23", result.stderr)
        self.assertNotIn("==>", result.stdout)

    def test_floating_python_interpreter_is_rejected(self):
        result, _ = self.run_check(
            {
                "uv": UV_FLOATING_PYTHON_STUB,
                "rustc": "printf 'rustc 1.98.1 (stub)\\n'\n",
                "cargo": "printf 'cargo 1.98.1 (stub)\\n'\n",
                "git": GIT_STUB,
            }
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "Expected an isolated pinned Python environment, found a floating interpreter",
            result.stderr,
        )

    def test_preflight_mode_never_starts_a_rust_build_and_reports_pins(self):
        result, cargo_was_invoked = self.run_check(
            {
                "uv": UV_STUB,
                "rustc": "printf 'rustc 1.98.1 (stub)\\n'\n",
                "cargo": CARGO_MARKING_STUB,
                "git": GIT_STUB,
            },
            arguments=["--preflight"],
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(cargo_was_invoked, "preflight mode must not build Rust code")
        self.assertIn("Rust: rustc 1.98.1 (stub)", result.stdout)
        self.assertIn("Cargo: cargo 1.98.1 (stub)", result.stdout)
        self.assertIn("uv: uv 0.12.23", result.stdout)
        self.assertIn("Python: 3.13.12", result.stdout)
        self.assertIn("Python environment: /repo/.venv", result.stdout)
        self.assertIn("==> Python tests", result.stdout)
        self.assertIn("==> Architecture validation", result.stdout)
        self.assertNotIn("==> Rust tests", result.stdout)

    def test_unsupported_check_argument_fails_without_running_the_gate(self):
        result, cargo_was_invoked = self.run_check(
            {
                "uv": UV_STUB,
                "rustc": "printf 'rustc 1.98.1 (stub)\\n'\n",
                "cargo": CARGO_MARKING_STUB,
                "git": GIT_STUB,
            },
            arguments=["--not-a-mode"],
        )

        self.assertEqual(result.returncode, 2)
        self.assertIn("unsupported argument --not-a-mode", result.stderr)
        self.assertFalse(cargo_was_invoked)

    def test_gate_reports_each_stage_with_its_duration(self):
        result, _ = self.run_check(
            {
                "uv": UV_STUB,
                "rustc": "printf 'rustc 1.98.1 (stub)\\n'\n",
                "cargo": "printf 'cargo 1.98.1 (stub)\\n'\n",
                "git": GIT_STUB,
            },
            arguments=["--preflight"],
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("==> Python tests", result.stdout)
        self.assertRegex(result.stdout, r"Python tests passed in \d+s")


if __name__ == "__main__":
    unittest.main()
