import {
  chmod,
  cp,
  copyFile,
  mkdir,
  readdir,
  rm,
} from "node:fs/promises";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import {
  openDatabase,
  writeRecords,
} from "../../src/db/index.js";
import { databaseClient } from "../../src/db/client.js";
import { withWriteTransaction } from "../../src/db/transaction.js";

export const PERFORMANCE_FIXTURE_SEED = "ath-v1-performance-seed-2";
export const RELEASE_RECORD_COUNTS = [
  100_000,
  3_000_000,
  30_000_000,
] as const;

const FIXTURE_DIRECTORY = resolve("examples/demo-athlete");
const FIXTURE_DATABASE = join(FIXTURE_DIRECTORY, "athlete.ath.sqlite");
const CREATED_AT = "2026-10-13T00:00:00.000Z";
const FIRST_BULK_SEQUENCE = 1_000_000;

const IDS = {
  sourceA: "01890f3e-e7d3-7cc0-88c8-000000000003",
  sourceB: "01890f3e-e7d3-7cc0-88c8-000000000064",
  sourceC: "01890f3e-e7d3-7cc0-88c8-000000000065",
} as const;

const TOOL_ACTOR = {
  author_type: "tool" as const,
  tool: "ath-performance-generator",
  tool_version: "1.0.0",
};

export interface GeneratePerformanceDatabaseOptions {
  output_directory: string;
  record_count: number;
}

export interface GeneratedPerformanceDatabase {
  database_path: string;
  record_count: number;
  sample_count: number;
  database_revision: number;
  seed: typeof PERFORMANCE_FIXTURE_SEED;
}

function assertRecordCount(recordCount: number): void {
  if (
    !Number.isSafeInteger(recordCount) ||
    recordCount < 100 ||
    recordCount > 30_000_000
  ) {
    throw new Error(
      "record count must be a whole number from 100 through 30000000",
    );
  }
}

async function copyBaseFixture(outputDirectory: string): Promise<string> {
  await rm(outputDirectory, { recursive: true, force: true });
  await mkdir(outputDirectory, { recursive: true, mode: 0o700 });
  const databasePath = join(outputDirectory, "athlete.ath.sqlite");
  await copyFile(FIXTURE_DATABASE, databasePath);
  await chmod(databasePath, 0o600);
  const attachmentDirectory = join(outputDirectory, "attachments");
  await cp(
    join(FIXTURE_DIRECTORY, "attachments"),
    attachmentDirectory,
    { recursive: true },
  );
  await chmod(attachmentDirectory, 0o700);
  for (const file of await readdir(attachmentDirectory)) {
    await chmod(join(attachmentDirectory, file), 0o600);
  }
  return databasePath;
}

async function addPerformanceSources(
  database: Awaited<ReturnType<typeof openDatabase>>,
): Promise<void> {
  await writeRecords(database, [
    {
      record_id: IDS.sourceB,
      record_type: "source",
      created_at: CREATED_AT,
      became_known_at: CREATED_AT,
      became_known_at_basis: "provider_import",
      changed_by: TOOL_ACTOR,
      data: {
        source_type: "wearable",
        data_writer: "Performance Watch B",
        data_route: "performance_fixture",
        vendor: "athletic-standard",
        sensor_type: "optical",
      },
    },
    {
      record_id: IDS.sourceC,
      record_type: "source",
      created_at: CREATED_AT,
      became_known_at: CREATED_AT,
      became_known_at_basis: "provider_import",
      changed_by: TOOL_ACTOR,
      data: {
        source_type: "wearable",
        data_writer: "Performance Watch C",
        data_route: "performance_fixture",
        vendor: "athletic-standard",
        sensor_type: "optical",
      },
    },
  ]);
}

const SAMPLE_DATA_SQL = `
  json_object(
    'source_id',
      CASE performance_numbers.n % 3
        WHEN 0 THEN '${IDS.sourceA}'
        WHEN 1 THEN '${IDS.sourceB}'
        ELSE '${IDS.sourceC}'
      END,
    'received_at', '${CREATED_AT}',
    'received_at_basis', 'import_time',
    'quantity',
      CASE
        WHEN performance_numbers.n < 7560 THEN 'heart_rate'
        WHEN (performance_numbers.n / 3) % 6 = 0 THEN 'heart_rate'
        WHEN (performance_numbers.n / 3) % 6 = 1 THEN 'steps'
        WHEN (performance_numbers.n / 3) % 6 = 2 THEN 'active_energy'
        WHEN (performance_numbers.n / 3) % 6 = 3 THEN 'hrv_sdnn'
        WHEN (performance_numbers.n / 3) % 6 = 4 THEN 'respiratory_rate'
        ELSE 'distance_walking_running'
      END,
    'recorded_at',
      CASE
        WHEN performance_numbers.n < 7560
          THEN strftime(
            '%Y-%m-%dT%H:%M:%fZ',
            1786382700 + CAST(performance_numbers.n / 3 AS INTEGER),
            'unixepoch'
          )
        ELSE strftime(
          '%Y-%m-%dT%H:%M:%fZ',
          1735689600 +
            CAST((performance_numbers.n - 7560) / 3 AS INTEGER) * 30,
          'unixepoch'
        )
      END,
    'input_timestamp',
      json_object(
        'timestamp_text',
          CASE
            WHEN performance_numbers.n < 7560
              THEN strftime(
                '%Y-%m-%dT%H:%M:%fZ',
                1786382700 + CAST(performance_numbers.n / 3 AS INTEGER),
                'unixepoch'
              )
            ELSE strftime(
              '%Y-%m-%dT%H:%M:%fZ',
              1735689600 +
                CAST((performance_numbers.n - 7560) / 3 AS INTEGER) * 30,
              'unixepoch'
            )
          END,
        'utc_offset_text', 'Z',
        'utc_offset_meaning', 'documented_recording_offset',
        'timestamp_basis', 'source_instant'
      ),
    'value',
      json_object(
        'standard_value',
          CASE
            WHEN performance_numbers.n < 7560
              THEN 90.0 + (performance_numbers.n % 100)
            WHEN (performance_numbers.n / 3) % 6 = 0
              THEN 50.0 + (performance_numbers.n % 130)
            WHEN (performance_numbers.n / 3) % 6 = 1
              THEN 1.0 + (performance_numbers.n % 12)
            WHEN (performance_numbers.n / 3) % 6 = 2
              THEN 0.1 + (performance_numbers.n % 20) / 10.0
            WHEN (performance_numbers.n / 3) % 6 = 3
              THEN 30.0 + (performance_numbers.n % 70)
            WHEN (performance_numbers.n / 3) % 6 = 4
              THEN 10.0 + (performance_numbers.n % 100) / 10.0
            ELSE 1.0 + (performance_numbers.n % 50)
          END,
        'standard_unit',
          CASE
            WHEN performance_numbers.n < 7560 THEN 'bpm'
            WHEN (performance_numbers.n / 3) % 6 = 0 THEN 'bpm'
            WHEN (performance_numbers.n / 3) % 6 = 1 THEN 'count'
            WHEN (performance_numbers.n / 3) % 6 = 2 THEN 'kcal'
            WHEN (performance_numbers.n / 3) % 6 = 3 THEN 'ms'
            WHEN (performance_numbers.n / 3) % 6 = 4 THEN 'brpm'
            ELSE 'm'
          END,
        'input_value',
          CASE
            WHEN performance_numbers.n < 7560
              THEN 90.0 + (performance_numbers.n % 100)
            WHEN (performance_numbers.n / 3) % 6 = 0
              THEN 50.0 + (performance_numbers.n % 130)
            WHEN (performance_numbers.n / 3) % 6 = 1
              THEN 1.0 + (performance_numbers.n % 12)
            WHEN (performance_numbers.n / 3) % 6 = 2
              THEN 0.1 + (performance_numbers.n % 20) / 10.0
            WHEN (performance_numbers.n / 3) % 6 = 3
              THEN 30.0 + (performance_numbers.n % 70)
            WHEN (performance_numbers.n / 3) % 6 = 4
              THEN 10.0 + (performance_numbers.n % 100) / 10.0
            ELSE 1.0 + (performance_numbers.n % 50)
          END,
        'input_unit',
          CASE
            WHEN performance_numbers.n < 7560 THEN 'bpm'
            WHEN (performance_numbers.n / 3) % 6 = 0 THEN 'bpm'
            WHEN (performance_numbers.n / 3) % 6 = 1 THEN 'count'
            WHEN (performance_numbers.n / 3) % 6 = 2 THEN 'kcal'
            WHEN (performance_numbers.n / 3) % 6 = 3 THEN 'ms'
            WHEN (performance_numbers.n / 3) % 6 = 4 THEN 'brpm'
            ELSE 'm'
          END
      )
  )
`;

async function addBulkSamples(
  database: Awaited<ReturnType<typeof openDatabase>>,
  sampleCount: number,
): Promise<number> {
  if (sampleCount === 0) {
    const state = await databaseClient(database).execute(
      "SELECT current_revision FROM database_state WHERE singleton_id = 1",
    );
    return Number(state.rows[0]?.current_revision);
  }
  return withWriteTransaction(database, async (transaction) => {
    await transaction.execute(
      "CREATE TEMP TABLE performance_numbers (n INTEGER PRIMARY KEY) STRICT",
    );
    await transaction.execute({
      sql: `
        WITH RECURSIVE sequence(n) AS (
          SELECT 0
          UNION ALL
          SELECT n + 1 FROM sequence WHERE n + 1 < ?
        )
        INSERT INTO performance_numbers (n)
        SELECT n FROM sequence
      `,
      args: [sampleCount],
    });

    await transaction.execute(`
      INSERT INTO records (
        record_id,
        record_type,
        current_revision,
        created_at,
        is_deleted
      )
      SELECT
        printf(
          '01890f3e-e7d3-7cc0-88c8-%012x',
          performance_numbers.n + ${FIRST_BULK_SEQUENCE}
        ),
        'measurement_sample',
        1,
        '${CREATED_AT}',
        0
      FROM performance_numbers
    `);

    await transaction.execute(`
      WITH sample AS (
        SELECT
          performance_numbers.n,
          ${SAMPLE_DATA_SQL} AS data_json
        FROM performance_numbers
      )
      INSERT INTO measurement_samples (
        record_id,
        record_type,
        source_id,
        quantity,
        recorded_at,
        standard_value,
        standard_unit,
        duration_ms,
        provider_record_id,
        received_at,
        data_json
      )
      SELECT
        printf(
          '01890f3e-e7d3-7cc0-88c8-%012x',
          sample.n + ${FIRST_BULK_SEQUENCE}
        ),
        'measurement_sample',
        json_extract(sample.data_json, '$.source_id'),
        json_extract(sample.data_json, '$.quantity'),
        json_extract(sample.data_json, '$.recorded_at'),
        json_extract(sample.data_json, '$.value.standard_value'),
        json_extract(sample.data_json, '$.value.standard_unit'),
        NULL,
        NULL,
        '${CREATED_AT}',
        sample.data_json
      FROM sample
    `);

    await transaction.execute(`
      INSERT INTO record_revisions (
        record_id,
        revision,
        record_type,
        record_json,
        became_known_at,
        became_known_at_basis,
        changed_by_json,
        previous_revision
      )
      SELECT
        measurement_samples.record_id,
        1,
        'measurement_sample',
        measurement_samples.data_json,
        '${CREATED_AT}',
        'provider_import',
        '{"author_type":"tool","tool":"ath-performance-generator","tool_version":"1.0.0"}',
        NULL
      FROM measurement_samples
      WHERE measurement_samples.record_id BETWEEN
        printf('01890f3e-e7d3-7cc0-88c8-%012x', ${FIRST_BULK_SEQUENCE})
        AND
        printf(
          '01890f3e-e7d3-7cc0-88c8-%012x',
          ${FIRST_BULK_SEQUENCE} + ${sampleCount} - 1
        )
    `);

    const revision = await transaction.execute(`
      UPDATE database_state
      SET current_revision = current_revision + 1
      WHERE singleton_id = 1
      RETURNING current_revision
    `);
    return Number(revision.rows[0]?.current_revision);
  });
}

export async function generatePerformanceDatabase(
  options: GeneratePerformanceDatabaseOptions,
): Promise<GeneratedPerformanceDatabase> {
  assertRecordCount(options.record_count);
  const outputDirectory = resolve(options.output_directory);
  const databasePath = await copyBaseFixture(outputDirectory);
  const database = await openDatabase(databasePath, {
    onSecurityWarning: () => undefined,
  });
  try {
    await addPerformanceSources(database);
    const currentCount = await databaseClient(database).execute(
      "SELECT COUNT(*) AS count FROM records",
    );
    const sampleCount = options.record_count - Number(currentCount.rows[0]?.count);
    if (sampleCount < 0) {
      throw new Error(
        `record count ${options.record_count} is smaller than the base fixture`,
      );
    }
    const databaseRevision = await addBulkSamples(database, sampleCount);
    const finalCount = await databaseClient(database).execute(
      "SELECT COUNT(*) AS count FROM records",
    );
    if (Number(finalCount.rows[0]?.count) !== options.record_count) {
      throw new Error("performance database record count does not match its target");
    }
    await databaseClient(database).execute("PRAGMA wal_checkpoint(TRUNCATE)");
    return {
      database_path: databasePath,
      record_count: options.record_count,
      sample_count: sampleCount,
      database_revision: databaseRevision,
      seed: PERFORMANCE_FIXTURE_SEED,
    };
  } finally {
    database.close();
  }
}

function commandArgument(name: string): string | undefined {
  const index = process.argv.indexOf(name);
  return index === -1 ? undefined : process.argv[index + 1];
}

async function main(): Promise<void> {
  const recordsText = commandArgument("--records");
  const output = commandArgument("--output");
  if (recordsText === undefined || output === undefined) {
    throw new Error(
      "usage: generate-database.ts --records <count> --output <directory>",
    );
  }
  const result = await generatePerformanceDatabase({
    output_directory: output,
    record_count: Number(recordsText.replaceAll("_", "")),
  });
  console.log(JSON.stringify(result, null, 2));
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
