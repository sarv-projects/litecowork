import type { RoutineInputField } from "../routines/routine-inputs";
import { pinnedResourceOptionKey, type ResourceOption } from "../routines/routine-inputs";

type Props = {
  fields: RoutineInputField[];
  values: Record<string, unknown>;
  onChange: (name: string, value: unknown) => void;
  disabled: boolean;
  workspaceId: string;
  resources: ResourceOption[];
  resourcesNextCursor: string | null;
  resourcesPageBusy: boolean;
  onLoadMoreResources: () => Promise<void>;
};

/** Shared bounded input renderer for a Manual run and an exact-revision test Task. */
export function AutomationInputFields(props: Props) {
  const { fields, values, onChange, disabled, workspaceId, resources, resourcesNextCursor, resourcesPageBusy, onLoadMoreResources } = props;
  return <>
    {fields.map(field => field.kind === "TEXT"
      ? <label className="automation-field" key={field.name}>
          <span>{field.label}{field.required ? " · required" : ""}</span>
          {field.enumValues
            ? <select required={field.required} disabled={disabled} value={typeof values[field.name] === "string" ? values[field.name] as string : ""} onChange={event => onChange(field.name, event.currentTarget.value)}>
                <option value="">Choose…</option>
                {field.enumValues.map(choice => <option key={choice} value={choice}>{choice}</option>)}
              </select>
            : <textarea rows={2} maxLength={Math.min(field.maxLength * 2, 32_768)} minLength={field.minLength || undefined} required={field.required} disabled={disabled} value={typeof values[field.name] === "string" ? values[field.name] as string : ""} onChange={event => onChange(field.name, event.currentTarget.value)} />}
          <small>{field.enumValues ? "Choose one of the values defined by this Routine." : `Maximum ${field.maxLength} characters and ${field.maxBytes} UTF-8 bytes.`}</small>
        </label>
      : <label className="automation-field" key={field.name}>
          <span>{field.label}{field.required ? " · required" : ""}</span>
          <select disabled={disabled} required={field.required} value={(() => {
            const ref = values[field.name];
            if (!ref || typeof ref !== "object" || Array.isArray(ref)) return "";
            const source = ref as Record<string, unknown>;
            const item = resources.find(resource => resource.workspaceId === workspaceId && resource.resourceId === source.resource_id && resource.resourceRevisionId === source.revision_id);
            return item ? pinnedResourceOptionKey(item) : "";
          })()} onChange={event => {
            const optionKey = event.currentTarget.value;
            const item = resources.find(resource => resource.workspaceId === workspaceId && pinnedResourceOptionKey(resource) === optionKey);
            onChange(field.name, item ? { workspace_id: workspaceId, resource_id: item.resourceId, revision_id: item.resourceRevisionId } : null);
          }}>
            <option value="">{field.required ? "Choose a Resource revision…" : "No Resource"}</option>
            {resources.filter(resource => resource.workspaceId === workspaceId).map(resource => <option key={pinnedResourceOptionKey(resource)} value={pinnedResourceOptionKey(resource)}>{resource.displayName} · {resource.mediaType} · {resource.resourceRevisionId}</option>)}
          </select>
          <small>The exact Workspace, Resource, and immutable revision are pinned to the Task.</small>
        </label>)}
    {fields.some(field => field.kind === "RESOURCE_REF") && resourcesNextCursor && <button type="button" className="text-button" disabled={resourcesPageBusy} onClick={() => void onLoadMoreResources()}>{resourcesPageBusy ? "Loading Resources…" : "Load more Resources"}</button>}
  </>;
}
