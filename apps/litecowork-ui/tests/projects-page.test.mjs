import assert from "node:assert/strict";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import react from "@vitejs/plugin-react";
import { createServer } from "vite";

const cssStubId = "\0litecowork-projects-test-css";
const vite = await createServer({
  configFile: false,
  root: fileURLToPath(new URL("..", import.meta.url)),
  plugins: [
    {
      name: "projects-component-test-css-stub",
      enforce: "pre",
      resolveId(source) {
        if (source.split("?", 1)[0].endsWith(".css")) return cssStubId;
        return null;
      },
      load(id) {
        if (id === cssStubId) return "export default {};";
        return null;
      },
    },
    react(),
  ],
  server: { middlewareMode: true, hmr: false },
  optimizeDeps: { noDiscovery: true, include: [] },
  appType: "custom",
});
const { WorkspaceProjectsPage } = await vite.ssrLoadModule("/src/projects/WorkspaceProjectsPage.tsx");
after(async () => vite.close());

const base = {
  workspaces: [],
  selectedWorkspaceId: "",
  runtimeReady: true,
  onRetry: () => {},
  onOpenWorkspace: () => {},
  onOpenSettings: () => {},
};

test("Projects distinguishes a pending Workspace list from an empty list", () => {
  const loading = renderToStaticMarkup(createElement(WorkspaceProjectsPage, {
    ...base,
    listState: "LOADING",
    error: null,
  }));
  assert.match(loading, /Loading Workspaces/);
  assert.doesNotMatch(loading, /No Workspaces yet/);

  const empty = renderToStaticMarkup(createElement(WorkspaceProjectsPage, {
    ...base,
    listState: "READY",
    error: null,
  }));
  assert.match(empty, /No Workspaces yet/);
  assert.doesNotMatch(empty, /Loading Workspaces/);
});

test("Projects hides cached Workspace rows while the local Runtime is unavailable", () => {
  const html = renderToStaticMarkup(createElement(WorkspaceProjectsPage, {
    ...base,
    runtimeReady: false,
    listState: "READY",
    error: null,
    selectedWorkspaceId: "workspace-current",
    workspaces: [
      { workspaceId: "workspace-current", name: "Previously loaded", status: "ACTIVE", version: 3, replicationPolicy: "LOCAL_ONLY", defaultAgentBindingId: null },
    ],
  }));
  assert.match(html, /Local Runtime is unavailable/);
  assert.match(html, /Open Settings/);
  assert.doesNotMatch(html, /Previously loaded/);
  assert.doesNotMatch(html, /No Workspaces yet/);
});

test("Projects exposes Workspace-list failures with a retry and does not render them as empty", () => {
  const html = renderToStaticMarkup(createElement(WorkspaceProjectsPage, {
    ...base,
    listState: "ERROR",
    error: "Workspace list is unavailable.",
  }));
  assert.match(html, /role="alert"/);
  assert.match(html, /Workspace list is unavailable\./);
  assert.match(html, /Retry/);
  assert.doesNotMatch(html, /No Workspaces yet/);
});

test("Projects presents existing Workspace rows and identifies the active security boundary", () => {
  const html = renderToStaticMarkup(createElement(WorkspaceProjectsPage, {
    ...base,
    listState: "READY",
    error: null,
    selectedWorkspaceId: "workspace-current",
    workspaces: [
      { workspaceId: "workspace-current", name: "Design research", status: "ACTIVE", version: 3, replicationPolicy: "LOCAL_ONLY", defaultAgentBindingId: null },
      { workspaceId: "workspace-archived", name: "Archived notes", status: "ARCHIVED", version: 6, replicationPolicy: "LOCAL_ONLY", defaultAgentBindingId: null },
    ],
  }));
  assert.match(html, /<h1>Projects<\/h1>/);
  assert.match(html, /Each project is an existing Workspace/);
  assert.match(html, /Design research/);
  assert.match(html, /Archived notes/);
  assert.match(html, /aria-pressed="true"/);
  assert.match(html, /Workspace is the security and resource boundary/);
  assert.doesNotMatch(html, /C:\\|\/home\//);
});
