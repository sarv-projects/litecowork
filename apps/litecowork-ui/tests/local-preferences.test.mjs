import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import {
  DEFAULT_LOCAL_PREFERENCES,
  LOCAL_PREFERENCES_STORAGE_KEY,
  applyLocalPreferences,
  loadLocalPreferences,
  parseLocalPreferences,
  saveLocalPreferences,
} from "../src/settings/local-preferences.ts";

function memoryStorage(initial = {}) {
  const map = new Map(Object.entries(initial));
  return {
    getItem: key => (map.has(key) ? map.get(key) : null),
    setItem: (key, value) => void map.set(key, value),
  };
}

function fakeRoot() {
  const attributes = {};
  const properties = {};
  return {
    attributes,
    properties,
    setAttribute: (name, value) => void (attributes[name] = value),
    style: { setProperty: (name, value) => void (properties[name] = value) },
  };
}

test("a saved preference survives a reload from the same available storage", () => {
  const storage = memoryStorage();
  const chosen = { theme: "dark", density: "compact", textScale: "125", reducedMotion: true, richPresentationEnabled: false };
  assert.equal(saveLocalPreferences(storage, chosen), true);
  assert.deepEqual(loadLocalPreferences(storage), chosen);
});

test("older preference snapshots keep rich presentation enabled by default", () => {
  const older = JSON.stringify({ theme: "dark", density: "compact", textScale: "110", reducedMotion: true });
  assert.equal(parseLocalPreferences(older).richPresentationEnabled, true);
});

test("changing a value applies it to the document root immediately", () => {
  const root = fakeRoot();
  applyLocalPreferences(root, { theme: "dark", density: "compact", textScale: "110", reducedMotion: true });
  assert.equal(root.attributes["data-theme"], "dark");
  assert.equal(root.attributes["data-density"], "compact");
  assert.equal(root.attributes["data-reduced-motion"], "true");
  assert.equal(root.properties["--lc-text-scale"], "1.1");
});

test("corrupt stored JSON resets to defaults without throwing", () => {
  for (const raw of ["{not json", "null", "[]", "42", '"dark"', ""]) {
    assert.deepEqual(parseLocalPreferences(raw), DEFAULT_LOCAL_PREFERENCES, raw);
  }
  const storage = memoryStorage({ [LOCAL_PREFERENCES_STORAGE_KEY]: "{oops" });
  assert.deepEqual(loadLocalPreferences(storage), DEFAULT_LOCAL_PREFERENCES);
});

test("unknown field values fall back per field and valid fields are kept", () => {
  const parsed = parseLocalPreferences(JSON.stringify({ theme: "neon", density: "compact", textScale: "300", reducedMotion: "yes" }));
  assert.deepEqual(parsed, { ...DEFAULT_LOCAL_PREFERENCES, density: "compact" });
});

test("storage failures never throw and report non-persistence", () => {
  const throwing = {
    getItem() { throw new Error("denied"); },
    setItem() { throw new Error("quota"); },
  };
  assert.deepEqual(loadLocalPreferences(throwing), DEFAULT_LOCAL_PREFERENCES);
  assert.equal(saveLocalPreferences(throwing, DEFAULT_LOCAL_PREFERENCES), false);
  assert.equal(saveLocalPreferences(null, DEFAULT_LOCAL_PREFERENCES), false);
});

test("preferences module and App wiring make no Operator call (AC-004)", async () => {
  const module = await readFile(new URL("../src/settings/local-preferences.ts", import.meta.url), "utf8");
  assert.doesNotMatch(module, /fetch\(|invoke\(|operator|Idempotency|If-Match/i);
  const app = await readFile(new URL("../src/App.tsx", import.meta.url), "utf8");
  const handler = app.match(/const changeLocalPreferences = useCallback\([\s\S]*?\}, \[\]\);/);
  assert.ok(handler, "changeLocalPreferences handler present");
  assert.doesNotMatch(handler[0], /operator|fetch|invoke|request/i);
  assert.match(app, /richPresentationEnabled=\{localPreferences\.richPresentationEnabled\}/, "the saved local display choice reaches the Conversation view");
  const settings = await readFile(new URL("../src/settings/SettingsView.tsx", import.meta.url), "utf8");
  assert.match(settings, /data-control-id="pref-rich-presentation"/);
  assert.match(settings, /aria-labelledby="pref-rich-presentation-label"/);
  assert.match(settings, /localPreferenceSaveFailed/);
  assert.match(settings, /only active for this session/i);
  assert.match(app, /return saveLocalPreferences\(browserPreferenceStorage\(\), next\)/);
});

test("Settings no longer shows display-only shortcut labels or component-local appearance state", async () => {
  const view = await readFile(new URL("../src/settings/SettingsView.tsx", import.meta.url), "utf8");
  assert.doesNotMatch(view, /<kbd/);
  assert.doesNotMatch(view, /useState<"system" \| "light" \| "dark">/);
});
