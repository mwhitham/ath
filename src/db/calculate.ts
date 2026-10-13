import {
  POINT_MEASUREMENT_UNITS,
  SERIES_QUANTITY_UNITS,
} from "../v1/schema.js";
import { databaseClient, type AthleticDatabase } from "./client.js";
import {
  calendarDateInZone,
  resolveCalendarRange,
  type SeriesViewOptions,
} from "./series-view.js";
import { readRecord } from "./records.js";

export const CALCULATION_OPERATIONS = [
  "summary",
  "baseline",
  "trend",
  "coverage",
  "compare-sources",
  "correlation",
  "workout-drift",
] as const;

export type CalculationOperation = (typeof CALCULATION_OPERATIONS)[number];
export type CalculationGroup = "none" | "day" | "week" | "month";

export interface CalculateOptions {
  operation: CalculationOperation;
  quantity: string;
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

function knownUnit(quantity: string): string {
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

function round(value: number, places = 4): number {
  const scale = 10 ** places;
  return Math.round(value * scale) / scale;
}

function summarize(values: readonly number[]): NumericSummary {
  if (values.length === 0) {
    throw new Error("The calculation has no numeric inputs");
  }
  const sum = values.reduce((total, value) => total + value, 0);
  const mean = sum / values.length;
  const variance =
    values.reduce((total, value) => total + (value - mean) ** 2, 0) /
    values.length;
  return {
    count: values.length,
    minimum: Math.min(...values),
    maximum: Math.max(...values),
    mean: round(mean),
    sum: round(sum),
    standard_deviation: round(Math.sqrt(variance)),
  };
}

async function readNumericInputs(
  database: AthleticDatabase,
  quantity: string,
  options: {
    source_id?: string;
    from?: string;
    to?: string;
  },
): Promise<NumericInput[]> {
  const conditions = ["records.is_deleted = 0", "sources.source_type <> 'manual'"];
  const args: Array<string | number> = [quantity];
  if (options.source_id !== undefined) {
    conditions.push("values_by_record.source_id = ?");
    args.push(options.source_id);
  }
  if (options.from !== undefined) {
    conditions.push("values_by_record.recorded_at >= ?");
    args.push(options.from);
  }
  if (options.to !== undefined) {
    conditions.push("values_by_record.recorded_at <= ?");
    args.push(options.to);
  }
  args.push(MAX_RUNTIME_INPUTS + 1);
  const result = await databaseClient(database).execute({
    sql: `
      WITH values_by_record AS (
        SELECT
          point_measurements.record_id,
          point_measurements.source_id,
          point_measurements.recorded_at,
          point_measurements.standard_value,
          point_measurements.standard_unit
        FROM point_measurements
        WHERE point_measurements.quantity = ?
          AND point_measurements.recorded_at IS NOT NULL
        UNION ALL
        SELECT
          measurement_samples.record_id,
          measurement_samples.source_id,
          measurement_samples.recorded_at,
          measurement_samples.standard_value,
          measurement_samples.standard_unit
        FROM measurement_samples
        WHERE measurement_samples.quantity = ?
      )
      SELECT
        values_by_record.record_id,
        records.current_revision,
        values_by_record.source_id,
        values_by_record.recorded_at,
        values_by_record.standard_value,
        values_by_record.standard_unit
      FROM values_by_record
      JOIN records ON records.record_id = values_by_record.record_id
      JOIN sources ON sources.record_id = values_by_record.source_id
      WHERE ${conditions.join(" AND ")}
      ORDER BY
        values_by_record.source_id,
        values_by_record.recorded_at,
        values_by_record.record_id
      LIMIT ?
    `,
    args: [quantity, ...args],
  });
  if (result.rows.length > MAX_RUNTIME_INPUTS) {
    throw new Error(
      `Calculation matched more than ${MAX_RUNTIME_INPUTS} records. Narrow --from and --to.`,
    );
  }
  return result.rows.map((row) => ({
    record_id: String(row.record_id),
    revision: Number(row.current_revision),
    source_id: String(row.source_id),
    recorded_at: String(row.recorded_at),
    value: Number(row.standard_value),
    unit: String(row.standard_unit),
  }));
}

function valuesBySource(inputs: readonly NumericInput[]): Map<string, NumericInput[]> {
  const groups = new Map<string, NumericInput[]>();
  for (const input of inputs) {
    const group = groups.get(input.source_id) ?? [];
    group.push(input);
    groups.set(input.source_id, group);
  }
  return groups;
}

function startOfWeek(date: string): string {
  const value = new Date(`${date}T00:00:00.000Z`);
  const day = value.getUTCDay();
  value.setUTCDate(value.getUTCDate() - (day === 0 ? 6 : day - 1));
  return value.toISOString().slice(0, 10);
}

function groupKey(
  timestamp: string,
  group: CalculationGroup,
  timeZone: string,
): string {
  const date = calendarDateInZone(timestamp, timeZone);
  if (group === "day") return date;
  if (group === "week") return startOfWeek(date);
  if (group === "month") return date.slice(0, 7);
  return "all";
}

function groupedSummaries(
  inputs: readonly NumericInput[],
  group: CalculationGroup,
  timeZone: string,
) {
  const groups = new Map<string, NumericInput[]>();
  for (const input of inputs) {
    const key = groupKey(input.recorded_at, group, timeZone);
    const values = groups.get(key) ?? [];
    values.push(input);
    groups.set(key, values);
  }
  return [...groups]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([period, values]) => ({
      period,
      ...summarize(values.map((input) => input.value)),
    }));
}

function trend(inputs: readonly NumericInput[]) {
  if (inputs.length < 2) {
    throw new Error("A trend needs at least two inputs");
  }
  const start = Date.parse(inputs[0]!.recorded_at);
  const points = inputs.map((input) => ({
    x: (Date.parse(input.recorded_at) - start) / 86_400_000,
    y: input.value,
  }));
  const xMean = points.reduce((total, point) => total + point.x, 0) / points.length;
  const yMean = points.reduce((total, point) => total + point.y, 0) / points.length;
  const numerator = points.reduce(
    (total, point) => total + (point.x - xMean) * (point.y - yMean),
    0,
  );
  const denominator = points.reduce(
    (total, point) => total + (point.x - xMean) ** 2,
    0,
  );
  if (denominator === 0) {
    throw new Error("A trend needs inputs at more than one instant");
  }
  return {
    count: points.length,
    slope_per_day: round(numerator / denominator),
    first_value: inputs[0]!.value,
    last_value: inputs.at(-1)!.value,
  };
}

function calendarDates(from: string, to: string): string[] {
  const dates: string[] = [];
  const current = new Date(`${from}T00:00:00.000Z`);
  const end = Date.parse(`${to}T00:00:00.000Z`);
  while (current.getTime() <= end) {
    dates.push(current.toISOString().slice(0, 10));
    current.setUTCDate(current.getUTCDate() + 1);
  }
  return dates;
}

function correlation(left: readonly number[], right: readonly number[]): number | null {
  if (left.length !== right.length || left.length < 2) {
    return null;
  }
  const leftMean = left.reduce((total, value) => total + value, 0) / left.length;
  const rightMean = right.reduce((total, value) => total + value, 0) / right.length;
  let numerator = 0;
  let leftSquares = 0;
  let rightSquares = 0;
  for (let index = 0; index < left.length; index++) {
    const leftDifference = left[index]! - leftMean;
    const rightDifference = right[index]! - rightMean;
    numerator += leftDifference * rightDifference;
    leftSquares += leftDifference ** 2;
    rightSquares += rightDifference ** 2;
  }
  const denominator = Math.sqrt(leftSquares * rightSquares);
  return denominator === 0 ? null : round(numerator / denominator);
}

function receipt(
  options: CalculateOptions,
  unit: string,
  inputs: readonly NumericInput[],
  range: { from?: string; to?: string; time_zone?: string },
  method: string,
  result: unknown,
  exclusions: string[] = ["manual sources excluded"],
): CalculationResult {
  return {
    calculation_version: "ath-runtime-v1",
    operation: options.operation,
    method,
    quantity: options.quantity,
    unit,
    range,
    sources: [...new Set(inputs.map((input) => input.source_id))].sort(),
    input: {
      record_count: inputs.length,
      record_references: inputs
        .slice(0, MAX_RECEIPT_REFERENCES)
        .map((input) => ({
          record_id: input.record_id,
          revision: input.revision,
        })),
      references_truncated: inputs.length > MAX_RECEIPT_REFERENCES,
    },
    exclusions,
    result,
  };
}

export async function calculate(
  database: AthleticDatabase,
  options: CalculateOptions,
): Promise<CalculationResult> {
  const unit = knownUnit(options.quantity);
  const group = options.group ?? "none";
  const needsCalendar =
    group !== "none" ||
    options.operation === "coverage" ||
    options.operation === "correlation";
  const rangeOptions: SeriesViewOptions = {
    ...(options.from === undefined ? {} : { from: options.from }),
    ...(options.to === undefined ? {} : { to: options.to }),
    ...(options.time_zone === undefined ? {} : { time_zone: options.time_zone }),
    ...(needsCalendar ? {} : { raw: true }),
  };
  const range = await resolveCalendarRange(database, rangeOptions);
  let inputs = await readNumericInputs(database, options.quantity, {
    ...(options.source_id === undefined ? {} : { source_id: options.source_id }),
    ...(range.from === undefined ? {} : { from: range.from }),
    ...(range.to === undefined ? {} : { to: range.to }),
  });
  if (inputs.length === 0) {
    throw new Error(`No current ${options.quantity} records matched`);
  }

  if (options.operation === "summary" || options.operation === "baseline") {
    const result = [...valuesBySource(inputs)].map(([sourceId, values]) => ({
      source_id: sourceId,
      ...(group === "none"
        ? summarize(values.map((input) => input.value))
        : {
            groups: groupedSummaries(values, group, range.time_zone!),
          }),
    }));
    return receipt(
      options,
      unit,
      inputs,
      range,
      options.operation === "baseline"
        ? "population mean and population standard deviation per source"
        : group === "none"
          ? "count, minimum, maximum, mean, and sum per source"
          : `${group} calendar groups per source`,
      result,
    );
  }

  if (options.operation === "trend") {
    const result = [...valuesBySource(inputs)].map(([sourceId, values]) => ({
      source_id: sourceId,
      ...trend(values),
    }));
    return receipt(
      options,
      unit,
      inputs,
      range,
      "ordinary least-squares slope against elapsed days, calculated separately per source",
      result,
    );
  }

  if (options.operation === "coverage") {
    const result = [...valuesBySource(inputs)].map(([sourceId, values]) => {
      const present = [
        ...new Set(
          values.map((input) =>
            calendarDateInZone(input.recorded_at, range.time_zone!),
          ),
        ),
      ].sort();
      const expected = calendarDates(
        options.from?.slice(0, 10) ?? present[0]!,
        options.to?.slice(0, 10) ?? present.at(-1)!,
      );
      const presentSet = new Set(present);
      return {
        source_id: sourceId,
        first_date: expected[0],
        last_date: expected.at(-1),
        days_expected: expected.length,
        days_present: present.length,
        missing_dates: expected.filter((date) => !presentSet.has(date)),
      };
    });
    return receipt(
      options,
      unit,
      inputs,
      range,
      "calendar dates containing at least one record, calculated separately per source",
      result,
    );
  }

  if (options.operation === "compare-sources") {
    if (options.source_id === undefined || options.compare_source_id === undefined) {
      throw new Error("compare-sources needs --source and --compare-source");
    }
    const comparisonInputs = await readNumericInputs(database, options.quantity, {
      source_id: options.compare_source_id,
      ...(range.from === undefined ? {} : { from: range.from }),
      ...(range.to === undefined ? {} : { to: range.to }),
    });
    if (comparisonInputs.length === 0) {
      throw new Error(`No ${options.quantity} records matched --compare-source`);
    }
    const left = summarize(inputs.map((input) => input.value));
    const right = summarize(comparisonInputs.map((input) => input.value));
    inputs = [...inputs, ...comparisonInputs];
    return receipt(
      options,
      unit,
      inputs,
      range,
      "separate source summaries and difference between their means",
      {
        source: { source_id: options.source_id, ...left },
        comparison_source: {
          source_id: options.compare_source_id,
          ...right,
        },
        mean_difference: round(left.mean - right.mean),
      },
      ["manual sources excluded", "records are not paired by timestamp"],
    );
  }

  if (options.operation === "correlation") {
    if (options.other_quantity === undefined) {
      throw new Error("correlation needs --other-quantity");
    }
    knownUnit(options.other_quantity);
    const otherInputs = await readNumericInputs(database, options.other_quantity, {
      ...(options.compare_source_id === undefined
        ? options.source_id === undefined
          ? {}
          : { source_id: options.source_id }
        : { source_id: options.compare_source_id }),
      ...(range.from === undefined ? {} : { from: range.from }),
      ...(range.to === undefined ? {} : { to: range.to }),
    });
    const dailyMeans = (values: readonly NumericInput[]) => {
      const days = new Map<string, number[]>();
      for (const input of values) {
        const date = calendarDateInZone(input.recorded_at, range.time_zone!);
        const day = days.get(date) ?? [];
        day.push(input.value);
        days.set(date, day);
      }
      return new Map(
        [...days].map(([date, day]) => [
          date,
          day.reduce((total, value) => total + value, 0) / day.length,
        ]),
      );
    };
    const left = dailyMeans(inputs);
    const right = dailyMeans(otherInputs);
    const dates = [...left.keys()].filter((date) => right.has(date)).sort();
    inputs = [...inputs, ...otherInputs];
    return receipt(
      options,
      unit,
      inputs,
      range,
      "Pearson correlation of same-calendar-day means",
      {
        other_quantity: options.other_quantity,
        paired_day_count: dates.length,
        correlation: correlation(
          dates.map((date) => left.get(date)!),
          dates.map((date) => right.get(date)!),
        ),
        paired_dates: dates,
      },
      ["manual sources excluded", "days missing either quantity excluded"],
    );
  }

  if (options.operation === "workout-drift") {
    if (options.workout_session_id === undefined) {
      throw new Error("workout-drift needs --session <workout-session-id>");
    }
    const session = await readRecord(database, options.workout_session_id);
    if (session?.record_type !== "workout_session" || session.is_deleted) {
      throw new Error(`${options.workout_session_id} is not a current workout session`);
    }
    inputs = await readNumericInputs(database, options.quantity, {
      ...(options.source_id === undefined ? {} : { source_id: options.source_id }),
      from: session.data.start_at,
      to: session.data.end_at,
    });
    if (inputs.length < 2) {
      throw new Error("workout-drift needs at least two samples inside the session");
    }
    const midpoint =
      Date.parse(session.data.start_at) +
      (Date.parse(session.data.end_at) - Date.parse(session.data.start_at)) / 2;
    const result = [...valuesBySource(inputs)].map(([sourceId, values]) => {
      const first = values.filter((input) => Date.parse(input.recorded_at) < midpoint);
      const second = values.filter((input) => Date.parse(input.recorded_at) >= midpoint);
      if (first.length === 0 || second.length === 0) {
        return {
          source_id: sourceId,
          available: false,
          reason: "one half has no samples",
        };
      }
      const firstMean = summarize(first.map((input) => input.value)).mean;
      const secondMean = summarize(second.map((input) => input.value)).mean;
      return {
        source_id: sourceId,
        available: true,
        first_half_mean: firstMean,
        second_half_mean: secondMean,
        change: round(secondMean - firstMean),
        change_percentage: round(((secondMean - firstMean) / firstMean) * 100),
      };
    });
    return receipt(
      options,
      unit,
      inputs,
      {
        from: session.data.start_at,
        to: session.data.end_at,
      },
      "difference between means in equal-duration first and second workout halves",
      {
        workout_session_id: session.record_id,
        sources: result,
      },
      ["manual sources excluded", "samples outside the workout session excluded"],
    );
  }

  throw new Error(`Unsupported calculation operation: ${options.operation}`);
}

export function renderCalculation(calculation: CalculationResult): string {
  return [
    `${calculation.operation}: ${calculation.quantity} (${calculation.unit})`,
    `method: ${calculation.method}`,
    `inputs: ${calculation.input.record_count} record${calculation.input.record_count === 1 ? "" : "s"}`,
    `sources: ${calculation.sources.join(", ")}`,
    JSON.stringify(calculation.result, null, 2),
  ].join("\n");
}
