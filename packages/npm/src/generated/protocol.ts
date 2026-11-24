// Generated from src/protocol by tests/generated.rs.
// Do not edit. Run `ATH_WRITE_GENERATED=1 cargo test --test generated`.

import type * as Model from "../schema.js";
import type * as Api from "../types.js";

/** A prepared attachment file sent with the write that records it. */
export interface AttachmentFile {
  record_id: string;
  created_at: string;
  changed_by: unknown;
  data: unknown;
  bytes_base64: string;
}

/** The client protocol version this package speaks. */
export const PROTOCOL_VERSION = 1 as const;

/** The sole argument that starts a protocol worker. */
export const WORKER_ARGUMENT = "__client" as const;

/** Values the native library defines and the JavaScript exports repeat. */
export const NATIVE_CONSTANTS = {
  ATHLETIC_STANDARD_VERSION: "2.1.3",
  SQL_SCHEMA_VERSION: 7,
  SQLITE_APPLICATION_ID: 1096042545,
  SQLITE_APPLICATION_MARKER: "ATH1",
  MAX_ATTACHMENT_BYTES: 26214400,
  CALCULATION_OPERATIONS: ["summary","baseline","trend","coverage","compare-sources","correlation","workout-drift","provider-period-summary"],
  CALCULATION_RECORD_KINDS: ["measurement","provider-score","provider-summary","session-summary"],
} as const;

/** The request and response types of every operation. */
export interface OperationTypes {
  "ids.uuid_v7": {
    input: { now?: number | string };
    output: string;
    item: never;
    callbacks: Record<never, never>;
  };
  "model.validate": {
    input: { schema: string; json_text: string };
    output: { valid: boolean; issues: Array<{ path: Array<string | number>; code: string; message: string }>; output_text: string | null };
    item: never;
    callbacks: Record<never, never>;
  };
  "model.validate_many": {
    input: { cases: ReadonlyArray<{ schema: string; json_text: string }> };
    output: null;
    item: { valid: boolean; issues: Array<{ path: Array<string | number>; code: string; message: string }>; output_text: string | null; error?: string };
    callbacks: Record<never, never>;
  };
  "model.parse": {
    input: { schema: string; value: unknown };
    output: unknown;
    item: never;
    callbacks: Record<never, never>;
  };
  "migration.needs_migration": {
    input: { metadata: unknown };
    output: boolean;
    item: never;
    callbacks: Record<never, never>;
  };
  "migration.preview": {
    input: { metadata: unknown; local_path?: string; now_iso: string; sequence: number };
    output: Api.MigrationPreview;
    item: never;
    callbacks: Record<never, never>;
  };
  "backup.timestamp": {
    input: { iso: string };
    output: string;
    item: never;
    callbacks: Record<never, never>;
  };
  "backup.default_path": {
    input: { database_path: string; now_iso: string; sequence: number };
    output: string;
    item: never;
    callbacks: Record<never, never>;
  };
  "backup.sqlite3_command": {
    input: { backup_path: string };
    output: string;
    item: never;
    callbacks: Record<never, never>;
  };
  "calendar.date_in_zone": {
    input: { timestamp: string; time_zone: string };
    output: string;
    item: never;
    callbacks: Record<never, never>;
  };
  "json.deterministic": {
    input: { value: unknown };
    output: string;
    item: never;
    callbacks: Record<never, never>;
  };
  "database.open": {
    input: { path: string; allow_outdated_schema?: boolean; defer_wal?: boolean; now_iso: string; sequence: number };
    output: { path: string; database_id: string; sql_schema_version: number; warnings: string[] };
    item: never;
    callbacks: Record<never, never>;
  };
  "database.inspect": {
    input: { path: string };
    output: { path: string; database_id: string; sql_schema_version: number; warnings: string[] };
    item: never;
    callbacks: Record<never, never>;
  };
  "database.create": {
    input: { path: string; database_id?: unknown; created_at?: unknown };
    output: { path: string; database_id: string; sql_schema_version: number; warnings: string[] };
    item: never;
    callbacks: Record<never, never>;
  };
  "database.metadata": {
    input: Record<string, never>;
    output: Api.DatabaseMetadata;
    item: never;
    callbacks: Record<never, never>;
  };
  "database.apply_migrations": {
    input: Record<string, never>;
    output: null;
    item: never;
    callbacks: Record<never, never>;
  };
  "database.migrate_confirmed": {
    input: { backup_path?: string; now_iso: string; sequence: number };
    output: Api.ConfirmedMigrationResult;
    item: never;
    callbacks: Record<never, never>;
  };
  "database.backup": {
    input: { source_path: string; backup_path: string; source_metadata: unknown };
    output: null;
    item: never;
    callbacks: Record<never, never>;
  };
  "database.check": {
    input: Record<string, never>;
    output: Api.DatabaseCheckResult;
    item: never;
    callbacks: Record<never, never>;
  };
  "database.stats": {
    input: Record<string, never>;
    output: Api.DatabaseStats;
    item: never;
    callbacks: Record<never, never>;
  };
  "repair.candidates": {
    input: Record<string, never>;
    output: { encoding_record_count: number; revision_align_record_ids: string[] };
    item: never;
    callbacks: Record<never, never>;
  };
  "repair.provider_encoding": {
    input: Record<string, never>;
    output: Api.ProviderEncodingRepairResult;
    item: never;
    callbacks: Record<never, never>;
  };
  "repair.confirmed": {
    input: { backup_path?: string; now_ms?: number; from_revision?: boolean };
    output: Api.ConfirmedProviderRepairResult;
    item: never;
    callbacks: Record<never, never>;
  };
  "database.release": {
    input: Record<string, never>;
    output: null;
    item: never;
    callbacks: Record<never, never>;
  };
  "records.write": {
    input: { inputs: readonly unknown[]; expected_current_revisions?: readonly unknown[]; attachments?: readonly AttachmentFile[] };
    output: { records: Model.CurrentRecordT[]; database_revision: number };
    item: never;
    callbacks: Record<never, never>;
  };
  "records.write_changes": {
    input: { corrections?: readonly unknown[]; records?: readonly unknown[]; expected_current_revisions?: readonly unknown[]; attachments?: readonly AttachmentFile[] };
    output: { records: Model.CurrentRecordT[]; database_revision: number };
    item: never;
    callbacks: Record<never, never>;
  };
  "records.correct": {
    input: unknown;
    output: { record: Model.CurrentRecordT; database_revision: number };
    item: never;
    callbacks: Record<never, never>;
  };
  "records.delete": {
    input: unknown;
    output: { record_id: string; record_revision: number; database_revision: number };
    item: never;
    callbacks: Record<never, never>;
  };
  "records.read": {
    input: { record_id: unknown };
    output: Model.CurrentRecordT | null;
    item: never;
    callbacks: Record<never, never>;
  };
  "records.list": {
    input: { record_type?: unknown; include_deleted?: boolean; limit?: unknown };
    output: null;
    item: Model.CurrentRecordT;
    callbacks: Record<never, never>;
  };
  "records.revisions": {
    input: { record_id: unknown };
    output: null;
    item: Model.RecordRevisionT;
    callbacks: Record<never, never>;
  };
  "records.links": {
    input: { record_id: string };
    output: Api.RecordLink[];
    item: never;
    callbacks: Record<never, never>;
  };
  "query.measurements": {
    input: { query: unknown };
    output: null;
    item: Model.CurrentRecordT;
    callbacks: Record<never, never>;
  };
  "query.series": {
    input: { query: unknown };
    output: null;
    item: Model.CurrentRecordT;
    callbacks: Record<never, never>;
  };
  "query.sessions": {
    input: { query: unknown };
    output: null;
    item: Model.CurrentRecordT;
    callbacks: Record<never, never>;
  };
  "query.as_of": {
    input: { query: unknown };
    output: null;
    item: Model.CurrentRecordT;
    callbacks: Record<never, never>;
  };
  "view.series": {
    input: { quantity: unknown; options: unknown };
    output: Api.SeriesView;
    item: never;
    callbacks: Record<never, never>;
  };
  "view.calendar_range": {
    input: { options: unknown };
    output: Api.InstantRange;
    item: never;
    callbacks: Record<never, never>;
  };
  "view.as_of_time": {
    input: { value?: string; time_zone?: string };
    output: { as_of: string; time_zone?: string };
    item: never;
    callbacks: Record<never, never>;
  };
  "calculate.range": {
    input: { options: unknown };
    output: Api.CalculationResult;
    item: never;
    callbacks: Record<never, never>;
  };
  "render.record_list": {
    input: { records: readonly unknown[] };
    output: string;
    item: never;
    callbacks: Record<never, never>;
  };
  "render.record": {
    input: { record: unknown; revision?: unknown; history?: readonly unknown[]; links?: readonly unknown[] };
    output: string;
    item: never;
    callbacks: Record<never, never>;
  };
  "render.series_view": {
    input: { view: unknown };
    output: string;
    item: never;
    callbacks: Record<never, never>;
  };
  "render.stats": {
    input: { stats: unknown };
    output: string;
    item: never;
    callbacks: Record<never, never>;
  };
  "render.calculation": {
    input: { calculation: unknown };
    output: string;
    item: never;
    callbacks: Record<never, never>;
  };
  "log.plan": {
    input: { text: unknown; actor: unknown; created_at?: unknown; benchmark?: unknown };
    output: Api.LogPlan;
    item: never;
    callbacks: Record<never, never>;
  };
  "log.add_attachments": {
    input: { plan: unknown; attachments: readonly unknown[] };
    output: Api.LogPlan;
    item: never;
    callbacks: Record<never, never>;
  };
  "attachments.prepare": {
    input: { paths: readonly string[]; actor: unknown; created_at: unknown };
    output: Array<{ input: Api.WriteRecordInput; bytes_base64: string; final_path: string }>;
    item: never;
    callbacks: Record<never, never>;
  };
  "grade.plan": {
    input: { prediction_id: unknown; result_id: unknown; actor: unknown; calculation_version?: unknown; created_at?: unknown };
    output: Api.PredictionGradePlan;
    item: never;
    callbacks: Record<never, never>;
  };
  "evidence.build": {
    input: { benchmark: unknown; as_of?: unknown; limit?: unknown; window_days?: unknown };
    output: Api.PredictionEvidence;
    item: never;
    callbacks: Record<never, never>;
  };
  "evidence.resolve_reference": {
    input: { reference: unknown };
    output: Api.EvidenceReferenceResolution;
    item: never;
    callbacks: Record<never, never>;
  };
  "backtest.run": {
    input: { models: readonly string[]; gateway: string; created_at?: string };
    output: Api.V1BacktestReport;
    item: never;
    callbacks: {
      complete: (input: Api.CompleteRequest) => Promise<Api.CompleteResponse>;
    };
  };
  "backtest.write_report": {
    input: { database_path: string; report: unknown };
    output: string;
    item: never;
    callbacks: Record<never, never>;
  };
  "export.run": {
    input: { output_directory: string; exported_at?: unknown };
    output: Api.ExportDatabaseResult;
    item: never;
    callbacks: Record<never, never>;
  };
  "package.load": {
    input: { input_path: string };
    output: { package: Record<string, unknown>; series_files: Array<{ reference: Model.AthleticStandardPackageT["measurement_series_files"][number]; path: string }>; sample_record_id_examples: string[]; source_path: string; source_fingerprint: string; source_directory: string; attachment_files: Array<[string, string]> };
    item: { key: string; value: unknown };
    callbacks: Record<never, never>;
  };
  "package.preview": {
    input: { source_path: string; source_fingerprint: string };
    output: Omit<Api.PackageImportPreview, "loaded">;
    item: never;
    callbacks: Record<never, never>;
  };
  "package.import": {
    input: { source_path: string; source_fingerprint: string; destination_database_id: unknown; destination_database_revision: unknown; resolutions: readonly unknown[] };
    output: { database_revision: number; restored: boolean };
    item: never;
    callbacks: Record<never, never>;
  };
  "package.read_imported": {
    input: { source_path: string; source_fingerprint: string; record_id: string };
    output: Model.CurrentRecordT | null;
    item: never;
    callbacks: Record<never, never>;
  };
  "conflicts.save": {
    input: { database_path: string; preview: unknown };
    output: Api.PackageConflictSet | null;
    item: never;
    callbacks: Record<never, never>;
  };
  "conflicts.read": {
    input: { database_path: string };
    output: Api.PackageConflictSet;
    item: never;
    callbacks: Record<never, never>;
  };
  "conflicts.clear": {
    input: { database_path: string };
    output: null;
    item: never;
    callbacks: Record<never, never>;
  };
  "conflicts.assert_current": {
    input: { state: unknown; preview: unknown };
    output: null;
    item: never;
    callbacks: Record<never, never>;
  };
  "conflicts.resolve": {
    input: { state: unknown; record_id: string; resolution: unknown };
    output: Api.PackageConflictSet;
    item: never;
    callbacks: Record<never, never>;
  };
  "conflicts.path": {
    input: { database_path: string };
    output: string;
    item: never;
    callbacks: Record<never, never>;
  };
  "conflicts.resolutions": {
    input: { state: unknown };
    output: Api.PackageImportResolution[];
    item: never;
    callbacks: Record<never, never>;
  };
  "erase.preview": {
    input: { record_id: unknown; expected_revision: unknown };
    output: Api.ErasurePreview;
    item: never;
    callbacks: Record<never, never>;
  };
  "erase.apply": {
    input: { preview: unknown; reason_code: unknown; erased_at?: unknown };
    output: { erased_record_ids: string[]; erased_attachment_count: number; database_revision: number };
    item: never;
    callbacks: Record<never, never>;
  };
  "provider.preview": {
    input: { input_path: string; received_at?: unknown };
    output: Omit<Api.ProviderImportPreview, "plan" | "changes"> & { received_at: string; match_key_method: string; input_fingerprint: string };
    item: { record_id: string; record_type: Model.RecordTypeT; action: "new" | "matching" | "changed"; match_key: string; data: unknown };
    callbacks: Record<never, never>;
  };
  "provider.import": {
    input: { input_path: string; received_at: string; input_fingerprint: string; database_revision: unknown; new_record_count: number; matching_record_count: number; changed_record_count: number; conflict_count: number };
    output: { database_revision: number; new_record_count: number; changed_record_count: number; matching_record_count: number };
    item: never;
    callbacks: Record<never, never>;
  };
}

/** Every operation name. */
export type OperationName = keyof OperationTypes;

/** Operations the client runs with a synchronous child process. */
export type SyncOperationName = {
  [Name in OperationName]: (typeof OPERATIONS)[Name]["kind"] extends "sync" ? Name : never;
}[OperationName];

/** Operations the client runs with a promise. */
export type AsyncOperationName = Exclude<OperationName, SyncOperationName>;

/** How the client dispatches each operation. */
export const OPERATIONS = {
  "ids.uuid_v7": { kind: "sync", target: "none", streams: false, callbacks: [] },
  "model.validate": { kind: "async", target: "none", streams: false, callbacks: [] },
  "model.validate_many": { kind: "async", target: "none", streams: true, callbacks: [] },
  "model.parse": { kind: "sync", target: "none", streams: false, callbacks: [] },
  "migration.needs_migration": { kind: "sync", target: "none", streams: false, callbacks: [] },
  "migration.preview": { kind: "sync", target: "none", streams: false, callbacks: [] },
  "backup.timestamp": { kind: "sync", target: "none", streams: false, callbacks: [] },
  "backup.default_path": { kind: "sync", target: "none", streams: false, callbacks: [] },
  "backup.sqlite3_command": { kind: "sync", target: "none", streams: false, callbacks: [] },
  "calendar.date_in_zone": { kind: "sync", target: "none", streams: false, callbacks: [] },
  "json.deterministic": { kind: "sync", target: "none", streams: false, callbacks: [] },
  "database.open": { kind: "async", target: "none", streams: false, callbacks: [] },
  "database.inspect": { kind: "async", target: "none", streams: false, callbacks: [] },
  "database.create": { kind: "async", target: "none", streams: false, callbacks: [] },
  "database.metadata": { kind: "async", target: "read", streams: false, callbacks: [] },
  "database.apply_migrations": { kind: "async", target: "write", streams: false, callbacks: [] },
  "database.migrate_confirmed": { kind: "async", target: "write", streams: false, callbacks: [] },
  "database.backup": { kind: "async", target: "none", streams: false, callbacks: [] },
  "database.check": { kind: "async", target: "read", streams: false, callbacks: [] },
  "database.stats": { kind: "async", target: "read", streams: false, callbacks: [] },
  "repair.candidates": { kind: "async", target: "read", streams: false, callbacks: [] },
  "repair.provider_encoding": { kind: "async", target: "write", streams: false, callbacks: [] },
  "repair.confirmed": { kind: "async", target: "write", streams: false, callbacks: [] },
  "database.release": { kind: "async", target: "write", streams: false, callbacks: [] },
  "records.write": { kind: "async", target: "write", streams: false, callbacks: [] },
  "records.write_changes": { kind: "async", target: "write", streams: false, callbacks: [] },
  "records.correct": { kind: "async", target: "write", streams: false, callbacks: [] },
  "records.delete": { kind: "async", target: "write", streams: false, callbacks: [] },
  "records.read": { kind: "async", target: "read", streams: false, callbacks: [] },
  "records.list": { kind: "async", target: "read", streams: true, callbacks: [] },
  "records.revisions": { kind: "async", target: "read", streams: true, callbacks: [] },
  "records.links": { kind: "async", target: "read", streams: false, callbacks: [] },
  "query.measurements": { kind: "async", target: "read", streams: true, callbacks: [] },
  "query.series": { kind: "async", target: "read", streams: true, callbacks: [] },
  "query.sessions": { kind: "async", target: "read", streams: true, callbacks: [] },
  "query.as_of": { kind: "async", target: "read", streams: true, callbacks: [] },
  "view.series": { kind: "async", target: "read", streams: false, callbacks: [] },
  "view.calendar_range": { kind: "async", target: "read", streams: false, callbacks: [] },
  "view.as_of_time": { kind: "async", target: "read", streams: false, callbacks: [] },
  "calculate.range": { kind: "async", target: "read", streams: false, callbacks: [] },
  "render.record_list": { kind: "sync", target: "none", streams: false, callbacks: [] },
  "render.record": { kind: "sync", target: "none", streams: false, callbacks: [] },
  "render.series_view": { kind: "sync", target: "none", streams: false, callbacks: [] },
  "render.stats": { kind: "sync", target: "none", streams: false, callbacks: [] },
  "render.calculation": { kind: "sync", target: "none", streams: false, callbacks: [] },
  "log.plan": { kind: "async", target: "read", streams: false, callbacks: [] },
  "log.add_attachments": { kind: "sync", target: "none", streams: false, callbacks: [] },
  "attachments.prepare": { kind: "async", target: "read", streams: false, callbacks: [] },
  "grade.plan": { kind: "async", target: "read", streams: false, callbacks: [] },
  "evidence.build": { kind: "async", target: "read", streams: false, callbacks: [] },
  "evidence.resolve_reference": { kind: "async", target: "read", streams: false, callbacks: [] },
  "backtest.run": { kind: "async", target: "read", streams: false, callbacks: ["complete"] },
  "backtest.write_report": { kind: "async", target: "none", streams: false, callbacks: [] },
  "export.run": { kind: "async", target: "read", streams: false, callbacks: [] },
  "package.load": { kind: "async", target: "none", streams: true, callbacks: [] },
  "package.preview": { kind: "async", target: "read", streams: false, callbacks: [] },
  "package.import": { kind: "async", target: "write", streams: false, callbacks: [] },
  "package.read_imported": { kind: "async", target: "none", streams: false, callbacks: [] },
  "conflicts.save": { kind: "async", target: "none", streams: false, callbacks: [] },
  "conflicts.read": { kind: "async", target: "none", streams: false, callbacks: [] },
  "conflicts.clear": { kind: "async", target: "none", streams: false, callbacks: [] },
  "conflicts.assert_current": { kind: "async", target: "read", streams: false, callbacks: [] },
  "conflicts.resolve": { kind: "async", target: "read", streams: false, callbacks: [] },
  "conflicts.path": { kind: "sync", target: "none", streams: false, callbacks: [] },
  "conflicts.resolutions": { kind: "sync", target: "none", streams: false, callbacks: [] },
  "erase.preview": { kind: "async", target: "read", streams: false, callbacks: [] },
  "erase.apply": { kind: "async", target: "write", streams: false, callbacks: [] },
  "provider.preview": { kind: "async", target: "read", streams: true, callbacks: [] },
  "provider.import": { kind: "async", target: "write", streams: false, callbacks: [] },
} as const;
