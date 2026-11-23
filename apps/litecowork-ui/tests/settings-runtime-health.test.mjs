import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { deriveRuntimeHealthRows, isRuntimeExecutionReady } from "../src/runtime/runtime-health.ts";

test("STOPPED, STARTING and RECOVERING never show execution open", () => {
  for (const recoveryState of ["STOPPED", "STARTING", "RECOVERING", "DRAINING", "STOPPING", null]) {
    for (const operatorReady of [true, false]) {
      const rows = deriveRuntimeHealthRows({ operatorReady, availability: "ONLINE", recoveryState }, "Linux x86_64");
      assert.equal(rows.executionReady, false, `${recoveryState}/${operatorReady}`);
      assert.match(rows.execution, /^Closed/);
    }
  }
});

test("DEGRADED while Operator is serving keeps commands available and execution closed", () => {
  const rows = deriveRuntimeHealthRows({ operatorReady: true, availability: "DEGRADED", recoveryState: "DEGRADED" }, "Linux");
  assert.equal(rows.executionReady, false);
  assert.match(rows.operator, /^Serving/);
  assert.match(rows.execution, /Operator commands remain available/);
});

test("READY and serving is the only open execution state", () => {
  const rows = deriveRuntimeHealthRows({ operatorReady: true, availability: "ONLINE", recoveryState: "READY" }, "Linux");
  assert.equal(rows.executionReady, true);
  assert.equal(rows.execution, "Open");
  assert.equal(deriveRuntimeHealthRows({ operatorReady: false, availability: "ONLINE", recoveryState: "READY" }, "Linux").executionReady, false);
});

test("READY incarnation with non-ONLINE Runtime does not enable dispatch", () => {
  const rows = deriveRuntimeHealthRows({ operatorReady: true, availability: "DEGRADED", recoveryState: "READY" }, "Linux");
  assert.equal(rows.executionReady, false);
  assert.match(rows.execution, /^Closed/);
});

test("Conversation execution requires the exact current lifecycle identity", () => {
  const ready = {
    operatorReady: true,
    runtimeId: "runtime-1",
    incarnationId: "incarnation-1",
    lifecycleIdentity: "runtime-1:incarnation-1",
    lifecycleRuntimeId: "runtime-1",
    lifecycleIncarnationId: "incarnation-1",
    availability: "ONLINE",
    recoveryState: "READY",
  };
  assert.equal(isRuntimeExecutionReady(ready), true);
  assert.equal(isRuntimeExecutionReady({ ...ready, lifecycleIdentity: "runtime-1:old-incarnation" }), false);
  assert.equal(isRuntimeExecutionReady({ ...ready, lifecycleIncarnationId: "old-incarnation" }), false);
  assert.equal(isRuntimeExecutionReady({ ...ready, operatorReady: false }), false);
});

test("transport label names the real transport per platform", () => {
  assert.equal(deriveRuntimeHealthRows({ operatorReady: true, availability: "ONLINE", recoveryState: "READY" }, "Win32").transport, "Local IPC (named pipe)");
  assert.equal(deriveRuntimeHealthRows({ operatorReady: true, availability: "ONLINE", recoveryState: "READY" }, "Linux x86_64").transport, "Local IPC (socket)");
});

test("Settings no longer says Localhost and renders loading and error states in live regions", async () => {
  const source = await readFile(new URL("../src/settings/SettingsView.tsx", import.meta.url), "utf8");
  assert.doesNotMatch(source, /Localhost/);
  assert.match(source, /props\.runtimeLifecycleLoading/);
  assert.match(source, /props\.runtimeLifecycleError/);
  assert.match(source, /props\.runtimeError/);
  assert.match(source, /role="status"/);
  assert.match(source, /role="alert"/);
});
