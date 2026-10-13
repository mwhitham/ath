import type { Transaction } from "@libsql/client";
import type { AthleticDatabase } from "./client.js";

export async function withWriteTransaction<T>(
  database: AthleticDatabase,
  operation: (transaction: Transaction) => Promise<T>,
): Promise<T> {
  const transaction = await database.client.transaction("write");
  try {
    await transaction.execute("PRAGMA defer_foreign_keys = ON");
    const result = await operation(transaction);
    await transaction.commit();
    return result;
  } catch (error) {
    if (!transaction.closed) {
      await transaction.rollback();
    }
    throw error;
  } finally {
    transaction.close();
  }
}
