import assert from "node:assert/strict";
import test from "node:test";
import {
  createNotificationApi,
  NotificationApiError,
  type Notification,
  type NotificationPreference,
} from "../src/notifications/notification-api.ts";
import { maybeDesktopNotificationApi } from "../src/notifications/desktop-notification-api.ts";

const workspaceId = "workspace-1";

function notification(overrides: Partial<Notification> = {}): Notification {
  return {
    delivery_id: "delivery-1",
    workspace_id: workspaceId,
    dedupe_key: "automation-occurrence:occurrence-1",
    source_event_id: "event-1",
    channel_binding_id: "channel-1",
    attempt_runtime_id: null,
    attempt_host_epoch: null,
    status: "PENDING",
    attempt_count: 0,
    next_attempt_at: "2026-10-21T10:05:00Z",
    last_error_code: null,
    created_at: "2026-10-21T10:00:00Z",
    settled_at: null,
    ...overrides,
  };
}

function preference(overrides: Partial<NotificationPreference> = {}): NotificationPreference {
  return {
    workspace_id: workspaceId,
    event_class: "TASK_FAILED",
    policy: "ON_FAILURE",
    preferred_channels: ["Primary desktop"],
    quiet_hours: {
      timezone: "Europe/London",
      weekdays: ["MONDAY", "TUESDAY"],
      start_local_time: "22:00",
      end_local_time: "07:30",
    },
    version: 3,
    ...overrides,
  };
}

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
}

test("the delivery list sends its Workspace, filter, cursor and bounded page size", async () => {
  const calls: Array<{ path: string; init: RequestInit }> = [];
  const api = createNotificationApi(workspaceId, async (path, init) => {
    calls.push({ path, init });
    return json({ items: [notification()], next_cursor: "cursor-2" });
  });

  const page = await api.list("PENDING", "cursor-1");

  assert.equal(calls.length, 1);
  assert.equal(calls[0].path, "/v1/notifications?limit=50&status=PENDING&cursor=cursor-1");
  assert.equal(new Headers(calls[0].init.headers).get("X-Workspace-ID"), workspaceId);
  assert.equal(page.items.length, 1);
  assert.equal(page.items[0].delivery_id, "delivery-1");
  assert.equal(page.next_cursor, "cursor-2");
});

test("a delivery from another Workspace is refused rather than displayed", async () => {
  const api = createNotificationApi(workspaceId, async () => json({ items: [notification({ workspace_id: "workspace-2" })], next_cursor: null }));
  await assert.rejects(api.list(null, null), /different Workspace/);
});

test("an unknown delivery status or an invalid page cursor is rejected", async () => {
  const badStatus = createNotificationApi(workspaceId, async () => json({ items: [notification({ status: "DELIVERED" as Notification["status"] })], next_cursor: null }));
  await assert.rejects(badStatus.list(null, null), /Invalid Notification status/);

  const badCursor = createNotificationApi(workspaceId, async () => json({ items: [], next_cursor: 7 }));
  await assert.rejects(badCursor.list(null, null), /Invalid Notification cursor/);
});

test("preferences decode quiet hours and every policy value", async () => {
  const api = createNotificationApi(workspaceId, async path => {
    assert.equal(path, "/v1/notification-preferences");
    return json([preference(), preference({ event_class: "AUTOMATION_OCCURRENCE_SETTLED", policy: "ALWAYS", quiet_hours: null })]);
  });

  const preferences = await api.preferences();

  assert.equal(preferences.length, 2);
  assert.equal(preferences[0].quiet_hours?.end_local_time, "07:30");
  assert.equal(preferences[1].quiet_hours, null);
});

test("a preference with a bad policy, local time or Workspace is refused", async () => {
  const broken: Array<[NotificationPreference, RegExp]> = [
    [preference({ policy: "SOMETIMES" as NotificationPreference["policy"] }), /Invalid Notification policy/],
    [preference({ workspace_id: "workspace-2" }), /different Workspace/],
    [preference({ quiet_hours: { timezone: "Europe/London", weekdays: ["MONDAY"], start_local_time: "24:00", end_local_time: "07:00" } }), /Invalid Notification quiet hours/],
    [preference({ preferred_channels: ["desktop", "desktop"] }), /Invalid Notification channels/],
  ];
  for (const [value, expected] of broken) {
    const api = createNotificationApi(workspaceId, async () => json([value]));
    await assert.rejects(api.preferences(), expected);
  }
});

test("a preference write sends a versioned idempotent PUT and decodes the new version", async () => {
  const calls: Array<{ path: string; init: RequestInit }> = [];
  const api = createNotificationApi(workspaceId, async (path, init) => {
    calls.push({ path, init });
    return json(preference({ policy: "ALWAYS", version: 4 }));
  });

  const saved = await api.setPreference("TASK_FAILED", 3, {
    policy: "ALWAYS",
    preferred_channels: ["Primary desktop"],
    quiet_hours: null,
  }, "request-1");

  assert.equal(calls[0].path, "/v1/notification-preferences/TASK_FAILED");
  assert.equal(calls[0].init.method, "PUT");
  const headers = new Headers(calls[0].init.headers);
  assert.equal(headers.get("If-Match"), "\"3\"");
  assert.equal(headers.get("Idempotency-Key"), "request-1");
  assert.equal(headers.get("X-Workspace-ID"), workspaceId);
  assert.deepEqual(JSON.parse(String(calls[0].init.body)), {
    policy: "ALWAYS",
    preferred_channels: ["Primary desktop"],
    quiet_hours: null,
  });
  assert.equal(saved.version, 4);
});

test("an event class cannot escape its preference path segment", async () => {
  const api = createNotificationApi(workspaceId, async () => json(preference()));
  await assert.rejects(
    api.setPreference("../workspaces", 3, { policy: "ALWAYS", preferred_channels: ["Primary desktop"], quiet_hours: null }, "request-1"),
    /event class is invalid/,
  );
});

test("a first preference write may expect version zero", async () => {
  const api = createNotificationApi(workspaceId, async (_path, init) => {
    assert.equal(new Headers(init.headers).get("If-Match"), "\"0\"");
    return json(preference({ version: 1 }));
  });
  const saved = await api.setPreference("TASK_FAILED", 0, {
    policy: "ON_FAILURE",
    preferred_channels: ["Primary desktop"],
    quiet_hours: null,
  }, "request-1");
  assert.equal(saved.version, 1);
});

test("an archived Workspace refusal keeps its canonical code and status", async () => {
  const api = createNotificationApi(workspaceId, async () =>
    json({ code: "WORKSPACE_ARCHIVED", message: "private detail", retryable: false, correlation_id: "cor-1" }, 409));

  await assert.rejects(api.setPreference("TASK_FAILED", 3, {
    policy: "ALWAYS",
    preferred_channels: ["Primary desktop"],
    quiet_hours: null,
  }, "request-1"), error => {
    assert.ok(error instanceof NotificationApiError);
    assert.equal(error.code, "WORKSPACE_ARCHIVED");
    assert.equal(error.status, 409);
    assert.equal(error.message, "WORKSPACE_ARCHIVED");
    return true;
  });
});

test("an unselected Workspace has no desktop Notification API", () => {
  assert.equal(maybeDesktopNotificationApi(""), null);
  assert.equal(maybeDesktopNotificationApi("   "), null);
  assert.equal(maybeDesktopNotificationApi(workspaceId)?.workspaceId, workspaceId);
});
