import assert from "node:assert/strict";
import { after, test } from "node:test";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const vite = await createServer({
  configFile: false,
  root: packageRoot,
  server: { middlewareMode: true, hmr: false, ws: false },
  optimizeDeps: { noDiscovery: true, include: [] },
  appType: "custom",
});
after(async () => vite.close());

const observed = [];
const projection = {
  delegation_profile_id: "profile-1",
  task_category: "RESEARCH",
  sample_count: 4,
  verifier_pass_rate: null,
  median_latency_ms: 1800,
  median_observed_cost: null,
  retry_rate: 0.25,
  human_intervention_rate: null,
  failure_categories: [],
  confidence: "LOW",
};

globalThis.window = {
  __TAURI_INTERNALS__: {
    async invoke(command, args) {
      observed.push({ command, args });
      return {
        status: 200,
        contentType: "application/json",
        bodyBase64: Buffer.from(JSON.stringify(projection)).toString("base64"),
      };
    },
  },
};

const { desktopDelegationProfileCatalogApi } = await vite.ssrLoadModule("/src/coworkers/desktop-delegation-profile-api.ts");

test("desktop performance read uses only the fixed Workspace-scoped bridge command", async () => {
  const result = await desktopDelegationProfileCatalogApi("workspace-1").getPerformance("profile-1", "RESEARCH");

  assert.deepEqual(observed, [{
    command: "get_delegation_profile_performance",
    args: { workspaceId: "workspace-1", profileId: "profile-1", taskCategory: "RESEARCH" },
  }]);
  assert.equal(result.delegation_profile_id, "profile-1");
  assert.equal(result.median_observed_cost, null);
  assert.equal(result.verifier_pass_rate, null);
});

test("desktop performance read rejects path-like profile IDs before invoking Runtime", async () => {
  const before = observed.length;
  await assert.rejects(
    desktopDelegationProfileCatalogApi("workspace-1").getPerformance("../other"),
    /identifier is invalid/i,
  );
  assert.equal(observed.length, before);
});
