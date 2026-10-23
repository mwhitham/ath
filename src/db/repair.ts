import { lstat } from "node:fs/promises";
import type { Transaction } from "@libsql/client";
import { databaseClient, type AthleticDatabase } from "./client.js";
import { typedTables } from "./check-tables.js";
import { createVerifiedSqliteBackup, defaultRepairBackupPath } from "./backup.js";
import { readDatabaseMetadata } from "./migrate.js";
import {
  incrementDatabaseRevision,
  rebuildTypedRowFromCurrentRevision,
} from "./records.js";
import { semanticJsonEqual } from "./stable-json.js";
import { listTypedDenormalizedMismatchRecordIds } from "./typed-denormalized.js";
import { withWriteTransaction } from "./transaction.js";
import { formatSchemaValidationError } from "./validation-message.js";

export interface TypedTableMismatch {
  record_id: string;
  record_type: string;
  encoding_only: boolean;
}

async function listTypedTableMismatches(
  database: AthleticDatabase,
): Promise<TypedTableMismatch[]> {
  const client = databaseClient(database);
  const mismatches: TypedTableMismatch[] = [];
  for (const [recordType, table] of Object.entries(typedTables)) {
    const result = await client.execute({
      sql: `
        SELECT
          records.record_id,
          record_revisions.record_json,
          ${table}.data_json AS typed_json
        FROM records
        JOIN record_revisions
          ON record_revisions.record_id = records.record_id
         AND record_revisions.revision = records.current_revision
        JOIN ${table} ON ${table}.record_id = records.record_id
        WHERE records.record_type = ?
          AND ${table}.data_json <> record_revisions.record_json
        ORDER BY records.record_id
      `,
      args: [recordType],
    });
    for (const row of result.rows) {
      const recordId = String(row.record_id);
      const recordJson = String(row.record_json);
      const typedJson = String(row.typed_json);
      mismatches.push({
        record_id: recordId,
        record_type: recordType,
        encoding_only: semanticJsonEqual(recordJson, typedJson),
      });
    }
  }
  return mismatches;
}

async function listDenormalizedMismatchRecordIds(
  database: AthleticDatabase,
): Promise<string[]> {
  return listTypedDenormalizedMismatchRecordIds(databaseClient(database));
}

export async function countProviderEncodingRepairCandidates(
  database: AthleticDatabase,
): Promise<number> {
  const mismatches = await listTypedTableMismatches(database);
  const encodingJsonIds = new Set(
    mismatches.filter((row) => row.encoding_only).map((row) => row.record_id),
  );
  const denormalizedIds = await listDenormalizedMismatchRecordIds(database);
  const revisionAlignIds = new Set(
    mismatches.filter((row) => !row.encoding_only).map((row) => row.record_id),
  );
  const denormalizedOnly = denormalizedIds.filter((id) => !revisionAlignIds.has(id));
  return new Set([...encodingJsonIds, ...denormalizedOnly]).size;
}

export async function countRevisionAlignRepairCandidates(
  database: AthleticDatabase,
): Promise<number> {
  const mismatches = await listTypedTableMismatches(database);
  return mismatches.filter((row) => !row.encoding_only).length;
}

export async function listRevisionAlignRepairRecordIds(
  database: AthleticDatabase,
): Promise<string[]> {
  const mismatches = await listTypedTableMismatches(database);
  return mismatches.filter((row) => !row.encoding_only).map((row) => row.record_id);
}

async function rebuildTypedRows(
  transaction: Transaction,
  recordIds: readonly string[],
  options: { allowSemanticMismatch: boolean },
): Promise<number> {
  if (recordIds.length === 0) {
    return 0;
  }
  let repaired = 0;
  for (const recordId of recordIds) {
    const meta = await transaction.execute({
      sql: "SELECT record_type FROM records WHERE record_id = ?",
      args: [recordId],
    });
    const recordType = String(meta.rows[0]?.record_type);
    const table = typedTables[recordType];
    if (table === undefined) {
      throw new Error(`record ${recordId} has unsupported record type ${recordType}`);
    }
    const row = await transaction.execute({
      sql: `
        SELECT
          record_revisions.record_json,
          ${table}.data_json AS typed_json
        FROM records
        JOIN record_revisions
          ON record_revisions.record_id = records.record_id
         AND record_revisions.revision = records.current_revision
        JOIN ${table} ON ${table}.record_id = records.record_id
        WHERE records.record_id = ?
      `,
      args: [recordId],
    });
    const match = row.rows[0];
    if (match === undefined) {
      throw new Error(`record ${recordId} is missing typed-table data for repair`);
    }
    const recordJson = String(match.record_json);
    const typedJson = String(match.typed_json);
    if (
      recordJson !== typedJson &&
      !options.allowSemanticMismatch &&
      !semanticJsonEqual(recordJson, typedJson)
    ) {
      throw new Error(
        `record ${recordId} has a typed-table mismatch that is not key-order only; ` +
          `run ath repair --from-revision after reviewing affected records, or restore from backup`,
      );
    }
    await rebuildTypedRowFromCurrentRevision(transaction, recordId);
    repaired++;
  }
  return repaired;
}

export async function repairProviderEncodingRows(
  transaction: Transaction,
): Promise<number> {
  const allIds: string[] = [];
  for (const [recordType, table] of Object.entries(typedTables)) {
    const rows = await transaction.execute({
      sql: `
        SELECT records.record_id
        FROM records
        JOIN record_revisions
          ON record_revisions.record_id = records.record_id
         AND record_revisions.revision = records.current_revision
        JOIN ${table} ON ${table}.record_id = records.record_id
        WHERE records.record_type = ?
          AND ${table}.data_json <> record_revisions.record_json
      `,
      args: [recordType],
    });
    for (const row of rows.rows) {
      allIds.push(String(row.record_id));
    }
  }
  return rebuildTypedRows(transaction, allIds, { allowSemanticMismatch: false });
}

export async function repairTypedTablesFromRevisionRows(
  transaction: Transaction,
  recordIds: readonly string[],
): Promise<number> {
  return rebuildTypedRows(transaction, recordIds, { allowSemanticMismatch: true });
}

export interface ProviderEncodingRepairResult {
  repaired_record_count: number;
  database_revision: number;
}

export interface ConfirmedProviderRepairResult {
  backup_path: string;
  repaired_record_count: number;
  revision_align_record_count: number;
  database_revision: number;
}

export interface ExecuteConfirmedRepairOptions {
  backupPath?: string;
  now?: Date;
  fromRevision?: boolean;
}

async function readDatabaseRevision(database: AthleticDatabase): Promise<number> {
  return Number(
    (
      await databaseClient(database).execute(
        "SELECT current_revision FROM database_state WHERE singleton_id = 1",
      )
    ).rows[0]?.current_revision,
  );
}

export async function executeConfirmedProviderEncodingRepair(
  database: AthleticDatabase,
  options: ExecuteConfirmedRepairOptions = {},
): Promise<ConfirmedProviderRepairResult> {
  if (database.localPath === undefined) {
    throw new Error("Provider-encoding repair requires a local database file");
  }
  const metadata = await readDatabaseMetadata(database);
  const mismatches = await listTypedTableMismatches(database);
  const encodingCandidates = mismatches.filter((row) => row.encoding_only);
  const revisionCandidates = mismatches.filter((row) => !row.encoding_only);
  const denormalizedIds = await listDenormalizedMismatchRecordIds(database);
  const revisionAlignIdSet = new Set(revisionCandidates.map((row) => row.record_id));
  const denormalizedOnlyIds = denormalizedIds.filter((id) => !revisionAlignIdSet.has(id));
  const encodingRecordIds = [
    ...new Set([
      ...encodingCandidates.map((row) => row.record_id),
      ...denormalizedOnlyIds,
    ]),
  ];
  if (
    mismatches.length === 0 &&
    denormalizedOnlyIds.length === 0
  ) {
    const revision = await readDatabaseRevision(database);
    return {
      backup_path: "",
      repaired_record_count: 0,
      revision_align_record_count: 0,
      database_revision: revision,
    };
  }
  if (revisionCandidates.length > 0 && options.fromRevision !== true) {
    const ids = revisionCandidates.map((row) => row.record_id).join(", ");
    throw new Error(
      `${revisionCandidates.length} record(s) have typed-table bytes that disagree with the current revision in meaning (${ids}). ` +
        "Pass --from-revision and --confirm-revision-align with the database ID after reviewing the preview.",
    );
  }

  const revisionBefore = await readDatabaseRevision(database);
  const backupPath =
    options.backupPath ?? defaultRepairBackupPath(database.localPath, options.now);

  const fileIdentity = await lstat(database.localPath, { bigint: true }).then((stat) => ({
    device: stat.dev,
    inode: stat.ino,
  }));

  await createVerifiedSqliteBackup(database, database.localPath, backupPath, metadata);
  const revisionAfterBackup = await readDatabaseRevision(database);
  if (revisionAfterBackup !== revisionBefore) {
    throw new Error(
      "Database revision changed during repair backup; stop other writers and retry",
    );
  }
  const statAfterBackup = await lstat(database.localPath, { bigint: true });
  if (
    statAfterBackup.dev !== fileIdentity.device ||
    statAfterBackup.ino !== fileIdentity.inode
  ) {
    throw new Error("Database file changed during repair backup");
  }

  try {
    const result = await repairProviderEncodingAndRevisionAlign(database, {
      encodingRecordIds,
      revisionRecordIds: revisionCandidates.map((row) => row.record_id),
    });
    return { backup_path: backupPath, ...result };
  } catch (error) {
    const revisionAfterFailure = await readDatabaseRevision(database);
    const changed = revisionAfterFailure !== revisionBefore;
    const causeMessage = formatSchemaValidationError(error);
    throw new Error(
      `Repair failed after creating verified backup at ${backupPath}. Cause: ${causeMessage}.` +
        (changed
          ? " The database revision changed; restore from the backup."
          : ` Retry with a new backup; previous attempt left ${backupPath} on disk.`),
      { cause: error },
    );
  }
}

async function repairProviderEncodingAndRevisionAlign(
  database: AthleticDatabase,
  options: {
    encodingRecordIds: string[];
    revisionRecordIds: string[];
  },
): Promise<{
  repaired_record_count: number;
  revision_align_record_count: number;
  database_revision: number;
}> {
  return withWriteTransaction(database, async (transaction) => {
    const revisionAligned = await repairTypedTablesFromRevisionRows(
      transaction,
      options.revisionRecordIds,
    );
    const encodingRepaired = await rebuildTypedRows(transaction, options.encodingRecordIds, {
      allowSemanticMismatch: false,
    });
    const total = encodingRepaired + revisionAligned;
    const databaseRevision =
      total === 0
        ? Number(
            (
              await transaction.execute(
                "SELECT current_revision FROM database_state WHERE singleton_id = 1",
              )
            ).rows[0]?.current_revision,
          )
        : await incrementDatabaseRevision(transaction);
    return {
      repaired_record_count: encodingRepaired,
      revision_align_record_count: revisionAligned,
      database_revision: databaseRevision,
    };
  });
}

export async function repairProviderEncoding(
  database: AthleticDatabase,
): Promise<ProviderEncodingRepairResult> {
  return withWriteTransaction(database, async (transaction) => {
    const repaired = await repairProviderEncodingRows(transaction);
    const databaseRevision =
      repaired === 0
        ? Number(
            (
              await transaction.execute(
                "SELECT current_revision FROM database_state WHERE singleton_id = 1",
              )
            ).rows[0]?.current_revision,
          )
        : await incrementDatabaseRevision(transaction);
    return { repaired_record_count: repaired, database_revision: databaseRevision };
  });
}
