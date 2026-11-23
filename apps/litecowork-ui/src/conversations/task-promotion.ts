import type {
  ConversationAgentBindingOption,
  ConversationAgentProfileOption,
} from "./conversation-agent-selection";
import type {
  ConversationMessageView,
  ConversationResourceRefView,
} from "./conversation-view";

export type TaskPromotionSource = {
  conversationId: string;
  turnId: string;
  userMessageId: string;
  agentMessageId: string;
  objective: string;
  pinnedInputs: PinnedConversationResourceRef[];
  unresolvedInputs: ConversationResourceRefView[];
};

export type PinnedConversationResourceRef = ConversationResourceRefView & { revision_id: string };

export type TaskPromotionDraft = {
  workspaceId: string;
  source: TaskPromotionSource;
  objective: string;
  leadAgentBindingId: string;
  inputRefs: PinnedConversationResourceRef[];
};

export type TaskPromotionRequest = {
  workspaceId: string;
  conversationId: string;
  sourceMessageRefs: readonly [string, string];
  objective: string;
  preferredLeadAgentBindingId: string;
  inputRefs: readonly PinnedConversationResourceRef[];
  requestId: string;
};

export type TaskPromotionLead = {
  agentBindingId: string;
  label: string;
};

function semanticText(message: ConversationMessageView): string {
  return message.content
    .filter((block): block is { kind: "TEXT"; text: string } => block.kind === "TEXT")
    .map(block => block.text)
    .join("\n")
    .trim();
}

/**
 * Resolves promotion only from immutable, loaded message identities and their
 * same-snapshot exact turn status. This is a display gate; Task storage rechecks
 * the entire source pair in its own transaction.
 */
export function findTaskPromotionSource(
  workspaceId: string,
  conversationId: string,
  messages: readonly ConversationMessageView[],
  agentMessageId: string,
): TaskPromotionSource | null {
  if (!workspaceId || !conversationId || !agentMessageId) return null;
  const agentIndex = messages.findIndex(message => message.message_id === agentMessageId);
  if (agentIndex < 0) return null;
  const agent = messages[agentIndex]!;
  if (agent.role !== "AGENT" || agent.turn_status !== "COMPLETED" || !agent.turn_id) return null;

  const userMessages = messages.slice(0, agentIndex).filter(message =>
    message.role === "USER"
      && message.turn_id === agent.turn_id
      && message.turn_status === "COMPLETED",
  );
  if (userMessages.length !== 1) return null;
  const user = userMessages[0]!;
  const objective = semanticText(user);
  if (!objective || new TextEncoder().encode(objective).byteLength > 32 * 1024) return null;

  const resourceRefs = user.resource_refs.filter(reference =>
    reference.workspace_id === workspaceId
      && reference.resource_id.trim().length > 0,
  );
  const pinnedInputs = resourceRefs.filter((reference): reference is PinnedConversationResourceRef =>
    typeof reference.revision_id === "string" && reference.revision_id.trim().length > 0,
  );
  const unresolvedInputs = resourceRefs.filter(reference =>
    reference.revision_id === null || reference.revision_id.trim().length === 0,
  );

  return {
    conversationId,
    turnId: agent.turn_id,
    userMessageId: user.message_id,
    agentMessageId: agent.message_id,
    objective,
    pinnedInputs: pinnedInputs.map(reference => ({ ...reference })),
    unresolvedInputs: unresolvedInputs.map(reference => ({ ...reference })),
  };
}

/** Task lead eligibility is independent from whether the binding can lead a Chat turn. */
export function taskPromotionLeads(
  workspaceId: string,
  bindings: readonly ConversationAgentBindingOption[],
  profiles: readonly ConversationAgentProfileOption[],
): TaskPromotionLead[] {
  const profileNames = new Map(profiles.map(profile => [profile.agentProfileId, profile.displayName.trim()]));
  const eligible = bindings.filter(binding =>
    binding.workspaceId === workspaceId
      && binding.enabled
      && binding.leadEligible
      && /^[A-Za-z0-9_-]{1,200}$/.test(binding.agentBindingId),
  );
  const unique = new Map<string, ConversationAgentBindingOption>();
  for (const binding of eligible) unique.set(binding.agentBindingId, binding);
  const base = [...unique.values()].map(binding => ({
    agentBindingId: binding.agentBindingId,
    baseLabel: profileNames.get(binding.agentProfileId) || `Agent · ${binding.agentBindingId}`,
  }));
  const counts = new Map<string, number>();
  for (const item of base) counts.set(item.baseLabel, (counts.get(item.baseLabel) ?? 0) + 1);
  return base.map(item => ({
    agentBindingId: item.agentBindingId,
    label: (counts.get(item.baseLabel) ?? 0) > 1
      ? `${item.baseLabel} · ${item.agentBindingId}`
      : item.baseLabel,
  })).sort((left, right) => left.label.localeCompare(right.label));
}

export function taskPromotionKey(workspaceId: string, source: TaskPromotionSource): string {
  return [workspaceId, source.conversationId, source.userMessageId, source.agentMessageId].join("\0");
}

/**
 * In-memory exact-request cache for this desktop session. An uncertain result may
 * only be retried with the original id and body; callers clear it only after an
 * exact committed response or a typed, non-retryable pre-commit refusal.
 */
export class ConversationTaskPromotionCache {
  private readonly requests = new Map<string, TaskPromotionRequest>();

  create(key: string, draft: TaskPromotionDraft, requestId: () => string): TaskPromotionRequest {
    const existing = this.requests.get(key);
    if (existing) return existing;
    const frozenRefs = Object.freeze(draft.inputRefs.map(reference => Object.freeze({ ...reference })));
    const request: TaskPromotionRequest = Object.freeze({
      workspaceId: draft.workspaceId,
      conversationId: draft.source.conversationId,
      sourceMessageRefs: Object.freeze([draft.source.userMessageId, draft.source.agentMessageId]) as readonly [string, string],
      objective: draft.objective,
      preferredLeadAgentBindingId: draft.leadAgentBindingId,
      inputRefs: frozenRefs,
      requestId: requestId(),
    });
    this.requests.set(key, request);
    return request;
  }

  get(key: string): TaskPromotionRequest | null {
    return this.requests.get(key) ?? null;
  }

  clear(key: string, requestId: string): boolean {
    if (this.requests.get(key)?.requestId !== requestId) return false;
    this.requests.delete(key);
    return true;
  }
}

export function isKnownNoWriteTaskError(error: unknown): boolean {
  if (typeof error !== "string") return false;
  const match = /\[([A-Z_]+); retryable=(true|false); reference=([^\]]{1,128})\]/.exec(error);
  return match !== null
    && match[2] === "false"
    && ["AGENT_UNAVAILABLE", "INVALID_ARGUMENT", "FORBIDDEN", "WORKSPACE_ARCHIVED"].includes(match[1]!);
}
