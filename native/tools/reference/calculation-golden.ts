/**
 * Generate golden outputs for calculations, queries, views, prediction, and
 * log planning by running the 2.1.0 implementation on invented data.
 *
 * Each output file holds a scenario (the write steps), and a list of cases.
 * A case is either more steps or one call with its input and observed output.
 * The Rust tests in native/tests/ replay the same steps through the native
 * store, run the same calls, and require the same JSON after the
 * field-specific normalization in native/tests/common/mod.rs.
 *
 * Output: native/tests/golden/{calculate,view,prediction,log}/*.json
 *
 * Run with the old implementation's dependencies installed, for example from
 * a 2.1.0 checkout:
 *   ATH_REFERENCE_ROOT=/path/to/2.1.0-checkout \
 *     node --import /path/to/2.1.0-checkout/node_modules/tsx/dist/esm/index.mjs \
 *     native/tools/reference/calculation-golden.ts
 *
 * ATH_REFERENCE_ROOT defaults to this repository. All data is invented.
 */
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repository = join(here, "..", "..", "..");
const root = process.env.ATH_REFERENCE_ROOT ?? repository;
const goldenRoot = join(repository, "native", "tests", "golden");
const REFERENCE_COMMIT = "c6218e88c035708bdd15b3a20888d19713e1df29";

const db = await import(`${root}/src/db/index.ts`);
const calculation = await import(`${root}/src/db/calculate.ts`);
const validation = await import(`${root}/src/db/validation-message.ts`);
const core = await import(`${root}/src/ath-core.ts`);
const skill = await import(`${root}/src/skill.ts`);
const benchmarks = await import(`${root}/src/benchmarks.ts`);

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
type Step = { write: Json[] } | { correct: Json } | { delete: Json };
type Case = { steps: Step[] } | { name: string; call: string; input: Json; output?: Json };

// ---------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------

const id = (n: number) => `01890f3e-e7d3-7cc0-88c8-${n.toString(16).padStart(12, "0")}`;
const tool = { author_type: "tool", tool: "test", tool_version: "1.0.0" };
const ath = { author_type: "tool", tool: "ath", tool_version: "2.1.0" };
const athlete = { author_type: "athlete" };
const receipt = (at: string) => ({
  timestamp_text: at,
  utc_offset_text: "Z",
  utc_offset_meaning: "documented_recording_offset",
  timestamp_basis: "source_instant",
});
const noTimestamp = {
  timestamp_missing_reason: "not_provided_by_source",
  utc_offset_missing_reason: "not_provided_by_source",
};
const value = (standard: number, unit: string) => ({
  standard_value: standard,
  standard_unit: unit,
  input_value: standard,
  input_unit: unit,
});

function source(recordId: string, writer: string, sourceType = "wearable", createdAt = "2026-01-01T00:00:00.000Z") {
  return {
    record_type: "source",
    record_id: recordId,
    created_at: createdAt,
    changed_by: tool,
    data: { source_type: sourceType, data_writer: writer, data_route: "test" },
  };
}

function known(input: Record<string, Json>, knownAt?: string): Record<string, Json> {
  return knownAt === undefined ? input : { ...input, became_known_at: knownAt };
}

function point(
  recordId: string,
  sourceId: string,
  quantity: string,
  at: string | null,
  standard: number,
  unit: string,
  extra: Record<string, Json> = {},
  createdAt = at ?? "2026-10-07T00:00:00.000Z",
) {
  return {
    record_type: "point_measurement",
    record_id: recordId,
    created_at: createdAt,
    changed_by: tool,
    data: {
      source_id: sourceId,
      received_at: createdAt,
      received_at_basis: "import_time",
      ...("provider_reporting_periods" in extra ? { provider_reporting_periods: extra.provider_reporting_periods } : {}),
      quantity,
      ...(at === null ? {} : { recorded_at: at }),
      input_timestamp: at === null ? noTimestamp : receipt(at),
      value: value(standard, unit),
    },
  };
}

function sample(recordId: string, sourceId: string, quantity: string, at: string, standard: number, unit: string, durationMs?: number) {
  return {
    record_type: "measurement_sample",
    record_id: recordId,
    created_at: at,
    changed_by: tool,
    data: {
      source_id: sourceId,
      received_at: at,
      received_at_basis: "import_time",
      quantity,
      recorded_at: at,
      input_timestamp: receipt(at),
      value: value(standard, unit),
      ...(durationMs === undefined ? {} : { duration_ms: durationMs }),
    },
  };
}

function workout(recordId: string, sourceId: string, start: string, end: string, extra: Record<string, Json> = {}) {
  return {
    record_type: "workout_session",
    record_id: recordId,
    created_at: start,
    changed_by: tool,
    data: {
      source_id: sourceId,
      received_at: end,
      received_at_basis: "import_time",
      ...("provider_reporting_periods" in extra ? { provider_reporting_periods: extra.provider_reporting_periods } : {}),
      ...("supplied_fields" in extra ? { supplied_fields: extra.supplied_fields } : {}),
      start_at: start,
      end_at: end,
      input_start_timestamp: receipt(start),
      input_end_timestamp: receipt(end),
      activity_type: "running",
      ...Object.fromEntries(
        Object.entries(extra).filter(([key]) => !["provider_reporting_periods", "supplied_fields", "source_reported_summaries"].includes(key)),
      ),
      source_reported_summaries: extra.source_reported_summaries ?? [],
    },
  };
}

function sleep(recordId: string, sourceId: string, start: string, end: string, extra: Record<string, Json> = {}) {
  return {
    record_type: "sleep_session",
    record_id: recordId,
    created_at: end,
    changed_by: tool,
    data: {
      source_id: sourceId,
      received_at: end,
      received_at_basis: "import_time",
      ...("provider_reporting_periods" in extra ? { provider_reporting_periods: extra.provider_reporting_periods } : {}),
      start_at: start,
      end_at: end,
      input_start_timestamp: receipt(start),
      input_end_timestamp: receipt(end),
      source_reported_summaries: extra.source_reported_summaries ?? [],
      ...Object.fromEntries(
        Object.entries(extra).filter(([key]) => !["provider_reporting_periods", "source_reported_summaries"].includes(key)),
      ),
    },
  };
}

function report(recordId: string, at: string, extra: Record<string, Json> = {}) {
  return {
    record_type: "athlete_report",
    record_id: recordId,
    created_at: at,
    changed_by: athlete,
    data: { report_type: "note", reported_at: at, input_reported_at: receipt(at), reported_via: "text", ...extra },
  };
}

function definition(recordId: string, key: string, at: string, text = `${key} for time`) {
  return {
    record_type: "benchmark_definition",
    record_id: recordId,
    created_at: at,
    changed_by: tool,
    data: {
      authorship: { author: athlete, authored_at: at, evidence: [], content_version: 1, status: "active" },
      benchmark_key: key,
      benchmark_type: "named_wod",
      score_type: "time",
      definition: text,
    },
  };
}

function result(recordId: string, sourceId: string, benchmarkId: string, at: string, seconds: number) {
  return {
    record_type: "benchmark_result",
    record_id: recordId,
    created_at: at,
    changed_by: athlete,
    data: {
      source_id: sourceId,
      received_at: at,
      received_at_basis: "local_write_time",
      benchmark_definition_id: benchmarkId,
      recorded_at: at,
      input_timestamp: receipt(at),
      score: { duration_s: seconds },
    },
  };
}

function prediction(recordId: string, benchmarkId: string, at: string, seconds: number) {
  return {
    record_type: "prediction",
    record_id: recordId,
    created_at: at,
    changed_by: { author_type: "agent", agent: "test-agent" },
    data: {
      authorship: {
        author: { author_type: "agent", agent: "test-agent", model: "test-model" },
        authored_at: at,
        evidence: [],
        content_version: 1,
        status: "active",
      },
      benchmark_definition_id: benchmarkId,
      predicted_score: { duration_s: seconds },
      predicted_range: { low: { duration_s: seconds - 10 }, high: { duration_s: seconds + 10 } },
      confidence: "moderate",
      reasoning: "Invented test prediction",
    },
  };
}

// ---------------------------------------------------------------------------
// Running the reference
// ---------------------------------------------------------------------------

function plain(value: unknown): Json {
  return JSON.parse(JSON.stringify(value)) as Json;
}

function errorText(error: unknown): string {
  return validation.formatSchemaValidationError(error) as string;
}

async function capture(run: () => Promise<unknown> | unknown): Promise<Json> {
  try {
    return plain(await run());
  } catch (error) {
    return { error: errorText(error) };
  }
}

async function replay(database: unknown, steps: Step[]): Promise<void> {
  for (const step of steps) {
    if ("write" in step) await db.writeRecords(database, step.write);
    else if ("correct" in step) await db.correctRecord(database, step.correct);
    else await db.deleteRecord(database, step.delete);
  }
}

type Runner = (database: { localPath: string }, input: Record<string, unknown>, directory: string) => Promise<Json>;

const runners: Record<string, Runner> = {
  calculate: async (database, input) =>
    capture(async () => {
      const receipt = await calculation.calculate(database, input);
      return { receipt, rendered: calculation.renderCalculation(receipt) };
    }),
  apply: async (database, input, directory) =>
    capture(() => {
      const cwd = join(directory, `apply-${Math.random().toString(16).slice(2)}`);
      mkdirSync(cwd);
      for (const [name, text] of Object.entries((input.installed ?? {}) as Record<string, string>)) {
        mkdirSync(join(cwd, ".agents", "skills", name), { recursive: true });
        writeFileSync(join(cwd, ".agents", "skills", name, "SKILL.md"), text);
      }
      const file = skill.resolveCalculationFile(input.calculation as string, cwd);
      return core.runAthCore(["calculate", "--database", database.localPath, "--skill", file, "--record", input.record_id as string]);
    }),
  series: async (database, input) =>
    capture(async () => {
      const view = await db.readSeriesView(database, input.quantity, input.options ?? {});
      return { view, rendered: db.renderSeriesView(view) };
    }),
  "calendar-range": async (database, input) => capture(() => db.resolveCalendarRange(database, input)),
  "as-of": async (database, input) => capture(() => db.resolveAsOfTime(database, input.value, input.time_zone)),
  "query-measurements": async (database, input) => capture(() => db.queryMeasurements(database, input)),
  "query-series": async (database, input) => capture(() => db.querySeries(database, input)),
  "query-sessions": async (database, input) => capture(() => db.querySessions(database, input)),
  "query-as-of": async (database, input) => capture(() => db.queryRecordsAsOf(database, input)),
  "records-list": async (database, input) =>
    capture(async () => {
      const records = await db.listRecords(database, input);
      return { list: { file: database.localPath, records }, rendered: db.renderRecordList(records) };
    }),
  "records-show": async (database, input) =>
    capture(async () => {
      const recordId = input.record_id as string;
      const record = await db.readRecord(database, recordId);
      if (record === null) throw new Error(`record not found: ${recordId}`);
      const revisions =
        input.revision !== undefined || input.history ? await db.readRecordRevisions(database, recordId) : undefined;
      const shown =
        input.revision === undefined
          ? undefined
          : revisions.find((revision: { revision: number }) => revision.revision === input.revision);
      if (input.revision !== undefined && shown === undefined) {
        throw new Error(`record ${recordId} has no revision ${input.revision}; current revision is ${record.current_revision}`);
      }
      const links = input.links ? await db.readRecordLinks(database, recordId) : undefined;
      return {
        show: {
          file: database.localPath,
          record,
          ...(shown === undefined ? {} : { shown_revision: shown }),
          ...(input.history ? { history: revisions } : {}),
          ...(links === undefined ? {} : { links }),
        },
        rendered: db.renderRecord(record, {
          ...(shown === undefined ? {} : { revision: shown }),
          ...(input.history && revisions !== undefined ? { history: revisions } : {}),
          ...(links === undefined ? {} : { links }),
        }),
      };
    }),
  evidence: async (database, input) => capture(() => db.buildEvidence(database, input)),
  "evidence-reference": async (database, input) => capture(() => db.resolveEvidenceReference(database, input)),
  "grade-plan": async (database, input) =>
    capture(async () => {
      const plan = await db.planPredictionGrade(database, { ...input, actor: ath });
      return gradePreview(database.localPath, plan);
    }),
  "grade-write": async (database, input) =>
    capture(async () => {
      const plan = await db.planPredictionGrade(database, { ...input, actor: ath });
      const written = await db.gradePrediction(database, plan);
      return {
        link: { record_id: written.link.record_id, current_revision: written.link.current_revision },
        grade: written.grade,
        database_revision: written.database_revision,
      };
    }),
  backtest: async (database, input) =>
    capture(async () => {
      const requests: Json[] = [];
      const replies = input.replies as Record<string, string>;
      const report = await db.runV1Backtest(database, {
        models: input.models,
        gateway: "vercel",
        created_at: input.created_at,
        complete: async (request: { model: string }) => {
          requests.push(plain(request));
          const reply = replies[request.model];
          if (reply === undefined || reply.startsWith("error:")) throw new Error((reply ?? "error:no reply").slice(6));
          return { content: reply, model: request.model };
        },
      });
      return { report, requests };
    }),
  "log-plan": async (database, input) =>
    capture(async () => {
      const plan = await db.planLogEntry(database, input);
      return { interpretation: plan.interpretation, records: logRecords(plan) };
    }),
  "log-write": async (database, input) =>
    capture(async () => {
      const plan = await db.planLogEntry(database, input);
      const written = await db.applyLogPlan(database, plan);
      return {
        interpretation: plan.interpretation,
        records: written.records.map((record: { record_type: string }) => record.record_type),
        database_revision: written.database_revision,
      };
    }),
};

function logRecords(plan: { inputs: Array<Record<string, unknown>> }): Json {
  return plain(
    plan.inputs.map((record) => ({
      record_id: record.record_id,
      record_type: record.record_type,
      created_at: record.created_at,
      data: record.data,
      ...(record.extensions === undefined ? {} : { extensions: record.extensions }),
    })),
  );
}

function gradePreview(file: string, plan: Record<string, any>): Json {
  return plain({
    file,
    prediction: { record_id: plan.prediction.record_id, revision: plan.prediction.current_revision },
    result: { record_id: plan.result.record_id, revision: plan.result.current_revision },
    prediction_result_link:
      plan.link_input !== null
        ? { action: "create", record_id: plan.link_input.record_id, data: plan.link_input.data }
        : plan.link_correction !== null
          ? {
              action: "revise",
              record_id: plan.existing_link.record_id,
              current_revision: plan.existing_link.current_revision,
              new_revision: plan.existing_link.current_revision + 1,
              data: plan.link_correction.data,
            }
          : { action: "reuse", record_id: plan.existing_link.record_id, revision: plan.existing_link.current_revision },
    grade: { record_id: plan.grade_input.record_id, data: plan.grade_input.data },
    write_confirmed: false,
  });
}

const DEMO = join(repository, "examples", "demo-athlete", "athlete.ath.sqlite");

async function generate(
  file: string,
  scenario: { base?: "demo"; created_at: string; steps: Step[]; cases: Case[] },
): Promise<void> {
  const directory = realpathSync(mkdtempSync(join(realpathSync(tmpdir()), "ath-golden-")));
  const path = join(directory, "athlete.ath.sqlite");
  let database;
  if (scenario.base === "demo") {
    copyFileSync(DEMO, path);
    database = await db.openDatabase(path, { onSecurityWarning: () => undefined });
  } else {
    database = await db.createDatabase(path, { createdAt: scenario.created_at });
  }
  try {
    await replay(database, scenario.steps);
    for (const item of scenario.cases) {
      if ("steps" in item) {
        await replay(database, item.steps);
        continue;
      }
      const runner = runners[item.call];
      if (runner === undefined) throw new Error(`unknown call ${item.call}`);
      item.output = await runner(database, item.input as Record<string, unknown>, directory);
    }
  } finally {
    database.close();
    rmSync(directory, { recursive: true, force: true });
  }
  const target = join(goldenRoot, file);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(
    target,
    `${JSON.stringify(
      {
        generated_by: "native/tools/reference/calculation-golden.ts",
        reference_commit: REFERENCE_COMMIT,
        ...(scenario.base === undefined ? {} : { base: scenario.base }),
        created_at: scenario.created_at,
        steps: scenario.steps,
        cases: scenario.cases,
      },
      null,
      2,
    )}\n`,
  );
  console.log(`wrote ${target}`);
}

// ---------------------------------------------------------------------------
// Range calculations and queries
// ---------------------------------------------------------------------------

const A = id(0x101);
const B = id(0x102);
const M = id(0x103);
const P = id(0x104);
const W = id(0x150);
const W2 = id(0x151);
const N1 = id(0x160);

const whoopCycle = (n: number, start: string, end: string) => ({
  period_type: "whoop_physiological_cycle",
  provider_period_id: `cycle-${n}`,
  start_at: start,
  end_at: end,
});
const ouraDay = (date: string) => ({ period_type: "oura_sleep_day", provider_date: date });

function score(recordId: string, metric: string, at: string | null, standard: number, periods?: Json[]) {
  const createdAt = "2026-10-07T00:00:00.000Z";
  return {
    record_type: "provider_score",
    record_id: recordId,
    created_at: createdAt,
    changed_by: tool,
    data: {
      source_id: P,
      received_at: createdAt,
      received_at_basis: "import_time",
      ...(periods === undefined ? {} : { provider_reporting_periods: periods }),
      metric,
      value: value(standard, "%"),
      scale: "0-100",
      ...(at === null ? {} : { recorded_at: at }),
      input_timestamp: at === null ? noTimestamp : receipt(at),
    },
  };
}

function summary(recordId: string, summaryType: string, at: string | null, standard: number, unit: string, periods?: Json[]) {
  const createdAt = "2026-10-07T00:00:00.000Z";
  return {
    record_type: "provider_summary",
    record_id: recordId,
    created_at: createdAt,
    changed_by: tool,
    data: {
      source_id: P,
      received_at: createdAt,
      received_at_basis: "import_time",
      ...(periods === undefined ? {} : { provider_reporting_periods: periods }),
      summary_type: summaryType,
      value: value(standard, unit),
      ...(at === null ? {} : { recorded_at: at }),
      input_timestamp: at === null ? noTimestamp : receipt(at),
    },
  };
}

const rangeSteps: Step[] = [
  {
    write: [
      source(A, "Device A"),
      source(B, "Device B"),
      source(M, "ath", "manual"),
      source(P, "Provider", "connector"),
    ],
  },
  {
    write: [
      point(id(0x201), A, "hrv_rmssd", "2026-10-01T12:00:00.000Z", 60, "ms"),
      point(id(0x202), A, "hrv_rmssd", "2026-10-02T12:00:00.000Z", 70, "ms"),
      point(id(0x203), A, "hrv_rmssd", "2026-10-03T04:30:00.000Z", 65, "ms"),
      point(id(0x204), A, "hrv_rmssd", "2026-10-04T12:00:00.000Z", 500, "ms"),
      point(id(0x205), A, "hrv_rmssd", "2026-10-05T12:00:00.000Z", 75, "ms"),
      point(id(0x206), B, "hrv_rmssd", "2026-10-01T12:00:00.000Z", 50, "ms"),
      point(id(0x207), B, "hrv_rmssd", "2026-10-02T12:00:00.000Z", 55, "ms"),
      point(id(0x208), B, "hrv_rmssd", "2026-10-04T12:00:00.000Z", 52.5, "ms"),
      point(id(0x209), M, "hrv_rmssd", "2026-10-01T12:00:00.000Z", 999, "ms"),
      point(id(0x20a), A, "hrv_sdnn", "2026-10-01T12:00:00.000Z", 40, "ms"),
      point(id(0x20b), A, "hrv_sdnn", "2026-10-02T12:00:00.000Z", 45.25, "ms"),
      point(id(0x20c), A, "resting_heart_rate", "2026-10-01T12:00:00.000Z", 52, "bpm"),
      point(id(0x20d), A, "resting_heart_rate", "2026-10-02T12:00:00.000Z", 50, "bpm"),
      point(id(0x20e), A, "resting_heart_rate", "2026-10-03T04:30:00.000Z", 51, "bpm"),
      point(id(0x20f), A, "resting_heart_rate", "2026-10-05T12:00:00.000Z", 49, "bpm"),
      point(id(0x210), P, "hrv_rmssd", null, 44, "ms", { provider_reporting_periods: [ouraDay("2026-10-03")] }),
      point(id(0x211), P, "hrv_rmssd", "2026-10-04T06:00:00.000Z", 46, "ms", {
        provider_reporting_periods: [ouraDay("2026-10-04")],
      }),
    ],
  },
  {
    write: [
      workout(W, A, "2026-10-06T12:00:00.000Z", "2026-10-06T12:20:00.000Z"),
      sample(id(0x301), A, "heart_rate", "2026-10-06T12:05:00.000Z", 120, "bpm"),
      sample(id(0x302), A, "heart_rate", "2026-10-06T12:08:00.000Z", 124, "bpm"),
      sample(id(0x303), A, "heart_rate", "2026-10-06T12:15:00.000Z", 140, "bpm"),
      sample(id(0x304), A, "heart_rate", "2026-10-06T12:18:00.000Z", 138, "bpm"),
      sample(id(0x305), A, "heart_rate", "2026-10-06T12:30:00.000Z", 100, "bpm"),
      sample(id(0x306), B, "heart_rate", "2026-10-06T12:02:00.000Z", 110, "bpm"),
      sample(id(0x307), M, "heart_rate", "2026-10-06T12:16:00.000Z", 200, "bpm"),
      sample(id(0x308), A, "steps", "2026-10-06T12:00:00.000Z", 420, "count", 300000),
      workout(W2, P, "2026-10-04T13:00:00.000Z", "2026-10-04T14:00:00.000Z", {
        provider_reporting_periods: [{ period_type: "oura_activity_day", provider_date: "2026-10-04" }],
        source_reported_summaries: [{ ...value(5000, "m"), summary_type: "total_distance" }],
      }),
      sleep(N1, P, "2026-10-03T04:00:00.000Z", "2026-10-03T12:00:00.000Z", {
        provider_reporting_periods: [{ period_type: "oura_activity_day", provider_date: "2026-10-03" }],
        source_reported_summaries: [{ ...value(4500, "m"), summary_type: "total_distance" }],
      }),
      score(id(0x401), "recovery", "2026-10-01T08:00:00.000Z", 70),
      score(id(0x402), "recovery", "2026-10-02T08:00:00.000Z", 80),
      score(id(0x403), "recovery", null, 60, [whoopCycle(1, "2026-10-02T22:00:00.000Z", "2026-10-03T21:00:00.000Z")]),
      score(id(0x404), "recovery", null, 64, [whoopCycle(1, "2026-10-02T22:00:00.000Z", "2026-10-03T21:00:00.000Z")]),
      score(id(0x405), "recovery", null, 90, [whoopCycle(2, "2026-10-03T21:00:00.000Z", "2026-10-04T22:00:00.000Z")]),
      summary(id(0x406), "sleep_need", null, 480, "min", [ouraDay("2026-10-03")]),
      summary(id(0x407), "sleep_need", null, 450, "min", [ouraDay("2026-10-02")]),
    ],
  },
  {
    correct: {
      record_id: id(0x205),
      expected_revision: 1,
      changed_by: tool,
      became_known_at: "2026-10-08T00:00:00.000Z",
      data: point(id(0x205), A, "hrv_rmssd", "2026-10-05T12:00:00.000Z", 80, "ms").data,
    },
  },
  { delete: { record_id: id(0x204), expected_revision: 1, deleted_by: tool, deleted_at: "2026-10-08T00:00:00.000Z" } },
];

const tz = "America/Chicago";
const calc = (name: string, input: Record<string, Json>): Case => ({ name, call: "calculate", input });
const rangeCases: Case[] = [
  calc("baseline keeps sources apart and leaves manual out", { operation: "baseline", quantity: "hrv_rmssd" }),
  calc("summary per source", { operation: "summary", quantity: "hrv_rmssd" }),
  calc("summary by day", { operation: "summary", quantity: "hrv_rmssd", group: "day", time_zone: tz }),
  calc("summary by week in a calendar range", {
    operation: "summary",
    quantity: "hrv_rmssd",
    group: "week",
    time_zone: tz,
    from: "2026-10-01",
    to: "2026-10-05",
  }),
  calc("baseline by month", { operation: "baseline", quantity: "hrv_rmssd", group: "month", time_zone: tz }),
  calc("summary with raw text bounds", { operation: "summary", quantity: "hrv_rmssd", from: "2026-10-02", to: "2026-10-03" }),
  calc("summary with exact bounds and a timezone", {
    operation: "summary",
    quantity: "hrv_rmssd",
    from: "2026-10-02T00:00:00.000Z",
    to: "2026-10-05T12:00:00.000Z",
    time_zone: tz,
  }),
  calc("summary keeps SDNN apart", { operation: "summary", quantity: "hrv_sdnn" }),
  calc("summary of a provider score", { operation: "summary", quantity: "recovery", record_kind: "provider-score" }),
  calc("summary of a series quantity", { operation: "summary", quantity: "heart_rate", source_id: A }),
  calc("summary with nothing recorded", { operation: "summary", quantity: "body_weight" }),
  calc("summary of an unknown quantity", { operation: "summary", quantity: "hrv" }),
  calc("calendar groups refuse a lower-case zone name", {
    operation: "summary",
    quantity: "hrv_rmssd",
    group: "day",
    time_zone: "america/chicago",
  }),
  calc("calendar bounds in the wrong order", {
    operation: "summary",
    quantity: "hrv_rmssd",
    group: "day",
    time_zone: tz,
    from: "2026-10-05",
    to: "2026-10-01",
  }),
  calc("trend per source", { operation: "trend", quantity: "hrv_rmssd" }),
  calc("trend of one source", { operation: "trend", quantity: "hrv_rmssd", source_id: B }),
  calc("trend needs two inputs", { operation: "trend", quantity: "recovery", record_kind: "provider-score", from: "2026-10-02" }),
  calc("coverage in a calendar range", {
    operation: "coverage",
    quantity: "hrv_rmssd",
    source_id: A,
    time_zone: tz,
    from: "2026-10-01",
    to: "2026-10-06",
  }),
  calc("coverage without bounds", { operation: "coverage", quantity: "hrv_rmssd", time_zone: tz }),
  calc("coverage needs a timezone", { operation: "coverage", quantity: "hrv_rmssd", source_id: A }),
  calc("compare sources", { operation: "compare-sources", quantity: "hrv_rmssd", source_id: A, compare_source_id: B }),
  calc("compare sources needs both", { operation: "compare-sources", quantity: "hrv_rmssd", source_id: A }),
  calc("compare with an empty source", { operation: "compare-sources", quantity: "hrv_sdnn", source_id: A, compare_source_id: B }),
  calc("correlation of daily means", {
    operation: "correlation",
    quantity: "hrv_rmssd",
    other_quantity: "resting_heart_rate",
    time_zone: tz,
    source_id: A,
  }),
  calc("correlation across all sources", {
    operation: "correlation",
    quantity: "hrv_rmssd",
    other_quantity: "resting_heart_rate",
    time_zone: tz,
  }),
  calc("correlation with an empty comparison", {
    operation: "correlation",
    quantity: "hrv_rmssd",
    other_quantity: "resting_heart_rate",
    time_zone: tz,
    source_id: A,
    compare_source_id: B,
  }),
  calc("correlation needs another quantity", { operation: "correlation", quantity: "hrv_rmssd", time_zone: tz }),
  calc("workout drift", { operation: "workout-drift", quantity: "heart_rate", workout_session_id: W }),
  calc("workout drift for one source", { operation: "workout-drift", quantity: "heart_rate", workout_session_id: W, source_id: A }),
  calc("workout drift needs a session", { operation: "workout-drift", quantity: "heart_rate" }),
  calc("workout drift needs a workout", { operation: "workout-drift", quantity: "heart_rate", workout_session_id: N1 }),
  calc("workout drift needs two samples", { operation: "workout-drift", quantity: "steps", workout_session_id: W }),
  calc("provider period summary of scores", {
    operation: "provider-period-summary",
    quantity: "recovery",
    record_kind: "provider-score",
    provider_period_type: "whoop_physiological_cycle",
  }),
  calc("provider period summary of summaries", {
    operation: "provider-period-summary",
    quantity: "sleep_need",
    record_kind: "provider-summary",
    provider_period_type: "oura_sleep_day",
    from: "2026-10-03",
  }),
  calc("provider period summary of session summaries", {
    operation: "provider-period-summary",
    quantity: "total_distance",
    record_kind: "session-summary",
    provider_period_type: "oura_activity_day",
  }),
  calc("provider period summary of measurements", {
    operation: "provider-period-summary",
    quantity: "hrv_rmssd",
    provider_period_type: "oura_sleep_day",
  }),
  calc("provider period summary with no match", {
    operation: "provider-period-summary",
    quantity: "recovery",
    record_kind: "provider-score",
    provider_period_type: "oura_calendar_day",
  }),
  calc("provider period summary needs a period type", { operation: "provider-period-summary", quantity: "recovery" }),
  calc("a period type needs provider period summary", {
    operation: "summary",
    quantity: "hrv_rmssd",
    provider_period_type: "oura_sleep_day",
  }),
  calc("session summaries need provider period summary", {
    operation: "summary",
    quantity: "total_distance",
    record_kind: "session-summary",
  }),
  { name: "series view needs a timezone", call: "series", input: { quantity: "heart_rate" } },
  { name: "measurements leave manual out", call: "query-measurements", input: { quantity: "hrv_rmssd" } },
  {
    name: "measurements with manual and a window",
    call: "query-measurements",
    input: { quantity: "hrv_rmssd", include_manual: true, from: "2026-10-01T00:00:00.000Z", to: "2026-10-02T23:59:59.999Z", limit: 3 },
  },
  { name: "measurements refuse a bad limit", call: "query-measurements", input: { limit: 0 } },
  { name: "measurements refuse a bad bound", call: "query-measurements", input: { from: "2026-10-01" } },
  { name: "series newest first", call: "query-series", input: { quantity: "heart_rate", order: "descending", limit: 3 } },
  { name: "sessions in start order", call: "query-sessions", input: {} },
  { name: "workout sessions only", call: "query-sessions", input: { session_type: "workout_session", source_id: A } },
  {
    name: "records as known before the correction",
    call: "query-as-of",
    input: { as_of: "2026-10-07T12:00:00.000Z", record_types: ["point_measurement"], limit: 50 },
  },
  {
    name: "records as known after the correction and deletion",
    call: "query-as-of",
    input: { as_of: "2026-10-08T00:00:00.000Z", record_types: ["point_measurement"], limit: 50 },
  },
  { name: "records as known before any write", call: "query-as-of", input: { as_of: "2025-01-01T00:00:00.000Z" } },
];

// ---------------------------------------------------------------------------
// One-record calculations
// ---------------------------------------------------------------------------

const zonesText = readFileSync(join(root, "skills", "heart-rate-zones", "SKILL.md"), "utf8");
const night = (n: number) => {
  const day = (offset: number) => `2026-10-${String(n + offset).padStart(2, "0")}`;
  const start = `${day(0)}T0${n % 3}:${n % 2 === 0 ? "00" : "30"}:00.000Z`;
  const asleep = 25_200 + n * 600;
  return sleep(id(0x500 + n), A, start, `${day(0)}T09:00:00.000Z`, {
    asleep_seconds: asleep,
    in_bed_seconds: asleep + 1800,
    interruptions: n % 4,
    stage_minutes: { deep: 60 + n, rem: 90, light: 200 - n },
  });
};
const applySteps: Step[] = [
  { write: [source(A, "Device A"), source(B, "Device B")] },
  {
    write: [
      workout(id(0x601), A, "2026-10-01T12:00:00.000Z", "2026-10-01T12:00:03.000Z", {
        maximum_heart_rate_bpm: 200,
        resting_heart_rate_bpm: 50,
        heart_rate_samples: [
          { offset_ms: 0, bpm: 100 },
          { offset_ms: 1000, bpm: 130 },
          { offset_ms: 2000, bpm: 160 },
          { offset_ms: 3000, bpm: 190 },
        ],
      }),
      workout(id(0x602), A, "2026-10-02T12:00:00.000Z", "2026-10-02T12:02:00.000Z", {
        heart_rate_samples: [0, 10, 20, 30, 40, 50, 60, 70, 80, 90].map((seconds) => ({
          offset_ms: seconds * 1000,
          bpm: seconds < 75 ? 55 : 70 + seconds / 10,
        })),
      }),
      workout(id(0x603), A, "2026-10-03T12:00:00.000Z", "2026-10-03T12:30:00.000Z", {
        activity_type: "strength",
        sets: [
          { reps: 5, weight: 100, weight_unit: "kg" },
          { reps: 5, weight: 110.5, weight_unit: "kg" },
        ],
      }),
      workout(id(0x604), A, "2026-10-04T12:00:00.000Z", "2026-10-04T12:30:00.000Z", { text_result: "5 rounds and 3 reps" }),
      workout(id(0x605), A, "2026-10-05T12:00:00.000Z", "2026-10-05T12:30:00.000Z", {
        supplied_fields: { Calories: 512, Gear: "road bike" },
      }),
      report(id(0x606), "2026-10-05T18:00:00.000Z", { rating: 7, rating_scale: "1-10", duration_seconds: 3600 }),
      ...[1, 2, 3, 4, 5, 6, 7, 8].map(night),
      sleep(id(0x509), B, "2026-10-04T04:00:00.000Z", "2026-10-04T11:00:00.000Z", { asleep_seconds: 20_000 }),
      point(id(0x701), A, "hrv_rmssd", "2026-10-01T12:00:00.000Z", 60, "ms"),
      point(id(0x702), A, "hrv_rmssd", "2026-10-02T12:00:00.000Z", 66, "ms"),
      point(id(0x703), A, "hrv_rmssd", "2026-10-03T12:00:00.000Z", 70.123, "ms"),
      point(id(0x704), A, "hrv_sdnn", "2026-10-03T12:00:00.000Z", 40, "ms"),
      point(id(0x705), B, "resting_heart_rate", "2026-10-03T12:00:00.000Z", 50, "bpm"),
      point(id(0x706), A, "resting_heart_rate", "2026-10-04T12:00:00.000Z", 48, "bpm"),
    ],
  },
  { delete: { record_id: id(0x706), expected_revision: 1, deleted_by: tool, deleted_at: "2026-10-08T00:00:00.000Z" } },
];
const apply = (name: string, calculationName: string, recordId: string, installed?: Record<string, string>): Case => ({
  name,
  call: "apply",
  input: { calculation: calculationName, record_id: recordId, ...(installed === undefined ? {} : { installed }) },
});
const applyCases: Case[] = [
  apply("zones worked example", "heart-rate-zones", id(0x601)),
  apply("zones without a maximum", "heart-rate-zones", id(0x602)),
  apply("zones by path and suffix", "skills/heart-rate-zones.md", id(0x601)),
  apply("recovery needs a later sample", "heart-rate-recovery", id(0x601)),
  apply("resting heart rate held for a minute", "resting-heart-rate", id(0x602)),
  apply("resting heart rate never held", "resting-heart-rate", id(0x601)),
  apply("training load from heart rate", "training-load-heart-rate", id(0x601)),
  apply("training load needs rest and maximum", "training-load-heart-rate", id(0x602)),
  apply("training load from effort", "training-load-effort", id(0x606)),
  apply("training load effort on a workout", "training-load-effort", id(0x601)),
  apply("sleep duration", "sleep-duration", id(0x503)),
  apply("sleep interruptions", "sleep-interruptions", id(0x503)),
  apply("sleep efficiency", "sleep-efficiency", id(0x503)),
  apply("sleep timing", "sleep-timing", id(0x503)),
  apply("sleep stages", "sleep-stages", id(0x503)),
  apply("sleep regularity", "sleep-regularity", id(0x508)),
  apply("sleep regularity needs seven nights", "sleep-regularity", id(0x509)),
  apply("sleep change", "sleep-change", id(0x508)),
  apply("sleep change with no other night", "sleep-change", id(0x509)),
  apply("hrv change", "hrv-change", id(0x703)),
  apply("hrv change keeps SDNN apart", "hrv-change", id(0x704)),
  apply("resting heart rate change", "resting-heart-rate-change", id(0x705)),
  apply("strength volume", "strength-volume", id(0x603)),
  apply("strength volume from text", "strength-volume", id(0x604)),
  apply("a range calculation on one record", "summary", id(0x601)),
  apply("the tool skill is not a calculation", "athletic-standard", id(0x601)),
  apply("an unknown calculation name", "no-such-calculation", id(0x601)),
  apply("a deleted record", "hrv-change", id(0x706)),
  apply("a missing record", "hrv-change", id(0xfff)),
  apply("an installed copy wins", "heart-rate-zones", id(0x601), {
    "heart-rate-zones": zonesText.replace("zone: 0.60", "zone: 0.50"),
  }),
  apply("an installed calculation reads a supplied field", "calories", id(0x605), {
    calories: "```ath-calculation\nname: calories\nuse: field\nfield: supplied:Calories\n```\n",
  }),
  apply("an installed calculation cannot read a source label", "feeling", id(0x605), {
    feeling: "```ath-calculation\nname: feeling\nuse: field\nfield: supplied:Feeling\n```\n",
  }),
  apply("an installed file without a block", "broken", id(0x605), { broken: "# no block\n" }),
];

// ---------------------------------------------------------------------------
// Series view
// ---------------------------------------------------------------------------

const seriesSteps: Step[] = [
  {
    write: [
      {
        record_type: "athlete_profile",
        record_id: id(0x100),
        created_at: "2026-01-01T00:00:00.000Z",
        changed_by: tool,
        data: { name: "Invented Athlete", default_time_zone: tz },
      },
      source(A, "Device A"),
      source(B, "Device B"),
      source(M, "ath", "manual"),
    ],
  },
  {
    write: [
      sample(id(0x801), A, "heart_rate", "2026-10-01T04:00:00.000Z", 61, "bpm"),
      sample(id(0x802), A, "heart_rate", "2026-10-01T05:30:00.000Z", 58, "bpm"),
      sample(id(0x803), A, "heart_rate", "2026-10-01T12:00:00.000Z", 97.25, "bpm"),
      sample(id(0x804), A, "heart_rate", "2026-10-02T12:00:00.000Z", 70, "bpm"),
      sample(id(0x805), A, "heart_rate", "2026-10-02T12:00:01.000Z", 71, "bpm"),
      sample(id(0x806), B, "heart_rate", "2026-10-01T12:00:00.000Z", 66, "bpm"),
      sample(id(0x807), M, "heart_rate", "2026-10-01T12:00:00.000Z", 150, "bpm"),
      sample(id(0x808), A, "steps", "2026-10-01T12:00:00.000Z", 420, "count", 300000),
    ],
  },
];
const series = (name: string, quantity: string, options: Record<string, Json> = {}): Case => ({
  name,
  call: "series",
  input: { quantity, options },
});
const seriesCases: Case[] = [
  series("days in the profile timezone", "heart_rate"),
  series("days in UTC", "heart_rate", { time_zone: "UTC" }),
  series("days in a fixed offset", "heart_rate", { time_zone: "+05:30" }),
  series("raw samples", "heart_rate", { raw: true }),
  series("raw samples in a calendar day", "heart_rate", { raw: true, from: "2026-10-01", to: "2026-10-01" }),
  series("one source", "heart_rate", { source_id: B }),
  series("calendar bounds", "heart_rate", { from: "2026-10-01", to: "2026-10-01" }),
  series("exact bounds", "heart_rate", { from: "2026-10-01T05:00:00.000Z" }),
  series("a limit", "heart_rate", { limit: 2 }),
  series("raw with a limit", "heart_rate", { raw: true, limit: 2 }),
  series("nothing recorded", "hrv_beats"),
  series("spans", "steps"),
  series("an unknown timezone", "heart_rate", { time_zone: "Mars/Olympus" }),
  series("an invalid calendar date", "heart_rate", { from: "2026-02-30" }),
  series("a bound that is not a time", "heart_rate", { from: "yesterday" }),
  series("bounds in the wrong order", "heart_rate", { from: "2026-10-02", to: "2026-10-01" }),
  {
    name: "calendar range uses the profile timezone",
    call: "calendar-range",
    input: { from: "2026-03-08", to: "2026-03-08" },
  },
  {
    name: "calendar range across the autumn change",
    call: "calendar-range",
    input: { from: "2026-11-01", to: "2026-11-01", time_zone: tz },
  },
  { name: "as-of a calendar day", call: "as-of", input: { value: "2026-10-01" } },
  { name: "as-of a calendar day elsewhere", call: "as-of", input: { value: "2026-10-01", time_zone: "Asia/Kolkata" } },
  { name: "as-of an exact time", call: "as-of", input: { value: "2026-10-01T12:00:00.000Z" } },
  { name: "as-of text that is not a time", call: "as-of", input: { value: "noon" } },
];

// ---------------------------------------------------------------------------
// Records, links, and grading on the demo database
// ---------------------------------------------------------------------------

const demoIds = Array.from({ length: 0x19 }, (_, index) => id(index + 2));
const recordsCases: Case[] = [
  { name: "list current records", call: "records-list", input: {} },
  { name: "list with deleted records", call: "records-list", input: { include_deleted: true, limit: 10000 } },
  { name: "list one type", call: "records-list", input: { record_type: "point_measurement" } },
  { name: "list a short page", call: "records-list", input: { limit: 3 } },
  { name: "list refuses a large limit", call: "records-list", input: { limit: 10001 } },
  ...demoIds.map(
    (recordId): Case => ({
      name: `show ${recordId} with history and links`,
      call: "records-show",
      input: { record_id: recordId, history: true, links: true },
    }),
  ),
  { name: "show one record", call: "records-show", input: { record_id: id(0x07) } },
  { name: "show a retained revision", call: "records-show", input: { record_id: id(0x07), revision: 1 } },
  { name: "show a revision with links", call: "records-show", input: { record_id: id(0x07), revision: 2, links: true } },
  { name: "show a revision that does not exist", call: "records-show", input: { record_id: id(0x07), revision: 9 } },
  { name: "show a record that does not exist", call: "records-show", input: { record_id: id(0xfff) } },
  { name: "show an invalid record ID", call: "records-show", input: { record_id: "not-an-id" } },
];

const PREDICTION = id(0x10);
const RESULT = id(0x0b);
const gradeCases: Case[] = [
  {
    name: "reuse the exact link",
    call: "grade-plan",
    input: { prediction_id: PREDICTION, result_id: RESULT, created_at: "2026-10-13T12:00:00.000Z" },
  },
  {
    name: "a new calculation version",
    call: "grade-plan",
    input: {
      prediction_id: PREDICTION,
      result_id: RESULT,
      calculation_version: "prediction-grade-v2-test",
      created_at: "2026-10-13T12:00:00.000Z",
    },
  },
  { name: "a prediction ID that is a result", call: "grade-plan", input: { prediction_id: RESULT, result_id: RESULT } },
  { name: "a result ID that is a prediction", call: "grade-plan", input: { prediction_id: PREDICTION, result_id: PREDICTION } },
  { name: "a missing prediction", call: "grade-plan", input: { prediction_id: id(0xfff), result_id: RESULT } },
  {
    name: "write a reused-link grade",
    call: "grade-write",
    input: { prediction_id: PREDICTION, result_id: RESULT, created_at: "2026-10-13T12:00:00.000Z" },
  },
  {
    steps: [
      {
        correct: {
          record_id: RESULT,
          expected_revision: 1,
          changed_by: athlete,
          became_known_at: "2026-10-14T00:00:00.000Z",
          data: {
            source_id: id(0x04),
            received_at: "2026-08-10T18:00:00.000Z",
            received_at_basis: "local_write_time",
            field_basis: { value: "entered_by_hand" },
            benchmark_definition_id: id(0x06),
            recorded_at: "2026-08-10T17:48:00.000Z",
            input_timestamp: receipt("2026-08-10T17:48:00.000Z"),
            score: { duration_s: 301 },
            scaling: "rx",
            workout_session_id: id(0x0a),
          },
        },
      },
    ],
  },
  {
    name: "revise the link for a corrected result",
    call: "grade-plan",
    input: { prediction_id: PREDICTION, result_id: RESULT, created_at: "2026-10-14T12:00:00.000Z" },
  },
  {
    name: "write the revised link and grade",
    call: "grade-write",
    input: { prediction_id: PREDICTION, result_id: RESULT, created_at: "2026-10-14T12:00:00.000Z" },
  },
  { name: "list grades after writing", call: "records-list", input: { record_type: "prediction_grade" } },
  { name: "show the revised link", call: "records-show", input: { record_id: id(0x11), history: true, links: true } },
];

// ---------------------------------------------------------------------------
// Prediction evidence and backtest
// ---------------------------------------------------------------------------

const FRAN = id(0x901);
const GRACE = id(0x902);
const evidenceSteps: Step[] = [
  {
    write: [
      source(A, "Device A", "wearable", "2026-01-01T00:00:00.000Z"),
      source(M, "ath", "manual", "2026-01-01T00:00:00.000Z"),
      definition(FRAN, "fran", "2026-01-01T00:00:00.000Z", "21-15-9 thrusters and pull-ups"),
      definition(GRACE, "grace", "2026-01-01T00:00:00.000Z"),
    ],
  },
  { write: [point(id(0xa01), A, "hrv_rmssd", "2026-01-15T12:00:00.000Z", 50, "ms")] },
  { write: [known(result(id(0xb01), M, FRAN, "2026-06-01T10:00:00.000Z", 300))] },
  { write: [point(id(0xa02), A, "hrv_rmssd", "2026-06-10T12:00:00.000Z", 61, "ms")] },
  { write: [point(id(0xa03), M, "hrv_rmssd", "2026-06-11T12:00:00.000Z", 999, "ms")] },
  {
    correct: {
      record_id: FRAN,
      expected_revision: 1,
      changed_by: athlete,
      became_known_at: "2026-06-15T00:00:00.000Z",
      data: definition(FRAN, "fran", "2026-01-01T00:00:00.000Z", "21-15-9 thrusters 43 kg and pull-ups").data,
    },
  },
  { write: [point(id(0xa04), A, "hrv_rmssd", "2026-06-20T12:00:00.000Z", 60, "ms")] },
  { write: [point(id(0xa05), A, "hrv_rmssd", "2026-06-22T12:00:00.000Z", 58, "ms")] },
  {
    write: [
      point(id(0xa06), A, "hrv_rmssd", null, 57, "ms", { provider_reporting_periods: [ouraDay("2026-06-23")] }, "2026-06-23T12:00:00.000Z"),
    ],
  },
  { write: [sleep(id(0xc01), A, "2026-06-24T04:00:00.000Z", "2026-06-24T11:00:00.000Z", { asleep_seconds: 24000 })] },
  { write: [report(id(0xd01), "2026-06-26T08:00:00.000Z", { report_type: "soreness", rating: 6, rating_scale: "1-10", note: "Sore legs" })] },
  { write: [workout(id(0xe01), A, "2026-06-28T12:00:00.000Z", "2026-06-28T13:00:00.000Z")] },
  { write: [prediction(id(0xf01), FRAN, "2026-06-30T12:00:00.000Z", 290)] },
  { write: [known(result(id(0xb02), M, FRAN, "2026-07-01T10:00:00.000Z", 295))] },
  { delete: { record_id: id(0xa05), expected_revision: 1, deleted_by: athlete, deleted_at: "2026-07-05T00:00:00.000Z" } },
  {
    correct: {
      record_id: id(0xa04),
      expected_revision: 1,
      changed_by: tool,
      became_known_at: "2026-07-10T00:00:00.000Z",
      data: point(id(0xa04), A, "hrv_rmssd", "2026-06-20T12:00:00.000Z", 65, "ms").data,
    },
  },
  { write: [known(result(id(0xb04), M, GRACE, "2026-07-15T10:00:00.000Z", 200))] },
  {
    write: [
      known(
        sleep(id(0xc02), A, "2026-06-25T04:00:00.000Z", "2026-06-25T11:00:00.000Z", { asleep_seconds: 25000 }),
        "2026-07-20T00:00:00.000Z",
      ),
    ],
  },
  { write: [known(result(id(0xb03), M, FRAN, "2026-08-01T10:00:00.000Z", 289))] },
];
const evidence = (name: string, input: Record<string, Json>): Case => ({ name, call: "evidence", input });
const evidenceCases: Case[] = [
  evidence("before the second result", { benchmark: "fran", as_of: "2026-07-01T09:59:59.999Z" }),
  evidence("after corrections, a deletion, and a late import", { benchmark: "fran", as_of: "2026-07-21T00:00:00.000Z" }),
  evidence("by record ID", { benchmark: FRAN, as_of: "2026-06-14T00:00:00.000Z" }),
  evidence("before any result", { benchmark: "fran", as_of: "2026-05-01T00:00:00.000Z" }),
  evidence("a short window", { benchmark: "fran", as_of: "2026-07-01T00:00:00.000Z", window_days: 10 }),
  evidence("a window that is too long", { benchmark: "fran", as_of: "2026-07-01T00:00:00.000Z", window_days: 3651 }),
  evidence("a limit that is reached", { benchmark: "fran", as_of: "2026-07-21T00:00:00.000Z", limit: 6 }),
  evidence("a benchmark not yet known", { benchmark: "fran", as_of: "2025-12-31T00:00:00.000Z" }),
  evidence("an unknown benchmark", { benchmark: "murph", as_of: "2026-07-01T00:00:00.000Z" }),
  evidence("a cut-off that is not a time", { benchmark: "fran", as_of: "2026-07-01" }),
  { name: "a retained revision", call: "evidence-reference", input: { record_id: id(0xa04), revision: 1 } },
  { name: "a revision that does not exist", call: "evidence-reference", input: { record_id: id(0xa04), revision: 3 } },
  {
    name: "backtest with a working, a fenced, and a failing model",
    call: "backtest",
    input: {
      models: ["model-a", "model-b", "model-c", "model-d"],
      created_at: "2026-10-13T12:00:00.000Z",
      replies: {
        "model-a": JSON.stringify({ predicted_score: { duration_s: 290 }, confidence: "moderate", reasoning: "x" }),
        "model-b": "```json\n{\"predicted_score\":{\"duration_s\":301.5}}\n```",
        "model-c": "error:gateway timeout",
        "model-d": "{\"predicted_score\":{\"reps\":5}}",
      },
    },
  },
  {
    name: "backtest needs a model",
    call: "backtest",
    input: { models: [], created_at: "2026-10-13T12:00:00.000Z", replies: {} },
  },
];

// ---------------------------------------------------------------------------
// Log planning
// ---------------------------------------------------------------------------

const initAt = "2026-10-01T00:00:00.000Z";
const logSteps: Step[] = [
  {
    write: [
      {
        record_type: "athlete_profile",
        record_id: id(0x100),
        created_at: initAt,
        changed_by: ath,
        data: { display_units: "metric" },
      },
      {
        record_type: "source",
        record_id: M,
        created_at: initAt,
        changed_by: ath,
        data: { source_type: "manual", data_writer: "ath", data_route: "manual_entry", detail: "Manually recorded data" },
      },
      ...benchmarks.SEED_BENCHMARKS.map((benchmark: Record<string, unknown>, index: number) => ({
        record_type: "benchmark_definition",
        record_id: id(0x110 + index),
        created_at: initAt,
        changed_by: ath,
        data: {
          authorship: {
            author: { ...ath, calculation_version: "benchmark-definition-v1" },
            authored_at: initAt,
            evidence: [],
            content_version: 1,
            status: "active",
          },
          benchmark_key: benchmark.id,
          benchmark_type: benchmark.kind,
          score_type: benchmark.score_type,
          definition: benchmark.definition,
          tags: benchmark.tags,
        },
      })),
      source(A, "Device A", "wearable", initAt),
    ],
  },
];
const at = "2026-10-13T12:00:00.000Z";
const log = (name: string, text: string, extra: Record<string, Json> = {}): Case => ({
  name,
  call: "log-plan",
  input: { text, actor: athlete, created_at: at, ...extra },
});
const note = {
  report_type: "note",
  reported_at: at,
  input_reported_at: receipt(at),
  reported_via: "text",
  note: "Valid note",
};
const logCases: Case[] = [
  log("a manual RMSSD measurement", "RMSSD 61 ms"),
  log("a measurement in its unit without saying so", "resting heart rate 52"),
  log("a measurement with other words after it", "weight 70.5 kg after breakfast"),
  log("a measurement in another unit", "weight 155 lb"),
  log("ambiguous HRV", "HRV 61 ms"),
  log("HRV named with its statistic", "hrv sdnn 41"),
  log("nothing", "   "),
  log("a known benchmark result", "Fran in 5:00 rx"),
  log("a new benchmark result", "Hill repeats 12:30 scaled"),
  log("a result named by the caller", "felt slow 4:41", { benchmark: "Grace" }),
  log("a soreness rating", "sore legs 6/10"),
  log("a stress rating", "stressed at work 3 / 5"),
  log("a mood rating", "mood 7/10"),
  log("an energy rating", "energy 7.5/10"),
  log("a plain rating", "sleep quality 4/5"),
  log("a note", "Felt good today"),
  log("an invalid event time", "Felt good", { created_at: "2026-10-13" }),
  log("one structured record", JSON.stringify({ record_type: "athlete_report", data: note })),
  log(
    "a structured batch keeps an explicit basis",
    JSON.stringify([
      { record_type: "point_measurement", data: point(id(0x1), M, "body_weight", at, 70, "kg").data },
      {
        record_type: "point_measurement",
        record_id: id(0x7777),
        created_at: "2026-10-13T11:00:00.000Z",
        data: { ...point(id(0x1), M, "height", at, 180, "cm").data, field_basis: { value: "provider" } },
      },
    ]),
  ),
  log(
    "structured extensions are kept",
    JSON.stringify({ record_type: "athlete_report", extensions: { "example.com/mood-app": { app_mood: "great" } }, data: note }),
  ),
  log("structured JSON that does not parse", "{\"record_type\":"),
  log("an empty structured batch", "[]"),
  log("a structured item that is not an object", "[1]"),
  log("unknown structured fields", JSON.stringify({ record_type: "athlete_report", mood_emoji: "ok", "2": 1, data: note })),
  log("an unknown record type", JSON.stringify({ record_type: "mood", data: note })),
  log("a caller-created link", JSON.stringify({ record_type: "prediction_result_link", data: {} })),
  log("a caller-created grade", JSON.stringify({ record_type: "prediction_grade", data: {} })),
  log("a second revision", JSON.stringify({ record_type: "athlete_report", current_revision: 2, data: note })),
  log("a deleted record", JSON.stringify({ record_type: "athlete_report", is_deleted: true, data: note })),
  log("an invalid record", JSON.stringify([{ record_type: "athlete_report", data: note }, { record_type: "point_measurement", data: { source_id: "not-a-record-id" } }])),
  log(
    "an invalid extension namespace",
    JSON.stringify({ record_type: "athlete_report", extensions: { "example.com/x": { app_mood: "great" } }, data: note }),
  ),
  log(
    "an extension that claims a source",
    JSON.stringify({ record_type: "athlete_report", extensions: { "example.com/mood-app": { source_id: id(0xaa) } }, data: note }),
  ),
  {
    name: "write an open prediction",
    call: "log-write",
    input: {
      text: JSON.stringify({ record_type: "prediction", data: prediction(id(0x1), id(0x110), "2026-10-12T12:00:00.000Z", 280).data }),
      actor: { author_type: "agent", agent: "test-agent" },
      created_at: "2026-10-12T12:00:00.000Z",
    },
  },
  log("a result grades the one open prediction", "Fran in 5:00 rx"),
  log("a structured result grades the one open prediction", JSON.stringify({
    record_type: "benchmark_result",
    data: { ...result(id(0x1), M, id(0x110), at, 291).data, received_at_basis: "local_write_time" },
  })),
  {
    name: "write a second open prediction",
    call: "log-write",
    input: {
      text: JSON.stringify({ record_type: "prediction", data: prediction(id(0x1), id(0x110), "2026-10-12T13:00:00.000Z", 285).data }),
      actor: { author_type: "agent", agent: "test-agent" },
      created_at: "2026-10-12T13:00:00.000Z",
    },
  },
  log("two open predictions are not graded", "Fran in 5:00 rx"),
  { name: "write a note", call: "log-write", input: { text: "Felt good", actor: athlete, created_at: at } },
];

// ---------------------------------------------------------------------------

await generate("calculate/range.json", { created_at: "2026-01-01T00:00:00.000Z", steps: rangeSteps, cases: rangeCases });
await generate("calculate/apply.json", { created_at: "2026-01-01T00:00:00.000Z", steps: applySteps, cases: applyCases });
await generate("view/series.json", { created_at: "2026-01-01T00:00:00.000Z", steps: seriesSteps, cases: seriesCases });
await generate("view/records.json", { base: "demo", created_at: "", steps: [], cases: recordsCases });
await generate("prediction/grade.json", { base: "demo", created_at: "", steps: [], cases: gradeCases });
await generate("prediction/evidence.json", { created_at: "2026-01-01T00:00:00.000Z", steps: evidenceSteps, cases: evidenceCases });
await generate("log/log.json", { created_at: "2026-01-01T00:00:00.000Z", steps: logSteps, cases: logCases });
