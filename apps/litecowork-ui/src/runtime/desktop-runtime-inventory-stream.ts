import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { RuntimeInventoryStreamDriver, RuntimeInventoryStreamEnvelope } from "./runtime-inventory-stream.ts";

export const desktopRuntimeInventoryStreamDriver: RuntimeInventoryStreamDriver = {
  listen: async listener => listen<RuntimeInventoryStreamEnvelope>("runtime-inventory-stream", event => listener(event.payload)),
  start: input => invoke("subscribe_runtime_inventory_stream", {
    sessionId: input.sessionId,
    workspaceId: input.workspaceId,
    runtimeId: input.runtimeId,
    resumeCursor: input.resumeCursor,
  }),
  acknowledge: (sessionId, frameId) => invoke("acknowledge_conversation_stream_frame", { sessionId, frameId }),
  stop: sessionId => invoke("unsubscribe_conversation_stream", { sessionId }),
  newSessionId: () => crypto.randomUUID(),
};
