import { cpSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  calculate,
  createDatabase,
  importProviderRecords,
  listRecords,
  previewProviderImport,
  readRecordRevisions,
  type AthleticDatabase,
} from "../src/db/index.js";
import { databaseClient } from "../src/db/client.js";

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

async function sourceRecordId(
  database: AthleticDatabase,
  writer: string,
): Promise<string> {
  const sources = await listRecords(database, {
    record_type: "source",
    limit: 10_000,
  });
  const source = sources.find(
    (record) =>
      record.record_type === "source" &&
      record.data.data_writer === writer,
  );
  if (source === undefined) {
    throw new Error(`test source is missing: ${writer}`);
  }
  return source.record_id;
}

async function providerImportKeys(
  database: AthleticDatabase,
): Promise<Array<{ method: string; key: unknown[] }>> {
  const result = await databaseClient(database).execute(`
    SELECT match_key_method, match_key
    FROM record_import_keys
    ORDER BY match_key_method, match_key
  `);
  return result.rows.map((row) => ({
    method: String(row.match_key_method),
    key: JSON.parse(String(row.match_key)) as unknown[],
  }));
}

function duplicateCsvFirstRow(text: string): string {
  const [header, first, ...rest] = text.trimEnd().split(/\r?\n/);
  if (header === undefined || first === undefined) {
    throw new Error("test CSV needs a header and data row");
  }
  return [header, first, first, ...rest].join("\n") + "\n";
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
    expect(
      measurements.find(
        (record) =>
          record.record_type === "point_measurement" &&
          record.data.quantity === "hrv_rmssd" &&
          record.data.value.standard_value === 68.4,
      ),
    ).toMatchObject({
      data: {
        input_timestamp: {
          timestamp_text: "2026-08-09 06:05:00",
          utc_offset_text: "UTC-07:00",
          utc_offset_meaning: "provider_cycle_offset",
          timestamp_basis:
            "source_local_timestamp_with_documented_offset",
        },
        value: { input_value: 68.4, input_unit: "ms" },
        provider_reporting_periods: [
          {
            period_type: "whoop_physiological_cycle",
            start_at: "2026-08-09T05:10:00.000Z",
            end_at: "2026-08-09T13:20:00.000Z",
            timezone_offset_text: "UTC-07:00",
          },
        ],
      },
    });
    expect(measurements.every(
      (record) =>
        record.record_type !== "point_measurement" ||
        record.data.recorded_at === undefined ||
        record.data.recorded_at.endsWith(".000Z"),
    )).toBe(true);

    const recoveryBaseline = await calculate(database, {
      operation: "baseline",
      quantity: "recovery",
      record_kind: "provider-score",
    });
    expect(recoveryBaseline).toMatchObject({
      record_kind: "provider-score",
      unit: "0-100",
      result: [{ source_id: expect.any(String), count: 2, mean: 74 }],
    });
    const cycleStrain = await calculate(database, {
      operation: "provider-period-summary",
      quantity: "strain",
      record_kind: "provider-score",
      provider_period_type: "whoop_physiological_cycle",
    });
    expect(cycleStrain).toMatchObject({
      provider_period_type: "whoop_physiological_cycle",
      result: [
        {
          source_id: expect.any(String),
          periods: [
            {
              provider_period: {
                period_type: "whoop_physiological_cycle",
                start_at: "2026-08-09T05:10:00.000Z",
              },
              count: 1,
              mean: 14.2,
            },
            {
              provider_period: {
                period_type: "whoop_physiological_cycle",
                start_at: "2026-08-10T05:05:00.000Z",
              },
              count: 1,
              mean: 9.8,
            },
          ],
        },
      ],
    });
    const workoutHeartRateByCycle = await calculate(database, {
      operation: "provider-period-summary",
      quantity: "average_heart_rate",
      record_kind: "session-summary",
      provider_period_type: "whoop_physiological_cycle",
    });
    expect(workoutHeartRateByCycle).toMatchObject({
      unit: "bpm",
      result: [
        {
          periods: [
            {
              provider_period: {
                start_at: "2026-08-09T05:10:00.000Z",
              },
              mean: 142,
            },
            {
              provider_period: {
                start_at: "2026-08-10T05:05:00.000Z",
              },
              mean: 138,
            },
          ],
        },
      ],
    });

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

    const sourceId = await sourceRecordId(database, "WHOOP");
    await expect(providerImportKeys(database)).resolves.toEqual(
      expect.arrayContaining([
        {
          method: "whoop_csv/v1",
          key: ["source", "WHOOP", "whoop_csv", ""],
        },
        {
          method: "whoop_csv/v1",
          key: [
            "point_measurement",
            sourceId,
            "hrv_rmssd",
            "2026-08-09T13:05:00.000Z",
            0,
          ],
        },
        {
          method: "whoop_csv/v1",
          key: [
            "provider_score",
            sourceId,
            "recovery",
            "2026-08-09T13:05:00.000Z",
            0,
          ],
        },
      ]),
    );
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
        record.data.metric === "readiness" &&
        record.data.provider_reporting_periods?.some(
          (period) => period.provider_date === "2026-08-09",
        ),
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
    const readinessByOuraDay = await calculate(database, {
      operation: "provider-period-summary",
      quantity: "readiness",
      record_kind: "provider-score",
      provider_period_type: "oura_day_unspecified",
    });
    expect(readinessByOuraDay).toMatchObject({
      unit: "0-100",
      result: [
        {
          periods: [
            {
              provider_period: {
                period_type: "oura_day_unspecified",
                provider_date: "2026-08-09",
              },
              mean: 74,
            },
            {
              provider_period: {
                period_type: "oura_day_unspecified",
                provider_date: "2026-08-10",
              },
              mean: 88,
            },
          ],
        },
      ],
    });
    expect(
      scores.find(
        (record) =>
          record.record_type === "provider_score" &&
          record.data.metric === "sleep_score" &&
          record.data.provider_reporting_periods?.some(
            (period) => period.provider_date === "2026-08-09",
          ),
      ),
    ).toMatchObject({
      data: {
        recorded_at: "2026-08-09T13:08:00.000Z",
        input_timestamp: {
          timestamp_text: "2026-08-09T06:08:00-07:00",
        },
        provider_reporting_periods: [
          {
            period_type: "oura_sleep_day",
            provider_date: "2026-08-09",
          },
        ],
      },
    });

    const sourceId = await sourceRecordId(database, "Oura");
    await expect(providerImportKeys(database)).resolves.toEqual(
      expect.arrayContaining([
        {
          method: "oura_csv/v1",
          key: ["source", "Oura", "oura_csv", ""],
        },
        {
          method: "oura_csv/v1",
          key: [
            "provider_score",
            sourceId,
            "readiness",
            "2026-08-09",
            0,
          ],
        },
        {
          method: "oura_csv/v1",
          key: [
            "point_measurement",
            sourceId,
            "hrv_rmssd",
            "2026-08-09T13:08:00.000Z",
            0,
          ],
        },
      ]),
    );
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
    expect(
      samples.some(
        (record) =>
          record.record_type === "measurement_sample" &&
          record.data.input_timestamp.timestamp_text ===
            "2026-08-09 18:30:00 -0700" &&
          record.data.input_timestamp.utc_offset_text === "-0700" &&
          record.data.value.input_value === 9.8 &&
          record.data.value.input_unit === "kcal" &&
          record.data.timestamp_reconstruction_method === undefined,
      ),
    ).toBe(true);
    expect(
      samples.some(
        (record) =>
          record.record_type === "measurement_sample" &&
          record.data.input_timestamp.timestamp_basis ===
            "reconstructed_from_series_start" &&
          record.data.timestamp_reconstruction_method ===
            "series_start_plus_offset_ms_v1",
      ),
    ).toBe(true);
    expect(
      samples.find(
        (record) =>
          record.record_type === "measurement_sample" &&
          record.data.quantity === "heart_rate" &&
          record.data.value.standard_value === 62,
      ),
    ).toMatchObject({
      data: {
        input_timestamp: {
          source_time_zone: "America/Los_Angeles",
          source_time_zone_meaning:
            "apple_health_object_creation_time_zone",
        },
      },
    });

    const measurements = await listRecords(database, {
      record_type: "point_measurement",
      limit: 10_000,
    });
    expect(
      measurements.find(
        (record) =>
          record.record_type === "point_measurement" &&
          record.data.quantity === "body_weight",
      ),
    ).toMatchObject({
      data: {
        input_timestamp: {
          timestamp_text: "2026-08-09 07:02:00 -0700",
          utc_offset_text: "-0700",
        },
        value: {
          input_value: 181.4,
          input_unit: "lb",
          standard_unit: "kg",
        },
      },
    });
    expect(
      measurements.find(
        (record) =>
          record.record_type === "point_measurement" &&
          record.data.quantity === "resting_heart_rate" &&
          record.data.value.standard_value === 51,
      ),
    ).toMatchObject({
      data: {
        input_timestamp: {
          source_time_zone: "America/Los_Angeles",
          source_time_zone_meaning:
            "apple_health_object_creation_time_zone",
        },
      },
    });

    const summaries = await listRecords(database, {
      record_type: "provider_summary",
      limit: 10_000,
    });
    expect(summaries).toHaveLength(6);
    expect(
      summaries.find(
        (record) =>
          record.record_type === "provider_summary" &&
          record.data.summary_type === "active_energy_burned",
      ),
    ).toMatchObject({
      data: {
        value: {
          standard_value: 612.4,
          standard_unit: "kcal",
          input_value: 612.4,
          input_unit: "kcal",
        },
        input_timestamp: {
          timestamp_missing_reason: "not_provided_by_source",
        },
        provider_reporting_periods: [
          {
            period_type: "apple_activity_summary_day",
            provider_date: "2026-08-09",
          },
        ],
      },
    });
    expect(
      summaries.every(
        (record) =>
          record.record_type !== "provider_summary" ||
          record.data.recorded_at === undefined,
      ),
    ).toBe(true);
    const activeEnergyByAppleDay = await calculate(database, {
      operation: "provider-period-summary",
      quantity: "active_energy_burned",
      record_kind: "provider-summary",
      provider_period_type: "apple_activity_summary_day",
    });
    expect(activeEnergyByAppleDay).toMatchObject({
      unit: "kcal",
      result: [
        {
          periods: [
            {
              provider_period: {
                period_type: "apple_activity_summary_day",
                provider_date: "2026-08-09",
              },
              mean: 612.4,
            },
          ],
        },
      ],
    });

    const repeat = await previewProviderImport(
      database,
      resolve(fixtures, "apple"),
      { received_at: "2026-10-14T12:00:00.000Z" },
    );
    expect(repeat.new_records).toEqual([]);
    expect(repeat.changed_records).toEqual([]);
    expect(repeat.conflicts).toEqual([]);

    const withingsSourceId = await sourceRecordId(database, "Withings");
    const activitySourceId = await sourceRecordId(database, "Apple Health");
    await expect(providerImportKeys(database)).resolves.toEqual(
      expect.arrayContaining([
        {
          method: "apple_health/v1",
          key: ["source", "Withings", "apple_health", ""],
        },
        {
          method: "apple_health/v1",
          key: [
            "point_measurement",
            withingsSourceId,
            "body_weight",
            "2026-08-09T14:02:00.000Z",
            0,
          ],
        },
        {
          method: "apple_health/v1",
          key: [
            "provider_summary",
            activitySourceId,
            "active_energy_burned",
            "2026-08-09",
            0,
          ],
        },
      ]),
    );
  });

  it("keeps duplicate-row ordinals stable for every provider route", async () => {
    const cases = [
      {
        provider: "whoop",
        file: "physiological_cycles.csv",
        duplicate: duplicateCsvFirstRow,
        matches: (key: unknown[]) =>
          key[0] === "point_measurement" &&
          key[2] === "hrv_rmssd" &&
          key[3] === "2026-08-09T13:05:00.000Z",
      },
      {
        provider: "oura",
        file: "oura_daily_readiness.csv",
        duplicate: duplicateCsvFirstRow,
        matches: (key: unknown[]) =>
          key[0] === "provider_score" &&
          key[2] === "readiness" &&
          key[3] === "2026-08-09",
      },
      {
        provider: "apple",
        file: "export.xml",
        duplicate: (text: string) => {
          const record = text.match(
            / <Record type="HKQuantityTypeIdentifierRestingHeartRate"[^\n]+\/>/,
          )?.[0];
          if (record === undefined) {
            throw new Error("test Apple record is missing");
          }
          return text.replace(record, `${record}\n${record}`);
        },
        matches: (key: unknown[]) =>
          key[0] === "point_measurement" &&
          key[2] === "resting_heart_rate" &&
          key[3] === "2026-08-09T13:20:00.000Z",
      },
    ] as const;

    for (const testCase of cases) {
      const database = await emptyDatabase();
      const source = resolve(fixtures, testCase.provider);
      const directory = mkdtempSync(
        join(tmpdir(), `ath-${testCase.provider}-duplicate-`),
      );
      cpSync(source, directory, { recursive: true });
      const file = join(directory, testCase.file);
      writeFileSync(file, testCase.duplicate(readFileSync(file, "utf8")));

      const preview = await previewProviderImport(database, directory, {
        received_at: receivedAt,
      });
      expect(preview.conflicts, testCase.provider).toEqual([]);
      await importProviderRecords(database, preview);

      const keys = await providerImportKeys(database);
      expect(
        keys
          .filter(
            (row) =>
              row.method === `${testCase.provider === "apple" ? "apple_health" : `${testCase.provider}_csv`}/v1` &&
              testCase.matches(row.key),
          )
          .map((row) => row.key.at(-1))
          .sort(),
        testCase.provider,
      ).toEqual([0, 1]);

      const repeat = await previewProviderImport(database, directory, {
        received_at: "2026-10-14T12:00:00.000Z",
      });
      expect(repeat.conflicts, testCase.provider).toEqual([]);
      expect(repeat.new_records, testCase.provider).toEqual([]);
      expect(repeat.changed_records, testCase.provider).toEqual([]);
      expect(repeat.matching_records.length, testCase.provider).toBeGreaterThan(0);

      if (testCase.provider === "whoop") {
        writeFileSync(
          file,
          readFileSync(file, "utf8").replace(",68.4,", ",69.4,"),
        );
        const ambiguousCorrection = await previewProviderImport(
          database,
          directory,
          { received_at: "2026-10-15T12:00:00.000Z" },
        );
        expect(ambiguousCorrection.conflicts).toEqual([
          expect.objectContaining({
            message:
              "an indistinguishable duplicate group changed; the importer will not guess which row was corrected",
          }),
        ]);
        await expect(
          importProviderRecords(database, ambiguousCorrection),
        ).rejects.toThrow("unresolved conflicts");
      }
    }
  });

  it("keeps an Oura sleep-day measurement date-only when bedtime is absent", async () => {
    const database = await emptyDatabase();
    const directory = mkdtempSync(join(tmpdir(), "ath-oura-date-only-sleep-"));
    writeFileSync(
      join(directory, "oura_sleep.csv"),
      "day,average_hrv\n2026-08-09,63\n",
    );

    const preview = await previewProviderImport(database, directory, {
      received_at: receivedAt,
    });
    expect(preview.conflicts).toEqual([]);
    await importProviderRecords(database, preview);
    const measurements = await listRecords(database, {
      record_type: "point_measurement",
      limit: 100,
    });
    expect(measurements).toMatchObject([
      {
        data: {
          quantity: "hrv_rmssd",
          input_timestamp: {
            timestamp_missing_reason: "not_provided_by_source",
          },
          provider_reporting_periods: [
            {
              period_type: "oura_sleep_day",
              provider_date: "2026-08-09",
            },
          ],
        },
      },
    ]);
    expect(
      measurements[0]?.record_type === "point_measurement"
        ? measurements[0].data.recorded_at
        : "wrong record type",
    ).toBeUndefined();

    const bySleepDay = await calculate(database, {
      operation: "provider-period-summary",
      quantity: "hrv_rmssd",
      provider_period_type: "oura_sleep_day",
    });
    expect(bySleepDay).toMatchObject({
      unit: "ms",
      result: [
        {
          periods: [
            {
              provider_period: {
                period_type: "oura_sleep_day",
                provider_date: "2026-08-09",
              },
              count: 1,
              mean: 63,
            },
          ],
        },
      ],
    });
  });
});
