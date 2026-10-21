#!/usr/bin/env python3
"""Shared helpers for the LiteCowork local release probes.

The helpers build a *real* LiteCowork store from the repository's own schema
files, so the soak and resilience probes measure the shipped schema rather than
a synthetic one.

The schema is applied with the same pragmas the Rust migration chain uses
(`PRAGMA legacy_alter_table = ON` while the v4 table rebuild runs), because the
v4 migration drops and recreates `runtimes`.
"""

from __future__ import annotations

import hashlib
import json
import pathlib
import sqlite3
from typing import Any

REPOSITORY_ROOT = pathlib.Path(__file__).resolve().parent.parent
SCHEMA_DIRECTORY = REPOSITORY_ROOT / "docs" / "schemas"

WORKSPACE_ID = "soak-workspace"
OWNER_PRINCIPAL_ID = "owner-local"
RECORDED_AT = "2026-10-20T10:00:00Z"


def schema_files() -> list[pathlib.Path]:
    """The repository schema files in migration order."""
    files: list[tuple[int, pathlib.Path]] = []
    for path in sorted(SCHEMA_DIRECTORY.glob("sqlite-v*.sql")):
        files.append((int(path.stem.split("-v")[1]), path))
    files.sort(key=lambda item: item[0])
    return [path for _, path in files]


def current_schema_version() -> int:
    return int(schema_files()[-1].stem.split("-v")[1])


def build_store(directory: pathlib.Path) -> sqlite3.Connection:
    """Creates (or reopens) a real LiteCowork store from the schema files."""
    database = directory / "litecowork.sqlite3"
    connection = sqlite3.connect(database, isolation_level=None)
    connection.execute("PRAGMA journal_mode = WAL")
    if not _has_schema(database):
        for path in schema_files():
            # The v4 migration drops and recreates `runtimes`; the legacy alter
            # behaviour and the disabled foreign key enforcement are what the
            # Rust migration chain relies on while the table is rebuilt.
            connection.execute("PRAGMA foreign_keys = OFF")
            connection.execute("PRAGMA legacy_alter_table = ON")
            connection.executescript(path.read_text(encoding="utf-8"))
        connection.execute("PRAGMA legacy_alter_table = OFF")
        connection.execute(f"PRAGMA user_version = {current_schema_version()}")
    connection.execute("PRAGMA foreign_keys = ON")
    ensure_workspace(connection)
    return connection


def _has_schema(database: pathlib.Path) -> bool:
    """Whether the database already carries the migrated schema."""
    if not database.exists():
        return False
    try:
        connection = sqlite3.connect(database, isolation_level=None)
        row = connection.execute(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'"
        ).fetchone()
        connection.close()
    except sqlite3.Error:
        return False
    return bool(row and int(row[0]) > 0)


def ensure_workspace(connection: sqlite3.Connection) -> None:
    """Creates the single Workspace the probes write into."""
    connection.execute(
        "INSERT OR IGNORE INTO workspaces (workspace_id, name, owner_principal_id, "
        "replication_policy, status, created_at, updated_at, version) "
        "VALUES (?, 'Soak workspace', ?, 'LOCAL_ONLY', 'ACTIVE', ?, ?, 1)",
        (WORKSPACE_ID, OWNER_PRINCIPAL_ID, RECORDED_AT, RECORDED_AT),
    )


def digest(value: str) -> str:
    return "sha256:" + hashlib.sha256(value.encode("utf-8")).hexdigest()


def insert_event(connection: sqlite3.Connection, event_id: str, sequence: int) -> None:
    """Writes one domain event with the exact schema the store expects."""
    payload = json.dumps({"sequence": sequence, "event_id": event_id}, sort_keys=True)
    connection.execute(
        "INSERT INTO domain_events (event_id, workspace_id, entity_type, entity_id, "
        "origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, "
        "causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, "
        "payload_digest) VALUES (?, ?, 'Task', ?, 'runtime-local', ?, 1, ?, 'soak-correlation', "
        "NULL, 1, 'soak.task.started.v1', ?, ?, ?, ?)",
        (
            event_id,
            WORKSPACE_ID,
            f"task-{event_id}",
            sequence,
            RECORDED_AT,
            payload,
            json.dumps(
                {
                    "digest": digest(event_id),
                    "size_bytes": 0,
                    "media_type": "application/json",
                },
                sort_keys=True,
            ),
            RECORDED_AT,
            digest(payload),
        ),
    )


def count_events(connection: sqlite3.Connection) -> int:
    row = connection.execute("SELECT COUNT(*) FROM domain_events").fetchone()
    return int(row[0]) if row else 0


def footprint(directory: pathlib.Path) -> tuple[int, int]:
    """The database and write-ahead log sizes in bytes."""
    database = directory / "litecowork.sqlite3"
    wal = directory / "litecowork.sqlite3-wal"
    database_size = database.stat().st_size if database.exists() else 0
    wal_size = wal.stat().st_size if wal.exists() else 0
    return database_size, wal_size
