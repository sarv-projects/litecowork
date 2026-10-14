#!/usr/bin/env python3
"""Read every first-party Markdown line, inventory/hash/cross-reference all Markdown sources.

Mechanical documentation coverage is NOT a semantic line-by-line or runtime conformance proof.
Runs without network, deletes nothing, modifies only generated implementation/V1-MARKDOWN-INVENTORY.csv
and implementation/V1-AUDIT-INVENTORY.md.
"""
from __future__ import annotations
import csv
import hashlib
import re
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parent.parent
SKIP = {".git", "node_modules", "target", ".venv", ".pytest_cache", ".agent-worktrees",
        "archive_code", "archives_docs", "ARCHIVE", "archives",
        ".code-intelligence", ".playwright-mcp", "dist", "build", "coverage", ".next"}
EXCLUDED = {
    "implementation/V1-AUDIT-INVENTORY.md",
    # Including this report would make it hash the Markdown inventory that hashes it.
    "implementation/V1-DESIGN-DEPTH-AUDIT.md",
}
LINK = re.compile(r'(?<!!)\[[^\]]+\]\(([^)]+)\)')
CATEGORY_KEYWORDS = {
    "agent/composer": re.compile(r"AgentBinding|AgentControlDescriptor|slash.command|native.agent|composer|session.option", re.I),
    "browser/computer": re.compile(r"Chrome|browser|takeover|ControlLease|computer.use", re.I),
    "shell/settings/motion": re.compile(r"Workbench|sidebar|Settings|accessibility|animation|motion", re.I),
    "rendering": re.compile(r"RichPresentation|PresentationItem|renderer|Markdown|ArtifactWorkbench", re.I),
    "workflow/recovery": re.compile(r"Automation|Trigger|Task|Attempt|Effect|recovery|outbox", re.I),
    "trust/security": re.compile(r"credential|permission|trust|grant|redact|secret", re.I),
}
def included(path: Path) -> bool:
    return path.suffix.lower() in {".md", ".mdx"} and not (set(path.parts) & SKIP)

def is_local_link(dest: str) -> bool:
    return not dest.startswith(("#", "http:", "https:", "mailto:", "data:", "javascript:", "file:", "app:"))

def read_all():
    records = []
    broken = []
    # Prune dependency/build directories before traversal; filtering after rglob
    # still visits huge node_modules/target trees and can stall UNC/WSL mounts.
    import os
    candidates = []
    for folder, dirs, names in os.walk(ROOT):
        dirs[:] = [name for name in dirs if name not in SKIP]
        for name in names:
            if Path(name).suffix.lower() in {".md", ".mdx"}:
                candidates.append(Path(folder) / name)
    for path in sorted(candidates):
        rel = path.relative_to(ROOT)
        if not included(rel) or str(rel) in EXCLUDED:
            continue
        data = path.read_bytes()
        decoded = data.decode("utf-8-sig", errors="replace")
        lines = decoded.splitlines()
        topics = [key for key, pattern in CATEGORY_KEYWORDS.items() if any(pattern.search(line) for line in lines)]
        for line_no, line in enumerate(lines, 1):
            for match in LINK.finditer(line):
                target = match.group(1).split('"', 1)[0].strip()
                target = unquote(target.split("#")[0].split("?", 1)[0])
                if not target or not is_local_link(target):
                    continue
                resolved = (path.parent / target).resolve()
                if not resolved.exists():
                    broken.append((str(rel), line_no, match.group(1)))
        records.append({
            "file": str(rel).replace("\\", "/"),
            "lines": len(lines),
            "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
            "categories": ";".join(topics) or "misc",
            "has_heading": any(line.startswith("# ") for line in lines),
        })
    return records, broken

def emit(records, broken):
    target = ROOT / "implementation"
    target.mkdir(exist_ok=True)
    csv_path = target / "V1-MARKDOWN-INVENTORY.csv"
    with csv_path.open("w", encoding="utf-8", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=list(records[0]) if records else
                                ["file", "lines", "bytes", "sha256", "categories", "has_heading"])
        writer.writeheader()
        writer.writerows(records)
    total_lines = sum(row["lines"] for row in records)
    total_bytes = sum(row["bytes"] for row in records)
    content = [
        "# V1 First-Party Markdown Mechanical Inventory",
        "",
        "This script read every line of every included first-party Markdown file and checked "
        "literal Markdown relative link targets. **Mechanical coverage does not imply that "
        "each statement was semantically audited, implemented or runtime-qualified.**",
        "",
        f"- Files read: {len(records)}",
        f"- Lines read: {total_lines}",
        f"- Bytes read: {total_bytes}",
        f"- Broken relative links: {len(broken)}",
        "- Excluded: dependency/cache/output worktrees and generated audit reports; "
        "CSV inventory includes SHA-256 for every read Markdown source.",
        "",
        "## Per-file line and topical inventory",
        "",
        "| File | Lines | SHA-256 prefix | Keyword categories |",
        "|---|---:|---|---|",
    ]
    for row in records:
        label = row["file"]
        content.append(f"| {label} | {row['lines']} | {row['sha256'][:12]} | {row['categories']} |")
    content += ["", "## Broken local targets", ""]
    if broken:
        content.extend(f"- {name}:{number}: {link}" for name, number, link in broken)
    else:
        content.append("None detected by this limited Markdown link parser.")
    content += [
        "", "## Semantic/source qualification still required", "",
        "The normative target docs and implementation/V1-DESIGN-COVERAGE.md require "
        "owner-specific CODE, SYSTEM and USER evidence. Keyword categories do not "
        "certify intent or correctness, and link checking is not an external URL status check.",
        "",
    ]
    (target / "V1-AUDIT-INVENTORY.md").write_text("\n".join(content), encoding="utf-8")
    print(f"mechanical markdown audit: files={len(records)} lines={total_lines} "
          f"bytes={total_bytes} broken_links={len(broken)}")
    for name, n, link in broken[:30]:
        print(f"BROKEN {name}:{n}: {link}")
    return len(broken)

if __name__ == "__main__":
    raise SystemExit(bool(emit(*read_all())))
