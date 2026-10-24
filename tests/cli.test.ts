import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createClient } from "@libsql/client";
import { describe, expect, it } from "vitest";
import { pathToFileURL } from "node:url";
import { ATH_PACKAGE_VERSION } from "../src/version.js";

const here = dirname(fileURLToPath(import.meta.url));
const CLI = resolve(here, "../src/cli.ts");
const TSX = resolve(here, "../node_modules/.bin/tsx");
const FIXTURE = resolve(here, "../examples/demo-athlete/athlete.ath.sqlite");

function ath(args: string[], cwd: string): { stdout: string; code: number } {
  try {
    const stdout = execFileSync(TSX, [CLI, ...args], { cwd, encoding: "utf8", stdio: "pipe" });
    return { stdout, code: 0 };
  } catch (e) {
    const err = e as { status: number; stdout?: string; stderr?: string };
    return { stdout: (err.stdout ?? "") + (err.stderr ?? ""), code: err.status };
  }
}

describe("ath --version", () => {
  it("prints the npm package version, not the record format version", () => {
    const res = ath(["--version"], process.cwd());
    expect(res.code).toBe(0);
    expect(res.stdout.trim()).toBe(ATH_PACKAGE_VERSION);
  });
});

describe("ath init", () => {
  it("creates a valid SQLite database with a profile, manual source, and seeded benchmarks", () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-"));
    const res = ath([
      "init",
      "-y",
      "--name",
      "Test",
      "--birth-year",
      "1990",
      "--sex",
      "male",
      "--timezone",
      "America/Chicago",
      "--no-skill",
    ], dir);
    expect(res.code).toBe(0);

    const profile = ath(["records", "list", "--type", "athlete_profile", "--json"], dir);
    expect(profile.code).toBe(0);
    const profileJson = JSON.parse(profile.stdout);
    expect(profileJson.records[0].data).toMatchObject({
      name: "Test",
      birth_year: 1990,
      sex: "male",
      default_time_zone: "America/Chicago",
    });

    const benchmarks = JSON.parse(
      ath(["records", "list", "--type", "benchmark_definition", "--json"], dir).stdout,
    );
    expect(benchmarks.records.map((record: { data: { benchmark_key: string } }) =>
      record.data.benchmark_key,
    )).toContain("fran");

    const check = ath(["check"], dir);
    expect(check.code).toBe(0);
    expect(check.stdout).toContain("valid Athletic Standard 1.0.0 database");
  });

  it("refuses to overwrite an existing file", () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-"));
    expect(ath(["init", "-y"], dir).code).toBe(0);
    const second = ath(["init", "-y"], dir);
    expect(second.code).toBe(1);
    expect(second.stdout).toContain("already exists");
    expect(second.stdout).toContain("--file");
  });
});

describe("ath migrate", () => {
  it("previews that no migration is required for a current database", () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-migrate-"));
    copyFileSync(FIXTURE, join(dir, "athlete.ath.sqlite"));
    const res = ath(["migrate", "--json"], dir);
    expect(res.code).toBe(0);
    const json = JSON.parse(res.stdout);
    expect(json.migration_needed).toBe(false);
    expect(json.source_sql_schema_version).toBe(2);
    expect(json.target_sql_schema_version).toBe(2);
  });

  it("does not switch journal mode during a dry-run preview", async () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-migrate-journal-"));
    const databasePath = join(dir, "athlete.ath.sqlite");
    copyFileSync(FIXTURE, databasePath);
    const reset = createClient({ url: pathToFileURL(databasePath).href });
    await reset.execute("PRAGMA journal_mode = DELETE");
    reset.close();

    const res = ath(["migrate", "--dry-run"], dir);
    expect(res.code).toBe(0);

    const after = createClient({ url: pathToFileURL(databasePath).href });
    const mode = await after.execute("PRAGMA journal_mode");
    expect(String(mode.rows[0]?.journal_mode).toLowerCase()).toBe("delete");
    after.close();
  });
});

describe("ath check", () => {
  it("passes the SQLite fixture", () => {
    const res = ath(["check", FIXTURE], process.cwd());
    expect(res.code).toBe(0);
    expect(res.stdout).toContain("valid Athletic Standard");
  });

  it("can validate a complete temporary export without leaving it behind", () => {
    const res = ath(
      ["check", FIXTURE, "--validate-export", "--json"],
      process.cwd(),
    );
    expect(res.code).toBe(0);
    expect(JSON.parse(res.stdout)).toMatchObject({
      valid: true,
      export_validated: true,
    });
  });

  it("fails a foreign-key violation with a pointed message", async () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-"));
    const copy = join(dir, "athlete.ath.sqlite");
    copyFileSync(FIXTURE, copy);
    const client = createClient({ url: pathToFileURL(copy).href });
    await client.execute("PRAGMA foreign_keys = OFF");
    await client.execute(
      "UPDATE point_measurements SET source_id = '01890f3e-e7d3-7cc0-88c8-ffffffffffff'",
    );
    client.close();
    const res = ath(["check"], dir);
    expect(res.code).toBe(1);
    expect(res.stdout).toContain("foreign_key");
  });

  it("fails cleanly when no file exists", () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-"));
    const res = ath(["check"], dir);
    expect(res.code).toBe(1);
    expect(res.stdout).toContain("ath init");
  });
});

describe("ath stats", () => {
  it("summarizes the fixture with source-specific baselines and series coverage", () => {
    const res = ath(["stats", FIXTURE], process.cwd());
    expect(res.code).toBe(0);
    expect(res.stdout).toContain("current records: 24");
    expect(res.stdout).toContain("hrv_rmssd");
    expect(res.stdout).toContain("90-day device baselines");
    expect(res.stdout).toMatch(/n=\d+/);
    expect(res.stdout).toContain("benchmarks defined: 1");
    expect(res.stdout).toContain("heart_rate");
  });
});

describe("ath series", () => {
  it("reads raw samples and calendar-day summaries from SQLite", () => {
    const raw = ath(["series", "heart_rate", "--raw", "--file", FIXTURE, "--json"], process.cwd());
    expect(raw.code).toBe(0);
    expect(JSON.parse(raw.stdout).samples).toHaveLength(1);

    const daily = ath(["series", "heart_rate", "--file", FIXTURE, "--json"], process.cwd());
    expect(daily.code).toBe(0);
    expect(JSON.parse(daily.stdout)).toMatchObject({
      time_zone: "America/Chicago",
      days: [{ date: "2026-08-09", sample_count: 1 }],
    });
  });
});

describe("ath records", () => {
  it("lists stable IDs and shows a retained revision with links", () => {
    const listed = ath([
      "records",
      "list",
      "--type",
      "point_measurement",
      "--file",
      FIXTURE,
      "--json",
    ], process.cwd());
    expect(listed.code).toBe(0);
    const id = JSON.parse(listed.stdout).records[0].record_id;

    const shown = ath([
      "records",
      "show",
      id,
      "--revision",
      "1",
      "--history",
      "--links",
      "--file",
      FIXTURE,
      "--json",
    ], process.cwd());
    expect(shown.code).toBe(0);
    expect(JSON.parse(shown.stdout)).toMatchObject({
      shown_revision: { revision: 1 },
      history: [{ revision: 1 }, { revision: 2 }],
    });
  });
});

describe("SQLite discovery", () => {
  it("refuses a prerelease JSON file with a clear message", () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-"));
    copyFileSync(
      resolve(here, "../examples/demo-athlete/athlete.ath.json"),
      join(dir, "athlete.ath.json"),
    );
    const res = ath(["check"], dir);
    expect(res.code).toBe(1);
    expect(res.stdout).toContain("prerelease JSON files are not supported");
    expect(res.stdout).toContain("ath init");
  });
});
