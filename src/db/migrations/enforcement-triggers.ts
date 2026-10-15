/**
 * SQLite triggers that enforce critical cross-table rules for direct writers (D116).
 *
 * Foreign keys remain required, but these triggers also refuse invalid states when
 * application code bypasses the Node library.
 */
export const ENFORCEMENT_TRIGGERS_SQL = `
CREATE TRIGGER record_revisions_record_type_matches_parent_insert
BEFORE INSERT ON record_revisions
FOR EACH ROW
WHEN (
  SELECT record_type FROM records WHERE record_id = NEW.record_id
) IS NOT NULL
  AND (
    SELECT record_type FROM records WHERE record_id = NEW.record_id
  ) <> NEW.record_type
BEGIN
  SELECT RAISE(ABORT, 'record_revisions.record_type must match records.record_type');
END;

CREATE TRIGGER record_revisions_record_type_matches_parent_update
BEFORE UPDATE OF record_type ON record_revisions
FOR EACH ROW
WHEN (
  SELECT record_type FROM records WHERE record_id = NEW.record_id
) IS NOT NULL
  AND (
    SELECT record_type FROM records WHERE record_id = NEW.record_id
  ) <> NEW.record_type
BEGIN
  SELECT RAISE(ABORT, 'record_revisions.record_type must match records.record_type');
END;

CREATE TRIGGER records_current_revision_must_exist
BEFORE UPDATE OF current_revision ON records
FOR EACH ROW
WHEN (
  SELECT revision
  FROM record_revisions
  WHERE record_id = NEW.record_id AND revision = NEW.current_revision
) IS NULL
BEGIN
  SELECT RAISE(ABORT, 'records.current_revision must reference an existing revision');
END;

CREATE TRIGGER athlete_report_data_cannot_claim_source_insert
BEFORE INSERT ON record_revisions
FOR EACH ROW
WHEN NEW.record_type IN ('athlete_report', 'nutrition_entry')
  AND json_extract(NEW.record_json, '$.source_id') IS NOT NULL
BEGIN
  SELECT RAISE(ABORT, 'athlete-reported record data cannot contain source_id');
END;

CREATE TRIGGER athlete_report_data_cannot_claim_source_update
BEFORE UPDATE OF record_json ON record_revisions
FOR EACH ROW
WHEN NEW.record_type IN ('athlete_report', 'nutrition_entry')
  AND json_extract(NEW.record_json, '$.source_id') IS NOT NULL
BEGIN
  SELECT RAISE(ABORT, 'athlete-reported record data cannot contain source_id');
END;

CREATE TRIGGER athlete_report_extensions_cannot_claim_source_insert
BEFORE INSERT ON record_revisions
FOR EACH ROW
WHEN NEW.record_type IN ('athlete_report', 'nutrition_entry', 'attachment', 'athlete_profile')
  AND NEW.extensions_json IS NOT NULL
  AND EXISTS (
    SELECT 1
    FROM json_tree(NEW.extensions_json)
    WHERE key = 'source_id'
  )
BEGIN
  SELECT RAISE(ABORT, 'athlete-reported record extensions cannot contain source_id');
END;

CREATE TRIGGER athlete_report_extensions_cannot_claim_source_update
BEFORE UPDATE OF extensions_json ON record_revisions
FOR EACH ROW
WHEN NEW.record_type IN ('athlete_report', 'nutrition_entry', 'attachment', 'athlete_profile')
  AND NEW.extensions_json IS NOT NULL
  AND EXISTS (
    SELECT 1
    FROM json_tree(NEW.extensions_json)
    WHERE key = 'source_id'
  )
BEGIN
  SELECT RAISE(ABORT, 'athlete-reported record extensions cannot contain source_id');
END;

CREATE TRIGGER records_deleted_flag_must_match_deletion_row
BEFORE UPDATE OF is_deleted ON records
FOR EACH ROW
WHEN (
  NEW.is_deleted = 1 AND (
    SELECT record_id FROM record_deletions WHERE record_id = NEW.record_id
  ) IS NULL
) OR (
  NEW.is_deleted = 0 AND (
    SELECT record_id FROM record_deletions WHERE record_id = NEW.record_id
  ) IS NOT NULL
)
BEGIN
  SELECT RAISE(ABORT, 'records.is_deleted must match record_deletions');
END;

CREATE TRIGGER record_erasure_blocks_current_record_insert
BEFORE INSERT ON records
FOR EACH ROW
WHEN (
  SELECT record_id FROM record_erasure_markers WHERE record_id = NEW.record_id
) IS NOT NULL
BEGIN
  SELECT RAISE(ABORT, 'erased records cannot have a current records row');
END;

CREATE TRIGGER record_erasure_blocks_current_record_update
BEFORE UPDATE ON records
FOR EACH ROW
WHEN (
  SELECT record_id FROM record_erasure_markers WHERE record_id = NEW.record_id
) IS NOT NULL
BEGIN
  SELECT RAISE(ABORT, 'erased records cannot have a current records row');
END;

CREATE TRIGGER typed_row_record_type_matches_insert
BEFORE INSERT ON athlete_profile
FOR EACH ROW
WHEN NEW.record_type <> 'athlete_profile'
BEGIN
  SELECT RAISE(ABORT, 'athlete_profile.record_type must be athlete_profile');
END;

CREATE TRIGGER typed_row_record_type_matches_sources_insert
BEFORE INSERT ON sources
FOR EACH ROW
WHEN NEW.record_type <> 'source'
BEGIN
  SELECT RAISE(ABORT, 'sources.record_type must be source');
END;

CREATE TRIGGER typed_row_record_type_matches_point_measurements_insert
BEFORE INSERT ON point_measurements
FOR EACH ROW
WHEN NEW.record_type <> 'point_measurement'
BEGIN
  SELECT RAISE(ABORT, 'point_measurements.record_type must be point_measurement');
END;

CREATE TRIGGER typed_row_record_type_matches_athlete_reports_insert
BEFORE INSERT ON athlete_reports
FOR EACH ROW
WHEN NEW.record_type <> 'athlete_report'
BEGIN
  SELECT RAISE(ABORT, 'athlete_reports.record_type must be athlete_report');
END;

CREATE TRIGGER point_measurements_require_source_insert
BEFORE INSERT ON point_measurements
FOR EACH ROW
WHEN NEW.source_id IS NULL
BEGIN
  SELECT RAISE(ABORT, 'measured records require source_id');
END;

CREATE TRIGGER prediction_result_links_revision_exists_insert
BEFORE INSERT ON prediction_result_links
FOR EACH ROW
WHEN (
  SELECT revision
  FROM record_revisions
  WHERE record_id = NEW.prediction_id AND revision = NEW.prediction_revision
) IS NULL
  OR (
    SELECT revision
    FROM record_revisions
    WHERE record_id = NEW.benchmark_result_id
      AND revision = NEW.benchmark_result_revision
  ) IS NULL
BEGIN
  SELECT RAISE(ABORT, 'prediction_result_links must reference existing revisions');
END;
`;
