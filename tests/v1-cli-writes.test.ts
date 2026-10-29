import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { openDatabase, resolveEvidenceReference } from "../src/db/index.js";

const here = dirname(fileURLToPath(import.meta.url));
const CLI = resolve(here, "../src/cli.ts");
const TSX = resolve(here, "../node_modules/.bin/tsx");
const FIXTURE = resolve(here, "../examples/demo-athlete/athlete.ath.sqlite");
const FIXTURE_ATTACHMENT = resolve(
  here,
  "../examples/demo-athlete/attachments/01890f3e-e7d3-7cc0-88c8-00000000000e.png",
);

function ath(args: string[], cwd: string, stdin = ""): { stdout: string; code: number } {
  try {
    const stdout = execFileSync(TSX, [CLI, ...args], {
      cwd,
      input: stdin,
      encoding: "utf8",
      stdio: "pipe",
    });
    return { stdout, code: 0 };
  } catch (error) {
    const failure = error as { status: number; stdout?: string; stderr?: string };
    return {
      stdout: `${failure.stdout ?? ""}${failure.stderr ?? ""}`,
      code: failure.status,
    };
  }
}

function newDatabase(): string {
  const directory = mkdtempSync(join(tmpdir(), "ath-v1-cli-"));
  expect(ath(["init", "-y", "--no-skill"], directory).code).toBe(0);
  return directory;
}

function fixtureCopy(): string {
  const directory = mkdtempSync(join(tmpdir(), "ath-v1-cli-"));
  copyFileSync(FIXTURE, join(directory, "athlete.ath.sqlite"));
  mkdirSync(join(directory, "attachments"));
  copyFileSync(
    FIXTURE_ATTACHMENT,
    join(directory, "attachments", "01890f3e-e7d3-7cc0-88c8-00000000000e.png"),
  );
  return directory;
}

function records(directory: string, recordType: string) {
  const result = ath(
    ["records", "list", "--type", recordType, "--include-deleted", "--json"],
    directory,
  );
  expect(result.code).toBe(0);
  return JSON.parse(result.stdout).records as Array<Record<string, unknown>>;
}

describe("v1 ath log", () => {
  it("previews under --json and writes only after a separate confirmation", () => {
    const directory = newDatabase();
    const preview = ath(
      [
        "log",
        "RMSSD",
        "61",
        "ms",
        "--at",
        "2026-10-13T12:00:00.000Z",
        "--json",
        "--yes",
      ],
      directory,
    );
    expect(preview.code).toBe(0);
    expect(JSON.parse(preview.stdout)).toMatchObject({
      interpretation: "manual hrv_rmssd measurement",
      write_confirmed: false,
    });
    expect(records(directory, "point_measurement")).toHaveLength(0);

    const written = ath(
      [
        "log",
        "RMSSD",
        "61",
        "ms",
        "--at",
        "2026-10-13T12:00:00.000Z",
        "--yes",
      ],
      directory,
    );
    expect(written.code).toBe(0);
    expect(records(directory, "point_measurement")).toMatchObject([
      {
        data: {
          quantity: "hrv_rmssd",
          value: { standard_value: 61, standard_unit: "ms" },
        },
      },
    ]);
  });

  it("refuses ambiguous HRV rather than mixing RMSSD and SDNN", () => {
    const directory = newDatabase();
    const result = ath(["log", "HRV", "61", "ms", "--yes"], directory);
    expect(result.code).toBe(1);
    expect(result.stdout).toContain("Say RMSSD or SDNN");
    expect(records(directory, "point_measurement")).toHaveLength(0);
  });

  it("rolls back a structured batch when one record is invalid", () => {
    const directory = newDatabase();
    const batch = JSON.stringify([
      {
        record_type: "athlete_report",
        data: {
          report_type: "note",
          reported_at: "2026-10-13T12:00:00.000Z",
          input_reported_at: {
            timestamp_text: "2026-10-13T12:00:00.000Z",
            utc_offset_text: "Z",
            utc_offset_meaning: "documented_recording_offset",
            timestamp_basis: "source_instant",
          },
          reported_via: "text",
          note: "Valid first record",
        },
      },
      {
        record_type: "point_measurement",
        data: { source_id: "not-a-record-id" },
      },
    ]);
    const result = ath(["log", batch, "--yes"], directory);
    expect(result.code).toBe(1);
    expect(records(directory, "athlete_report")).toHaveLength(0);
  });

  it("refuses unknown top-level fields on a structured log", () => {
    const directory = newDatabase();
    const result = ath(
      [
        "log",
        JSON.stringify({
          record_type: "athlete_report",
          mood_emoji: "ok",
          data: {
            report_type: "note",
            reported_at: "2026-10-13T04:00:00.000Z",
            input_reported_at: {
              timestamp_text: "2026-10-13T04:00:00.000Z",
              utc_offset_text: "Z",
              utc_offset_meaning: "documented_recording_offset",
              timestamp_basis: "source_instant",
            },
            reported_via: "text",
            note: "x",
          },
        }),
        "--yes",
      ],
      directory,
    );
    expect(result.code).toBe(1);
    expect(result.stdout).toContain("unknown field");
    expect(result.stdout).toContain("mood_emoji");
    expect(records(directory, "athlete_report")).toHaveLength(0);
  });

  it("preserves valid extensions on a structured log", () => {
    const directory = newDatabase();
    const extensions = {
      "example.com/mood-app": { app_mood: "great" },
    };
    const result = ath(
      [
        "log",
        JSON.stringify({
          record_type: "athlete_report",
          extensions,
          data: {
            report_type: "note",
            reported_at: "2026-10-13T04:00:00.000Z",
            input_reported_at: {
              timestamp_text: "2026-10-13T04:00:00.000Z",
              utc_offset_text: "Z",
              utc_offset_meaning: "documented_recording_offset",
              timestamp_basis: "source_instant",
            },
            reported_via: "text",
            note: "x",
          },
        }),
        "--yes",
      ],
      directory,
    );
    expect(result.code).toBe(0);
    expect(records(directory, "athlete_report")).toMatchObject([
      { extensions },
    ]);
  });

  it("refuses invalid extensions on a structured log", () => {
    const directory = newDatabase();
    const note = {
      report_type: "note",
      reported_at: "2026-10-13T04:00:00.000Z",
      input_reported_at: {
        timestamp_text: "2026-10-13T04:00:00.000Z",
        utc_offset_text: "Z",
        utc_offset_meaning: "documented_recording_offset",
        timestamp_basis: "source_instant",
      },
      reported_via: "text",
      note: "x",
    };
    const invalidId = ath(
      [
        "log",
        JSON.stringify({
          record_type: "athlete_report",
          extensions: {
            "example.com/x": { app_mood: "great" },
          },
          data: note,
        }),
        "--yes",
      ],
      directory,
    );
    expect(invalidId.code).toBe(1);
    expect(invalidId.stdout).toContain("namespaced extension");
    expect(records(directory, "athlete_report")).toHaveLength(0);

    const forbiddenKey = ath(
      [
        "log",
        JSON.stringify({
          record_type: "athlete_report",
          extensions: {
            "example.com/mood-app": {
              source_id: "01890f3e-e7d3-7cc0-88c8-0000000000aa",
            },
          },
          data: note,
        }),
        "--yes",
      ],
      directory,
    );
    expect(forbiddenKey.code).toBe(1);
    expect(forbiddenKey.stdout).toContain("source_id");
    expect(records(directory, "athlete_report")).toHaveLength(0);
  });

  it.each(["prediction_result_link", "prediction_grade"])(
    "refuses caller-created %s records",
    (recordType) => {
      const directory = newDatabase();
      const result = ath(
        ["log", JSON.stringify({ record_type: recordType, data: {} }), "--yes"],
        directory,
      );
      expect(result.code).toBe(1);
      expect(result.stdout).toContain(
        `structured log cannot create ${recordType}; use ath grade`,
      );
      expect(records(directory, recordType)).toHaveLength(0);
    },
  );

  it("ingests and links an attachment only after confirmation", () => {
    const directory = newDatabase();
    const preview = ath(
      [
        "log",
        "Meal photo",
        "--attachment",
        FIXTURE_ATTACHMENT,
        "--json",
        "--yes",
      ],
      directory,
    );
    expect(preview.code).toBe(0);
    expect(JSON.parse(preview.stdout)).toMatchObject({
      write_confirmed: false,
      records: [
        { record_type: "attachment" },
        {
          record_type: "athlete_report",
          data: { attachment_ids: [expect.any(String)] },
        },
      ],
    });
    expect(records(directory, "attachment")).toHaveLength(0);

    expect(
      ath(
        [
          "log",
          "Meal photo",
          "--attachment",
          FIXTURE_ATTACHMENT,
          "--yes",
        ],
        directory,
      ).code,
    ).toBe(0);
    const attachments = records(directory, "attachment");
    const reports = records(directory, "athlete_report");
    expect(attachments).toHaveLength(1);
    expect(reports).toMatchObject([
      {
        data: {
          attachment_ids: [attachments[0]!.record_id],
        },
      },
    ]);
  });

  it("links and grades one open prediction when a result is unambiguous", () => {
    const directory = newDatabase();
    const benchmark = records(directory, "benchmark_definition").find(
      (record) =>
        (record.data as { benchmark_key: string }).benchmark_key === "fran",
    )!;
    const prediction = JSON.stringify({
      record_type: "prediction",
      data: {
        authorship: {
          author: {
            author_type: "agent",
            agent: "test-agent",
            model: "test-model",
          },
          authored_at: "2026-10-12T12:00:00.000Z",
          evidence: [],
          content_version: 1,
          status: "active",
        },
        benchmark_definition_id: benchmark.record_id,
        predicted_score: { duration_s: 280 },
        predicted_range: {
          low: { duration_s: 270 },
          high: { duration_s: 290 },
        },
        confidence: "moderate",
        reasoning: "Test prediction",
      },
    });
    expect(
      ath([
        "log",
        prediction,
        "--agent",
        "test-agent",
        "--at",
        "2026-10-12T12:00:00.000Z",
        "--yes",
      ], directory).code,
    ).toBe(0);

    const result = ath(
      [
        "log",
        "Fran",
        "in",
        "5:00",
        "rx",
        "--at",
        "2026-10-13T12:00:00.000Z",
        "--yes",
      ],
      directory,
    );
    expect(result.code).toBe(0);
    expect(result.stdout).toContain("wrote 3 records");
    expect(records(directory, "benchmark_result")).toHaveLength(1);
    expect(records(directory, "prediction_result_link")).toHaveLength(1);
    expect(records(directory, "prediction_grade")).toHaveLength(1);
  });

  it("logs a prediction error analysis as authored work", () => {
    const directory = fixtureCopy();
    const grade = records(directory, "prediction_grade")[0]!;
    const analysis = JSON.stringify({
      record_type: "prediction_error_analysis",
      data: {
        authorship: {
          author: {
            author_type: "agent",
            agent: "test-agent",
            model: "test-model",
          },
          authored_at: "2026-10-13T12:00:00.000Z",
          evidence: [
            {
              record_id: grade.record_id,
              revision: grade.current_revision,
            },
          ],
          content_version: 1,
          status: "active",
        },
        prediction_grade: {
          record_id: grade.record_id,
          revision: grade.current_revision,
        },
        error_direction: "slower",
        error_severity: "minor",
        causes: [],
        unexplained: true,
        lesson: "Collect another result before assigning a cause.",
      },
    });
    expect(
      ath(
        [
          "log",
          analysis,
          "--agent",
          "test-agent",
          "--at",
          "2026-10-13T12:00:00.000Z",
          "--yes",
        ],
        directory,
      ).code,
    ).toBe(0);
    expect(records(directory, "prediction_error_analysis")).toHaveLength(2);
  });
});

describe("v1 corrections, links, deletion, and explicit grading", () => {
  it("corrects by expected revision and retains history", () => {
    const directory = newDatabase();
    const profile = records(directory, "athlete_profile")[0]!;
    const data = { ...(profile.data as Record<string, unknown>), name: "Corrected" };
    const result = ath([
      "correct",
      String(profile.record_id),
      "--revision",
      "1",
      "--data",
      JSON.stringify(data),
      "--yes",
    ], directory);
    expect(result.code).toBe(0);
    const shown = JSON.parse(
      ath([
        "records",
        "show",
        String(profile.record_id),
        "--history",
        "--json",
      ], directory).stdout,
    );
    expect(shown.record).toMatchObject({
      current_revision: 2,
      data: { name: "Corrected" },
    });
    expect(shown.history).toHaveLength(2);
  });

  // Multi-command CLI cases use the same CI budget as their neighbors.
  it("adds an attachment and report correction in one database revision", () => {
    const directory = newDatabase();
    expect(ath(["log", "Meal note", "--yes"], directory).code).toBe(0);
    const report = records(directory, "athlete_report")[0]!;
    const before = JSON.parse(
      ath(["stats", "--json"], directory).stdout,
    ).database_revision as number;
    const result = ath(
      [
        "correct",
        String(report.record_id),
        "--revision",
        "1",
        "--data",
        JSON.stringify({
          ...(report.data as Record<string, unknown>),
          note: "Meal note with receipt",
        }),
        "--attachment",
        FIXTURE_ATTACHMENT,
        "--yes",
      ],
      directory,
    );
    expect(result.code).toBe(0);
    const after = JSON.parse(
      ath(["stats", "--json"], directory).stdout,
    ).database_revision as number;
    expect(after).toBe(before + 1);
    const attachment = records(directory, "attachment")[0]!;
    expect(records(directory, "athlete_report")).toMatchObject([
      {
        current_revision: 2,
        data: {
          note: "Meal note with receipt",
          attachment_ids: [attachment.record_id],
        },
      },
    ]);
  }, 15_000);

  it("links stable result and session IDs by creating a result revision", () => {
    const directory = fixtureCopy();
    const resultId = "01890f3e-e7d3-7cc0-88c8-00000000000b";
    const sessionId = "01890f3e-e7d3-7cc0-88c8-00000000000a";
    const result = ath([
      "link",
      resultId,
      sessionId,
      "--revision",
      "1",
      "--yes",
    ], directory);
    expect(result.code).toBe(0);
    expect(
      JSON.parse(
        ath(["records", "show", resultId, "--history", "--json"], directory).stdout,
      ),
    ).toMatchObject({ record: { current_revision: 2 } });
  });

  it("writes a deletion record while retaining the record and revisions", () => {
    const directory = newDatabase();
    const profile = records(directory, "athlete_profile")[0]!;
    const result = ath([
      "delete",
      String(profile.record_id),
      "--revision",
      "1",
      "--yes",
    ], directory);
    expect(result.code).toBe(0);
    expect(records(directory, "athlete_profile")).toMatchObject([
      { record_id: profile.record_id, is_deleted: true },
    ]);
  });

  it("requires two confirmations and erases dependent records and history", async () => {
    const directory = fixtureCopy();
    const resultId = "01890f3e-e7d3-7cc0-88c8-00000000000b";
    const preview = ath([
      "erase",
      resultId,
      "--revision",
      "1",
      "--json",
      "--yes",
      "--confirm-erasure",
      resultId,
    ], directory);
    expect(preview.code).toBe(0);
    expect(JSON.parse(preview.stdout)).toMatchObject({
      root_record_id: resultId,
      permanent: true,
      write_confirmed: false,
    });
    expect(records(directory, "benchmark_result")).toHaveLength(1);

    const oneConfirmation = ath([
      "erase",
      resultId,
      "--revision",
      "1",
      "--yes",
    ], directory);
    expect(oneConfirmation.code).toBe(1);
    expect(oneConfirmation.stdout).toContain("second confirmation");
    expect(records(directory, "benchmark_result")).toHaveLength(1);

    const erased = ath([
      "erase",
      resultId,
      "--revision",
      "1",
      "--yes",
      "--confirm-erasure",
      resultId,
    ], directory);
    expect(erased.code).toBe(0);
    expect(records(directory, "benchmark_result")).toHaveLength(0);
    expect(records(directory, "prediction_result_link")).toHaveLength(0);
    expect(records(directory, "prediction_grade")).toHaveLength(0);
    expect(records(directory, "prediction")).toHaveLength(1);

    const exportDirectory = join(directory, "after-erasure");
    expect(ath(["export", exportDirectory], directory).code).toBe(0);
    const document = JSON.parse(
      readFileSync(join(exportDirectory, "athlete.ath.json"), "utf8"),
    ) as { record_erasure_markers: Array<{ record_id: string }> };
    expect(document.record_erasure_markers.map((marker) => marker.record_id)).toContain(
      resultId,
    );

    const database = await openDatabase(join(directory, "athlete.ath.sqlite"), {
      onSecurityWarning: () => undefined,
    });
    try {
      await expect(
        resolveEvidenceReference(database, {
          record_id: resultId,
          revision: 1,
        }),
      ).resolves.toMatchObject({
        status: "erased",
        reference: {
          record_id: resultId,
          revision: 1,
        },
        erasure_marker: {
          record_id: resultId,
          reason_code: "athlete_request",
        },
      });
    } finally {
      database.close();
    }
  }, 15_000);

  // This also launches several CLI processes in sequence.
  it("explicit grade reuses an exact result link and writes no result", () => {
    const directory = fixtureCopy();
    const predictionId = "01890f3e-e7d3-7cc0-88c8-000000000010";
    const resultId = "01890f3e-e7d3-7cc0-88c8-00000000000b";
    expect(records(directory, "prediction_grade")).toHaveLength(1);
    const preview = ath([
      "grade",
      predictionId,
      "--result",
      resultId,
      "--json",
      "--yes",
    ], directory);
    expect(preview.code).toBe(0);
    expect(JSON.parse(preview.stdout)).toMatchObject({
      prediction_result_link: { action: "reuse" },
      write_confirmed: false,
    });
    expect(records(directory, "prediction_grade")).toHaveLength(1);

    expect(
      ath(["grade", predictionId, "--result", resultId, "--yes"], directory).code,
    ).toBe(0);
    expect(records(directory, "prediction_grade")).toHaveLength(2);
    expect(
      ath(
        [
          "grade",
          predictionId,
          "--result",
          resultId,
          "--calculation-version",
          "prediction-grade-v2-test",
          "--yes",
        ],
        directory,
      ).code,
    ).toBe(0);
    const grades = records(directory, "prediction_grade");
    expect(grades).toHaveLength(3);
    expect(
      grades.map((grade) => {
        const data = grade.data as {
          authorship: { author: { calculation_version?: string } };
          prediction_result_link: { revision: number };
        };
        return {
          calculation_version:
            data.authorship.author.calculation_version,
          link_revision: data.prediction_result_link.revision,
        };
      }),
    ).toEqual(
      expect.arrayContaining([
        {
          calculation_version: "prediction-grade-v1",
          link_revision: 1,
        },
        {
          calculation_version: "prediction-grade-v2-test",
          link_revision: 1,
        },
      ]),
    );
    expect(records(directory, "benchmark_result")).toHaveLength(1);
  }, 15_000);

  it("revises the existing result link when grading a corrected result", () => {
    const directory = fixtureCopy();
    const predictionId = "01890f3e-e7d3-7cc0-88c8-000000000010";
    const resultId = "01890f3e-e7d3-7cc0-88c8-00000000000b";
    const result = records(directory, "benchmark_result")[0]!;
    const correctedData = {
      ...(result.data as Record<string, unknown>),
      score: { duration_s: 301 },
    };
    expect(
      ath(
        [
          "correct",
          resultId,
          "--revision",
          "1",
          "--data",
          JSON.stringify(correctedData),
          "--yes",
        ],
        directory,
      ).code,
    ).toBe(0);

    const preview = ath(
      ["grade", predictionId, "--result", resultId, "--json"],
      directory,
    );
    expect(preview.code).toBe(0);
    expect(JSON.parse(preview.stdout)).toMatchObject({
      result: { record_id: resultId, revision: 2 },
      prediction_result_link: {
        action: "revise",
        current_revision: 1,
        new_revision: 2,
      },
    });

    expect(
      ath(["grade", predictionId, "--result", resultId, "--yes"], directory).code,
    ).toBe(0);
    const links = records(directory, "prediction_result_link");
    expect(links).toHaveLength(1);
    expect(links[0]).toMatchObject({
      current_revision: 2,
      data: {
        benchmark_result: { record_id: resultId, revision: 2 },
        content_version: 2,
      },
    });
    const grades = records(directory, "prediction_grade");
    expect(grades).toHaveLength(2);
    expect(
      grades.map((grade) =>
        (grade.data as {
          prediction_result_link: { revision: number };
        }).prediction_result_link.revision,
      ).sort(),
    ).toEqual([1, 2]);
  });
});

describe("a 17-digit value survives a correction", () => {
  it("stays valid after ath correct", () => {
    const directory = newDatabase();
    const sourceId = String(records(directory, "source")[0]?.record_id);
    const precise = 14.384010205470487;
    const written = ath(
      [
        "log",
        JSON.stringify({
          record_type: "workout_session",
          data: {
            source_id: sourceId,
            received_at: "2026-08-09T16:00:00.000Z",
            received_at_basis: "import_time",
            start_at: "2026-08-09T15:00:00.000Z",
            end_at: "2026-08-09T15:10:00.000Z",
            input_start_timestamp: {
              timestamp_text: "2026-08-09T15:00:00.000Z",
              utc_offset_text: "Z",
              utc_offset_meaning: "documented_recording_offset",
              timestamp_basis: "source_instant",
            },
            input_end_timestamp: {
              timestamp_text: "2026-08-09T15:10:00.000Z",
              utc_offset_text: "Z",
              utc_offset_meaning: "documented_recording_offset",
              timestamp_basis: "source_instant",
            },
            source_reported_summaries: [],
            route: [
              {
                recorded_at: "2026-08-09T15:00:00.000Z",
                latitude_deg: precise,
                longitude_deg: -105,
              },
            ],
          },
        }),
        "--yes",
      ],
      directory,
    );
    expect(written.code, written.stdout).toBe(0);
    const workout = records(directory, "workout_session")[0]!;
    const before = ath(["check"], directory);
    expect(before.code, before.stdout).toBe(0);
    expect(before.stdout).not.toContain("INVALID");
    const data = {
      ...(workout.data as Record<string, unknown>),
      maximum_heart_rate_bpm: 190,
    };
    const corrected = ath(
      [
        "correct",
        String(workout.record_id),
        "--revision",
        String(workout.current_revision),
        "--data",
        JSON.stringify(data),
        "--yes",
      ],
      directory,
    );
    expect(corrected.code, corrected.stdout).toBe(0);
    const after = ath(["check"], directory);
    expect(after.code, after.stdout).toBe(0);
    expect(after.stdout).not.toContain("INVALID");
    expect(after.stdout).toContain("valid");
  });
});
