import { cpSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  createDatabase,
  importProviderRecords,
  listRecords,
  previewProviderImport,
  readRecordRevisions,
  type AthleticDatabase,
} from "../src/db/index.js";

const fixtures = resolve("tests/fixtures/exports");
const opened: AthleticDatabase[] = [];
const receivedAt = "2026-10-13T12:00:00.000Z";

afterEach(() => {
  for (const database of opened.splice(0)) {
    database.close();
  }
});

async function emptyDatabase(): Promise<AthleticDatabase> {
  const directory = mkdtempSync(join(tmpdir(), "ath-provider-v1-"));
  const database = await createDatabase(join(directory, "athlete.ath.sqlite"), {
    createdAt: "2026-10-13T11:00:00.000Z",
  });
  opened.push(database);
  return database;
}

describe("v1 provider import", () => {
  it("imports WHOOP records, matches a repeat, and revisions a correction", async () => {
    const database = await emptyDatabase();
    const source = resolve(fixtures, "whoop");
    const preview = await previewProviderImport(database, source, {
      received_at: receivedAt,
    });
    expect(preview.provider).toBe("whoop");
    expect(preview.conflicts).toEqual([]);
    expect(preview.new_records.length).toBeGreaterThan(10);
    expect(preview.matching_records).toEqual([]);

    const imported = await importProviderRecords(database, preview);
    expect(imported.new_record_count).toBe(preview.new_records.length);
    const measurements = await listRecords(database, {
      record_type: "point_measurement",
      limit: 10_000,
    });
    expect(measurements.some(
      (record) =>
        record.record_type === "point_measurement" &&
        record.data.quantity === "hrv_rmssd" &&
        record.data.value.standard_value === 68.4,
    )).toBe(true);
    expect(measurements.every(
      (record) =>
        record.record_type !== "point_measurement" ||
        record.data.recorded_at === undefined ||
        record.data.recorded_at.endsWith(".000Z"),
    )).toBe(true);

    const repeat = await previewProviderImport(database, source, {
      received_at: "2026-10-14T12:00:00.000Z",
    });
    expect(repeat.conflicts).toEqual([]);
    expect(repeat.new_records).toEqual([]);
    expect(repeat.changed_records).toEqual([]);
    expect(repeat.matching_records).toHaveLength(preview.new_records.length);

    const correctedDirectory = mkdtempSync(join(tmpdir(), "ath-whoop-correction-"));
    cpSync(source, correctedDirectory, { recursive: true });
    const cycles = join(correctedDirectory, "physiological_cycles.csv");
    writeFileSync(
      cycles,
      readFileSync(cycles, "utf8").replace(",68.4,", ",69.4,"),
    );
    const correction = await previewProviderImport(database, correctedDirectory, {
      received_at: "2026-10-15T12:00:00.000Z",
    });
    expect(correction.conflicts).toEqual([]);
    expect(correction.new_records).toEqual([]);
    expect(correction.changed_records).toHaveLength(1);
    await importProviderRecords(database, correction);
    const revisions = await readRecordRevisions(
      database,
      correction.changed_records[0]!.record_id,
    );
    expect(revisions).toHaveLength(2);
    expect(revisions[1]).toMatchObject({
      revision: 2,
      became_known_at: "2026-10-15T12:00:00.000Z",
      became_known_at_basis: "provider_import",
      data: { value: { standard_value: 69.4 } },
    });
  });

  it("keeps Oura date-only scores as provider days rather than invented instants", async () => {
    const database = await emptyDatabase();
    const preview = await previewProviderImport(
      database,
      resolve(fixtures, "oura"),
      { received_at: receivedAt },
    );
    expect(preview.conflicts).toEqual([]);
    await importProviderRecords(database, preview);

    const scores = await listRecords(database, {
      record_type: "provider_score",
      limit: 10_000,
    });
    const readiness = scores.find(
      (record) =>
        record.record_type === "provider_score" &&
        record.data.metric === "readiness",
    );
    expect(readiness).toMatchObject({
      record_type: "provider_score",
      data: {
        input_timestamp: {
          timestamp_missing_reason: "not_provided_by_source",
        },
        provider_reporting_periods: [
          {
            period_type: "oura_day_unspecified",
            provider_date: "2026-08-09",
          },
        ],
      },
    });
    expect(
      readiness?.record_type === "provider_score"
        ? readiness.data.recorded_at
        : "wrong record type",
    ).toBeUndefined();
  });

  it("imports Apple dense samples as stable SQLite records", async () => {
    const database = await emptyDatabase();
    const preview = await previewProviderImport(
      database,
      resolve(fixtures, "apple"),
      { received_at: receivedAt },
    );
    expect(preview.provider).toBe("apple");
    expect(preview.conflicts).toEqual([]);
    expect(
      preview.new_records.some((record) => record.record_type === "measurement_sample"),
    ).toBe(true);
    await importProviderRecords(database, preview);

    const samples = await listRecords(database, {
      record_type: "measurement_sample",
      limit: 10_000,
    });
    expect(samples.length).toBeGreaterThan(0);
    expect(samples[0]).toMatchObject({
      record_type: "measurement_sample",
      current_revision: 1,
      data: {
        input_timestamp: {
          timestamp_basis: "reconstructed_from_series_start",
        },
        timestamp_reconstruction_method: "series_start_plus_offset_ms_v1",
      },
    });

    const repeat = await previewProviderImport(
      database,
      resolve(fixtures, "apple"),
      { received_at: "2026-10-14T12:00:00.000Z" },
    );
    expect(repeat.new_records).toEqual([]);
    expect(repeat.changed_records).toEqual([]);
    expect(repeat.conflicts).toEqual([]);
  });
});
