import os
import shlex
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import yaml

from scripts import validate_architecture
from scripts import audit_v1_contracts


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
        return self.run_with_stubs({executable_name: content})

    def run_with_stubs(self, executables):
        with tempfile.TemporaryDirectory() as temporary_directory:
            bin_directory = Path(temporary_directory)
            for executable_name, content in executables.items():
                executable = bin_directory / executable_name
                executable.write_text(
                    f"#!/bin/sh\n{content}\n", encoding="utf-8"
                )
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

    def test_wrong_cargo_version_fails_before_uv_sync(self):
        result = self.run_with_stubs(
            {
                "cargo": 'if [ "$1" = "--version" ]; then '
                'printf "cargo 0.0.0 (stub)\\n"; else '
                'printf "cargo unexpectedly invoked\\n" >&2; exit 99; fi',
                "uv": 'if [ "$1" = "--version" ]; then '
                'printf "uv 0.12.23\\n"; else '
                'printf "uv unexpectedly invoked\\n" >&2; exit 98; fi',
            }
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Expected Cargo 1.98.1, found 0.0.0", result.stderr)
        self.assertNotIn("uv unexpectedly invoked", result.stderr)

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

    def test_ci_installs_linux_dbus_build_dependencies(self):
        root = Path(__file__).resolve().parents[1]
        workflow = yaml.safe_load(
            (root / ".github/workflows/architecture-docs.yml").read_text(
                encoding="utf-8"
            )
        )
        steps = workflow["jobs"]["check"]["steps"]
        dbus_step = next(
            (
                step
                for step in steps
                if "libdbus-1-dev" in step.get("run", "")
            ),
            None,
        )

        self.assertIsNotNone(
            dbus_step, "CI must install libdbus-1-dev before Cargo builds"
        )
        self.assertIn("pkg-config", dbus_step["run"])
        check_index = next(
            index
            for index, step in enumerate(steps)
            if step.get("run") == "scripts/check.sh"
        )
        self.assertLess(steps.index(dbus_step), check_index)

    def test_ci_installs_bubblewrap_for_process_scope_contract_tests(self):
        root = Path(__file__).resolve().parents[1]
        workflow = yaml.safe_load(
            (root / ".github/workflows/architecture-docs.yml").read_text(
                encoding="utf-8"
            )
        )
        steps = workflow["jobs"]["check"]["steps"]
        native_dependencies = next(
            (
                step
                for step in steps
                if "Install Linux native build dependencies" == step.get("name")
            ),
            None,
        )

        self.assertIsNotNone(
            native_dependencies,
            "CI must declare native dependencies required by workspace tests",
        )
        self.assertIn("bubblewrap", native_dependencies["run"])
        check_index = next(
            index
            for index, step in enumerate(steps)
            if step.get("run") == "scripts/check.sh"
        )
        self.assertLess(steps.index(native_dependencies), check_index)


class CICoverageContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = Path(__file__).resolve().parents[1]
        cls.workflow = yaml.safe_load(
            (cls.root / ".github/workflows/architecture-docs.yml").read_text(
                encoding="utf-8"
            )
        )

    def test_ui_job_uses_pinned_node_pnpm_install_tests_and_production_build(self):
        job = self.workflow["jobs"].get("ui")
        self.assertIsNotNone(job, "CI must test and build the desktop UI independently")
        steps = job["steps"]
        node_step = next(
            step for step in steps if step.get("uses", "").startswith("actions/setup-node@")
        )
        self.assertEqual(
            node_step["uses"],
            "actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020",
        )
        self.assertEqual(
            node_step["with"]["node-version-file"], "apps/litecowork-ui/.node-version"
        )
        commands = [step.get("run", "") for step in steps]
        self.assertIn("corepack pnpm@12.10.1 install --frozen-lockfile", commands)
        self.assertIn("corepack pnpm@12.10.1 test", commands)
        self.assertIn("corepack pnpm@12.10.1 build", commands)
        self.assertLess(
            commands.index("corepack pnpm@12.10.1 install --frozen-lockfile"),
            commands.index("corepack pnpm@12.10.1 test"),
        )
        self.assertLess(
            commands.index("corepack pnpm@12.10.1 test"),
            commands.index("corepack pnpm@12.10.1 build"),
        )
        self.assertEqual(
            job["defaults"]["run"]["working-directory"], "apps/litecowork-ui"
        )

    def test_tauri_job_tests_separate_workspace_with_linux_prerequisites(self):
        job = self.workflow["jobs"].get("tauri")
        self.assertIsNotNone(job, "CI must test the separate Tauri Cargo workspace")
        steps = job["steps"]
        rust_step = next(
            step
            for step in steps
            if step.get("uses", "").startswith(
                "actions-rust-lang/setup-rust-toolchain@"
            )
        )
        self.assertEqual(
            rust_step["uses"],
            "actions-rust-lang/setup-rust-toolchain@ecabd13d1c56bd1345c230e542e9144811ad706f",
        )
        dependencies = next(
            step
            for step in steps
            if step.get("name") == "Install Tauri Linux native dependencies"
        )["run"]
        for package in (
            "libwebkit2gtk-4.1-dev",
            "build-essential",
            "curl",
            "file",
            "libxdo-dev",
            "libssl-dev",
            "libayatana-appindicator3-dev",
            "libdbus-1-dev",
            "librsvg2-dev",
            "pkg-config",
            "wget",
        ):
            with self.subTest(package=package):
                self.assertIn(package, dependencies)
        cargo_test = next(
            step.get("run", "") for step in steps if "cargo test" in step.get("run", "")
        )
        self.assertIn(
            "--manifest-path apps/litecowork-ui/src-tauri/Cargo.toml", cargo_test
        )
        self.assertIn("--workspace", cargo_test)
        self.assertIn("--locked", cargo_test)
        dependency_index = next(
            i
            for i, step in enumerate(steps)
            if step.get("name") == "Install Tauri Linux native dependencies"
        )
        test_index = next(
            i for i, step in enumerate(steps) if "cargo test" in step.get("run", "")
        )
        self.assertLess(dependency_index, test_index)

    def test_root_ci_runs_strict_v1_audits_and_checks_generated_output_freshness(self):
        script = (self.root / "scripts/check.sh").read_text(encoding="utf-8")
        for command in (
            "scripts/audit_v1_contracts.py --strict --write-report",
            "scripts/audit_v1_design_depth.py --strict --write",
            "scripts/audit_v1_markdown.py",
            "scripts/validate_design_owner_consistency.py",
            "scripts/validate_implementation_coverage.py",
        ):
            with self.subTest(command=command):
                self.assertIn(command, script)
        self.assertIn("run_freshness_check", script)
        self.assertIn("implementation/V1-CONTRACT-CHECK.md", script)
        self.assertIn("implementation/V1-DESIGN-DEPTH-AUDIT.md", script)
        self.assertIn("implementation/V1-AUDIT-INVENTORY.md", script)
        self.assertIn("implementation/V1-MARKDOWN-INVENTORY.csv", script)
        self.assertIn("implementation/V1-CROSS-CONTRACT-GUARD.json", script)
        self.assertLess(
            script.index("scripts/audit_v1_contracts.py --strict --write-report"),
            script.index("scripts/audit_v1_markdown.py"),
        )
        self.assertLess(
            script.index("scripts/audit_v1_markdown.py"),
            script.index("scripts/validate_implementation_coverage.py --write"),
        )
        self.assertEqual(
            script.count("uv run --locked python scripts/audit_v1_markdown.py"),
            2,
            "Markdown freshness must be rechecked after coverage regeneration updates its input document",
        )
        self.assertLess(
            script.index("scripts/validate_implementation_coverage.py --write"),
            script.rindex("scripts/audit_v1_markdown.py"),
        )
        self.assertLess(
            script.index("scripts/validate_implementation_coverage.py --write"),
            script.index("scripts/audit_v1_design_depth.py --strict --write"),
        )
        self.assertLess(
            script.index("scripts/audit_v1_design_depth.py --strict --write"),
            script.index("scripts/validate_design_owner_consistency.py"),
        )
        self.assertLess(
            script.index("scripts/validate_implementation_coverage.py --write"),
            script.index("scripts/validate_implementation_plan.py"),
        )
        self.assertLess(
            script.index("scripts/validate_implementation_plan.py"),
            script.index("scripts/validate_architecture.py"),
        )
        self.assertIn(
            "actions-rust-lang/setup-rust-toolchain@ecabd13d1c56bd1345c230e542e9144811ad706f",
            (self.root / ".github/workflows/architecture-docs.yml").read_text(
                encoding="utf-8"
            ),
        )

    def test_contract_audit_labels_openapi_operations_as_specification_only(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            isolated_root = Path(temporary_directory)
            source_paths = (
                "docs/schemas/operator-api.openapi.yaml",
                "docs/V1-DESIGN-INDEX.md",
                "docs/NATIVE-COMPOSER-LLD.md",
                "docs/CHROME-BROWSER-LLD.md",
                "docs/DESKTOP-SHELL-LLD.md",
                "docs/RENDERING-CONFORMANCE-LLD.md",
                "docs/V1-FUNCTION-FILE-CATALOG.md",
                "docs/STATE-MACHINES.md",
                "docs/V1-WORKFLOW-EVENT-LLD.md",
                "apps/litecowork-ui/src/conversations/ConversationsPage.tsx",
                "implementation/V1-DESIGN-COVERAGE.md",
                "implementation/TESTING.md",
                "implementation/backlog.json",
            )
            for relative in source_paths:
                destination = isolated_root / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes((self.root / relative).read_bytes())

            with (
                patch.object(audit_v1_contracts, "ROOT", isolated_root),
                patch.object(sys, "argv", ["audit_v1_contracts.py", "--write-report"]),
            ):
                self.assertEqual(audit_v1_contracts.main(), 0)

            report = (isolated_root / "implementation/V1-CONTRACT-CHECK.md").read_text(
                encoding="utf-8"
            )
        self.assertIn("# V1 Contract Specification and Backlog Audit", report)
        self.assertIn("OpenAPI operations specified (not served-route count)", report)
        self.assertIn("| Method | Route | In OpenAPI? | OperationId |", report)
        self.assertIn("not counts of mounted daemon handlers", report)
        self.assertIn("V1-SERVED-CAPABILITY-INVENTORY.md", report)
        self.assertNotIn("Actual OpenAPI?", report)

    def test_freshness_helper_detects_stale_output_and_restores_snapshot(self):
        check_script = (self.root / "scripts/check.sh").read_text(encoding="utf-8")
        function_start = check_script.index("run_freshness_check() {")
        function_end = check_script.index("\n}\n", function_start) + 2
        function = check_script[function_start:function_end]

        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary = Path(temporary_directory)
            output = temporary / "generated.txt"
            output.write_text("reviewed\n", encoding="utf-8")
            same_output = temporary / "same-output.sh"
            same_output.write_text("printf 'reviewed\\n' > generated.txt\n", encoding="utf-8")
            stale_output = temporary / "stale-output.sh"
            stale_output.write_text("printf 'stale\\n' > generated.txt\n", encoding="utf-8")
            driver = temporary / "driver.sh"
            driver.write_text(
                "\n".join(
                    (
                        "#!/usr/bin/env bash",
                        "set -Eeuo pipefail",
                        f"cd {shlex.quote(str(temporary))}",
                        function,
                        "run_freshness_check generated.txt -- bash "
                        + shlex.quote(str(same_output)),
                        "if run_freshness_check generated.txt -- bash "
                        + shlex.quote(str(stale_output))
                        + "; then exit 1; fi",
                        "[[ \"$(<generated.txt)\" == reviewed ]]",
                    )
                )
                + "\n",
                encoding="utf-8",
            )

            result = subprocess.run(
                ["bash", str(driver)], capture_output=True, text=True, check=False
            )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Generated output is stale", result.stderr)


if __name__ == "__main__":
    unittest.main()
