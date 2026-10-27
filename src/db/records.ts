import type { Transaction } from "@libsql/client";
import {
  BecameKnownAtBasis,
  CurrentRecord,
  BenchmarkResultData,
  RecordActor,
  RecordId,
  RecordRevision,
  RecordType,
  PredictionData,
  PredictionResultLinkData,
  UtcTimestamp,
  type CurrentRecordT,
  type RecordActorT,
  type RecordRevisionT,
  type RecordTypeT,
} from "../v1/schema.js";
import { typedTables } from "./check-tables.js";
import { runAthCore } from "../ath-core.js";
import { databaseClient, enableLocalWal, type AthleticDatabase } from "./client.js";
import {
  currentRecordPayloadFromRow,
  parseExtensionsJson,
  recordRevisionPayloadFromRow,
  serializeExtensionsJson,
} from "./revision-encoding.js";
import { withWriteTransaction } from "./transaction.js";
import { createUuidV7 } from "./uuid.js";
import type { RecordExtensionsT } from "../v1/extensions.js";

type SqlValue = string | number | bigint | boolean | Uint8Array | ArrayBuffer | null;

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

export interface CorrectRecordInput {
  record_id: string;
  expected_revision: number;
  data: unknown;
  extensions?: RecordExtensionsT;
  changed_by: RecordActorT;
  became_known_at?: string;
  became_known_at_basis?: "local_write" | "provider_import" | "package_merge";
}

export interface DeleteRecordInput {
  record_id: string;
  expected_revision: number;
  deleted_by: RecordActorT;
  deleted_at?: string;
}

export interface ListRecordOptions {
  record_type?: RecordTypeT;
  include_deleted?: boolean;
  limit?: number;
}

export interface ExpectedCurrentRevision {
  record_id: string;
  revision: number;
}

export interface WriteRecordsOptions {
  expected_current_revisions?: readonly ExpectedCurrentRevision[];
}

export interface WriteRecordChangesInput extends WriteRecordsOptions {
  corrections?: readonly CorrectRecordInput[];
  records?: readonly WriteRecordInput[];
}

export async function persistActions(
  database: AthleticDatabase,
  actions: readonly unknown[],
): Promise<number> {
  if (database.localPath === undefined) {
    throw new Error("ath-core writes a local database file");
  }
  await enableLocalWal(database);
  const result = runAthCore(
    ["persist", "--database", database.localPath],
    process.env,
    JSON.stringify({ actions }),
  ) as { database_revision: number };
  return result.database_revision;
}

function requiredJson(value: unknown): string {
  const json = JSON.stringify(value);
  if (json === undefined) {
    throw new Error("Record value cannot be serialized as JSON");
  }
  return json;
}

function nullable(value: string | number | undefined): string | number | null {
  return value ?? null;
}

async function upsertTypedRow(
  transaction: Transaction,
  table: string,
  record: CurrentRecordT,
  fields: ReadonlyArray<readonly [string, SqlValue]>,
): Promise<void> {
  const info = await transaction.execute(`PRAGMA table_info(${table})`);
  const present = new Set(info.rows.map((row) => String(row.name)));
  const usable = fields.filter(([name]) => present.has(name));
  const columns = ["record_id", "record_type", ...usable.map(([name]) => name), "data_json"];
  const values: SqlValue[] = [
    record.record_id,
    record.record_type,
    ...usable.map(([, value]) => value),
    requiredJson(record.data),
  ];
  const updates = columns
    .filter((column) => column !== "record_id")
    .map((column) => `${column} = excluded.${column}`)
    .join(", ");
  await transaction.execute({
    sql: `
      INSERT INTO ${table} (${columns.join(", ")})
      VALUES (${columns.map(() => "?").join(", ")})
      ON CONFLICT(record_id) DO UPDATE SET ${updates}
    `,
    args: values,
  });
}

async function replaceOrderedJsonChildren(
  transaction: Transaction,
  table: string,
  parentColumn: string,
  orderColumn: string,
  jsonColumn: string,
  parentId: string,
  children: readonly unknown[],
): Promise<void> {
  const tableInfo = await transaction.execute({
    sql: "SELECT 1 AS present FROM sqlite_master WHERE type = 'table' AND name = ?",
    args: [table],
  });
  if (tableInfo.rows.length === 0) {
    if (children.length === 0) return;
    throw new Error(`${table} is missing. Run ath migrate before writing this record.`);
  }
  await transaction.execute({
    sql: `DELETE FROM ${table} WHERE ${parentColumn} = ?`,
    args: [parentId],
  });
  for (const [index, child] of children.entries()) {
    await transaction.execute({
      sql: `
        INSERT INTO ${table} (${parentColumn}, ${orderColumn}, ${jsonColumn})
        VALUES (?, ?, ?)
      `,
      args: [parentId, index, requiredJson(child)],
    });
  }
}

async function replaceProviderReportingPeriods(
  transaction: Transaction,
  record: CurrentRecordT,
): Promise<void> {
  await transaction.execute({
    sql: "DELETE FROM provider_reporting_periods WHERE owner_record_id = ?",
    args: [record.record_id],
  });
  if (!("provider_reporting_periods" in record.data)) {
    return;
  }
  const periods = record.data.provider_reporting_periods ?? [];
  for (const [index, period] of periods.entries()) {
    await transaction.execute({
      sql: `
        INSERT INTO provider_reporting_periods (
          owner_record_id,
          period_order,
          period_type,
          provider_period_id,
          provider_date,
          start_at,
          end_at,
          period_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
      `,
      args: [
        record.record_id,
        index,
        period.period_type,
        nullable(period.provider_period_id),
        nullable(period.provider_date),
        nullable(period.start_at),
        nullable(period.end_at),
        requiredJson(period),
      ],
    });
  }
}

async function replaceAuthoredEvidence(
  transaction: Transaction,
  record: CurrentRecordT,
): Promise<void> {
  await transaction.execute({
    sql: "DELETE FROM authored_record_evidence WHERE authored_record_id = ?",
    args: [record.record_id],
  });
  if (!("authorship" in record.data)) {
    return;
  }
  for (const [index, evidence] of record.data.authorship.evidence.entries()) {
    await transaction.execute({
      sql: `
        INSERT INTO authored_record_evidence (
          authored_record_id,
          evidence_order,
          evidence_record_id,
          evidence_revision
        ) VALUES (?, ?, ?, ?)
      `,
      args: [record.record_id, index, evidence.record_id, evidence.revision],
    });
  }
}

/** Denormalized typed-table columns derived the same way as upsertTypedRow. */
export function typedMainRowFields(
  record: CurrentRecordT,
): ReadonlyArray<readonly [string, SqlValue]> {
  switch (record.record_type) {
    case "athlete_profile":
      return [["default_time_zone", nullable(record.data.default_time_zone)]];
    case "source":
      return [
        ["source_type", record.data.source_type],
        ["data_writer", record.data.data_writer],
        ["data_route", record.data.data_route],
        ["vendor", nullable(record.data.vendor)],
        ["sensor_type", nullable(record.data.sensor_type)],
      ];
    case "source_device":
      return [
        ["source_id", record.data.source_id],
        ["name", nullable(record.data.name)],
        ["manufacturer", nullable(record.data.manufacturer)],
        ["model", nullable(record.data.model)],
        ["hardware", nullable(record.data.hardware)],
        ["serial_number", nullable(record.data.serial_number)],
        ["coverage_start_date", nullable(record.data.coverage_start_date)],
        ["coverage_end_date", nullable(record.data.coverage_end_date)],
        ["accepted_record_count", nullable(record.data.accepted_record_count)],
      ];
    case "point_measurement":
      return [
        ["source_id", record.data.source_id],
        ["quantity", record.data.quantity],
        ["recorded_at", nullable(record.data.recorded_at)],
        ["standard_value", record.data.value.standard_value],
        ["standard_unit", record.data.value.standard_unit],
        ["provider_record_id", nullable(record.data.provider_record_id)],
        ["received_at", record.data.received_at],
      ];
    case "measurement_sample":
      return [
        ["source_id", record.data.source_id],
        ["quantity", record.data.quantity],
        ["recorded_at", record.data.recorded_at],
        ["standard_value", record.data.value.standard_value],
        ["standard_unit", record.data.value.standard_unit],
        ["duration_ms", nullable(record.data.duration_ms)],
        ["provider_record_id", nullable(record.data.provider_record_id)],
        ["received_at", record.data.received_at],
      ];
    case "sleep_session":
      return [
        ["source_id", record.data.source_id],
        ["start_at", record.data.start_at],
        ["end_at", record.data.end_at],
        ["provider_record_id", nullable(record.data.provider_record_id)],
        ["received_at", record.data.received_at],
      ];
    case "workout_session":
      return [
        ["source_id", record.data.source_id],
        ["start_at", record.data.start_at],
        ["end_at", record.data.end_at],
        ["activity_type", nullable(record.data.activity_type)],
        ["name", nullable(record.data.name)],
        ["description", nullable(record.data.description)],
        ["text_result", nullable(record.data.text_result)],
        ["provider_record_id", nullable(record.data.provider_record_id)],
        ["received_at", record.data.received_at],
      ];
    case "benchmark_result":
      return [
        ["source_id", record.data.source_id],
        ["benchmark_definition_id", record.data.benchmark_definition_id],
        ["recorded_at", record.data.recorded_at],
        ["workout_session_id", nullable(record.data.workout_session_id)],
        ["provider_record_id", nullable(record.data.provider_record_id)],
        ["received_at", record.data.received_at],
      ];
    case "provider_score":
      return [
        ["source_id", record.data.source_id],
        ["metric", record.data.metric],
        ["recorded_at", nullable(record.data.recorded_at)],
        ["standard_value", record.data.value.standard_value],
        ["standard_unit", record.data.value.standard_unit],
        ["scale", record.data.scale],
        ["provider_record_id", nullable(record.data.provider_record_id)],
        ["received_at", record.data.received_at],
      ];
    case "provider_summary":
      return [
        ["source_id", record.data.source_id],
        ["summary_type", record.data.summary_type],
        ["recorded_at", nullable(record.data.recorded_at)],
        ["standard_value", record.data.value.standard_value],
        ["standard_unit", record.data.value.standard_unit],
        ["provider_record_id", nullable(record.data.provider_record_id)],
        ["received_at", record.data.received_at],
      ];
    case "athlete_report":
      return [
        ["report_type", record.data.report_type],
        ["reported_at", record.data.reported_at],
      ];
    case "nutrition_entry":
      return [
        ["reported_at", record.data.reported_at],
        ["athlete_description", record.data.athlete_description],
      ];
    case "attachment":
      return [
        ["file_name", record.data.file_name],
        ["media_type", record.data.media_type],
        ["byte_count", record.data.byte_count],
        ["sha256", record.data.sha256],
      ];
    case "benchmark_definition":
      return [
        ["benchmark_key", record.data.benchmark_key],
        ["benchmark_type", record.data.benchmark_type],
        ["score_type", record.data.score_type],
        ["definition", record.data.definition],
      ];
    case "prediction":
      return [
        ["benchmark_definition_id", record.data.benchmark_definition_id],
        ["authored_at", record.data.authorship.authored_at],
        ["author_type", record.data.authorship.author.author_type],
        ["content_version", record.data.authorship.content_version],
        ["status", record.data.authorship.status],
      ];
    case "prediction_result_link":
      return [
        ["prediction_id", record.data.prediction.record_id],
        ["prediction_revision", record.data.prediction.revision],
        ["benchmark_result_id", record.data.benchmark_result.record_id],
        ["benchmark_result_revision", record.data.benchmark_result.revision],
        ["linked_at", record.data.linked_at],
        ["created_by_type", record.data.created_by.author_type],
        ["content_version", record.data.content_version],
        ["status", record.data.status],
      ];
    case "prediction_grade":
      return [
        ["prediction_result_link_id", record.data.prediction_result_link.record_id],
        ["prediction_result_link_revision", record.data.prediction_result_link.revision],
        ["authored_at", record.data.authorship.authored_at],
        ["author_type", record.data.authorship.author.author_type],
        ["content_version", record.data.authorship.content_version],
        ["status", record.data.authorship.status],
        ["signed_error", record.data.signed_error],
        ["absolute_error_percentage", record.data.absolute_error_percentage],
        ["result_in_predicted_range", record.data.result_in_predicted_range ? 1 : 0],
      ];
    case "prediction_error_analysis":
      return [
        ["prediction_grade_id", record.data.prediction_grade.record_id],
        ["prediction_grade_revision", record.data.prediction_grade.revision],
        ["authored_at", record.data.authorship.authored_at],
        ["author_type", record.data.authorship.author.author_type],
        ["content_version", record.data.authorship.content_version],
        ["status", record.data.authorship.status],
      ];
    case "training_plan":
      return [
        ["title", record.data.title],
        ["start_date", nullable(record.data.start_date)],
        ["end_date", nullable(record.data.end_date)],
        ["authored_at", record.data.authorship.authored_at],
        ["author_type", record.data.authorship.author.author_type],
        ["content_version", record.data.authorship.content_version],
        ["status", record.data.authorship.status],
      ];
    case "planned_session":
      return [
        ["training_plan_id", nullable(record.data.training_plan_id)],
        ["title", record.data.title],
        ["scheduled_date", nullable(record.data.scheduled_date)],
        ["scheduled_start_at", nullable(record.data.scheduled_start_at)],
        ["completion_status", record.data.completion_status],
        ["completed_workout_session_id", nullable(record.data.completed_workout_session_id)],
        ["authored_at", record.data.authorship.authored_at],
        ["author_type", record.data.authorship.author.author_type],
        ["content_version", record.data.authorship.content_version],
        ["status", record.data.authorship.status],
      ];
    case "coaching_note":
      return [
        ["authored_at", record.data.authorship.authored_at],
        ["author_type", record.data.authorship.author.author_type],
        ["content_version", record.data.authorship.content_version],
        ["status", record.data.authorship.status],
      ];
    case "saved_training_review":
      return [
        ["period_start", record.data.period_start],
        ["period_end", record.data.period_end],
        ["authored_at", record.data.authorship.authored_at],
        ["author_type", record.data.authorship.author.author_type],
        ["content_version", record.data.authorship.content_version],
        ["status", record.data.authorship.status],
      ];
    case "goal":
      return [
        ["title", record.data.title],
        ["target_date", nullable(record.data.target_date)],
        [
          "target_benchmark_definition_id",
          nullable(record.data.target_benchmark_definition_id),
        ],
        ["authored_at", record.data.authorship.authored_at],
        ["author_type", record.data.authorship.author.author_type],
        ["content_version", record.data.authorship.content_version],
        ["status", record.data.authorship.status],
      ];
    case "nutrition_model_interpretation":
      return [
        ["nutrition_entry_id", record.data.nutrition_entry_id],
        ["confidence", record.data.confidence],
        ["authored_at", record.data.authorship.authored_at],
        ["author_type", record.data.authorship.author.author_type],
        ["content_version", record.data.authorship.content_version],
        ["status", record.data.authorship.status],
      ];
  }
}

async function upsertTypedRowForRecord(
  transaction: Transaction,
  table: string,
  record: CurrentRecordT,
): Promise<void> {
  await upsertTypedRow(transaction, table, record, typedMainRowFields(record));
}

async function replaceAttachmentLinks(
  transaction: Transaction,
  recordId: string,
  attachmentIds: readonly string[],
): Promise<void> {
  await transaction.execute({
    sql: "DELETE FROM record_attachment_links WHERE owner_record_id = ?",
    args: [recordId],
  });
  for (const [index, attachmentId] of attachmentIds.entries()) {
    await transaction.execute({
      sql: `
        INSERT INTO record_attachment_links (
          owner_record_id,
          attachment_order,
          attachment_record_id
        ) VALUES (?, ?, ?)
      `,
      args: [recordId, index, attachmentId],
    });
  }
}

export async function upsertTypedRecord(
  transaction: Transaction,
  record: CurrentRecordT,
): Promise<void> {
  switch (record.record_type) {
    case "athlete_profile":
      await upsertTypedRowForRecord(transaction, "athlete_profile", record);
      break;
    case "source":
      await upsertTypedRowForRecord(transaction, "sources", record);
      break;
    case "source_device":
      await upsertTypedRowForRecord(transaction, "source_devices", record);
      break;
    case "point_measurement":
      await upsertTypedRowForRecord(transaction, "point_measurements", record);
      await replaceOrderedJsonChildren(
        transaction,
        "point_measurement_calculations",
        "point_measurement_id",
        "calculation_order",
        "calculation_json",
        record.record_id,
        record.data.calculations ?? [],
      );
      await transaction.execute({
        sql: "DELETE FROM point_measurement_calculation_inputs WHERE point_measurement_id = ?",
        args: [record.record_id],
      });
      for (const [calculationIndex, calculation] of (
        record.data.calculations ?? []
      ).entries()) {
        for (const [inputIndex, input] of calculation.input_records.entries()) {
          await transaction.execute({
            sql: `
              INSERT INTO point_measurement_calculation_inputs (
                point_measurement_id,
                calculation_order,
                input_order,
                evidence_record_id,
                evidence_revision
              ) VALUES (?, ?, ?, ?, ?)
            `,
            args: [
              record.record_id,
              calculationIndex,
              inputIndex,
              input.record_id,
              input.revision,
            ],
          });
        }
      }
      break;
    case "measurement_sample":
      await upsertTypedRowForRecord(transaction, "measurement_samples", record);
      break;
    case "sleep_session":
      await upsertTypedRowForRecord(transaction, "sleep_sessions", record);
      break;
    case "workout_session":
      await upsertTypedRowForRecord(transaction, "workout_sessions", record);
      await replaceOrderedJsonChildren(
        transaction,
        "workout_session_segments",
        "workout_session_id",
        "segment_order",
        "segment_json",
        record.record_id,
        record.data.segments ?? [],
      );
      await replaceOrderedJsonChildren(
        transaction,
        "workout_route_points",
        "workout_session_id",
        "point_order",
        "point_json",
        record.record_id,
        record.data.route ?? [],
      );
      break;
    case "benchmark_result":
      await upsertTypedRowForRecord(transaction, "benchmark_results", record);
      await replaceOrderedJsonChildren(
        transaction,
        "benchmark_result_segments",
        "benchmark_result_id",
        "segment_order",
        "segment_json",
        record.record_id,
        record.data.segments ?? [],
      );
      break;
    case "provider_score":
      await upsertTypedRowForRecord(transaction, "provider_scores", record);
      break;
    case "provider_summary":
      await upsertTypedRowForRecord(transaction, "provider_summaries", record);
      break;
    case "athlete_report":
      await upsertTypedRowForRecord(transaction, "athlete_reports", record);
      await replaceAttachmentLinks(
        transaction,
        record.record_id,
        record.data.attachment_ids ?? [],
      );
      break;
    case "nutrition_entry":
      await upsertTypedRowForRecord(transaction, "nutrition_entries", record);
      await replaceOrderedJsonChildren(
        transaction,
        "reported_nutrition_items",
        "nutrition_entry_id",
        "item_order",
        "item_json",
        record.record_id,
        record.data.reported_items,
      );
      await replaceAttachmentLinks(
        transaction,
        record.record_id,
        record.data.attachment_ids ?? [],
      );
      break;
    case "attachment":
      await upsertTypedRowForRecord(transaction, "attachments", record);
      break;
    case "benchmark_definition":
      await upsertTypedRowForRecord(transaction, "benchmark_definitions", record);
      break;
    case "prediction":
      await upsertTypedRowForRecord(transaction, "predictions", record);
      break;
    case "prediction_result_link":
      await upsertTypedRowForRecord(transaction, "prediction_result_links", record);
      break;
    case "prediction_grade":
      await upsertTypedRowForRecord(transaction, "prediction_grades", record);
      break;
    case "prediction_error_analysis":
      await upsertTypedRowForRecord(transaction, "prediction_error_analyses", record);
      await transaction.execute({
        sql: "DELETE FROM prediction_error_causes WHERE prediction_error_analysis_id = ?",
        args: [record.record_id],
      });
      for (const [index, cause] of record.data.causes.entries()) {
        await transaction.execute({
          sql: `
            INSERT INTO prediction_error_causes (
              prediction_error_analysis_id,
              cause_order,
              evidence_record_id,
              evidence_revision,
              explanation
            ) VALUES (?, ?, ?, ?, ?)
          `,
          args: [
            record.record_id,
            index,
            cause.evidence.record_id,
            cause.evidence.revision,
            cause.explanation,
          ],
        });
      }
      break;
    case "training_plan":
      await upsertTypedRowForRecord(transaction, "training_plans", record);
      break;
    case "planned_session":
      await upsertTypedRowForRecord(transaction, "planned_sessions", record);
      await replaceOrderedJsonChildren(
        transaction,
        "planned_session_blocks",
        "planned_session_id",
        "block_order",
        "block_json",
        record.record_id,
        record.data.blocks,
      );
      break;
    case "coaching_note":
      await upsertTypedRowForRecord(transaction, "coaching_notes", record);
      break;
    case "saved_training_review":
      await upsertTypedRowForRecord(transaction, "saved_training_reviews", record);
      break;
    case "goal":
      await upsertTypedRowForRecord(transaction, "goals", record);
      break;
    case "nutrition_model_interpretation":
      await upsertTypedRowForRecord(transaction, "nutrition_model_interpretations", record);
      break;
  }

  await replaceProviderReportingPeriods(transaction, record);
  await replaceAuthoredEvidence(transaction, record);
}

/** Rebuild the typed table row and children from the current revision bytes. */
export async function rebuildTypedRowFromCurrentRevision(
  transaction: Transaction,
  recordId: string,
): Promise<void> {
  const result = await transaction.execute({
    sql: `
      SELECT
        records.record_id,
        records.record_type,
        records.current_revision,
        records.created_at,
        records.is_deleted,
        record_revisions.record_json,
        record_revisions.extensions_json
      FROM records
      JOIN record_revisions
        ON record_revisions.record_id = records.record_id
       AND record_revisions.revision = records.current_revision
      WHERE records.record_id = ?
    `,
    args: [recordId],
  });
  const row = result.rows[0];
  if (row === undefined) {
    throw new Error(`record ${recordId} is missing its current revision`);
  }
  const recordJson = String(row.record_json);
  const record = CurrentRecord.parse(currentRecordPayloadFromRow(row));
  await upsertTypedRecord(transaction, record);
  const table = typedTables[record.record_type];
  if (table === undefined) {
    throw new Error(`record type ${record.record_type} has no typed table`);
  }
  await transaction.execute({
    sql: `UPDATE ${table} SET data_json = ? WHERE record_id = ?`,
    args: [recordJson, recordId],
  });
}

async function readTypedRevisionData(
  transaction: Transaction,
  recordId: string,
  revision: number,
  expectedRecordType: RecordTypeT,
): Promise<unknown> {
  const result = await transaction.execute({
    sql: `
      SELECT record_type, record_json
      FROM record_revisions
      WHERE record_id = ? AND revision = ?
    `,
    args: [recordId, revision],
  });
  const row = result.rows[0];
  if (row === undefined) {
    throw new Error(`Referenced record revision does not exist: ${recordId}@${revision}`);
  }
  if (row.record_type !== expectedRecordType) {
    throw new Error(
      `Referenced ${recordId}@${revision} is ${String(row.record_type)}, not ${expectedRecordType}`,
    );
  }
  return JSON.parse(String(row.record_json)) as unknown;
}

function nativeScoreValue(
  score: {
    duration_s?: number | undefined;
    reps?: number | undefined;
    weight_kg?: number | undefined;
  },
  scoreType: string,
): number {
  const value =
    scoreType === "time"
      ? score.duration_s
      : scoreType === "reps"
        ? score.reps
        : score.weight_kg;
  if (value === undefined) {
    throw new Error(`Score does not contain the benchmark's ${scoreType} value`);
  }
  return value;
}

async function linkedPredictionAndResult(
  transaction: Transaction,
  linkData: {
    prediction: { record_id: string; revision: number };
    benchmark_result: { record_id: string; revision: number };
  },
): Promise<{
  prediction: ReturnType<typeof PredictionData.parse>;
  result: ReturnType<typeof BenchmarkResultData.parse>;
}> {
  const prediction = PredictionData.parse(
    await readTypedRevisionData(
      transaction,
      linkData.prediction.record_id,
      linkData.prediction.revision,
      "prediction",
    ),
  );
  const result = BenchmarkResultData.parse(
    await readTypedRevisionData(
      transaction,
      linkData.benchmark_result.record_id,
      linkData.benchmark_result.revision,
      "benchmark_result",
    ),
  );
  if (prediction.benchmark_definition_id !== result.benchmark_definition_id) {
    throw new Error("Prediction and benchmark result use different benchmark definitions");
  }
  return { prediction, result };
}

export async function validateTypedRelationships(
  transaction: Transaction,
  record: CurrentRecordT,
): Promise<void> {
  if (record.record_type === "prediction_result_link") {
    await linkedPredictionAndResult(transaction, record.data);
    return;
  }
  if (record.record_type !== "prediction_grade") {
    return;
  }

  const linkData = PredictionResultLinkData.parse(
    await readTypedRevisionData(
      transaction,
      record.data.prediction_result_link.record_id,
      record.data.prediction_result_link.revision,
      "prediction_result_link",
    ),
  );
  const { prediction, result } = await linkedPredictionAndResult(transaction, linkData);
  if (requiredJson(record.data.predicted_score) !== requiredJson(prediction.predicted_score)) {
    throw new Error("Saved grade predicted score does not match the linked prediction revision");
  }
  if (requiredJson(record.data.actual_score) !== requiredJson(result.score)) {
    throw new Error("Saved grade actual score does not match the linked result revision");
  }

  const benchmark = await transaction.execute({
    sql: "SELECT score_type FROM benchmark_definitions WHERE record_id = ?",
    args: [prediction.benchmark_definition_id],
  });
  const scoreType = benchmark.rows[0]?.score_type;
  if (typeof scoreType !== "string") {
    throw new Error("Linked benchmark definition does not exist");
  }
  const predictedValue = nativeScoreValue(prediction.predicted_score, scoreType);
  const actualValue = nativeScoreValue(result.score, scoreType);
  const signedError = Math.round((predictedValue - actualValue) * 100) / 100;
  const absoluteErrorPercentage =
    Math.round((Math.abs(signedError) / actualValue) * 1_000) / 10;
  const inRange =
    prediction.predicted_range !== undefined &&
    actualValue >= nativeScoreValue(prediction.predicted_range.low, scoreType) &&
    actualValue <= nativeScoreValue(prediction.predicted_range.high, scoreType);

  if (
    record.data.signed_error !== signedError ||
    record.data.absolute_error_percentage !== absoluteErrorPercentage ||
    record.data.result_in_predicted_range !== inRange
  ) {
    throw new Error("Saved grade arithmetic does not match its linked revisions");
  }
}

export async function insertRevision(
  transaction: Transaction,
  record: CurrentRecordT,
  changedBy: RecordActorT,
  becameKnownAt: string,
  becameKnownAtBasis: string,
  previousRevision: number | null,
): Promise<void> {
  const revision = RecordRevision.parse({
    record_id: record.record_id,
    record_type: record.record_type,
    revision: record.current_revision,
    previous_revision: previousRevision,
    became_known_at: becameKnownAt,
    became_known_at_basis: becameKnownAtBasis,
    changed_by: changedBy,
    data: record.data,
    extensions: record.extensions,
  });
  await transaction.execute({
    sql: `
      INSERT INTO record_revisions (
        record_id,
        revision,
        record_type,
        record_json,
        extensions_json,
        became_known_at,
        became_known_at_basis,
        changed_by_json,
        previous_revision
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
    `,
    args: [
      revision.record_id,
      revision.revision,
      revision.record_type,
      requiredJson(revision.data),
      serializeExtensionsJson(revision.extensions),
      revision.became_known_at,
      revision.became_known_at_basis,
      requiredJson(revision.changed_by),
      revision.previous_revision,
    ],
  });
}

export async function incrementDatabaseRevision(transaction: Transaction): Promise<number> {
  await transaction.execute(
    "UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1",
  );
  const result = await transaction.execute(
    "SELECT current_revision FROM database_state WHERE singleton_id = 1",
  );
  const value = result.rows[0]?.current_revision;
  if (typeof value !== "number" && typeof value !== "bigint") {
    throw new Error("Database revision is missing");
  }
  return Number(value);
}

export async function writeRecords(
  database: AthleticDatabase,
  inputs: readonly WriteRecordInput[],
  options: WriteRecordsOptions = {},
): Promise<{ records: CurrentRecordT[]; database_revision: number }> {
  if (inputs.length === 0) {
    throw new Error("writeRecords needs at least one record");
  }

  const actions: unknown[] = [];
  for (const expected of options.expected_current_revisions ?? []) {
    actions.push({
      kind: "expect_preview",
      record_id: RecordId.parse(expected.record_id),
      revision: expected.revision,
    });
  }
  const records: CurrentRecordT[] = [];
  for (const input of inputs) {
    const changedBy = RecordActor.parse(input.changed_by);
    const createdAt = UtcTimestamp.parse(input.created_at ?? new Date().toISOString());
    const becameKnownAt = UtcTimestamp.parse(input.became_known_at ?? createdAt);
    const becameKnownAtBasis = BecameKnownAtBasis.parse(
      input.became_known_at_basis ?? "local_write",
    );
    const record = CurrentRecord.parse({
      record_id: RecordId.parse(input.record_id ?? createUuidV7()),
      record_type: RecordType.parse(input.record_type),
      created_at: createdAt,
      current_revision: 1,
      is_deleted: false,
      data: input.data,
      extensions: input.extensions,
    });
    actions.push({
      kind: "insert",
      record_id: record.record_id,
      record_type: record.record_type,
      created_at: record.created_at,
      data_json: requiredJson(record.data),
      extensions_json: serializeExtensionsJson(record.extensions),
      changed_by_json: requiredJson(changedBy),
      became_known_at: becameKnownAt,
      became_known_at_basis: becameKnownAtBasis,
    });
    records.push(record);
  }
  actions.push({ kind: "validate" }, { kind: "bump" });
  const databaseRevision = await persistActions(database, actions);
  return { records, database_revision: databaseRevision };
}

export async function writeRecordChanges(
  database: AthleticDatabase,
  input: WriteRecordChangesInput,
): Promise<{ records: CurrentRecordT[]; database_revision: number }> {
  const corrections = input.corrections ?? [];
  const creates = input.records ?? [];
  if (corrections.length === 0 && creates.length === 0) {
    throw new Error("writeRecordChanges needs at least one change");
  }

  const actions: unknown[] = [];
  for (const expected of input.expected_current_revisions ?? []) {
    actions.push({
      kind: "expect_preview",
      record_id: RecordId.parse(expected.record_id),
      revision: expected.revision,
    });
  }
  const changedRecords: CurrentRecordT[] = [];
  const pushCreate = (create: WriteRecordInput): void => {
    const changedBy = RecordActor.parse(create.changed_by);
    const createdAt = UtcTimestamp.parse(create.created_at ?? new Date().toISOString());
    const becameKnownAt = UtcTimestamp.parse(create.became_known_at ?? createdAt);
    const becameKnownAtBasis = BecameKnownAtBasis.parse(
      create.became_known_at_basis ?? "local_write",
    );
    const record = CurrentRecord.parse({
      record_id: RecordId.parse(create.record_id ?? createUuidV7()),
      record_type: RecordType.parse(create.record_type),
      created_at: createdAt,
      current_revision: 1,
      is_deleted: false,
      data: create.data,
    });
    actions.push({
      kind: "insert",
      record_id: record.record_id,
      record_type: record.record_type,
      created_at: record.created_at,
      data_json: requiredJson(record.data),
      extensions_json: null,
      changed_by_json: requiredJson(changedBy),
      became_known_at: becameKnownAt,
      became_known_at_basis: becameKnownAtBasis,
    });
    changedRecords.push(record);
  };
  for (const create of creates) {
    if (create.record_type === "attachment") pushCreate(create);
  }
  for (const correction of corrections) {
    const recordId = RecordId.parse(correction.record_id);
    const changedBy = RecordActor.parse(correction.changed_by);
    const becameKnownAt = UtcTimestamp.parse(
      correction.became_known_at ?? new Date().toISOString(),
    );
    const becameKnownAtBasis = BecameKnownAtBasis.parse(
      correction.became_known_at_basis ?? "local_write",
    );
    const current = await databaseClient(database).execute({
      sql: `
        SELECT records.record_type, records.current_revision, records.created_at, records.is_deleted,
               record_revisions.became_known_at
        FROM records
        JOIN record_revisions
          ON record_revisions.record_id = records.record_id
         AND record_revisions.revision = records.current_revision
        WHERE records.record_id = ?
      `,
      args: [recordId],
    });
    const row = current.rows[0];
    if (row === undefined) throw new Error(`Record not found: ${recordId}`);
    const currentRevision = Number(row.current_revision);
    if (currentRevision !== correction.expected_revision || Number(row.is_deleted) !== 0) {
      throw new Error(`Record ${recordId} changed after preview; preview it again`);
    }
    if (Date.parse(becameKnownAt) < Date.parse(String(row.became_known_at))) {
      throw new Error(`Revision knowledge time cannot precede revision ${currentRevision}`);
    }
    const record = CurrentRecord.parse({
      record_id: recordId,
      record_type: row.record_type,
      created_at: row.created_at,
      current_revision: currentRevision + 1,
      is_deleted: false,
      data: correction.data,
    });
    actions.push({
      kind: "revise",
      style: "change",
      record_id: recordId,
      record_type: record.record_type,
      expected_revision: correction.expected_revision,
      data_json: requiredJson(record.data),
      extensions_json: null,
      keep_extensions: true,
      changed_by_json: requiredJson(changedBy),
      became_known_at: becameKnownAt,
      became_known_at_basis: becameKnownAtBasis,
    });
    changedRecords.push(record);
  }
  for (const create of creates) {
    if (create.record_type !== "attachment") pushCreate(create);
  }
  actions.push({ kind: "validate" }, { kind: "bump" });
  const databaseRevision = await persistActions(database, actions);
  return { records: changedRecords, database_revision: databaseRevision };
}

export async function writeRecord(
  database: AthleticDatabase,
  input: WriteRecordInput,
): Promise<{ record: CurrentRecordT; database_revision: number }> {
  const result = await writeRecords(database, [input]);
  return { record: result.records[0]!, database_revision: result.database_revision };
}

export async function correctRecord(
  database: AthleticDatabase,
  input: CorrectRecordInput,
): Promise<{ record: CurrentRecordT; database_revision: number }> {
  const recordId = RecordId.parse(input.record_id);
  const changedBy = RecordActor.parse(input.changed_by);
  const becameKnownAt = UtcTimestamp.parse(
    input.became_known_at ?? new Date().toISOString(),
  );
  const becameKnownAtBasis = BecameKnownAtBasis.parse(
    input.became_known_at_basis ?? "local_write",
  );

  const result = await databaseClient(database).execute({
    sql: `
      SELECT
        records.record_type,
        records.current_revision,
        records.created_at,
        records.is_deleted,
        record_revisions.became_known_at,
        record_revisions.extensions_json
      FROM records
      JOIN record_revisions
        ON record_revisions.record_id = records.record_id
       AND record_revisions.revision = records.current_revision
      WHERE records.record_id = ?
    `,
    args: [recordId],
  });
  const row = result.rows[0];
  if (row === undefined) {
    throw new Error(`Record not found: ${recordId}`);
  }
  const currentRevision = Number(row.current_revision);
  if (currentRevision !== input.expected_revision) {
    throw new Error(
      `Record ${recordId} is at revision ${currentRevision}, not expected revision ${input.expected_revision}`,
    );
  }
  if (Number(row.is_deleted) !== 0) {
    throw new Error(`Deleted record cannot be corrected: ${recordId}`);
  }
  if (Date.parse(becameKnownAt) < Date.parse(String(row.became_known_at))) {
    throw new Error(
      `Revision knowledge time cannot precede revision ${currentRevision}`,
    );
  }
  const record = CurrentRecord.parse({
    record_id: recordId,
    record_type: row.record_type,
    created_at: row.created_at,
    current_revision: currentRevision + 1,
    is_deleted: false,
    data: input.data,
    extensions: input.extensions ?? parseExtensionsJson(row.extensions_json ?? null),
  });
  const databaseRevision = await persistActions(database, [
    {
      kind: "revise",
      style: "correct",
      record_id: recordId,
      record_type: record.record_type,
      expected_revision: input.expected_revision,
      data_json: requiredJson(record.data),
      extensions_json: serializeExtensionsJson(record.extensions),
      keep_extensions: input.extensions === undefined,
      changed_by_json: requiredJson(changedBy),
      became_known_at: becameKnownAt,
      became_known_at_basis: becameKnownAtBasis,
    },
    { kind: "validate" },
    { kind: "bump" },
  ]);
  return { record, database_revision: databaseRevision };
}

export async function deleteRecord(
  database: AthleticDatabase,
  input: DeleteRecordInput,
): Promise<{ record_id: string; record_revision: number; database_revision: number }> {
  const recordId = RecordId.parse(input.record_id);
  const deletedBy = RecordActor.parse(input.deleted_by);
  const deletedAt = UtcTimestamp.parse(input.deleted_at ?? new Date().toISOString());
  const databaseRevision = await persistActions(database, [
    {
      kind: "delete",
      record_id: recordId,
      expected_revision: input.expected_revision,
      deleted_at: deletedAt,
      deleted_by_json: requiredJson(deletedBy),
    },
    { kind: "bump" },
  ]);
  return {
    record_id: recordId,
    record_revision: input.expected_revision,
    database_revision: databaseRevision,
  };
}

export async function readRecord(
  database: AthleticDatabase,
  recordId: string,
): Promise<CurrentRecordT | null> {
  const result = await databaseClient(database).execute({
    sql: `
      SELECT
        records.record_id,
        records.record_type,
        records.current_revision,
        records.created_at,
        records.is_deleted,
        record_revisions.record_json,
        record_revisions.extensions_json
      FROM records
      JOIN record_revisions
        ON record_revisions.record_id = records.record_id
       AND record_revisions.revision = records.current_revision
      WHERE records.record_id = ?
    `,
    args: [RecordId.parse(recordId)],
  });
  const row = result.rows[0];
  if (row === undefined) {
    return null;
  }
  return CurrentRecord.parse(currentRecordPayloadFromRow(row));
}

export async function listRecords(
  database: AthleticDatabase,
  options: ListRecordOptions = {},
): Promise<CurrentRecordT[]> {
  const conditions: string[] = [];
  const args: SqlValue[] = [];
  if (options.record_type !== undefined) {
    conditions.push("records.record_type = ?");
    args.push(RecordType.parse(options.record_type));
  }
  if (options.include_deleted !== true) {
    conditions.push("records.is_deleted = 0");
  }
  const limit = options.limit ?? 100;
  if (!Number.isInteger(limit) || limit < 1 || limit > 10_000) {
    throw new Error("Record list limit must be between 1 and 10000");
  }
  args.push(limit);

  const result = await databaseClient(database).execute({
    sql: `
      SELECT
        records.record_id,
        records.record_type,
        records.current_revision,
        records.created_at,
        records.is_deleted,
        record_revisions.record_json,
        record_revisions.extensions_json
      FROM records
      JOIN record_revisions
        ON record_revisions.record_id = records.record_id
       AND record_revisions.revision = records.current_revision
      ${conditions.length === 0 ? "" : `WHERE ${conditions.join(" AND ")}`}
      ORDER BY records.created_at, records.record_id
      LIMIT ?
    `,
    args,
  });

  return result.rows.map((row) => CurrentRecord.parse(currentRecordPayloadFromRow(row)));
}

export async function readRecordRevisions(
  database: AthleticDatabase,
  recordId: string,
): Promise<RecordRevisionT[]> {
  const parsedRecordId = RecordId.parse(recordId);
  const result = await databaseClient(database).execute({
    sql: `
      SELECT
        record_id,
        record_type,
        revision,
        previous_revision,
        became_known_at,
        became_known_at_basis,
        changed_by_json,
        record_json,
        extensions_json
      FROM record_revisions
      WHERE record_id = ?
      ORDER BY revision
    `,
    args: [parsedRecordId],
  });
  return result.rows.map((row) => RecordRevision.parse(recordRevisionPayloadFromRow(row)));
}
