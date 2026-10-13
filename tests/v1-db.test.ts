import { lstat, mkdir, mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { createClient } from "@libsql/client";
import { afterEach, describe, expect, it } from "vitest";
import { database as publicDatabase } from "../src/index.js";
import {
  createDatabase,
  correctRecord,
  createUuidV7,
  listRecords,
  migrateDatabase,
  openDatabase,
  readDatabaseMetadata,
  readRecord,
  readRecordRevisions,
  SQLITE_APPLICATION_ID,
  SQL_SCHEMA_VERSION,
  writeRecord,
  writeRecords,
  type AthleticDatabase,
} from "../src/db/index.js";
import {
  databaseClient,
  readApplicationId,
  readUserVersion,
} from "../src/db/client.js";

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

    const foreignKeys = await databaseClient(database).execute("PRAGMA foreign_keys");
    expect(Number(foreignKeys.rows[0]?.foreign_keys)).toBe(1);
    const journalMode = await databaseClient(database).execute("PRAGMA journal_mode");
    expect(String(journalMode.rows[0]?.journal_mode).toLowerCase()).toBe("wal");

    const tables = await databaseClient(database).execute(
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
        "athlete_profile",
        "sources",
        "source_devices",
        "point_measurements",
        "point_measurement_calculations",
        "point_measurement_calculation_inputs",
        "measurement_samples",
        "sleep_sessions",
        "workout_sessions",
        "workout_session_segments",
        "benchmark_results",
        "benchmark_result_segments",
        "provider_scores",
        "provider_reporting_periods",
        "athlete_reports",
        "nutrition_entries",
        "reported_nutrition_items",
        "attachments",
        "record_attachment_links",
        "benchmark_definitions",
        "predictions",
        "prediction_result_links",
        "prediction_grades",
        "prediction_error_analyses",
        "prediction_error_causes",
        "training_plans",
        "planned_sessions",
        "planned_session_blocks",
        "coaching_notes",
        "saved_training_reviews",
        "goals",
        "nutrition_model_interpretations",
        "authored_record_evidence",
      ]),
    );

    if (process.platform !== "win32") {
      await writeRecord(database, sourceInput());
      for (const suffix of ["", "-wal", "-shm"]) {
        const mode = (await lstat(`${path}${suffix}`)).mode & 0o777;
        expect(mode).toBe(0o600);
      }
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
    const before = await client.execute("PRAGMA journal_mode");
    expect(String(before.rows[0]?.journal_mode).toLowerCase()).toBe("delete");
    client.close();

    await expect(openDatabase(path)).rejects.toThrow("does not belong");

    const afterClient = createClient({ url: pathToFileURL(path).href });
    const after = await afterClient.execute("PRAGMA journal_mode");
    expect(String(after.rows[0]?.journal_mode).toLowerCase()).toBe("delete");
    afterClient.close();
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

  it.runIf(process.platform !== "win32")(
    "refuses a database path through a symbolic-link parent",
    async () => {
      const directory = await mkdtemp(join(tmpdir(), "ath-v1-parent-"));
      temporaryDirectories.push(directory);
      const realParent = join(directory, "real");
      const linkedParent = join(directory, "linked");
      await mkdir(realParent);
      await symlink(realParent, linkedParent);
      await expect(createDatabase(join(linkedParent, "athlete.ath.sqlite"))).rejects.toThrow(
        "symbolic link",
      );
    },
  );

  it("applies the busy timeout to transaction connections", async () => {
    const { database } = await newDatabase();
    const transaction = await databaseClient(database).transaction("read");
    try {
      const timeout = await transaction.execute("PRAGMA busy_timeout");
      expect(Number(timeout.rows[0]?.timeout)).toBe(5_000);
    } finally {
      await transaction.rollback();
      transaction.close();
    }
  });

  it("refuses a local user_version that disagrees with metadata", async () => {
    const { database, path } = await newDatabase();
    await databaseClient(database).execute("PRAGMA user_version = 2");
    database.close();
    openDatabases.pop();
    await expect(openDatabase(path)).rejects.toThrow("does not match");
  });

  it("does not expose a raw SQL client or transaction callback publicly", () => {
    expect(publicDatabase).not.toHaveProperty("connectDatabase");
    expect(publicDatabase).not.toHaveProperty("databaseClient");
    expect(publicDatabase).not.toHaveProperty("withWriteTransaction");
  });
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
    const state = await databaseClient(database).execute(
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

  it("enforces typed prediction-ledger references", async () => {
    const { database } = await newDatabase();
    await writeRecord(database, sourceInput());
    await expect(
      writeRecord(database, {
        record_type: "prediction_result_link",
        record_id: "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e05",
        created_at: createdAt,
        changed_by: toolActor,
        data: {
          prediction: { record_id: sourceId, revision: 1 },
          benchmark_result: { record_id: sourceId, revision: 1 },
          created_by: toolActor,
          linked_at: createdAt,
          content_version: 1,
          status: "active",
        },
      }),
    ).rejects.toThrow();
  });

  it("enforces attachment, calculation-input, and goal-target links", async () => {
    const { database } = await newDatabase();
    const missingId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1eee";

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
          attachment_ids: [missingId],
        },
      }),
    ).rejects.toThrow();

    await writeRecord(database, sourceInput());
    await expect(
      writeRecord(database, {
        ...pointInput(),
        data: {
          ...pointInput().data,
          calculations: [
            {
              from_quantity: "hrv_beats",
              method: "rmssd",
              calculation_version: "1",
              input_records: [{ record_id: missingId, revision: 1 }],
            },
          ],
        },
      }),
    ).rejects.toThrow();

    await expect(
      writeRecord(database, {
        record_type: "goal",
        record_id: "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e06",
        created_at: createdAt,
        changed_by: toolActor,
        data: {
          authorship: {
            author: { author_type: "athlete" },
            authored_at: createdAt,
            evidence: [],
            content_version: 1,
            status: "active",
          },
          title: "Improve Fran",
          target_benchmark_definition_id: missingId,
          target_score: { duration_s: 240 },
        },
      }),
    ).rejects.toThrow();
  });
});

describe("UUIDv7 creation", () => {
  it("creates lowercase UUIDv7 values", () => {
    expect(createUuidV7(1_728_000_000_000)).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    );
  });
});
