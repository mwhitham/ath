import { chmod, copyFile, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createClient } from "@libsql/client";
import { pathToFileURL } from "node:url";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import {
  buildMigrationPreview,
  createDatabase,
  openDatabase,
  readDatabaseMetadata,
} from "../src/db/index.js";
import { databaseClient } from "../src/db/client.js";
import type { AthleticDatabase } from "../src/db/index.js";
import { interactiveMigrationOnOpen, useTerminalAnswer } from "../src/program.js";
import { ath } from "./helpers/cli-exec.js";
import { materializeNpm100ProviderDatabase } from "./helpers/npm100-database.js";
import { prepareMigratedNpm100 } from "./helpers/npm100-prepared.js";
import { hashDatabaseFilesRaw, useDeleteJournalMode } from "./helpers/raw-database-hash.js";

const DEMO_FIXTURE = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../examples/demo-athlete/athlete.ath.sqlite",
);
const DEMO_PROFILE_ID = "01890f3e-e7d3-7cc0-88c8-000000000002";

const openDatabases: AthleticDatabase[] = [];
const directories: string[] = [];

async function sidecarPaths(databasePath: string): Promise<string[]> {
  const paths: string[] = [];
  for (const suffix of ["-wal", "-shm"]) {
    try {
      await import("node:fs/promises").then(({ access }) => access(`${databasePath}${suffix}`));
      paths.push(`${databasePath}${suffix}`);
    } catch {
      // absent
    }
  }
  return paths;
}

async function runCliExpectSidecarFree(
  directory: string,
  databasePath: string,
  args: string[],
  options: { checkHash?: boolean } = {},
): Promise<void> {
  const checkHash = options.checkHash ?? true;
  if (checkHash) {
    await useDeleteJournalMode(databasePath);
  }
  const hashBefore = checkHash ? await hashDatabaseFilesRaw(databasePath) : "";
  ath(args, directory);
  expect(await sidecarPaths(databasePath)).toEqual([]);
  if (checkHash) {
    expect(await hashDatabaseFilesRaw(databasePath)).toBe(hashBefore);
  }
}

async function injectPointMeasurementDivergence(databasePath: string): Promise<string> {
  const database = await openDatabase(databasePath, { allowOutdatedSchema: true, deferWal: true });
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
    sql: "UPDATE point_measurements SET data_json = ?, standard_value = 999 WHERE record_id = ?",
    args: [divergent, recordId],
  });
  const idRow = await databaseClient(database).execute(
    "SELECT database_id FROM database_metadata WHERE singleton_id = 1",
  );
  const databaseId = String(idRow.rows[0]?.database_id);
  await databaseClient(database).execute("PRAGMA journal_mode = DELETE");
  const { releaseLocalDatabase } = await import("../src/db/index.js");
  await releaseLocalDatabase(database);
  openDatabases.pop();
  return databaseId;
}

afterEach(async () => {
  useTerminalAnswer(undefined);
  for (const database of openDatabases.splice(0)) {
    database.close();
  }
  for (const directory of directories.splice(0)) {
    await rm(directory, { recursive: true, force: true });
  }
});

describe.runIf(process.platform !== "win32")("CLI refusal paths and WAL sidecars", () => {
  let migratedTemplatePath: string;
  let migratedTemplateDir: string;

  beforeAll(async () => {
    const prepared = await prepareMigratedNpm100();
    migratedTemplatePath = prepared.databasePath;
    migratedTemplateDir = prepared.directory;
  }, 300_000);

  async function freshMigratedCopy(): Promise<{ directory: string; databasePath: string }> {
    const directory = await mkdtemp(join(tmpdir(), "ath-sidecar-"));
    directories.push(directory);
    const databasePath = join(directory, "athlete.ath.sqlite");
    await copyFile(migratedTemplatePath, databasePath);
    return { directory, databasePath };
  }

  it("schema-1 write command refusal leaves no WAL sidecars", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    await runCliExpectSidecarFree(
      directory,
      databasePath,
      ["log", "-y", "felt good"],
      { checkHash: false },
    );
  });

  it("header mismatch refusal leaves no WAL sidecars", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    const client = createClient({ url: pathToFileURL(databasePath).href, intMode: "number" });
    await client.execute("UPDATE database_metadata SET sql_schema_version = 2");
    await client.close();
    await runCliExpectSidecarFree(
      directory,
      databasePath,
      ["log", "-y", "felt good"],
      { checkHash: false },
    );
  });

  it("failed divergent migrate leaves no WAL sidecars", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    const databaseId = await injectPointMeasurementDivergence(databasePath);
    await runCliExpectSidecarFree(
      directory,
      databasePath,
      ["migrate", "-y", "--confirm-migration", databaseId],
      { checkHash: false },
    );
  });

  it("read-only database file refusal leaves no WAL sidecars", async () => {
    const { directory, databasePath } = await freshMigratedCopy();
    await chmod(databasePath, 0o444);
    await runCliExpectSidecarFree(
      directory,
      databasePath,
      ["log", "-y", "felt good"],
      { checkHash: false },
    );
    await chmod(databasePath, 0o600);
  });

  it("non-TTY log refusal leaves no WAL sidecars", async () => {
    const { directory, databasePath } = await freshMigratedCopy();
    await runCliExpectSidecarFree(
      directory,
      databasePath,
      ["log", "felt good"],
      { checkHash: false },
    );
  });

  it("erase without confirm leaves no WAL sidecars", async () => {
    const directory = await mkdtemp(join(tmpdir(), "ath-sidecar-erase-"));
    directories.push(directory);
    const databasePath = join(directory, "athlete.ath.sqlite");
    await copyFile(DEMO_FIXTURE, databasePath);
    await runCliExpectSidecarFree(
      directory,
      databasePath,
      ["erase", DEMO_PROFILE_ID, "--revision", "1"],
      { checkHash: false },
    );
  });

  it("corrupt database refusal leaves no WAL sidecars", async () => {
    const { directory, databasePath } = await freshMigratedCopy();
    await writeFile(databasePath, "not a sqlite database", { mode: 0o600 });
    await runCliExpectSidecarFree(directory, databasePath, ["check"], { checkHash: false });
  });

  it("declined TTY migration leaves no WAL sidecars", async () => {
    const directory = await mkdtemp(join(tmpdir(), "ath-sidecar-migrate-decline-"));
    directories.push(directory);
    const databasePath = join(directory, "athlete.ath.sqlite");
    const database = await createDatabase(databasePath);
    openDatabases.push(database);
    await databaseClient(database).execute("PRAGMA journal_mode = DELETE");
    const { releaseLocalDatabase } = await import("../src/db/index.js");
    await releaseLocalDatabase(database);
    openDatabases.pop();
    const client = createClient({ url: pathToFileURL(databasePath).href, intMode: "number" });
    await client.execute("UPDATE database_metadata SET sql_schema_version = 1");
    await client.execute("PRAGMA user_version = 1");
    await client.close();
    const reader = await openDatabase(databasePath, { allowOutdatedSchema: true, deferWal: true });
    openDatabases.push(reader);
    const metadata = await readDatabaseMetadata(reader);
    reader.close();
    openDatabases.pop();
    const preview = buildMigrationPreview(metadata, databasePath);
    useTerminalAnswer(async () => "n");
    await expect(interactiveMigrationOnOpen(databasePath, preview)).rejects.toThrow(
      "migration was not confirmed",
    );
    expect(await sidecarPaths(databasePath)).toEqual([]);
  });

  it("conflicts show leaves no WAL sidecars on a DELETE-mode database", async () => {
    const directory = await mkdtemp(join(tmpdir(), "ath-sidecar-conflicts-"));
    directories.push(directory);
    const basePackage = join(directory, "base-package");
    expect(ath(["export", basePackage, "--file", DEMO_FIXTURE], directory).code).toBe(0);
    const local = join(directory, "local.ath.sqlite");
    const imported = join(directory, "imported.ath.sqlite");
    await copyFile(DEMO_FIXTURE, local);
    await copyFile(DEMO_FIXTURE, imported);
    expect(ath(["import", basePackage, "--file", local, "--yes"], directory).code).toBe(0);
    expect(ath(["import", basePackage, "--file", imported, "--yes"], directory).code).toBe(0);
    const profile = JSON.parse(
      ath(["records", "show", DEMO_PROFILE_ID, "--file", local, "--json"], directory).stdout,
    ).record as { data: Record<string, unknown> };
    for (const [db, name] of [
      [local, "Local name"],
      [imported, "Imported name"],
    ] as const) {
      expect(
        ath(
          [
            "correct",
            DEMO_PROFILE_ID,
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
    await runCliExpectSidecarFree(directory, local, [
      "conflicts",
      "show",
      DEMO_PROFILE_ID,
      "--file",
      local,
      "--json",
    ]);
  });

  afterAll(async () => {
    await rm(migratedTemplateDir, { recursive: true, force: true });
  });
});
