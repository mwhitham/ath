import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  generatePerformanceDatabase,
  PERFORMANCE_FIXTURE_SEED,
} from "../scripts/performance/generate-database.js";
import { runPerformanceBenchmarks } from "../scripts/performance/run-benchmarks.js";
import { checkDatabase, openDatabase } from "../src/db/index.js";
import { databaseClient } from "../src/db/client.js";

const temporaryDirectories: string[] = [];

afterEach(async () => {
  await Promise.all(
    temporaryDirectories.splice(0).map((path) =>
      rm(path, { recursive: true, force: true }),
    ),
  );
});

describe("v1 performance database generator", () => {
  it("builds the requested deterministic mix without dropping fixture records", async () => {
    const root = await mkdtemp(join(tmpdir(), "ath-v1-performance-"));
    temporaryDirectories.push(root);
    const generated = await generatePerformanceDatabase({
      output_directory: join(root, "10k"),
      record_count: 10_000,
    });

    expect(generated).toMatchObject({
      record_count: 10_000,
      seed: PERFORMANCE_FIXTURE_SEED,
    });
    const database = await openDatabase(generated.database_path, {
      onSecurityWarning: () => undefined,
    });
    try {
      const client = databaseClient(database);
      const counts = await client.execute(`
        SELECT
          (SELECT COUNT(*) FROM records) AS records,
          (SELECT COUNT(*) FROM measurement_samples) AS samples,
          (SELECT COUNT(DISTINCT source_id) FROM measurement_samples) AS sample_sources,
          (SELECT COUNT(*) FROM record_revisions WHERE revision > 1) AS corrections,
          (SELECT COUNT(*) FROM record_deletions) AS deletions
      `);
      expect(Number(counts.rows[0]?.records)).toBe(10_000);
      expect(Number(counts.rows[0]?.samples)).toBe(generated.sample_count + 1);
      expect(Number(counts.rows[0]?.sample_sources)).toBe(3);
      expect(Number(counts.rows[0]?.corrections)).toBeGreaterThan(0);
      expect(Number(counts.rows[0]?.deletions)).toBeGreaterThan(0);
      const workoutSamples = await client.execute({
        sql: `
          SELECT
            COUNT(*) AS count,
            MIN(recorded_at) AS first_at,
            MAX(recorded_at) AS last_at
          FROM measurement_samples
          WHERE source_id = ?
            AND quantity = 'heart_rate'
            AND recorded_at >= '2026-08-10T17:25:00.000Z'
            AND recorded_at <= '2026-08-10T17:48:00.000Z'
        `,
        args: ["01890f3e-e7d3-7cc0-88c8-000000000003"],
      });
      expect(Number(workoutSamples.rows[0]?.count)).toBeGreaterThan(2);
      expect(String(workoutSamples.rows[0]?.first_at)).toBe(
        "2026-08-10T17:25:00.000Z",
      );
      expect(
        String(workoutSamples.rows[0]?.last_at) >
          "2026-08-10T17:36:30.000Z",
      ).toBe(true);

      const types = await client.execute(
        "SELECT COUNT(DISTINCT record_type) AS count FROM records",
      );
      expect(Number(types.rows[0]?.count)).toBe(24);

      const evidencePlan = await client.execute(`
        EXPLAIN QUERY PLAN
        WITH visible_revisions AS (
          SELECT
            candidate_records.record_id,
            (
              SELECT MAX(candidate_revisions.revision)
              FROM record_revisions AS candidate_revisions
              WHERE candidate_revisions.record_id = candidate_records.record_id
                AND candidate_revisions.became_known_at <= '2026-10-13T23:59:59.999Z'
            ) AS revision
          FROM records AS candidate_records
          WHERE candidate_records.record_type IN (
            'benchmark_definition',
            'source',
            'point_measurement',
            'athlete_report',
            'sleep_session',
            'workout_session',
            'benchmark_result',
            'prediction'
          )
        )
        SELECT visible_revisions.record_id
        FROM visible_revisions
        JOIN record_revisions
          ON record_revisions.record_id = visible_revisions.record_id
         AND record_revisions.revision = visible_revisions.revision
        LIMIT 100000
      `);
      expect(
        evidencePlan.rows.some((row) =>
          String(row.detail).includes("records_record_type_idx"),
        ),
      ).toBe(true);

      const integrity = await client.execute("PRAGMA integrity_check");
      expect(integrity.rows).toEqual([{ integrity_check: "ok" }]);
      const foreignKeys = await client.execute("PRAGMA foreign_key_check");
      expect(foreignKeys.rows).toEqual([]);
      const check = await checkDatabase(database);
      expect(check.issues).toEqual([]);
      expect(check.valid).toBe(true);
    } finally {
      database.close();
    }
  });

  it("runs all nine release measurements and writes a report", async () => {
    const root = await mkdtemp(join(tmpdir(), "ath-v1-performance-run-"));
    temporaryDirectories.push(root);
    const generated = await generatePerformanceDatabase({
      output_directory: join(root, "database"),
      record_count: 10_000,
    });
    const report = await runPerformanceBenchmarks({
      database_path: generated.database_path,
      output_path: join(root, "result.json"),
      read_repetitions: 1,
      keep_work: false,
      measured_at: "2026-10-13T06:00:00.000Z",
    });

    expect(report.performance_seed).toBe(PERFORMANCE_FIXTURE_SEED);
    expect(report.target_record_count).toBe(10_000);
    expect(report.operations.map((operation) => operation.id)).toEqual([
      "latest_value",
      "evidence_28d",
      "baseline_90d",
      "workout_drift",
      "cross_source_comparison",
      "full_json_export",
      "database_check",
      "initial_import",
      "incremental_import",
    ]);
    expect(report.operations.every((operation) => operation.success)).toBe(true);
  }, 30_000);
});
