import { rmSync } from "node:fs";

/**
 * Remove a test folder that held a SQLite file. The libsql driver lets go of
 * the file only once its statements are garbage-collected, and Windows will
 * not delete a file that is still open. Vitest runs with --expose-gc.
 */
export async function removeTemporaryDirectory(path: string): Promise<void> {
  (globalThis as { gc?: () => void }).gc?.();
  await new Promise((resolve) => setImmediate(resolve));
  rmSync(path, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
}
