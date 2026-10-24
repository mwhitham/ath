import { copyFile, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { checkDatabase, openDatabase } from "../src/db/index.js";
import { databaseClient } from "../src/db/client.js";
import type { AthleticDatabase } from "../src/db/index.js";
import { ath } from "./helpers/cli-exec.js";
import { prepareMigratedNpm100 } from "./helpers/npm100-prepared.js";

const openDatabases: AthleticDatabase[] = [];
const directories: string[] = [];
let migratedTemplatePath = "";
let migratedTemplateDir = "";

beforeAll(async () => {
  const prepared = await prepareMigratedNpm100();
  migratedTemplatePath = prepared.databasePath;
  migratedTemplateDir = prepared.directory;
}, 300_000);

afterAll(async () => {
  if (migratedTemplateDir !== "") {
    await rm(migratedTemplateDir, { recursive: true, force: true });
  }
});

async function freshMigratedDatabase(): Promise<{ directory: string; databasePath: string }> {
  const directory = await mkdtemp(join(tmpdir(), "ath-denorm-"));
  directories.push(directory);
  const databasePath = join(directory, "athlete.ath.sqlite");
  await copyFile(migratedTemplatePath, databasePath);
  return { directory, databasePath };
}

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

async function expectDenormalizedRepairCycle(
  directory: string,
  databasePath: string,
  corruptSql: string,
): Promise<void> {
  await corruptColumn(databasePath, corruptSql);
  const database = await openDatabase(databasePath);
  openDatabases.push(database);
  const before = await checkDatabase(database);
  expect(before.issues.some((issue) => issue.check === "typed_denormalized")).toBe(true);
  database.close();
  openDatabases.pop();

  const repair = ath(["repair", "-y"], directory);
  expect(repair.code, repair.stdout).toBe(0);

  const check = ath(["check"], directory);
  expect(check.code, check.stdout).toBe(0);
  expect(check.stdout).toContain("valid Athletic Standard");
}

describe("typed denormalized check and repair", () => {
  it("recorded_at mismatch is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE point_measurements SET recorded_at = '2000-01-01T00:00:00.000Z'
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN point_measurements ON point_measurements.record_id = records.record_id
         WHERE records.record_type = 'point_measurement' LIMIT 1
       )`,
    );
  });

  it("NULL recorded_at vs data_json value is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE point_measurements SET recorded_at = NULL
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN point_measurements ON point_measurements.record_id = records.record_id
         WHERE records.record_type = 'point_measurement'
           AND point_measurements.recorded_at IS NOT NULL
         LIMIT 1
       )`,
    );
  });

  it("quantity mismatch is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE point_measurements SET quantity = 'heart_rate'
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN point_measurements ON point_measurements.record_id = records.record_id
         WHERE records.record_type = 'point_measurement'
           AND point_measurements.quantity != 'heart_rate'
         LIMIT 1
       )`,
    );
  });

  it("source_id mismatch is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE point_measurements SET source_id = (
         SELECT sources.record_id
         FROM sources
         WHERE sources.record_id <> point_measurements.source_id
         LIMIT 1
       )
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN point_measurements ON point_measurements.record_id = records.record_id
         WHERE records.record_type = 'point_measurement' LIMIT 1
       )`,
    );
  });

  it("received_at mismatch is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE point_measurements SET received_at = '2000-01-01T00:00:00.000Z'
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN point_measurements ON point_measurements.record_id = records.record_id
         WHERE records.record_type = 'point_measurement' LIMIT 1
       )`,
    );
  });

  it("duration_ms mismatch is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE measurement_samples SET duration_ms = 12345
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN measurement_samples ON measurement_samples.record_id = records.record_id
         WHERE records.record_type = 'measurement_sample' LIMIT 1
       )`,
    );
  });

  it("standard_unit mismatch is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE point_measurements SET standard_unit = 'bogus'
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN point_measurements ON point_measurements.record_id = records.record_id
         WHERE records.record_type = 'point_measurement' LIMIT 1
       )`,
    );
  });

  it("standard_value mismatch is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE provider_scores SET standard_value = 999
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN provider_scores ON provider_scores.record_id = records.record_id
         WHERE records.record_type = 'provider_score' LIMIT 1
       )`,
    );
  });

  it("metric mismatch is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE provider_scores SET metric = 'bogus_metric'
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN provider_scores ON provider_scores.record_id = records.record_id
         WHERE records.record_type = 'provider_score' LIMIT 1
       )`,
    );
  });

  it("scale mismatch is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE provider_scores SET scale = 'bogus'
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN provider_scores ON provider_scores.record_id = records.record_id
         WHERE records.record_type = 'provider_score' LIMIT 1
       )`,
    );
  });

  it("summary_type mismatch is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE provider_summaries SET summary_type = 'corrupt'
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN provider_summaries ON provider_summaries.record_id = records.record_id
         WHERE records.record_type = 'provider_summary' LIMIT 1
       )`,
    );
  });

  it("sleep start_at mismatch is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE sleep_sessions SET start_at = '2000-01-01T00:00:00.000Z'
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN sleep_sessions ON sleep_sessions.record_id = records.record_id
         WHERE records.record_type = 'sleep_session' LIMIT 1
       )`,
    );
  });

  it("sleep end_at mismatch is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE sleep_sessions SET end_at = '2099-12-31T23:59:59.000Z'
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN sleep_sessions ON sleep_sessions.record_id = records.record_id
         WHERE records.record_type = 'sleep_session' LIMIT 1
       )`,
    );
  });

  it("workout activity_type mismatch is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE workout_sessions SET activity_type = 'corrupt_activity'
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN workout_sessions ON workout_sessions.record_id = records.record_id
         WHERE records.record_type = 'workout_session'
           AND workout_sessions.activity_type IS NOT NULL
         LIMIT 1
       )`,
    );
  });

  it("athlete_report report_type mismatch is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE athlete_reports SET report_type = 'corrupt_report'
       WHERE record_id = (
         SELECT records.record_id FROM records
         JOIN athlete_reports ON athlete_reports.record_id = records.record_id
         WHERE records.record_type = 'athlete_report' LIMIT 1
       )`,
    );
  });

  it("provider_reporting_period period_type mismatch is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE provider_reporting_periods SET period_type = 'corrupt_period'
       WHERE rowid = (SELECT rowid FROM provider_reporting_periods LIMIT 1)`,
    );
  });

  it("provider_reporting_period provider_date mismatch is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE provider_reporting_periods SET provider_date = '2000-01-01'
       WHERE rowid = (
         SELECT rowid FROM provider_reporting_periods WHERE provider_date IS NOT NULL LIMIT 1
       )`,
    );
  });

  it("workout segment_json mismatch is flagged and repaired", async () => {
    const { directory, databasePath } = await freshMigratedDatabase();
    await expectDenormalizedRepairCycle(
      directory,
      databasePath,
      `UPDATE workout_session_segments SET segment_json = '{"label":"corrupt","source_reported_summaries":[]}'
       WHERE rowid = (SELECT rowid FROM workout_session_segments LIMIT 1)`,
    );
  });
});
