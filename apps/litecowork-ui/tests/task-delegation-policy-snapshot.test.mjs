import assert from "node:assert/strict";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createServer, createServerModuleRunnerTransport } from "vite";
import { ModuleRunner } from "vite/module-runner";

const vite = await createServer({
  configFile: false,
  root: fileURLToPath(new URL("..", import.meta.url)),
  // Static SSR needs TSX conversion, not React Refresh/Babel work.
  plugins: [],
  optimizeDeps: { noDiscovery: true },
  server: { middlewareMode: true, hmr: false, ws: false },
  appType: "custom",
});
const transport = createServerModuleRunnerTransport({ channel: vite.environments.ssr.hot });
transport.timeout = 120_000;
const runner = new ModuleRunner({
  transport,
  hmr: false,
  root: vite.config.root,
  sourcemapInterceptor: false,
});
after(async () => {
  await runner.close();
  await vite.close();
});
const { TaskDelegationPolicySnapshot } = await runner.import(
  "/src/tasks/TaskDelegationPolicySnapshot.tsx",
);

test("Task detail exposes the exact policy pin and clarifies it is not execution evidence", () => {
  const html = renderToStaticMarkup(createElement(TaskDelegationPolicySnapshot, {
    snapshot: {
      source: "COWORKER_OVERRIDE",
      workspace_settings_revision: 4,
      coworker_id: "coworker-research",
      coworker_revision: 7,
      strategy: "BALANCED",
      native_child_model_policy: "INHERIT_PARENT_IF_SUPPORTED",
      worker_profiles: [{
        profile_id: "profile-reviewer",
        revision: 3,
        worker_model_selection_policy: "MATCH_PARENT_IF_EXACT_ROUTE_AVAILABLE",
      }],
      resolved_at: "2026-10-21T10:00:00.000000000Z",
    },
  }));

  assert.match(html, /Delegation policy at this revision/);
  assert.match(html, /Coworker override/);
  assert.match(html, /Workspace settings/);
  assert.match(html, /Revision 4/);
  assert.match(html, /coworker-research/);
  assert.match(html, /revision 7/);
  assert.match(html, /Balanced/);
  assert.match(html, /profile-reviewer/);
  assert.match(html, /revision 3/);
  assert.match(html, /exact available route/);
  assert.match(html, /does not indicate that a worker has run or been qualified/);
});

test("legacy Task revisions show missing provenance as unavailable instead of inferring it", () => {
  const html = renderToStaticMarkup(createElement(TaskDelegationPolicySnapshot, { snapshot: null }));

  assert.match(html, /This legacy Task revision has no saved policy snapshot/);
  assert.match(html, /delegation policy is unavailable/);
  assert.doesNotMatch(html, /Workspace defaults|Balanced|worker profiles are pinned/);
});
