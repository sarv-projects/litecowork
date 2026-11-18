import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import audit_v1_markdown
from scripts import audit_v1_design_depth
from scripts import build_active_corpus_inventory


class MarkdownAuditArchiveExclusionTests(unittest.TestCase):
    def test_active_corpus_inventory_excludes_local_audit_output(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            current = root / "docs" / "current.md"
            current.parent.mkdir(parents=True)
            current.write_text("# Current\n", encoding="utf-8")
            local_audit = root / "audit-output" / "private-audit.md"
            local_audit.parent.mkdir(parents=True)
            local_audit.write_text("# Local audit data\n", encoding="utf-8")

            with patch.object(build_active_corpus_inventory, "ROOT", root):
                records = build_active_corpus_inventory.collect()

        self.assertEqual([record["path"] for record in records], ["docs/current.md"])

    def test_design_depth_inventory_excludes_local_audit_output(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            current = root / "docs" / "current.md"
            current.parent.mkdir(parents=True)
            current.write_text("# Current\n", encoding="utf-8")
            local_audit = root / "audit-output" / "private-audit.md"
            local_audit.parent.mkdir(parents=True)
            local_audit.write_text("# Local audit data\n", encoding="utf-8")

            with patch.object(audit_v1_design_depth, "ROOT", root):
                files = audit_v1_design_depth.active_markdown()

        self.assertEqual([path.relative_to(root).as_posix() for path in files], ["docs/current.md"])

    def test_local_audit_output_is_pruned_before_markdown_traversal(self):
        excluded = {"audit-output"}
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            included = root / "docs" / "current.md"
            included.parent.mkdir(parents=True)
            included.write_text("# Current\n", encoding="utf-8")
            local_audit = root / "audit-output" / "private-audit.md"
            local_audit.parent.mkdir(parents=True)
            local_audit.write_text("# Local audit data\n", encoding="utf-8")

            visited: list[Path] = []
            real_walk = os.walk

            def recording_walk(top):
                for folder, dirs, names in real_walk(top):
                    visited.append(Path(folder).relative_to(root))
                    yield folder, dirs, names

            with patch.object(audit_v1_markdown, "ROOT", root), patch("os.walk", recording_walk):
                records, broken = audit_v1_markdown.read_all()

        self.assertEqual([record["file"] for record in records], ["docs/current.md"])
        self.assertEqual(broken, [])
        self.assertTrue(
            all(not (set(path.parts) & excluded) for path in visited),
            visited,
        )

    def test_archive_directories_are_pruned_before_markdown_traversal(self):
        excluded = {"archive_code", "archives_docs", "ARCHIVE", "archives"}
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            included = root / "docs" / "current.md"
            included.parent.mkdir(parents=True)
            included.write_text("# Current\n", encoding="utf-8")
            for directory in excluded:
                path = root / directory / "nested" / "old.md"
                path.parent.mkdir(parents=True)
                path.write_text("# Historical\n", encoding="utf-8")

            visited: list[Path] = []
            real_walk = os.walk

            def recording_walk(top):
                for folder, dirs, names in real_walk(top):
                    visited.append(Path(folder).relative_to(root))
                    yield folder, dirs, names

            with patch.object(audit_v1_markdown, "ROOT", root), patch("os.walk", recording_walk):
                records, broken = audit_v1_markdown.read_all()

        self.assertEqual([record["file"] for record in records], ["docs/current.md"])
        self.assertEqual(broken, [])
        self.assertTrue(
            all(not (path.parts and path.parts[0] in excluded) for path in visited),
            visited,
        )

    def test_local_superpowers_artifacts_are_pruned_before_markdown_traversal(self):
        excluded = {".superpowers", "superpowers"}
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            included = root / "docs" / "current.md"
            included.parent.mkdir(parents=True)
            included.write_text("# Current\n", encoding="utf-8")
            for directory in excluded:
                path = root / directory / "plans" / "owner-work.md"
                path.parent.mkdir(parents=True)
                path.write_text("# Local planning artifact\n", encoding="utf-8")

            visited: list[Path] = []
            real_walk = os.walk

            def recording_walk(top):
                for folder, dirs, names in real_walk(top):
                    visited.append(Path(folder).relative_to(root))
                    yield folder, dirs, names

            with patch.object(audit_v1_markdown, "ROOT", root), patch("os.walk", recording_walk):
                records, broken = audit_v1_markdown.read_all()

        self.assertEqual([record["file"] for record in records], ["docs/current.md"])
        self.assertEqual(broken, [])
        self.assertTrue(
            all(not (set(path.parts) & excluded) for path in visited),
            visited,
        )

    def test_generated_audit_reports_are_excluded_to_break_their_hash_cycle(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            current = root / "docs" / "current.md"
            current.parent.mkdir(parents=True)
            current.write_text("# Current\n", encoding="utf-8")
            generated = root / "implementation"
            generated.mkdir()
            for name in ("V1-AUDIT-INVENTORY.md", "V1-DESIGN-DEPTH-AUDIT.md"):
                (generated / name).write_text("# Generated\n", encoding="utf-8")

            with patch.object(audit_v1_markdown, "ROOT", root):
                records, broken = audit_v1_markdown.read_all()

        self.assertEqual([record["file"] for record in records], ["docs/current.md"])
        self.assertEqual(broken, [])


if __name__ == "__main__":
    unittest.main()
