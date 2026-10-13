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
import { databaseClient, type AthleticDatabase } from "./client.js";
import { withWriteTransaction } from "./transaction.js";
import { createUuidV7 } from "./uuid.js";

type SqlValue = string | number | bigint | boolean | Uint8Array | ArrayBuffer | null;

export interface WriteRecordInput {
  record_type: RecordTypeT;
  data: unknown;
  changed_by: RecordActorT;
  record_id?: string;
  created_at?: string;
  became_known_at?: string;
  became_known_at_basis?: "local_write" | "provider_import";
}

export interface CorrectRecordInput {
  record_id: string;
  expected_revision: number;
  data: unknown;
  changed_by: RecordActorT;
  became_known_at?: string;
  became_known_at_basis?: "local_write" | "provider_import" | "package_merge";
}

export interface ListRecordOptions {
  record_type?: RecordTypeT;
  include_deleted?: boolean;
  limit?: number;
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
  const columns = ["record_id", "record_type", ...fields.map(([name]) => name), "data_json"];
  const values: SqlValue[] = [
    record.record_id,
    record.record_type,
    ...fields.map(([, value]) => value),
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

async function upsertTypedRecord(
  transaction: Transaction,
  record: CurrentRecordT,
): Promise<void> {
  switch (record.record_type) {
    case "athlete_profile":
      await upsertTypedRow(transaction, "athlete_profile", record, [
        ["default_time_zone", nullable(record.data.default_time_zone)],
      ]);
      break;
    case "source":
      await upsertTypedRow(transaction, "sources", record, [
        ["source_type", record.data.source_type],
        ["data_writer", record.data.data_writer],
        ["data_route", record.data.data_route],
        ["vendor", nullable(record.data.vendor)],
        ["sensor_type", nullable(record.data.sensor_type)],
      ]);
      break;
    case "source_device":
      await upsertTypedRow(transaction, "source_devices", record, [
        ["source_id", record.data.source_id],
        ["name", nullable(record.data.name)],
        ["manufacturer", nullable(record.data.manufacturer)],
        ["model", nullable(record.data.model)],
        ["hardware", nullable(record.data.hardware)],
        ["serial_number", nullable(record.data.serial_number)],
        ["coverage_start_date", nullable(record.data.coverage_start_date)],
        ["coverage_end_date", nullable(record.data.coverage_end_date)],
        ["accepted_record_count", nullable(record.data.accepted_record_count)],
      ]);
      break;
    case "point_measurement":
      await upsertTypedRow(transaction, "point_measurements", record, [
        ["source_id", record.data.source_id],
        ["quantity", record.data.quantity],
        ["recorded_at", nullable(record.data.recorded_at)],
        ["standard_value", record.data.value.standard_value],
        ["standard_unit", record.data.value.standard_unit],
        ["provider_record_id", nullable(record.data.provider_record_id)],
        ["received_at", record.data.received_at],
      ]);
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
      await upsertTypedRow(transaction, "measurement_samples", record, [
        ["source_id", record.data.source_id],
        ["quantity", record.data.quantity],
        ["recorded_at", record.data.recorded_at],
        ["standard_value", record.data.value.standard_value],
        ["standard_unit", record.data.value.standard_unit],
        ["duration_ms", nullable(record.data.duration_ms)],
        ["provider_record_id", nullable(record.data.provider_record_id)],
        ["received_at", record.data.received_at],
      ]);
      break;
    case "sleep_session":
      await upsertTypedRow(transaction, "sleep_sessions", record, [
        ["source_id", record.data.source_id],
        ["start_at", record.data.start_at],
        ["end_at", record.data.end_at],
        ["provider_record_id", nullable(record.data.provider_record_id)],
        ["received_at", record.data.received_at],
      ]);
      break;
    case "workout_session":
      await upsertTypedRow(transaction, "workout_sessions", record, [
        ["source_id", record.data.source_id],
        ["start_at", record.data.start_at],
        ["end_at", record.data.end_at],
        ["activity_type", nullable(record.data.activity_type)],
        ["provider_record_id", nullable(record.data.provider_record_id)],
        ["received_at", record.data.received_at],
      ]);
      await replaceOrderedJsonChildren(
        transaction,
        "workout_session_segments",
        "workout_session_id",
        "segment_order",
        "segment_json",
        record.record_id,
        record.data.segments ?? [],
      );
      break;
    case "benchmark_result":
      await upsertTypedRow(transaction, "benchmark_results", record, [
        ["source_id", record.data.source_id],
        ["benchmark_definition_id", record.data.benchmark_definition_id],
        ["recorded_at", record.data.recorded_at],
        ["workout_session_id", nullable(record.data.workout_session_id)],
        ["provider_record_id", nullable(record.data.provider_record_id)],
        ["received_at", record.data.received_at],
      ]);
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
      await upsertTypedRow(transaction, "provider_scores", record, [
        ["source_id", record.data.source_id],
        ["metric", record.data.metric],
        ["recorded_at", nullable(record.data.recorded_at)],
        ["standard_value", record.data.value.standard_value],
        ["standard_unit", record.data.value.standard_unit],
        ["scale", record.data.scale],
        ["provider_record_id", nullable(record.data.provider_record_id)],
        ["received_at", record.data.received_at],
      ]);
      break;
    case "athlete_report":
      await upsertTypedRow(transaction, "athlete_reports", record, [
        ["report_type", record.data.report_type],
        ["reported_at", record.data.reported_at],
      ]);
      await replaceAttachmentLinks(
        transaction,
        record.record_id,
        record.data.attachment_ids ?? [],
      );
      break;
    case "nutrition_entry":
      await upsertTypedRow(transaction, "nutrition_entries", record, [
        ["reported_at", record.data.reported_at],
        ["athlete_description", record.data.athlete_description],
      ]);
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
      await upsertTypedRow(transaction, "attachments", record, [
        ["file_name", record.data.file_name],
        ["media_type", record.data.media_type],
        ["byte_count", record.data.byte_count],
        ["sha256", record.data.sha256],
      ]);
      break;
    case "benchmark_definition":
      await upsertTypedRow(transaction, "benchmark_definitions", record, [
        ["benchmark_key", record.data.benchmark_key],
        ["benchmark_type", record.data.benchmark_type],
        ["score_type", record.data.score_type],
        ["definition", record.data.definition],
      ]);
      break;
    case "prediction":
      await upsertTypedRow(transaction, "predictions", record, [
        ["benchmark_definition_id", record.data.benchmark_definition_id],
        ["authored_at", record.data.authorship.authored_at],
        ["author_type", record.data.authorship.author.author_type],
        ["content_version", record.data.authorship.content_version],
        ["status", record.data.authorship.status],
      ]);
      break;
    case "prediction_result_link":
      await upsertTypedRow(transaction, "prediction_result_links", record, [
        ["prediction_id", record.data.prediction.record_id],
        ["prediction_revision", record.data.prediction.revision],
        ["benchmark_result_id", record.data.benchmark_result.record_id],
        ["benchmark_result_revision", record.data.benchmark_result.revision],
        ["linked_at", record.data.linked_at],
        ["created_by_type", record.data.created_by.author_type],
        ["content_version", record.data.content_version],
        ["status", record.data.status],
      ]);
      break;
    case "prediction_grade":
      await upsertTypedRow(transaction, "prediction_grades", record, [
        ["prediction_result_link_id", record.data.prediction_result_link.record_id],
        ["prediction_result_link_revision", record.data.prediction_result_link.revision],
        ["authored_at", record.data.authorship.authored_at],
        ["author_type", record.data.authorship.author.author_type],
        ["content_version", record.data.authorship.content_version],
        ["status", record.data.authorship.status],
        ["signed_error", record.data.signed_error],
        ["absolute_error_percentage", record.data.absolute_error_percentage],
        ["result_in_predicted_range", record.data.result_in_predicted_range ? 1 : 0],
      ]);
      break;
    case "prediction_error_analysis":
      await upsertTypedRow(transaction, "prediction_error_analyses", record, [
        ["prediction_grade_id", record.data.prediction_grade.record_id],
        ["prediction_grade_revision", record.data.prediction_grade.revision],
        ["authored_at", record.data.authorship.authored_at],
        ["author_type", record.data.authorship.author.author_type],
        ["content_version", record.data.authorship.content_version],
        ["status", record.data.authorship.status],
      ]);
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
      await upsertTypedRow(transaction, "training_plans", record, [
        ["title", record.data.title],
        ["start_date", nullable(record.data.start_date)],
        ["end_date", nullable(record.data.end_date)],
        ["authored_at", record.data.authorship.authored_at],
        ["author_type", record.data.authorship.author.author_type],
        ["content_version", record.data.authorship.content_version],
        ["status", record.data.authorship.status],
      ]);
      break;
    case "planned_session":
      await upsertTypedRow(transaction, "planned_sessions", record, [
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
      ]);
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
      await upsertTypedRow(transaction, "coaching_notes", record, [
        ["authored_at", record.data.authorship.authored_at],
        ["author_type", record.data.authorship.author.author_type],
        ["content_version", record.data.authorship.content_version],
        ["status", record.data.authorship.status],
      ]);
      break;
    case "saved_training_review":
      await upsertTypedRow(transaction, "saved_training_reviews", record, [
        ["period_start", record.data.period_start],
        ["period_end", record.data.period_end],
        ["authored_at", record.data.authorship.authored_at],
        ["author_type", record.data.authorship.author.author_type],
        ["content_version", record.data.authorship.content_version],
        ["status", record.data.authorship.status],
      ]);
      break;
    case "goal":
      await upsertTypedRow(transaction, "goals", record, [
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
      ]);
      break;
    case "nutrition_model_interpretation":
      await upsertTypedRow(transaction, "nutrition_model_interpretations", record, [
        ["nutrition_entry_id", record.data.nutrition_entry_id],
        ["confidence", record.data.confidence],
        ["authored_at", record.data.authorship.authored_at],
        ["author_type", record.data.authorship.author.author_type],
        ["content_version", record.data.authorship.content_version],
        ["status", record.data.authorship.status],
      ]);
      break;
  }

  await replaceProviderReportingPeriods(transaction, record);
  await replaceAuthoredEvidence(transaction, record);
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

async function validateTypedRelationships(
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

async function insertRevision(
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
  });
  await transaction.execute({
    sql: `
      INSERT INTO record_revisions (
        record_id,
        revision,
        record_type,
        record_json,
        became_known_at,
        became_known_at_basis,
        changed_by_json,
        previous_revision
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
    `,
    args: [
      revision.record_id,
      revision.revision,
      revision.record_type,
      requiredJson(revision.data),
      revision.became_known_at,
      revision.became_known_at_basis,
      requiredJson(revision.changed_by),
      revision.previous_revision,
    ],
  });
}

async function incrementDatabaseRevision(transaction: Transaction): Promise<number> {
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
): Promise<{ records: CurrentRecordT[]; database_revision: number }> {
  if (inputs.length === 0) {
    throw new Error("writeRecords needs at least one record");
  }

  return withWriteTransaction(database, async (transaction) => {
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
      });

      await transaction.execute({
        sql: `
          INSERT INTO records (
            record_id,
            record_type,
            current_revision,
            created_at,
            is_deleted
          ) VALUES (?, ?, 1, ?, 0)
        `,
        args: [record.record_id, record.record_type, record.created_at],
      });
      await insertRevision(
        transaction,
        record,
        changedBy,
        becameKnownAt,
        becameKnownAtBasis,
        null,
      );
      await upsertTypedRecord(transaction, record);
      records.push(record);
    }
    for (const record of records) {
      await validateTypedRelationships(transaction, record);
    }
    const databaseRevision = await incrementDatabaseRevision(transaction);
    return { records, database_revision: databaseRevision };
  });
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

  return withWriteTransaction(database, async (transaction) => {
    const result = await transaction.execute({
      sql: `
        SELECT
          records.record_type,
          records.current_revision,
          records.created_at,
          records.is_deleted,
          record_revisions.became_known_at
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
    });

    await insertRevision(
      transaction,
      record,
      changedBy,
      becameKnownAt,
      becameKnownAtBasis,
      currentRevision,
    );
    await upsertTypedRecord(transaction, record);
    await validateTypedRelationships(transaction, record);
    await transaction.execute({
      sql: "UPDATE records SET current_revision = ? WHERE record_id = ?",
      args: [record.current_revision, record.record_id],
    });
    const databaseRevision = await incrementDatabaseRevision(transaction);
    return { record, database_revision: databaseRevision };
  });
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
        record_revisions.record_json
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
  return CurrentRecord.parse({
    record_id: row.record_id,
    record_type: row.record_type,
    current_revision: Number(row.current_revision),
    created_at: row.created_at,
    is_deleted: Number(row.is_deleted) === 1,
    data: JSON.parse(String(row.record_json)) as unknown,
  });
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
        record_revisions.record_json
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

  return result.rows.map((row) =>
    CurrentRecord.parse({
      record_id: row.record_id,
      record_type: row.record_type,
      current_revision: Number(row.current_revision),
      created_at: row.created_at,
      is_deleted: Number(row.is_deleted) === 1,
      data: JSON.parse(String(row.record_json)) as unknown,
    }),
  );
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
        record_json
      FROM record_revisions
      WHERE record_id = ?
      ORDER BY revision
    `,
    args: [parsedRecordId],
  });
  return result.rows.map((row) =>
    RecordRevision.parse({
      record_id: row.record_id,
      record_type: row.record_type,
      revision: Number(row.revision),
      previous_revision:
        row.previous_revision === null ? null : Number(row.previous_revision),
      became_known_at: row.became_known_at,
      became_known_at_basis: row.became_known_at_basis,
      changed_by: JSON.parse(String(row.changed_by_json)) as unknown,
      data: JSON.parse(String(row.record_json)) as unknown,
    }),
  );
}
