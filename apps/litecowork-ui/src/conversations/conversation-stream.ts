import { emptyPresentationStream, reducePresentationFrame, type PresentationStreamState } from "../presentation/presentation-stream.ts";
import type { ConversationSnapshotView } from "./conversation-view.ts";

export type StreamEnvelope =
  | { sessionId: string; workspaceId: string; conversationId: string; kind: "frame"; frameId: string; frame: unknown }
  | { sessionId: string; workspaceId: string; conversationId: string; kind: "closed" }
  | { sessionId: string; workspaceId: string; conversationId: string; kind: "error"; message?: string };

export interface ConversationStreamDriver {
  listen(listener: (envelope: StreamEnvelope) => void): Promise<() => void>;
  start(input: { sessionId: string; workspaceId: string; conversationId: string; resumeCursor: string | null }): Promise<void>;
  acknowledge(sessionId: string, frameId: string): Promise<void>;
  stop(sessionId: string): Promise<void>;
  newSessionId(): string;
}

export type ConversationStreamStatus = "CONNECTING" | "LIVE" | "RECONNECTING";

const RETRY_DELAYS_MS = [500, 1_000, 2_000, 4_000, 8_000, 15_000, 30_000] as const;
const wait = (milliseconds: number) => new Promise<void>(resolve => setTimeout(resolve, milliseconds));

function exactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  return Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
}

function validEnvelope(value: unknown): value is StreamEnvelope {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const envelope = value as Record<string, unknown>;
  if (typeof envelope.sessionId !== "string" || typeof envelope.workspaceId !== "string"
    || typeof envelope.conversationId !== "string") return false;
  if (envelope.kind === "frame") return exactKeys(envelope, ["sessionId", "workspaceId", "conversationId", "kind", "frameId", "frame"])
    && typeof envelope.frameId === "string" && envelope.frameId.length > 0 && envelope.frameId.length <= 128;
  if (envelope.kind === "closed") return exactKeys(envelope, ["sessionId", "workspaceId", "conversationId", "kind"]);
  if (envelope.kind === "error") return exactKeys(envelope, ["sessionId", "workspaceId", "conversationId", "kind", ...(Object.hasOwn(envelope, "message") ? ["message"] : [])])
    && (envelope.message === undefined || typeof envelope.message === "string");
  return false;
}

export class ConversationStreamController {
  private currentState: PresentationStreamState = emptyPresentationStream;
  private currentStatus: ConversationStreamStatus = "CONNECTING";
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
    private readonly conversationId: string,
    private readonly driver: ConversationStreamDriver,
    private readonly onState: (state: PresentationStreamState, frame: unknown) => void,
    private readonly onStatus: (status: ConversationStreamStatus) => void = () => {},
    private readonly delay: (milliseconds: number) => Promise<void> = wait,
  ) {}

  get state(): PresentationStreamState { return this.currentState; }
  get status(): ConversationStreamStatus { return this.currentStatus; }

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

  private setStatus(status: ConversationStreamStatus): void {
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
        // Install the event listener before invoking Tauri: the native stream can
        // deliver stream.ready immediately after the subscribe command returns.
        unlisten = await this.driver.listen(value => {
          if (!this.isCurrent(generation) || !value || typeof value !== "object" || Array.isArray(value)) return;
          const candidate = value as Record<string, unknown>;
          if (candidate.sessionId !== sessionId) return;
          if (candidate.kind === "frame" && typeof candidate.frameId === "string") {
            try {
              if (validEnvelope(value) && value.kind === "frame" && value.workspaceId === this.workspaceId
                && value.conversationId === this.conversationId) {
                const next = reducePresentationFrame(this.currentState, value.frame, {
                  workspaceId: this.workspaceId,
                  conversationId: this.conversationId,
                });
                if (next !== this.currentState) {
                  this.currentState = next;
                  if (next.ready) this.setStatus("LIVE");
                  this.onState(next, value.frame);
                }
              }
            } finally {
              // A discarded but correctly identified native frame still returns
              // its single unit of credit. The native side rejects stale IDs.
              void this.driver.acknowledge(sessionId, candidate.frameId).catch(close);
            }
            return;
          }
          if (!validEnvelope(value) || value.workspaceId !== this.workspaceId
            || value.conversationId !== this.conversationId) return;
          close();
        });
        this.unlisten = unlisten;
        if (!this.isCurrent(generation)) break;
        await this.driver.start({ sessionId, workspaceId: this.workspaceId, conversationId: this.conversationId, resumeCursor: this.currentState.cursor });
        if (!this.isCurrent(generation)) break;
        await closed;
      } catch {
        // The UI reports only that the local history stream is reconnecting. Native
        // transport details are deliberately not exposed to the WebView.
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

export type { ConversationSnapshotView };
