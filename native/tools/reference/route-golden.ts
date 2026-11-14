/**
 * Freeze the 2.1.0 results of the Oura, Garmin, Strava, Garmin pull, and mapped
 * import routes, which the native import must reproduce.
 *
 * This is the sibling of `import-golden.ts`, which covers WHOOP and shared
 * import rules. It records, for each step of each scenario, the output the
 * 2.1.0 command printed and the complete database afterwards: every record's
 * JSON for every revision, revision metadata, and import keys.
 *
 * Oura ran through `src/db/provider-import.ts`, so its scenarios call that
 * library exactly as `import-golden.ts` does, with fixed import times.
 *
 * Garmin, Strava, the Garmin pull, and mapped imports ran in the Rust helper
 * `ath-core`, which reads the clock and writes record JSON with Rust's
 * formatting. For those routes:
 *
 * - Every time the helper wrote during a step is replaced by `<step N>`. The
 *   script notes the clock before and after each step, and a timestamp inside
 *   that window is the helper's clock. Source data times are years away.
 * - Each stored record JSON is recorded parsed, with keys sorted, because the
 *   native store writes every record through one encoder in the published
 *   field order.
 * - Sources have no import key on these routes, so a source is labelled by its
 *   writer, route, and sensor.
 *
 * Record IDs are replaced by labels built from each record's type and match
 * key, and the tool version in `changed_by` by `<ath version>`.
 *
 * Run from the repository root with the sandbox allowing child processes and a
 * local HTTP server. ATH_REFERENCE_ROOT names a checkout of the reference
 * commit with its dependencies installed and its Rust helper built in release
 * mode:
 *
 *   ATH_REFERENCE_ROOT=/path/to/reference node --import tsx native/tools/reference/route-golden.ts
 *
 * Output: native/tests/golden/routes/*.json. File names avoid provider names,
 * so the public-data check does not read them as health exports.
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

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const reference = resolve(process.env.ATH_REFERENCE_ROOT ?? root);
const outDirectory = join(root, "native", "tests", "golden", "routes");
const exportsRoot = join(reference, "tests", "fixtures", "exports");

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
const { ZipFile } = createRequire(join(reference, "package.json"))("yazl") as {
  ZipFile: new () => {
    addBuffer(bytes: Buffer, name: string, options?: { mtime?: Date; mode?: number }): void;
    outputStream: NodeJS.ReadableStream;
    end(): void;
  };
};

const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g;
const ISO = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;
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

  /** A source with no import key is named by its writer, route, and sensor. */
  name(recordId: string, label: string): void {
    if (!this.labels.has(recordId)) this.labels.set(recordId, label);
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

/** Replaces each time the helper's clock wrote with the step that wrote it. */
class Clock {
  private windows: Array<{ step: number; from: number; to: number }> = [];

  record(step: number, from: number, to: number): void {
    this.windows.push({ step, from: from - 1, to: to + 1 });
  }

  text(value: string): string {
    if (!ISO.test(value)) return value;
    const millis = Date.parse(value);
    const window = this.windows.find((candidate) => millis >= candidate.from && millis <= candidate.to);
    return window === undefined ? value : `<step ${window.step}>`;
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

interface SnapshotOptions {
  /** Record JSON as parsed, sorted JSON instead of the stored text. */
  parsed?: boolean;
  clock?: Clock;
}

async function snapshot(database: Database, labels: Labels, options: SnapshotOptions = {}): Promise<unknown> {
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
  for (const row of (await client.execute("SELECT record_id, data_writer, data_route, sensor_type FROM sources")).rows) {
    labels.name(
      String(row.record_id),
      `source ${String(row.data_writer)} ${String(row.data_route)} ${row.sensor_type === null ? "" : String(row.sensor_type)}`,
    );
  }
  const time = (value: string) => (options.clock === undefined ? value : options.clock.text(value));
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
        created_at: time(String(row.created_at)),
        current_revision: Number(row.current_revision),
        is_deleted: Number(row.is_deleted) === 1,
        revisions: revisions
          .filter((revisionRow) => String(revisionRow.record_id) === id)
          .map((revisionRow) => {
            const changedBy = JSON.parse(String(revisionRow.changed_by_json)) as Record<string, unknown>;
            if (changedBy.author_type === "tool") changedBy.tool_version = "<ath version>";
            const text = labels.text(String(revisionRow.record_json));
            return {
              revision: Number(revisionRow.revision),
              previous_revision: revisionRow.previous_revision === null ? null : Number(revisionRow.previous_revision),
              became_known_at: time(String(revisionRow.became_known_at)),
              became_known_at_basis: String(revisionRow.became_known_at_basis),
              changed_by: changedBy,
              ...(options.parsed === true
                ? { record: canonical(options.clock === undefined ? JSON.parse(text) : options.clock.value(JSON.parse(text))) }
                : { record_json: text }),
              extensions_json: revisionRow.extensions_json === null ? null : String(revisionRow.extensions_json),
            };
          }),
        import_keys: keyRows
          .filter((keyRow) => String(keyRow.record_id) === id)
          .map((keyRow) => ({
            method: String(keyRow.match_key_method),
            key: labels.text(String(keyRow.match_key)),
            imported_at: time(String(keyRow.imported_at)),
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

/**
 * How a step's input is laid out. The native test builds the same layout.
 * A file's text is UTF-8, except a `base64:` text, which holds binary bytes.
 */
type Input =
  | { kind: "directory"; files: Record<string, string> }
  | { kind: "zip"; files: Record<string, string> }
  | { kind: "file"; name: string; text: string };

function bytesOf(text: string): Buffer {
  return text.startsWith("base64:") ? Buffer.from(text.slice("base64:".length), "base64") : Buffer.from(text);
}

function fixtureText(path: string): string {
  const bytes = readFileSync(path);
  return path.endsWith(".fit") ? `base64:${bytes.toString("base64")}` : bytes.toString("utf8");
}

/** Every file under a fixture directory, by relative path, sorted. */
function fixtureFiles(directory: string): Record<string, string> {
  const out: Record<string, string> = {};
  const walk = (current: string) => {
    for (const name of readdirSync(current).sort()) {
      const path = join(current, name);
      if (statSync(path).isDirectory()) walk(path);
      else out[relative(directory, path).split("\\").join("/")] = fixtureText(path);
    }
  };
  walk(directory);
  return out;
}

async function materialize(input: Input, directory: string): Promise<string> {
  if (input.kind === "file") {
    const path = join(directory, input.name);
    writeFileSync(path, bytesOf(input.text));
    return path;
  }
  if (input.kind === "directory") {
    const path = join(directory, "input");
    mkdirSync(path, { recursive: true });
    for (const [name, text] of Object.entries(input.files)) {
      mkdirSync(dirname(join(path, name)), { recursive: true });
      writeFileSync(join(path, name), bytesOf(text));
    }
    return path;
  }
  // Entries go in sorted order, the order the golden file lists them, so the
  // native test can build the same archive.
  const path = join(directory, "input.zip");
  const zip = new ZipFile();
  for (const name of Object.keys(input.files).sort(compareText)) {
    zip.addBuffer(bytesOf(input.files[name]!), name, { mtime: new Date(0) });
  }
  const output = createWriteStream(path);
  zip.outputStream.pipe(output);
  zip.end();
  await once(output, "close");
  return path;
}

async function deleteByLabel(database: Database, labels: Labels, prefix: string, deletedAt: string): Promise<void> {
  const before = (await snapshot(database, labels)) as { records: Array<{ label: string; current_revision: number }> };
  const target = before.records.find((record) => record.label.startsWith(prefix));
  if (target === undefined) throw new Error(`no record to delete: ${prefix}`);
  const rows = (await databaseClient(database).execute("SELECT record_id FROM records")).rows;
  const id = rows.map((row) => String(row.record_id)).find((recordId) => labels.text(recordId) === target.label);
  await db.deleteRecord(database, {
    record_id: id,
    expected_revision: target.current_revision,
    deleted_by: { author_type: "athlete" },
    deleted_at: deletedAt,
  });
}

// ---- Provider routes (Oura) ------------------------------------------------

interface ProviderStep {
  received_at: string;
  input: Input;
  /** Delete the record with this label at this time before the step. */
  delete_before?: { label_prefix: string; deleted_at: string };
}

async function providerScenario(name: string, description: string, steps: ProviderStep[]): Promise<void> {
  const directory = mkdtempSync(join(realTmp, "ath-golden-"));
  const database = await db.createDatabase(join(directory, "athlete.ath.sqlite"), { createdAt: CREATED_AT });
  const labels = new Labels();
  const out: unknown[] = [];
  try {
    for (const [index, step] of steps.entries()) {
      const stepDirectory = join(directory, `step-${index}`);
      mkdirSync(stepDirectory);
      if (step.delete_before !== undefined) {
        await deleteByLabel(database, labels, step.delete_before.label_prefix, step.delete_before.deleted_at);
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
          preview_error: (error as Error).message,
          database: await snapshot(database, labels),
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

function duplicateFirstRow(text: string): string {
  const [header, first, ...rest] = text.trimEnd().split(/\r?\n/);
  return [header, first, first, ...rest].join("\n") + "\n";
}

const R1 = "2026-10-13T12:00:00.000Z";
const R2 = "2026-10-14T12:00:00.000Z";
const R3 = "2026-10-15T12:00:00.000Z";

async function providerScenarios(): Promise<void> {
  const ring = fixtureFiles(join(exportsRoot, "oura"));
  const correctedRing = {
    ...ring,
    "oura_daily_readiness.csv": ring["oura_daily_readiness.csv"]!.replace("2026-08-09,74,", "2026-08-09,75,"),
  };
  await providerScenario("ring-fixture", "The committed ring fixture: a fresh import, an unchanged repeat, and one corrected score.", [
    { received_at: R1, input: { kind: "directory", files: ring } },
    { received_at: R2, input: { kind: "directory", files: ring } },
    { received_at: R3, input: { kind: "directory", files: correctedRing } },
  ]);

  const duplicated = {
    ...ring,
    "oura_daily_readiness.csv": duplicateFirstRow(ring["oura_daily_readiness.csv"]!),
  };
  await providerScenario(
    "ring-duplicate",
    "A repeated readiness row: numbered keys, an unchanged repeat, then an ambiguous correction refused as a conflict.",
    [
      { received_at: R1, input: { kind: "directory", files: duplicated } },
      { received_at: R2, input: { kind: "directory", files: duplicated } },
      {
        received_at: R3,
        input: {
          kind: "directory",
          files: { ...duplicated, "oura_daily_readiness.csv": duplicated["oura_daily_readiness.csv"]!.replace("2026-08-09,74,", "2026-08-09,75,") },
        },
      },
    ],
  );

  await providerScenario("ring-tables", "The other committed ring tables, each in a new database step by step.", [
    { received_at: R1, input: { kind: "directory", files: fixtureFiles(join(exportsRoot, "oura-member-portal")) } },
    { received_at: R2, input: { kind: "directory", files: fixtureFiles(join(exportsRoot, "oura-portal")) } },
    { received_at: R3, input: { kind: "directory", files: fixtureFiles(join(exportsRoot, "oura-spo2")) } },
    {
      received_at: "2026-10-16T12:00:00.000Z",
      input: { kind: "file", name: "sleep.csv", text: readFileSync(join(exportsRoot, "oura-offset", "sleep.csv"), "utf8") },
    },
  ]);

  await providerScenario("ring-containers", "The ring fixture as a zip, then one table as a single file.", [
    { received_at: R1, input: { kind: "zip", files: { "export/oura_sleep.csv": ring["oura_sleep.csv"]!, "export/oura_daily_readiness.csv": ring["oura_daily_readiness.csv"]! } } },
    { received_at: R2, input: { kind: "file", name: "oura_sleep.csv", text: ring["oura_sleep.csv"]! } },
  ]);

  await providerScenario("ring-edges", "Invented rows that exercise placement, skips, contributors, and unread columns.", [
    {
      received_at: R1,
      input: {
        kind: "directory",
        files: {
          "a_sleep.csv": [
            "day,bedtime_start,bedtime_end,average_hrv,lowest_heart_rate,average_breath,spo2_percentage,vo2_max,temperature_deviation,total_sleep_duration,time_in_bed,efficiency,breathing_disturbance_index,cardiovascular_age",
            "2026-03-01,2026-02-28T23:00:00+01:00,2026-03-01T07:00:00+01:00,55.555,48,14.25,\"{\"\"average\"\": 0}\",41,-0.135,25000.4,28000,101,3,40",
            "2026-03-02,2026-03-01T23:00:00,2026-03-02T07:00:00,50,49,,97.5,,,,,,,",
            "2026-03-03,2026-03-02T22:00:00Z,2026-03-03T00:00:00Z,52,,,,,0,,,,,",
            "2026-03-04,2026-03-04T06:00:00.000z,2026-03-04T06:00:00.000z,53,,,,,,,,,,",
            "2026-03-05,2026-03-04T23:00:00-04:00,2026-03-05T06:30:00-04:00,0,-1,abc,\"{\"\"score\"\": 96}\",,,,,,,",
            ",2026-03-05T23:00:00Z,2026-03-06T07:00:00.000Z,54,,,,,,600,,,,",
            ",,,56,,,,,,,,,,",
          ].join("\n"),
          "b_readiness.csv": [
            "summary_date;score;contributors;contributor_sleep_regularity;steps;met;readiness_score",
            "2026-03-01;70;{\"activity_balance\":90,\"hrv_balance\":\"x\",\"mystery\":5};61;1200;{\"items\":[1]};71",
            "2026-03-02;72;{};;;;",
            "2026-03-03;73;{bad;;;;",
            "2026-03-04;;{\"previous_night\":44};;3;1.5;",
          ].join("\n"),
          "c_empty.csv": "",
          "d_header.csv": "day,score\n",
          "e_dates.csv": "date,activity_score,sleep_score\n2026-03-07,60,61\n",
        },
      },
    },
    { received_at: R2, input: { kind: "file", name: "ring_sleep.csv", text: "day,average_hrv\n2026-08-09,63\n" } },
  ]);
}

// ---- Output -----------------------------------------------------------------

mkdirSync(outDirectory, { recursive: true });
await providerScenarios();

for (const name of readdirSync(outDirectory).sort()) {
  console.log(`${relative(root, join(outDirectory, name))} ${statSync(join(outDirectory, name)).size} bytes`);
}
