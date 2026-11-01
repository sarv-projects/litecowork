export type QuotaObservationState = "NORMAL" | "LOW" | "EXHAUSTED" | "UNKNOWN";

export type UsageQuantity = {
  quantity: number;
  unit: string;
  currency?: string | null;
  confidence: "EXACT" | "ESTIMATED";
};

export type QuotaObservation = {
  agent_binding_id: string;
  source: string;
  observed_at: string;
  expires_at: string | null;
  state: QuotaObservationState;
  remaining_hint: UsageQuantity | null;
  reset_at: string | null;
};

export type AgentQuotaObservationApi = {
  getQuotaObservation(agentBindingId: string, signal?: AbortSignal): Promise<QuotaObservation | null>;
};

function object(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`Invalid ${label}.`);
  return value as Record<string, unknown>;
}

function boundedText(value: unknown, label: string, maximum: number): string {
  if (typeof value !== "string" || value.trim().length === 0 || value.length > maximum
    || /[\u0000-\u001f\u007f]/.test(value)) throw new Error(`Invalid ${label}.`);
  return value;
}

function bindingIdentifier(value: unknown): string {
  const identifier = boundedText(value, "agent binding ID", 256);
  if (identifier === "." || identifier === ".." || /[\\/]/.test(identifier)) {
    throw new Error("Invalid agent binding identifier.");
  }
  return identifier;
}

function timestamp(value: unknown, label: string): string {
  const text = boundedText(value, label, 64);
  if (!/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?(?:Z|[+-]\d\d:\d\d)$/.test(text)
    || !Number.isFinite(Date.parse(text))) throw new Error(`Invalid ${label} timestamp.`);
  return text;
}

function nullableTimestamp(value: unknown, label: string): string | null {
  return value === null ? null : timestamp(value, label);
}

export function decodeQuotaObservation(value: unknown, expectedBindingId: string): QuotaObservation | null {
  if (value === null) return null;
  const row = object(value, "quota observation");
  const allowed = new Set(["agent_binding_id", "source", "observed_at", "expires_at", "state", "remaining_hint", "reset_at"]);
  if (Object.keys(row).some(key => !allowed.has(key))) throw new Error("Invalid quota observation fields.");
  const bindingId = boundedText(row.agent_binding_id, "agent binding ID", 256);
  if (bindingId !== expectedBindingId) throw new Error("Quota observation belongs to a different agent binding.");
  const state = row.state;
  if (state !== "NORMAL" && state !== "LOW" && state !== "EXHAUSTED" && state !== "UNKNOWN") {
    throw new Error("Invalid quota observation state.");
  }
  let remainingHint: QuotaObservation["remaining_hint"] = null;
  if (row.remaining_hint !== null) {
    const quantity = object(row.remaining_hint, "remaining usage hint");
    if (Object.keys(quantity).some(key => !["quantity", "unit", "currency", "confidence"].includes(key))) {
      throw new Error("Invalid remaining usage hint fields.");
    }
    if (typeof quantity.quantity !== "number" || !Number.isFinite(quantity.quantity) || quantity.quantity < 0) {
      throw new Error("Invalid remaining usage quantity.");
    }
    const currency = quantity.currency;
    if (currency !== null && currency !== undefined && (typeof currency !== "string" || !/^[A-Z]{3}$/.test(currency))) {
      throw new Error("Invalid remaining usage currency.");
    }
    if (quantity.confidence !== "EXACT" && quantity.confidence !== "ESTIMATED") {
      throw new Error("Invalid remaining usage confidence.");
    }
    remainingHint = {
      quantity: quantity.quantity,
      unit: boundedText(quantity.unit, "remaining usage unit", 64),
      currency: currency ?? null,
      confidence: quantity.confidence,
    };
  }
  return {
    agent_binding_id: bindingId,
    source: boundedText(row.source, "quota observation source", 256),
    observed_at: timestamp(row.observed_at, "observation time"),
    expires_at: nullableTimestamp(row.expires_at, "expiry time"),
    state,
    remaining_hint: remainingHint,
    reset_at: nullableTimestamp(row.reset_at, "reset time"),
  };
}

export function isQuotaObservationExpired(observation: QuotaObservation, now = Date.now()): boolean {
  return observation.expires_at !== null && Date.parse(observation.expires_at) <= now;
}

export function createAgentQuotaObservationApi(
  load: (agentBindingId: string, signal?: AbortSignal) => Promise<unknown>,
): AgentQuotaObservationApi {
  return {
    async getQuotaObservation(agentBindingId, signal) {
      bindingIdentifier(agentBindingId);
      if (signal?.aborted) throw new DOMException("Aborted", "AbortError");
      const result = decodeQuotaObservation(await load(agentBindingId, signal), agentBindingId);
      if (signal?.aborted) throw new DOMException("Aborted", "AbortError");
      return result;
    },
  };
}
