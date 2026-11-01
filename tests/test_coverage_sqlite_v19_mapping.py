import unittest

from scripts.validate_implementation_coverage import contract_file_story


class SqliteV19CoverageMappingTests(unittest.TestCase):
    def test_resource_index_migration_maps_to_resource_intake_story(self):
        self.assertEqual(contract_file_story("docs/schemas/sqlite-v19.sql"), "E02-S03")


if __name__ == "__main__":
    unittest.main()
