import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chmodSync, copyFileSync, mkdirSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { athCoreBinary } from "../src/ath-core.js";

const here = dirname(fileURLToPath(import.meta.url));
const CLI = resolve(here, "../src/cli.ts");
const TSX = resolve(here, "../node_modules/.bin/tsx");
const DATABASE = resolve(here, "fixtures/version-2-0-0/database.ath.sqlite");
const ATTACHMENT = resolve(
  here,
  "fixtures/version-2-0-0/package/attachments/01890f3e-e7d3-7cc0-88c8-00000000000e.png",
);
const DATABASE_ID = "01890f3e-e7d3-7cc0-88c8-000000000001";
const DATABASE_SHA256 = "442e9a966711660c2a03d701b5de2c55479a78330702083a3a67a182992a7856";

function run(command: string, args: string[], cwd: string): { stdout: string; stderr: string; code: number } {
  try {
    const stdout = execFileSync(command, args, { cwd, encoding: "utf8", stdio: "pipe" });
    return { stdout, stderr: "", code: 0 };
  } catch (error) {
    const err = error as { status: number; stdout?: string; stderr?: string };
    return { stdout: err.stdout ?? "", stderr: err.stderr ?? "", code: err.status ?? 1 };
  }
}

function sql(path: string, query: string): string {
  return execFileSync("sqlite3", [path, query], { encoding: "utf8" }).trim();
}

function counts(path: string): number[] {
  return [
    "records",
    "record_revisions",
    "record_attachment_links",
    "attachments",
    "record_deletions",
  ].map((table) => Number(sql(path, `SELECT count(*) FROM ${table}`)));
}

describe("a 2.0.0 database under 2.1.0", () => {
  it("migrate_version_2_database_to_schema_4", () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-version-2-0-0-"));
    chmodSync(directory, 0o700);
    const copy = join(directory, "athlete.ath.sqlite");
    copyFileSync(DATABASE, copy);
    chmodSync(copy, 0o600);
    mkdirSync(join(directory, "attachments"));
    copyFileSync(ATTACHMENT, join(directory, "attachments", "01890f3e-e7d3-7cc0-88c8-00000000000e.png"));
    expect(sql(copy, "SELECT athleticstandard_version || ' ' || sql_schema_version FROM database_metadata")).toBe(
      "2.0.0 3",
    );
    expect(counts(copy)).toEqual([25, 26, 1, 1, 1]);

    const ath = (args: string[]) => run(TSX, [CLI, ...args], directory);
    const refused = run(athCoreBinary(), ["show", "--database", copy, "--record", DATABASE_ID], directory);
    expect(refused.code).toBe(1);
    expect(refused.stderr.trim()).toBe("database version is 2.0.0; run ath migrate before using ath-core");

    const preview = ath(["migrate", "--file", copy, "--json"]);
    expect(preview.code, preview.stderr).toBe(0);
    const planned = JSON.parse(preview.stdout) as {
      migration_needed: boolean;
      source_sql_schema_version: number;
      target_sql_schema_version: number;
      planned_steps: Array<{ target_sql_schema_version: number; migration_file: string }>;
    };
    expect(planned.migration_needed).toBe(true);
    expect(planned.source_sql_schema_version).toBe(3);
    expect(planned.target_sql_schema_version).toBe(4);
    expect(planned.planned_steps.map((step) => step.migration_file)).toEqual([
      "migrations/004-version-2-1.sql",
    ]);
    expect(sql(copy, "SELECT athleticstandard_version || ' ' || sql_schema_version FROM database_metadata")).toBe(
      "2.0.0 3",
    );

    const migrated = ath(["migrate", "--yes", "--confirm-migration", DATABASE_ID, "--file", copy]);
    expect(migrated.code, migrated.stderr + migrated.stdout).toBe(0);
    expect(sql(copy, "SELECT athleticstandard_version || ' ' || sql_schema_version FROM database_metadata")).toBe(
      "2.1.0 4",
    );
    expect(sql(copy, "PRAGMA user_version")).toBe("4");
    expect(counts(copy)).toEqual([25, 26, 1, 1, 1]);
    const records = sql(copy, "SELECT count(*) FROM record_revisions WHERE record_json LIKE '%field_basis%'");
    expect(records).toBe("0");

    const check = ath(["check", "--file", copy]);
    expect(check.code, check.stdout + check.stderr).toBe(0);
    expect(check.stdout).toContain("valid Athletic Standard 2.1.0 database");

    const again = ath(["migrate", "--file", copy, "--json"]);
    expect(JSON.parse(again.stdout).migration_needed).toBe(false);
    expect(createHash("sha256").update(readFileSync(DATABASE)).digest("hex")).toBe(DATABASE_SHA256);
  }, 60_000);
});
