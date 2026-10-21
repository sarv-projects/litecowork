/**
 * One seam for the authenticated native Operator bridge.
 *
 * Every desktop transport receives the same envelope from its Tauri command:
 * an HTTP status, a content type, and a base64 body. Before this module each
 * surface re-declared that shape, re-implemented base64 decoding, re-validated
 * `Idempotency-Key`, and rebuilt the `Response` by hand. Those copies drifted:
 * one surface enforced a response size limit and the others silently did not.
 *
 * Transports now decode through `operatorBridgeResponse`, so an abort check, a
 * size bound, and the response shape are decided once. The bridge still carries
 * no authority of its own: the Rust command remains the authenticated boundary.
 */

/** Exact reply shape of every Operator bridge command. */
export type OperatorBridgeResponse = {
  status: number;
  contentType: string;
  bodyBase64: string;
};

/**
 * Decode a bridge body.
 *
 * `maxEncodedLength` bounds the encoded payload so a compromised or confused
 * bridge cannot force an unbounded allocation. Omitted means the caller's
 * contract sets no explicit response budget.
 */
export function decodeBase64(encoded: string, maxEncodedLength?: number): Uint8Array {
  if (maxEncodedLength !== undefined && encoded.length > maxEncodedLength) {
    throw new Error("Operator bridge response exceeded its size limit.");
  }
  const binary = atob(encoded);
  return Uint8Array.from(binary, character => character.charCodeAt(0));
}

/**
 * Turn a bridge reply into the `Response` a transport must return.
 *
 * Re-checking `signal` after the await closes the race where the caller aborted
 * while the command was in flight.
 */
export function operatorBridgeResponse(
  response: OperatorBridgeResponse,
  options: { signal?: AbortSignal | null; maxEncodedLength?: number } = {},
): Response {
  if (options.signal?.aborted) throw new DOMException("Aborted", "AbortError");
  return new Response(decodeBase64(response.bodyBase64, options.maxEncodedLength), {
    status: response.status,
    headers: { "Content-Type": response.contentType },
  });
}
