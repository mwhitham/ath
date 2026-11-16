/**
 * Freeze the 2.1.0 Apple Health import results that the native import must reproduce.
 *
 * This is the Apple sibling of `import-golden.ts` and writes in the same shape:
 * every record's complete JSON for every revision, revision metadata, import
 * keys, the `--json` preview, and the result of the confirmed write. Record IDs
 * become labels built from match keys, and the tool version in `changed_by`
 * becomes `<ath version>`. Nothing else is normalized.
 *
 * One deliberate change is applied while recording (listed in build-history/v2.1.1/appendix/deliberate-changes.md):
 * 2.1.0 read a GPX time without a UTC offset in the computer's own timezone. The
 * native reader leaves that point out. `Date.parse` is wrapped here so any text
 * whose instant depends on the computer's timezone reads as invalid, which is
 * the native rule. The process runs in a timezone away from UTC, so a missed
 * case shows up as a difference instead of hiding behind UTC.
 *
 * It also records numeric vectors: V8's haversine distances, route summaries
 * of invented noisy tracks, and R-peak detection and RMSSD on invented ECG
 * waveforms with noise, drift, T waves, and motion artifacts.
 *
 *   ATH_REFERENCE_ROOT=<checkout> node --import <checkout>/node_modules/tsx/dist/esm/index.mjs tools/reference/import-golden-apple.ts
 *
 * Output: tests/golden/import/watch-*.json, route-numerics.json, and
 * beat-numerics.json. File names avoid provider words.
 */
import { once } from "node:events";
import {
  createWriteStream,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

process.env.TZ = "America/Denver";

import { reference, repository as root } from "./reference.ts";

const outDirectory = join(root, "tests", "golden", "import");

// ---- The deliberate change: no reading in the computer's timezone ----------

const originalParse = Date.parse;
const zoneChecked = new Map<string, number>();
Date.parse = ((text: string): number => {
  const here = originalParse(text);
  if (!Number.isFinite(here)) return here;
  const known = zoneChecked.get(text);
  if (known !== undefined) return known;
  const saved = process.env.TZ;
  process.env.TZ = "Pacific/Kiritimati";
  const east = originalParse(text);
  process.env.TZ = "Pacific/Pago_Pago";
  const west = originalParse(text);
  process.env.TZ = saved;
  const result = east === here && west === here ? here : Number.NaN;
  zoneChecked.set(text, result);
  return result;
}) as typeof Date.parse;

// ---- The 2.1.0 library ------------------------------------------------------

type Database = { close(): void };
interface Db {
  createDatabase(path: string, options: { createdAt: string }): Promise<Database>;
  previewProviderImport(database: Database, path: string, options: { received_at: string }): Promise<Preview>;
  importProviderRecords(database: Database, preview: Preview): Promise<unknown>;
  deleteRecord(database: Database, input: unknown): Promise<unknown>;
  releaseLocalDatabase(database: Database): Promise<void>;
}
interface Draft {
  record_id: string;
  record_type: string;
  match_key_method: string;
  match_key: string;
}
interface Preview {
  provider: string;
  label: string;
  database_revision: number;
  new_records: Array<{ record_id: string; record_type: string }>;
  matching_records: Array<{ record_id: string; record_type: string }>;
  changed_records: Array<{ record_id: string; record_type: string; current_revision: number }>;
  conflicts: Array<{ match_key: string; record_id?: string; message: string }>;
  skipped: Record<string, { count: number; example?: string }>;
  changes: Array<{ draft: Draft & { data: unknown } }>;
}

const load = async (path: string) => import(pathToFileURL(join(reference, path)).href);
const db = (await load("src/db/index.ts")) as Db;
const { databaseClient } = (await load("src/db/client.ts")) as {
  databaseClient(database: Database): {
    execute(query: string | { sql: string; args: unknown[] }): Promise<{ rows: Array<Record<string, unknown>> }>;
  };
};
const routes = (await load("src/import/routes.ts")) as {
  haversineM(lat1: number, lon1: number, lat2: number, lon2: number): number;
  parseGpx(text: string): unknown[];
  summarizeRoute(points: unknown[]): unknown;
};
const rpeaks = (await load("src/rpeaks.ts")) as {
  detectRPeaks(microvolts: number[], rate: number): { peaks: number[]; quality: number };
};
const hrv = (await load("src/hrv.ts")) as { rmssdFromIntervals(intervals: number[]): unknown };
const ecg = (await load("src/import/ecg.ts")) as {
  parseEcgCsv(text: string): { microvolts: number[]; sampleRateHz: number } | { kind: string; detail: string };
  beatsFromEcg(recording: unknown): { intervalsMs: number[]; offsetsMs: number[]; quality: number } | { kind: string };
};
const { ZipFile } = createRequire(join(reference, "package.json"))("yazl") as {
  ZipFile: new () => {
    addBuffer(bytes: Buffer, name: string, options?: { mtime?: Date; mode?: number }): void;
    outputStream: NodeJS.ReadableStream;
    end(): void;
  };
};

const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g;
const realTmp = realpathSync(tmpdir());
const CREATED_AT = "2026-10-13T11:00:00.000Z";

function compareText(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

function canonical(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonical);
  if (typeof value !== "object" || value === null) return value;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .sort(([left], [right]) => compareText(left, right))
      .map(([key, child]) => [key, canonical(child)]),
  );
}

function write(name: string, value: unknown): void {
  writeFileSync(join(outDirectory, name), `${JSON.stringify(canonical(value), null, 2)}\n`);
}

class Labels {
  private labels = new Map<string, string>();

  learn(entries: Array<{ record_id: string; record_type: string; method: string; key: string }>): void {
    const pending = entries.filter((entry) => !this.labels.has(entry.record_id));
    for (let pass = 0; pass < 3 && pending.length > 0; pass += 1) {
      for (let index = pending.length - 1; index >= 0; index -= 1) {
        const entry = pending[index]!;
        const ids = entry.key.match(UUID) ?? [];
        if (ids.some((id) => !this.labels.has(id))) continue;
        this.labels.set(entry.record_id, `${entry.record_type} ${entry.method} ${this.text(entry.key)}`);
        pending.splice(index, 1);
      }
    }
  }

  text(value: string): string {
    return value.replace(UUID, (id) => (this.labels.has(id) ? `<${this.labels.get(id)}>` : "<unlabeled>"));
  }

  value(value: unknown): unknown {
    if (typeof value === "string") return this.text(value);
    if (Array.isArray(value)) return value.map((child) => this.value(child));
    if (typeof value !== "object" || value === null) return value;
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>).map(([key, child]) => [key, this.value(child)]),
    );
  }
}

async function snapshot(database: Database, labels: Labels): Promise<unknown> {
  const client = databaseClient(database);
  const keyRows = (
    await client.execute(
      `SELECT record_import_keys.record_id, records.record_type, match_key_method, match_key, imported_at
         FROM record_import_keys JOIN records USING (record_id)`,
    )
  ).rows;
  labels.learn(
    keyRows.map((row) => ({
      record_id: String(row.record_id),
      record_type: String(row.record_type),
      method: String(row.match_key_method),
      key: String(row.match_key),
    })),
  );
  const revision = Number((await client.execute("SELECT current_revision FROM database_state")).rows[0]?.current_revision);
  const recordRows = (
    await client.execute("SELECT record_id, record_type, current_revision, created_at, is_deleted FROM records")
  ).rows;
  const revisions = (
    await client.execute(
      `SELECT record_id, revision, previous_revision, became_known_at, became_known_at_basis, changed_by_json,
              record_json, extensions_json
         FROM record_revisions ORDER BY record_id, revision`,
    )
  ).rows;
  const deletions = (await client.execute("SELECT record_id, record_revision, deleted_at FROM record_deletions")).rows;
  const records = recordRows
    .map((row) => {
      const id = String(row.record_id);
      return {
        label: labels.text(id),
        record_type: String(row.record_type),
        created_at: String(row.created_at),
        current_revision: Number(row.current_revision),
        is_deleted: Number(row.is_deleted) === 1,
        revisions: revisions
          .filter((revisionRow) => String(revisionRow.record_id) === id)
          .map((revisionRow) => {
            const changedBy = JSON.parse(String(revisionRow.changed_by_json)) as Record<string, unknown>;
            if (changedBy.author_type === "tool") changedBy.tool_version = "<ath version>";
            return {
              revision: Number(revisionRow.revision),
              previous_revision: revisionRow.previous_revision === null ? null : Number(revisionRow.previous_revision),
              became_known_at: String(revisionRow.became_known_at),
              became_known_at_basis: String(revisionRow.became_known_at_basis),
              changed_by: changedBy,
              record_json: labels.text(String(revisionRow.record_json)),
              extensions_json: revisionRow.extensions_json === null ? null : String(revisionRow.extensions_json),
            };
          }),
        import_keys: keyRows
          .filter((keyRow) => String(keyRow.record_id) === id)
          .map((keyRow) => ({
            method: String(keyRow.match_key_method),
            key: labels.text(String(keyRow.match_key)),
            imported_at: String(keyRow.imported_at),
          })),
        deletions: deletions
          .filter((deletion) => String(deletion.record_id) === id)
          .map((deletion) => ({ record_revision: Number(deletion.record_revision), deleted_at: String(deletion.deleted_at) })),
      };
    })
    .sort((left, right) => compareText(left.label, right.label));
  return { database_revision: revision, records };
}

function previewOutput(preview: Preview, labels: Labels): unknown {
  labels.learn(
    preview.changes.map(({ draft }) => ({
      record_id: draft.record_id,
      record_type: draft.record_type,
      method: draft.match_key_method,
      key: draft.match_key,
    })),
  );
  const routePoints = preview.changes.reduce((sum, change) => {
    const route = (change.draft.data as { route?: unknown }).route;
    return sum + (Array.isArray(route) ? route.length : 0);
  }, 0);
  return labels.value({
    provider: preview.provider,
    imported: preview.label,
    destination_database_revision: preview.database_revision,
    new_records: preview.new_records,
    matching_records: preview.matching_records,
    changed_records: preview.changed_records,
    conflicts: preview.conflicts,
    skipped: preview.skipped,
    route_points_stored: routePoints,
    write_confirmed: false,
  });
}

// ---- Inputs -------------------------------------------------------------------

/** A file is text, or `{ fixture }`: a path in the repository whose bytes are copied. */
type FileBody = string | { fixture: string };
type Input =
  | { kind: "directory"; files: Record<string, FileBody>; open?: string }
  | { kind: "zip"; files: Record<string, FileBody> };

function bytes(body: FileBody): Buffer {
  return typeof body === "string" ? Buffer.from(body) : readFileSync(join(reference, body.fixture));
}

async function materialize(input: Input, directory: string): Promise<string> {
  if (input.kind === "directory") {
    const path = join(directory, "input");
    mkdirSync(path, { recursive: true });
    for (const [name, body] of Object.entries(input.files)) {
      mkdirSync(dirname(join(path, name)), { recursive: true });
      writeFileSync(join(path, name), bytes(body));
    }
    return input.open === undefined ? path : join(path, input.open);
  }
  // Entries are written in the sorted order the golden file lists them, so the
  // native test builds the same archive order.
  const path = join(directory, "input.zip");
  const zip = new ZipFile();
  const entries = Object.entries(input.files).sort(([left], [right]) => compareText(left, right));
  for (const [name, body] of entries) zip.addBuffer(bytes(body), name, { mtime: new Date(0) });
  const output = createWriteStream(path);
  zip.outputStream.pipe(output);
  zip.end();
  await once(output, "close");
  return path;
}

/** Every file of an approved fixture folder, as fixture references. */
function fixtureFiles(folder: string, prefix = ""): Record<string, FileBody> {
  const base = join(reference, "tests", "fixtures", "exports", folder);
  const walk = (directory: string): string[] =>
    readdirSync(directory)
      .sort()
      .flatMap((name) => {
        const path = join(directory, name);
        return statSync(path).isDirectory() ? walk(path) : [path];
      });
  return Object.fromEntries(
    walk(base).map((path) => [`${prefix}${relative(base, path)}`, { fixture: relative(reference, path) }]),
  );
}

interface Step {
  received_at: string;
  input: Input;
  delete_before?: { label_prefix: string; deleted_at: string };
}

async function scenario(name: string, description: string, steps: Step[]): Promise<void> {
  const directory = mkdtempSync(join(realTmp, "ath-golden-"));
  const database = await db.createDatabase(join(directory, "athlete.ath.sqlite"), { createdAt: CREATED_AT });
  const labels = new Labels();
  const out: unknown[] = [];
  try {
    for (const [index, step] of steps.entries()) {
      const stepDirectory = join(directory, `step-${index}`);
      mkdirSync(stepDirectory);
      if (step.delete_before !== undefined) {
        const before = (await snapshot(database, labels)) as {
          records: Array<{ label: string; current_revision: number }>;
        };
        const target = before.records.find((record) => record.label.startsWith(step.delete_before!.label_prefix));
        if (target === undefined) throw new Error(`no record to delete: ${step.delete_before.label_prefix}`);
        const keyRows = (await databaseClient(database).execute("SELECT record_id FROM records")).rows;
        const id = keyRows.map((row) => String(row.record_id)).find((recordId) => labels.text(recordId) === target.label);
        await db.deleteRecord(database, {
          record_id: id,
          expected_revision: target.current_revision,
          deleted_by: { author_type: "athlete" },
          deleted_at: step.delete_before.deleted_at,
        });
      }
      const path = await materialize(step.input, stepDirectory);
      let preview: Preview;
      try {
        preview = await db.previewProviderImport(database, path, { received_at: step.received_at });
      } catch (error) {
        out.push({
          received_at: step.received_at,
          input: step.input,
          delete_before: step.delete_before ?? null,
          preview_error: (error as Error).message.split(stepDirectory).join("<dir>"),
        });
        continue;
      }
      const output = previewOutput(preview, labels);
      let apply: unknown;
      try {
        apply = { result: await db.importProviderRecords(database, preview) };
      } catch (error) {
        apply = { error: (error as Error).message };
      }
      out.push({
        received_at: step.received_at,
        input: step.input,
        delete_before: step.delete_before ?? null,
        preview: output,
        apply,
        database: await snapshot(database, labels),
      });
    }
  } finally {
    await db.releaseLocalDatabase(database);
    rmSync(directory, { recursive: true, force: true });
  }
  write(`${name}.json`, { reference: "2.1.0", description, steps: out });
}

const R1 = "2026-10-13T12:00:00.000Z";
const R2 = "2026-10-14T12:00:00.000Z";
const R3 = "2026-10-15T12:00:00.000Z";
const R4 = "2026-10-16T12:00:00.000Z";

mkdirSync(outDirectory, { recursive: true });

// ---- Approved fixtures --------------------------------------------------------

const APPLE_XML = "tests/fixtures/exports/apple/export.xml";
const appleXml = readFileSync(join(reference, APPLE_XML), "utf8");
const apple = fixtureFiles("apple");
const corrected = {
  ...apple,
  "export.xml": appleXml.replace('unit="lb" startDate="2026-08-09 07:02:00 -0700" endDate="2026-08-09 07:02:00 -0700" value="181.4"', 'unit="lb" startDate="2026-08-09 07:02:00 -0700" endDate="2026-08-09 07:02:00 -0700" value="182.4"').replace('startDate="2026-08-09 08:05:00 -0700" endDate="2026-08-09 08:05:00 -0700" value="64"', 'startDate="2026-08-09 08:05:00 -0700" endDate="2026-08-09 08:05:00 -0700" value="65"'),
};
if (corrected["export.xml"] === appleXml) throw new Error("the correction did not apply");

await scenario("watch-folder", "The committed fixture as a folder: a fresh import, an unchanged repeat, then two corrected values.", [
  { received_at: R1, input: { kind: "directory", files: apple } },
  { received_at: R2, input: { kind: "directory", files: apple } },
  { received_at: R3, input: { kind: "directory", files: corrected } },
]);

await scenario("watch-containers", "The fixture as a zip under apple_health_export/, then as a folder, then as a bare export.xml.", [
  { received_at: R1, input: { kind: "zip", files: fixtureFiles("apple", "apple_health_export/") } },
  { received_at: R2, input: { kind: "directory", files: apple } },
  { received_at: R3, input: { kind: "directory", files: apple, open: "export.xml" } },
]);

const duplicateRecord = appleXml.match(/ <Record type="HKQuantityTypeIdentifierRestingHeartRate"[^\n]+\/>/)?.[0];
if (duplicateRecord === undefined) throw new Error("the duplicate record is missing");
const duplicated = { ...apple, "export.xml": appleXml.replace(duplicateRecord, `${duplicateRecord}\n${duplicateRecord}`) };
const duplicatedCorrected = {
  ...apple,
  "export.xml": (duplicated["export.xml"] as string).replaceAll(duplicateRecord, duplicateRecord.replace('value="49"', 'value="48"')),
};
await scenario("watch-duplicate", "A repeated record: numbered keys, an unchanged repeat, then an ambiguous correction refused as a conflict.", [
  { received_at: R1, input: { kind: "directory", files: duplicated } },
  { received_at: R2, input: { kind: "directory", files: duplicated } },
  { received_at: R3, input: { kind: "directory", files: duplicatedCorrected } },
]);

await scenario("watch-deleted", "A deleted measurement that a later import names is reported as a conflict.", [
  { received_at: R1, input: { kind: "directory", files: apple } },
  {
    received_at: R2,
    input: { kind: "directory", files: apple },
    delete_before: { label_prefix: '<point_measurement apple_health/v1 ["point_measurement","<source', deleted_at: "2026-10-13T13:00:00.000Z" },
  },
]);

await scenario("watch-inline-route", "Inline WorkoutRoute locations: stored points, a bad point, and an empty route.", [
  { received_at: R1, input: { kind: "directory", files: fixtureFiles("apple-inline-route") } },
  { received_at: R2, input: { kind: "directory", files: fixtureFiles("apple-inline-route") } },
]);

await scenario("watch-file-route", "A FileReference route in a folder, then the same export as a zip.", [
  { received_at: R1, input: { kind: "directory", files: fixtureFiles("apple-file-reference") } },
  { received_at: R2, input: { kind: "zip", files: fixtureFiles("apple-file-reference", "apple_health_export/") } },
]);

await scenario("watch-route-mismatch", "Route files outside a workout's time, a file named twice, and a GPX time without an offset.", [
  { received_at: R1, input: { kind: "directory", files: fixtureFiles("apple-route-mismatch") } },
]);

// ---- Invented route scope cases ---------------------------------------------------

function workoutXml(...references: string[]): string {
  const routesXml = references
    .map((reference) => `  <WorkoutRoute sourceName="Demo Phone">\n   <FileReference path="${reference}"/>\n  </WorkoutRoute>\n`)
    .join("");
  return `<?xml version="1.0" encoding="UTF-8"?>
<HealthData>
 <Workout workoutActivityType="HKWorkoutActivityTypeRunning" duration="40" durationUnit="min" sourceName="Apple Watch" startDate="2026-09-03 09:00:00 -0500" endDate="2026-09-03 09:40:00 -0500">
${routesXml} </Workout>
</HealthData>
`;
}

function gpxAt(lat: string, lon: string, first: string, second: string): string {
  return `<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1"><trk><trkseg>
 <trkpt lat="${lat}" lon="${lon}"><time>${first}</time></trkpt>
 <trkpt lat="${lat}" lon="${lon}"><time>${second}</time></trkpt>
</trkseg></trk></gpx>
`;
}

await scenario("watch-route-scope", "Route files stay inside the export: a bare XML ignores sibling folders, a missing file is named, and two files on one workout are both stored.", [
  {
    received_at: R1,
    input: {
      kind: "directory",
      open: "export.xml",
      files: {
        "export.xml": workoutXml("/workout-routes/near.gpx"),
        "workout-routes/near.gpx": gpxAt("16.5", "60.5", "2026-09-03T09:01:00-05:00", "2026-09-03T09:02:00-05:00"),
        "other/workout-routes/near.gpx": gpxAt("17.5", "61.5", "2026-09-03T09:01:00-05:00", "2026-09-03T09:02:00-05:00"),
        "other/electrocardiograms/ecg_2026-09-03.csv": "not an ecg\n",
      },
    },
  },
  {
    received_at: R2,
    input: { kind: "directory", open: "export.xml", files: { "export.xml": workoutXml("/workout-routes/missing.gpx") } },
  },
  {
    received_at: R3,
    input: {
      kind: "directory",
      files: {
        "export.xml": workoutXml("/workout-routes/first.gpx", "\\workout-routes\\SECOND.gpx"),
        "workout-routes/first.gpx": gpxAt("18.5", "62.5", "2026-09-03T09:01:00-05:00", "2026-09-03T09:02:00-05:00"),
        "workout-routes/second.gpx": gpxAt("18.75", "62.75", "2026-09-03T09:10:00-05:00", "2026-09-03T09:11:00-05:00"),
      },
    },
  },
]);

// ---- Invented edge cases ------------------------------------------------------------

const DEVICE_A = "&lt;&lt;HKDevice: 0x1&gt;, name:Apple Watch, manufacturer:Apple Inc., model:Watch, hardware:Watch6,2, software:10.2&gt;";
const DEVICE_A2 = "&lt;&lt;HKDevice: 0x9&gt;, name:Apple Watch, manufacturer:Apple Inc., model:Watch, hardware:Watch6,2, software:10.4&gt;";
const DEVICE_B = "&lt;&lt;HKDevice: 0x2&gt;, name:Apple Watch, manufacturer:Apple Inc., model:Watch, hardware:Watch7,1, software:11.0&gt;";
const DEVICE_EMPTY = "&lt;&lt;HKDevice: 0x3&gt;, software:10.2&gt;";
const DEVICE_SCALE = "name:Body Scale, manufacturer:Withings SA";

function record(type: string, attributes: Record<string, string>, body = ""): string {
  const text = Object.entries(attributes)
    .map(([key, value]) => ` ${key}="${value}"`)
    .join("");
  return body === "" ? ` <Record type="${type}"${text}/>` : ` <Record type="${type}"${text}>\n${body}\n </Record>`;
}

const Q = (name: string) => `HKQuantityTypeIdentifier${name}`;
const at = (time: string, offset = "-0700") => `2026-03-01 ${time} ${offset}`;
const point = (name: string, writer: string, time: string, value: string, unit?: string, extra: Record<string, string> = {}) =>
  record(Q(name), { sourceName: writer, ...(unit === undefined ? {} : { unit }), startDate: time, endDate: time, value, ...extra });
const span = (name: string, writer: string, start: string, end: string, value: string, unit?: string, extra: Record<string, string> = {}) =>
  record(Q(name), { sourceName: writer, ...(unit === undefined ? {} : { unit }), startDate: start, endDate: end, value, ...extra });
const sleep = (writer: string, start: string, end: string, value: string, extra: Record<string, string> = {}) =>
  record("HKCategoryTypeIdentifierSleepAnalysis", { sourceName: writer, startDate: start, endDate: end, value: `HKCategoryValueSleepAnalysis${value}`, ...extra });

/** Beats a second apart with a varying rate; `time` writes the clock text. */
function beats(count: number, startSecond: number, rate: (index: number) => string, time: (second: number) => string): string {
  return Array.from({ length: count }, (_, index) => {
    const second = startSecond + index * 0.95;
    return `   <InstantaneousBeatsPerMinute bpm="${rate(index)}" time="${time(second)}"/>`;
  }).join("\n");
}
const clock = (hour: number, minute: number) => (second: number) =>
  `${hour}:${String(minute + Math.floor(second / 60)).padStart(2, "0")}:${(second % 60).toFixed(2).padStart(5, "0")}`;

const EDGE_XML = [
  `<?xml version="1.0" encoding="UTF-8"?>`,
  `<!DOCTYPE HealthData [\n<!ELEMENT HealthData ANY>\n]>`,
  `<HealthData locale="en_US">`,
  ` <ExportDate value="2026-03-10 09:00:00 -0700"/>`,
  ` <!-- Writers, hand-entered values, and devices seen on several days. -->`,
  point("RestingHeartRate", "Alex's Apple Watch", at("06:00:00"), "52", "count/min", { device: DEVICE_A }),
  point("RestingHeartRate", "Alex's Apple Watch", "2026-03-03 06:00:00 -0700", "51", "count/min", { device: DEVICE_B }),
  point("RestingHeartRate", "Alex's Apple Watch", "2026-02-27 06:00:00 -0700", "53", "count/min", { device: DEVICE_A2 }),
  point("RestingHeartRate", "Alex's Apple Watch", "2026-03-02 06:00:00 -0700", "50", "count/min", { device: DEVICE_EMPTY }),
  point("RestingHeartRate", "Alex's Apple Watch", "2026-03-04 06:00:00 -0700", "49", "count/min", { device: DEVICE_A }),
  point("RestingHeartRate", "Alex's Apple Watch", "not a date", "49", "count/min", { device: DEVICE_A }),
  point("BodyMass", "  Health  ", at("07:00:00"), "80.5", "kg"),
  point("BodyMass", "", at("07:01:00"), "176", "lb"),
  point("BodyMass", "Scale Co", at("07:02:00"), "12.5", "st", { device: DEVICE_SCALE }),
  point("BodyMass", "Scale Co", at("07:03:00", "-07"), "80500", "g"),
  point("Height", "MANUAL", at("07:04:00"), "70", "in"),
  point("BodyFatPercentage", "iPad", at("07:05:00"), "18.5", "%"),
  point("LeanBodyMass", "iPad", at("07:05:00"), "65", "kg"),
  point("BodyTemperature", "Thermo", at("06:30:00", "+0530"), "98.6", "degF"),
  point("BodyTemperature", "Thermo", "2026-03-01T06:31:00Z", "99", "°F"),
  point("AppleSleepingWristTemperature", "Alex's Apple Watch", "2026-03-01T03:00:00.250-07:00", "33.4", "degC"),
  point("VO2Max", "Alex's Apple Watch", at("08:00:00"), "0", "mL/min·kg"),
  point("VO2Max", "Alex's Apple Watch", at("08:01:00"), "abc", "mL/min·kg"),
  point("VO2Max", "Alex's Apple Watch", "yesterday", "50", "mL/min·kg"),
  point("BloodPressureSystolic", "Cuff", at("08:02:00"), "16", "kPa"),
  point("BloodPressureDiastolic", "Cuff", at("08:02:00"), "70", "mmHg"),
  point("RestingHeartRate", "Ring", at("06:10:00"), "55"),
  point("WalkingHeartRateAverage", "Ring", at("12:00:00"), "1e2", "count/min"),
  point("HeartRateRecoveryOneMinute", "Ring", at("12:10:00"), "-3", "count/min"),
  point("RestingHeartRate", "Ring", at("06:20:00"), "56", "bpm"),
  point("RestingHeartRate", "Ring", at("06:20:00"), "56", "bpm"),
  record(Q("RestingHeartRate"), { sourceName: "Ring", unit: "count/min", startDate: at("06:25:00"), endDate: at("06:25:00"), value: "57" }, `  <MetadataEntry key="HKTimeZone" value=" Asia/Kolkata "/>\n  <MetadataEntry key="HKTimeZone" value=""/>`),
  ` <!-- Series: two writers at one instant, days, offsets, spans, and units. -->`,
  span("HeartRate", "Alex's Apple Watch", at("08:10:00"), at("08:10:00"), "71", "count/min"),
  span("HeartRate", "Chest Strap", at("08:10:00"), at("08:10:00"), "70", "count/min"),
  span("HeartRate", "Alex's Apple Watch", at("08:05:00"), at("08:05:00"), "64", "count/min"),
  span("HeartRate", "Alex's Apple Watch", at("08:00:00"), at("08:00:30"), "62", "count/min"),
  span("HeartRate", "Alex's Apple Watch", at("23:30:00"), at("23:30:00"), "58", "count/min"),
  span("HeartRate", "Alex's Apple Watch", "2026-03-02 07:30:00 +0100", "2026-03-02 07:30:00 +0100", "59", "count/min"),
  span("HeartRate", "Alex's Apple Watch", "2026-03-01 23:30:00 -0700", "2026-03-01 23:30:00 -0700", "58", "count/min"),
  span("HeartRate", "Alex's Apple Watch", "2026-03-02 06:30:00 -0700", "2026-03-02 06:30:00 -0700", "57", "count/min"),
  span("HeartRate", "Alex's Apple Watch", "2026-03-02 06:31:00 -0700", "2026-03-02 06:31:00 -0700", "57"),
  span("HeartRate", "Alex's Apple Watch", "2026-03-02 06:32:00 -0700", "2026-03-02 06:32:00 -0700", "x", "count/min"),
  span("HeartRate", "Alex's Apple Watch", "soon", "soon", "60", "count/min"),
  span("HeartRate", "Alex's Apple Watch", "2026-03-02 06:33:00 -0700", "2026-03-02 06:33:00 -0700", "60", "count/s"),
  span("OxygenSaturation", "Alex's Apple Watch", at("04:00:00"), at("04:00:00"), "0.95", "%"),
  span("OxygenSaturation", "Alex's Apple Watch", at("04:05:00"), at("04:05:00"), "96", "%"),
  span("StepCount", "iPhone", at("09:00:00"), at("09:05:00"), "420", "count"),
  span("StepCount", "iPhone", at("09:05:00"), at("09:04:00"), "380", "count"),
  span("StepCount", "iPhone", at("09:10:00"), at("09:15:00"), "12", "steps"),
  span("ActiveEnergyBurned", "Alex's Apple Watch", at("18:30:00"), at("18:31:00"), "41.84", "kJ"),
  span("BasalEnergyBurned", "Alex's Apple Watch", at("18:30:00"), at("19:30:00"), "72.4", "Cal"),
  span("DistanceWalkingRunning", "Alex's Apple Watch", at("18:30:00"), at("18:40:00"), "5280", "ft"),
  span("DistanceCycling", "Alex's Apple Watch", at("17:00:00"), at("17:30:00"), "1.5", "mi"),
  span("DistanceSwimming", "Alex's Apple Watch", at("16:00:00"), at("16:20:00"), "1200", "yd"),
  span("RunningSpeed", "Alex's Apple Watch", at("18:31:00"), at("18:31:00"), "7.5", "mi/hr"),
  span("RunningSpeed", "Alex's Apple Watch", at("18:32:00"), at("18:32:00"), "11", "ft/s"),
  span("RunningPower", "Alex's Apple Watch", at("18:31:00"), at("18:31:00"), "0.3", "kW"),
  span("RunningStrideLength", "Alex's Apple Watch", at("18:31:00"), at("18:31:00"), "118", "cm"),
  span("RunningVerticalOscillation", "Alex's Apple Watch", at("18:31:00"), at("18:31:00"), "84", "mm"),
  span("RunningGroundContactTime", "Alex's Apple Watch", at("18:31:00"), at("18:31:00"), "0.243", "s"),
  span("PhysicalEffort", "Alex's Apple Watch", at("18:31:00"), at("18:31:00"), "9.4", "MET"),
  span("AppleExerciseTime", "Alex's Apple Watch", at("18:30:00"), at("18:31:00"), "60", "s"),
  span("TimeInDaylight", "Alex's Apple Watch", at("12:00:00"), at("13:00:00"), "1", "hr"),
  span("FlightsClimbed", "iPhone", at("12:00:00"), at("12:05:00"), "4"),
  span("WalkingSpeed", "iPhone", at("12:00:00"), at("12:05:00"), "5", "km/h"),
  span("WalkingStepLength", "iPhone", at("12:00:00"), at("12:05:00"), "30", "in"),
  span("WalkingAsymmetryPercentage", "iPhone", at("12:00:00"), at("12:05:00"), "1.4", "%"),
  span("RespiratoryRate", "Alex's Apple Watch", at("03:00:00"), at("03:00:00"), "14.5", "count/min"),
  ` <!-- HRV: beats under SDNN, with bad rates, clocks that fit nowhere, a missed beat, and short and long windows. -->`,
  record(Q("HeartRateVariabilitySDNN"), { sourceName: "Alex's Apple Watch", device: DEVICE_A, unit: "ms", startDate: at("06:12:00"), endDate: at("06:13:01"), value: "52.3" }, [
    `  <MetadataEntry key="HKTimeZone" value="America/Denver"/>`,
    `  <HeartRateVariabilityMetadataList>`,
    beats(30, 0.22, (index) => String(58 + ((index * 7) % 13)), clock(6, 12)),
    `   <InstantaneousBeatsPerMinute bpm="0" time="6:12:30.00"/>`,
    `   <InstantaneousBeatsPerMinute bpm="abc" time="6:12:30.50"/>`,
    `   <InstantaneousBeatsPerMinute time="6:12:30.70"/>`,
    `   <InstantaneousBeatsPerMinute bpm="61" time="noon"/>`,
    `   <InstantaneousBeatsPerMinute bpm="61"/>`,
    `   <InstantaneousBeatsPerMinute bpm="250" time="6:12:31.00"/>`,
    beats(20, 34.5, (index) => String(60 + ((index * 5) % 9)), clock(6, 12)),
    `  </HeartRateVariabilityMetadataList>`,
  ].join("\n")),
  record(Q("HeartRateVariabilitySDNN"), { sourceName: "Alex's Apple Watch", unit: "s", startDate: "2026-03-01 22:30:00 -0700", endDate: "2026-03-01 22:30:06 -0700", value: "0.0418" }, [
    `  <HeartRateVariabilityMetadataList>`,
    beats(4, 0.1, () => "64", clock(10, 30)),
    `  </HeartRateVariabilityMetadataList>`,
  ].join("\n")),
  record(Q("HeartRateVariabilitySDNN"), { sourceName: "Alex's Apple Watch", unit: "ms", startDate: "2026-03-02 01:00:00 -0700", endDate: "2026-03-02 02:00:00 -0700", value: "40" }, [
    `  <HeartRateVariabilityMetadataList>`,
    beats(3, 0.1, () => "64", clock(1, 0)),
    `  </HeartRateVariabilityMetadataList>`,
  ].join("\n")),
  record(Q("HeartRateVariabilitySDNN"), { sourceName: "Health", unit: "ms", startDate: "2026-03-02 05:00:00 -0700", endDate: "2026-03-02 05:01:00 -0700", value: "45" }, [
    `  <HeartRateVariabilityMetadataList>`,
    beats(40, 0.3, (index) => String(55 + ((index * 3) % 7)), clock(5, 0)),
    `  </HeartRateVariabilityMetadataList>`,
  ].join("\n")),
  ` <InstantaneousBeatsPerMinute bpm="61" time="6:12:30.00"/>`,
  ` <!-- Sleep: stages, two nights, a gap, two hand-entered writers, and a fragment that ends before it starts. -->`,
  sleep("Alex's Apple Watch", "2026-03-01 23:00:00 -0700", "2026-03-02 07:00:00 -0700", "InBed", { device: DEVICE_A }),
  sleep("Alex's Apple Watch", "2026-03-02 01:00:00 -0700", "2026-03-02 02:00:00 -0700", "AsleepDeep"),
  sleep("Alex's Apple Watch", "2026-03-01 23:15:00 -0700", "2026-03-02 01:00:00 -0700", "AsleepCore", { device: DEVICE_EMPTY }),
  sleep("Alex's Apple Watch", "2026-03-02 02:00:00 -0700", "2026-03-02 02:10:00 -0700", "Awake"),
  sleep("Alex's Apple Watch", "2026-03-02 02:10:00 -0700", "2026-03-02 03:40:00 -0700", "AsleepREM"),
  sleep("Alex's Apple Watch", "2026-03-02 03:40:00 -0700", "2026-03-02 06:20:00 -0700", "Asleep"),
  sleep("Alex's Apple Watch", "2026-03-02 06:20:00 -0700", "2026-03-02 06:25:00 -0700", "Awake"),
  sleep("Alex's Apple Watch", "2026-03-02 06:25:00 -0700", "2026-03-02 06:50:00 -0700", "AsleepUnspecified"),
  sleep("Alex's Apple Watch", "2026-03-02 06:50:00 -0700", "2026-03-02 06:55:00 -0700", "Mystery"),
  sleep("Alex's Apple Watch", "2026-03-02 13:00:00 -0700", "2026-03-02 13:40:00 -0700", "AsleepCore"),
  sleep("Alex's Apple Watch", "2026-03-02 22:45:00 -0700", "2026-03-03 06:30:00 -0700", "AsleepCore"),
  sleep("Health", "2026-03-01 22:00:00 -0700", "2026-03-02 06:00:00 -0700", "InBed"),
  sleep("Manual", "2026-03-01 22:30:00 -0700", "2026-03-02 05:30:00 -0700", "Asleep"),
  sleep("Night Owl", "2026-03-05 23:00:00 -0700", "2026-03-05 22:00:00 -0700", "InBed"),
  sleep("Night Owl", "late", "2026-03-05 22:00:00 -0700", "InBed"),
  record("HKCategoryTypeIdentifierSleepAnalysis", { sourceName: "Night Owl", startDate: "2026-03-06 23:00:00 -0700", endDate: "2026-03-07 06:00:00 -0700", value: "HKCategoryValueSleepAnalysisInBed" }, `  <MetadataEntry key="HKTimeZone" value="Europe/Berlin"/>`),
  ` <!-- Workouts: laps, heart rate, units, timezone, unusable times, and routes. -->`,
  ` <WorkoutRoute sourceName="Stray"><Location date="2026-03-01 08:00:00 -0700" latitude="1" longitude="1"/></WorkoutRoute>`,
  ` <Workout workoutActivityType="HKWorkoutActivityTypeFunctionalStrengthTraining" sourceName="Alex's Apple Watch" device="${DEVICE_A}" totalDistance="1.5" totalDistanceUnit="mi" totalEnergyBurned="1000" totalEnergyBurnedUnit="kJ" startDate="2026-03-01 18:00:00 -0700" endDate="2026-03-01 18:45:00 -0700">`,
  `  <MetadataEntry key="HKAverageHeartRate" value="140"/>`,
  `  <MetadataEntry key="HKMaximumHeartRate" value="0"/>`,
  `  <MetadataEntry key="HKMaximumHeartRate" value="171.456"/>`,
  `  <MetadataEntry key="HKAverageHeartRate" value="142.005"/>`,
  `  <MetadataEntry key="HKTimeZone" value="America/Phoenix"/>`,
  `  <MetadataEntry key="HKIndoorWorkout" value="1"/>`,
  `  <WorkoutEvent type="HKWorkoutEventTypeLap" duration="10.0" durationUnit="min"/>`,
  `  <WorkoutEvent type="HKWorkoutEventTypePause" duration="1.0" durationUnit="min"/>`,
  `  <WorkoutEvent type="HKWorkoutEventTypeLap"/>`,
  `  <WorkoutEvent type="HKWorkoutEventTypeLap" duration="9.25" durationUnit="min"/>`,
  ` </Workout>`,
  ` <Workout workoutActivityType="HKWorkoutActivityTypeRunning" sourceName="Alex's Apple Watch" startDate="2026-03-01 19:00:00 -0700" endDate="2026-03-01 18:00:00 -0700"/>`,
  ` <Workout workoutActivityType="HKWorkoutActivityType" sourceName="Alex's Apple Watch" endDate="2026-03-01 18:00:00 -0700"/>`,
  ` <Workout workoutActivityType="HKWorkoutActivityTypeRunning" sourceName="Alex's Apple Watch" totalDistance="5" totalDistanceUnit="furlong" startDate="2026-03-02 07:00:00 -0700" endDate="2026-03-02 07:40:00 -0700">`,
  `  <WorkoutRoute sourceName="Demo Phone" sourceVersion="1">`,
  `   <MetadataEntry key="HKMetadataKeySyncVersion" value="2"/>`,
  `   <MetadataEntry key="OnlyKey"/>`,
  `   <MetadataEntry value="only value"/>`,
  `   <MetadataEntry/>`,
  `   <Location date="2026-03-02 07:01:00 -0700" latitude="40.0" longitude="-105.0" altitude="1600" speed="3.1" course=""/>`,
  `   <Location date="2026-03-02 07:02:00 -0700" latitude="40.001" longitude="-105.0"/>`,
  `   <Location date="2026-03-02 07:03:00 -0700" latitude="40.002"/>`,
  `   <Location date="2026-03-02 07:04:00" latitude="40.003" longitude="-105.0"/>`,
  `   <Location date="2026-03-02 07:05:00 -0700" latitude="40.004" longitude="200"/>`,
  `   <FileReference path="/workout-routes/run.gpx"/>`,
  `  </WorkoutRoute>`,
  `  <WorkoutRoute sourceName="Demo Phone"/>`,
  ` </Workout>`,
  ` <Workout workoutActivityType="HKWorkoutActivityTypeRunning" sourceName="Alex's Apple Watch" startDate="2026-03-03 07:00:00 -0700" endDate="2026-03-03 07:45:00 -0700">`,
  `  <WorkoutRoute sourceName="Demo Phone"><FileReference path="/workout-routes/Route.GPX"/></WorkoutRoute>`,
  ` </Workout>`,
  ` <Workout workoutActivityType="HKWorkoutActivityTypeWalking" sourceName="Alex's Apple Watch" startDate="2026-03-03 07:10:00 -0700" endDate="2026-03-03 07:20:00 -0700">`,
  `  <WorkoutRoute sourceName="Demo Phone"><FileReference path="workout-routes/route.gpx"/></WorkoutRoute>`,
  ` </Workout>`,
  ` <Workout workoutActivityType="HKWorkoutActivityTypeCycling" sourceName="Alex's Apple Watch" startDate="2026-03-04 07:00:00 -0700" endDate="2026-03-04 07:30:00 -0700">`,
  `  <WorkoutRoute sourceName="Demo Phone"><FileReference path="/workout-routes/outside.gpx"/></WorkoutRoute>`,
  `  <WorkoutRoute sourceName="Demo Phone"><FileReference path="/workout-routes/notimes.gpx"/></WorkoutRoute>`,
  `  <WorkoutRoute sourceName="Demo Phone"><FileReference path="/workout-routes/gone.gpx"/></WorkoutRoute>`,
  `  <WorkoutRoute sourceName="Demo Phone"><FileReference path=" "/></WorkoutRoute>`,
  ` </Workout>`,
  ` <Workout workoutActivityType="HKWorkoutActivityTypeHiking" sourceName="Alex's Apple Watch" startDate="2026-03-05 07:00:00 -0700" endDate="2026-03-05 09:00:00 -0700">`,
  `  <WorkoutEvent type="HKWorkoutEventTypeLap" duration="30" durationUnit="min"/>`,
  ` </Workout>`,
  ` <!-- Activity summaries: units, goals, and values that cannot be read. -->`,
  ` <ActivitySummary dateComponents="2026-03-01" activeEnergyBurned="2510" activeEnergyBurnedGoal="2900" activeEnergyBurnedUnit="kJ" appleExerciseTime="48" appleStandHours="13"/>`,
  ` <ActivitySummary dateComponents="2026-03-02" activeEnergyBurned="400" activeEnergyBurnedGoal="500" appleExerciseTimeGoal="30" appleStandHoursGoal="12"/>`,
  ` <ActivitySummary dateComponents="2026-03-03" activeEnergyBurned="NaN" activeEnergyBurnedUnit="kcal" appleExerciseTime="-5" appleStandHours="x"/>`,
  ` <ActivitySummary dateComponents="2026-3-4" activeEnergyBurned="1" activeEnergyBurnedUnit="kcal"/>`,
  ` <ActivitySummary activeEnergyBurned="1" activeEnergyBurnedUnit="kcal"/>`,
  ` <ActivitySummary sourceName="Fitness" dateComponents=" 2026-03-05 " activeEnergyBurned="300" activeEnergyBurnedUnit="Cal"/>`,
  ` <ActivitySummary dateComponents="2026-03-06" activeEnergyBurned="300" activeEnergyBurnedUnit="joules"/>`,
  ` <!-- Out of scope or unmapped. -->`,
  record(Q("HeadphoneAudioExposure"), { sourceName: "iPhone", unit: "dBASPL", startDate: at("14:00:00"), endDate: at("14:30:00"), value: "72.5" }),
  record("HKDataTypeSleepDurationGoal", { sourceName: "Health", startDate: at("14:00:00"), endDate: at("14:00:00"), value: "8" }),
  record("HKQuantityTypeIdentifier", { sourceName: "iPhone", startDate: at("14:00:00"), endDate: at("14:00:00"), value: "1" }),
  record("", { sourceName: "iPhone", startDate: at("14:00:00"), endDate: at("14:00:00"), value: "1" }),
  ` <Correlation type="HKCorrelationTypeIdentifierBloodPressure" sourceName="Cuff" startDate="${at("08:02:00")}" endDate="${at("08:02:00")}"/>`,
  ` <ClinicalRecord type="Immunization" identifier="x"/>`,
  ` <ClinicalRecord type="Condition" identifier="y"/>`,
  `</HealthData>`,
  ``,
].join("\n");

function track(points: Array<[number, number, string, string | null]>): string {
  const body = points
    .map(([lat, lon, time, elevation]) =>
      `  <trkpt lat="${lat}" lon="${lon}">${elevation === null ? "" : `<ele>${elevation}</ele>`}<time>${time}</time></trkpt>`,
    )
    .join("\n");
  return `<?xml version="1.0" encoding="UTF-8"?>\n<gpx version="1.1"><trk><trkseg>\n${body}\n</trkseg></trk></gpx>\n`;
}

/** A northward run with GPS wobble in position and altitude. */
function noisyRun(startIso: string, seconds: number, metresPerSecond: number, seed: number): string {
  let state = seed;
  const next = () => {
    state = (state * 16807) % 2147483647;
    return state / 2147483647;
  };
  const startMs = Date.parse(startIso);
  const points: Array<[number, number, string, string | null]> = [];
  let latitude = 40;
  for (let second = 0; second <= seconds; second += 3) {
    latitude += (metresPerSecond * 3) / 111_195;
    const wobble = (next() - 0.5) * 0.00003;
    const elevation = 1600 + second / 20 + (next() - 0.5) * 1.6;
    const iso = new Date(startMs + second * 1000).toISOString().replace(".000Z", "-00:00");
    points.push([Number((latitude + wobble).toFixed(7)), Number((-105 + wobble).toFixed(7)), iso, elevation.toFixed(2)]);
  }
  return track(points);
}

const EDGE_ROUTES: Record<string, string> = {
  "workout-routes/Route.GPX": noisyRun("2026-03-03T14:01:00Z", 1500, 3.4, 7),
  "workout-routes/run.gpx": track([
    [40, -105, "2026-03-02T07:01:00-07:00", "1600"],
    [40.01, -105, "2026-03-02T07:06:00-07:00", "1610"],
  ]),
  "workout-routes/outside.gpx": track([
    [41, -105, "2026-03-10T07:01:00-07:00", null],
    [41.01, -105, "2026-03-10T07:06:00-07:00", null],
  ]),
  "workout-routes/notimes.gpx": `<gpx><trk><trkseg>\n  <trkpt lat="41" lon="-105"><ele>5</ele></trkpt>\n  <trkpt lat="41.1" lon="-105"><time>later</time></trkpt>\n  <trkpt lat="41.2" lon="-105"><time>   </time></trkpt>\n</trkseg></trk></gpx>\n`,
  "workout-routes/late.gpx": noisyRun("2026-03-08T14:00:00Z", 900, 2.9, 11),
  "workout-routes/tiny.gpx": track([[40, -105, "2026-03-09T07:01:00Z", "1"]]),
  "workout-routes/quirks.gpx": [
    `<gpx><trk><trkseg>`,
    `<trkpt lat="40.5" lon="-105.5"/>`,
    `<trkpt lat="40.6" lon="-105.5"><ele> 12 </ele><time>2026-03-05T14:10:00Z</time></trkpt>`,
    `<trkpt lat="9999" lon="-105.5"><ele>13</ele><time>2026-03-05T14:20:00Z</time></trkpt>`,
    `<trkpt data-lat="1" lat="40.7" lon = "-105.5"><ele>  </ele><time>2026-03-05T14:30:00+00:00</time></trkpt>`,
    `<trkpts lat="40.8" lon="-105.5"><time>2026-03-05T14:40:00Z</time></trkpts>`,
    `<trkpt lat="40.9" lon="-105.5"><time>2026-03-05 14:50:00Z</time></trkpt>`,
    `<trkpt lat="40.95" lon="-105.5"><ele>abc</ele><time>2026-03-05T15:00:00.123456Z</time></trkpt>`,
    `</trkseg></trk></gpx>`,
  ].join("\n"),
  "workout-routes/nested/deep.gpx": track([
    [40, -105, "2026-03-05T14:00:00Z", "1"],
    [40.1, -105, "2026-03-05T14:30:00Z", "1"],
  ]),
  "workout-routes/notes.txt": "not a route\n",
};

// ---- Invented ECG recordings ------------------------------------------------------

interface Waveform {
  rate: number;
  seconds: number;
  intervals: (index: number) => number;
  noise?: number;
  drift?: number;
  tWave?: number;
  artifact?: number;
  seed?: number;
}

/** Samples built only from exact arithmetic, so any platform builds the same signal. */
function waveform(spec: Waveform): number[] {
  const total = Math.round(spec.seconds * spec.rate);
  const signal = new Array<number>(total).fill(0);
  let state = spec.seed ?? 1;
  const next = () => {
    state = (state * 16807) % 2147483647;
    return state / 2147483647;
  };
  const width = Math.round(0.04 * spec.rate);
  const tWidth = Math.round(0.08 * spec.rate);
  let atMs = 400;
  for (let beat = 0; atMs < spec.seconds * 1000 - 200; beat += 1) {
    const centre = Math.round((atMs / 1000) * spec.rate);
    for (let k = -width; k <= width; k += 1) {
      const index = centre + k;
      if (index < 0 || index >= total) continue;
      const x = k / (0.02 * spec.rate);
      signal[index]! += 1000 / (1 + x * x * x * x);
    }
    if (spec.tWave !== undefined) {
      const tCentre = Math.round(((atMs + 220) / 1000) * spec.rate);
      for (let k = -tWidth; k <= tWidth; k += 1) {
        const index = tCentre + k;
        if (index < 0 || index >= total) continue;
        const y = k / (0.05 * spec.rate);
        signal[index]! += spec.tWave / (1 + y * y);
      }
    }
    atMs += spec.intervals(beat);
  }
  const period = Math.round(spec.rate * 3);
  for (let index = 0; index < total; index += 1) {
    if (spec.noise !== undefined) signal[index]! += (next() - 0.5) * 2 * spec.noise;
    if (spec.drift !== undefined) {
      const phase = (index % period) / period;
      signal[index]! += spec.drift * (phase < 0.5 ? 4 * phase - 1 : 3 - 4 * phase);
    }
  }
  if (spec.artifact !== undefined) {
    const centre = Math.round(spec.seconds * 0.4 * spec.rate);
    const half = Math.round(0.05 * spec.rate);
    for (let index = centre - half; index <= centre + half; index += 1) {
      const x = (index - centre) / (0.01 * spec.rate);
      signal[index]! += spec.artifact / (1 + x * x * x * x);
    }
  }
  return signal;
}

function ecgCsv(header: string[], samples: number[]): string {
  return `${header.join("\n")}\n\nLead,Lead I\nUnit,µV\n\n${samples.map((value) => value.toFixed(1)).join("\n")}\n`;
}

const sinus = (date: string, rate: string, classification = "Sinus Rhythm") => [
  "Name,Invented Person",
  "Date of Birth,1990-01-01",
  `Recorded Date,${date}`,
  `Classification,${classification}`,
  "Symptoms,",
  "Software Version,2.1",
  'Device,"Watch7,2"',
  `Sample Rate,${rate}`,
];
const alternate = (low: number, high: number) => (index: number) => (index % 2 === 0 ? low : high);

const EDGE_ECGS: Record<string, string> = {
  "electrocardiograms/ecg_clean.csv": ecgCsv(
    sinus("2026-03-01 07:20:11 -0700", "128 hertz"),
    waveform({ rate: 128, seconds: 30, intervals: alternate(880, 920) }),
  ),
  "electrocardiograms/ecg_noisy.csv": ecgCsv(
    sinus("2026-03-01 07:30:00 -0700", "510.844 hertz"),
    waveform({ rate: 510.844, seconds: 30, intervals: (index) => 850 + ((index * 37) % 90), noise: 120, drift: 300, tWave: 350, seed: 5 }),
  ),
  "electrocardiograms/ecg_artifact.csv": ecgCsv(
    sinus("2026-03-02T07:00:00Z", "128,0 hertz"),
    waveform({ rate: 128, seconds: 30, intervals: alternate(900, 950), artifact: 60_000, seed: 9 }),
  ),
  "electrocardiograms/ecg_afib.csv": ecgCsv(
    sinus("2026-03-02 07:10:00 -0700", "128 hertz", "Atrial Fibrillation"),
    waveform({ rate: 128, seconds: 30, intervals: (index) => 600 + ((index * 211) % 500) }),
  ),
  "electrocardiograms/ecg_flat.csv": ecgCsv(sinus("2026-03-02 07:20:00 -0700", "128 hertz", "sinus  rhythm"), new Array(3840).fill(0)),
  "electrocardiograms/ecg_fast.csv": ecgCsv(
    sinus("2026-03-02 07:30:00 -0700", "128 hertz"),
    waveform({ rate: 128, seconds: 30, intervals: () => 260 }),
  ),
  "electrocardiograms/ecg_short.csv": ecgCsv(
    sinus("2026-03-02 07:40:00 -0700", "128 hertz"),
    waveform({ rate: 128, seconds: 3, intervals: () => 900 }),
  ),
  "electrocardiograms/ecg_nodate.csv": ecgCsv(sinus("someday", "128 hertz"), [1, 2, 3]),
  "electrocardiograms/ecg_norate.csv": ecgCsv(sinus("2026-03-02 07:50:00 -0700", "fast"), [1, 2, 3]),
  "electrocardiograms/ecg_nosamples.csv": sinus("2026-03-02 07:55:00 -0700", "128 hertz").join("\n"),
  "electrocardiograms/ecg_reordered.csv": ecgCsv(
    ["Sample Rate,128 hertz", "", "Classification,Sinus Rhythm", "Recorded Date,2026-03-03 07:00:00 -0700 (local)"],
    waveform({ rate: 128, seconds: 20, intervals: alternate(800, 860), noise: 40, seed: 3 }),
  ),
};

await scenario("watch-edges", "Invented XML, route files, and ECG recordings that exercise every reading rule and skip reason.", [
  { received_at: R1, input: { kind: "directory", files: { "export.xml": EDGE_XML, ...EDGE_ROUTES, ...EDGE_ECGS } } },
  { received_at: R2, input: { kind: "directory", files: { "export.xml": EDGE_XML, ...EDGE_ROUTES, ...EDGE_ECGS } } },
  {
    received_at: R3,
    input: {
      kind: "zip",
      files: Object.fromEntries(
        Object.entries({ "export.xml": EDGE_XML, ...EDGE_ROUTES, ...EDGE_ECGS }).map(([name, text]) => [`apple_health_export/${name}`, text]),
      ),
    },
  },
]);

await scenario("watch-refusals", "Exports refused before any record: broken XML, a missing export.xml, and an unreadable ECG date.", [
  { received_at: R1, input: { kind: "directory", files: { "export.xml": "<HealthData><Record type=\"x\"></HealthData>" } } },
  {
    received_at: R2,
    input: {
      kind: "directory",
      files: {
        "export.xml": `<HealthData>${point("RestingHeartRate", "Watch", "2026-13-01 06:00:00 -0700", "50", "count/min")}<Record></HealthData>`,
      },
    },
  },
  {
    received_at: R3,
    input: { kind: "directory", files: { "apple_health_export/notes.txt": "x", "apple_health_export/export.xml": "<HealthData/>", "export.xml/readme.txt": "x" } },
  },
  {
    received_at: R4,
    input: {
      kind: "directory",
      files: {
        "export.xml": `<HealthData>${point("RestingHeartRate", "Watch", "2026-13-01 06:00:00 -0700", "50", "count/min")}</HealthData>`,
        "electrocardiograms/ecg_bad.csv": ecgCsv(
          sinus("2026-13-01 07:20:11 -0700", "128 hertz"),
          waveform({ rate: 128, seconds: 30, intervals: alternate(880, 920) }),
        ),
      },
    },
  },
]);

// 2.1.0 planned these records and then refused the write, because the stored
// record failed the format. The native preview refuses them (deliberate change 9).
await scenario("watch-invalid", "Records 2.1.0 planned but could not write: an empty unit text and an RMSSD of zero.", [
  {
    received_at: R1,
    input: {
      kind: "directory",
      files: {
        "export.xml": `<HealthData>\n${point("RestingHeartRate", "Watch", at("06:00:00"), "50", "count/min")}\n${span("OxygenSaturation", "Watch", at("04:10:00"), at("04:10:00"), "0.97", "")}\n</HealthData>\n`,
      },
    },
  },
  {
    received_at: R2,
    input: {
      kind: "directory",
      files: {
        "export.xml": `<HealthData>\n${record(Q("HeartRateVariabilitySDNN"), { sourceName: "Watch", unit: "ms", startDate: at("06:12:00"), endDate: at("06:13:01"), value: "30" }, `  <HeartRateVariabilityMetadataList>\n${beats(20, 0.2, () => "60", clock(6, 12))}\n  </HeartRateVariabilityMetadataList>`)}\n</HealthData>\n`,
      },
    },
  },
]);

// ---- Numeric vectors ------------------------------------------------------------------

{
  let state = 42;
  const next = () => {
    state = (state * 16807) % 2147483647;
    return state / 2147483647;
  };
  const cases: Array<[number, number, number, number]> = [
    [0, 0, 1, 0],
    [37.7749, -122.4194, 37.7749, -122.4194],
    [0, 0, 0, 180],
    [89.9999, 0, -89.9999, 180],
    [90, 0, -90, 0],
    [0, -179.9999, 0, 179.9999],
    [1000, 10, 1001, 11],
    [40, -105, 40.0089932, -105],
  ];
  for (let index = 0; index < 1500; index += 1) {
    const lat = (next() - 0.5) * 179.8;
    const lon = (next() - 0.5) * 359.8;
    const step = index % 3 === 0 ? 0.00001 : index % 3 === 1 ? 0.001 : 2;
    cases.push([lat, lon, lat + (next() - 0.5) * step, lon + (next() - 0.5) * step]);
  }
  const tracks = [
    noisyRun("2026-03-03T14:01:00Z", 1500, 3.4, 7),
    noisyRun("2026-03-03T14:01:00Z", 600, 0.4, 23),
    track([
      [40, -105, "2026-03-03T14:00:00Z", "10"],
      [40.0449661, -105, "2026-03-03T14:20:00Z", "12"],
    ]),
    // The quirks file with nearby positions, so it does not make thousands of splits.
    EDGE_ROUTES["workout-routes/quirks.gpx"]!.replace('lat="9999"', 'lat="40.65"').replace('data-lat="1"', 'data-lat="40.7"'),
  ];
  write("route-numerics.json", {
    reference: "2.1.0",
    description: "haversineM as V8 computed it, and summarizeRoute(parseGpx(text)) on invented tracks.",
    haversine: cases.map(([lat1, lon1, lat2, lon2]) => ({
      input: [lat1, lon1, lat2, lon2],
      metres: routes.haversineM(lat1, lon1, lat2, lon2),
    })),
    summaries: tracks.map((text) => ({ gpx: text, summary: routes.summarizeRoute(routes.parseGpx(text)) })),
  });
}

{
  const recordings = Object.entries(EDGE_ECGS).map(([name, text]) => {
    const parsed = ecg.parseEcgCsv(text);
    if ("kind" in parsed) return { name, parsed: parsed };
    const peaks = rpeaks.detectRPeaks(parsed.microvolts, parsed.sampleRateHz);
    const beats = ecg.beatsFromEcg(parsed);
    return {
      name,
      sample_count: parsed.microvolts.length,
      peaks,
      beats: "kind" in beats ? beats : { intervals_ms: beats.intervalsMs, quality: beats.quality, rmssd: hrv.rmssdFromIntervals(beats.intervalsMs) },
    };
  });
  write("beat-numerics.json", {
    reference: "2.1.0",
    description: "detectRPeaks, beatsFromEcg, and rmssdFromIntervals on the invented ECG recordings of watch-edges.json.",
    recordings,
  });
}

for (const name of readdirSync(outDirectory).sort()) {
  if (!name.startsWith("watch-") && !name.endsWith("-numerics.json")) continue;
  console.log(`${relative(root, join(outDirectory, name))} ${statSync(join(outDirectory, name)).size} bytes`);
}
