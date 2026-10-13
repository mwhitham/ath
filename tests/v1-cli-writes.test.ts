import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const here = dirname(fileURLToPath(import.meta.url));
const CLI = resolve(here, "../src/cli.ts");
const TSX = resolve(here, "../node_modules/.bin/tsx");
const FIXTURE = resolve(here, "../examples/demo-athlete/athlete.ath.sqlite");

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

  it("requires two confirmations and erases dependent records and history", () => {
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
  });

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
    expect(records(directory, "benchmark_result")).toHaveLength(1);
  });
});
