export type CoworkerRosterRequest = { workspaceId: string; epoch: number };
export type CoworkerRosterPage<T extends { coworker_id: string; workspace_id: string }> = {
  items: T[];
  next_cursor: string | null;
};

export function validateQuickCoworkerIdentity(name: string, purpose: string): string | null {
  const normalizedName = name.trim();
  const normalizedPurpose = purpose.trim();
  if (!normalizedName || normalizedName.length > 120) return "Enter a name between 1 and 120 characters.";
  if (!normalizedPurpose) return "Add a short description of what this Coworker should help with.";
  if (normalizedPurpose.length > 2000) return "Keep the purpose within 2,000 characters.";
  return null;
}

export function appendCoworkerRosterPage<T extends { coworker_id: string; workspace_id: string }>(
  current: readonly T[],
  page: CoworkerRosterPage<T>,
  expectedWorkspaceId: string,
  previousCursor: string,
): { items: T[]; nextCursor: string | null } {
  if (page.items.some((item) => item.workspace_id !== expectedWorkspaceId)) {
    throw new Error("Coworker page belongs to a different Workspace.");
  }
  if (page.next_cursor === previousCursor) {
    throw new Error("Coworker roster did not advance its pagination cursor.");
  }
  const seen = new Set(current.map((item) => item.coworker_id));
  const items = [...current];
  for (const item of page.items) {
    if (seen.has(item.coworker_id)) continue;
    seen.add(item.coworker_id);
    items.push(item);
  }
  return { items, nextCursor: page.next_cursor };
}

/** Fences roster and detail responses to the Workspace selection that admitted them. */
export class CoworkerRosterRequestEpoch {
  private workspaceId: string | null = null;
  private epoch = 0;

  begin(workspaceId: string): CoworkerRosterRequest {
    if (this.workspaceId !== workspaceId) this.workspaceId = workspaceId;
    this.epoch += 1;
    return { workspaceId, epoch: this.epoch };
  }

  current(): CoworkerRosterRequest | null {
    return this.workspaceId === null ? null : { workspaceId: this.workspaceId, epoch: this.epoch };
  }

  isCurrent(request: CoworkerRosterRequest): boolean {
    return request.workspaceId === this.workspaceId && request.epoch === this.epoch;
  }

  invalidate(): void {
    this.epoch += 1;
  }
}

/** A detail read or Coworker-specific command may update only its still-selected identity. */
export function isCoworkerDetailRequestCurrent(
  requestWorkspaceId: string,
  activeWorkspaceId: string,
  requestCoworkerId: string | null,
  activeCoworkerId: string | null,
): boolean {
  return requestWorkspaceId.trim().length > 0
    && requestWorkspaceId === activeWorkspaceId
    && requestCoworkerId === activeCoworkerId;
}
