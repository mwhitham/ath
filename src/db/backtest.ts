import { open as openFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import type { CompleteRequest, CompleteResponse } from "../gateway.js";
import type { CurrentRecordT } from "../v1/schema.js";
import type { AthleticDatabase } from "./client.js";
import { buildEvidence } from "./evidence.js";
import { listRecords, readRecordRevisions } from "./records.js";

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

export interface V1BacktestReport {
  athleticstandard_version: "1.0.0";
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

function nativeScore(
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
    throw new Error(`score does not contain ${scoreType}`);
  }
  return value;
}

function parseModelScore(content: string, scoreType: string): number {
  const trimmed = content
    .trim()
    .replace(/^```(?:json)?\s*/i, "")
    .replace(/\s*```$/, "");
  const parsed = JSON.parse(trimmed) as Record<string, unknown>;
  const score = parsed.predicted_score;
  if (typeof score !== "object" || score === null || Array.isArray(score)) {
    throw new Error("model response has no predicted_score object");
  }
  return nativeScore(score as {
    duration_s?: number;
    reps?: number;
    weight_kg?: number;
  }, scoreType);
}

function median(values: readonly number[]): number | null {
  if (values.length === 0) {
    return null;
  }
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  const value =
    sorted.length % 2 === 1
      ? sorted[middle]!
      : (sorted[middle - 1]! + sorted[middle]!) / 2;
  return Math.round(value * 10) / 10;
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
  if (input.models.length === 0) {
    throw new Error("backtest needs at least one model");
  }
  const records = await listRecords(database, { include_deleted: true, limit: 10_000 });
  const definitions = new Map(
    records
      .filter(
        (
          record,
        ): record is Extract<
          CurrentRecordT,
          { record_type: "benchmark_definition" }
        > => record.record_type === "benchmark_definition",
      )
      .map((record) => [record.record_id, record]),
  );
  const resultsByBenchmark = new Map<
    string,
    Extract<CurrentRecordT, { record_type: "benchmark_result" }>[]
  >();
  for (const record of records) {
    if (record.record_type !== "benchmark_result" || record.is_deleted) {
      continue;
    }
    const group = resultsByBenchmark.get(record.data.benchmark_definition_id) ?? [];
    group.push(record);
    resultsByBenchmark.set(record.data.benchmark_definition_id, group);
  }
  const targets: Array<{
    result: Extract<CurrentRecordT, { record_type: "benchmark_result" }>;
    definition: Extract<CurrentRecordT, { record_type: "benchmark_definition" }>;
    asOf: string;
  }> = [];
  let skippedFirst = 0;
  for (const [benchmarkId, benchmarkResults] of resultsByBenchmark) {
    const definition = definitions.get(benchmarkId);
    if (definition === undefined) {
      throw new Error(`benchmark definition is missing: ${benchmarkId}`);
    }
    benchmarkResults.sort(
      (left, right) =>
        left.data.recorded_at.localeCompare(right.data.recorded_at) ||
        left.record_id.localeCompare(right.record_id),
    );
    skippedFirst += Math.min(benchmarkResults.length, 1);
    for (const result of benchmarkResults.slice(1)) {
      const revisions = await readRecordRevisions(database, result.record_id);
      const knownAt = revisions.at(-1)?.became_known_at;
      if (knownAt === undefined) {
        throw new Error(`result revision is missing: ${result.record_id}`);
      }
      targets.push({
        result,
        definition,
        asOf: new Date(Date.parse(knownAt) - 1).toISOString(),
      });
    }
  }

  const replays: BacktestReplay[] = [];
  for (const target of targets) {
    const evidence = await buildEvidence(database, {
      benchmark: target.definition.record_id,
      as_of: target.asOf,
    });
    const actual = nativeScore(
      target.result.data.score,
      target.definition.data.score_type,
    );
    for (const model of input.models) {
      try {
        const response = await input.complete({
          gateway: input.gateway,
          model,
          system:
            "Return only JSON with predicted_score, optional predicted_range, confidence, and reasoning.",
          user: JSON.stringify(evidence),
        });
        const predicted = parseModelScore(
          response.content,
          target.definition.data.score_type,
        );
        replays.push({
          model,
          benchmark_result_id: target.result.record_id,
          benchmark_key: target.definition.data.benchmark_key,
          as_of: target.asOf,
          predicted_value: predicted,
          actual_value: actual,
          absolute_error_percentage:
            Math.round((Math.abs(predicted - actual) / actual) * 1_000) / 10,
        });
      } catch (error) {
        replays.push({
          model,
          benchmark_result_id: target.result.record_id,
          benchmark_key: target.definition.data.benchmark_key,
          as_of: target.asOf,
          actual_value: actual,
          error: (error as Error).message,
        });
      }
    }
  }

  return {
    athleticstandard_version: "1.0.0",
    created_at: input.created_at ?? new Date().toISOString(),
    models: [...input.models],
    replayable_result_count: targets.length,
    skipped_first_result_count: skippedFirst,
    replays,
    summary: input.models.map((model) => {
      const modelRows = replays.filter((row) => row.model === model);
      const completed = modelRows.filter(
        (row) => row.absolute_error_percentage !== undefined,
      );
      return {
        model,
        completed: completed.length,
        failed: modelRows.length - completed.length,
        median_absolute_error_percentage: median(
          completed.map((row) => row.absolute_error_percentage!),
        ),
      };
    }),
  };
}

export async function writeV1BacktestReport(
  databasePath: string,
  report: V1BacktestReport,
): Promise<string> {
  const stamp = report.created_at.replace(/[:.]/g, "").replace(/Z$/, "Z");
  const path = join(dirname(databasePath), `backtest-${stamp}.json`);
  const file = await openFile(path, "wx", 0o600);
  try {
    await file.writeFile(`${JSON.stringify(report, null, 2)}\n`, "utf8");
  } finally {
    await file.close();
  }
  return path;
}
