import { open, readFile, rename, rm } from "node:fs/promises";
import { z } from "zod";
import { CurrentRecord } from "../v1/schema.js";
import {
  databaseClient,
  refuseSymbolicLinkComponents,
  type AthleticDatabase,
} from "./client.js";
import type {
  PackageImportPreview,
  PackageImportResolution,
} from "./import-package.js";
import { readDatabaseMetadata } from "./migrate.js";
import { readRecord } from "./records.js";

const Resolution = z.discriminatedUnion("choice", [
  z.object({ choice: z.literal("local") }).strict(),
  z.object({ choice: z.literal("imported") }).strict(),
  z.object({ choice: z.literal("corrected"), data: z.unknown() }).strict(),
]);

const ConflictSet = z.object({
  conflict_set_version: z.literal(1),
  source_path: z.string().min(1),
  source_fingerprint: z.string().regex(/^[a-f0-9]{64}$/),
  source_database_id: z.string().min(1),
  source_database_revision: z.number().int().nonnegative(),
  destination_database_id: z.string().min(1),
  destination_database_revision: z.number().int().nonnegative(),
  conflicts: z.array(
    z.object({
      record_id: z.string().min(1),
      message: z.string().min(1),
      resolution: Resolution.optional(),
    }).strict(),
  ),
}).strict();

export type PackageConflictSet = z.infer<typeof ConflictSet>;

export function packageConflictSetPath(database: AthleticDatabase): string {
  if (database.localPath === undefined) {
    throw new Error("temporary import conflicts require a local SQLite database");
  }
  return `${database.localPath}.import-conflicts.json`;
}

async function writeConflictSet(
  database: AthleticDatabase,
  state: PackageConflictSet,
): Promise<void> {
  const path = packageConflictSetPath(database);
  await refuseSymbolicLinkComponents(path, { allowMissingTail: true });
  const temporary = `${path}.tmp-${process.pid}`;
  const handle = await open(temporary, "wx", 0o600);
  try {
    await handle.writeFile(`${JSON.stringify(state, null, 2)}\n`, "utf8");
  } finally {
    await handle.close();
  }
  try {
    await rm(path, { force: true });
    await rename(temporary, path);
  } catch (error) {
    await rm(temporary, { force: true });
    throw error;
  }
}

export async function savePackageConflictSet(
  database: AthleticDatabase,
  preview: PackageImportPreview,
): Promise<PackageConflictSet | null> {
  const conflicts = preview.conflicts.flatMap((conflict) =>
    conflict.record_id === undefined
      ? []
      : [{
          record_id: conflict.record_id,
          message: conflict.message,
        }],
  );
  if (conflicts.length !== preview.conflicts.length || conflicts.length === 0) {
    await clearPackageConflictSet(database);
    return null;
  }
  const state = ConflictSet.parse({
    conflict_set_version: 1,
    source_path: preview.loaded.source_path,
    source_fingerprint: preview.loaded.source_fingerprint,
    source_database_id: preview.source_database_id,
    source_database_revision: preview.source_database_revision,
    destination_database_id: preview.destination_database_id,
    destination_database_revision: preview.destination_database_revision,
    conflicts,
  });
  await writeConflictSet(database, state);
  return state;
}

export async function readPackageConflictSet(
  database: AthleticDatabase,
): Promise<PackageConflictSet> {
  const path = packageConflictSetPath(database);
  await refuseSymbolicLinkComponents(path);
  let text: string;
  try {
    text = await readFile(path, "utf8");
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      throw new Error(
        "no temporary import conflicts; run ath import <package> to create a preview",
      );
    }
    throw error;
  }
  try {
    return ConflictSet.parse(JSON.parse(text));
  } catch (error) {
    throw new Error(`temporary import conflict file is invalid: ${(error as Error).message}`);
  }
}

export async function clearPackageConflictSet(
  database: AthleticDatabase,
): Promise<void> {
  if (database.localPath === undefined) {
    return;
  }
  const path = packageConflictSetPath(database);
  await refuseSymbolicLinkComponents(path, { allowMissingTail: true });
  await rm(path, { force: true });
}

export async function assertPackageConflictSetCurrent(
  database: AthleticDatabase,
  state: PackageConflictSet,
  preview: PackageImportPreview,
): Promise<void> {
  const metadata = await readDatabaseMetadata(database);
  const revisionResult = await databaseClient(database).execute(
    "SELECT current_revision FROM database_state WHERE singleton_id = 1",
  );
  const currentRevision = Number(revisionResult.rows[0]?.current_revision);
  const previewConflicts = preview.conflicts
    .filter((conflict) => conflict.record_id !== undefined)
    .map((conflict) => `${conflict.record_id}\u0000${conflict.message}`)
    .sort();
  const savedConflicts = state.conflicts
    .map((conflict) => `${conflict.record_id}\u0000${conflict.message}`)
    .sort();
  if (
    state.source_path !== preview.loaded.source_path ||
    state.source_fingerprint !== preview.loaded.source_fingerprint ||
    state.source_database_id !== preview.source_database_id ||
    state.source_database_revision !== preview.source_database_revision ||
    state.destination_database_id !== metadata.database_id ||
    state.destination_database_revision !== currentRevision ||
    preview.conflicts.some((conflict) => conflict.record_id === undefined) ||
    JSON.stringify(previewConflicts) !== JSON.stringify(savedConflicts)
  ) {
    throw new Error(
      "the package or database changed after the conflict preview; run ath import again",
    );
  }
}

export async function resolvePackageConflict(
  database: AthleticDatabase,
  state: PackageConflictSet,
  recordId: string,
  resolution: PackageImportResolution,
): Promise<PackageConflictSet> {
  const index = state.conflicts.findIndex(
    (conflict) => conflict.record_id === recordId,
  );
  if (index === -1) {
    throw new Error(`record is not in the temporary conflict set: ${recordId}`);
  }
  const local = await readRecord(database, recordId);
  if (local === null) {
    throw new Error(`local conflict record no longer exists: ${recordId}`);
  }
  if (resolution.choice === "corrected") {
    CurrentRecord.parse({
      ...local,
      current_revision: local.current_revision + 1,
      is_deleted: false,
      data: resolution.data,
    });
  }
  const next = structuredClone(state);
  next.conflicts[index]!.resolution = Resolution.parse(resolution);
  await writeConflictSet(database, next);
  return next;
}

export function conflictResolutions(
  state: PackageConflictSet,
): PackageImportResolution[] {
  return state.conflicts.flatMap((conflict) =>
    conflict.resolution === undefined
      ? []
      : [{ record_id: conflict.record_id, ...conflict.resolution }],
  );
}
