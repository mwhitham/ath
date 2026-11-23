/**
 * The Zod compatibility facade and the native Rust validator must give the
 * same verdict on every recorded validation vector.
 *
 * The vectors in tests/vectors/model/ were observed from the 2.1.0
 * Zod schemas. Each case runs through the facade in process and through the
 * native validator in one protocol worker. The cases skipped here are the
 * deliberate differences in build-history/v2.1.1/appendix/deliberate-changes.md, items 2
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

const vectorDirectory = join(repositoryRoot, "tests", "vectors", "model");

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

/**
 * The 2.1.3 coach author (D255) has no recorded vector, because 2.1.0 did not
 * have it. These cases hold the facade and the native validator to the same
 * verdict on its shape, its record types, deletion, and the package version.
 */
describe("the 2.1.3 coach author", () => {
  const coach = { author_type: "coach", coach: "Example Coach", coach_id: "6f1c2e4a-3b5d-4c7e-9f80-1a2b3c4d5e6f" };
  const at = "2026-11-20T12:00:00.000Z";
  const noteId = "01890f3e-e7d3-7cc0-88c8-0000000000c1";
  const authorship = { author: coach, authored_at: at, evidence: [], content_version: 1, status: "active" };
  const note = { authorship, note: "Keep every session easy this week." };
  const noteRecord = {
    record_id: noteId,
    created_at: at,
    current_revision: 1,
    is_deleted: false,
    record_type: "coaching_note",
    data: note,
  };
  const noteRevision = {
    record_id: noteId,
    revision: 1,
    previous_revision: null,
    became_known_at: at,
    became_known_at_basis: "local_write",
    changed_by: coach,
    record_type: "coaching_note",
    data: note,
  };
  const report = {
    report_type: "note",
    reported_at: at,
    input_reported_at: { timestamp_missing_reason: "not_applicable", utc_offset_missing_reason: "not_applicable" },
    reported_via: "text",
    note: "Legs felt heavy.",
  };
  const packageAt = (version: string) => ({
    athleticstandard_version: version,
    database_id: "01890f3e-e7d3-7cc0-88c8-000000000001",
    database_revision: 1,
    records: [noteRecord],
    record_revisions: [noteRevision],
    record_deletions: [],
    record_erasure_markers: [],
    record_import_keys: [],
    measurement_series_files: [],
  });
  const cases: Array<{ schema: string; input: unknown; valid: boolean }> = [
    { schema: "RecordActor", input: coach, valid: true },
    { schema: "AuthoredRecordAuthor", input: coach, valid: true },
    { schema: "RecordActor", input: { ...coach, coach_id: coach.coach_id.toUpperCase() }, valid: false },
    { schema: "RecordActor", input: { ...coach, coach_id: "coach@example.com" }, valid: false },
    { schema: "RecordActor", input: { ...coach, coach: "" }, valid: false },
    { schema: "CoachingNoteRecord", input: noteRecord, valid: true },
    { schema: "CoachingNoteRevision", input: noteRevision, valid: true },
    {
      schema: "GoalRecord",
      input: { ...noteRecord, record_type: "goal", data: { authorship, title: "Row 2000 m under eight minutes" } },
      valid: false,
    },
    {
      schema: "AthleteReportRevision",
      input: { ...noteRevision, record_type: "athlete_report", data: report },
      valid: false,
    },
    {
      schema: "AthleteReportRevision",
      input: { ...noteRevision, record_type: "athlete_report", data: report, changed_by: { author_type: "athlete" } },
      valid: true,
    },
    {
      schema: "RecordDeletion",
      input: { record_id: noteId, record_revision: 1, deleted_at: at, deleted_by: coach },
      valid: false,
    },
    { schema: "AthleticStandardPackage", input: packageAt("2.1.3"), valid: true },
    { schema: "AthleticStandardPackage", input: packageAt("2.1.2"), valid: false },
  ];

  it("gets the same verdict from the facade and the native validator", async () => {
    const facade = schemas as unknown as Record<string, ZodType>;
    const native = await callAsync("model.validate_many", {
      cases: cases.map((entry) => ({ schema: entry.schema, json_text: JSON.stringify(entry.input) })),
    });
    cases.forEach((entry, index) => {
      const label = `${entry.schema} ${JSON.stringify(entry.input).slice(0, 80)}`;
      expect(facade[entry.schema]!.safeParse(entry.input).success, `facade: ${label}`).toBe(entry.valid);
      expect(native.items[index]!.valid, `native: ${label}`).toBe(entry.valid);
    });
  });
});
