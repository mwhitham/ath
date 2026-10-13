import { lstat, rename, rm } from "node:fs/promises";
import { dirname, join } from "node:path";
import type { Transaction } from "@libsql/client";
import {
  CurrentRecord,
  ErasureReasonCode,
  RecordId,
  UtcTimestamp,
  type CurrentRecordT,
} from "../v1/schema.js";
import { storedAttachmentName } from "./attachments.js";
import {
  databaseClient,
  refuseSymbolicLinkComponents,
  type AthleticDatabase,
} from "./client.js";
import { incrementDatabaseRevision } from "./records.js";
import { withWriteTransaction } from "./transaction.js";

export interface ErasureRecord {
  record_id: string;
  record_type: CurrentRecordT["record_type"];
  current_revision: number;
}

export interface ErasureAttachment {
  record_id: string;
  path: string;
  present: boolean;
}

export interface ErasurePreview {
  root_record_id: string;
  root_expected_revision: number;
  database_revision: number;
  records: ErasureRecord[];
  attachments: ErasureAttachment[];
}

export interface EraseRecordInput {
  preview: ErasurePreview;
  reason_code: "athlete_request" | "legal_requirement" | "recorded_in_error" | "other";
  erased_at?: string;
}

function dependencySql(): string {
  return `
    WITH RECURSIVE
    dependency_edges(parent_id, dependent_id) AS (
      SELECT source_id, record_id FROM source_devices
      UNION ALL SELECT source_id, record_id FROM point_measurements
      UNION ALL SELECT source_id, record_id FROM measurement_samples
      UNION ALL SELECT source_id, record_id FROM sleep_sessions
      UNION ALL SELECT source_id, record_id FROM workout_sessions
      UNION ALL SELECT source_id, record_id FROM benchmark_results
      UNION ALL SELECT source_id, record_id FROM provider_summaries
      UNION ALL SELECT source_id, record_id FROM provider_scores
      UNION ALL SELECT benchmark_definition_id, record_id FROM benchmark_results
      UNION ALL SELECT workout_session_id, record_id FROM benchmark_results
        WHERE workout_session_id IS NOT NULL
      UNION ALL SELECT benchmark_definition_id, record_id FROM predictions
      UNION ALL SELECT prediction_id, record_id FROM prediction_result_links
      UNION ALL SELECT benchmark_result_id, record_id FROM prediction_result_links
      UNION ALL SELECT prediction_result_link_id, record_id FROM prediction_grades
      UNION ALL SELECT prediction_grade_id, record_id FROM prediction_error_analyses
      UNION ALL SELECT training_plan_id, record_id FROM planned_sessions
        WHERE training_plan_id IS NOT NULL
      UNION ALL SELECT completed_workout_session_id, record_id FROM planned_sessions
        WHERE completed_workout_session_id IS NOT NULL
      UNION ALL SELECT target_benchmark_definition_id, record_id FROM goals
        WHERE target_benchmark_definition_id IS NOT NULL
      UNION ALL SELECT nutrition_entry_id, record_id FROM nutrition_model_interpretations
      UNION ALL SELECT evidence_record_id, point_measurement_id
        FROM point_measurement_calculation_inputs
      UNION ALL SELECT evidence_record_id, authored_record_id
        FROM authored_record_evidence
      UNION ALL SELECT evidence_record_id, prediction_error_analysis_id
        FROM prediction_error_causes
      UNION ALL SELECT owner_record_id, attachment_record_id
        FROM record_attachment_links
      UNION ALL SELECT attachment_record_id, owner_record_id
        FROM record_attachment_links
    ),
    dependencies(record_id) AS (
      VALUES (?)
      UNION
      SELECT dependency_edges.dependent_id
      FROM dependency_edges
      JOIN dependencies
        ON dependencies.record_id = dependency_edges.parent_id
    )
    SELECT
      records.record_id,
      records.record_type,
      records.current_revision,
      records.created_at,
      records.is_deleted,
      record_revisions.record_json
    FROM dependencies
    JOIN records ON records.record_id = dependencies.record_id
    JOIN record_revisions
      ON record_revisions.record_id = records.record_id
     AND record_revisions.revision = records.current_revision
    ORDER BY records.created_at, records.record_id
  `;
}

async function currentDatabaseRevision(database: AthleticDatabase): Promise<number> {
  const result = await databaseClient(database).execute(
    "SELECT current_revision FROM database_state WHERE singleton_id = 1",
  );
  const value = result.rows[0]?.current_revision;
  if (typeof value !== "number" && typeof value !== "bigint") {
    throw new Error("Database revision is missing");
  }
  return Number(value);
}

async function dependentRecords(
  database: AthleticDatabase,
  rootRecordId: string,
): Promise<CurrentRecordT[]> {
  const result = await databaseClient(database).execute({
    sql: dependencySql(),
    args: [RecordId.parse(rootRecordId)],
  });
  return result.rows.map((row) =>
    CurrentRecord.parse({
      record_id: row.record_id,
      record_type: row.record_type,
      current_revision: Number(row.current_revision),
      created_at: row.created_at,
      is_deleted: Number(row.is_deleted) === 1,
      data: JSON.parse(String(row.record_json)) as unknown,
    }),
  );
}

export async function previewRecordErasure(
  database: AthleticDatabase,
  recordId: string,
  expectedRevision: number,
): Promise<ErasurePreview> {
  const parsedRecordId = RecordId.parse(recordId);
  if (!Number.isInteger(expectedRevision) || expectedRevision < 1) {
    throw new Error("Expected revision must be a positive whole number");
  }
  const records = await dependentRecords(database, parsedRecordId);
  const root = records.find((record) => record.record_id === parsedRecordId);
  if (root === undefined) {
    const erased = await databaseClient(database).execute({
      sql: "SELECT 1 AS present FROM record_erasure_markers WHERE record_id = ?",
      args: [parsedRecordId],
    });
    throw new Error(
      erased.rows.length > 0
        ? `Record was already erased: ${parsedRecordId}`
        : `Record not found: ${parsedRecordId}`,
    );
  }
  if (root.current_revision !== expectedRevision) {
    throw new Error(
      `Record ${parsedRecordId} is at revision ${root.current_revision}, not expected revision ${expectedRevision}`,
    );
  }

  const attachments: ErasureAttachment[] = [];
  if (database.localPath !== undefined) {
    for (const record of records) {
      if (record.record_type !== "attachment") {
        continue;
      }
      const path = join(dirname(database.localPath), "attachments", storedAttachmentName(record));
      await refuseSymbolicLinkComponents(path, { allowMissingTail: true });
      let present = false;
      try {
        const stat = await lstat(path);
        if (!stat.isFile()) {
          throw new Error(`Attachment is not a regular file: ${path}`);
        }
        present = true;
      } catch (error) {
        if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) {
          throw error;
        }
      }
      attachments.push({ record_id: record.record_id, path, present });
    }
  }

  return {
    root_record_id: parsedRecordId,
    root_expected_revision: expectedRevision,
    database_revision: await currentDatabaseRevision(database),
    records: records.map((record) => ({
      record_id: record.record_id,
      record_type: record.record_type,
      current_revision: record.current_revision,
    })),
    attachments,
  };
}

async function removeRecords(
  transaction: Transaction,
  recordIds: readonly string[],
  erasedAt: string,
  reasonCode: string,
): Promise<void> {
  const chunkSize = 500;
  for (let offset = 0; offset < recordIds.length; offset += chunkSize) {
    const chunk = recordIds.slice(offset, offset + chunkSize);
    const placeholders = chunk.map(() => "?").join(", ");
    await transaction.execute({
      sql: `DELETE FROM record_deletions WHERE record_id IN (${placeholders})`,
      args: chunk,
    });
    await transaction.execute({
      sql: `DELETE FROM records WHERE record_id IN (${placeholders})`,
      args: chunk,
    });
  }
  for (const recordId of recordIds) {
    await transaction.execute({
      sql: `
        INSERT INTO record_erasure_markers (record_id, erased_at, reason_code)
        VALUES (?, ?, ?)
      `,
      args: [recordId, erasedAt, reasonCode],
    });
  }
}

export async function eraseRecord(
  database: AthleticDatabase,
  input: EraseRecordInput,
): Promise<{
  erased_record_ids: string[];
  erased_attachment_count: number;
  database_revision: number;
}> {
  const reasonCode = ErasureReasonCode.parse(input.reason_code);
  const erasedAt = UtcTimestamp.parse(input.erased_at ?? new Date().toISOString());
  const stagedAttachments: Array<{ original: string; temporary: string }> = [];

  try {
    for (const attachment of input.preview.attachments) {
      if (!attachment.present) {
        continue;
      }
      await refuseSymbolicLinkComponents(attachment.path);
      const temporary = `${attachment.path}.erase-${process.pid}`;
      await rename(attachment.path, temporary);
      stagedAttachments.push({ original: attachment.path, temporary });
    }

    const result = await withWriteTransaction(database, async (transaction) => {
      const state = await transaction.execute(
        "SELECT current_revision FROM database_state WHERE singleton_id = 1",
      );
      if (Number(state.rows[0]?.current_revision) !== input.preview.database_revision) {
        throw new Error("Database changed after the erasure preview; preview it again");
      }
      const root = await transaction.execute({
        sql: "SELECT current_revision FROM records WHERE record_id = ?",
        args: [input.preview.root_record_id],
      });
      if (
        root.rows.length !== 1 ||
        Number(root.rows[0]?.current_revision) !== input.preview.root_expected_revision
      ) {
        throw new Error("Record changed after the erasure preview; preview it again");
      }

      const freshIds = (await transaction.execute({
        sql: dependencySql(),
        args: [input.preview.root_record_id],
      })).rows.map((row) => String(row.record_id));
      const expectedIds = input.preview.records.map((record) => record.record_id);
      if (
        freshIds.length !== expectedIds.length ||
        freshIds.some((recordId, index) => recordId !== expectedIds[index])
      ) {
        throw new Error("Erasure dependencies changed after preview; preview it again");
      }

      await removeRecords(transaction, expectedIds, erasedAt, reasonCode);
      return incrementDatabaseRevision(transaction);
    });

    await Promise.all(stagedAttachments.map((attachment) => rm(attachment.temporary, { force: true })));
    return {
      erased_record_ids: input.preview.records.map((record) => record.record_id),
      erased_attachment_count: stagedAttachments.length,
      database_revision: result,
    };
  } catch (error) {
    for (const attachment of stagedAttachments.reverse()) {
      try {
        await rename(attachment.temporary, attachment.original);
      } catch {
        // Keep the original error. A staged file remains discoverable by its .erase suffix.
      }
    }
    throw error;
  }
}
