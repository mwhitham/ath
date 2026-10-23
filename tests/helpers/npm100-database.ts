import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createClient } from "@libsql/client";
import { pathToFileURL } from "node:url";
import { countProviderEncodingRepairCandidates, openDatabase } from "../../src/db/index.js";

const fixtures = resolve(import.meta.dirname, "../fixtures/exports");
const NPM_CLI = ["--yes", "athleticstandard@1.0.0"];

function runAth100(args: string[], cwd: string): void {
  execFileSync("npx", [...NPM_CLI, ...args], {
    cwd,
    encoding: "utf8",
    stdio: "pipe",
  });
}

export function createNpm100ProviderDatabase(): {
  directory: string;
  databasePath: string;
} {
  const directory = mkdtempSync(join(tmpdir(), "ath-npm-100-"));
  runAth100(["init", "-y", "--no-skill"], directory);
  for (const provider of ["whoop", "oura", "apple"] as const) {
    runAth100(["import", "-y", resolve(fixtures, provider)], directory);
  }
  return { directory, databasePath: join(directory, "athlete.ath.sqlite") };
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

export function copyDatabaseTree(sourceDirectory: string, targetDirectory: string): string {
  const sourcePath = join(sourceDirectory, "athlete.ath.sqlite");
  const targetPath = join(targetDirectory, "athlete.ath.sqlite");
  copyFileSync(sourcePath, targetPath);
  return targetPath;
}

export function readDatabaseBytes(path: string): Buffer {
  return readFileSync(path);
}
