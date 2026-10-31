import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { athCoreBinary } from "../src/ath-core.js";

const here = dirname(fileURLToPath(import.meta.url));
const CLI = resolve(here, "../src/cli.ts");
const TSX = resolve(here, "../node_modules/.bin/tsx");
const GARMIN = resolve(here, "fixtures/exports/garmin/park-loop.json");
const STRAVA = resolve(here, "fixtures/exports/strava");
const ZONES = resolve(here, "../skills/heart-rate-zones/SKILL.md");

function ath(args: string[], cwd: string): { stdout: string; stderr: string; code: number } {
  try {
    const stdout = execFileSync(TSX, [CLI, ...args], { cwd, encoding: "utf8", stdio: "pipe" });
    return { stdout, stderr: "", code: 0 };
  } catch (error) {
    const err = error as { status: number; stdout?: string; stderr?: string };
    return { stdout: err.stdout ?? "", stderr: err.stderr ?? "", code: err.status ?? 1 };
  }
}

function core(args: string[]): { stdout: string; stderr: string; code: number } {
  try {
    const stdout = execFileSync(athCoreBinary(), args, { encoding: "utf8", stdio: "pipe" });
    return { stdout, stderr: "", code: 0 };
  } catch (error) {
    const err = error as { status: number; stdout?: string; stderr?: string };
    return { stdout: err.stdout ?? "", stderr: err.stderr ?? "", code: err.status ?? 1 };
  }
}

describe("version 2.0.0 shared library", () => {
  it("imports Garmin, checks the file, and runs the zone calculation", () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-v2-"));
    expect(ath(["init", "-y", "--empty"], dir).code).toBe(0);
    const preview = ath(["import", GARMIN, "--json"], dir);
    expect(preview.code, preview.stderr + preview.stdout).toBe(0);
    const previewBody = JSON.parse(preview.stdout) as { write_confirmed: boolean };
    expect(previewBody.write_confirmed).toBe(false);
    const imported = ath(["import", GARMIN, "--yes"], dir);
    expect(imported.code, imported.stderr + imported.stdout).toBe(0);
    const body = JSON.parse(imported.stdout.slice(imported.stdout.indexOf("{"))) as {
      understood: string[];
      preserved: string[];
      record_ids: string[];
    };
    expect(body.understood).toContain("name");
    expect(body.understood).toContain("max_heart_rate");
    expect(body.understood).not.toContain("maximum_heart_rate_bpm");
    expect(body.preserved).toContain("Feeling");
    expect(body.record_ids).toHaveLength(2);
    const recordId = body.record_ids[0]!;

    const workout = JSON.parse(ath(["records", "show", recordId, "--json"], dir).stdout) as {
      record: { current_revision: number; data: Record<string, unknown> };
    };
    expect(workout.record.data.maximum_heart_rate_bpm).toBeUndefined();
    expect(workout.record.data.field_basis).toEqual({ value: "provider", activity_type: "provider_label" });
    const summaries = JSON.parse(
      ath(["records", "list", "--type", "provider_summary", "--json"], dir).stdout,
    ).records as Array<{ data: { summary_type: string; value: { standard_value: number; standard_unit: string } } }>;
    expect(summaries).toEqual([
      expect.objectContaining({
        data: expect.objectContaining({
          summary_type: "max_heart_rate",
          value: expect.objectContaining({ standard_value: 200, standard_unit: "bpm" }),
          field_basis: { value: "provider" },
        }),
      }),
    ]);

    const check = ath(["check"], dir);
    expect(check.code, check.stdout + check.stderr).toBe(0);

    const missingMaximum = ath(["apply", "heart-rate-zones", "--record", recordId, "--json"], dir);
    expect(missingMaximum.code, missingMaximum.stderr + missingMaximum.stdout).toBe(0);
    expect(JSON.parse(missingMaximum.stdout).missing).toContain("maximum_heart_rate_bpm");

    const withMaximum = {
      ...workout.record.data,
      maximum_heart_rate_bpm: 200,
    };
    const corrected = ath(
      [
        "correct",
        recordId,
        "--revision",
        String(workout.record.current_revision),
        "--data",
        JSON.stringify(withMaximum),
        "--yes",
      ],
      dir,
    );
    expect(corrected.code, corrected.stdout).toBe(0);

    const zones = ath(["apply", "heart-rate-zones", "--record", recordId, "--json"], dir);
    expect(zones.code, zones.stderr + zones.stdout).toBe(0);
    const zoneBody = JSON.parse(zones.stdout) as { result: { seconds: number[] }; missing: string[] };
    expect(zoneBody.missing).toEqual([]);
    expect(zoneBody.result.seconds).toEqual([1, 1, 0, 1, 1]);

    const database = join(dir, "athlete.ath.sqlite");
    const revised = core([
      "revise",
      "--database",
      database,
      "--record",
      recordId,
      "--patch",
      JSON.stringify({ name: "Park Loop corrected" }),
    ]);
    expect(revised.code, revised.stderr).toBe(0);
    const shown = ath(["records", "show", recordId, "--json"], dir);
    expect(shown.code, shown.stderr).toBe(0);
    expect(shown.stdout).toContain("Park Loop corrected");

    const fromLibrary = core(["show", "--database", database, "--record", recordId]);
    expect(fromLibrary.code, fromLibrary.stderr).toBe(0);
    expect(JSON.parse(fromLibrary.stdout).revision).toBe(3);

    const again = ath(["import", GARMIN, "--yes"], dir);
    expect(again.code, again.stderr + again.stdout).toBe(0);
    const againBody = JSON.parse(again.stdout.slice(again.stdout.indexOf("{"))) as {
      record_ids: string[];
    };
    expect(againBody.record_ids).toEqual(body.record_ids);

    const strava = ath(["import", STRAVA, "--yes"], dir);
    expect(strava.code, strava.stderr + strava.stdout).toBe(0);
    const stravaBody = JSON.parse(strava.stdout.slice(strava.stdout.indexOf("{"))) as {
      preserved: string[];
      understood: string[];
      record_ids: string[];
    };
    expect(stravaBody.preserved).toContain("Gear");
    expect(stravaBody.understood).toContain("route");
    expect(stravaBody.record_ids).toHaveLength(1);
    const ride = JSON.parse(ath(["records", "show", stravaBody.record_ids[0]!, "--json"], dir).stdout) as {
      record: { data: Record<string, unknown> };
    };
    expect(ride.record.data.field_basis).toEqual({ value: "provider", activity_type: "provider_label" });
    const after = ath(["check"], dir);
    expect(after.code, after.stdout + after.stderr).toBe(0);

    const edited = join(dir, "zones-edited.md");
    writeFileSync(
      edited,
      readFileSync(ZONES, "utf8").replace("zone: 0.60", "zone: 0.50"),
    );
    const changed = core([
      "calculate",
      "--database",
      database,
      "--skill",
      edited,
      "--record",
      recordId,
    ]);
    expect(changed.code, changed.stderr).toBe(0);
    const changedBody = JSON.parse(changed.stdout) as { result: { seconds: number[] } };
    expect(changedBody.result.seconds).not.toEqual([1, 1, 0, 1, 1]);
    expect(readFileSync(ZONES, "utf8")).toContain("zone: 0.60");
  }, 20_000);
});
