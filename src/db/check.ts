import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import {
  CurrentRecord,
  RecordDeletion,
  RecordErasureMarker,
  RecordRevision,
  type CurrentRecordT,
} from "../v1/schema.js";
import {
  databaseClient,
  readApplicationId,
  readUserVersion,
  refuseSymbolicLinkComponents,
  SQL_SCHEMA_VERSION,
  SQLITE_APPLICATION_ID,
  type AthleticDatabase,
} from "./client.js";
import { readDatabaseMetadata, type DatabaseMetadata } from "./migrate.js";
import { storedAttachmentName } from "./attachments.js";
import { validateTypedRelationships } from "./records.js";

export interface DatabaseCheckIssue {
  severity: "error" | "warning";
  check: string;
  message: string;
  record_id?: string;
}

export interface DatabaseCheckResult {
  metadata: DatabaseMetadata | null;
  valid: boolean;
  record_count: number;
  issues: DatabaseCheckIssue[];
}

const typedTables: Readonly<Record<string, string>> = {
  athlete_profile: "athlete_profile",
  source: "sources",
  source_device: "source_devices",
  point_measurement: "point_measurements",
  measurement_sample: "measurement_samples",
  sleep_session: "sleep_sessions",
  workout_session: "workout_sessions",
  benchmark_result: "benchmark_results",
  provider_score: "provider_scores",
  athlete_report: "athlete_reports",
  nutrition_entry: "nutrition_entries",
  attachment: "attachments",
  benchmark_definition: "benchmark_definitions",
  prediction: "predictions",
  prediction_result_link: "prediction_result_links",
  prediction_grade: "prediction_grades",
  prediction_error_analysis: "prediction_error_analyses",
  training_plan: "training_plans",
  planned_session: "planned_sessions",
  coaching_note: "coaching_notes",
  saved_training_review: "saved_training_reviews",
  goal: "goals",
  nutrition_model_interpretation: "nutrition_model_interpretations",
};

function validationMessage(error: { issues: readonly { path: PropertyKey[]; message: string }[] }): string {
  const first = error.issues[0];
  if (first === undefined) {
    return "record does not match its domain schema";
  }
  const path = first.path.length === 0 ? "record" : first.path.join(".");
  return `${path}: ${first.message}`;
}

async function checkAttachment(
  database: AthleticDatabase,
  record: Extract<CurrentRecordT, { record_type: "attachment" }>,
  issues: DatabaseCheckIssue[],
): Promise<void> {
  if (database.localPath === undefined) {
    return;
  }
  const path = join(database.localPath, "..", "attachments", storedAttachmentName(record));
  let bytes: Buffer;
  try {
    await refuseSymbolicLinkComponents(path);
    bytes = await readFile(path);
  } catch (error) {
    issues.push({
      severity: "error",
      check: "attachment",
      record_id: record.record_id,
      message: `referenced attachment is missing: ${path} (${(error as Error).message})`,
    });
    return;
  }
  if (bytes.byteLength !== record.data.byte_count) {
    issues.push({
      severity: "error",
      check: "attachment",
      record_id: record.record_id,
      message: `attachment byte count is ${bytes.byteLength}, expected ${record.data.byte_count}`,
    });
  }
  const sha256 = createHash("sha256").update(bytes).digest("hex");
  if (sha256 !== record.data.sha256) {
    issues.push({
      severity: "error",
      check: "attachment",
      record_id: record.record_id,
      message: `attachment SHA-256 is ${sha256}, expected ${record.data.sha256}`,
    });
  }
}

export async function checkDatabase(
  database: AthleticDatabase,
): Promise<DatabaseCheckResult> {
  const issues: DatabaseCheckIssue[] = [];
  let metadata: DatabaseMetadata | null = null;
  try {
    metadata = await readDatabaseMetadata(database);
  } catch (error) {
    issues.push({
      severity: "error",
      check: "metadata",
      message: (error as Error).message,
    });
  }

  const client = databaseClient(database);
  const foreignKeysEnabled = await client.execute("PRAGMA foreign_keys");
  if (Number(foreignKeysEnabled.rows[0]?.foreign_keys) !== 1) {
    issues.push({
      severity: "error",
      check: "foreign_key",
      message: "SQLite foreign-key enforcement is disabled",
    });
  }
  if (metadata !== null) {
    if (metadata.sql_schema_version !== SQL_SCHEMA_VERSION) {
      issues.push({
        severity: "error",
        check: "schema_version",
        message:
          `database metadata schema version is ${metadata.sql_schema_version}, ` +
          `expected ${SQL_SCHEMA_VERSION}`,
      });
    }
    const userVersion = await readUserVersion(database);
    if (userVersion !== null && userVersion !== metadata.sql_schema_version) {
      issues.push({
        severity: "error",
        check: "schema_version",
        message:
          `SQLite user_version is ${userVersion}, but database metadata says ` +
          `${metadata.sql_schema_version}`,
      });
    }
    const applicationId = await readApplicationId(database);
    if (applicationId !== null && applicationId !== SQLITE_APPLICATION_ID) {
      issues.push({
        severity: "error",
        check: "application_id",
        message:
          `SQLite application_id is ${applicationId}, expected ${SQLITE_APPLICATION_ID}`,
      });
    }
  }
  const integrity = await client.execute("PRAGMA integrity_check");
  for (const row of integrity.rows) {
    const message = String(row.integrity_check);
    if (message !== "ok") {
      issues.push({ severity: "error", check: "integrity", message });
    }
  }

  const foreignKeys = await client.execute("PRAGMA foreign_key_check");
  for (const row of foreignKeys.rows) {
    issues.push({
      severity: "error",
      check: "foreign_key",
      message:
        `${String(row.table)} row ${String(row.rowid)} references missing row ` +
        `${String(row.parent)} (${String(row.fkid)})`,
    });
  }

  for (const table of ["athlete_reports", "nutrition_entries", "reported_nutrition_items"]) {
    const columns = await client.execute(`PRAGMA table_info(${table})`);
    if (columns.rows.some((row) => row.name === "source_id")) {
      issues.push({
        severity: "error",
        check: "two_tier_wall",
        message: `${table} must not have a source_id column`,
      });
    }
  }

  const currentRows = await client.execute(`
    SELECT
      records.record_id,
      records.record_type,
      records.current_revision,
      records.created_at,
      records.is_deleted,
      record_revisions.record_json
    FROM records
    LEFT JOIN record_revisions
      ON record_revisions.record_id = records.record_id
     AND record_revisions.revision = records.current_revision
    ORDER BY records.record_id
  `);
  const records: CurrentRecordT[] = [];
  for (const row of currentRows.rows) {
    if (row.record_json === null) {
      issues.push({
        severity: "error",
        check: "revision",
        record_id: String(row.record_id),
        message: `current revision ${String(row.current_revision)} is missing`,
      });
      continue;
    }
    const parsed = CurrentRecord.safeParse({
      record_id: row.record_id,
      record_type: row.record_type,
      current_revision: Number(row.current_revision),
      created_at: row.created_at,
      is_deleted: Number(row.is_deleted) === 1,
      data: JSON.parse(String(row.record_json)) as unknown,
    });
    if (!parsed.success) {
      issues.push({
        severity: "error",
        check: "domain",
        record_id: String(row.record_id),
        message: validationMessage(parsed.error),
      });
      continue;
    }
    records.push(parsed.data);
  }

  const revisionRows = await client.execute(`
    SELECT
      record_id,
      record_type,
      revision,
      previous_revision,
      became_known_at,
      became_known_at_basis,
      changed_by_json,
      record_json
    FROM record_revisions
    ORDER BY record_id, revision
  `);
  const revisionsByRecord = new Map<string, Array<{
    revision: number;
    previous_revision: number | null;
    record_type: string;
  }>>();
  for (const row of revisionRows.rows) {
    const parsed = RecordRevision.safeParse({
      record_id: row.record_id,
      record_type: row.record_type,
      revision: Number(row.revision),
      previous_revision:
        row.previous_revision === null ? null : Number(row.previous_revision),
      became_known_at: row.became_known_at,
      became_known_at_basis: row.became_known_at_basis,
      changed_by: JSON.parse(String(row.changed_by_json)) as unknown,
      data: JSON.parse(String(row.record_json)) as unknown,
    });
    if (!parsed.success) {
      issues.push({
        severity: "error",
        check: "revision",
        record_id: String(row.record_id),
        message: validationMessage(parsed.error),
      });
    } else {
      const group = revisionsByRecord.get(parsed.data.record_id) ?? [];
      group.push({
        revision: parsed.data.revision,
        previous_revision: parsed.data.previous_revision,
        record_type: parsed.data.record_type,
      });
      revisionsByRecord.set(parsed.data.record_id, group);
    }
  }

  for (const row of currentRows.rows) {
    const recordId = String(row.record_id);
    const revisions = revisionsByRecord.get(recordId) ?? [];
    for (const [index, revision] of revisions.entries()) {
      const expectedRevision = index + 1;
      const expectedPrevious = index === 0 ? null : index;
      if (
        revision.revision !== expectedRevision ||
        revision.previous_revision !== expectedPrevious ||
        revision.record_type !== row.record_type
      ) {
        issues.push({
          severity: "error",
          check: "revision_chain",
          record_id: recordId,
          message:
            `revision chain breaks at stored revision ${revision.revision}; ` +
            `expected ${expectedRevision} after ${String(expectedPrevious)}`,
        });
        break;
      }
    }
    if (
      revisions.length > 0 &&
      revisions.at(-1)?.revision !== Number(row.current_revision)
    ) {
      issues.push({
        severity: "error",
        check: "revision_chain",
        record_id: recordId,
        message:
          `current revision is ${String(row.current_revision)}, but retained history ` +
          `ends at ${String(revisions.at(-1)?.revision)}`,
      });
    }
  }

  const deletionRows = await client.execute(
    "SELECT record_id, record_revision, deleted_at, deleted_by_json FROM record_deletions ORDER BY record_id",
  );
  const deletedIds = new Set<string>();
  for (const row of deletionRows.rows) {
    deletedIds.add(String(row.record_id));
    const parsed = RecordDeletion.safeParse({
      record_id: row.record_id,
      record_revision: Number(row.record_revision),
      deleted_at: row.deleted_at,
      deleted_by: JSON.parse(String(row.deleted_by_json)) as unknown,
    });
    if (!parsed.success) {
      issues.push({
        severity: "error",
        check: "deletion",
        record_id: String(row.record_id),
        message: validationMessage(parsed.error),
      });
    }
  }

  const erasureRows = await client.execute(
    "SELECT record_id, erased_at, reason_code FROM record_erasure_markers ORDER BY record_id",
  );
  for (const row of erasureRows.rows) {
    const parsed = RecordErasureMarker.safeParse({
      record_id: row.record_id,
      erased_at: row.erased_at,
      reason_code: row.reason_code,
    });
    if (!parsed.success) {
      issues.push({
        severity: "error",
        check: "erasure_marker",
        record_id: String(row.record_id),
        message: validationMessage(parsed.error),
      });
      continue;
    }
    if (currentRows.rows.some((record) => record.record_id === row.record_id)) {
      issues.push({
        severity: "error",
        check: "erasure_marker",
        record_id: parsed.data.record_id,
        message: "erased record still has a current record row",
      });
    }
  }

  for (const record of records) {
    const table = typedTables[record.record_type];
    if (table === undefined) {
      issues.push({
        severity: "error",
        check: "typed_table",
        record_id: record.record_id,
        message: `record type ${record.record_type} has no typed table`,
      });
      continue;
    }
    const typed = await client.execute({
      sql: `SELECT 1 AS present FROM ${table} WHERE record_id = ?`,
      args: [record.record_id],
    });
    if (typed.rows.length !== 1) {
      issues.push({
        severity: "error",
        check: "typed_table",
        record_id: record.record_id,
        message: `${record.record_type} is missing its ${table} row`,
      });
    }
    const typedData = await client.execute({
      sql: `SELECT data_json FROM ${table} WHERE record_id = ?`,
      args: [record.record_id],
    });
    if (
      typedData.rows[0] !== undefined &&
      String(typedData.rows[0].data_json) !== JSON.stringify(record.data)
    ) {
      issues.push({
        severity: "error",
        check: "typed_table",
        record_id: record.record_id,
        message: `${table}.data_json does not match the current revision`,
      });
    }
    if (record.is_deleted !== deletedIds.has(record.record_id)) {
      issues.push({
        severity: "error",
        check: "deletion",
        record_id: record.record_id,
        message: "records.is_deleted does not match record_deletions",
      });
    }
    if (record.record_type === "attachment" && !record.is_deleted) {
      await checkAttachment(database, record, issues);
    }
    if (record.record_type === "workout_session") {
      const durationMs =
        Date.parse(record.data.end_at) - Date.parse(record.data.start_at);
      for (const [index, segment] of (record.data.segments ?? []).entries()) {
        if (
          segment.start_offset_ms !== undefined &&
          segment.end_offset_ms !== undefined &&
          segment.end_offset_ms < segment.start_offset_ms
        ) {
          issues.push({
            severity: "error",
            check: "session_order",
            record_id: record.record_id,
            message: `segment ${index + 1} ends before it starts`,
          });
        }
        if (
          (segment.start_offset_ms ?? 0) > durationMs ||
          (segment.end_offset_ms ?? 0) > durationMs
        ) {
          issues.push({
            severity: "error",
            check: "session_order",
            record_id: record.record_id,
            message: `segment ${index + 1} falls outside the workout session`,
          });
        }
      }
    }
  }

  const relationshipTransaction = await client.transaction("read");
  try {
    for (const record of records) {
      if (record.is_deleted) {
        continue;
      }
      try {
        await validateTypedRelationships(relationshipTransaction, record);
      } catch (error) {
        issues.push({
          severity: "error",
          check: "record_relationship",
          record_id: record.record_id,
          message: (error as Error).message,
        });
      }
    }
    await relationshipTransaction.rollback();
  } finally {
    relationshipTransaction.close();
  }

  return {
    metadata,
    valid: !issues.some((issue) => issue.severity === "error"),
    record_count: currentRows.rows.length,
    issues,
  };
}
