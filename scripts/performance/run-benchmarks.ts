import { execFileSync } from "node:child_process";
import {
  mkdir,
  mkdtemp,
  readFile,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { arch, cpus, platform, release, tmpdir, totalmem } from "node:os";
import { join, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { pathToFileURL } from "node:url";
import {
  AthleticStandardPackage,
  ATHLETIC_STANDARD_VERSION,
} from "../../src/v1/schema.js";
import {
  buildEvidence,
  calculate,
  checkDatabase,
  createDatabase,
  deterministicJson,
  exportRecords,
  importRecords,
  loadAthleticStandardPackage,
  openDatabase,
  previewPackageImport,
  querySeries,
  type AthleticDatabase,
} from "../../src/db/index.js";
import { databaseClient } from "../../src/db/client.js";
import { PERFORMANCE_FIXTURE_SEED } from "./generate-database.js";

const SOURCE_A = "01890f3e-e7d3-7cc0-88c8-000000000003";
const SOURCE_B = "01890f3e-e7d3-7cc0-88c8-000000000064";
const WORKOUT_SESSION = "01890f3e-e7d3-7cc0-88c8-00000000000a";
const DELTA_RECORD_ID = "01890f3e-e7d3-7cc0-88c8-0000fffffff0";
const DELTA_TIME = "2026-10-14T00:00:00.000Z";

interface Distribution {
  min: number;
  p50: number;
  p95: number;
  p99: number;
  max: number;
  mean: number;
}

interface OperationResult {
  id: string;
  api: string;
  parameters: Record<string, unknown>;
  repetitions: number;
  samples_ms: number[];
  summary_ms: Distribution | null;
  memory: {
    peak_heap_used_delta_bytes: number;
    peak_rss_delta_bytes: number;
  };
  success: boolean;
  error: string | null;
}

export interface PerformanceReport {
  report_version: 1;
  performance_seed: typeof PERFORMANCE_FIXTURE_SEED;
  target_record_count: number;
  measured_at: string;
  counts: Record<string, number>;
  database_files: Record<string, number | string>;
  environment: Record<string, string | number>;
  sqlite: Record<string, string | number>;
  cache_reset: {
    profile: "warm_same_connection";
    method: string;
    limitations: string;
  };
  read_repetitions: number;
  operations: OperationResult[];
  query_plans: Record<string, unknown[]>;
  notes: string[];
}

interface BenchmarkOptions {
  database_path: string;
  output_path: string;
  read_repetitions: number;
  keep_work: boolean;
  measured_at: string;
}

function round(value: number, places = 3): number {
  const scale = 10 ** places;
  return Math.round(value * scale) / scale;
}

function percentile(sorted: readonly number[], fraction: number): number {
  const index = Math.max(
    0,
    Math.min(sorted.length - 1, Math.ceil(sorted.length * fraction) - 1),
  );
  return sorted[index]!;
}

function distribution(samples: readonly number[]): Distribution | null {
  if (samples.length === 0) return null;
  const sorted = [...samples].sort((left, right) => left - right);
  return {
    min: round(sorted[0]!),
    p50: round(percentile(sorted, 0.5)),
    p95: round(percentile(sorted, 0.95)),
    p99: round(percentile(sorted, 0.99)),
    max: round(sorted.at(-1)!),
    mean: round(
      sorted.reduce((total, sample) => total + sample, 0) / sorted.length,
    ),
  };
}

async function measureOperation(
  id: string,
  api: string,
  parameters: Record<string, unknown>,
  repetitions: number,
  operation: () => Promise<void>,
  options: { warmup?: boolean } = {},
): Promise<OperationResult> {
  const samples: number[] = [];
  let peakHeapDelta = 0;
  let peakRssDelta = 0;
  let error: string | null = null;
  try {
    if (options.warmup) {
      await operation();
    }
    for (let iteration = 0; iteration < repetitions; iteration++) {
      const before = process.memoryUsage();
      let peakHeap = before.heapUsed;
      let peakRss = before.rss;
      const sampler = setInterval(() => {
        const current = process.memoryUsage();
        peakHeap = Math.max(peakHeap, current.heapUsed);
        peakRss = Math.max(peakRss, current.rss);
      }, 50);
      sampler.unref();
      const started = performance.now();
      try {
        await operation();
      } finally {
        clearInterval(sampler);
      }
      samples.push(performance.now() - started);
      const after = process.memoryUsage();
      peakHeap = Math.max(peakHeap, after.heapUsed);
      peakRss = Math.max(peakRss, after.rss);
      peakHeapDelta = Math.max(peakHeapDelta, peakHeap - before.heapUsed);
      peakRssDelta = Math.max(peakRssDelta, peakRss - before.rss);
    }
  } catch (caught) {
    error = caught instanceof Error ? caught.message : String(caught);
  }
  return {
    id,
    api,
    parameters,
    repetitions,
    samples_ms: samples.map((sample) => round(sample)),
    summary_ms: distribution(samples),
    memory: {
      peak_heap_used_delta_bytes: peakHeapDelta,
      peak_rss_delta_bytes: peakRssDelta,
    },
    success: error === null && samples.length === repetitions,
    error,
  };
}

async function pragmaValue(
  database: AthleticDatabase,
  pragma: string,
): Promise<string | number> {
  const result = await databaseClient(database).execute(`PRAGMA ${pragma}`);
  const value = Object.values(result.rows[0] ?? {})[0];
  return typeof value === "number" ? value : String(value);
}

async function fileBytes(path: string): Promise<number> {
  try {
    return (await stat(path)).size;
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      return 0;
    }
    throw error;
  }
}

async function queryPlan(
  database: AthleticDatabase,
  sql: string,
  args: Array<string | number>,
): Promise<unknown[]> {
  const result = await databaseClient(database).execute({
    sql: `EXPLAIN QUERY PLAN ${sql}`,
    args,
  });
  return result.rows;
}

async function writeIncrementalPackage(
  directory: string,
  databaseId: string,
  databaseRevision: number,
): Promise<void> {
  await rm(directory, { recursive: true, force: true });
  await mkdir(directory, { recursive: true, mode: 0o700 });
  const data = {
    report_type: "note" as const,
    reported_at: DELTA_TIME,
    input_reported_at: {
      timestamp_text: DELTA_TIME,
      utc_offset_text: "Z",
      utc_offset_meaning: "documented_recording_offset" as const,
      timestamp_basis: "source_instant" as const,
    },
    reported_via: "text" as const,
    note: "Deterministic incremental-import performance record",
  };
  const packageValue = AthleticStandardPackage.parse({
    athleticstandard_version: ATHLETIC_STANDARD_VERSION,
    database_id: databaseId,
    database_revision: databaseRevision + 1,
    records: [
      {
        record_id: DELTA_RECORD_ID,
        record_type: "athlete_report",
        created_at: DELTA_TIME,
        current_revision: 1,
        is_deleted: false,
        data,
      },
    ],
    record_revisions: [
      {
        record_id: DELTA_RECORD_ID,
        record_type: "athlete_report",
        revision: 1,
        previous_revision: null,
        became_known_at: DELTA_TIME,
        became_known_at_basis: "package_merge",
        changed_by: {
          author_type: "tool",
          tool: "ath-performance-runner",
          tool_version: ATHLETIC_STANDARD_VERSION,
        },
        data,
      },
    ],
    record_deletions: [],
    record_erasure_markers: [],
    record_import_keys: [],
    measurement_series_files: [],
  });
  await writeFile(
    join(directory, "athlete.ath.json"),
    deterministicJson(packageValue),
    { mode: 0o600 },
  );
}

function gitSha(): string {
  try {
    return execFileSync("git", ["rev-parse", "HEAD"], {
      encoding: "utf8",
    }).trim();
  } catch {
    return "unavailable";
  }
}

async function clientVersion(): Promise<string> {
  try {
    const value = JSON.parse(
      await readFile(
        resolve("node_modules/@libsql/client/package.json"),
        "utf8",
      ),
    ) as { version?: string };
    return value.version ?? "unavailable";
  } catch {
    return "unavailable";
  }
}

export async function runPerformanceBenchmarks(
  options: BenchmarkOptions,
): Promise<PerformanceReport> {
  if (
    !Number.isInteger(options.read_repetitions) ||
    options.read_repetitions < 1
  ) {
    throw new Error("read repetitions must be a positive whole number");
  }
  const databasePath = resolve(options.database_path);
  const database = await openDatabase(databasePath, {
    onSecurityWarning: () => undefined,
  });
  const work = await mkdtemp(join(tmpdir(), "ath-v1-performance-run-"));
  const packageDirectory = join(work, "full-package");
  const restorePath = join(work, "restored.ath.sqlite");
  const deltaDirectory = join(work, "delta-package");
  let restored: AthleticDatabase | undefined;
  try {
    const client = databaseClient(database);
    const countsResult = await client.execute(`
      SELECT
        (SELECT COUNT(*) FROM records) AS records_total,
        (SELECT COUNT(*) FROM records WHERE is_deleted = 0) AS records_current,
        (SELECT COUNT(*) FROM records WHERE is_deleted = 1) AS records_deleted,
        (SELECT COUNT(*) FROM record_revisions) AS record_revisions,
        (SELECT COUNT(*) FROM measurement_samples) AS measurement_samples
    `);
    const countRow = countsResult.rows[0]!;
    const counts = Object.fromEntries(
      Object.entries(countRow).map(([key, value]) => [key, Number(value)]),
    );
    const metadata = await client.execute(`
      SELECT database_id FROM database_metadata WHERE singleton_id = 1
    `);
    const state = await client.execute(`
      SELECT current_revision FROM database_state WHERE singleton_id = 1
    `);
    const databaseId = String(metadata.rows[0]?.database_id);
    const databaseRevision = Number(state.rows[0]?.current_revision);
    const lastTimestamp = await client.execute({
      sql: `
        SELECT MAX(recorded_at) AS recorded_at
        FROM measurement_samples
        WHERE quantity = 'hrv_sdnn' AND source_id = ?
      `,
      args: [SOURCE_A],
    });
    const to = String(lastTimestamp.rows[0]?.recorded_at);
    if (!to || to === "null") {
      throw new Error("performance database has no HRV samples");
    }
    const from = new Date(Date.parse(to) - 90 * 86_400_000).toISOString();

    const operations: OperationResult[] = [];
    operations.push(
      await measureOperation(
        "latest_value",
        "querySeries",
        { quantity: "hrv_sdnn", source_id: SOURCE_A, order: "descending" },
        options.read_repetitions,
        async () => {
          const records = await querySeries(database, {
            quantity: "hrv_sdnn",
            source_id: SOURCE_A,
            order: "descending",
            limit: 1,
          });
          if (records.length !== 1) throw new Error("latest value was not found");
        },
        { warmup: true },
      ),
    );
    operations.push(
      await measureOperation(
        "evidence_28d",
        "buildEvidence",
        { benchmark: "fran", as_of: "2026-10-13T23:59:59.999Z", window_days: 28 },
        options.read_repetitions,
        async () => {
          await buildEvidence(database, {
            benchmark: "fran",
            as_of: "2026-10-13T23:59:59.999Z",
            window_days: 28,
          });
        },
        { warmup: true },
      ),
    );
    operations.push(
      await measureOperation(
        "baseline_90d",
        "calculate",
        { operation: "baseline", quantity: "hrv_sdnn", source_id: SOURCE_A, from, to },
        options.read_repetitions,
        async () => {
          await calculate(database, {
            operation: "baseline",
            quantity: "hrv_sdnn",
            source_id: SOURCE_A,
            from,
            to,
          });
        },
        { warmup: true },
      ),
    );
    operations.push(
      await measureOperation(
        "workout_drift",
        "calculate",
        {
          operation: "workout-drift",
          quantity: "heart_rate",
          workout_session_id: WORKOUT_SESSION,
          source_id: SOURCE_A,
        },
        options.read_repetitions,
        async () => {
          await calculate(database, {
            operation: "workout-drift",
            quantity: "heart_rate",
            workout_session_id: WORKOUT_SESSION,
            source_id: SOURCE_A,
          });
        },
        { warmup: true },
      ),
    );
    operations.push(
      await measureOperation(
        "cross_source_comparison",
        "calculate",
        {
          operation: "compare-sources",
          quantity: "hrv_sdnn",
          source_id: SOURCE_A,
          compare_source_id: SOURCE_B,
          from,
          to,
        },
        options.read_repetitions,
        async () => {
          await calculate(database, {
            operation: "compare-sources",
            quantity: "hrv_sdnn",
            source_id: SOURCE_A,
            compare_source_id: SOURCE_B,
            from,
            to,
          });
        },
        { warmup: true },
      ),
    );
    operations.push(
      await measureOperation(
        "full_json_export",
        "exportRecords",
        { output_directory: packageDirectory },
        1,
        async () => {
          await rm(packageDirectory, { recursive: true, force: true });
          await exportRecords(database, packageDirectory);
        },
      ),
    );
    operations.push(
      await measureOperation(
        "database_check",
        "checkDatabase",
        {},
        1,
        async () => {
          const checked = await checkDatabase(database);
          if (!checked.valid) {
            throw new Error(
              `database check reported ${checked.issues.length} issue(s)`,
            );
          }
        },
      ),
    );
    operations.push(
      await measureOperation(
        "initial_import",
        "loadAthleticStandardPackage + previewPackageImport + importRecords",
        { source: packageDirectory },
        1,
        async () => {
          restored?.close();
          restored = undefined;
          await Promise.all(
            [restorePath, `${restorePath}-wal`, `${restorePath}-shm`].map(
              (path) => rm(path, { force: true }),
            ),
          );
          const destination = await createDatabase(restorePath, {
            createdAt: DELTA_TIME,
          });
          try {
            const loaded = await loadAthleticStandardPackage(packageDirectory);
            const preview = await previewPackageImport(destination, loaded);
            if (!preview.restore || preview.conflicts.length > 0) {
              throw new Error("initial import did not produce a clean restore");
            }
            await importRecords(destination, preview);
            restored = destination;
          } catch (error) {
            destination.close();
            throw error;
          }
        },
      ),
    );
    await writeIncrementalPackage(
      deltaDirectory,
      databaseId,
      databaseRevision,
    );
    operations.push(
      await measureOperation(
        "incremental_import",
        "loadAthleticStandardPackage + previewPackageImport + importRecords",
        { source: deltaDirectory, added_records: 1 },
        1,
        async () => {
          if (restored === undefined) {
            throw new Error("initial import did not leave a restored database");
          }
          const loaded = await loadAthleticStandardPackage(deltaDirectory);
          const preview = await previewPackageImport(restored, loaded);
          if (
            preview.new_record_count !== 1 ||
            preview.conflicts.length > 0
          ) {
            throw new Error("incremental import did not find one clean new record");
          }
          await importRecords(restored, preview);
        },
      ),
    );

    const queryPlans = {
      latest_value: await queryPlan(
        database,
        `
          SELECT record_id
          FROM measurement_samples
          WHERE quantity = ? AND source_id = ?
          ORDER BY recorded_at DESC, record_id DESC
          LIMIT 1
        `,
        ["hrv_sdnn", SOURCE_A],
      ),
      baseline_90d: await queryPlan(
        database,
        `
          SELECT record_id, standard_value
          FROM measurement_samples
          WHERE quantity = ? AND source_id = ?
            AND recorded_at >= ? AND recorded_at <= ?
          ORDER BY recorded_at, record_id
          LIMIT 100001
        `,
        ["hrv_sdnn", SOURCE_A, from, to],
      ),
      evidence_as_of: await queryPlan(
        database,
        `
          WITH visible_revisions AS (
            SELECT
              candidate_records.record_id,
              (
                SELECT MAX(candidate_revisions.revision)
                FROM record_revisions AS candidate_revisions
                WHERE candidate_revisions.record_id = candidate_records.record_id
                  AND candidate_revisions.became_known_at <= ?
              ) AS revision
            FROM records AS candidate_records
            WHERE candidate_records.record_type IN (?, ?, ?, ?, ?, ?, ?, ?)
          )
          SELECT visible_revisions.record_id
          FROM visible_revisions
          JOIN records ON records.record_id = visible_revisions.record_id
          ORDER BY records.created_at, records.record_id
          LIMIT 100000
        `,
        [
          "2026-10-13T23:59:59.999Z",
          "benchmark_definition",
          "source",
          "point_measurement",
          "athlete_report",
          "sleep_session",
          "workout_session",
          "benchmark_result",
          "prediction",
        ],
      ),
    };
    const cpuList = cpus();
    const report: PerformanceReport = {
      report_version: 1,
      performance_seed: PERFORMANCE_FIXTURE_SEED,
      target_record_count: Number(counts.records_total),
      measured_at: options.measured_at,
      counts,
      database_files: {
        database_path: databasePath,
        database_bytes: await fileBytes(databasePath),
        wal_bytes: await fileBytes(`${databasePath}-wal`),
        shm_bytes: await fileBytes(`${databasePath}-shm`),
      },
      environment: {
        git_sha: gitSha(),
        node_version: process.version,
        platform: platform(),
        platform_release: release(),
        arch: arch(),
        cpu_model: cpuList[0]?.model ?? "unavailable",
        logical_cpus: cpuList.length,
        total_memory_bytes: totalmem(),
        libsql_client_version: await clientVersion(),
      },
      sqlite: {
        journal_mode: await pragmaValue(database, "journal_mode"),
        foreign_keys: await pragmaValue(database, "foreign_keys"),
        page_size: await pragmaValue(database, "page_size"),
        page_count: await pragmaValue(database, "page_count"),
        cache_size: await pragmaValue(database, "cache_size"),
        mmap_size: await pragmaValue(database, "mmap_size"),
        synchronous: await pragmaValue(database, "synchronous"),
        busy_timeout: await pragmaValue(database, "busy_timeout"),
      },
      cache_reset: {
        profile: "warm_same_connection",
        method:
          "one discarded warmup, then repeated reads through one open database connection",
        limitations:
          "The run does not clear the SQLite page cache or operating-system file cache.",
      },
      read_repetitions: options.read_repetitions,
      operations,
      query_plans: queryPlans,
      notes: [
        "Import, export, and database-check operations run once because they process the complete fixture.",
        "The 28-day evidence measurement uses buildEvidence window_days=28.",
        "The incremental package contains one deterministic athlete report and no omitted record deletes local data.",
      ],
    };
    await mkdir(resolve(options.output_path, ".."), {
      recursive: true,
      mode: 0o700,
    });
    await writeFile(
      resolve(options.output_path),
      `${JSON.stringify(report, null, 2)}\n`,
      { mode: 0o600 },
    );
    return report;
  } finally {
    restored?.close();
    database.close();
    if (!options.keep_work) {
      await rm(work, { recursive: true, force: true });
    }
  }
}

function commandArgument(name: string): string | undefined {
  const index = process.argv.indexOf(name);
  return index === -1 ? undefined : process.argv[index + 1];
}

async function main(): Promise<void> {
  const database = commandArgument("--database");
  const output = commandArgument("--output");
  const repetitions = Number(commandArgument("--repetitions") ?? "3");
  const measuredAt = commandArgument("--measured-at");
  if (database === undefined || output === undefined || measuredAt === undefined) {
    throw new Error(
      "usage: run-benchmarks.ts --database <file> --output <json> " +
        "--measured-at <UTC timestamp> [--repetitions <count>] [--keep-work]",
    );
  }
  const report = await runPerformanceBenchmarks({
    database_path: database,
    output_path: output,
    read_repetitions: repetitions,
    keep_work: process.argv.includes("--keep-work"),
    measured_at: measuredAt,
  });
  console.log(JSON.stringify({
    output: resolve(output),
    target_record_count: report.target_record_count,
    operations: report.operations.map((operation) => ({
      id: operation.id,
      success: operation.success,
      p95_ms: operation.summary_ms?.p95 ?? null,
      error: operation.error,
    })),
  }, null, 2));
  if (report.operations.some((operation) => !operation.success)) {
    process.exitCode = 1;
  }
}

if (
  process.argv[1] !== undefined &&
  pathToFileURL(resolve(process.argv[1])).href === import.meta.url
) {
  main().catch((error: unknown) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
