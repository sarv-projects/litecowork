import type { components } from "../api/operator-api.ts";

export type AgentControlDescriptor = components["schemas"]["AgentControlDescriptor"];
export type ConversationAttachment = { id: string; kind: "IMAGE" | "FILE" | "RESOURCE"; name: string };
export type ConversationNativeToken = {
  id: string;
  kind: "SLASH_COMMAND" | "REFERENCE";
  range: [number, number];
  ownerBindingId: string;
  descriptorDigest: string;
  nativeId: string;
  rawLiteral: string;
  status: "VALID" | "NEEDS_REFRESH" | "INCOMPATIBLE";
};

export type ConversationComposerState = {
  workspaceId: string;
  conversationId: string;
  agentBindingId: string;
  descriptor: AgentControlDescriptor | null;
  descriptorObservedAt: string;
  enabled: boolean;
  leadEligible: boolean;
  draft: string;
  attachments: ConversationAttachment[];
  nativeTokens: ConversationNativeToken[];
  selectedOptions: Record<string, unknown>;
};

export type ConversationAgentSelection = Pick<ConversationComposerState,
  "workspaceId" | "conversationId" | "agentBindingId" | "descriptor" |
  "descriptorObservedAt" | "enabled" | "leadEligible">;

export type ConversationAgentSurfaceStatus =
  | "READY" | "BLOCKED" | "NEEDS_AUTH" | "NEEDS_REFRESH"
  | "INCOMPATIBLE_DRAFT" | "INVALID_INPUT";

export type ConversationAgentSurface = {
  status: ConversationAgentSurfaceStatus;
  controls: {
    commands: AgentControlDescriptor["input_surface"]["slash_commands"];
    references: AgentControlDescriptor["input_surface"]["reference_kinds"];
    options: NonNullable<AgentControlDescriptor["session_options"]>["options"];
    images: boolean;
    files: boolean;
    resources: boolean;
    hostActionsAreSeparate: true;
  } | null;
  blockers: string[];
};

function freshDescriptor(state: ConversationComposerState, nowMs: number): boolean {
  const descriptor = state.descriptor;
  if (!descriptor || !state.descriptorObservedAt.trim()) return false;
  const expires = Date.parse(descriptor.expires_at);
  const observed = Date.parse(state.descriptorObservedAt);
  const descriptorObserved = Date.parse(descriptor.observed_at);
  const installation = descriptor.installation;
  const installationObserved = Date.parse(installation.observed_at);
  const installationExpires = Date.parse(installation.expires_at);
  const installed = installation.state === "INSTALLED" || installation.state === "UPDATE_AVAILABLE";
  return Number.isFinite(expires) && Number.isFinite(observed) && Number.isFinite(descriptorObserved)
    && Number.isFinite(installationObserved) && Number.isFinite(installationExpires)
    && descriptorObserved <= nowMs && descriptorObserved < expires
    && observed <= nowMs && expires > nowMs && observed < expires
    && installationObserved <= nowMs && installationExpires > nowMs
    && installationObserved < installationExpires && installationExpires <= expires
    && installation.registry_agent_id === descriptor.registry_agent_id
    && installation.module_id === descriptor.module_id
    && installation.runtime_id.trim().length > 0
    && installation.runtime_incarnation_id.trim().length > 0
    && installed;
}

function tokenExists(token: ConversationNativeToken, descriptor: AgentControlDescriptor): boolean {
  if (token.kind === "SLASH_COMMAND") {
    return descriptor.input_surface.slash_commands.some(command =>
      command.command_id === token.nativeId
      && (token.rawLiteral === command.token || token.rawLiteral.startsWith(`${command.token} `)));
  }
  return descriptor.input_surface.reference_kinds.some(reference =>
    reference.reference_kind_id === token.nativeId && token.rawLiteral.startsWith(reference.trigger));
}

function tokenMatchesDraft(token: ConversationNativeToken, draft: string): boolean {
  const [start, end] = token.range;
  return Number.isInteger(start) && Number.isInteger(end)
    && start >= 0 && end > start && end <= draft.length
    && draft.slice(start, end) === token.rawLiteral;
}

export function deriveConversationAgentSurface(
  state: ConversationComposerState,
  nowMs: number,
): ConversationAgentSurface {
  if (!state.enabled || !state.leadEligible) {
    return { status: "BLOCKED", controls: null, blockers: ["This agent is not enabled for new conversations."] };
  }
  const descriptor = state.descriptor;
  if (!descriptor || !freshDescriptor(state, nowMs)) {
    return { status: "NEEDS_REFRESH", controls: null, blockers: ["Refresh this agent’s current controls before sending."] };
  }
  if (descriptor.auth.state !== "AUTHENTICATED") {
    return { status: "NEEDS_AUTH", controls: null, blockers: ["Complete this agent’s sign-in before sending."] };
  }

  const blockers: string[] = [];
  if (state.draft.trim() && !descriptor.input_surface.text) blockers.push("This agent does not accept text input.");
  for (const attachment of state.attachments) {
    const supported = attachment.kind === "IMAGE" ? descriptor.input_surface.images
      : attachment.kind === "FILE" ? descriptor.input_surface.files
        : descriptor.input_surface.resources;
    if (!supported) blockers.push(`${attachment.name} is retained, but this agent does not support ${attachment.kind.toLowerCase()} input.`);
  }
  for (const token of state.nativeTokens) {
    if (token.status !== "VALID" || token.ownerBindingId !== state.agentBindingId
      || token.descriptorDigest !== descriptor.descriptor_digest || !tokenExists(token, descriptor)
      || !tokenMatchesDraft(token, state.draft)) {
      blockers.push(`${token.rawLiteral} needs to be selected again for this agent.`);
    }
  }

  return {
    status: blockers.length ? "INCOMPATIBLE_DRAFT" : "READY",
    controls: {
      commands: descriptor.input_surface.slash_commands,
      references: descriptor.input_surface.reference_kinds,
      options: descriptor.session_options?.options ?? [],
      images: descriptor.input_surface.images,
      files: descriptor.input_surface.files,
      resources: descriptor.input_surface.resources,
      hostActionsAreSeparate: true,
    },
    blockers,
  };
}

/** Agent-owned choices are cleared atomically; user text, attachments and token literals survive. */
export function switchConversationAgent(
  state: ConversationComposerState,
  selection: ConversationAgentSelection,
): ConversationComposerState {
  const sameBinding = state.agentBindingId === selection.agentBindingId;
  const sameScope = state.workspaceId === selection.workspaceId
    && state.conversationId === selection.conversationId;
  const descriptor = selection.descriptor;
  const sameDescriptor = sameBinding && state.descriptor !== null && descriptor !== null
    && state.descriptor.descriptor_digest === descriptor.descriptor_digest;
  const nativeTokens = state.nativeTokens.map(token => {
    const remainsValid = sameBinding && sameScope && descriptor !== null
      && token.ownerBindingId === selection.agentBindingId
      && token.descriptorDigest === descriptor.descriptor_digest
      && tokenExists(token, descriptor);
    return { ...token, status: remainsValid ? "VALID" as const : "INCOMPATIBLE" as const };
  });
  return {
    ...state,
    ...selection,
    draft: sameScope ? state.draft : "",
    attachments: sameScope ? state.attachments : [],
    nativeTokens: sameScope ? nativeTokens : [],
    selectedOptions: sameDescriptor && sameScope ? state.selectedOptions : {},
  };
}
