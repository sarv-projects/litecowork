import unittest

from scripts.validate_implementation_coverage import implementation_plan_docs


class CoverageGenerationExclusionTests(unittest.TestCase):
    def test_generated_audit_reports_are_not_coverage_sources(self):
        documents = set(implementation_plan_docs())

        for generated_report in (
            "implementation/V1-CONTRACT-CHECK.md",
            "implementation/V1-AUDIT-INVENTORY.md",
            "implementation/V1-DESIGN-DEPTH-AUDIT.md",
        ):
            with self.subTest(generated_report=generated_report):
                self.assertNotIn(generated_report, documents)

        self.assertIn("implementation/epics/E01.md", documents)


if __name__ == "__main__":
    unittest.main()
