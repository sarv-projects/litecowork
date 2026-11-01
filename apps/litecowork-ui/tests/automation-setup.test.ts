import assert from "node:assert/strict";
import test from "node:test";
import { automationSetupSteps } from "../src/automations/automation-setup.ts";

const base = {
  status: "PAUSED" as const,
  hasDefinition: true,
  hasTestTask: false,
  routineActive: true,
  hostReady: true,
};

test("a paused revision with no test Task points to the real test step and enable step", () => {
  const steps = automationSetupSteps(base);
  assert.deepEqual(steps.map(step => [step.id, step.state]), [
    ["REVIEW", "DONE"],
    ["TEST", "TODO"],
    ["ENABLE", "TODO"],
  ]);
  assert.match(steps[1].detail, /real work/);
});

test("a completed test does not imply the Automation is enabled", () => {
  const steps = automationSetupSteps({ ...base, hasTestTask: true });
  assert.equal(steps.find(step => step.id === "TEST")?.state, "DONE");
  assert.equal(steps.find(step => step.id === "ENABLE")?.state, "TODO");
});

test("an enabled Automation marks enable done but a missing definition is still blocked", () => {
  const steps = automationSetupSteps({ ...base, status: "ENABLED", hasDefinition: false });
  assert.equal(steps.find(step => step.id === "REVIEW")?.state, "BLOCKED");
  assert.equal(steps.find(step => step.id === "ENABLE")?.state, "DONE");
});

test("disabled is terminal for the test and enable steps", () => {
  const steps = automationSetupSteps({ ...base, status: "DISABLED", hasTestTask: true });
  assert.equal(steps.find(step => step.id === "TEST")?.state, "DONE");
  assert.equal(steps.find(step => step.id === "ENABLE")?.state, "BLOCKED");
  assert.match(steps.find(step => step.id === "ENABLE")?.detail ?? "", /terminal/);
});

test("missing host readiness is a blocker; unknown readiness is an explicit recheck", () => {
  const blocked = automationSetupSteps({ ...base, hostReady: false });
  assert.equal(blocked.find(step => step.id === "ENABLE")?.state, "BLOCKED");
  const unknown = automationSetupSteps({ ...base, hostReady: null });
  assert.equal(unknown.find(step => step.id === "ENABLE")?.state, "TODO");
  assert.match(unknown.find(step => step.id === "ENABLE")?.detail ?? "", /recheck/);
});
