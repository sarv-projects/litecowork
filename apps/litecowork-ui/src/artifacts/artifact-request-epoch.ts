export type ArtifactRequest = { scope: string; epoch: number };

/** Fences asynchronous Workbench reads and writes to one Workspace/Artifact visit. */
export class ArtifactRequestEpoch {
  private scope: string | null = null;
  private epoch = 0;

  activate(workspaceId: string, artifactId: string): ArtifactRequest {
    this.scope = JSON.stringify([workspaceId, artifactId]);
    this.epoch += 1;
    return { scope: this.scope, epoch: this.epoch };
  }

  capture(): ArtifactRequest | null {
    return this.scope === null ? null : { scope: this.scope, epoch: this.epoch };
  }

  isCurrent(request: ArtifactRequest): boolean {
    return request.scope === this.scope && request.epoch === this.epoch;
  }

  invalidate(): void {
    this.epoch += 1;
  }
}
