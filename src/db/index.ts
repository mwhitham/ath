export {
  SQL_SCHEMA_VERSION,
  SQLITE_APPLICATION_ID,
  SQLITE_APPLICATION_MARKER,
  type AthleticDatabase,
} from "./client.js";
export {
  createVerifiedSqliteBackup,
  defaultMigrationBackupPath,
  sqlite3BackupDotCommand,
} from "./backup.js";
export {
  applyPendingDatabaseMigrations,
  buildMigrationPreview,
  createDatabase,
  databaseNeedsMigration,
  executeConfirmedDatabaseMigration,
  migrateDatabase,
  MigrationRequiredError,
  inspectDatabase,
  openDatabase,
  readDatabaseMetadata,
  type ConfirmedMigrationResult,
  type DatabaseMetadata,
  type MigrationPreview,
  type MigrationStepPreview,
  type OpenDatabaseOptions,
} from "./migrate.js";
export {
  correctRecord,
  deleteRecord,
  readRecord as getRecord,
  listRecords,
  readRecord,
  readRecordRevisions,
  writeRecord,
  writeRecordChanges,
  writeRecords,
  type CorrectRecordInput,
  type DeleteRecordInput,
  type ExpectedCurrentRevision,
  type ListRecordOptions,
  type WriteRecordInput,
  type WriteRecordChangesInput,
  type WriteRecordsOptions,
} from "./records.js";
export {
  queryMeasurements,
  queryRecordsAsOf,
  querySeries,
  querySessions,
  type AsOfQuery,
  type MeasurementQuery,
  type SeriesQuery,
  type SessionQuery,
  type TimeRange,
} from "./query.js";
export { createUuidV7 } from "./uuid.js";
export {
  checkDatabase,
  type DatabaseCheckIssue,
  type DatabaseCheckResult,
} from "./check.js";
export {
  countProviderEncodingRepairCandidates,
  repairProviderEncoding,
  type ProviderEncodingRepairResult,
} from "./repair.js";
export {
  databaseStats,
  renderDatabaseStats,
  type BaselineStats,
  type DatabaseStats,
  type SeriesCoverageStats,
  type SourceStats,
} from "./stats.js";
export {
  calendarDateInZone,
  readSeriesView,
  resolveCalendarRange,
  resolveAsOfTime,
  renderSeriesView,
  type SeriesDay,
  type SeriesView,
  type SeriesViewOptions,
} from "./series-view.js";
export {
  readRecordLinks,
  renderRecord,
  renderRecordList,
  type RecordLink,
} from "./records-view.js";
export {
  applyLogPlan,
  planLogEntry,
  type LogPlan,
  type StructuredLogInput,
} from "./log-entry.js";
export {
  gradePrediction,
  planPredictionGrade,
  type PredictionGradePlan,
} from "./prediction-grade.js";
export {
  buildEvidence,
  type PredictionEvidence,
} from "./evidence.js";
export {
  runV1Backtest,
  writeV1BacktestReport,
  type BacktestReplay,
  type V1BacktestReport,
} from "./backtest.js";
export {
  deterministicJson,
  exportRecords,
  type ExportDatabaseOptions,
  type ExportDatabaseResult,
} from "./export.js";
export {
  importRecords,
  loadAthleticStandardPackage,
  PackageRepresentationError,
  previewPackageImport,
  readImportedPackageRecord,
  type LoadedPackage,
  type PackageImportConflict,
  type PackageImportPreview,
  type PackageImportResolution,
} from "./import-package.js";
export {
  assertPackageConflictSetCurrent,
  clearPackageConflictSet,
  conflictResolutions,
  packageConflictSetPath,
  previewConflict,
  readPackageConflictSet,
  resolveConflict,
  resolvePackageConflict,
  savePackageConflictSet,
  type PackageConflictSet,
} from "./conflicts.js";
export {
  eraseRecord,
  previewRecordErasure,
  type EraseRecordInput,
  type ErasureAttachment,
  type ErasurePreview,
  type ErasureRecord,
} from "./erase.js";
export {
  resolveEvidenceReference,
  type EvidenceReferenceResolution,
} from "./evidence-reference.js";
export {
  importProviderRecords,
  previewProviderImport,
  type ProviderImportConflict,
  type ProviderImportPreview,
} from "./provider-import.js";
export {
  CALCULATION_OPERATIONS,
  CALCULATION_RECORD_KINDS,
  calculate,
  renderCalculation,
  type CalculateOptions,
  type CalculationGroup,
  type CalculationOperation,
  type CalculationRecordKind,
  type CalculationResult,
  type ProviderPeriodType,
} from "./calculate.js";
export {
  addAttachmentsToLogPlan,
  MAX_ATTACHMENT_BYTES,
  prepareAttachments,
  withPreparedAttachmentFiles,
  type PreparedAttachment,
} from "./attachment-ingest.js";
