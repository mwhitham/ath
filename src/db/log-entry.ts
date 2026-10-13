import {
  CurrentRecord,
  POINT_MEASUREMENT_UNITS,
  PointMeasurementTypeSchema,
  RecordType,
  UtcTimestamp,
  type RecordActorT,
} from "../v1/schema.js";
import { databaseClient, type AthleticDatabase } from "./client.js";
import {
  readRecord,
  writeRecords,
  type ExpectedCurrentRevision,
  type WriteRecordInput,
} from "./records.js";
import { createUuidV7 } from "./uuid.js";

export interface LogPlan {
  inputs: WriteRecordInput[];
  expected_current_revisions: ExpectedCurrentRevision[];
  interpretation: string;
}

export interface StructuredLogInput {
  record_type: string;
  data: unknown;
  record_id?: string;
  created_at?: string;
}

const quantityAliases: Readonly<Record<string, string>> = {
  "hrv rmssd": "hrv_rmssd",
  rmssd: "hrv_rmssd",
  sdnn: "hrv_sdnn",
  "resting heart rate": "resting_heart_rate",
  rhr: "resting_heart_rate",
  weight: "body_weight",
  "body weight": "body_weight",
  "respiratory rate": "respiratory_rate",
  "oxygen saturation": "oxygen_saturation",
  "body temperature": "body_temperature",
  "skin temperature": "skin_temperature",
  "body fat": "body_fat_percentage",
};

function inputTimestamp(timestamp: string) {
  return {
    timestamp_text: timestamp,
    utc_offset_text: "Z",
    utc_offset_meaning: "documented_recording_offset" as const,
    timestamp_basis: "source_instant" as const,
  };
}

async function manualSourceId(database: AthleticDatabase): Promise<string> {
  const result = await databaseClient(database).execute(`
    SELECT sources.record_id
    FROM sources
    JOIN records ON records.record_id = sources.record_id
    WHERE sources.source_type = 'manual' AND records.is_deleted = 0
    ORDER BY records.created_at, records.record_id
    LIMIT 1
  `);
  const value = result.rows[0]?.record_id;
  if (typeof value !== "string") {
    throw new Error("manual source is missing; run ath check");
  }
  return value;
}

async function benchmarkByKey(
  database: AthleticDatabase,
  benchmarkKey: string,
): Promise<{ record_id: string; score_type: string } | null> {
  const result = await databaseClient(database).execute({
    sql: `
      SELECT benchmark_definitions.record_id, benchmark_definitions.score_type
      FROM benchmark_definitions
      JOIN records ON records.record_id = benchmark_definitions.record_id
      WHERE benchmark_definitions.benchmark_key = ? AND records.is_deleted = 0
    `,
    args: [benchmarkKey],
  });
  const row = result.rows[0];
  return row === undefined
    ? null
    : { record_id: String(row.record_id), score_type: String(row.score_type) };
}

function parseDuration(text: string): number | null {
  const match = /^(\d+):([0-5]\d)$/.exec(text);
  if (match === null) {
    return null;
  }
  return Number(match[1]) * 60 + Number(match[2]);
}

function normalizedBenchmarkKey(value: string): string {
  const key = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  if (key === "") {
    throw new Error("benchmark name does not contain a usable name");
  }
  return key;
}

function scoreValue(
  score: { duration_s?: number; reps?: number; weight_kg?: number },
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

async function addAutomaticGrades(
  database: AthleticDatabase,
  plan: LogPlan,
  actor: RecordActorT,
  createdAt: string,
): Promise<LogPlan> {
  const additions: WriteRecordInput[] = [];
  const expected = [...plan.expected_current_revisions];
  for (const resultInput of plan.inputs.filter(
    (input) => input.record_type === "benchmark_result",
  )) {
    const result = CurrentRecord.parse({
      record_id: resultInput.record_id,
      record_type: resultInput.record_type,
      created_at: resultInput.created_at,
      current_revision: 1,
      is_deleted: false,
      data: resultInput.data,
    });
    if (result.record_type !== "benchmark_result") {
      continue;
    }
    const predictionRows = await databaseClient(database).execute({
      sql: `
        SELECT predictions.record_id
        FROM predictions
        JOIN records ON records.record_id = predictions.record_id
        WHERE
          records.is_deleted = 0
          AND predictions.benchmark_definition_id = ?
          AND predictions.status = 'active'
          AND NOT EXISTS (
            SELECT 1
            FROM prediction_result_links
            JOIN records AS link_records
              ON link_records.record_id = prediction_result_links.record_id
            WHERE
              link_records.is_deleted = 0
              AND prediction_result_links.prediction_id = predictions.record_id
              AND prediction_result_links.prediction_revision = records.current_revision
          )
        ORDER BY predictions.authored_at DESC, predictions.record_id
        LIMIT 2
      `,
      args: [result.data.benchmark_definition_id],
    });
    if (predictionRows.rows.length !== 1) {
      continue;
    }
    const predictionId = String(predictionRows.rows[0]!.record_id);
    const prediction = await readRecord(database, predictionId);
    if (prediction?.record_type !== "prediction" || prediction.is_deleted) {
      continue;
    }
    const benchmark = await databaseClient(database).execute({
      sql: "SELECT score_type FROM benchmark_definitions WHERE record_id = ?",
      args: [result.data.benchmark_definition_id],
    });
    const scoreType = benchmark.rows[0]?.score_type;
    if (typeof scoreType !== "string") {
      throw new Error("benchmark definition is missing while planning automatic grade");
    }
    const predictedValue = scoreValue(prediction.data.predicted_score, scoreType);
    const actualValue = scoreValue(result.data.score, scoreType);
    const signedError = Math.round((predictedValue - actualValue) * 100) / 100;
    const absoluteErrorPercentage =
      Math.round((Math.abs(signedError) / actualValue) * 1_000) / 10;
    const resultInRange =
      prediction.data.predicted_range !== undefined &&
      actualValue >= scoreValue(prediction.data.predicted_range.low, scoreType) &&
      actualValue <= scoreValue(prediction.data.predicted_range.high, scoreType);
    const linkId = createUuidV7();
    additions.push(
      {
        record_type: "prediction_result_link",
        record_id: linkId,
        created_at: createdAt,
        changed_by: actor,
        data: {
          prediction: {
            record_id: prediction.record_id,
            revision: prediction.current_revision,
          },
          benchmark_result: {
            record_id: result.record_id,
            revision: 1,
          },
          created_by: actor,
          linked_at: createdAt,
          content_version: 1,
          status: "active",
        },
      },
      {
        record_type: "prediction_grade",
        record_id: createUuidV7(),
        created_at: createdAt,
        changed_by: {
          author_type: "tool",
          tool: "ath",
          tool_version: "1.0.0",
        },
        data: {
          authorship: {
            author: {
              author_type: "tool",
              tool: "ath",
              tool_version: "1.0.0",
              calculation_version: "prediction-grade-v1",
            },
            authored_at: createdAt,
            evidence: [{ record_id: linkId, revision: 1 }],
            content_version: 1,
            status: "active",
          },
          prediction_result_link: { record_id: linkId, revision: 1 },
          signed_error: signedError,
          absolute_error_percentage: absoluteErrorPercentage,
          result_in_predicted_range: resultInRange,
          predicted_score: prediction.data.predicted_score,
          actual_score: result.data.score,
        },
      },
    );
    expected.push({
      record_id: prediction.record_id,
      revision: prediction.current_revision,
    });
  }
  if (additions.length === 0) {
    return plan;
  }
  return {
    interpretation:
      `${plan.interpretation}; added ${additions.length / 2} unambiguous ` +
      `prediction link and grade`,
    inputs: [...plan.inputs, ...additions],
    expected_current_revisions: expected,
  };
}

async function planPlainText(
  database: AthleticDatabase,
  text: string,
  actor: RecordActorT,
  createdAt: string,
  benchmarkOverride?: string,
): Promise<LogPlan> {
  const trimmed = text.trim();
  if (trimmed === "") {
    throw new Error("nothing to log");
  }
  if (/\bhrv\b/i.test(trimmed) && !/\b(?:rmssd|sdnn)\b/i.test(trimmed)) {
    throw new Error(
      "HRV is ambiguous. Say RMSSD or SDNN so the two statistics never share a baseline.",
    );
  }

  const measurementMatch = /^(.+?)\s+(-?\d+(?:\.\d+)?)\s*(\S+)?(?:\s+.*)?$/i.exec(
    trimmed,
  );
  if (measurementMatch !== null) {
    const quantityName = measurementMatch[1]!.trim().toLowerCase();
    const aliased = quantityAliases[quantityName];
    const parsedQuantity =
      aliased === undefined ? undefined : PointMeasurementTypeSchema.safeParse(aliased);
    if (parsedQuantity?.success) {
      const value = Number(measurementMatch[2]);
      const expectedUnit = POINT_MEASUREMENT_UNITS[parsedQuantity.data];
      const inputUnit = measurementMatch[3] ?? expectedUnit;
      if (inputUnit.toLowerCase() !== expectedUnit.toLowerCase()) {
        throw new Error(
          `${parsedQuantity.data} must be logged in ${expectedUnit}; '${inputUnit}' is not converted by plain-text log`,
        );
      }
      const sourceId = await manualSourceId(database);
      return {
        interpretation: `manual ${parsedQuantity.data} measurement`,
        expected_current_revisions: [],
        inputs: [
          {
            record_type: "point_measurement",
            record_id: createUuidV7(),
            created_at: createdAt,
            changed_by: actor,
            data: {
              source_id: sourceId,
              received_at: createdAt,
              received_at_basis: "local_write_time",
              quantity: parsedQuantity.data,
              recorded_at: createdAt,
              input_timestamp: inputTimestamp(createdAt),
              value: {
                standard_value: value,
                standard_unit: expectedUnit,
                input_value: value,
                input_unit: inputUnit,
              },
            },
          },
        ],
      };
    }
  }

  const resultMatch = /^(.*?)(?:\s+in)?\s+(\d+:[0-5]\d)(?:\s+(rx|scaled))?$/i.exec(
    trimmed,
  );
  if (resultMatch !== null) {
    const duration = parseDuration(resultMatch[2]!);
    const suppliedName = benchmarkOverride ?? resultMatch[1]!.trim();
    const benchmarkKey = normalizedBenchmarkKey(suppliedName || createdAt.slice(0, 10));
    let benchmark = await benchmarkByKey(database, benchmarkKey);
    const inputs: WriteRecordInput[] = [];
    if (benchmark === null) {
      const benchmarkId = createUuidV7();
      inputs.push({
        record_type: "benchmark_definition",
        record_id: benchmarkId,
        created_at: createdAt,
        changed_by: actor,
        data: {
          authorship: {
            author: { author_type: "athlete" },
            authored_at: createdAt,
            evidence: [],
            content_version: 1,
            status: "active",
          },
          benchmark_key: benchmarkKey,
          benchmark_type: "custom",
          score_type: "time",
          definition: suppliedName || text,
        },
      });
      benchmark = { record_id: benchmarkId, score_type: "time" };
    }
    const sourceId = await manualSourceId(database);
    inputs.push({
      record_type: "benchmark_result",
      record_id: createUuidV7(),
      created_at: createdAt,
      changed_by: actor,
      data: {
        source_id: sourceId,
        received_at: createdAt,
        received_at_basis: "local_write_time",
        benchmark_definition_id: benchmark.record_id,
        recorded_at: createdAt,
        input_timestamp: inputTimestamp(createdAt),
        score: { duration_s: duration! },
        input_score_text: resultMatch[2],
        ...(resultMatch[3] === undefined
          ? {}
          : { scaling: resultMatch[3].toLowerCase() }),
      },
    });
    return {
      interpretation: `${benchmarkKey} benchmark result`,
      inputs,
      expected_current_revisions: [],
    };
  }

  const ratingMatch = /^(.*?)(\d+(?:\.\d+)?)\s*\/\s*(\d+(?:\.\d+)?)$/i.exec(
    trimmed,
  );
  if (ratingMatch !== null) {
    const words = ratingMatch[1]!.trim();
    const reportType =
      /\bsore|soreness|aching|ache\b/i.test(words)
        ? "soreness"
        : /\bstress|stressed\b/i.test(words)
          ? "stress"
          : /\bmood\b/i.test(words)
            ? "mood"
            : /\benergy|energetic\b/i.test(words)
              ? "energy"
              : "note";
    return {
      interpretation: `${reportType} athlete report`,
      expected_current_revisions: [],
      inputs: [
        {
          record_type: "athlete_report",
          record_id: createUuidV7(),
          created_at: createdAt,
          changed_by: actor,
          data: {
            report_type: reportType,
            reported_at: createdAt,
            input_reported_at: inputTimestamp(createdAt),
            reported_via: "text",
            rating: Number(ratingMatch[2]),
            rating_scale: `1-${ratingMatch[3]}`,
            note: trimmed,
          },
        },
      ],
    };
  }

  return {
    interpretation: "athlete note",
    expected_current_revisions: [],
    inputs: [
      {
        record_type: "athlete_report",
        record_id: createUuidV7(),
        created_at: createdAt,
        changed_by: actor,
        data: {
          report_type: "note",
          reported_at: createdAt,
          input_reported_at: inputTimestamp(createdAt),
          reported_via: "text",
          note: trimmed,
        },
      },
    ],
  };
}

function planStructured(
  value: unknown,
  actor: RecordActorT,
  createdAt: string,
): LogPlan {
  const items = Array.isArray(value) ? value : [value];
  if (items.length === 0) {
    throw new Error("structured log array cannot be empty");
  }
  const inputs: WriteRecordInput[] = items.map((item, index) => {
    if (typeof item !== "object" || item === null || Array.isArray(item)) {
      throw new Error(`structured log item ${index + 1} must be an object`);
    }
    const raw = item as Record<string, unknown>;
    const recordType = RecordType.parse(raw.record_type);
    const recordId =
      typeof raw.record_id === "string" ? raw.record_id : createUuidV7();
    const itemCreatedAt =
      typeof raw.created_at === "string"
        ? UtcTimestamp.parse(raw.created_at)
        : createdAt;
    CurrentRecord.parse({
      record_id: recordId,
      record_type: recordType,
      created_at: itemCreatedAt,
      current_revision: 1,
      is_deleted: false,
      data: raw.data,
    });
    return {
      record_type: recordType,
      record_id: recordId,
      created_at: itemCreatedAt,
      changed_by: actor,
      data: raw.data,
    };
  });
  return {
    interpretation: `${inputs.length} structured record${inputs.length === 1 ? "" : "s"}`,
    inputs,
    expected_current_revisions: [],
  };
}

export async function planLogEntry(
  database: AthleticDatabase,
  input: {
    text: string;
    actor: RecordActorT;
    created_at?: string;
    benchmark?: string;
  },
): Promise<LogPlan> {
  const createdAt = UtcTimestamp.parse(input.created_at ?? new Date().toISOString());
  const trimmed = input.text.trim();
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    let parsed: unknown;
    try {
      parsed = JSON.parse(trimmed);
    } catch (error) {
      throw new Error(`structured log is not valid JSON: ${(error as Error).message}`);
    }
    return addAutomaticGrades(
      database,
      planStructured(parsed, input.actor, createdAt),
      input.actor,
      createdAt,
    );
  }
  return addAutomaticGrades(
    database,
    await planPlainText(database, trimmed, input.actor, createdAt, input.benchmark),
    input.actor,
    createdAt,
  );
}

export async function applyLogPlan(
  database: AthleticDatabase,
  plan: LogPlan,
) {
  return writeRecords(database, plan.inputs, {
    expected_current_revisions: plan.expected_current_revisions,
  });
}
