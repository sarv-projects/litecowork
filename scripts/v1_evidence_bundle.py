#!/usr/bin/env python3
"""Assemble and validate release evidence bundles (019-T15, AC-013, AC-014).

    python3 scripts/v1_evidence_bundle.py assemble --journey E2E-01 \
        --row linux-x64-opencode-v2.0.32 --details details.json --out-dir OWNER_STORAGE \
        [--attachment PATH:KIND:STORAGE_REF]... [--index docs/release/evidence/INDEX.md]
    python3 scripts/v1_evidence_bundle.py validate BUNDLE.json [--attachments-dir DIR]
    python3 scripts/v1_evidence_bundle.py validate docs/release/evidence/INDEX.md [--bundles-dir DIR]

Rules: a bundle missing the SHA, OS build, agent version, negative cases or reviewer is rejected
by the schema; a secret-scan finding in the bundle or in any attachment rejects it; attachments
are referenced by SHA-256 digest only, so changing one invalidates the bundle; the repository
receives only a redacted index line, never the bundle, screenshots or video.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))

import v1_secret_scan  # noqa: E402

SCHEMA_PATH = REPOSITORY_ROOT / "docs" / "schemas" / "release-evidence-bundle.schema.json"
DEFAULT_INDEX = REPOSITORY_ROOT / "docs" / "release" / "evidence" / "INDEX.md"
INDEX_HEADER = (
    "| Journey | Row | Bundle SHA-256 | USER sign-off | Reviewer | Date |\r\n"
    "|---|---|---|---|---|---|\r\n"
)
INDEX_ROW = re.compile(
    r"^\| (E2E-\d{2}) \| ([a-z0-9.-]+) \| ([0-9a-f]{64}) \| (ACCEPT|PARTIAL|REJECT|NONE) \| ([^|]+) \| (\d{4}-\d{2}-\d{2}) \|$"
)


class BundleError(Exception):
    pass


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_digest(document: Any) -> str:
    encoded = json.dumps(document, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    return hashlib.sha256(encoded.encode("utf-8")).hexdigest()


def schema_errors(document: Any) -> list[str]:
    from jsonschema import Draft202012Validator

    schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
    return sorted(
        f"{'/'.join(str(part) for part in error.absolute_path) or '<root>'}: {error.message}"
        for error in Draft202012Validator(schema).iter_errors(document)
    )


def secret_findings(paths: list[Path]) -> list[str]:
    """Returns redacted finding descriptions; the matched bytes are never included."""
    result = v1_secret_scan.ScanResult()
    for path in paths:
        v1_secret_scan.scan_file(path, Path(path.name), result)
    return [f"{finding.pattern} in {finding.path}:{finding.line}" for finding in result.findings]


def validate_bundle(
    document: Any, bundle_path: Path | None = None, attachments_dir: Path | None = None
) -> list[str]:
    problems = [f"schema: {message}" for message in schema_errors(document)]
    scan_targets: list[Path] = []
    temporary: tempfile.TemporaryDirectory[str] | None = None
    try:
        if bundle_path is not None:
            scan_targets.append(bundle_path)
        else:
            temporary = tempfile.TemporaryDirectory()
            candidate = Path(temporary.name) / "bundle.json"
            candidate.write_text(json.dumps(document, indent=2), encoding="utf-8")
            scan_targets.append(candidate)
        if attachments_dir is not None and isinstance(document, dict):
            for artifact in document.get("user_artifacts", []):
                storage_ref = str(artifact.get("storage_ref", ""))
                if not storage_ref.startswith("file:"):
                    continue
                name = storage_ref[len("file:"):]
                path = (attachments_dir / name).resolve()
                if attachments_dir.resolve() not in path.parents or not path.is_file():
                    problems.append(f"attachment {name!r} is missing or outside the attachments directory")
                    continue
                if sha256_file(path) != artifact.get("digest"):
                    problems.append(f"attachment {name!r} no longer matches its recorded digest")
                scan_targets.append(path)
        problems += [f"secret scan: {finding}" for finding in secret_findings(scan_targets)]
    finally:
        if temporary is not None:
            temporary.cleanup()
    return problems


def run_text(command: list[str]) -> str | None:
    try:
        completed = subprocess.run(command, capture_output=True, text=True, timeout=60, check=False)
    except (OSError, subprocess.TimeoutExpired):
        return None
    return completed.stdout.strip() if completed.returncode == 0 else None


def format_os_build(
    system: str, version: str, release: str, os_release: dict[str, str] | None = None
) -> str:
    """Capture an OS identifier that can be compared with the declared release minimum."""
    if system == "Windows":
        return f"Windows {version or release}".strip()
    if system == "Linux":
        release_facts = os_release
        if release_facts is None:
            try:
                release_facts = platform.freedesktop_os_release()
            except (AttributeError, OSError, ValueError):
                release_facts = {}
        name = release_facts.get("NAME", "Linux")
        version_id = release_facts.get("VERSION_ID", "")
        return f"{name} {version_id}".strip() if version_id else f"{name} {release}".strip()
    if system == "Darwin":
        mac_version = platform.mac_ver()[0]
        return f"macOS {mac_version or version or release}".strip()
    return f"{system} {version or release}".strip()


def collect_facts(row_id: str, agent_version: str | None) -> dict[str, Any]:
    sha = run_text(["git", "-C", str(REPOSITORY_ROOT), "rev-parse", "HEAD"]) or ""
    status = run_text(["git", "-C", str(REPOSITORY_ROOT), "status", "--porcelain"])
    agent = row_id.split("-")[2] if row_id.count("-") >= 3 else "agent"
    if agent_version is None:
        output = run_text([agent, "--version"])
        agent_version = output.splitlines()[0] if output else None
    facts: dict[str, Any] = {
        "repo_sha": sha,
        "dirty": bool(status),
        "os_build": format_os_build(platform.system(), platform.version(), platform.release()),
        "arch": "x86_64" if platform.machine().lower() in ("x86_64", "amd64") else platform.machine(),
    }
    if agent_version:
        facts["agent_versions"] = {agent: agent_version}
    return facts


def parse_attachment(spec: str) -> dict[str, str]:
    path, _, rest = spec.partition(":")
    kind, _, storage_ref = rest.partition(":")
    if not path or not kind or not storage_ref:
        raise BundleError(f"attachment must be PATH:KIND:STORAGE_REF, got {spec!r}")
    file_path = Path(path)
    if not file_path.is_file():
        raise BundleError(f"attachment not found: {path}")
    return {"digest": sha256_file(file_path), "kind": kind, "storage_ref": storage_ref}


def index_line(document: dict[str, Any], digest: str) -> str:
    signoff = document.get("user_signoff")
    return (
        f"| {document['journey_id']} | {document['row_id']} | {digest} | "
        f"{signoff['decision'] if signoff else 'NONE'} | {document['reviewer']} | {document['accepted_at']} |"
    )


def append_index(index_path: Path, line: str) -> None:
    index_path.parent.mkdir(parents=True, exist_ok=True)
    if not index_path.exists():
        index_path.write_bytes(
            ("# Release evidence index\r\n\r\nRedacted index only. Bundles, screenshots and video stay in owner storage.\r\n\r\n" + INDEX_HEADER).encode()
        )
    with index_path.open("ab") as handle:
        handle.write((line + "\r\n").encode())


def assemble(arguments: argparse.Namespace) -> int:
    details = json.loads(Path(arguments.details).read_text(encoding="utf-8"))
    document: dict[str, Any] = dict(details)
    document["journey_id"] = arguments.journey
    document["row_id"] = arguments.row
    document.update(collect_facts(arguments.row, arguments.agent_version))
    if arguments.repo_sha:
        document["repo_sha"] = arguments.repo_sha
    document["user_artifacts"] = [parse_attachment(spec) for spec in arguments.attachment]
    attachments_dir = None
    problems = validate_bundle(document, None, attachments_dir)
    for spec in arguments.attachment:
        problems += [f"secret scan: {finding}" for finding in secret_findings([Path(spec.partition(':')[0])])]
    if problems:
        print("bundle rejected:", file=sys.stderr)
        for problem in problems:
            print(f"  {problem}", file=sys.stderr)
        return 1
    out_dir = Path(arguments.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    digest = canonical_digest(document)
    (out_dir / f"{arguments.journey}-{arguments.row}-{digest[:12]}.json").write_text(
        json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    if arguments.index:
        append_index(Path(arguments.index), index_line(document, digest))
    print(f"bundle accepted: {digest}")
    return 0


def validate_index(path: Path, bundles_dir: Path | None) -> list[str]:
    problems: list[str] = []
    rows = 0
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.startswith("| E2E-"):
            continue
        match = INDEX_ROW.match(line)
        if not match:
            problems.append(f"line {number}: malformed index row")
            continue
        rows += 1
        if bundles_dir is not None:
            matches = [
                candidate for candidate in sorted(bundles_dir.glob("*.json"))
                if canonical_digest(json.loads(candidate.read_text(encoding="utf-8"))) == match.group(3)
            ]
            if not matches:
                problems.append(f"line {number}: no bundle in {bundles_dir} has digest {match.group(3)[:12]}")
    return problems


def validate(arguments: argparse.Namespace) -> int:
    path = Path(arguments.path)
    if path.suffix == ".md":
        problems = validate_index(path, Path(arguments.bundles_dir) if arguments.bundles_dir else None)
        problems += [f"secret scan: {finding}" for finding in secret_findings([path])]
    else:
        try:
            document = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError) as error:
            print(f"cannot read bundle: {error}", file=sys.stderr)
            return 1
        problems = validate_bundle(document, path, Path(arguments.attachments_dir) if arguments.attachments_dir else None)
    if problems:
        for problem in problems:
            print(problem, file=sys.stderr)
        return 1
    print(f"valid: {path}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    build = commands.add_parser("assemble")
    build.add_argument("--journey", required=True)
    build.add_argument("--row", required=True)
    build.add_argument("--details", required=True)
    build.add_argument("--out-dir", required=True)
    build.add_argument("--attachment", action="append", default=[])
    build.add_argument("--index", default=None)
    build.add_argument("--agent-version", default=None)
    build.add_argument("--repo-sha", default=None, help="override for tests; normally read from git")
    check = commands.add_parser("validate")
    check.add_argument("path")
    check.add_argument("--attachments-dir", default=None)
    check.add_argument("--bundles-dir", default=None)
    arguments = parser.parse_args()
    try:
        return assemble(arguments) if arguments.command == "assemble" else validate(arguments)
    except BundleError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
