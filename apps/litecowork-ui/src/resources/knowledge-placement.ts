/**
 * Explicit local/cloud placement for a Resource.
 *
 * LiteCowork Core owns the placement decision. The UI decodes that decision and shows
 * it; it never infers placement from a file path, a Provider, or an Agent, and it
 * never offers a cloud consumer a local-only Resource.
 */

export type ResourcePlacement = "LOCAL_ONLY" | "CLOUD_ELIGIBLE";

export type WorkspaceReplicationPolicy =
  | "LOCAL_ONLY"
  | "METADATA_ONLY"
  | "ACTIVE_TASK_INPUTS"
  | "SELECTED_FOLDERS";

export type TaskPlacement = "LOCAL" | "CLOUD";

export type PlacementDecision = {
  resourceId: string;
  placement: ResourcePlacement;
  policy: WorkspaceReplicationPolicy;
  blockedCode: string | null;
  blockedReason: string | null;
};

const POLICIES: WorkspaceReplicationPolicy[] = [
  "LOCAL_ONLY",
  "METADATA_ONLY",
  "ACTIVE_TASK_INPUTS",
  "SELECTED_FOLDERS",
];

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

export function decodePlacementDecision(
  value: unknown,
  expected: { workspaceId: string; resourceId: string },
): PlacementDecision {
  if (!isRecord(value)) {
    throw new Error("The local Runtime returned an invalid Resource placement.");
  }
  if (value.workspaceId !== expected.workspaceId || value.resourceId !== expected.resourceId) {
    throw new Error("The local Runtime returned a Resource placement for another Resource.");
  }
  if (value.placement !== "LOCAL_ONLY" && value.placement !== "CLOUD_ELIGIBLE") {
    throw new Error("The local Runtime returned an unsupported Resource placement.");
  }
  if (typeof value.policy !== "string" || !(POLICIES as string[]).includes(value.policy)) {
    throw new Error("The local Runtime returned an unsupported Workspace placement policy.");
  }
  const blockedCode = typeof value.blockedCode === "string" ? value.blockedCode : null;
  const blockedReason = typeof value.blockedReason === "string" ? value.blockedReason : null;
  if (value.placement === "LOCAL_ONLY" && blockedCode === null && value.blockedFor === "CLOUD") {
    throw new Error("The local Runtime reported a local-only Resource without a placement blocker.");
  }
  return {
    resourceId: value.resourceId,
    placement: value.placement,
    policy: value.policy as WorkspaceReplicationPolicy,
    blockedCode,
    blockedReason,
  };
}

/** True when a consumer at this placement may use the Resource. */
export function placementAllowsTask(
  decision: PlacementDecision,
  taskPlacement: TaskPlacement,
): boolean {
  if (taskPlacement === "LOCAL") return true;
  return decision.placement === "CLOUD_ELIGIBLE";
}

/** The owner-visible placement label. */
export function placementLabel(decision: PlacementDecision): string {
  switch (decision.placement) {
    case "LOCAL_ONLY":
      return "Local only. This source stays on this device.";
    case "CLOUD_ELIGIBLE":
      return `Cloud eligible under ${decision.policy}.`;
    default:
      return "The placement is unknown.";
  }
}

/** The reason a cloud consumer may not use this Resource, when there is one. */
export function placementBlocker(
  decision: PlacementDecision,
  taskPlacement: TaskPlacement,
): { code: string; reason: string } | null {
  if (placementAllowsTask(decision, taskPlacement)) return null;
  return {
    code: decision.blockedCode ?? "RESOURCE_LOCAL_ONLY",
    reason: decision.blockedReason
      ?? "This Resource is local-only and cannot be resolved for a cloud Task.",
  };
}
