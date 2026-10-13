import unittest
from pathlib import Path

import yaml


ROOT = Path(__file__).resolve().parents[1]
OPENAPI = yaml.safe_load(
    (ROOT / "docs/schemas/operator-api.openapi.yaml").read_text(encoding="utf-8")
)


class RuntimeOffersContractTests(unittest.TestCase):
    def test_runtime_offer_page_has_bounded_opaque_pagination(self):
        operation = OPENAPI["paths"]["/runtimes/{runtimeId}/offers"]["get"]
        parameters = operation["parameters"]
        self.assertIn({"$ref": "#/components/parameters/Cursor"}, parameters)
        self.assertIn({"$ref": "#/components/parameters/Limit"}, parameters)
        self.assertEqual(
            operation["responses"]["200"]["content"]["application/json"]["schema"],
            {"$ref": "#/components/schemas/RuntimeOfferPage"},
        )


if __name__ == "__main__":
    unittest.main()
