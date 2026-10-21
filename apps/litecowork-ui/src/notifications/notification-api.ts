export type NotificationStatus = "PENDING" | "SENDING" | "SENT" | "FAILED" | "AMBIGUOUS" | "SUPPRESSED";
export type NotificationPolicy = "ALWAYS" | "ON_SUCCESS" | "ON_FAILURE" | "ON_CONDITION" | "SILENT";
export type NotificationWeekday = "MONDAY" | "TUESDAY" | "WEDNESDAY" | "THURSDAY" | "FRIDAY" | "SATURDAY" | "SUNDAY";

export type QuietHours = {
  timezone: string;
  weekdays: NotificationWeekday[];
  start_local_time: string;
  end_local_time: string;
};

export type Notification = {
  delivery_id: string;
  workspace_id: string;
  dedupe_key: string;
  source_event_id: string;
  channel_binding_id: string | null;
  attempt_runtime_id: string | null;
  attempt_host_epoch: number | null;
  status: NotificationStatus;
  attempt_count: number;
  next_attempt_at: string | null;
  last_error_code: string | null;
  created_at: string;
  settled_at: string | null;
};

export type NotificationPage = { items: Notification[]; next_cursor: string | null };

export type NotificationPreference = {
  workspace_id: string;
  event_class: string;
  policy: NotificationPolicy;
  preferred_channels: string[];
  quiet_hours: QuietHours | null;
  version: number;
};

export type NotificationPreferenceUpdate = {
  policy: NotificationPolicy;
  preferred_channels: string[];
  quiet_hours: QuietHours | null;
};

/** The notification API runs over a caller-supplied transport so tests and the
 * desktop bridge share one decoder and one request shape. */
export type NotificationTransport = (path: string, init: RequestInit) => Promise<Response>;

export type NotificationApi = {
  workspaceId: string;
  list(status: NotificationStatus | null, cursor: string | null, signal?: AbortSignal): Promise<NotificationPage>;
  preferences(signal?: AbortSignal): Promise<NotificationPreference[]>;
  setPreference(
    eventClass: string,
    expectedVersion: number,
    update: NotificationPreferenceUpdate,
    requestId: string,
  ): Promise<NotificationPreference>;
};

const STATUSES: readonly NotificationStatus[] = ["PENDING", "SENDING", "SENT", "FAILED", "AMBIGUOUS", "SUPPRESSED"];
const POLICIES: readonly NotificationPolicy[] = ["ALWAYS", "ON_SUCCESS", "ON_FAILURE", "ON_CONDITION", "SILENT"];
const WEEKDAYS: readonly NotificationWeekday[] = ["MONDAY", "TUESDAY", "WEDNESDAY", "THURSDAY", "FRIDAY", "SATURDAY", "SUNDAY"];
const EVENT_CLASS = /^[A-Za-z0-9_-]{1,128}$/;
const LOCAL_TIME = /^([01][0-9]|2[0-3]):[0-5][0-9]$/;

export class NotificationApiError extends Error {
  readonly code: string;
  readonly status: number;

  constructor(code: string, status: number) {
    super(code);
    this.name = "NotificationApiError";
    this.code = code;
    this.status = status;
  }
}

function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid Notification API response.");
  return value as Record<string, unknown>;
}

function text(value: unknown): string {
  if (typeof value !== "string" || value.trim().length === 0) throw new Error("Invalid Notification API response.");
  return value;
}

function optionalText(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  return text(value);
}

function number(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) throw new Error("Invalid Notification API response.");
  return value;
}

function channels(value: unknown): string[] {
  if (!Array.isArray(value)) throw new Error("Invalid Notification channels.");
  const items = value.map(text);
  if (new Set(items).size !== items.length) throw new Error("Invalid Notification channels.");
  return items;
}

function quietHours(value: unknown): QuietHours | null {
  if (value === null || value === undefined) return null;
  const row = object(value);
  const timezone = text(row.timezone);
  if (!Array.isArray(row.weekdays) || row.weekdays.length === 0) throw new Error("Invalid Notification quiet hours.");
  const weekdays = row.weekdays.map(day => {
    if (typeof day !== "string" || !WEEKDAYS.includes(day as NotificationWeekday)) throw new Error("Invalid Notification quiet hours.");
    return day as NotificationWeekday;
  });
  if (new Set(weekdays).size !== weekdays.length) throw new Error("Invalid Notification quiet hours.");
  const start = text(row.start_local_time);
  const end = text(row.end_local_time);
  if (!LOCAL_TIME.test(start) || !LOCAL_TIME.test(end)) throw new Error("Invalid Notification quiet hours.");
  return { timezone, weekdays, start_local_time: start, end_local_time: end };
}

function status(value: unknown): NotificationStatus {
  if (typeof value !== "string" || !STATUSES.includes(value as NotificationStatus)) throw new Error("Invalid Notification status.");
  return value as NotificationStatus;
}

function policy(value: unknown): NotificationPolicy {
  if (typeof value !== "string" || !POLICIES.includes(value as NotificationPolicy)) throw new Error("Invalid Notification policy.");
  return value as NotificationPolicy;
}

function errorCode(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  if (typeof value !== "string" || !/^[A-Z][A-Z0-9_]{0,63}$/.test(value)) throw new Error("Invalid Notification error code.");
  return value;
}

export function decodeNotification(value: unknown, workspaceId: string): Notification {
  const row = object(value);
  if (text(row.workspace_id) !== workspaceId) throw new Error("Notification belongs to a different Workspace.");
  const hostEpoch = row.attempt_host_epoch;
  if (hostEpoch !== null && hostEpoch !== undefined && (typeof hostEpoch !== "number" || !Number.isSafeInteger(hostEpoch) || hostEpoch < 0)) {
    throw new Error("Invalid Notification host epoch.");
  }
  return {
    delivery_id: text(row.delivery_id),
    workspace_id: workspaceId,
    dedupe_key: text(row.dedupe_key),
    source_event_id: text(row.source_event_id),
    channel_binding_id: optionalText(row.channel_binding_id),
    attempt_runtime_id: optionalText(row.attempt_runtime_id),
    attempt_host_epoch: typeof hostEpoch === "number" ? hostEpoch : null,
    status: status(row.status),
    attempt_count: number(row.attempt_count),
    next_attempt_at: optionalText(row.next_attempt_at),
    last_error_code: errorCode(row.last_error_code),
    created_at: text(row.created_at),
    settled_at: optionalText(row.settled_at),
  };
}

export function decodeNotificationPreference(value: unknown, workspaceId: string): NotificationPreference {
  const row = object(value);
  if (text(row.workspace_id) !== workspaceId) throw new Error("Notification preference belongs to a different Workspace.");
  const eventClass = text(row.event_class);
  if (!EVENT_CLASS.test(eventClass)) throw new Error("Invalid Notification event class.");
  return {
    workspace_id: workspaceId,
    event_class: eventClass,
    policy: policy(row.policy),
    preferred_channels: channels(row.preferred_channels),
    quiet_hours: quietHours(row.quiet_hours),
    version: number(row.version),
  };
}

export function createNotificationApi(workspaceId: string, transport: NotificationTransport): NotificationApi {
  const request = async (path: string, init: RequestInit = {}) => {
    if (!workspaceId) throw new Error("Select a Workspace before loading notifications.");
    const headers = new Headers(init.headers);
    headers.set("X-Workspace-ID", workspaceId);
    const response = await transport(path, { ...init, headers, cache: "no-store" });
    if (!response.ok) {
      let code = "NOTIFICATION_REQUEST_FAILED";
      try {
        const body = object(await response.json());
        if (typeof body.code === "string" && /^[A-Z][A-Z0-9_]{0,63}$/.test(body.code)) code = body.code;
      } catch {
        // Never surface untrusted server text.
      }
      throw new NotificationApiError(code, response.status);
    }
    return response;
  };

  return {
    workspaceId,
    async list(statusFilter, cursor, signal) {
      if (statusFilter !== null && !STATUSES.includes(statusFilter)) throw new Error("Notification status filter is invalid.");
      const query = new URLSearchParams({ limit: "50" });
      if (statusFilter !== null) query.set("status", statusFilter);
      if (cursor !== null) {
        if (cursor.length === 0 || cursor.length > 2048) throw new Error("Notification cursor is invalid.");
        query.set("cursor", cursor);
      }
      const body = object(await (await request(`/v1/notifications?${query}`, { signal })).json());
      if (!Array.isArray(body.items)) throw new Error("Invalid Notification page.");
      const items = body.items.map(item => decodeNotification(item, workspaceId));
      const next = body.next_cursor;
      if (next !== null && next !== undefined && typeof next !== "string") throw new Error("Invalid Notification cursor.");
      return { items, next_cursor: (next as string | null | undefined) ?? null };
    },
    async preferences(signal) {
      const body = await (await request("/v1/notification-preferences", { signal })).json();
      if (!Array.isArray(body)) throw new Error("Invalid Notification preferences.");
      return body.map(item => decodeNotificationPreference(item, workspaceId));
    },
    async setPreference(eventClass, expectedVersion, update, requestId) {
      if (!EVENT_CLASS.test(eventClass)) throw new Error("Notification event class is invalid.");
      if (!Number.isSafeInteger(expectedVersion) || expectedVersion < 0) throw new Error("Notification preference version is invalid.");
      if (requestId.trim().length === 0 || requestId.length > 200) throw new Error("Notification request identity is invalid.");
      const body = {
        policy: policy(update.policy),
        preferred_channels: channels(update.preferred_channels),
        quiet_hours: quietHours(update.quiet_hours),
      };
      const response = await request(`/v1/notification-preferences/${encodeURIComponent(eventClass)}`, {
        method: "PUT",
        headers: {
          "Content-Type": "application/json",
          "Idempotency-Key": requestId,
          "If-Match": `"${expectedVersion}"`,
        },
        body: JSON.stringify(body),
      });
      return decodeNotificationPreference(await response.json(), workspaceId);
    },
  };
}
