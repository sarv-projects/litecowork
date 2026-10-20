import { invoke } from "@tauri-apps/api/core";
import {
  parseAgentControlDescriptor,
  type AgentControlDescriptor,
  type AgentControlDescriptorApi,
} from "./agent-control-api";

export const desktopAgentControlDescriptorApi: AgentControlDescriptorApi = {
  async get(workspaceId: string, profileId: string): Promise<AgentControlDescriptor> {
    const value = await invoke<unknown>("get_agent_control_descriptor", { workspaceId, profileId });
    return parseAgentControlDescriptor(value);
  },
  async refresh(workspaceId: string, profileId: string): Promise<AgentControlDescriptor> {
    const value = await invoke<unknown>("refresh_agent_control_descriptor", { workspaceId, profileId });
    return parseAgentControlDescriptor(value);
  },
};
