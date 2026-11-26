import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { after, test } from "node:test";
import { createElement, Fragment } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createServer, createServerModuleRunnerTransport } from "vite";
import { ModuleRunner } from "vite/module-runner";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const panelPath = "/src/workbench/WorkbenchPanel.tsx";

const vite = await createServer({
  configFile: false,
  root: packageRoot,
  // Static SSR needs Vite's TSX conversion, not React Refresh/Babel work.
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
const { WorkbenchPanel, TerminalPane, ReviewPane, ActivityPane } = await runner.import(panelPath);

const render = node => renderToStaticMarkup(createElement(Fragment, null, node));

/**
 * Platform and process facts the mounted Workbench cannot back: this build serves no
 * shell runtime, no process-containment surface and no diff producer, so any of these
 * strings in the panel means it is claiming something it did not observe.
 */
const FABRICATED_PROCESS_CLAIMS = [
  "exit code",
  "ConPTY",
  "Job Object",
  "KILL_ON_CLOSE",
  "pwsh",
  "DACL",
  "v1.0.0-release",
];

function panelProps(overrides = {}) {
  return {
    panelRef: null,
    modal: false,
    workspaceId: "ws-alpha",
    workspaceName: "Workspace Alpha",
    runtimeState: "ONLINE",
    operatorReady: true,
    resources: [],
    resourceNextCursor: null,
    resourcePageBusy: false,
    resourceBusy: false,
    resourceListFailed: false,
    previewResourceId: null,
    previewResourceRevisionId: null,
    previewResourceName: null,
    preview: null,
    previewError: null,
    onClose() {},
    onOpenLibrary() {},
    onOpenWork() {},
    onOpenSettings() {},
    onLoadMoreResources() {},
    onRefreshResources() {},
    onPreviewResource() {},
    onClosePreview() {},
    ...overrides,
  };
}

test("the terminal pane executes nothing and states that plainly", () => {
  const html = render(createElement(TerminalPane, { platform: "Linux x86_64" }));
  assert.match(html, /Terminal execution is unavailable/);
  assert.match(html, /No terminal runtime served/);
  assert.ok(
    !html.includes("terminal-scrollback-screen"),
    "no shell surface may be rendered while no shell runtime is served",
  );
  for (const claim of FABRICATED_PROCESS_CLAIMS) {
    assert.ok(!html.includes(claim), `terminal pane must not claim ${JSON.stringify(claim)}`);
  }
});

test("terminal availability names the platform-specific Environment gap", () => {
  const windows = render(createElement(TerminalPane, { platform: "Win32" }));
  const linux = render(createElement(TerminalPane, { platform: "Linux x86_64" }));
  assert.match(windows, /qualified Windows Environment provider/);
  assert.match(linux, /admitted Environment for this session/);
});

test("activity has its own empty state", () => {
  const html = render(createElement(ActivityPane));
  assert.match(html, /No recorded activity source is served by this build/);
  assert.match(html, /Activity/);
  assert.doesNotMatch(html, /Nothing to review yet/);
});

test("the activity tab renders its own empty state instead of the review pane", () => {
  const html = render(createElement(WorkbenchPanel, panelProps({ initialTab: "activity" })));
  assert.match(html, /No recorded activity source is served by this build/);
  assert.doesNotMatch(html, /Nothing to review yet/);
  assert.match(html, /role="status" aria-labelledby="[^"]+" tabindex="0"/);
});

test("computer use is status-only", () => {
  const html = render(createElement(WorkbenchPanel, panelProps()));
  assert.match(html, /Computer use is unavailable in this build/);
  assert.match(html, /role="status" aria-label="Computer use availability"/);
  assert.doesNotMatch(html, /Computer use[^<]*(button|action)/i);
});

test("narrow Workbench supports Escape close and ordered keyboard navigation", async () => {
  const source = await readFile(join(packageRoot, "src/App.tsx"), "utf8");
  assert.match(source, /event\.key === "Escape"\)[\s\S]{0,100}closeWorkbench\(\)/);
  assert.match(source, /querySelectorAll<HTMLElement>\([\s\S]{0,180}\[tabindex\]:not\(\[tabindex="-1"\]\)/);
  const html = render(createElement(WorkbenchPanel, panelProps({ initialTab: "activity" })));
  assert.ok(html.indexOf("workbench-tools-nav") < html.indexOf("workbench-activity-view"));
});

test("the review pane renders no diff and no change statistics", () => {
  const html = render(createElement(ReviewPane));
  assert.match(html, /Nothing to review yet/);
  assert.ok(
    !html.includes("diff-line line-add") && !html.includes("diff-line line-del"),
    "no diff rows may be rendered without a diff source",
  );
  assert.ok(
    !html.includes("additions") && !html.includes("deletions"),
    "no change statistics may be rendered without a diff source",
  );
});

test("the panel shows the real Workspace and operator state", () => {
  const html = render(createElement(WorkbenchPanel, panelProps()));
  assert.match(html, /Workspace Alpha/);
  assert.match(html, /Operator connected/);
  assert.ok(html.includes("ONLINE"), "the reported runtime state is shown as reported");
  for (const claim of FABRICATED_PROCESS_CLAIMS) {
    assert.ok(!html.includes(claim), `Workbench must not claim ${JSON.stringify(claim)}`);
  }
});

test("the panel reports a missing operator connection as missing", () => {
  const html = render(
    createElement(WorkbenchPanel, panelProps({ operatorReady: false, runtimeState: null })),
  );
  assert.match(html, /Operator not connected/);
  assert.ok(
    !html.includes("Operator connected<!"),
    "a disconnected operator must not read as connected",
  );
});

test("the panel shows no Workspace rather than inventing one", () => {
  const html = render(
    createElement(WorkbenchPanel, panelProps({ workspaceId: "", workspaceName: "" })),
  );
  assert.match(html, /No Workspace selected/);
});

test("the Workbench module keeps no fabricated process, diff or containment claim", async () => {
  const source = await readFile(join(packageRoot, "src/workbench/WorkbenchPanel.tsx"), "utf8");
  for (const claim of FABRICATED_PROCESS_CLAIMS) {
    assert.ok(
      !source.includes(claim),
      `WorkbenchPanel.tsx must not reintroduce the claim ${JSON.stringify(claim)}`,
    );
  }
  assert.doesNotMatch(source, /Sandboxed/, "the browser pane may not claim containment");
  assert.doesNotMatch(source, /127\.0\.0\.1:\d+/, "no invented loopback endpoint may be the default");
  assert.doesNotMatch(source, /selectedDiffFile/, "no diff selection may be faked from local state");
  assert.doesNotMatch(source, /idx % \d+ === \d+/, "no synthetic diff may be generated from text");
  assert.doesNotMatch(source, /additions \/ -1 deletions/);
  assert.match(source, /Embedded page preview/, "the browser pane states what it really is");
});

test("no iframe combines scripts with same-origin access", async () => {
  const source = await readFile(join(packageRoot, "src/workbench/WorkbenchPanel.tsx"), "utf8");
  const iframeOpeningTags = [...source.matchAll(/<iframe\b[^>]*>/gi)].map((match) => match[0]);
  const sandboxAttributes = iframeOpeningTags
    .map((tag) => tag.match(/\bsandbox="([^"]*)"/i)?.[1])
    .filter((value) => value !== undefined)
    .map((value) => value.split(/\s+/));

  assert.equal(
    sandboxAttributes.length,
    iframeOpeningTags.length,
    "every iframe must declare an explicit sandbox policy",
  );

  for (const tokens of sandboxAttributes) {
    assert.ok(
      !(tokens.includes("allow-scripts") && tokens.includes("allow-same-origin")),
      "an iframe must not combine script execution with same-origin privileges",
    );
  }
});
