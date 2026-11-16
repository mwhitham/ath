/**
 * The README imports, used the way a caller uses them: create a database,
 * write a device measurement, query it back, export the complete package,
 * and check the package document with the published Zod schema.
 *
 * Run: node --import tsx examples/readme.ts
 */
import { AthleticStandardPackage } from "athleticstandard/schema";
import {
  createDatabase,
  exportRecords,
  openDatabase,
  queryMeasurements,
  releaseLocalDatabase,
  writeRecords,
} from "athleticstandard/database";
import { mkdtemp, readFile, realpath, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const folder = await mkdtemp(join(await realpath(tmpdir()), "ath-example-"));
try {
  const path = join(folder, "athlete.ath.sqlite");
  const created = await createDatabase(path);
  const athlete = { author_type: "athlete" } as const;
  const missing = {
    timestamp_missing_reason: "not_provided_by_source",
    utc_offset_missing_reason: "not_provided_by_source",
  } as const;
  const [source] = (
    await writeRecords(created, [
      {
        record_type: "source",
        data: { source_type: "wearable", data_writer: "Example Band", data_route: "provider_export" },
        changed_by: athlete,
      },
    ])
  ).records;
  await writeRecords(created, [
    {
      record_type: "point_measurement",
      data: {
        source_id: source!.record_id,
        received_at: "2026-01-02T07:00:00.000Z",
        received_at_basis: "import_time",
        quantity: "hrv_rmssd",
        recorded_at: "2026-01-02T06:00:00.000Z",
        input_timestamp: missing,
        value: {
          standard_value: 48,
          standard_unit: "ms",
          input_value_missing_reason: "not_provided_by_source",
          input_unit_missing_reason: "not_provided_by_source",
        },
      },
      changed_by: athlete,
    },
  ]);
  await releaseLocalDatabase(created);

  const database = await openDatabase(path);
  const measurements = await queryMeasurements(database, { quantity: "hrv_rmssd" });
  const exported = await exportRecords(database, join(folder, "export"));
  await releaseLocalDatabase(database);

  const document: unknown = JSON.parse(await readFile(join(exported.directory, "athlete.ath.json"), "utf8"));
  const parsed = AthleticStandardPackage.parse(document);
  console.log(
    JSON.stringify({
      measurements: measurements.length,
      package_version: parsed.athleticstandard_version,
      package_records: parsed.records.length,
    }),
  );
} finally {
  await rm(folder, { recursive: true, force: true });
}
