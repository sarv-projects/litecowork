import assert from "node:assert/strict";
import { after, test } from "node:test";
import { createElement, Fragment } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import react from "@vitejs/plugin-react";
import { createServer } from "vite";

const vite = await createServer({
  configFile: false,
  root: new URL("..", import.meta.url).pathname,
  plugins: [
    {
      name: "expose-task-progress-summary-to-focused-test",
      enforce: "pre",
      transform(code, id) {
        if (!id.endsWith("/src/presentation/TaskPresentationPanel.tsx")) return null;
        return `${code}\nexport { TaskProgressSummary as __TaskProgressSummaryForTest, TaskSnapshotView as __TaskSnapshotViewForTest };`;
      },
    },
    react(),
  ],
  optimizeDeps: { noDiscovery: true },
  server: { middlewareMode: true, hmr: false, ws: false },
  appType: "custom",
});
const { __TaskProgressSummaryForTest, __TaskSnapshotViewForTest } = await vite.ssrLoadModule(
  "/src/presentation/TaskPresentationPanel.tsx",
);
after(async () => vite.close());

function progress(lastEvidenceAt) {
  return {
    task_id: "task-truth-1",
    computed_at: "2026-10-15T12:01:00Z",
    last_activity_at: "2026-10-15T12:00:00Z",
    last_activity_source: "ATTEMPT_EVENT",
    last_evidence_at: lastEvidenceAt,
    activity_summary: "A recent attempt update was observed.",
    active_workstreams: [{
      step_id: "step-truth-1",
      title: "Inspect the saved result",
      step_status: "RUNNING",
      active_attempt_ids: ["attempt-truth-1"],
      worker_labels: [],
      last_activity_at: "2026-10-15T12:00:00Z",
    }],
    blockers: [],
    newest_artifact: null,
  };
}

function taskSnapshot() {
  return [{
    item_key: "task-truth-card",
    kind: "TASK_CARD",
    payload_version: 1,
    source_refs: [{ kind: "TASK", id: "task-truth-1" }],
    order_key: "001",
    freshness: "CURRENT",
    status: "IN_PROGRESS",
    payload: { task_id: "task-truth-1", objective: "Inspect the saved result", status: "RUNNING" },
  }];
}

function renderTruth(progressValue) {
  return renderToStaticMarkup(createElement(Fragment, null,
    createElement(__TaskSnapshotViewForTest, { items: taskSnapshot() }),
    createElement(__TaskProgressSummaryForTest, { progress: progressValue }),
  ));
}

test("recent running-attempt activity without saved Evidence stays Working, not complete", () => {
  const html = renderTruth(progress(null));

  assert.match(html, /Working/);
  assert.match(html, /Latest saved progress/);
  assert.match(html, /Work attempt update/);
  assert.match(html, /A recent attempt update was observed\./);
  assert.match(html, /No saved evidence is available yet\./);
  assert.doesNotMatch(html, /Completed|Verified/);
});

test("a saved Evidence timestamp is labeled separately from activity", () => {
  const html = renderTruth(progress("2026-10-15T11:59:30Z"));

  assert.match(html, /Latest saved progress/);
  assert.match(html, /Work attempt update/);
  assert.match(html, /Latest saved evidence/);
  assert.doesNotMatch(html, /No saved evidence is available yet\./);
  assert.doesNotMatch(html, /Completed/);
});
