import { chmod, lstat, mkdir, mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";
import { createClient } from "@libsql/client";
import { afterEach, describe, expect, it } from "vitest";
import { database as publicDatabase } from "../src/index.js";
import {
  createDatabase,
  correctRecord,
  createUuidV7,
  deleteRecord,
  listRecords,
  migrateDatabase,
  openDatabase,
  queryMeasurements,
  queryRecordsAsOf,
  querySeries,
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
      sql_schema_version: 2,
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
        "provider_summaries",
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

    await expect(
      openDatabase(path, { onSecurityWarning: () => undefined }),
    ).rejects.toThrow("does not belong");

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
    await databaseClient(database).execute("PRAGMA user_version = 0");
    database.close();
    openDatabases.pop();
    await expect(openDatabase(path)).rejects.toThrow("does not match");
  });

  it("reconciles user_version when it is one step behind metadata after a partial migration", async () => {
    const { database, path } = await newDatabase();
    await databaseClient(database).execute("PRAGMA user_version = 1");
    database.close();
    openDatabases.pop();
    const reopened = await openDatabase(path);
    openDatabases.push(reopened);
    const userVersion = await databaseClient(reopened).execute("PRAGMA user_version");
    expect(Number(userVersion.rows[0]?.user_version)).toBe(2);
  });

  it.runIf(process.platform !== "win32")(
    "warns before opening a database readable by other local users",
    async () => {
      const { database, path } = await newDatabase();
      database.close();
      openDatabases.pop();
      await chmod(path, 0o644);
      const warnings: string[] = [];
      const reopened = await openDatabase(path, {
        onSecurityWarning: (warning) => warnings.push(warning),
      });
      openDatabases.push(reopened);
      expect(warnings).toContain(
        `Sensitive athlete data path is readable by other local users: ${path}`,
      );
    },
  );

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

  it("refuses a correction whose knowledge time precedes its current revision", async () => {
    const { database } = await newDatabase();
    await writeRecord(database, sourceInput());
    await expect(
      correctRecord(database, {
        record_id: sourceId,
        expected_revision: 1,
        changed_by: toolActor,
        became_known_at: "2026-08-08T12:00:00.000Z",
        data: sourceInput().data,
      }),
    ).rejects.toThrow("cannot precede");
    expect(await readRecordRevisions(database, sourceId)).toHaveLength(1);
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

  it("enforces matching benchmarks and exact grade arithmetic", async () => {
    const { database } = await newDatabase();
    const benchmarkOne = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e10";
    const benchmarkTwo = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e11";
    const resultOne = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e12";
    const resultTwo = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e13";
    const prediction = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e14";
    const link = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e15";

    const authorship = {
      author: { author_type: "athlete" as const },
      authored_at: createdAt,
      evidence: [],
      content_version: 1,
      status: "active" as const,
    };
    await writeRecords(database, [
      sourceInput(),
      {
        record_type: "benchmark_definition",
        record_id: benchmarkOne,
        created_at: createdAt,
        changed_by: toolActor,
        data: {
          authorship,
          benchmark_key: "fran",
          benchmark_type: "named_wod",
          score_type: "time",
          definition: "21-15-9 thrusters and pull-ups",
        },
      },
      {
        record_type: "benchmark_definition",
        record_id: benchmarkTwo,
        created_at: createdAt,
        changed_by: toolActor,
        data: {
          authorship,
          benchmark_key: "grace",
          benchmark_type: "named_wod",
          score_type: "time",
          definition: "30 clean and jerks",
        },
      },
      ...([
        [resultOne, benchmarkOne],
        [resultTwo, benchmarkTwo],
      ] as const).map(([record_id, benchmark_definition_id]) => ({
        record_type: "benchmark_result" as const,
        record_id,
        created_at: createdAt,
        changed_by: toolActor,
        data: {
          source_id: sourceId,
          received_at: createdAt,
          received_at_basis: "local_write_time" as const,
          benchmark_definition_id,
          recorded_at: "2026-08-10T12:00:00.000Z",
          input_timestamp: timestampReceipt,
          score: { duration_s: 300 },
        },
      })),
      {
        record_type: "prediction",
        record_id: prediction,
        created_at: createdAt,
        changed_by: toolActor,
        data: {
          authorship,
          benchmark_definition_id: benchmarkOne,
          predicted_score: { duration_s: 280 },
          predicted_range: {
            low: { duration_s: 270 },
            high: { duration_s: 290 },
          },
          confidence: "moderate",
          reasoning: "Based on linked evidence",
        },
      },
    ]);

    await expect(
      writeRecord(database, {
        record_type: "prediction_result_link",
        record_id: link,
        created_at: createdAt,
        changed_by: toolActor,
        data: {
          prediction: { record_id: prediction, revision: 1 },
          benchmark_result: { record_id: resultTwo, revision: 1 },
          created_by: toolActor,
          linked_at: createdAt,
          content_version: 1,
          status: "active",
        },
      }),
    ).rejects.toThrow("different benchmark definitions");

    await writeRecord(database, {
      record_type: "prediction_result_link",
      record_id: link,
      created_at: createdAt,
      changed_by: toolActor,
      data: {
        prediction: { record_id: prediction, revision: 1 },
        benchmark_result: { record_id: resultOne, revision: 1 },
        created_by: toolActor,
        linked_at: createdAt,
        content_version: 1,
        status: "active",
      },
    });

    await expect(
      writeRecord(database, {
        record_type: "prediction_grade",
        record_id: "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e16",
        created_at: createdAt,
        changed_by: toolActor,
        data: {
          authorship: {
            author: {
              ...toolActor,
              calculation_version: "prediction-grade-v1",
            },
            authored_at: createdAt,
            evidence: [{ record_id: link, revision: 1 }],
            content_version: 1,
            status: "active",
          },
          prediction_result_link: { record_id: link, revision: 1 },
          signed_error: 0,
          absolute_error_percentage: 0,
          result_in_predicted_range: true,
          predicted_score: { duration_s: 280 },
          actual_score: { duration_s: 300 },
        },
      }),
    ).rejects.toThrow("arithmetic");
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

describe("v1 record queries", () => {
  it("keeps manual measurements out of device queries by default", async () => {
    const { database } = await newDatabase();
    const manualSourceId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e20";
    const manualPointId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e21";
    await writeRecords(database, [
      sourceInput(),
      pointInput(),
      {
        record_type: "source",
        record_id: manualSourceId,
        created_at: createdAt,
        changed_by: toolActor,
        data: {
          source_type: "manual",
          data_writer: "ath",
          data_route: "manual_entry",
        },
      },
      {
        ...pointInput(manualSourceId),
        record_id: manualPointId,
      },
    ]);

    expect((await queryMeasurements(database, { quantity: "hrv_rmssd" }))).toHaveLength(1);
    expect(
      await queryMeasurements(database, {
        quantity: "hrv_rmssd",
        include_manual: true,
      }),
    ).toHaveLength(2);
  });

  it("queries dense samples by quantity, source, and UTC bounds", async () => {
    const { database } = await newDatabase();
    const sampleId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e22";
    const laterSampleId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e23";
    await writeRecords(database, [
      sourceInput(),
      {
        record_type: "measurement_sample",
        record_id: sampleId,
        created_at: createdAt,
        changed_by: toolActor,
        became_known_at_basis: "provider_import",
        data: {
          source_id: sourceId,
          quantity: "heart_rate",
          recorded_at: "2026-08-09T11:12:00.000Z",
          input_timestamp: timestampReceipt,
          value: {
            standard_value: 142,
            standard_unit: "bpm",
            input_value: 142,
            input_unit: "count/min",
          },
          received_at: createdAt,
          received_at_basis: "import_time",
        },
      },
      {
        record_type: "measurement_sample",
        record_id: laterSampleId,
        created_at: createdAt,
        changed_by: toolActor,
        became_known_at_basis: "provider_import",
        data: {
          source_id: sourceId,
          quantity: "heart_rate",
          recorded_at: "2026-08-09T11:13:00.000Z",
          input_timestamp: {
            ...timestampReceipt,
            timestamp_text: "2026-08-09T11:13:00.000Z",
          },
          value: {
            standard_value: 143,
            standard_unit: "bpm",
            input_value: 143,
            input_unit: "count/min",
          },
          received_at: createdAt,
          received_at_basis: "import_time",
        },
      },
    ]);

    expect(
      await querySeries(database, {
        quantity: "heart_rate",
        source_id: sourceId,
        from: "2026-08-09T11:00:00.000Z",
        to: "2026-08-09T12:00:00.000Z",
      }),
    ).toMatchObject([
      { record_id: sampleId },
      { record_id: laterSampleId },
    ]);
    expect(
      await querySeries(database, {
        quantity: "heart_rate",
        source_id: sourceId,
        order: "descending",
        limit: 1,
      }),
    ).toMatchObject([{ record_id: laterSampleId }]);
    expect(
      await querySeries(database, {
        quantity: "heart_rate",
        from: "2026-08-10T00:00:00.000Z",
      }),
    ).toEqual([]);
  });

  it("reads the revision visible at an exact historical cutoff", async () => {
    const { database } = await newDatabase();
    await writeRecords(database, [sourceInput(), pointInput()]);
    await correctRecord(database, {
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

    const before = await queryRecordsAsOf(database, {
      as_of: "2026-08-10T11:59:59.000Z",
      record_type: "point_measurement",
    });
    const after = await queryRecordsAsOf(database, {
      as_of: "2026-08-10T12:00:00.000Z",
      record_type: "point_measurement",
    });
    expect(before[0]?.current_revision).toBe(1);
    expect(after[0]?.current_revision).toBe(2);
    if (before[0]?.record_type === "point_measurement") {
      expect(before[0].data.value.standard_value).toBe(62);
    }
    if (after[0]?.record_type === "point_measurement") {
      expect(after[0].data.value.standard_value).toBe(64);
    }
    const selectedTypes = await queryRecordsAsOf(database, {
      as_of: "2026-08-10T12:00:00.000Z",
      record_types: ["source", "point_measurement"],
    });
    expect(selectedTypes.map((record) => record.record_type).sort()).toEqual([
      "point_measurement",
      "source",
    ]);
  });

  it("hides a deletion from current and later historical queries", async () => {
    const { database } = await newDatabase();
    await writeRecord(database, sourceInput());
    const deletion = await deleteRecord(database, {
      record_id: sourceId,
      expected_revision: 1,
      deleted_by: toolActor,
      deleted_at: "2026-08-10T12:00:00.000Z",
    });
    expect(deletion.database_revision).toBe(2);
    expect(await listRecords(database)).toEqual([]);
    expect(
      await queryRecordsAsOf(database, {
        as_of: "2026-08-10T11:59:59.000Z",
        record_type: "source",
      }),
    ).toHaveLength(1);
    expect(
      await queryRecordsAsOf(database, {
        as_of: "2026-08-10T12:00:00.000Z",
        record_type: "source",
      }),
    ).toEqual([]);
  });
});

describe("UUIDv7 creation", () => {
  it("creates lowercase UUIDv7 values", () => {
    expect(createUuidV7(1_728_000_000_000)).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    );
  });
});

describe("v1 WAL behavior", () => {
  it("allows a writer to commit while a reader keeps a stable snapshot", async () => {
    const { database, path } = await newDatabase();
    const writer = await openDatabase(path);
    openDatabases.push(writer);
    const reader = await databaseClient(database).transaction("read");
    try {
      const before = await reader.execute(
        "SELECT COUNT(*) AS count FROM records",
      );
      expect(Number(before.rows[0]?.count)).toBe(0);

      await writeRecord(writer, sourceInput());

      const stable = await reader.execute(
        "SELECT COUNT(*) AS count FROM records",
      );
      expect(Number(stable.rows[0]?.count)).toBe(0);
      await reader.commit();
    } finally {
      if (!reader.closed) {
        await reader.rollback();
      }
      reader.close();
    }
    const after = await databaseClient(database).execute(
      "SELECT COUNT(*) AS count FROM records",
    );
    expect(Number(after.rows[0]?.count)).toBe(1);
  });

  it("discards an uncommitted write when a connection closes", async () => {
    const { database, path } = await newDatabase();
    const interrupted = spawnSync(
      process.execPath,
      [
        "--input-type=module",
        "--eval",
        `
          import { createClient } from "@libsql/client";
          const [databaseUrl, recordId, erasedAt] = process.argv.slice(1);
          const client = createClient({ url: databaseUrl });
          await client.execute("PRAGMA foreign_keys = ON");
          const transaction = await client.transaction("write");
          await transaction.execute({
            sql: \`
              INSERT INTO record_erasure_markers (
                record_id,
                erased_at,
                reason_code
              ) VALUES (?, ?, 'other')
            \`,
            args: [recordId, erasedAt],
          });
          process.exit(0);
        `,
        pathToFileURL(path).href,
        reportId,
        createdAt,
      ],
      {
        cwd: process.cwd(),
        encoding: "utf8",
      },
    );
    expect(interrupted).toMatchObject({
      status: 0,
      signal: null,
    });

    const recovered = await databaseClient(database).execute({
      sql: "SELECT COUNT(*) AS count FROM record_erasure_markers WHERE record_id = ?",
      args: [reportId],
    });
    expect(Number(recovered.rows[0]?.count)).toBe(0);
    expect(
      (await databaseClient(database).execute("PRAGMA integrity_check")).rows,
    ).toMatchObject([{ integrity_check: "ok" }]);
  });
});
