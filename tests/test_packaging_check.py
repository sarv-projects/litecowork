from __future__ import annotations

import json
import subprocess
import unittest
from pathlib import Path
from unittest.mock import patch

import scripts.v1_packaging_check as packaging_check


class PackagingCheckTests(unittest.TestCase):
    def test_windows_binary_candidates_include_the_tauri_sidecar(self) -> None:
        candidates = packaging_check.daemon_binary_candidates("win32")

        self.assertIn(
            packaging_check.TAURI_DIRECTORY
            / "binaries"
            / "litecoworkd-x86_64-pc-windows-msvc.exe",
            candidates,
        )
        self.assertIn(
            packaging_check.REPOSITORY_ROOT / "target" / "release" / "litecoworkd.exe",
            candidates,
        )

    def test_linux_binary_candidates_use_the_linux_sidecar_name(self) -> None:
        candidates = packaging_check.daemon_binary_candidates("linux")

        self.assertIn(
            packaging_check.TAURI_DIRECTORY
            / "binaries"
            / "litecoworkd-x86_64-unknown-linux-gnu",
            candidates,
        )
        self.assertIn(
            packaging_check.REPOSITORY_ROOT / "target" / "release" / "litecoworkd",
            candidates,
        )

    @patch.object(packaging_check.subprocess, "run")
    def test_permission_check_requires_both_daemon_attestations(self, run: object) -> None:
        run.return_value = subprocess.CompletedProcess(
            args=["litecoworkd", "self-test-state-directory"],
            returncode=0,
            stdout=json.dumps(
                {"state_directory_private": True, "state_file_private": True}
            ),
            stderr="",
        )
        report = packaging_check.Report()

        packaging_check.check_state_directory_permissions(report, Path("litecoworkd"))

        self.assertEqual(len(report.checks), 1)
        self.assertTrue(report.checks[0].passed)

    @patch.object(packaging_check.subprocess, "run")
    def test_permission_check_rejects_missing_or_failed_attestation(self, run: object) -> None:
        run.return_value = subprocess.CompletedProcess(
            args=["litecoworkd", "self-test-state-directory"],
            returncode=1,
            stdout=json.dumps(
                {"state_directory_private": True, "state_file_private": False}
            ),
            stderr="",
        )
        report = packaging_check.Report()

        packaging_check.check_state_directory_permissions(report, Path("litecoworkd"))

        self.assertEqual(len(report.checks), 1)
        self.assertFalse(report.checks[0].passed)


if __name__ == "__main__":
    unittest.main()
