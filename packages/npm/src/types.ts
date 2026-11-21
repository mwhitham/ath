/**
 * The public types of `athleticstandard/database`, as 2.1.0 declared them.
 *
 * These declarations are the JavaScript contract. The native library fills
 * them; nothing here validates or computes. Types the 2.1.0 package used but
 * did not export by name are declared at the end.
 */
import type { Buffer } from "node:buffer";
import type {
  ATHLETIC_STANDARD_VERSION,
  AthleticStandardPackageT,
  CurrentRecordT,
  EvidenceReferenceT,
  ExportManifestT,
  PointMeasurementType,
  ProviderReportingPeriodT,
  RecordActorT,
  RecordDeletionT,
  RecordErasureMarkerT,
  RecordImportKeyT,
  RecordRevisionT,
  RecordTypeT,
  SeriesQuantity,
} from "./schema.js";
import type { RecordExtensionsT } from "./extensions.js";
import type { NATIVE_CONSTANTS } from "./generated/protocol.js";

export interface AsOfQuery {
  as_of: string;
  record_type?: RecordTypeT;
  record_types?: readonly RecordTypeT[];
  limit?: number;
}

export interface AthleticDatabase {
  readonly url: string;
  readonly isLocal: boolean;
  readonly localPath?: string;
  close(): void;
}

export interface BacktestReplay {
  model: string;
  benchmark_result_id: string;
  benchmark_key: string;
  as_of: string;
  predicted_value?: number;
  actual_value: number;
  absolute_error_percentage?: number;
  error?: string;
}

export interface BaselineStats {
  source_id: string;
  quantity: PointMeasurementType;
  unit: string;
  mean: number;
  standard_deviation: number;
  observation_count: number;
  from: string;
  to: string;
  window_days: 90;
  method: "sample_mean_and_standard_deviation";
}

export interface CalculateOptions {
  operation: CalculationOperation;
  quantity: string;
  record_kind?: CalculationRecordKind;
  provider_period_type?: ProviderPeriodType;
  source_id?: string;
  compare_source_id?: string;
  other_quantity?: string;
  workout_session_id?: string;
  from?: string;
  to?: string;
  time_zone?: string;
  group?: CalculationGroup;
}

export type CalculationGroup = "none" | "day" | "week" | "month";

export type CalculationOperation = (typeof NATIVE_CONSTANTS.CALCULATION_OPERATIONS)[number];

export type CalculationRecordKind = (typeof NATIVE_CONSTANTS.CALCULATION_RECORD_KINDS)[number];

export interface CalculationResult {
  /** 2.1.2 writes `ath-runtime-v2`, which adds result fields (D244). */
  calculation_version: "ath-runtime-v1" | "ath-runtime-v2";
  operation: CalculationOperation;
  record_kind: CalculationRecordKind;
  provider_period_type?: ProviderPeriodType;
  method: string;
  quantity: string;
  unit: string;
  range: {
    from?: string;
    to?: string;
    time_zone?: string;
  };
  sources: string[];
  input: {
    record_count: number;
    record_references: Array<{ record_id: string; revision: number }>;
    references_truncated: boolean;
  };
  exclusions: string[];
  result: unknown;
}

export interface ConfirmedMigrationResult {
  backup_path: string | null;
  migrated: boolean;
  preview: MigrationPreview;
}

export interface ConfirmedProviderRepairResult {
  backup_path: string;
  repaired_record_count: number;
  revision_align_record_count: number;
  database_revision: number;
}

export interface CorrectRecordInput {
  record_id: string;
  expected_revision: number;
  data: unknown;
  extensions?: RecordExtensionsT;
  changed_by: RecordActorT;
  became_known_at?: string;
  became_known_at_basis?: "local_write" | "provider_import" | "package_merge";
}

export interface DatabaseCheckIssue {
  severity: "error" | "warning";
  check: string;
  message: string;
  record_id?: string;
}

export interface DatabaseCheckResult {
  metadata: DatabaseMetadata | null;
  valid: boolean;
  record_count: number;
  issues: DatabaseCheckIssue[];
}

export interface DatabaseMetadata {
  application_marker: typeof NATIVE_CONSTANTS.SQLITE_APPLICATION_MARKER;
  sql_schema_version: number;
  athleticstandard_version: string;
  database_id: string;
  created_at: string;
}

export interface DatabaseStats {
  athleticstandard_version: string;
  database_id: string;
  database_revision: number;
  athlete: Record<string, unknown> | null;
  current_records: {
    total: number;
    by_type: Record<string, number>;
  };
  sources: SourceStats[];
  baselines: BaselineStats[];
  series: SeriesCoverageStats[];
  benchmarks: {
    defined: number;
    results: number;
  };
  predictions: {
    total: number;
    graded: number;
    by_author: Array<{
      author: string;
      predictions: number;
    }>;
  };
}

export interface DeleteRecordInput {
  record_id: string;
  expected_revision: number;
  deleted_by: RecordActorT;
  deleted_at?: string;
}

export interface EraseRecordInput {
  preview: ErasurePreview;
  reason_code: "athlete_request" | "legal_requirement" | "recorded_in_error" | "other";
  erased_at?: string;
}

export interface ErasureAttachment {
  record_id: string;
  path: string;
  present: boolean;
}

export interface ErasurePreview {
  root_record_id: string;
  root_expected_revision: number;
  database_revision: number;
  records: ErasureRecord[];
  attachments: ErasureAttachment[];
}

export interface ErasureRecord {
  record_id: string;
  record_type: CurrentRecordT["record_type"];
  current_revision: number;
}

export type EvidenceReferenceResolution =
  | {
      status: "available";
      reference: EvidenceReferenceT;
      revision: RecordRevisionT;
    }
  | {
      status: "erased";
      reference: EvidenceReferenceT;
      erasure_marker: RecordErasureMarkerT;
    }
  | {
      status: "missing";
      reference: EvidenceReferenceT;
    };

export interface ExpectedCurrentRevision {
  record_id: string;
  revision: number;
}

export interface ExportDatabaseOptions {
  exported_at?: string;
}

export interface ExportDatabaseResult {
  directory: string;
  database_id: string;
  database_revision: number;
  manifest: ExportManifestT;
}

export interface ListRecordOptions {
  record_type?: RecordTypeT;
  include_deleted?: boolean;
  limit?: number;
}

export interface LoadedPackage {
  package: AthleticStandardPackageT;
  series_files: Array<{
    reference: AthleticStandardPackageT["measurement_series_files"][number];
    path: string;
  }>;
  /** Non-sample records held in athlete.ath.json. Series records are loaded one file at a time. */
  all_records: CurrentRecordT[];
  /** Non-sample revisions held in athlete.ath.json. */
  all_revisions: RecordRevisionT[];
  /** Non-sample deletions held in athlete.ath.json. */
  all_deletions: RecordDeletionT[];
  /** Non-sample import keys held in athlete.ath.json. */
  all_import_keys: RecordImportKeyT[];
  sample_record_id_examples: string[];
  source_path: string;
  source_fingerprint: string;
  source_directory: string;
  attachment_files: Map<string, string>;
}

export interface LogPlan {
  inputs: WriteRecordInput[];
  expected_current_revisions: ExpectedCurrentRevision[];
  interpretation: string;
}

export interface MeasurementQuery extends TimeRange {
  quantity?: PointMeasurementType;
  source_id?: string;
  include_manual?: boolean;
  limit?: number;
}

export interface MigrationPreview {
  migration_needed: boolean;
  source_sql_schema_version: number;
  target_sql_schema_version: number;
  database_id: string;
  planned_steps: MigrationStepPreview[];
  planned_backup_path: string | null;
  compatibility_effect: string;
}

export interface MigrationStepPreview {
  target_sql_schema_version: number;
  migration_file: string;
  description: string;
}

export interface OpenDatabaseOptions {
  onSecurityWarning?: (message: string) => void;
  /** When false, refuse to open a database that still needs a SQL schema migration. */
  allowOutdatedSchema?: boolean;
  /** Do not switch the local database to WAL. Use for migration preview and pre-backup opens. */
  deferWal?: boolean;
}

export type PackageConflictSet = {
  conflict_set_version: 1;
  source_path: string;
  source_fingerprint: string;
  source_database_id: string;
  source_database_revision: number;
  destination_database_id: string;
  destination_database_revision: number;
  conflicts: {
    record_id: string;
    message: string;
    resolution?: { choice: "local" } | { choice: "imported" } | { choice: "corrected"; data: unknown } | undefined;
  }[];
};

export interface PackageImportConflict {
  record_id?: string;
  message: string;
}

export interface PackageImportPreview {
  source_database_id: string;
  source_database_revision: number;
  destination_database_id: string;
  destination_database_revision: number;
  restore: boolean;
  new_records: string[];
  new_record_count: number;
  matching_records: string[];
  matching_record_count: number;
  changed_records: string[];
  changed_record_count: number;
  record_lists_truncated: boolean;
  conflicts: PackageImportConflict[];
  unrepresentable_records: string[];
  unknown_fields: string[];
  loaded: LoadedPackage;
}

export interface PackageImportResolution {
  record_id: string;
  choice: "local" | "imported" | "corrected";
  data?: unknown;
}

export interface PredictionEvidence {
  purpose: "prediction_evidence";
  is_prediction: false;
  as_of: string;
  benchmark: Extract<CurrentRecordT, { record_type: "benchmark_definition" }>;
  prior_results: Extract<CurrentRecordT, { record_type: "benchmark_result" }>[];
  device_measurements: Extract<
    CurrentRecordT,
    { record_type: "point_measurement" }
  >[];
  athlete_reports: Extract<CurrentRecordT, { record_type: "athlete_report" }>[];
  sleep_sessions: Extract<CurrentRecordT, { record_type: "sleep_session" }>[];
  workout_sessions: Extract<
    CurrentRecordT,
    { record_type: "workout_session" }
  >[];
  prior_predictions: Extract<CurrentRecordT, { record_type: "prediction" }>[];
  gaps: string[];
}

export interface PredictionGradePlan {
  prediction: Extract<CurrentRecordT, { record_type: "prediction" }>;
  result: Extract<CurrentRecordT, { record_type: "benchmark_result" }>;
  existing_link: Extract<
    CurrentRecordT,
    { record_type: "prediction_result_link" }
  > | null;
  link_input: WriteRecordInput | null;
  link_correction: CorrectRecordInput | null;
  grade_input: WriteRecordInput;
}

export interface PreparedAttachment {
  input: WriteRecordInput;
  bytes: Buffer;
  final_path: string;
}

export interface ProviderEncodingRepairResult {
  repaired_record_count: number;
  database_revision: number;
}

export interface ProviderImportConflict {
  match_key: string;
  record_id?: string;
  message: string;
}

export interface ProviderImportPreview {
  provider: ProviderExportFormat;
  label: string;
  database_revision: number;
  new_records: Array<{ record_id: string; record_type: RecordTypeT }>;
  matching_records: Array<{ record_id: string; record_type: RecordTypeT }>;
  changed_records: Array<{
    record_id: string;
    record_type: RecordTypeT;
    current_revision: number;
  }>;
  conflicts: ProviderImportConflict[];
  skipped: Record<string, { count: number; example?: string }>;
  plan: ProviderImportPlan;
  changes: ProviderImportChange[];
}

export type ProviderPeriodType = ProviderReportingPeriodT["period_type"];

export interface RecordLink {
  direction: "outgoing" | "incoming";
  relation: string;
  record_id: string;
  revision?: number;
}

export interface SeriesCoverageStats {
  source_id: string;
  quantity: string;
  unit: string;
  sample_count: number;
  from: string;
  to: string;
}

export interface SeriesDay {
  date: string;
  source_id: string;
  sample_count: number;
  minimum: number;
  maximum: number;
  mean: number;
  first_at: string;
  last_at: string;
}

export interface SeriesQuery extends TimeRange {
  quantity: SeriesQuantity;
  source_id?: string;
  include_manual?: boolean;
  limit?: number;
  order?: "ascending" | "descending";
}

export interface SeriesView {
  quantity: SeriesQuantity;
  unit: string;
  time_zone?: string;
  from?: string;
  to?: string;
  source_id?: string;
  truncated: boolean;
  samples?: CurrentRecordT[];
  days?: SeriesDay[];
}

export interface SeriesViewOptions {
  from?: string;
  to?: string;
  source_id?: string;
  time_zone?: string;
  raw?: boolean;
  limit?: number;
}

export interface SessionQuery extends TimeRange {
  session_type?: "sleep_session" | "workout_session";
  source_id?: string;
  include_manual?: boolean;
  limit?: number;
}

export interface SourceStats {
  record_id: string;
  source_type: string;
  data_writer: string;
  data_route: string;
  vendor: string | null;
  sensor_type: string | null;
  current_record_count: number;
  first_recorded_at: string | null;
  last_recorded_at: string | null;
}

export interface StructuredLogInput {
  record_type: string;
  data: unknown;
  record_id?: string;
  created_at?: string;
  current_revision?: number;
  is_deleted?: boolean;
  extensions?: RecordExtensionsT;
}

export interface TimeRange {
  from?: string;
  to?: string;
}

export interface V1BacktestReport {
  athleticstandard_version: typeof ATHLETIC_STANDARD_VERSION;
  created_at: string;
  models: string[];
  replayable_result_count: number;
  skipped_first_result_count: number;
  replays: BacktestReplay[];
  summary: Array<{
    model: string;
    completed: number;
    failed: number;
    median_absolute_error_percentage: number | null;
  }>;
}

export interface WriteRecordChangesInput extends WriteRecordsOptions {
  corrections?: readonly CorrectRecordInput[];
  records?: readonly WriteRecordInput[];
}

export interface WriteRecordInput {
  record_type: RecordTypeT;
  data: unknown;
  changed_by: RecordActorT;
  extensions?: RecordExtensionsT;
  record_id?: string;
  created_at?: string;
  became_known_at?: string;
  became_known_at_basis?: "local_write" | "provider_import";
}

export interface WriteRecordsOptions {
  expected_current_revisions?: readonly ExpectedCurrentRevision[];
}

/** The provider routes `previewProviderImport` reads. */
export type ProviderExportFormat = "apple" | "whoop" | "oura";

/** One provider record a preview plans to write. */
export interface ProviderRecordDraft {
  record_id: string;
  record_type: RecordTypeT;
  data: unknown;
  match_key_method: string;
  match_key: string;
  collision_group?: string;
  existing_record_id?: string;
}

/** The records a provider preview plans to write. */
export interface ProviderImportPlan {
  drafts: ProviderRecordDraft[];
  received_at: string;
}

/** What a provider preview decided for one draft. */
export interface ProviderImportChange {
  draft: ProviderRecordDraft;
  current?: CurrentRecordT;
  action: "new" | "matching" | "changed";
}

/** A calendar range resolved to UTC instants. */
export interface InstantRange {
  from?: string;
  to?: string;
  time_zone?: string;
}

/** Options for `executeConfirmedProviderEncodingRepair`. */
export interface ExecuteConfirmedRepairOptions {
  backupPath?: string;
  now?: Date;
  fromRevision?: boolean;
}

/** One model completion request a backtest makes. */
export interface CompleteRequest {
  gateway: "vercel" | "openrouter";
  model: string;
  system: string;
  user: string;
}

/** A model's reply to one completion request. */
export interface CompleteResponse {
  content: string;
  model: string;
}
