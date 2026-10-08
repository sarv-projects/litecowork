import { invoke } from "@tauri-apps/api/core";
import { createGoalApi, type GoalTransport } from "./goal-api";

type BridgeResponse = { status: number; contentType: string; bodyBase64: string };
function decodeBase64(encoded: string): Uint8Array {
  const binary = atob(encoded);
  return Uint8Array.from(binary, character => character.charCodeAt(0));
}
function parseBody(body: BodyInit | null | undefined): Record<string, unknown> {
  if (typeof body !== "string") throw new Error("Goal command body is invalid.");
  const value: unknown = JSON.parse(body);
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Goal command body is invalid.");
  return value as Record<string, unknown>;
}

/** Finite, selected-Workspace Goal routes over authenticated native Operator IPC. */
function desktopTransport(workspaceId: string): GoalTransport {
  return async (path, init) => {
    if (init.signal?.aborted) throw new DOMException("Aborted", "AbortError");
    const url = new URL(path, "https://operator.invalid");
    const method = init.method ?? "GET";
    const headers = new Headers(init.headers);
    if (headers.get("X-Workspace-ID") !== workspaceId) throw new Error("Goal Workspace context changed.");
    const requestId = headers.get("Idempotency-Key");
    let operation: string;
    let goalId: string | null = null;
    let cursor = url.searchParams.get("cursor");
    let coworkerId: string | null = null;
    let revision: Record<string, unknown> | null = null;
    let status: string | null = null;
    let expectedVersion: number | null = null;

    if (url.pathname === "/v1/tasks" && method === "GET") operation = "related-tasks";
    else if (url.pathname === "/v1/artifacts" && method === "GET") operation = "related-artifacts";
    else if (url.pathname === "/v1/goals" && method === "GET") operation = "list";
    else if (url.pathname === "/v1/goals" && method === "POST") {
      operation = "create";
      const body = parseBody(init.body);
      if (body.workspace_id !== workspaceId || !(body.coworker_id === null || typeof body.coworker_id === "string") || !body.revision || typeof body.revision !== "object" || Array.isArray(body.revision)) {
        throw new Error("Goal create request scope is invalid.");
      }
      coworkerId = body.coworker_id as string | null;
      revision = body.revision as Record<string, unknown>;
    } else {
      const match = /^\/v1\/goals\/([^/]+)(?:\/(revisions|status))?$/.exec(url.pathname);
      if (!match) throw new Error("Unsupported Goal path.");
      goalId = decodeURIComponent(match[1]);
      const suffix = match[2];
      if (!/^[A-Za-z0-9_-]{1,200}$/.test(goalId)) throw new Error("Goal selection is invalid.");
      if (method === "GET" && !suffix) operation = "get";
      else if (method === "POST" && suffix === "revisions") {
        operation = "revise";
        revision = parseBody(init.body);
      } else if (method === "POST" && suffix === "status") {
        operation = "status";
        const body = parseBody(init.body);
        if (typeof body.status !== "string") throw new Error("Goal status request is invalid.");
        status = body.status;
      } else throw new Error("Unsupported Goal operation.");
    }

    if (method !== "GET") {
      if (!requestId || requestId.length > 128) throw new Error("Goal request identity is missing.");
      const rawVersion = headers.get("If-Match");
      if (rawVersion !== null) {
        const parsed = Number(rawVersion.replace(/^\"|\"$/g, ""));
        if (!Number.isSafeInteger(parsed) || parsed < 1) throw new Error("Goal version is invalid.");
        expectedVersion = parsed;
      }
      if (operation !== "create" && expectedVersion === null) throw new Error("Goal aggregate version is required.");
    }

    const response = await invoke<BridgeResponse>("goal_request", {
      workspaceId, operation, goalId, cursor, coworkerId, revision, status, expectedVersion, requestId,
    });
    if (init.signal?.aborted) throw new DOMException("Aborted", "AbortError");
    return new Response(decodeBase64(response.bodyBase64), {
      status: response.status,
      headers: { "Content-Type": response.contentType },
    });
  };
}

export function desktopGoalApi(workspaceId: string) {
  return createGoalApi(workspaceId, desktopTransport(workspaceId));
}
