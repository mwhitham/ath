import { execFileSync } from "node:child_process";
import { lstatSync, mkdtempSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { createDatabase } from "../src/db/index.js";
import {
  defaultModelPath,
  readDefaultModel,
  resolveModel,
  writeDefaultModel,
} from "../src/models.js";
import { useCatalog } from "../src/gateway.js";

const here = dirname(fileURLToPath(import.meta.url));
const CLI = resolve(here, "../src/cli.ts");
const TSX = resolve(here, "../node_modules/.bin/tsx");
function ath(args: string[], cwd: string): { stdout: string; code: number } {
  try {
    const stdout = execFileSync(TSX, [CLI, ...args], {
      cwd,
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

afterEach(() => {
  useCatalog(undefined);
});

describe("default model path beside the live database", () => {
  it("maps athlete.ath.sqlite to athlete.ath.model", () => {
    expect(defaultModelPath("/data/athlete.ath.sqlite")).toBe("/data/athlete.ath.model");
  });

  it("writes the model file without changing the database bytes", async () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-models-"));
    const databasePath = join(directory, "athlete.ath.sqlite");
    const database = await createDatabase(databasePath, {
      createdAt: "2026-10-13T12:00:00.000Z",
    });
    database.close();
    const before = readFileSync(databasePath);

    writeDefaultModel(databasePath, "openai/gpt-oss-120b");

    expect(readFileSync(databasePath)).toEqual(before);
    expect(readFileSync(join(directory, "athlete.ath.model"), "utf8")).toBe(
      "openai/gpt-oss-120b\n",
    );
    writeDefaultModel(databasePath, "qwen/qwen3-235b-a22b");
    expect(readFileSync(databasePath)).toEqual(before);
    expect(readFileSync(join(directory, "athlete.ath.model"), "utf8")).toBe(
      "qwen/qwen3-235b-a22b\n",
    );
    expect(readDefaultModel(databasePath)).toBe("qwen/qwen3-235b-a22b");
    const check = ath(["check"], directory);
    expect(check.code).toBe(0);
    expect(check.stdout).toContain("valid Athletic Standard");
  });

  it("resolveModel reads the saved default", async () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-models-"));
    expect(ath(["init", "-y", "--no-skill"], directory).code).toBe(0);
    const databasePath = join(directory, "athlete.ath.sqlite");
    writeDefaultModel(databasePath, "openai/gpt-oss-120b");
    useCatalog(async () => [{ id: "openai/gpt-oss-120b" }, { id: "qwen/qwen3-235b-a22b" }]);

    const resolved = await resolveModel(databasePath, undefined, "vercel");
    expect(resolved.model).toBe("openai/gpt-oss-120b");
  });

  it("refuses a symlink at the model path and leaves the database unchanged", () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-models-"));
    const databasePath = join(directory, "athlete.ath.sqlite");
    writeFileSync(databasePath, "database-bytes");
    const before = readFileSync(databasePath);
    symlinkSync(databasePath, join(directory, "athlete.ath.model"));

    expect(() => writeDefaultModel(databasePath, "openai/gpt-oss-120b")).toThrow(
      /symbolic link/,
    );

    expect(readFileSync(databasePath)).toEqual(before);
    expect(lstatSync(join(directory, "athlete.ath.model")).isSymbolicLink()).toBe(true);
  });

  it("refuses to write when the path would hit the database or a portable package", () => {
    expect(() => writeDefaultModel(join(tmpdir(), "athlete.ath.json"), "x")).toThrow(
      /refusing to write the default model/,
    );
    expect(() => writeDefaultModel(join(tmpdir(), "no-suffix"), "x")).toThrow(
      /refusing to write/,
    );
  });
});
