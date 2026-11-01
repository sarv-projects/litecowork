import type { AgentBinding, AgentInstallation, AgentProfile } from "./AgentCatalogSettings";

export type AgentDistribution = {
  distributionId: string;
  kind: string;
  packageRef: string;
  versionConstraint: string | null;
  os: string[];
  architecture: string[];
};

export type AgentRegistryEntry = {
  registryAgentId: string;
  displayName: string;
  description: string | null;
  homepage: string | null;
  sourceRegistry: string;
  sourceRevision: string | null;
  distributions: AgentDistribution[];
  adapterSupport: "SUPPORTED" | "NO_ADAPTER" | "UNSUPPORTED_OS" | string;
  metadataObservedAt: string;
  cacheExpiresAt: string | null;
};

export type AgentRegistryPage = {
  items: AgentRegistryEntry[];
  nextCursor: string | null;
  observedAt: string;
  stale: boolean;
};

export type AgentRegistrySnapshot = {
  sourceRegistry: string;
  sourceRevision: string | null;
  observedAt: string;
  expiresAt: string | null;
  stale: boolean;
  items: AgentRegistryEntry[];
};

export type AgentProviderKey = "CODEX" | "OPENCODE";

export interface AgentCatalogApi {
  listInstallations(): Promise<AgentInstallation[]>;
  listRegistry(input?: { query?: string; cursor?: string; limit?: number }): Promise<AgentRegistryPage>;
  refreshRegistry(): Promise<AgentRegistrySnapshot>;
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
  disableBinding(input: {
    workspaceId: string;
    agentBindingId: string;
    expectedVersion: number;
    requestId: string;
  }): Promise<AgentBinding>;
}
