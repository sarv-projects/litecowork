import { invoke } from "@tauri-apps/api/core";
import { createCoworkerApi, type CoworkerTransport } from "./coworker-api";

type BridgeResponse = { status: number; contentType: string; bodyBase64: string };

function decodeBase64(encoded: string): Uint8Array {
  const binary = atob(encoded);
  return Uint8Array.from(binary, character => character.charCodeAt(0));
}

function parseBody(body: BodyInit | null | undefined): Record<string, unknown> {
  if (typeof body !== "string") throw new Error("Coworker command body is invalid.");
  const value: unknown = JSON.parse(body);
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Coworker command body is invalid.");
  return value as Record<string, unknown>;
}

/** Maps the Coworker REST contract to a finite set of authenticated native IPC calls. */
function desktopTransport(workspaceId: string): CoworkerTransport {
  return async (path, init) => {
    if (init.signal?.aborted) throw new DOMException("Aborted", "AbortError");
    const url = new URL(path, "https://operator.invalid");
    const method = init.method ?? "GET";
    const headers = new Headers(init.headers);
    if (headers.get("X-Workspace-ID") !== workspaceId) throw new Error("Coworker Workspace context changed.");

    let operation: string;
    let coworkerId: string | null = null;
    let cursor = url.searchParams.get("cursor");
    let revision: Record<string, unknown> | null = null;
    let revisionNumber: number | null = null;
    let status: string | null = null;
    let primaryCoworkerId: string | null = null;
    let expectedVersion: number | null = null;
    let requestId: string | null = headers.get("Idempotency-Key");

    if (url.pathname === "/v1/coworkers" && method === "GET") {
      operation = "list";
    } else if (url.pathname === "/v1/coworkers" && method === "POST") {
      operation = "create";
      const body = parseBody(init.body);
      if (body.workspace_id !== workspaceId || !body.revision || typeof body.revision !== "object" || Array.isArray(body.revision)) {
        throw new Error("Coworker create request scope is invalid.");
      }
      revision = body.revision as Record<string, unknown>;
    } else {
      const primary = /^\/v1\/workspaces\/([^/]+)\/primary-coworker$/.exec(url.pathname);
      const exactRevision = /^\/v1\/coworkers\/([^/]+)\/revisions\/([0-9]+)$/.exec(url.pathname);
      const coworker = /^\/v1\/coworkers\/([^/]+)(?:\/(presence|revisions|status))?$/.exec(url.pathname);
      if (primary && method === "POST") {
        if (decodeURIComponent(primary[1]) !== workspaceId) throw new Error("Primary Coworker request scope is invalid.");
        operation = "primary";
        const body = parseBody(init.body);
        if (!(body.coworker_id === null || typeof body.coworker_id === "string")) throw new Error("Primary Coworker request is invalid.");
        primaryCoworkerId = body.coworker_id as string | null;
      } else if (exactRevision && method === "GET") {
        coworkerId = decodeURIComponent(exactRevision[1]);
        revisionNumber = Number(exactRevision[2]);
        if (!Number.isSafeInteger(revisionNumber) || revisionNumber < 1) throw new Error("Coworker revision selection is invalid.");
        operation = "get_revision";
      } else if (coworker) {
        coworkerId = decodeURIComponent(coworker[1]);
        const suffix = coworker[2];
        if (method === "GET" && suffix === undefined) operation = "get";
        else if (method === "GET" && suffix === "presence") operation = "presence";
        else if (method === "POST" && suffix === "revisions") {
          operation = "revise";
          const body = parseBody(init.body);
          revision = body;
        } else if (method === "POST" && suffix === "status") {
          operation = "status";
          const body = parseBody(init.body);
          if (typeof body.status !== "string") throw new Error("Coworker status request is invalid.");
          status = body.status;
        } else throw new Error("Unsupported Coworker operation.");
      } else throw new Error("Unsupported Coworker path.");
    }

    if (method !== "GET") {
      const rawVersion = headers.get("If-Match");
      if (rawVersion !== null) {
        const parsed = Number(rawVersion.replace(/^\"|\"$/g, ""));
        if (!Number.isSafeInteger(parsed) || parsed < 1) throw new Error("Coworker version is invalid.");
        expectedVersion = parsed;
      }
      if (!requestId) throw new Error("Coworker request identity is missing.");
    }

    const response = await invoke<BridgeResponse>("coworker_request", {
      workspaceId,
      operation,
      coworkerId,
      cursor,
      revision,
      revisionNumber,
      status,
      primaryCoworkerId,
      expectedVersion,
      requestId,
    });
    if (init.signal?.aborted) throw new DOMException("Aborted", "AbortError");
    return new Response(decodeBase64(response.bodyBase64), {
      status: response.status,
      headers: { "Content-Type": response.contentType },
    });
  };
}

export function desktopCoworkerApi(workspaceId: string) {
  return createCoworkerApi(workspaceId, desktopTransport(workspaceId));
}
