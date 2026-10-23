/**
 * Oura's export: either a bundle of per-category CSVs or a single Trends table.
 *
 * Two traps this importer avoids. Oura publishes a readiness contributor called
 * "resting heart rate" that is a 0-100 sub-score, not a pulse; reading it as one
 * would put a number near 90 into a resting-HR baseline. And `temperature_deviation`
 * is a signed delta from Oura's own baseline, so it gets its own type rather than
 * being written as a body temperature (D28).
 */
import type { HardSignalT } from "../schema.js";
import type { ProviderReportingPeriodT } from "../v1/schema.js";
import { countSkip, emptyPayload, type ImportPayload, type SourceBook } from "./merge.js";
import { cell, normalizeHeader, num, parseCsv, positive, withOffset, type Row } from "./csv.js";

/** Measurements taken from a sleep row. Oura already reports durations in seconds. */
const SLEEP_POINTS: { columns: string[]; type: string; unit: string }[] = [
  { columns: ["average_hrv", "Average HRV", "hrv"], type: "hrv_rmssd", unit: "ms" },
  // The genuine pulse figure. Oura's own "resting heart rate" contributor is a score.
  { columns: ["lowest_heart_rate", "Lowest Resting Heart Rate", "lowest_resting_heart_rate"], type: "resting_heart_rate", unit: "bpm" },
  { columns: ["average_breath", "Respiratory Rate", "respiratory_rate"], type: "respiratory_rate", unit: "brpm" },
  { columns: ["spo2_percentage", "Average SpO2", "spo2"], type: "oxygen_saturation", unit: "%" },
  { columns: ["vo2_max"], type: "vo2_max", unit: "ml/kg/min" },
];

const SLEEP_STAGES: { columns: string[]; key: string }[] = [
  { columns: ["total_sleep_duration", "Total Sleep Duration"], key: "duration_s" },
  { columns: ["time_in_bed", "Time in Bed"], key: "time_in_bed_s" },
  { columns: ["deep_sleep_duration", "Deep Sleep Duration"], key: "deep_s" },
  { columns: ["rem_sleep_duration", "REM Sleep Duration"], key: "rem_s" },
  { columns: ["light_sleep_duration", "Light Sleep Duration"], key: "light_s" },
  { columns: ["awake_time", "Awake Time", "awake_duration"], key: "awake_s" },
];

/** Vendor composites. Kept, labelled, and never mistaken for measurements (D27). */
const VENDOR_SCORES: { columns: string[]; metric: string }[] = [
  { columns: ["readiness_score", "Readiness Score"], metric: "readiness" },
  { columns: ["sleep_score", "Sleep Score"], metric: "sleep_score" },
  { columns: ["activity_score", "Activity Score"], metric: "activity_score" },
];

/**
 * Readiness contributors are sub-scores on a 0-100 scale. They are named after
 * measurements ("resting heart rate", "hrv balance") but they are not measurements,
 * so they import prefixed and clearly labelled.
 */
const CONTRIBUTORS = [
  "activity_balance",
  "body_temperature",
  "hrv_balance",
  "previous_day_activity",
  "previous_night",
  "recovery_index",
  "resting_heart_rate",
  "sleep_balance",
  "sleep_regularity",
];

const OURA_SLEEP_SCORE_CONTRIBUTOR_KEYS = new Set([
  "timing",
  "latency",
  "deep_sleep",
  "rem_sleep",
  "total_sleep",
  "efficiency",
  "restfulness",
]);

const OURA_ACTIVITY_SCORE_CONTRIBUTOR_KEYS = new Set([
  "stay_active",
  "move_every_hour",
  "meet_daily_targets",
  "training_frequency",
  "training_volume",
  "recovery_time",
]);

const OURA_READINESS_CONTRIBUTOR_KEYS = new Set(CONTRIBUTORS);

/** Activity CSV columns we do not import yet; count them as skips when present. */
const OURA_ACTIVITY_COLUMNS = [
  "steps",
  "active_calories",
  "total_calories",
  "target_calories",
  "met_minutes",
  "low_activity_met_minutes",
  "medium_activity_met_minutes",
  "high_activity_met_minutes",
  "equivalent_walking_distance",
  "target_meters",
  "meters_to_target",
  "inactivity_alerts",
  "resting_time",
  "sedentary_met_minutes",
];

type OuraPortalScoreMetric = "readiness" | "sleep_score" | "activity_score";

function contributorKeysFromCell(contributorsJson: string | undefined): string[] | undefined {
  if (contributorsJson === undefined) {
    return undefined;
  }
  const trimmed = contributorsJson.trim();
  if (trimmed.length === 0 || trimmed === "{}") {
    return [];
  }
  if (!trimmed.startsWith("{")) {
    return undefined;
  }
  try {
    const parsed = JSON.parse(trimmed) as Record<string, unknown>;
    return Object.keys(parsed);
  } catch {
    return undefined;
  }
}

function ouraPortalScoreMetricFromContributorKeys(
  keys: string[],
): OuraPortalScoreMetric | undefined {
  let readiness = 0;
  let sleep = 0;
  let activity = 0;
  for (const key of keys) {
    if (OURA_READINESS_CONTRIBUTOR_KEYS.has(key)) readiness++;
    if (OURA_SLEEP_SCORE_CONTRIBUTOR_KEYS.has(key)) sleep++;
    if (OURA_ACTIVITY_SCORE_CONTRIBUTOR_KEYS.has(key)) activity++;
  }
  const best = Math.max(readiness, sleep, activity);
  if (best === 0) {
    return undefined;
  }
  if (sleep === best && sleep > readiness && sleep >= activity) {
    return "sleep_score";
  }
  if (activity === best && activity > readiness && activity >= sleep) {
    return "activity_score";
  }
  return "readiness";
}

function detectFilePortalScoreMetric(rows: Row[]): OuraPortalScoreMetric | undefined {
  for (const row of rows) {
    const keys = contributorKeysFromCell(cell(row, "contributors"));
    if (keys === undefined || keys.length === 0) {
      continue;
    }
    const metric = ouraPortalScoreMetricFromContributorKeys(keys);
    if (metric !== undefined) {
      return metric;
    }
  }
  return undefined;
}

function ouraTimestampReceipt(timestamp: string) {
  const offset = /(Z|[+-]\d{2}:\d{2})$/i.exec(timestamp.trim())?.[1];
  return {
    timestamp_text: timestamp,
    timestamp_basis: "source_offset_timestamp" as const,
    ...(offset === undefined
      ? { utc_offset_missing_reason: "not_provided_by_source" as const }
      : {
          utc_offset_text: offset,
          utc_offset_meaning: "meaning_not_documented" as const,
        }),
  };
}

function parseJsonNumber(value: string | undefined): number | undefined {
  if (value === undefined) {
    return undefined;
  }
  const trimmed = value.trim();
  if (!trimmed.startsWith("{")) {
    const direct = Number(trimmed.replace(/,/g, ""));
    return Number.isFinite(direct) ? direct : undefined;
  }
  try {
    const parsed = JSON.parse(trimmed) as Record<string, unknown>;
    if (typeof parsed.average === "number" && Number.isFinite(parsed.average)) {
      return parsed.average;
    }
    if (typeof parsed.score === "number" && Number.isFinite(parsed.score)) {
      return parsed.score;
    }
  } catch {
    return undefined;
  }
  return undefined;
}

function ouraPeriod(
  day: string | undefined,
  periodType:
    | "oura_activity_day"
    | "oura_sleep_day"
    | "oura_calendar_day"
    | "oura_day_unspecified",
): ProviderReportingPeriodT[] | undefined {
  if (day === undefined) return undefined;
  return [{
    period_type: periodType,
    provider_date: day,
    timezone_offset_missing_reason: "not_provided_by_source",
  }];
}

export function importOura(files: Map<string, string>, sources: SourceBook): ImportPayload {
  const payload = emptyPayload("oura", "Oura via CSV export");
  const sourceId = sources.forWriter({ writer: "Oura", vendor: "oura" });

  for (const [name, text] of files) {
    const rows = parseCsv(text);
    if (rows.length === 0) {
      countSkip(payload, `empty CSV: ${name}`);
      continue;
    }
    importRows(rows, sourceId, payload, name);
  }

  return payload;
}

/**
 * Oura's export has one file per category, and its Trends download combines them.
 * Rather than switch on filename, each row contributes whatever columns it has —
 * a readiness file simply has no sleep columns.
 */
function importRows(rows: Row[], sourceId: string, payload: ImportPayload, name: string): void {
  let used = 0;
  const filePortalScoreMetric = detectFilePortalScoreMetric(rows);

  for (const row of rows) {
    const bedtimeStartText = cell(row, "bedtime_start", "Bedtime Start");
    const bedtimeEndText = cell(row, "bedtime_end", "Bedtime End");
    const bedtimeStart = withOffset(bedtimeStartText, undefined);
    const bedtimeEnd = withOffset(bedtimeEndText, undefined);
    // The legacy parser needs an internal anchor. The v1 receipt below removes this
    // placeholder and keeps only the provider date when no instant was supplied.
    const day = cell(row, "day", "date", "summary_date");
    const sleepPeriods = ouraPeriod(day, "oura_sleep_day");
    const activityPeriods = ouraPeriod(day, "oura_activity_day");
    const unspecifiedPeriods = ouraPeriod(day, "oura_day_unspecified");
    const anchor = bedtimeEnd ?? (day ? `${day}T00:00:00Z` : null);

    if (!anchor) {
      countSkip(payload, `${name}: rows without a date`);
      continue;
    }
    used++;

    const spo2Cell = cell(row, "spo2_percentage", "Average SpO2", "spo2");
    if (spo2Cell !== undefined && spo2Cell.startsWith("{")) {
      const spo2FromJson = parseJsonNumber(spo2Cell);
      if (spo2FromJson !== undefined && spo2FromJson > 0) {
        const signal: HardSignalT = {
          type: "oxygen_saturation",
          value: Math.round(spo2FromJson * 100) / 100,
          unit: "%",
          recorded_at: anchor,
          source: sourceId,
        };
        payload.inputReceipts.set(signal, {
          input_value: spo2FromJson,
          input_unit: "%",
          ...(unspecifiedPeriods === undefined
            ? {}
            : { provider_reporting_periods: unspecifiedPeriods }),
        });
        payload.hardSignals.push(signal);
      }
    }

    for (const mapping of SLEEP_POINTS) {
      if (mapping.type === "oxygen_saturation") {
        const raw = cell(row, ...mapping.columns);
        if (raw !== undefined && raw.trim().startsWith("{")) {
          continue;
        }
      }
      const value = positive(row, ...mapping.columns);
      if (value === undefined) continue;
      const signal: HardSignalT = {
        type: mapping.type as HardSignalT["type"],
        value: Math.round(value * 100) / 100,
        unit: mapping.unit,
        recorded_at: anchor,
        source: sourceId,
      } as HardSignalT;
      payload.inputReceipts.set(signal, {
        ...(bedtimeEndText === undefined
          ? {}
          : { input_timestamp: ouraTimestampReceipt(bedtimeEndText) }),
        input_value: value,
        input_unit: mapping.unit,
        ...(sleepPeriods === undefined
          ? {}
          : { provider_reporting_periods: sleepPeriods }),
      });
      payload.hardSignals.push(signal);
    }

    // A signed delta from Oura's own baseline, so it may be negative or zero.
    const deviation = num(row, "temperature_deviation", "Temperature Deviation");
    if (deviation !== undefined) {
      const signal: HardSignalT = {
        type: "temperature_deviation",
        value: Math.round(deviation * 100) / 100,
        unit: "°C",
        recorded_at: anchor,
        source: sourceId,
      };
      payload.inputReceipts.set(signal, {
        ...(bedtimeEndText === undefined
          ? {}
          : { input_timestamp: ouraTimestampReceipt(bedtimeEndText) }),
        input_value: deviation,
        input_unit: "°C",
        ...(unspecifiedPeriods === undefined
          ? {}
          : { provider_reporting_periods: unspecifiedPeriods }),
      });
      payload.hardSignals.push(signal);
    }

    if (bedtimeStart && bedtimeEnd && Date.parse(bedtimeEnd) > Date.parse(bedtimeStart)) {
      const aggregates: Record<string, number> = {};
      for (const stage of SLEEP_STAGES) {
        const seconds = positive(row, ...stage.columns);
        if (seconds !== undefined) aggregates[stage.key] = Math.round(seconds);
      }
      const efficiency = positive(row, "efficiency", "sleep_efficiency", "Sleep Efficiency");
      if (efficiency !== undefined) {
        aggregates.efficiency_pct = Math.min(100, Math.round(efficiency * 10) / 10);
      }
      const session = {
        type: "sleep_session",
        start: bedtimeStart,
        end: bedtimeEnd,
        source: sourceId,
        aggregates: aggregates as never,
      } as HardSignalT;
      const inputSummaries: Record<
        string,
        { input_value: number; input_unit: string }
      > = {};
      for (const stage of SLEEP_STAGES) {
        const value = positive(row, ...stage.columns);
        if (value !== undefined) {
          inputSummaries[stage.key] = {
            input_value: value,
            input_unit: "s",
          };
        }
      }
      if (efficiency !== undefined) {
        inputSummaries.efficiency_pct = {
          input_value: efficiency,
          input_unit: "%",
        };
      }
      payload.inputReceipts.set(session, {
        input_start_timestamp: ouraTimestampReceipt(bedtimeStartText!),
        input_end_timestamp: ouraTimestampReceipt(bedtimeEndText!),
        input_summaries: inputSummaries,
        ...(sleepPeriods === undefined
          ? {}
          : { provider_reporting_periods: sleepPeriods }),
      });
      payload.hardSignals.push(session);
    }

    const memberPortalScore = num(row, "score", "Score");
    const contributorsJson = cell(row, "contributors");
    const hasContributorsColumn = contributorsJson !== undefined;
    const contributorKeys = contributorKeysFromCell(contributorsJson);
    if (
      memberPortalScore !== undefined &&
      hasContributorsColumn &&
      (contributorKeys === undefined || contributorKeys.length === 0)
    ) {
      countSkip(payload, `${name}: score row with empty contributors`);
    }
    if (
      memberPortalScore !== undefined &&
      hasContributorsColumn &&
      contributorKeys !== undefined &&
      contributorKeys.length > 0 &&
      filePortalScoreMetric !== undefined
    ) {
      const periodType =
        filePortalScoreMetric === "sleep_score"
          ? "oura_sleep_day"
          : filePortalScoreMetric === "activity_score"
            ? "oura_activity_day"
            : "oura_day_unspecified";
      const periods =
        periodType === "oura_sleep_day"
          ? sleepPeriods
          : periodType === "oura_activity_day"
            ? activityPeriods
            : unspecifiedPeriods;
      const signal: HardSignalT = {
        type: "vendor_score",
        metric: filePortalScoreMetric,
        value: memberPortalScore,
        scale: "0-100",
        recorded_at: anchor,
        source: sourceId,
      };
      payload.inputReceipts.set(signal, {
        input_value: memberPortalScore,
        input_unit_missing_reason: "not_provided_by_source",
        ...(periods === undefined ? {} : { provider_reporting_periods: periods }),
      });
      payload.hardSignals.push(signal);
    }

    if (num(row, "breathing_disturbance_index", "breathing_disturbance") !== undefined) {
      countSkip(payload, `${name}: breathing disturbance column`);
    }
    if (num(row, "cardiovascular_age") !== undefined) {
      countSkip(payload, `${name}: cardiovascular age column`);
    }
    for (const column of OURA_ACTIVITY_COLUMNS) {
      if (num(row, column) !== undefined) {
        countSkip(payload, `${name}: activity column ${column}`);
      }
    }

    for (const score of VENDOR_SCORES) {
      const value = num(row, ...score.columns);
      if (value === undefined) continue;
      const signal: HardSignalT = {
        type: "vendor_score",
        metric: score.metric,
        value,
        scale: "0-100",
        recorded_at: anchor,
        source: sourceId,
      };
      const periodType =
        score.metric === "activity_score"
          ? "oura_activity_day"
          : score.metric === "sleep_score"
            ? "oura_sleep_day"
            : "oura_day_unspecified";
      const periods =
        periodType === "oura_activity_day"
          ? activityPeriods
          : periodType === "oura_sleep_day"
            ? sleepPeriods
            : unspecifiedPeriods;
      payload.inputReceipts.set(signal, {
        ...(bedtimeEndText === undefined
          ? {}
          : { input_timestamp: ouraTimestampReceipt(bedtimeEndText) }),
        input_value: value,
        input_unit_missing_reason: "not_provided_by_source",
        ...(periods === undefined
          ? {}
          : { provider_reporting_periods: periods }),
      });
      payload.hardSignals.push(signal);
    }

    const contributorValues = new Map<string, number>();
    if (contributorsJson !== undefined && contributorsJson.startsWith("{")) {
      try {
        const parsed = JSON.parse(contributorsJson) as Record<string, unknown>;
        for (const [key, value] of Object.entries(parsed)) {
          if (CONTRIBUTORS.includes(key)) {
            if (typeof value === "number" && Number.isFinite(value)) {
              contributorValues.set(key, value);
            }
          } else if (typeof value === "number" && Number.isFinite(value)) {
            countSkip(payload, `${name}: contributor column ${key}`);
          }
        }
      } catch {
        countSkip(payload, `${name}: contributors cell is not valid JSON`);
      }
    }

    for (const contributor of CONTRIBUTORS) {
      const value =
        contributorValues.get(contributor) ??
        num(row, `contributor_${contributor}`, `contributors_${contributor}`);
      if (value === undefined) continue;
      const signal: HardSignalT = {
        type: "vendor_score",
        metric: `readiness_contributor_${contributor}`,
        value,
        scale: "0-100",
        recorded_at: anchor,
        source: sourceId,
      };
      payload.inputReceipts.set(signal, {
        ...(bedtimeEndText === undefined
          ? {}
          : { input_timestamp: ouraTimestampReceipt(bedtimeEndText) }),
        input_value: value,
        input_unit_missing_reason: "not_provided_by_source",
        ...(unspecifiedPeriods === undefined
          ? {}
          : { provider_reporting_periods: unspecifiedPeriods }),
      });
      payload.hardSignals.push(signal);
    }
  }

  if (used === 0) countSkip(payload, `${name}: no rows we could place`);
}
