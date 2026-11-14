/**
 * Freeze the 2.1.0 provider import results that the native import must reproduce.
 *
 * Spec 2.1.1 section 13.1 compares old and new outputs at the strongest useful
 * level. This script runs the 2.1.0 library on synthetic WHOOP inputs and writes
 * what it stored: every record's complete JSON for every revision, revision
 * metadata, import keys, the preview the CLI printed with `--json`, the result of
 * the confirmed write, and refusals. It also records the 2.1.0 order of skip
 * reasons (`localeCompare`), the instants `Date.parse` reads, and detection errors.
 *
 * Record IDs are random, so each ID is replaced by a label built from the
 * record's type and match key. A source ID inside a key is replaced by the
 * source's own label. The tool version in `changed_by` is replaced by
 * `<ath version>`, because the native build stores its own version there.
 * Nothing else is normalized: `received_at`, `created_at`, `became_known_at`,
 * and `imported_at` are fixed by the script and compared exactly.
 *
 * Run from the repository root. ATH_REFERENCE_ROOT names a checkout of the
 * reference commit with its dependencies installed and its Rust helper built:
 *
 *   ATH_REFERENCE_ROOT=/path/to/reference node --import tsx native/tools/reference/import-golden.ts
 *
 * Output: native/tests/golden/import/*.json. File names avoid provider names, so
 * the public-data check does not read them as health exports.
 */
import { once } from "node:events";
import {
  cpSync,
  createWriteStream,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const reference = resolve(process.env.ATH_REFERENCE_ROOT ?? root);
const outDirectory = join(root, "native", "tests", "golden", "import");
const fixture = join(reference, "tests", "fixtures", "exports", "whoop");

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

const db = (await import(pathToFileURL(join(reference, "src/db/index.ts")).href)) as Db;
const { databaseClient } = (await import(pathToFileURL(join(reference, "src/db/client.ts")).href)) as {
  databaseClient(database: Database): {
    execute(query: string | { sql: string; args: unknown[] }): Promise<{ rows: Array<Record<string, unknown>> }>;
  };
};
const { detectExport } = (await import(pathToFileURL(join(reference, "src/import/detect.ts")).href)) as {
  detectExport(path: string): Promise<unknown>;
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

/** Keys sorted at every depth, as the native comparison writes them. */
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

/** Labels for record IDs, from each record's type and match key. */
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

/** The `ath import <export> --json` output, with IDs labelled. */
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

/** How a step's input is laid out. The native test builds the same layout. */
type Input =
  | { kind: "directory"; files: Record<string, string> }
  | { kind: "zip"; files: Record<string, string> }
  | { kind: "file"; name: string; text: string };

function fixtureFiles(): Record<string, string> {
  return Object.fromEntries(
    readdirSync(fixture)
      .sort()
      .map((name) => [name, readFileSync(join(fixture, name), "utf8")]),
  );
}

async function materialize(input: Input, directory: string): Promise<string> {
  if (input.kind === "file") {
    const path = join(directory, input.name);
    writeFileSync(path, input.text);
    return path;
  }
  if (input.kind === "directory") {
    const path = join(directory, "input");
    for (const [name, text] of Object.entries(input.files)) {
      mkdirSync(dirname(join(path, name)), { recursive: true });
      writeFileSync(join(path, name), text);
    }
    return path;
  }
  const path = join(directory, "input.zip");
  const zip = new ZipFile();
  for (const [name, text] of Object.entries(input.files)) {
    zip.addBuffer(Buffer.from(text), name, { mtime: new Date(0) });
  }
  const output = createWriteStream(path);
  zip.outputStream.pipe(output);
  zip.end();
  await once(output, "close");
  return path;
}

interface Step {
  received_at: string;
  input: Input;
  /** Delete the record with this label at this time before the step. */
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
      const preview = await db.previewProviderImport(database, path, { received_at: step.received_at });
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

function duplicateFirstRow(text: string): string {
  const [header, first, ...rest] = text.trimEnd().split(/\r?\n/);
  return [header, first, first, ...rest].join("\n") + "\n";
}

function corrected(files: Record<string, string>): Record<string, string> {
  return {
    ...files,
    "physiological_cycles.csv": files["physiological_cycles.csv"]!.replace(",68.4,", ",69.4,"),
  };
}

const EDGE_CYCLES = [
  "Cycle start time,Cycle end time,Cycle timezone,Recovery score %,Resting heart rate (bpm),Heart rate variability (ms),Skin temp (celsius),Blood oxygen %,Day Strain,Wake onset,Respiratory rate (rpm)",
  "2026-03-01 23:00:00,2026-03-02 07:00:00,UTC+05:30,0,48,55.555,33.257,0,12.345,2026-03-02 06:30,15.04",
  "2026-03-02 23:00,2026-03-03 07:00,-0700,-5,\"1,050\",61,,97.25,,,",
  ",,UTC-07:00,70,50,60,,,,,",
  "2026-03-04 22:00:00,2026-03-05 06:00:00,,71,50,60,,,,2026-03-05 05:00:00,",
  "2026-03-05 22:00:00,2026-03-06 06:00:00,UTCZ,72,51,62,,,1e1,2026-03-06T05:00:00.000-04:00,",
  "2026-03-06 22:00:00,,GMT,73,,,,,,,",
  ",2026-03-08 06:00:00,+530,74,52,,,,,,",
  "2026-03-08 22:00:00,2026-03-09 06:00:00,UTC +0000,abc,inf,-1,,,,,",
].join("\n");

const EDGE_SLEEPS = [
  "Cycle start time;Cycle end time;Cycle timezone;Sleep onset;Wake onset;Asleep duration (min);In bed duration (min);Light sleep duration (min);Deep (SWS) duration (min);REM duration (min);Awake duration (min);Sleep efficiency %",
  "2026-03-01 23:00:00;2026-03-02 07:00:00;UTC+05:30;2026-03-01 23:10:00;2026-03-02 06:30:00;400.5;440;200;0;100.25;\"40\";100.04",
  "2026-03-02 23:00:00;2026-03-03 07:00:00;UTC-07:00;2026-03-03 06:00:00;2026-03-03 06:00:00;1;1;1;1;1;1;1",
  "2026-03-03 23:00:00;2026-03-04 07:00:00;UTC-07:00;2026-03-03 23:30:00;;1;1;1;1;1;1;1",
  "2026-03-04 23:00:00;2026-03-05 07:00:00;UTC-07:00;2026-03-04 23:30:00;2026-03-05 06:00:00;;;;;;;0",
  "",
].join("\n");

const EDGE_WORKOUTS =
  "﻿Cycle start time,Cycle end time,Cycle timezone,Workout start time,Workout end time,Activity name,Activity Strain,Energy burned (cal),Max HR (bpm),Average HR (bpm),Distance (meters)\r\n" +
  "2026-03-01 23:00:00,2026-03-02 07:00:00,UTC+05:30,2026-03-02 17:00:00,2026-03-02 17:45:30,Functional Fitness,0,\"1,234\",181.6,140.5,\r\n" +
  "2026-03-02 23:00:00,2026-03-03 07:00:00,UTC-07:00,2026-03-03 18:00:00,2026-03-03 17:00:00,Running,5,100,150,120,5000\r\n" +
  "2026-03-02 23:00:00,2026-03-03 07:00:00,UTC-07:00,2026-03-03 19:00:00,2026-03-03 19:30:00,,,,,,\r\n";

const EDGE_JOURNAL = [
  "Cycle start time,Cycle end time,Cycle timezone,Question text,Answered yes,Notes",
  "2026-03-01 23:00:00,2026-03-02 07:00:00,UTC+05:30,Drink any coffee?,YES,\"two cups, black\"",
  "2026-03-01 23:00:00,2026-03-02 07:00:00,UTC+05:30,Feel sore?,1,",
  "2026-03-01 23:00:00,2026-03-02 07:00:00,UTC+05:30,Energy levels low?,no,",
  "2026-03-01 23:00:00,2026-03-02 07:00:00,UTC+05:30,Good mood?,,",
  "2026-03-01 23:00:00,2026-03-02 07:00:00,UTC+05:30,Screen time before bed?,true,",
  "2026-03-01 23:00:00,2026-03-02 07:00:00,UTC+05:30,Practice mediation or breathwork?,false,ten minutes",
  "2026-03-01 23:00:00,2026-03-02 07:00:00,UTC+05:30,Spend time stretching?,true,",
  "2026-03-01 23:00:00,2026-03-02 07:00:00,UTC+05:30,,true,",
  ",,UTC+05:30,Feel sore?,true,",
  "2026-03-03 23:00:00,2026-03-04 07:00:00,,Feel sore?,true,",
].join("\n");

const R1 = "2026-10-13T12:00:00.000Z";
const R2 = "2026-10-14T12:00:00.000Z";
const R3 = "2026-10-15T12:00:00.000Z";

mkdirSync(outDirectory, { recursive: true });

const files = fixtureFiles();
await scenario("cycles-fixture", "The committed fixture: a fresh import, an unchanged repeat, and one corrected value.", [
  { received_at: R1, input: { kind: "directory", files } },
  { received_at: R2, input: { kind: "directory", files } },
  { received_at: R3, input: { kind: "directory", files: corrected(files) } },
]);

const duplicated = { ...files, "physiological_cycles.csv": duplicateFirstRow(files["physiological_cycles.csv"]!) };
await scenario(
  "cycles-duplicate",
  "A repeated cycle row: numbered keys, an unchanged repeat, then an ambiguous correction refused as a conflict.",
  [
    { received_at: R1, input: { kind: "directory", files: duplicated } },
    { received_at: R2, input: { kind: "directory", files: duplicated } },
    { received_at: R3, input: { kind: "directory", files: corrected(duplicated) } },
  ],
);

await scenario("cycles-edges", "Invented rows that exercise offsets, skips, delimiters, quoting, and unknown files.", [
  {
    received_at: R1,
    input: {
      kind: "directory",
      files: {
        "physiological_cycles.csv": EDGE_CYCLES,
        "sleeps.csv": EDGE_SLEEPS,
        "workouts.csv": EDGE_WORKOUTS,
        "journal_entries.csv": EDGE_JOURNAL,
        "a_b.csv": "x\n",
        "a-b.csv": "x\n",
        "a1.csv": "x\n",
        "nested/Extra.CSV": "x\n",
        "notes.txt": "not a table\n",
      },
    },
  },
]);

await scenario("cycles-containers", "A folder import, then the same files as a zip, then single CSV files.", [
  { received_at: R1, input: { kind: "directory", files } },
  { received_at: R2, input: { kind: "zip", files: { "my_whoop_data/physiological_cycles.csv": files["physiological_cycles.csv"]!, "my_whoop_data/sleeps.csv": files["sleeps.csv"]!, "my_whoop_data/workouts.csv": files["workouts.csv"]!, "my_whoop_data/journal_entries.csv": files["journal_entries.csv"]! } } },
  { received_at: R3, input: { kind: "file", name: "physiological_cycles.csv", text: files["physiological_cycles.csv"]! } },
  { received_at: "2026-10-16T12:00:00.000Z", input: { kind: "file", name: "Cycles-Copy.csv", text: files["physiological_cycles.csv"]! } },
]);

await scenario("cycles-deleted", "A normally deleted record that a later import names is reported as a conflict.", [
  { received_at: R1, input: { kind: "directory", files } },
  {
    received_at: R2,
    input: { kind: "directory", files },
    delete_before: { label_prefix: "<point_measurement whoop_csv/v1 [\"point_measurement\",\"<source", deleted_at: "2026-10-13T13:00:00.000Z" },
  },
]);

// The order 2.1.0 lists skip reasons in.
const reasons = [
  "unrecognized WHOOP file: a_b.csv",
  "unrecognized WHOOP file: a-b.csv",
  "unrecognized WHOOP file: a1.csv",
  "unrecognized WHOOP file: A2.csv",
  "unrecognized WHOOP file: a 2.csv",
  "unrecognized WHOOP file: a.csv",
  "unrecognized WHOOP file: ab.csv",
  "unrecognized WHOOP file: aB.csv",
  "unrecognized WHOOP file: Ab.csv",
  "cycle rows with an unreadable timestamp",
  "Cycle rows with an unreadable timestamp",
  "sleep rows with an unusable time range",
  "HKQuantityTypeIdentifierHeartRate: unsupported unit count/s",
  "HKQuantityTypeIdentifierHeartRateVariabilitySDNN",
  "hk(x)",
  "hk[x]",
  "hk{x}",
  "hk@x",
  "hk*x",
  "hk/x",
  "hk\\x",
  "hk&x",
  "hk#x",
  "hk%x",
  "hk`x",
  "hk^x",
  "hk+x",
  "hk<x",
  "hk=x",
  "hk>x",
  "hk|x",
  "hk~x",
  "hk$x",
  "hk'x",
  "hk\"x",
  "hk!x",
  "hk?x",
  "hk.x",
  "hk;x",
  "hk:x",
  "hk,x",
  "hk\tx",
  "",
  "z",
  "Z",
  "0",
];
write("collation.json", {
  reference: "2.1.0",
  description: "Skip reasons sorted as 2.1.0 sorted them, with String.prototype.localeCompare.",
  input: reasons,
  sorted: [...reasons].sort((left, right) => left.localeCompare(right)),
});

const instantInputs = [
  "2026-08-09T06:20:00-07:00",
  "2026-08-09T06:20:00Z",
  "2026-08-09T06:20:00z",
  "2026-08-09t06:20:00Z",
  "2026-08-09T06:20-07:00",
  "2026-08-09T06:20:00.0005Z",
  "2026-08-09T06:20:00.9999Z",
  "2026-08-09T06:20:00.5+01:00",
  "2026-13-01T00:00:00-07:00",
  "2026-02-30T00:00:00Z",
  "2026-02-29T00:00:00Z",
  "2026-08-09T24:00:00Z",
  "2026-08-09T24:00:01Z",
  "2026-08-09T06:20:60Z",
  "2026-08-09T06:20:00+05:30",
  "2026-08-09T06:20:00-0700",
  "2026-08-09T06:20:00T-0700",
  "2026-08-09T06:20:00+ab:cd",
  "2026-08-09T06:20:00-07000",
  "2026-08-09T18:00:00.000-04:00",
  "2026-08-09 06:20:00Z",
  "2026-08-09 06:20:00 +0700",
  "2026-08-09T06:20:00 -07:00",
  "2026-08-09T06:20:00+24:00",
  "2026-08-09T06:20:00+23:59",
  "2026-08-09T6:20:00Z",
  "2026-08-09T06:20:00.Z",
  "2026-08-00T00:00:00Z",
  "2026-00-09T00:00:00Z",
  "2026-08-09T00:00:00-23:59",
  "2026-08-09Tab:cd:00Z",
  "2026/08/09T06:20:00Z",
  "1970-01-01T00:00:00Z",
  "1969-12-31T23:59:59.999Z",
];
write("instants.json", {
  reference: "2.1.0",
  description: "Provider timestamps read as Date.parse read them; null where it returned NaN.",
  cases: instantInputs.map((text) => {
    const millis = Date.parse(text);
    return { text, utc: Number.isFinite(millis) ? new Date(millis).toISOString() : null };
  }),
});

// Detection refusals, with the temporary directory written as <dir>.
async function detectionError(setup: (directory: string) => Promise<string> | string): Promise<string> {
  const directory = mkdtempSync(join(realTmp, "ath-detect-"));
  try {
    const path = await setup(directory);
    await detectExport(path);
    return "detected";
  } catch (error) {
    return (error as Error).message.split(directory).join("<dir>");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

async function zipWith(path: string, entries: Array<{ name: string; text: string }>): Promise<string> {
  const zip = new ZipFile();
  for (const entry of entries) zip.addBuffer(Buffer.from(entry.text), entry.name, { mtime: new Date(0) });
  const output = createWriteStream(path);
  zip.outputStream.pipe(output);
  zip.end();
  await once(output, "close");
  return path;
}

const detectionCases: Array<{ name: string; message: string }> = [];
const detection = async (name: string, setup: (directory: string) => Promise<string> | string) =>
  detectionCases.push({ name, message: await detectionError(setup) });
await detection("missing", (directory) => join(directory, "missing"));
await detection("empty_directory", (directory) => {
  mkdirSync(join(directory, "input"));
  return join(directory, "input");
});
await detection("privacy_portal_directory", (directory) => {
  mkdirSync(join(directory, "input", "health"), { recursive: true });
  return join(directory, "input");
});
await detection("unknown_csv", (directory) => {
  writeFileSync(join(directory, "table.csv"), "a,b\n1,2\n");
  return join(directory, "table.csv");
});
await detection("unsupported_file", (directory) => {
  writeFileSync(join(directory, "notes.txt"), "x");
  return join(directory, "notes.txt");
});
await detection("unknown_zip", (directory) => zipWith(join(directory, "data.zip"), [{ name: "a.csv", text: "a,b\n" }]));
await detection("privacy_portal_zip", (directory) =>
  zipWith(join(directory, "data.zip"), [{ name: "health/sleeps.csv", text: "a\n" }]),
);
await detection("symbolic_link", (directory) => {
  cpSync(fixture, join(directory, "real"), { recursive: true });
  symlinkSync(join(directory, "real"), join(directory, "link"));
  return join(directory, "link");
});
await detection("linked_csv_in_directory", (directory) => {
  mkdirSync(join(directory, "input"));
  writeFileSync(join(directory, "outside.csv"), "a\n");
  symlinkSync(join(directory, "outside.csv"), join(directory, "input", "a.csv"));
  return join(directory, "input");
});
write("detection.json", {
  reference: "2.1.0",
  description: "Provider export detection refusals. <dir> is the temporary directory.",
  cases: detectionCases,
});

// Keep the reviewer's view small: list what was written.
for (const name of readdirSync(outDirectory).sort()) {
  console.log(`${relative(root, join(outDirectory, name))} ${statSync(join(outDirectory, name)).size} bytes`);
}
