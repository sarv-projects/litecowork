export type PlanningBlocker =
  | "TASK_STATE_NOT_ELIGIBLE"
  | "PLAN_ALREADY_ACCEPTED"
  | "CURRENT_LEAD_OR_ENDPOINT_UNAVAILABLE"
  | "TASK_ISOLATION_UNAVAILABLE"
  | "NATIVE_CAPABILITIES_UNMEDIATED"
  | "PROTOCOL_UNQUALIFIED"
  | "PROCESS_CONTAINMENT_UNQUALIFIED"
  | "PLANNING_CONTEXT_RESOURCE_UNAVAILABLE"
  | "SESSION_SETTLEMENT_UNAVAILABLE"
  | "PROVIDER_UNSUPPORTED";

export type TaskStatus =
  | "READY"
  | "RUNNING"
  | "WAITING_USER"
  | "BLOCKED"
  | "VERIFYING"
  | "NEEDS_USER"
  | "INCOMPLETE"
  | "PAUSE_REQUESTED"
  | "PAUSED"
  | "COMPLETED"
  | "FAILED"
  | "CANCEL_REQUESTED"
  | "CANCELLED";

export type TaskPlanningReadinessView = {
  taskId: string;
  taskVersion: number;
  taskSpecRevision: number;
  taskStatus: TaskStatus;
  observedAt: string;
  dispatchAvailable: false;
  planningStarted: false;
  agentSessionStarted: false;
  planCreated: false;
  blockers: PlanningBlocker[];
};

export type TaskPlanningReadinessExpectation = Pick<
  TaskPlanningReadinessView,
  "taskId" | "taskVersion" | "taskSpecRevision" | "taskStatus"
>;

const PLANNING_BLOCKERS: readonly PlanningBlocker[] = [
  "TASK_STATE_NOT_ELIGIBLE",
  "PLAN_ALREADY_ACCEPTED",
  "CURRENT_LEAD_OR_ENDPOINT_UNAVAILABLE",
  "TASK_ISOLATION_UNAVAILABLE",
  "NATIVE_CAPABILITIES_UNMEDIATED",
  "PROTOCOL_UNQUALIFIED",
  "PROCESS_CONTAINMENT_UNQUALIFIED",
  "PLANNING_CONTEXT_RESOURCE_UNAVAILABLE",
  "SESSION_SETTLEMENT_UNAVAILABLE",
  "PROVIDER_UNSUPPORTED",
];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isPlanningBlocker(value: unknown): value is PlanningBlocker {
  return typeof value === "string" && PLANNING_BLOCKERS.includes(value as PlanningBlocker);
}

/**
 * Confirms the readiness response still describes the visible Task and remains
 * a diagnostic only. A malformed or execution-capable response is rejected.
 */
export function isTaskPlanningReadinessFor(
  value: unknown,
  expected: TaskPlanningReadinessExpectation,
): value is TaskPlanningReadinessView {
  if (!isRecord(value)) return false;
  return value.taskId === expected.taskId
    && value.taskVersion === expected.taskVersion
    && value.taskSpecRevision === expected.taskSpecRevision
    && value.taskStatus === expected.taskStatus
    && typeof value.observedAt === "string"
    && value.dispatchAvailable === false
    && value.planningStarted === false
    && value.agentSessionStarted === false
    && value.planCreated === false
    && Array.isArray(value.blockers)
    && value.blockers.every(isPlanningBlocker);
}

/** Returns the diagnostic's empty-blocker message without implying readiness. */
export function planningReadinessNoBlockersMessage(
  blockers: readonly PlanningBlocker[],
): string | null {
  return blockers.length === 0
    ? "No local preflight blocker was observed. Planning dispatch is unavailable; this result does not authorize or start work."
    : null;
}
