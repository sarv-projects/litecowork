-- Extend ResourceLocation availability for fail-closed local WorkspaceRoot recovery.
-- Applied with foreign_keys and legacy_alter_table enabled before BEGIN IMMEDIATE; the
-- migration runner restores both PRAGMAs and checks every dependent FK before accepting
-- the schema. This preserves immutable triggers while ResourceLocation is rebuilt.

CREATE TABLE resource_locations_v6 (
  location_id TEXT PRIMARY KEY,
  resource_id TEXT NOT NULL REFERENCES resources(resource_id),
  runtime_id TEXT REFERENCES runtimes(runtime_id),
  environment_id TEXT REFERENCES environments(environment_id),
  connection_id TEXT REFERENCES connections(connection_id),
  provider_ref TEXT,
  locator_ref_id TEXT NOT NULL,
  availability TEXT NOT NULL CHECK (availability IN ('AVAILABLE', 'OFFLINE', 'PLACEHOLDER', 'REVOKED', 'UNKNOWN', 'UNAVAILABLE')),
  writable INTEGER NOT NULL CHECK (writable IN (0, 1)),
  observed_revision_id TEXT,
  observed_digest TEXT CHECK (observed_digest IS NULL OR (length(observed_digest) = 71 AND substr(observed_digest, 1, 7) = 'sha256:' AND substr(observed_digest, 8) NOT GLOB '*[^0-9a-f]*')),
  observed_at TEXT NOT NULL,
  last_checked_at TEXT,
  UNIQUE(resource_id, location_id),
  UNIQUE(location_id, locator_ref_id),
  FOREIGN KEY(resource_id, observed_revision_id)
    REFERENCES resource_revisions(resource_id, resource_revision_id)
);

INSERT INTO resource_locations_v6 (
  location_id, resource_id, runtime_id, environment_id, connection_id, provider_ref,
  locator_ref_id, availability, writable, observed_revision_id, observed_digest,
  observed_at, last_checked_at
)
SELECT
  location_id, resource_id, runtime_id, environment_id, connection_id, provider_ref,
  locator_ref_id, availability, writable, observed_revision_id, observed_digest,
  observed_at, last_checked_at
FROM resource_locations;

DROP TABLE resource_locations;
ALTER TABLE resource_locations_v6 RENAME TO resource_locations;
CREATE INDEX idx_resource_locations_resource ON resource_locations(resource_id, availability);
