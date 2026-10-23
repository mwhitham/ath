import type { Transaction } from "@libsql/client";
import { databaseClient, type AthleticDatabase } from "./client.js";
import { typedTables } from "./check-tables.js";
import { incrementDatabaseRevision } from "./records.js";
import { withWriteTransaction } from "./transaction.js";

export interface ProviderEncodingRepairResult {
  repaired_record_count: number;
  database_revision: number;
}

function jsonDeepEqual(left: string, right: string): boolean {
  try {
    return JSON.stringify(JSON.parse(left)) === JSON.stringify(JSON.parse(right));
  } catch {
    return false;
  }
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
      if (!jsonDeepEqual(recordJson, typedJson)) {
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
