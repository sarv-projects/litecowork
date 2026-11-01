import assert from "node:assert/strict";
import test from "node:test";
import {
  decodePlacementDecision,
  placementAllowsTask,
  placementBlocker,
  placementLabel,
} from "../src/resources/knowledge-placement.ts";
import {
  decodeLocalModelAvailability,
  localModelInventoryLabel,
  localModelIsUsableByAgent,
} from "../src/resources/local-model-inventory.ts";

const workspaceId = "workspace-1";
const resourceId = "resource-1";

test("a local-only Resource blocks a cloud consumer", () => {
  const decision = decodePlacementDecision(
    {
      workspaceId,
      resourceId,
      placement: "LOCAL_ONLY",
      policy: "LOCAL_ONLY",
      blockedCode: "RESOURCE_LOCAL_ONLY",
      blockedReason: "This Resource is local-only.",
    },
    { workspaceId, resourceId },
  );
  assert.equal(placementAllowsTask(decision, "LOCAL"), true);
  assert.equal(placementAllowsTask(decision, "CLOUD"), false);
  assert.match(placementLabel(decision), /Local only/);
  const blocker = placementBlocker(decision, "CLOUD");
  assert.equal(blocker?.code, "RESOURCE_LOCAL_ONLY");
});

test("a cloud-eligible Resource may be used by a cloud consumer", () => {
  const decision = decodePlacementDecision(
    {
      workspaceId,
      resourceId,
      placement: "CLOUD_ELIGIBLE",
      policy: "ACTIVE_TASK_INPUTS",
      blockedCode: null,
      blockedReason: null,
    },
    { workspaceId, resourceId },
  );
  assert.equal(placementAllowsTask(decision, "CLOUD"), true);
  assert.equal(placementBlocker(decision, "CLOUD"), null);
  assert.match(placementLabel(decision), /Cloud eligible/);
});

test("an unsupported placement or policy is refused", () => {
  assert.throws(
    () => decodePlacementDecision(
      { workspaceId, resourceId, placement: "SOMEWHERE", policy: "LOCAL_ONLY" },
      { workspaceId, resourceId },
    ),
    /unsupported Resource placement/,
  );
  assert.throws(
    () => decodePlacementDecision(
      { workspaceId, resourceId, placement: "LOCAL_ONLY", policy: "ANYWHERE" },
      { workspaceId, resourceId },
    ),
    /unsupported Workspace placement policy/,
  );
  assert.throws(
    () => decodePlacementDecision(
      { workspaceId, resourceId: "resource-2", placement: "LOCAL_ONLY", policy: "LOCAL_ONLY" },
      { workspaceId, resourceId },
    ),
    /another Resource/,
  );
});

test("the local model inventory stays inventory only and agent scoped", () => {
  const availability = decodeLocalModelAvailability({
    capabilityId: "litecowork.local-models",
    status: "NOT_CONFIGURED",
    reasonCode: "LOCAL_MODEL_PROVIDER_NOT_QUALIFIED",
    reason: "No qualified local inference provider is registered.",
    inventoryOnly: true,
    surface: "AGENT_SCOPED",
    agentBindingPolicy: "AGENT_MODULE_COMPATIBILITY_REQUIRED",
    models: [],
  });
  assert.equal(availability.inventoryOnly, true);
  assert.equal(availability.surface, "AGENT_SCOPED");
  assert.match(localModelInventoryLabel(availability), /inventory only/i);
  assert.equal(
    localModelIsUsableByAgent(availability, null, "model-a"),
    false,
    "an inventory alone never authorises a local model",
  );
});

test("a local model becomes usable only through an exact AgentModule compatibility", () => {
  const availability = decodeLocalModelAvailability({
    capabilityId: "litecowork.local-models",
    status: "NOT_CONFIGURED",
    reasonCode: "LOCAL_MODEL_PROVIDER_NOT_QUALIFIED",
    reason: "",
    inventoryOnly: true,
    surface: "AGENT_SCOPED",
    agentBindingPolicy: "AGENT_MODULE_COMPATIBILITY_REQUIRED",
    models: [
      {
        modelId: "model-a",
        engineId: "engine-a",
        displayName: "Model A",
        state: "INVENTORY_ONLY",
        contextWindowTokens: 8192,
        agentModuleId: null,
        reasonCode: null,
      },
    ],
  });
  const compatible = {
    agentModuleId: "module-a",
    modelId: "model-a",
    compatible: true,
    reasonCode: null,
  };
  assert.equal(localModelIsUsableByAgent(availability, compatible, "model-a"), true);
  assert.equal(
    localModelIsUsableByAgent(availability, { ...compatible, modelId: "model-b" }, "model-a"),
    false,
    "compatibility for another model never applies",
  );
  assert.equal(
    localModelIsUsableByAgent(availability, { ...compatible, compatible: false }, "model-a"),
    false,
  );
});

test("a local model provider that is not inventory only is refused", () => {
  assert.throws(
    () => decodeLocalModelAvailability({
      capabilityId: "litecowork.local-models",
      status: "CONNECTED",
      reasonCode: "OK",
      reason: "",
      inventoryOnly: false,
      surface: "AGENT_SCOPED",
      agentBindingPolicy: "AGENT_MODULE_COMPATIBILITY_REQUIRED",
      models: [],
    }),
    /not inventory only/,
  );
  assert.throws(
    () => decodeLocalModelAvailability({
      capabilityId: "litecowork.local-models",
      status: "NOT_CONFIGURED",
      reasonCode: "X",
      reason: "",
      inventoryOnly: true,
      surface: "GLOBAL",
      agentBindingPolicy: "AGENT_MODULE_COMPATIBILITY_REQUIRED",
      models: [],
    }),
    /outside an Agent surface/,
  );
});
