import { invoke } from "@tauri-apps/api/core";
import { validIdempotencyKey } from "../api/idempotency-key.ts";
import { type OperatorBridgeResponse as BridgeResponse, operatorBridgeResponse } from "../api/operator-bridge.ts";
import { createNotificationApi, type NotificationTransport } from "./notification-api.ts";

function parseBody(body: BodyInit | null | undefined): Record<string, unknown> {
  if (typeof body !== "string") throw new Error("Notification command body is invalid.");
  const value: unknown = JSON.parse(body);
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Notification command body is invalid.");
  return value as Record<string, unknown>;
}

/** Finite, selected-Workspace Notification routes over authenticated native Operator IPC. */
function desktopTransport(workspaceId: string): NotificationTransport {
  return async (path, init) => {
    if (init.signal?.aborted) throw new DOMException("Aborted", "AbortError");
    const url = new URL(path, "https://operator.invalid");
    const method = init.method ?? "GET";
    const headers = new Headers(init.headers);
    if (headers.get("X-Workspace-ID") !== workspaceId) throw new Error("Notification Workspace context changed.");

    let operation: string;
    let eventClass: string | null = null;
    let status: string | null = null;
    let cursor: string | null = null;
    let limit: number | null = null;
    let policy: string | null = null;
    let preferredChannels: string[] | null = null;
    let quietHours: Record<string, unknown> | null = null;
    let expectedVersion: number | null = null;
    let requestId: string | null = null;

    if (url.pathname === "/v1/notifications" && method === "GET") {
      operation = "list";
      status = url.searchParams.get("status");
      cursor = url.searchParams.get("cursor");
      const rawLimit = url.searchParams.get("limit");
      if (rawLimit !== null) {
        limit = Number(rawLimit);
        if (!Number.isSafeInteger(limit) || limit < 1 || limit > 200) throw new Error("Notification page limit is invalid.");
      }
    } else if (url.pathname === "/v1/notification-preferences" && method === "GET") {
      operation = "preferences";
    } else {
      const match = /^\/v1\/notification-preferences\/([^/]+)$/.exec(url.pathname);
      if (!match || method !== "PUT") throw new Error("Unsupported Notification operation.");
      eventClass = decodeURIComponent(match[1]);
      const body = parseBody(init.body);
      if (typeof body.policy !== "string" || !Array.isArray(body.preferred_channels)) {
        throw new Error("Notification preference body is invalid.");
      }
      policy = body.policy;
      preferredChannels = body.preferred_channels.map(value => {
        if (typeof value !== "string") throw new Error("Notification preference channel is invalid.");
        return value;
      });
      const quiet = body.quiet_hours;
      if (quiet !== null && quiet !== undefined) {
        if (typeof quiet !== "object" || Array.isArray(quiet)) throw new Error("Notification quiet hours are invalid.");
        quietHours = quiet as Record<string, unknown>;
      }
      requestId = headers.get("Idempotency-Key");
      if (!requestId || !validIdempotencyKey(requestId)) throw new Error("Notification request identity is missing.");
      const rawVersion = headers.get("If-Match");
      if (rawVersion === null) throw new Error("Notification preference version is required.");
      expectedVersion = Number(rawVersion.replace(/^"|"$/g, ""));
      if (!Number.isSafeInteger(expectedVersion) || expectedVersion < 0) throw new Error("Notification preference version is invalid.");
      operation = "set-preference";
    }

    const response = await invoke<BridgeResponse>("notification_request", {
      workspaceId,
      operation,
      eventClass,
      status,
      cursor,
      limit,
      policy,
      preferredChannels,
      quietHours,
      expectedVersion,
      requestId,
    });
    return operatorBridgeResponse(response, { signal: init.signal, maxEncodedLength: 2 * 1024 * 1024 });
  };
}

export function desktopNotificationApi(workspaceId: string) {
  return createNotificationApi(workspaceId, desktopTransport(workspaceId));
}

export function maybeDesktopNotificationApi(workspaceId: string) {
  return workspaceId.trim().length === 0 ? null : desktopNotificationApi(workspaceId);
}
