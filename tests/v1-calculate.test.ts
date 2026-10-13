import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  calculate,
  createDatabase,
  createUuidV7,
  writeRecords,
  type AthleticDatabase,
} from "../src/db/index.js";

const opened: AthleticDatabase[] = [];
const actor = {
  author_type: "tool" as const,
  tool: "test",
  tool_version: "1.0.0",
};

afterEach(() => {
  for (const database of opened.splice(0)) {
    database.close();
  }
});

async function calculationDatabase(): Promise<{
  database: AthleticDatabase;
  sourceA: string;
  sourceB: string;
}> {
  const database = await createDatabase(
    join(mkdtempSync(join(tmpdir(), "ath-calculate-")), "athlete.ath.sqlite"),
    { createdAt: "2026-10-01T00:00:00.000Z" },
  );
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
});
