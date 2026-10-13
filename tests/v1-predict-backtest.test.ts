import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  databaseStats,
  openDatabase,
  runV1Backtest,
  writeRecord,
  type AthleticDatabase,
} from "../src/db/index.js";

const cli = resolve("src/cli.ts");
const tsx = resolve("node_modules/.bin/tsx");
const fixture = resolve("examples/demo-athlete/athlete.ath.sqlite");
const databases: AthleticDatabase[] = [];

afterEach(() => {
  for (const database of databases.splice(0)) {
    database.close();
  }
});

describe("v1 prediction evidence and backtesting", () => {
  it("returns bounded SQLite evidence without calling a gateway", () => {
    const stdout = execFileSync(
      tsx,
      [
        cli,
        "predict",
        "fran",
        "--file",
        fixture,
        "--as-of",
        "2026-08-11T12:00:00.000Z",
        "--json",
      ],
      { encoding: "utf8", stdio: "pipe" },
    );
    expect(JSON.parse(stdout)).toMatchObject({
      purpose: "prediction_evidence",
      is_prediction: false,
      benchmark: {
        record_type: "benchmark_definition",
        data: { benchmark_key: "fran" },
      },
      prior_results: expect.any(Array),
      device_measurements: expect.any(Array),
      athlete_reports: expect.any(Array),
      gaps: expect.any(Array),
    });
  });

  it("replays a later result without changing athlete data", async () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-v1-backtest-"));
    const path = join(directory, "athlete.ath.sqlite");
    copyFileSync(fixture, path);
    const database = await openDatabase(path, {
      onSecurityWarning: () => undefined,
    });
    databases.push(database);

    await writeRecord(database, {
      record_type: "benchmark_result",
      record_id: "01890f3e-e7d3-7cc0-88c8-0000000000bb",
      created_at: "2026-08-11T18:00:00.000Z",
      became_known_at: "2026-08-11T18:00:00.000Z",
      changed_by: { author_type: "athlete" },
      data: {
        source_id: "01890f3e-e7d3-7cc0-88c8-000000000004",
        received_at: "2026-08-11T18:00:00.000Z",
        received_at_basis: "local_write_time",
        benchmark_definition_id:
          "01890f3e-e7d3-7cc0-88c8-000000000006",
        recorded_at: "2026-08-11T17:55:00.000Z",
        input_timestamp: {
          timestamp_text: "2026-08-11T17:55:00.000Z",
          utc_offset_text: "Z",
          utc_offset_meaning: "documented_recording_offset",
          timestamp_basis: "source_instant",
        },
        score: { duration_s: 295 },
        scaling: "rx",
      },
    });
    const before = await databaseStats(database);

    const report = await runV1Backtest(database, {
      models: ["test-model"],
      gateway: "vercel",
      created_at: "2026-10-13T12:00:00.000Z",
      complete: async () => ({
        content: JSON.stringify({
          predicted_score: { duration_s: 290 },
        }),
        model: "test-model",
      }),
    });

    expect(report).toMatchObject({
      replayable_result_count: 1,
      skipped_first_result_count: 1,
      replays: [
        {
          model: "test-model",
          predicted_value: 290,
          actual_value: 295,
          absolute_error_percentage: 1.7,
        },
      ],
      summary: [
        {
          model: "test-model",
          completed: 1,
          failed: 0,
          median_absolute_error_percentage: 1.7,
        },
      ],
    });
    expect((await databaseStats(database)).database_revision).toBe(
      before.database_revision,
    );
  });
});
