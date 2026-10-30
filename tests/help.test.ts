import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const here = dirname(fileURLToPath(import.meta.url));
const CLI = resolve(here, "../src/cli.ts");
const TSX = resolve(here, "../node_modules/.bin/tsx");

function ath(args: string[], cwd: string): { stdout: string; code: number } {
  const res = spawnSync(TSX, [CLI, ...args], { cwd, encoding: "utf8" });
  return { stdout: (res.stdout ?? "") + (res.stderr ?? ""), code: res.status ?? 1 };
}

const emptyDir = () => mkdtempSync(join(tmpdir(), "ath-help-"));

function dirWithFile(): string {
  const dir = emptyDir();
  expect(ath(["init", "-y", "--timezone", "America/Chicago"], dir).code).toBe(0);
  return dir;
}

describe("ath --help — written for someone who has never run this (D50)", () => {
  it("groups the commands by what the reader is trying to do", () => {
    const res = ath(["--help"], emptyDir());
    expect(res.stdout).toContain("Set up:");
    expect(res.stdout).toContain("Get your data in:");
    expect(res.stdout).toContain("Read your data:");
    expect(res.stdout).toContain("Predict and grade:");
    // Set up comes before predicting, because that is the order a reader meets them.
    expect(res.stdout.indexOf("Set up:")).toBeLessThan(res.stdout.indexOf("Get your data in:"));
    expect(res.stdout.indexOf("Read your data:")).toBeLessThan(res.stdout.indexOf("Predict and grade:"));
  });

  it(
    "gives every command worked examples with real values",
    () => {
      for (const command of [
        "init",
        "migrate",
        "check",
        "import",
        "conflicts",
        "export",
        "log",
        "link",
        "correct",
        "delete",
        "erase",
        "apply",
        "pull",
        "calculate",
        "predict",
        "grade",
        "stats",
        "series",
        "records",
        "key",
        "models",
        "backtest",
        "share",
        "skill",
      ]) {
        const res = ath([command, "--help"], emptyDir());
        expect(res.stdout, `${command} --help`).toContain("Examples:");
        expect(res.stdout, `${command} --help`).toContain(`$ ath ${command}`);
      }
    },
    20_000,
  );

  it("shows plain-text and structured log examples with the confirmation rule", () => {
    const res = ath(["log", "--help"], emptyDir());
    expect(res.stdout).toContain("$ ath log slept badly, about 5 hours");
    expect(res.stdout).toContain("structured record");
    expect(res.stdout).toContain("--json");
    expect(res.stdout).toContain("never confirms a write");
  });

  it("says when an option would be used, not only what it is", () => {
    // Help wraps to the terminal width, so compare on the words rather than the lines.
    const flat = (args: string[]) => ath(args, emptyDir()).stdout.replace(/\s+/g, " ");
    expect(flat(["series", "--help"])).toContain(
      "one source only, for when two measured the same thing",
    );
    expect(flat(["predict", "--help"])).toContain(
      "A prediction needs a model",
    );
  });
});

describe("bare ath — a short guide, not the full list", () => {
  it("tells someone with no file how to make one", () => {
    const res = ath([], emptyDir());
    expect(res.code).toBe(0);
    expect(res.stdout).toContain("There is no .ath.sqlite file in this folder");
    expect(res.stdout).toContain("ath init");
    expect(res.stdout).not.toContain("Set up:");
  });

  it("tells someone with a file what can be done to it", () => {
    const dir = dirWithFile();
    const res = ath([], dir);
    expect(res.code).toBe(0);
    expect(res.stdout).toContain("Working on athlete.ath.sqlite in this folder");
    expect(res.stdout).toContain("ath stats");
    expect(res.stdout).not.toContain("There is no");
  });

  it("differs with and without a file", () => {
    expect(ath([], emptyDir()).stdout).not.toBe(ath([], dirWithFile()).stdout);
  });
});

describe("errors name the remedy", () => {
  it("says what to do instead of overwriting a file", () => {
    const dir = dirWithFile();
    const res = ath(["init", "-y"], dir);
    expect(res.code).toBe(1);
    expect(res.stdout).toContain("ath stats");
    expect(res.stdout).toContain("--file");
  });

  it("points at ath stats when a series is asked for from the wrong source", () => {
    const dir = dirWithFile();
    const res = ath(["series", "heart_rate", "--source", "whoop-9"], dir);
    expect(res.code).toBe(1);
    expect(res.stdout).toContain("ath stats");
  });

  it("names the known quantities, and where to see the ones this file holds", () => {
    const dir = dirWithFile();
    const res = ath(["series", "heartrate"], dir);
    expect(res.code).toBe(1);
    expect(res.stdout).toContain("unknown quantity 'heartrate'");
    expect(res.stdout).toContain("ath stats");
  });

  it("says how to log something when nothing readable was given", () => {
    const dir = dirWithFile();
    const res = ath(["log", "--dry-run"], dir);
    expect(res.code).toBe(1);
    expect(res.stdout).toContain("nothing to log");
  });

  it("says why an invalid package cannot be imported", () => {
    const dir = dirWithFile();
    writeFileSync(join(dir, "nonsense.txt"), "this is not an export\n");
    const res = ath(["import", "nonsense.txt"], dir);
    expect(res.code).toBe(1);
    expect(res.stdout).toContain("is not a supported export");
    expect(res.stdout).toContain("nonsense.txt");
  });
});
