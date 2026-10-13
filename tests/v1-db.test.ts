import { lstat, mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { createClient } from "@libsql/client";
import { afterEach, describe, expect, it } from "vitest";
import {
  createDatabase,
  correctRecord,
  createUuidV7,
  listRecords,
  migrateDatabase,
  openDatabase,
  readApplicationId,
  readDatabaseMetadata,
  readRecord,
  readRecordRevisions,
  readUserVersion,
  SQLITE_APPLICATION_ID,
  SQL_SCHEMA_VERSION,
  writeRecord,
  writeRecords,
  type AthleticDatabase,
} from "../src/db/index.js";

const databaseId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e01";
const sourceId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e02";
const pointId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e03";
const reportId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e04";
const createdAt = "2026-08-09T12:00:00.000Z";

const toolActor = {
  author_type: "tool" as const,
  tool: "ath",
  tool_version: "1.0.0",
};

const timestampReceipt = {
  timestamp_text: "2026-08-09T06:12:00-05:00",
  utc_offset_text: "-05:00",
  utc_offset_meaning: "provider_cycle_offset" as const,
  timestamp_basis: "source_offset_timestamp" as const,
};

const openDatabases: AthleticDatabase[] = [];
const temporaryDirectories: string[] = [];

async function tempDatabasePath(): Promise<string> {
  const directory = await mkdtemp(join(tmpdir(), "ath-v1-db-"));
  temporaryDirectories.push(directory);
  return join(directory, "athlete.ath.sqlite");
}

async function newDatabase(): Promise<{
  database: AthleticDatabase;
  path: string;
}> {
  const path = await tempDatabasePath();
  const database = await createDatabase(path, { databaseId, createdAt });
  openDatabases.push(database);
  return { database, path };
}

function sourceInput() {
  return {
    record_type: "source" as const,
    record_id: sourceId,
    created_at: createdAt,
    became_known_at: createdAt,
    changed_by: toolActor,
    data: {
      source_type: "wearable" as const,
      data_writer: "WHOOP",
      data_route: "whoop_csv",
      vendor: "whoop",
    },
  };
}

function pointInput(source_id = sourceId) {
  return {
    record_type: "point_measurement" as const,
    record_id: pointId,
    created_at: createdAt,
    became_known_at: createdAt,
    became_known_at_basis: "provider_import" as const,
    changed_by: toolActor,
    data: {
      source_id,
      received_at: createdAt,
      received_at_basis: "import_time" as const,
      quantity: "hrv_rmssd" as const,
      recorded_at: "2026-08-09T11:12:00.000Z",
      input_timestamp: timestampReceipt,
      value: {
        standard_value: 62,
        standard_unit: "ms",
        input_value: 62,
        input_unit: "ms",
      },
    },
  };
}

afterEach(async () => {
  while (openDatabases.length > 0) {
    openDatabases.pop()!.close();
  }
  await Promise.all(
    temporaryDirectories.splice(0).map((directory) =>
      rm(directory, { recursive: true, force: true }),
    ),
  );
});

describe("v1 database creation and opening", () => {
  it("creates the marked v1 schema with local SQLite settings", async () => {
    const { database, path } = await newDatabase();
    expect(await readApplicationId(database)).toBe(SQLITE_APPLICATION_ID);
    expect(await readUserVersion(database)).toBe(SQL_SCHEMA_VERSION);
    expect(await readDatabaseMetadata(database)).toEqual({
      application_marker: "ATH1",
      sql_schema_version: 1,
      athleticstandard_version: "1.0.0",
      database_id: databaseId,
      created_at: createdAt,
    });

    const foreignKeys = await database.client.execute("PRAGMA foreign_keys");
    expect(Number(foreignKeys.rows[0]?.foreign_keys)).toBe(1);
    const journalMode = await database.client.execute("PRAGMA journal_mode");
    expect(String(journalMode.rows[0]?.journal_mode).toLowerCase()).toBe("wal");

    const tables = await database.client.execute(
      "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
    );
    const tableNames = tables.rows.map((row) => String(row.name));
    expect(tableNames).toEqual(
      expect.arrayContaining([
        "database_metadata",
        "database_state",
        "records",
        "record_revisions",
        "record_deletions",
        "record_erasure_markers",
        "record_import_keys",
        "sources",
        "point_measurements",
        "measurement_samples",
        "athlete_reports",
        "authored_record_evidence",
      ]),
    );

    if (process.platform !== "win32") {
      const mode = (await lstat(path)).mode & 0o777;
      expect(mode).toBe(0o600);
    }
  });

  it("opens a valid database and treats the current migration as a no-op", async () => {
    const { database, path } = await newDatabase();
    database.close();
    openDatabases.pop();

    const reopened = await openDatabase(path);
    openDatabases.push(reopened);
    await expect(migrateDatabase(reopened)).resolves.toBeUndefined();
  });

  it("refuses to replace an existing file", async () => {
    const { path } = await newDatabase();
    await expect(createDatabase(path)).rejects.toThrow("Refusing to replace");
  });

  it("refuses another nonzero SQLite application ID", async () => {
    const path = await tempDatabasePath();
    const client = createClient({ url: pathToFileURL(path).href });
    await client.execute("PRAGMA application_id = 1234");
    client.close();

    await expect(openDatabase(path)).rejects.toThrow("does not belong");
  });

  it.runIf(process.platform !== "win32")(
    "refuses to open through a symbolic link",
    async () => {
      const { database, path } = await newDatabase();
      database.close();
      openDatabases.pop();
      const link = join(path, "..", "linked.ath.sqlite");
      await symlink(path, link);
      await expect(openDatabase(link)).rejects.toThrow("symbolic link");
    },
  );
});

describe("v1 record writes and revisions", () => {
  it("writes a dependency batch in one database revision", async () => {
    const { database } = await newDatabase();
    const result = await writeRecords(database, [pointInput(), sourceInput()]);
    expect(result.database_revision).toBe(1);
    expect(result.records).toHaveLength(2);

    const point = await readRecord(database, pointId);
    expect(point?.record_type).toBe("point_measurement");
    if (point?.record_type === "point_measurement") {
      expect(point.data.value.standard_value).toBe(62);
    }

    const listed = await listRecords(database);
    expect(listed.map((record) => record.record_id).sort()).toEqual(
      [pointId, sourceId].sort(),
    );
  });

  it("rolls back the whole batch when a measured source does not exist", async () => {
    const { database } = await newDatabase();
    const missingSource = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1eff";
    await expect(
      writeRecords(database, [sourceInput(), pointInput(missingSource)]),
    ).rejects.toThrow();

    expect(await listRecords(database)).toEqual([]);
    const state = await database.client.execute(
      "SELECT current_revision FROM database_state WHERE singleton_id = 1",
    );
    expect(Number(state.rows[0]?.current_revision)).toBe(0);
  });

  it("creates a complete correction revision without changing the stable ID", async () => {
    const { database } = await newDatabase();
    await writeRecords(database, [sourceInput(), pointInput()]);

    const correction = await correctRecord(database, {
      record_id: pointId,
      expected_revision: 1,
      changed_by: toolActor,
      became_known_at: "2026-08-10T12:00:00.000Z",
      became_known_at_basis: "provider_import",
      data: {
        ...pointInput().data,
        value: {
          ...pointInput().data.value,
          standard_value: 64,
          input_value: 64,
        },
      },
    });
    expect(correction.database_revision).toBe(2);
    expect(correction.record.record_id).toBe(pointId);
    expect(correction.record.current_revision).toBe(2);

    const revisions = await readRecordRevisions(database, pointId);
    expect(revisions.map((revision) => revision.revision)).toEqual([1, 2]);
    expect(revisions[1]?.previous_revision).toBe(1);
    if (revisions[1]?.record_type === "point_measurement") {
      expect(revisions[1].data.value.standard_value).toBe(64);
    }
  });

  it("refuses a stale correction without changing data", async () => {
    const { database } = await newDatabase();
    await writeRecord(database, sourceInput());
    await expect(
      correctRecord(database, {
        record_id: sourceId,
        expected_revision: 2,
        changed_by: toolActor,
        data: sourceInput().data,
      }),
    ).rejects.toThrow("expected revision");
    expect((await readRecord(database, sourceId))?.current_revision).toBe(1);
  });

  it("enforces the two-tier wall before SQL", async () => {
    const { database } = await newDatabase();
    await expect(
      writeRecord(database, {
        record_type: "athlete_report",
        record_id: reportId,
        created_at: createdAt,
        changed_by: toolActor,
        data: {
          report_type: "soreness",
          reported_at: createdAt,
          input_reported_at: timestampReceipt,
          reported_via: "text",
          source_id: sourceId,
        },
      }),
    ).rejects.toThrow();
    expect(await readRecord(database, reportId)).toBeNull();
  });
});

describe("UUIDv7 creation", () => {
  it("creates lowercase UUIDv7 values", () => {
    expect(createUuidV7(1_728_000_000_000)).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    );
  });
});
