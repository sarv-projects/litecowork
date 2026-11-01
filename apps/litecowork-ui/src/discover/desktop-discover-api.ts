import { invoke } from "@tauri-apps/api/core";
import { type OperatorBridgeResponse, operatorBridgeResponse } from "../api/operator-bridge.ts";
import {
  createDiscoverApi,
  type DiscoverEntryKind,
  type DiscoverItemTransport,
  type DiscoverSearchTransport,
} from "./discover-api.ts";

const MAX_DISCOVER_RESPONSE_BASE64 = 4 * 1024 * 1024;

type DiscoverOperation = "search" | "item";

type DiscoverRequest = {
  query?: string | null;
  kind?: DiscoverEntryKind | null;
  cursor?: string;
  itemRef?: string;
};

async function requestDiscover(
  workspaceId: string,
  operation: DiscoverOperation,
  request: DiscoverRequest,
  signal?: AbortSignal,
): Promise<unknown> {
  if (signal?.aborted) throw new DOMException("Aborted", "AbortError");
  const bridge = await invoke<OperatorBridgeResponse>("discover_request", {
    workspaceId,
    operation,
    query: request.query ?? null,
    kind: request.kind ?? null,
    cursor: request.cursor ?? null,
    itemRef: request.itemRef ?? null,
  });
  const response = operatorBridgeResponse(bridge, {
    signal,
    maxEncodedLength: MAX_DISCOVER_RESPONSE_BASE64,
  });
  if (!response.ok) {
    throw new Error("Local Discover service could not return the requested catalog data.");
  }
  return response.json();
}

export function desktopDiscoverApi(workspaceId: string) {
  const searchTransport: DiscoverSearchTransport = (id, query, kind, cursor, signal) =>
    requestDiscover(id, "search", { query, kind, cursor }, signal);
  const itemTransport: DiscoverItemTransport = (id, itemRef, signal) =>
    requestDiscover(id, "item", { itemRef }, signal);
  return createDiscoverApi(workspaceId, searchTransport, itemTransport);
}
