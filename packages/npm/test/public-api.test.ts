/**
 * Every 2.1.0 public export is present with the same name and kind, and the
 * package keeps every 2.1.0 subpath. Types and function signatures are
 * checked at compile time by test/public-api.generated.ts.
 */
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { z } from "zod";
import * as database from "../src/database.js";
import * as root from "../src/index.js";
import * as schema from "../src/schema.js";
// @ts-expect-error The generator is plain JavaScript without declarations.
import { generate } from "../scripts/generate-api-check.mjs";
import { repositoryRoot } from "./support.js";

interface InventorySymbol {
  entry_point: string;
  name: string;
  kinds: string[];
  value_export: boolean;
}

const inventory = JSON.parse(
  readFileSync(join(repositoryRoot, "native", "reference", "public-api.json"), "utf8"),
) as { symbols: InventorySymbol[]; json_only_subpaths: Array<{ subpath: string }> };

const modules: Record<string, Record<string, unknown>> = {
  ".": root as Record<string, unknown>,
  "./database": database as Record<string, unknown>,
  "./schema": schema as Record<string, unknown>,
};

const packageRoot = join(repositoryRoot, "packages", "npm");
const packageJson = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8")) as {
  name: string;
  version: string;
  bin: Record<string, string>;
  exports: Record<string, unknown>;
};

describe("public exports", () => {
  for (const [entry, module] of Object.entries(modules)) {
    it(`keeps every 2.1.0 value export of ${entry} and adds none`, () => {
      const expected = inventory.symbols
        .filter((symbol) => symbol.entry_point === entry && symbol.value_export)
        .map((symbol) => symbol.name)
        .sort();
      expect(Object.keys(module).sort()).toEqual(expected);
      for (const symbol of inventory.symbols.filter((item) => item.entry_point === entry && item.value_export)) {
        const value = module[symbol.name];
        if (symbol.kinds.includes("function") || symbol.kinds.includes("class")) {
          expect(typeof value, symbol.name).toBe("function");
        } else if (symbol.kinds.includes("const Zod schema")) {
          expect(value, symbol.name).toBeInstanceOf(z.ZodType);
        } else if (symbol.kinds.includes("namespace")) {
          expect(typeof value, symbol.name).toBe("object");
        } else {
          expect(value, symbol.name).not.toBeUndefined();
        }
      }
    });
  }

  it("keeps the 2.1.0 aliases and the database namespace", () => {
    expect(database.getRecord).toBe(database.readRecord);
    expect(database.resolveConflict).toBe(database.resolvePackageConflict);
    expect(root.database).toEqual(expect.objectContaining({ openDatabase: database.openDatabase }));
    expect(root.ATH_PACKAGE_VERSION).toBe("2.1.1");
    expect(root.ATHLETIC_STANDARD_VERSION).toBe("2.1.1");
    expect(schema.ACCEPTED_RECORD_VERSIONS).toEqual(["1.0.0", "2.0.0", "2.1.0", "2.1.1"]);
    expect(schema.IMPORTABLE_PACKAGE_VERSIONS).toEqual(["2.0.0", "2.1.0", "2.1.1"]);
  });

  it("keeps the generated compile-time check current", () => {
    const committed = readFileSync(join(packageRoot, "test", "public-api.generated.ts"), "utf8");
    expect(committed).toBe(generate(inventory));
  });

  it("repeats the native constants", () => {
    expect(database.SQL_SCHEMA_VERSION).toBe(5);
    expect(database.SQLITE_APPLICATION_ID).toBe(0x41544831);
    expect(database.SQLITE_APPLICATION_MARKER).toBe("ATH1");
    expect(database.MAX_ATTACHMENT_BYTES).toBe(25 * 1024 * 1024);
    expect(database.CALCULATION_OPERATIONS).toEqual([
      "summary",
      "baseline",
      "trend",
      "coverage",
      "compare-sources",
      "correlation",
      "workout-drift",
      "provider-period-summary",
    ]);
    expect(database.CALCULATION_RECORD_KINDS).toEqual([
      "measurement",
      "provider-score",
      "provider-summary",
      "session-summary",
    ]);
  });
});

describe("package layout", () => {
  it("keeps the name, the ath command, and every 2.1.0 subpath", () => {
    expect(packageJson.name).toBe("athleticstandard");
    expect(packageJson.version).toBe("2.1.1");
    expect(Object.keys(packageJson.bin)).toEqual(["ath"]);
    expect(Object.keys(packageJson.exports).sort()).toEqual(
      [".", "./database", "./schema", ...inventory.json_only_subpaths.map((entry) => entry.subpath)].sort(),
    );
  });

  it("generates the JSON Schema files for 2.1.1", () => {
    for (const subpath of ["./json-schema", "./json-schema/export-manifest", "./json-schema/measurement-series"]) {
      const target = packageJson.exports[subpath] as string;
      expect(target).toMatch(/^\.\/schema\/2\.1\.1\//);
      const document = JSON.parse(readFileSync(join(packageRoot, target), "utf8")) as { $id: string; $schema: string };
      expect(document.$schema).toBe("https://json-schema.org/draft/2020-12/schema");
      expect(document.$id).toContain("/2.1.1/");
    }
  });

  it("points the skill export at the shipped skill", () => {
    expect(packageJson.exports["./skill"]).toBe("./skills/athletic-standard/SKILL.md");
    expect(existsSync(join(repositoryRoot, "skills", "athletic-standard", "SKILL.md"))).toBe(true);
  });
});
