import { createHash } from "node:crypto";
import { lstat, mkdir, readFile, rm } from "node:fs/promises";
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
import {
  currentRecordPayloadFromRow,
  recordRevisionPayloadFromRow,
} from "./revision-encoding.js";
import { writeOwnerOnlyDurable } from "./durable-write.js";
import { compareUtf8Keys, deterministicJson } from "./stable-json.js";

export interface ExportDatabaseOptions {
  exported_at?: string;
}

export interface ExportDatabaseResult {
  directory: string;
  database_id: string;
  database_revision: number;
  manifest: ExportManifestT;
}

export { deterministicJson } from "./stable-json.js";

function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function parseCurrentRow(row: Record<string, unknown>): CurrentRecordT {
  return CurrentRecord.parse(currentRecordPayloadFromRow(row));
}

function parseRevisionRow(row: Record<string, unknown>): RecordRevisionT {
  return RecordRevision.parse(recordRevisionPayloadFromRow(row));
}

function parseDeletionRow(row: Record<string, unknown>): RecordDeletionT {
  return RecordDeletion.parse({
    record_id: row.record_id,
    record_revision: Number(row.record_revision),
    deleted_at: row.deleted_at,
    deleted_by: JSON.parse(String(row.deleted_by_json)) as unknown,
  });
}

function parseImportKeyRow(row: Record<string, unknown>): RecordImportKeyT {
  return RecordImportKey.parse({
    record_id: row.record_id,
    match_key_method: row.match_key_method,
    match_key: row.match_key,
    imported_at: row.imported_at,
  });
}

async function readCurrentRecords(
  transaction: Transaction,
  includeSamples = false,
): Promise<CurrentRecordT[]> {
  const result = await transaction.execute({
    sql: `
    SELECT
      records.record_id,
      records.record_type,
      records.current_revision,
      records.created_at,
      records.is_deleted,
      record_revisions.record_json,
      record_revisions.extensions_json
    FROM records
    JOIN record_revisions
      ON record_revisions.record_id = records.record_id
     AND record_revisions.revision = records.current_revision
    WHERE (? = 1 OR records.record_type <> 'measurement_sample')
    ORDER BY records.record_id
  `,
    args: [includeSamples ? 1 : 0],
  });
  return result.rows.map((row) => parseCurrentRow(row));
}

async function readRevisions(
  transaction: Transaction,
  includeSamples = false,
): Promise<RecordRevisionT[]> {
  const result = await transaction.execute({
    sql: `
    SELECT
      record_id,
      record_type,
      revision,
      previous_revision,
      became_known_at,
      became_known_at_basis,
      changed_by_json,
      record_json,
      extensions_json
    FROM record_revisions
    WHERE (? = 1 OR record_type <> 'measurement_sample')
    ORDER BY record_id, revision
  `,
    args: [includeSamples ? 1 : 0],
  });
  return result.rows.map((row) => parseRevisionRow(row));
}

async function readDeletions(
  transaction: Transaction,
  includeSamples = false,
): Promise<RecordDeletionT[]> {
  const result = await transaction.execute({
    sql: `
      SELECT
        record_deletions.record_id,
        record_deletions.record_revision,
        record_deletions.deleted_at,
        record_deletions.deleted_by_json
      FROM record_deletions
      JOIN records ON records.record_id = record_deletions.record_id
      WHERE (? = 1 OR records.record_type <> 'measurement_sample')
      ORDER BY record_deletions.record_id
    `,
    args: [includeSamples ? 1 : 0],
  });
  return result.rows.map((row) => parseDeletionRow(row));
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
  includeSamples = false,
): Promise<RecordImportKeyT[]> {
  const result = await transaction.execute({
    sql: `
      SELECT
        record_import_keys.record_id,
        record_import_keys.match_key_method,
        record_import_keys.match_key,
        record_import_keys.imported_at
      FROM record_import_keys
      JOIN records ON records.record_id = record_import_keys.record_id
      WHERE (? = 1 OR records.record_type <> 'measurement_sample')
      ORDER BY
        record_import_keys.record_id,
        record_import_keys.match_key_method,
        record_import_keys.match_key
    `,
    args: [includeSamples ? 1 : 0],
  });
  return result.rows.map((row) => parseImportKeyRow(row));
}

async function recordCounts(transaction: Transaction) {
  const current = await transaction.execute(`
    SELECT record_type, COUNT(*) AS count
    FROM records
    GROUP BY record_type
  `);
  const revisions = await transaction.execute(`
    SELECT record_type, COUNT(*) AS count
    FROM record_revisions
    GROUP BY record_type
  `);
  const deletions = await transaction.execute(`
    SELECT records.record_type, COUNT(*) AS count
    FROM record_deletions
    JOIN records ON records.record_id = record_deletions.record_id
    GROUP BY records.record_type
  `);
  const importKeys = await transaction.execute(`
    SELECT records.record_type, COUNT(*) AS count
    FROM record_import_keys
    JOIN records ON records.record_id = record_import_keys.record_id
    GROUP BY records.record_type
  `);
  const countMap = (rows: typeof current.rows) =>
    new Map(
      rows.map((row) => [String(row.record_type), Number(row.count)]),
    );
  const currentCounts = countMap(current.rows);
  const revisionCounts = countMap(revisions.rows);
  const deletionCounts = countMap(deletions.rows);
  const importKeyCounts = countMap(importKeys.rows);
  return RecordType.options.map((recordType) => {
    return {
      record_type: recordType,
      current_record_count: currentCounts.get(recordType) ?? 0,
      revision_count: revisionCounts.get(recordType) ?? 0,
      deletion_count: deletionCounts.get(recordType) ?? 0,
      import_key_count: importKeyCounts.get(recordType) ?? 0,
    };
  });
}

const SERIES_CHUNK_RECORDS = 50_000;

async function readSampleChunk(
  transaction: Transaction,
  sourceId: string,
  quantity: string,
  afterAt: string,
  afterId: string,
): Promise<Array<Extract<CurrentRecordT, { record_type: "measurement_sample" }>>> {
  const result = await transaction.execute({
    sql: `
      SELECT
        records.record_id,
        records.record_type,
        records.current_revision,
        records.created_at,
        records.is_deleted,
        record_revisions.record_json,
        record_revisions.extensions_json
      FROM measurement_samples
      JOIN records ON records.record_id = measurement_samples.record_id
      JOIN record_revisions
        ON record_revisions.record_id = records.record_id
       AND record_revisions.revision = records.current_revision
      WHERE measurement_samples.source_id = ?
        AND measurement_samples.quantity = ?
        AND (
          measurement_samples.recorded_at > ? OR
          (
            measurement_samples.recorded_at = ? AND
            records.record_id > ?
          )
        )
      ORDER BY measurement_samples.recorded_at, records.record_id
      LIMIT ?
    `,
    args: [
      sourceId,
      quantity,
      afterAt,
      afterAt,
      afterId,
      SERIES_CHUNK_RECORDS,
    ],
  });
  return result.rows.map((row) => parseCurrentRow(row)) as Array<
    Extract<CurrentRecordT, { record_type: "measurement_sample" }>
  >;
}

function sampleRangeWhere(): string {
  return `
    measurement_samples.source_id = ? AND
    measurement_samples.quantity = ? AND
    (
      measurement_samples.recorded_at > ? OR
      (
        measurement_samples.recorded_at = ? AND
        measurement_samples.record_id >= ?
      )
    ) AND
    (
      measurement_samples.recorded_at < ? OR
      (
        measurement_samples.recorded_at = ? AND
        measurement_samples.record_id <= ?
      )
    )
  `;
}

function sampleRangeArgs(
  sourceId: string,
  quantity: string,
  first: Extract<CurrentRecordT, { record_type: "measurement_sample" }>,
  last: Extract<CurrentRecordT, { record_type: "measurement_sample" }>,
): Array<string> {
  return [
    sourceId,
    quantity,
    first.data.recorded_at,
    first.data.recorded_at,
    first.record_id,
    last.data.recorded_at,
    last.data.recorded_at,
    last.record_id,
  ];
}

async function readSampleChunkHistory(
  transaction: Transaction,
  sourceId: string,
  quantity: string,
  first: Extract<CurrentRecordT, { record_type: "measurement_sample" }>,
  last: Extract<CurrentRecordT, { record_type: "measurement_sample" }>,
): Promise<{
  revisions: RecordRevisionT[];
  deletions: RecordDeletionT[];
  importKeys: RecordImportKeyT[];
}> {
  const args = sampleRangeArgs(sourceId, quantity, first, last);
  const revisions = await transaction.execute({
    sql: `
      SELECT
        record_revisions.record_id,
        record_revisions.record_type,
        record_revisions.revision,
        record_revisions.previous_revision,
        record_revisions.became_known_at,
        record_revisions.became_known_at_basis,
        record_revisions.changed_by_json,
        record_revisions.record_json,
        record_revisions.extensions_json
      FROM measurement_samples
      JOIN record_revisions
        ON record_revisions.record_id = measurement_samples.record_id
      WHERE ${sampleRangeWhere()}
      ORDER BY record_revisions.record_id, record_revisions.revision
    `,
    args,
  });
  const deletions = await transaction.execute({
    sql: `
      SELECT
        record_deletions.record_id,
        record_deletions.record_revision,
        record_deletions.deleted_at,
        record_deletions.deleted_by_json
      FROM measurement_samples
      JOIN record_deletions
        ON record_deletions.record_id = measurement_samples.record_id
      WHERE ${sampleRangeWhere()}
      ORDER BY record_deletions.record_id
    `,
    args,
  });
  const importKeys = await transaction.execute({
    sql: `
      SELECT
        record_import_keys.record_id,
        record_import_keys.match_key_method,
        record_import_keys.match_key,
        record_import_keys.imported_at
      FROM measurement_samples
      JOIN record_import_keys
        ON record_import_keys.record_id = measurement_samples.record_id
      WHERE ${sampleRangeWhere()}
      ORDER BY
        record_import_keys.record_id,
        record_import_keys.match_key_method,
        record_import_keys.match_key
    `,
    args,
  });
  return {
    revisions: revisions.rows.map((row) => parseRevisionRow(row)),
    deletions: deletions.rows.map((row) => parseDeletionRow(row)),
    importKeys: importKeys.rows.map((row) => parseImportKeyRow(row)),
  };
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
    const seriesDirectory = join(outputDirectory, "series");
    const attachmentDirectory = join(outputDirectory, "attachments");
    const files: Array<{ file: string; sha256: string; byte_count: number }> = [];
    const seriesReferences: Array<{
      file: string;
      sha256: string;
      source_id: string;
      quantity: MeasurementSeriesFileT["quantity"];
      start_at: string;
      end_at: string;
      sample_count: number;
      revision_count: number;
      deletion_count: number;
      import_key_count: number;
    }> = [];
    const groups = await transaction.execute(`
      SELECT source_id, quantity
      FROM measurement_samples
      GROUP BY source_id, quantity
      ORDER BY source_id, quantity
    `);
    if (groups.rows.length > 0) {
      await mkdir(seriesDirectory, { mode: 0o700 });
    }
    for (const groupRow of groups.rows) {
      const sourceId = String(groupRow.source_id);
      const quantity = String(groupRow.quantity);
      let afterAt = "";
      let afterId = "";
      let chunkNumber = 0;
      while (true) {
        const chunk = await readSampleChunk(
          transaction,
          sourceId,
          quantity,
          afterAt,
          afterId,
        );
        if (chunk.length === 0) {
          break;
        }
        chunkNumber++;
        const first = chunk[0]!;
        const last = chunk.at(-1)!;
        const history = await readSampleChunkHistory(
          transaction,
          sourceId,
          quantity,
          first,
          last,
        );
        const relativePath =
          `series/${sourceId}-${quantity}-` +
          `${String(chunkNumber).padStart(6, "0")}.ath-series.json`;
        const series = MeasurementSeriesFile.parse({
          athleticstandard_version: ATHLETIC_STANDARD_VERSION,
          source_id: sourceId,
          quantity,
          start_at: first.data.recorded_at,
          end_at: last.data.recorded_at,
          records: chunk,
          record_revisions: history.revisions,
          record_deletions: history.deletions,
          record_import_keys: history.importKeys,
        });
        const bytes = deterministicJson(series);
        await writeOwnerOnlyDurable(join(outputDirectory, relativePath), bytes);
        const hash = sha256(bytes);
        files.push({
          file: relativePath,
          sha256: hash,
          byte_count: bytes.byteLength,
        });
        seriesReferences.push({
          file: relativePath,
          sha256: hash,
          source_id: sourceId,
          quantity: series.quantity,
          start_at: series.start_at,
          end_at: series.end_at,
          sample_count: series.records.length,
          revision_count: series.record_revisions.length,
          deletion_count: series.record_deletions.length,
          import_key_count: series.record_import_keys.length,
        });
        afterAt = last.data.recorded_at;
        afterId = last.record_id;
      }
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
      compareUtf8Keys(left.record_id, right.record_id),
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
      await writeOwnerOnlyDurable(join(outputDirectory, relativePath), bytes);
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
      records,
      record_revisions: revisions,
      record_deletions: deletions,
      record_erasure_markers: erasureMarkers,
      record_import_keys: importKeys,
      measurement_series_files: seriesReferences,
    });
    const packageBytes = deterministicJson(packageDocument);
    const packagePath = join(outputDirectory, "athlete.ath.json");
    await writeOwnerOnlyDurable(packagePath, packageBytes);
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
      files: files.sort((left, right) => compareUtf8Keys(left.file, right.file)),
      record_counts: await recordCounts(transaction),
      erasure_marker_count: erasureMarkers.length,
    });
    const manifestBytes = deterministicJson(manifest);
    await writeOwnerOnlyDurable(
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
