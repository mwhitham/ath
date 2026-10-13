import {
  SERIES_QUANTITY_UNITS,
  UtcTimestamp,
  type CurrentRecordT,
  type SeriesQuantity,
} from "../v1/schema.js";
import { databaseClient, type AthleticDatabase } from "./client.js";
import { querySeries } from "./query.js";

interface InstantRange {
  from?: string;
  to?: string;
  time_zone?: string;
}

export interface SeriesViewOptions {
  from?: string;
  to?: string;
  source_id?: string;
  time_zone?: string;
  raw?: boolean;
  limit?: number;
}

export interface SeriesDay {
  date: string;
  source_id: string;
  sample_count: number;
  minimum: number;
  maximum: number;
  mean: number;
  first_at: string;
  last_at: string;
}

export interface SeriesView {
  quantity: SeriesQuantity;
  unit: string;
  time_zone?: string;
  from?: string;
  to?: string;
  source_id?: string;
  truncated: boolean;
  samples?: CurrentRecordT[];
  days?: SeriesDay[];
}

function calendarDateParts(value: string): {
  year: number;
  month: number;
  day: number;
} {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) {
    throw new Error(`expected YYYY-MM-DD, not '${value}'`);
  }
  const [year, month, day] = value.split("-").map(Number) as [number, number, number];
  const date = new Date(Date.UTC(year, month - 1, day));
  if (
    date.getUTCFullYear() !== year ||
    date.getUTCMonth() !== month - 1 ||
    date.getUTCDate() !== day
  ) {
    throw new Error(`invalid calendar date '${value}'`);
  }
  return { year, month, day };
}

function formatterFor(timeZone: string): Intl.DateTimeFormat {
  try {
    return new Intl.DateTimeFormat("en-CA", {
      timeZone,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hourCycle: "h23",
    });
  } catch {
    throw new Error(`'${timeZone}' is not an IANA timezone such as America/Chicago`);
  }
}

function partsAt(
  instantMs: number,
  formatter: Intl.DateTimeFormat,
): { year: number; month: number; day: number; hour: number; minute: number; second: number } {
  const values = Object.fromEntries(
    formatter
      .formatToParts(new Date(instantMs))
      .filter((part) => part.type !== "literal")
      .map((part) => [part.type, Number(part.value)]),
  );
  return {
    year: values.year!,
    month: values.month!,
    day: values.day!,
    hour: values.hour!,
    minute: values.minute!,
    second: values.second!,
  };
}

function startOfDateInZone(dateText: string, timeZone: string): string {
  const target = calendarDateParts(dateText);
  const formatter = formatterFor(timeZone);
  const targetWallTime = Date.UTC(target.year, target.month - 1, target.day);
  let candidate = targetWallTime;

  for (let attempt = 0; attempt < 6; attempt++) {
    const seen = partsAt(candidate, formatter);
    const seenWallTime = Date.UTC(
      seen.year,
      seen.month - 1,
      seen.day,
      seen.hour,
      seen.minute,
      seen.second,
    );
    const difference = targetWallTime - seenWallTime;
    if (difference === 0) {
      return new Date(candidate).toISOString();
    }
    candidate += difference;
  }
  throw new Error(`could not find the start of ${dateText} in ${timeZone}`);
}

function nextCalendarDate(dateText: string): string {
  const { year, month, day } = calendarDateParts(dateText);
  return new Date(Date.UTC(year, month - 1, day + 1)).toISOString().slice(0, 10);
}

async function savedTimeZone(database: AthleticDatabase): Promise<string | undefined> {
  const result = await databaseClient(database).execute(`
    SELECT athlete_profile.default_time_zone
    FROM athlete_profile
    JOIN records ON records.record_id = athlete_profile.record_id
    WHERE records.is_deleted = 0
  `);
  const value = result.rows[0]?.default_time_zone;
  return typeof value === "string" ? value : undefined;
}

async function resolveRange(
  database: AthleticDatabase,
  options: SeriesViewOptions,
): Promise<InstantRange> {
  const hasCalendarBound =
    (options.from !== undefined && /^\d{4}-\d{2}-\d{2}$/.test(options.from)) ||
    (options.to !== undefined && /^\d{4}-\d{2}-\d{2}$/.test(options.to));
  const needsTimeZone = hasCalendarBound || options.raw !== true;
  const timeZone =
    options.time_zone ?? (needsTimeZone ? await savedTimeZone(database) : undefined);
  if (needsTimeZone && timeZone === undefined) {
    throw new Error(
      "a calendar-day series view needs --timezone <IANA name> or a default timezone in the athlete profile",
    );
  }
  if (timeZone !== undefined) {
    formatterFor(timeZone);
  }

  let from: string | undefined;
  if (options.from !== undefined) {
    from = /^\d{4}-\d{2}-\d{2}$/.test(options.from)
      ? startOfDateInZone(options.from, timeZone!)
      : UtcTimestamp.parse(options.from);
  }

  let to: string | undefined;
  if (options.to !== undefined) {
    if (/^\d{4}-\d{2}-\d{2}$/.test(options.to)) {
      const nextDay = startOfDateInZone(nextCalendarDate(options.to), timeZone!);
      to = new Date(Date.parse(nextDay) - 1).toISOString();
    } else {
      to = UtcTimestamp.parse(options.to);
    }
  }
  if (from !== undefined && to !== undefined && from > to) {
    throw new Error("--from must not be after --to");
  }
  return {
    ...(from === undefined ? {} : { from }),
    ...(to === undefined ? {} : { to }),
    ...(timeZone === undefined ? {} : { time_zone: timeZone }),
  };
}

function dateInZone(timestamp: string, timeZone: string): string {
  const parts = partsAt(Date.parse(timestamp), formatterFor(timeZone));
  return `${parts.year.toString().padStart(4, "0")}-${parts.month
    .toString()
    .padStart(2, "0")}-${parts.day.toString().padStart(2, "0")}`;
}

export async function readSeriesView(
  database: AthleticDatabase,
  quantity: SeriesQuantity,
  options: SeriesViewOptions = {},
): Promise<SeriesView> {
  const range = await resolveRange(database, options);
  const limit = options.limit ?? 10_000;
  const records = await querySeries(database, {
    quantity,
    ...(range.from === undefined ? {} : { from: range.from }),
    ...(range.to === undefined ? {} : { to: range.to }),
    ...(options.source_id === undefined ? {} : { source_id: options.source_id }),
    limit,
  });
  const base = {
    quantity,
    unit: SERIES_QUANTITY_UNITS[quantity],
    ...(range.time_zone === undefined ? {} : { time_zone: range.time_zone }),
    ...(range.from === undefined ? {} : { from: range.from }),
    ...(range.to === undefined ? {} : { to: range.to }),
    ...(options.source_id === undefined ? {} : { source_id: options.source_id }),
    truncated: records.length === limit,
  };

  if (options.raw === true) {
    return { ...base, samples: records };
  }

  const groups = new Map<string, {
    date: string;
    source_id: string;
    values: number[];
    times: string[];
  }>();
  for (const record of records) {
    if (record.record_type !== "measurement_sample") {
      continue;
    }
    const date = dateInZone(record.data.recorded_at, range.time_zone!);
    const key = `${date}\u0000${record.data.source_id}`;
    const group = groups.get(key) ?? {
      date,
      source_id: record.data.source_id,
      values: [],
      times: [],
    };
    group.values.push(record.data.value.standard_value);
    group.times.push(record.data.recorded_at);
    groups.set(key, group);
  }
  const days: SeriesDay[] = [...groups.values()]
    .map((group) => ({
      date: group.date,
      source_id: group.source_id,
      sample_count: group.values.length,
      minimum: Math.min(...group.values),
      maximum: Math.max(...group.values),
      mean:
        Math.round(
          (group.values.reduce((sum, value) => sum + value, 0) / group.values.length) *
            10,
        ) / 10,
      first_at: group.times[0]!,
      last_at: group.times.at(-1)!,
    }))
    .sort(
      (left, right) =>
        left.date.localeCompare(right.date) ||
        left.source_id.localeCompare(right.source_id),
    );
  return { ...base, days };
}

export function renderSeriesView(view: SeriesView): string {
  if (view.samples !== undefined) {
    if (view.samples.length === 0) {
      return `no ${view.quantity} samples matched`;
    }
    const lines = [`${view.quantity} (${view.unit}) — raw samples`];
    for (const sample of view.samples) {
      if (sample.record_type !== "measurement_sample") {
        continue;
      }
      lines.push(
        `${sample.data.recorded_at}  ${sample.data.value.standard_value}${view.unit}  ` +
          `${sample.data.source_id}  ${sample.record_id}`,
      );
    }
    if (view.truncated) {
      lines.push("result reached --limit; narrow the time range or raise the limit");
    }
    return lines.join("\n");
  }

  if (view.days?.length === 0) {
    return `no ${view.quantity} samples matched`;
  }
  const lines = [
    `${view.quantity} (${view.unit}) by calendar day in ${view.time_zone}`,
    "date        source                                n       mean      min       max",
  ];
  for (const day of view.days ?? []) {
    lines.push(
      `${day.date}  ${day.source_id}  ${day.sample_count
        .toString()
        .padStart(6)}  ${day.mean.toString().padStart(8)}  ${day.minimum
        .toString()
        .padStart(8)}  ${day.maximum.toString().padStart(8)}`,
    );
  }
  if (view.truncated) {
    lines.push("result reached --limit; narrow the time range or raise the limit");
  }
  return lines.join("\n");
}
