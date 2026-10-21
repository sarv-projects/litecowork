// Skill, SkillProposal, and demonstration API for the local Runtime.
//
// A SkillProposal is a draft that an owner must review. Nothing in this module
// approves, activates, installs, or publishes anything: the review commands
// record the owner's decision and report the publication blocker truthfully.
//
// Authority: docs/LEARNING.md, docs/CAPABILITY-FABRIC.md, docs/API.md.

export type SkillProposalStatus = "DRAFT" | "REVIEW" | "APPROVED" | "REJECTED" | "PUBLISHED";
export type SkillRedactionStatus = "PENDING" | "PASSED" | "FAILED";
export type SkillProposalSource = "DEMONSTRATION" | "LEARNING_CANDIDATE" | "OWNER_AUTHORED";
export type SkillPlaceholderKind = "PATH" | "URL" | "IDENTIFIER" | "LITERAL";
export type SkillStatus = "DRAFT" | "ACTIVE" | "SUSPENDED" | "REVOKED";
export type SkillKind = "LOCAL_PROCEDURE" | "MCP_SKILL" | "PACKAGE_COMPONENT";

export type SkillPlaceholder = {
  name: string;
  kind: SkillPlaceholderKind;
  sample: string | null;
};

export type SkillProposal = {
  skill_proposal_id: string;
  workspace_id: string;
  source: SkillProposalSource;
  source_task_id: string;
  source_artifact_id: string;
  draft_resource_ref: { workspace_id: string; resource_id: string; revision_id: string | null };
  draft_digest: string;
  redaction_status: SkillRedactionStatus;
  status: SkillProposalStatus;
  approved_by: { principal_id: string; kind: string } | null;
  published_capability_ref: unknown | null;
  placeholders: SkillPlaceholder[];
  declared_capabilities: string[];
  drift: {
    expected_manifest_digest: string;
    observed_manifest_digest: string;
    drifted_resources: string[];
    detected_at: string;
  } | null;
  created_at: string;
  updated_at: string;
  version: number;
};

export type SkillProposalPage = { items: SkillProposal[]; next_cursor: string | null };

export type SkillRevision = {
  revision: number;
  title: string;
  summary: string;
  manifest_digest: string;
  content_digest: string;
  content_ref: string;
  declared_capabilities: string[];
  declared_requirements: string[];
  created_at: string;
  created_by: string;
};

export type Skill = {
  skill_id: string;
  workspace_id: string;
  kind: SkillKind;
  status: SkillStatus;
  capability_ref: {
    capability_id: string;
    identity_kind: string;
    source: string;
    package_version: string | null;
    digest: string;
    component: string;
  };
  current_revision: number;
  revision: SkillRevision | null;
  created_at: string;
  updated_at: string;
  version: number;
};

export type SkillPage = { items: Skill[]; next_cursor: string | null };

export type SkillProposalTransport = (
  workspaceId: string,
  status: SkillProposalStatus | undefined,
  cursor: string | undefined,
  signal?: AbortSignal,
) => Promise<unknown>;
export type SkillProposalReviewTransport = (
  workspaceId: string,
  proposalId: string,
  decision: "approve" | "reject",
  expectedVersion: number,
  requestId: string,
) => Promise<unknown>;
export type SkillTransport = (
  workspaceId: string,
  cursor: string | undefined,
  signal?: AbortSignal,
) => Promise<unknown>;

export interface SkillApi {
  readonly workspaceId: string;
  proposals(status?: SkillProposalStatus, cursor?: string, signal?: AbortSignal): Promise<SkillProposalPage>;
  approve(proposalId: string, expectedVersion: number, requestId: string): Promise<SkillProposal>;
  reject(proposalId: string, expectedVersion: number, requestId: string): Promise<SkillProposal>;
  skills(cursor?: string, signal?: AbortSignal): Promise<SkillPage>;
}

export class SkillApiError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SkillApiError";
  }
}

type Obj = Record<string, unknown>;
function object(value: unknown): Obj {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new SkillApiError("Local Runtime returned an invalid Skill record.");
  }
  return value as Obj;
}
function text(value: unknown, empty = false): string {
  if (typeof value !== "string" || (!empty && value.trim().length === 0)) {
    throw new SkillApiError("Local Runtime returned an invalid Skill record.");
  }
  return value;
}
function nullableText(value: unknown): string | null {
  return value == null ? null : text(value);
}
function oneOf<T extends string>(value: unknown, allowed: readonly T[], label: string): T {
  if (typeof value !== "string" || !(allowed as readonly string[]).includes(value)) {
    throw new SkillApiError(`Local Runtime returned an unsupported ${label}.`);
  }
  return value as T;
}
function version(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) {
    throw new SkillApiError("Local Runtime returned an invalid Skill version.");
  }
  return value;
}
function stringList(value: unknown): string[] {
  if (!Array.isArray(value)) throw new SkillApiError("Local Runtime returned an invalid Skill record.");
  return value.map(item => text(item));
}

const PROPOSAL_STATUSES = ["DRAFT", "REVIEW", "APPROVED", "REJECTED", "PUBLISHED"] as const;
const REDACTION_STATUSES = ["PENDING", "PASSED", "FAILED"] as const;
const PROPOSAL_SOURCES = ["DEMONSTRATION", "LEARNING_CANDIDATE", "OWNER_AUTHORED"] as const;
const PLACEHOLDER_KINDS = ["PATH", "URL", "IDENTIFIER", "LITERAL"] as const;
const SKILL_STATUSES = ["DRAFT", "ACTIVE", "SUSPENDED", "REVOKED"] as const;
const SKILL_KINDS = ["LOCAL_PROCEDURE", "MCP_SKILL", "PACKAGE_COMPONENT"] as const;

export function decodeSkillProposal(value: unknown, workspaceId: string): SkillProposal {
  const row = object(value);
  if (text(row.workspace_id) !== workspaceId) {
    throw new SkillApiError("A SkillProposal belongs to another Workspace.");
  }
  const digest = text(row.draft_digest);
  if (!/^sha256:[a-f0-9]{64}$/.test(digest)) {
    throw new SkillApiError("Local Runtime returned an invalid SkillProposal digest.");
  }
  const ref = object(row.draft_resource_ref);
  if (text(ref.workspace_id) !== workspaceId) {
    throw new SkillApiError("A SkillProposal draft belongs to another Workspace.");
  }
  return {
    skill_proposal_id: text(row.skill_proposal_id),
    workspace_id: workspaceId,
    source: oneOf(row.source, PROPOSAL_SOURCES, "SkillProposal source"),
    source_task_id: text(row.source_task_id),
    source_artifact_id: text(row.source_artifact_id),
    draft_resource_ref: {
      workspace_id: workspaceId,
      resource_id: text(ref.resource_id),
      revision_id: nullableText(ref.revision_id),
    },
    draft_digest: digest,
    redaction_status: oneOf(row.redaction_status, REDACTION_STATUSES, "redaction status"),
    status: oneOf(row.status, PROPOSAL_STATUSES, "SkillProposal status"),
    approved_by:
      row.approved_by == null
        ? null
        : (() => {
            const principal = object(row.approved_by);
            return { principal_id: text(principal.principal_id), kind: text(principal.kind) };
          })(),
    published_capability_ref: row.published_capability_ref ?? null,
    placeholders: (() => {
      if (!Array.isArray(row.placeholders)) throw new SkillApiError("Local Runtime returned invalid SkillProposal placeholders.");
      return row.placeholders.map(raw => {
        const placeholder = object(raw);
        return {
          name: text(placeholder.name),
          kind: oneOf(placeholder.kind, PLACEHOLDER_KINDS, "placeholder kind"),
          sample: nullableText(placeholder.sample),
        };
      });
    })(),
    declared_capabilities: stringList(row.declared_capabilities),
    drift:
      row.drift == null
        ? null
        : (() => {
            const drift = object(row.drift);
            return {
              expected_manifest_digest: text(drift.expected_manifest_digest),
              observed_manifest_digest: text(drift.observed_manifest_digest),
              drifted_resources: stringList(drift.drifted_resources),
              detected_at: text(drift.detected_at),
            };
          })(),
    created_at: text(row.created_at),
    updated_at: text(row.updated_at),
    version: version(row.version),
  };
}

export function decodeSkillProposalPage(value: unknown, workspaceId: string): SkillProposalPage {
  const page = object(value);
  if (!Array.isArray(page.items)) throw new SkillApiError("Local Runtime returned an invalid SkillProposal page.");
  return {
    items: page.items.map(item => decodeSkillProposal(item, workspaceId)),
    next_cursor: page.next_cursor == null ? null : text(page.next_cursor),
  };
}

export function decodeSkill(value: unknown, workspaceId: string): Skill {
  const row = object(value);
  if (text(row.workspace_id) !== workspaceId) {
    throw new SkillApiError("A Skill belongs to another Workspace.");
  }
  const capability = object(row.capability_ref);
  const digest = text(capability.digest);
  if (!/^sha256:[a-f0-9]{64}$/.test(digest)) {
    throw new SkillApiError("Local Runtime returned an invalid Skill digest.");
  }
  return {
    skill_id: text(row.skill_id),
    workspace_id: workspaceId,
    kind: oneOf(row.kind, SKILL_KINDS, "Skill kind"),
    status: oneOf(row.status, SKILL_STATUSES, "Skill status"),
    capability_ref: {
      capability_id: text(capability.capability_id),
      identity_kind: text(capability.identity_kind),
      source: text(capability.source),
      package_version: nullableText(capability.package_version),
      digest,
      component: text(capability.component),
    },
    current_revision: version(row.current_revision),
    revision:
      row.revision == null
        ? null
        : (() => {
            const revision = object(row.revision);
            return {
              revision: version(revision.revision),
              title: text(revision.title),
              summary: text(revision.summary, true),
              manifest_digest: text(revision.manifest_digest),
              content_digest: text(revision.content_digest),
              content_ref: text(revision.content_ref),
              declared_capabilities: stringList(revision.declared_capabilities),
              declared_requirements: stringList(revision.declared_requirements),
              created_at: text(revision.created_at),
              created_by: text(revision.created_by),
            };
          })(),
    created_at: text(row.created_at),
    updated_at: text(row.updated_at),
    version: version(row.version),
  };
}

export function decodeSkillPage(value: unknown, workspaceId: string): SkillPage {
  const page = object(value);
  if (!Array.isArray(page.items)) throw new SkillApiError("Local Runtime returned an invalid Skill page.");
  return {
    items: page.items.map(item => decodeSkill(item, workspaceId)),
    next_cursor: page.next_cursor == null ? null : text(page.next_cursor),
  };
}

export function createSkillApi(
  workspaceId: string,
  proposalTransport: SkillProposalTransport,
  reviewTransport: SkillProposalReviewTransport,
  skillTransport: SkillTransport,
): SkillApi {
  if (!workspaceId.trim()) throw new Error("Select a Workspace before reviewing Skill proposals.");
  return {
    workspaceId,
    async proposals(status, cursor, signal) {
      return decodeSkillProposalPage(await proposalTransport(workspaceId, status, cursor, signal), workspaceId);
    },
    async approve(proposalId, expectedVersion, requestId) {
      return decodeSkillProposal(
        await reviewTransport(workspaceId, proposalId, "approve", expectedVersion, requestId),
        workspaceId,
      );
    },
    async reject(proposalId, expectedVersion, requestId) {
      return decodeSkillProposal(
        await reviewTransport(workspaceId, proposalId, "reject", expectedVersion, requestId),
        workspaceId,
      );
    },
    async skills(cursor, signal) {
      return decodeSkillPage(await skillTransport(workspaceId, cursor, signal), workspaceId);
    },
  };
}
