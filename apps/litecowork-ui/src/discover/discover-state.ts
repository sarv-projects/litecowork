// The user-visible projection of one Discover catalog row.
//
// A catalog row is never authority. This module turns the Runtime's observed
// availability into exactly one truthful label and one disabled decision, and
// it never upgrades an unobserved row into an "installed" or "connected" one.
//
// Authority: docs/CAPABILITY-FABRIC.md, docs/V1-UI-ACTION-STATE-LEDGER.md.

import type { DiscoverAvailability, DiscoverItem, DiscoverProviderStatus } from "./discover-api.ts";

export type DiscoverRowTone = "ready" | "needs-action" | "unavailable" | "not-qualified";

export type DiscoverRowView = {
  itemRef: string;
  kind: DiscoverItem["kind"];
  title: string;
  summary: string;
  /** The single truthful availability label. */
  availabilityLabel: string;
  /** The stable reason code behind the label. */
  availabilityReason: string;
  tone: DiscoverRowTone;
  /** True when the owner must act before the row becomes usable. */
  requiresUserAction: boolean;
  /** True when the UI must not offer the row as an action. */
  disabled: boolean;
  /** The install state as observed, never as assumed. */
  installState: DiscoverItem["install_state"];
  /** The authentication state as observed, never as assumed. */
  authState: DiscoverItem["auth_state"];
  /** Names of the grants the row would need. Names only. */
  grantsRequired: string[];
  /** The owning source. A catalog row is never authority. */
  source: DiscoverItem["source"];
};

const AVAILABILITY_LABELS: Record<DiscoverAvailability, string> = {
  AVAILABLE: "Available",
  NEEDS_AUTH: "Sign in required",
  UNSUPPORTED: "Not supported here",
  ERROR: "State unknown",
  NOT_QUALIFIED: "Package service not qualified",
};

function tone(availability: DiscoverAvailability): DiscoverRowTone {
  switch (availability) {
    case "AVAILABLE":
      return "ready";
    case "NEEDS_AUTH":
      return "needs-action";
    case "NOT_QUALIFIED":
      return "not-qualified";
    case "UNSUPPORTED":
    case "ERROR":
      return "unavailable";
  }
}

export function discoverRowView(item: DiscoverItem): DiscoverRowView {
  const availabilityLabel = AVAILABILITY_LABELS[item.availability];
  const disabled = item.availability !== "AVAILABLE";
  return {
    itemRef: item.item_ref,
    kind: item.kind,
    title: item.display_name,
    summary: item.summary,
    availabilityLabel,
    availabilityReason: item.availability_reason,
    tone: tone(item.availability),
    requiresUserAction: item.availability === "NEEDS_AUTH",
    disabled,
    installState: item.install_state,
    authState: item.auth_state,
    grantsRequired: [...item.grants_required],
    source: item.source,
  };
}

export type DiscoverCatalogView = {
  rows: DiscoverRowView[];
  /** The Runtime's observed capability hosts. Observations, not catalog rows. */
  observationRows: DiscoverRowView[];
  /** The provider's truthful state, shown once and plainly. */
  providerMessage: string;
  packageServiceQualified: boolean;
  /** True when the catalog has nothing to offer and says so. */
  empty: boolean;
};

export function discoverCatalogView(
  page: { items: DiscoverItem[]; runtime_capabilities: DiscoverItem[]; provider_status: DiscoverProviderStatus },
): DiscoverCatalogView {
  const rows = page.items.map(discoverRowView);
  const observationRows = page.runtime_capabilities.map(discoverRowView);
  return {
    rows,
    observationRows,
    providerMessage: page.provider_status.message,
    packageServiceQualified: page.provider_status.package_service_qualified,
    empty: rows.length === 0 && observationRows.length === 0,
  };
}

/**
 * The message a row shows when the owner presses the row's action.
 *
 * The LiteSPM package service owns installation and its wire contract is not
 * published, so the action reports that truthfully instead of pretending the
 * package was installed.
 */
export function connectActionOutcome(row: DiscoverRowView): { allowed: boolean; message: string } {
  if (row.disabled) {
    return { allowed: false, message: `${row.availabilityLabel} (${row.availabilityReason})` };
  }
  return {
    allowed: false,
    message: "Package installation is unavailable until the LiteSPM package contract is published and qualified.",
  };
}
