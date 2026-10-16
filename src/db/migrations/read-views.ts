/**
 * Stable SQL read views for direct SQLite clients (D125).
 *
 * View names use the sql_v1_ prefix for sql_schema_version 1. A future migration
 * may add sql_v2_* views without removing these names until a documented policy
 * retires them.
 */
export const READ_VIEWS_SQL = `
CREATE VIEW sql_v1_database_info AS
SELECT
  database_metadata.application_marker,
  database_metadata.sql_schema_version,
  database_metadata.athleticstandard_version,
  database_metadata.database_id,
  database_metadata.created_at,
  database_state.current_revision AS database_revision
FROM database_metadata
CROSS JOIN database_state
WHERE database_metadata.singleton_id = 1
  AND database_state.singleton_id = 1;

CREATE VIEW sql_v1_current_records AS
SELECT
  records.record_id,
  records.record_type,
  records.current_revision,
  records.created_at,
  records.is_deleted,
  record_revisions.became_known_at,
  record_revisions.became_known_at_basis,
  record_revisions.changed_by_json,
  record_revisions.previous_revision,
  record_revisions.record_json,
  record_revisions.extensions_json
FROM records
JOIN record_revisions
  ON record_revisions.record_id = records.record_id
 AND record_revisions.revision = records.current_revision;

CREATE VIEW sql_v1_active_records AS
SELECT
  records.record_id,
  records.record_type,
  records.current_revision,
  records.created_at,
  record_revisions.became_known_at,
  record_revisions.became_known_at_basis,
  record_revisions.changed_by_json,
  record_revisions.previous_revision,
  record_revisions.record_json,
  record_revisions.extensions_json
FROM records
JOIN record_revisions
  ON record_revisions.record_id = records.record_id
 AND record_revisions.revision = records.current_revision
WHERE records.is_deleted = 0;

CREATE VIEW sql_v1_record_revisions AS
SELECT
  record_id,
  revision,
  record_type,
  became_known_at,
  became_known_at_basis,
  changed_by_json,
  previous_revision,
  record_json,
  extensions_json
FROM record_revisions;

CREATE VIEW sql_v1_record_deletions AS
SELECT
  record_id,
  record_revision,
  deleted_at,
  deleted_by_json
FROM record_deletions;
`;
