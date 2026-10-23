import { lstat, mkdir, open as openFile, unlink } from "node:fs/promises";
import { PROVIDER_ENCODING_REPAIR_SQL } from "./migrations/002-provider-encoding-repair.js";
import { repairProviderEncodingRows } from "./repair.js";
import { withWriteTransaction } from "./transaction.js";
import { incrementDatabaseRevision } from "./records.js";
import { dirname } from "node:path";
import type { Transaction } from "@libsql/client";
import {
  ATHLETIC_STANDARD_VERSION,
  DatabaseId,
  UtcTimestamp,
} from "../v1/schema.js";
import {
  connectDatabase,
  databaseClient,
  trackOpenLocalDatabase,
  markSkipWalCheckpointOnRelease,
  enableLocalWal,
  tryRevertDeleteJournalModeAfterFailedWal,
  parseDatabaseTarget,
  readApplicationId,
  readUserVersion,
  refuseSymbolicLinkComponents,
  setOwnerOnlyDatabasePermissions,
  SQL_SCHEMA_VERSION,
  SQLITE_APPLICATION_ID,
  SQLITE_APPLICATION_MARKER,
  type AthleticDatabase,
} from "./client.js";
import { INITIAL_SCHEMA_SQL } from "./migrations/001-initial.js";
import { createUuidV7 } from "./uuid.js";
import {
  createVerifiedSqliteBackup,
  defaultMigrationBackupPath,
} from "./backup.js";
import { MIGRATION_CHAIN } from "./sql-contract.js";

export interface DatabaseMetadata {
  application_marker: typeof SQLITE_APPLICATION_MARKER;
  sql_schema_version: number;
  athleticstandard_version: typeof ATHLETIC_STANDARD_VERSION;
  database_id: string;
  created_at: string;
}

function valueAsString(value: unknown, name: string): string {
  if (typeof value !== "string") {
    throw new Error(`Database metadata ${name} is not text`);
  }
  return value;
}

function valueAsNumber(value: unknown, name: string): number {
  if (typeof value !== "number" && typeof value !== "bigint") {
    throw new Error(`Database metadata ${name} is not numeric`);
  }
  return Number(value);
}

export async function readDatabaseMetadata(
  database: AthleticDatabase,
): Promise<DatabaseMetadata> {
  let result;
  try {
    result = await databaseClient(database).execute(
      "SELECT application_marker, sql_schema_version, athleticstandard_version, database_id, created_at FROM database_metadata WHERE singleton_id = 1",
    );
  } catch (error) {
    throw new Error("File is not an Athletic Standard database: metadata table is missing", {
      cause: error,
    });
  }

  const row = result.rows[0];
  if (row === undefined || result.rows.length !== 1) {
    throw new Error("Athletic Standard database metadata is missing or duplicated");
  }

  const applicationMarker = valueAsString(row.application_marker, "application_marker");
  if (applicationMarker !== SQLITE_APPLICATION_MARKER) {
    throw new Error(`Unsupported database application marker: ${applicationMarker}`);
  }

  const athleticstandardVersion = valueAsString(
    row.athleticstandard_version,
    "athleticstandard_version",
  );
  if (athleticstandardVersion !== ATHLETIC_STANDARD_VERSION) {
    throw new Error(
      `Unsupported Athletic Standard database version: ${athleticstandardVersion}`,
    );
  }

  const databaseId = valueAsString(row.database_id, "database_id");
  DatabaseId.parse(databaseId);
  const createdAt = valueAsString(row.created_at, "created_at");
  UtcTimestamp.parse(createdAt);

  return {
    application_marker: SQLITE_APPLICATION_MARKER,
    sql_schema_version: valueAsNumber(row.sql_schema_version, "sql_schema_version"),
    athleticstandard_version: ATHLETIC_STANDARD_VERSION,
    database_id: databaseId,
    created_at: createdAt,
  };
}

async function rollback(transaction: Transaction): Promise<void> {
  if (!transaction.closed) {
    await transaction.rollback();
  }
}

async function installInitialSchema(
  database: AthleticDatabase,
  databaseId: string,
  createdAt: string,
): Promise<void> {
  const transaction = await databaseClient(database).transaction("write");
  try {
    const foreignKeys = await transaction.execute("PRAGMA foreign_keys");
    if (Number(foreignKeys.rows[0]?.foreign_keys) !== 1) {
      throw new Error("SQLite foreign-key enforcement is disabled in migration transaction");
    }
    await transaction.executeMultiple(INITIAL_SCHEMA_SQL);
    await transaction.execute({
      sql: `
        INSERT INTO database_metadata (
          singleton_id,
          application_marker,
          sql_schema_version,
          athleticstandard_version,
          database_id,
          created_at
        ) VALUES (1, ?, ?, ?, ?, ?)
      `,
      args: [
        SQLITE_APPLICATION_MARKER,
        SQL_SCHEMA_VERSION,
        ATHLETIC_STANDARD_VERSION,
        databaseId,
        createdAt,
      ],
    });
    await transaction.execute(
      "INSERT INTO database_state (singleton_id, current_revision) VALUES (1, 0)",
    );
    if (database.isLocal) {
      await transaction.execute(`PRAGMA application_id = ${SQLITE_APPLICATION_ID}`);
      await transaction.execute(`PRAGMA user_version = ${SQL_SCHEMA_VERSION}`);
    }
    await transaction.commit();
  } catch (error) {
    await rollback(transaction);
    throw error;
  } finally {
    transaction.close();
  }
}

interface PreparedLocalFile {
  path: string;
  device: bigint;
  inode: bigint;
}

async function assertParentNotWritableByOtherUsers(path: string): Promise<void> {
  if (process.platform === "win32") {
    return;
  }
  const parent = await lstat(dirname(path));
  if ((parent.mode & 0o022) !== 0) {
    throw new Error(
      `Refusing database path in a directory writable by other local users: ${dirname(path)}`,
    );
  }
}

async function assertSameFile(
  path: string,
  expected: { device: bigint; inode: bigint },
): Promise<void> {
  const current = await lstat(path, { bigint: true });
  if (
    current.isSymbolicLink() ||
    current.dev !== expected.device ||
    current.ino !== expected.inode
  ) {
    throw new Error(`Database file changed while it was being opened: ${path}`);
  }
}

async function prepareLocalDatabaseFile(path: string): Promise<PreparedLocalFile> {
  await refuseSymbolicLinkComponents(path, { allowMissingTail: true });
  await mkdir(dirname(path), { recursive: true, mode: 0o700 });
  await refuseSymbolicLinkComponents(dirname(path));
  await assertParentNotWritableByOtherUsers(path);

  let file;
  try {
    file = await openFile(path, "wx", 0o600);
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "EEXIST") {
      throw new Error(`Refusing to replace existing database: ${path}`);
    }
    throw error;
  }

  try {
    const stat = await file.stat({ bigint: true });
    return { path, device: stat.dev, inode: stat.ino };
  } finally {
    await file.close();
  }
}

async function removePartialLocalDatabase(prepared: PreparedLocalFile): Promise<void> {
  let current;
  try {
    current = await lstat(prepared.path, { bigint: true });
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      return;
    }
    throw error;
  }
  if (
    current.isSymbolicLink() ||
    current.dev !== prepared.device ||
    current.ino !== prepared.inode
  ) {
    return;
  }

  await Promise.all(
    [prepared.path, `${prepared.path}-wal`, `${prepared.path}-shm`].map(async (candidate) => {
      try {
        await unlink(candidate);
      } catch (error) {
        if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) {
          throw error;
        }
      }
    }),
  );
}

export async function createDatabase(
  target: string,
  options: {
    databaseId?: string;
    createdAt?: string;
  } = {},
): Promise<AthleticDatabase> {
  const databaseId = DatabaseId.parse(options.databaseId ?? createUuidV7());
  const createdAt = UtcTimestamp.parse(options.createdAt ?? new Date().toISOString());

  const parsedTarget = parseDatabaseTarget(target);
  const prepared =
    parsedTarget.localPath === undefined
      ? undefined
      : await prepareLocalDatabaseFile(parsedTarget.localPath);
  let database: AthleticDatabase;
  try {
    database = await connectDatabase(target);
  } catch (error) {
    if (prepared !== undefined) {
      await removePartialLocalDatabase(prepared);
    }
    throw error;
  }
  try {
    if (prepared !== undefined) {
      await assertSameFile(prepared.path, prepared);
    }
    await enableLocalWal(database);
    await installInitialSchema(database, databaseId, createdAt);
    await setOwnerOnlyDatabasePermissions(database);
    trackOpenLocalDatabase(database);
    return database;
  } catch (error) {
    database.close();
    if (prepared !== undefined) {
      await removePartialLocalDatabase(prepared);
    }
    throw error;
  }
}

export interface MigrationStepPreview {
  target_sql_schema_version: number;
  migration_file: string;
  description: string;
}

export interface MigrationPreview {
  migration_needed: boolean;
  source_sql_schema_version: number;
  target_sql_schema_version: number;
  database_id: string;
  planned_steps: MigrationStepPreview[];
  planned_backup_path: string | null;
  compatibility_effect: string;
}

export class MigrationRequiredError extends Error {
  readonly preview: MigrationPreview;

  constructor(preview: MigrationPreview) {
    super(
      `Database SQL schema ${preview.source_sql_schema_version} must be upgraded to ${preview.target_sql_schema_version} before other commands can run. Run ath migrate.`,
    );
    this.name = "MigrationRequiredError";
    this.preview = preview;
  }
}

export function databaseNeedsMigration(metadata: DatabaseMetadata): boolean {
  return metadata.sql_schema_version < SQL_SCHEMA_VERSION;
}

function plannedMigrationSteps(sourceVersion: number): MigrationStepPreview[] {
  return MIGRATION_CHAIN.filter(
    (step) => step.version > sourceVersion && step.version <= SQL_SCHEMA_VERSION,
  ).map((step) => ({
    target_sql_schema_version: step.version,
    migration_file: step.file,
    description: step.description,
  }));
}

function migrationCompatibilityEffect(
  metadata: DatabaseMetadata,
  steps: MigrationStepPreview[],
): string {
  if (!databaseNeedsMigration(metadata)) {
    return "No migration is required. Direct-read views and writable tables match this ath version.";
  }
  if (steps.length === 0) {
    return (
      `No ordered migration exists from SQL schema ${metadata.sql_schema_version} to ${SQL_SCHEMA_VERSION}. ` +
      "Update ath to a version that supports this database, or restore from a portable JSON package."
    );
  }
  return (
    "The database will be upgraded in place after a verified SQLite backup. " +
    "Older direct-SQL clients may need the sql_vN_* views documented for the new schema version."
  );
}

export function buildMigrationPreview(
  metadata: DatabaseMetadata,
  localPath?: string,
  now = new Date(),
): MigrationPreview {
  const steps = plannedMigrationSteps(metadata.sql_schema_version);
  const migrationNeeded = databaseNeedsMigration(metadata);
  return {
    migration_needed: migrationNeeded,
    source_sql_schema_version: metadata.sql_schema_version,
    target_sql_schema_version: SQL_SCHEMA_VERSION,
    database_id: metadata.database_id,
    planned_steps: steps,
    planned_backup_path:
      migrationNeeded && steps.length > 0 && localPath !== undefined
        ? defaultMigrationBackupPath(localPath, now)
        : null,
    compatibility_effect: migrationCompatibilityEffect(metadata, steps),
  };
}

export interface OpenDatabaseOptions {
  onSecurityWarning?: (message: string) => void;
  /** When false, refuse to open a database that still needs a SQL schema migration. */
  allowOutdatedSchema?: boolean;
  /** Do not switch the local database to WAL. Use for migration preview and pre-backup opens. */
  deferWal?: boolean;
}

async function warnAboutReadablePaths(
  path: string,
  warn: (message: string) => void,
): Promise<void> {
  if (process.platform === "win32") {
    return;
  }
  for (const candidate of [path, `${path}-wal`, `${path}-shm`, dirname(path)]) {
    try {
      const stat = await lstat(candidate);
      if ((stat.mode & 0o044) !== 0) {
        warn(`Sensitive athlete data path is readable by other local users: ${candidate}`);
      }
    } catch (error) {
      if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) {
        throw error;
      }
    }
  }
}

/**
 * Read-only inspection open for migration preview. Never enables WAL or upgrades journal mode.
 */
export async function inspectDatabase(
  target: string,
  options: Pick<OpenDatabaseOptions, "onSecurityWarning"> = {},
): Promise<AthleticDatabase> {
  const parsedTarget = parseDatabaseTarget(target);
  if (parsedTarget.localPath !== undefined) {
    await refuseSymbolicLinkComponents(parsedTarget.localPath);
    await assertParentNotWritableByOtherUsers(parsedTarget.localPath);
    await warnAboutReadablePaths(
      parsedTarget.localPath,
      options.onSecurityWarning ?? console.warn,
    );
  }

  const database = await connectDatabase(target);
  try {
    const applicationId = await readApplicationId(database);
    if (
      applicationId !== null &&
      applicationId !== 0 &&
      applicationId !== SQLITE_APPLICATION_ID
    ) {
      throw new Error(
        `SQLite application ID ${applicationId} does not belong to Athletic Standard`,
      );
    }

    const metadata = await readDatabaseMetadata(database);
    if (metadata.sql_schema_version > SQL_SCHEMA_VERSION) {
      throw new Error(
        `Database SQL schema ${metadata.sql_schema_version} is newer than supported schema ${SQL_SCHEMA_VERSION}`,
      );
    }

    const userVersion = await readUserVersion(database);
    if (userVersion !== null && userVersion !== metadata.sql_schema_version) {
      throw new Error(
        `SQLite user_version ${userVersion} does not match database metadata ${metadata.sql_schema_version}`,
      );
    }
    markSkipWalCheckpointOnRelease(database);
    return database;
  } catch (error) {
    database.close();
    throw error;
  }
}

export async function openDatabase(
  target: string,
  options: OpenDatabaseOptions = {},
): Promise<AthleticDatabase> {
  const parsedTarget = parseDatabaseTarget(target);
  let expectedFile: { device: bigint; inode: bigint } | undefined;
  if (parsedTarget.localPath !== undefined) {
    await refuseSymbolicLinkComponents(parsedTarget.localPath);
    await assertParentNotWritableByOtherUsers(parsedTarget.localPath);
    await warnAboutReadablePaths(
      parsedTarget.localPath,
      options.onSecurityWarning ?? console.warn,
    );
    const stat = await lstat(parsedTarget.localPath, { bigint: true });
    expectedFile = { device: stat.dev, inode: stat.ino };
  }

  const database = await connectDatabase(target);
  try {
    if (parsedTarget.localPath !== undefined && expectedFile !== undefined) {
      await assertSameFile(parsedTarget.localPath, expectedFile);
    }
    const applicationId = await readApplicationId(database);
    if (
      applicationId !== null &&
      applicationId !== 0 &&
      applicationId !== SQLITE_APPLICATION_ID
    ) {
      throw new Error(
        `SQLite application ID ${applicationId} does not belong to Athletic Standard`,
      );
    }

    const metadata = await readDatabaseMetadata(database);
    if (metadata.sql_schema_version > SQL_SCHEMA_VERSION) {
      throw new Error(
        `Database SQL schema ${metadata.sql_schema_version} is newer than supported schema ${SQL_SCHEMA_VERSION}`,
      );
    }

    const userVersion = await readUserVersion(database);
    if (userVersion !== null && userVersion !== metadata.sql_schema_version) {
      throw new Error(
        `SQLite user_version ${userVersion} does not match database metadata ${metadata.sql_schema_version}`,
      );
    }

    if (databaseNeedsMigration(metadata) && options.allowOutdatedSchema !== true) {
      const preview = buildMigrationPreview(metadata, parsedTarget.localPath);
      throw new MigrationRequiredError(preview);
    }

    if (options.deferWal !== true) {
      await enableLocalWal(database);
    }
    trackOpenLocalDatabase(database);
    return database;
  } catch (error) {
    database.close();
    throw error;
  }
}

async function applyMigrationStep(
  database: AthleticDatabase,
  targetVersion: number,
): Promise<void> {
  if (targetVersion === 1) {
    throw new Error(
      "The initial SQL schema cannot be applied to an existing Athletic Standard database",
    );
  }
  const step = MIGRATION_CHAIN.find((candidate) => candidate.version === targetVersion);
  if (step === undefined) {
    throw new Error(`No migration is registered for SQL schema version ${targetVersion}`);
  }
  if (targetVersion === 2) {
    await withWriteTransaction(database, async (transaction) => {
      const repaired = await repairProviderEncodingRows(transaction);
      await transaction.executeMultiple(PROVIDER_ENCODING_REPAIR_SQL);
      if (database.isLocal) {
        await transaction.execute(`PRAGMA user_version = ${targetVersion}`);
      }
      if (repaired > 0) {
        await incrementDatabaseRevision(transaction);
      }
    });
    return;
  }
  throw new Error(
    `Migration ${step.file} is not implemented in this ath build (target SQL schema ${targetVersion})`,
  );
}

export async function applyPendingDatabaseMigrations(
  database: AthleticDatabase,
): Promise<void> {
  const metadata = await readDatabaseMetadata(database);
  if (!databaseNeedsMigration(metadata)) {
    return;
  }

  const steps = plannedMigrationSteps(metadata.sql_schema_version);
  if (steps.length === 0) {
    throw new Error(
      `No migration is available from SQL schema ${metadata.sql_schema_version} to ${SQL_SCHEMA_VERSION}`,
    );
  }

  for (const step of steps) {
    await applyMigrationStep(database, step.target_sql_schema_version);
    const current = await readDatabaseMetadata(database);
    if (current.sql_schema_version !== step.target_sql_schema_version) {
      throw new Error(
        `Migration to SQL schema ${step.target_sql_schema_version} did not update database metadata`,
      );
    }
    if (database.isLocal) {
      const userVersion = await readUserVersion(database);
      if (userVersion !== step.target_sql_schema_version) {
        throw new Error(
          `SQLite user_version ${userVersion ?? "null"} does not match SQL schema ${step.target_sql_schema_version} after migration`,
        );
      }
    }
  }

  const finalMetadata = await readDatabaseMetadata(database);
  if (finalMetadata.sql_schema_version !== SQL_SCHEMA_VERSION) {
    throw new Error(
      `Migration finished at SQL schema ${finalMetadata.sql_schema_version}, expected ${SQL_SCHEMA_VERSION}`,
    );
  }
}

export interface ConfirmedMigrationResult {
  backup_path: string | null;
  migrated: boolean;
  preview: MigrationPreview;
}

export async function executeConfirmedDatabaseMigration(
  database: AthleticDatabase,
  options: { backupPath?: string; now?: Date } = {},
): Promise<ConfirmedMigrationResult> {
  const metadata = await readDatabaseMetadata(database);
  const preview = buildMigrationPreview(metadata, database.localPath, options.now);
  if (!preview.migration_needed) {
    return { backup_path: null, migrated: false, preview };
  }
  if (preview.planned_steps.length === 0) {
    throw new Error(preview.compatibility_effect);
  }
  if (database.localPath === undefined) {
    throw new Error("SQL schema migration requires a local database file");
  }

  const backupPath = options.backupPath ?? preview.planned_backup_path;
  if (backupPath === null) {
    throw new Error("Migration backup path is missing");
  }

  const fileIdentity =
    database.localPath === undefined
      ? undefined
      : await lstat(database.localPath, { bigint: true }).then((stat) => ({
          device: stat.dev,
          inode: stat.ino,
        }));
  const revisionBefore = Number(
    (
      await databaseClient(database).execute(
        "SELECT current_revision FROM database_state WHERE singleton_id = 1",
      )
    ).rows[0]?.current_revision,
  );
  await createVerifiedSqliteBackup(database, database.localPath, backupPath, metadata);
  if (fileIdentity !== undefined && database.localPath !== undefined) {
    await assertSameFile(database.localPath, fileIdentity);
  }
  const revisionAfterBackup = Number(
    (
      await databaseClient(database).execute(
        "SELECT current_revision FROM database_state WHERE singleton_id = 1",
      )
    ).rows[0]?.current_revision,
  );
  if (revisionAfterBackup !== revisionBefore) {
    throw new Error(
      "Database revision changed during migration backup; stop other writers and retry",
    );
  }
  const journalBefore = String(
    (await databaseClient(database).execute("PRAGMA journal_mode")).rows[0]?.journal_mode,
  ).toLowerCase();
  try {
    await enableLocalWal(database);
    if (fileIdentity !== undefined && database.localPath !== undefined) {
      await assertSameFile(database.localPath, fileIdentity);
    }
    const revisionBeforeMigrate = Number(
      (
        await databaseClient(database).execute(
          "SELECT current_revision FROM database_state WHERE singleton_id = 1",
        )
      ).rows[0]?.current_revision,
    );
    if (revisionBeforeMigrate !== revisionBefore) {
      throw new Error("Database revision changed before migration could start");
    }
    await applyPendingDatabaseMigrations(database);
  } catch (error) {
    try {
      await tryRevertDeleteJournalModeAfterFailedWal(database, journalBefore);
    } catch {
      // Best-effort journal revert; the original migration failure is reported below.
    }
    const revisionAfterFailure = Number(
      (
        await databaseClient(database).execute(
          "SELECT current_revision FROM database_state WHERE singleton_id = 1",
        )
      ).rows[0]?.current_revision,
    );
    const cause =
      error instanceof Error
        ? error.message
        : "migration step failed";
    const changed = revisionAfterFailure !== revisionBefore;
    let detail = `Cause: ${cause}.`;
    if (changed) {
      detail += " The database revision changed; restore from the verified backup.";
    } else {
      detail += " The database revision did not change.";
    }
    throw new Error(
      `Migration failed after creating verified backup at ${backupPath}. ${detail}`,
      { cause: error },
    );
  }
  await setOwnerOnlyDatabasePermissions(database);
  return { backup_path: backupPath, migrated: true, preview };
}

/** Applies pending SQL schema migrations. Use only through confirmed migration flows. */
export async function migrateDatabase(database: AthleticDatabase): Promise<void> {
  await applyPendingDatabaseMigrations(database);
}
