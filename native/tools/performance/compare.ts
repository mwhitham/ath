/**
 * End-to-end command comparison of the 2.1.0 reference and the native 2.1.1
 * executable on the same generated datasets.
 *
 * Each measurement starts a new process, so process startup is included. The
 * old command is `node <reference>/dist/cli.js`, the published entry point.
 * The native command is the release executable. Rust compilation is never
 * part of a measurement.
 *
 * Every run is wrapped in `/usr/bin/time -l`, which reports the peak resident
 * set of the command. A sampler also reads `ps` for the whole process tree,
 * so a helper process (the 2.1.0 `ath-core`) is counted together with its
 * parent. Each run is one JSON line in `<root>/<label>/results/`.
 *
 * Usage (Node 22.18 or later runs this file directly):
 *
 *   node compare.ts prepare --label 100k --records 100000
 *   node compare.ts bulk    --label 100k --runs 3
 *   node compare.ts queries --label 100k --runs 50 --memory-runs 5
 *   node compare.ts scaling --label 300k --runs 1 --only native
 *   node compare.ts library --label 100k --runs 3 --repetitions 50
 *   node compare.ts large-record --label large
 *   node compare.ts summary --label 100k
 *
 * Common options: --root <folder> (default $PERF_ROOT or /private/tmp/ath-perf),
 * --reference <2.1.0 checkout> (default $ATH_REFERENCE), --native <ath>
 * (default native/target/release/ath beside this file).
 */
import { execFile, spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  appendFileSync,
  closeSync,
  copyFileSync,
  createReadStream,
  existsSync,
  mkdirSync,
  openSync,
  readdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { cpus, loadavg, totalmem } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SOURCE_A = "01890f3e-e7d3-7cc0-88c8-000000000003";
const SOURCE_B = "01890f3e-e7d3-7cc0-88c8-000000000064";
const WORKOUT_SESSION = "01890f3e-e7d3-7cc0-88c8-00000000000a";
const EVIDENCE_AS_OF = "2026-10-13T23:59:59.999Z";
const DAY_MS = 86_400_000;

type Implementation = "old" | "native";
const IMPLEMENTATIONS: Implementation[] = ["old", "native"];

interface Args {
  command: string;
  values: Map<string, string>;
  flags: Set<string>;
}

function parseArgs(argv: string[]): Args {
  const [command = "help", ...rest] = argv;
  const values = new Map<string, string>();
  const flags = new Set<string>();
  for (let index = 0; index < rest.length; index++) {
    const item = rest[index]!;
    if (!item.startsWith("--")) throw new Error(`unexpected argument ${item}`);
    const next = rest[index + 1];
    if (next === undefined || next.startsWith("--")) {
      flags.add(item.slice(2));
    } else {
      values.set(item.slice(2), next);
      index++;
    }
  }
  return { command, values, flags };
}

const here = dirname(fileURLToPath(import.meta.url));
const args = parseArgs(process.argv.slice(2));
const root = resolve(args.values.get("root") ?? process.env.PERF_ROOT ?? "/private/tmp/ath-perf");
const reference = resolve(args.values.get("reference") ?? process.env.ATH_REFERENCE ?? "");
const nativeBinary = resolve(args.values.get("native") ?? join(here, "../../target/release/ath"));
const libraryBinary = resolve(
  args.values.get("native-library") ?? join(here, "../../target/release/examples/performance-library"),
);
const label = args.values.get("label") ?? "";
const dataset = join(root, label);

function number(name: string, fallback: number): number {
  const text = args.values.get(name);
  if (text === undefined) return fallback;
  const value = Number(text.replaceAll("_", ""));
  if (!Number.isSafeInteger(value) || value < 0) throw new Error(`--${name} must be a whole number`);
  return value;
}

function implementations(): Implementation[] {
  const only = args.values.get("only");
  if (only === undefined) return IMPLEMENTATIONS;
  if (only !== "old" && only !== "native") throw new Error("--only must be old or native");
  return [only];
}

function command(implementation: Implementation): string[] {
  return implementation === "old"
    ? [process.execPath, join(reference, "dist/cli.js")]
    : [nativeBinary];
}

function databasePath(implementation: Implementation): string {
  return join(dataset, implementation, "athlete.ath.sqlite");
}

function privateDirectory(path: string): string {
  mkdirSync(path, { recursive: true, mode: 0o700 });
  return path;
}

function run(program: string, programArgs: string[], options: { cwd?: string; env?: NodeJS.ProcessEnv } = {}): string {
  const result = spawnSync(program, programArgs, {
    cwd: options.cwd,
    env: options.env ?? process.env,
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`${program} ${programArgs.join(" ")} failed: ${result.stderr || result.stdout}`);
  }
  return result.stdout;
}

function sql(database: string, query: string): string {
  return run("sqlite3", [database, query]).trim();
}

// ---- Hashing ----------------------------------------------------------------

async function fileSha256(path: string): Promise<string> {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk as Buffer);
  return hash.digest("hex");
}

function listFiles(directory: string): string[] {
  const out: string[] = [];
  const walk = (folder: string): void => {
    for (const entry of readdirSync(folder, { withFileTypes: true })) {
      const path = join(folder, entry.name);
      if (entry.isDirectory()) walk(path);
      else out.push(path);
    }
  };
  walk(directory);
  return out.sort();
}

interface PackageFacts {
  file_count: number;
  bytes: number;
  /** SHA-256 over sorted "<relative path> <sha256> <bytes>" lines. */
  package_sha256: string;
  manifest_sha256: string | null;
  main_document_sha256: string;
}

async function packageFacts(directory: string): Promise<PackageFacts> {
  const lines: string[] = [];
  let bytes = 0;
  let manifest: string | null = null;
  let main = "";
  for (const path of listFiles(directory)) {
    const name = relative(directory, path);
    const size = statSync(path).size;
    const sha = await fileSha256(path);
    bytes += size;
    lines.push(`${name} ${sha} ${size}`);
    if (name === "export-manifest.json") manifest = sha;
    if (name === "athlete.ath.json") main = sha;
  }
  return {
    file_count: lines.length,
    bytes,
    package_sha256: createHash("sha256").update(lines.join("\n")).digest("hex"),
    manifest_sha256: manifest,
    main_document_sha256: main,
  };
}

/**
 * Compare two packages line by line after removing what differs by design:
 * the format version text and the file hashes that depend on it. Byte counts
 * are compared as written: "2.1.0" and "2.1.1" have the same length.
 */
async function normalizedPackagesEqual(left: string, right: string): Promise<{ equal: boolean; detail: string }> {
  const leftFiles = listFiles(left).map((path) => relative(left, path));
  const rightFiles = listFiles(right).map((path) => relative(right, path));
  if (leftFiles.join("\n") !== rightFiles.join("\n")) {
    return { equal: false, detail: "file lists differ" };
  }
  for (const name of leftFiles) {
    const result = spawnSync(
      "python3",
      [
        "-I",
        "-c",
        `
import re, sys
pattern = [(re.compile(rb'"athleticstandard_version": "2\\.1\\.[01]"'), b'"athleticstandard_version": "<version>"'),
           (re.compile(rb'"sha256": "[0-9a-f]{64}"'), b'"sha256": "<sha256>"')]
def norm(line):
    for regex, text in pattern:
        line = regex.sub(text, line)
    return line
with open(sys.argv[1], "rb") as a, open(sys.argv[2], "rb") as b:
    number = 0
    for x, y in zip(a, b):
        number += 1
        if norm(x) != norm(y):
            print(f"line {number}"); sys.exit(1)
    if a.read(1) or b.read(1):
        print("length"); sys.exit(1)
`,
        join(left, name),
        join(right, name),
      ],
      { encoding: "utf8" },
    );
    if (result.status !== 0) {
      return { equal: false, detail: `${name}: ${result.stdout.trim() || result.stderr.trim()}` };
    }
  }
  return {
    equal: true,
    detail: `${leftFiles.length} files equal after replacing the version text and file hashes`,
  };
}

// ---- One measured command ------------------------------------------------------

interface TimeReport {
  real_s: number | null;
  user_s: number | null;
  sys_s: number | null;
  max_rss_bytes: number | null;
  peak_footprint_bytes: number | null;
}

function parseTime(stderr: string): TimeReport {
  const value = (pattern: RegExp): number | null => {
    const match = stderr.match(pattern);
    return match ? Number(match[1]) : null;
  };
  return {
    real_s: value(/([\d.]+) real/),
    user_s: value(/([\d.]+) user/),
    sys_s: value(/([\d.]+) sys/),
    max_rss_bytes: value(/(\d+)\s+maximum resident set size/),
    peak_footprint_bytes: value(/(\d+)\s+peak memory footprint/),
  };
}

interface Measured {
  kind: string;
  label: string;
  implementation: Implementation;
  operation: string;
  run: number;
  warmup: boolean;
  started_at: string;
  loadavg_before: number[];
  argv: string[];
  exit_code: number | null;
  wall_ms: number;
  time: TimeReport;
  tree: { sample_interval_ms: number; samples: number; peak_rss_bytes: number; processes: string[] } | null;
  disk: { files: string[]; peak_bytes: number; samples: number } | null;
  stdout_bytes: number;
  stdout_sha256: string;
  stderr_tail: string;
  extra: Record<string, unknown>;
}

interface MeasureOptions {
  kind: string;
  implementation: Implementation;
  operation: string;
  run: number;
  warmup?: boolean;
  args: string[];
  cwd: string;
  sampleMs?: number;
  diskFiles?: string[];
}

function psTree(): Promise<Array<{ pid: number; ppid: number; rss: number; comm: string }>> {
  return new Promise((resolvePs) => {
    execFile("ps", ["-A", "-o", "pid=,ppid=,rss=,comm="], { maxBuffer: 16 * 1024 * 1024 }, (error, stdout) => {
      if (error) return resolvePs([]);
      const rows = stdout
        .split("\n")
        .map((line) => line.trim().match(/^(\d+)\s+(\d+)\s+(\d+)\s+(.*)$/))
        .filter((match): match is RegExpMatchArray => match !== null)
        .map((match) => ({
          pid: Number(match[1]),
          ppid: Number(match[2]),
          rss: Number(match[3]) * 1024,
          comm: match[4]!.split("/").at(-1)!,
        }));
      resolvePs(rows);
    });
  });
}

function diskBytes(files: string[]): number {
  let total = 0;
  for (const file of files) {
    try {
      total += statSync(file).size;
    } catch {
      // Not created yet, or already removed.
    }
  }
  return total;
}

async function measure(options: MeasureOptions): Promise<Measured> {
  const argv = [...command(options.implementation), ...options.args];
  const scratch = privateDirectory(join(dataset, "work", "stdout"));
  const stdoutPath = join(scratch, `${options.implementation}-${options.operation}.out`);
  const stdoutFd = openSync(stdoutPath, "w", 0o600);
  const loadBefore = loadavg().map((value) => Math.round(value * 100) / 100);
  const startedAt = new Date().toISOString();
  const started = process.hrtime.bigint();
  const child = spawn("/usr/bin/time", ["-l", ...argv], {
    cwd: options.cwd,
    stdio: ["ignore", stdoutFd, "pipe"],
    env: { ...process.env, NO_COLOR: "1" },
  });
  closeSync(stdoutFd);
  let stderr = "";
  child.stderr!.setEncoding("utf8");
  child.stderr!.on("data", (chunk: string) => {
    stderr += chunk;
    if (stderr.length > 1_000_000) stderr = stderr.slice(-200_000);
  });

  let treePeak = 0;
  let treeSamples = 0;
  const processNames = new Set<string>();
  let diskPeak = 0;
  let diskSamples = 0;
  let sampling = false;
  let finished = false;
  const sampleOnce = async (): Promise<void> => {
    if (sampling || finished) return;
    sampling = true;
    try {
      if (options.diskFiles) {
        diskPeak = Math.max(diskPeak, diskBytes(options.diskFiles));
        diskSamples++;
      }
      const rows = await psTree();
      const children = new Map<number, number[]>();
      for (const row of rows) {
        const list = children.get(row.ppid) ?? [];
        list.push(row.pid);
        children.set(row.ppid, list);
      }
      const byPid = new Map(rows.map((row) => [row.pid, row]));
      let total = 0;
      const stack = [...(children.get(child.pid!) ?? [])];
      while (stack.length > 0) {
        const pid = stack.pop()!;
        const row = byPid.get(pid);
        if (row === undefined) continue;
        total += row.rss;
        processNames.add(row.comm);
        stack.push(...(children.get(pid) ?? []));
      }
      if (!finished) {
        treePeak = Math.max(treePeak, total);
        treeSamples++;
      }
    } finally {
      sampling = false;
    }
  };
  const timer =
    options.sampleMs !== undefined
      ? setInterval(() => {
          void sampleOnce();
        }, options.sampleMs)
      : undefined;
  const exitCode = await new Promise<number | null>((resolveExit) => {
    child.on("exit", (code) => {
      finished = true;
      resolveExit(code);
    });
  });
  const wallMs = Number(process.hrtime.bigint() - started) / 1e6;
  if (timer !== undefined) clearInterval(timer);
  if (options.diskFiles) diskPeak = Math.max(diskPeak, diskBytes(options.diskFiles));
  await new Promise<void>((resolveClose) => {
    if (child.stderr!.readableEnded) resolveClose();
    else child.stderr!.on("end", () => resolveClose());
  });
  const stdoutBytes = statSync(stdoutPath).size;
  const stdoutSha = await fileSha256(stdoutPath);
  const time = parseTime(stderr);
  const tailStart = stderr.lastIndexOf(" real ");
  const commandStderr = (tailStart > 0 ? stderr.slice(0, stderr.lastIndexOf("\n", tailStart)) : stderr).trim();
  return {
    kind: options.kind,
    label,
    implementation: options.implementation,
    operation: options.operation,
    run: options.run,
    warmup: options.warmup ?? false,
    started_at: startedAt,
    loadavg_before: loadBefore,
    argv,
    exit_code: exitCode,
    wall_ms: Math.round(wallMs * 1000) / 1000,
    time,
    tree:
      options.sampleMs === undefined
        ? null
        : {
            sample_interval_ms: options.sampleMs,
            samples: treeSamples,
            peak_rss_bytes: treePeak,
            processes: [...processNames].sort(),
          },
    disk: options.diskFiles ? { files: options.diskFiles, peak_bytes: diskPeak, samples: diskSamples } : null,
    stdout_bytes: stdoutBytes,
    stdout_sha256: stdoutSha,
    stderr_tail: commandStderr.slice(-2000),
    extra: {},
  };
}

function resultsFile(kind: string): string {
  const folder = privateDirectory(join(dataset, "results"));
  return join(folder, `${kind}.jsonl`);
}

function record(kind: string, measured: Measured): void {
  appendFileSync(resultsFile(kind), `${JSON.stringify(measured)}\n`, { mode: 0o600 });
  const memory = measured.time.max_rss_bytes === null ? "?" : `${(measured.time.max_rss_bytes / 1048576).toFixed(1)} MiB`;
  const tree = measured.tree === null ? "" : ` tree ${(measured.tree.peak_rss_bytes / 1048576).toFixed(1)} MiB`;
  console.log(
    `${measured.label} ${measured.operation} ${measured.implementation} run ${measured.run}${measured.warmup ? " (warmup)" : ""}: ` +
      `${measured.wall_ms.toFixed(1)} ms, rss ${memory}${tree}, exit ${measured.exit_code}`,
  );
  if (measured.exit_code !== 0) {
    console.log(`  stderr: ${measured.stderr_tail.slice(-500)}`);
  }
}

// ---- prepare ----------------------------------------------------------------------

interface DatasetFacts {
  label: string;
  record_count: number;
  database_id: string;
  database_revision: number;
  to: string;
  from: string;
  counts: Record<string, number>;
  old_database_bytes: number;
  native_database_bytes: number;
  input_package: PackageFacts;
  delta_package_sha256: string;
}

function readFacts(): DatasetFacts {
  return JSON.parse(readFileSync(join(dataset, "dataset.json"), "utf8")) as DatasetFacts;
}

function removeDatabase(path: string): void {
  for (const suffix of ["", "-wal", "-shm"]) rmSync(`${path}${suffix}`, { force: true });
}

function cloneFile(from: string, to: string): void {
  const result = spawnSync("cp", ["-c", from, to]);
  if (result.status !== 0) copyFileSync(from, to);
}

async function prepare(): Promise<void> {
  const records = number("records", 0);
  const oldDirectory = join(dataset, "old");
  const oldDatabase = databasePath("old");
  if (!existsSync(oldDatabase)) {
    if (records === 0) throw new Error("--records is required to generate a dataset");
    privateDirectory(dataset);
    console.log(`generating ${records} records with the 2.1.0 generator`);
    run(
      process.execPath,
      ["--import", "tsx", "scripts/performance/generate-database.ts", "--records", String(records), "--output", oldDirectory],
      { cwd: reference },
    );
  }

  const nativeDirectory = join(dataset, "native");
  rmSync(nativeDirectory, { recursive: true, force: true });
  privateDirectory(nativeDirectory);
  cloneFile(oldDatabase, databasePath("native"));
  privateDirectory(join(nativeDirectory, "attachments"));
  for (const name of readdirSync(join(oldDirectory, "attachments"))) {
    copyFileSync(join(oldDirectory, "attachments", name), join(nativeDirectory, "attachments", name));
  }
  const databaseId = sql(oldDatabase, "SELECT database_id FROM database_metadata");
  console.log(`migrating the native copy with ${nativeBinary}`);
  run(nativeBinary, ["migrate", "--file", databasePath("native"), "-y", "--confirm-migration", databaseId], {
    cwd: nativeDirectory,
  });
  for (const name of readdirSync(nativeDirectory)) {
    if (name.includes(".migration-backup.")) rmSync(join(nativeDirectory, name), { force: true });
  }

  const inputPackage = join(dataset, "input-package");
  rmSync(inputPackage, { recursive: true, force: true });
  console.log("exporting the shared import input with the 2.1.0 command");
  run(process.execPath, [join(reference, "dist/cli.js"), "export", inputPackage, "--file", oldDatabase], {
    cwd: dataset,
  });

  const revision = Number(sql(oldDatabase, "SELECT current_revision FROM database_state"));
  const deltaPackage = join(dataset, "delta-package");
  rmSync(deltaPackage, { recursive: true, force: true });
  privateDirectory(deltaPackage);
  // The 2.1.0 runner's one-record package, written by the 2.1.0 library itself.
  const script = `
    import { writeFile } from "node:fs/promises";
    import { AthleticStandardPackage, ATHLETIC_STANDARD_VERSION } from "./src/v1/schema.ts";
    import { deterministicJson } from "./src/db/index.ts";
    const DELTA_RECORD_ID = "01890f3e-e7d3-7cc0-88c8-0000fffffff0";
    const DELTA_TIME = "2026-10-14T00:00:00.000Z";
    const data = {
      report_type: "note", reported_at: DELTA_TIME,
      input_reported_at: { timestamp_text: DELTA_TIME, utc_offset_text: "Z",
        utc_offset_meaning: "documented_recording_offset", timestamp_basis: "source_instant" },
      reported_via: "text", note: "Deterministic incremental-import performance record",
    };
    const value = AthleticStandardPackage.parse({
      athleticstandard_version: ATHLETIC_STANDARD_VERSION,
      database_id: ${JSON.stringify(databaseId)},
      database_revision: ${revision + 1},
      records: [{ record_id: DELTA_RECORD_ID, record_type: "athlete_report", created_at: DELTA_TIME,
        current_revision: 1, is_deleted: false, data }],
      record_revisions: [{ record_id: DELTA_RECORD_ID, record_type: "athlete_report", revision: 1,
        previous_revision: null, became_known_at: DELTA_TIME, became_known_at_basis: "package_merge",
        changed_by: { author_type: "tool", tool: "ath-performance-runner", tool_version: ATHLETIC_STANDARD_VERSION },
        data }],
      record_deletions: [], record_erasure_markers: [], record_import_keys: [], measurement_series_files: [],
    });
    await writeFile(${JSON.stringify(join(deltaPackage, "athlete.ath.json"))}, deterministicJson(value), { mode: 0o600 });
  `;
  run(process.execPath, ["--import", "tsx", "--input-type=module", "-e", script], { cwd: reference });

  const to = sql(
    oldDatabase,
    `SELECT MAX(recorded_at) FROM measurement_samples WHERE quantity = 'hrv_sdnn' AND source_id = '${SOURCE_A}'`,
  );
  const from = new Date(Date.parse(to) - 90 * DAY_MS).toISOString();
  const countText = sql(
    oldDatabase,
    `SELECT (SELECT COUNT(*) FROM records), (SELECT COUNT(*) FROM records WHERE is_deleted = 0),
            (SELECT COUNT(*) FROM records WHERE is_deleted = 1), (SELECT COUNT(*) FROM record_revisions),
            (SELECT COUNT(*) FROM measurement_samples)`,
  ).split("|");
  const counts = {
    records_total: Number(countText[0]),
    records_current: Number(countText[1]),
    records_deleted: Number(countText[2]),
    record_revisions: Number(countText[3]),
    measurement_samples: Number(countText[4]),
  };
  const facts: DatasetFacts = {
    label,
    record_count: counts.records_total,
    database_id: databaseId,
    database_revision: revision,
    to,
    from,
    counts,
    old_database_bytes: statSync(oldDatabase).size,
    native_database_bytes: statSync(databasePath("native")).size,
    input_package: await packageFacts(inputPackage),
    delta_package_sha256: await fileSha256(join(deltaPackage, "athlete.ath.json")),
  };
  writeFileSync(join(dataset, "dataset.json"), `${JSON.stringify(facts, null, 2)}\n`, { mode: 0o600 });
  console.log(JSON.stringify(facts, null, 2));
}

// ---- bulk ---------------------------------------------------------------------------

function orderFor(runNumber: number): Implementation[] {
  const chosen = implementations();
  return runNumber % 2 === 1 ? chosen : [...chosen].reverse();
}

async function bulk(kind = "bulk"): Promise<void> {
  const runs = number("runs", 3);
  const sampleMs = number("sample-ms", 250);
  const operations = new Set((args.values.get("operations") ?? "export,check,import").split(","));
  const keep = !args.flags.has("no-keep");
  const facts = readFacts();
  const work = privateDirectory(join(dataset, "work"));
  const kept: Partial<Record<Implementation, string>> = {};
  const firstRun = number("first-run", 1);
  for (let runNumber = firstRun; runNumber < firstRun + runs; runNumber++) {
    if (operations.has("export")) {
      for (const implementation of orderFor(runNumber)) {
        const destination = join(work, `export-${implementation}-${runNumber}`);
        rmSync(destination, { recursive: true, force: true });
        const measured = await measure({
          kind,
          implementation,
          operation: "full_json_export",
          run: runNumber,
          args: ["export", destination, "--file", databasePath(implementation)],
          cwd: work,
          sampleMs,
        });
        if (measured.exit_code === 0) {
          measured.extra.package = await packageFacts(destination);
        }
        record(kind, measured);
        if (runNumber === 1 && keep && measured.exit_code === 0) {
          const target = join(work, `export-${implementation}-kept`);
          rmSync(target, { recursive: true, force: true });
          renameSync(destination, target);
          kept[implementation] = target;
        } else {
          rmSync(destination, { recursive: true, force: true });
        }
      }
    }
    if (operations.has("check")) {
      for (const implementation of orderFor(runNumber)) {
        record(
          kind,
          await measure({
            kind,
            implementation,
            operation: "database_check",
            run: runNumber,
            args: ["check", databasePath(implementation), "--json"],
            cwd: work,
            sampleMs,
          }),
        );
      }
    }
    if (operations.has("import")) {
      for (const implementation of orderFor(runNumber)) {
        const restored = join(work, `restored-${implementation}-${runNumber}.ath.sqlite`);
        removeDatabase(restored);
        record(
          kind,
          await measure({
            kind,
            implementation,
            operation: "init_empty",
            run: runNumber,
            args: ["init", "-y", "--empty", "--no-skill", "--file", restored],
            cwd: work,
          }),
        );
        const initial = await measure({
          kind,
          implementation,
          operation: "initial_import",
          run: runNumber,
          args: ["import", join(dataset, "input-package"), "--yes", "--file", restored],
          cwd: work,
          sampleMs,
          diskFiles: [restored, `${restored}-wal`, `${restored}-shm`],
        });
        initial.extra.restored_database_bytes = existsSync(restored) ? statSync(restored).size : null;
        record(kind, initial);
        const incremental = await measure({
          kind,
          implementation,
          operation: "incremental_import",
          run: runNumber,
          args: ["import", join(dataset, "delta-package"), "--yes", "--file", restored],
          cwd: work,
          sampleMs: Math.min(sampleMs, 20),
        });
        record(kind, incremental);
        if (runNumber === 1 && initial.exit_code === 0) {
          const content = restoredContent(restored);
          const summary: Measured = { ...initial, operation: "restored_content", extra: content };
          appendFileSync(resultsFile(`${kind}-content`), `${JSON.stringify(summary)}\n`, { mode: 0o600 });
          console.log(`  restored content ${implementation}: ${JSON.stringify(content)}`);
        }
        removeDatabase(restored);
      }
    }
  }
  if (kept.old && kept.native) {
    const compared = await normalizedPackagesEqual(kept.old, kept.native);
    const oldFacts = await packageFacts(kept.old);
    const inputMatches = oldFacts.package_sha256 === facts.input_package.package_sha256;
    const line = { label, kind: `${kind}-export-comparison`, ...compared, old_export_equals_input_package: inputMatches };
    appendFileSync(resultsFile(`${kind}-content`), `${JSON.stringify(line)}\n`, { mode: 0o600 });
    console.log(`  export comparison: ${JSON.stringify(line)}`);
  }
  if (!args.flags.has("keep-exports")) {
    for (const path of Object.values(kept)) rmSync(path!, { recursive: true, force: true });
  }
}

/** Counts and a content hash of the restored records, independent of SQLite page layout. */
function restoredContent(database: string): Record<string, unknown> {
  const hashQuery = (query: string): string => {
    const result = spawnSync("sh", ["-c", `sqlite3 "$1" "$2" | shasum -a 256`, "sh", database, query], {
      encoding: "utf8",
      maxBuffer: 1024 * 1024,
    });
    return result.stdout.trim().split(/\s+/)[0] ?? "";
  };
  return {
    records: Number(sql(database, "SELECT COUNT(*) FROM records")),
    record_revisions: Number(sql(database, "SELECT COUNT(*) FROM record_revisions")),
    measurement_samples: Number(sql(database, "SELECT COUNT(*) FROM measurement_samples")),
    database_revision: Number(sql(database, "SELECT current_revision FROM database_state")),
    records_sha256: hashQuery(
      "SELECT record_id, record_type, current_revision, created_at, is_deleted FROM records ORDER BY record_id",
    ),
    revision_keys_sha256: hashQuery(
      "SELECT record_id, revision, record_type, became_known_at, became_known_at_basis, previous_revision FROM record_revisions ORDER BY record_id, revision",
    ),
    revision_json_sha256: hashQuery(
      "SELECT record_id, revision, record_json, changed_by_json FROM record_revisions ORDER BY record_id, revision",
    ),
    sample_columns_sha256: hashQuery(
      "SELECT record_id, source_id, quantity, recorded_at, standard_value, standard_unit FROM measurement_samples ORDER BY record_id",
    ),
  };
}

// ---- queries -------------------------------------------------------------------------

function queryArguments(implementation: Implementation, facts: DatasetFacts): Array<[string, string[]]> {
  const database = databasePath(implementation);
  return [
    ["help", ["--help"]],
    ["version", ["--version"]],
    ["stats", ["stats", "--json", "--file", database]],
    [
      "latest_value",
      ["series", "hrv_sdnn", "--raw", "--source", SOURCE_A, "--from", facts.to, "--to", facts.to, "--json", "--file", database],
    ],
    ["evidence", ["predict", "fran", "--json", "--as-of", EVIDENCE_AS_OF, "--file", database]],
    [
      "baseline_90d",
      ["calculate", "baseline", "hrv_sdnn", "--source", SOURCE_A, "--from", facts.from, "--to", facts.to, "--json", "--file", database],
    ],
    [
      "workout_drift",
      ["calculate", "workout-drift", "heart_rate", "--session", WORKOUT_SESSION, "--source", SOURCE_A, "--json", "--file", database],
    ],
    [
      "cross_source_comparison",
      [
        "calculate",
        "compare-sources",
        "hrv_sdnn",
        "--source",
        SOURCE_A,
        "--compare-source",
        SOURCE_B,
        "--from",
        facts.from,
        "--to",
        facts.to,
        "--json",
        "--file",
        database,
      ],
    ],
  ];
}

async function queries(): Promise<void> {
  const runs = number("runs", 50);
  const memoryRuns = number("memory-runs", 5);
  const facts = readFacts();
  const cwd = privateDirectory(join(dataset, "work", "empty-cwd"));
  const selected = args.values.get("operations")?.split(",");
  const table = new Map(
    implementations().map((implementation) => [
      implementation,
      queryArguments(implementation, facts).filter(([operation]) => selected === undefined || selected.includes(operation)),
    ]),
  );
  const operations = table.get(implementations()[0]!)!.map(([operation]) => operation);
  // The first run of each command, kept apart as the first-start measurement.
  for (const operation of operations) {
    for (const implementation of implementations()) {
      const commandArgs = table.get(implementation)!.find(([name]) => name === operation)![1];
      record("queries", await measure({ kind: "queries", implementation, operation, run: 0, warmup: true, args: commandArgs, cwd }));
    }
  }
  for (let runNumber = 1; runNumber <= runs; runNumber++) {
    for (const operation of operations) {
      for (const implementation of orderFor(runNumber)) {
        const commandArgs = table.get(implementation)!.find(([name]) => name === operation)![1];
        record("queries", await measure({ kind: "queries", implementation, operation, run: runNumber, args: commandArgs, cwd }));
      }
    }
  }
  // Separate memory runs with a fast process-tree sampler. Their times are not used.
  for (let runNumber = 1; runNumber <= memoryRuns; runNumber++) {
    for (const operation of operations) {
      for (const implementation of orderFor(runNumber)) {
        const commandArgs = table.get(implementation)!.find(([name]) => name === operation)![1];
        record(
          "queries-memory",
          await measure({ kind: "queries-memory", implementation, operation, run: runNumber, args: commandArgs, cwd, sampleMs: 5 }),
        );
      }
    }
  }
}

// ---- library --------------------------------------------------------------------------

async function library(): Promise<void> {
  const runs = number("runs", 3);
  const repetitions = number("repetitions", 50);
  const outputDirectory = privateDirectory(join(dataset, "results", "library"));
  const work = privateDirectory(join(dataset, "work", "library"));
  for (let runNumber = 1; runNumber <= runs; runNumber++) {
    for (const implementation of orderFor(runNumber)) {
      const output = join(outputDirectory, `${implementation}-${runNumber}.json`);
      const started = Date.now();
      if (implementation === "old") {
        run(
          process.execPath,
          [
            "--expose-gc",
            "--import",
            "tsx",
            "scripts/performance/run-benchmarks.ts",
            "--database",
            databasePath("old"),
            "--output",
            output,
            "--measured-at",
            new Date().toISOString(),
            "--repetitions",
            String(repetitions),
          ],
          { cwd: reference, env: { ...process.env, TMPDIR: work } },
        );
      } else {
        const runWork = join(work, `native-${runNumber}`);
        rmSync(runWork, { recursive: true, force: true });
        run(libraryBinary, [
          "--database",
          databasePath("native"),
          "--package",
          join(dataset, "input-package"),
          "--delta",
          join(dataset, "delta-package"),
          "--work",
          runWork,
          "--output",
          output,
          "--repetitions",
          String(repetitions),
          "--bulk-repetitions",
          "1",
          ...(runNumber === 1 && !args.flags.has("no-plans") ? ["--plans"] : []),
        ]);
        rmSync(runWork, { recursive: true, force: true });
      }
      console.log(`${label} library ${implementation} run ${runNumber}: ${((Date.now() - started) / 1000).toFixed(1)} s`);
    }
  }
}

// ---- large record ----------------------------------------------------------------------

/**
 * One valid workout session with a long route and dense heart-rate samples,
 * added to the demo package. The base is the 2.1.0 demo fixture exported by
 * the 2.1.0 command.
 */
async function largeRecord(): Promise<void> {
  const points = number("points", 200_000);
  privateDirectory(dataset);
  const base = join(dataset, "base-package");
  rmSync(base, { recursive: true, force: true });
  run(
    process.execPath,
    [join(reference, "dist/cli.js"), "export", base, "--file", join(reference, "examples/demo-athlete/athlete.ath.sqlite")],
    { cwd: dataset },
  );
  const document = JSON.parse(readFileSync(join(base, "athlete.ath.json"), "utf8")) as Record<string, unknown> & {
    records: Array<Record<string, unknown>>;
    record_revisions: Array<Record<string, unknown>>;
    database_revision: number;
  };
  const start = Date.parse("2026-07-01T05:00:00.000Z");
  const stamp = (offsetMs: number): string => new Date(start + offsetMs).toISOString();
  const end = stamp((points - 1) * 1000);
  const timestamp = (text: string) => ({
    timestamp_text: text,
    utc_offset_text: "Z",
    utc_offset_meaning: "documented_recording_offset",
    timestamp_basis: "source_instant",
  });
  const route = [];
  const heartRate = [];
  for (let index = 0; index < points; index++) {
    const at = stamp(index * 1000);
    route.push({
      recorded_at: at,
      latitude_deg: Math.round((45 + Math.sin(index / 5000) * 0.5) * 1e6) / 1e6,
      longitude_deg: Math.round((-110 + Math.cos(index / 5000) * 0.5) * 1e6) / 1e6,
    });
    heartRate.push({ offset_ms: index * 1000, bpm: 100 + (index % 60) });
  }
  const recordId = "01890f3e-e7d3-7cc0-88c8-0000ffffff01";
  const data = {
    source_id: SOURCE_A,
    received_at: "2026-07-04T00:00:00.000Z",
    received_at_basis: "import_time",
    start_at: stamp(0),
    end_at: end,
    input_start_timestamp: timestamp(stamp(0)),
    input_end_timestamp: timestamp(end),
    activity_type: "running",
    name: "Synthetic long route",
    heart_rate_samples: heartRate,
    route,
    source_reported_summaries: [],
  };
  document.records.push({
    record_id: recordId,
    record_type: "workout_session",
    created_at: "2026-07-04T00:00:00.000Z",
    current_revision: 1,
    is_deleted: false,
    data,
  });
  document.record_revisions.push({
    record_id: recordId,
    record_type: "workout_session",
    revision: 1,
    previous_revision: null,
    became_known_at: "2026-07-04T00:00:00.000Z",
    became_known_at_basis: "provider_import",
    changed_by: { author_type: "tool", tool: "ath-performance-generator", tool_version: "1.0.0" },
    data,
  });
  document.database_revision += 1;
  const packageDirectory = join(dataset, "input-package");
  rmSync(packageDirectory, { recursive: true, force: true });
  privateDirectory(packageDirectory);
  for (const entry of readdirSync(base)) {
    if (entry === "athlete.ath.json" || entry === "export-manifest.json") continue;
    spawnSync("cp", ["-R", join(base, entry), join(packageDirectory, entry)]);
  }
  // Without a manifest, the package is read as written: both versions accept that.
  writeFileSync(join(packageDirectory, "athlete.ath.json"), `${JSON.stringify(sortKeys(document), null, 2)}\n`, {
    mode: 0o600,
  });
  const recordBytes = Buffer.byteLength(JSON.stringify(data));
  console.log(`large workout session: ${points} route points and heart-rate samples, ${recordBytes} bytes of record JSON`);
  writeFileSync(
    join(dataset, "large-record.json"),
    `${JSON.stringify({ points, record_id: recordId, record_json_bytes: recordBytes }, null, 2)}\n`,
    { mode: 0o600 },
  );
  rmSync(base, { recursive: true, force: true });

  const runs = number("runs", 3);
  const work = privateDirectory(join(dataset, "work"));
  for (let runNumber = 1; runNumber <= runs; runNumber++) {
    for (const implementation of orderFor(runNumber)) {
      const restored = join(work, `restored-${implementation}.ath.sqlite`);
      removeDatabase(restored);
      run(command(implementation)[0]!, [...command(implementation).slice(1), "init", "-y", "--empty", "--no-skill", "--file", restored], {
        cwd: work,
      });
      record(
        "large",
        await measure({
          kind: "large",
          implementation,
          operation: "initial_import",
          run: runNumber,
          args: ["import", packageDirectory, "--yes", "--file", restored],
          cwd: work,
          sampleMs: 100,
          diskFiles: [restored, `${restored}-wal`, `${restored}-shm`],
        }),
      );
      record(
        "large",
        await measure({
          kind: "large",
          implementation,
          operation: "database_check",
          run: runNumber,
          args: ["check", restored, "--json"],
          cwd: work,
          sampleMs: 100,
        }),
      );
      const exported = join(work, `export-${implementation}`);
      rmSync(exported, { recursive: true, force: true });
      const measured = await measure({
        kind: "large",
        implementation,
        operation: "full_json_export",
        run: runNumber,
        args: ["export", exported, "--file", restored],
        cwd: work,
        sampleMs: 100,
      });
      if (measured.exit_code === 0) measured.extra.package = await packageFacts(exported);
      record("large", measured);
      rmSync(exported, { recursive: true, force: true });
      removeDatabase(restored);
    }
  }
}

function sortKeys(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sortKeys);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value as Record<string, unknown>)
        .sort()
        .map((key) => [key, sortKeys((value as Record<string, unknown>)[key])]),
    );
  }
  return value;
}

// ---- summary -------------------------------------------------------------------------------

function percentile(sorted: number[], fraction: number): number {
  const index = Math.max(0, Math.min(sorted.length - 1, Math.ceil(sorted.length * fraction) - 1));
  return sorted[index]!;
}

function summarizeRuns(rows: Measured[]): Record<string, unknown> {
  const times = rows.map((row) => row.wall_ms).sort((left, right) => left - right);
  const rss = rows.map((row) => row.time.max_rss_bytes ?? 0);
  const footprint = rows.map((row) => row.time.peak_footprint_bytes ?? 0);
  const tree = rows.map((row) => row.tree?.peak_rss_bytes ?? 0);
  const disk = rows.map((row) => row.disk?.peak_bytes ?? 0);
  return {
    n: times.length,
    failures: rows.filter((row) => row.exit_code !== 0).length,
    min_ms: times[0],
    p50_ms: percentile(times, 0.5),
    p95_ms: percentile(times, 0.95),
    p99_ms: percentile(times, 0.99),
    max_ms: times.at(-1),
    mean_ms: Math.round((times.reduce((sum, value) => sum + value, 0) / times.length) * 1000) / 1000,
    spread_ms: Math.round(((times.at(-1) ?? 0) - (times[0] ?? 0)) * 1000) / 1000,
    max_rss_bytes: Math.max(...rss),
    peak_footprint_bytes: Math.max(...footprint),
    tree_peak_rss_bytes: Math.max(...tree),
    peak_disk_bytes: Math.max(...disk),
    processes: [...new Set(rows.flatMap((row) => row.tree?.processes ?? []))].sort(),
    stdout_bytes: [...new Set(rows.map((row) => row.stdout_bytes))],
    stdout_sha256: [...new Set(rows.map((row) => row.stdout_sha256))],
    package_bytes: [...new Set(rows.map((row) => (row.extra.package as PackageFacts | undefined)?.bytes).filter((value) => value !== undefined))],
    package_sha256: [
      ...new Set(rows.map((row) => (row.extra.package as PackageFacts | undefined)?.package_sha256).filter((value) => value !== undefined)),
    ],
    load_average_1m: [Math.min(...rows.map((row) => row.loadavg_before[0]!)), Math.max(...rows.map((row) => row.loadavg_before[0]!))],
    samples_ms: rows.map((row) => row.wall_ms),
  };
}

function summary(): void {
  const folder = join(dataset, "results");
  const out: Record<string, unknown> = { label };
  for (const file of readdirSync(folder).filter((name) => name.endsWith(".jsonl") && !name.includes("content"))) {
    const kind = file.replace(/\.jsonl$/, "");
    const rows = readFileSync(join(folder, file), "utf8")
      .split("\n")
      .filter((line) => line.trim() !== "")
      .map((line) => JSON.parse(line) as Measured);
    const groups = new Map<string, Measured[]>();
    for (const row of rows) {
      const key = `${row.operation}|${row.implementation}|${row.warmup ? "first" : "measured"}`;
      groups.set(key, [...(groups.get(key) ?? []), row]);
    }
    const kindSummary: Record<string, unknown> = {};
    for (const [key, group] of [...groups.entries()].sort()) kindSummary[key] = summarizeRuns(group);
    out[kind] = kindSummary;
  }
  const target = join(folder, "summary.json");
  writeFileSync(target, `${JSON.stringify(out, null, 2)}\n`, { mode: 0o600 });
  console.log(target);
}

// ---- environment ----------------------------------------------------------------------------

function environment(): void {
  const info: Record<string, unknown> = {
    cpu_model: cpus()[0]?.model,
    logical_cpus: cpus().length,
    total_memory_bytes: totalmem(),
    node_version: process.version,
    sw_vers: spawnSync("sw_vers", { encoding: "utf8" }).stdout.trim(),
    uname: spawnSync("uname", ["-a"], { encoding: "utf8" }).stdout.trim(),
    native_version: run(nativeBinary, ["--version"]).trim(),
    old_version: run(process.execPath, [join(reference, "dist/cli.js"), "--version"]).trim(),
    native_binary_sha256: createHash("sha256").update(readFileSync(nativeBinary)).digest("hex"),
    native_binary_bytes: statSync(nativeBinary).size,
    sqlite3_cli: spawnSync("sqlite3", ["--version"], { encoding: "utf8" }).stdout.trim(),
  };
  console.log(JSON.stringify(info, null, 2));
}

const commands: Record<string, () => Promise<void> | void> = {
  prepare,
  bulk: () => bulk("bulk"),
  scaling: () => bulk("scaling"),
  queries,
  library,
  "large-record": largeRecord,
  summary,
  environment,
};

const selected = commands[args.command];
if (selected === undefined) {
  console.error(`commands: ${Object.keys(commands).join(", ")}`);
  process.exitCode = 2;
} else {
  if (args.command !== "environment" && label === "") throw new Error("--label is required");
  if (reference === "" || !existsSync(join(reference, "dist/cli.js"))) {
    throw new Error("--reference must name a 2.1.0 checkout with dist/cli.js (or set ATH_REFERENCE)");
  }
  await selected();
}
