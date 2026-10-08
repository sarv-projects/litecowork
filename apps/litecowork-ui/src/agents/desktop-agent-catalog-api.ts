import { invoke } from "@tauri-apps/api/core";
import type { AgentCatalogApi } from "./agent-catalog-api";
import type { AgentBinding, AgentInstallation, AgentProfile } from "./AgentCatalogSettings";

/** Thin desktop bridge over existing authenticated local Operator commands.
 * This adapter deliberately exposes catalog/configuration actions only; it has no
 * session, Attempt, Task execution, resume, or switching methods.
 */
export const desktopAgentCatalogApi: AgentCatalogApi = {
  listInstallations: () => invoke<AgentInstallation[]>("list_agent_installations"),
  listProfiles: async (workspaceId) => {
    return invoke<{ items: AgentProfile[]; nextCursor: string | null }>("list_agent_profiles", { workspaceId })
      .then((page) => page.items);
  },
  listBindings: async (workspaceId) => {
    return invoke<{ items: AgentBinding[]; nextCursor: string | null }>("list_agent_bindings", { workspaceId })
      .then((page) => page.items);
  },
  probeCodex: (workspaceId) => invoke<AgentProfile>("probe_agent_profile", { workspaceId, providerKey: "CODEX" }),
  probeOpenCode: (workspaceId) => invoke<AgentProfile>("probe_agent_profile", { workspaceId, providerKey: "OPENCODE" }),
  createBinding: (input) => invoke<AgentBinding>("create_agent_binding", {
    workspaceId: input.workspaceId,
    agentProfileId: input.agentProfileId,
    leadEligible: input.leadEligible,
    runtimeId: input.runtimeId,
    requestId: input.requestId,
  }),
  enableBinding: (input) => invoke<AgentBinding>("enable_agent_binding", {
    workspaceId: input.workspaceId,
    agentBindingId: input.agentBindingId,
    expectedVersion: input.expectedVersion,
    requestId: input.requestId,
  }),
};
