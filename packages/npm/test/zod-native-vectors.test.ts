/**
 * The Zod compatibility facade and the native Rust validator must give the
 * same verdict on every recorded validation vector.
 *
 * The vectors in native/tests/vectors/model/ were observed from the 2.1.0
 * Zod schemas. Each case runs through the facade in process and through the
 * native validator in one protocol worker. The cases skipped here are the
 * deliberate differences in native/reference/deliberate-changes.md, items 2
 * to 4: lone UTF-16 surrogates, numbers outside the double range, and a
 * `__proto__` key. Version 2.1.1 (item 1) is accepted by both, so those cases
 * still compare.
 */
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import type { ZodType } from "zod";
import * as extensions from "../src/extensions.js";
import * as schemas from "../src/schema.js";
import { callAsync } from "../src/client/worker.js";
import { repositoryRoot } from "./support.js";

interface VectorCase {
  name: string;
  input?: unknown;
  input_text?: string;
  valid: boolean;
}

interface VectorFile {
  schema: string;
  cases: VectorCase[];
}

const vectorDirectory = join(repositoryRoot, "native", "tests", "vectors", "model");

function deliberateDifference(text: string): string | null {
  if (/\\u[dD][89abAB]|\\u[dD][c-fC-F]/.test(text)) {
    return "lone UTF-16 surrogates are refused when JSON is read";
  }
  if (text.includes("__proto__")) {
    return "a __proto__ key is kept and checked";
  }
  if (text.includes("1e400")) {
    return "a number outside the double range is refused when JSON is read";
  }
  return null;
}

function versionCase(schema: string, name: string): boolean {
  return schema === "RecordFormatVersion" || name.includes("2.1.1") || name.startsWith("athleticstandard_version:");
}

const files: VectorFile[] = readdirSync(vectorDirectory)
  .filter((file) => file.endsWith(".json"))
  .sort()
  .map((file) => JSON.parse(readFileSync(join(vectorDirectory, file), "utf8")) as VectorFile);

describe("Zod facade and native validator", () => {
  it("covers the full recorded vector set", () => {
    expect(files.length).toBeGreaterThanOrEqual(140);
  });

  it("gives the same verdict on every vector apart from the listed differences", async () => {
    const compared: Array<{ schema: string; name: string; text: string; facadeValid: boolean; recorded: boolean }> = [];
    const skipped = new Map<string, number>();
    for (const file of files) {
      // Extension shapes live in the extension module the schema module uses.
      const schema = ({ ...extensions, ...schemas } as unknown as Record<string, ZodType | undefined>)[file.schema];
      expect(schema, `facade export ${file.schema}`).toBeDefined();
      for (const vector of file.cases) {
        const text = vector.input_text ?? JSON.stringify(vector.input);
        const reason = deliberateDifference(text);
        if (reason !== null) {
          skipped.set(reason, (skipped.get(reason) ?? 0) + 1);
          continue;
        }
        const input = vector.input_text === undefined ? vector.input : (JSON.parse(vector.input_text) as unknown);
        compared.push({
          schema: file.schema,
          name: vector.name,
          text,
          facadeValid: schema!.safeParse(input).success,
          recorded: vector.valid,
        });
      }
    }
    const native = await callAsync("model.validate_many", {
      cases: compared.map((entry) => ({ schema: entry.schema, json_text: entry.text })),
    });
    expect(native.items).toHaveLength(compared.length);

    const disagreements: string[] = [];
    const facadeChanges: string[] = [];
    compared.forEach((entry, index) => {
      const nativeValid = native.items[index]!.valid;
      if (nativeValid !== entry.facadeValid) {
        disagreements.push(
          `${entry.schema} / ${entry.name}: facade ${entry.facadeValid}, native ${nativeValid}${
            native.items[index]!.error === undefined ? "" : ` (${native.items[index]!.error})`
          }`,
        );
      }
      if (entry.facadeValid !== entry.recorded && !versionCase(entry.schema, entry.name)) {
        facadeChanges.push(`${entry.schema} / ${entry.name}: recorded ${entry.recorded}, facade ${entry.facadeValid}`);
      }
    });
    expect(disagreements, disagreements.slice(0, 20).join("\n")).toEqual([]);
    // The facade is the 2.1.0 schema with new version declarations, so the
    // recorded 2.1.0 verdicts still hold outside the version cases.
    expect(facadeChanges, facadeChanges.slice(0, 20).join("\n")).toEqual([]);
    expect(compared.length).toBeGreaterThan(3_000);
    expect([...skipped.keys()].sort()).toEqual([
      "a __proto__ key is kept and checked",
      "a number outside the double range is refused when JSON is read",
      "lone UTF-16 surrogates are refused when JSON is read",
    ]);
  });
});
