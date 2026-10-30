import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import * as publicApi from "../src/index.js";
import * as publicDatabase from "../src/db/index.js";
import * as publicSchema from "../src/v1/schema.js";
import { ATH_PACKAGE_VERSION } from "../src/version.js";

describe("v1 package exports", () => {
  it("exports only the v1 schema and the domain database API", () => {
    expect(publicApi.ATHLETIC_STANDARD_VERSION).toBe("2.0.0");
    expect(publicApi.ATH_PACKAGE_VERSION).toBe(ATH_PACKAGE_VERSION);
    expect(publicApi.database.createDatabase).toBe(publicDatabase.createDatabase);
    expect(publicDatabase.getRecord).toBe(publicDatabase.readRecord);
    expect(publicDatabase.previewConflict).toBeTypeOf("function");
    expect(publicDatabase.resolveConflict).toBe(
      publicDatabase.resolvePackageConflict,
    );
    expect(publicApi).not.toHaveProperty("legacy");
    expect(publicSchema.AthleticStandardPackage).toBeDefined();
  });

  it("uses 2.0.0 for the package and the record", () => {
    const packageJson = JSON.parse(
      readFileSync(resolve("package.json"), "utf8"),
    ) as { version: string };
    expect(packageJson.version).toBe(ATH_PACKAGE_VERSION);
    expect(ATH_PACKAGE_VERSION).toBe("2.0.0");
    expect(publicApi.ATHLETIC_STANDARD_VERSION).toBe("2.0.0");
  });

  it("publishes stable subpaths for code, JSON schemas, and the Skill", () => {
    const packageJson = JSON.parse(
      readFileSync(resolve("package.json"), "utf8"),
    ) as {
      exports: Record<string, string | { types: string; import: string }>;
    };
    expect(packageJson.exports).toMatchObject({
      "./database": {
        types: "./dist/database.d.ts",
        import: "./dist/database.js",
      },
      "./schema": {
        types: "./dist/schema.d.ts",
        import: "./dist/schema.js",
      },
      "./json-schema": "./schema/2.0.0/athleticstandard.schema.json",
      "./json-schema/export-manifest":
        "./schema/2.0.0/export-manifest.schema.json",
      "./json-schema/measurement-series":
        "./schema/2.0.0/measurement-series.schema.json",
      "./skill": "./skills/athletic-standard/SKILL.md",
    });

    for (const target of Object.values(packageJson.exports)) {
      if (typeof target !== "string" || target.startsWith("./dist/")) {
        continue;
      }
      expect(existsSync(resolve(target))).toBe(true);
    }
  });
});
