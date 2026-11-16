/**
 * Freeze the 2.1.0 database maintenance results that the native store must match.
 *
 * The script runs the old TypeScript implementation on synthetic fixtures only:
 * the 2.1.0 demo database, the npm 1.0.0 encoding fixture, and the 2.0.0
 * fixture. It writes:
 *
 * - `check-scenarios.json`: named damage scenarios as plain SQL statements and
 *   file changes, with the old `checkDatabase` result, repair counts, and the
 *   table hashes after a confirmed `--from-revision` repair.
 * - `stats.json`: `databaseStats` and `renderDatabaseStats` for each fixture.
 * - `migration.json`: the migration preview and the athlete table hashes after
 *   the old confirmed migration of each fixture.
 *
 * The native tests replay the same SQL on the same fixtures. Damage is written
 * as SQL so that both implementations receive identical bytes.
 *
 * Run with a checkout of the reference commit, as tools/reference/README.md
 * describes, and a `sqlite3` program on the path (the old backup needs it):
 *
 *   ATH_REFERENCE_ROOT=<checkout> node --import <checkout>/node_modules/tsx/dist/esm/index.mjs \
 *     tools/reference/maintenance-golden.ts
 *
 * Output: tests/golden/maintenance/*.json (2-space JSON, trailing newline).
 */
import { copyFileSync, mkdirSync, mkdtempSync, readdirSync, realpathSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createHash } from "node:crypto";
import { reference, referenceModule, referencePackage, repository } from "./reference.ts";

const { createClient } = await referencePackage<typeof import("@libsql/client")>("@libsql/client");
const {
  checkDatabase,
  countProviderEncodingRepairCandidates,
  countRevisionAlignRepairCandidates,
  databaseStats,
  executeConfirmedDatabaseMigration,
  executeConfirmedProviderEncodingRepair,
  inspectDatabase,
  listRevisionAlignRepairRecordIds,
  openDatabase,
  readDatabaseMetadata,
  buildMigrationPreview,
  releaseLocalDatabase,
  renderDatabaseStats,
  markSkipWalCheckpointOnRelease,
} = await referenceModule("src/db/index.ts");

const root = reference;
const outDir = join(repository, "tests", "golden", "maintenance");

const FIXTURES = {
  demo: join(root, "examples/demo-athlete/athlete.ath.sqlite"),
  npm100: join(root, "tests/fixtures/npm-100-encoding/athlete.ath.sqlite"),
  v200: join(root, "tests/fixtures/version-2-0-0/database.ath.sqlite"),
} as const;

const ATTACHMENTS = {
  demo: join(root, "examples/demo-athlete/attachments"),
  npm100: undefined,
  v200: join(root, "tests/fixtures/version-2-0-0/package/attachments"),
} as const;

/** Every athlete-data table. The order matches the native tests. */
const ATHLETE_TABLES = [
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

type FixtureName = keyof typeof FIXTURES;

interface Scenario {
  name: string;
  fixture: FixtureName;
  /** Statements run on a plain connection with foreign keys off. */
  sql: string[];
  /** Files removed, relative to the database folder. */
  remove_files?: string[];
  /** Files replaced, relative to the database folder, as UTF-8 text. */
  write_files?: Record<string, string>;
  /** Run a confirmed `--from-revision` repair and record the table hashes. */
  repair?: boolean;
}

const firstPoint = "(SELECT record_id FROM point_measurements ORDER BY record_id LIMIT 1)";
const currentRevisionOf = (id: string) => `(SELECT current_revision FROM records WHERE record_id = ${id})`;

const SCENARIOS: Scenario[] = [
  { name: "clean", fixture: "demo", sql: [], repair: true },
  {
    name: "typed_column_value",
    fixture: "demo",
    sql: [`UPDATE point_measurements SET standard_value = standard_value + 1 WHERE record_id = ${firstPoint}`],
    repair: true,
  },
  {
    name: "typed_column_null",
    fixture: "demo",
    sql: [`UPDATE point_measurements SET recorded_at = NULL WHERE record_id = ${firstPoint}`],
    repair: true,
  },
  {
    name: "typed_text_columns",
    fixture: "demo",
    sql: [
      "UPDATE sleep_sessions SET start_at = '2001-01-01T00:00:00.000Z'",
      "UPDATE workout_sessions SET activity_type = 'damaged'",
      "UPDATE provider_scores SET metric = 'damaged', scale = 'damaged'",
      "UPDATE provider_summaries SET summary_type = 'damaged'",
      "UPDATE athlete_reports SET report_type = 'damaged'",
      "UPDATE measurement_samples SET duration_ms = 12345, standard_unit = 'damaged'",
    ],
    repair: true,
  },
  {
    name: "data_json_key_order",
    fixture: "demo",
    sql: [
      "UPDATE sources SET data_json = json_set(json_remove(data_json, '$.source_type'), '$.source_type', json_extract(data_json, '$.source_type'))",
    ],
    repair: true,
  },
  {
    name: "data_json_meaning",
    fixture: "demo",
    sql: [
      `UPDATE point_measurements SET data_json = json_set(data_json, '$.value.standard_value', 999), standard_value = 999 WHERE record_id = ${firstPoint}`,
    ],
    repair: true,
  },
  {
    name: "record_json_shape",
    fixture: "demo",
    sql: [
      `UPDATE record_revisions SET record_json = '{}' WHERE record_id = ${firstPoint} AND revision = ${currentRevisionOf(firstPoint)}`,
    ],
  },
  {
    name: "child_rows",
    fixture: "demo",
    sql: [
      "DELETE FROM workout_session_segments",
      "UPDATE provider_reporting_periods SET provider_date = '2000-01-01'",
      "DELETE FROM authored_record_evidence WHERE evidence_order = 0 AND authored_record_id = (SELECT record_id FROM predictions LIMIT 1)",
    ],
    repair: true,
  },
  {
    name: "child_json",
    fixture: "demo",
    sql: [
      "UPDATE planned_session_blocks SET block_json = json_set(block_json, '$.damaged', 1)",
      "UPDATE reported_nutrition_items SET item_json = json_set(item_json, '$.damaged', 1)",
    ],
    repair: true,
  },
  {
    name: "attachment_missing",
    fixture: "demo",
    sql: [],
    remove_files: ["attachments/01890f3e-e7d3-7cc0-88c8-00000000000e.png"],
  },
  {
    name: "attachment_bytes",
    fixture: "demo",
    sql: [],
    write_files: { "attachments/01890f3e-e7d3-7cc0-88c8-00000000000e.png": "not a png" },
  },
  {
    name: "revision_chain",
    fixture: "demo",
    sql: [`DELETE FROM record_revisions WHERE record_id = ${firstPoint} AND revision = 1`],
  },
  {
    name: "deletions",
    fixture: "demo",
    sql: [
      "UPDATE record_deletions SET deleted_at = 'yesterday'",
      "INSERT INTO record_erasure_markers (record_id, erased_at, reason_code) SELECT record_id, 'soon', 'other' FROM sources ORDER BY record_id LIMIT 1",
    ],
  },
  {
    name: "deletion_row_removed",
    fixture: "demo",
    sql: ["DELETE FROM record_deletions"],
  },
  {
    name: "grade_arithmetic",
    fixture: "demo",
    sql: [
      "UPDATE record_revisions SET record_json = json_set(record_json, '$.signed_error', 99) WHERE record_type = 'prediction_grade'",
      "UPDATE prediction_grades SET data_json = json_set(data_json, '$.signed_error', 99), signed_error = 99",
    ],
  },
  {
    name: "foreign_key_and_wall",
    fixture: "demo",
    sql: [
      "INSERT INTO record_attachment_links (owner_record_id, attachment_order, attachment_record_id) SELECT record_id, 7, '01890f3e-e7d3-7cc0-88c8-ffffffffffff' FROM nutrition_entries",
      "ALTER TABLE athlete_reports ADD COLUMN source_id TEXT",
      "PRAGMA application_id = 0",
    ],
  },
  {
    name: "issue_limit",
    fixture: "demo",
    sql: [
      "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 1100) INSERT INTO record_erasure_markers (record_id, erased_at, reason_code) SELECT printf('01890f3e-e7d3-7cc0-88c8-%012d', i), 'bad', 'other' FROM n",
    ],
  },
  { name: "npm100", fixture: "npm100", sql: [], repair: true },
  { name: "v200", fixture: "v200", sql: [], repair: true },
];

function copyFixture(fixture: FixtureName, prefix: string): { dir: string; path: string } {
  const dir = mkdtempSync(join(realpathSync(tmpdir()), prefix));
  const path = join(dir, "athlete.ath.sqlite");
  copyFileSync(FIXTURES[fixture], path);
  const attachments = ATTACHMENTS[fixture];
  if (attachments !== undefined) {
    mkdirSync(join(dir, "attachments"));
    for (const name of readdirSync(attachments)) {
      copyFileSync(join(attachments, name), join(dir, "attachments", name));
    }
  }
  return { dir, path };
}

async function applySql(path: string, statements: string[]): Promise<void> {
  const client = createClient({ url: pathToFileURL(path).href });
  await client.execute("PRAGMA foreign_keys = OFF");
  for (const statement of statements) {
    await client.execute(statement);
  }
  client.close();
}

function cell(value: unknown): unknown {
  if (value === null || value === undefined) return null;
  if (typeof value === "bigint") return Number(value);
  if (value instanceof ArrayBuffer) return Buffer.from(value).toString("hex");
  return value;
}

async function tableHashes(path: string): Promise<Record<string, { rows: number; sha256: string }>> {
  const client = createClient({ url: pathToFileURL(path).href });
  const out: Record<string, { rows: number; sha256: string }> = {};
  for (const table of ATHLETE_TABLES) {
    const exists = await client.execute({
      sql: "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?",
      args: [table],
    });
    let rows: string[] = [];
    if (exists.rows.length > 0) {
      const result = await client.execute(`SELECT * FROM ${table}`);
      rows = result.rows.map((row) =>
        JSON.stringify(result.columns.map((_, index) => cell(row[index]))),
      );
    }
    rows.sort((left, right) => Buffer.compare(Buffer.from(left, "utf8"), Buffer.from(right, "utf8")));
    const hash = createHash("sha256");
    for (const row of rows) hash.update(`${row}\n`, "utf8");
    out[table] = { rows: rows.length, sha256: hash.digest("hex") };
  }
  const revision = await client.execute("SELECT current_revision FROM database_state");
  out.database_state = { rows: 1, sha256: String(revision.rows[0]?.current_revision) };
  client.close();
  return out;
}

async function open(path: string) {
  const database = await openDatabase(path, { allowOutdatedSchema: true, deferWal: true, onSecurityWarning: () => {} });
  markSkipWalCheckpointOnRelease(database);
  return database;
}

async function runScenario(scenario: Scenario): Promise<Record<string, unknown>> {
  const { dir, path } = copyFixture(scenario.fixture, "ath-golden-check-");
  try {
    await applySql(path, scenario.sql);
    for (const file of scenario.remove_files ?? []) unlinkSync(join(dir, file));
    for (const [file, text] of Object.entries(scenario.write_files ?? {})) writeFileSync(join(dir, file), text);
    const database = await open(path);
    let check;
    let repair;
    try {
      check = await checkDatabase(database);
      repair = {
        repair_candidates: await countProviderEncodingRepairCandidates(database),
        revision_align_candidates: await countRevisionAlignRepairCandidates(database),
        revision_align_record_ids: await listRevisionAlignRepairRecordIds(database),
      };
    } finally {
      await releaseLocalDatabase(database);
    }
    let repaired: Record<string, unknown> | undefined;
    if (scenario.repair === true) {
      const writable = await openDatabase(path, { allowOutdatedSchema: true, deferWal: true, onSecurityWarning: () => {} });
      try {
        const result = await executeConfirmedProviderEncodingRepair(writable, { fromRevision: true });
        repaired = {
          made_backup: result.backup_path !== "",
          repaired_record_count: result.repaired_record_count,
          revision_align_record_count: result.revision_align_record_count,
          database_revision: result.database_revision,
          tables: await tableHashes(path),
        };
      } finally {
        await releaseLocalDatabase(writable);
      }
    }
    return {
      name: scenario.name,
      fixture: scenario.fixture,
      sql: scenario.sql,
      ...(scenario.remove_files === undefined ? {} : { remove_files: scenario.remove_files }),
      ...(scenario.write_files === undefined ? {} : { write_files: scenario.write_files }),
      check,
      repair,
      ...(repaired === undefined ? {} : { repaired }),
    };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

async function stats(fixture: FixtureName): Promise<Record<string, unknown>> {
  const { dir, path } = copyFixture(fixture, "ath-golden-stats-");
  try {
    const database = await open(path);
    try {
      const value = await databaseStats(database);
      return { stats: value, text: renderDatabaseStats(value) };
    } finally {
      await releaseLocalDatabase(database);
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

async function migration(fixture: FixtureName): Promise<Record<string, unknown>> {
  const { dir, path } = copyFixture(fixture, "ath-golden-migrate-");
  try {
    const inspector = await inspectDatabase(path, { onSecurityWarning: () => {} });
    let preview;
    try {
      preview = buildMigrationPreview(await readDatabaseMetadata(inspector), path);
    } finally {
      await releaseLocalDatabase(inspector);
    }
    const database = await openDatabase(path, { allowOutdatedSchema: true, deferWal: true, onSecurityWarning: () => {} });
    let migrated;
    try {
      migrated = await executeConfirmedDatabaseMigration(database);
    } finally {
      await releaseLocalDatabase(database);
    }
    const { planned_backup_path: _ignored, ...rest } = preview;
    return {
      fixture,
      preview: { ...rest, planned_backup_path_present: preview.planned_backup_path !== null },
      migrated: migrated.migrated,
      tables: await tableHashes(path),
    };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

function write(name: string, value: unknown): void {
  writeFileSync(join(outDir, name), `${JSON.stringify(value, null, 2)}\n`);
}

mkdirSync(outDir, { recursive: true });
const scenarios = [];
for (const scenario of SCENARIOS) {
  scenarios.push(await runScenario(scenario));
}
write("check-scenarios.json", {
  generated_by: "tools/reference/maintenance-golden.ts",
  reference: "2.1.0 TypeScript implementation (checkDatabase, repair counts, executeConfirmedProviderEncodingRepair)",
  scenarios,
});
write("stats.json", {
  generated_by: "tools/reference/maintenance-golden.ts",
  reference: "2.1.0 TypeScript implementation (databaseStats, renderDatabaseStats)",
  demo: await stats("demo"),
  npm100: await stats("npm100"),
  v200: await stats("v200"),
});
write("migration.json", {
  generated_by: "tools/reference/maintenance-golden.ts",
  reference: "2.1.0 TypeScript implementation (executeConfirmedDatabaseMigration to SQL schema 4)",
  fixtures: [await migration("npm100"), await migration("v200"), await migration("demo")],
});
console.log(`wrote ${outDir}`);
