/** The subset of Routine input contracts that the desktop can render safely. */
export type ResourceOption = {
  workspaceId: string;
  resourceId: string;
  resourceRevisionId: string;
  displayName: string;
  mediaType: string;
};

type TextInputField = {
  kind: "TEXT";
  name: string;
  label: string;
  required: boolean;
  maxLength: number;
  minLength: number;
  maxBytes: number;
  enumValues: string[] | null;
};

type ResourceInputField = {
  kind: "RESOURCE_REF";
  name: string;
  label: string;
  required: boolean;
};

export type RoutineInputField = TextInputField | ResourceInputField;
export type RoutineInputContract = { fields: RoutineInputField[]; blockedReason: string | null };

export function routineInputFields(revision: Record<string, unknown>): RoutineInputContract {
  const rawSchema = revision.input_schema as Record<string, unknown> | undefined;
  const schema = rawSchema && Object.keys(rawSchema).length === 0
    ? { type: "object", properties: {}, required: [] as string[] }
    : rawSchema;
  const properties = schema?.properties;
  const bindings = revision.input_bindings;
  if (!schema || (schema.type !== "object" && Object.keys(schema).length > 0) || !properties || typeof properties !== "object" || Array.isArray(properties) || !Array.isArray(bindings)) {
    return { fields: [], blockedReason: "This Routine uses an input contract the desktop form cannot safely present." };
  }
  const requiredNames = new Set(Array.isArray(schema.required) ? schema.required.filter((value): value is string => typeof value === "string") : []);
  const fields: RoutineInputField[] = [];
  for (const raw of bindings) {
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) return { fields: [], blockedReason: "This Routine has an unsupported input binding." };
    const binding = raw as Record<string, unknown>;
    if (typeof binding.required !== "boolean" || typeof binding.template_variable !== "string") return { fields: [], blockedReason: "This Routine has an unsupported input binding." };
    const pointer = binding.source_pointer;
    if (typeof pointer !== "string" || !pointer.startsWith("/") || pointer.slice(1).includes("/")) return { fields: [], blockedReason: "This Routine has an unsupported input binding." };
    const encodedName = pointer.slice(1);
    if (/~(?![01])/.test(encodedName)) return { fields: [], blockedReason: "This Routine has an unsupported input binding." };
    const name = encodedName.replace(/~1/g, "/").replace(/~0/g, "~");
    if (name === "__proto__" || name === "constructor" || name === "prototype") return { fields: [], blockedReason: "This Routine uses an input name the desktop form cannot safely handle." };
    const fieldSchema = (properties as Record<string, unknown>)[name];
    if (!fieldSchema || typeof fieldSchema !== "object" || Array.isArray(fieldSchema)) return { fields: [], blockedReason: "This Routine uses an unsupported input schema." };
    if (binding.value_kind === "RESOURCE_REF") {
      const field = fieldSchema as Record<string, unknown>;
      const resourceProperties = field.properties;
      const requiredResourceProperties = field.required;
      if (field.type !== "object" || field.additionalProperties !== false
        || !resourceProperties || typeof resourceProperties !== "object" || Array.isArray(resourceProperties)
        || Object.keys(resourceProperties).sort().join(",") !== "resource_id,revision_id,workspace_id"
        || !Array.isArray(requiredResourceProperties)
        || [...requiredResourceProperties].sort().join(",") !== "resource_id,revision_id,workspace_id"
        || Object.values(resourceProperties as Record<string, unknown>).some(value => !value || typeof value !== "object" || Array.isArray(value) || Object.keys(value).join(",") !== "type" || (value as Record<string, unknown>).type !== "string")) {
        return { fields: [], blockedReason: "This Routine uses a Resource input contract the desktop cannot safely present." };
      }
      fields.push({ kind: "RESOURCE_REF", name, label: name.replaceAll("_", " "), required: binding.required === true || requiredNames.has(name) });
      continue;
    }
    if (binding.value_kind !== "TEXT" || (fieldSchema as Record<string, unknown>).type !== "string" || typeof binding.template_variable !== "string") {
      return { fields: [], blockedReason: "This Routine uses an input type the desktop form cannot safely present." };
    }
    const field = fieldSchema as Record<string, unknown>;
    const enumValues = field.enum;
    if (enumValues !== undefined && (!Array.isArray(enumValues) || enumValues.length === 0 || enumValues.length > 128 || enumValues.some(value => typeof value !== "string"))) {
      return { fields: [], blockedReason: "This Routine uses a text-choice contract the desktop cannot safely present." };
    }
    fields.push({
      kind: "TEXT",
      name,
      label: name.replaceAll("_", " "),
      required: binding.required === true || requiredNames.has(name),
      maxLength: typeof field.maxLength === "number" ? Math.min(field.maxLength, 16_384) : 16_384,
      minLength: typeof field.minLength === "number" ? field.minLength : 0,
      maxBytes: typeof binding.max_bytes === "number" ? binding.max_bytes : 16_384,
      enumValues: Array.isArray(enumValues) ? enumValues as string[] : null,
    });
  }
  return { fields, blockedReason: null };
}

export function pinnedResourceOptionKey(resource: ResourceOption): string {
  return JSON.stringify([resource.workspaceId, resource.resourceId, resource.resourceRevisionId]);
}
