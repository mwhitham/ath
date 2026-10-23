import { chmod, lstat } from "node:fs/promises";
import { parse, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createClient, type Client, type ResultSet } from "@libsql/client";

export const SQLITE_APPLICATION_ID = 0x41544831;
export const SQLITE_APPLICATION_MARKER = "ATH1";
export const SQLITE_BUSY_TIMEOUT_MS = 5_000;
export const SQL_SCHEMA_VERSION = 2;

export interface AthleticDatabase {
  readonly url: string;
  readonly isLocal: boolean;
  readonly localPath?: string;
  close(): void;
}

const clients = new WeakMap<AthleticDatabase, Client>();
const openLocalDatabases = new Set<AthleticDatabase>();
const skipWalCheckpointOnRelease = new WeakMap<AthleticDatabase, boolean>();

export function markSkipWalCheckpointOnRelease(database: AthleticDatabase): void {
  skipWalCheckpointOnRelease.set(database, true);
}

export function trackOpenLocalDatabase(database: AthleticDatabase): void {
  if (database.isLocal) {
    openLocalDatabases.add(database);
  }
}

export function untrackOpenLocalDatabase(database: AthleticDatabase): void {
  openLocalDatabases.delete(database);
}

export async function releaseAllOpenLocalDatabases(): Promise<void> {
  for (const database of [...openLocalDatabases]) {
    await releaseLocalDatabase(database);
  }
}

export function databaseClient(database: AthleticDatabase): Client {
  const client = clients.get(database);
  if (client === undefined) {
    throw new Error("Athletic Standard database is closed");
  }
  return client;
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
  const client = databaseClient(database);
  await client.execute("PRAGMA foreign_keys = ON");

  const foreignKeys = firstNumber(
    await client.execute("PRAGMA foreign_keys"),
    "foreign_keys",
  );
  if (foreignKeys !== 1) {
    throw new Error("SQLite foreign-key enforcement could not be enabled");
  }
}

export function parseDatabaseTarget(target: string): {
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

export async function refuseSymbolicLinkComponents(
  path: string,
  options: { allowMissingTail?: boolean } = {},
): Promise<void> {
  const absolutePath = resolve(path);
  const pathParts = absolutePath
    .slice(parse(absolutePath).root.length)
    .split(sep)
    .filter(Boolean);
  let currentPath = parse(absolutePath).root;
  for (const part of pathParts) {
    currentPath = resolve(currentPath, part);
    try {
      const stat = await lstat(currentPath);
      if (stat.isSymbolicLink()) {
        throw new Error(`Refusing to use path through a symbolic link: ${currentPath}`);
      }
    } catch (error) {
      if (
        options.allowMissingTail === true &&
        error instanceof Error &&
        "code" in error &&
        error.code === "ENOENT"
      ) {
        return;
      }
      throw error;
    }
  }
}

export async function connectDatabase(target: string): Promise<AthleticDatabase> {
  const parsed = parseDatabaseTarget(target);
  if (parsed.localPath !== undefined) {
    await refuseSymbolicLinkComponents(parsed.localPath);
  }

  const client = createClient({
    url: parsed.url,
    intMode: "number",
    timeout: SQLITE_BUSY_TIMEOUT_MS,
  });
  const database: AthleticDatabase = {
    url: parsed.url,
    isLocal: parsed.isLocal,
    ...(parsed.localPath === undefined ? {} : { localPath: parsed.localPath }),
    close: () => {
      client.close();
      clients.delete(database);
    },
  };
  clients.set(database, client);

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
  return firstNumber(await databaseClient(database).execute("PRAGMA application_id"), "application_id");
}

export async function readUserVersion(database: AthleticDatabase): Promise<number | null> {
  if (!database.isLocal) {
    return null;
  }
  return firstNumber(await databaseClient(database).execute("PRAGMA user_version"), "user_version");
}

export async function enableLocalWal(database: AthleticDatabase): Promise<void> {
  if (!database.isLocal) {
    return;
  }
  const result = await databaseClient(database).execute("PRAGMA journal_mode = WAL");
  const mode = String(result.rows[0]?.journal_mode).toLowerCase();
  if (mode !== "wal") {
    throw new Error(`SQLite did not enable WAL mode: ${mode}`);
  }
}

/** Close a local database and truncate WAL sidecars when possible. */
export async function releaseLocalDatabase(database: AthleticDatabase): Promise<void> {
  if (!database.isLocal) {
    database.close();
    return;
  }
  try {
    const journalMode = String(
      (await databaseClient(database).execute("PRAGMA journal_mode")).rows[0]?.journal_mode,
    ).toLowerCase();
    if (journalMode === "wal" && skipWalCheckpointOnRelease.get(database) !== true) {
      await databaseClient(database).execute("PRAGMA wal_checkpoint(TRUNCATE)");
    }
  } catch {
    // The file may already be closed or read-only.
  }
  skipWalCheckpointOnRelease.delete(database);
  untrackOpenLocalDatabase(database);
  database.close();
}

async function chmodIfPresent(path: string, mode: number): Promise<void> {
  try {
    await chmod(path, mode);
  } catch (error) {
    if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) {
      throw error;
    }
  }
}

export async function setOwnerOnlyDatabasePermissions(
  database: AthleticDatabase,
): Promise<void> {
  if (database.localPath === undefined || process.platform === "win32") {
    return;
  }
  await Promise.all([
    chmodIfPresent(database.localPath, 0o600),
    chmodIfPresent(`${database.localPath}-wal`, 0o600),
    chmodIfPresent(`${database.localPath}-shm`, 0o600),
  ]);
}
