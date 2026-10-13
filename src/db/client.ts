import { chmod, lstat, mkdir } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createClient, type Client, type ResultSet } from "@libsql/client";

export const SQLITE_APPLICATION_ID = 0x41544831;
export const SQLITE_APPLICATION_MARKER = "ATH1";
export const SQLITE_BUSY_TIMEOUT_MS = 5_000;
export const SQL_SCHEMA_VERSION = 1;

export interface AthleticDatabase {
  readonly client: Client;
  readonly url: string;
  readonly isLocal: boolean;
  readonly localPath?: string;
  close(): void;
}

function firstNumber(result: ResultSet, column: string): number {
  const row = result.rows[0];
  const value = row?.[column];
  if (typeof value === "bigint") {
    return Number(value);
  }
  if (typeof value === "number") {
    return value;
  }
  throw new Error(`SQLite did not return numeric ${column}`);
}

async function configureConnection(database: AthleticDatabase): Promise<void> {
  await database.client.execute("PRAGMA foreign_keys = ON");
  await database.client.execute(`PRAGMA busy_timeout = ${SQLITE_BUSY_TIMEOUT_MS}`);

  const foreignKeys = firstNumber(
    await database.client.execute("PRAGMA foreign_keys"),
    "foreign_keys",
  );
  if (foreignKeys !== 1) {
    throw new Error("SQLite foreign-key enforcement could not be enabled");
  }

  if (database.isLocal) {
    await database.client.execute("PRAGMA journal_mode = WAL");
  }
}

function parseTarget(target: string): {
  url: string;
  isLocal: boolean;
  localPath?: string;
} {
  if (target.startsWith("file:")) {
    const localPath = resolve(fileURLToPath(target));
    return { url: pathToFileURL(localPath).href, isLocal: true, localPath };
  }
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(target)) {
    return { url: target, isLocal: false };
  }
  const localPath = resolve(target);
  return { url: pathToFileURL(localPath).href, isLocal: true, localPath };
}

async function refuseSymbolicLink(path: string, allowMissing: boolean): Promise<void> {
  try {
    const stat = await lstat(path);
    if (stat.isSymbolicLink()) {
      throw new Error(`Refusing to open database through a symbolic link: ${path}`);
    }
  } catch (error) {
    if (
      allowMissing &&
      error instanceof Error &&
      "code" in error &&
      error.code === "ENOENT"
    ) {
      return;
    }
    throw error;
  }
}

export async function connectDatabase(
  target: string,
  options: { allowMissingLocalFile?: boolean } = {},
): Promise<AthleticDatabase> {
  const parsed = parseTarget(target);
  if (parsed.localPath !== undefined) {
    await refuseSymbolicLink(parsed.localPath, options.allowMissingLocalFile === true);
    if (options.allowMissingLocalFile === true) {
      await mkdir(dirname(parsed.localPath), { recursive: true, mode: 0o700 });
    }
  }

  const client = createClient({ url: parsed.url, intMode: "number" });
  const database: AthleticDatabase = {
    client,
    url: parsed.url,
    isLocal: parsed.isLocal,
    ...(parsed.localPath === undefined ? {} : { localPath: parsed.localPath }),
    close: () => client.close(),
  };

  try {
    await configureConnection(database);
    return database;
  } catch (error) {
    client.close();
    throw error;
  }
}

export async function readApplicationId(database: AthleticDatabase): Promise<number | null> {
  if (!database.isLocal) {
    return null;
  }
  return firstNumber(await database.client.execute("PRAGMA application_id"), "application_id");
}

export async function readUserVersion(database: AthleticDatabase): Promise<number | null> {
  if (!database.isLocal) {
    return null;
  }
  return firstNumber(await database.client.execute("PRAGMA user_version"), "user_version");
}

export async function setOwnerOnlyDatabasePermissions(
  database: AthleticDatabase,
): Promise<void> {
  if (database.localPath === undefined || process.platform === "win32") {
    return;
  }
  await chmod(database.localPath, 0o600);
}
