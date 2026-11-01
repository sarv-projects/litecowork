import { parsePresentationItem, type PresentationFreshness, type PresentationItem } from "../presentation/presentation-types.ts";

export type TaskStreamEnvelope =
  | { sessionId: string; workspaceId: string; taskId: string; kind: "frame"; frameId: string; frame: unknown }
  | { sessionId: string; workspaceId: string; taskId: string; kind: "closed" }
  | { sessionId: string; workspaceId: string; taskId: string; kind: "error"; message?: string };

export interface TaskSnapshotStreamDriver {
  listen(listener: (envelope: TaskStreamEnvelope) => void): Promise<() => void>;
  start(input: { sessionId: string; workspaceId: string; taskId: string; resumeCursor: string | null }): Promise<void>;
  acknowledge(sessionId: string, frameId: string): Promise<void>;
  stop(sessionId: string): Promise<void>;
  newSessionId(): string;
}

export type TaskPresentationStreamStatus = "CONNECTING" | "LIVE" | "RECONNECTING" | "RESYNCING";

/** A unary SQLite read must not overwrite state after a newer stream snapshot arrived. */
export function isTaskSnapshotEpochCurrent(requestEpoch: number, streamEpoch: number): boolean {
  return Number.isSafeInteger(requestEpoch) && Number.isSafeInteger(streamEpoch) && requestEpoch === streamEpoch;
}

export type TaskStreamState = {
  ready: boolean;
  resyncRequired: boolean;
  snapshotReady: boolean;
  workspaceId: string | null;
  taskId: string | null;
  projectionVersion: number | null;
  projectionRevision: number;
  originSequence: string | null;
  computedAt: string | null;
  freshness: PresentationFreshness | null;
  cursor: string | null;
  maxFrameBytes: number;
  items: PresentationItem[];
};

export const emptyTaskStream: TaskStreamState = {
  ready: false,
  resyncRequired: false,
  snapshotReady: false,
  workspaceId: null,
  taskId: null,
  projectionVersion: null,
  projectionRevision: 0,
  originSequence: null,
  computedAt: null,
  freshness: null,
  cursor: null,
  maxFrameBytes: 0,
  items: [],
};

type TaskPresentationSnapshot = {
  workspace_id: string;
  task_id: string;
  origin_sequence: string;
  computed_at: string;
  freshness: PresentationFreshness;
  items: PresentationItem[];
};

const RETRY_DELAYS_MS = [500, 1_000, 2_000, 4_000, 8_000, 15_000, 30_000] as const;
const MAX_FRAME_BYTES = 1024 * 1024;
const MAX_CURSOR_LENGTH = 2048;
const wait = (milliseconds: number) => new Promise<void>(resolve => setTimeout(resolve, milliseconds));

function object(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : null;
}

function exactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  return Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
}

function boundedText(value: unknown, maximum: number): value is string {
  return typeof value === "string" && value.length > 0 && value.length <= maximum && !value.includes("\0");
}

function validCursor(value: unknown): value is string { return boundedText(value, MAX_CURSOR_LENGTH); }

function validEnvelope(value: unknown): value is TaskStreamEnvelope {
  const envelope = object(value);
  if (!envelope || typeof envelope.sessionId !== "string" || typeof envelope.workspaceId !== "string"
    || typeof envelope.taskId !== "string") return false;
  if (envelope.kind === "frame") return exactKeys(envelope, ["sessionId", "workspaceId", "taskId", "kind", "frameId", "frame"])
    && boundedText(envelope.frameId, 128);
  if (envelope.kind === "closed") return exactKeys(envelope, ["sessionId", "workspaceId", "taskId", "kind"]);
  if (envelope.kind === "error") return exactKeys(envelope, ["sessionId", "workspaceId", "taskId", "kind", ...(Object.hasOwn(envelope, "message") ? ["message"] : [])])
    && (envelope.message === undefined || typeof envelope.message === "string");
  return false;
}

function closedTaskItem(value: unknown): boolean {
  const item = object(value);
  if (!item) return false;
  const allowedItemKeys = ["item_key", "kind", "payload_version", "source_refs", "occurred_at", "order_key", "status", "label", "payload", "freshness"];
  const requiredItemKeys = ["item_key", "kind", "payload_version", "source_refs", "order_key", "status", "payload", "freshness"];
  if (Object.keys(item).some(key => !allowedItemKeys.includes(key)) || requiredItemKeys.some(key => !Object.hasOwn(item, key))) return false;
  if (!(item.kind === "TASK_CARD" || item.kind === "ACTIVITY" || item.kind === "ARTIFACT")) return false;
  if (item.occurred_at !== undefined && item.occurred_at !== null && !boundedText(item.occurred_at, 64)) return false;
  if (item.label !== undefined && item.label !== null && typeof item.label !== "string") return false;
  if (!Array.isArray(item.source_refs) || item.source_refs.length > 4) return false;
  for (const valueRef of item.source_refs) {
    const ref = object(valueRef);
    if (!ref || Object.keys(ref).some(key => !["kind", "id", "revision"].includes(key))
      || !["TASK", "STEP", "ARTIFACT_VERSION", "RESOURCE_REVISION"].includes(String(ref.kind))) return false;
  }
  const payload = object(item.payload);
  if (!payload) return false;
  const payloadKeys = item.kind === "TASK_CARD"
    ? ["task_id", "objective", "status", "step_summary"]
    : item.kind === "ACTIVITY" ? ["summary", "detail"]
      : ["artifact_id", "version", "display_name", "artifact_kind", "verification_status"];
  return Object.keys(payload).every(key => payloadKeys.includes(key));
}

function parseTaskSnapshot(value: unknown, workspaceId: string, taskId: string): TaskPresentationSnapshot | null {
  const payload = object(value);
  const fields = ["workspace_id", "task_id", "origin_sequence", "computed_at", "freshness", "items"];
  if (!payload || !exactKeys(payload, fields)
    || payload.workspace_id !== workspaceId || payload.task_id !== taskId
    || typeof payload.origin_sequence !== "string" || !/^[1-9][0-9]{0,18}$/.test(payload.origin_sequence)
    || !boundedText(payload.computed_at, 64)
    || !["CURRENT", "STALE", "UNKNOWN"].includes(String(payload.freshness))
    || !Array.isArray(payload.items) || payload.items.length > 301
    || !payload.items.every(closedTaskItem)) return null;
  if (payload.items.some(valueItem => {
    const item = object(valueItem);
    if (item?.kind !== "TASK_CARD") return false;
    return object(item.payload)?.task_id !== taskId;
  })) return null;
  const items = payload.items.map(parsePresentationItem);
  if (items.some(item => item === null)) return null;
  return {
    workspace_id: workspaceId,
    task_id: taskId,
    origin_sequence: payload.origin_sequence,
    computed_at: payload.computed_at,
    freshness: payload.freshness as PresentationFreshness,
    items: items as PresentationItem[],
  };
}

/** Reduce only the three Task stream frame forms; projection snapshots replace as one unit. */
export function reduceTaskStreamFrame(
  state: TaskStreamState,
  frame: unknown,
  expectedScope: { workspaceId: string; taskId: string },
): TaskStreamState {
  const value = object(frame);
  if (!value) return state;
  if (value.type === "stream.ready") {
    if (!exactKeys(value, ["type", "workspace_id", "projection_version", "cursor", "max_frame_bytes"])
      || value.workspace_id !== expectedScope.workspaceId
      || !Number.isSafeInteger(value.projection_version) || (value.projection_version as number) < 1
      || !validCursor(value.cursor)
      || !Number.isSafeInteger(value.max_frame_bytes) || (value.max_frame_bytes as number) < 1
      || (value.max_frame_bytes as number) > MAX_FRAME_BYTES) return state;
    return {
      ...state,
      ready: true,
      workspaceId: expectedScope.workspaceId,
      taskId: expectedScope.taskId,
      projectionVersion: value.projection_version as number,
      maxFrameBytes: value.max_frame_bytes as number,
    };
  }
  if (value.type === "stream.resync_required") {
    if (!exactKeys(value, ["type", "workspace_id", "reason"]) || !state.ready
      || value.workspace_id !== expectedScope.workspaceId || !boundedText(value.reason, 128)) return state;
    return {
      ...state,
      resyncRequired: true,
      snapshotReady: false,
      projectionRevision: 0,
      originSequence: null,
      computedAt: null,
      freshness: null,
      cursor: null,
      items: [],
    };
  }
  if (value.type !== "projection" || !state.ready
    || !exactKeys(value, ["type", "workspace_id", "projection_type", "projection_revision", "cursor", "payload"])
    || value.workspace_id !== expectedScope.workspaceId || value.projection_type !== "task_presentation"
    || !Number.isSafeInteger(value.projection_revision) || (value.projection_revision as number) < 1
    || !validCursor(value.cursor)) return state;
  const snapshot = parseTaskSnapshot(value.payload, expectedScope.workspaceId, expectedScope.taskId);
  if (!snapshot) return state;
  const originSequence = Number(snapshot.origin_sequence);
  if (!Number.isSafeInteger(originSequence)
    || Math.max(originSequence, 1) !== value.projection_revision
    || (state.snapshotReady && (value.projection_revision as number) <= state.projectionRevision)) return state;
  return {
    ...state,
    resyncRequired: false,
    snapshotReady: true,
    workspaceId: snapshot.workspace_id,
    taskId: snapshot.task_id,
    projectionRevision: value.projection_revision as number,
    originSequence: snapshot.origin_sequence,
    computedAt: snapshot.computed_at,
    freshness: snapshot.freshness,
    cursor: value.cursor,
    items: snapshot.items,
  };
}

export class TaskPresentationStreamController {
  private currentState: TaskStreamState = emptyTaskStream;
  private currentStatus: TaskPresentationStreamStatus = "CONNECTING";
  private generation = 0;
  private active = false;
  private runTask: Promise<void> | null = null;
  private sessionId: string | null = null;
  private unlisten: (() => void) | null = null;
  private closeCurrent: (() => void) | null = null;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;
  private retryWake: (() => void) | null = null;

  constructor(
    private readonly workspaceId: string,
    private readonly taskId: string,
    private readonly driver: TaskSnapshotStreamDriver,
    private readonly onState: (state: TaskStreamState, frame: unknown) => void = () => {},
    private readonly onStatus: (status: TaskPresentationStreamStatus) => void = () => {},
    private readonly delay: (milliseconds: number) => Promise<void> = wait,
  ) {}

  get state(): TaskStreamState { return this.currentState; }
  get status(): TaskPresentationStreamStatus { return this.currentStatus; }

  start(): void {
    if (this.active) return;
    this.active = true;
    const generation = ++this.generation;
    this.runTask = this.run(generation);
  }

  async dispose(): Promise<void> {
    if (!this.active && !this.runTask) return;
    this.active = false;
    this.generation += 1;
    this.closeCurrent?.();
    this.wakeRetry();
    this.unlisten?.();
    await this.runTask;
    this.runTask = null;
  }

  private setStatus(status: TaskPresentationStreamStatus): void {
    if (this.currentStatus === status) return;
    this.currentStatus = status;
    this.onStatus(status);
  }

  private isCurrent(generation: number): boolean { return this.active && generation === this.generation; }

  private async run(generation: number): Promise<void> {
    let retry = 0;
    while (this.isCurrent(generation)) {
      this.setStatus(retry === 0 ? "CONNECTING" : "RECONNECTING");
      const sessionId = this.driver.newSessionId();
      this.sessionId = sessionId;
      let close!: () => void;
      let hasClosed = false;
      const closed = new Promise<void>(resolve => {
        close = () => {
          if (hasClosed) return;
          hasClosed = true;
          resolve();
        };
      });
      this.closeCurrent = close;
      let unlisten: (() => void) | null = null;
      try {
        unlisten = await this.driver.listen(candidate => {
          if (!this.isCurrent(generation) || !candidate || typeof candidate !== "object" || Array.isArray(candidate)) return;
          const envelope = candidate as unknown as Record<string, unknown>;
          if (envelope.sessionId !== sessionId) return;
          if (envelope.kind === "frame" && typeof envelope.frameId === "string") {
            try {
              if (validEnvelope(candidate) && candidate.kind === "frame"
                && candidate.workspaceId === this.workspaceId && candidate.taskId === this.taskId) {
                const next = reduceTaskStreamFrame(this.currentState, candidate.frame, {
                  workspaceId: this.workspaceId,
                  taskId: this.taskId,
                });
                if (next !== this.currentState) {
                  this.currentState = next;
                  if (next.snapshotReady) this.setStatus("LIVE");
                  else if (next.resyncRequired) this.setStatus("RESYNCING");
                  this.onState(next, candidate.frame);
                }
              }
            } finally {
              // Every identified frame returns its credit once, even if its scope or
              // payload is malformed and therefore discarded by the reducer.
              void this.driver.acknowledge(sessionId, envelope.frameId as string).catch(close);
            }
            return;
          }
          if (!validEnvelope(candidate) || candidate.workspaceId !== this.workspaceId || candidate.taskId !== this.taskId) return;
          close();
        });
        this.unlisten = unlisten;
        if (!this.isCurrent(generation)) break;
        await this.driver.start({
          sessionId,
          workspaceId: this.workspaceId,
          taskId: this.taskId,
          resumeCursor: this.currentState.cursor,
        });
        if (!this.isCurrent(generation)) break;
        await closed;
      } catch {
        // Keep transport details out of the local presentation surface.
      } finally {
        unlisten?.();
        if (this.unlisten === unlisten) this.unlisten = null;
        if (this.sessionId === sessionId) this.sessionId = null;
        if (this.closeCurrent === close) this.closeCurrent = null;
        await this.driver.stop(sessionId).catch(() => {});
      }
      if (!this.isCurrent(generation)) break;
      retry += 1;
      this.setStatus("RECONNECTING");
      await this.sleep(RETRY_DELAYS_MS[Math.min(retry - 1, RETRY_DELAYS_MS.length - 1)]!);
    }
  }

  private async sleep(milliseconds: number): Promise<void> {
    let wake!: () => void;
    const awakened = new Promise<void>(resolve => { wake = resolve; });
    this.retryWake = wake;
    this.retryTimer = setTimeout(wake, milliseconds);
    await Promise.race([this.delay(milliseconds).catch(() => {}), awakened]);
    if (this.retryTimer) clearTimeout(this.retryTimer);
    this.retryTimer = null;
    this.retryWake = null;
  }

  private wakeRetry(): void {
    if (this.retryTimer) clearTimeout(this.retryTimer);
    this.retryTimer = null;
    this.retryWake?.();
    this.retryWake = null;
  }
}
