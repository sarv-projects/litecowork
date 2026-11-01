export type OfferExpiry = { offerExpiresAt: string };

export function scheduleOfferExpiryRefresh(
  offers: readonly OfferExpiry[],
  onExpiry: () => void,
  nowMs = Date.now(),
  schedule: typeof setTimeout = setTimeout,
  cancelTimer: typeof clearTimeout = clearTimeout,
): () => void {
  const nextExpiry = offers
    .map(offer => Date.parse(offer.offerExpiresAt))
    .filter(expiry => Number.isFinite(expiry) && expiry > nowMs)
    .reduce<number | null>((earliest, expiry) => earliest === null || expiry < earliest ? expiry : earliest, null);
  if (nextExpiry === null) return () => {};
  const timer = schedule(onExpiry, Math.min(nextExpiry - nowMs, 2_147_483_647));
  return () => cancelTimer(timer);
}
