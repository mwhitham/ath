/**
 * A small invented athlete record for the database export tests: a device
 * source, RMSSD measurements, heart-rate samples, a sleep session, the Fran
 * benchmark with three results, and one athlete prediction.
 */
import { join } from "node:path";
import * as database from "../src/database.js";
import type { AthleticDatabase, WriteRecordInput } from "../src/database.js";
import type { CurrentRecordT } from "../src/schema.js";
import { athlete } from "./support.js";

const missing = {
  timestamp_missing_reason: "not_provided_by_source",
  utc_offset_missing_reason: "not_provided_by_source",
};

const missingValue = {
  input_value_missing_reason: "not_provided_by_source",
  input_unit_missing_reason: "not_provided_by_source",
};

export function day(index: number, hour = 6): string {
  return new Date(Date.UTC(2026, 0, 1 + index, hour, 0, 0)).toISOString();
}

export interface Scenario {
  handle: AthleticDatabase;
  path: string;
  source: CurrentRecordT;
  measurements: CurrentRecordT[];
  samples: CurrentRecordT[];
  sleep: CurrentRecordT;
  fran: CurrentRecordT;
  results: CurrentRecordT[];
  prediction: CurrentRecordT;
}

function input(record_type: string, data: unknown, created_at: string): WriteRecordInput {
  return { record_type, data, changed_by: athlete, created_at } as WriteRecordInput;
}

export async function buildScenario(folder: string, name = "athlete.ath.sqlite"): Promise<Scenario> {
  const path = join(folder, name);
  const handle = await database.createDatabase(path, { createdAt: "2025-12-31T00:00:00.000Z" });
  const { record: source } = await database.writeRecord(
    handle,
    input(
      "source",
      { source_type: "wearable", data_writer: "Example Band", data_route: "provider_export" },
      "2025-12-31T00:00:00.000Z",
    ),
  );
  const measurements = (
    await database.writeRecords(
      handle,
      [42, 45, 40, 47, 44].map((value, index) =>
        input(
          "point_measurement",
          {
            source_id: source.record_id,
            received_at: day(index, 7),
            received_at_basis: "import_time",
            quantity: "hrv_rmssd",
            recorded_at: day(index),
            input_timestamp: missing,
            value: { standard_value: value, standard_unit: "ms", ...missingValue },
          },
          day(index, 7),
        ),
      ),
    )
  ).records;
  const samples = (
    await database.writeRecords(
      handle,
      [58, 61, 64, 60].map((value, index) =>
        input(
          "measurement_sample",
          {
            source_id: source.record_id,
            received_at: day(1, 8),
            received_at_basis: "import_time",
            quantity: "heart_rate",
            recorded_at: new Date(Date.UTC(2026, 0, 2, 6, index * 10)).toISOString(),
            input_timestamp: missing,
            value: { standard_value: value, standard_unit: "bpm", ...missingValue },
          },
          day(1, 8),
        ),
      ),
    )
  ).records;
  const { record: sleep } = await database.writeRecord(
    handle,
    input(
      "sleep_session",
      {
        source_id: source.record_id,
        received_at: day(1, 9),
        received_at_basis: "import_time",
        start_at: day(0, 23),
        end_at: day(1, 6),
        input_start_timestamp: missing,
        input_end_timestamp: missing,
        source_reported_summaries: [],
      },
      day(1, 9),
    ),
  );
  const { record: fran } = await database.writeRecord(
    handle,
    input(
      "benchmark_definition",
      {
        authorship: {
          author: { author_type: "athlete" },
          authored_at: "2025-12-31T00:00:00.000Z",
          evidence: [],
          content_version: 1,
          status: "active",
        },
        benchmark_key: "fran",
        benchmark_type: "named_wod",
        score_type: "time",
        definition: "21-15-9 thrusters and pull-ups",
      },
      "2025-12-31T00:00:00.000Z",
    ),
  );
  const results: CurrentRecordT[] = [];
  for (const [index, seconds] of [300, 290, 280].entries()) {
    const { record } = await database.writeRecord(
      handle,
      input(
        "benchmark_result",
        {
          source_id: source.record_id,
          received_at: day(index * 2, 12),
          received_at_basis: "import_time",
          benchmark_definition_id: fran.record_id,
          recorded_at: day(index * 2, 11),
          input_timestamp: missing,
          score: { duration_s: seconds },
        },
        day(index * 2, 12),
      ),
    );
    results.push(record);
  }
  const { record: prediction } = await database.writeRecord(
    handle,
    input(
      "prediction",
      {
        authorship: {
          author: { author_type: "athlete" },
          authored_at: day(3),
          evidence: [],
          content_version: 1,
          status: "active",
        },
        benchmark_definition_id: fran.record_id,
        predicted_score: { duration_s: 285 },
        confidence: "moderate",
        reasoning: "Recent intervals trend faster.",
      },
      day(3),
    ),
  );
  return { handle, path, source, measurements, samples, sleep, fran, results, prediction };
}
