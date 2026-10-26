import assert from "node:assert/strict";
import test from "node:test";
import { AUTOMATION_TRIGGER_OPTIONS, automationSetupSteps, isSupportedAutomationTriggerKind, unsupportedAutomationTriggerKinds } from "../src/automations/automation-setup.ts";
import * as automationSetup from "../src/automations/automation-setup.ts";

const base = {
  status: "PAUSED" as const,
  hasDefinition: true,
  hasTestTask: false,
  routineActive: true,
  hostReady: true,
  triggerSupported: true,
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

test("an enabled Automation with a missing pinned definition is not presented as complete", () => {
  const steps = automationSetupSteps({ ...base, status: "ENABLED", hasDefinition: false });
  assert.equal(steps.find(step => step.id === "REVIEW")?.state, "BLOCKED");
  assert.equal(steps.find(step => step.id === "ENABLE")?.state, "BLOCKED");
  assert.match(steps.find(step => step.id === "ENABLE")?.detail ?? "", /stored status is enabled.*definition/i);
});

test("an enabled Automation with an archived pinned Routine is not presented as complete", () => {
  const enable = automationSetupSteps({ ...base, status: "ENABLED", routineActive: false })
    .find(step => step.id === "ENABLE");
  assert.equal(enable?.state, "BLOCKED");
  assert.match(enable?.detail ?? "", /stored status is enabled.*routine/i);
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

test("one-time triggers remain visible as unavailable and cannot be selected", () => {
  const oneTime = AUTOMATION_TRIGGER_OPTIONS.find(option => option.kind === "ONE_SHOT");
  assert.ok(oneTime);
  assert.equal(oneTime.supported, false);
  assert.match(oneTime.reason, /not supported/i);
  assert.equal(isSupportedAutomationTriggerKind("ONE_SHOT"), false);
  assert.equal(isSupportedAutomationTriggerKind("SCHEDULE"), true);
  assert.equal(isSupportedAutomationTriggerKind("MANUAL"), true);
  assert.deepEqual(unsupportedAutomationTriggerKinds(["SCHEDULE", "ONE_SHOT", "WEBHOOK", "ONE_SHOT"]), ["ONE_SHOT", "WEBHOOK"]);
});

test("unsupported trigger types block enable without rewriting the saved definition", () => {
  const steps = automationSetupSteps({ ...base, triggerSupported: false });
  const enable = steps.find(step => step.id === "ENABLE");
  assert.equal(enable?.state, "BLOCKED");
  assert.match(enable?.detail ?? "", /cannot host one or more trigger types/i);
  assert.equal(steps.find(step => step.id === "REVIEW")?.state, "DONE");
});

test("an enabled definition with unsupported local triggers is not presented as complete", () => {
  const enable = automationSetupSteps({ ...base, status: "ENABLED", triggerSupported: false })
    .find(step => step.id === "ENABLE");

  assert.equal(enable?.state, "BLOCKED");
  assert.match(enable?.detail ?? "", /stored status is enabled.*cannot host/i);
});

test("numeric five-field schedules enforce the local host's field bounds", () => {
  const validate = (automationSetup as unknown as { isValidLocalCron5?: (expression: string) => boolean }).isValidLocalCron5;
  assert.equal(typeof validate, "function");

  assert.equal(validate?.("*/15 9 1-31 1-12 1-5"), true);
  assert.equal(validate?.("99 99 * * *"), false);
  assert.equal(validate?.("60 9 * * *"), false);
  assert.equal(validate?.("0 24 * * *"), false);
  assert.equal(validate?.("0 9 0 * *"), false);
  assert.equal(validate?.("0 9 * * 8"), false);
  assert.equal(validate?.("0 9 31-1 * *"), false);
});

test("editor schedule input is numeric and range-valid before save", () => {
  const validate = (automationSetup as unknown as { isValidEditorCron5?: (expression: string) => boolean }).isValidEditorCron5;
  assert.equal(typeof validate, "function");

  assert.equal(validate?.("*/15 9 1-31 1-12 1-5"), true);
  assert.equal(validate?.("99 99 * * *"), false);
  assert.equal(validate?.("0 9 * JAN MON"), false);
});
