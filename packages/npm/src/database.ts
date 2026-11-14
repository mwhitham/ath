/**
 * `athleticstandard/database`: the 2.1.0 database exports over the native
 * library.
 *
 * Every function keeps its 2.1.0 name, signature, and result shape. The work
 * runs in the native `ath` executable through the client protocol: one worker
 * per call, which opens and checks the database file again. This module
 * holds no SQL, import rule, arithmetic, or record validation. Where 2.1.0
 * composed other public functions, such as `applyLogPlan`, this module makes
 * the same composition.
 */
import { AsyncLocalStorage } from "node:async_hooks";
import { Buffer } from "node:buffer";
import { mkdtemp, realpath, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { NATIVE_CONSTANTS, type AttachmentFile } from "./generated/protocol.js";
import { defined, itemsOf, onDatabase, outputOf } from "./client/call.js";
import {
  MigrationRequiredError,
  PackageRepresentationError,
} from "./client/errors.js";
import {
  isOpen,
  makeHandle,
  markSkipCheckpoint,
  parseDatabaseTarget,
  skipsCheckpoint,
  targetOf,
} from "./client/handle.js";
import { remoteTargetRefusal } from "./client/remote.js";
import { callAsync, callSync } from "./client/worker.js";
import type {
  CurrentRecordT,
  EvidenceReferenceT,
  RecordActorT,
  RecordRevisionT,
  SeriesQuantity,
} from "./schema.js";
import type {
  AsOfQuery,
  AthleticDatabase,
  BacktestReplay,
  CalculateOptions,
  CalculationResult,
  CompleteRequest,
  CompleteResponse,
  ConfirmedMigrationResult,
  ConfirmedProviderRepairResult,
  CorrectRecordInput,
  DatabaseCheckResult,
  DatabaseMetadata,
  DatabaseStats,
  DeleteRecordInput,
  EraseRecordInput,
  ErasurePreview,
  EvidenceReferenceResolution,
  ExecuteConfirmedRepairOptions,
  ExportDatabaseOptions,
  ExportDatabaseResult,
  InstantRange,
  ListRecordOptions,
  LoadedPackage,
  LogPlan,
  MeasurementQuery,
  MigrationPreview,
  OpenDatabaseOptions,
  PackageConflictSet,
  PackageImportPreview,
  PackageImportResolution,
  PredictionEvidence,
  PredictionGradePlan,
  PreparedAttachment,
  ProviderEncodingRepairResult,
  ProviderImportChange,
  ProviderImportPreview,
  ProviderRecordDraft,
  RecordLink,
  SeriesQuery,
  SeriesView,
  SeriesViewOptions,
  SessionQuery,
  V1BacktestReport,
  WriteRecordChangesInput,
  WriteRecordInput,
  WriteRecordsOptions,
} from "./types.js";

export type {
  AsOfQuery,
  AthleticDatabase,
  BacktestReplay,
  BaselineStats,
  CalculateOptions,
  CalculationGroup,
  CalculationOperation,
  CalculationRecordKind,
  CalculationResult,
  ConfirmedMigrationResult,
  ConfirmedProviderRepairResult,
  CorrectRecordInput,
  DatabaseCheckIssue,
  DatabaseCheckResult,
  DatabaseMetadata,
  DatabaseStats,
  DeleteRecordInput,
  EraseRecordInput,
  ErasureAttachment,
  ErasurePreview,
  ErasureRecord,
  EvidenceReferenceResolution,
  ExpectedCurrentRevision,
  ExportDatabaseOptions,
  ExportDatabaseResult,
  ListRecordOptions,
  LoadedPackage,
  LogPlan,
  MeasurementQuery,
  MigrationPreview,
  MigrationStepPreview,
  OpenDatabaseOptions,
  PackageConflictSet,
  PackageImportConflict,
  PackageImportPreview,
  PackageImportResolution,
  PredictionEvidence,
  PredictionGradePlan,
  PreparedAttachment,
  ProviderEncodingRepairResult,
  ProviderImportConflict,
  ProviderImportPreview,
  ProviderPeriodType,
  RecordLink,
  SeriesCoverageStats,
  SeriesDay,
  SeriesQuery,
  SeriesView,
  SeriesViewOptions,
  SessionQuery,
  SourceStats,
  StructuredLogInput,
  TimeRange,
  V1BacktestReport,
  WriteRecordChangesInput,
  WriteRecordInput,
  WriteRecordsOptions,
} from "./types.js";
export { MigrationRequiredError, PackageRepresentationError };

// ---- Constants --------------------------------------------------------------

export const SQL_SCHEMA_VERSION = NATIVE_CONSTANTS.SQL_SCHEMA_VERSION;
export const SQLITE_APPLICATION_ID = NATIVE_CONSTANTS.SQLITE_APPLICATION_ID;
export const SQLITE_APPLICATION_MARKER = NATIVE_CONSTANTS.SQLITE_APPLICATION_MARKER;
export const MAX_ATTACHMENT_BYTES: number = NATIVE_CONSTANTS.MAX_ATTACHMENT_BYTES;
export const CALCULATION_OPERATIONS = NATIVE_CONSTANTS.CALCULATION_OPERATIONS;
export const CALCULATION_RECORD_KINDS = NATIVE_CONSTANTS.CALCULATION_RECORD_KINDS;

// ---- Handles and temporary folders -----------------------------------------

/** Each worker starts fresh, so the backup-name counter 2.1.0 kept in its process lives here. */
let backupNameSequence = 0;

function nextBackupSequence(): number {
  backupNameSequence = (backupNameSequence + 1) % 10_000;
  return backupNameSequence;
}

function warnAll(warnings: readonly string[], warn: (message: string) => void): void {
  for (const warning of warnings) {
    warn(warning);
  }
}

export async function createDatabase(
  target: string,
  options: { databaseId?: string; createdAt?: string } = {},
): Promise<AthleticDatabase> {
  const parsed = parseDatabaseTarget(target);
  if (parsed.localPath === undefined) {
    // 2.1.0 checked the options before it connected.
    if (options.databaseId !== undefined) {
      callSync("model.parse", { schema: "DatabaseId", value: options.databaseId });
    }
    if (options.createdAt !== undefined) {
      callSync("model.parse", { schema: "UtcTimestamp", value: options.createdAt });
    }
    throw remoteTargetRefusal(parsed.url);
  }
  const created = await callAsync(
    "database.create",
    defined({
      path: parsed.localPath,
      database_id: options.databaseId,
      created_at: options.createdAt,
    }),
  );
  return makeHandle(parsed.localPath, created.output.database_id, {
    allowOutdatedSchema: false,
    skipCheckpointOnRelease: false,
  });
}

export async function openDatabase(
  target: string,
  options: OpenDatabaseOptions = {},
): Promise<AthleticDatabase> {
  const parsed = parseDatabaseTarget(target);
  if (parsed.localPath === undefined) {
    throw remoteTargetRefusal(parsed.url);
  }
  const opened = await callAsync("database.open", {
    path: parsed.localPath,
    allow_outdated_schema: options.allowOutdatedSchema === true,
    defer_wal: options.deferWal === true,
    now_iso: new Date().toISOString(),
    sequence: nextBackupSequence(),
  });
  warnAll(opened.output.warnings, options.onSecurityWarning ?? console.warn);
  return makeHandle(parsed.localPath, opened.output.database_id, {
    allowOutdatedSchema: options.allowOutdatedSchema === true,
    skipCheckpointOnRelease: false,
  });
}

/** Read-only inspection open for migration previews. Never enables WAL or upgrades journal mode. */
export async function inspectDatabase(
  target: string,
  options: Pick<OpenDatabaseOptions, "onSecurityWarning"> = {},
): Promise<AthleticDatabase> {
  const parsed = parseDatabaseTarget(target);
  if (parsed.localPath === undefined) {
    throw remoteTargetRefusal(parsed.url);
  }
  const inspected = await callAsync("database.inspect", { path: parsed.localPath });
  warnAll(inspected.output.warnings, options.onSecurityWarning ?? console.warn);
  return makeHandle(parsed.localPath, inspected.output.database_id, {
    allowOutdatedSchema: true,
    skipCheckpointOnRelease: true,
  });
}

export function markSkipWalCheckpointOnRelease(database: AthleticDatabase): void {
  markSkipCheckpoint(database);
}

/** Close a local database and truncate WAL sidecars when possible. */
export async function releaseLocalDatabase(database: AthleticDatabase): Promise<void> {
  if (!database.isLocal) {
    database.close();
    return;
  }
  if (isOpen(database) && !skipsCheckpoint(database)) {
    try {
      await onDatabase(database, "database.release", {});
    } catch {
      // The file may be read-only or already gone; 2.1.0 ignored this too.
    }
  }
  database.close();
}

/**
 * Make a private folder for this program's own temporary files. The system
 * temp folder can sit behind an operating-system symlink, such as /var on
 * macOS, so the folder is made under its real path.
 */
export async function makeTemporaryDirectory(prefix: string): Promise<string> {
  return mkdtemp(join(await realpath(tmpdir()), prefix));
}

/** Remove a temporary folder, retrying while another process lets go of its files. */
export async function removeTemporaryDirectory(path: string): Promise<void> {
  (globalThis as { gc?: () => void }).gc?.();
  await new Promise((resolve) => setImmediate(resolve));
  await rm(path, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
}

// ---- Backups and migration --------------------------------------------------

export function formatMigrationBackupTimestamp(date: Date): string {
  return callSync("backup.timestamp", { iso: date.toISOString() });
}

export function defaultMigrationBackupPath(databasePath: string, now = new Date()): string {
  return callSync("backup.default_path", {
    database_path: databasePath,
    now_iso: now.toISOString(),
    sequence: nextBackupSequence(),
  });
}

/** Dot-command argument for sqlite3 `.backup`, using double-quoted paths with sqlite3 backslash escapes. */
export function sqlite3BackupDotCommand(backupPath: string): string {
  return callSync("backup.sqlite3_command", { backup_path: backupPath });
}

export async function createVerifiedSqliteBackup(
  sourceDatabase: AthleticDatabase,
  sourcePath: string,
  backupPath: string,
  sourceMetadata: DatabaseMetadata,
): Promise<void> {
  if (!sourceDatabase.isLocal) {
    throw new Error("SQL schema migration backup requires a local database file");
  }
  targetOf(sourceDatabase);
  await callAsync("database.backup", {
    source_path: sourcePath,
    backup_path: backupPath,
    source_metadata: sourceMetadata,
  });
}

export function databaseNeedsMigration(metadata: DatabaseMetadata): boolean {
  return callSync("migration.needs_migration", { metadata });
}

export function buildMigrationPreview(
  metadata: DatabaseMetadata,
  localPath?: string,
  now = new Date(),
): MigrationPreview {
  return callSync(
    "migration.preview",
    defined({
      metadata,
      local_path: localPath,
      now_iso: now.toISOString(),
      sequence: nextBackupSequence(),
    }),
  );
}

export async function readDatabaseMetadata(database: AthleticDatabase): Promise<DatabaseMetadata> {
  return outputOf(database, "database.metadata", {});
}

export async function applyPendingDatabaseMigrations(database: AthleticDatabase): Promise<void> {
  await onDatabase(database, "database.apply_migrations", {});
}

/** Applies pending SQL schema migrations. Use only through confirmed migration flows. */
export async function migrateDatabase(database: AthleticDatabase): Promise<void> {
  await applyPendingDatabaseMigrations(database);
}

export async function executeConfirmedDatabaseMigration(
  database: AthleticDatabase,
  options: { backupPath?: string; now?: Date } = {},
): Promise<ConfirmedMigrationResult> {
  return outputOf(
    database,
    "database.migrate_confirmed",
    defined({
      backup_path: options.backupPath,
      now_iso: (options.now ?? new Date()).toISOString(),
      sequence: nextBackupSequence(),
    }),
  );
}

// ---- Records ----------------------------------------------------------------

/** Attachment files prepared for the write that runs inside withPreparedAttachmentFiles. */
interface AttachmentScope {
  database: AthleticDatabase;
  files: Map<string, AttachmentFile>;
}

const attachmentScope = new AsyncLocalStorage<AttachmentScope>();

/** The prepared files whose attachment records this write creates. */
function attachmentFilesFor(
  database: AthleticDatabase,
  inputs: readonly WriteRecordInput[],
): AttachmentFile[] | undefined {
  const scope = attachmentScope.getStore();
  if (scope === undefined || scope.database !== database) {
    return undefined;
  }
  const files: AttachmentFile[] = [];
  for (const input of inputs) {
    const file = input.record_id === undefined ? undefined : scope.files.get(input.record_id);
    if (input.record_type === "attachment" && file !== undefined) {
      files.push(file);
      scope.files.delete(input.record_id!);
    }
  }
  return files.length === 0 ? undefined : files;
}

export async function writeRecords(
  database: AthleticDatabase,
  inputs: readonly WriteRecordInput[],
  options: WriteRecordsOptions = {},
): Promise<{ records: CurrentRecordT[]; database_revision: number }> {
  targetOf(database);
  return outputOf(
    database,
    "records.write",
    defined({
      inputs,
      expected_current_revisions: options.expected_current_revisions,
      attachments: attachmentFilesFor(database, inputs),
    }),
  );
}

export async function writeRecord(
  database: AthleticDatabase,
  input: WriteRecordInput,
): Promise<{ record: CurrentRecordT; database_revision: number }> {
  const result = await writeRecords(database, [input]);
  return { record: result.records[0]!, database_revision: result.database_revision };
}

export async function writeRecordChanges(
  database: AthleticDatabase,
  input: WriteRecordChangesInput,
): Promise<{ records: CurrentRecordT[]; database_revision: number }> {
  targetOf(database);
  return outputOf(
    database,
    "records.write_changes",
    defined({
      corrections: input.corrections,
      records: input.records,
      expected_current_revisions: input.expected_current_revisions,
      attachments: attachmentFilesFor(database, input.records ?? []),
    }),
  );
}

export async function correctRecord(
  database: AthleticDatabase,
  input: CorrectRecordInput,
): Promise<{ record: CurrentRecordT; database_revision: number }> {
  return outputOf(database, "records.correct", input);
}

export async function deleteRecord(
  database: AthleticDatabase,
  input: DeleteRecordInput,
): Promise<{ record_id: string; record_revision: number; database_revision: number }> {
  return outputOf(database, "records.delete", input);
}

export async function readRecord(
  database: AthleticDatabase,
  recordId: string,
): Promise<CurrentRecordT | null> {
  return outputOf(database, "records.read", { record_id: recordId });
}

export { readRecord as getRecord };

export async function listRecords(
  database: AthleticDatabase,
  options: ListRecordOptions = {},
): Promise<CurrentRecordT[]> {
  return itemsOf(database, "records.list", options);
}

export async function readRecordRevisions(
  database: AthleticDatabase,
  recordId: string,
): Promise<RecordRevisionT[]> {
  return itemsOf(database, "records.revisions", { record_id: recordId });
}

export async function readRecordLinks(
  database: AthleticDatabase,
  recordId: string,
): Promise<RecordLink[]> {
  return outputOf(database, "records.links", { record_id: recordId });
}

export function renderRecord(
  record: CurrentRecordT,
  options: {
    revision?: RecordRevisionT;
    history?: readonly RecordRevisionT[];
    links?: readonly RecordLink[];
  } = {},
): string {
  return callSync(
    "render.record",
    defined({ record, revision: options.revision, history: options.history, links: options.links }),
  );
}

export function renderRecordList(records: readonly CurrentRecordT[]): string {
  return callSync("render.record_list", { records });
}

export function createUuidV7(now?: number): string {
  if (now === undefined) {
    return callSync("ids.uuid_v7", {});
  }
  // JSON has no NaN or Infinity; text keeps them distinct from a missing time.
  return callSync("ids.uuid_v7", { now: Number.isFinite(now) ? now : String(now) });
}

// ---- Queries and views ------------------------------------------------------

export async function queryMeasurements(
  database: AthleticDatabase,
  query: MeasurementQuery = {},
): Promise<CurrentRecordT[]> {
  return itemsOf(database, "query.measurements", { query });
}

export async function querySeries(
  database: AthleticDatabase,
  query: SeriesQuery,
): Promise<CurrentRecordT[]> {
  return itemsOf(database, "query.series", { query });
}

export async function querySessions(
  database: AthleticDatabase,
  query: SessionQuery = {},
): Promise<CurrentRecordT[]> {
  return itemsOf(database, "query.sessions", { query });
}

export async function queryRecordsAsOf(
  database: AthleticDatabase,
  query: AsOfQuery,
): Promise<CurrentRecordT[]> {
  return itemsOf(database, "query.as_of", { query });
}

export function calendarDateInZone(timestamp: string, timeZone: string): string {
  return callSync("calendar.date_in_zone", { timestamp, time_zone: timeZone });
}

export async function readSeriesView(
  database: AthleticDatabase,
  quantity: SeriesQuantity,
  options: SeriesViewOptions = {},
): Promise<SeriesView> {
  return outputOf(database, "view.series", { quantity, options });
}

export async function resolveCalendarRange(
  database: AthleticDatabase,
  options: SeriesViewOptions,
): Promise<InstantRange> {
  return outputOf(database, "view.calendar_range", { options });
}

export async function resolveAsOfTime(
  database: AthleticDatabase,
  value: string | undefined,
  explicitTimeZone?: string,
): Promise<{ as_of: string; time_zone?: string }> {
  return outputOf(database, "view.as_of_time", defined({ value, time_zone: explicitTimeZone }));
}

export function renderSeriesView(view: SeriesView): string {
  return callSync("render.series_view", { view });
}

// ---- Checks, statistics, and repair ------------------------------------------

export async function checkDatabase(database: AthleticDatabase): Promise<DatabaseCheckResult> {
  return outputOf(database, "database.check", {});
}

export async function databaseStats(database: AthleticDatabase): Promise<DatabaseStats> {
  return outputOf(database, "database.stats", {});
}

export function renderDatabaseStats(stats: DatabaseStats): string {
  return callSync("render.stats", { stats });
}

export async function countProviderEncodingRepairCandidates(
  database: AthleticDatabase,
): Promise<number> {
  return (await outputOf(database, "repair.candidates", {})).encoding_record_count;
}

export async function countRevisionAlignRepairCandidates(
  database: AthleticDatabase,
): Promise<number> {
  return (await outputOf(database, "repair.candidates", {})).revision_align_record_ids.length;
}

export async function listRevisionAlignRepairRecordIds(
  database: AthleticDatabase,
): Promise<string[]> {
  return (await outputOf(database, "repair.candidates", {})).revision_align_record_ids;
}

export async function repairProviderEncoding(
  database: AthleticDatabase,
): Promise<ProviderEncodingRepairResult> {
  return outputOf(database, "repair.provider_encoding", {});
}

export async function executeConfirmedProviderEncodingRepair(
  database: AthleticDatabase,
  options: ExecuteConfirmedRepairOptions = {},
): Promise<ConfirmedProviderRepairResult> {
  return outputOf(
    database,
    "repair.confirmed",
    defined({
      backup_path: options.backupPath,
      now_ms: options.now?.getTime(),
      from_revision: options.fromRevision,
    }),
  );
}

// ---- Calculations -------------------------------------------------------------

export async function calculate(
  database: AthleticDatabase,
  options: CalculateOptions,
): Promise<CalculationResult> {
  return outputOf(database, "calculate.range", { options });
}

export function renderCalculation(calculation: CalculationResult): string {
  return callSync("render.calculation", { calculation });
}

// ---- Logging, attachments, grading, evidence, and backtests -------------------

export async function planLogEntry(
  database: AthleticDatabase,
  input: { text: string; actor: RecordActorT; created_at?: string; benchmark?: string },
): Promise<LogPlan> {
  return outputOf(database, "log.plan", input);
}

export async function applyLogPlan(
  database: AthleticDatabase,
  plan: LogPlan,
): Promise<{ records: CurrentRecordT[]; database_revision: number }> {
  return writeRecords(database, plan.inputs, {
    expected_current_revisions: plan.expected_current_revisions,
  });
}

export async function prepareAttachments(
  database: AthleticDatabase,
  paths: readonly string[],
  actor: RecordActorT,
  createdAt: string,
): Promise<PreparedAttachment[]> {
  if (paths.length === 0) {
    return [];
  }
  if (database.localPath === undefined) {
    throw new Error("attachment ingestion requires a local SQLite database");
  }
  const prepared = await outputOf(database, "attachments.prepare", {
    paths,
    actor,
    created_at: createdAt,
  });
  return prepared.map((attachment) => ({
    input: attachment.input,
    bytes: Buffer.from(attachment.bytes_base64, "base64"),
    final_path: attachment.final_path,
  }));
}

export function addAttachmentsToLogPlan(
  plan: LogPlan,
  attachments: readonly PreparedAttachment[],
): LogPlan {
  if (attachments.length === 0) {
    return plan;
  }
  return callSync("log.add_attachments", {
    plan,
    attachments: attachments.map((attachment) => attachment.input),
  });
}

/**
 * Run `write` with the prepared attachment files. A write inside it that
 * creates a prepared attachment record sends the file with that write, so
 * the file and the record commit together.
 */
export async function withPreparedAttachmentFiles<T>(
  database: AthleticDatabase,
  attachments: readonly PreparedAttachment[],
  write: () => Promise<T>,
): Promise<T> {
  if (attachments.length === 0) {
    return write();
  }
  if (database.localPath === undefined) {
    throw new Error("attachment ingestion requires a local SQLite database");
  }
  const files = new Map<string, AttachmentFile>();
  for (const attachment of attachments) {
    const input = attachment.input;
    files.set(input.record_id!, {
      record_id: input.record_id!,
      created_at: input.created_at!,
      changed_by: input.changed_by,
      data: input.data,
      bytes_base64: Buffer.from(attachment.bytes).toString("base64"),
    });
  }
  return attachmentScope.run({ database, files }, write);
}

export async function planPredictionGrade(
  database: AthleticDatabase,
  input: {
    prediction_id: string;
    result_id: string;
    actor: RecordActorT;
    calculation_version?: string;
    created_at?: string;
  },
): Promise<PredictionGradePlan> {
  return outputOf(database, "grade.plan", input);
}

export async function gradePrediction(
  database: AthleticDatabase,
  plan: PredictionGradePlan,
): Promise<{ link: CurrentRecordT; grade: CurrentRecordT; database_revision: number }> {
  const written = await writeRecordChanges(database, {
    corrections: plan.link_correction === null ? [] : [plan.link_correction],
    records: plan.link_input === null ? [plan.grade_input] : [plan.link_input, plan.grade_input],
    expected_current_revisions: [
      { record_id: plan.prediction.record_id, revision: plan.prediction.current_revision },
      { record_id: plan.result.record_id, revision: plan.result.current_revision },
      ...(plan.existing_link === null
        ? []
        : [{ record_id: plan.existing_link.record_id, revision: plan.existing_link.current_revision }]),
    ],
  });
  const link =
    written.records.find((record) => record.record_type === "prediction_result_link") ??
    plan.existing_link;
  const grade = written.records.find((record) => record.record_type === "prediction_grade");
  if (link === undefined || link === null || grade === undefined) {
    throw new Error("prediction grade write did not return its link and grade");
  }
  return { link, grade, database_revision: written.database_revision };
}

export async function buildEvidence(
  database: AthleticDatabase,
  input: { benchmark: string; as_of?: string; limit?: number; window_days?: number },
): Promise<PredictionEvidence> {
  return outputOf(database, "evidence.build", input);
}

export async function resolveEvidenceReference(
  database: AthleticDatabase,
  input: EvidenceReferenceT,
): Promise<EvidenceReferenceResolution> {
  return outputOf(database, "evidence.resolve_reference", { reference: input });
}

export async function runV1Backtest(
  database: AthleticDatabase,
  input: {
    models: readonly string[];
    gateway: "vercel" | "openrouter";
    complete: (request: CompleteRequest) => Promise<CompleteResponse>;
    created_at?: string;
  },
): Promise<V1BacktestReport> {
  const result = await onDatabase(
    database,
    "backtest.run",
    defined({ models: input.models, gateway: input.gateway, created_at: input.created_at }),
    { callbacks: { complete: input.complete } },
  );
  return result.output;
}

export async function writeV1BacktestReport(
  databasePath: string,
  report: V1BacktestReport,
): Promise<string> {
  return (await callAsync("backtest.write_report", { database_path: databasePath, report })).output;
}

// ---- Export and package import ------------------------------------------------

/** Turn a JavaScript value into plain JSON data as `JSON.stringify` would see it after 2.1.0's key walk. */
function plainJson(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(plainJson);
  }
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>).map(([key, child]) => [key, plainJson(child)]),
    );
  }
  return value;
}

export function deterministicJson(value: unknown): Buffer {
  const plain = plainJson(value);
  const text = JSON.stringify(plain);
  if (text === undefined) {
    return Buffer.from(`${text}\n`, "utf8");
  }
  return Buffer.from(callSync("json.deterministic", { value: JSON.parse(text) as unknown }), "utf8");
}

export async function exportRecords(
  database: AthleticDatabase,
  outputDirectory: string,
  options: ExportDatabaseOptions = {},
): Promise<ExportDatabaseResult> {
  return outputOf(
    database,
    "export.run",
    defined({ output_directory: outputDirectory, exported_at: options.exported_at }),
  );
}

export async function loadAthleticStandardPackage(inputPath: string): Promise<LoadedPackage> {
  const result = await callAsync("package.load", { input_path: inputPath });
  const document = result.output.package;
  const arrays = new Map<string, unknown[]>();
  for (const key of Object.keys(document)) {
    if (Array.isArray(document[key]) && (document[key] as unknown[]).length === 0) {
      arrays.set(key, document[key] as unknown[]);
    }
  }
  for (const item of result.items) {
    let list = arrays.get(item.key);
    if (list === undefined) {
      list = [];
      arrays.set(item.key, list);
      document[item.key] = list;
    }
    list.push(item.value);
  }
  const packageValue = document as unknown as LoadedPackage["package"];
  return {
    package: packageValue,
    series_files: result.output.series_files,
    all_records: packageValue.records,
    all_revisions: packageValue.record_revisions,
    all_deletions: packageValue.record_deletions,
    all_import_keys: packageValue.record_import_keys,
    sample_record_id_examples: result.output.sample_record_id_examples,
    source_path: result.output.source_path,
    source_fingerprint: result.output.source_fingerprint,
    source_directory: result.output.source_directory,
    attachment_files: new Map(result.output.attachment_files),
  };
}

export async function readImportedPackageRecord(
  loaded: LoadedPackage,
  recordId: string,
): Promise<CurrentRecordT | null> {
  const main = loaded.all_records.find((record) => record.record_id === recordId);
  if (main !== undefined) {
    return main;
  }
  if (loaded.series_files.length === 0) {
    return null;
  }
  return (
    await callAsync("package.read_imported", {
      source_path: loaded.source_path,
      source_fingerprint: loaded.source_fingerprint,
      record_id: recordId,
    })
  ).output;
}

export async function previewPackageImport(
  database: AthleticDatabase,
  loaded: LoadedPackage,
): Promise<PackageImportPreview> {
  const preview = await outputOf(database, "package.preview", {
    source_path: loaded.source_path,
    source_fingerprint: loaded.source_fingerprint,
  });
  return { ...preview, loaded };
}

export async function importRecords(
  database: AthleticDatabase,
  preview: PackageImportPreview,
  resolutions: readonly PackageImportResolution[] = [],
): Promise<{ database_revision: number; restored: boolean }> {
  return outputOf(database, "package.import", {
    source_path: preview.loaded.source_path,
    source_fingerprint: preview.loaded.source_fingerprint,
    destination_database_id: preview.destination_database_id,
    destination_database_revision: preview.destination_database_revision,
    resolutions,
  });
}

/** The preview fields the conflict file compares, without the loaded records. */
function previewSummary(preview: PackageImportPreview): Record<string, unknown> {
  return {
    source_database_id: preview.source_database_id,
    source_database_revision: preview.source_database_revision,
    destination_database_id: preview.destination_database_id,
    destination_database_revision: preview.destination_database_revision,
    restore: preview.restore,
    conflicts: preview.conflicts,
    loaded: {
      source_path: preview.loaded.source_path,
      source_fingerprint: preview.loaded.source_fingerprint,
    },
  };
}

function conflictFileDatabase(database: AthleticDatabase): string {
  if (database.localPath === undefined) {
    throw new Error("temporary import conflicts require a local SQLite database");
  }
  return database.localPath;
}

export function packageConflictSetPath(database: AthleticDatabase): string {
  return callSync("conflicts.path", { database_path: conflictFileDatabase(database) });
}

export async function savePackageConflictSet(
  database: AthleticDatabase,
  preview: PackageImportPreview,
): Promise<PackageConflictSet | null> {
  if (database.localPath === undefined) {
    // 2.1.0 cleared nothing and returned null when there was nothing to
    // save, and refused only when it had a file to write.
    const savable =
      preview.conflicts.length > 0 && preview.conflicts.every((conflict) => conflict.record_id !== undefined);
    if (!savable) {
      return null;
    }
  }
  return (
    await callAsync("conflicts.save", {
      database_path: conflictFileDatabase(database),
      preview: previewSummary(preview),
    })
  ).output;
}

export async function readPackageConflictSet(
  database: AthleticDatabase,
): Promise<PackageConflictSet> {
  return (await callAsync("conflicts.read", { database_path: conflictFileDatabase(database) })).output;
}

export async function clearPackageConflictSet(database: AthleticDatabase): Promise<void> {
  if (database.localPath === undefined) {
    return;
  }
  await callAsync("conflicts.clear", { database_path: database.localPath });
}

export async function assertPackageConflictSetCurrent(
  database: AthleticDatabase,
  state: PackageConflictSet,
  preview: PackageImportPreview,
): Promise<void> {
  await onDatabase(database, "conflicts.assert_current", {
    state,
    preview: previewSummary(preview),
  });
}

export async function previewConflict(
  database: AthleticDatabase,
  preview: PackageImportPreview,
  recordId: string,
): Promise<{
  record_id: string;
  message: string;
  local: Awaited<ReturnType<typeof readRecord>>;
  imported: Awaited<ReturnType<typeof readImportedPackageRecord>>;
}> {
  const conflict = preview.conflicts.find((candidate) => candidate.record_id === recordId);
  if (conflict === undefined) {
    throw new Error(`record is not in the package conflict preview: ${recordId}`);
  }
  return {
    record_id: recordId,
    message: conflict.message,
    local: await readRecord(database, recordId),
    imported: await readImportedPackageRecord(preview.loaded, recordId),
  };
}

export async function resolvePackageConflict(
  database: AthleticDatabase,
  state: PackageConflictSet,
  recordId: string,
  resolution: PackageImportResolution,
): Promise<PackageConflictSet> {
  return outputOf(database, "conflicts.resolve", { state, record_id: recordId, resolution });
}

export const resolveConflict: (
  database: AthleticDatabase,
  state: PackageConflictSet,
  recordId: string,
  resolution: PackageImportResolution,
) => Promise<PackageConflictSet> = resolvePackageConflict;

export function conflictResolutions(state: PackageConflictSet): PackageImportResolution[] {
  return callSync("conflicts.resolutions", { state });
}

// ---- Privacy erasure ------------------------------------------------------------

export async function previewRecordErasure(
  database: AthleticDatabase,
  recordId: string,
  expectedRevision: number,
): Promise<ErasurePreview> {
  return outputOf(database, "erase.preview", {
    record_id: recordId,
    expected_revision: expectedRevision,
  });
}

export async function eraseRecord(
  database: AthleticDatabase,
  input: EraseRecordInput,
): Promise<{ erased_record_ids: string[]; erased_attachment_count: number; database_revision: number }> {
  return outputOf(
    database,
    "erase.apply",
    defined({ preview: input.preview, reason_code: input.reason_code, erased_at: input.erased_at }),
  );
}

// ---- Provider imports -------------------------------------------------------------

/** The export path and import time of a preview this client made. */
const providerPreviewSources = new WeakMap<ProviderImportPreview, { input_path: string; received_at: string }>();

export async function previewProviderImport(
  database: AthleticDatabase,
  inputPath: string,
  options: { received_at?: string } = {},
): Promise<ProviderImportPreview> {
  const result = await onDatabase(
    database,
    "provider.preview",
    defined({ input_path: inputPath, received_at: options.received_at }),
  );
  const drafts: ProviderRecordDraft[] = [];
  const changes: ProviderImportChange[] = [];
  for (const planned of result.items) {
    const draft: ProviderRecordDraft = {
      record_id: planned.record_id,
      record_type: planned.record_type,
      data: planned.data,
      match_key_method: result.output.match_key_method,
      match_key: planned.match_key,
    };
    drafts.push(draft);
    changes.push({ draft, action: planned.action });
  }
  const { received_at: receivedAt, match_key_method: _method, ...summary } = result.output;
  const preview: ProviderImportPreview = {
    ...summary,
    plan: { drafts, received_at: receivedAt },
    changes,
  };
  providerPreviewSources.set(preview, { input_path: inputPath, received_at: receivedAt });
  return preview;
}

export async function importProviderRecords(
  database: AthleticDatabase,
  preview: ProviderImportPreview,
): Promise<{
  database_revision: number;
  new_record_count: number;
  changed_record_count: number;
  matching_record_count: number;
}> {
  const source = providerPreviewSources.get(preview);
  targetOf(database);
  if (source === undefined) {
    throw new Error("provider import preview was not made by previewProviderImport; preview it again");
  }
  return outputOf(database, "provider.import", {
    input_path: source.input_path,
    received_at: source.received_at,
    database_revision: preview.database_revision,
    new_record_count: preview.new_records.length,
    matching_record_count: preview.matching_records.length,
    changed_record_count: preview.changed_records.length,
    conflict_count: preview.conflicts.length,
  });
}
