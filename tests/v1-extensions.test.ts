import { describe, expect, it, afterAll } from "vitest";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  createDatabase,
  exportRecords,
  importRecords,
  loadAthleticStandardPackage,
  previewPackageImport,
  readRecord,
  writeRecord,
  type AthleticDatabase,
} from "../src/db/index.js";
import { AthleteReportRecord } from "../src/v1/schema.js";

const databaseId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f01";
const sourceId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e02";
const pointId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e03";
const reportId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e04";
const createdAt = "2026-08-09T12:00:00.000Z";

const timestampReceipt = {
  timestamp_text: "2026-08-09T06:12:00-05:00",
  utc_offset_text: "-05:00",
  utc_offset_meaning: "provider_cycle_offset" as const,
  timestamp_basis: "source_offset_timestamp" as const,
};

const toolActor = {
  author_type: "tool" as const,
  tool: "ath",
  tool_version: "1.0.0",
};

const openDatabases: AthleticDatabase[] = [];

async function tempDatabasePath(): Promise<string> {
  const directory = await mkdtemp(join(tmpdir(), "ath-v1-ext-"));
  return join(directory, "athlete.ath.sqlite");
}

async function newDatabase(): Promise<AthleticDatabase> {
  const path = await tempDatabasePath();
  const database = await createDatabase(path, { databaseId, createdAt });
  openDatabases.push(database);
  return database;
}

describe("v1 record extensions", () => {
  afterAll(async () => {
    for (const database of openDatabases) {
      await database.close();
    }
  });

  it("rejects athlete-reported extensions that contain source_id", () => {
    const parsed = AthleteReportRecord.safeParse({
      record_id: reportId,
      record_type: "athlete_report",
      created_at: createdAt,
      current_revision: 1,
      is_deleted: false,
      data: {
        report_type: "soreness",
        reported_at: createdAt,
        input_reported_at: timestampReceipt,
        reported_via: "text",
      },
      extensions: {
        "ath.fit/demo-widget": { source_id: sourceId },
      },
    });
    expect(parsed.success).toBe(false);
  });

  it("preserves extensions through SQLite write, read, export, and import", async () => {
    const database = await newDatabase();
    await writeRecord(database, {
      record_type: "source",
      record_id: sourceId,
      created_at: createdAt,
      changed_by: toolActor,
      data: {
        source_type: "wearable",
        data_writer: "WHOOP",
        data_route: "whoop_csv",
        vendor: "whoop",
      },
    });
    const extensions = {
      "ath.fit/demo-widget": { note: "synthetic", count: 2 },
    };
    await writeRecord(database, {
      record_type: "point_measurement",
      record_id: pointId,
      created_at: createdAt,
      became_known_at: createdAt,
      became_known_at_basis: "provider_import",
      changed_by: toolActor,
      extensions,
      data: {
        source_id: sourceId,
        received_at: createdAt,
        received_at_basis: "import_time",
        quantity: "hrv_rmssd",
        recorded_at: "2026-08-09T11:12:00.000Z",
        input_timestamp: timestampReceipt,
        value: {
          standard_value: 62,
          standard_unit: "ms",
          input_value: 62,
          input_unit: "ms",
        },
      },
    });

    const readBack = await readRecord(database, pointId);
    expect(readBack?.extensions).toEqual(extensions);

    const exportRoot = await mkdtemp(join(tmpdir(), "ath-v1-ext-export-"));
    const exportDir = join(exportRoot, "package");
    await exportRecords(database, exportDir);
    const importedPath = await tempDatabasePath();
    const imported = await createDatabase(importedPath, {
      databaseId: "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f99",
      createdAt,
    });
    openDatabases.push(imported);
    const loaded = await loadAthleticStandardPackage(exportDir);
    const preview = await previewPackageImport(imported, loaded);
    await importRecords(imported, preview);
    const importedRecord = await readRecord(imported, pointId);
    expect(importedRecord?.extensions).toEqual(extensions);
    await rm(exportRoot, { recursive: true, force: true });
  });
});
