export type RuntimeInventoryStreamEnvelope =
  | { sessionId: string; workspaceId: string; runtimeId: string; kind: "frame"; frameId: string; frame: unknown }
  | { sessionId: string; workspaceId: string; runtimeId: string; kind: "closed" }
  | { sessionId: string; workspaceId: string; runtimeId: string; kind: "error"; message?: string };

export interface RuntimeInventoryStreamDriver {
  listen(listener: (envelope: RuntimeInventoryStreamEnvelope) => void): Promise<() => void>;
  start(input: { sessionId: string; workspaceId: string; runtimeId: string; resumeCursor: string | null }): Promise<void>;
  acknowledge(sessionId: string, frameId: string): Promise<void>;
  stop(sessionId: string): Promise<void>;
  newSessionId(): string;
}

export type RuntimeInventoryStreamStatus = "CONNECTING" | "LIVE" | "RECONNECTING" | "STOPPED";

const RETRY_DELAYS_MS = [500, 1_000, 2_000, 4_000, 8_000, 15_000, 30_000] as const;
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

function validEnvelope(value: unknown): value is RuntimeInventoryStreamEnvelope {
  const envelope = object(value);
  if (!envelope || !boundedText(envelope.sessionId, 128) || !boundedText(envelope.workspaceId, 256)
    || !boundedText(envelope.runtimeId, 256)) return false;
  if (envelope.kind === "frame") return exactKeys(envelope, ["sessionId", "workspaceId", "runtimeId", "kind", "frameId", "frame"])
    && boundedText(envelope.frameId, 128);
  if (envelope.kind === "closed") return exactKeys(envelope, ["sessionId", "workspaceId", "runtimeId", "kind"]);
  if (envelope.kind === "error") return exactKeys(envelope, ["sessionId", "workspaceId", "runtimeId", "kind", ...(Object.hasOwn(envelope, "message") ? ["message"] : [])])
    && (envelope.message === undefined || typeof envelope.message === "string");
  return false;
}

function validReady(frame: unknown, workspaceId: string): boolean {
  const value = object(frame);
  return value !== null && exactKeys(value, ["type", "workspace_id", "projection_version", "cursor", "max_frame_bytes"])
    && value.type === "stream.ready" && value.workspace_id === workspaceId
    && Number.isSafeInteger(value.projection_version) && Number(value.projection_version) >= 1
    && boundedText(value.cursor, 2048) && Number.isSafeInteger(value.max_frame_bytes)
    && Number(value.max_frame_bytes) > 0 && Number(value.max_frame_bytes) <= 1_048_576;
}

function validNotification(frame: unknown, workspaceId: string, runtimeId: string): frame is Record<string, unknown> {
  const value = object(frame);
  const payload = value && object(value.payload);
  return value !== null && payload !== null
    && exactKeys(value, ["type", "workspace_id", "projection_type", "projection_revision", "cursor", "payload"])
    && value.type === "projection" && value.workspace_id === workspaceId
    && value.projection_type === "runtime_inventory"
    && Number.isSafeInteger(value.projection_revision) && Number(value.projection_revision) >= 1
    && boundedText(value.cursor, 2048)
    && exactKeys(payload, ["runtime_id", "change_kind"])
    && payload.runtime_id === runtimeId && payload.change_kind === "OFFERS_CHANGED";
}

function validResync(frame: unknown, workspaceId: string): boolean {
  const value = object(frame);
  return value !== null && exactKeys(value, ["type", "workspace_id", "reason"])
    && value.type === "stream.resync_required" && value.workspace_id === workspaceId
    && boundedText(value.reason, 128);
}

export class RuntimeInventoryStreamController {
  private currentStatus: RuntimeInventoryStreamStatus = "STOPPED";
  private cursor: string | null = null;
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
    private readonly runtimeId: string,
    private readonly driver: RuntimeInventoryStreamDriver,
    private readonly onInvalidation: () => void,
    private readonly onStatus: (status: RuntimeInventoryStreamStatus) => void = () => {},
    private readonly delay: (milliseconds: number) => Promise<void> = wait,
  ) {}

  get status(): RuntimeInventoryStreamStatus { return this.currentStatus; }

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
    this.setStatus("STOPPED");
  }

  private setStatus(status: RuntimeInventoryStreamStatus): void {
    if (this.currentStatus === status) return;
    this.currentStatus = status;
    this.onStatus(status);
  }

  private isCurrent(generation: number): boolean {
    return this.active && generation === this.generation;
  }

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
        unlisten = await this.driver.listen(value => {
          if (!this.isCurrent(generation) || !validEnvelope(value) || value.sessionId !== sessionId) return;
          if (value.kind === "frame") {
            try {
              if (value.workspaceId === this.workspaceId && value.runtimeId === this.runtimeId) {
                if (validReady(value.frame, this.workspaceId)) return;
                if (validResync(value.frame, this.workspaceId)) {
                  this.cursor = null;
                  return;
                }
                if (validNotification(value.frame, this.workspaceId, this.runtimeId)) {
                  this.cursor = value.frame.cursor as string;
                  this.setStatus("LIVE");
                  this.onInvalidation();
                }
              }
            } finally {
              void this.driver.acknowledge(sessionId, value.frameId).catch(close);
            }
            return;
          }
          if (value.workspaceId === this.workspaceId && value.runtimeId === this.runtimeId) close();
        });
        this.unlisten = unlisten;
        if (!this.isCurrent(generation)) break;
        await this.driver.start({
          sessionId,
          workspaceId: this.workspaceId,
          runtimeId: this.runtimeId,
          resumeCursor: this.cursor,
        });
        if (!this.isCurrent(generation)) break;
        await closed;
      } catch {
        // Transport details are private; the visible status reports reconnecting.
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
    await Promise.race([this.delay(milliseconds), awakened]);
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
