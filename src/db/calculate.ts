import {
  POINT_MEASUREMENT_UNITS,
  SERIES_QUANTITY_UNITS,
  type ProviderReportingPeriodT,
} from "../v1/schema.js";
import { runAthCore } from "../ath-core.js";
import { readFileSync } from "node:fs";
import { type AthleticDatabase } from "./client.js";
import { resolveCalendarRange } from "./series-view.js";

export const CALCULATION_OPERATIONS = [
  "summary",
  "baseline",
  "trend",
  "coverage",
  "compare-sources",
  "correlation",
  "workout-drift",
  "provider-period-summary",
] as const;

export type CalculationOperation = (typeof CALCULATION_OPERATIONS)[number];
export type CalculationGroup = "none" | "day" | "week" | "month";
export const CALCULATION_RECORD_KINDS = [
  "measurement",
  "provider-score",
  "provider-summary",
  "session-summary",
] as const;
export type CalculationRecordKind = (typeof CALCULATION_RECORD_KINDS)[number];
export type ProviderPeriodType = ProviderReportingPeriodT["period_type"];

export interface CalculateOptions {
  operation: CalculationOperation;
  quantity: string;
  record_kind?: CalculationRecordKind;
  provider_period_type?: ProviderPeriodType;
  source_id?: string;
  compare_source_id?: string;
  other_quantity?: string;
  workout_session_id?: string;
  from?: string;
  to?: string;
  time_zone?: string;
  group?: CalculationGroup;
}

interface NumericInput {
  record_id: string;
  revision: number;
  source_id: string;
  recorded_at: string;
  value: number;
  unit: string;
}

interface NumericSummary {
  count: number;
  minimum: number;
  maximum: number;
  mean: number;
  sum: number;
  standard_deviation: number;
}

export interface CalculationResult {
  calculation_version: "ath-runtime-v1";
  operation: CalculationOperation;
  record_kind: CalculationRecordKind;
  provider_period_type?: ProviderPeriodType;
  method: string;
  quantity: string;
  unit: string;
  range: {
    from?: string;
    to?: string;
    time_zone?: string;
  };
  sources: string[];
  input: {
    record_count: number;
    record_references: Array<{ record_id: string; revision: number }>;
    references_truncated: boolean;
  };
  exclusions: string[];
  result: unknown;
}

const MAX_RUNTIME_INPUTS = 100_000;
const MAX_RECEIPT_REFERENCES = 1_000;

function knownMeasurementUnit(quantity: string): string {
  const unit =
    (POINT_MEASUREMENT_UNITS as Record<string, string>)[quantity] ??
    (SERIES_QUANTITY_UNITS as Record<string, string>)[quantity];
  if (unit === undefined) {
    throw new Error(
      `Unknown quantity '${quantity}'. Known quantities: ${[
        ...new Set([
          ...Object.keys(POINT_MEASUREMENT_UNITS),
          ...Object.keys(SERIES_QUANTITY_UNITS),
        ]),
      ].join(", ")}`,
    );
  }
  return unit;
}


function skillMethod(operation: string): string {
  const path = new URL(`../../skills/${operation}/SKILL.md`, import.meta.url);
  try {
    const text = readFileSync(path, "utf8");
    const match = text.match(/^method:\s*(.+)$/m);
    return match?.[1]?.trim() ?? "";
  } catch {
    return "";
  }
}

export async function calculate(
  database: AthleticDatabase,
  options: CalculateOptions,
): Promise<CalculationResult> {
  const recordKind = options.record_kind ?? "measurement";
  if (
    options.operation !== "provider-period-summary" &&
    recordKind === "session-summary"
  ) {
    throw new Error("session-summary records require the provider-period-summary operation");
  }
  if (options.operation === "provider-period-summary" && options.provider_period_type === undefined) {
    throw new Error("provider-period-summary needs --period-type <provider-period-type>");
  }
  if (options.provider_period_type !== undefined && options.operation !== "provider-period-summary") {
    throw new Error("--period-type is used only with provider-period-summary");
  }
  if (options.operation === "workout-drift" && options.workout_session_id === undefined) {
    throw new Error("workout-drift needs --session <workout-session-id>");
  }
  if (options.operation === "compare-sources" && (options.source_id === undefined || options.compare_source_id === undefined)) {
    throw new Error("compare-sources needs --source and --compare-source");
  }
  if (options.operation === "correlation" && options.other_quantity === undefined) {
    throw new Error("correlation needs --other-quantity");
  }
  const group = options.group ?? "none";
  const needsCalendar =
    options.operation !== "provider-period-summary" &&
    options.operation !== "workout-drift" &&
    (group !== "none" || options.operation === "coverage" || options.operation === "correlation");
  const range = needsCalendar
    ? await resolveCalendarRange(database, {
        ...(options.from === undefined ? {} : { from: options.from }),
        ...(options.to === undefined ? {} : { to: options.to }),
        ...(options.time_zone === undefined ? {} : { time_zone: options.time_zone }),
      })
    : {
        ...(options.from === undefined ? {} : { from: options.from }),
        ...(options.to === undefined ? {} : { to: options.to }),
        ...(options.time_zone === undefined ? {} : { time_zone: options.time_zone }),
      };
  const expectedUnit =
    options.operation !== "provider-period-summary" && recordKind === "measurement"
      ? knownMeasurementUnit(options.quantity)
      : undefined;
  const expectedOther =
    options.other_quantity !== undefined && recordKind === "measurement"
      ? knownMeasurementUnit(options.other_quantity)
      : undefined;
  if (database.localPath === undefined) {
    throw new Error("ath-core reads a local database file");
  }
  return runAthCore(
    ["calculate-range", "--database", database.localPath],
    process.env,
    JSON.stringify({
      ...options,
      record_kind: recordKind,
      group,
      ...(range.from === undefined ? {} : { from: range.from }),
      ...(range.to === undefined ? {} : { to: range.to }),
      ...(options.from === undefined ? {} : { calendar_from: options.from }),
      ...(options.to === undefined ? {} : { calendar_to: options.to }),
      ...(range.time_zone === undefined ? {} : { time_zone: range.time_zone }),
      ...(expectedUnit === undefined ? {} : { expected_unit: expectedUnit }),
      ...(expectedOther === undefined ? {} : { expected_other_unit: expectedOther }),
      method: skillMethod(options.operation),
    }),
  ) as CalculationResult;
}


export function renderCalculation(calculation: CalculationResult): string {
  return [
    `${calculation.operation}: ${calculation.record_kind} ${calculation.quantity} (${calculation.unit})`,
    ...(calculation.provider_period_type === undefined
      ? []
      : [`provider period: ${calculation.provider_period_type}`]),
    `method: ${calculation.method}`,
    `inputs: ${calculation.input.record_count} record${calculation.input.record_count === 1 ? "" : "s"}`,
    `sources: ${calculation.sources.join(", ")}`,
    JSON.stringify(calculation.result, null, 2),
  ].join("\n");
}
