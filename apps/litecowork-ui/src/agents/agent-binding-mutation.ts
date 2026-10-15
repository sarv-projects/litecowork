export type AgentBindingMutationOperation = "enable" | "disable";

export type AgentBindingMutationIdentity = {
  operation: AgentBindingMutationOperation;
  workspaceId: string;
  agentBindingId: string;
  expectedVersion: number;
};

export type PendingAgentBindingMutation = AgentBindingMutationIdentity & {
  requestId: string;
};

export type AgentBindingVersionProjection = {
  agentBindingId: string;
  version: number;
};

export type WorkspaceAgentBindingVersionProjection = AgentBindingVersionProjection & {
  workspaceId: string;
};

export class AgentBindingRefreshFence {
  private readonly generations = new Map<string, number>();

  begin(workspaceId: string): number {
    if (!workspaceId.trim()) throw new Error("Workspace is required for binding refresh.");
    const generation = (this.generations.get(workspaceId) ?? 0) + 1;
    this.generations.set(workspaceId, generation);
    return generation;
  }

  isCurrent(workspaceId: string, generation: number, selectedWorkspaceId: string | null): boolean {
    return selectedWorkspaceId === workspaceId && this.generations.get(workspaceId) === generation;
  }
}

export type AgentActionFeedback = {
  message: string | null;
  error: string | null;
  busy: string | null;
};

export type AgentActionFeedbackByWorkspace = Record<string, AgentActionFeedback>;

const EMPTY_AGENT_ACTION_FEEDBACK: AgentActionFeedback = {
  message: null,
  error: null,
  busy: null,
};

export function agentActionFeedbackForWorkspace(
  feedback: AgentActionFeedbackByWorkspace,
  workspaceId: string | null,
): AgentActionFeedback {
  return workspaceId ? feedback[workspaceId] ?? EMPTY_AGENT_ACTION_FEEDBACK : EMPTY_AGENT_ACTION_FEEDBACK;
}

export function updateAgentActionFeedback(
  current: AgentActionFeedbackByWorkspace,
  workspaceId: string,
  update: Partial<AgentActionFeedback>,
): AgentActionFeedbackByWorkspace {
  if (!workspaceId.trim()) return current;
  return {
    ...current,
    [workspaceId]: {
      ...EMPTY_AGENT_ACTION_FEEDBACK,
      ...current[workspaceId],
      ...update,
    },
  };
}

export function clearAgentActionBusyIfMatches(
  current: AgentActionFeedbackByWorkspace,
  workspaceId: string,
  expectedAction: string,
): AgentActionFeedbackByWorkspace {
  const feedback = agentActionFeedbackForWorkspace(current, workspaceId);
  if (feedback.busy !== expectedAction) return current;
  return updateAgentActionFeedback(current, workspaceId, { busy: null });
}

export function mergeAgentBindingProjection<T extends WorkspaceAgentBindingVersionProjection>(
  current: readonly T[],
  updated: T,
): T[] {
  const index = current.findIndex(binding => binding.workspaceId === updated.workspaceId
    && binding.agentBindingId === updated.agentBindingId);
  if (index < 0 || current[index].version >= updated.version) return [...current];
  const merged = [...current];
  merged[index] = updated;
  return merged;
}

export function mergeAgentBindingProjections<T extends WorkspaceAgentBindingVersionProjection>(
  current: readonly T[],
  updates: readonly T[],
): T[] {
  return updates.reduce((bindings, updated) => {
    const index = bindings.findIndex(binding => binding.workspaceId === updated.workspaceId
      && binding.agentBindingId === updated.agentBindingId);
    if (index < 0) return [...bindings, updated];
    if (bindings[index].version >= updated.version) return bindings;
    const merged = [...bindings];
    merged[index] = updated;
    return merged;
  }, [...current]);
}

export function agentBindingMutationSuccessMessage(
  operation: AgentBindingMutationOperation,
  latestBinding: (AgentBindingVersionProjection & { enabled: boolean }) | undefined,
  refreshSucceeded = true,
): string {
  const desiredEnabled = operation === "enable";
  if (refreshSucceeded && !latestBinding) {
    return "The binding status could not be confirmed after the update. Refresh before relying on its current state.";
  }
  if (latestBinding && latestBinding.enabled !== desiredEnabled) {
    return "This binding changed again while its status was being confirmed. Review its current status before relying on the update.";
  }

  const success = operation === "enable"
    ? "This agent is enabled for future work. It will not become the Workspace default or start anything on its own."
    : "New Conversation and Task sessions cannot use this agent. Sessions already admitted stay attached; later Task steps may need another enabled agent.";
  return refreshSucceeded
    ? success
    : `${success} The binding list could not refresh; refresh to confirm its current status.`;
}

export function agentBindingMutationFailureMessage(
  operation: AgentBindingMutationOperation,
  bindingId: string,
  expectedVersion: number,
  currentBinding: WorkspaceAgentBindingVersionProjection | undefined,
  currentDefaultBindingId: string | null | undefined,
): string {
  if (operation === "disable" && currentDefaultBindingId === bindingId) {
    return "Clear or change the Workspace default before disabling this agent.";
  }
  if (currentBinding && currentBinding.version > expectedVersion) {
    return "The binding changed while this action was in progress. Review its current status before trying again.";
  }
  return `The binding could not be ${operation === "enable" ? "enabled" : "disabled"}. Refresh or retry; an ambiguous retry will reuse the same request.`;
}

function mutationKey(identity: AgentBindingMutationIdentity): string {
  return JSON.stringify([
    identity.operation,
    identity.workspaceId,
    identity.agentBindingId,
    identity.expectedVersion,
  ]);
}

export function getOrCreateAgentBindingMutationRequest(
  pending: Map<string, PendingAgentBindingMutation>,
  identity: AgentBindingMutationIdentity,
  createRequestId: () => string,
): PendingAgentBindingMutation {
  if (!identity.workspaceId.trim() || !identity.agentBindingId.trim()) {
    throw new Error("Workspace and AgentBinding are required.");
  }
  if (!Number.isSafeInteger(identity.expectedVersion) || identity.expectedVersion < 1) {
    throw new Error("Expected AgentBinding version is invalid.");
  }
  const key = mutationKey(identity);
  const current = pending.get(key);
  if (current) return current;
  const requestId = createRequestId();
  if (!requestId.trim()) throw new Error("AgentBinding request ID is required.");
  const request = Object.freeze({ ...identity, requestId });
  pending.set(key, request);
  return request;
}

export function clearAgentBindingMutationRequest(
  pending: Map<string, PendingAgentBindingMutation>,
  request: PendingAgentBindingMutation,
): void {
  const key = mutationKey(request);
  if (pending.get(key) === request) pending.delete(key);
}

export function reconcileAgentBindingMutationRequests(
  pending: Map<string, PendingAgentBindingMutation>,
  workspaceId: string,
  bindings: readonly AgentBindingVersionProjection[],
): void {
  if (!workspaceId) return;
  const versions = new Map(bindings.map(binding => [binding.agentBindingId, binding.version]));
  for (const [key, request] of pending) {
    if (request.workspaceId !== workspaceId) continue;
    const currentVersion = versions.get(request.agentBindingId);
    if (currentVersion !== undefined && currentVersion > request.expectedVersion) pending.delete(key);
  }
}
