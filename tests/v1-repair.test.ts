import { copyFile, cp, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createClient } from "@libsql/client";
import { afterEach, describe, expect, it } from "vitest";
import {
  checkDatabase,
  createDatabase,
  executeConfirmedDatabaseMigration,
  importProviderRecords,
  openDatabase,
  previewProviderImport,
  repairProviderEncoding,
  type AthleticDatabase,
} from "../src/db/index.js";
import { databaseClient } from "../src/db/client.js";

const FIXTURE = join(dirname(fileURLToPath(import.meta.url)), "../examples/demo-athlete/athlete.ath.sqlite");
const providerFixtures = join(dirname(fileURLToPath(import.meta.url)), "fixtures/exports");

const openDatabases: AthleticDatabase[] = [];
const directories: string[] = [];

function jsonDeepEqual(left: string, right: string): boolean {
  return JSON.stringify(JSON.parse(left)) === JSON.stringify(JSON.parse(right));
}

function whitespaceMismatch(canonical: string): string {
  const spaced = canonical.replace(",", ", ");
  if (spaced === canonical) {
    return `${canonical.slice(0, -1)} `;
  }
  return spaced;
}

afterEach(async () => {
  for (const database of openDatabases.splice(0)) {
    database.close();
  }
  for (const directory of directories.splice(0)) {
    await rm(directory, { recursive: true, force: true });
  }
});

async function tempDatabasePath(): Promise<string> {
  const directory = await mkdtemp(join(tmpdir(), "ath-v1-repair-"));
  directories.push(directory);
  return join(directory, "athlete.ath.sqlite");
}

describe("v1 provider-encoding repair", () => {
  it("repairs typed-table JSON bytes after a simulated npm 1.0.0 provider import", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, {
      createdAt: "2026-10-13T11:00:00.000Z",
    });
    openDatabases.push(database);
    const preview = await previewProviderImport(database, join(providerFixtures, "whoop"), {
      received_at: "2026-10-13T12:00:00.000Z",
    });
    await importProviderRecords(database, preview);

    const mismatch = await databaseClient(database).execute(`
      SELECT records.record_id, point_measurements.data_json, record_revisions.record_json
      FROM records
      JOIN record_revisions
        ON record_revisions.record_id = records.record_id
       AND record_revisions.revision = records.current_revision
      JOIN point_measurements ON point_measurements.record_id = records.record_id
      WHERE records.record_type = 'point_measurement'
      LIMIT 1
    `);
    const row = mismatch.rows[0];
    expect(row).toBeDefined();
    const recordJson = String(row!.record_json);
    const shuffled = whitespaceMismatch(recordJson);
    expect(jsonDeepEqual(shuffled, recordJson)).toBe(true);
    expect(shuffled).not.toBe(recordJson);
    const recordId = String(row!.record_id);
    await databaseClient(database).execute({
      sql: "UPDATE point_measurements SET data_json = ? WHERE record_id = ?",
      args: [shuffled, recordId],
    });

    const broken = await checkDatabase(database);
    expect(broken.valid).toBe(false);

    const result = await repairProviderEncoding(database);
    expect(result.repaired_record_count).toBeGreaterThan(0);

    const fixed = await checkDatabase(database);
    expect(fixed.valid, JSON.stringify(fixed.issues, null, 2)).toBe(true);
  });

  it("migrates SQL schema 1 to 2 on a database copy after a verified backup", async () => {
    const path = await tempDatabasePath();
    await copyFile(FIXTURE, path);
    await cp(join(dirname(FIXTURE), "attachments"), join(dirname(path), "attachments"), {
      recursive: true,
    });
    const client = createClient({ url: pathToFileURL(path).href, intMode: "number" });
    await client.execute("UPDATE database_metadata SET sql_schema_version = 1");
    await client.execute("PRAGMA user_version = 1");
    await client.close();

    const database = await openDatabase(path, { allowOutdatedSchema: true, deferWal: true });
    openDatabases.push(database);
    const migration = await executeConfirmedDatabaseMigration(database);
    expect(migration.migrated).toBe(true);
    expect(migration.preview.target_sql_schema_version).toBe(2);
    const check = await checkDatabase(database);
    expect(check.valid, JSON.stringify(check.issues.slice(0, 5), null, 2)).toBe(true);
  });
});
