export type WorkspaceInstructionSaveInput = {
  workspaceId: string;
  text: string;
  expectedVersion: number;
  parentRevisions: number[];
};

export type PendingWorkspaceInstructionSave = WorkspaceInstructionSaveInput & {
  requestId: string;
  resourceRequestId: string;
};

export type WorkspaceInstructionRequestContext = {
  workspaceId: string;
  generation: number;
};

export class WorkspaceInstructionDraftCache {
  private readonly drafts = new Map<string, string>();

  get(workspaceId: string): string | undefined {
    return this.drafts.get(workspaceId);
  }

  set(workspaceId: string, text: string): void {
    this.drafts.set(workspaceId, text);
  }

  delete(workspaceId: string): void {
    this.drafts.delete(workspaceId);
  }
}

export function canApplyWorkspaceInstructionResponse(
  response: WorkspaceInstructionRequestContext,
  current: WorkspaceInstructionRequestContext,
): boolean {
  return response.workspaceId === current.workspaceId
    && response.generation === current.generation;
}

export function getOrCreateInstructionSave(
  existing: PendingWorkspaceInstructionSave | null,
  input: WorkspaceInstructionSaveInput,
  createId: () => string,
): PendingWorkspaceInstructionSave {
  if (existing?.workspaceId === input.workspaceId && existing.text === input.text) {
    return existing;
  }
  return {
    ...input,
    parentRevisions: [...input.parentRevisions],
    requestId: createId(),
    resourceRequestId: createId(),
  };
}

export function instructionSaveFailureKind(input: {
  stale: boolean;
  resourceImported: boolean;
}): "STALE" | "STALE_AFTER_RESOURCE_IMPORT" | "RESOURCE_IMPORTED" | "RETRY_SAME_REQUEST" {
  if (input.stale) {
    return input.resourceImported ? "STALE_AFTER_RESOURCE_IMPORT" : "STALE";
  }
  return input.resourceImported ? "RESOURCE_IMPORTED" : "RETRY_SAME_REQUEST";
}

export function instructionSaveRefreshKind(input: {
  workspace: boolean;
  history: boolean;
}): "CURRENT" | "REFRESH_PENDING" {
  return input.workspace && input.history ? "CURRENT" : "REFRESH_PENDING";
}

export function workspaceInstructionByteLength(text: string): number {
  return new TextEncoder().encode(text).byteLength;
}
