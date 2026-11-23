import assert from "node:assert/strict";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import react from "@vitejs/plugin-react";
import { createServer } from "vite";

const vite = await createServer({
  configFile: false,
  root: fileURLToPath(new URL("..", import.meta.url)),
  plugins: [react()],
  server: { middlewareMode: true, hmr: false },
  optimizeDeps: { noDiscovery: true, include: [] },
  appType: "custom",
});
const { TurnActivityView } = await vite.ssrLoadModule("/src/conversations/TurnActivityView.tsx");
const { parseActivityEntry } = await vite.ssrLoadModule("/src/conversations/turn-activity.ts");
after(async () => vite.close());

test("renders reported activity collapsed and keeps failed observations clearly attributed", () => {
  const entries = [
    parseActivityEntry({ sequence: 1, kind: "TOOL", status: "STARTED", call_ref: "c1", label: "build" }),
    parseActivityEntry({ sequence: 2, kind: "TOOL", status: "FAILED", call_ref: "c1", label: null }),
  ];
  assert.ok(entries.every(Boolean));
  const html = renderToStaticMarkup(createElement(TurnActivityView, { entries }));
  assert.match(html, /<details class="turn-activity" data-testid="turn-activity">/);
  assert.match(html, /Activity reported by the agent \(1\)/);
  assert.match(html, /Tool build/);
  assert.match(html, /Reported by the agent/);
  assert.match(html, /failed/);
  assert.doesNotMatch(html, /Effect receipt|verified file change/);
});
