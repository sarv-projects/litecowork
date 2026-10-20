/** Move a selected lead by `delta` positions without mutating the editor draft. */
export function moveFallbackLead(
  orderedBindingIds: readonly string[],
  bindingId: string,
  delta: number,
): string[] {
  const currentIndex = orderedBindingIds.indexOf(bindingId);
  if (
    currentIndex < 0 ||
    !Number.isInteger(delta) ||
    new Set(orderedBindingIds).size !== orderedBindingIds.length
  ) {
    return [...orderedBindingIds];
  }
  const nextIndex = Math.max(0, Math.min(orderedBindingIds.length - 1, currentIndex + delta));
  if (nextIndex === currentIndex) return [...orderedBindingIds];
  const reordered = [...orderedBindingIds];
  reordered.splice(currentIndex, 1);
  reordered.splice(nextIndex, 0, bindingId);
  return reordered;
}

/** A Task's current lead cannot also be an automatic fallback for itself. */
export function excludeCurrentLeadFallbacks(
  orderedBindingIds: readonly string[],
  currentLeadBindingId: string | null | undefined,
): string[] {
  return [...new Set(orderedBindingIds)].filter(id => id !== currentLeadBindingId);
}

export type LeadFailoverDraft = {
  mode: "DISABLED" | "ASK" | "ALLOW_LISTED";
  triggers: readonly string[];
  fallback_agent_binding_ids: readonly string[];
  max_lead_changes: number;
};

/** Mirror the bounded policy shape so invalid revisions cannot be submitted. */
export function isValidLeadFailoverDraft(
  policy: LeadFailoverDraft,
  currentLeadBindingId: string | null | undefined,
): boolean {
  const fallbacks = policy.fallback_agent_binding_ids;
  if (fallbacks.length > 3 || new Set(fallbacks).size !== fallbacks.length) return false;
  if (currentLeadBindingId && fallbacks.includes(currentLeadBindingId)) return false;
  if (policy.mode === "DISABLED") {
    return policy.triggers.length === 0 && fallbacks.length === 0 && policy.max_lead_changes === 0;
  }
  if (policy.triggers.length === 0 || new Set(policy.triggers).size !== policy.triggers.length) return false;
  if (policy.mode === "ASK") return policy.max_lead_changes === 0;
  return fallbacks.length > 0
    && Number.isInteger(policy.max_lead_changes)
    && policy.max_lead_changes >= 1
    && policy.max_lead_changes <= fallbacks.length;
}
