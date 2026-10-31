import importlib
import copy
import tempfile
import unittest
from pathlib import Path


def handoff_integrity_api():
    try:
        return importlib.import_module(
            "implementation.spikes.agent_switching_poc.handoff_integrity"
        )
    except ModuleNotFoundError as error:
        if error.name == "implementation.spikes.agent_switching_poc.handoff_integrity":
            raise AssertionError("PoC checkpoint integrity verifier has not been implemented") from error
        raise


class HandoffIntegrityTests(unittest.TestCase):
    def test_changed_checkpoint_file_is_rejected(self):
        api = handoff_integrity_api()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            checkpoint_file = root / "module.py"
            checkpoint_file.write_text("def complete(): return False\n")
            manifest = api.build_checkpoint_manifest(root, ["module.py"])

            checkpoint_file.write_text("def complete(): return True\n")

            with self.assertRaises(api.CheckpointIntegrityError):
                api.verify_checkpoint_manifest(root, manifest)

    def test_valid_checkpoint_manifest_round_trips(self):
        api = handoff_integrity_api()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "a.txt").write_text("alpha")
            (root / "nested").mkdir()
            (root / "nested" / "b.txt").write_text("beta")

            manifest = api.build_checkpoint_manifest(
                root,
                ["nested/b.txt", "a.txt"],
            )

            verified_digest = api.verify_checkpoint_manifest(root, manifest)
            self.assertEqual(verified_digest, manifest["manifest_digest"])
            self.assertEqual(list(manifest["files"]), ["a.txt", "nested/b.txt"])

    def test_missing_checkpoint_file_is_rejected(self):
        api = handoff_integrity_api()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            checkpoint_file = root / "module.py"
            checkpoint_file.write_text("value = 1\n")
            manifest = api.build_checkpoint_manifest(root, ["module.py"])
            checkpoint_file.unlink()

            with self.assertRaises(api.CheckpointIntegrityError):
                api.verify_checkpoint_manifest(root, manifest)

    def test_modified_manifest_fields_are_rejected(self):
        api = handoff_integrity_api()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "module.py").write_text("value = 1\n")
            manifest = api.build_checkpoint_manifest(root, ["module.py"])
            tampered = copy.deepcopy(manifest)
            tampered["files"]["module.py"] = "0" * 64

            with self.assertRaises(api.CheckpointIntegrityError):
                api.verify_checkpoint_manifest(root, tampered)

    def test_checkpoint_manifest_rejects_escape_and_symlink_paths(self):
        api = handoff_integrity_api()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "workspace"
            root.mkdir()
            outside = Path(directory) / "outside.py"
            outside.write_text("private = True\n")
            with self.assertRaises(api.CheckpointIntegrityError):
                api.build_checkpoint_manifest(root, ["../outside.py"])

            link = root / "link.py"
            link.symlink_to(outside)
            with self.assertRaises(api.CheckpointIntegrityError):
                api.build_checkpoint_manifest(root, ["link.py"])

    def test_checkpoint_manifest_rejects_null_byte_paths(self):
        api = handoff_integrity_api()
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(api.CheckpointIntegrityError):
                api.build_checkpoint_manifest(Path(directory), ["source\x00.py"])


if __name__ == "__main__":
    unittest.main()
