/**
 * Local model inventory rules for the desktop UI.
 *
 * A local model is inventory only. There is no global Providers feature: a local model
 * becomes selectable only through an Agent, and only when that Agent's AgentModule
 * reports compatibility for the exact model. The UI never presents an engine that is
 * merely present on the machine as a connected provider.
 */

export type LocalModelState =
  | "INVENTORY_ONLY"
  | "AGENT_COMPATIBLE"
  | "AGENT_INCOMPATIBLE"
  | "UNAVAILABLE";

export type LocalModelInventoryEntry = {
  modelId: string;
  engineId: string;
  displayName: string;
  state: LocalModelState;
  contextWindowTokens: number | null;
  agentModuleId: string | null;
  reasonCode: string | null;
};

export type LocalModelAvailability = {
  capabilityId: string;
  status: string;
  reasonCode: string;
  reason: string;
  inventoryOnly: boolean;
  surface: string;
  agentBindingPolicy: string;
  models: readonly LocalModelInventoryEntry[];
};

export type AgentModuleCompatibility = {
  agentModuleId: string;
  modelId: string;
  compatible: boolean;
  reasonCode: string | null;
};

const MODEL_STATES: LocalModelState[] = [
  "INVENTORY_ONLY",
  "AGENT_COMPATIBLE",
  "AGENT_INCOMPATIBLE",
  "UNAVAILABLE",
];

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function optionalIdentifier(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 && value.length <= 256 ? value : null;
}

export function decodeLocalModelAvailability(value: unknown): LocalModelAvailability {
  if (!isRecord(value)) {
    throw new Error("The local Runtime returned an invalid local model inventory.");
  }
  if (value.capabilityId !== "litecowork.local-models") {
    throw new Error("The local Runtime returned an unsupported local model capability.");
  }
  if (value.inventoryOnly !== true) {
    throw new Error("The local Runtime presented a local model provider that is not inventory only.");
  }
  if (value.surface !== "AGENT_SCOPED") {
    throw new Error("The local Runtime presented local models outside an Agent surface.");
  }
  if (!Array.isArray(value.models)) {
    throw new Error("The local Runtime returned an invalid local model inventory.");
  }
  const models = value.models.map((entry) => {
    if (!isRecord(entry)) {
      throw new Error("The local Runtime returned an invalid local model entry.");
    }
    if (typeof entry.modelId !== "string" || entry.modelId.trim().length === 0) {
      throw new Error("The local Runtime returned a local model without an identity.");
    }
    if (typeof entry.state !== "string" || !(MODEL_STATES as string[]).includes(entry.state)) {
      throw new Error("The local Runtime returned an unsupported local model state.");
    }
    return {
      modelId: entry.modelId,
      engineId: typeof entry.engineId === "string" ? entry.engineId : "",
      displayName: typeof entry.displayName === "string" ? entry.displayName : entry.modelId,
      state: entry.state as LocalModelState,
      contextWindowTokens: typeof entry.contextWindowTokens === "number"
        ? entry.contextWindowTokens
        : null,
      agentModuleId: optionalIdentifier(entry.agentModuleId),
      reasonCode: optionalIdentifier(entry.reasonCode),
    };
  });
  return {
    capabilityId: value.capabilityId,
    status: typeof value.status === "string" ? value.status : "UNKNOWN",
    reasonCode: typeof value.reasonCode === "string" ? value.reasonCode : "UNKNOWN",
    reason: typeof value.reason === "string" ? value.reason : "",
    inventoryOnly: true,
    surface: value.surface,
    agentBindingPolicy: typeof value.agentBindingPolicy === "string"
      ? value.agentBindingPolicy
      : "AGENT_MODULE_COMPATIBILITY_REQUIRED",
    models,
  };
}

/**
 * A local model may be offered to an Agent only when that Agent's AgentModule reports
 * compatibility for the exact model. The inventory alone never authorises a model.
 */
export function localModelIsUsableByAgent(
  availability: LocalModelAvailability,
  agentModule: AgentModuleCompatibility | null,
  modelId: string,
): boolean {
  if (!availability.inventoryOnly) return false;
  if (!agentModule) return false;
  if (agentModule.modelId !== modelId) return false;
  return agentModule.compatible;
}

/** The owner-visible inventory label. */
export function localModelInventoryLabel(availability: LocalModelAvailability): string {
  if (availability.status !== "UNAVAILABLE" && availability.status !== "NOT_CONFIGURED") {
    return `${availability.models.length} local model(s) inventoried.`;
  }
  const reason = availability.reason
    || "No qualified local inference provider is registered.";
  return `Local models stay inventory only. ${reason}`;
}
