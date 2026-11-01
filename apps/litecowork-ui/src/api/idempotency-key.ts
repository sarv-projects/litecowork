/**
 * The published `Idempotency-Key` grammar: 1-200 visible ASCII bytes.
 *
 * Every mutating Operator surface must accept exactly the same set. A surface
 * with a narrower rule would reject a key another surface accepted, and a wider
 * one would forward a key the daemon rejects.
 */
export function validIdempotencyKey(value: string): boolean {
  return /^[\x21-\x7e]{1,200}$/.test(value);
}
