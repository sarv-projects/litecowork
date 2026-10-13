import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import audit_v1_markdown


class MarkdownAuditArchiveExclusionTests(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
