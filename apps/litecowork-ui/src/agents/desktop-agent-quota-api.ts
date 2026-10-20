import { invoke } from "@tauri-apps/api/core";
import { createAgentQuotaObservationApi } from "./agent-quota-api";

export function desktopAgentQuotaObservationApi(workspaceId: string) {
  if (!workspaceId || workspaceId.length > 256 || /[\u0000-\u001f\u007f]/.test(workspaceId)) {
    throw new Error("A valid Workspace is required to load usage observations.");
  }
  return createAgentQuotaObservationApi((agentBindingId) => invoke<unknown>("get_agent_quota_observation", {
    workspaceId,
    agentBindingId,
  }));
}
