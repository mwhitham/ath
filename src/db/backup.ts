import { chmod, lstat, open as openFile, unlink } from "node:fs/promises";
import { dirname } from "node:path";
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";
import { createClient } from "@libsql/client";
import {
  connectDatabase,
  databaseClient,
  refuseSymbolicLinkComponents,
  type AthleticDatabase,
} from "./client.js";
import type { DatabaseMetadata } from "./migrate.js";

let backupNameSequence = 0;

export function formatMigrationBackupTimestamp(date: Date): string {
  const iso = date.toISOString().replace(/[-:]/g, "");
  return `${iso.replace("Z", "")}Z`;
}

export function defaultMigrationBackupPath(databasePath: string, now = new Date()): string {
  backupNameSequence = (backupNameSequence + 1) % 10_000;
  return `${databasePath}.migration-backup.${formatMigrationBackupTimestamp(now)}-${backupNameSequence}.sqlite`;
}

export function defaultRepairBackupPath(databasePath: string, now = new Date()): string {
  backupNameSequence = (backupNameSequence + 1) % 10_000;
  return `${databasePath}.repair-backup.${formatMigrationBackupTimestamp(now)}-${backupNameSequence}.sqlite`;
}

function requireSqlite3Cli(): void {
  const probe = spawnSync("sqlite3", [":memory:", "SELECT 1;"], { encoding: "utf8" });
  if (probe.status !== 0) {
    throw new Error(
      "SQLite backup requires the sqlite3 command-line tool on PATH.",
    );
  }
}

/** Dot-command argument for sqlite3 `.backup`, using double-quoted paths with sqlite3 backslash escapes. */
export function sqlite3BackupDotCommand(backupPath: string): string {
  const escaped = backupPath
    .replace(/\\/g, "\\\\")
    .replace(/"/g, '\\"')
    .replace(/\r/g, "\\r")
    .replace(/\n/g, "\\n");
  return `.backup "${escaped}"\n`;
}

async function prepareExclusiveBackupFile(backupPath: string): Promise<void> {
  await refuseSymbolicLinkComponents(backupPath, { allowMissingTail: true });
  await refuseSymbolicLinkComponents(dirname(backupPath));
  try {
    const stat = await lstat(backupPath);
    if (stat.isSymbolicLink()) {
      throw new Error(`Refusing to write a migration backup through a symbolic link: ${backupPath}`);
    }
    throw new Error(`Refusing to overwrite existing migration backup: ${backupPath}`);
  } catch (error) {
    if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) {
      throw error;
    }
  }

  let file;
  try {
    file = await openFile(backupPath, "wx", 0o600);
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "EEXIST") {
      throw new Error(`Refusing to overwrite existing migration backup: ${backupPath}`);
    }
    throw error;
  } finally {
    await file?.close();
  }
}

async function removeBackupFileIfPresent(backupPath: string): Promise<void> {
  try {
    await unlink(backupPath);
  } catch (error) {
    if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) {
      throw error;
    }
  }
}

async function readRecordCount(database: AthleticDatabase): Promise<number> {
  const result = await databaseClient(database).execute("SELECT COUNT(*) AS count FROM records");
  const value = result.rows[0]?.count;
  if (typeof value === "number" || typeof value === "bigint") {
    return Number(value);
  }
  throw new Error("Could not read record count while verifying a migration backup");
}

async function readBackupMetadata(backupPath: string): Promise<DatabaseMetadata> {
  const backupDatabase = await connectDatabase(backupPath);
  try {
    const result = await databaseClient(backupDatabase).execute(
      "SELECT application_marker, sql_schema_version, athleticstandard_version, database_id, created_at FROM database_metadata WHERE singleton_id = 1",
    );
    const row = result.rows[0];
    if (row === undefined || result.rows.length !== 1) {
      throw new Error("Migration backup is missing Athletic Standard metadata");
    }
    const applicationMarker = String(row.application_marker);
    if (applicationMarker !== "ATH1") {
      throw new Error("Migration backup has the wrong application marker");
    }
    return {
      application_marker: "ATH1",
      sql_schema_version: Number(row.sql_schema_version),
      athleticstandard_version: String(
        row.athleticstandard_version,
      ) as DatabaseMetadata["athleticstandard_version"],
      database_id: String(row.database_id),
      created_at: String(row.created_at),
    };
  } finally {
    backupDatabase.close();
  }
}

async function readBackupRecordCount(backupPath: string): Promise<number> {
  const client = createClient({ url: pathToFileURL(backupPath).href, intMode: "number" });
  try {
    const result = await client.execute("SELECT COUNT(*) AS count FROM records");
    const value = result.rows[0]?.count;
    if (typeof value === "number" || typeof value === "bigint") {
      return Number(value);
    }
    throw new Error("Migration backup is missing record counts");
  } finally {
    client.close();
  }
}

export async function verifySqliteBackup(
  sourceDatabase: AthleticDatabase,
  backupPath: string,
  sourceMetadata: DatabaseMetadata,
): Promise<void> {
  const integrity = spawnSync("sqlite3", [backupPath, "PRAGMA integrity_check;"], {
    encoding: "utf8",
  });
  if (integrity.status !== 0) {
    throw new Error(`Migration backup integrity check failed: ${integrity.stderr || integrity.stdout}`);
  }
  const integrityResult = integrity.stdout.trim();
  if (integrityResult !== "ok") {
    throw new Error(`Migration backup failed integrity check: ${integrityResult}`);
  }

  const backupMetadata = await readBackupMetadata(backupPath);
  if (backupMetadata.database_id !== sourceMetadata.database_id) {
    throw new Error("Migration backup database_id does not match the source database");
  }
  if (backupMetadata.sql_schema_version !== sourceMetadata.sql_schema_version) {
    throw new Error("Migration backup sql_schema_version does not match the source database");
  }

  const sourceCount = await readRecordCount(sourceDatabase);
  const backupCount = await readBackupRecordCount(backupPath);
  if (backupCount !== sourceCount) {
    throw new Error(
      `Migration backup record count ${backupCount} does not match the source database ${sourceCount}`,
    );
  }
}

export async function createVerifiedSqliteBackup(
  sourceDatabase: AthleticDatabase,
  sourcePath: string,
  backupPath: string,
  sourceMetadata: DatabaseMetadata,
): Promise<void> {
  if (!sourceDatabase.isLocal) {
    throw new Error("SQL schema migration backup requires a local database file");
  }
  requireSqlite3Cli();
  await prepareExclusiveBackupFile(backupPath);
  let backupCreated = true;
  try {
    const backup = spawnSync("sqlite3", [sourcePath], {
      input: sqlite3BackupDotCommand(backupPath),
      encoding: "utf8",
    });
    if (backup.status !== 0) {
      throw new Error(`SQLite backup failed: ${backup.stderr || backup.stdout}`);
    }
    if (process.platform !== "win32") {
      await chmod(backupPath, 0o600);
    }
    await verifySqliteBackup(sourceDatabase, backupPath, sourceMetadata);
    await removeBackupFileIfPresent(`${backupPath}-wal`);
    await removeBackupFileIfPresent(`${backupPath}-shm`);
    backupCreated = false;
  } catch (error) {
    if (backupCreated) {
      await removeBackupFileIfPresent(backupPath);
    }
    throw error;
  }
}
