// Derives the Settings > Runtime health rows from observed Runtime state only.
// Nothing here is inferred from a click: every value comes from the readiness
// probe (`operatorReady`) or the lifecycle view (`recoveryState`).

export type RuntimeHealthInput = {
  operatorReady: boolean;
  availability: string | null;
  recoveryState: string | null;
};

export type RuntimeHealthRows = {
  transport: string;
  operator: string;
  recoveryState: string;
  execution: string;
  executionReady: boolean;
};

export type RuntimeExecutionReadinessInput = {
  operatorReady: boolean;
  runtimeId: string | null | undefined;
  incarnationId: string | null | undefined;
  lifecycleIdentity: string | null | undefined;
  lifecycleRuntimeId: string | null | undefined;
  lifecycleIncarnationId: string | null | undefined;
  availability: string | null | undefined;
  recoveryState: string | null | undefined;
};

/** The UI may enable fresh Conversation work only from the exact current lifecycle projection. */
export function isRuntimeExecutionReady(input: RuntimeExecutionReadinessInput): boolean {
  const runtimeId = input.runtimeId?.trim();
  const incarnationId = input.incarnationId?.trim();
  if (!input.operatorReady || !runtimeId || !incarnationId) return false;
  return input.lifecycleIdentity === `${runtimeId}:${incarnationId}`
    && input.lifecycleRuntimeId === runtimeId
    && input.lifecycleIncarnationId === incarnationId
    && input.availability === "ONLINE"
    && input.recoveryState === "READY";
}

export function localIpcTransportLabel(platform: string): string {
  return /^win/i.test(platform) ? "Local IPC (named pipe)" : "Local IPC (socket)";
}

export function deriveRuntimeHealthRows(
  input: RuntimeHealthInput,
  platform: string,
): RuntimeHealthRows {
  const transport = localIpcTransportLabel(platform);
  const recoveryState = input.recoveryState ?? "UNKNOWN";
  const availability = input.availability ?? "UNKNOWN";
  const executionReady = input.operatorReady && availability === "ONLINE" && recoveryState === "READY";
  let execution = "Closed";
  if (executionReady) {
    execution = "Open";
  } else if (input.operatorReady && (availability === "DEGRADED" || recoveryState === "DEGRADED")) {
    execution = "Closed (degraded; Operator commands remain available)";
  } else if (input.operatorReady && availability !== "ONLINE") {
    execution = "Closed (Runtime is not online)";
  } else if (!input.operatorReady) {
    execution = "Closed (Operator not serving)";
  }
  return {
    transport,
    operator: input.operatorReady ? `Serving over ${transport}` : "Not serving",
    recoveryState,
    execution,
    executionReady,
  };
}
