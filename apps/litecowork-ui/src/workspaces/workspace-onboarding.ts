export type PendingWorkspaceCreate = { name: string; requestId: string };

export class WorkspaceListRequestFence {
  #generation = 0;

  begin(): number {
    return ++this.#generation;
  }

  invalidate(): void {
    this.#generation += 1;
  }

  isCurrent(generation: number): boolean {
    return generation === this.#generation;
  }

  commitCreated<T extends { workspaceId: string }>(
    created: T,
    update: (apply: (current: T[]) => T[]) => void,
  ): void {
    this.invalidate();
    update((current) => current.some((item) => item.workspaceId === created.workspaceId)
      ? current
      : [...current, created]);
  }
}

export function nextWorkspaceCreateRequest(
  value: string,
  pending: PendingWorkspaceCreate | null,
  createRequestId: () => string,
): PendingWorkspaceCreate | null {
  const name = value.trim();
  if (!name || (pending !== null && pending.name !== name)) return null;
  if (pending) return pending;
  return { name, requestId: createRequestId() };
}

export class WorkspaceCreateGuard {
  #active = false;

  tryAcquire(): boolean {
    if (this.#active) return false;
    this.#active = true;
    return true;
  }

  release(): void {
    this.#active = false;
  }
}

export function canAutoSelectCreatedWorkspace(requestEpoch: number, currentEpoch: number): boolean {
  return requestEpoch === currentEpoch;
}

export function canRestoreInitialWorkspaceSelection(selectionEpoch: number): boolean {
  return selectionEpoch === 0;
}
