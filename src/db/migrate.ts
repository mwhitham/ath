import { lstat, unlink } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { Transaction } from "@libsql/client";
import {
  ATHLETIC_STANDARD_VERSION,
  DatabaseId,
  UtcTimestamp,
} from "../v1/schema.js";
import {
  connectDatabase,
  readApplicationId,
  readUserVersion,
  setOwnerOnlyDatabasePermissions,
  SQL_SCHEMA_VERSION,
  SQLITE_APPLICATION_ID,
  SQLITE_APPLICATION_MARKER,
  type AthleticDatabase,
} from "./client.js";
import { INITIAL_SCHEMA_SQL } from "./migrations/001-initial.js";
import { createUuidV7 } from "./uuid.js";

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
    result = await database.client.execute(
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
  const transaction = await database.client.transaction("write");
  try {
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

async function pathExists(path: string): Promise<boolean> {
  try {
    await lstat(path);
    return true;
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      return false;
    }
    throw error;
  }
}

function localPathForTarget(target: string): string | undefined {
  if (target.startsWith("file:")) {
    return resolve(fileURLToPath(target));
  }
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(target)) {
    return undefined;
  }
  return resolve(target);
}

async function removePartialLocalDatabase(path: string): Promise<void> {
  await Promise.all(
    [path, `${path}-wal`, `${path}-shm`].map(async (candidate) => {
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

  const localPath = localPathForTarget(target);
  if (localPath !== undefined && (await pathExists(localPath))) {
    throw new Error(`Refusing to replace existing database: ${localPath}`);
  }

  const database = await connectDatabase(target, { allowMissingLocalFile: true });
  try {
    await installInitialSchema(database, databaseId, createdAt);
    await setOwnerOnlyDatabasePermissions(database);
    return database;
  } catch (error) {
    database.close();
    if (database.localPath !== undefined) {
      await removePartialLocalDatabase(database.localPath);
    }
    throw error;
  }
}

export async function openDatabase(target: string): Promise<AthleticDatabase> {
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
    return database;
  } catch (error) {
    database.close();
    throw error;
  }
}

export async function migrateDatabase(database: AthleticDatabase): Promise<void> {
  const metadata = await readDatabaseMetadata(database);
  if (metadata.sql_schema_version === SQL_SCHEMA_VERSION) {
    return;
  }
  throw new Error(
    `No migration is available from SQL schema ${metadata.sql_schema_version} to ${SQL_SCHEMA_VERSION}`,
  );
}
