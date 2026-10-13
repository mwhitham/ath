import { createHash } from "node:crypto";
import {
  lstat,
  mkdir,
  open as openFile,
  readFile,
  rename,
  rm,
} from "node:fs/promises";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import type { Transaction } from "@libsql/client";
import {
  ATHLETIC_STANDARD_VERSION,
  AthleticStandardPackage,
  CurrentRecord,
  ExportManifest,
  MeasurementSeriesFile,
  RecordRevision,
  type AthleticStandardPackageT,
  type CurrentRecordT,
  type MeasurementSeriesFileT,
  type RecordDeletionT,
  type RecordErasureMarkerT,
  type RecordImportKeyT,
  type RecordRevisionT,
} from "../v1/schema.js";
import {
  assertAttachmentFileType,
  storedAttachmentName,
} from "./attachments.js";
import {
  databaseClient,
  refuseSymbolicLinkComponents,
  type AthleticDatabase,
} from "./client.js";
import { readDatabaseMetadata } from "./migrate.js";
import {
  incrementDatabaseRevision,
  upsertTypedRecord,
  validateTypedRelationships,
} from "./records.js";
import { withWriteTransaction } from "./transaction.js";

const MAX_DOCUMENT_BYTES = 512 * 1024 * 1024;
const MAX_PACKAGE_FILES = 10_000;
const MAX_PACKAGE_RECORDS = 50_000_000;
const MAX_JSON_NESTING = 128;

export interface LoadedPackage {
  package: AthleticStandardPackageT;
  series: MeasurementSeriesFileT[];
  all_records: CurrentRecordT[];
  all_revisions: RecordRevisionT[];
  all_deletions: RecordDeletionT[];
  all_import_keys: RecordImportKeyT[];
  source_path: string;
  source_fingerprint: string;
  source_directory: string;
  attachment_files: Map<string, string>;
}

export interface PackageImportConflict {
  record_id?: string;
  message: string;
}

export interface PackageImportResolution {
  record_id: string;
  choice: "local" | "imported" | "corrected";
  data?: unknown;
}

export interface PackageImportPreview {
  source_database_id: string;
  source_database_revision: number;
  destination_database_id: string;
  destination_database_revision: number;
  restore: boolean;
  new_records: string[];
  matching_records: string[];
  changed_records: string[];
  conflicts: PackageImportConflict[];
  unrepresentable_records: string[];
  unknown_fields: string[];
  loaded: LoadedPackage;
}

function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function assertJsonNesting(bytes: Uint8Array, path: string): void {
  let depth = 0;
  let inString = false;
  let escaped = false;
  for (const byte of bytes) {
    if (inString) {
      if (escaped) {
        escaped = false;
      } else if (byte === 0x5c) {
        escaped = true;
      } else if (byte === 0x22) {
        inString = false;
      }
      continue;
    }
    if (byte === 0x22) {
      inString = true;
    } else if (byte === 0x7b || byte === 0x5b) {
      depth++;
      if (depth > MAX_JSON_NESTING) {
        throw new Error(
          `package JSON exceeds nesting limit ${MAX_JSON_NESTING}: ${path}`,
        );
      }
    } else if (byte === 0x7d || byte === 0x5d) {
      depth--;
    }
  }
}

async function safeReadJson(path: string): Promise<{ bytes: Buffer; value: unknown }> {
  await refuseSymbolicLinkComponents(path);
  const stat = await lstat(path);
  if (!stat.isFile()) {
    throw new Error(`package path is not a regular file: ${path}`);
  }
  if (stat.size > MAX_DOCUMENT_BYTES) {
    throw new Error(`package JSON exceeds ${MAX_DOCUMENT_BYTES} bytes: ${path}`);
  }
  const bytes = await readFile(path);
  assertJsonNesting(bytes, path);
  let value: unknown;
  try {
    value = JSON.parse(bytes.toString("utf8"));
  } catch (error) {
    throw new Error(`package JSON is invalid: ${path}: ${(error as Error).message}`);
  }
  return { bytes, value };
}

function packageVersion(value: unknown): string | undefined {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return undefined;
  }
  const version = (value as Record<string, unknown>).athleticstandard_version;
  return typeof version === "string" ? version : undefined;
}

function resolvePackageFile(sourceDirectory: string, file: string): string {
  const path = resolve(sourceDirectory, file);
  const within = relative(resolve(sourceDirectory), path);
  if (within === "" || within.startsWith("..") || isAbsolute(within)) {
    throw new Error(`package file escapes its directory: ${file}`);
  }
  return path;
}

async function packageDocumentPath(inputPath: string): Promise<{
  documentPath: string;
  sourceDirectory: string;
}> {
  const absolute = resolve(inputPath);
  await refuseSymbolicLinkComponents(absolute);
  const stat = await lstat(absolute);
  if (stat.isDirectory()) {
    return {
      documentPath: join(absolute, "athlete.ath.json"),
      sourceDirectory: absolute,
    };
  }
  if (!stat.isFile()) {
    throw new Error(`import path is not a regular file or directory: ${absolute}`);
  }
  return { documentPath: absolute, sourceDirectory: dirname(absolute) };
}

export async function loadAthleticStandardPackage(
  inputPath: string,
): Promise<LoadedPackage> {
  const { documentPath, sourceDirectory } = await packageDocumentPath(inputPath);
  const document = await safeReadJson(documentPath);
  const fingerprintParts = [
    `athlete.ath.json\u0000${document.bytes.byteLength}\u0000${sha256(document.bytes)}`,
  ];
  const version = packageVersion(document.value);
  if (version !== undefined && /^0\./.test(version)) {
    throw new Error(
      `Athletic Standard ${version} is a prerelease JSON format. v1 does not import 0.x files.`,
    );
  }
  if (version !== ATHLETIC_STANDARD_VERSION) {
    throw new Error(
      `unsupported Athletic Standard package version: ${version ?? "missing"}`,
    );
  }
  const parsedPackage = AthleticStandardPackage.safeParse(document.value);
  if (!parsedPackage.success) {
    const first = parsedPackage.error.issues[0];
    throw new Error(
      `package cannot be represented: ${first?.path.join(".") || "root"}: ` +
        `${first?.message ?? "invalid package"}`,
    );
  }
  const packageValue = parsedPackage.data;
  if (packageValue.measurement_series_files.length > MAX_PACKAGE_FILES) {
    throw new Error(`package has more than ${MAX_PACKAGE_FILES} series files`);
  }

  const manifestPath = join(sourceDirectory, "export-manifest.json");
  let manifest: ReturnType<typeof ExportManifest.parse> | undefined;
  try {
    const manifestDocument = await safeReadJson(manifestPath);
    fingerprintParts.push(
      `export-manifest.json\u0000${manifestDocument.bytes.byteLength}\u0000${sha256(manifestDocument.bytes)}`,
    );
    manifest = ExportManifest.parse(manifestDocument.value);
    if (manifest.files.length > MAX_PACKAGE_FILES + 2) {
      throw new Error(`package has more than ${MAX_PACKAGE_FILES + 2} files`);
    }
    if (
      manifest.source_database_id !== packageValue.database_id ||
      manifest.database_revision !== packageValue.database_revision
    ) {
      throw new Error("export manifest does not match the package database");
    }
    for (const file of manifest.files) {
      const path = resolvePackageFile(sourceDirectory, file.file);
      await refuseSymbolicLinkComponents(path);
      const bytes = await readFile(path);
      if (bytes.byteLength !== file.byte_count || sha256(bytes) !== file.sha256) {
        throw new Error(`package file does not match its manifest: ${file.file}`);
      }
    }
  } catch (error) {
    if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) {
      throw error;
    }
  }

  const series: MeasurementSeriesFileT[] = [];
  for (const reference of packageValue.measurement_series_files) {
    const path = resolvePackageFile(sourceDirectory, reference.file);
    const file = await safeReadJson(path);
    fingerprintParts.push(
      `${reference.file}\u0000${file.bytes.byteLength}\u0000${sha256(file.bytes)}`,
    );
    if (sha256(file.bytes) !== reference.sha256) {
      throw new Error(`series hash does not match package reference: ${reference.file}`);
    }
    const parsed = MeasurementSeriesFile.parse(file.value);
    if (
      parsed.source_id !== reference.source_id ||
      parsed.quantity !== reference.quantity ||
      parsed.records.length !== reference.sample_count ||
      parsed.record_revisions.length !== reference.revision_count ||
      parsed.record_deletions.length !== reference.deletion_count ||
      parsed.record_import_keys.length !== reference.import_key_count
    ) {
      throw new Error(`series counts or grouping do not match: ${reference.file}`);
    }
    series.push(parsed);
  }

  const allRecords = [
    ...packageValue.records,
    ...series.flatMap((file) => file.records),
  ];
  const allRevisions = [
    ...packageValue.record_revisions,
    ...series.flatMap((file) => file.record_revisions),
  ];
  const allDeletions = [
    ...packageValue.record_deletions,
    ...series.flatMap((file) => file.record_deletions),
  ];
  const allImportKeys = [
    ...packageValue.record_import_keys,
    ...series.flatMap((file) => file.record_import_keys),
  ];
  const packageObjectCount =
    allRecords.length +
    allRevisions.length +
    allDeletions.length +
    allImportKeys.length +
    packageValue.record_erasure_markers.length;
  if (packageObjectCount > MAX_PACKAGE_RECORDS) {
    throw new Error(
      `package has more than ${MAX_PACKAGE_RECORDS} record and history objects`,
    );
  }
  const recordIds = new Set<string>();
  for (const record of allRecords) {
    if (recordIds.has(record.record_id)) {
      throw new Error(`package repeats record ID across files: ${record.record_id}`);
    }
    recordIds.add(record.record_id);
  }

  const attachmentFiles = new Map<string, string>();
  for (const record of allRecords) {
    if (record.record_type !== "attachment" || record.is_deleted) {
      continue;
    }
    const relativePath = `attachments/${storedAttachmentName(record)}`;
    const path = resolvePackageFile(sourceDirectory, relativePath);
    await refuseSymbolicLinkComponents(path);
    const bytes = await readFile(path);
    assertAttachmentFileType(record, bytes);
    fingerprintParts.push(
      `${relativePath}\u0000${bytes.byteLength}\u0000${sha256(bytes)}`,
    );
    if (
      bytes.byteLength !== record.data.byte_count ||
      sha256(bytes) !== record.data.sha256
    ) {
      throw new Error(`attachment does not match record ${record.record_id}`);
    }
    attachmentFiles.set(record.record_id, path);
    if (
      manifest !== undefined &&
      !manifest.files.some(
        (file) => file.file === relativePath && file.sha256 === record.data.sha256,
      )
    ) {
      throw new Error(`attachment is missing from the manifest: ${relativePath}`);
    }
  }

  return {
    package: packageValue,
    series,
    all_records: allRecords,
    all_revisions: allRevisions,
    all_deletions: allDeletions,
    all_import_keys: allImportKeys,
    source_path: resolve(inputPath),
    source_fingerprint: sha256(
      Buffer.from(fingerprintParts.sort().join("\n"), "utf8"),
    ),
    source_directory: sourceDirectory,
    attachment_files: attachmentFiles,
  };
}

function sameValue(left: unknown, right: unknown): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

async function localRecordsById(
  database: AthleticDatabase,
  recordIds: readonly string[],
): Promise<Map<string, CurrentRecordT>> {
  const records = new Map<string, CurrentRecordT>();
  const chunkSize = 400;
  for (let offset = 0; offset < recordIds.length; offset += chunkSize) {
    const chunk = recordIds.slice(offset, offset + chunkSize);
    const result = await databaseClient(database).execute({
      sql: `
        SELECT
          records.record_id,
          records.record_type,
          records.current_revision,
          records.created_at,
          records.is_deleted,
          record_revisions.record_json
        FROM records
        JOIN record_revisions
          ON record_revisions.record_id = records.record_id
         AND record_revisions.revision = records.current_revision
        WHERE records.record_id IN (${chunk.map(() => "?").join(", ")})
      `,
      args: chunk,
    });
    for (const row of result.rows) {
      const record = CurrentRecord.parse({
        record_id: row.record_id,
        record_type: row.record_type,
        current_revision: Number(row.current_revision),
        created_at: row.created_at,
        is_deleted: Number(row.is_deleted) === 1,
        data: JSON.parse(String(row.record_json)) as unknown,
      });
      records.set(record.record_id, record);
    }
  }
  return records;
}

async function localRevisionsById(
  database: AthleticDatabase,
  recordIds: readonly string[],
): Promise<Map<string, RecordRevisionT[]>> {
  const revisions = new Map<string, RecordRevisionT[]>();
  const chunkSize = 400;
  for (let offset = 0; offset < recordIds.length; offset += chunkSize) {
    const chunk = recordIds.slice(offset, offset + chunkSize);
    const result = await databaseClient(database).execute({
      sql: `
        SELECT
          record_id,
          record_type,
          revision,
          previous_revision,
          became_known_at,
          became_known_at_basis,
          changed_by_json,
          record_json
        FROM record_revisions
        WHERE record_id IN (${chunk.map(() => "?").join(", ")})
        ORDER BY record_id, revision
      `,
      args: chunk,
    });
    for (const row of result.rows) {
      const revision = RecordRevision.parse({
        record_id: row.record_id,
        record_type: row.record_type,
        revision: Number(row.revision),
        previous_revision:
          row.previous_revision === null ? null : Number(row.previous_revision),
        became_known_at: row.became_known_at,
        became_known_at_basis: row.became_known_at_basis,
        changed_by: JSON.parse(String(row.changed_by_json)) as unknown,
        data: JSON.parse(String(row.record_json)) as unknown,
      });
      const group = revisions.get(revision.record_id) ?? [];
      group.push(revision);
      revisions.set(revision.record_id, group);
    }
  }
  return revisions;
}

export async function previewPackageImport(
  database: AthleticDatabase,
  loaded: LoadedPackage,
): Promise<PackageImportPreview> {
  const metadata = await readDatabaseMetadata(database);
  const state = await databaseClient(database).execute(
    "SELECT current_revision FROM database_state WHERE singleton_id = 1",
  );
  const destinationRevision = Number(state.rows[0]?.current_revision);
  const localRecordCount = await databaseClient(database).execute(
    "SELECT COUNT(*) AS count FROM records",
  );
  const restore =
    destinationRevision === 0 &&
    Number(localRecordCount.rows[0]?.count) === 0;
  const conflicts: PackageImportConflict[] = [];
  if (!restore && metadata.database_id !== loaded.package.database_id) {
    conflicts.push({
      message:
        `destination database ${metadata.database_id} differs from package ` +
        `${loaded.package.database_id}; live databases are not merged`,
    });
  }
  const importedIds = loaded.all_records.map((record) => record.record_id);
  const localById = await localRecordsById(database, importedIds);
  const localRevisions = await localRevisionsById(
    database,
    [...localById.keys()],
  );
  const importedRevisions = new Map<string, RecordRevisionT[]>();
  for (const revision of loaded.all_revisions) {
    const group = importedRevisions.get(revision.record_id) ?? [];
    group.push(revision);
    importedRevisions.set(revision.record_id, group);
  }
  const newRecords: string[] = [];
  const matchingRecords: string[] = [];
  const changedRecords: string[] = [];
  for (const imported of loaded.all_records) {
    const local = localById.get(imported.record_id);
    if (local === undefined) {
      newRecords.push(imported.record_id);
      continue;
    }
    const currentLocalRevisions = localRevisions.get(imported.record_id) ?? [];
    const incoming = importedRevisions.get(imported.record_id) ?? [];
    const overlap = Math.min(currentLocalRevisions.length, incoming.length);
    let prefixMatches = true;
    for (let index = 0; index < overlap; index++) {
      if (!sameValue(currentLocalRevisions[index], incoming[index])) {
        prefixMatches = false;
        break;
      }
    }
    if (!prefixMatches || incoming.length < currentLocalRevisions.length) {
      conflicts.push({
        record_id: imported.record_id,
        message: "record revision history differs",
      });
      continue;
    }
    if (
      incoming.length === currentLocalRevisions.length &&
      sameValue(local, imported)
    ) {
      matchingRecords.push(imported.record_id);
      continue;
    }
    if (
      incoming.length > currentLocalRevisions.length &&
      incoming.length === imported.current_revision
    ) {
      changedRecords.push(imported.record_id);
      continue;
    }
    conflicts.push({
      record_id: imported.record_id,
      message: "current record does not match its imported revision history",
    });
  }
  return {
    source_database_id: loaded.package.database_id,
    source_database_revision: loaded.package.database_revision,
    destination_database_id: metadata.database_id,
    destination_database_revision: destinationRevision,
    restore,
    new_records: newRecords.sort(),
    matching_records: matchingRecords.sort(),
    changed_records: changedRecords.sort(),
    conflicts,
    unrepresentable_records: [],
    unknown_fields: [],
    loaded,
  };
}

async function insertRevision(
  transaction: Transaction,
  revision: RecordRevisionT,
): Promise<void> {
  await transaction.execute({
    sql: `
      INSERT INTO record_revisions (
        record_id,
        revision,
        record_type,
        record_json,
        became_known_at,
        became_known_at_basis,
        changed_by_json,
        previous_revision
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
    `,
    args: [
      revision.record_id,
      revision.revision,
      revision.record_type,
      JSON.stringify(revision.data),
      revision.became_known_at,
      revision.became_known_at_basis,
      JSON.stringify(revision.changed_by),
      revision.previous_revision,
    ],
  });
}

async function insertCurrentParent(
  transaction: Transaction,
  record: CurrentRecordT,
): Promise<void> {
  await transaction.execute({
    sql: `
      INSERT INTO records (
        record_id,
        record_type,
        current_revision,
        created_at,
        is_deleted
      ) VALUES (?, ?, ?, ?, ?)
    `,
    args: [
      record.record_id,
      record.record_type,
      record.current_revision,
      record.created_at,
      record.is_deleted ? 1 : 0,
    ],
  });
}

async function insertDeletion(
  transaction: Transaction,
  deletion: RecordDeletionT,
): Promise<void> {
  await transaction.execute({
    sql: `
      INSERT INTO record_deletions (
        record_id,
        record_revision,
        deleted_at,
        deleted_by_json
      ) VALUES (?, ?, ?, ?)
    `,
    args: [
      deletion.record_id,
      deletion.record_revision,
      deletion.deleted_at,
      JSON.stringify(deletion.deleted_by),
    ],
  });
}

async function insertImportKey(
  transaction: Transaction,
  importKey: RecordImportKeyT,
): Promise<void> {
  await transaction.execute({
    sql: `
      INSERT INTO record_import_keys (
        record_id,
        match_key_method,
        match_key,
        imported_at
      ) VALUES (?, ?, ?, ?)
    `,
    args: [
      importKey.record_id,
      importKey.match_key_method,
      importKey.match_key,
      importKey.imported_at,
    ],
  });
}

async function insertErasureMarker(
  transaction: Transaction,
  marker: RecordErasureMarkerT,
): Promise<void> {
  await transaction.execute({
    sql: `
      INSERT INTO record_erasure_markers (record_id, erased_at, reason_code)
      VALUES (?, ?, ?)
    `,
    args: [marker.record_id, marker.erased_at, marker.reason_code],
  });
}

async function prepareAttachments(
  database: AthleticDatabase,
  loaded: LoadedPackage,
  resolutions: ReadonlyMap<string, PackageImportResolution>,
): Promise<Array<{ temporary: string; final: string }>> {
  if (database.localPath === undefined || loaded.attachment_files.size === 0) {
    return [];
  }
  const directory = join(dirname(database.localPath), "attachments");
  await mkdir(directory, { recursive: true, mode: 0o700 });
  await refuseSymbolicLinkComponents(directory);
  const prepared: Array<{ temporary: string; final: string }> = [];
  for (const record of loaded.all_records) {
    if (resolutions.get(record.record_id)?.choice === "local") {
      continue;
    }
    if (record.record_type !== "attachment" || record.is_deleted) {
      continue;
    }
    const source = loaded.attachment_files.get(record.record_id);
    if (source === undefined) {
      throw new Error(`attachment bytes are missing: ${record.record_id}`);
    }
    const final = join(directory, storedAttachmentName(record));
    try {
      const existing = await readFile(final);
      if (sha256(existing) !== record.data.sha256) {
        throw new Error(`local attachment conflicts: ${record.record_id}`);
      }
      continue;
    } catch (error) {
      if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) {
        throw error;
      }
    }
    const temporary = `${final}.import-${process.pid}`;
    const file = await openFile(temporary, "wx", 0o600);
    try {
      await file.writeFile(await readFile(source));
    } finally {
      await file.close();
    }
    prepared.push({ temporary, final });
  }
  return prepared;
}

export async function importRecords(
  database: AthleticDatabase,
  preview: PackageImportPreview,
  resolutions: readonly PackageImportResolution[] = [],
): Promise<{ database_revision: number; restored: boolean }> {
  const resolutionById = new Map<string, PackageImportResolution>();
  for (const resolution of resolutions) {
    if (resolutionById.has(resolution.record_id)) {
      throw new Error(`conflict has more than one resolution: ${resolution.record_id}`);
    }
    resolutionById.set(resolution.record_id, resolution);
  }
  const unresolvedConflicts = preview.conflicts.filter(
    (conflict) =>
      conflict.record_id === undefined ||
      !resolutionById.has(conflict.record_id),
  );
  if (
    unresolvedConflicts.length > 0 ||
    preview.unrepresentable_records.length > 0 ||
    preview.unknown_fields.length > 0
  ) {
    throw new Error("import has unresolved conflicts or unsupported records");
  }
  for (const recordId of resolutionById.keys()) {
    if (!preview.conflicts.some((conflict) => conflict.record_id === recordId)) {
      throw new Error(`resolution does not match an import conflict: ${recordId}`);
    }
  }
  const preparedAttachments = await prepareAttachments(
    database,
    preview.loaded,
    resolutionById,
  );
  const movedAttachments: string[] = [];
  const resolutionTime = new Date().toISOString();
  try {
    const result = await withWriteTransaction(database, async (transaction) => {
      const state = await transaction.execute(
        "SELECT current_revision FROM database_state WHERE singleton_id = 1",
      );
      if (
        Number(state.rows[0]?.current_revision) !==
        preview.destination_database_revision
      ) {
        throw new Error("destination database changed after preview");
      }
      const metadata = await transaction.execute(
        "SELECT database_id FROM database_metadata WHERE singleton_id = 1",
      );
      if (
        metadata.rows[0]?.database_id !== preview.destination_database_id
      ) {
        throw new Error("destination database changed after preview");
      }

      const newIds = new Set(preview.new_records);
      const changedIds = new Set(preview.changed_records);
      const resolvedRecords: CurrentRecordT[] = [];

      if (preview.restore) {
        for (const record of preview.loaded.all_records) {
          await insertCurrentParent(transaction, record);
        }
        for (const revision of preview.loaded.all_revisions) {
          await insertRevision(transaction, revision);
        }
        for (const record of preview.loaded.all_records) {
          await upsertTypedRecord(transaction, record);
        }
        for (const record of preview.loaded.all_records) {
          await validateTypedRelationships(transaction, record);
        }
        for (const deletion of preview.loaded.all_deletions) {
          await insertDeletion(transaction, deletion);
        }
        for (const importKey of preview.loaded.all_import_keys) {
          await insertImportKey(transaction, importKey);
        }
        for (const marker of preview.loaded.package.record_erasure_markers) {
          await insertErasureMarker(transaction, marker);
        }
        await transaction.execute({
          sql: "UPDATE database_metadata SET database_id = ? WHERE singleton_id = 1",
          args: [preview.source_database_id],
        });
        await transaction.execute({
          sql: "UPDATE database_state SET current_revision = ? WHERE singleton_id = 1",
          args: [preview.source_database_revision],
        });
      } else {
        for (const record of preview.loaded.all_records) {
          if (newIds.has(record.record_id)) {
            await insertCurrentParent(transaction, record);
          }
        }
        for (const revision of preview.loaded.all_revisions) {
          if (newIds.has(revision.record_id)) {
            await insertRevision(transaction, revision);
          } else if (changedIds.has(revision.record_id)) {
            const existing = await transaction.execute({
              sql: `
                SELECT 1 AS present
                FROM record_revisions
                WHERE record_id = ? AND revision = ?
              `,
              args: [revision.record_id, revision.revision],
            });
            if (existing.rows.length === 0) {
              await insertRevision(transaction, revision);
            }
          }
        }
        for (const record of preview.loaded.all_records) {
          if (newIds.has(record.record_id) || changedIds.has(record.record_id)) {
            await upsertTypedRecord(transaction, record);
            await transaction.execute({
              sql: `
                UPDATE records
                SET current_revision = ?, is_deleted = ?
                WHERE record_id = ?
              `,
              args: [
                record.current_revision,
                record.is_deleted ? 1 : 0,
                record.record_id,
              ],
            });
          }
        }
        for (const resolution of resolutionById.values()) {
          if (resolution.choice === "local") {
            continue;
          }
          const imported = preview.loaded.all_records.find(
            (record) => record.record_id === resolution.record_id,
          );
          if (imported === undefined) {
            throw new Error(
              `imported conflict record is missing: ${resolution.record_id}`,
            );
          }
          const currentResult = await transaction.execute({
            sql: `
              SELECT
                records.record_type,
                records.current_revision,
                records.created_at,
                records.is_deleted,
                record_revisions.became_known_at
              FROM records
              JOIN record_revisions
                ON record_revisions.record_id = records.record_id
               AND record_revisions.revision = records.current_revision
              WHERE records.record_id = ?
            `,
            args: [resolution.record_id],
          });
          const current = currentResult.rows[0];
          if (current === undefined) {
            throw new Error(
              `local conflict record is missing: ${resolution.record_id}`,
            );
          }
          if (
            current.record_type !== imported.record_type ||
            Number(current.is_deleted) !== 0 ||
            imported.is_deleted
          ) {
            throw new Error(
              `conflict cannot change record type or deleted state: ${resolution.record_id}`,
            );
          }
          if (
            Date.parse(resolutionTime) <
            Date.parse(String(current.became_known_at))
          ) {
            throw new Error(
              `conflict resolution time precedes the current revision: ${resolution.record_id}`,
            );
          }
          const currentRevision = Number(current.current_revision);
          const resolvedRecord = CurrentRecord.parse({
            record_id: resolution.record_id,
            record_type: current.record_type,
            created_at: current.created_at,
            current_revision: currentRevision + 1,
            is_deleted: false,
            data:
              resolution.choice === "imported"
                ? imported.data
                : resolution.data,
          });
          const revision = RecordRevision.parse({
            record_id: resolvedRecord.record_id,
            record_type: resolvedRecord.record_type,
            revision: resolvedRecord.current_revision,
            previous_revision: currentRevision,
            became_known_at: resolutionTime,
            became_known_at_basis: "package_merge",
            changed_by: { author_type: "athlete" },
            data: resolvedRecord.data,
          });
          await insertRevision(transaction, revision);
          await upsertTypedRecord(transaction, resolvedRecord);
          await transaction.execute({
            sql: "UPDATE records SET current_revision = ? WHERE record_id = ?",
            args: [resolvedRecord.current_revision, resolvedRecord.record_id],
          });
          resolvedRecords.push(resolvedRecord);
        }
        for (const record of preview.loaded.all_records) {
          if (newIds.has(record.record_id) || changedIds.has(record.record_id)) {
            await validateTypedRelationships(transaction, record);
          }
        }
        for (const record of resolvedRecords) {
          await validateTypedRelationships(transaction, record);
        }
        for (const deletion of preview.loaded.all_deletions) {
          if (resolutionById.has(deletion.record_id)) {
            continue;
          }
          const existing = await transaction.execute({
            sql: "SELECT 1 AS present FROM record_deletions WHERE record_id = ?",
            args: [deletion.record_id],
          });
          if (existing.rows.length === 0) {
            await insertDeletion(transaction, deletion);
          }
        }
        for (const importKey of preview.loaded.all_import_keys) {
          if (resolutionById.get(importKey.record_id)?.choice === "local") {
            continue;
          }
          const existing = await transaction.execute({
            sql: `
              SELECT record_id
              FROM record_import_keys
              WHERE match_key_method = ? AND match_key = ?
            `,
            args: [importKey.match_key_method, importKey.match_key],
          });
          if (existing.rows.length === 0) {
            await insertImportKey(transaction, importKey);
          } else if (existing.rows[0]?.record_id !== importKey.record_id) {
            throw new Error("provider import key changed after preview");
          }
        }
        for (const marker of preview.loaded.package.record_erasure_markers) {
          const existing = await transaction.execute({
            sql: `
              SELECT erased_at, reason_code
              FROM record_erasure_markers
              WHERE record_id = ?
            `,
            args: [marker.record_id],
          });
          if (existing.rows.length === 0) {
            await insertErasureMarker(transaction, marker);
          } else if (
            existing.rows[0]?.erased_at !== marker.erased_at ||
            existing.rows[0]?.reason_code !== marker.reason_code
          ) {
            throw new Error("erasure marker changed after preview");
          }
        }
        await incrementDatabaseRevision(transaction);
      }

      for (const attachment of preparedAttachments) {
        await rename(attachment.temporary, attachment.final);
        movedAttachments.push(attachment.final);
      }
      const finalState = await transaction.execute(
        "SELECT current_revision FROM database_state WHERE singleton_id = 1",
      );
      return Number(finalState.rows[0]?.current_revision);
    });
    return { database_revision: result, restored: preview.restore };
  } catch (error) {
    await Promise.all([
      ...preparedAttachments.map((attachment) =>
        rm(attachment.temporary, { force: true }),
      ),
      ...movedAttachments.map((path) => rm(path, { force: true })),
    ]);
    throw error;
  }
}
