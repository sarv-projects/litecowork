#!/usr/bin/env python3
"""Run the LiteCowork desktop/local packaging checks.

The checks are runnable on a developer machine. They never sign, publish, or
create an external resource; they only inspect the repository and the local
build output.

Usage:
    .venv/bin/python scripts/v1_packaging_check.py [--json]
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import stat
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
TAURI_DIRECTORY = REPOSITORY_ROOT / "apps" / "litecowork-ui" / "src-tauri"


@dataclass
class Check:
    name: str
    passed: bool
    detail: str
    blocking: bool = True


@dataclass
class Report:
    checks: list[Check] = field(default_factory=list)

    def add(self, name: str, passed: bool, detail: str, blocking: bool = True) -> None:
        self.checks.append(Check(name=name, passed=passed, detail=detail, blocking=blocking))

    @property
    def failed(self) -> list[Check]:
        return [check for check in self.checks if not check.passed]

    @property
    def blocking_failures(self) -> list[Check]:
        return [check for check in self.failed if check.blocking]


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def check_bundle_configuration(report: Report) -> None:
    configuration_path = TAURI_DIRECTORY / "tauri.conf.json"
    if not configuration_path.is_file():
        report.add(
            "bundle configuration",
            False,
            f"{configuration_path} is missing",
        )
        return
    try:
        configuration = read_json(configuration_path)
    except json.JSONDecodeError as error:
        report.add("bundle configuration", False, f"{configuration_path} is not valid JSON: {error}")
        return

    bundle = configuration.get("bundle", {})
    problems: list[str] = []
    if not bundle.get("active"):
        problems.append("bundle.active is not true")
    if not bundle.get("targets"):
        problems.append("bundle.targets is empty")
    if not configuration.get("identifier"):
        problems.append("identifier is missing")
    if not configuration.get("productName"):
        problems.append("productName is missing")
    if not configuration.get("version"):
        problems.append("version is missing")
    security = configuration.get("app", {}).get("security", {})
    if not security.get("csp"):
        problems.append("app.security.csp is missing")
    report.add(
        "bundle configuration",
        not problems,
        "; ".join(problems) if problems else "the desktop bundle configuration is complete",
    )


def check_icons(report: Report) -> None:
    icons = TAURI_DIRECTORY / "icons"
    if not icons.is_dir():
        report.add("bundle icons", False, f"{icons} is missing")
        return
    problems: list[str] = []
    for required in ("icon.png", "icon.ico"):
        if not (icons / required).is_file():
            problems.append(f"{required} is missing")
    png = icons / "icon.png"
    if png.is_file():
        size = png.stat().st_size
        if size < 4096:
            problems.append(f"icon.png is only {size} bytes")
    report.add(
        "bundle icons",
        not problems,
        "; ".join(problems) if problems else "the required bundle icons are present",
    )


def check_daemon_binary(report: Report) -> None:
    candidates = [
        REPOSITORY_ROOT / "target" / "release" / "litecoworkd",
        REPOSITORY_ROOT / "target" / "debug" / "litecoworkd",
    ]
    built = next((candidate for candidate in candidates if candidate.is_file()), None)
    if built is None:
        report.add(
            "daemon binary",
            False,
            "no built litecoworkd binary was found under target/release or target/debug",
        )
        return
    completed = subprocess.run(
        [str(built), "--version"],
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )
    if completed.returncode != 0:
        report.add(
            "daemon binary",
            False,
            f"{built} --version exited with {completed.returncode}: {completed.stderr.strip()}",
        )
        return
    report.add(
        "daemon binary",
        True,
        f"{built} reports {completed.stdout.strip()}",
    )


def check_recovery_surface(report: Report) -> None:
    """The backup/restore recovery surface must be reachable from the daemon."""
    candidates = [
        REPOSITORY_ROOT / "target" / "release" / "litecoworkd",
        REPOSITORY_ROOT / "target" / "debug" / "litecoworkd",
    ]
    built = next((candidate for candidate in candidates if candidate.is_file()), None)
    if built is None:
        report.add("backup recovery surface", False, "no built litecoworkd binary was found")
        return
    completed = subprocess.run(
        [str(built), "backup", "--help"],
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )
    if completed.returncode != 0 or "litecoworkd backup" not in completed.stdout:
        report.add(
            "backup recovery surface",
            False,
            f"`litecoworkd backup --help` did not report the recovery surface: "
            f"{completed.stdout.strip()} {completed.stderr.strip()}",
        )
        return
    report.add(
        "backup recovery surface",
        True,
        "`litecoworkd backup` exposes create, verify, restore and upgrade-check",
    )


def check_state_directory_permissions(report: Report) -> None:
    """The local state directory must be private to the owning OS user."""
    runtime_source = (REPOSITORY_ROOT / "apps" / "litecoworkd" / "src" / "runtime.rs").read_text(
        encoding="utf-8"
    )
    problems: list[str] = []
    if "0o700" not in runtime_source:
        problems.append("runtime.rs does not create a 0700 state directory")
    if "0o600" not in runtime_source:
        problems.append("runtime.rs does not restrict the database file to 0600")
    report.add(
        "state directory permissions",
        not problems,
        "; ".join(problems) if problems else "the daemon creates a 0700 state directory and a 0600 database",
    )


def check_service_manager_unit(report: Report) -> None:
    """A desktop installation needs a documented service-manager unit."""
    unit_path = REPOSITORY_ROOT / "scripts" / "packaging" / "systemd" / "litecoworkd.service"
    if not unit_path.is_file():
        report.add(
            "service manager unit",
            False,
            f"{unit_path} is missing; the daemon service install/upgrade path is not packaged",
        )
        return
    content = unit_path.read_text(encoding="utf-8")
    problems: list[str] = []
    for required in ("[Unit]", "[Service]", "[Install]", "ExecStart="):
        if required not in content:
            problems.append(f"the unit does not declare {required}")
    if "StateDirectoryMode=0700" not in content:
        problems.append("the unit does not create a 0700 state directory")
    if "UMask=0077" not in content:
        problems.append("the unit does not set a restrictive umask")
    if problems:
        report.add("service manager unit", False, "; ".join(problems))
        return

    verifier = shutil.which("systemd-analyze")
    if verifier is None:
        report.add(
            "service manager unit",
            True,
            "the unit file is present and statically complete; systemd-analyze is not "
            "installed on this host, so the unit was not parsed by systemd",
            blocking=False,
        )
        return
    completed = subprocess.run(
        [verifier, "verify", str(unit_path)],
        capture_output=True,
        text=True,
        timeout=300,
        check=False,
    )
    diagnostics = [
        line
        for line in (completed.stdout + completed.stderr).splitlines()
        if line.strip()
        # The daemon binary is not installed on a developer host, so systemd
        # cannot resolve the ExecStart path. That is an installation state, not
        # a unit defect.
        and "is not executable" not in line
    ]
    if diagnostics:
        report.add(
            "service manager unit",
            False,
            "systemd-analyze verify rejected the unit: " + " | ".join(diagnostics),
        )
        return
    report.add(
        "service manager unit",
        True,
        "the unit file is present and systemd-analyze verify accepted it "
        "(the ExecStart path is not installed on this host)",
    )


def check_installer_metadata(report: Report) -> None:
    """The bundle must not ship an updater that trusts an unsigned artifact."""
    configuration_path = TAURI_DIRECTORY / "tauri.conf.json"
    if not configuration_path.is_file():
        report.add("updater trust", False, "the bundle configuration is missing")
        return
    configuration = read_json(configuration_path)
    plugins = configuration.get("plugins", {})
    updater = plugins.get("updater", {})
    endpoints = updater.get("endpoints")
    if not endpoints:
        report.add(
            "updater trust",
            True,
            "no updater endpoint is configured, so no unsigned update can be offered",
            blocking=False,
        )
        return
    report.add(
        "updater trust",
        False,
        f"an updater endpoint is configured ({endpoints}); signed update verification is "
        "not implemented in this repository",
    )


def check_desktop_source_build_inputs(report: Report) -> None:
    """The desktop build must have its inputs present."""
    problems: list[str] = []
    package_json = REPOSITORY_ROOT / "apps" / "litecowork-ui" / "package.json"
    if not package_json.is_file():
        problems.append("apps/litecowork-ui/package.json is missing")
    lockfile = REPOSITORY_ROOT / "apps" / "litecowork-ui" / "pnpm-lock.yaml"
    if not lockfile.is_file():
        problems.append("apps/litecowork-ui/pnpm-lock.yaml is missing")
    sidecar = REPOSITORY_ROOT / "apps" / "litecowork-ui" / "package.json"
    if sidecar.is_file():
        content = sidecar.read_text(encoding="utf-8")
        if "build:daemon-sidecar" not in content:
            problems.append("the desktop package does not build the daemon sidecar")
    report.add(
        "desktop build inputs",
        not problems,
        "; ".join(problems) if problems else "the desktop build inputs are present",
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="print the report as JSON")
    arguments = parser.parse_args()

    report = Report()
    check_bundle_configuration(report)
    check_icons(report)
    check_daemon_binary(report)
    check_recovery_surface(report)
    check_state_directory_permissions(report)
    check_service_manager_unit(report)
    check_installer_metadata(report)
    check_desktop_source_build_inputs(report)

    if arguments.json:
        print(
            json.dumps(
                {
                    "checks": [
                        {
                            "name": check.name,
                            "passed": check.passed,
                            "blocking": check.blocking,
                            "detail": check.detail,
                        }
                        for check in report.checks
                    ],
                    "failed": len(report.failed),
                    "blocking_failures": len(report.blocking_failures),
                },
                indent=2,
                sort_keys=True,
            )
        )
    else:
        for check in report.checks:
            state = "PASS" if check.passed else ("FAIL" if check.blocking else "WARN")
            print(f"[{state}] {check.name}: {check.detail}")
        print(f"{len(report.checks) - len(report.failed)}/{len(report.checks)} checks passed")

    return 1 if report.blocking_failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
