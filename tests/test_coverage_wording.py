import unittest

from scripts.validate_implementation_coverage import report_text


class CoverageReportWordingTests(unittest.TestCase):
    def test_report_does_not_claim_all_product_behavior_is_unimplemented(self):
        report = report_text([], {"operator_operation_unreachable_schemas": []})

        self.assertIn("This coverage index does not establish implementation or qualification.", report)
        self.assertNotIn("Product behavior is not implemented or qualified", report)


if __name__ == "__main__":
    unittest.main()
