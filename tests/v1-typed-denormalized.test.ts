import { rm } from "node:fs/promises";
import { afterEach, describe, expect, it } from "vitest";
import { checkDatabase, openDatabase } from "../src/db/index.js";
import { databaseClient } from "../src/db/client.js";
import type { AthleticDatabase } from "../src/db/index.js";
import { materializeNpm100ProviderDatabase } from "./helpers/npm100-database.js";

const openDatabases: AthleticDatabase[] = [];
const directories: string[] = [];

afterEach(async () => {
  for (const database of openDatabases.splice(0)) {
    database.close();
  }
  for (const directory of directories.splice(0)) {
    await rm(directory, { recursive: true, force: true });
  }
});

async function corruptColumn(databasePath: string, sql: string): Promise<void> {
  const database = await openDatabase(databasePath, { allowOutdatedSchema: true });
  openDatabases.push(database);
  await databaseClient(database).execute(sql);
  database.close();
  openDatabases.pop();
}

describe("typed denormalized check", () => {
  it("flags corrupted recorded_at on a point measurement", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    await corruptColumn(
      databasePath,
      `UPDATE point_measurements SET recorded_at = '2000-01-01T00:00:00.000Z'
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN point_measurements ON point_measurements.record_id = records.record_id
         WHERE records.record_type = 'point_measurement' LIMIT 1
       )`,
    );
    const database = await openDatabase(databasePath, { allowOutdatedSchema: true });
    openDatabases.push(database);
    const result = await checkDatabase(database);
    expect(result.issues.some((issue) => issue.check === "typed_denormalized")).toBe(true);
  });

  it("flags corrupted duration_ms on a measurement sample", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    await corruptColumn(
      databasePath,
      `UPDATE measurement_samples SET duration_ms = 12345
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN measurement_samples ON measurement_samples.record_id = records.record_id
         WHERE records.record_type = 'measurement_sample' LIMIT 1
       )`,
    );
    const database = await openDatabase(databasePath, { allowOutdatedSchema: true });
    openDatabases.push(database);
    const result = await checkDatabase(database);
    expect(result.issues.some((issue) => issue.check === "typed_denormalized")).toBe(true);
  });

  it("flags corrupted standard_value on a provider score", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    await corruptColumn(
      databasePath,
      `UPDATE provider_scores SET standard_value = 999
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN provider_scores ON provider_scores.record_id = records.record_id
         WHERE records.record_type = 'provider_score' LIMIT 1
       )`,
    );
    const database = await openDatabase(databasePath, { allowOutdatedSchema: true });
    openDatabases.push(database);
    const result = await checkDatabase(database);
    expect(result.issues.some((issue) => issue.check === "typed_denormalized")).toBe(true);
  });

  it("flags corrupted summary_type on a provider summary", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    await corruptColumn(
      databasePath,
      `UPDATE provider_summaries SET summary_type = 'corrupt'
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN provider_summaries ON provider_summaries.record_id = records.record_id
         WHERE records.record_type = 'provider_summary' LIMIT 1
       )`,
    );
    const database = await openDatabase(databasePath, { allowOutdatedSchema: true });
    openDatabases.push(database);
    const result = await checkDatabase(database);
    expect(result.issues.some((issue) => issue.check === "typed_denormalized")).toBe(true);
  });
});
