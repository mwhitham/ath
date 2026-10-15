import {
  CurrentRecord,
  RecordType,
  SeriesQuantitySchema,
  PointMeasurementTypeSchema,
  UtcTimestamp,
  type CurrentRecordT,
  type PointMeasurementType,
  type RecordTypeT,
  type SeriesQuantity,
} from "../v1/schema.js";
import { databaseClient, type AthleticDatabase } from "./client.js";
import { currentRecordPayloadFromRow } from "./revision-encoding.js";

type SqlValue = string | number | bigint | boolean | Uint8Array | ArrayBuffer | null;

export interface TimeRange {
  from?: string;
  to?: string;
}

export interface MeasurementQuery extends TimeRange {
  quantity?: PointMeasurementType;
  source_id?: string;
  include_manual?: boolean;
  limit?: number;
}

export interface SeriesQuery extends TimeRange {
  quantity: SeriesQuantity;
  source_id?: string;
  include_manual?: boolean;
  limit?: number;
  order?: "ascending" | "descending";
}

export interface SessionQuery extends TimeRange {
  session_type?: "sleep_session" | "workout_session";
  source_id?: string;
  include_manual?: boolean;
  limit?: number;
}

export interface AsOfQuery {
  as_of: string;
  record_type?: RecordTypeT;
  record_types?: readonly RecordTypeT[];
  limit?: number;
}

function queryLimit(value: number | undefined): number {
  const limit = value ?? 10_000;
  if (!Number.isInteger(limit) || limit < 1 || limit > 100_000) {
    throw new Error("Query limit must be between 1 and 100000");
  }
  return limit;
}

function timeRangeConditions(
  column: string,
  query: TimeRange,
  conditions: string[],
  args: SqlValue[],
): void {
  if (query.from !== undefined) {
    conditions.push(`${column} >= ?`);
    args.push(UtcTimestamp.parse(query.from));
  }
  if (query.to !== undefined) {
    conditions.push(`${column} <= ?`);
    args.push(UtcTimestamp.parse(query.to));
  }
}

function parseCurrentRow(row: Record<string, unknown>): CurrentRecordT {
  return CurrentRecord.parse(currentRecordPayloadFromRow(row));
}

export async function queryMeasurements(
  database: AthleticDatabase,
  query: MeasurementQuery = {},
): Promise<CurrentRecordT[]> {
  const conditions = ["records.is_deleted = 0"];
  const args: SqlValue[] = [];
  if (query.quantity !== undefined) {
    conditions.push("point_measurements.quantity = ?");
    args.push(PointMeasurementTypeSchema.parse(query.quantity));
  }
  if (query.source_id !== undefined) {
    conditions.push("point_measurements.source_id = ?");
    args.push(query.source_id);
  }
  if (query.include_manual !== true) {
    conditions.push("sources.source_type <> 'manual'");
  }
  timeRangeConditions("point_measurements.recorded_at", query, conditions, args);
  args.push(queryLimit(query.limit));

  const result = await databaseClient(database).execute({
    sql: `
      SELECT
        records.record_id,
        records.record_type,
        records.current_revision,
        records.created_at,
        records.is_deleted,
        record_revisions.record_json,
        record_revisions.extensions_json
      FROM point_measurements
      JOIN records ON records.record_id = point_measurements.record_id
      JOIN record_revisions
        ON record_revisions.record_id = records.record_id
       AND record_revisions.revision = records.current_revision
      JOIN sources ON sources.record_id = point_measurements.source_id
      WHERE ${conditions.join(" AND ")}
      ORDER BY point_measurements.recorded_at, records.record_id
      LIMIT ?
    `,
    args,
  });
  return result.rows.map((row) => parseCurrentRow(row));
}

export async function querySeries(
  database: AthleticDatabase,
  query: SeriesQuery,
): Promise<CurrentRecordT[]> {
  const conditions = ["records.is_deleted = 0", "measurement_samples.quantity = ?"];
  const args: SqlValue[] = [SeriesQuantitySchema.parse(query.quantity)];
  if (query.source_id !== undefined) {
    conditions.push("measurement_samples.source_id = ?");
    args.push(query.source_id);
  }
  if (query.include_manual !== true) {
    conditions.push("sources.source_type <> 'manual'");
  }
  timeRangeConditions("measurement_samples.recorded_at", query, conditions, args);
  args.push(queryLimit(query.limit));

  const result = await databaseClient(database).execute({
    sql: `
      SELECT
        records.record_id,
        records.record_type,
        records.current_revision,
        records.created_at,
        records.is_deleted,
        record_revisions.record_json,
        record_revisions.extensions_json
      FROM measurement_samples
      JOIN records ON records.record_id = measurement_samples.record_id
      JOIN record_revisions
        ON record_revisions.record_id = records.record_id
       AND record_revisions.revision = records.current_revision
      JOIN sources ON sources.record_id = measurement_samples.source_id
      WHERE ${conditions.join(" AND ")}
      ORDER BY
        measurement_samples.recorded_at ${query.order === "descending" ? "DESC" : "ASC"},
        records.record_id ${query.order === "descending" ? "DESC" : "ASC"}
      LIMIT ?
    `,
    args,
  });
  return result.rows.map((row) => parseCurrentRow(row));
}

export async function querySessions(
  database: AthleticDatabase,
  query: SessionQuery = {},
): Promise<CurrentRecordT[]> {
  const types =
    query.session_type === undefined
      ? (["sleep_session", "workout_session"] as const)
      : ([query.session_type] as const);
  const records: CurrentRecordT[] = [];
  const limit = queryLimit(query.limit);

  for (const sessionType of types) {
    const table = sessionType === "sleep_session" ? "sleep_sessions" : "workout_sessions";
    const conditions = ["records.is_deleted = 0"];
    const args: SqlValue[] = [];
    if (query.source_id !== undefined) {
      conditions.push(`${table}.source_id = ?`);
      args.push(query.source_id);
    }
    if (query.include_manual !== true) {
      conditions.push("sources.source_type <> 'manual'");
    }
    timeRangeConditions(`${table}.start_at`, query, conditions, args);
    args.push(limit);

    const result = await databaseClient(database).execute({
      sql: `
        SELECT
          records.record_id,
          records.record_type,
          records.current_revision,
          records.created_at,
          records.is_deleted,
          record_revisions.record_json,
        record_revisions.extensions_json
        FROM ${table}
        JOIN records ON records.record_id = ${table}.record_id
        JOIN record_revisions
          ON record_revisions.record_id = records.record_id
         AND record_revisions.revision = records.current_revision
        JOIN sources ON sources.record_id = ${table}.source_id
        WHERE ${conditions.join(" AND ")}
        ORDER BY ${table}.start_at, records.record_id
        LIMIT ?
      `,
      args,
    });
    records.push(...result.rows.map((row) => parseCurrentRow(row)));
  }

  return records
    .sort((left, right) => {
      const leftStart =
        left.record_type === "sleep_session" || left.record_type === "workout_session"
          ? left.data.start_at
          : "";
      const rightStart =
        right.record_type === "sleep_session" || right.record_type === "workout_session"
          ? right.data.start_at
          : "";
      return leftStart.localeCompare(rightStart) || left.record_id.localeCompare(right.record_id);
    })
    .slice(0, limit);
}

export async function queryRecordsAsOf(
  database: AthleticDatabase,
  query: AsOfQuery,
): Promise<CurrentRecordT[]> {
  const asOf = UtcTimestamp.parse(query.as_of);
  if (query.record_type !== undefined && query.record_types !== undefined) {
    throw new Error("Use record_type or record_types, not both");
  }
  const recordTypes =
    query.record_types === undefined
      ? query.record_type === undefined
        ? []
        : [RecordType.parse(query.record_type)]
      : query.record_types.map((recordType) => RecordType.parse(recordType));
  if (query.record_types !== undefined && recordTypes.length === 0) {
    throw new Error("record_types must contain at least one record type");
  }
  const typeCondition =
    recordTypes.length === 0
      ? ""
      : `WHERE candidate_records.record_type IN (${recordTypes.map(() => "?").join(", ")})`;
  const conditions = [
    "visible_revisions.revision IS NOT NULL",
    `NOT EXISTS (
      SELECT 1
      FROM record_deletions
      WHERE record_deletions.record_id = record_revisions.record_id
        AND record_deletions.deleted_at <= ?
    )`,
  ];
  const args: SqlValue[] = [asOf, ...recordTypes, asOf];
  args.push(queryLimit(query.limit));

  const result = await databaseClient(database).execute({
    sql: `
      WITH visible_revisions AS (
        SELECT
          candidate_records.record_id,
          (
            SELECT MAX(candidate_revisions.revision)
            FROM record_revisions AS candidate_revisions
            WHERE candidate_revisions.record_id = candidate_records.record_id
              AND candidate_revisions.became_known_at <= ?
          ) AS revision
        FROM records AS candidate_records
        ${typeCondition}
      )
      SELECT
        records.record_id,
        record_revisions.record_type,
        record_revisions.revision AS current_revision,
        records.created_at,
        0 AS is_deleted,
        record_revisions.record_json,
        record_revisions.extensions_json
      FROM visible_revisions
      JOIN record_revisions
        ON record_revisions.record_id = visible_revisions.record_id
       AND record_revisions.revision = visible_revisions.revision
      JOIN records ON records.record_id = record_revisions.record_id
      WHERE ${conditions.join(" AND ")}
      ORDER BY records.created_at, records.record_id
      LIMIT ?
    `,
    args,
  });
  return result.rows.map((row) => parseCurrentRow(row));
}
