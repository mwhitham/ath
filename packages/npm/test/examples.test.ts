/**
 * The examples compile with the package type check and run against the built
 * package through its own name, as a caller's code would.
 */
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { join } from "node:path";
import { beforeAll, describe, expect, it } from "vitest";
import { repositoryRoot } from "./support.js";

const packageRoot = join(repositoryRoot, "packages", "npm");
const require = createRequire(join(packageRoot, "package.json"));

function run(command: string, args: string[]): string {
  const result = spawnSync(command, args, { cwd: packageRoot, encoding: "utf8", env: process.env });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed (${String(result.status)}):\n${result.stderr}${result.stdout}`);
  }
  return result.stdout;
}

beforeAll(() => {
  // The examples import "athleticstandard", which resolves to this package's dist/.
  run(process.execPath, [require.resolve("typescript/bin/tsc"), "-p", "tsconfig.build.json"]);
}, 300_000);

function runExample(name: string): Record<string, unknown> {
  const tsx = require.resolve("tsx");
  const output = run(process.execPath, ["--import", tsx, join("examples", name)]);
  return JSON.parse(output.trim().split("\n").at(-1)!) as Record<string, unknown>;
}

describe("examples", () => {
  it("runs the README imports against a real database", () => {
    expect(runExample("readme.ts")).toEqual({
      measurements: 1,
      package_version: "2.1.2",
      package_records: 2,
    });
  });

  it("composes, infers, and parses with the published Zod schemas", () => {
    expect(runExample("zod-composition.ts")).toEqual({
      note: "RMSSD is steady this week.",
      reviewed: true,
      record_id_only: true,
      picked_is_strict: true,
      describe: "hrv_rmssd 42ms",
      quantity_custom: "custom",
      sourceless_refused: true,
      sourceless_path: "records.0.data.source_id",
      uppercase_id_refused: true,
      thrown: "invalid_format",
    });
  });
});
