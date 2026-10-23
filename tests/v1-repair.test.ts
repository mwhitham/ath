import { copyFile, cp, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { execFileSync } from "node:child_process";
import { createClient } from "@libsql/client";
import { afterEach, describe, expect, it } from "vitest";
import {
  checkDatabase,
  executeConfirmedDatabaseMigration,
  executeConfirmedProviderEncodingRepair,
  openDatabase,
  reconcileSqliteUserVersion,
  readDatabaseMetadata,
  repairProviderEncoding,
  type AthleticDatabase,
} from "../src/db/index.js";
import { databaseClient } from "../src/db/client.js";
import { semanticJsonEqual } from "../src/db/stable-json.js";
import {
  countEncodingMismatches,
  materializeNpm100ProviderDatabase,
  snapshotRecordPayloadBytes,
} from "./helpers/npm100-database.js";

const here = dirname(fileURLToPath(import.meta.url));
const CLI = resolve(here, "../src/cli.ts");
const TSX = resolve(here, "../node_modules/.bin/tsx");
const providerFixtures = join(here, "fixtures/exports");

const openDatabases: AthleticDatabase[] = [];
const directories: string[] = [];

function ath(args: string[], cwd: string): { stdout: string; code: number } {
  try {
    const stdout = execFileSync(TSX, [CLI, ...args], { cwd, encoding: "utf8", stdio: "pipe" });
    return { stdout, code: 0 };
  } catch (error) {
    const err = error as { status: number; stdout?: string; stderr?: string };
    return { stdout: (err.stdout ?? "") + (err.stderr ?? ""), code: err.status };
  }
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

function reorderTopLevelKeys(json: string): string {
  const parsed = JSON.parse(json) as Record<string, unknown>;
  const reversed = Object.fromEntries(Object.entries(parsed).reverse());
  return JSON.stringify(reversed);
}

describe("v1 provider-encoding repair", () => {
  it("treats key-order-only typed-table differences as repairable", () => {
    const canonical = '{"type":"hrv_rmssd","value":68.4,"unit":"ms","recorded_at":"2026-08-09T13:05:00.000Z","source":"01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e02"}';
    const shuffled = reorderTopLevelKeys(canonical);
    expect(shuffled).not.toBe(canonical);
    expect(semanticJsonEqual(shuffled, canonical)).toBe(true);
  });

  it("repairs typed-table JSON bytes after npm 1.0.0 provider imports", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    const mismatchesBefore = await countEncodingMismatches(databasePath);
    expect(mismatchesBefore).toBeGreaterThan(0);

    const database = await openDatabase(databasePath, { allowOutdatedSchema: true });
    openDatabases.push(database);
    const beforeBytes = await snapshotRecordPayloadBytes(databasePath);

    const result = await executeConfirmedProviderEncodingRepair(database);
    expect(result.repaired_record_count).toBe(mismatchesBefore);

    expect(await countEncodingMismatches(databasePath)).toBe(0);
    const metadata = await databaseClient(database).execute(
      "SELECT sql_schema_version FROM database_metadata WHERE singleton_id = 1",
    );
    expect(Number(metadata.rows[0]?.sql_schema_version)).toBe(1);

    const afterBytes = await snapshotRecordPayloadBytes(databasePath);
    expect(afterBytes).toEqual(beforeBytes);
  }, 180_000);

  it("migrates SQL schema 1 to 2 on an npm 1.0.0 provider database", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    const beforeBytes = await snapshotRecordPayloadBytes(databasePath);
    expect(await countEncodingMismatches(databasePath)).toBeGreaterThan(0);

    const database = await openDatabase(databasePath, { allowOutdatedSchema: true, deferWal: true });
    openDatabases.push(database);
    const migration = await executeConfirmedDatabaseMigration(database);
    expect(migration.migrated).toBe(true);
    expect(migration.preview.target_sql_schema_version).toBe(2);

    const check = await checkDatabase(database);
    expect(check.valid, JSON.stringify(check.issues.slice(0, 5), null, 2)).toBe(true);
    expect(await countEncodingMismatches(databasePath)).toBe(0);

    const afterBytes = await snapshotRecordPayloadBytes(databasePath);
    expect(afterBytes).toEqual(beforeBytes);
  }, 180_000);

  it("runs ath migrate then ath check without changing record bytes", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    const beforeBytes = await snapshotRecordPayloadBytes(databasePath);
    const client = createClient({ url: pathToFileURL(databasePath).href });
    const idRow = await client.execute(
      "SELECT database_id FROM database_metadata WHERE singleton_id = 1",
    );
    const databaseId = String(idRow.rows[0]?.database_id);
    await client.close();

    const migrate = ath(["migrate", "-y", "--confirm-migration", databaseId], directory);
    expect(migrate.code, migrate.stdout).toBe(0);

    const check = ath(["check"], directory);
    expect(check.code).toBe(0);
    expect(check.stdout).toContain("valid Athletic Standard 1.0.0 database");

    const afterBytes = await snapshotRecordPayloadBytes(databasePath);
    expect(afterBytes).toEqual(beforeBytes);
  }, 180_000);
});

describe("v1 provider-encoding repair (local regression)", () => {
  it("refuses migration when typed-table bytes diverge in meaning", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    const database = await openDatabase(databasePath, { allowOutdatedSchema: true });
    openDatabases.push(database);
    const row = await databaseClient(database).execute(`
      SELECT records.record_id, record_revisions.record_json
      FROM records
      JOIN record_revisions
        ON record_revisions.record_id = records.record_id
       AND record_revisions.revision = records.current_revision
      JOIN point_measurements ON point_measurements.record_id = records.record_id
      WHERE records.record_type = 'point_measurement'
      LIMIT 1
    `);
    const recordId = String(row.rows[0]!.record_id);
    const divergent = String(row.rows[0]!.record_json).replace("68.4", "999");
    await databaseClient(database).execute({
      sql: "UPDATE point_measurements SET data_json = ? WHERE record_id = ?",
      args: [divergent, recordId],
    });
    database.close();
    const reopened = await openDatabase(databasePath, { allowOutdatedSchema: true, deferWal: true });
    openDatabases.push(reopened);
    await expect(executeConfirmedDatabaseMigration(reopened)).rejects.toThrow(
      /not key-order only/,
    );
  });

  it("reconciles user_version when metadata advanced one step ahead", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    const client = createClient({ url: pathToFileURL(databasePath).href, intMode: "number" });
    await client.execute("UPDATE database_metadata SET sql_schema_version = 2");
    await client.close();
    const database = await openDatabase(databasePath, { allowOutdatedSchema: true });
    openDatabases.push(database);
    const metadata = await readDatabaseMetadata(database);
    await reconcileSqliteUserVersion(database, metadata);
    const userVersion = await databaseClient(database).execute("PRAGMA user_version");
    expect(Number(userVersion.rows[0]?.user_version)).toBe(2);
  });

  it("repairs a single injected key-order mismatch", async () => {
    const path = await tempDatabasePath();
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    await copyFile(databasePath, path);

    const database = await openDatabase(path, { allowOutdatedSchema: true });
    openDatabases.push(database);
    const mismatch = await databaseClient(database).execute(`
      SELECT records.record_id, record_revisions.record_json
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
    const shuffled = reorderTopLevelKeys(recordJson);
    await databaseClient(database).execute({
      sql: "UPDATE point_measurements SET data_json = ? WHERE record_id = ?",
      args: [shuffled, String(row!.record_id)],
    });

    const broken = await checkDatabase(database);
    expect(broken.valid).toBe(false);

    const result = await repairProviderEncoding(database);
    expect(result.repaired_record_count).toBeGreaterThan(0);
    expect(await countEncodingMismatches(path)).toBe(0);
  }, 180_000);
});
