/**
 * Freeze what the 2.1.0 `ath` command line prints, so the native executable
 * can be compared with it.
 *
 * Each scenario starts in a new private folder with a copy of one synthetic
 * fixture: the 2.1.0 demo database, the 2.0.0 database, the npm 1.0.0
 * database, or none. Provider files are copied from the approved fixtures.
 * The script then runs the old command line as a child process with pipes
 * for its standard streams, so every run is non-interactive, and records the
 * exit status, standard output, and standard error of each step.
 *
 * Model calls go to a local 127.0.0.1 server started by this script. The old
 * program has no setting for its gateway roots, so a small module loaded
 * before it rewrites the two published roots to that server. The native
 * executable reads the same root from ATH_GATEWAY_BASE_URL. A fake `security`
 * and `secret-tool` first on the path stand in for the password store, so the
 * real one is never read.
 *
 * Only values that differ by design are normalized, by the rules in
 * `NORMALIZATION` below. The native test applies the same rules to its own
 * output (`native/tests/cli_golden.rs`).
 *
 * Run from a checkout of the reference commit with its dependencies
 * installed. `ATH_REFERENCE_ROOT` names that checkout when the script runs
 * from another one:
 *
 *   ATH_REFERENCE_ROOT=<checkout> node --import tsx native/tools/reference/cli-golden.ts
 *
 * Output: native/tests/golden/cli/*.json (2-space JSON, trailing newline).
 */
import { spawn } from "node:child_process";
import { chmodSync, copyFileSync, cpSync, mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, "..", "..", "..");
const reference = process.env.ATH_REFERENCE_ROOT ?? repo;
const outDir = join(repo, "native", "tests", "golden", "cli");

/** Synthetic databases a scenario can start from, relative to the reference checkout. */
const FIXTURES = {
  demo: "examples/demo-athlete/athlete.ath.sqlite",
  v200: "tests/fixtures/version-2-0-0/database.ath.sqlite",
  npm100: "tests/fixtures/npm-100-encoding/athlete.ath.sqlite",
} as const;

type FixtureName = keyof typeof FIXTURES | "none";

/** The attachment files each fixture database names, copied beside it. */
const ATTACHMENTS: Partial<Record<FixtureName, string>> = {
  demo: "examples/demo-athlete/attachments",
  v200: "tests/fixtures/version-2-0-0/package/attachments",
};

/** Provider folders a scenario can copy, relative to the reference checkout. */
const PROVIDER_FILES = {
  wearable_csv: "tests/fixtures/exports/whoop",
} as const;

interface Step {
  argv: string[];
  stdin?: string;
  env?: Record<string, string>;
  status?: number;
  stdout?: string;
  stderr?: string;
}

interface Scenario {
  name: string;
  fixture: FixtureName;
  /** Bring the database to the running program's SQL step before the steps. */
  current?: boolean;
  /** Folders to create in the working folder first. */
  dirs?: string[];
  /** Provider folders to copy: destination name to `PROVIDER_FILES` key. */
  provider?: Record<string, keyof typeof PROVIDER_FILES>;
  /** Statements run on the database before the steps, with foreign keys off. */
  sql?: string[];
  steps: Step[];
}

const DEMO_ID = "01890f3e-e7d3-7cc0-88c8-000000000001";
const NPM100_ID = "01a1672e-255a-765a-970c-e28eef8e8080";
const RESULT = "01890f3e-e7d3-7cc0-88c8-00000000000b";
const PREDICTION = "01890f3e-e7d3-7cc0-88c8-000000000010";
const WORKOUT = "01890f3e-e7d3-7cc0-88c8-00000000000a";
const MISSING = "01890f3e-e7d3-7cc0-88c8-0000000000ff";
const KEY = { AI_GATEWAY_API_KEY: "test-vercel-key" };
const BOTH_KEYS = { AI_GATEWAY_API_KEY: "test-vercel-key", OPENROUTER_API_KEY: "test-openrouter-key" };
const DAMAGE = [
  "UPDATE point_measurements SET standard_value = standard_value + 1 WHERE record_id = (SELECT record_id FROM point_measurements ORDER BY record_id LIMIT 1)",
];

const step = (argv: string[], extra: Partial<Step> = {}): Step => ({ argv, ...extra });

const GROUPS: Record<string, Scenario[]> = {
  setup: [
    {
      name: "init_json",
      fixture: "none",
      steps: [step(["init", "--yes", "--json"]), step(["stats", "--json"]), step(["skill", "--json"])],
    },
    {
      name: "init_profile",
      fixture: "none",
      steps: [
        step([
          "init",
          "--yes",
          "--no-skill",
          "--name",
          "Sam Lee",
          "--birth-year",
          "1990",
          "--sex",
          "female",
          "--units",
          "imperial",
          "--timezone",
          "America/Chicago",
        ]),
        step(["records", "list", "--type", "athlete_profile", "--json"]),
      ],
    },
    {
      name: "init_dropped_values",
      fixture: "none",
      steps: [
        step(["init", "--yes", "--no-skill", "--sex", "other", "--units", "furlongs", "--json"]),
        step(["records", "list", "--type", "athlete_profile", "--json"]),
      ],
    },
    {
      name: "init_empty_and_refusals",
      fixture: "none",
      steps: [
        step(["init", "--yes", "--empty", "--no-skill", "--file", "second.ath.sqlite"]),
        step(["init", "--yes", "--empty", "--no-skill", "--file", "second.ath.sqlite"]),
        step(["init", "--yes", "--file", "plain.sqlite"]),
        step(["stats", "--file", "second.ath.sqlite", "--json"]),
        step(["init", "--yes", "--empty", "--json", "--no-skill", "--file", "third.ath.sqlite"]),
      ],
    },
    {
      name: "init_with_harness_folder",
      fixture: "none",
      dirs: [".claude"],
      steps: [step(["init", "--yes"]), step(["skill"])],
    },
    {
      name: "skill_install",
      fixture: "none",
      steps: [
        step(["skill"]),
        step(["skill", "--json"]),
        step(["skill", "install"]),
        step(["skill"]),
        step(["skill", "install", "--json"]),
        step(["skill", "--json", "install"]),
        step(["skill", "install"]),
      ],
    },
    {
      name: "migrate_version_2_0_0",
      fixture: "v200",
      steps: [
        step(["migrate", "--json"]),
        step(["migrate"]),
        step(["migrate", "--dry-run"]),
        step(["migrate", "--yes"]),
        step(["migrate", "--yes", "--confirm-migration", "wrong"]),
        step(["migrate", "--yes", "--json", "--confirm-migration", DEMO_ID]),
        step(["log", "--yes", "slept", "fine"]),
        step(["migrate", "--yes", "--confirm-migration", DEMO_ID]),
        step(["migrate"]),
        step(["check", "--json"]),
        step(["stats"]),
      ],
    },
    {
      name: "migrate_version_1_0_0",
      fixture: "npm100",
      steps: [
        step(["migrate", "athlete.ath.sqlite", "--json"]),
        step(["migrate", "--file", "athlete.ath.sqlite", "--yes", "--confirm-migration", NPM100_ID]),
        step(["check"]),
      ],
    },
    {
      name: "repair_version_1_0_0",
      fixture: "npm100",
      steps: [
        step(["repair", "--json"]),
        step(["repair"]),
        step(["repair", "--dry-run"]),
        step(["repair", "--yes", "--json"]),
        step(["repair", "--yes"]),
        step(["repair"]),
      ],
    },
    {
      name: "repair_revision_align",
      fixture: "demo",
      current: true,
      sql: DAMAGE,
      steps: [
        step(["repair", "--json"]),
        step(["repair", "--yes"]),
        step(["repair", "--yes", "--from-revision"]),
        step(["repair", "--yes", "--from-revision", "--confirm-revision-align", "wrong"]),
        step(["repair", "--yes", "--from-revision", "--confirm-revision-align", DEMO_ID]),
        step(["repair"]),
        step(["check"]),
      ],
    },
    {
      name: "keys",
      fixture: "none",
      steps: [
        step(["key"]),
        step(["key", "--json"]),
        step(["key"], { env: KEY }),
        step(["key", "--json"], { env: BOTH_KEYS }),
        step(["key", "set", "vercel"]),
        step(["key", "set", "garmin"]),
        step(["key", "set", "nothing"]),
        step(["key", "clear"]),
        step(["key", "clear", "openrouter"]),
        step(["key", "clear", "garmin"]),
      ],
    },
    {
      name: "update_and_share",
      fixture: "none",
      steps: [step(["--update"]), step(["share"]), step(["share", "--json"]), step([])],
    },
  ],
  read: [
    {
      name: "stats_and_check",
      fixture: "demo",
      current: true,
      steps: [
        step(["stats"]),
        step(["stats", "--json"]),
        step(["stats", "athlete.ath.sqlite"]),
        step(["check"]),
        step(["check", "--json"]),
        step(["check", "athlete.ath.sqlite", "--file", "athlete.ath.sqlite"]),
        step([]),
      ],
    },
    {
      name: "check_invalid",
      fixture: "demo",
      current: true,
      sql: DAMAGE,
      steps: [step(["check"]), step(["check", "--json"])],
    },
    {
      name: "records",
      fixture: "demo",
      current: true,
      steps: [
        step(["records", "list"]),
        step(["records", "list", "--json", "--limit", "3"]),
        step(["records", "list", "--type", "benchmark_result", "--json"]),
        step(["records", "list", "--type", "nope"]),
        step(["records", "list", "--include-deleted", "--limit", "2"]),
        step(["records", "show", RESULT]),
        step(["records", "show", RESULT, "--json", "--history", "--links"]),
        step(["records", "show", RESULT, "--revision", "1", "--json"]),
        step(["records", "show", RESULT, "--revision", "1"]),
        step(["records", "show", RESULT, "--revision", "2"]),
        step(["records", "show", PREDICTION, "--links"]),
        step(["records", "show", MISSING]),
        step(["records", "show", "not-a-uuid"]),
      ],
    },
    {
      name: "series",
      fixture: "demo",
      current: true,
      steps: [
        step(["series", "heart_rate"]),
        step(["series", "heart_rate", "--json"]),
        step(["series", "heart_rate", "--raw", "--json"]),
        step(["series", "heart_rate", "--raw"]),
        step(["series", "heart_rate", "--source", "nope"]),
        step(["series", "steps"]),
        step(["series", "nope"]),
        step(["series", "heart_rate", "--timezone", "UTC", "--from", "2026-08-09", "--to", "2026-08-09", "--json"]),
        step(["series", "heart_rate", "--timezone", "Not/AZone"]),
      ],
    },
    {
      name: "calculate_and_series_version_1_0_0",
      fixture: "npm100",
      current: true,
      steps: [
        step(["stats"]),
        step(["series", "active_energy", "--timezone", "UTC"]),
        step(["calculate", "summary", "resting_heart_rate", "--json"]),
        step(["calculate", "summary", "resting_heart_rate"]),
        step(["calculate", "baseline", "hrv_rmssd", "--group", "month"]),
        step(["calculate", "trend", "resting_heart_rate", "--json"]),
        step(["calculate", "coverage", "hrv_rmssd"]),
        step(["calculate", "summary", "nope_quantity"]),
        step(["calculate", "summary", "vo2_max", "--from", "2020-01-01", "--to", "2020-01-02", "--timezone", "UTC"]),
      ],
    },
    {
      name: "calculate_refusals",
      fixture: "demo",
      current: true,
      steps: [
        step(["calculate", "nope", "heart_rate"]),
        step(["calculate", "summary", "heart_rate", "--group", "year"]),
        step(["calculate", "summary", "heart_rate", "--record-kind", "nope"]),
        step(["calculate", "provider-period-summary", "recovery", "--record-kind", "provider-score", "--period-type", "nope"]),
        step(["calculate", "summary", "hrv_rmssd", "--json"]),
      ],
    },
    {
      name: "apply",
      fixture: "demo",
      current: true,
      steps: [
        step(["apply", "heart-rate-zones", "--record", WORKOUT, "--json"]),
        step(["apply", "heart-rate-zones", "--record", WORKOUT]),
        step(["apply", "skills/training-load-heart-rate.md", "--record", WORKOUT]),
        step(["apply", "nope", "--record", WORKOUT]),
        step(["apply", "heart-rate-zones", "--record", MISSING]),
      ],
    },
    {
      name: "prediction_evidence",
      fixture: "demo",
      current: true,
      steps: [
        step(["predict", "fran", "--json", "--yes"]),
        step(["predict", "fran", "--json", "--as-of", "2026-08-11"]),
        step(["predict", "fran", "--json", "--as-of", "2026-08-10T12:00:00.000Z", "--timezone", "UTC"]),
        step(["predict", "nope", "--json"]),
        step(["predict", "fran", "--json", "--as-of", "yesterday"]),
      ],
    },
  ],
  model: [
    {
      name: "models",
      fixture: "demo",
      current: true,
      steps: [
        step(["models"]),
        step(["models"], { env: KEY }),
        step(["models", "--json"], { env: KEY }),
        step(["models", "--gateway", "bogus"], { env: KEY }),
        step(["models"], { env: BOTH_KEYS }),
        step(["models", "--gateway", "openrouter", "--json"], { env: BOTH_KEYS }),
        step(["models", "--default", "nope"], { env: KEY }),
        step(["models", "--default", "TEST/STEADY", "--json"], { env: KEY }),
        step(["models", "--default", "test/steady"], { env: KEY }),
      ],
    },
    {
      name: "predict_with_a_model",
      fixture: "demo",
      current: true,
      steps: [
        step(["predict", "fran"]),
        step(["predict", "fran"], { env: KEY }),
        step(["predict", "fran", "--model", "nope"], { env: KEY }),
        step(["predict", "fran", "--model", "test/steady", "--dry-run"], { env: KEY }),
        step(["predict", "fran", "--model", "test/steady"], { env: KEY }),
        step(["predict", "fran", "--model", "test/down", "--yes"], { env: KEY }),
        step(["predict", "fran", "--model", "test/steady", "--yes", "--gateway", "vercel"], { env: KEY }),
        step(["records", "list", "--type", "prediction"]),
      ],
    },
    {
      name: "backtest",
      fixture: "demo",
      current: true,
      steps: [
        step(["backtest"]),
        step(["backtest"], { env: KEY }),
        step(["log", "--yes", "Fran", "in", "4:41", "rx"]),
        step(["backtest", "--model", "test/steady", "--model", "test/down"], { env: KEY }),
        step(["share"]),
        step(["backtest", "--all"], { env: KEY }),
        step(["backtest", "--all", "--json"], { env: KEY }),
        step(["models", "--default", "test/steady"], { env: KEY }),
        step(["backtest", "--json"], { env: KEY }),
        step(["share", "--json"]),
        step(["records", "list", "--type", "prediction"]),
      ],
    },
  ],
  write: [
    {
      name: "log",
      fixture: "demo",
      current: true,
      steps: [
        step(["log", "--json", "slept", "badly,", "about", "5", "hours"]),
        step(["log", "slept", "badly,", "about", "5", "hours"]),
        step(["log", "--dry-run", "RMSSD", "61", "ms"]),
        step(["log", "--yes", "RMSSD", "61", "ms"]),
        step(["log", "--yes", "--json", "sore", "quads", "4/5"]),
        step(["log", "--yes", "--at", "2026-08-12T07:00:00.000Z", "sore", "quads", "4/5"]),
        step(["log", "--yes", "--agent", "coach-bot"], { stdin: "felt strong today\n" }),
        step(["log", "--yes", "HRV", "61"]),
        step(["log", "--yes"], { stdin: '{"record_type":"nope","data":{}}' }),
        step(["log", "--yes", "--at", "yesterday", "note"]),
        step(["records", "list", "--type", "athlete_report", "--json"]),
        step(["records", "list", "--type", "point_measurement"]),
      ],
    },
    {
      name: "log_result_grades_open_prediction",
      fixture: "demo",
      current: true,
      steps: [
        step(["log", "--yes", "--benchmark", "fran", "4:30"]),
        step(["log", "--json", "--at", "2026-08-20T10:00:00.000Z", "Fran", "in", "4:41", "rx"]),
        step(["stats"]),
      ],
    },
    {
      name: "grade",
      fixture: "demo",
      current: true,
      steps: [
        step(["grade", PREDICTION, "--result", RESULT, "--json"]),
        step(["grade", PREDICTION, "--result", RESULT]),
        step(["grade", PREDICTION, "--result", RESULT, "--dry-run"]),
        step(["grade", PREDICTION, "--result", PREDICTION]),
        step(["grade", MISSING, "--result", RESULT]),
        step(["grade", PREDICTION, "--result", RESULT, "--calculation-version", "prediction-grade-v1", "--yes"]),
        step(["records", "list", "--type", "prediction_grade"]),
      ],
    },
    {
      name: "provider_import_new_database",
      fixture: "none",
      provider: { "provider-data": "wearable_csv" },
      steps: [
        step(["init", "--yes", "--no-skill"]),
        step(["import", "provider-data", "--json"]),
        step(["import", "provider-data"]),
        step(["import", "provider-data", "--yes", "--json"]),
        step(["import", "provider-data", "--yes"]),
        step(["import", "provider-data"]),
        step(["import", "provider-data", "--yes"]),
        step(["stats"]),
        step(["import", "missing-folder"]),
      ],
    },
    {
      name: "provider_import_existing_records",
      fixture: "demo",
      current: true,
      provider: { "provider-data": "wearable_csv" },
      steps: [step(["import", "provider-data"]), step(["import", "provider-data", "--yes"]), step(["check"])],
    },
  ],
};

// ---------------------------------------------------------------------------
// Normalization. `native/tests/cli_golden.rs` applies the same rules.

const NORMALIZATION = [
  "The scenario's working folder is replaced with <work>.",
  "The program versions 2.1.0 and 2.1.1 are replaced with <version> (D152, D208).",
  'SQL schema steps 4 and 5 in "sql_schema_version" are replaced with <step>; each program writes its own current step.',
  "UTC times with milliseconds, and the compact forms in backup and report names, that fall within the scenario's run are replaced with <now>.",
  "UUIDv7 values whose time falls within the scenario's run are new IDs. Each is replaced with <id-N>, numbered by first appearance in the scenario.",
  "Native output only: migration step 5 (migrations/005-version-2-1-1.sql) is removed from planned steps, and a source, target, or required step 5 reads 4.",
];

const ISO = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z/g;
const COMPACT = /\d{8}T\d{6}\.\d{3}Z/g;
const REPORT = /\d{4}-\d{2}-\d{2}T\d{9}Z/g;
const UUID7 = /[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}/g;

interface Window {
  from: number;
  to: number;
  ids: Map<string, string>;
}

function within(window: Window, millis: number): boolean {
  return Number.isFinite(millis) && millis >= window.from && millis <= window.to;
}

function normalize(text: string, work: string, window: Window): string {
  let out = text.split(work).join("<work>");
  out = out.split("2.1.0").join("<version>").split("2.1.1").join("<version>");
  out = out.replace(/"sql_schema_version": [45]\b/g, '"sql_schema_version": <step>');
  out = out.replace(ISO, (match) => (within(window, Date.parse(match)) ? "<now>" : match));
  out = out.replace(COMPACT, (match) => {
    const iso = `${match.slice(0, 4)}-${match.slice(4, 6)}-${match.slice(6, 8)}T${match.slice(9, 11)}:${match.slice(11, 13)}:${match.slice(13)}`;
    return within(window, Date.parse(iso)) ? "<now>" : match;
  });
  out = out.replace(REPORT, (match) => {
    const iso = `${match.slice(0, 13)}:${match.slice(13, 15)}:${match.slice(15, 17)}.${match.slice(17, 20)}Z`;
    return within(window, Date.parse(iso)) ? "<now>" : match;
  });
  out = out.replace(UUID7, (match) => {
    const millis = parseInt(match.slice(0, 8) + match.slice(9, 13), 16);
    if (!within(window, millis)) return match;
    let name = window.ids.get(match);
    if (name === undefined) {
      name = `<id-${window.ids.size + 1}>`;
      window.ids.set(match, name);
    }
    return name;
  });
  return out;
}

// ---------------------------------------------------------------------------
// The local model server.

const CATALOG = {
  data: [
    { id: "test/steady", name: "Steady", reasoning: true },
    { id: "test/down", reasoning: true },
    { id: "test/embedder", type: "embedding", reasoning: true },
  ],
};

const STEADY_REPLY = JSON.stringify({
  predicted_score: { duration_s: 290 },
  predicted_range: { low: { duration_s: 280 }, high: { duration_s: 300 } },
  confidence: "moderate",
  reasoning: "Anchored on the latest result.",
});

function respond(response: ServerResponse, status: number, body: unknown): void {
  response.writeHead(status, { "content-type": "application/json" });
  response.end(JSON.stringify(body));
}

function handle(request: IncomingMessage, response: ServerResponse): void {
  const chunks: Buffer[] = [];
  request.on("data", (chunk: Buffer) => chunks.push(chunk));
  request.on("end", () => {
    if (request.method === "GET" && request.url === "/models") return respond(response, 200, CATALOG);
    if (request.method === "POST" && request.url === "/chat/completions") {
      const body = JSON.parse(Buffer.concat(chunks).toString("utf8")) as { model: string };
      if (body.model === "test/steady") {
        return respond(response, 200, { model: body.model, choices: [{ message: { content: STEADY_REPLY } }] });
      }
      return respond(response, 503, { error: { message: "the test model is down" } });
    }
    respond(response, 404, { error: "not found" });
  });
}

// ---------------------------------------------------------------------------
// Running the old program.

const FETCH_PRELOAD = `
const root = process.env.ATH_GATEWAY_BASE_URL;
if (root) {
  const original = globalThis.fetch;
  const published = ["https://ai-gateway.vercel.sh/v1", "https://openrouter.ai/api/v1"];
  globalThis.fetch = (input, init) => {
    let url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
    for (const base of published) if (url.startsWith(base)) url = root + url.slice(base.length);
    return original(url, init);
  };
}
`;

/** A fake password store: nothing is saved, and nothing can be saved. */
const FAKE_STORE = "#!/bin/sh\necho 'the test password store is read-only' >&2\nexit 44\n";

interface Run {
  status: number;
  stdout: string;
  stderr: string;
}

function runOld(argv: string[], cwd: string, env: Record<string, string>, stdin: string, preload: string): Promise<Run> {
  const loader = pathToFileURL(join(reference, "node_modules", "tsx", "dist", "loader.mjs")).href;
  const child = spawn(
    process.execPath,
    ["--import", loader, "--import", pathToFileURL(preload).href, join(reference, "src", "cli.ts"), ...argv],
    { cwd, env, stdio: ["pipe", "pipe", "pipe"] },
  );
  const stdout: Buffer[] = [];
  const stderr: Buffer[] = [];
  child.stdout.on("data", (chunk: Buffer) => stdout.push(chunk));
  child.stderr.on("data", (chunk: Buffer) => stderr.push(chunk));
  child.stdin.end(stdin);
  return new Promise((resolvePromise, reject) => {
    child.on("error", reject);
    child.on("close", (code) =>
      resolvePromise({
        status: code ?? 1,
        stdout: Buffer.concat(stdout).toString("utf8"),
        stderr: Buffer.concat(stderr).toString("utf8"),
      }),
    );
  });
}

async function migrateToCurrent(path: string, backupFolder: string): Promise<void> {
  const db = await import(pathToFileURL(join(reference, "src", "db", "index.ts")).href);
  const database = await db.openDatabase(path, { allowOutdatedSchema: true, deferWal: true });
  try {
    const metadata = await db.readDatabaseMetadata(database);
    if (metadata.sql_schema_version < 4) {
      await db.executeConfirmedDatabaseMigration(database, { backupPath: join(backupFolder, "backup.sqlite") });
    }
  } finally {
    await db.releaseLocalDatabase(database);
  }
}

function prepare(scenario: Scenario, work: string): void {
  for (const folder of scenario.dirs ?? []) mkdirSync(join(work, folder), { recursive: true });
  for (const [name, key] of Object.entries(scenario.provider ?? {})) {
    cpSync(join(reference, PROVIDER_FILES[key]), join(work, name), { recursive: true });
  }
  if (scenario.fixture === "none") return;
  // Owner-only, as `ath init` creates it, so no permission warning is printed.
  copyFileSync(join(reference, FIXTURES[scenario.fixture]), join(work, "athlete.ath.sqlite"));
  chmodSync(join(work, "athlete.ath.sqlite"), 0o600);
  const attachments = ATTACHMENTS[scenario.fixture];
  if (attachments !== undefined) cpSync(join(reference, attachments), join(work, "attachments"), { recursive: true });
}

function applySql(path: string, statements: string[]): void {
  const database = new DatabaseSync(path);
  try {
    database.exec("PRAGMA foreign_keys = OFF");
    for (const statement of statements) database.exec(statement);
  } finally {
    database.close();
  }
}

async function main(): Promise<void> {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "ath-cli-golden-")));
  chmodSync(root, 0o700);
  const bin = join(root, "bin");
  mkdirSync(bin, { mode: 0o700 });
  for (const tool of ["security", "secret-tool"]) {
    writeFileSync(join(bin, tool), FAKE_STORE, { mode: 0o700 });
  }
  const preload = join(root, "fetch-preload.mjs");
  writeFileSync(preload, FETCH_PRELOAD);
  const server = createServer(handle);
  await new Promise<void>((resolvePromise) => server.listen(0, "127.0.0.1", resolvePromise));
  const address = server.address();
  if (address === null || typeof address === "string") throw new Error("no server address");
  const gateway = `http://127.0.0.1:${address.port}`;
  mkdirSync(outDir, { recursive: true });
  try {
    for (const [group, scenarios] of Object.entries(GROUPS)) {
      for (const scenario of scenarios) {
        const work = join(root, "work", scenario.name);
        const home = join(root, "home", scenario.name);
        mkdirSync(work, { recursive: true, mode: 0o700 });
        mkdirSync(home, { recursive: true, mode: 0o700 });
        const window: Window = { from: Date.now() - 5_000, to: 0, ids: new Map() };
        prepare(scenario, work);
        const database = join(work, "athlete.ath.sqlite");
        if (scenario.current) await migrateToCurrent(database, home);
        if (scenario.sql !== undefined) applySql(database, scenario.sql);
        const runs: Run[] = [];
        for (const item of scenario.steps) {
          const env = {
            PATH: `${bin}:/usr/bin:/bin`,
            HOME: home,
            TMPDIR: home,
            TZ: "America/Chicago",
            ATH_GATEWAY_BASE_URL: gateway,
            ...(item.env ?? {}),
          };
          runs.push(await runOld(item.argv, work, env, item.stdin ?? "", preload));
        }
        window.to = Date.now() + 5_000;
        scenario.steps.forEach((item, index) => {
          const run = runs[index]!;
          item.status = run.status;
          item.stdout = normalize(run.stdout, work, window);
          item.stderr = normalize(run.stderr, work, window);
        });
        process.stderr.write(`${group}/${scenario.name}: ${runs.map((run) => run.status).join(" ")}\n`);
      }
      const document = {
        generated_by: "native/tools/reference/cli-golden.ts",
        reference_version: "2.1.0",
        normalization: NORMALIZATION,
        environment: {
          PATH: "<fake password-store tools>:/usr/bin:/bin",
          TZ: "America/Chicago",
          ATH_GATEWAY_BASE_URL: "<local test server>",
        },
        scenarios,
      };
      writeFileSync(join(outDir, `${group}.json`), `${JSON.stringify(document, null, 2)}\n`);
    }
  } finally {
    server.close();
    rmSync(root, { recursive: true, force: true });
  }
}

await main();
