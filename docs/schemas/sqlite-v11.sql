-- AutomationOccurrence aggregate revision is independent from claim fencing.
ALTER TABLE automation_occurrences
  ADD COLUMN version INTEGER NOT NULL DEFAULT 1 CHECK(version >= 1);

-- Preserve the latest known aggregate revision for databases that already journaled
-- occurrence events. Rows without prior occurrence events start at revision 1.
UPDATE automation_occurrences
SET version = COALESCE((
  SELECT MAX(e.entity_revision)
  FROM domain_events e
  WHERE e.workspace_id = automation_occurrences.workspace_id
    AND e.entity_type = 'AutomationOccurrence'
    AND e.entity_id = automation_occurrences.occurrence_id
), 1);

CREATE TRIGGER automation_occurrence_version_insert_guard
BEFORE INSERT ON automation_occurrences
WHEN NEW.version <> 1
BEGIN
  SELECT RAISE(ABORT, 'AUTOMATION_OCCURRENCE_VERSION_MUST_START_AT_ONE');
END;

CREATE TRIGGER automation_occurrence_version_update_guard
BEFORE UPDATE ON automation_occurrences
WHEN NEW.version <> OLD.version + 1
BEGIN
  SELECT RAISE(ABORT, 'AUTOMATION_OCCURRENCE_VERSION_MUST_INCREMENT_ONCE');
END;
