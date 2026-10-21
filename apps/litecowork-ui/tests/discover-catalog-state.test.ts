import assert from "node:assert/strict";
import { test } from "node:test";

const {
  decodeDiscoverItem,
  decodeDiscoverItemDetail,
  decodeDiscoverItemPage,
  decodeDiscoverProviderStatus,
} = await import("../src/discover/discover-api.ts");
const { connectActionOutcome, discoverCatalogView, discoverRowView } = await import("../src/discover/discover-state.ts");

function baseItem(overrides: Record<string, unknown> = {}) {
  return {
    item_ref: "skill:skill-1",
    kind: "SKILL",
    display_name: "Invoice cleanup",
    summary: "Clean up an invoice",
    description: null,
    compatibility: "COMPATIBLE",
    compatibility_reason: null,
    requires_user_action: false,
    availability: "AVAILABLE",
    availability_reason: "READY",
    install_state: "INSTALLED",
    auth_state: "NOT_REQUIRED",
    source: "LITECOWORK_LOCAL",
    publisher: "LiteCowork",
    version: "1",
    digest: `sha256:${"a".repeat(64)}`,
    grants_required: [],
    authoritative: false,
    ...overrides,
  };
}

test("a catalog row that claims authority is refused", () => {
  assert.throws(() => decodeDiscoverItem(baseItem({ authoritative: true })), /claims authority/);
});

test("a row that is available but incompatible is refused as inconsistent", () => {
  assert.throws(
    () => decodeDiscoverItem(baseItem({ compatibility: "INCOMPATIBLE" })),
    /inconsistent Discover availability/,
  );
});

test("a row that is available but needs authentication is refused as inconsistent", () => {
  assert.throws(
    () => decodeDiscoverItem(baseItem({ auth_state: "REQUIRED" })),
    /inconsistent Discover availability/,
  );
});

test("a row that is available but has an unknown install state is refused as inconsistent", () => {
  assert.throws(
    () => decodeDiscoverItem(baseItem({ install_state: "UNKNOWN" })),
    /inconsistent Discover availability/,
  );
});

test("a row with a malformed digest is refused", () => {
  assert.throws(() => decodeDiscoverItem(baseItem({ digest: "not-a-digest" })), /invalid Discover digest/);
});

test("an unsupported availability value is refused rather than defaulted", () => {
  assert.throws(() => decodeDiscoverItem(baseItem({ availability: "MAYBE" })), /unsupported Discover availability/);
});

test("an available local Skill row is decoded and stays non-authoritative", () => {
  const item = decodeDiscoverItem(baseItem());
  assert.equal(item.availability, "AVAILABLE");
  assert.equal(item.authoritative, false);
  assert.equal(item.grants_required.length, 0);
});

test("a row that is not qualified reports the reason and is disabled", () => {
  const row = discoverRowView(
    decodeDiscoverItem(
      baseItem({
        availability: "NOT_QUALIFIED",
        availability_reason: "PACKAGE_SERVICE_UNAVAILABLE",
        install_state: "UNKNOWN",
        auth_state: "UNKNOWN",
      }),
    ),
  );
  assert.equal(row.disabled, true);
  assert.equal(row.tone, "not-qualified");
  assert.equal(row.availabilityLabel, "Package service not qualified");
  assert.equal(row.availabilityReason, "PACKAGE_SERVICE_UNAVAILABLE");
});

test("a row that needs authentication asks the owner to act", () => {
  const row = discoverRowView(
    decodeDiscoverItem(
      baseItem({
        availability: "NEEDS_AUTH",
        availability_reason: "AUTH_REQUIRED",
        auth_state: "REQUIRED",
      }),
    ),
  );
  assert.equal(row.requiresUserAction, true);
  assert.equal(row.disabled, true);
  assert.equal(row.tone, "needs-action");
});

test("an incompatible entry is disabled and says why", () => {
  const row = discoverRowView(
    decodeDiscoverItem(
      baseItem({
        availability: "UNSUPPORTED",
        availability_reason: "INCOMPATIBLE_ENTRY",
        compatibility: "INCOMPATIBLE",
        compatibility_reason: "INCOMPATIBLE_ENTRY",
      }),
    ),
  );
  assert.equal(row.disabled, true);
  assert.equal(row.tone, "unavailable");
});

test("the provider status reports that the package service is not qualified", () => {
  const status = decodeDiscoverProviderStatus({
    package_service_qualified: false,
    reason_code: "LITESPOM_PACKAGE_SERVICE_NOT_QUALIFIED",
    message: "Package discovery is unavailable until the LiteSPM package contract is published and qualified.",
  });
  assert.equal(status.package_service_qualified, false);
  assert.equal(status.reason_code, "LITESPOM_PACKAGE_SERVICE_NOT_QUALIFIED");
});

test("a catalog page keeps the provider status and the host observations apart", () => {
  const page = decodeDiscoverItemPage({
    items: [baseItem()],
    next_cursor: null,
    provider_status: {
      package_service_qualified: false,
      reason_code: "LITESPOM_PACKAGE_SERVICE_NOT_QUALIFIED",
      message: "Package discovery is unavailable.",
    },
    runtime_capabilities: [
      baseItem({
        item_ref: "mcp-skill:host-1",
        kind: "SKILL",
        source: "MCP_SERVER",
        availability: "AVAILABLE",
      }),
    ],
  });
  const view = discoverCatalogView(page);
  assert.equal(view.rows.length, 1);
  assert.equal(view.observationRows.length, 1);
  assert.equal(view.observationRows[0].itemRef, "mcp-skill:host-1");
  assert.equal(view.packageServiceQualified, false);
  assert.equal(view.empty, false);
});

test("an empty catalog is reported as empty rather than as an error", () => {
  const page = decodeDiscoverItemPage({
    items: [],
    next_cursor: null,
    provider_status: {
      package_service_qualified: false,
      reason_code: "LITESPOM_PACKAGE_SERVICE_NOT_QUALIFIED",
      message: "Package discovery is unavailable.",
    },
    runtime_capabilities: [],
  });
  const view = discoverCatalogView(page);
  assert.equal(view.empty, true);
});

test("a discover detail never carries a raw provider payload", () => {
  const detail = decodeDiscoverItemDetail({
    item: baseItem(),
    capabilities: [
      {
        capability_id: "acme:list",
        display_name: "List invoices",
        operation_class: "READ",
        required_scopes: ["invoices.read"],
      },
    ],
    provider_note: null,
  });
  assert.equal(detail.capabilities.length, 1);
  assert.equal(detail.capabilities[0].operation_class, "READ");
  assert.equal(Object.prototype.hasOwnProperty.call(detail, "provider_payload"), false);
});

test("the connect action never claims that a package was installed", () => {
  const row = discoverRowView(decodeDiscoverItem(baseItem()));
  const outcome = connectActionOutcome(row);
  assert.equal(outcome.allowed, false);
  assert.match(outcome.message, /LiteSPM package contract is published and qualified/);
});

test("the connect action reports a disabled row's own reason", () => {
  const row = discoverRowView(
    decodeDiscoverItem(
      baseItem({
        availability: "NOT_QUALIFIED",
        availability_reason: "PACKAGE_SERVICE_UNAVAILABLE",
        install_state: "UNKNOWN",
        auth_state: "UNKNOWN",
      }),
    ),
  );
  const outcome = connectActionOutcome(row);
  assert.equal(outcome.allowed, false);
  assert.match(outcome.message, /PACKAGE_SERVICE_UNAVAILABLE/);
});
