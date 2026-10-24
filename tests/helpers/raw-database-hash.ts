import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { unlink } from "node:fs/promises";
import { readFile } from "node:fs/promises";

export async function useDeleteJournalMode(databasePath: string): Promise<void> {
  const journal = spawnSync("sqlite3", [databasePath, "PRAGMA journal_mode = DELETE;"], {
    encoding: "utf8",
  });
  if (journal.status !== 0) {
    throw new Error(journal.stderr || "failed to switch test database to DELETE journal mode");
  }
  for (const suffix of ["-wal", "-shm"]) {
    try {
      await unlink(`${databasePath}${suffix}`);
    } catch (error) {
      if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) {
        throw error;
      }
    }
  }
}

export async function hashDatabaseFilesRaw(databasePath: string): Promise<string> {
  const hash = createHash("sha256");
  hash.update(await readFile(databasePath));
  for (const suffix of ["-wal", "-shm"]) {
    try {
      hash.update(await readFile(`${databasePath}${suffix}`));
    } catch (error) {
      if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) {
        throw error;
      }
    }
  }
  return hash.digest("hex");
}
