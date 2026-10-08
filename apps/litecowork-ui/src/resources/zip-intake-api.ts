export type ZipIntakeReadiness = {
  capability_id: "litecowork.zip-intake";
  status: "UNAVAILABLE";
  resource_behavior: "OPAQUE_RESOURCE_ONLY";
  provider_integrated: false;
  extraction_enabled: false;
  reason_code: "ISOLATED_WORKER_NOT_QUALIFIED";
  reason: string;
};

export function decodeZipIntakeReadiness(value: unknown): ZipIntakeReadiness {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Local Runtime returned an invalid ZIP-intake status.");
  }
  const row = value as Record<string, unknown>;
  if (row.capability_id !== "litecowork.zip-intake"
    || row.status !== "UNAVAILABLE"
    || row.resource_behavior !== "OPAQUE_RESOURCE_ONLY"
    || row.provider_integrated !== false
    || row.extraction_enabled !== false
    || row.reason_code !== "ISOLATED_WORKER_NOT_QUALIFIED"
    || typeof row.reason !== "string") {
    throw new Error("Local Runtime returned an unsupported ZIP-intake status.");
  }
  return row as ZipIntakeReadiness;
}
