import { createHash } from "node:crypto";
import {
  lstat,
  mkdir,
  open as openFile,
  readFile,
  rm,
} from "node:fs/promises";
import { dirname, join } from "node:path";
import type { Transaction } from "@libsql/client";
import {
  ATHLETIC_STANDARD_VERSION,
  AthleticStandardPackage,
  CurrentRecord,
  ExportManifest,
  MeasurementSeriesFile,
  RecordDeletion,
  RecordErasureMarker,
  RecordImportKey,
  RecordRevision,
  RecordType,
  UtcTimestamp,
  type CurrentRecordT,
  type ExportManifestT,
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

export interface ExportDatabaseOptions {
  exported_at?: string;
}

export interface ExportDatabaseResult {
  directory: string;
  database_id: string;
  database_revision: number;
  manifest: ExportManifestT;
}

function stableValue(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(stableValue);
  }
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, child]) => [key, stableValue(child)]),
    );
  }
  return value;
}

export function deterministicJson(value: unknown): Buffer {
  return Buffer.from(`${JSON.stringify(stableValue(value), null, 2)}\n`, "utf8");
}

function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

async function writeOwnerOnly(path: string, bytes: Uint8Array): Promise<void> {
  const file = await openFile(path, "wx", 0o600);
  try {
    await file.writeFile(bytes);
  } finally {
    await file.close();
  }
}

function parseCurrentRow(row: Record<string, unknown>): CurrentRecordT {
  return CurrentRecord.parse({
    record_id: row.record_id,
    record_type: row.record_type,
    current_revision: Number(row.current_revision),
    created_at: row.created_at,
    is_deleted: Number(row.is_deleted) === 1,
    data: JSON.parse(String(row.record_json)) as unknown,
  });
}

async function readCurrentRecords(
  transaction: Transaction,
): Promise<CurrentRecordT[]> {
  const result = await transaction.execute(`
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
    ORDER BY records.record_id
  `);
  return result.rows.map((row) => parseCurrentRow(row));
}

async function readRevisions(
  transaction: Transaction,
): Promise<RecordRevisionT[]> {
  const result = await transaction.execute(`
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
    ORDER BY record_id, revision
  `);
  return result.rows.map((row) =>
    RecordRevision.parse({
      record_id: row.record_id,
      record_type: row.record_type,
      revision: Number(row.revision),
      previous_revision:
        row.previous_revision === null ? null : Number(row.previous_revision),
      became_known_at: row.became_known_at,
      became_known_at_basis: row.became_known_at_basis,
      changed_by: JSON.parse(String(row.changed_by_json)) as unknown,
      data: JSON.parse(String(row.record_json)) as unknown,
    }),
  );
}

async function readDeletions(
  transaction: Transaction,
): Promise<RecordDeletionT[]> {
  const result = await transaction.execute(`
    SELECT record_id, record_revision, deleted_at, deleted_by_json
    FROM record_deletions
    ORDER BY record_id
  `);
  return result.rows.map((row) =>
    RecordDeletion.parse({
      record_id: row.record_id,
      record_revision: Number(row.record_revision),
      deleted_at: row.deleted_at,
      deleted_by: JSON.parse(String(row.deleted_by_json)) as unknown,
    }),
  );
}

async function readErasureMarkers(
  transaction: Transaction,
): Promise<RecordErasureMarkerT[]> {
  const result = await transaction.execute(`
    SELECT record_id, erased_at, reason_code
    FROM record_erasure_markers
    ORDER BY record_id
  `);
  return result.rows.map((row) =>
    RecordErasureMarker.parse({
      record_id: row.record_id,
      erased_at: row.erased_at,
      reason_code: row.reason_code,
    }),
  );
}

async function readImportKeys(
  transaction: Transaction,
): Promise<RecordImportKeyT[]> {
  const result = await transaction.execute(`
    SELECT record_id, match_key_method, match_key, imported_at
    FROM record_import_keys
    ORDER BY record_id, match_key_method, match_key
  `);
  return result.rows.map((row) =>
    RecordImportKey.parse({
      record_id: row.record_id,
      match_key_method: row.match_key_method,
      match_key: row.match_key,
      imported_at: row.imported_at,
    }),
  );
}

function recordCounts(
  records: readonly CurrentRecordT[],
  revisions: readonly RecordRevisionT[],
  deletions: readonly RecordDeletionT[],
  importKeys: readonly RecordImportKeyT[],
) {
  return RecordType.options.map((recordType) => {
    const ids = new Set(
      records
        .filter((record) => record.record_type === recordType)
        .map((record) => record.record_id),
    );
    return {
      record_type: recordType,
      current_record_count: ids.size,
      revision_count: revisions.filter(
        (revision) => revision.record_type === recordType,
      ).length,
      deletion_count: deletions.filter((deletion) => ids.has(deletion.record_id))
        .length,
      import_key_count: importKeys.filter((key) => ids.has(key.record_id)).length,
    };
  });
}

export async function exportRecords(
  database: AthleticDatabase,
  outputDirectory: string,
  options: ExportDatabaseOptions = {},
): Promise<ExportDatabaseResult> {
  if (database.localPath === undefined) {
    throw new Error("attachment export requires a local Athletic Standard database");
  }
  const exportedAt =
    options.exported_at === undefined
      ? undefined
      : UtcTimestamp.parse(options.exported_at);
  await refuseSymbolicLinkComponents(dirname(outputDirectory));
  try {
    const existing = await lstat(outputDirectory);
    if (existing.isSymbolicLink()) {
      throw new Error(`refusing export through a symbolic link: ${outputDirectory}`);
    }
    throw new Error(`export directory already exists: ${outputDirectory}`);
  } catch (error) {
    if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) {
      throw error;
    }
  }
  await mkdir(outputDirectory, { mode: 0o700 });

  const transaction = await databaseClient(database).transaction("read");
  try {
    const metadataResult = await transaction.execute(`
      SELECT athleticstandard_version, database_id
      FROM database_metadata
      WHERE singleton_id = 1
    `);
    const stateResult = await transaction.execute(`
      SELECT current_revision
      FROM database_state
      WHERE singleton_id = 1
    `);
    const metadata = metadataResult.rows[0];
    if (
      metadata?.athleticstandard_version !== ATHLETIC_STANDARD_VERSION ||
      typeof metadata.database_id !== "string"
    ) {
      throw new Error("database metadata is invalid during export");
    }
    const databaseRevision = Number(stateResult.rows[0]?.current_revision);
    const records = await readCurrentRecords(transaction);
    const revisions = await readRevisions(transaction);
    const deletions = await readDeletions(transaction);
    const erasureMarkers = await readErasureMarkers(transaction);
    const importKeys = await readImportKeys(transaction);
    const sampleRecords = records.filter(
      (
        record,
      ): record is Extract<CurrentRecordT, { record_type: "measurement_sample" }> =>
        record.record_type === "measurement_sample",
    );
    const sampleIds = new Set(sampleRecords.map((record) => record.record_id));
    const sampleRevisions = revisions.filter(
      (revision) => revision.record_type === "measurement_sample",
    );
    const sampleDeletions = deletions.filter((deletion) =>
      sampleIds.has(deletion.record_id),
    );
    const sampleImportKeys = importKeys.filter((key) => sampleIds.has(key.record_id));
    const seriesDirectory = join(outputDirectory, "series");
    const attachmentDirectory = join(outputDirectory, "attachments");
    const files: Array<{ file: string; sha256: string; byte_count: number }> = [];
    const seriesReferences: MeasurementSeriesFileT[] = [];
    const groups = new Map<string, typeof sampleRecords>();
    for (const record of sampleRecords) {
      const key = `${record.data.source_id}\u0000${record.data.quantity}`;
      const group = groups.get(key) ?? [];
      group.push(record);
      groups.set(key, group);
    }
    if (groups.size > 0) {
      await mkdir(seriesDirectory, { mode: 0o700 });
    }
    for (const [key, group] of [...groups.entries()].sort(([left], [right]) =>
      left.localeCompare(right),
    )) {
      group.sort(
        (left, right) =>
          left.data.recorded_at.localeCompare(right.data.recorded_at) ||
          left.record_id.localeCompare(right.record_id),
      );
      const [sourceId, quantity] = key.split("\u0000") as [string, string];
      const groupIds = new Set(group.map((record) => record.record_id));
      const groupRevisions = sampleRevisions.filter((revision) =>
        groupIds.has(revision.record_id),
      );
      const groupDeletions = sampleDeletions.filter((deletion) =>
        groupIds.has(deletion.record_id),
      );
      const groupImportKeys = sampleImportKeys.filter((importKey) =>
        groupIds.has(importKey.record_id),
      );
      const first = group[0]!;
      const last = group.at(-1)!;
      const relativePath = `series/${sourceId}-${quantity}.ath-series.json`;
      const series = MeasurementSeriesFile.parse({
        athleticstandard_version: ATHLETIC_STANDARD_VERSION,
        source_id: sourceId,
        quantity,
        start_at: first.data.recorded_at,
        end_at: last.data.recorded_at,
        records: group,
        record_revisions: groupRevisions,
        record_deletions: groupDeletions,
        record_import_keys: groupImportKeys,
      });
      const bytes = deterministicJson(series);
      await writeOwnerOnly(join(outputDirectory, relativePath), bytes);
      files.push({
        file: relativePath,
        sha256: sha256(bytes),
        byte_count: bytes.byteLength,
      });
      seriesReferences.push(series);
    }

    const attachments = records.filter(
      (
        record,
      ): record is Extract<CurrentRecordT, { record_type: "attachment" }> =>
        record.record_type === "attachment" && !record.is_deleted,
    );
    if (attachments.length > 0) {
      await mkdir(attachmentDirectory, { mode: 0o700 });
    }
    for (const attachment of attachments.sort((left, right) =>
      left.record_id.localeCompare(right.record_id),
    )) {
      const storedName = storedAttachmentName(attachment);
      const sourcePath = join(
        dirname(database.localPath),
        "attachments",
        storedName,
      );
      const bytes = await readFile(sourcePath);
      assertAttachmentFileType(attachment, bytes);
      if (
        bytes.byteLength !== attachment.data.byte_count ||
        sha256(bytes) !== attachment.data.sha256
      ) {
        throw new Error(`attachment does not match its record: ${attachment.record_id}`);
      }
      const relativePath = `attachments/${storedName}`;
      await writeOwnerOnly(join(outputDirectory, relativePath), bytes);
      files.push({
        file: relativePath,
        sha256: attachment.data.sha256,
        byte_count: bytes.byteLength,
      });
    }

    const packageDocument = AthleticStandardPackage.parse({
      athleticstandard_version: ATHLETIC_STANDARD_VERSION,
      database_id: metadata.database_id,
      database_revision: databaseRevision,
      records: records.filter(
        (record) => record.record_type !== "measurement_sample",
      ),
      record_revisions: revisions.filter(
        (revision) => revision.record_type !== "measurement_sample",
      ),
      record_deletions: deletions.filter(
        (deletion) => !sampleIds.has(deletion.record_id),
      ),
      record_erasure_markers: erasureMarkers,
      record_import_keys: importKeys.filter((key) => !sampleIds.has(key.record_id)),
      measurement_series_files: seriesReferences.map((series, index) => {
        const file = files[index]!;
        return {
          file: file.file,
          sha256: file.sha256,
          source_id: series.source_id,
          quantity: series.quantity,
          start_at: series.start_at,
          end_at: series.end_at,
          sample_count: series.records.length,
          revision_count: series.record_revisions.length,
          deletion_count: series.record_deletions.length,
          import_key_count: series.record_import_keys.length,
        };
      }),
    });
    const packageBytes = deterministicJson(packageDocument);
    const packagePath = join(outputDirectory, "athlete.ath.json");
    await writeOwnerOnly(packagePath, packageBytes);
    files.push({
      file: "athlete.ath.json",
      sha256: sha256(packageBytes),
      byte_count: packageBytes.byteLength,
    });

    const manifest = ExportManifest.parse({
      athleticstandard_version: ATHLETIC_STANDARD_VERSION,
      source_database_id: metadata.database_id,
      database_revision: databaseRevision,
      ...(exportedAt === undefined ? {} : { exported_at: exportedAt }),
      files: files.sort((left, right) => left.file.localeCompare(right.file)),
      record_counts: recordCounts(records, revisions, deletions, importKeys),
      erasure_marker_count: erasureMarkers.length,
    });
    const manifestBytes = deterministicJson(manifest);
    await writeOwnerOnly(
      join(outputDirectory, "export-manifest.json"),
      manifestBytes,
    );
    await transaction.commit();
    return {
      directory: outputDirectory,
      database_id: metadata.database_id,
      database_revision: databaseRevision,
      manifest,
    };
  } catch (error) {
    if (!transaction.closed) {
      await transaction.rollback();
    }
    await rm(outputDirectory, { recursive: true, force: true });
    throw error;
  } finally {
    transaction.close();
  }
}
