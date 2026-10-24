import type { Transaction } from "@libsql/client";
import {
  databaseClient,
  enableLocalWal,
  tryRevertDeleteJournalModeAfterFailedWal,
  type AthleticDatabase,
} from "./client.js";

export async function withWriteTransaction<T>(
  database: AthleticDatabase,
  operation: (transaction: Transaction) => Promise<T>,
): Promise<T> {
  const journalBefore = String(
    (await databaseClient(database).execute("PRAGMA journal_mode")).rows[0]?.journal_mode,
  ).toLowerCase();
  await enableLocalWal(database);
  const transaction = await databaseClient(database).transaction("write");
  try {
    const foreignKeys = await transaction.execute("PRAGMA foreign_keys");
    if (Number(foreignKeys.rows[0]?.foreign_keys) !== 1) {
      throw new Error("SQLite foreign-key enforcement is disabled in write transaction");
    }
    await transaction.execute("PRAGMA defer_foreign_keys = ON");
    const result = await operation(transaction);
    await transaction.commit();
    return result;
  } catch (error) {
    if (!transaction.closed) {
      await transaction.rollback();
    }
    try {
      await tryRevertDeleteJournalModeAfterFailedWal(database, journalBefore);
    } catch {
      // Best-effort journal revert; the original write failure is thrown below.
    }
    throw error;
  } finally {
    transaction.close();
  }
}
