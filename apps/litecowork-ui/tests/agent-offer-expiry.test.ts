import assert from "node:assert/strict";
import test from "node:test";
import { scheduleOfferExpiryRefresh } from "../src/agents/agent-offer-expiry.ts";

test("schedules one refresh for the earliest future offer expiration", () => {
  let scheduledDelay = -1;
  let scheduled: (() => void) | null = null;
  let cancelled: ReturnType<typeof setTimeout> | null = null;
  let refreshes = 0;
  const cancel = scheduleOfferExpiryRefresh([
    { offerExpiresAt: "2030-01-01T00:00:10.000Z" },
    { offerExpiresAt: "2030-01-01T00:00:30.000Z" },
    { offerExpiresAt: "invalid" },
    { offerExpiresAt: "2029-12-31T23:59:59.000Z" },
  ], () => { refreshes += 1; }, 1_893_456_000_000, (callback, delay) => {
    scheduled = callback;
    scheduledDelay = delay;
    return 42 as unknown as ReturnType<typeof setTimeout>;
  }, handle => { cancelled = handle; });

  assert.equal(scheduledDelay, 10_000);
  assert.equal(refreshes, 0);
  scheduled?.();
  assert.equal(refreshes, 1);
  cancel();
  assert.equal(cancelled, 42);
});

test("does not schedule a refresh when all offers are expired or invalid", () => {
  let scheduled = false;
  const cancel = scheduleOfferExpiryRefresh([
    { offerExpiresAt: "2029-12-31T23:59:59.000Z" },
    { offerExpiresAt: "invalid" },
  ], () => assert.fail("expired offers must not refresh"), 1_893_456_000_000, () => {
    scheduled = true;
    return 42 as unknown as ReturnType<typeof setTimeout>;
  });

  cancel();
  assert.equal(scheduled, false);
});
