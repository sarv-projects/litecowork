import type { components } from "../api/operator-api";

export type AgentControlDescriptor = components["schemas"]["AgentControlDescriptor"];

export type AgentControlReadError = Error & {
  code?: string;
  retryable?: boolean;
  reference?: string;
};

export interface AgentControlDescriptorApi {
  get(workspaceId: string, profileId: string): Promise<AgentControlDescriptor>;
  refresh(workspaceId: string, profileId: string): Promise<AgentControlDescriptor>;
}

const validId = (value: unknown): value is string =>
  typeof value === "string" && value.trim().length > 0 && value.length <= 256 && !/[\u0000-\u001f\u007f]/.test(value);

const validDigest = (value: unknown): value is string =>
  typeof value === "string" && /^sha256:[a-f0-9]{64}$/.test(value);

const validTimestamp = (value: unknown): value is string =>
  typeof value === "string" && value.length <= 64 && Number.isFinite(Date.parse(value));

const isRecord = (value: unknown): value is Record<string, unknown> =>
  Boolean(value) && typeof value === "object" && !Array.isArray(value);

const isBoundedArray = (value: unknown, max: number): value is unknown[] =>
  Array.isArray(value) && value.length <= max;

const validCommands = (value: unknown): boolean => isBoundedArray(value, 256) && value.every((item) =>
  isRecord(item) && validId(item.command_id) && validId(item.token) && validId(item.display_name)
  && (item.description == null || typeof item.description === "string"));

const validReferenceKinds = (value: unknown): boolean => isBoundedArray(value, 128) && value.every((item) =>
  isRecord(item) && validId(item.reference_kind_id) && validId(item.trigger) && validId(item.display_name)
  && ["NATIVE_PICKER", "HOST_RESOURCE_PICKER", "TEXT_COMPLETION"].includes(String(item.selection_mode)));

function hasOnlyLocalSchemaReferences(value: unknown): boolean {
  const pending: Array<{ value: unknown; depth: number }> = [{ value, depth: 0 }];
  let visited = 0;
  while (pending.length) {
    const current = pending.pop()!;
    visited += 1;
    if (visited > 4096 || current.depth > 32) return false;
    if (Array.isArray(current.value)) {
      if (current.value.length > 256) return false;
      for (const item of current.value) pending.push({ value: item, depth: current.depth + 1 });
      continue;
    }
    if (!isRecord(current.value)) continue;
    const entries = Object.entries(current.value);
    if (entries.length > 128) return false;
    for (const [key, nested] of entries) {
      if (key === "$id") return false;
      if (["$ref", "$dynamicRef", "$recursiveRef"].includes(key)) {
        if (typeof nested !== "string" || !nested.startsWith("#")) return false;
      } else {
        pending.push({ value: nested, depth: current.depth + 1 });
      }
    }
  }
  return true;
}

function validSessionOptions(value: unknown): boolean {
  if (!isRecord(value) || !validDigest(value.descriptor_digest) || !isBoundedArray(value.options, 64)) return false;
  const ids = new Set<string>();
  return value.options.every(option => {
    if (!isRecord(option) || !validId(option.option_id) || !validId(option.display_name)
      || !isRecord(option.value_schema) || !["LIVE_SESSION", "NEW_SESSION", "UNSUPPORTED"].includes(String(option.mutable_scope))
      || (option.semantic_hint != null && !["MODEL", "REASONING", "EFFORT", "MODE", "OTHER"].includes(String(option.semantic_hint)))
      || JSON.stringify(option.value_schema).length > 16 * 1024
      || !hasOnlyLocalSchemaReferences(option.value_schema)
      || ids.has(option.option_id)) return false;
    ids.add(option.option_id);
    return true;
  });
}

export function parseAgentControlDescriptor(value: unknown): AgentControlDescriptor {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Agent control descriptor has an invalid shape");
  }
  const row = value as Record<string, unknown>;
  const installation = row.installation;
  const auth = row.auth;
  const configuration = row.configuration;
  const input = row.input_surface;
  const native = row.native_surface;
  const capabilities = row.capabilities;
  if (
    !validDigest(row.descriptor_digest) || !Number.isInteger(row.descriptor_api_version) ||
    !validId(row.registry_agent_id) || !validId(row.module_id) || !validId(row.module_version) ||
    !isRecord(installation) || !["MISSING", "INSTALLING", "INSTALLED", "UPDATE_AVAILABLE", "UPDATING", "VERSION_UNAVAILABLE", "BROKEN"].includes(String(installation.state)) ||
    !isRecord(auth) || !["UNKNOWN", "NEEDS_AUTH", "AUTHENTICATING", "AUTHENTICATED", "EXPIRED", "DEGRADED", "ERROR"].includes(String(auth.state)) ||
    !isBoundedArray(auth.methods, 32) || !isBoundedArray(auth.credential_slots, 32) ||
    !isRecord(configuration) || !validDigest(configuration.schema_digest) ||
    !isBoundedArray(configuration.fields, 128) || !isBoundedArray(configuration.native_config_targets, 64) ||
    !isRecord(input) || typeof input.text !== "boolean" || typeof input.images !== "boolean" ||
    typeof input.files !== "boolean" || typeof input.resources !== "boolean" ||
    typeof input.supports_command_discovery !== "boolean" ||
    typeof input.supports_reference_discovery !== "boolean" ||
    !validCommands(input.slash_commands) || !validReferenceKinds(input.reference_kinds) ||
    !isRecord(native) || typeof native.native_config_preserved !== "boolean" ||
    typeof native.native_extensions_preserved !== "boolean" || typeof native.native_subagents_preserved !== "boolean" ||
    !isRecord(capabilities) ||
    !validTimestamp(row.observed_at) || !validTimestamp(row.expires_at) ||
    Date.parse(row.expires_at as string) <= Date.parse(row.observed_at as string) ||
    (row.session_options != null && !validSessionOptions(row.session_options))
  ) {
    throw new Error("Agent control descriptor is incomplete or outside its bounds");
  }
  return value as AgentControlDescriptor;
}

export function parseAgentControlReadError(error: unknown): AgentControlReadError {
  const message = error instanceof Error ? error.message : String(error);
  const code = message.match(/\[([A-Z][A-Z0-9_]{0,63}); retryable=(true|false); reference=([^\]\s]{1,128})\]/);
  const parsed = new Error(message) as AgentControlReadError;
  if (code) {
    parsed.code = code[1];
    parsed.retryable = code[2] === "true";
    parsed.reference = code[3];
  }
  return parsed;
}
