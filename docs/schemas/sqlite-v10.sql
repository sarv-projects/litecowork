-- Version ten adds immutable, same-Workspace Artifact-version references to Goal
-- revisions. Removing a link creates a new Goal revision; historical links remain.
CREATE TABLE goal_artifact_links (
  goal_id TEXT NOT NULL,
  revision INTEGER NOT NULL,
  workspace_id TEXT NOT NULL,
  artifact_id TEXT NOT NULL,
  artifact_version INTEGER NOT NULL CHECK (artifact_version >= 1),
  PRIMARY KEY(goal_id, revision, artifact_id, artifact_version),
  FOREIGN KEY(goal_id, revision, workspace_id)
    REFERENCES goal_revisions(goal_id, revision, workspace_id),
  FOREIGN KEY(artifact_id, artifact_version)
    REFERENCES artifact_versions(artifact_id, version)
);
CREATE INDEX idx_goal_artifact_links_artifact
  ON goal_artifact_links(artifact_id, artifact_version, goal_id, revision);

CREATE TRIGGER goal_artifact_link_current_revision_guard
BEFORE INSERT ON goal_artifact_links
WHEN NOT EXISTS (
  SELECT 1
  FROM goals g
  JOIN artifacts a ON a.artifact_id = NEW.artifact_id
  JOIN artifact_versions av ON av.artifact_id = a.artifact_id
  WHERE g.goal_id = NEW.goal_id
    AND g.workspace_id = NEW.workspace_id
    AND g.current_revision = NEW.revision
    AND g.status <> 'ARCHIVED'
    AND a.workspace_id = NEW.workspace_id
    AND av.version = NEW.artifact_version
)
BEGIN
  SELECT RAISE(ABORT, 'GOAL_ARTIFACT_REFERENCE_SCOPE_INVALID');
END;

CREATE TRIGGER goal_artifact_link_immutable_update
BEFORE UPDATE ON goal_artifact_links
BEGIN
  SELECT RAISE(ABORT, 'GOAL_REVISION_LINKS_IMMUTABLE');
END;

CREATE TRIGGER goal_artifact_link_immutable_delete
BEFORE DELETE ON goal_artifact_links
BEGIN
  SELECT RAISE(ABORT, 'GOAL_REVISION_LINKS_IMMUTABLE');
END;
