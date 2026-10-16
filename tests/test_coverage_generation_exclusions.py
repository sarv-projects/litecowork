import unittest

from scripts.validate_implementation_coverage import api_documented_routes, implementation_plan_docs


class CoverageGenerationExclusionTests(unittest.TestCase):
    def test_api_inventory_includes_explicit_target_only_operations(self):
        routes = api_documented_routes(
            """```text
GET /v1/workspace-roots?status=&cursor=&limit=
# Target only operation: POST /v1/workspace-roots
# Target only operation: PATCH /v1/workspace-roots/{id}
```"""
        )

        self.assertEqual(
            routes,
            {
                ("GET", "/workspace-roots"),
                ("POST", "/workspace-roots"),
                ("PATCH", "/workspace-roots/{}"),
            },
        )

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
