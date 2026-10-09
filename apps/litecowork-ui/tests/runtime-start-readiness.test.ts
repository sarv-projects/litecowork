import test from "node:test";
import assert from "node:assert/strict";
import { canOfferLocalRuntimeStart } from "../src/runtime/runtime-start-readiness.ts";

test("offers first start when daemon is installed but Linux service is not yet verified", () => {
  assert.equal(canOfferLocalRuntimeStart({
    state: "DEGRADED",
    processRunning: false,
    operatorReady: false,
    daemonAvailable: true,
  }), true);
});

test("does not offer a second start while a daemon process has not authenticated readiness", () => {
  assert.equal(canOfferLocalRuntimeStart({
    state: "DEGRADED",
    processRunning: true,
    operatorReady: false,
    daemonAvailable: true,
  }), false);
});

test("does not offer startup after readiness or when the daemon executable is missing", () => {
  assert.equal(canOfferLocalRuntimeStart({
    state: "READY",
    processRunning: true,
    operatorReady: true,
    daemonAvailable: true,
  }), false);
  assert.equal(canOfferLocalRuntimeStart({
    state: "UNAVAILABLE",
    processRunning: false,
    operatorReady: false,
    daemonAvailable: false,
  }), false);
  assert.equal(canOfferLocalRuntimeStart(null), false);
});
