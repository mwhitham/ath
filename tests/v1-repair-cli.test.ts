import { execFileSync } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createClient } from "@libsql/client";
import { afterEach, describe, expect, it } from "vitest";
import { hashDatabaseFilesRaw } from "./helpers/raw-database-hash.js";
import { openDatabase, type AthleticDatabase } from "../src/db/index.js";
import { databaseClient } from "../src/db/client.js";
import { materializeNpm100ProviderDatabase } from "./helpers/npm100-database.js";

const here = dirname(fileURLToPath(import.meta.url));
const CLI = resolve(here, "../src/cli.ts");
const TSX = resolve(here, "../node_modules/.bin/tsx");

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

async function injectPointMeasurementDivergence(databasePath: string): Promise<string> {
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
      AND point_measurements.quantity = 'hrv_rmssd'
    LIMIT 1
  `);
  const recordId = String(row.rows[0]!.record_id);
  const divergent = String(row.rows[0]!.record_json).replace("68.4", "999");
  await databaseClient(database).execute({
    sql: "UPDATE point_measurements SET data_json = ?, standard_value = 999 WHERE record_id = ?",
    args: [divergent, recordId],
  });
  database.close();
  openDatabases.pop();
  const idClient = createClient({ url: pathToFileURL(databasePath).href });
  const idRow = await idClient.execute(
    "SELECT database_id FROM database_metadata WHERE singleton_id = 1",
  );
  const databaseId = String(idRow.rows[0]?.database_id);
  await idClient.close();
  return databaseId;
}

describe("v1 repair CLI", () => {
  it("ath migrate on a divergent npm 1.0.0 database reports mismatch, backup path, and repair hint", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    const databaseId = await injectPointMeasurementDivergence(databasePath);

    const migrate = ath(["migrate", "-y", "--confirm-migration", databaseId], directory);
    expect(migrate.code, migrate.stdout).not.toBe(0);
    expect(migrate.stdout).toMatch(/not key-order only/i);
    expect(migrate.stdout).toMatch(/migration-backup/i);
    expect(migrate.stdout).toMatch(/from-revision/i);
    expect(migrate.stdout).not.toMatch(/SQLITE_BUSY/i);
  }, 180_000);

  it("ath check reports denormalized typed columns when data_json matches the revision", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    const database = await openDatabase(databasePath, { allowOutdatedSchema: true });
    openDatabases.push(database);
    const row = await databaseClient(database).execute(`
      SELECT records.record_id
      FROM records
      JOIN point_measurements ON point_measurements.record_id = records.record_id
      WHERE records.record_type = 'point_measurement'
        AND point_measurements.quantity = 'hrv_rmssd'
      LIMIT 1
    `);
    const recordId = String(row.rows[0]!.record_id);
    await databaseClient(database).execute({
      sql: "UPDATE point_measurements SET standard_value = 999 WHERE record_id = ?",
      args: [recordId],
    });
    database.close();
    openDatabases.pop();

    const check = ath(["check"], directory);
    expect(check.code, check.stdout).not.toBe(0);
    expect(check.stdout).toMatch(/denormalized/i);
  });

  it("ath repair fixes column-only typed_denormalized corruption without --from-revision", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    const database = await openDatabase(databasePath, { allowOutdatedSchema: true });
    openDatabases.push(database);
    const row = await databaseClient(database).execute(`
      SELECT records.record_id
      FROM records
      JOIN point_measurements ON point_measurements.record_id = records.record_id
      WHERE records.record_type = 'point_measurement'
        AND point_measurements.quantity = 'hrv_rmssd'
      LIMIT 1
    `);
    const recordId = String(row.rows[0]!.record_id);
    await databaseClient(database).execute({
      sql: "UPDATE point_measurements SET standard_value = 999 WHERE record_id = ?",
      args: [recordId],
    });
    database.close();
    openDatabases.pop();

    const preview = ath(["repair"], directory);
    expect(preview.stdout).not.toMatch(/no provider-encoding repair is required/);
    expect(preview.stdout).toMatch(/"repair_candidates": [1-9]/);

    const repair = ath(["repair", "-y"], directory);
    expect(repair.code, repair.stdout).toBe(0);

    const idClient = createClient({ url: pathToFileURL(databasePath).href });
    const idRow = await idClient.execute(
      "SELECT database_id FROM database_metadata WHERE singleton_id = 1",
    );
    const databaseId = String(idRow.rows[0]?.database_id);
    await idClient.close();

    const migrate = ath(["migrate", "-y", "--confirm-migration", databaseId], directory);
    expect(migrate.code, migrate.stdout).toBe(0);

    const check = ath(["check"], directory);
    expect(check.code, check.stdout).toBe(0);

    const baseline = ath(["calculate", "baseline", "hrv_rmssd", "--json"], directory);
    expect(baseline.stdout).not.toContain('"maximum": 999');
  }, 120_000);

  it("ath repair --from-revision restores calculate baseline values", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    const databaseId = await injectPointMeasurementDivergence(databasePath);

    const before = ath(["calculate", "baseline", "hrv_rmssd", "--json"], directory);
    expect(before.code, before.stdout).toBe(0);
    expect(before.stdout).toContain('"maximum": 999');

    const repair = ath(
      ["repair", "-y", "--from-revision", "--confirm-revision-align", databaseId],
      directory,
    );
    expect(repair.code, repair.stdout).toBe(0);

    const after = ath(["calculate", "baseline", "hrv_rmssd", "--json"], directory);
    expect(after.code, after.stdout).toBe(0);
    expect(after.stdout).not.toContain('"maximum": 999');
    expect(after.stdout).toContain("68.4");

    const migrate = ath(["migrate", "-y", "--confirm-migration", databaseId], directory);
    expect(migrate.code, migrate.stdout).toBe(0);

    const check = ath(["check"], directory);
    expect(check.code, check.stdout).toBe(0);
  }, 180_000);

  it.runIf(process.platform !== "win32")(
    "ath conflicts show does not enable WAL on a delete-mode database",
    async () => {
      const directory = await mkdtemp(join(tmpdir(), "ath-conflicts-hash-"));
      directories.push(directory);
      const demoFixture = resolve(here, "../examples/demo-athlete/athlete.ath.sqlite");
      const profileId = "01890f3e-e7d3-7cc0-88c8-000000000002";
      const basePackage = join(directory, "base-package");
      expect(ath(["export", basePackage, "--file", demoFixture], directory).code).toBe(0);
      const local = join(directory, "local.ath.sqlite");
      const imported = join(directory, "imported.ath.sqlite");
      const { copyFile } = await import("node:fs/promises");
      await copyFile(demoFixture, local);
      await copyFile(demoFixture, imported);
      expect(ath(["import", basePackage, "--file", local, "--yes"], directory).code).toBe(0);
      expect(ath(["import", basePackage, "--file", imported, "--yes"], directory).code).toBe(0);
      const profile = JSON.parse(
        ath(["records", "show", profileId, "--file", local, "--json"], directory).stdout,
      ).record as { data: Record<string, unknown> };
      for (const [db, name] of [
        [local, "Local name"],
        [imported, "Imported name"],
      ] as const) {
        expect(
          ath(
            [
              "correct",
              profileId,
              "--revision",
              "1",
              "--data",
              JSON.stringify({ ...profile.data, name }),
              "--file",
              db,
              "--yes",
            ],
            directory,
          ).code,
        ).toBe(0);
      }
      const incomingPackage = join(directory, "incoming-package");
      expect(ath(["export", incomingPackage, "--file", imported], directory).code).toBe(0);
      expect(ath(["import", incomingPackage, "--file", local, "--json"], directory).code).toBe(0);
      const client = createClient({ url: pathToFileURL(local).href });
      await client.execute("PRAGMA journal_mode = DELETE");
      await client.close();
      const hashBefore = await hashDatabaseFilesRaw(local);
      ath(["conflicts", "show", profileId, "--file", local, "--json"], directory);
      expect(await hashDatabaseFilesRaw(local)).toBe(hashBefore);
    },
  );

  it.runIf(process.platform !== "win32")(
    "ath repair and repair --dry-run do not enable WAL on a delete-mode database",
    async () => {
      const { directory, databasePath } = materializeNpm100ProviderDatabase();
      directories.push(directory);
      const client = createClient({ url: pathToFileURL(databasePath).href });
      await client.execute("PRAGMA journal_mode = DELETE");
      await client.close();
      const hashBefore = await hashDatabaseFilesRaw(databasePath);
      ath(["repair", "--dry-run"], directory);
      expect(await hashDatabaseFilesRaw(databasePath)).toBe(hashBefore);
      ath(["repair"], directory);
      expect(await hashDatabaseFilesRaw(databasePath)).toBe(hashBefore);
    },
  );
});
