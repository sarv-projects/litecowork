export type ResourceIndexRebuildPin = {
  workspaceId: string;
  resourceId: string;
  resourceRevisionId: string;
  contentDigest: string;
};

export type ResourceIndexRebuildReceipt = ResourceIndexRebuildPin & {
  requestId: string;
  correlationId: string;
  outcome: "INDEXED" | "NOT_INDEXABLE";
  reason: "UNSUPPORTED_TYPE" | "OVER_SIZE_LIMIT" | "INVALID_UTF8" | "CONTROL_CHARACTERS" | "TERM_LIMIT_EXCEEDED" | null;
};

export type ResourceIndexRebuildOutcome =
  | { kind: "confirmed"; receipt: ResourceIndexRebuildReceipt; key: string }
  | { kind: "failed"; message: string; retryPin: ResourceIndexRebuildPin | null; key: string };

export function resourceIndexRebuildKey(pin: ResourceIndexRebuildPin): string {
  return JSON.stringify([pin.workspaceId, pin.resourceId, pin.resourceRevisionId, pin.contentDigest]);
}

export function isResourceIndexRebuildReceipt(
  result: ResourceIndexRebuildReceipt,
  pin: ResourceIndexRebuildPin,
  requestId: string,
): boolean {
  if (!result || typeof result !== "object") return false;
  const reasonIsValid = result.reason === null
    || ["UNSUPPORTED_TYPE", "OVER_SIZE_LIMIT", "INVALID_UTF8", "CONTROL_CHARACTERS", "TERM_LIMIT_EXCEEDED"].includes(result.reason);
  return result.workspaceId === pin.workspaceId
    && result.resourceId === pin.resourceId
    && result.resourceRevisionId === pin.resourceRevisionId
    && result.contentDigest === pin.contentDigest
    && result.requestId === requestId
    && typeof result.correlationId === "string"
    && result.correlationId.length > 0
    && result.correlationId.length <= 128
    && /^[\x21-\x7e]+$/.test(result.correlationId)
    && (result.outcome === "INDEXED" || result.outcome === "NOT_INDEXABLE")
    && reasonIsValid
    && (result.outcome === "INDEXED" ? result.reason === null : result.reason !== null);
}

function errorMessage(error: unknown): string {
  return typeof error === "string" ? error : error instanceof Error ? error.message : "The local text index could not be rebuilt.";
}

export async function performResourceIndexRebuild(input: {
  requests: Map<string, string>;
  pin: ResourceIndexRebuildPin;
  createRequestId: () => string;
  execute: (requestId: string, pin: ResourceIndexRebuildPin) => Promise<ResourceIndexRebuildReceipt>;
}): Promise<ResourceIndexRebuildOutcome> {
  const key = resourceIndexRebuildKey(input.pin);
  const requestId = input.requests.get(key) ?? input.createRequestId();
  input.requests.set(key, requestId);
  try {
    const receipt = await input.execute(requestId, input.pin);
    if (!isResourceIndexRebuildReceipt(receipt, input.pin, requestId)) {
      throw new Error("The local Runtime returned a result for a different Resource revision.");
    }
    input.requests.delete(key);
    return { kind: "confirmed", receipt, key };
  } catch (error) {
    const message = errorMessage(error);
    if (message.includes("Reload the Library")) {
      input.requests.delete(key);
      return { kind: "failed", message, retryPin: null, key };
    }
    return { kind: "failed", message, retryPin: input.pin, key };
  }
}
