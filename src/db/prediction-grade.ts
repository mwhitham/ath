import {
  type CurrentRecordT,
  type RecordActorT,
} from "../v1/schema.js";
import { databaseClient, type AthleticDatabase } from "./client.js";
import { createUuidV7 } from "./uuid.js";
import { readRecord, writeRecords, type WriteRecordInput } from "./records.js";

export interface PredictionGradePlan {
  prediction: Extract<CurrentRecordT, { record_type: "prediction" }>;
  result: Extract<CurrentRecordT, { record_type: "benchmark_result" }>;
  existing_link: Extract<
    CurrentRecordT,
    { record_type: "prediction_result_link" }
  > | null;
  link_input: WriteRecordInput | null;
  grade_input: WriteRecordInput;
}

function scoreValue(
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
    throw new Error(`score does not contain the benchmark's ${scoreType} value`);
  }
  return value;
}

async function currentRecordOrThrow(
  database: AthleticDatabase,
  recordId: string,
): Promise<CurrentRecordT> {
  const record = await readRecord(database, recordId);
  if (record === null || record.is_deleted) {
    throw new Error(`current record not found: ${recordId}`);
  }
  return record;
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
  const predictionRecord = await currentRecordOrThrow(database, input.prediction_id);
  if (predictionRecord.record_type !== "prediction") {
    throw new Error(`${input.prediction_id} is not a prediction`);
  }
  const resultRecord = await currentRecordOrThrow(database, input.result_id);
  if (resultRecord.record_type !== "benchmark_result") {
    throw new Error(`${input.result_id} is not a benchmark result`);
  }
  if (
    predictionRecord.data.benchmark_definition_id !==
    resultRecord.data.benchmark_definition_id
  ) {
    throw new Error("prediction and result use different benchmark definitions");
  }

  const benchmark = await databaseClient(database).execute({
    sql: "SELECT score_type FROM benchmark_definitions WHERE record_id = ?",
    args: [predictionRecord.data.benchmark_definition_id],
  });
  const scoreType = benchmark.rows[0]?.score_type;
  if (typeof scoreType !== "string") {
    throw new Error("prediction benchmark definition is missing");
  }
  const predictedValue = scoreValue(predictionRecord.data.predicted_score, scoreType);
  const actualValue = scoreValue(resultRecord.data.score, scoreType);
  const signedError = Math.round((predictedValue - actualValue) * 100) / 100;
  const absoluteErrorPercentage =
    Math.round((Math.abs(signedError) / actualValue) * 1_000) / 10;
  const resultInRange =
    predictionRecord.data.predicted_range !== undefined &&
    actualValue >=
      scoreValue(predictionRecord.data.predicted_range.low, scoreType) &&
    actualValue <=
      scoreValue(predictionRecord.data.predicted_range.high, scoreType);

  const existingLinkResult = await databaseClient(database).execute({
    sql: `
      SELECT records.record_id
      FROM prediction_result_links
      JOIN records ON records.record_id = prediction_result_links.record_id
      WHERE
        records.is_deleted = 0
        AND prediction_result_links.prediction_id = ?
        AND prediction_result_links.prediction_revision = ?
        AND prediction_result_links.benchmark_result_id = ?
        AND prediction_result_links.benchmark_result_revision = ?
      ORDER BY records.created_at, records.record_id
      LIMIT 1
    `,
    args: [
      predictionRecord.record_id,
      predictionRecord.current_revision,
      resultRecord.record_id,
      resultRecord.current_revision,
    ],
  });
  const existingLinkId = existingLinkResult.rows[0]?.record_id;
  const existingLink =
    typeof existingLinkId === "string"
      ? await currentRecordOrThrow(database, existingLinkId)
      : null;
  if (existingLink !== null && existingLink.record_type !== "prediction_result_link") {
    throw new Error("stored prediction-result link has the wrong record type");
  }

  const createdAt = input.created_at ?? new Date().toISOString();
  const linkId = existingLink?.record_id ?? createUuidV7();
  const linkRevision = existingLink?.current_revision ?? 1;
  const linkInput: WriteRecordInput | null =
    existingLink === null
      ? {
          record_type: "prediction_result_link",
          record_id: linkId,
          created_at: createdAt,
          changed_by: input.actor,
          data: {
            prediction: {
              record_id: predictionRecord.record_id,
              revision: predictionRecord.current_revision,
            },
            benchmark_result: {
              record_id: resultRecord.record_id,
              revision: resultRecord.current_revision,
            },
            created_by: input.actor,
            linked_at: createdAt,
            content_version: 1,
            status: "active",
          },
        }
      : null;
  const calculationVersion =
    input.calculation_version ?? "prediction-grade-v1";
  const gradeInput: WriteRecordInput = {
    record_type: "prediction_grade",
    record_id: createUuidV7(),
    created_at: createdAt,
    changed_by: input.actor,
    data: {
      authorship: {
        author: {
          author_type: "tool",
          tool: "ath",
          tool_version: "1.0.0",
          calculation_version: calculationVersion,
        },
        authored_at: createdAt,
        evidence: [{ record_id: linkId, revision: linkRevision }],
        content_version: 1,
        status: "active",
      },
      prediction_result_link: {
        record_id: linkId,
        revision: linkRevision,
      },
      signed_error: signedError,
      absolute_error_percentage: absoluteErrorPercentage,
      result_in_predicted_range: resultInRange,
      predicted_score: predictionRecord.data.predicted_score,
      actual_score: resultRecord.data.score,
    },
  };

  return {
    prediction: predictionRecord,
    result: resultRecord,
    existing_link: existingLink,
    link_input: linkInput,
    grade_input: gradeInput,
  };
}

export async function gradePrediction(
  database: AthleticDatabase,
  plan: PredictionGradePlan,
): Promise<{
  link: CurrentRecordT;
  grade: CurrentRecordT;
  database_revision: number;
}> {
  const inputs =
    plan.link_input === null
      ? [plan.grade_input]
      : [plan.link_input, plan.grade_input];
  const written = await writeRecords(database, inputs, {
    expected_current_revisions: [
      {
        record_id: plan.prediction.record_id,
        revision: plan.prediction.current_revision,
      },
      {
        record_id: plan.result.record_id,
        revision: plan.result.current_revision,
      },
      ...(plan.existing_link === null
        ? []
        : [
            {
              record_id: plan.existing_link.record_id,
              revision: plan.existing_link.current_revision,
            },
          ]),
    ],
  });
  const link =
    plan.existing_link ??
    written.records.find((record) => record.record_type === "prediction_result_link");
  const grade = written.records.find(
    (record) => record.record_type === "prediction_grade",
  );
  if (link === undefined || grade === undefined) {
    throw new Error("prediction grade write did not return its link and grade");
  }
  return { link, grade, database_revision: written.database_revision };
}
