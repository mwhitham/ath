import { ENFORCEMENT_TRIGGERS_SQL } from "./enforcement-triggers.js";
import { READ_VIEWS_SQL } from "./read-views.js";

/**
 * Initial Athletic Standard v1 SQLite schema.
 *
 * The SQL is intentionally kept as plain SQL. Application values are written by
 * parameterized statements after these tables exist.
 */
export const INITIAL_SCHEMA_SQL = `
CREATE TABLE database_metadata (
  singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
  application_marker TEXT NOT NULL CHECK (application_marker = 'ATH1'),
  sql_schema_version INTEGER NOT NULL CHECK (sql_schema_version >= 1),
  athleticstandard_version TEXT NOT NULL,
  database_id TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
) STRICT;

CREATE TABLE database_state (
  singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
  current_revision INTEGER NOT NULL CHECK (current_revision >= 0)
) STRICT;

CREATE TABLE records (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL CHECK (record_type IN (
    'athlete_profile',
    'source',
    'source_device',
    'point_measurement',
    'measurement_sample',
    'sleep_session',
    'workout_session',
    'benchmark_result',
    'provider_summary',
    'provider_score',
    'athlete_report',
    'nutrition_entry',
    'attachment',
    'benchmark_definition',
    'prediction',
    'prediction_result_link',
    'prediction_grade',
    'prediction_error_analysis',
    'training_plan',
    'planned_session',
    'coaching_note',
    'saved_training_review',
    'goal',
    'nutrition_model_interpretation'
  )),
  current_revision INTEGER NOT NULL CHECK (current_revision >= 1),
  created_at TEXT NOT NULL,
  is_deleted INTEGER NOT NULL DEFAULT 0 CHECK (is_deleted IN (0, 1)),
  UNIQUE (record_id, record_type),
  FOREIGN KEY (record_id, current_revision)
    REFERENCES record_revisions (record_id, revision)
    DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE TABLE record_revisions (
  record_id TEXT NOT NULL,
  revision INTEGER NOT NULL CHECK (revision >= 1),
  record_type TEXT NOT NULL,
  record_json TEXT NOT NULL CHECK (json_valid(record_json)),
  extensions_json TEXT CHECK (
    extensions_json IS NULL OR (
      json_valid(extensions_json) AND
      json_type(extensions_json) = 'object' AND
      length(extensions_json) <= 65536
    )
  ),
  became_known_at TEXT NOT NULL,
  became_known_at_basis TEXT NOT NULL CHECK (became_known_at_basis IN (
    'local_write',
    'provider_import',
    'package_restore',
    'package_merge'
  )),
  changed_by_json TEXT NOT NULL CHECK (json_valid(changed_by_json)),
  previous_revision INTEGER,
  PRIMARY KEY (record_id, revision),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE,
  FOREIGN KEY (record_id, previous_revision)
    REFERENCES record_revisions (record_id, revision),
  CHECK (
    (revision = 1 AND previous_revision IS NULL) OR
    (revision > 1 AND previous_revision = revision - 1)
  )
) STRICT;

CREATE TABLE record_deletions (
  record_id TEXT PRIMARY KEY,
  record_revision INTEGER NOT NULL,
  deleted_at TEXT NOT NULL,
  deleted_by_json TEXT NOT NULL CHECK (json_valid(deleted_by_json)),
  FOREIGN KEY (record_id, record_revision)
    REFERENCES record_revisions (record_id, revision)
) STRICT;

CREATE TABLE record_erasure_markers (
  record_id TEXT PRIMARY KEY,
  erased_at TEXT NOT NULL,
  reason_code TEXT NOT NULL CHECK (reason_code IN (
    'athlete_request',
    'legal_requirement',
    'recorded_in_error',
    'other'
  ))
) STRICT;

CREATE TABLE record_import_keys (
  record_id TEXT NOT NULL,
  match_key_method TEXT NOT NULL,
  match_key TEXT NOT NULL,
  imported_at TEXT NOT NULL,
  PRIMARY KEY (match_key_method, match_key),
  FOREIGN KEY (record_id) REFERENCES records (record_id) ON DELETE CASCADE
) STRICT;

CREATE INDEX record_import_keys_record_id_idx
  ON record_import_keys (record_id);
CREATE INDEX record_revisions_known_at_idx
  ON record_revisions (became_known_at, record_id, revision);
CREATE INDEX records_record_type_idx
  ON records (record_type, record_id);

CREATE TABLE athlete_profile (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'athlete_profile'
    CHECK (record_type = 'athlete_profile'),
  default_time_zone TEXT,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE
) STRICT;

CREATE UNIQUE INDEX athlete_profile_single_record_idx
  ON athlete_profile ((1));

CREATE TABLE sources (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'source' CHECK (record_type = 'source'),
  source_type TEXT NOT NULL CHECK (source_type IN (
    'wearable',
    'export_file',
    'connector',
    'manual'
  )),
  data_writer TEXT NOT NULL,
  data_route TEXT NOT NULL,
  vendor TEXT,
  sensor_type TEXT,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE
) STRICT;

CREATE UNIQUE INDEX sources_writer_route_sensor_idx
  ON sources (data_writer, data_route, COALESCE(sensor_type, ''));

CREATE TABLE source_devices (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'source_device'
    CHECK (record_type = 'source_device'),
  source_id TEXT NOT NULL,
  name TEXT,
  manufacturer TEXT,
  model TEXT,
  hardware TEXT,
  serial_number TEXT,
  coverage_start_date TEXT,
  coverage_end_date TEXT,
  accepted_record_count INTEGER,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE,
  FOREIGN KEY (source_id) REFERENCES sources (record_id)
) STRICT;

CREATE TABLE point_measurements (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'point_measurement'
    CHECK (record_type = 'point_measurement'),
  source_id TEXT NOT NULL,
  quantity TEXT NOT NULL,
  recorded_at TEXT,
  standard_value REAL NOT NULL,
  standard_unit TEXT NOT NULL,
  provider_record_id TEXT,
  received_at TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE,
  FOREIGN KEY (source_id) REFERENCES sources (record_id)
) STRICT;

CREATE INDEX point_measurements_lookup_idx
  ON point_measurements (quantity, source_id, recorded_at);

CREATE TABLE point_measurement_calculations (
  point_measurement_id TEXT NOT NULL,
  calculation_order INTEGER NOT NULL CHECK (calculation_order >= 0),
  calculation_json TEXT NOT NULL CHECK (json_valid(calculation_json)),
  PRIMARY KEY (point_measurement_id, calculation_order),
  FOREIGN KEY (point_measurement_id)
    REFERENCES point_measurements (record_id)
    ON DELETE CASCADE
) STRICT;

CREATE TABLE point_measurement_calculation_inputs (
  point_measurement_id TEXT NOT NULL,
  calculation_order INTEGER NOT NULL,
  input_order INTEGER NOT NULL CHECK (input_order >= 0),
  evidence_record_id TEXT NOT NULL,
  evidence_revision INTEGER NOT NULL,
  PRIMARY KEY (point_measurement_id, calculation_order, input_order),
  FOREIGN KEY (point_measurement_id, calculation_order)
    REFERENCES point_measurement_calculations (
      point_measurement_id,
      calculation_order
    )
    ON DELETE CASCADE,
  FOREIGN KEY (evidence_record_id, evidence_revision)
    REFERENCES record_revisions (record_id, revision)
) STRICT;

CREATE TABLE measurement_samples (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'measurement_sample'
    CHECK (record_type = 'measurement_sample'),
  source_id TEXT NOT NULL,
  quantity TEXT NOT NULL,
  recorded_at TEXT NOT NULL,
  standard_value REAL NOT NULL,
  standard_unit TEXT NOT NULL,
  duration_ms INTEGER,
  provider_record_id TEXT,
  received_at TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE,
  FOREIGN KEY (source_id) REFERENCES sources (record_id)
) STRICT;

CREATE INDEX measurement_samples_lookup_idx
  ON measurement_samples (quantity, source_id, recorded_at);

CREATE TABLE sleep_sessions (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'sleep_session'
    CHECK (record_type = 'sleep_session'),
  source_id TEXT NOT NULL,
  start_at TEXT NOT NULL,
  end_at TEXT NOT NULL,
  provider_record_id TEXT,
  received_at TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE,
  FOREIGN KEY (source_id) REFERENCES sources (record_id),
  CHECK (end_at >= start_at)
) STRICT;

CREATE INDEX sleep_sessions_time_idx
  ON sleep_sessions (source_id, start_at, end_at);

CREATE TABLE workout_sessions (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'workout_session'
    CHECK (record_type = 'workout_session'),
  source_id TEXT NOT NULL,
  start_at TEXT NOT NULL,
  end_at TEXT NOT NULL,
  activity_type TEXT,
  provider_record_id TEXT,
  received_at TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE,
  FOREIGN KEY (source_id) REFERENCES sources (record_id),
  CHECK (end_at >= start_at)
) STRICT;

CREATE INDEX workout_sessions_time_idx
  ON workout_sessions (source_id, start_at, end_at);

CREATE TABLE workout_session_segments (
  workout_session_id TEXT NOT NULL,
  segment_order INTEGER NOT NULL CHECK (segment_order >= 0),
  segment_json TEXT NOT NULL CHECK (json_valid(segment_json)),
  PRIMARY KEY (workout_session_id, segment_order),
  FOREIGN KEY (workout_session_id)
    REFERENCES workout_sessions (record_id)
    ON DELETE CASCADE
) STRICT;

CREATE TABLE benchmark_results (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'benchmark_result'
    CHECK (record_type = 'benchmark_result'),
  source_id TEXT NOT NULL,
  benchmark_definition_id TEXT NOT NULL,
  recorded_at TEXT NOT NULL,
  workout_session_id TEXT,
  provider_record_id TEXT,
  received_at TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE,
  FOREIGN KEY (source_id) REFERENCES sources (record_id),
  FOREIGN KEY (benchmark_definition_id)
    REFERENCES benchmark_definitions (record_id),
  FOREIGN KEY (workout_session_id)
    REFERENCES workout_sessions (record_id)
) STRICT;

CREATE INDEX benchmark_results_lookup_idx
  ON benchmark_results (benchmark_definition_id, recorded_at);

CREATE TABLE benchmark_result_segments (
  benchmark_result_id TEXT NOT NULL,
  segment_order INTEGER NOT NULL CHECK (segment_order >= 0),
  segment_json TEXT NOT NULL CHECK (json_valid(segment_json)),
  PRIMARY KEY (benchmark_result_id, segment_order),
  FOREIGN KEY (benchmark_result_id)
    REFERENCES benchmark_results (record_id)
    ON DELETE CASCADE
) STRICT;

CREATE TABLE provider_scores (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'provider_score'
    CHECK (record_type = 'provider_score'),
  source_id TEXT NOT NULL,
  metric TEXT NOT NULL,
  recorded_at TEXT,
  standard_value REAL NOT NULL,
  standard_unit TEXT NOT NULL,
  scale TEXT NOT NULL,
  provider_record_id TEXT,
  received_at TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE,
  FOREIGN KEY (source_id) REFERENCES sources (record_id)
) STRICT;

CREATE INDEX provider_scores_lookup_idx
  ON provider_scores (metric, source_id, recorded_at);

CREATE TABLE provider_summaries (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'provider_summary'
    CHECK (record_type = 'provider_summary'),
  source_id TEXT NOT NULL,
  summary_type TEXT NOT NULL,
  recorded_at TEXT,
  standard_value REAL NOT NULL,
  standard_unit TEXT NOT NULL,
  provider_record_id TEXT,
  received_at TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE,
  FOREIGN KEY (source_id) REFERENCES sources (record_id)
) STRICT;

CREATE INDEX provider_summaries_lookup_idx
  ON provider_summaries (summary_type, source_id, recorded_at);

CREATE TABLE provider_reporting_periods (
  owner_record_id TEXT NOT NULL,
  period_order INTEGER NOT NULL CHECK (period_order >= 0),
  period_type TEXT NOT NULL,
  provider_period_id TEXT,
  provider_date TEXT,
  start_at TEXT,
  end_at TEXT,
  period_json TEXT NOT NULL CHECK (json_valid(period_json)),
  PRIMARY KEY (owner_record_id, period_order),
  FOREIGN KEY (owner_record_id)
    REFERENCES records (record_id)
    ON DELETE CASCADE
) STRICT;

CREATE INDEX provider_reporting_periods_lookup_idx
  ON provider_reporting_periods (period_type, provider_date, start_at);

CREATE TABLE athlete_reports (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'athlete_report'
    CHECK (record_type = 'athlete_report'),
  report_type TEXT NOT NULL,
  reported_at TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE
) STRICT;

CREATE INDEX athlete_reports_lookup_idx
  ON athlete_reports (report_type, reported_at);

CREATE TABLE nutrition_entries (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'nutrition_entry'
    CHECK (record_type = 'nutrition_entry'),
  reported_at TEXT NOT NULL,
  athlete_description TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE
) STRICT;

CREATE TABLE reported_nutrition_items (
  nutrition_entry_id TEXT NOT NULL,
  item_order INTEGER NOT NULL CHECK (item_order >= 0),
  item_json TEXT NOT NULL CHECK (json_valid(item_json)),
  PRIMARY KEY (nutrition_entry_id, item_order),
  FOREIGN KEY (nutrition_entry_id)
    REFERENCES nutrition_entries (record_id)
    ON DELETE CASCADE
) STRICT;

CREATE TABLE attachments (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'attachment'
    CHECK (record_type = 'attachment'),
  file_name TEXT NOT NULL,
  media_type TEXT NOT NULL,
  byte_count INTEGER NOT NULL CHECK (byte_count >= 0),
  sha256 TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE
) STRICT;

CREATE TABLE record_attachment_links (
  owner_record_id TEXT NOT NULL,
  attachment_order INTEGER NOT NULL CHECK (attachment_order >= 0),
  attachment_record_id TEXT NOT NULL,
  PRIMARY KEY (owner_record_id, attachment_order),
  UNIQUE (owner_record_id, attachment_record_id),
  FOREIGN KEY (owner_record_id)
    REFERENCES records (record_id)
    ON DELETE CASCADE,
  FOREIGN KEY (attachment_record_id)
    REFERENCES attachments (record_id)
) STRICT;

CREATE TABLE benchmark_definitions (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'benchmark_definition'
    CHECK (record_type = 'benchmark_definition'),
  benchmark_key TEXT NOT NULL UNIQUE,
  benchmark_type TEXT NOT NULL,
  score_type TEXT NOT NULL,
  definition TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE
) STRICT;

CREATE TABLE predictions (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'prediction'
    CHECK (record_type = 'prediction'),
  benchmark_definition_id TEXT NOT NULL,
  authored_at TEXT NOT NULL,
  author_type TEXT NOT NULL,
  content_version INTEGER NOT NULL,
  status TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE,
  FOREIGN KEY (benchmark_definition_id)
    REFERENCES benchmark_definitions (record_id)
) STRICT;

CREATE TABLE prediction_result_links (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'prediction_result_link'
    CHECK (record_type = 'prediction_result_link'),
  prediction_id TEXT NOT NULL,
  prediction_revision INTEGER NOT NULL,
  benchmark_result_id TEXT NOT NULL,
  benchmark_result_revision INTEGER NOT NULL,
  linked_at TEXT NOT NULL,
  created_by_type TEXT NOT NULL,
  content_version INTEGER NOT NULL,
  status TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE,
  FOREIGN KEY (prediction_id)
    REFERENCES predictions (record_id),
  FOREIGN KEY (prediction_id, prediction_revision)
    REFERENCES record_revisions (record_id, revision),
  FOREIGN KEY (benchmark_result_id)
    REFERENCES benchmark_results (record_id),
  FOREIGN KEY (benchmark_result_id, benchmark_result_revision)
    REFERENCES record_revisions (record_id, revision)
) STRICT;

CREATE TABLE prediction_grades (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'prediction_grade'
    CHECK (record_type = 'prediction_grade'),
  prediction_result_link_id TEXT NOT NULL,
  prediction_result_link_revision INTEGER NOT NULL,
  authored_at TEXT NOT NULL,
  author_type TEXT NOT NULL,
  content_version INTEGER NOT NULL,
  status TEXT NOT NULL,
  signed_error REAL NOT NULL,
  absolute_error_percentage REAL NOT NULL,
  result_in_predicted_range INTEGER NOT NULL CHECK (result_in_predicted_range IN (0, 1)),
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE,
  FOREIGN KEY (prediction_result_link_id)
    REFERENCES prediction_result_links (record_id),
  FOREIGN KEY (prediction_result_link_id, prediction_result_link_revision)
    REFERENCES record_revisions (record_id, revision)
) STRICT;

CREATE TABLE prediction_error_analyses (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'prediction_error_analysis'
    CHECK (record_type = 'prediction_error_analysis'),
  prediction_grade_id TEXT NOT NULL,
  prediction_grade_revision INTEGER NOT NULL,
  authored_at TEXT NOT NULL,
  author_type TEXT NOT NULL,
  content_version INTEGER NOT NULL,
  status TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE,
  FOREIGN KEY (prediction_grade_id)
    REFERENCES prediction_grades (record_id),
  FOREIGN KEY (prediction_grade_id, prediction_grade_revision)
    REFERENCES record_revisions (record_id, revision)
) STRICT;

CREATE TABLE prediction_error_causes (
  prediction_error_analysis_id TEXT NOT NULL,
  cause_order INTEGER NOT NULL CHECK (cause_order >= 0),
  evidence_record_id TEXT NOT NULL,
  evidence_revision INTEGER NOT NULL,
  explanation TEXT NOT NULL,
  PRIMARY KEY (prediction_error_analysis_id, cause_order),
  FOREIGN KEY (prediction_error_analysis_id)
    REFERENCES prediction_error_analyses (record_id)
    ON DELETE CASCADE,
  FOREIGN KEY (evidence_record_id, evidence_revision)
    REFERENCES record_revisions (record_id, revision)
) STRICT;

CREATE TABLE training_plans (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'training_plan'
    CHECK (record_type = 'training_plan'),
  title TEXT NOT NULL,
  start_date TEXT,
  end_date TEXT,
  authored_at TEXT NOT NULL,
  author_type TEXT NOT NULL,
  content_version INTEGER NOT NULL,
  status TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE
) STRICT;

CREATE TABLE planned_sessions (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'planned_session'
    CHECK (record_type = 'planned_session'),
  training_plan_id TEXT,
  title TEXT NOT NULL,
  scheduled_date TEXT,
  scheduled_start_at TEXT,
  completion_status TEXT NOT NULL,
  completed_workout_session_id TEXT,
  authored_at TEXT NOT NULL,
  author_type TEXT NOT NULL,
  content_version INTEGER NOT NULL,
  status TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE,
  FOREIGN KEY (training_plan_id) REFERENCES training_plans (record_id),
  FOREIGN KEY (completed_workout_session_id) REFERENCES workout_sessions (record_id)
) STRICT;

CREATE TABLE planned_session_blocks (
  planned_session_id TEXT NOT NULL,
  block_order INTEGER NOT NULL CHECK (block_order >= 0),
  block_json TEXT NOT NULL CHECK (json_valid(block_json)),
  PRIMARY KEY (planned_session_id, block_order),
  FOREIGN KEY (planned_session_id)
    REFERENCES planned_sessions (record_id)
    ON DELETE CASCADE
) STRICT;

CREATE TABLE coaching_notes (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'coaching_note'
    CHECK (record_type = 'coaching_note'),
  authored_at TEXT NOT NULL,
  author_type TEXT NOT NULL,
  content_version INTEGER NOT NULL,
  status TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE
) STRICT;

CREATE TABLE saved_training_reviews (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'saved_training_review'
    CHECK (record_type = 'saved_training_review'),
  period_start TEXT NOT NULL,
  period_end TEXT NOT NULL,
  authored_at TEXT NOT NULL,
  author_type TEXT NOT NULL,
  content_version INTEGER NOT NULL,
  status TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE
) STRICT;

CREATE TABLE goals (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'goal' CHECK (record_type = 'goal'),
  title TEXT NOT NULL,
  target_date TEXT,
  target_benchmark_definition_id TEXT,
  authored_at TEXT NOT NULL,
  author_type TEXT NOT NULL,
  content_version INTEGER NOT NULL,
  status TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE,
  FOREIGN KEY (target_benchmark_definition_id)
    REFERENCES benchmark_definitions (record_id)
) STRICT;

CREATE TABLE nutrition_model_interpretations (
  record_id TEXT PRIMARY KEY,
  record_type TEXT NOT NULL DEFAULT 'nutrition_model_interpretation'
    CHECK (record_type = 'nutrition_model_interpretation'),
  nutrition_entry_id TEXT NOT NULL,
  confidence REAL NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
  authored_at TEXT NOT NULL,
  author_type TEXT NOT NULL,
  content_version INTEGER NOT NULL,
  status TEXT NOT NULL,
  data_json TEXT NOT NULL CHECK (json_valid(data_json)),
  FOREIGN KEY (record_id, record_type)
    REFERENCES records (record_id, record_type)
    ON DELETE CASCADE,
  FOREIGN KEY (nutrition_entry_id)
    REFERENCES nutrition_entries (record_id)
) STRICT;

CREATE TABLE authored_record_evidence (
  authored_record_id TEXT NOT NULL,
  evidence_order INTEGER NOT NULL CHECK (evidence_order >= 0),
  evidence_record_id TEXT NOT NULL,
  evidence_revision INTEGER NOT NULL,
  PRIMARY KEY (authored_record_id, evidence_order),
  FOREIGN KEY (authored_record_id)
    REFERENCES records (record_id)
    ON DELETE CASCADE,
  FOREIGN KEY (evidence_record_id, evidence_revision)
    REFERENCES record_revisions (record_id, revision)
) STRICT;

${ENFORCEMENT_TRIGGERS_SQL}
${READ_VIEWS_SQL}
`;
