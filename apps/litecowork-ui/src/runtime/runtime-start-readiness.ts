export type RuntimeStartObservation = {
  state?: string;
  processRunning: boolean;
  operatorReady: boolean;
  daemonAvailable: boolean;
};

/**
 * Whether the desktop may offer a start/retry action for the local Runtime.
 *
 * Runtime lifecycle verification can mark an otherwise stopped installation as
 * DEGRADED (for example, when the Linux managed unit has not been installed yet).
 * Startup itself performs the authoritative lifecycle checks and fails closed on
 * conflicting or unsafe service configuration. The UI therefore gates this action
 * on observed process/readiness state and daemon availability, not the summary label.
 */
export function canOfferLocalRuntimeStart(
  status: RuntimeStartObservation | null | undefined,
): boolean {
  return status != null
    && status.daemonAvailable
    && !status.processRunning
    && !status.operatorReady;
}
