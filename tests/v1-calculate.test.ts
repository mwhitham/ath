import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  calculate,
  createDatabase,
  createUuidV7,
  resolveCalendarRange,
  writeRecords,
  type AthleticDatabase,
} from "../src/db/index.js";
import { databaseClient } from "../src/db/client.js";

const opened: AthleticDatabase[] = [];
const temporaryDirectories: string[] = [];
const actor = {
  author_type: "tool" as const,
  tool: "test",
  tool_version: "1.0.0",
};

afterEach(() => {
  for (const database of opened.splice(0)) {
    database.close();
  }
  for (const directory of temporaryDirectories.splice(0)) {
    rmSync(directory, { recursive: true, force: true });
  }
});

async function calculationDatabase(): Promise<{
  database: AthleticDatabase;
  sourceA: string;
  sourceB: string;
}> {
  const directory = mkdtempSync(join(tmpdir(), "ath-calculate-"));
  temporaryDirectories.push(directory);
  const database = await createDatabase(join(directory, "athlete.ath.sqlite"), {
    createdAt: "2026-10-01T00:00:00.000Z",
  });
  opened.push(database);
  const sourceA = createUuidV7(Date.parse("2026-10-01T00:00:00.000Z"));
  const sourceB = createUuidV7(Date.parse("2026-10-01T00:00:01.000Z"));
  const manual = createUuidV7(Date.parse("2026-10-01T00:00:02.000Z"));
  const timestamp = (value: string) => ({
    timestamp_text: value,
    utc_offset_text: "Z",
    utc_offset_meaning: "documented_recording_offset" as const,
    timestamp_basis: "source_instant" as const,
  });
  const point = (
    sourceId: string,
    at: string,
    value: number,
  ) => ({
    record_type: "point_measurement" as const,
    record_id: createUuidV7(Date.parse(at)),
    created_at: "2026-10-03T00:00:00.000Z",
    changed_by: actor,
    data: {
      source_id: sourceId,
      received_at: "2026-10-03T00:00:00.000Z",
      received_at_basis: "import_time" as const,
      quantity: "hrv_rmssd" as const,
      recorded_at: at,
      input_timestamp: timestamp(at),
      value: {
        standard_value: value,
        standard_unit: "ms",
        input_value: value,
        input_unit: "ms",
      },
    },
  });
  await writeRecords(database, [
    {
      record_type: "source",
      record_id: sourceA,
      created_at: "2026-10-01T00:00:00.000Z",
      changed_by: actor,
      data: {
        source_type: "wearable",
        data_writer: "Device A",
        data_route: "test",
      },
    },
    {
      record_type: "source",
      record_id: sourceB,
      created_at: "2026-10-01T00:00:01.000Z",
      changed_by: actor,
      data: {
        source_type: "wearable",
        data_writer: "Device B",
        data_route: "test",
      },
    },
    {
      record_type: "source",
      record_id: manual,
      created_at: "2026-10-01T00:00:02.000Z",
      changed_by: actor,
      data: {
        source_type: "manual",
        data_writer: "ath",
        data_route: "manual_entry",
      },
    },
    point(sourceA, "2026-10-01T12:00:00.000Z", 60),
    point(sourceA, "2026-10-02T12:00:00.000Z", 70),
    point(sourceB, "2026-10-01T12:00:00.000Z", 50),
    point(sourceB, "2026-10-02T12:00:00.000Z", 55),
    point(manual, "2026-10-01T12:00:00.000Z", 999),
  ]);
  return { database, sourceA, sourceB };
}

describe("runtime calculations", () => {
  it("keeps source baselines separate and excludes manual measurements", async () => {
    const { database, sourceA, sourceB } = await calculationDatabase();
    const result = await calculate(database, {
      operation: "baseline",
      quantity: "hrv_rmssd",
    });
    expect(result.sources).toEqual([sourceA, sourceB].sort());
    expect(result.input.record_count).toBe(4);
    expect(result.result).toEqual([
      {
        source_id: sourceA,
        count: 2,
        minimum: 60,
        maximum: 70,
        mean: 65,
        sum: 130,
        standard_deviation: 5,
      },
      {
        source_id: sourceB,
        count: 2,
        minimum: 50,
        maximum: 55,
        mean: 52.5,
        sum: 105,
        standard_deviation: 2.5,
      },
    ]);
  });

  it("requires a timezone for calendar coverage", async () => {
    const { database, sourceA } = await calculationDatabase();
    await expect(
      calculate(database, {
        operation: "coverage",
        quantity: "hrv_rmssd",
        source_id: sourceA,
      }),
    ).rejects.toThrow("timezone");

    const result = await calculate(database, {
      operation: "coverage",
      quantity: "hrv_rmssd",
      source_id: sourceA,
      time_zone: "America/Chicago",
      from: "2026-10-01",
      to: "2026-10-03",
    });
    expect(result.result).toEqual([
      expect.objectContaining({
        source_id: sourceA,
        days_expected: 3,
        days_present: 2,
        missing_dates: ["2026-10-03"],
      }),
    ]);
  });

  it("compares named sources without pooling their values", async () => {
    const { database, sourceA, sourceB } = await calculationDatabase();
    const result = await calculate(database, {
      operation: "compare-sources",
      quantity: "hrv_rmssd",
      source_id: sourceA,
      compare_source_id: sourceB,
    });
    expect(result.result).toMatchObject({
      source: { source_id: sourceA, mean: 65 },
      comparison_source: { source_id: sourceB, mean: 52.5 },
      mean_difference: 12.5,
    });
  });

  it("bounds workout drift inputs before applying the calculation safety limit", async () => {
    const { database, sourceA } = await calculationDatabase();
    const workoutId = createUuidV7(Date.parse("2026-10-05T12:00:00.000Z"));
    const timestamp = (value: string) => ({
      timestamp_text: value,
      utc_offset_text: "Z",
      utc_offset_meaning: "documented_recording_offset" as const,
      timestamp_basis: "source_instant" as const,
    });
    const sample = (at: string, value: number) => ({
      record_type: "measurement_sample" as const,
      record_id: createUuidV7(Date.parse(at)),
      created_at: at,
      changed_by: actor,
      data: {
        source_id: sourceA,
        received_at: at,
        received_at_basis: "import_time" as const,
        quantity: "heart_rate" as const,
        recorded_at: at,
        input_timestamp: timestamp(at),
        value: {
          standard_value: value,
          standard_unit: "bpm",
          input_value: value,
          input_unit: "bpm",
        },
      },
    });
    await writeRecords(database, [
      {
        record_type: "workout_session",
        record_id: workoutId,
        created_at: "2026-10-05T12:00:00.000Z",
        changed_by: actor,
        data: {
          source_id: sourceA,
          received_at: "2026-10-05T12:20:00.000Z",
          received_at_basis: "import_time",
          start_at: "2026-10-05T12:00:00.000Z",
          end_at: "2026-10-05T12:20:00.000Z",
          input_start_timestamp: timestamp("2026-10-05T12:00:00.000Z"),
          input_end_timestamp: timestamp("2026-10-05T12:20:00.000Z"),
          activity_type: "running",
        },
      },
      sample("2026-10-05T12:05:00.000Z", 120),
      sample("2026-10-05T12:15:00.000Z", 140),
    ]);

    const transaction = await databaseClient(database).transaction("write");
    try {
      await transaction.execute("PRAGMA defer_foreign_keys = ON");
      await transaction.execute(`
        CREATE TEMP TABLE calculation_outside_samples (
          n INTEGER PRIMARY KEY
        ) STRICT
      `);
      await transaction.execute(`
        WITH RECURSIVE sequence(n) AS (
          SELECT 1
          UNION ALL
          SELECT n + 1 FROM sequence WHERE n < 100001
        )
        INSERT INTO calculation_outside_samples (n)
        SELECT n FROM sequence
      `);
      await transaction.execute(`
        INSERT INTO records (
          record_id,
          record_type,
          current_revision,
          created_at,
          is_deleted
        )
        SELECT
          printf('outside-%06d', n),
          'measurement_sample',
          1,
          '2026-10-06T12:00:00.000Z',
          0
        FROM calculation_outside_samples
      `);
      await transaction.execute(`
        INSERT INTO measurement_samples (
          record_id,
          record_type,
          source_id,
          quantity,
          recorded_at,
          standard_value,
          standard_unit,
          received_at,
          data_json
        )
        SELECT
          printf('outside-%06d', n),
          'measurement_sample',
          '${sourceA}',
          'heart_rate',
          '2026-10-06T12:00:00.000Z',
          100,
          'bpm',
          '2026-10-06T12:00:00.000Z',
          '{}'
        FROM calculation_outside_samples
      `);
      await transaction.execute(`
        INSERT INTO record_revisions (
          record_id,
          revision,
          record_type,
          record_json,
          became_known_at,
          became_known_at_basis,
          changed_by_json
        )
        SELECT
          printf('outside-%06d', n),
          1,
          'measurement_sample',
          '{}',
          '2026-10-06T12:00:00.000Z',
          'provider_import',
          '{"author_type":"tool","tool":"test","tool_version":"1.0.0"}'
        FROM calculation_outside_samples
      `);
      await transaction.commit();
    } finally {
      transaction.close();
    }

    const result = await calculate(database, {
      operation: "workout-drift",
      quantity: "heart_rate",
      source_id: sourceA,
      workout_session_id: workoutId,
    });
    expect(result.input.record_count).toBe(2);
    expect(result.result).toMatchObject({
      workout_session_id: workoutId,
      sources: [
        {
          source_id: sourceA,
          available: true,
          first_half_mean: 120,
          second_half_mean: 140,
        },
      ],
    });
  }, 20_000);

  it("uses the real short and long day boundaries in an IANA timezone", async () => {
    const { database } = await calculationDatabase();
    await expect(
      resolveCalendarRange(database, {
        from: "2026-03-08",
        to: "2026-03-08",
        time_zone: "America/Chicago",
      }),
    ).resolves.toMatchObject({
      from: "2026-03-08T06:00:00.000Z",
      to: "2026-03-09T04:59:59.999Z",
      time_zone: "America/Chicago",
    });
    await expect(
      resolveCalendarRange(database, {
        from: "2026-11-01",
        to: "2026-11-01",
        time_zone: "America/Chicago",
      }),
    ).resolves.toMatchObject({
      from: "2026-11-01T05:00:00.000Z",
      to: "2026-11-02T05:59:59.999Z",
      time_zone: "America/Chicago",
    });
  });
});
