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
import {
  assertAttachmentFileType,
  storedAttachmentName,
} from "./attachments.js";
import { validateTypedRelationships } from "./records.js";
import {
  currentRecordPayloadFromRow,
  recordRevisionPayloadFromRow,
} from "./revision-encoding.js";

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

const MAX_REPORTED_ISSUES = 1_000;

class CheckIssues extends Array<DatabaseCheckIssue> {
  truncatedCount = 0;
  truncatedErrorCount = 0;

  override push(...items: DatabaseCheckIssue[]): number {
    for (const item of items) {
      if (this.length < MAX_REPORTED_ISSUES) {
        super.push(item);
      } else {
        this.truncatedCount++;
        if (item.severity === "error") {
          this.truncatedErrorCount++;
        }
      }
    }
    return this.length;
  }
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
  provider_summary: "provider_summaries",
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
  try {
    assertAttachmentFileType(record, bytes);
  } catch (error) {
    issues.push({
      severity: "error",
      check: "attachment",
      record_id: record.record_id,
      message: (error as Error).message,
    });
  }
}

export async function checkDatabase(
  database: AthleticDatabase,
): Promise<DatabaseCheckResult> {
  const issues = new CheckIssues();
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

  const recordCountResult = await client.execute(
    "SELECT COUNT(*) AS count FROM records",
  );
  const recordCount = Number(recordCountResult.rows[0]?.count);
  const relationshipRecords: CurrentRecordT[] = [];
  const pageSize = 5_000;
  let currentAfter = "";
  while (true) {
    const currentRows = await client.execute({
      sql: `
        SELECT
          records.record_id,
          records.record_type,
          records.current_revision,
          records.created_at,
          records.is_deleted,
          record_revisions.record_json,
          record_revisions.extensions_json
        FROM records
        LEFT JOIN record_revisions
          ON record_revisions.record_id = records.record_id
         AND record_revisions.revision = records.current_revision
        WHERE records.record_id > ?
        ORDER BY records.record_id
        LIMIT ?
      `,
      args: [currentAfter, pageSize],
    });
    if (currentRows.rows.length === 0) {
      break;
    }
    for (const row of currentRows.rows) {
      currentAfter = String(row.record_id);
      if (row.record_json === null) {
        issues.push({
          severity: "error",
          check: "revision",
          record_id: currentAfter,
          message: `current revision ${String(row.current_revision)} is missing`,
        });
        continue;
      }
      const parsed = CurrentRecord.safeParse(currentRecordPayloadFromRow(row));
      if (!parsed.success) {
        issues.push({
          severity: "error",
          check: "domain",
          record_id: currentAfter,
          message: validationMessage(parsed.error),
        });
        continue;
      }
      const record = parsed.data;
      if (
        record.record_type === "prediction_result_link" ||
        record.record_type === "prediction_grade"
      ) {
        relationshipRecords.push(record);
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
  }

  let revisionAfterId = "";
  let revisionAfterNumber = 0;
  while (true) {
    const revisionRows = await client.execute({
      sql: `
        SELECT
          record_id,
          record_type,
          revision,
          previous_revision,
          became_known_at,
          became_known_at_basis,
          changed_by_json,
          record_json,
          extensions_json
        FROM record_revisions
        WHERE record_id > ? OR (record_id = ? AND revision > ?)
        ORDER BY record_id, revision
        LIMIT ?
      `,
      args: [
        revisionAfterId,
        revisionAfterId,
        revisionAfterNumber,
        pageSize,
      ],
    });
    if (revisionRows.rows.length === 0) {
      break;
    }
    for (const row of revisionRows.rows) {
      revisionAfterId = String(row.record_id);
      revisionAfterNumber = Number(row.revision);
      const parsed = RecordRevision.safeParse(recordRevisionPayloadFromRow(row));
      if (!parsed.success) {
        issues.push({
          severity: "error",
          check: "revision",
          record_id: revisionAfterId,
          message: validationMessage(parsed.error),
        });
      }
    }
  }

  const brokenRevisionChains = await client.execute(`
    WITH ordered_revisions AS (
      SELECT
        record_revisions.record_id,
        record_revisions.record_type,
        record_revisions.revision,
        record_revisions.previous_revision,
        records.record_type AS current_record_type,
        records.current_revision,
        ROW_NUMBER() OVER (
          PARTITION BY record_revisions.record_id
          ORDER BY record_revisions.revision
        ) AS expected_revision,
        MAX(record_revisions.revision) OVER (
          PARTITION BY record_revisions.record_id
        ) AS final_revision
      FROM record_revisions
      JOIN records ON records.record_id = record_revisions.record_id
    )
    SELECT *
    FROM ordered_revisions
    WHERE
      revision <> expected_revision OR
      previous_revision IS NOT (
        CASE WHEN expected_revision = 1 THEN NULL ELSE expected_revision - 1 END
      ) OR
      record_type <> current_record_type OR
      final_revision <> current_revision
    ORDER BY record_id, revision
  `);
  for (const row of brokenRevisionChains.rows) {
    issues.push({
      severity: "error",
      check: "revision_chain",
      record_id: String(row.record_id),
      message:
        `revision chain breaks at stored revision ${String(row.revision)}; ` +
        `expected ${String(row.expected_revision)} after ` +
        `${String(Number(row.expected_revision) === 1 ? null : Number(row.expected_revision) - 1)}`,
    });
  }

  const deletionRows = await client.execute(
    "SELECT record_id, record_revision, deleted_at, deleted_by_json FROM record_deletions ORDER BY record_id",
  );
  for (const row of deletionRows.rows) {
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
  const deletionMismatches = await client.execute(`
    SELECT records.record_id
    FROM records
    LEFT JOIN record_deletions
      ON record_deletions.record_id = records.record_id
    WHERE
      records.is_deleted <>
        CASE WHEN record_deletions.record_id IS NULL THEN 0 ELSE 1 END
    ORDER BY records.record_id
  `);
  for (const row of deletionMismatches.rows) {
    issues.push({
      severity: "error",
      check: "deletion",
      record_id: String(row.record_id),
      message: "records.is_deleted does not match record_deletions",
    });
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
  }
  const erasuresWithCurrentRecords = await client.execute(`
    SELECT record_erasure_markers.record_id
    FROM record_erasure_markers
    JOIN records ON records.record_id = record_erasure_markers.record_id
    ORDER BY record_erasure_markers.record_id
  `);
  for (const row of erasuresWithCurrentRecords.rows) {
    issues.push({
      severity: "error",
      check: "erasure_marker",
      record_id: String(row.record_id),
      message: "erased record still has a current record row",
    });
  }

  for (const [recordType, table] of Object.entries(typedTables)) {
    const typedMismatches = await client.execute({
      sql: `
        SELECT
          records.record_id,
          ${table}.record_id AS typed_record_id,
          ${table}.data_json,
          record_revisions.record_json
        FROM records
        JOIN record_revisions
          ON record_revisions.record_id = records.record_id
         AND record_revisions.revision = records.current_revision
        LEFT JOIN ${table} ON ${table}.record_id = records.record_id
        WHERE records.record_type = ?
          AND (
            ${table}.record_id IS NULL OR
            ${table}.data_json <> record_revisions.record_json
          )
        ORDER BY records.record_id
      `,
      args: [recordType],
    });
    for (const row of typedMismatches.rows) {
      issues.push({
        severity: "error",
        check: "typed_table",
        record_id: String(row.record_id),
        message:
          row.typed_record_id === null
            ? `${recordType} is missing its ${table} row`
            : `${table}.data_json does not match the current revision`,
      });
    }
  }

  const relationshipTransaction = await client.transaction("read");
  try {
    for (const record of relationshipRecords) {
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

  const reportedIssues: DatabaseCheckIssue[] = [...issues];
  if (issues.truncatedCount > 0) {
    reportedIssues.push({
      severity: "warning",
      check: "issue_limit",
      message:
        `${issues.truncatedCount} more issue${issues.truncatedCount === 1 ? " was" : "s were"} ` +
        `not listed after the first ${MAX_REPORTED_ISSUES}`,
    });
  }
  return {
    metadata,
    valid:
      issues.truncatedErrorCount === 0 &&
      !issues.some((issue) => issue.severity === "error"),
    record_count: recordCount,
    issues: reportedIssues,
  };
}
