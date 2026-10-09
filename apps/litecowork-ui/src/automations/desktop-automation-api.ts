import { invoke } from "@tauri-apps/api/core";
import { createAutomationApi, type AutomationTransport } from "./automation-api";

type BridgeResponse = { status: number; contentType: string; bodyBase64: string };
function decodeBase64(encoded: string): Uint8Array {
  const binary = atob(encoded);
  return Uint8Array.from(binary, character => character.charCodeAt(0));
}

/** Finite Automation routes over authenticated native Operator IPC. */
function desktopTransport(workspaceId: string): AutomationTransport {
  return async (path, init) => {
    if (init.signal?.aborted) throw new DOMException("Aborted", "AbortError");
    const url = new URL(path, "https://operator.invalid");
    const method = init.method ?? "GET";
    const headers = new Headers(init.headers);
    if (headers.get("X-Workspace-ID") !== workspaceId) throw new Error("Automation Workspace context changed.");
    const requestId = headers.get("Idempotency-Key");
    const rawVersion = headers.get("If-Match");
    let operation: string;
    const cursor = url.searchParams.get("cursor");
    let automationId: string | null = null;
    let routineId: string | null = null;
    let name: string | null = null;
    let routineRevision: number | null = null;
    let triggers: unknown[] | null = null;
    let executionPolicy: Record<string, unknown> | null = null;
    let coworkerId: string | null = null;
    let coworkerRef: Record<string, unknown> | null = null;
    let automationRevision: number | null = null;
    let inputs: Record<string, unknown> | null = null;
    let expectedVersion: number | null = null;
    const automationMatch = /^\/v1\/automations(?:\/([^/]+)(?:\/(pause|disable|revisions|run))?)?$/.exec(url.pathname);
    const routineMatch = /^\/v1\/routines(?:\/([^/]+)(?:\/(revisions))?)?$/.exec(url.pathname);
    const coworkerMatch = /^\/v1\/coworkers(?:\/([^/]+))?$/.exec(url.pathname);
    if (automationMatch) {
      automationId = automationMatch[1] ? decodeURIComponent(automationMatch[1]) : null;
      const suffix = automationMatch[2] ?? null;
      if (automationId && !/^[A-Za-z0-9_-]{1,200}$/.test(automationId)) throw new Error("Automation selection is invalid.");
      if (method === "GET" && automationId === null && suffix === null) operation = "list";
      else if (method === "GET" && automationId !== null && suffix === null) operation = "get";
      else if (method === "GET" && automationId !== null && suffix === "revisions") operation = "list_automation_revisions";
      else if (method === "POST" && automationId !== null && (suffix === "pause" || suffix === "disable")) operation = suffix;
      else if (method === "POST" && automationId !== null && suffix === "run") {
        operation = "run";
        const body = parseObject(init.body);
        if (typeof body.automation_revision !== "number" || !Number.isSafeInteger(body.automation_revision) || body.automation_revision < 1
          || !isObject(body.inputs)) throw new Error("Manual Automation Run request is invalid.");
        automationRevision = body.automation_revision;
        inputs = body.inputs;
      }
      else if (method === "POST" && automationId === null && suffix === null) {
        operation = "create";
        const body = parseObject(init.body);
        if (body.workspace_id !== workspaceId || typeof body.name !== "string" || typeof body.routine_id !== "string"
          || typeof body.routine_revision !== "number" || !Array.isArray(body.triggers) || !isObject(body.execution_policy)
          || !("coworker_ref" in body)) throw new Error("Automation create request is invalid.");
        coworkerRef = parseCoworkerRef(body.coworker_ref);
        name = body.name; routineId = body.routine_id; routineRevision = body.routine_revision;
        triggers = body.triggers; executionPolicy = body.execution_policy;
      } else if (method === "PATCH" && automationId !== null && suffix === null) {
        operation = "revise";
        const body = parseObject(init.body);
        if (typeof body.name !== "string" || typeof body.routine_id !== "string" || typeof body.routine_revision !== "number"
          || !Array.isArray(body.triggers) || !isObject(body.execution_policy) || !("coworker_ref" in body)) throw new Error("Automation revision request is invalid.");
        coworkerRef = parseCoworkerRef(body.coworker_ref);
        name = body.name; routineId = body.routine_id; routineRevision = body.routine_revision;
        triggers = body.triggers; executionPolicy = body.execution_policy;
      } else throw new Error("Unsupported Automation operation.");
    } else if (routineMatch) {
      routineId = routineMatch[1] ? decodeURIComponent(routineMatch[1]) : null;
      if (routineId && !/^[A-Za-z0-9_-]{1,200}$/.test(routineId)) throw new Error("Routine selection is invalid.");
      if (method === "GET" && routineId === null) operation = "list_routines";
      else if (method === "GET" && routineId !== null && routineMatch[2] === "revisions") operation = "list_routine_revisions";
      else if (method === "GET" && routineId !== null) operation = "get_routine";
      else throw new Error("Unsupported Routine selection operation.");
    } else if (coworkerMatch) {
      coworkerId = coworkerMatch[1] ? decodeURIComponent(coworkerMatch[1]) : null;
      if (coworkerId && !/^[A-Za-z0-9_-]{1,200}$/.test(coworkerId)) throw new Error("Coworker selection is invalid.");
      if (method === "GET" && coworkerId === null) operation = "list_coworkers";
      else if (method === "GET" && coworkerId !== null) operation = "get_coworker";
      else throw new Error("Unsupported Coworker selection operation.");
    } else throw new Error("Unsupported Automation path.");

    if (method !== "GET") {
      if (!requestId || requestId.length > 128) throw new Error("Automation request identity is missing.");
      if (operation !== "create") {
        if (!rawVersion) throw new Error("Automation version is required.");
        const parsed = Number(rawVersion.replace(/^\"|\"$/g, ""));
        if (!Number.isSafeInteger(parsed) || parsed < 1) throw new Error("Automation version is invalid.");
        expectedVersion = parsed;
      }
    }

    const response = await invoke<BridgeResponse>("automation_request", {
      workspaceId, operation, automationId, routineId, coworkerId, cursor, expectedVersion, requestId,
      name, routineRevision, triggers, executionPolicy, coworkerRef, automationRevision, inputs,
    });
    if (init.signal?.aborted) throw new DOMException("Aborted", "AbortError");
    return new Response(decodeBase64(response.bodyBase64), {
      status: response.status,
      headers: { "Content-Type": response.contentType },
    });
  };
}

function isObject(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
function parseCoworkerRef(value: unknown): Record<string, unknown> | null {
  if (value === null) return null;
  if (!isObject(value) || typeof value.coworker_id !== "string" || value.coworker_id.length === 0
    || typeof value.revision !== "number" || !Number.isSafeInteger(value.revision) || value.revision < 1) {
    throw new Error("Automation Coworker pin is invalid.");
  }
  return { coworker_id: value.coworker_id, revision: value.revision };
}
function parseObject(body: BodyInit | null | undefined): Record<string, unknown> {
  if (typeof body !== "string") throw new Error("Automation command body is invalid.");
  const value: unknown = JSON.parse(body);
  if (!isObject(value)) throw new Error("Automation command body is invalid.");
  return value;
}

export function desktopAutomationApi(workspaceId: string) {
  return createAutomationApi(workspaceId, desktopTransport(workspaceId));
}
