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
 *   ATH_REFERENCE_ROOT=<checkout> node --import <checkout>/node_modules/tsx/dist/esm/index.mjs tools/reference/route-golden.ts
 *
 * Output: tests/golden/routes/*.json. File names avoid provider names,
 * so the public-data check does not read them as health exports.
 */
import { execFile } from "node:child_process";
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
import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { promisify } from "node:util";

import { reference, repository as root } from "./reference.ts";

const outDirectory = join(root, "tests", "golden", "routes");
const exportsRoot = join(reference, "tests", "fixtures", "exports");
const mappingsRoot = join(reference, "mappings");
const testMappingsRoot = join(reference, "tests", "fixtures", "mappings");
const helper = join(reference, "crates", "ath-core", "target", "release", "ath-core");
const run = promisify(execFile);

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
    this.windows.push({ step, from, to });
  }

  /** The latest step whose window holds the time: a step can start in the millisecond the last one ended. */
  text(value: string): string {
    if (!ISO.test(value)) return value;
    const millis = Date.parse(value);
    const window = [...this.windows].reverse().find((candidate) => millis >= candidate.from && millis <= candidate.to);
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
          .map((deletion) => ({ record_revision: Number(deletion.record_revision), deleted_at: time(String(deletion.deleted_at)) })),
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


// ---- Keyed routes (Garmin, Strava, the Garmin pull, mapped imports) --------

interface KeyedStep {
  /** The helper command: `import-garmin`, `import-strava`, `import-mapped`, or `pull-garmin`. */
  command: "import-garmin" | "import-strava" | "import-mapped" | "pull-garmin";
  input?: Input;
  /** A shipped mapping name, or a mapping file's text under `text`. */
  mapping?: { shipped: string } | { test: string } | { text: string };
  /** The day a pull reads, and what the local server answers. */
  pull?: { date: string; activities: unknown; hrv: unknown | null };
  delete_before?: { label_prefix: string; deleted_at: string };
}

async function helperRun(args: string[], env: NodeJS.ProcessEnv = {}): Promise<{ result?: unknown; error?: string }> {
  try {
    const { stdout } = await run(helper, args, { env: { ...process.env, ...env }, maxBuffer: 64 * 1024 * 1024 });
    return { result: JSON.parse(stdout) };
  } catch (error) {
    const failed = error as { stderr?: string; message: string };
    return { error: (failed.stderr ?? failed.message).trim() };
  }
}

/** A local server that answers the Garmin personal connection's three requests. */
async function personalServer(day: string, activities: unknown, hrv: unknown | null): Promise<{ server: Server; base: string }> {
  const server = createServer((request, response) => {
    const path = request.url ?? "";
    if (request.method === "POST" && path.endsWith("/personal-api-key/token")) {
      response.end("aaa.bbb.ccc");
      return;
    }
    if (path.startsWith("/activitylist-service/activities/search/activities")) {
      response.setHeader("content-type", "application/json");
      response.end(JSON.stringify(activities));
      return;
    }
    if (path === `/hrv-service/hrv/${day}` && hrv !== null) {
      response.setHeader("content-type", "application/json");
      response.end(JSON.stringify(hrv));
      return;
    }
    response.statusCode = 404;
    response.end("not found");
  });
  await new Promise<void>((done) => server.listen(0, "127.0.0.1", () => done()));
  return { server, base: `http://127.0.0.1:${(server.address() as AddressInfo).port}` };
}

const PERSONAL_KEY = "invented-personal-key";

async function keyedScenario(name: string, description: string, steps: KeyedStep[]): Promise<void> {
  const directory = mkdtempSync(join(realTmp, "ath-golden-"));
  const databasePath = join(directory, "athlete.ath.sqlite");
  const database = await db.createDatabase(databasePath, { createdAt: CREATED_AT });
  const labels = new Labels();
  const clock = new Clock();
  const out: unknown[] = [];
  try {
    for (const [index, step] of steps.entries()) {
      const stepDirectory = join(directory, `step-${index}`);
      mkdirSync(stepDirectory);
      // The helper's clock is now, so a deletion happens now too, inside the step's window.
      const from = Date.now();
      if (step.delete_before !== undefined) {
        await deleteByLabel(database, labels, step.delete_before.label_prefix, new Date().toISOString());
      }
      let preview: { result?: unknown; error?: string } = {};
      let written: { result?: unknown; error?: string };
      let mappingText: string | undefined;
      if (step.command === "pull-garmin") {
        const pull = step.pull!;
        const { server, base } = await personalServer(pull.date, pull.activities, pull.hrv);
        try {
          written = await helperRun(
            ["pull-garmin", "--database", databasePath, "--date", pull.date, "--base-url", base],
            { ATH_CORE_GARMIN_KEY: PERSONAL_KEY },
          );
        } finally {
          server.close();
        }
      } else {
        const path = await materialize(step.input!, stepDirectory);
        const args = [step.command, "--database", databasePath];
        if (step.command === "import-mapped") {
          const mapping = step.mapping!;
          mappingText =
            "shipped" in mapping
              ? readFileSync(join(mappingsRoot, `${mapping.shipped}.md`), "utf8")
              : "test" in mapping
                ? readFileSync(join(testMappingsRoot, `${mapping.test}.md`), "utf8")
                : mapping.text;
          const mappingFile = join(stepDirectory, "mapping.md");
          writeFileSync(mappingFile, mappingText);
          args.push("--mapping-file", mappingFile);
        }
        args.push("--input", path);
        preview = await helperRun([...args, "--preview"]);
        written = await helperRun(args);
      }
      clock.record(index, from, Date.now());
      const database_ = await snapshot(database, labels, { parsed: true, clock });
      out.push({
        command: step.command,
        input: step.input ?? null,
        mapping: step.mapping === undefined ? null : { ...step.mapping, ...(mappingText === undefined ? {} : { text: mappingText }) },
        pull: step.pull ?? null,
        delete_before: step.delete_before ?? null,
        preview: preview.error !== undefined ? { error: preview.error } : preview.result === undefined ? null : { result: labels.value(preview.result) },
        write: written.error !== undefined ? { error: labels.text(written.error) } : { result: labels.value(written.result) },
        database: database_,
      });
    }
  } finally {
    await db.releaseLocalDatabase(database);
    rmSync(directory, { recursive: true, force: true });
  }
  write(`${name}.json`, { reference: "2.1.0", description, steps: out });
}

/** A FIT file with a header, the given data bytes, and a two-byte checksum. */
function fitBytes(data: number[]): string {
  const header = [12, 0x10, 0, 0, 0, 0, 0, 0, 0x2e, 0x46, 0x49, 0x54];
  const size = data.length;
  header[4] = size & 0xff;
  header[5] = (size >> 8) & 0xff;
  return `base64:${Buffer.from([...header, ...data, 0, 0]).toString("base64")}`;
}

const R = (day: number) => `2026-10-${String(13 + day).padStart(2, "0")}T12:00:00.000Z`;

async function keyedScenarios(): Promise<void> {
  const garmin = fixtureFiles(join(exportsRoot, "garmin"));
  const parkLoop = garmin["park-loop.json"]!;
  const changedLoop = parkLoop.replace('"description": "easy jog",\n', "").replace('"Feeling": "smooth"', '"Feeling": "steady"');
  const activityList = JSON.stringify({
    summarizedActivities: [
      { activityId: 2001, activityName: "Hill repeats", startTimeGMT: "2026-01-05T15:00:00Z", elapsedDuration: 600, activityType: { typeKey: "running" }, maxHR: 0, steps: 900 },
      { activity_id: "2002", name: "Row", start_at: "2026-01-06T15:00:00.000+02:00", duration: 61.2345, activityType: { type: "rowing" }, heart_rate_samples: [{ offset_ms: 0, bpm: 90 }], route: [{ recorded_at: "2026-01-06T13:00:00.000Z", latitude_deg: 1, longitude_deg: 2 }] },
      "not an activity",
    ],
  });
  // A folder is read in byte order by the native build and in the system's
  // order by 2.1.0, so scenarios with several files use a zip archive, whose
  // order both read the same way.
  const garminFiles = { ...garmin, "list/activities.json": activityList, "notes.txt": "hello\n" };
  await keyedScenario("watch-files", "Garmin files in a zip archive: a fresh import, an unchanged repeat, and a changed activity.", [
    { command: "import-garmin", input: { kind: "zip", files: garminFiles } },
    { command: "import-garmin", input: { kind: "zip", files: garminFiles } },
    { command: "import-garmin", input: { kind: "zip", files: { ...garminFiles, "park-loop.json": changedLoop } } },
  ]);
  await keyedScenario("watch-folder", "One Garmin activity file inside a nested folder.", [
    { command: "import-garmin", input: { kind: "directory", files: { "nested/deeper/park-loop.json": parkLoop } } },
  ]);

  await keyedScenario("watch-containers", "Each Garmin file alone, then the files in a zip archive.", [
    { command: "import-garmin", input: { kind: "file", name: "park-loop.json", text: parkLoop } },
    { command: "import-garmin", input: { kind: "file", name: "namespaced-hr.gpx", text: garmin["namespaced-hr.gpx"]! } },
    { command: "import-garmin", input: { kind: "file", name: "synthetic-position.fit", text: garmin["synthetic-position.fit"]! } },
    { command: "import-garmin", input: { kind: "zip", files: { "garmin/park-loop.json": parkLoop, "garmin/namespaced-hr.gpx": garmin["namespaced-hr.gpx"]! } } },
  ]);

  const definitionOnly = fitBytes([0x40, 0, 0, 20, 0, 1, 253, 4, 0x86]);
  const compressed = fitBytes([0x80, 0, 0, 0]);
  await keyedScenario("watch-edges", "Invented Garmin files that are left out or refuse the import.", [
    {
      command: "import-garmin",
      input: {
        kind: "zip",
        files: {
          "a-empty.fit": definitionOnly,
          "b-compressed.fit": compressed,
          "c-short.fit": "base64:YWJj",
          "d-loop.gpx": '<gpx><trk><name>Two segments</name><trkseg><trkpt lat="40.0" lon="-105.0"><time>2026-08-09T15:00:00.123456-04:00</time><hr>91.5</hr></trkpt><trkpt lat="40.1" lon="-105.1"><time></time></trkpt><trkpt lat="x" lon="-105.1"><time>2026-08-09T15:00:01Z</time></trkpt><trkpt lat="40.2" lon="-105.2"><time>2026-08-09T19:00:20Z</time></trkpt></trkseg></trk></gpx>',
          "e-object.json": JSON.stringify({ activityId: 3001, startTimeGMT: "2026-02-01T10:00:00.000Z", duration: "60", elapsedDuration: 90, description: 7, nested: { a: [1, 2] } }),
          "f-array.json": JSON.stringify([{ activityId: 3002.5, startTimeGMT: "2026-02-02T10:00:00.000Z", activityName: "Twice", maxHR: 171.5 }]),
        },
      },
    },
  ]);
  await keyedScenario("watch-refusals", "Garmin inputs that refuse the import, each against a fresh database.", [
    { command: "import-garmin", input: { kind: "file", name: "plain-times.gpx", text: '<gpx><trk><trkseg><trkpt lat="40.0" lon="-105.0"><time>2026-08-09T15:00:00</time></trkpt></trkseg></trk></gpx>' } },
  ]);
  await keyedScenario("watch-refusals-2", "A track with no timed point refuses the import.", [
    { command: "import-garmin", input: { kind: "file", name: "untimed.gpx", text: '<gpx><trk><trkseg><trkpt lat="40.0" lon="-105.0"></trkpt></trkseg></trk></gpx>' } },
  ]);
  await keyedScenario("watch-refusals-3", "An activity with a local start time refuses the import.", [
    { command: "import-garmin", input: { kind: "file", name: "local.json", text: JSON.stringify({ activityName: "Local", startTimeGMT: "2026-01-02 15:00:00" }) } },
  ]);
  await keyedScenario("watch-refusals-4", "A latitude past 90 refuses the import.", [
    { command: "import-garmin", input: { kind: "file", name: "far.json", text: JSON.stringify({ activityId: 9, startTimeGMT: "2026-01-02T15:00:00Z", route: [{ recorded_at: "2026-01-02T15:00:00.000Z", latitude_deg: 179.99999991618097, longitude_deg: 179.99999991618097 }] }) } },
  ]);
  await keyedScenario("watch-refusals-5", "A supplied field without a name refuses the import.", [
    { command: "import-garmin", input: { kind: "file", name: "blank.json", text: JSON.stringify({ activityId: "b-1", startTimeGMT: "2026-01-02T15:00:00Z", "": 1 }) } },
  ]);
  await keyedScenario("watch-deleted", "A deleted Garmin workout named by a later import.", [
    { command: "import-garmin", input: { kind: "file", name: "park-loop.json", text: parkLoop } },
    {
      command: "import-garmin",
      input: { kind: "file", name: "park-loop.json", text: parkLoop },
      delete_before: { label_prefix: "<workout_session garmin_account/v1 1001>", deleted_at: "2026-10-13T13:00:00.000Z" },
    },
  ]);

  const strava = fixtureFiles(join(exportsRoot, "strava"));
  const stravaRows = [
    "Activity ID,Activity Date,Activity Name,Activity Type,Elapsed Time,Filename,Gear,Activity Description,,Commute",
    "2002,2026-01-03T16:00:00.000Z,River Path,Ride,2,activities/river.gpx,Blue bike,\"Easy, flat\",x,false",
    ",\"Jan 4, 2026, 4:05:06 PM\",Lunch Walk,Walk,90.5,activities/missing.gpx,,,,",
    "2004,2026-01-05T07:00:00+01:00,Swim,Swim,abc,activities/swim.fit.gz,,,,true",
    ",,,,,,,,,",
  ].join("\r\n");
  await keyedScenario("bulk-folder", "A Strava export folder: a fresh import, an unchanged repeat, and changed gear.", [
    { command: "import-strava", input: { kind: "directory", files: { ...strava, "activities.csv": stravaRows, "activities/swim.fit.gz": "base64:AAAA", "profile.csv": "a\n" } } },
    { command: "import-strava", input: { kind: "directory", files: { ...strava, "activities.csv": stravaRows, "activities/swim.fit.gz": "base64:AAAA", "profile.csv": "a\n" } } },
    { command: "import-strava", input: { kind: "zip", files: { ...strava, "activities.csv": stravaRows.replace("Blue bike", "Red bike") } } },
  ]);
  await keyedScenario("bulk-fixture", "The committed Strava fixture.", [
    { command: "import-strava", input: { kind: "directory", files: strava } },
  ]);
  await keyedScenario("bulk-refusals", "A Strava export without activities.csv.", [
    { command: "import-strava", input: { kind: "directory", files: { "activities/river.gpx": strava["activities/river.gpx"]!, "my_activities.csv": "x\n" } } },
  ]);
  await keyedScenario("bulk-refusals-2", "A Strava export with an empty activities.csv.", [
    { command: "import-strava", input: { kind: "directory", files: { "activities.csv": "\n\n" } } },
  ]);

  const personalActivities = [
    { activityId: 7001, activityName: "Invented personal loop", startTimeGMT: "2026-08-09T15:00:00.000Z", duration: 600, activityType: { typeKey: "running" }, Feeling: "steady" },
    { activityId: "7002", name: "Second", start_at: "2026-08-09T18:00:00.000Z" },
    { summaryOnly: true, distance: 5 },
    7,
  ];
  await keyedScenario("connection-day", "The Garmin personal connection: a day, the same day again, then a changed name and HRV.", [
    { command: "pull-garmin", pull: { date: "2026-08-09", activities: personalActivities, hrv: { LastNightAvg: 41, weeklyAvg: 44, status: "BALANCED" } } },
    { command: "pull-garmin", pull: { date: "2026-08-09", activities: personalActivities, hrv: { LastNightAvg: 41, weeklyAvg: 44, status: "BALANCED" } } },
    {
      command: "pull-garmin",
      pull: {
        date: "2026-08-09",
        activities: [{ ...personalActivities[0] as object, activityName: "Renamed loop" }],
        hrv: { LastNightAvg: 43, weeklyAvg: 44 },
      },
    },
    { command: "pull-garmin", pull: { date: "2026-08-10", activities: { activityId: 7003, startTimeGMT: "2026-08-10T15:00:00.000Z" }, hrv: { status: "NONE" } } },
    { command: "pull-garmin", pull: { date: "2026-08-11", activities: [], hrv: null } },
  ]);

  const ow = fixtureFiles(join(exportsRoot, "open-wearables"));
  const owSummaries = ow["summaries.json"]!;
  const owChanged = JSON.parse(owSummaries) as { sleep: Array<Record<string, unknown>> };
  owChanged.sleep[0]!.end_time = "2026-08-09T10:05:00.000-04:00";
  const mapped = (file: string, text: string, mapping: KeyedStep["mapping"]): KeyedStep => ({
    command: "import-mapped",
    input: { kind: "file", name: file, text },
    mapping,
  });
  await keyedScenario("aggregator-summaries", "The shipped aggregator mapping: a fresh import, an unchanged repeat, and a changed sleep.", [
    mapped("summaries.json", owSummaries, { shipped: "open-wearables" }),
    mapped("summaries.json", owSummaries, { shipped: "open-wearables" }),
    mapped("summaries.json", JSON.stringify(owChanged), { shipped: "open-wearables" }),
  ]);
  await keyedScenario("aggregator-pages", "The shipped aggregator mapping on each page fixture.", [
    mapped("page-raw.json", ow["page-raw.json"]!, { shipped: "open-wearables" }),
    mapped("page-5min.json", ow["page-5min.json"]!, { shipped: "open-wearables" }),
    mapped("page-empty.json", ow["page-empty.json"]!, { shipped: "open-wearables" }),
    mapped("page-null-resolution.json", ow["page-null-resolution.json"]!, { shipped: "open-wearables" }),
    mapped("page-workouts.json", ow["page-workouts.json"]!, { shipped: "open-wearables" }),
    mapped("page-sleep.json", ow["page-sleep.json"]!, { shipped: "open-wearables" }),
  ]);
  await keyedScenario("aggregator-deleted-source", "A deleted mapped source leaves its rows out.", [
    mapped("summaries.json", owSummaries, { shipped: "open-wearables" }),
    {
      ...mapped("summaries.json", owSummaries, { shipped: "open-wearables" }),
      delete_before: { label_prefix: "<source oura open_wearables ring|invented-ring|invented-app", deleted_at: "2026-10-13T13:00:00.000Z" },
    },
  ]);
  await keyedScenario("aggregator-deleted-record", "A deleted mapped record is left out.", [
    mapped("summaries.json", owSummaries, { shipped: "open-wearables" }),
    {
      ...mapped("summaries.json", owSummaries, { shipped: "open-wearables" }),
      delete_before: { label_prefix: "<sleep_session open_wearables/v1 oura:ring|invented-ring|invented-app:sleep:ow-sleep-1", deleted_at: "2026-10-13T13:00:00.000Z" },
    },
  ]);
  await keyedScenario("connector-summaries", "The shipped connector mapping, then the aggregator mapping into the same database.", [
    mapped("summaries.json", fixtureFiles(join(exportsRoot, "junction"))["summaries.json"]!, { shipped: "junction" }),
    mapped("summaries.json", owSummaries, { shipped: "open-wearables" }),
  ]);

  const SLEEP_HEADER = "name: invented\nroute: invented_route\nwriter_field: source.provider\nvalue_basis: provider\n\n[sleep]\nid_field: id\nstart_field: start_time\nend_field: end_time\n";
  const block = (body: string) => `# Invented\n\n\`\`\`ath-mapping\n${body}\n\`\`\`\n`;
  const night = (id: string, maker: string, day: string, extra: string) =>
    `{"id":"${id}","start_time":"2026-08-${day}T01:00:00.000Z","end_time":"2026-08-${day}T07:00:00.000Z","source":{"provider":"${maker}"},"asleep":20000${extra}}`;
  await keyedScenario("invented-mappings", "Invented mapping files: fallback order, a failure partway, and a page with its own fields.", [
    mapped("nights.json", `{"sleep":[${night("n-1", "invented-maker", "09", ',"asleep_total":21000,"rmssd_first":40,"rmssd_second":44')}]}`, {
      text: block(`${SLEEP_HEADER}rmssd_first: hrv_rmssd ms ms\nrmssd_second: hrv_rmssd ms ms\nasleep_total: asleep_seconds s s when-absent\nasleep: asleep_seconds s s`),
    }),
    mapped("nights.json", `{"sleep":[${night("n-1", "invented-maker", "09", ',"asleep_total":21000,"rmssd_first":40,"rmssd_second":44')}]}`, {
      text: block(`${SLEEP_HEADER}rmssd_first: hrv_rmssd ms ms\nrmssd_second: hrv_rmssd ms ms\nasleep_total: asleep_seconds s s when-absent\nasleep: asleep_seconds s s`),
    }),
    mapped(
      "second.json",
      `{"sleep":[${[night("n-1", "invented-maker", "09", ',"mood_word":"fine"'), night("next-night", "invented-maker", "10", ""), night("bad-night", "other-maker", "11", ',"":"no name"')].join(",")}]}`,
      { text: block(`${SLEEP_HEADER}asleep: asleep_seconds s s`) },
    ),
    mapped(
      "proto.json",
      `{"sleep":[${night("bad-night", "invented-maker", "10", ',"extra":{"__proto__":1}')}]}`,
      { text: block(`${SLEEP_HEADER}asleep: asleep_seconds s s`) },
    ),
    mapped(
      "raw.json",
      `{"meta":{"grain":"each"},"items":[{"at":"2026-08-09T06:00:00.000Z","kind":"rmssd_night","reading":41,"unit":"ms","maker":"invented-maker","span":60},{"at":"2026-08-09T06:01:00.000Z","kind":"rmssd_night","reading":41,"unit":"ms","maker":"invented-maker","until":"2026-08-09T07:00:00.000Z"},{"at":"2026-08-09T06:02:00.000Z","kind":"rmssd_night","reading":41,"unit":"ms","maker":"invented-maker","origin":"typed"},{"at":"2026-08-09T06:03:00.000+02:00","kind":"rmssd_night","reading":0,"unit":"ms","maker":"invented-maker"},{"at":"2026-08-09T06:04:00.000Z","kind":"other","reading":1,"unit":"ms","maker":"invented-maker"},{"at":"2026-08-09T06:05:00.000Z","kind":"rmssd_night","reading":"x","unit":"ms","maker":"invented-maker"},{"at":"2026-08-09T06:00:00.000Z","kind":"rmssd_night","reading":41,"unit":"ms","maker":"invented-maker"},{"at":"2026-08-09T06:06:00.000Z","kind":"rmssd_night","reading":42,"unit":"bpm","maker":"invented-maker"},{"kind":"rmssd_night","reading":42,"unit":"ms","maker":"invented-maker"},{"at":"2026-08-09T06:07:00.000Z","reading":42,"unit":"ms","maker":"invented-maker"},{"at":"2026-08-09T06:08:00.000Z","kind":"rmssd_night","reading":42,"unit":"ms"},7]}`,
      {
        text: block(
          [
            "name: samples",
            "route: samples_route",
            "writer_field: maker",
            "value_basis: provider_or_aggregator",
            "personal_when: origin typed",
            "page_rows: items",
            "page_marker: meta",
            "page_section: timeseries",
            "page_resolution: meta.grain",
            "raw_resolution: each",
            "",
            "[timeseries]",
            "time_field: at",
            "type_field: kind",
            "value_field: reading",
            "unit_field: unit",
            "aggregate_field: until",
            "window_field: span",
            "kind.rmssd_night: hrv_rmssd ms ms",
          ].join("\n"),
        ),
      },
    ),
    mapped(
      "sessions.json",
      JSON.stringify({
        workout: [
          { id: "w-1", start_time: "2026-08-09T10:00:00+02:00", end_time: "2026-08-09T11:00:00+02:00", source: { provider: "maker:a\\b" }, sport: "  ", label: "Intervals", distance_m: 1000.5, kcal: null, avg_hr: 140, note: "kept" },
          { id: "w-2", start_time: "2026-08-09T12:00:00Z", end_time: "2026-08-09T11:00:00Z", source: { provider: "maker" } },
          { id: "w-3", start_time: "not a time", end_time: "2026-08-09T11:00:00Z", source: { provider: "maker" } },
          { id: "w-4", start_time: "2026-08-09T12:00:00Z", source: { provider: "maker" } },
          { id: "w-5", end_time: "2026-08-09T12:00:00Z", start_time: "2026-08-09T11:00:00Z" },
          { start_time: "2026-08-09T11:00:00Z" },
          "nope",
        ],
        sleep: [{ id: "s-1", start_time: "2026-08-09T01:00:00Z", end_time: "2026-08-09T07:00:00Z", source: { provider: "maker" }, awake_s: 90, wakes: 2.5, temp_dev: -0.2, rhr: 0 }],
        extra: [{ a: 1 }],
        empty: [],
      }),
      {
        text: block(
          [
            "name: sessions",
            "route: sessions_route",
            "writer_field: source.provider",
            "value_basis: provider",
            "",
            "[workout]",
            "id_field: id",
            "start_field: start_time",
            "end_field: end_time",
            "sport: activity_type basis provider_label",
            "label: name",
            "distance_m: source_reported_summaries.distance m m",
            "kcal: source_reported_summaries.energy kcal kcal",
            "avg_hr: provider_summary.average_heart_rate bpm bpm",
            "",
            "[sleep]",
            "id_field: id",
            "start_field: start_time",
            "end_field: end_time",
            "awake_s: stage_minutes.awake s min",
            "wakes: interruptions count count",
            "temp_dev: temperature_deviation celsius °C",
            "rhr: resting_heart_rate bpm bpm",
          ].join("\n"),
        ),
      },
    ),
  ]);

  // Refusals that stop before anything is read or written.
  const refusalBodies: Array<[string, string]> = [
    ["report-section", "name: r\nroute: r_route\nwriter_field: source.provider\n[athlete_report]\nrating: rating score score"],
    ["report-field", `${SLEEP_HEADER}quality: rating score score`],
    ["no-statistic", `${SLEEP_HEADER}average_hrv: hrv_rmssd ms ms`],
    ["builtin-route", SLEEP_HEADER.replace("invented_route", "garmin_account_export") + "x: asleep_seconds s s"],
    ["builtin-method", SLEEP_HEADER.replace("invented_route", "garmin_account") + "x: asleep_seconds s s"],
    ["unreadable", `${SLEEP_HEADER}sleep_duration_seconds asleep_seconds`],
    ["unknown-field", `${SLEEP_HEADER}x: steps count count`],
    ["two-when-absent", `${SLEEP_HEADER}x: asleep_seconds s s when-absent\ny: asleep_seconds s s when-absent`],
    ["two-fallbacks", `${SLEEP_HEADER}x: asleep_seconds s s\nx: in_bed_seconds s s`],
    ["text-unit", "name: w\nroute: w_route\nwriter_field: source.provider\nvalue_basis: provider\n[workout]\nid_field: id\nstart_field: s\nend_field: e\nlabel: name s s"],
    ["no-unit", `${SLEEP_HEADER}x: asleep_seconds`],
    ["needs-scale", `${SLEEP_HEADER}x: provider_score.x score score`],
    ["page-section", `${SLEEP_HEADER.replace("value_basis: provider\n", "value_basis: provider\npage_rows: data\npage_marker: meta\npage_section: workout\n")}x: asleep_seconds s s`],
    ["sdnn-basis", `${SLEEP_HEADER}heart_rate_variability_sdnn: hrv_sdnn ms ms basis hrv_mean_rmssd`],
  ];
  for (const [index, [label, body]] of refusalBodies.entries()) {
    await keyedScenario(`invented-refusal-${String(index + 1).padStart(2, "0")}`, `A mapping file refused before anything is read: ${label}.`, [
      mapped("nights.json", `{"sleep":[${night("n-1", "invented-maker", "09", "")}]}`, { text: block(body) }),
    ]);
  }
  for (const name of ["last-night-rmssd", "last-night-sdnn", "efficiency", "athlete-maximum"]) {
    await keyedScenario(`invented-refusal-${name}`, `A test mapping file refused before anything is read: ${name}.`, [
      mapped("summaries.json", owSummaries, { test: name }),
    ]);
  }
  await keyedScenario("invented-refusal-personal-input", "An input row that is a personal report.", [
    mapped("report.json", JSON.stringify({ sleep: [{ id: "report-1", report_type: "sleep_quality", reported_via: "text", rating: 2 }] }), { shipped: "open-wearables" }),
  ]);
  await keyedScenario("invented-refusal-not-json", "An input that is not JSON, then one that is not an object.", [
    mapped("broken.json", "{", { shipped: "open-wearables" }),
    mapped("list.json", "[]", { shipped: "open-wearables" }),
  ]);
}

// ---- Output -----------------------------------------------------------------

mkdirSync(outDirectory, { recursive: true });
await providerScenarios();
await keyedScenarios();

for (const name of readdirSync(outDirectory).sort()) {
  console.log(`${relative(root, join(outDirectory, name))} ${statSync(join(outDirectory, name)).size} bytes`);
}
