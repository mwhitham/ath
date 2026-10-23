import { spawnSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync, unlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createClient } from "@libsql/client";
import { pathToFileURL } from "node:url";
import { countProviderEncodingRepairCandidates, openDatabase } from "../../src/db/index.js";

const fixtureDatabase = resolve(
  import.meta.dirname,
  "../fixtures/npm-100-encoding/athlete.ath.sqlite",
);

export function materializeNpm100ProviderDatabase(): {
  directory: string;
  databasePath: string;
} {
  const directory = mkdtempSync(join(tmpdir(), "ath-npm-100-"));
  const databasePath = join(directory, "athlete.ath.sqlite");
  copyFileSync(fixtureDatabase, databasePath);
  const journal = spawnSync("sqlite3", [databasePath, "PRAGMA journal_mode = DELETE;"], {
    encoding: "utf8",
  });
  if (journal.status !== 0) {
    throw new Error(journal.stderr || "failed to set DELETE journal mode on npm 1.0.0 test copy");
  }
  for (const suffix of ["-wal", "-shm"]) {
    try {
      unlinkSync(`${databasePath}${suffix}`);
    } catch {
      // no sidecar
    }
  }
  return { directory, databasePath };
}

export async function snapshotRecordPayloadBytes(
  databasePath: string,
): Promise<Map<string, string>> {
  const client = createClient({ url: pathToFileURL(databasePath).href });
  const result = await client.execute(`
    SELECT records.record_id, record_revisions.record_json
    FROM records
    JOIN record_revisions
      ON record_revisions.record_id = records.record_id
     AND record_revisions.revision = records.current_revision
    ORDER BY records.record_id
  `);
  await client.close();
  const map = new Map<string, string>();
  for (const row of result.rows) {
    map.set(String(row.record_id), String(row.record_json));
  }
  return map;
}

export async function countEncodingMismatches(databasePath: string): Promise<number> {
  const database = await openDatabase(databasePath, {
    allowOutdatedSchema: true,
    deferWal: true,
  });
  try {
    return await countProviderEncodingRepairCandidates(database);
  } finally {
    database.close();
  }
}

export function readDatabaseBytes(path: string): Buffer {
  return readFileSync(path);
}
