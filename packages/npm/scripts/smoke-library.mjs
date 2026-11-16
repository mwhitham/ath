/**
 * The library half of the packed-tarball smoke test. smoke.mjs copies this
 * file into a project that installed the tarball and runs it there.
 *
 * Each step prints one JSON line: {"step", "ok", "detail"} or {"step", "ok",
 * "error"}. Every step runs even after an earlier one fails, so one run
 * reports everything that works. The exit status is 1 when any step failed.
 *
 * The calls use the 2.1.0 public API, which 2.1.1 keeps.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

let failed = false;

async function step(name, run) {
  try {
    const detail = await run();
    console.log(JSON.stringify({ step: name, ok: true, detail: detail ?? null }));
  } catch (error) {
    failed = true;
    const text = error instanceof Error ? (error.stack ?? error.message) : String(error);
    console.log(JSON.stringify({ step: name, ok: false, error: text }));
  }
}

function check(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

// A synthetic athlete note, in the shape the 2.1.0 tests write.
const report = {
  report_type: "note",
  reported_at: "2026-10-13T12:00:00.000Z",
  input_reported_at: {
    timestamp_text: "2026-10-13T12:00:00.000Z",
    utc_offset_text: "Z",
    utc_offset_meaning: "documented_recording_offset",
    timestamp_basis: "source_instant",
  },
  reported_via: "text",
  note: "Synthetic smoke-test note",
};

let root;
let database;
let schema;

await step("import athleticstandard", async () => {
  root = await import("athleticstandard");
  return `${Object.keys(root).length} exports`;
});
await step("import athleticstandard/database", async () => {
  database = await import("athleticstandard/database");
  for (const name of ["createDatabase", "writeRecord", "readRecord", "checkDatabase"]) {
    check(typeof database[name] === "function", `${name} is not a function`);
  }
  return `${Object.keys(database).length} exports`;
});
await step("import athleticstandard/schema", async () => {
  schema = await import("athleticstandard/schema");
  check(typeof schema.AthleteReportData?.safeParse === "function", "AthleteReportData is not a Zod schema");
  return `${Object.keys(schema).length} exports`;
});
for (const subpath of [
  "athleticstandard/json-schema",
  "athleticstandard/json-schema/export-manifest",
  "athleticstandard/json-schema/measurement-series",
]) {
  await step(`import ${subpath}`, async () => {
    const document = (await import(subpath, { with: { type: "json" } })).default;
    check(typeof document?.$schema === "string", "not a JSON Schema document");
    return document.$id ?? document.title ?? "loaded";
  });
}
await step("resolve athleticstandard/skill", () => {
  const path = fileURLToPath(import.meta.resolve("athleticstandard/skill"));
  const text = readFileSync(path, "utf8");
  check(text.startsWith("---") && /\nname:/.test(text), "SKILL.md has no front matter name");
  return path.split(/[\\/]/).slice(-3).join("/");
});
await step("parse a record with the Zod schema", () => {
  const parsed = schema.AthleteReportData.parse(report);
  check(parsed.note === report.note, "the parsed note changed");
  const refused = schema.AthleteReportData.safeParse({ ...report, rating: 3 });
  check(!refused.success, "a rating without its scale was accepted");
  check(schema.AthleteReportData.array().parse([report]).length === 1, "an array of reports did not parse");
  return "accepted one report and refused one";
});

let handle;
let recordId;
const path = join(process.cwd(), "smoke.ath.sqlite");
await step("createDatabase", async () => {
  handle = await database.createDatabase(path);
  return path.split(/[\\/]/).pop();
});
await step("writeRecord", async () => {
  const written = await database.writeRecord(handle, {
    record_type: "athlete_report",
    changed_by: { author_type: "athlete" },
    data: report,
  });
  recordId = written.record.record_id;
  check(typeof recordId === "string", "no record_id came back");
  return `${recordId} at database revision ${written.database_revision}`;
});
await step("readRecord", async () => {
  const record = await database.readRecord(handle, recordId);
  check(record?.record_type === "athlete_report", "the record was not read back");
  check(record.data.note === report.note, "the note changed");
  return `revision ${record.current_revision}`;
});
await step("checkDatabase", async () => {
  const result = await database.checkDatabase(handle);
  check(result.valid === true, `the check found problems: ${JSON.stringify(result.issues)}`);
  check(result.record_count >= 1, "the check counted no records");
  return `${result.record_count} records, valid`;
});
await step("close", () => {
  handle.close();
  return "closed";
});

process.exitCode = failed ? 1 : 0;
