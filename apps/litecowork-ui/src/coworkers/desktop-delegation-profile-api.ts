import { invoke } from "@tauri-apps/api/core";
import {
  createDelegationProfileCatalogApi,
  type DelegationProfileOperation,
  type DelegationProfileTransport,
} from "./delegation-profile-api";

type BridgeResponse = { status: number; contentType: string; bodyBase64: string };

function decodeBase64(encoded: string): Uint8Array {
  if (encoded.length > 2 * 1024 * 1024) throw new Error("Worker profile response exceeded its size limit.");
  const binary = atob(encoded);
  return Uint8Array.from(binary, character => character.charCodeAt(0));
}

function desktopTransport(workspaceId: string): DelegationProfileTransport {
  return async (operation: DelegationProfileOperation, signal) => {
    if (signal?.aborted) throw new DOMException("Aborted", "AbortError");
    const url = new URL(operation.path, "https://operator.invalid");
    let response: BridgeResponse;
    if ((operation.method ?? "GET") === "GET"
      && url.pathname === "/v1/delegation-profiles"
      && [...url.searchParams.keys()].every(key => key === "limit" || key === "cursor")
      && url.searchParams.get("limit") === "200"
      && !operation.requestId && operation.expectedVersion === undefined && operation.body === undefined) {
      response = await invoke<BridgeResponse>("list_delegation_profiles", {
        workspaceId,
        cursor: url.searchParams.get("cursor"),
      });
    } else if (operation.method === "POST"
      && url.pathname === "/v1/delegation-profiles"
      && url.search === ""
      && operation.requestId && operation.body
      && operation.expectedVersion === undefined) {
      const body = operation.body as { workspace_id?: unknown; agent_binding_id?: unknown; revision?: unknown };
      if (body.workspace_id !== workspaceId || typeof body.agent_binding_id !== "string") {
        throw new Error("Worker profile request does not match the selected Workspace.");
      }
      response = await invoke<BridgeResponse>("create_delegation_profile", {
        workspaceId,
        agentBindingId: body.agent_binding_id,
        requestId: operation.requestId,
        revision: body.revision,
      });
    } else if (operation.method === "POST"
      && /^\/v1\/delegation-profiles\/[^/]+\/revisions$/.test(url.pathname)
      && url.search === ""
      && operation.requestId && operation.body
      && Number.isSafeInteger(operation.expectedVersion) && (operation.expectedVersion ?? 0) > 0) {
      const profileId = decodeURIComponent(url.pathname.split("/")[3] ?? "");
      if (!profileId || profileId.length > 256 || /[\u0000-\u001f\u007f]/.test(profileId)) throw new Error("Worker profile request identifier is invalid.");
      response = await invoke<BridgeResponse>("revise_delegation_profile", {
        workspaceId,
        profileId,
        expectedVersion: operation.expectedVersion,
        requestId: operation.requestId,
        revision: operation.body,
      });
    } else if (operation.method === "POST"
      && /^\/v1\/delegation-profiles\/[^/]+\/duplicate$/.test(url.pathname)
      && url.search === ""
      && operation.requestId && operation.body
      && Number.isSafeInteger(operation.expectedVersion) && (operation.expectedVersion ?? 0) > 0) {
      const profileId = decodeURIComponent(url.pathname.split("/")[3] ?? "");
      const body = operation.body as { name?: unknown };
      if (!profileId || profileId.length > 256 || /[\u0000-\u001f\u007f]/.test(profileId) || typeof body.name !== "string") {
        throw new Error("Worker profile duplication request is invalid.");
      }
      response = await invoke<BridgeResponse>("duplicate_delegation_profile", {
        workspaceId,
        profileId,
        expectedVersion: operation.expectedVersion,
        requestId: operation.requestId,
        name: body.name,
      });
    } else if (operation.method === "POST"
      && /^\/v1\/delegation-profiles\/[^/]+\/status$/.test(url.pathname)
      && url.search === ""
      && operation.requestId && operation.body
      && Number.isSafeInteger(operation.expectedVersion) && (operation.expectedVersion ?? 0) > 0) {
      const profileId = decodeURIComponent(url.pathname.split("/")[3] ?? "");
      const body = operation.body as { status?: unknown };
      if (!profileId || profileId.length > 256 || /[\u0000-\u001f\u007f]/.test(profileId)) throw new Error("Worker profile status request is invalid.");
      const command = body.status === "DISABLED" ? "disable_delegation_profile"
        : body.status === "ARCHIVED" ? "archive_delegation_profile" : null;
      if (!command) throw new Error("Worker profile enablement is unavailable.");
      response = await invoke<BridgeResponse>(command, {
        workspaceId,
        profileId,
        expectedVersion: operation.expectedVersion,
        requestId: operation.requestId,
      });
    } else {
      throw new Error("Unsupported worker profile operation.");
    }
    if (signal?.aborted) throw new DOMException("Aborted", "AbortError");
    if (!Number.isInteger(response.status) || response.status < 100 || response.status > 599
      || typeof response.contentType !== "string" || typeof response.bodyBase64 !== "string") {
      throw new Error("The local worker profile service returned an invalid response.");
    }
    return new Response(decodeBase64(response.bodyBase64), {
      status: response.status,
      headers: { "Content-Type": response.contentType },
    });
  };
}

export function desktopDelegationProfileCatalogApi(workspaceId: string) {
  return createDelegationProfileCatalogApi(workspaceId, desktopTransport(workspaceId));
}
