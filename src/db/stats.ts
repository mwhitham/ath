import {
  POINT_MEASUREMENT_UNITS,
  type PointMeasurementType,
} from "../v1/schema.js";
import { databaseClient, type AthleticDatabase } from "./client.js";
import { readDatabaseMetadata } from "./migrate.js";

export interface SourceStats {
  record_id: string;
  source_type: string;
  data_writer: string;
  data_route: string;
  vendor: string | null;
  sensor_type: string | null;
  current_record_count: number;
  first_recorded_at: string | null;
  last_recorded_at: string | null;
}

export interface BaselineStats {
  source_id: string;
  quantity: PointMeasurementType;
  unit: string;
  mean: number;
  standard_deviation: number;
  observation_count: number;
  from: string;
  to: string;
  window_days: 90;
  method: "sample_mean_and_standard_deviation";
}

export interface SeriesCoverageStats {
  source_id: string;
  quantity: string;
  unit: string;
  sample_count: number;
  from: string;
  to: string;
}

export interface DatabaseStats {
  athleticstandard_version: string;
  database_id: string;
  database_revision: number;
  athlete: Record<string, unknown> | null;
  current_records: {
    total: number;
    by_type: Record<string, number>;
  };
  sources: SourceStats[];
  baselines: BaselineStats[];
  series: SeriesCoverageStats[];
  benchmarks: {
    defined: number;
    results: number;
  };
  predictions: {
    total: number;
    graded: number;
    by_author: Array<{
      author: string;
      predictions: number;
    }>;
  };
}

function rounded(value: number): number {
  return Math.round(value * 10) / 10;
}

function mean(values: readonly number[]): number {
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function standardDeviation(values: readonly number[], average: number): number {
  if (values.length < 2) {
    return 0;
  }
  const squareSum = values.reduce(
    (sum, value) => sum + (value - average) ** 2,
    0,
  );
  return Math.sqrt(squareSum / (values.length - 1));
}

export async function databaseStats(
  database: AthleticDatabase,
): Promise<DatabaseStats> {
  const client = databaseClient(database);
  const metadata = await readDatabaseMetadata(database);
  const state = await client.execute(
    "SELECT current_revision FROM database_state WHERE singleton_id = 1",
  );
  const databaseRevision = Number(state.rows[0]?.current_revision);

  const profileResult = await client.execute(`
    SELECT athlete_profile.data_json
    FROM athlete_profile
    JOIN records ON records.record_id = athlete_profile.record_id
    WHERE records.is_deleted = 0
  `);
  const athlete =
    profileResult.rows[0] === undefined
      ? null
      : (JSON.parse(String(profileResult.rows[0].data_json)) as Record<string, unknown>);

  const countResult = await client.execute(`
    SELECT record_type, COUNT(*) AS record_count
    FROM records
    WHERE is_deleted = 0
    GROUP BY record_type
    ORDER BY record_type
  `);
  const byType = Object.fromEntries(
    countResult.rows.map((row) => [
      String(row.record_type),
      Number(row.record_count),
    ]),
  );
  const total = Object.values(byType).reduce((sum, count) => sum + count, 0);

  const sourceResult = await client.execute(`
    WITH measured AS (
      SELECT record_id, source_id, recorded_at AS at FROM point_measurements
      UNION ALL
      SELECT record_id, source_id, recorded_at AS at FROM measurement_samples
      UNION ALL
      SELECT record_id, source_id, start_at AS at FROM sleep_sessions
      UNION ALL
      SELECT record_id, source_id, start_at AS at FROM workout_sessions
      UNION ALL
      SELECT record_id, source_id, recorded_at AS at FROM benchmark_results
      UNION ALL
      SELECT record_id, source_id, recorded_at AS at FROM provider_summaries
      UNION ALL
      SELECT record_id, source_id, recorded_at AS at FROM provider_scores
    ),
    current_measured AS (
      SELECT measured.source_id, measured.at
      FROM measured
      JOIN records ON records.record_id = measured.record_id
      WHERE records.is_deleted = 0
    )
    SELECT
      sources.record_id,
      sources.source_type,
      sources.data_writer,
      sources.data_route,
      sources.vendor,
      sources.sensor_type,
      COUNT(current_measured.source_id) AS current_record_count,
      MIN(current_measured.at) AS first_recorded_at,
      MAX(current_measured.at) AS last_recorded_at
    FROM sources
    JOIN records
      ON records.record_id = sources.record_id
     AND records.is_deleted = 0
    LEFT JOIN current_measured ON current_measured.source_id = sources.record_id
    GROUP BY sources.record_id
    ORDER BY sources.data_writer, sources.data_route, sources.record_id
  `);
  const sources: SourceStats[] = sourceResult.rows.map((row) => ({
    record_id: String(row.record_id),
    source_type: String(row.source_type),
    data_writer: String(row.data_writer),
    data_route: String(row.data_route),
    vendor: row.vendor === null ? null : String(row.vendor),
    sensor_type: row.sensor_type === null ? null : String(row.sensor_type),
    current_record_count: Number(row.current_record_count),
    first_recorded_at:
      row.first_recorded_at === null ? null : String(row.first_recorded_at),
    last_recorded_at:
      row.last_recorded_at === null ? null : String(row.last_recorded_at),
  }));

  const baselineGroups = await client.execute(`
    SELECT
      point_measurements.source_id,
      point_measurements.quantity,
      MAX(point_measurements.recorded_at) AS latest_at
    FROM point_measurements
    JOIN records ON records.record_id = point_measurements.record_id
    JOIN sources ON sources.record_id = point_measurements.source_id
    WHERE
      records.is_deleted = 0
      AND sources.source_type <> 'manual'
      AND point_measurements.recorded_at IS NOT NULL
    GROUP BY point_measurements.source_id, point_measurements.quantity
    ORDER BY point_measurements.source_id, point_measurements.quantity
  `);
  const baselines: BaselineStats[] = [];
  for (const group of baselineGroups.rows) {
    const sourceId = String(group.source_id);
    const quantity = String(group.quantity) as PointMeasurementType;
    const latestAt = String(group.latest_at);
    const cutoff = new Date(Date.parse(latestAt) - 90 * 86_400_000).toISOString();
    const observations = await client.execute({
      sql: `
        SELECT point_measurements.standard_value, point_measurements.recorded_at
        FROM point_measurements
        JOIN records ON records.record_id = point_measurements.record_id
        WHERE
          records.is_deleted = 0
          AND point_measurements.source_id = ?
          AND point_measurements.quantity = ?
          AND point_measurements.recorded_at >= ?
          AND point_measurements.recorded_at <= ?
        ORDER BY point_measurements.recorded_at, point_measurements.record_id
      `,
      args: [sourceId, quantity, cutoff, latestAt],
    });
    const values = observations.rows.map((row) => Number(row.standard_value));
    if (values.length === 0) {
      continue;
    }
    const average = mean(values);
    baselines.push({
      source_id: sourceId,
      quantity,
      unit: POINT_MEASUREMENT_UNITS[quantity],
      mean: rounded(average),
      standard_deviation: rounded(standardDeviation(values, average)),
      observation_count: values.length,
      from: String(observations.rows[0]!.recorded_at),
      to: String(observations.rows.at(-1)!.recorded_at),
      window_days: 90,
      method: "sample_mean_and_standard_deviation",
    });
  }

  const seriesResult = await client.execute(`
    SELECT
      measurement_samples.source_id,
      measurement_samples.quantity,
      measurement_samples.standard_unit,
      COUNT(*) AS sample_count,
      MIN(measurement_samples.recorded_at) AS first_at,
      MAX(measurement_samples.recorded_at) AS last_at
    FROM measurement_samples
    JOIN records ON records.record_id = measurement_samples.record_id
    WHERE records.is_deleted = 0
    GROUP BY
      measurement_samples.source_id,
      measurement_samples.quantity,
      measurement_samples.standard_unit
    ORDER BY measurement_samples.source_id, measurement_samples.quantity
  `);
  const series: SeriesCoverageStats[] = seriesResult.rows.map((row) => ({
    source_id: String(row.source_id),
    quantity: String(row.quantity),
    unit: String(row.standard_unit),
    sample_count: Number(row.sample_count),
    from: String(row.first_at),
    to: String(row.last_at),
  }));

  const predictionAuthors = await client.execute(`
    SELECT
      COALESCE(
        json_extract(predictions.data_json, '$.authorship.author.agent') ||
          ' running ' ||
          json_extract(predictions.data_json, '$.authorship.author.model'),
        json_extract(predictions.data_json, '$.authorship.author.tool'),
        'athlete'
      ) AS author,
      COUNT(*) AS prediction_count
    FROM predictions
    JOIN records ON records.record_id = predictions.record_id
    WHERE records.is_deleted = 0
    GROUP BY author
    ORDER BY prediction_count DESC, author
  `);

  return {
    athleticstandard_version: metadata.athleticstandard_version,
    database_id: metadata.database_id,
    database_revision: databaseRevision,
    athlete,
    current_records: { total, by_type: byType },
    sources,
    baselines,
    series,
    benchmarks: {
      defined: byType.benchmark_definition ?? 0,
      results: byType.benchmark_result ?? 0,
    },
    predictions: {
      total: byType.prediction ?? 0,
      graded: byType.prediction_grade ?? 0,
      by_author: predictionAuthors.rows.map((row) => ({
        author: String(row.author),
        predictions: Number(row.prediction_count),
      })),
    },
  };
}

export function renderDatabaseStats(stats: DatabaseStats): string {
  const athleteName =
    typeof stats.athlete?.name === "string" ? stats.athlete.name : "unnamed athlete";
  const lines = [
    `${athleteName} · Athletic Standard ${stats.athleticstandard_version}`,
    `database revision: ${stats.database_revision}`,
    "",
    `current records: ${stats.current_records.total}`,
  ];
  for (const [recordType, count] of Object.entries(stats.current_records.by_type)) {
    lines.push(`  ${recordType}: ${count}`);
  }

  lines.push("", `sources: ${stats.sources.length}`);
  for (const source of stats.sources) {
    const period =
      source.first_recorded_at === null
        ? ""
        : `, ${source.first_recorded_at.slice(0, 10)} → ${source.last_recorded_at!.slice(0, 10)}`;
    lines.push(
      `  ${source.record_id}: ${source.data_writer} via ${source.data_route} ` +
        `(${source.source_type}) — ${source.current_record_count} record` +
        `${source.current_record_count === 1 ? "" : "s"}${period}`,
    );
  }

  if (stats.baselines.length > 0) {
    lines.push("", "90-day device baselines (each source stays separate):");
    for (const baseline of stats.baselines) {
      lines.push(
        `  ${baseline.source_id} ${baseline.quantity}: ${baseline.mean}${baseline.unit} ` +
          `(sd ${baseline.standard_deviation}, n=${baseline.observation_count}, ` +
          `${baseline.from.slice(0, 10)} → ${baseline.to.slice(0, 10)})`,
      );
    }
  }

  if (stats.series.length > 0) {
    lines.push("", "sample series:");
    for (const series of stats.series) {
      lines.push(
        `  ${series.source_id} ${series.quantity}: ${series.sample_count} samples, ` +
          `${series.from} → ${series.to}`,
      );
    }
  }

  lines.push(
    "",
    `benchmarks defined: ${stats.benchmarks.defined} · results recorded: ${stats.benchmarks.results}`,
    `predictions: ${stats.predictions.total} · grades: ${stats.predictions.graded}`,
  );
  for (const author of stats.predictions.by_author) {
    lines.push(`  ${author.author}: ${author.predictions}`);
  }
  return lines.join("\n");
}
