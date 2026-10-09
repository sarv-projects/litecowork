import { invoke } from "@tauri-apps/api/core";
import { createRoutineApi, type RoutineTransport } from "./routine-api";

type BridgeResponse = { status: number; contentType: string; bodyBase64: string };
function decodeBase64(encoded: string): Uint8Array {
  const binary = atob(encoded);
  return Uint8Array.from(binary, character => character.charCodeAt(0));
}

/** Narrow bridge to saved-Routine routes and save-only manual Task materialization. */
function desktopTransport(workspaceId: string): RoutineTransport {
  return async (path, init) => {
    if (init.signal?.aborted) throw new DOMException("Aborted", "AbortError");
    const url = new URL(path, "https://operator.invalid");
    const method = init.method ?? "GET";
    const headers = new Headers(init.headers);
    if (headers.get("X-Workspace-ID") !== workspaceId) throw new Error("Routine Workspace context changed.");
    const requestId = headers.get("Idempotency-Key");
    const rawVersion = headers.get("If-Match");
    const match = /^\/v1\/routines(?:\/([^/]+)(?:\/(revisions|archive|run))?)?$/.exec(url.pathname);
    if (!match) throw new Error("Unsupported Routine path.");
    const routineId = match[1] ? decodeURIComponent(match[1]) : null;
    const child = match[2] ?? null;
    if (routineId && !/^[A-Za-z0-9_-]{1,200}$/.test(routineId)) throw new Error("Routine selection is invalid.");

    let operation: string;
    if (method === "GET" && !routineId && !child) operation = "list";
    else if (method === "GET" && routineId && !child) operation = "get";
    else if (method === "GET" && routineId && child === "revisions") operation = "revisions";
    else if (method === "POST" && !routineId && !child) operation = "create";
    else if (method === "POST" && routineId && child === "revisions") operation = "revise";
    else if (method === "POST" && routineId && child === "archive") operation = "archive";
    else if (method === "POST" && routineId && child === "run") operation = "run";
    else throw new Error("Unsupported Routine operation.");

    let expectedVersion: number | null = null;
    if (operation === "revise" || operation === "archive") {
      if (!requestId || requestId.length > 128 || !rawVersion) throw new Error("Routine mutation identity or version is missing.");
      expectedVersion = Number(rawVersion.replace(/^\"|\"$/g, ""));
      if (!Number.isSafeInteger(expectedVersion) || expectedVersion < 1) throw new Error("Routine version is invalid.");
    }
    if (["create", "run"].includes(operation) && (!requestId || requestId.length > 128)) throw new Error("Routine request identity is missing.");
    if (operation === "run" && rawVersion) throw new Error("Routine request is invalid.");
    const body = typeof init.body === "string" ? init.body : null;
    if (body && body.length > 128 * 1024) throw new Error("Routine definition exceeds its size limit.");
    const response = await invoke<BridgeResponse>("routine_request", {
      workspaceId, operation, routineId, cursor: url.searchParams.get("cursor"), expectedVersion, requestId, body,
    });
    if (init.signal?.aborted) throw new DOMException("Aborted", "AbortError");
    return new Response(decodeBase64(response.bodyBase64), {
      status: response.status,
      headers: { "Content-Type": response.contentType },
    });
  };
}

export function desktopRoutineApi(workspaceId: string) {
  return createRoutineApi(workspaceId, desktopTransport(workspaceId));
}
