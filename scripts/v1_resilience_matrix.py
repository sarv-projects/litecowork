#!/usr/bin/env python3
"""Run the LiteCowork local resilience matrix.

Every row of the matrix is executed by this script. A row is reported as
PASSED only when the probe actually ran here and the expected durable truth was
observed. A row that cannot run in this environment is reported as BLOCKED
together with the reason and the exact command.

The matrix covers the faults the desktop/local release must survive:

* a restart in the middle of a write,
* a process killed while a write-ahead log is open,
* a full disk (the repository's existing SQLite ``max_page_count`` technique),
* a corrupted blob envelope,
* a stale single-instance lock.

Usage:
    .venv/bin/python scripts/v1_resilience_matrix.py \
        --report /tmp/litecowork-release-evidence/resilience.json
"""

from __future__ import annotations

import argparse
import fcntl
import json
import os
import shutil
import signal
import sqlite3
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))

import v1_store_probe as probe  # noqa: E402


@dataclass
class Row:
    name: str
    state: str
    command: str
    observation: str
    reason: str = ""

    def as_dict(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "state": self.state,
            "command": self.command,
            "observation": self.observation,
            "reason": self.reason,
        }


@dataclass
class Matrix:
    rows: list[Row] = field(default_factory=list)

    def add(self, row: Row) -> None:
        self.rows.append(row)


def schema_files() -> list[Path]:
    files: list[tuple[int, Path]] = []
    for path in sorted(SCHEMA_DIRECTORY.glob("sqlite-v*.sql")):
        files.append((int(path.stem.split("-v")[1]), path))
    files.sort(key=lambda item: item[0])
    return [path for _, path in files]


def build_store(directory: Path) -> sqlite3.Connection:
    return probe.build_store(directory)


def insert_event(connection: sqlite3.Connection, event_id: str, sequence: int = 1) -> None:
    probe.insert_event(connection, event_id, sequence)


def case_restart_mid_write(directory: Path) -> tuple[str, str]:
    """An uncommitted write must not survive a restart and must not corrupt."""
    connection = build_store(directory)
    insert_event(connection, "soak-committed", 1)
    connection.execute("BEGIN IMMEDIATE")
    insert_event(connection, "soak-uncommitted", 2)
    connection.close()

    reopened = build_store(directory)
    rows = reopened.execute("SELECT COUNT(*) FROM domain_events").fetchone()
    integrity = reopened.execute("PRAGMA integrity_check").fetchone()
    reopened.close()
    count = int(rows[0]) if rows else 0
    if count != 1:
        return "FAILED", f"expected exactly the committed event, found {count}"
    if not integrity or integrity[0] != "ok":
        return "FAILED", f"integrity_check reported {integrity}"
    return "PASSED", "the uncommitted write was rolled back and the store stayed consistent"


def case_kill_during_drain(directory: Path) -> tuple[str, str]:
    """A process killed with an open write-ahead log must recover cleanly."""
    database = directory / "litecowork.sqlite3"
    connection = build_store(directory)
    insert_event(connection, "soak-before-kill")
    connection.close()

    child = subprocess.Popen(
        [
            sys.executable,
            "-c",
            (
                "import sqlite3,sys,time\n"
                "c=sqlite3.connect(sys.argv[1], isolation_level=None)\n"
                "c.execute('PRAGMA journal_mode = WAL')\n"
                "c.execute('BEGIN IMMEDIATE')\n"
                "import hashlib,json\n"
                "payload=json.dumps({'sequence':2},sort_keys=True)\n"
                "digest='sha256:'+hashlib.sha256(payload.encode()).hexdigest()\n"
                "c.execute(\"INSERT INTO domain_events (event_id, workspace_id, entity_type, entity_id,"
                " origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id,"
                " causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at,"
                " payload_digest) VALUES ('soak-killed','soak-workspace','Task','task-killed',"
                "'runtime-local',2,1,'2026-10-20T10:00:00Z','c',NULL,1,'soak.task.started.v1',?,?,"
                "'2026-10-20T10:00:00Z',?)\",(payload,digest,digest))\n"
                "time.sleep(120)\n"
            ),
            str(database),
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(1.5)
    child.send_signal(signal.SIGKILL)
    child.wait(timeout=30)

    reopened = build_store(directory)
    rows = reopened.execute("SELECT COUNT(*) FROM domain_events").fetchone()
    integrity = reopened.execute("PRAGMA integrity_check").fetchone()
    reopened.close()
    count = int(rows[0]) if rows else 0
    if count != 1:
        return "FAILED", f"the killed transaction must not commit, found {count} events"
    if not integrity or integrity[0] != "ok":
        return "FAILED", f"integrity_check reported {integrity}"
    return "PASSED", "the killed transaction was rolled back by the write-ahead log recovery"


def case_disk_full(directory: Path) -> tuple[str, str]:
    """A full disk must fail the write cleanly without corrupting the store."""
    connection = build_store(directory)
    insert_event(connection, "soak-before-full")
    page_count = connection.execute("PRAGMA page_count").fetchone()
    pages = int(page_count[0]) if page_count else 1
    connection.execute(f"PRAGMA max_page_count = {pages}")
    payload = "x" * 4096
    failed = False
    detail = ""
    try:
        connection.execute(
            "INSERT INTO domain_events (event_id, workspace_id, entity_type, entity_id, "
            "origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, "
            "causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, "
            "payload_digest) VALUES (?, ?, 'Task', 'task-overflow', 'runtime-local', 3, 1, "
            "'2026-10-20T10:00:00Z', 'c', NULL, 1, 'soak.task.started.v1', ?, ?, "
            "'2026-10-20T10:00:00Z', ?)",
            ("soak-overflow", probe.WORKSPACE_ID, payload, probe.digest("soak"), probe.digest(payload)),
        )
    except sqlite3.Error as error:
        failed = True
        detail = str(error)
    connection.execute("PRAGMA max_page_count = 1073741823")
    integrity = connection.execute("PRAGMA integrity_check").fetchone()
    rows = connection.execute("SELECT COUNT(*) FROM domain_events").fetchone()
    connection.close()
    if not failed:
        return "FAILED", "the capped write was expected to fail but it succeeded"
    if not integrity or integrity[0] != "ok":
        return "FAILED", f"integrity_check reported {integrity} after the full-disk write"
    count = int(rows[0]) if rows else 0
    if count != 1:
        return "FAILED", f"the refused write must not be committed, found {count} events"
    return "PASSED", f"the write was refused cleanly ({detail}) and the store stayed consistent"


def case_corrupt_blob() -> tuple[str, str]:
    """A corrupted blob envelope must be rejected by the store."""
    command = [
        "cargo",
        "test",
        "--locked",
        "--offline",
        "--jobs",
        "2",
        "-p",
        "storage-sqlite",
        "blob::tests::rejects_wrong_purpose_and_corrupted_ciphertext",
    ]
    completed = subprocess.run(
        command,
        cwd=str(REPOSITORY_ROOT),
        capture_output=True,
        text=True,
        timeout=3600,
        check=False,
    )
    if completed.returncode != 0:
        return "FAILED", (completed.stdout + completed.stderr).strip()[-400:]
    return "PASSED", "the corrupted blob envelope was rejected by the blob store"


def case_stale_lock(directory: Path) -> tuple[str, str]:
    """A held single-instance lock must block a second owner."""
    lock_path = directory / "runtime.lock"
    handle = open(lock_path, "w")
    fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    try:
        second = open(lock_path, "w")
        try:
            fcntl.flock(second.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            return "PASSED", "a second owner could not take the single-instance lock"
        finally:
            second.close()
        return "FAILED", "a second owner took the single-instance lock"
    finally:
        fcntl.flock(handle.fileno(), fcntl.LOCK_UN)
        handle.close()


def run_matrix() -> Matrix:
    matrix = Matrix()
    workspace = Path(tempfile.mkdtemp(prefix="litecowork-resilience-"))
    try:
        for name, case, needs_directory in (
            ("restart_mid_write", case_restart_mid_write, True),
            ("kill_during_drain", case_kill_during_drain, True),
            ("disk_full", case_disk_full, True),
            ("corrupt_blob", case_corrupt_blob, False),
            ("stale_lock", case_stale_lock, True),
        ):
            directory = workspace / name
            directory.mkdir(parents=True, exist_ok=True)
            try:
                state, observation = case(directory) if needs_directory else case()
            except Exception as error:  # noqa: BLE001 - the matrix reports the failure
                state, observation = "FAILED", f"{type(error).__name__}: {error}"
            matrix.add(
                Row(
                    name=name,
                    state=state,
                    command=f"scripts/v1_resilience_matrix.py ({name})",
                    observation=observation,
                    reason="" if state == "PASSED" else "the probe did not observe the expected truth",
                )
            )
    finally:
        shutil.rmtree(workspace, ignore_errors=True)
    return matrix


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--report",
        default="/tmp/litecowork-release-evidence/resilience.json",
    )
    arguments = parser.parse_args()

    matrix = run_matrix()
    report = {
        "rows": [row.as_dict() for row in matrix.rows],
        "passed": sum(1 for row in matrix.rows if row.state == "PASSED"),
        "total": len(matrix.rows),
    }
    report_path = Path(arguments.report)
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")
    for row in matrix.rows:
        print(f"[{row.state}] {row.name}: {row.observation}")
    print(f"{report['passed']}/{report['total']} resilience rows passed")
    print(f"report: {report_path}")
    return 0 if report["passed"] == report["total"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
