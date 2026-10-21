import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

/** Stylesheets owned by the E08 surfaces this agent may change. */
const SURFACE_STYLESHEETS = [
  "src/coworkers/coworker-settings.css",
  "src/goals/goals-page.css",
  "src/presentation/presentation-runtime.css",
  "src/presentation/rich-response.css",
  "src/presentation/task-presentation.css",
  "src/artifacts/artifact-workbench.css",
  "src/artifacts/structured-text-preview.css",
  "src/artifacts/text-version-diff.css",
];

async function readStylesheet(relativePath: string): Promise<string> {
  return readFile(join(packageRoot, relativePath), "utf8");
}

test("no E08 surface animates continuously", async () => {
  for (const relativePath of SURFACE_STYLESHEETS) {
    const css = await readStylesheet(relativePath);
    assert.doesNotMatch(
      css,
      /\binfinite\b/,
      `${relativePath} must not use a continuous animation (docs/MOTION.md)`,
    );
    assert.doesNotMatch(
      css,
      /animation[^;]*\b(shimmer|pulse|loop)\b/i,
      `${relativePath} must not use a decorative loop (docs/MOTION.md)`,
    );
  }
});

test("every E08 surface that animates also honours the reduced-motion preference", async () => {
  for (const relativePath of SURFACE_STYLESHEETS) {
    const css = await readStylesheet(relativePath);
    const animates = /(?:^|[;{\s])(?:animation|transition)\s*:/.test(css);
    if (!animates) continue;
    assert.match(
      css,
      /@media\s*\(prefers-reduced-motion:\s*reduce\)/,
      `${relativePath} animates and must honour prefers-reduced-motion (docs/MOTION.md)`,
    );
  }
});

test("the Goals surface shows a static busy state instead of a spinner", async () => {
  const css = await readStylesheet("src/goals/goals-page.css");
  assert.doesNotMatch(css, /goal-spinner/, "the looping Goals spinner is removed");
  assert.match(css, /\.goal-busy-mark\b/, "the Goals busy state has a static marker");
});
