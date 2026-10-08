import type { AgentBinding, AgentInstallation, AgentProfile } from "./AgentCatalogSettings";

export type AgentProviderKey = "CODEX" | "OPENCODE";

export interface AgentCatalogApi {
  listInstallations(): Promise<AgentInstallation[]>;
  listProfiles(workspaceId: string): Promise<AgentProfile[]>;
  listBindings(workspaceId: string): Promise<AgentBinding[]>;
  probeCodex(workspaceId: string): Promise<AgentProfile>;
  probeOpenCode(workspaceId: string): Promise<AgentProfile>;
  createBinding(input: {
    workspaceId: string;
    agentProfileId: string;
    leadEligible: boolean;
    runtimeId: string | null;
    requestId: string;
  }): Promise<AgentBinding>;
  enableBinding(input: {
    workspaceId: string;
    agentBindingId: string;
    expectedVersion: number;
    requestId: string;
  }): Promise<AgentBinding>;
}
