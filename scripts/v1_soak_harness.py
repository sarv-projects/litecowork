#!/usr/bin/env python3
"""Bounded local soak harness for the LiteCowork desktop store.

The harness is deterministic and bounded:

* the workload is driven by an explicit ``--seed``;
* the run stops after ``--duration-seconds`` or ``--max-iterations``;
* the store is a real SQLite database built from the repository's own schema
  files, so the measured latencies are SQLite latencies on the shipped schema;
* the harness asserts that the on-disk footprint stays bounded and that the
  database still passes ``PRAGMA integrity_check`` at the end.

The harness never grows without bound: it truncates the write-ahead log on a
fixed cadence and it records the observed file sizes so a regression is visible.

Usage:
    .venv/bin/python scripts/v1_soak_harness.py \
        --duration-seconds 30 --seed 20261020 \
        --report /tmp/litecowork-release-evidence/soak.json
"""

from __future__ import annotations

import argparse
import json
import pathlib
import random
import shutil
import statistics
import sys
import tempfile
import time
from typing import Any

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import v1_store_probe as probe  # noqa: E402


def workload(connection, rng: random.Random, index: int) -> None:
    """Writes one deterministic batch of domain events."""
    connection.execute("BEGIN IMMEDIATE")
    try:
        for offset in range(8):
            sequence = index * 8 + offset
            payload = json.dumps({"sequence": sequence, "nonce": rng.random()}, sort_keys=True)
            connection.execute(
                "INSERT INTO domain_events (event_id, workspace_id, entity_type, entity_id, "
                "origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, "
                "causation_id, schema_version, type, payload_json, aggregate_state_ref_json, "
                "recorded_at, payload_digest) VALUES (?, ?, 'Task', ?, 'runtime-local', ?, 1, ?, "
                "'soak-correlation', NULL, 1, 'soak.task.started.v1', ?, ?, ?, ?)",
                (
                    f"soak-event-{sequence:012d}",
                    probe.WORKSPACE_ID,
                    f"task-{sequence:012d}",
                    sequence,
                    probe.RECORDED_AT,
                    payload,
                    json.dumps(
                        {
                            "digest": probe.digest(f"soak-{sequence}"),
                            "size_bytes": 0,
                            "media_type": "application/json",
                        },
                        sort_keys=True,
                    ),
                    probe.RECORDED_AT,
                    probe.digest(payload),
                ),
            )
        connection.execute("COMMIT")
    except Exception:
        connection.execute("ROLLBACK")
        raise


def run(arguments: argparse.Namespace) -> dict[str, Any]:
    rng = random.Random(arguments.seed)
    directory = (
        pathlib.Path(arguments.state_dir)
        if arguments.state_dir
        else pathlib.Path(tempfile.mkdtemp(prefix="litecowork-soak-"))
    )
    directory.mkdir(parents=True, exist_ok=True)
    measurement: dict[str, Any] = {
        "iterations": 0,
        "restarts": 0,
        "latencies_ms": [],
        "max_database_bytes": 0,
        "max_wal_bytes": 0,
        "failures": [],
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }
    try:
        connection = probe.build_store(directory)
        deadline = time.monotonic() + arguments.duration_seconds
        index = 0
        while index < arguments.max_iterations and time.monotonic() < deadline:
            started = time.perf_counter()
            try:
                workload(connection, rng, index)
            except Exception as error:  # noqa: BLE001 - the report records it
                measurement["failures"].append(f"iteration {index} failed: {error}")
                break
            measurement["latencies_ms"].append((time.perf_counter() - started) * 1000.0)
            measurement["iterations"] += 1
            index += 1

            if arguments.restart_every and index % arguments.restart_every == 0:
                connection.close()
                connection = probe.build_store(directory)
                measurement["restarts"] += 1
                connection.execute("PRAGMA wal_checkpoint(TRUNCATE)")

            if arguments.checkpoint_every and index % arguments.checkpoint_every == 0:
                connection.execute("PRAGMA wal_checkpoint(TRUNCATE)")

            database_size, wal_size = probe.footprint(directory)
            measurement["max_database_bytes"] = max(
                measurement["max_database_bytes"], database_size
            )
            measurement["max_wal_bytes"] = max(measurement["max_wal_bytes"], wal_size)

        measurement["committed_rows"] = probe.count_events(connection)
        connection.execute("PRAGMA wal_checkpoint(TRUNCATE)")
        integrity = connection.execute("PRAGMA integrity_check").fetchone()
        if not integrity or integrity[0] != "ok":
            measurement["failures"].append(f"integrity_check reported {integrity}")
        connection.close()
        measurement["finished_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        return measurement
    finally:
        if not arguments.state_dir:
            shutil.rmtree(directory, ignore_errors=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--duration-seconds", type=float, default=30.0)
    parser.add_argument("--max-iterations", type=int, default=100_000)
    parser.add_argument("--seed", type=int, default=20261020)
    parser.add_argument("--restart-every", type=int, default=50)
    parser.add_argument("--checkpoint-every", type=int, default=25)
    parser.add_argument(
        "--max-bytes-per-iteration",
        type=int,
        default=64 * 1024,
        help="the declared bound on the database growth per workload iteration",
    )
    parser.add_argument("--state-dir", default=None)
    parser.add_argument(
        "--report",
        default="/tmp/litecowork-release-evidence/soak.json",
    )
    arguments = parser.parse_args()

    measurement = run(arguments)
    iterations = max(1, measurement["iterations"])
    latencies = sorted(measurement["latencies_ms"])

    def percentile(fraction: float) -> float:
        if not latencies:
            return 0.0
        index = min(len(latencies) - 1, int(len(latencies) * fraction))
        return latencies[index]

    bytes_per_iteration = measurement["max_database_bytes"] / iterations
    bound = arguments.max_bytes_per_iteration * iterations
    bounded = measurement["max_database_bytes"] <= bound

    report = {
        "seed": arguments.seed,
        "duration_seconds": arguments.duration_seconds,
        "iterations": measurement["iterations"],
        "committed_rows": measurement.get("committed_rows", 0),
        "restarts": measurement["restarts"],
        "latency_ms": {
            "p50": percentile(0.50),
            "p95": percentile(0.95),
            "p99": percentile(0.99),
            "max": latencies[-1] if latencies else 0.0,
            "mean": statistics.fmean(latencies) if latencies else 0.0,
        },
        "footprint_bytes": {
            "max_database": measurement["max_database_bytes"],
            "max_wal": measurement["max_wal_bytes"],
        },
        "bytes_per_iteration": bytes_per_iteration,
        "bounded_growth": bounded,
        "failures": measurement["failures"],
        "started_at": measurement["started_at"],
        "finished_at": measurement["finished_at"],
    }
    report_path = pathlib.Path(arguments.report)
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")
    print(json.dumps(report, indent=2, sort_keys=True))
    print(f"report: {report_path}")
    return 0 if not measurement["failures"] and bounded else 1


if __name__ == "__main__":
    raise SystemExit(main())
