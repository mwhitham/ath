import { lstat, readdir } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import type { Transaction } from "@libsql/client";
import { connectDatabase, databaseClient, type AthleticDatabase } from "./client.js";
import { typedTables } from "./check-tables.js";
import { createVerifiedSqliteBackup, defaultRepairBackupPath } from "./backup.js";
import { readDatabaseMetadata, type DatabaseMetadata } from "./migrate.js";
import { incrementDatabaseRevision } from "./records.js";
import { semanticJsonEqual } from "./stable-json.js";
import { withWriteTransaction } from "./transaction.js";

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

export async function countProviderEncodingRepairCandidates(
  database: AthleticDatabase,
): Promise<number> {
  const mismatches = await listTypedTableMismatches(database);
  return mismatches.filter((row) => row.encoding_only).length;
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

export async function repairProviderEncodingRows(
  transaction: Transaction,
): Promise<number> {
  let repaired = 0;
  for (const [recordType, table] of Object.entries(typedTables)) {
    const mismatches = await transaction.execute({
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
    for (const row of mismatches.rows) {
      const recordId = String(row.record_id);
      const recordJson = String(row.record_json);
      const typedJson = String(row.typed_json);
      if (!semanticJsonEqual(recordJson, typedJson)) {
        throw new Error(
          `record ${recordId} has a typed-table mismatch that is not key-order only; ` +
            `run ath repair --from-revision after reviewing affected records, or restore from backup`,
        );
      }
      await transaction.execute({
        sql: `UPDATE ${table} SET data_json = ? WHERE record_id = ?`,
        args: [recordJson, recordId],
      });
      repaired++;
    }
  }
  return repaired;
}

export async function repairTypedTablesFromRevisionRows(
  transaction: Transaction,
  recordIds: readonly string[],
): Promise<number> {
  if (recordIds.length === 0) {
    return 0;
  }
  const idSet = new Set(recordIds);
  let repaired = 0;
  for (const [recordType, table] of Object.entries(typedTables)) {
    const mismatches = await transaction.execute({
      sql: `
        SELECT
          records.record_id,
          record_revisions.record_json
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
    for (const row of mismatches.rows) {
      const recordId = String(row.record_id);
      if (!idSet.has(recordId)) {
        continue;
      }
      const recordJson = String(row.record_json);
      await transaction.execute({
        sql: `UPDATE ${table} SET data_json = ? WHERE record_id = ?`,
        args: [recordJson, recordId],
      });
      repaired++;
    }
  }
  return repaired;
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

async function findReusableRepairBackupPath(
  database: AthleticDatabase,
  metadata: DatabaseMetadata,
  revisionBefore: number,
): Promise<string | undefined> {
  if (database.localPath === undefined) {
    return undefined;
  }
  const directory = dirname(database.localPath);
  const prefix = `${basename(database.localPath)}.repair-backup.`;
  let entries: string[];
  try {
    entries = await readdir(directory);
  } catch {
    return undefined;
  }
  const candidates = entries
    .filter((name) => name.startsWith(prefix) && name.endsWith(".sqlite"))
    .map((name) => join(directory, name))
    .sort()
    .reverse();
  for (const candidate of candidates) {
    const backupDatabase = await connectDatabase(candidate);
    try {
      const backupMetadata = await readDatabaseMetadata(backupDatabase);
      if (
        backupMetadata.database_id !== metadata.database_id ||
        backupMetadata.sql_schema_version !== metadata.sql_schema_version
      ) {
        continue;
      }
      const backupRevision = await readDatabaseRevision(backupDatabase);
      if (backupRevision === revisionBefore) {
        return candidate;
      }
    } catch {
      // Try the next backup candidate.
    } finally {
      backupDatabase.close();
    }
  }
  return undefined;
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
  if (mismatches.length === 0) {
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

  let backupPath = options.backupPath;
  let reuseBackup = false;
  if (backupPath === undefined) {
    const reusable = await findReusableRepairBackupPath(database, metadata, revisionBefore);
    if (reusable !== undefined) {
      backupPath = reusable;
      reuseBackup = true;
    } else {
      backupPath = defaultRepairBackupPath(database.localPath, options.now);
    }
  }

  const fileIdentity = await lstat(database.localPath, { bigint: true }).then((stat) => ({
    device: stat.dev,
    inode: stat.ino,
  }));

  if (!reuseBackup) {
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
  }

  try {
    const result = await repairProviderEncodingAndRevisionAlign(database, {
      encodingRecordIds: encodingCandidates.map((row) => row.record_id),
      revisionRecordIds: revisionCandidates.map((row) => row.record_id),
    });
    return { backup_path: backupPath, ...result };
  } catch (error) {
    const revisionAfterFailure = await readDatabaseRevision(database);
    const changed = revisionAfterFailure !== revisionBefore;
    const causeMessage =
      error instanceof Error ? error.message : "repair step failed";
    throw new Error(
      `Repair failed${reuseBackup ? "" : ` after creating verified backup at ${backupPath}`}. Cause: ${causeMessage}.` +
        (changed
          ? " The database revision changed; restore from the backup."
          : ` Retry with the same backup at ${backupPath}.`),
      { cause: error },
    );
  }
}

async function repairEncodingOnlyRows(
  transaction: Transaction,
  recordIds: readonly string[],
): Promise<number> {
  if (recordIds.length === 0) {
    return 0;
  }
  const idSet = new Set(recordIds);
  let repaired = 0;
  for (const [recordType, table] of Object.entries(typedTables)) {
    const mismatches = await transaction.execute({
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
    for (const row of mismatches.rows) {
      const recordId = String(row.record_id);
      if (!idSet.has(recordId)) {
        continue;
      }
      const recordJson = String(row.record_json);
      const typedJson = String(row.typed_json);
      if (!semanticJsonEqual(recordJson, typedJson)) {
        throw new Error(
          `record ${recordId} has a typed-table mismatch that is not key-order only`,
        );
      }
      await transaction.execute({
        sql: `UPDATE ${table} SET data_json = ? WHERE record_id = ?`,
        args: [recordJson, recordId],
      });
      repaired++;
    }
  }
  return repaired;
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
    const encodingRepaired = await repairEncodingOnlyRows(
      transaction,
      options.encodingRecordIds,
    );
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
