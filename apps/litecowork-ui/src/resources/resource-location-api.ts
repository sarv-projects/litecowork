import { invoke } from "@tauri-apps/api/core";
import type { paths } from "../api/operator-api";

export type ResourceLocationView = paths["/resources/{resourceId}/locations"]["get"]["responses"]["200"]["content"]["application/json"][number];

export type ResourceLocationLoadState = {
  workspaceId: string;
  resourceId: string;
  busy: boolean;
  error: string | null;
  locations: ResourceLocationView[] | null;
};

export function resourceLocationStateForSelection(
  state: ResourceLocationLoadState | null,
  workspaceId: string,
  resourceId: string,
): ResourceLocationLoadState | null {
  return state?.workspaceId === workspaceId && state.resourceId === resourceId ? state : null;
}

type NativeCommand = (command: string, args: Record<string, string>) => Promise<unknown>;

const availabilityValues = new Set(["AVAILABLE", "OFFLINE", "PLACEHOLDER", "REVOKED", "UNKNOWN", "UNAVAILABLE"]);
const freshnessValues = new Set(["CURRENT", "STALE", "UNKNOWN", "UNAVAILABLE"]);
const publicFields = new Set([
  "location_id", "resource_id", "runtime_id", "environment_id", "connection_id",
  "availability", "writable", "observed_revision_id", "observed_digest", "observed_at",
  "last_checked_at", "freshness",
]);

function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Local Runtime returned invalid Resource locations.");
  }
  return value as Record<string, unknown>;
}

function requiredText(value: unknown): value is string {
  return typeof value === "string" && value.length > 0;
}

function optionalText(value: unknown): boolean {
  return value === undefined || value === null || requiredText(value);
}

export function decodeResourceLocations(value: unknown, expectedResourceId: string): ResourceLocationView[] {
  if (!Array.isArray(value)) throw new Error("Local Runtime returned invalid Resource locations.");
  return value.map((candidate): ResourceLocationView => {
    const row = record(candidate);
    if (Object.keys(row).some((key) => !publicFields.has(key))) {
      throw new Error("Local Runtime returned unsupported Resource location fields.");
    }
    if (!requiredText(row.location_id) || row.resource_id !== expectedResourceId) {
      throw new Error("Local Runtime returned a different Resource location.");
    }
    if (typeof row.availability !== "string" || !availabilityValues.has(row.availability)) {
      throw new Error("Local Runtime returned unsupported Resource location availability.");
    }
    if (typeof row.freshness !== "string" || !freshnessValues.has(row.freshness)) {
      throw new Error("Local Runtime returned unsupported Resource location freshness.");
    }
    if (typeof row.writable !== "boolean" || !requiredText(row.observed_at)
      || !optionalText(row.runtime_id) || !optionalText(row.environment_id)
      || !optionalText(row.connection_id) || !optionalText(row.observed_revision_id)
      || !optionalText(row.last_checked_at)
      || (row.observed_digest != null && (typeof row.observed_digest !== "string"
        || !/^sha256:[a-f0-9]{64}$/.test(row.observed_digest)))) {
      throw new Error("Local Runtime returned malformed Resource location data.");
    }
    return row as ResourceLocationView;
  });
}

export function createResourceLocationApi(nativeCommand: NativeCommand) {
  return {
    async list(workspaceId: string, resourceId: string): Promise<ResourceLocationView[]> {
      if (typeof workspaceId !== "string" || workspaceId.trim().length === 0
        || typeof resourceId !== "string" || resourceId.length === 0 || resourceId.length > 160
        || !/^[A-Za-z0-9_-]+$/.test(resourceId)) {
        throw new Error("Resource location selection is invalid.");
      }
      const response = await nativeCommand("list_resource_locations", { workspaceId, resourceId });
      return decodeResourceLocations(response, resourceId);
    },
  };
}

export const resourceLocationApi = createResourceLocationApi((command, args) => invoke<unknown>(command, args));
