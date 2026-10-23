import type { Client } from "@libsql/client";
import { CurrentRecord, type CurrentRecordT } from "../v1/schema.js";
import { typedTables } from "./check-tables.js";
import { typedMainRowFields } from "./records.js";

function normalizeSqlValue(value: unknown): string | number | null {
  if (value === null || value === undefined) {
    return null;
  }
  if (typeof value === "bigint") {
    return Number(value);
  }
  if (typeof value === "boolean") {
    return value ? 1 : 0;
  }
  if (typeof value === "number") {
    return value;
  }
  return String(value);
}

function sqlValuesEqual(left: unknown, right: unknown): boolean {
  const a = normalizeSqlValue(left);
  const b = normalizeSqlValue(right);
  if (a === null && b === null) {
    return true;
  }
  if (a === null || b === null) {
    return false;
  }
  if (typeof a === "number" && typeof b === "number") {
    return a === b;
  }
  return String(a) === String(b);
}

function parseRecordFromJoinedRow(row: Record<string, unknown>): CurrentRecordT {
  return CurrentRecord.parse({
    record_id: String(row.record_id),
    record_type: String(row.record_type),
    created_at: String(row.created_at),
    current_revision: Number(row.current_revision),
    is_deleted: Number(row.is_deleted) === 1,
    data: JSON.parse(String(row.data_json)),
  });
}

function mainRowMismatch(row: Record<string, unknown>, record: CurrentRecordT): boolean {
  for (const [column, expected] of typedMainRowFields(record)) {
    if (!sqlValuesEqual(row[column], expected)) {
      return true;
    }
  }
  return false;
}

function jsonChildEqual(left: unknown, right: unknown): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

async function providerReportingPeriodsMismatch(
  client: Client,
  record: CurrentRecordT,
): Promise<boolean> {
  if (!("provider_reporting_periods" in record.data)) {
    const count = await client.execute({
      sql: "SELECT COUNT(*) AS count FROM provider_reporting_periods WHERE owner_record_id = ?",
      args: [record.record_id],
    });
    return Number(count.rows[0]?.count) !== 0;
  }
  const periods = record.data.provider_reporting_periods ?? [];
  const rows = await client.execute({
    sql: `
      SELECT period_order, period_type, provider_period_id, provider_date, start_at, end_at, period_json
      FROM provider_reporting_periods
      WHERE owner_record_id = ?
      ORDER BY period_order
    `,
    args: [record.record_id],
  });
  if (rows.rows.length !== periods.length) {
    return true;
  }
  for (const [index, period] of periods.entries()) {
    const row = rows.rows[index];
    if (row === undefined) {
      return true;
    }
    if (Number(row.period_order) !== index) {
      return true;
    }
    if (!sqlValuesEqual(row.period_type, period.period_type)) {
      return true;
    }
    if (!sqlValuesEqual(row.provider_period_id, period.provider_period_id ?? null)) {
      return true;
    }
    if (!sqlValuesEqual(row.provider_date, period.provider_date ?? null)) {
      return true;
    }
    if (!sqlValuesEqual(row.start_at, period.start_at ?? null)) {
      return true;
    }
    if (!sqlValuesEqual(row.end_at, period.end_at ?? null)) {
      return true;
    }
    if (!jsonChildEqual(JSON.parse(String(row.period_json)), period)) {
      return true;
    }
  }
  return false;
}

async function orderedJsonChildrenMismatch(
  client: Client,
  options: {
    table: string;
    parentColumn: string;
    orderColumn: string;
    jsonColumn: string;
    parentId: string;
    children: readonly unknown[];
  },
): Promise<boolean> {
  const rows = await client.execute({
    sql: `
      SELECT ${options.orderColumn}, ${options.jsonColumn}
      FROM ${options.table}
      WHERE ${options.parentColumn} = ?
      ORDER BY ${options.orderColumn}
    `,
    args: [options.parentId],
  });
  if (rows.rows.length !== options.children.length) {
    return true;
  }
  for (const [index, child] of options.children.entries()) {
    const row = rows.rows[index];
    if (row === undefined) {
      return true;
    }
    if (Number(row[options.orderColumn]) !== index) {
      return true;
    }
    if (!jsonChildEqual(JSON.parse(String(row[options.jsonColumn])), child)) {
      return true;
    }
  }
  return false;
}

async function attachmentLinksMismatch(
  client: Client,
  recordId: string,
  attachmentIds: readonly string[],
): Promise<boolean> {
  const rows = await client.execute({
    sql: `
      SELECT attachment_order, attachment_record_id
      FROM record_attachment_links
      WHERE owner_record_id = ?
      ORDER BY attachment_order
    `,
    args: [recordId],
  });
  if (rows.rows.length !== attachmentIds.length) {
    return true;
  }
  for (const [index, attachmentId] of attachmentIds.entries()) {
    const row = rows.rows[index];
    if (row === undefined) {
      return true;
    }
    if (Number(row.attachment_order) !== index) {
      return true;
    }
    if (String(row.attachment_record_id) !== attachmentId) {
      return true;
    }
  }
  return false;
}

async function typedChildrenMismatch(client: Client, record: CurrentRecordT): Promise<boolean> {
  switch (record.record_type) {
    case "point_measurement": {
      const calculations = record.data.calculations ?? [];
      if (
        await orderedJsonChildrenMismatch(client, {
          table: "point_measurement_calculations",
          parentColumn: "point_measurement_id",
          orderColumn: "calculation_order",
          jsonColumn: "calculation_json",
          parentId: record.record_id,
          children: calculations,
        })
      ) {
        return true;
      }
      const inputRows = await client.execute({
        sql: `
          SELECT calculation_order, input_order, evidence_record_id, evidence_revision
          FROM point_measurement_calculation_inputs
          WHERE point_measurement_id = ?
          ORDER BY calculation_order, input_order
        `,
        args: [record.record_id],
      });
      const expectedInputs: Array<{
        calculation_order: number;
        input_order: number;
        evidence_record_id: string;
        evidence_revision: number;
      }> = [];
      for (const [calculationIndex, calculation] of calculations.entries()) {
        for (const [inputIndex, input] of calculation.input_records.entries()) {
          expectedInputs.push({
            calculation_order: calculationIndex,
            input_order: inputIndex,
            evidence_record_id: input.record_id,
            evidence_revision: input.revision,
          });
        }
      }
      if (inputRows.rows.length !== expectedInputs.length) {
        return true;
      }
      for (const [index, expected] of expectedInputs.entries()) {
        const row = inputRows.rows[index];
        if (row === undefined) {
          return true;
        }
        if (
          Number(row.calculation_order) !== expected.calculation_order ||
          Number(row.input_order) !== expected.input_order ||
          String(row.evidence_record_id) !== expected.evidence_record_id ||
          Number(row.evidence_revision) !== expected.evidence_revision
        ) {
          return true;
        }
      }
      break;
    }
    case "workout_session":
      if (
        await orderedJsonChildrenMismatch(client, {
          table: "workout_session_segments",
          parentColumn: "workout_session_id",
          orderColumn: "segment_order",
          jsonColumn: "segment_json",
          parentId: record.record_id,
          children: record.data.segments ?? [],
        })
      ) {
        return true;
      }
      break;
    case "benchmark_result":
      if (
        await orderedJsonChildrenMismatch(client, {
          table: "benchmark_result_segments",
          parentColumn: "benchmark_result_id",
          orderColumn: "segment_order",
          jsonColumn: "segment_json",
          parentId: record.record_id,
          children: record.data.segments ?? [],
        })
      ) {
        return true;
      }
      break;
    case "nutrition_entry":
      if (
        await orderedJsonChildrenMismatch(client, {
          table: "reported_nutrition_items",
          parentColumn: "nutrition_entry_id",
          orderColumn: "item_order",
          jsonColumn: "item_json",
          parentId: record.record_id,
          children: record.data.reported_items,
        })
      ) {
        return true;
      }
      if (
        await attachmentLinksMismatch(
          client,
          record.record_id,
          record.data.attachment_ids ?? [],
        )
      ) {
        return true;
      }
      break;
    case "athlete_report":
      if (
        await attachmentLinksMismatch(
          client,
          record.record_id,
          record.data.attachment_ids ?? [],
        )
      ) {
        return true;
      }
      break;
    case "planned_session":
      if (
        await orderedJsonChildrenMismatch(client, {
          table: "planned_session_blocks",
          parentColumn: "planned_session_id",
          orderColumn: "block_order",
          jsonColumn: "block_json",
          parentId: record.record_id,
          children: record.data.blocks,
        })
      ) {
        return true;
      }
      break;
    case "prediction_error_analysis": {
      const causeRows = await client.execute({
        sql: `
          SELECT cause_order, evidence_record_id, evidence_revision, explanation
          FROM prediction_error_causes
          WHERE prediction_error_analysis_id = ?
          ORDER BY cause_order
        `,
        args: [record.record_id],
      });
      const causes = record.data.causes;
      if (causeRows.rows.length !== causes.length) {
        return true;
      }
      for (const [index, cause] of causes.entries()) {
        const row = causeRows.rows[index];
        if (row === undefined) {
          return true;
        }
        if (
          Number(row.cause_order) !== index ||
          String(row.evidence_record_id) !== cause.evidence.record_id ||
          Number(row.evidence_revision) !== cause.evidence.revision ||
          String(row.explanation) !== cause.explanation
        ) {
          return true;
        }
      }
      break;
    }
    default:
      break;
  }

  if ("authorship" in record.data) {
    const evidenceRows = await client.execute({
      sql: `
        SELECT evidence_order, evidence_record_id, evidence_revision
        FROM authored_record_evidence
        WHERE authored_record_id = ?
        ORDER BY evidence_order
      `,
      args: [record.record_id],
    });
    const evidence = record.data.authorship.evidence;
    if (evidenceRows.rows.length !== evidence.length) {
      return true;
    }
    for (const [index, item] of evidence.entries()) {
      const row = evidenceRows.rows[index];
      if (row === undefined) {
        return true;
      }
      if (
        Number(row.evidence_order) !== index ||
        String(row.evidence_record_id) !== item.record_id ||
        Number(row.evidence_revision) !== item.revision
      ) {
        return true;
      }
    }
  }

  return providerReportingPeriodsMismatch(client, record);
}

async function recordHasDenormalizedMismatch(
  client: Client,
  recordType: string,
  table: string,
  recordId: string,
): Promise<boolean> {
  const result = await client.execute({
    sql: `
      SELECT
        records.record_id,
        records.record_type,
        records.current_revision,
        records.created_at,
        records.is_deleted,
        ${table}.data_json AS data_json
      FROM records
      JOIN ${table} ON ${table}.record_id = records.record_id
      WHERE records.record_id = ?
    `,
    args: [recordId],
  });
  const row = result.rows[0];
  if (row === undefined) {
    return false;
  }
  const joined = row as Record<string, unknown>;
  const record = parseRecordFromJoinedRow(joined);
  const typedRow = await client.execute({
    sql: `SELECT * FROM ${table} WHERE record_id = ?`,
    args: [recordId],
  });
  const typed = typedRow.rows[0] as Record<string, unknown> | undefined;
  if (typed === undefined) {
    return true;
  }
  if (mainRowMismatch(typed, record)) {
    return true;
  }
  return typedChildrenMismatch(client, record);
}

/** Record IDs whose typed-table denormalized columns or child rows disagree with data_json. */
export async function listTypedDenormalizedMismatchRecordIds(
  client: Client,
): Promise<string[]> {
  const mismatches: string[] = [];
  for (const [recordType, table] of Object.entries(typedTables)) {
    const rows = await client.execute({
      sql: `
        SELECT records.record_id
        FROM records
        JOIN ${table} ON ${table}.record_id = records.record_id
        WHERE records.record_type = ?
          AND records.is_deleted = 0
        ORDER BY records.record_id
      `,
      args: [recordType],
    });
    for (const row of rows.rows) {
      const recordId = String(row.record_id);
      if (await recordHasDenormalizedMismatch(client, recordType, table, recordId)) {
        mismatches.push(recordId);
      }
    }
  }
  return mismatches;
}
