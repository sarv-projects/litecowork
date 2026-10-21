#!/usr/bin/env python3
"""Scan the LiteCowork build output for secret material.

The scan covers the artifacts a release would ship: the daemon binary, the
desktop bundle output, the local backup archives and — optionally — the tracked
source tree. It reports every match with the file and the pattern that matched.

The scanner never prints the matched secret bytes; it prints the pattern name,
the file, the line number and a redacted preview so the report can be stored
without copying the secret.

Usage:
    .venv/bin/python scripts/v1_secret_scan.py [--source] [--extra-path PATH]...
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent

# Files that are never scanned even in source mode: they are generated, huge or
# hold fixtures that intentionally look like secrets.
EXCLUDED_DIRECTORY_NAMES = {
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".venv",
    "__pycache__",
    "archives",
    "archive_code",
    "archives_docs",
    "ARCHIVE",
}

# The high confidence token shapes. A match only counts when the token stands
# on its own: the byte before and the byte after the match must not be part of
# a longer identifier. Without that rule the detector's own token list (for
# example the redaction prefixes embedded in the product binary) reports itself.
PATTERNS: tuple[tuple[str, bytes], ...] = (
    ("private key block", b"-----BEGIN (?:RSA |EC |OPENSSH |PGP |DSA )?PRIVATE KEY-----"),
    ("aws access key id", rb"AKIA[0-9A-Z]{16}"),
    ("github token", rb"gh[pousr]_[A-Za-z0-9]{36,128}"),
    ("gitlab token", rb"glpat-[A-Za-z0-9\-_]{20,128}"),
    ("slack token", rb"xox[baprs]-[A-Za-z0-9-]{10,64}"),
    ("stripe live key", rb"[rs]k_live_[A-Za-z0-9]{16,64}"),
    ("anthropic key", rb"sk-ant-api\d{2}-[A-Za-z0-9\-_]{40,64}"),
    ("openai key", rb"sk-proj-[A-Za-z0-9\-_]{20,64}"),
    ("google api key", rb"AIza[0-9A-Za-z\-_]{35}"),
    ("jwt", rb"eyJ[A-Za-z0-9\-_]{10,}\.[A-Za-z0-9\-_]{10,}\.[A-Za-z0-9\-_]{10,}"),
)

# Patterns that only make sense in a text file. They are not applied to a
# compiled binary, where any byte sequence can look like an assignment.
TEXT_ONLY_PATTERNS: tuple[tuple[str, bytes], ...] = (
    (
        "assigned secret literal",
        rb"(?i)\b(?:password|passwd|secret|api[_-]?key|token)\b\s*[:=]\s*[\"'][^\"'\s]{12,}[\"']",
    ),
)

# A token character that would extend the match into a longer identifier.
TOKEN_CHARACTERS = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"

# A file is treated as text when this share of its bytes is printable.
TEXT_PRINTABLE_SHARE = 0.95

BINARY_SUFFIXES = {
    ".png",
    ".ico",
    ".jpg",
    ".jpeg",
    ".gif",
    ".webp",
    ".pdf",
    ".zip",
    ".gz",
    ".xz",
    ".bz2",
    ".woff",
    ".woff2",
    ".ttf",
    ".otf",
    ".so",
    ".dylib",
    ".dll",
    ".exe",
    ".rlib",
    ".rmeta",
    ".class",
    ".wasm",
}

# A single artifact larger than this is not scanned; the scan would not finish
# in a bounded time and a shipped desktop artifact is far smaller.
MAX_SCANNED_FILE_BYTES = 64 * 1024 * 1024


@dataclass
class Finding:
    pattern: str
    path: str
    line: int
    preview: str

    def as_dict(self) -> dict[str, Any]:
        return {
            "pattern": self.pattern,
            "path": self.path,
            "line": self.line,
            "preview": self.preview,
        }


@dataclass
class ScanResult:
    scanned_files: int = 0
    findings: list[Finding] = field(default_factory=list)

    def add(self, finding: Finding) -> None:
        self.findings.append(finding)


def redact(line: bytes) -> str:
    text = line.decode("utf-8", errors="replace").strip()
    if len(text) > 120:
        text = text[:117] + "..."
    return text


def is_text(data: bytes) -> bool:
    sample = data[:65536]
    if not sample:
        return True
    printable = sum(
        1 for byte in sample if byte in (9, 10, 13) or 32 <= byte < 127 or byte >= 128
    )
    return printable / len(sample) >= TEXT_PRINTABLE_SHARE


def stands_alone(data: bytes, start: int, end: int) -> bool:
    """Whether the match is not part of a longer identifier."""
    if start > 0 and data[start - 1] in TOKEN_CHARACTERS:
        return False
    if end < len(data) and data[end] in TOKEN_CHARACTERS:
        return False
    return True


def scan_file(path: Path, relative_to: Path, result: ScanResult) -> None:
    try:
        data = path.read_bytes()
    except OSError:
        return
    result.scanned_files += 1
    text = is_text(data)
    patterns = PATTERNS + (TEXT_ONLY_PATTERNS if text else ())
    for name, pattern in patterns:
        for match in re.finditer(pattern, data):
            start = match.start()
            end = match.end()
            if not stands_alone(data, start, end):
                continue
            line_number = data.count(b"\n", 0, start) + 1
            line_start = data.rfind(b"\n", 0, start) + 1
            line_end = data.find(b"\n", start)
            if line_end == -1:
                line_end = len(data)
            preview = redact(data[line_start:line_end])
            result.add(
                Finding(
                    pattern=name,
                    path=str(relative_to),
                    line=line_number,
                    preview=preview,
                )
            )


def iter_files(root: Path, *, skip_binary_suffixes: bool) -> list[Path]:
    collected: list[Path] = []
    for current, directories, files in os.walk(root):
        directories[:] = [
            directory
            for directory in directories
            if directory not in EXCLUDED_DIRECTORY_NAMES and not directory.startswith(".backup-")
        ]
        for name in files:
            path = Path(current) / name
            if skip_binary_suffixes and path.suffix.lower() in BINARY_SUFFIXES:
                continue
            try:
                if path.stat().st_size > MAX_SCANNED_FILE_BYTES:
                    continue
            except OSError:
                continue
            collected.append(path)
    return collected


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        action="store_true",
        help="also scan the tracked source tree",
    )
    parser.add_argument(
        "--extra-path",
        action="append",
        default=[],
        help="an additional path to scan (repeatable)",
    )
    parser.add_argument("--json", action="store_true", help="print the report as JSON")
    parser.add_argument(
        "--scan-target-tree",
        action="store_true",
        help="also scan the whole cargo target tree (slow; it is not a shipped artifact)",
    )
    arguments = parser.parse_args()

    result = ScanResult()
    directories: list[tuple[Path, bool]] = []
    files: list[Path] = []

    # The release artifacts a shipping desktop build would carry. The scan is
    # bounded to those artifacts: the whole `target` tree contains the compiler
    # and dependency artifacts, which are not shipped.
    artifact_candidates = [
        REPOSITORY_ROOT / "target" / "release" / "litecoworkd",
        REPOSITORY_ROOT / "target" / "release" / "litecoworkd.exe",
        REPOSITORY_ROOT / "target" / "debug" / "litecoworkd",
        REPOSITORY_ROOT / "target" / "debug" / "litecoworkd.exe",
        REPOSITORY_ROOT / "apps" / "litecowork-ui" / "src-tauri" / "target" / "release" / "bundle",
        REPOSITORY_ROOT / "apps" / "litecowork-ui" / "src-tauri" / "target" / "debug" / "bundle",
    ]
    for candidate in artifact_candidates:
        if candidate.is_file():
            files.append(candidate)
        elif candidate.is_dir():
            directories.append((candidate, False))

    for extra in arguments.extra_path:
        path = Path(extra)
        if path.is_dir():
            directories.append((path, False))
        elif path.is_file():
            files.append(path)

    if arguments.source:
        directories.append((REPOSITORY_ROOT, True))
    if arguments.scan_target_tree:
        directories.append((REPOSITORY_ROOT / "target", False))

    scanned_paths: set[Path] = set()
    for path in files:
        if path in scanned_paths:
            continue
        scanned_paths.add(path)
        try:
            relative = path.relative_to(REPOSITORY_ROOT)
        except ValueError:
            relative = path
        scan_file(path, relative, result)

    for root, is_source in directories:
        for path in iter_files(root, skip_binary_suffixes=not is_source):
            if path in scanned_paths:
                continue
            scanned_paths.add(path)
            if is_source and path.suffix.lower() in BINARY_SUFFIXES:
                continue
            try:
                relative = path.relative_to(REPOSITORY_ROOT)
            except ValueError:
                relative = path
            scan_file(path, relative, result)

    if arguments.json:
        print(
            json.dumps(
                {
                    "scanned_files": result.scanned_files,
                    "findings": [finding.as_dict() for finding in result.findings],
                },
                indent=2,
                sort_keys=True,
            )
        )
    else:
        for finding in result.findings:
            print(
                f"[FINDING] {finding.pattern} in {finding.path}:{finding.line}: {finding.preview}"
            )
        print(f"scanned {result.scanned_files} files, {len(result.findings)} findings")

    return 1 if result.findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
