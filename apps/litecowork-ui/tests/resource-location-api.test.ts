import assert from "node:assert/strict";
import test from "node:test";
import { createResourceLocationApi, resourceLocationStateForSelection } from "../src/resources/resource-location-api.ts";

const resourceId = "resource-1";
const location = {
  location_id: "location-1",
  resource_id: resourceId,
  runtime_id: "runtime-local",
  environment_id: null,
  connection_id: null,
  availability: "AVAILABLE",
  writable: false,
  observed_revision_id: "revision-1",
  observed_digest: `sha256:${"a".repeat(64)}`,
  observed_at: "2026-10-10T12:00:00Z",
  last_checked_at: null,
  freshness: "CURRENT",
} as const;

test("uses only the finite native command and returns the generated public location shape", async () => {
  const calls: Array<{ command: string; args: Record<string, unknown> }> = [];
  const api = createResourceLocationApi(async (command, args) => {
    calls.push({ command, args });
    return [location];
  });

  assert.deepEqual(await api.list("workspace-1", resourceId), [location]);
  assert.deepEqual(calls, [{
    command: "list_resource_locations",
    args: { workspaceId: "workspace-1", resourceId },
  }]);
});

test("does not request empty selections", async () => {
  let called = false;
  const api = createResourceLocationApi(async () => { called = true; return []; });

  await assert.rejects(api.list("", resourceId), /selection is invalid/i);
  await assert.rejects(api.list("workspace-1", "../resource"), /selection is invalid/i);
  assert.equal(called, false);
});

test("rejects a location projection for another Resource", async () => {
  const api = createResourceLocationApi(async () => [{ ...location, resource_id: "resource-2" }]);

  await assert.rejects(api.list("workspace-1", resourceId), /different Resource/i);
});

test("rejects unknown private locator fields and invalid observation enums", async () => {
  const privateField = createResourceLocationApi(async () => [{ ...location, locator_ref_id: "private-locator" }]);
  const invalidAvailability = createResourceLocationApi(async () => [{ ...location, availability: "LIVE_PROBED" }]);

  await assert.rejects(privateField.list("workspace-1", resourceId), /unsupported Resource location fields/i);
  await assert.rejects(invalidAvailability.list("workspace-1", resourceId), /unsupported Resource location availability/i);
});

test("rejects a malformed collection instead of rendering an empty state", async () => {
  const api = createResourceLocationApi(async () => ({ items: [] }));

  await assert.rejects(api.list("workspace-1", resourceId), /invalid Resource locations/i);
});

test("hides saved observations and status when the selected Workspace or Resource changes", () => {
  const snapshot = {
    workspaceId: "workspace-1",
    resourceId,
    busy: false,
    error: null,
    locations: [location],
  };

  assert.deepEqual(resourceLocationStateForSelection(snapshot, "workspace-1", resourceId), snapshot);
  assert.equal(resourceLocationStateForSelection(snapshot, "workspace-2", resourceId), null);
  assert.equal(resourceLocationStateForSelection(snapshot, "workspace-1", "resource-2"), null);
});
