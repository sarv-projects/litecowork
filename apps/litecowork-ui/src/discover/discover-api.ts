// Discover catalog API for the local Runtime.
//
// The catalog is a projection, never authority. Every row carries its own
// availability, install state, authentication state, and source, and the page
// carries the owning provider's truthfulness. A row is never rendered as
// installed, connected, or authorized when the Runtime did not observe that.
//
// Authority: docs/CAPABILITY-FABRIC.md, docs/CONNECTORS-WORKFLOWS.md,
// docs/API.md.

export type DiscoverEntryKind = "APP" | "CAPABILITY" | "SKILL" | "AGENT";
export type DiscoverAvailability =
  | "AVAILABLE"
  | "NEEDS_AUTH"
  | "UNSUPPORTED"
  | "ERROR"
  | "NOT_QUALIFIED";
export type DiscoverCompatibility = "COMPATIBLE" | "INCOMPATIBLE" | "UNKNOWN";
export type DiscoverInstallState = "NOT_INSTALLED" | "INSTALLED" | "INSTALL_PENDING" | "UNKNOWN";
export type DiscoverAuthState = "NOT_REQUIRED" | "REQUIRED" | "AUTHENTICATED" | "UNKNOWN";
export type DiscoverEntrySource = "LITE_SPM_PACKAGE" | "MCP_SERVER" | "LITECOWORK_LOCAL";

export type DiscoverItem = {
  item_ref: string;
  kind: DiscoverEntryKind;
  display_name: string;
  summary: string;
  description: string | null;
  compatibility: DiscoverCompatibility;
  compatibility_reason: string | null;
  requires_user_action: boolean;
  availability: DiscoverAvailability;
  availability_reason: string;
  install_state: DiscoverInstallState;
  auth_state: DiscoverAuthState;
  source: DiscoverEntrySource;
  publisher: string | null;
  version: string | null;
  digest: string | null;
  grants_required: string[];
  /** A catalog row is never an authorization. This is always false. */
  authoritative: false;
};

export type DiscoverProviderStatus = {
  package_service_qualified: boolean;
  reason_code: string;
  message: string;
};

export type DiscoverItemPage = {
  items: DiscoverItem[];
  next_cursor: string | null;
  provider_status: DiscoverProviderStatus;
  runtime_capabilities: DiscoverItem[];
};

export type DiscoverCapabilitySummary = {
  capability_id: string;
  display_name: string;
  operation_class: string;
  required_scopes: string[];
};

export type DiscoverItemDetail = {
  item: DiscoverItem;
  capabilities: DiscoverCapabilitySummary[];
  provider_note: string | null;
};

export type DiscoverSearchTransport = (
  workspaceId: string,
  query: string | null,
  kind: DiscoverEntryKind | null,
  cursor: string | undefined,
  signal?: AbortSignal,
) => Promise<unknown>;
export type DiscoverItemTransport = (workspaceId: string, itemRef: string, signal?: AbortSignal) => Promise<unknown>;

export interface DiscoverApi {
  readonly workspaceId: string;
  search(query?: string | null, kind?: DiscoverEntryKind | null, cursor?: string, signal?: AbortSignal): Promise<DiscoverItemPage>;
  item(itemRef: string, signal?: AbortSignal): Promise<DiscoverItemDetail>;
}

export class DiscoverApiError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DiscoverApiError";
  }
}

type Obj = Record<string, unknown>;
function object(value: unknown): Obj {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new DiscoverApiError("Local Runtime returned an invalid Discover catalog.");
  }
  return value as Obj;
}
function text(value: unknown, empty = false): string {
  if (typeof value !== "string" || (!empty && value.trim().length === 0)) {
    throw new DiscoverApiError("Local Runtime returned an invalid Discover catalog.");
  }
  return value;
}
function nullableText(value: unknown): string | null {
  return value == null ? null : text(value);
}
function oneOf<T extends string>(value: unknown, allowed: readonly T[], label: string): T {
  if (typeof value !== "string" || !(allowed as readonly string[]).includes(value)) {
    throw new DiscoverApiError(`Local Runtime returned an unsupported Discover ${label}.`);
  }
  return value as T;
}
function boolean(value: unknown): boolean {
  if (typeof value !== "boolean") {
    throw new DiscoverApiError("Local Runtime returned an invalid Discover catalog.");
  }
  return value;
}
function stringList(value: unknown): string[] {
  if (!Array.isArray(value)) {
    throw new DiscoverApiError("Local Runtime returned an invalid Discover catalog.");
  }
  return value.map(item => text(item));
}

const KINDS = ["APP", "CAPABILITY", "SKILL", "AGENT"] as const;
const AVAILABILITIES = ["AVAILABLE", "NEEDS_AUTH", "UNSUPPORTED", "ERROR", "NOT_QUALIFIED"] as const;
const COMPATIBILITIES = ["COMPATIBLE", "INCOMPATIBLE", "UNKNOWN"] as const;
const INSTALL_STATES = ["NOT_INSTALLED", "INSTALLED", "INSTALL_PENDING", "UNKNOWN"] as const;
const AUTH_STATES = ["NOT_REQUIRED", "REQUIRED", "AUTHENTICATED", "UNKNOWN"] as const;
const SOURCES = ["LITE_SPM_PACKAGE", "MCP_SERVER", "LITECOWORK_LOCAL"] as const;

export function decodeDiscoverItem(value: unknown): DiscoverItem {
  const row = object(value);
  const availability = oneOf(row.availability, AVAILABILITIES, "availability");
  const compatibility = oneOf(row.compatibility, COMPATIBILITIES, "compatibility");
  const installState = oneOf(row.install_state, INSTALL_STATES, "install state");
  const authState = oneOf(row.auth_state, AUTH_STATES, "authentication state");
  const source = oneOf(row.source, SOURCES, "source");
  const authoritative = boolean(row.authoritative);
  if (authoritative) {
    // A catalog row that claims authority is never rendered as usable.
    throw new DiscoverApiError("Local Runtime returned a Discover row that claims authority.");
  }
  if (availability === "AVAILABLE" && (compatibility === "INCOMPATIBLE" || installState === "UNKNOWN" || authState === "REQUIRED" || authState === "UNKNOWN")) {
    throw new DiscoverApiError("Local Runtime returned an inconsistent Discover availability.");
  }
  if (availability === "AVAILABLE" && authState === "AUTHENTICATED" && installState === "NOT_INSTALLED") {
    throw new DiscoverApiError("Local Runtime returned an inconsistent Discover install state.");
  }
  const digest = nullableText(row.digest);
  if (digest !== null && !/^sha256:[a-f0-9]{64}$/.test(digest)) {
    throw new DiscoverApiError("Local Runtime returned an invalid Discover digest.");
  }
  return {
    item_ref: text(row.item_ref),
    kind: oneOf(row.kind, KINDS, "entry kind"),
    display_name: text(row.display_name),
    summary: text(row.summary, true),
    description: nullableText(row.description),
    compatibility,
    compatibility_reason: nullableText(row.compatibility_reason),
    requires_user_action: boolean(row.requires_user_action),
    availability,
    availability_reason: text(row.availability_reason),
    install_state: installState,
    auth_state: authState,
    source,
    publisher: nullableText(row.publisher),
    version: nullableText(row.version),
    digest,
    grants_required: stringList(row.grants_required),
    authoritative: false,
  };
}

export function decodeDiscoverProviderStatus(value: unknown): DiscoverProviderStatus {
  const row = object(value);
  return {
    package_service_qualified: boolean(row.package_service_qualified),
    reason_code: text(row.reason_code),
    message: text(row.message, true),
  };
}

export function decodeDiscoverItemPage(value: unknown): DiscoverItemPage {
  const page = object(value);
  if (!Array.isArray(page.items) || !Array.isArray(page.runtime_capabilities)) {
    throw new DiscoverApiError("Local Runtime returned an invalid Discover page.");
  }
  const nextCursor = page.next_cursor == null ? null : text(page.next_cursor);
  return {
    items: page.items.map(decodeDiscoverItem),
    next_cursor: nextCursor,
    provider_status: decodeDiscoverProviderStatus(page.provider_status),
    runtime_capabilities: page.runtime_capabilities.map(decodeDiscoverItem),
  };
}

export function decodeDiscoverItemDetail(value: unknown): DiscoverItemDetail {
  const detail = object(value);
  if (!Array.isArray(detail.capabilities)) {
    throw new DiscoverApiError("Local Runtime returned an invalid Discover detail.");
  }
  return {
    item: decodeDiscoverItem(detail.item),
    capabilities: detail.capabilities.map(raw => {
      const summary = object(raw);
      return {
        capability_id: text(summary.capability_id),
        display_name: text(summary.display_name),
        operation_class: text(summary.operation_class),
        required_scopes: stringList(summary.required_scopes),
      };
    }),
    provider_note: nullableText(detail.provider_note),
  };
}

export function createDiscoverApi(
  workspaceId: string,
  searchTransport: DiscoverSearchTransport,
  itemTransport: DiscoverItemTransport,
): DiscoverApi {
  if (!workspaceId.trim()) throw new Error("Select a Workspace before browsing capabilities.");
  return {
    workspaceId,
    async search(query = null, kind = null, cursor, signal) {
      return decodeDiscoverItemPage(await searchTransport(workspaceId, query, kind, cursor, signal));
    },
    async item(itemRef, signal) {
      return decodeDiscoverItemDetail(await itemTransport(workspaceId, itemRef, signal));
    },
  };
}
