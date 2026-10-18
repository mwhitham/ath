import { mkdtemp, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  buildMigrationPreview,
  createDatabase,
  createVerifiedSqliteBackup,
  databaseNeedsMigration,
  defaultMigrationBackupPath,
  executeConfirmedDatabaseMigration,
  migrateDatabase,
  MigrationRequiredError,
  openDatabase,
  readDatabaseMetadata,
  writeRecord,
} from "../src/db/index.js";
import * as clientModule from "../src/db/client.js";
import type { AthleticDatabase } from "../src/db/index.js";

const databaseId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f30";
const createdAt = "2026-08-09T12:00:00.000Z";
const sourceId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e02";

const toolActor = {
  author_type: "tool" as const,
  tool: "ath",
  tool_version: "1.0.0",
};

const openDatabases: AthleticDatabase[] = [];
const directories: string[] = [];

async function tempDatabasePath(): Promise<string> {
  const directory = await mkdtemp(join(tmpdir(), "ath-v1-migrate-"));
  directories.push(directory);
  return join(directory, "athlete.ath.sqlite");
}

afterEach(async () => {
  vi.restoreAllMocks();
  for (const database of openDatabases.splice(0)) {
    database.close();
  }
  for (const directory of directories.splice(0)) {
    await rm(directory, { recursive: true, force: true });
  }
});

describe("v1 database migration", () => {
  it("reports no migration for the current SQL schema version", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    openDatabases.push(database);
    const metadata = await readDatabaseMetadata(database);
    expect(databaseNeedsMigration(metadata)).toBe(false);
    const preview = buildMigrationPreview(metadata, path);
    expect(preview.migration_needed).toBe(false);
    expect(preview.planned_steps).toEqual([]);
    await expect(migrateDatabase(database)).resolves.toBeUndefined();
  });

  it("refuses a noninteractive open when a newer SQL schema is required", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    database.close();
    vi.spyOn(clientModule, "SQL_SCHEMA_VERSION", "get").mockReturnValue(2 as 1);
    await expect(openDatabase(path)).rejects.toBeInstanceOf(MigrationRequiredError);
  });

  it("creates and verifies a SQLite backup before migration would run", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    openDatabases.push(database);
    await writeRecord(database, {
      record_type: "source",
      record_id: sourceId,
      created_at: createdAt,
      changed_by: toolActor,
      data: {
        source_type: "wearable",
        data_writer: "WHOOP",
        data_route: "whoop_csv",
        vendor: "whoop",
      },
    });
    const metadata = await readDatabaseMetadata(database);
    const backupPath = defaultMigrationBackupPath(path);
    await createVerifiedSqliteBackup(database, path, backupPath, metadata);
    const files = await readdir(dirname(path));
    expect(files.some((name) => name.includes("migration-backup"))).toBe(true);
  });

  it("leaves the source database unchanged when no migration path exists", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    openDatabases.push(database);
    vi.spyOn(clientModule, "SQL_SCHEMA_VERSION", "get").mockReturnValue(2 as 1);
    const metadata = await readDatabaseMetadata(database);
    await expect(executeConfirmedDatabaseMigration(database)).rejects.toThrow(
      /No ordered migration/,
    );
    const after = await readDatabaseMetadata(database);
    expect(after.sql_schema_version).toBe(metadata.sql_schema_version);
    expect(after.database_id).toBe(databaseId);
  });

  it("plans a future migration step when the supported schema is newer", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    openDatabases.push(database);
    vi.spyOn(clientModule, "SQL_SCHEMA_VERSION", "get").mockReturnValue(2 as 1);
    const preview = buildMigrationPreview(await readDatabaseMetadata(database), path);
    expect(preview.migration_needed).toBe(true);
    expect(preview.source_sql_schema_version).toBe(1);
    expect(preview.target_sql_schema_version).toBe(2);
    expect(preview.planned_steps).toEqual([]);
    expect(preview.planned_backup_path).toBeNull();
  });
});
