import type { Transaction } from "@libsql/client";
import {
  PointMeasurementTypeSchema,
  RecordActor,
  RecordType,
  SeriesQuantitySchema,
  UtcTimestamp,
  type CurrentRecordT,
  type RecordTypeT,
} from "../v1/schema.js";
import type {
  HardSignalT,
  SoftSignalT,
  SourceT as LegacySource,
} from "../schema.js";
import { atOffsetOf } from "../series.js";
import { parseProviderExport, type ParsedProviderExport } from "../import/index.js";
import { databaseClient, type AthleticDatabase } from "./client.js";
import {
  incrementDatabaseRevision,
  insertRevision,
  readRecord,
  upsertTypedRecord,
  validateTypedRelationships,
} from "./records.js";
import { withWriteTransaction } from "./transaction.js";
import { createUuidV7 } from "./uuid.js";

const TOOL_ACTOR = RecordActor.parse({
  author_type: "tool",
  tool: "ath",
  tool_version: "1.0.0",
});

interface ProviderRecordDraft {
  record_id: string;
  record_type: RecordTypeT;
  data: unknown;
  match_key_method: string;
  match_key: string;
  existing_record_id?: string;
}

interface ProviderImportPlan {
  drafts: ProviderRecordDraft[];
  received_at: string;
}

interface ProviderImportChange {
  draft: ProviderRecordDraft;
  current?: CurrentRecordT;
  action: "new" | "matching" | "changed";
}

export interface ProviderImportConflict {
  match_key: string;
  record_id?: string;
  message: string;
}

export interface ProviderImportPreview {
  provider: ParsedProviderExport["format"];
  label: string;
  database_revision: number;
  new_records: Array<{ record_id: string; record_type: RecordTypeT }>;
  matching_records: Array<{ record_id: string; record_type: RecordTypeT }>;
  changed_records: Array<{
    record_id: string;
    record_type: RecordTypeT;
    current_revision: number;
  }>;
  conflicts: ProviderImportConflict[];
  skipped: Record<string, { count: number; example?: string }>;
  plan: ProviderImportPlan;
  changes: ProviderImportChange[];
}

function normalizedUtc(timestamp: string): string {
  const milliseconds = Date.parse(timestamp);
  if (!Number.isFinite(milliseconds)) {
    throw new Error(`Provider timestamp is not an absolute instant: ${timestamp}`);
  }
  return UtcTimestamp.parse(new Date(milliseconds).toISOString());
}

function offsetText(timestamp: string): string | undefined {
  const match = /(Z|[+-]\d{2}:\d{2})$/i.exec(timestamp);
  return match?.[1]?.toUpperCase();
}

function timestampReceipt(
  timestamp: string,
  route: ParsedProviderExport["route"],
): {
  timestamp_text: string;
  utc_offset_text?: string;
  utc_offset_meaning?: "provider_cycle_offset" | "meaning_not_documented";
  utc_offset_missing_reason?: "not_provided_by_source";
  timestamp_basis: "source_offset_timestamp";
} {
  const offset = offsetText(timestamp);
  return {
    timestamp_text: timestamp,
    ...(offset === undefined
      ? { utc_offset_missing_reason: "not_provided_by_source" as const }
      : {
          utc_offset_text: offset,
          utc_offset_meaning:
            route === "whoop_csv"
              ? ("provider_cycle_offset" as const)
              : ("meaning_not_documented" as const),
        }),
    timestamp_basis: "source_offset_timestamp",
  };
}

const missingTimestampReceipt = {
  timestamp_missing_reason: "not_provided_by_source" as const,
  utc_offset_missing_reason: "not_provided_by_source" as const,
};

function sourceValue(value: number, unit: string) {
  return {
    standard_value: value,
    standard_unit: unit,
    input_value: value,
    input_unit: unit,
  };
}

const summaryNames: Readonly<Record<string, { summary_type: string; unit: string }>> = {
  duration_s: { summary_type: "sleep_duration", unit: "s" },
  time_in_bed_s: { summary_type: "time_in_bed", unit: "s" },
  efficiency_pct: { summary_type: "sleep_efficiency", unit: "%" },
  deep_s: { summary_type: "deep_sleep", unit: "s" },
  rem_s: { summary_type: "rem_sleep", unit: "s" },
  light_s: { summary_type: "light_sleep", unit: "s" },
  awake_s: { summary_type: "awake", unit: "s" },
  interruptions: { summary_type: "sleep_interruptions", unit: "count" },
  avg_hr_bpm: { summary_type: "average_heart_rate", unit: "bpm" },
  max_hr_bpm: { summary_type: "max_heart_rate", unit: "bpm" },
  energy_kcal: { summary_type: "energy", unit: "kcal" },
  distance_m: { summary_type: "distance", unit: "m" },
  elevation_gain_m: { summary_type: "elevation_gain", unit: "m" },
};

function sourceReportedSummaries(aggregates: Record<string, unknown>) {
  const summaries = [];
  for (const [key, value] of Object.entries(aggregates)) {
    const named = summaryNames[key];
    if (named === undefined || typeof value !== "number") {
      continue;
    }
    summaries.push({
      summary_type: named.summary_type,
      ...sourceValue(value, named.unit),
    });
  }
  return summaries;
}

function sourceData(source: LegacySource): {
  source_type: "wearable" | "export_file" | "connector" | "manual";
  data_writer: string;
  data_route: string;
  vendor?: string;
  sensor_type?: string;
  detail?: string;
} {
  if (source.kind === "manual") {
    return {
      source_type: "manual",
      data_writer: "ath",
      data_route: "manual_entry",
      detail: "Manually recorded data",
    };
  }
  return {
    source_type: source.kind,
    data_writer: source.writer ?? source.vendor ?? "Unknown writer",
    data_route: source.via ?? "unknown_import",
    ...(source.vendor === undefined ? {} : { vendor: source.vendor }),
    ...(source.sensor === undefined ? {} : { sensor_type: source.sensor }),
    ...(source.detail === undefined ? {} : { detail: source.detail }),
  };
}

async function resolveSourceIds(
  database: AthleticDatabase,
  parsed: ParsedProviderExport,
): Promise<{
  ids: Map<string, string>;
  drafts: ProviderRecordDraft[];
}> {
  const ids = new Map<string, string>();
  const drafts: ProviderRecordDraft[] = [];
  for (const source of parsed.sources) {
    const data = sourceData(source);
    let existingId: string | undefined;
    if (data.source_type === "manual") {
      const manual = await databaseClient(database).execute(`
        SELECT sources.record_id
        FROM sources
        JOIN records ON records.record_id = sources.record_id
        WHERE sources.source_type = 'manual' AND records.is_deleted = 0
        ORDER BY records.created_at, sources.record_id
        LIMIT 1
      `);
      const value = manual.rows[0]?.record_id;
      existingId = typeof value === "string" ? value : undefined;
    } else {
      const existing = await databaseClient(database).execute({
        sql: `
          SELECT sources.record_id
          FROM sources
          JOIN records ON records.record_id = sources.record_id
          WHERE sources.data_writer = ?
            AND sources.data_route = ?
            AND COALESCE(sources.sensor_type, '') = COALESCE(?, '')
            AND records.is_deleted = 0
          LIMIT 1
        `,
        args: [data.data_writer, data.data_route, data.sensor_type ?? null],
      });
      const value = existing.rows[0]?.record_id;
      existingId = typeof value === "string" ? value : undefined;
    }
    const recordId = existingId ?? createUuidV7();
    ids.set(source.id, recordId);
    if (data.source_type === "manual" && existingId !== undefined) {
      continue;
    }
    const key = [
      "source",
      data.data_writer,
      data.data_route,
      data.sensor_type ?? "",
    ].join("\u0000");
    drafts.push({
      record_id: recordId,
      record_type: "source",
      data,
      match_key_method: `${parsed.route}/v1`,
      match_key: key,
      ...(existingId === undefined ? {} : { existing_record_id: existingId }),
    });
  }
  return { ids, drafts };
}

function providerPeriods(
  parsed: ParsedProviderExport,
  timestamp: string,
): {
  recorded_at?: string;
  input_timestamp: ReturnType<typeof timestampReceipt> | typeof missingTimestampReceipt;
  provider_reporting_periods?: Array<{
    period_type: "oura_day_unspecified";
    provider_date: string;
    timezone_offset_missing_reason: "not_provided_by_source";
  }>;
} {
  const normalized = normalizedUtc(timestamp);
  if (
    parsed.route === "oura_csv" &&
    /T00:00:00(?:\.000)?Z$/.test(timestamp)
  ) {
    return {
      input_timestamp: missingTimestampReceipt,
      provider_reporting_periods: [
        {
          period_type: "oura_day_unspecified",
          provider_date: normalized.slice(0, 10),
          timezone_offset_missing_reason: "not_provided_by_source",
        },
      ],
    };
  }
  return {
    recorded_at: normalized,
    input_timestamp: timestampReceipt(timestamp, parsed.route),
  };
}

function draftKey(
  method: string,
  base: readonly string[],
  collisions: Map<string, number>,
): string {
  const stem = base.join("\u0000");
  const ordinal = collisions.get(stem) ?? 0;
  collisions.set(stem, ordinal + 1);
  return `${stem}\u0000${ordinal}`;
}

function hardSignalDraft(
  parsed: ParsedProviderExport,
  signal: HardSignalT,
  sourceId: string,
  receivedAt: string,
  collisions: Map<string, number>,
): ProviderRecordDraft | null {
  const method = `${parsed.route}/v1`;
  if (signal.type === "series_ref" || signal.type === "benchmark_result") {
    return null;
  }
  if (signal.type === "sleep_session") {
    const start = normalizedUtc(signal.start);
    const end = normalizedUtc(signal.end);
    return {
      record_id: createUuidV7(),
      record_type: "sleep_session",
      data: {
        source_id: sourceId,
        received_at: receivedAt,
        received_at_basis: "import_time",
        start_at: start,
        end_at: end,
        input_start_timestamp: timestampReceipt(signal.start, parsed.route),
        input_end_timestamp: timestampReceipt(signal.end, parsed.route),
        source_reported_summaries: sourceReportedSummaries(signal.aggregates),
      },
      match_key_method: method,
      match_key: draftKey(
        method,
        ["sleep_session", sourceId, start],
        collisions,
      ),
    };
  }
  if (signal.type === "workout_session") {
    const start = normalizedUtc(signal.start);
    const end = normalizedUtc(signal.end);
    return {
      record_id: createUuidV7(),
      record_type: "workout_session",
      data: {
        source_id: sourceId,
        received_at: receivedAt,
        received_at_basis: "import_time",
        start_at: start,
        end_at: end,
        input_start_timestamp: timestampReceipt(signal.start, parsed.route),
        input_end_timestamp: timestampReceipt(signal.end, parsed.route),
        ...(signal.aggregates.activity === undefined
          ? {}
          : { activity_type: signal.aggregates.activity }),
        source_reported_summaries: sourceReportedSummaries(signal.aggregates),
        ...(signal.segments === undefined
          ? {}
          : {
              segments: signal.segments.map((segment) => ({
                label: segment.label,
                source_reported_summaries: sourceReportedSummaries({
                  duration_s: segment.duration_s,
                  distance_m: segment.distance_m,
                  avg_hr_bpm: segment.avg_hr_bpm,
                }),
              })),
            }),
      },
      match_key_method: method,
      match_key: draftKey(
        method,
        ["workout_session", sourceId, start],
        collisions,
      ),
    };
  }
  if (signal.type === "vendor_score") {
    const time = providerPeriods(parsed, signal.recorded_at);
    const anchor =
      time.recorded_at ??
      time.provider_reporting_periods?.[0]?.provider_date ??
      signal.recorded_at;
    return {
      record_id: createUuidV7(),
      record_type: "provider_score",
      data: {
        source_id: sourceId,
        received_at: receivedAt,
        received_at_basis: "import_time",
        metric: signal.metric,
        value: sourceValue(signal.value, "score"),
        scale: signal.scale,
        ...time,
      },
      match_key_method: method,
      match_key: draftKey(
        method,
        ["provider_score", sourceId, signal.metric, anchor],
        collisions,
      ),
    };
  }

  const quantity = PointMeasurementTypeSchema.parse(signal.type);
  const time = providerPeriods(parsed, signal.recorded_at);
  const anchor =
    time.recorded_at ??
    time.provider_reporting_periods?.[0]?.provider_date ??
    signal.recorded_at;
  return {
    record_id: createUuidV7(),
    record_type: "point_measurement",
    data: {
      source_id: sourceId,
      received_at: receivedAt,
      received_at_basis: "import_time",
      quantity,
      ...time,
      value: sourceValue(signal.value, signal.unit),
      ...(signal.derived === undefined
        ? {}
        : {
            calculations: [
              {
                from_quantity: signal.derived.from,
                method: signal.derived.method,
                calculation_version: "provider-parser-v1",
                ...(signal.derived.window_s === undefined
                  ? {}
                  : { window_s: signal.derived.window_s }),
                ...(signal.derived.n_beats === undefined
                  ? {}
                  : { usable_sample_count: signal.derived.n_beats }),
                ...(signal.derived.n_dropped === undefined
                  ? {}
                  : { dropped_sample_count: signal.derived.n_dropped }),
                ...(signal.derived.signal_quality === undefined
                  ? {}
                  : { signal_quality: signal.derived.signal_quality }),
                input_records: [],
              },
            ],
          }),
    },
    match_key_method: method,
    match_key: draftKey(
      method,
      ["point_measurement", sourceId, quantity, anchor],
      collisions,
    ),
  };
}

function softSignalDraft(
  parsed: ParsedProviderExport,
  signal: SoftSignalT,
  receivedAt: string,
  collisions: Map<string, number>,
): ProviderRecordDraft {
  const reportedAt = normalizedUtc(signal.reported_at);
  const method = `${parsed.route}/v1`;
  const key = draftKey(
    method,
    ["athlete_report", signal.type, reportedAt],
    collisions,
  );
  if (signal.type === "nutrition") {
    return {
      record_id: createUuidV7(),
      record_type: "nutrition_entry",
      data: {
        reported_at: reportedAt,
        input_reported_at: timestampReceipt(signal.reported_at, parsed.route),
        reported_via: signal.provenance?.via ?? "text",
        athlete_description: signal.note ?? "Nutrition journal answer",
        reported_items: [],
      },
      match_key_method: method,
      match_key: key,
    };
  }
  return {
    record_id: createUuidV7(),
    record_type: "athlete_report",
    data: {
      report_type: signal.type,
      reported_at: reportedAt,
      input_reported_at: timestampReceipt(signal.reported_at, parsed.route),
      reported_via: signal.provenance?.via ?? "text",
      ...(signal.rating === undefined ? {} : { rating: signal.rating }),
      ...(signal.scale === undefined ? {} : { rating_scale: signal.scale }),
      ...(signal.body_region === undefined ? {} : { body_region: signal.body_region }),
      ...(signal.note === undefined ? {} : { note: signal.note }),
    },
    match_key_method: method,
    match_key: key,
  };
}

function seriesDrafts(
  parsed: ParsedProviderExport,
  sourceIds: Map<string, string>,
  receivedAt: string,
  collisions: Map<string, number>,
): ProviderRecordDraft[] {
  const drafts: ProviderRecordDraft[] = [];
  const method = `${parsed.route}/v1`;
  for (const built of parsed.payload.series) {
    const sourceId = sourceIds.get(built.source);
    if (sourceId === undefined) {
      throw new Error(`Provider series uses unknown source: ${built.source}`);
    }
    const file = JSON.parse(built.content) as {
      quantity: string;
      unit: string;
      start: string;
      offsets_ms: number[];
      values: number[];
      durations_ms?: number[];
    };
    const quantity = SeriesQuantitySchema.parse(file.quantity);
    const startMs = Date.parse(file.start);
    for (const [index, offset] of file.offsets_ms.entries()) {
      const inputTimestamp = atOffsetOf(file.start, startMs + offset);
      const recordedAt = normalizedUtc(inputTimestamp);
      drafts.push({
        record_id: createUuidV7(),
        record_type: "measurement_sample",
        data: {
          source_id: sourceId,
          received_at: receivedAt,
          received_at_basis: "import_time",
          quantity,
          recorded_at: recordedAt,
          input_timestamp: {
            ...timestampReceipt(inputTimestamp, parsed.route),
            timestamp_basis: "reconstructed_from_series_start",
          },
          value: sourceValue(file.values[index]!, file.unit),
          ...(file.durations_ms?.[index] === undefined
            ? {}
            : { duration_ms: file.durations_ms[index] }),
          timestamp_base_text: file.start,
          timestamp_reconstruction_method: "series_start_plus_offset_ms_v1",
        },
        match_key_method: method,
        match_key: draftKey(
          method,
          ["measurement_sample", sourceId, quantity, recordedAt],
          collisions,
        ),
      });
    }
  }
  return drafts;
}

async function buildProviderPlan(
  database: AthleticDatabase,
  parsed: ParsedProviderExport,
  receivedAt: string,
): Promise<ProviderImportPlan> {
  const resolved = await resolveSourceIds(database, parsed);
  const collisions = new Map<string, number>();
  const drafts = [...resolved.drafts];
  const acceptedByDevice = new Map<string, number>();
  for (const signal of parsed.payload.hardSignals) {
    const deviceKey = parsed.payload.deviceOf.get(signal);
    if (deviceKey !== undefined) {
      acceptedByDevice.set(deviceKey, (acceptedByDevice.get(deviceKey) ?? 0) + 1);
    }
  }

  for (const [deviceKey, observation] of parsed.payload.deviceIndex) {
    const sourceId = resolved.ids.get(observation.source);
    if (sourceId === undefined) {
      throw new Error(`Provider device uses unknown source: ${observation.source}`);
    }
    drafts.push({
      record_id: createUuidV7(),
      record_type: "source_device",
      data: {
        source_id: sourceId,
        ...observation.device,
        coverage_start_date: observation.from,
        coverage_end_date: observation.to,
        accepted_record_count: acceptedByDevice.get(deviceKey) ?? observation.n,
      },
      match_key_method: `${parsed.route}/v1`,
      match_key: [
        "source_device",
        sourceId,
        observation.device.name ?? "",
        observation.device.manufacturer ?? "",
        observation.device.model ?? "",
        observation.device.hardware ?? "",
      ].join("\u0000"),
    });
  }

  for (const signal of parsed.payload.hardSignals) {
    if (!("source" in signal)) {
      continue;
    }
    const sourceId = resolved.ids.get(signal.source);
    if (sourceId === undefined) {
      throw new Error(`Provider record uses unknown source: ${signal.source}`);
    }
    const draft = hardSignalDraft(
      parsed,
      signal,
      sourceId,
      receivedAt,
      collisions,
    );
    if (draft !== null) {
      drafts.push(draft);
    }
  }
  drafts.push(
    ...seriesDrafts(parsed, resolved.ids, receivedAt, collisions),
    ...parsed.payload.softSignals.map((signal) =>
      softSignalDraft(parsed, signal, receivedAt, collisions),
    ),
  );
  return { drafts, received_at: receivedAt };
}

function sameLogicalData(left: unknown, right: unknown): boolean {
  const withoutReceipt = (value: unknown): unknown => {
    if (Array.isArray(value)) {
      return value.map(withoutReceipt);
    }
    if (typeof value !== "object" || value === null) {
      return value;
    }
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .filter(([key]) => key !== "received_at" && key !== "received_at_basis")
        .map(([key, child]) => [key, withoutReceipt(child)]),
    );
  };
  return JSON.stringify(withoutReceipt(left)) === JSON.stringify(withoutReceipt(right));
}

async function recordsByImportKey(
  database: AthleticDatabase,
  drafts: readonly ProviderRecordDraft[],
): Promise<Map<string, CurrentRecordT>> {
  const records = new Map<string, CurrentRecordT>();
  const chunkSize = 400;
  for (let offset = 0; offset < drafts.length; offset += chunkSize) {
    const chunk = drafts.slice(offset, offset + chunkSize);
    const clauses = chunk.map(() => "(record_import_keys.match_key_method = ? AND record_import_keys.match_key = ?)");
    const args = chunk.flatMap((draft) => [draft.match_key_method, draft.match_key]);
    const result = await databaseClient(database).execute({
      sql: `
        SELECT
          record_import_keys.match_key_method,
          record_import_keys.match_key,
          records.record_id,
          records.record_type,
          records.current_revision,
          records.created_at,
          records.is_deleted,
          record_revisions.record_json
        FROM record_import_keys
        JOIN records ON records.record_id = record_import_keys.record_id
        JOIN record_revisions
          ON record_revisions.record_id = records.record_id
         AND record_revisions.revision = records.current_revision
        WHERE ${clauses.join(" OR ")}
      `,
      args,
    });
    for (const row of result.rows) {
      records.set(
        `${String(row.match_key_method)}\u0000${String(row.match_key)}`,
        {
          record_id: String(row.record_id),
          record_type: RecordType.parse(row.record_type),
          current_revision: Number(row.current_revision),
          created_at: UtcTimestamp.parse(row.created_at),
          is_deleted: Number(row.is_deleted) === 1,
          data: JSON.parse(String(row.record_json)) as never,
        } as CurrentRecordT,
      );
    }
  }
  return records;
}

function skipSummary(parsed: ParsedProviderExport): Record<string, { count: number; example?: string }> {
  return Object.fromEntries(
    [...parsed.payload.skipped]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([reason, count]) => [
        reason,
        {
          count,
          ...(parsed.payload.skipExamples.get(reason) === undefined
            ? {}
            : { example: parsed.payload.skipExamples.get(reason) }),
        },
      ]),
  );
}

export async function previewProviderImport(
  database: AthleticDatabase,
  inputPath: string,
  options: { received_at?: string } = {},
): Promise<ProviderImportPreview> {
  const receivedAt = UtcTimestamp.parse(options.received_at ?? new Date().toISOString());
  const parsed = await parseProviderExport(inputPath);
  const plan = await buildProviderPlan(database, parsed, receivedAt);
  const keyedRecords = await recordsByImportKey(database, plan.drafts);
  const conflicts: ProviderImportConflict[] = [];
  const changes: ProviderImportChange[] = [];
  const seenKeys = new Map<string, ProviderRecordDraft>();

  for (const draft of plan.drafts) {
    const key = `${draft.match_key_method}\u0000${draft.match_key}`;
    const duplicate = seenKeys.get(key);
    if (duplicate !== undefined) {
      conflicts.push({
        match_key: draft.match_key,
        message:
          sameLogicalData(duplicate.data, draft.data)
            ? "provider export repeats one import key"
            : "provider export gives one import key two different records",
      });
      continue;
    }
    seenKeys.set(key, draft);

    let current = keyedRecords.get(key);
    if (current === undefined && draft.existing_record_id !== undefined) {
      current = await readRecord(database, draft.existing_record_id) ?? undefined;
    }
    if (current === undefined) {
      changes.push({ draft, action: "new" });
      continue;
    }
    draft.record_id = current.record_id;
    if (current.record_type !== draft.record_type || current.is_deleted) {
      conflicts.push({
        match_key: draft.match_key,
        record_id: current.record_id,
        message: current.is_deleted
          ? "provider import key points to a normally deleted record"
          : `provider import key points to ${current.record_type}, not ${draft.record_type}`,
      });
      continue;
    }
    changes.push({
      draft,
      current,
      action: sameLogicalData(current.data, draft.data) ? "matching" : "changed",
    });
  }

  const collisionGroups = new Map<string, ProviderImportChange[]>();
  for (const change of changes) {
    const parts = change.draft.match_key.split("\u0000");
    const ordinal = parts.at(-1);
    if (ordinal === undefined || !/^\d+$/.test(ordinal)) {
      continue;
    }
    const base = parts.slice(0, -1).join("\u0000");
    const group = collisionGroups.get(base) ?? [];
    group.push(change);
    collisionGroups.set(base, group);
  }
  for (const group of collisionGroups.values()) {
    if (
      group.length > 1 &&
      group.some((change) => change.action === "changed")
    ) {
      conflicts.push({
        match_key: group[0]!.draft.match_key,
        message:
          "an indistinguishable duplicate group changed; the importer will not guess which row was corrected",
      });
    }
  }

  const revision = await databaseClient(database).execute(
    "SELECT current_revision FROM database_state WHERE singleton_id = 1",
  );
  return {
    provider: parsed.format,
    label: parsed.label,
    database_revision: Number(revision.rows[0]?.current_revision),
    new_records: changes
      .filter((change) => change.action === "new")
      .map(({ draft }) => ({ record_id: draft.record_id, record_type: draft.record_type })),
    matching_records: changes
      .filter((change) => change.action === "matching")
      .map(({ draft }) => ({ record_id: draft.record_id, record_type: draft.record_type })),
    changed_records: changes
      .filter(
        (change): change is ProviderImportChange & { current: CurrentRecordT } =>
          change.action === "changed" && change.current !== undefined,
      )
      .map(({ draft, current }) => ({
        record_id: draft.record_id,
        record_type: draft.record_type,
        current_revision: current.current_revision,
      })),
    conflicts,
    skipped: skipSummary(parsed),
    plan,
    changes,
  };
}

async function insertNewRecord(
  transaction: Transaction,
  draft: ProviderRecordDraft,
  becameKnownAt: string,
): Promise<CurrentRecordT> {
  const record = {
    record_id: draft.record_id,
    record_type: draft.record_type,
    current_revision: 1,
    created_at: becameKnownAt,
    is_deleted: false,
    data: draft.data,
  } as CurrentRecordT;
  await transaction.execute({
    sql: `
      INSERT INTO records (record_id, record_type, current_revision, created_at, is_deleted)
      VALUES (?, ?, 1, ?, 0)
    `,
    args: [record.record_id, record.record_type, record.created_at],
  });
  await insertRevision(
    transaction,
    record,
    TOOL_ACTOR,
    becameKnownAt,
    "provider_import",
    null,
  );
  await upsertTypedRecord(transaction, record);
  return record;
}

async function insertChangedRecord(
  transaction: Transaction,
  change: ProviderImportChange & { current: CurrentRecordT },
  becameKnownAt: string,
): Promise<CurrentRecordT> {
  const record = {
    ...change.current,
    current_revision: change.current.current_revision + 1,
    data: change.draft.data,
  } as CurrentRecordT;
  await insertRevision(
    transaction,
    record,
    TOOL_ACTOR,
    becameKnownAt,
    "provider_import",
    change.current.current_revision,
  );
  await upsertTypedRecord(transaction, record);
  await transaction.execute({
    sql: "UPDATE records SET current_revision = ? WHERE record_id = ?",
    args: [record.current_revision, record.record_id],
  });
  return record;
}

export async function importProviderRecords(
  database: AthleticDatabase,
  preview: ProviderImportPreview,
): Promise<{
  database_revision: number;
  new_record_count: number;
  changed_record_count: number;
  matching_record_count: number;
}> {
  if (preview.conflicts.length > 0) {
    throw new Error("Provider import has unresolved conflicts");
  }
  return withWriteTransaction(database, async (transaction) => {
    const state = await transaction.execute(
      "SELECT current_revision FROM database_state WHERE singleton_id = 1",
    );
    if (Number(state.rows[0]?.current_revision) !== preview.database_revision) {
      throw new Error("Database changed after provider import preview; preview it again");
    }

    const written: CurrentRecordT[] = [];
    for (const change of preview.changes) {
      if (change.action === "new") {
        written.push(
          await insertNewRecord(transaction, change.draft, preview.plan.received_at),
        );
      } else if (change.action === "changed" && change.current !== undefined) {
        written.push(
          await insertChangedRecord(
            transaction,
            change as ProviderImportChange & { current: CurrentRecordT },
            preview.plan.received_at,
          ),
        );
      }
    }
    for (const record of written) {
      await validateTypedRelationships(transaction, record);
    }

    let keyAdded = false;
    for (const change of preview.changes) {
      const existing = await transaction.execute({
        sql: `
          SELECT record_id
          FROM record_import_keys
          WHERE match_key_method = ? AND match_key = ?
        `,
        args: [change.draft.match_key_method, change.draft.match_key],
      });
      if (existing.rows.length === 0) {
        await transaction.execute({
          sql: `
            INSERT INTO record_import_keys (
              record_id,
              match_key_method,
              match_key,
              imported_at
            ) VALUES (?, ?, ?, ?)
          `,
          args: [
            change.draft.record_id,
            change.draft.match_key_method,
            change.draft.match_key,
            preview.plan.received_at,
          ],
        });
        keyAdded = true;
      } else if (existing.rows[0]?.record_id !== change.draft.record_id) {
        throw new Error("Provider import key changed after preview");
      }
    }

    const databaseRevision =
      written.length > 0 || keyAdded
        ? await incrementDatabaseRevision(transaction)
        : preview.database_revision;
    return {
      database_revision: databaseRevision,
      new_record_count: preview.new_records.length,
      changed_record_count: preview.changed_records.length,
      matching_record_count: preview.matching_records.length,
    };
  });
}
