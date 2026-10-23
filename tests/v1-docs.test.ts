import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { RecordType } from "../src/v1/schema.js";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const read = (file: string) => readFileSync(join(root, file), "utf8");

describe("v1 docs match the implemented format", () => {
  const count = RecordType.options.length;
  const files = ["SPEC.md", "README.md", "skill/references/format.md"] as const;

  it("states the same record-type count as the schema", () => {
    expect(count).toBe(24);
    for (const file of files) {
      const text = read(file);
      expect(text, file).not.toMatch(/\b23 record types\b/);
      expect(text, file).not.toMatch(/\bone of 23\b/);
      expect(text, file).toContain(String(count));
    }
  });

  it("documents chunked series file names", () => {
    for (const file of files) {
      const text = read(file);
      expect(text, file).toContain(
        "<source-id>-<quantity>-000001.ath-series.json",
      );
      expect(text, file).not.toMatch(
        /<source-id>-<quantity>\.ath-series\.json/,
      );
    }
  });
});
