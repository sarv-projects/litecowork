-- ResourceEdge rows are append-only provenance records. Resource deletion/revocation
-- changes Resource availability through its owning lifecycle; it does not erase edges.
CREATE TRIGGER resource_edges_immutable_update
BEFORE UPDATE ON resource_edges
BEGIN
  SELECT RAISE(ABORT, 'ResourceEdge records are immutable');
END;

CREATE TRIGGER resource_edges_immutable_delete
BEFORE DELETE ON resource_edges
BEGIN
  SELECT RAISE(ABORT, 'ResourceEdge records are immutable');
END;
