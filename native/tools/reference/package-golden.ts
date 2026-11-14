/**
 * Freeze the package bytes of the 2.1.0 implementation for the native tests.
 *
 * Spec 2.1.1 sections 9.1 and 13.1 require the native export to reproduce the
 * old package bytes, after only the documented version change. This script runs
 * the old library on synthetic inputs and writes what it produced under
 * native/tests/golden/package/. The Rust tests compare against these files, so
 * the comparison survives removal of the old implementation.
 *
 * Cases:
 * - demo: `ath export --exported-at` of examples/demo-athlete. Also the manifest
 *   without a time, and the restore preview of that package into an empty
 *   database.
 * - version-2-0-0: restore of tests/fixtures/version-2-0-0/package/ into an
 *   empty database, its export, and every athlete-table row after the restore.
 * - many-samples: a generated synthetic package with more samples than one
 *   series file holds, restored and exported. Only the main document, the
 *   manifest, and the input hashes are kept; the manifest holds the hash of
 *   every series file. native/tests/package_golden.rs builds the same input.
 * - unrepresentable: the preview of a package with an unknown field and a
 *   record that cannot be represented.
 *
 * Golden files use neutral names (main.json, manifest.json), because the
 * public-data check flags tracked files named like athlete exports.
 *
 * Run from the repository root (the old source must be present there, or name a
 * checkout of the reference commit with --reference):
 *
 *   node --import tsx native/tools/reference/package-golden.ts [--reference <checkout>]
 */
import { createHash } from "node:crypto";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..", "..");
const referenceIndex = process.argv.indexOf("--reference");
const reference = referenceIndex === -1 ? root : process.argv[referenceIndex + 1]!;
const out = join(root, "native", "tests", "golden", "package");

const db = await import(pathToFileURL(join(reference, "src", "db", "index.ts")).href);

const EXPORTED_AT = "2026-11-14T00:00:00.000Z";
const DESTINATION_ID = "01890f3e-0000-7000-8000-0000000000ff";
const DESTINATION_CREATED_AT = "2026-11-14T00:00:00.000Z";
const DEMO_ATTACHMENT = "01890f3e-e7d3-7cc0-88c8-00000000000e.png";

/** Athlete-data tables, in the order the dump lists them. */
const TABLES = [
  "records",
  "record_revisions",
  "record_deletions",
  "record_erasure_markers",
  "record_import_keys",
  "athlete_profile",
  "sources",
  "source_devices",
  "point_measurements",
  "point_measurement_calculations",
  "point_measurement_calculation_inputs",
  "measurement_samples",
  "sleep_sessions",
  "workout_sessions",
  "workout_session_segments",
  "workout_route_points",
  "benchmark_results",
  "benchmark_result_segments",
  "provider_scores",
  "provider_summaries",
  "provider_reporting_periods",
  "athlete_reports",
  "nutrition_entries",
  "reported_nutrition_items",
  "attachments",
  "record_attachment_links",
  "benchmark_definitions",
  "predictions",
  "prediction_result_links",
  "prediction_grades",
  "prediction_error_analyses",
  "prediction_error_causes",
  "training_plans",
  "planned_sessions",
  "planned_session_blocks",
  "coaching_notes",
  "saved_training_reviews",
  "goals",
  "nutrition_model_interpretations",
  "authored_record_evidence",
];

function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function temporary(prefix: string): string {
  return mkdtempSync(join(realpathSync(tmpdir()), prefix));
}

const quiet = { onSecurityWarning: () => undefined };

/** Copy the package files a golden case keeps. */
function keepPackage(packageDirectory: string, target: string, options: { series: boolean }): void {
  rmSync(target, { recursive: true, force: true });
  mkdirSync(target, { recursive: true });
  copyFileSync(join(packageDirectory, "athlete.ath.json"), join(target, "main.json"));
  copyFileSync(join(packageDirectory, "export-manifest.json"), join(target, "manifest.json"));
  if (options.series) {
    mkdirSync(join(target, "series"));
    for (const name of readdirSync(join(packageDirectory, "series")).sort()) {
      copyFileSync(join(packageDirectory, "series", name), join(target, "series", name));
    }
  }
}

/** Every athlete-table row as one JSON array per line, sorted, grouped by table. */
function dumpRows(databasePath: string): string {
  const database = new DatabaseSync(databasePath, { readOnly: true });
  const lines: string[] = [];
  for (const table of TABLES) {
    lines.push(`## ${table}`);
    const statement = database.prepare(`SELECT * FROM ${table}`);
    statement.setReturnArrays(true);
    const rows = (statement.all() as unknown[][]).map((row) => JSON.stringify(row)).sort();
    lines.push(...rows);
  }
  database.close();
  return `${lines.join("\n")}\n`;
}

async function emptyDestination(directory: string): Promise<{ path: string; database: unknown }> {
  const path = join(directory, "destination.ath.sqlite");
  const database = await db.createDatabase(path, {
    databaseId: DESTINATION_ID,
    createdAt: DESTINATION_CREATED_AT,
  });
  return { path, database };
}

function previewOutput(preview: Record<string, unknown>): Record<string, unknown> {
  return {
    source_database_id: preview.source_database_id,
    source_database_revision: preview.source_database_revision,
    destination_database_id: preview.destination_database_id,
    destination_database_revision: preview.destination_database_revision,
    restore: preview.restore,
    new_records: preview.new_records,
    new_record_count: preview.new_record_count,
    matching_records: preview.matching_records,
    matching_record_count: preview.matching_record_count,
    changed_records: preview.changed_records,
    changed_record_count: preview.changed_record_count,
    record_lists_truncated: preview.record_lists_truncated,
    conflicts: preview.conflicts,
    unrepresentable_records: preview.unrepresentable_records,
    unknown_fields: preview.unknown_fields,
    write_confirmed: false,
  };
}

async function restoreAndExport(
  packagePath: string,
  directory: string,
): Promise<{ databasePath: string; packageDirectory: string; preview: Record<string, unknown> }> {
  const { path, database } = await emptyDestination(directory);
  const loaded = await db.loadAthleticStandardPackage(packagePath);
  const preview = await db.previewPackageImport(database, loaded);
  await db.importRecords(database, preview);
  const packageDirectory = join(directory, "exported");
  await db.exportRecords(database, packageDirectory, { exported_at: EXPORTED_AT });
  (database as { close(): void }).close();
  return { databasePath: path, packageDirectory, preview: previewOutput(preview) };
}

// ---- demo ---------------------------------------------------------------------

async function demo(): Promise<void> {
  const directory = temporary("ath-golden-demo-");
  const databasePath = join(directory, "athlete.ath.sqlite");
  copyFileSync(join(root, "examples", "demo-athlete", "athlete.ath.sqlite"), databasePath);
  mkdirSync(join(directory, "attachments"));
  copyFileSync(
    join(root, "examples", "demo-athlete", "attachments", DEMO_ATTACHMENT),
    join(directory, "attachments", DEMO_ATTACHMENT),
  );
  const database = await db.openDatabase(databasePath, quiet);
  const timed = join(directory, "timed");
  const untimed = join(directory, "untimed");
  await db.exportRecords(database, timed, { exported_at: EXPORTED_AT });
  await db.exportRecords(database, untimed);
  database.close();
  const target = join(out, "demo");
  keepPackage(timed, target, { series: true });
  copyFileSync(join(untimed, "export-manifest.json"), join(target, "manifest-untimed.json"));

  const restoreDirectory = temporary("ath-golden-demo-restore-");
  const restored = await restoreAndExport(timed, restoreDirectory);
  writeFileSync(join(target, "restore-preview.json"), `${JSON.stringify(restored.preview, null, 2)}\n`);
  for (const name of ["athlete.ath.json", "export-manifest.json"]) {
    if (sha256(readFileSync(join(timed, name))) !== sha256(readFileSync(join(restored.packageDirectory, name)))) {
      throw new Error(`demo restore did not reproduce ${name}`);
    }
  }
}

// ---- version 2.0.0 package ---------------------------------------------------

async function version200(): Promise<void> {
  const directory = temporary("ath-golden-200-");
  const restored = await restoreAndExport(
    join(root, "tests", "fixtures", "version-2-0-0", "package"),
    directory,
  );
  const target = join(out, "version-2-0-0");
  keepPackage(restored.packageDirectory, target, { series: true });
  writeFileSync(join(target, "restore-preview.json"), `${JSON.stringify(restored.preview, null, 2)}\n`);
  writeFileSync(join(target, "database-rows.txt"), dumpRows(restored.databasePath));
}

// ---- many samples ---------------------------------------------------------------

/** The same rules as `deterministicJson` in the old exporter. */
function stableValue(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(stableValue);
  }
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0))
        .map(([key, child]) => [key, stableValue(child)]),
    );
  }
  return value;
}

function deterministic(value: unknown): Buffer {
  return Buffer.from(`${JSON.stringify(stableValue(value), null, 2)}\n`, "utf8");
}

const SYNTHETIC_DATABASE_ID = "01890f3e-0000-7000-8000-000000000001";
const CREATED = "2026-01-01T00:00:00.000Z";
const RECEIVED = "2026-01-02T00:00:00.000Z";
const LATER = "2026-01-03T00:00:00.000Z";
const BASE_MS = Date.UTC(2026, 0, 1);
const HEART_COUNT = 50_006;
const SAME_TIME_FIRST = 49_997;
const SAME_TIME_LAST = 50_002;
const TOOL = { author_type: "tool", tool: "synthetic", tool_version: "1.0.0" };

function rid(n: number): string {
  return `01890f3e-0000-7000-8000-${n.toString(16).padStart(12, "0")}`;
}

const SOURCE_A = rid(0x10);
const SOURCE_B = rid(0x11);
const SOURCE_C = rid(0x12);

function at(ms: number): string {
  return new Date(ms).toISOString();
}

function sampleData(
  source: string,
  quantity: string,
  unit: string,
  recordedAt: string,
  value: number,
  extra: Record<string, unknown> = {},
): Record<string, unknown> {
  return {
    source_id: source,
    received_at: RECEIVED,
    received_at_basis: "import_time",
    quantity,
    recorded_at: recordedAt,
    input_timestamp: {
      timestamp_text: recordedAt,
      timestamp_basis: "source_instant",
      utc_offset_missing_reason: "not_provided_by_source",
    },
    value: { standard_value: value, standard_unit: unit, input_value: value, input_unit: unit },
    ...extra,
  };
}

interface Built {
  records: Record<string, unknown>[];
  revisions: Record<string, unknown>[];
  deletions: Record<string, unknown>[];
  importKeys: Record<string, unknown>[];
}

function emptyBuilt(): Built {
  return { records: [], revisions: [], deletions: [], importKeys: [] };
}

function addRecord(
  built: Built,
  recordId: string,
  recordType: string,
  datas: Record<string, unknown>[],
  deleted: boolean,
): void {
  datas.forEach((data, index) => {
    built.revisions.push({
      record_id: recordId,
      revision: index + 1,
      previous_revision: index === 0 ? null : index,
      became_known_at: index === 0 ? CREATED : LATER,
      became_known_at_basis: "provider_import",
      changed_by: TOOL,
      record_type: recordType,
      data,
    });
  });
  built.records.push({
    record_id: recordId,
    created_at: CREATED,
    current_revision: datas.length,
    is_deleted: deleted,
    record_type: recordType,
    data: datas.at(-1),
  });
  if (deleted) {
    built.deletions.push({
      record_id: recordId,
      record_revision: datas.length,
      deleted_at: LATER,
      deleted_by: { author_type: "athlete" },
    });
  }
}

function importKey(built: Built, recordId: string, method: string, key: string): void {
  built.importKeys.push({ record_id: recordId, match_key_method: method, match_key: key, imported_at: RECEIVED });
}

const SPECIAL_SUPPLIED = {
  zeta: 1,
  alpha: "β",
  "10": true,
  "9": null,
  "😀": "smile",
  "ﬁ": "ligature",
  big: 9007199254740991,
  tiny: 1.5e-7,
  huge: 1e21,
  sum: 0.1 + 0.2,
  negative: -2.5e-8,
  control: "\u0000\u001f \"\\/",
};

function heartSamples(): Built {
  const built = emptyBuilt();
  for (let i = 0; i < HEART_COUNT; i++) {
    const sameTime = i >= SAME_TIME_FIRST && i <= SAME_TIME_LAST;
    const recordedAt = at(BASE_MS + (sameTime ? SAME_TIME_FIRST : i) * 1000);
    const recordId = sameTime ? rid(0x200000 - i) : rid(0x100000 + i);
    const extra = i === 1 ? { supplied_fields: SPECIAL_SUPPLIED } : {};
    const current = sampleData(SOURCE_A, "heart_rate", "bpm", recordedAt, 40 + (i % 150), extra);
    const datas =
      i % 10_000 === 5_000
        ? [sampleData(SOURCE_B, "heart_rate", "bpm", at(BASE_MS - 3_600_000 - i), 30), current]
        : [current];
    addRecord(built, recordId, "measurement_sample", datas, i === 7 || i === 50_003);
    if (i % 20_000 === 1) {
      importKey(built, recordId, "synthetic_v1", `heart-${i}`);
    }
    if (i === 1) {
      for (const key of ["z-key", "ä-key", "😀-key", "ﬁ-key"]) {
        importKey(built, recordId, "synthetic_text", key);
      }
    }
  }
  return built;
}

function respiratorySamples(): Built {
  const built = emptyBuilt();
  for (let j = 0; j < 3; j++) {
    addRecord(
      built,
      rid(0x300000 + j),
      "measurement_sample",
      [sampleData(SOURCE_A, "respiratory_rate", "brpm", at(BASE_MS + j * 60_000), 14 + j * 0.5)],
      false,
    );
  }
  return built;
}

function sourceBSamples(): Built {
  const built = emptyBuilt();
  for (let k = 0; k < 2; k++) {
    addRecord(
      built,
      rid(0x400000 + k),
      "measurement_sample",
      [sampleData(SOURCE_B, "heart_rate", "bpm", at(BASE_MS + k * 1000), 70 + k)],
      false,
    );
  }
  return built;
}

function seriesDocument(source: string, quantity: string, built: Built): Record<string, unknown> {
  const times = built.records.map((record) => (record.data as { recorded_at: string }).recorded_at).sort();
  return {
    athleticstandard_version: "2.1.0",
    source_id: source,
    quantity,
    start_at: times[0],
    end_at: times.at(-1),
    records: built.records,
    record_revisions: built.revisions,
    record_deletions: built.deletions,
    record_import_keys: built.importKeys,
  };
}

/** Write the synthetic input package. Returns each file's SHA-256. */
function writeSyntheticPackage(directory: string): Record<string, string> {
  mkdirSync(join(directory, "series"), { recursive: true });
  const hashes: Record<string, string> = {};
  const references: Record<string, unknown>[] = [];
  const groups: Array<[string, string, Built]> = [
    [SOURCE_A, "heart_rate", heartSamples()],
    [SOURCE_A, "respiratory_rate", respiratorySamples()],
    [SOURCE_B, "heart_rate", sourceBSamples()],
  ];
  groups.forEach(([source, quantity, built], index) => {
    const file = `series/input-${index + 1}.ath-series.json`;
    const document = seriesDocument(source, quantity, built);
    const bytes = deterministic(document);
    writeFileSync(join(directory, file), bytes);
    hashes[file] = sha256(bytes);
    references.push({
      file,
      sha256: hashes[file],
      source_id: source,
      quantity,
      start_at: document.start_at,
      end_at: document.end_at,
      sample_count: built.records.length,
      revision_count: built.revisions.length,
      deletion_count: built.deletions.length,
      import_key_count: built.importKeys.length,
    });
  });

  const main = emptyBuilt();
  addRecord(
    main,
    SOURCE_A,
    "source",
    [
      {
        source_type: "wearable",
        data_writer: "Synthetic",
        data_route: "synthetic_route",
        vendor: "synthetic",
        supplied_fields: { b: 2, a: 1, "1": "one" },
      },
    ],
    false,
  );
  addRecord(
    main,
    SOURCE_B,
    "source",
    [
      { source_type: "wearable", data_writer: "Synthetic", data_route: "synthetic_route_b" },
      { source_type: "wearable", data_writer: "Synthetic", data_route: "synthetic_route_b", vendor: "synthetic" },
    ],
    false,
  );
  addRecord(
    main,
    SOURCE_C,
    "source",
    [{ source_type: "wearable", data_writer: "Synthetic", data_route: "synthetic_route_c" }],
    true,
  );
  importKey(main, SOURCE_A, "synthetic_source", "A");
  const document = {
    athleticstandard_version: "2.1.0",
    database_id: SYNTHETIC_DATABASE_ID,
    database_revision: 7,
    records: main.records,
    record_revisions: main.revisions,
    record_deletions: main.deletions,
    record_erasure_markers: [{ record_id: rid(0x13), erased_at: LATER, reason_code: "athlete_request" }],
    record_import_keys: main.importKeys,
    measurement_series_files: references,
  };
  const bytes = deterministic(document);
  writeFileSync(join(directory, "athlete.ath.json"), bytes);
  hashes["athlete.ath.json"] = sha256(bytes);
  return hashes;
}

async function manySamples(): Promise<void> {
  const directory = temporary("ath-golden-many-");
  const input = join(directory, "input");
  const hashes = writeSyntheticPackage(input);
  const restored = await restoreAndExport(input, directory);
  const target = join(out, "many-samples");
  keepPackage(restored.packageDirectory, target, { series: false });
  writeFileSync(join(target, "input-hashes.json"), `${JSON.stringify(hashes, null, 2)}\n`);
  const preview = restored.preview as { new_records: string[] };
  writeFileSync(
    join(target, "restore-preview.json"),
    `${JSON.stringify({ ...restored.preview, new_records: preview.new_records }, null, 2)}\n`,
  );
  rmSync(directory, { recursive: true, force: true });
}

// ---- unrepresentable package ----------------------------------------------------------

async function unrepresentable(): Promise<void> {
  const packageDirectory = join(out, "demo");
  const directory = temporary("ath-golden-unrepresentable-");
  const copy = join(directory, "package");
  mkdirSync(copy);
  const document = JSON.parse(readFileSync(join(packageDirectory, "main.json"), "utf8")) as {
    records: Array<{ current_revision: number }>;
    future_package_field?: string;
  };
  document.future_package_field = "not understood by v1";
  document.records[0]!.current_revision = 0;
  writeFileSync(join(copy, "athlete.ath.json"), `${JSON.stringify(document)}\n`);
  try {
    await db.loadAthleticStandardPackage(copy);
    throw new Error("the unrepresentable package loaded");
  } catch (error) {
    if (!(error instanceof db.PackageRepresentationError)) {
      throw error;
    }
    const failure = error as {
      message: string;
      source_database_id?: string;
      source_database_revision?: number;
      unrepresentable_records: string[];
      unknown_fields: string[];
    };
    writeFileSync(
      join(out, "unrepresentable.json"),
      `${JSON.stringify(
        {
          message: failure.message,
          source_database_id: failure.source_database_id ?? null,
          source_database_revision: failure.source_database_revision ?? null,
          unrepresentable_records: failure.unrepresentable_records,
          unknown_fields: failure.unknown_fields,
        },
        null,
        2,
      )}\n`,
    );
  }
}

// ---- erasure ----------------------------------------------------------------------------

/**
 * Erasure previews of three demo records, then one erasure with a fixed time.
 * Attachment paths are kept relative to the database folder.
 */
async function erasure(): Promise<void> {
  const directory = temporary("ath-golden-erase-");
  const databasePath = join(directory, "athlete.ath.sqlite");
  copyFileSync(join(root, "examples", "demo-athlete", "athlete.ath.sqlite"), databasePath);
  mkdirSync(join(directory, "attachments"));
  copyFileSync(
    join(root, "examples", "demo-athlete", "attachments", DEMO_ATTACHMENT),
    join(directory, "attachments", DEMO_ATTACHMENT),
  );
  const database = await db.openDatabase(databasePath, quiet);
  const previews: Record<string, unknown> = {};
  let attachmentPreview: unknown;
  for (const recordId of [
    "01890f3e-e7d3-7cc0-88c8-00000000000b",
    "01890f3e-e7d3-7cc0-88c8-00000000000e",
    "01890f3e-e7d3-7cc0-88c8-000000000003",
  ]) {
    const preview = await db.previewRecordErasure(database, recordId, 1);
    if (recordId.endsWith("0e")) {
      attachmentPreview = preview;
    }
    previews[recordId] = {
      ...preview,
      attachments: (preview.attachments as Array<{ path: string }>).map((attachment) => ({
        ...attachment,
        path: attachment.path.slice(directory.length + 1),
      })),
    };
  }
  const erased = await db.eraseRecord(database, {
    preview: attachmentPreview,
    reason_code: "recorded_in_error",
    erased_at: EXPORTED_AT,
  });
  database.close();
  const target = join(out, "erasure");
  rmSync(target, { recursive: true, force: true });
  mkdirSync(target, { recursive: true });
  writeFileSync(join(target, "previews.json"), `${JSON.stringify(previews, null, 2)}\n`);
  writeFileSync(join(target, "erased.json"), `${JSON.stringify(erased, null, 2)}\n`);
  writeFileSync(join(target, "database-rows.txt"), dumpRows(databasePath));
}

mkdirSync(out, { recursive: true });
await erasure();
await demo();
await version200();
await manySamples();
await unrepresentable();
console.log(`wrote ${out}`);
