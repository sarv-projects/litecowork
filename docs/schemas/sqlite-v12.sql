-- Presentation preference is pinned to the durable ConversationTurn. Old turns migrate
-- as AUTO; retries reuse the stored preference.
ALTER TABLE conversation_turns
  ADD COLUMN presentation_preference TEXT NOT NULL DEFAULT 'AUTO'
  CHECK (presentation_preference IN ('AUTO', 'SIMPLE', 'RICH'));

CREATE TRIGGER conversation_turn_presentation_preference_immutable
BEFORE UPDATE OF presentation_preference ON conversation_turns
WHEN NEW.presentation_preference <> OLD.presentation_preference
BEGIN
  SELECT RAISE(ABORT, 'CONVERSATION_TURN_PRESENTATION_PREFERENCE_IMMUTABLE');
END;

CREATE TABLE rich_presentations (
  presentation_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
  conversation_id TEXT NOT NULL,
  message_id TEXT NOT NULL UNIQUE REFERENCES conversation_messages(message_id),
  schema_version INTEGER NOT NULL CHECK (schema_version = 1),
  renderer_contract_version INTEGER NOT NULL CHECK (renderer_contract_version >= 1),
  semantic_content_digest TEXT NOT NULL CHECK (length(semantic_content_digest) = 71 AND substr(semantic_content_digest, 1, 7) = 'sha256:' AND substr(semantic_content_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  document_ref_json TEXT NOT NULL CHECK (json_valid(document_ref_json)),
  document_digest TEXT NOT NULL CHECK (length(document_digest) = 71 AND substr(document_digest, 1, 7) = 'sha256:' AND substr(document_digest, 8) NOT GLOB '*[^0-9a-f]*'),
  document_size_bytes INTEGER NOT NULL CHECK (document_size_bytes BETWEEN 1 AND 1048576),
  producer_agent_session_id TEXT,
  host_instruction_digest TEXT CHECK (host_instruction_digest IS NULL OR (length(host_instruction_digest) = 71 AND substr(host_instruction_digest, 1, 7) = 'sha256:' AND substr(host_instruction_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  host_skill_refs_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(host_skill_refs_json)),
  created_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1 CHECK (version = 1),
  FOREIGN KEY(workspace_id, conversation_id)
    REFERENCES conversations(workspace_id, conversation_id),
  UNIQUE(workspace_id, conversation_id, message_id)
);

CREATE INDEX idx_rich_presentations_conversation
  ON rich_presentations(workspace_id, conversation_id, created_at);

CREATE TRIGGER rich_presentation_insert_guard
BEFORE INSERT ON rich_presentations
WHEN NOT EXISTS (
  SELECT 1
  FROM conversation_messages m
  JOIN conversations c ON c.conversation_id = m.conversation_id
  WHERE m.message_id = NEW.message_id
    AND m.conversation_id = NEW.conversation_id
    AND c.workspace_id = NEW.workspace_id
    AND m.role = 'AGENT'
    AND m.turn_id IS NOT NULL
)
BEGIN
  SELECT RAISE(ABORT, 'RICH_PRESENTATION_REQUIRES_SAME_WORKSPACE_AGENT_MESSAGE');
END;

CREATE TRIGGER rich_presentation_no_update
BEFORE UPDATE ON rich_presentations
BEGIN
  SELECT RAISE(ABORT, 'RICH_PRESENTATION_IMMUTABLE');
END;

CREATE TRIGGER rich_presentation_no_delete
BEFORE DELETE ON rich_presentations
BEGIN
  SELECT RAISE(ABORT, 'RICH_PRESENTATION_IMMUTABLE');
END;
