export type NativePermissionDecision = "ALLOW_ONCE" | "REJECT_SESSION_PENDING";
export type NativePermissionPromptState = "PENDING" | "REPLYING" | "ANSWERED" | "OUTCOME_UNKNOWN" | "SESSION_ENDED";

export type NativePermissionPrompt = {
  workspaceId: string;
  conversationId: string;
  turnId: string;
  retryOrdinal: number;
  sequence: number;
  promptToken: string;
  state: NativePermissionPromptState;
  action: string;
  explanation: string | null;
  resourceCount: number;
  resources: string[];
  decision: NativePermissionDecision | null;
};

export type NativePermissionScope = Pick<NativePermissionPrompt, "workspaceId" | "conversationId" | "turnId" | "retryOrdinal">;
export type NativePermissionState = {
  lastSequence: number;
  needsRefresh: boolean;
  prompts: Record<string, NativePermissionPrompt>;
};

export type ConversationPermissionSnapshot = NativePermissionScope & {
  sequence: number;
  items: NativePermissionPrompt[];
};

export type WorkspacePermissionItem = NativePermissionPrompt & {
  conversationTitle: string | null;
  attemptSequence: number;
};

export type WorkspacePermissionSnapshot = {
  workspaceId: string;
  items: WorkspacePermissionItem[];
};

export type NativePermissionReplyReceipt = {
  promptToken: string;
  state: NativePermissionPromptState;
  decision: NativePermissionDecision | null;
  sequence: number;
};

export const emptyNativePermissionState: NativePermissionState = {
  lastSequence: 0,
  needsRefresh: false,
  prompts: {},
};

const MAX_SEQUENCE = 4096;
const STATES = new Set<NativePermissionPromptState>(["PENDING", "REPLYING", "ANSWERED", "OUTCOME_UNKNOWN", "SESSION_ENDED"]);
const DECISIONS = new Set<NativePermissionDecision>(["ALLOW_ONCE", "REJECT_SESSION_PENDING"]);

function record(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : null;
}

function exactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  return Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
}

function safeText(value: unknown, maxLength: number): value is string {
  return typeof value === "string" && value.length > 0 && value.length <= maxLength
    && ![...value].some(character => character.charCodeAt(0) < 0x20 || character.charCodeAt(0) === 0x7f);
}

function validId(value: unknown): value is string {
  return safeText(value, 256) && /^[A-Za-z0-9_-]+$/.test(value);
}

function validSequence(value: unknown, allowZero = false): value is number {
  return Number.isSafeInteger(value) && (value as number) >= (allowZero ? 0 : 1) && (value as number) <= MAX_SEQUENCE;
}

function parsePrompt(
  value: unknown,
  scope: NativePermissionScope,
): NativePermissionPrompt | null {
  const prompt = record(value);
  if (!prompt || !exactKeys(prompt, ["sequence", "prompt_token", "state", "action", "explanation", "resource_count", "resources", "decision"])) return null;
  const decision = prompt.decision;
  if (!safeText(prompt.prompt_token, 128) || prompt.prompt_token.length < 32 || !/^[a-fA-F0-9-]+$/.test(prompt.prompt_token)
    || !validSequence(prompt.sequence)
    || typeof prompt.state !== "string" || !STATES.has(prompt.state as NativePermissionPromptState)
    || !safeText(prompt.action, 128)
    || !(prompt.explanation === null || safeText(prompt.explanation, 512))
    || !Number.isSafeInteger(prompt.resource_count) || (prompt.resource_count as number) < 0 || (prompt.resource_count as number) > 64
    || !Array.isArray(prompt.resources) || prompt.resources.length > 16
    || !prompt.resources.every(resource => safeText(resource, 128))
    || !(decision === null || (typeof decision === "string" && DECISIONS.has(decision as NativePermissionDecision)))) return null;
  return {
    ...scope,
    sequence: prompt.sequence as number,
    promptToken: prompt.prompt_token,
    state: prompt.state as NativePermissionPromptState,
    action: prompt.action,
    explanation: prompt.explanation,
    resourceCount: prompt.resource_count as number,
    resources: prompt.resources as string[],
    decision: decision as NativePermissionDecision | null,
  };
}

function parseWorkspacePrompt(value: unknown, workspaceId: string): WorkspacePermissionItem | null {
  const item = record(value);
  if (!item || !exactKeys(item, ["conversation_id", "conversation_title", "turn_id", "retry_ordinal", "sequence", "prompt"])) return null;
  if (!validId(item.conversation_id) || !(item.conversation_title === null || safeText(item.conversation_title, 160))
    || !validId(item.turn_id) || !Number.isSafeInteger(item.retry_ordinal) || (item.retry_ordinal as number) < 0
    || !validSequence(item.sequence, true)) return null;
  const scope: NativePermissionScope = {
    workspaceId,
    conversationId: item.conversation_id,
    turnId: item.turn_id,
    retryOrdinal: item.retry_ordinal as number,
  };
  const prompt = parsePrompt(item.prompt, scope);
  if (!prompt || prompt.sequence > (item.sequence as number)) return null;
  return { ...prompt, conversationTitle: item.conversation_title as string | null, attemptSequence: item.sequence as number };
}

export function parseConversationPermissionSnapshot(value: unknown, expected: NativePermissionScope): ConversationPermissionSnapshot | null {
  const snapshot = record(value);
  if (!snapshot || !exactKeys(snapshot, ["workspace_id", "conversation_id", "turn_id", "retry_ordinal", "sequence", "items"])
    || snapshot.workspace_id !== expected.workspaceId || snapshot.conversation_id !== expected.conversationId
    || snapshot.turn_id !== expected.turnId || snapshot.retry_ordinal !== expected.retryOrdinal
    || !validSequence(snapshot.sequence, true) || !Array.isArray(snapshot.items) || snapshot.items.length > 32) return null;
  const items = snapshot.items.map(item => parsePrompt(item, expected));
  if (items.some(item => item === null) || new Set(items.map(item => item!.promptToken)).size !== items.length
    || items.some(item => item!.sequence > (snapshot.sequence as number))) return null;
  return { ...expected, sequence: snapshot.sequence as number, items: items as NativePermissionPrompt[] };
}

export function parseWorkspacePermissionSnapshot(value: unknown, expectedWorkspaceId: string): WorkspacePermissionSnapshot | null {
  const snapshot = record(value);
  if (!snapshot || !exactKeys(snapshot, ["workspace_id", "items"]) || snapshot.workspace_id !== expectedWorkspaceId
    || !Array.isArray(snapshot.items) || snapshot.items.length > 256) return null;
  const items = snapshot.items.map(item => parseWorkspacePrompt(item, expectedWorkspaceId));
  if (items.some(item => item === null) || new Set(items.map(item => item!.promptToken)).size !== items.length) return null;
  return { workspaceId: expectedWorkspaceId, items: items as WorkspacePermissionItem[] };
}

export function reduceNativePermissionFrame(
  state: NativePermissionState,
  value: unknown,
  expected: NativePermissionScope,
): NativePermissionState {
  const frame = record(value);
  if (!frame || !exactKeys(frame, ["type", "workspace_id", "conversation_id", "turn_id", "retry_ordinal", "sequence", "prompt_token", "state", "action", "explanation", "resource_count", "resources", "decision"])
    || frame.type !== "turn.permission" || frame.workspace_id !== expected.workspaceId
    || frame.conversation_id !== expected.conversationId || frame.turn_id !== expected.turnId || frame.retry_ordinal !== expected.retryOrdinal) return state;
  const prompt = parsePrompt({
    sequence: frame.sequence,
    prompt_token: frame.prompt_token,
    state: frame.state,
    action: frame.action,
    explanation: frame.explanation,
    resource_count: frame.resource_count,
    resources: frame.resources,
    decision: frame.decision,
  }, expected);
  if (!prompt || prompt.sequence <= state.lastSequence) return state;
  if (prompt.sequence !== state.lastSequence + 1) return { ...state, needsRefresh: true };
  return {
    lastSequence: prompt.sequence,
    needsRefresh: state.needsRefresh,
    prompts: { ...state.prompts, [prompt.promptToken]: prompt },
  };
}

export function reconcileConversationPermissionSnapshot(
  state: NativePermissionState,
  snapshot: ConversationPermissionSnapshot,
): NativePermissionState {
  if (snapshot.sequence < state.lastSequence) return state;
  return {
    lastSequence: snapshot.sequence,
    needsRefresh: false,
    prompts: Object.fromEntries(snapshot.items.map(prompt => [prompt.promptToken, prompt])),
  };
}

export function requireNativePermissionSnapshot(state: NativePermissionState): NativePermissionState {
  return state.needsRefresh ? state : { ...state, needsRefresh: true };
}

export function applyNativePermissionReceipt(state: NativePermissionState, value: unknown): NativePermissionState {
  const receipt = record(value);
  if (!receipt || !exactKeys(receipt, ["prompt_token", "state", "decision", "sequence"])
    || !safeText(receipt.prompt_token, 128) || receipt.prompt_token.length < 32
    || typeof receipt.state !== "string" || !STATES.has(receipt.state as NativePermissionPromptState)
    || !(receipt.decision === null || (typeof receipt.decision === "string" && DECISIONS.has(receipt.decision as NativePermissionDecision)))
    || !validSequence(receipt.sequence)) return state;
  const current = state.prompts[receipt.prompt_token];
  if (!current || (receipt.sequence as number) <= state.lastSequence) return state;
  if ((receipt.sequence as number) !== state.lastSequence + 1) return { ...state, needsRefresh: true };
  return {
    lastSequence: receipt.sequence as number,
    needsRefresh: state.needsRefresh,
    prompts: {
      ...state.prompts,
      [receipt.prompt_token]: {
        ...current,
        sequence: receipt.sequence as number,
        state: receipt.state as NativePermissionPromptState,
        decision: receipt.decision as NativePermissionDecision | null,
      },
    },
  };
}

export function markNativePermissionOutcomeUnknown(
  state: NativePermissionState,
  promptToken: string,
  decision: NativePermissionDecision,
): NativePermissionState {
  const current = state.prompts[promptToken];
  if (!current || current.state === "ANSWERED" || current.state === "SESSION_ENDED") return state;
  return {
    ...state,
    needsRefresh: true,
    prompts: {
      ...state.prompts,
      [promptToken]: { ...current, state: "OUTCOME_UNKNOWN", decision },
    },
  };
}

export function endLiveNativePermissions(state: NativePermissionState): NativePermissionState {
  let changed = false;
  const prompts = Object.fromEntries(Object.entries(state.prompts).map(([token, prompt]) => {
    if (prompt.state !== "PENDING" && prompt.state !== "REPLYING" && prompt.state !== "OUTCOME_UNKNOWN") return [token, prompt];
    changed = true;
    return [token, { ...prompt, state: "SESSION_ENDED" as const }];
  }));
  return changed ? { ...state, prompts } : state;
}
