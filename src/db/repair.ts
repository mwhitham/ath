import { lstat } from "node:fs/promises";
import type { Transaction } from "@libsql/client";
import { databaseClient, type AthleticDatabase } from "./client.js";
import { typedTables } from "./check-tables.js";
import {
  createVerifiedSqliteBackup,
  defaultRepairBackupPath,
} from "./backup.js";
import { readDatabaseMetadata } from "./migrate.js";
import { incrementDatabaseRevision } from "./records.js";
import { semanticJsonEqual } from "./stable-json.js";
import { withWriteTransaction } from "./transaction.js";

export interface ProviderEncodingRepairResult {
  repaired_record_count: number;
  database_revision: number;
}

export async function countProviderEncodingRepairCandidates(
  database: AthleticDatabase,
): Promise<number> {
  const client = databaseClient(database);
  let total = 0;
  for (const [recordType, table] of Object.entries(typedTables)) {
    const result = await client.execute({
      sql: `
        SELECT COUNT(*) AS count
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
    total += Number(result.rows[0]?.count ?? 0);
  }
  return total;
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

export interface ConfirmedProviderRepairResult {
  backup_path: string;
  repaired_record_count: number;
  database_revision: number;
}

export async function executeConfirmedProviderEncodingRepair(
  database: AthleticDatabase,
  options: { backupPath?: string; now?: Date } = {},
): Promise<ConfirmedProviderRepairResult> {
  if (database.localPath === undefined) {
    throw new Error("Provider-encoding repair requires a local database file");
  }
  const metadata = await readDatabaseMetadata(database);
  const candidates = await countProviderEncodingRepairCandidates(database);
  if (candidates === 0) {
    const revision = Number(
      (
        await databaseClient(database).execute(
          "SELECT current_revision FROM database_state WHERE singleton_id = 1",
        )
      ).rows[0]?.current_revision,
    );
    return { backup_path: "", repaired_record_count: 0, database_revision: revision };
  }

  const backupPath =
    options.backupPath ?? defaultRepairBackupPath(database.localPath, options.now);
  const fileIdentity = await lstat(database.localPath, { bigint: true }).then((stat) => ({
    device: stat.dev,
    inode: stat.ino,
  }));
  const revisionBefore = Number(
    (
      await databaseClient(database).execute(
        "SELECT current_revision FROM database_state WHERE singleton_id = 1",
      )
    ).rows[0]?.current_revision,
  );

  await createVerifiedSqliteBackup(database, database.localPath, backupPath, metadata);

  const revisionAfterBackup = Number(
    (
      await databaseClient(database).execute(
        "SELECT current_revision FROM database_state WHERE singleton_id = 1",
      )
    ).rows[0]?.current_revision,
  );
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
    const result = await repairProviderEncoding(database);
    return { backup_path: backupPath, ...result };
  } catch (error) {
    const revisionAfterFailure = Number(
      (
        await databaseClient(database).execute(
          "SELECT current_revision FROM database_state WHERE singleton_id = 1",
        )
      ).rows[0]?.current_revision,
    );
    const changed = revisionAfterFailure !== revisionBefore;
    throw new Error(
      `Repair failed after creating verified backup at ${backupPath}.` +
        (changed
          ? " The database revision changed; restore from the backup."
          : " The database revision did not change."),
      { cause: error },
    );
  }
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
