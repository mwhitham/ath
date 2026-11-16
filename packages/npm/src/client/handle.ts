/**
 * Database handles and target parsing.
 *
 * A handle is the 2.1.0 `AthleticDatabase` object: `url`, `isLocal`,
 * `localPath` for a file, and `close()`. It holds no connection. It remembers
 * the file, the open options, and the database ID, and each request sends
 * them so the native worker opens and checks the file again. A closed handle,
 * or an object this module did not make, is refused as 2.1.0 refused it.
 *
 * Remote libSQL URLs keep their 2.1.0 parse: they are never local files. See
 * REMOTE-TARGETS.md for what each operation does with one.
 */
import { resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import type { AthleticDatabase } from "../types.js";
import type { WorkerTarget } from "./worker.js";

interface HandleState {
  path: string;
  databaseId: string;
  allowOutdatedSchema: boolean;
  skipCheckpointOnRelease: boolean;
}

const states = new WeakMap<AthleticDatabase, HandleState>();

/** The 2.1.0 target parse: a `file:` URL or a path is local; another URL is remote. */
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

/** Make a handle for a local file the worker opened. */
export function makeHandle(
  localPath: string,
  databaseId: string,
  options: { allowOutdatedSchema: boolean; skipCheckpointOnRelease: boolean },
): AthleticDatabase {
  const database: AthleticDatabase = {
    url: pathToFileURL(localPath).href,
    isLocal: true,
    localPath,
    close: () => {
      states.delete(database);
    },
  };
  states.set(database, {
    path: localPath,
    databaseId,
    allowOutdatedSchema: options.allowOutdatedSchema,
    skipCheckpointOnRelease: options.skipCheckpointOnRelease,
  });
  return database;
}

/** The 2.1.0 refusal for a closed handle. */
export function closedError(): Error {
  return new Error("Athletic Standard database is closed");
}

/** True while the handle is open. */
export function isOpen(database: AthleticDatabase): boolean {
  return states.has(database);
}

/** The target to send for an open handle. */
export function targetOf(database: AthleticDatabase): WorkerTarget {
  const state = states.get(database);
  if (state === undefined) {
    throw closedError();
  }
  return {
    path: state.path,
    allow_outdated_schema: state.allowOutdatedSchema,
    database_id: state.databaseId,
  };
}

/** Record the database ID after an operation that can replace it, such as a restore. */
export function noteDatabaseId(database: AthleticDatabase, databaseId: string | undefined): void {
  const state = states.get(database);
  if (state !== undefined && databaseId !== undefined) {
    state.databaseId = databaseId;
  }
}

export function markSkipCheckpoint(database: AthleticDatabase): void {
  const state = states.get(database);
  if (state !== undefined) {
    state.skipCheckpointOnRelease = true;
  }
}

export function skipsCheckpoint(database: AthleticDatabase): boolean {
  return states.get(database)?.skipCheckpointOnRelease === true;
}
