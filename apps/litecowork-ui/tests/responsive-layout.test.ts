import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const [styles, conversationStyles, app, tauriConfig] = await Promise.all([
  readFile(resolve(root, "src/styles.css"), "utf8"),
  readFile(resolve(root, "src/conversations/conversations-page.css"), "utf8"),
  readFile(resolve(root, "src/App.tsx"), "utf8"),
  readFile(resolve(root, "src-tauri/tauri.conf.json"), "utf8").then(JSON.parse),
]);

test("desktop shell does not impose a fixed width at 200% zoom", () => {
  const bodyRule = styles.match(/\bbody\s*\{([^}]*)\}/)?.[1] ?? "";
  assert.ok(!/\bmin-width\s*:/.test(bodyRule), "body must allow the viewport to shrink");
  assert.ok(tauriConfig.app.windows[0].minWidth <= 480);
});

test("narrow navigation is a labeled, controlled drawer with keyboard dismissal", () => {
  assert.ok(/@media\s*\(max-width:\s*900px\)/.test(styles), "drawer breakpoint is defined");
  assert.ok(/\.icon-button\.mobile-navigation-toggle[^}]*display:\s*none/.test(styles), "mobile menu toggle is hidden at desktop widths even with generic icon-button display rules");
  assert.ok(/\.sidebar\.is-open/.test(styles), "drawer has an explicit open state");
  assert.ok(/\.sidebar-backdrop/.test(styles), "drawer has a dismissible backdrop");
  assert.ok(/aria-controls="main-navigation"/.test(app), "menu control names its target");
  assert.ok(/aria-expanded=\{navigationOpen\}/.test(app), "menu control exposes expanded state");
  assert.ok(/event\.key\s*===\s*"Escape"/.test(app), "Escape closes the drawer");
  assert.ok(/aria-label="Close navigation"/.test(app), "drawer has a named close control");
  assert.ok(/querySelector<HTMLElement>\("\.sidebar-close"\)\?\.focus\(\)/.test(app), "opening the drawer moves focus to its close control");
});

test("narrow content uses compact single-column spacing", () => {
  assert.ok(/@media\s*\(max-width:\s*620px\)[\s\S]*?\.page-content\s*\{[^}]*padding/.test(styles), "page padding compacts on narrow views");
  assert.ok(/\.conversation-layout\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*1fr\)/.test(conversationStyles), "conversation list stacks above its reader at narrow widths");
});
