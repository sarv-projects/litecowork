export type ConversationAgentBindingOption = {
  agentBindingId: string;
  workspaceId: string;
  agentProfileId: string;
  enabled: boolean;
  leadEligible: boolean;
};

export type ConversationAgentProfileOption = {
  agentProfileId: string;
  displayName: string;
};

export type ConversationAgentChoice = {
  agentBindingId: string;
  label: string;
  selectable: boolean;
};

/** Only current-workspace bindings that can lead a Conversation are selectable. */
export function buildConversationAgentChoices(
  workspaceId: string,
  bindings: readonly ConversationAgentBindingOption[],
  profiles: readonly ConversationAgentProfileOption[],
  currentBindingId: string | null,
): ConversationAgentChoice[] {
  const profileNames = new Map(profiles.map(profile => [
    profile.agentProfileId,
    profile.displayName.trim(),
  ]));
  const seenBindingIds = new Set<string>();
  const eligible = bindings.filter(binding => {
    if (binding.workspaceId !== workspaceId
      || !binding.enabled
      || !binding.leadEligible
      || !binding.agentBindingId
      || seenBindingIds.has(binding.agentBindingId)) return false;
    seenBindingIds.add(binding.agentBindingId);
    return true;
  });
  const baseNames = eligible.map(binding =>
    profileNames.get(binding.agentProfileId) || `Agent · ${binding.agentBindingId}`,
  );
  const nameCounts = new Map<string, number>();
  for (const name of baseNames) nameCounts.set(name, (nameCounts.get(name) ?? 0) + 1);
  const choices = eligible.map((binding, index) => {
    const name = baseNames[index]!;
    return {
      agentBindingId: binding.agentBindingId,
      label: (nameCounts.get(name) ?? 0) > 1 ? `${name} · ${binding.agentBindingId}` : name,
      selectable: true,
    };
  });
  if (currentBindingId && !eligible.some(binding => binding.agentBindingId === currentBindingId)) {
    choices.push({
      agentBindingId: currentBindingId,
      label: "Unavailable selected agent",
      selectable: false,
    });
  }
  return choices.sort((left, right) => left.label.localeCompare(right.label));
}
