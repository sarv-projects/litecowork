import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const navigationSource = await readFile(new URL("../src/resources/library-navigation.tsx", import.meta.url), "utf8");
const appSource = await readFile(new URL("../src/App.tsx", import.meta.url), "utf8");

function productionFunction(name: string) {
  const declaration = navigationSource.match(new RegExp(`^export function ${name}\\(([\\s\\S]*?)\\)(?:\\s*:[^\\{\\n]+)?\\s*\\{([\\s\\S]*?)^\\}`, "m"));
  assert.ok(declaration, `The production Library navigation module must export ${name} as a directly testable decision.`);
  const parameters = declaration[1].split(",").map((parameter) => parameter.trim().split(":", 1)[0].trim()).join(",");
  return new Function(`return function ${name}(${parameters}) {${declaration[2]}}`)() as (...args: unknown[]) => unknown;
}

test("leaving Library with an unsaved Resource draft requires an explicit choice", () => {
  const decide = productionFunction("libraryNavigationDecision");

  assert.equal(decide("Library", "Work", true, null), "CONFIRM");
  assert.equal(decide("Library", "Settings", true, "STAY"), "STAY");
  assert.equal(decide("Library", "Settings", true, "SAVE"), "SAVE");
  assert.equal(decide("Library", "Settings", true, "DISCARD"), "DISCARD");
});

test("navigation remains unblocked when Library has no dirty Resource editor", () => {
  const decide = productionFunction("libraryNavigationDecision");

  assert.equal(decide("Library", "Work", false, null), "NAVIGATE");
  assert.equal(decide("Library", "Work", false, "DISCARD"), "NAVIGATE");
  assert.equal(decide("Home", "Library", true, null), "NAVIGATE");
  assert.equal(decide("Library", "Library", true, null), "NAVIGATE");
});

test("the Resource editor exposes explicit dirty and clean draft states to the Library guard", () => {
  const hasDraft = productionFunction("resourceEditorHasUnsavedDraft");

  assert.equal(hasDraft("DIRTY"), true);
  assert.equal(hasDraft("CLEAN"), false);
  assert.equal(hasDraft(null), false);
});

test("a pending Library guard makes the production main area inert", () => {
  const isMainInert = productionFunction("libraryNavigationMainIsInert");

  assert.equal(isMainInert(false, false), false);
  assert.equal(isMainInert(true, false), true);
  assert.equal(isMainInert(false, true), true);
  assert.match(appSource, /libraryNavigationMainIsInert\(navigationOpen, pendingLibraryNavigation !== null\)/);
  assert.match(appSource, /<main className="main-area" inert=\{mainAreaInert\} aria-hidden=\{mainAreaInert \|\| undefined\}>/);
});
