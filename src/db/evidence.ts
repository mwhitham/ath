import {
  UtcTimestamp,
  type CurrentRecordT,
} from "../v1/schema.js";
import type { AthleticDatabase } from "./client.js";
import { queryRecordsAsOf } from "./query.js";

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

function eventAt(record: CurrentRecordT): string | null {
  switch (record.record_type) {
    case "point_measurement":
      return record.data.recorded_at ?? null;
    case "athlete_report":
      return record.data.reported_at;
    case "sleep_session":
    case "workout_session":
      return record.data.start_at;
    case "benchmark_result":
      return record.data.recorded_at;
    case "prediction":
      return record.data.authorship.authored_at;
    default:
      return null;
  }
}

function visibleByEventTime(record: CurrentRecordT, asOf: string): boolean {
  const at = eventAt(record);
  return at === null || at <= asOf;
}

export async function buildEvidence(
  database: AthleticDatabase,
  input: {
    benchmark: string;
    as_of?: string;
    limit?: number;
    window_days?: number;
  },
): Promise<PredictionEvidence> {
  const asOf = UtcTimestamp.parse(input.as_of ?? new Date().toISOString());
  const limit = input.limit ?? 100_000;
  const windowDays = input.window_days ?? 90;
  if (!Number.isInteger(windowDays) || windowDays < 1 || windowDays > 3_650) {
    throw new Error("Evidence window must be between 1 and 3650 days");
  }
  const records = await queryRecordsAsOf(database, {
    as_of: asOf,
    record_types: [
      "benchmark_definition",
      "source",
      "point_measurement",
      "athlete_report",
      "sleep_session",
      "workout_session",
      "benchmark_result",
      "prediction",
    ],
    limit,
  });
  const benchmark = records.find(
    (
      record,
    ): record is Extract<CurrentRecordT, { record_type: "benchmark_definition" }> =>
      record.record_type === "benchmark_definition" &&
      (record.record_id === input.benchmark ||
        record.data.benchmark_key === input.benchmark),
  );
  if (benchmark === undefined) {
    const available = records
      .filter(
        (
          record,
        ): record is Extract<
          CurrentRecordT,
          { record_type: "benchmark_definition" }
        > => record.record_type === "benchmark_definition",
      )
      .map((record) => record.data.benchmark_key)
      .sort();
    throw new Error(
      `no benchmark '${input.benchmark}' was visible at ${asOf}. ` +
        `Available: ${available.join(", ") || "none"}`,
    );
  }

  const sources = new Map(
    records
      .filter(
        (
          record,
        ): record is Extract<CurrentRecordT, { record_type: "source" }> =>
          record.record_type === "source",
      )
      .map((source) => [source.record_id, source]),
  );
  const withinWindow = (record: CurrentRecordT) => {
    const at = eventAt(record);
    return (
      at !== null &&
      at <= asOf &&
      Date.parse(at) >= Date.parse(asOf) - windowDays * 86_400_000
    );
  };
  const deviceMeasurements = records.filter(
    (
      record,
    ): record is Extract<CurrentRecordT, { record_type: "point_measurement" }> =>
      record.record_type === "point_measurement" &&
      visibleByEventTime(record, asOf) &&
      withinWindow(record) &&
      sources.get(record.data.source_id)?.data.source_type !== "manual",
  );
  const athleteReports = records.filter(
    (
      record,
    ): record is Extract<CurrentRecordT, { record_type: "athlete_report" }> =>
      record.record_type === "athlete_report" && withinWindow(record),
  );
  const sleepSessions = records.filter(
    (
      record,
    ): record is Extract<CurrentRecordT, { record_type: "sleep_session" }> =>
      record.record_type === "sleep_session" && withinWindow(record),
  );
  const workoutSessions = records.filter(
    (
      record,
    ): record is Extract<CurrentRecordT, { record_type: "workout_session" }> =>
      record.record_type === "workout_session" && withinWindow(record),
  );
  const priorResults = records.filter(
    (
      record,
    ): record is Extract<CurrentRecordT, { record_type: "benchmark_result" }> =>
      record.record_type === "benchmark_result" &&
      record.data.benchmark_definition_id === benchmark.record_id &&
      visibleByEventTime(record, asOf),
  );
  const priorPredictions = records.filter(
    (
      record,
    ): record is Extract<CurrentRecordT, { record_type: "prediction" }> =>
      record.record_type === "prediction" &&
      record.data.benchmark_definition_id === benchmark.record_id &&
      visibleByEventTime(record, asOf),
  );
  const gaps: string[] = [];
  if (priorResults.length === 0) {
    gaps.push("No earlier result exists for this benchmark.");
  }
  if (deviceMeasurements.length === 0) {
    gaps.push(
      `No device point measurements exist in the prior ${windowDays} days.`,
    );
  }
  if (sleepSessions.length === 0) {
    gaps.push(
      `No recorded sleep session exists in the prior ${windowDays} days.`,
    );
  }
  if (records.length === limit) {
    gaps.push(`Evidence reached the ${limit}-record query limit.`);
  }

  const byEvent = (left: CurrentRecordT, right: CurrentRecordT) =>
    (eventAt(left) ?? "").localeCompare(eventAt(right) ?? "") ||
    left.record_id.localeCompare(right.record_id);
  return {
    purpose: "prediction_evidence",
    is_prediction: false,
    as_of: asOf,
    benchmark,
    prior_results: priorResults.sort(byEvent),
    device_measurements: deviceMeasurements.sort(byEvent),
    athlete_reports: athleteReports.sort(byEvent),
    sleep_sessions: sleepSessions.sort(byEvent),
    workout_sessions: workoutSessions.sort(byEvent),
    prior_predictions: priorPredictions.sort(byEvent),
    gaps,
  };
}
