import { copyFile, lstat, mkdtemp, readFile, readdir, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createClient } from "@libsql/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  buildMigrationPreview,
  createDatabase,
  createVerifiedSqliteBackup,
  databaseNeedsMigration,
  defaultMigrationBackupPath,
  formatMigrationBackupTimestamp,
  executeConfirmedDatabaseMigration,
  inspectDatabase,
  migrateDatabase,
  MigrationRequiredError,
  openDatabase,
  readDatabaseMetadata,
  sqlite3BackupDotCommand,
  writeRecord,
} from "../src/db/index.js";
import { databaseClient } from "../src/db/client.js";
import type { AthleticDatabase, MigrationPreview } from "../src/db/index.js";
import { interactiveMigrationOnOpen, useTerminalAnswer } from "../src/program.js";
const FIXTURE = join(dirname(fileURLToPath(import.meta.url)), "../examples/demo-athlete/athlete.ath.sqlite");

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
  useTerminalAnswer(undefined);
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

  it("refuses a noninteractive open when SQL schema migration is required", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    database.close();
    const client = createClient({ url: pathToFileURL(path).href, intMode: "number" });
    await client.execute("UPDATE database_metadata SET sql_schema_version = 1");
    await client.execute("PRAGMA user_version = 1");
    await client.close();
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

  it("refuses open when user_version disagrees with metadata", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    database.close();
    const client = createClient({ url: pathToFileURL(path).href, intMode: "number" });
    await client.execute("UPDATE database_metadata SET sql_schema_version = 1");
    await client.execute("PRAGMA user_version = 2");
    await client.close();
    await expect(openDatabase(path)).rejects.toThrow(/user_version/);
  });

  it("escapes sqlite3 backup dot-command paths with backslashes", () => {
    expect(sqlite3BackupDotCommand("/tmp/athlete's-backup.sqlite")).toBe(
      `.backup "/tmp/athlete's-backup.sqlite"\n`,
    );
    expect(sqlite3BackupDotCommand(`/tmp/a"b.sqlite`)).toBe(
      `.backup "/tmp/a\\"b.sqlite"\n`,
    );
    expect(sqlite3BackupDotCommand("/tmp/a\\b.sqlite")).toBe(
      `.backup "/tmp/a\\\\b.sqlite"\n`,
    );
    expect(sqlite3BackupDotCommand("/tmp/a\\t.sqlite")).toBe(
      `.backup "/tmp/a\\\\t.sqlite"\n`,
    );
  });

  it("backs up to a path that contains an apostrophe", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    openDatabases.push(database);
    const metadata = await readDatabaseMetadata(database);
    const backupPath = join(dirname(path), "athlete's-migration-backup.sqlite");
    await createVerifiedSqliteBackup(database, path, backupPath, metadata);
    const stat = await lstat(backupPath);
    expect(stat.isFile()).toBe(true);
    if (process.platform !== "win32") {
      expect(stat.mode & 0o777).toBe(0o600);
    }
  });

  it("backs up to paths containing a double quote, backslash, or backslash-letter sequences", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    openDatabases.push(database);
    const metadata = await readDatabaseMetadata(database);
    const directory = dirname(path);
    const paths = [
      join(directory, `athlete"quote.sqlite`),
      join(directory, "athlete\\slash.sqlite"),
      join(directory, "athlete\\t.sqlite"),
    ];
    for (const backupPath of paths) {
      await createVerifiedSqliteBackup(database, path, backupPath, metadata);
      const stat = await lstat(backupPath);
      expect(stat.isFile()).toBe(true);
    }
  });

  it("writes the verified backup at the reviewed path when execution passes it explicitly", async () => {
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
    const reviewedAt = new Date("2026-01-15T12:00:00.000Z");
    const executedAt = new Date("2026-01-16T12:00:00.000Z");
    const reviewedBackupPath = defaultMigrationBackupPath(path, reviewedAt);
    const regeneratedBackupPath = defaultMigrationBackupPath(path, executedAt);
    expect(reviewedBackupPath).not.toBe(regeneratedBackupPath);

    const migrationOptions =
      reviewedBackupPath === null ? {} : { backupPath: reviewedBackupPath };
    const backupPath =
      migrationOptions.backupPath ??
      defaultMigrationBackupPath(path, executedAt);
    expect(backupPath).toBe(reviewedBackupPath);

    await createVerifiedSqliteBackup(database, path, backupPath, metadata);
    expect((await lstat(reviewedBackupPath)).isFile()).toBe(true);
    await expect(lstat(regeneratedBackupPath)).rejects.toMatchObject({ code: "ENOENT" });
  });

  it("uses millisecond timestamps and a counter so same-second backups differ", () => {
    const instant = new Date("2026-01-15T12:00:00.123Z");
    const first = defaultMigrationBackupPath("/tmp/athlete.ath.sqlite", instant);
    const second = defaultMigrationBackupPath("/tmp/athlete.ath.sqlite", instant);
    expect(first).not.toBe(second);
    expect(first).toContain("123");
  });

  it("refuses to overwrite an existing migration backup path", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    openDatabases.push(database);
    const metadata = await readDatabaseMetadata(database);
    const backupPath = defaultMigrationBackupPath(path);
    await writeFile(backupPath, "stale", { mode: 0o600 });
    await expect(
      createVerifiedSqliteBackup(database, path, backupPath, metadata),
    ).rejects.toThrow(/overwrite existing migration backup/);
    expect(await lstat(backupPath).then((s) => s.size)).toBe("stale".length);
  });

  it("removes a partial backup file when verification fails", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    openDatabases.push(database);
    const metadata = await readDatabaseMetadata(database);
    const backupPath = defaultMigrationBackupPath(path);

    await expect(
      createVerifiedSqliteBackup(database, path, backupPath, {
        ...metadata,
        database_id: "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f99",
      }),
    ).rejects.toThrow(/database_id does not match/);
    await expect(lstat(backupPath)).rejects.toMatchObject({ code: "ENOENT" });
  });

  it("refuses a migration backup path through a symbolic link", async () => {
    if (process.platform === "win32") {
      return;
    }
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    openDatabases.push(database);
    const metadata = await readDatabaseMetadata(database);
    const directory = dirname(path);
    const linkPath = join(directory, "backup-link.sqlite");
    await symlink(join(directory, "target.sqlite"), linkPath);
    await expect(
      createVerifiedSqliteBackup(database, path, linkPath, metadata),
    ).rejects.toThrow(/symbolic link/);
  });

  it("does not enable WAL during migration inspection", async () => {
    const path = await tempDatabasePath();
    await copyFile(FIXTURE, path);
    const client = createClient({ url: pathToFileURL(path).href, intMode: "number" });
    await client.execute("PRAGMA journal_mode = DELETE");
    await client.close();

    const inspector = await inspectDatabase(path);
    try {
      const mode = await databaseClient(inspector).execute("PRAGMA journal_mode");
      expect(String(mode.rows[0]?.journal_mode).toLowerCase()).toBe("delete");
    } finally {
      inspector.close();
    }

    const afterClient = createClient({ url: pathToFileURL(path).href, intMode: "number" });
    const after = await afterClient.execute("PRAGMA journal_mode");
    expect(String(after.rows[0]?.journal_mode).toLowerCase()).toBe("delete");
    await afterClient.close();
  });

  it("refuses open when the database SQL schema is newer than this build supports", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    database.close();
    const client = createClient({ url: pathToFileURL(path).href, intMode: "number" });
    await client.execute("UPDATE database_metadata SET sql_schema_version = 99");
    await client.execute("PRAGMA user_version = 99");
    await client.close();
    await expect(openDatabase(path)).rejects.toThrow(/newer than supported schema/);
  });

  it("writes nothing when an interactive migration is declined", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    database.close();
    const flushClient = createClient({ url: pathToFileURL(path).href, intMode: "number" });
    await flushClient.execute("PRAGMA wal_checkpoint(FULL)");
    await flushClient.close();
    const directory = dirname(path);
    const beforeNames = (await readdir(directory)).sort();
    const beforeBytes = await readFile(path);
    const backupPath = `${path}.migration-backup.declined.sqlite`;
    const preview: MigrationPreview = {
      migration_needed: true,
      source_sql_schema_version: 2,
      target_sql_schema_version: 3,
      database_id: databaseId,
      planned_steps: [{
        target_sql_schema_version: 3,
        migration_file: "migrations/003-test.sql",
        description: "test step",
      }],
      planned_backup_path: backupPath,
      compatibility_effect: "upgrade",
    };
    const errors: string[] = [];
    vi.spyOn(console, "error").mockImplementation((line?: unknown) => {
      errors.push(String(line));
    });
    useTerminalAnswer(async () => "n");
    await expect(interactiveMigrationOnOpen(path, preview)).rejects.toThrow(
      "migration was not confirmed",
    );

    expect(errors.join("\n")).toContain("migration was not confirmed");
    expect((await readdir(directory)).sort()).toEqual(beforeNames);
    expect(await readFile(path)).toEqual(beforeBytes);
    await expect(lstat(backupPath)).rejects.toMatchObject({ code: "ENOENT" });
  });
});
