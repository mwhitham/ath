import { unlink } from "node:fs/promises";
import type { Transaction } from "@libsql/client";
import { databaseClient, enableLocalWal, type AthleticDatabase } from "./client.js";

async function removeWalSidecarsIfPresent(localPath: string): Promise<void> {
  for (const suffix of ["-wal", "-shm"]) {
    try {
      await unlink(`${localPath}${suffix}`);
    } catch (error) {
      if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) {
        throw error;
      }
    }
  }
}

async function revertFailedWriteJournalMode(database: AthleticDatabase): Promise<void> {
  if (database.localPath === undefined) {
    return;
  }
  try {
    await databaseClient(database).execute("PRAGMA journal_mode = DELETE");
  } catch {
    // The database may already be closed or read-only.
  }
  await removeWalSidecarsIfPresent(database.localPath);
}

export async function withWriteTransaction<T>(
  database: AthleticDatabase,
  operation: (transaction: Transaction) => Promise<T>,
): Promise<T> {
  const journalBefore = String(
    (await databaseClient(database).execute("PRAGMA journal_mode")).rows[0]?.journal_mode,
  ).toLowerCase();
  let walEnabledForAttempt = false;
  try {
    await enableLocalWal(database);
    walEnabledForAttempt = true;
  } catch (error) {
    throw error;
  }
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
    if (walEnabledForAttempt) {
      if (journalBefore !== "wal") {
        await revertFailedWriteJournalMode(database);
      } else if (database.localPath !== undefined) {
        await removeWalSidecarsIfPresent(database.localPath);
      }
    }
    throw error;
  } finally {
    transaction.close();
  }
}
