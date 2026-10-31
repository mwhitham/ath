import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createClient } from "@libsql/client";
import { describe, expect, it } from "vitest";
import { athCoreBinary } from "../src/ath-core.js";
import { installMappings } from "../src/skill.js";
import { POINT_MEASUREMENT_UNITS } from "../src/v1/schema.js";

const here = dirname(fileURLToPath(import.meta.url));
const CLI = resolve(here, "../src/cli.ts");
const TSX = resolve(here, "../node_modules/.bin/tsx");
const OW = resolve(here, "fixtures/exports/open-wearables");
const JUNCTION = resolve(here, "fixtures/exports/junction/summaries.json");
const GARMIN = resolve(here, "fixtures/exports/garmin/park-loop.json");
const MAPPINGS = resolve(here, "fixtures/mappings");
const SHIPPED = resolve(here, "../mappings");

const S_RING = "ring|invented-ring|invented-app";
const S_WATCH = "watch|invented-watch|invented-app";
const S_PHONE = "phone|iPhone15,2|Health";
const S_AWATCH = "watch|Watch6,1|Health";
const S_JORA = "ring|ring-j1|oura-app";
const S_JAPPLE = "watch|watch-a|health";
const PERSONAL_REFUSAL = "refusing to turn a personal report into a measured record";
const ENVELOPE_LINE = "envelope is not a timeseries page: pass its data array under workout or sleep";

type Run = { stdout: string; stderr: string; code: number };

function run(command: string, args: string[], cwd?: string): Run {
  try {
    const stdout = execFileSync(command, args, { cwd, encoding: "utf8", stdio: "pipe" });
    return { stdout, stderr: "", code: 0 };
  } catch (error) {
    const err = error as { status: number; stdout?: string; stderr?: string };
    return { stdout: err.stdout ?? "", stderr: err.stderr ?? "", code: err.status ?? 1 };
  }
}

const ath = (args: string[], cwd: string) => run(TSX, [CLI, ...args], cwd);
const core = (args: string[]) => run(athCoreBinary(), args);

function emptyDatabase(options: { skill?: boolean } = {}): string {
  const dir = mkdtempSync(join(tmpdir(), "ath-mapping-"));
  const init = ath(["init", "-y", "--empty", ...(options.skill ? [] : ["--no-skill"])], dir);
  expect(init.code, init.stdout + init.stderr).toBe(0);
  return dir;
}

function mapped(dir: string, input: string, mapping: string): Record<string, unknown> & {
  record_ids: string[];
  left_out: string[];
  understood: string[];
  preserved: string[];
} {
  const preview = ath(["import", input, "--mapping", mapping, "--json"], dir);
  expect(preview.code, preview.stderr + preview.stdout).toBe(0);
  const result = ath(["import", input, "--mapping", mapping, "--yes"], dir);
  expect(result.code, result.stderr + result.stdout).toBe(0);
  return JSON.parse(preview.stdout);
}

function part(value: string): string {
  return value.replaceAll("\\", "\\\\").replaceAll(":", "\\:");
}

function key(...parts: string[]): string {
  return parts.map(part).join(":");
}

interface Stored {
  record_id: string;
  record_type: string;
  current_revision: number;
  method: string;
  json: string;
  data: Record<string, any>;
}

async function stored(dir: string): Promise<{
  byKey: Map<string, Stored>;
  sources: Array<Record<string, any>>;
  recordCount: number;
  keyCount: (method: string, matchKey: string) => Promise<number>;
}> {
  const client = createClient({ url: pathToFileURL(join(dir, "athlete.ath.sqlite")).href });
  const rows = await client.execute(`
    SELECT k.match_key_method, k.match_key, r.record_id, r.record_type, r.current_revision, rv.record_json
    FROM record_import_keys k
    JOIN records r ON r.record_id = k.record_id
    JOIN record_revisions rv ON rv.record_id = r.record_id AND rv.revision = r.current_revision
  `);
  const byKey = new Map<string, Stored>();
  for (const row of rows.rows) {
    byKey.set(String(row.match_key), {
      record_id: String(row.record_id),
      record_type: String(row.record_type),
      current_revision: Number(row.current_revision),
      method: String(row.match_key_method),
      json: String(row.record_json),
      data: JSON.parse(String(row.record_json)),
    });
  }
  const sourceRows = await client.execute("SELECT record_id, data_json FROM sources");
  const sources = sourceRows.rows.map((row) => ({
    record_id: String(row.record_id),
    ...JSON.parse(String(row.data_json)),
  }));
  const count = await client.execute("SELECT count(*) AS n FROM records");
  const recordCount = Number(count.rows[0]!.n);
  const keyCount = async (method: string, matchKey: string) => {
    const result = await client.execute({
      sql: "SELECT count(*) AS n FROM record_import_keys WHERE match_key_method = ? AND match_key = ?",
      args: [method, matchKey],
    });
    return Number(result.rows[0]!.n);
  };
  return { byKey, sources, recordCount, keyCount };
}

function sourceOf(sources: Array<Record<string, any>>, id: string): Record<string, any> {
  const source = sources.find((candidate) => candidate.record_id === id);
  expect(source, `source ${id}`).toBeDefined();
  return source!;
}

function writeMapping(dir: string, name: string, body: string): string {
  const path = join(dir, `${name}.md`);
  writeFileSync(path, `# ${name}\n\n\`\`\`ath-mapping\n${body}\n\`\`\`\n`);
  return path;
}

const SLEEP_HEADER = "name: invented\nroute: invented_route\nwriter_field: source.provider\n\n[sleep]\nid_field: id\nstart_field: start_time\nend_field: end_time\n";
const Z_RECEIPT = (text: string) => ({
  timestamp_text: text,
  utc_offset_text: "Z",
  utc_offset_meaning: "documented_recording_offset",
  timestamp_basis: "source_instant",
});

describe("ath import --mapping open-wearables", () => {
  it("open_wearables_fixture_maps_fields", async () => {
    const dir = emptyDatabase();
    const result = mapped(dir, join(OW, "summaries.json"), "open-wearables");
    const { byKey, sources } = await stored(dir);

    const night = byKey.get(key("oura", S_RING, "sleep", "ow-sleep-1"))!;
    expect(night.method).toBe("open_wearables/v1");
    expect(night.record_type).toBe("sleep_session");
    expect(night.data.start_at).toBe("2026-08-09T06:00:00.000Z");
    expect(night.data.end_at).toBe("2026-08-09T14:00:00.000Z");
    expect(night.data.input_start_timestamp).toEqual({
      timestamp_text: "2026-08-09T02:00:00.000-04:00",
      utc_offset_text: "-04:00",
      utc_offset_meaning: "meaning_not_documented",
      timestamp_basis: "source_offset_timestamp",
    });
    expect(night.data.asleep_seconds).toBe(600);
    expect(night.data.in_bed_seconds).toBe(700);
    expect(night.data.stage_minutes).toEqual({ awake: 2, light: 3, deep: 4 });
    expect(night.data.supplied_fields).toEqual({
      zone_offset: "-04:00",
      duration_seconds: 800,
      efficiency_percent: 91,
      is_nap: false,
    });
    expect(sourceOf(sources, night.data.source_id)).toMatchObject({
      data_writer: "oura",
      data_route: "open_wearables",
      vendor: "oura",
      source_type: "export_file",
      sensor_type: S_RING,
    });
    const ringDevice = byKey.get(key("oura", S_RING, "source_device"))!;
    expect(ringDevice.data).toMatchObject({
      source_id: night.data.source_id,
      name: "Invented Ring",
      model: "invented-ring",
      hardware: "ring-1",
      manufacturer: "oura",
    });

    const nap = byKey.get(key("oura", S_RING, "sleep", "ow-nap-1"))!;
    expect(nap.record_id).not.toBe(night.record_id);
    expect(nap.data.supplied_fields).toEqual({ efficiency_percent: 80, is_nap: true });
    expect(nap.data.asleep_seconds).toBe(1000);
    expect(nap.data.in_bed_seconds).toBe(1200);
    expect(nap.data.source_id).toBe(night.data.source_id);

    const workout = byKey.get(key("garmin", S_WATCH, "workout", "ow-workout-1"))!;
    expect(sourceOf(sources, workout.data.source_id)).toMatchObject({
      data_writer: "garmin",
      data_route: "open_wearables",
      sensor_type: S_WATCH,
    });
    expect(workout.data.activity_type).toBe("running");
    expect(workout.data.name).toBe("Invented loop");
    expect(workout.data.source_reported_summaries).toEqual([
      { summary_type: "distance", standard_value: 12, standard_unit: "m", input_value: 12, input_unit: "m" },
      { summary_type: "elevation_gain", standard_value: 4, standard_unit: "m", input_value: 4, input_unit: "m" },
    ]);
    expect(workout.data.maximum_heart_rate_bpm).toBeUndefined();
    expect(workout.data.supplied_fields).toEqual({
      duration_seconds: 3,
      intensity: "moderate",
      start_latitude: 12.345,
      start_longitude: -45.678,
      calories_kcal: 1,
      hr_zones: { max_hr: 190 },
    });
    const average = byKey.get(key("garmin", S_WATCH, "workout", "ow-workout-1", "summary", "average_heart_rate"))!;
    expect(average.data.value).toEqual({ standard_value: 140, standard_unit: "bpm", input_value: 140, input_unit: "bpm" });
    expect(average.data.supplied_fields).toBeUndefined();
    const peak = byKey.get(key("garmin", S_WATCH, "workout", "ow-workout-1", "summary", "max_heart_rate"))!;
    expect(peak.data.value.standard_value).toBe(180);
    expect(peak.data.supplied_fields).toBeUndefined();

    expect(result.left_out).toContain("workout ow-workout-manual: entry_source manual is a personal report");
    expect([...byKey.keys()].some((matchKey) => matchKey.includes("ow-workout-manual"))).toBe(false);

    const at = "2026-08-09T06:12:00.000Z";
    const points: Array<[string, string, number, string]> = [
      ["heart_rate_variability_rmssd", "hrv_rmssd", 42.5, "ms"],
      ["heart_rate_variability_sdnn", "hrv_sdnn", 61.2, "ms"],
      ["resting_heart_rate", "resting_heart_rate", 48, "bpm"],
      ["respiratory_rate", "respiratory_rate", 16, "brpm"],
      ["skin_temperature", "skin_temperature", 33.2, "°C"],
      ["oxygen_saturation", "oxygen_saturation", 98, "%"],
      ["heart_rate_recovery_one_minute", "hr_recovery", 22, "bpm"],
    ];
    for (const [type, quantity, value, unit] of points) {
      const point = byKey.get(key("oura", S_RING, "timeseries", "point", type, at))!;
      expect(point, type).toBeDefined();
      expect(point.data.quantity).toBe(quantity);
      expect(point.data.value.standard_value).toBe(value);
      expect(point.data.value.standard_unit).toBe(unit);
      expect(point.data.supplied_fields).toBeUndefined();
      expect(point.data.input_timestamp).toEqual(Z_RECEIPT(at));
      expect(point.data.source_id).toBe(night.data.source_id);
    }
    const skin = byKey.get(key("oura", S_RING, "timeseries", "point", "skin_temperature", at))!;
    expect(skin.data.value.input_unit).toBe("celsius");
    const oxygen = byKey.get(key("oura", S_RING, "timeseries", "point", "oxygen_saturation", at))!;
    expect(oxygen.data.value.input_unit).toBe("percent");
    expect(night.json).toContain('"asleep_seconds":600,');

    const wrist = byKey.get(key("apple", S_AWATCH, "timeseries", "point", "skin_temperature", at))!;
    expect(wrist.data.quantity).toBe("wrist_temperature_sleeping");
    expect(wrist.data.value).toEqual({ standard_value: 33.4, standard_unit: "°C", input_value: 33.4, input_unit: "celsius" });
    expect(sourceOf(sources, wrist.data.source_id)).toMatchObject({ data_writer: "apple", sensor_type: S_AWATCH });

    const vo2 = byKey.get(key("oura", S_RING, "timeseries", "summary", "vo2_max", at))!;
    expect(vo2.record_type).toBe("provider_summary");
    expect(vo2.data.summary_type).toBe("vo2_max");
    expect(vo2.data.value).toEqual({ standard_value: 45, standard_unit: "ml/kg/min", input_value: 45, input_unit: "ml_kg_min" });

    const stress = byKey.get(key("garmin", S_WATCH, "timeseries", "score", "garmin_stress_level", at))!;
    expect(stress.record_type).toBe("provider_score");
    expect(stress.data).toMatchObject({ metric: "garmin_stress_level", scale: "1-100" });
    expect(stress.data.value.standard_value).toBe(15);
    expect(stress.data.supplied_fields).toBeUndefined();
    expect(sourceOf(sources, stress.data.source_id)).toMatchObject({ data_writer: "garmin", sensor_type: S_WATCH });

    const battery = byKey.get(key("garmin", S_WATCH, "timeseries", "score", "garmin_body_battery", "2026-08-09T06:13:00.000Z"))!;
    expect(battery.data).toMatchObject({ metric: "garmin_body_battery", scale: "0-100" });
    expect(battery.data.value).toEqual({ standard_value: 70, standard_unit: "score", input_value: 70, input_unit: "percent" });
    expect(sourceOf(sources, battery.data.source_id)).toMatchObject({ data_writer: "garmin", vendor: "garmin" });
    expect(byKey.get(key("garmin", S_WATCH, "source_device"))!.data.manufacturer).toBe("garmin");

    const phone = byKey.get(key("apple", S_PHONE, "timeseries", "point", "resting_heart_rate", "2026-08-09T08:00:00.000Z"))!;
    const watch = byKey.get(key("apple", S_AWATCH, "timeseries", "point", "resting_heart_rate", "2026-08-09T08:00:00.000Z"))!;
    expect(phone.data.value.standard_value).toBe(61);
    expect(watch.data.value.standard_value).toBe(52);
    expect(phone.data.source_id).not.toBe(watch.data.source_id);
    expect(sourceOf(sources, phone.data.source_id).data_writer).toBe("apple");
    expect(sourceOf(sources, watch.data.source_id).data_writer).toBe("apple");

    for (const type of ["dietary_caffeine", "workout_effort_score", "skin_temperature_deviation"]) {
      expect(result.left_out).toContain(`timeseries ${type} ${at}: unmapped type ${type}`);
      expect([...byKey.keys()].some((matchKey) => matchKey.includes(type))).toBe(false);
    }
  }, 60_000);

  it("missing_computed_by_uses_the_provider", async () => {
    const dir = emptyDatabase();
    mapped(dir, join(OW, "summaries.json"), "open-wearables");
    const { byKey, sources } = await stored(dir);
    const stress = byKey.get(key("garmin", S_WATCH, "timeseries", "score", "garmin_stress_level", "2026-08-09T06:12:00.000Z"))!;
    expect(stress.data.supplied_fields?.computed_by).toBeUndefined();
    expect(sourceOf(sources, stress.data.source_id).data_writer).toBe("garmin");
  }, 60_000);

  it("sample_unit_mismatch_is_left_out, window_seconds_without_resolution_is_left_out, and period_end_is_an_aggregate", async () => {
    const dir = emptyDatabase();
    const result = mapped(dir, join(OW, "summaries.json"), "open-wearables");
    const { byKey } = await stored(dir);
    expect(result.left_out).toContain(
      "timeseries heart_rate_variability_rmssd 2026-08-09T07:00:00.000Z: unit bpm does not match ms",
    );
    expect(result.left_out).toContain(
      "timeseries garmin_stress_level 2026-08-09T06:14:00.000Z: window_seconds present without resolution: cannot confirm raw",
    );
    expect(result.left_out).toContain(
      "timeseries garmin_stress_level 2026-08-09T06:15:00.000Z: aggregate row, not a raw sample",
    );
    const keys = [...byKey.keys()];
    expect(keys.some((matchKey) => matchKey.endsWith(part("2026-08-09T07:00:00.000Z")))).toBe(false);
    expect(keys.some((matchKey) => matchKey.endsWith(part("2026-08-09T06:14:00.000Z")))).toBe(false);
    expect(keys.some((matchKey) => matchKey.endsWith(part("2026-08-09T06:15:00.000Z")))).toBe(false);
  }, 60_000);

  it("manual_entry_source_is_left_out and intensity_stays_in_supplied_fields", async () => {
    const dir = emptyDatabase();
    const result = mapped(dir, join(OW, "summaries.json"), "open-wearables");
    expect(result.left_out).toContain("workout ow-workout-manual: entry_source manual is a personal report");
    const { byKey } = await stored(dir);
    const workout = byKey.get(key("garmin", S_WATCH, "workout", "ow-workout-1"))!;
    expect(workout.data.supplied_fields.intensity).toBe("moderate");
    expect(workout.data.intensity).toBeUndefined();
    expect([...byKey.values()].some((record) => record.data.summary_type === "intensity")).toBe(false);
  }, 60_000);

  it("raw_envelope_keeps_window_seconds", async () => {
    const dir = emptyDatabase();
    const result = mapped(dir, join(OW, "page-raw.json"), "open-wearables");
    expect(result.left_out).toEqual([]);
    const { byKey, sources } = await stored(dir);
    const stress = byKey.get(key("garmin", S_WATCH, "timeseries", "score", "garmin_stress_level", "2026-08-09T06:14:00.000Z"))!;
    expect(stress.data).toMatchObject({ metric: "garmin_stress_level", scale: "1-100" });
    expect(stress.data.value.standard_value).toBe(18);
    expect(stress.data.supplied_fields).toEqual({ window_seconds: 60 });
    expect(sourceOf(sources, stress.data.source_id).data_writer).toBe("garmin");
  }, 60_000);

  it("workout_page_is_not_a_timeseries_envelope and sleep_page_is_not_a_timeseries_envelope", async () => {
    for (const page of ["page-workouts.json", "page-sleep.json"]) {
      const dir = emptyDatabase();
      const result = mapped(dir, join(OW, page), "open-wearables");
      expect(result.left_out).toEqual([ENVELOPE_LINE]);
      expect(result.record_ids).toEqual([]);
      expect((await stored(dir)).recordCount).toBe(0);
    }
  }, 60_000);

  it("an empty envelope writes nothing and adds no left_out line", async () => {
    const dir = emptyDatabase();
    const result = mapped(dir, join(OW, "page-empty.json"), "open-wearables");
    expect(result.left_out).toEqual([]);
    expect(result.record_ids).toEqual([]);
    expect((await stored(dir)).recordCount).toBe(0);
  }, 60_000);

  it("null_resolution_is_treated_as_absent", async () => {
    const dir = emptyDatabase();
    const result = mapped(dir, join(OW, "page-null-resolution.json"), "open-wearables");
    expect(result.left_out).toEqual([
      "timeseries garmin_stress_level 2026-08-09T06:14:00.000Z: window_seconds present without resolution: cannot confirm raw",
    ]);
    const { byKey } = await stored(dir);
    expect(byKey.has(key("garmin", S_WATCH, "timeseries", "score", "garmin_stress_level", "2026-08-09T06:14:00.000Z"))).toBe(false);
    const kept = byKey.get(key("garmin", S_WATCH, "timeseries", "score", "garmin_stress_level", "2026-08-09T06:17:00.000Z"))!;
    expect(kept.data).toMatchObject({ metric: "garmin_stress_level", scale: "1-100" });
    expect(kept.data.value.standard_value).toBe(12);
    expect(kept.data.supplied_fields).toBeUndefined();
  }, 60_000);

  it("five_minute_envelope_is_left_out", async () => {
    const dir = emptyDatabase();
    const result = mapped(dir, join(OW, "page-5min.json"), "open-wearables");
    expect(result.left_out).toContain(
      "timeseries garmin_stress_level 2026-08-09T06:10:00.000Z: resolution 5min: not raw samples",
    );
    expect((await stored(dir)).recordCount).toBe(0);
  }, 60_000);

  it("rerun_does_not_add_a_record", async () => {
    const dir = emptyDatabase();
    mapped(dir, join(OW, "summaries.json"), "open-wearables");
    const nightKey = key("oura", S_RING, "sleep", "ow-sleep-1");
    const first = await stored(dir);
    mapped(dir, join(OW, "summaries.json"), "open-wearables");
    const second = await stored(dir);
    expect(second.recordCount).toBe(first.recordCount);
    const nights = [...second.byKey.values()].filter((record) => record.data.provider_record_id === "ow-sleep-1");
    expect(nights).toHaveLength(1);
    expect(nights[0]!.current_revision).toBe(1);
    expect(await second.keyCount("open_wearables/v1", nightKey)).toBe(1);

    const changed = JSON.parse(readFileSync(join(OW, "summaries.json"), "utf8"));
    changed.sleep[0].end_time = "2026-08-09T10:05:00.000-04:00";
    const copy = join(dir, "changed.json");
    writeFileSync(copy, JSON.stringify(changed));
    mapped(dir, copy, "open-wearables");
    const third = await stored(dir);
    const revised = third.byKey.get(nightKey)!;
    expect(revised.record_id).toBe(first.byKey.get(nightKey)!.record_id);
    expect(revised.current_revision).toBe(2);
    expect(revised.data.end_at).toBe("2026-08-09T14:05:00.000Z");
    expect(revised.data.received_at).toBe(first.byKey.get(nightKey)!.data.received_at);
    const check = ath(["check"], dir);
    expect(check.code, check.stdout + check.stderr).toBe(0);
  }, 90_000);
});

describe("ath import --mapping junction", () => {
  it("junction_fixture_maps_fields and apple_average_hrv_stays_unmapped", async () => {
    const dir = emptyDatabase();
    const result = mapped(dir, JUNCTION, "junction");
    const { byKey, sources } = await stored(dir);

    const night = byKey.get(key("oura", S_JORA, "sleep", "jn-sleep-1"))!;
    expect(night.method).toBe("junction/v1");
    expect(night.data.asleep_seconds).toBe(600);
    expect(night.data.in_bed_seconds).toBeUndefined();
    expect(night.data.stage_minutes).toEqual({ awake: 2, light: 3, deep: 4 });
    expect(night.data.supplied_fields).toEqual({
      duration: 700,
      total: 500,
      awake: 0,
      light: 0,
      rem: 0,
      deep: 0,
      efficiency: 0.88,
      LastNightAvg: 40,
      type: "long_sleep",
    });
    const hrv = byKey.get(key("oura", S_JORA, "sleep", "jn-sleep-1", "point", "hrv_rmssd"))!;
    expect(hrv.data.value.standard_value).toBe(55.5);
    expect(hrv.data.input_timestamp).toEqual({
      timestamp_text: "2026-08-09T10:00:00.000-04:00",
      utc_offset_text: "-04:00",
      utc_offset_meaning: "meaning_not_documented",
      timestamp_basis: "source_offset_timestamp",
    });
    expect(byKey.get(key("oura", S_JORA, "sleep", "jn-sleep-1", "point", "resting_heart_rate"))!.data.value.standard_value).toBe(47);
    const breathing = byKey.get(key("oura", S_JORA, "sleep", "jn-sleep-1", "point", "respiratory_rate"))!;
    expect(breathing.data.value).toMatchObject({ standard_value: 14, standard_unit: "brpm" });
    expect([...byKey.values()].some((record) => record.data.quantity === "hrv_sdnn")).toBe(false);

    const sleepScore = byKey.get(key("oura", S_JORA, "sleep", "jn-sleep-1", "score", "sleep.score"))!;
    expect(sleepScore.data).toMatchObject({ metric: "sleep.score", scale: "1-100" });
    expect(sleepScore.data.value.standard_value).toBe(80);
    expect(sourceOf(sources, sleepScore.data.source_id).data_writer).toBe("oura");
    const readiness = byKey.get(key("oura", S_JORA, "sleep", "jn-sleep-1", "score", "recovery_readiness_score"))!;
    expect(readiness.data).toMatchObject({ metric: "recovery_readiness_score", scale: "0-100" });
    expect(readiness.data.value.standard_value).toBe(70);

    const second = byKey.get(key("oura", S_JORA, "sleep", "jn-sleep-2"))!;
    expect(second.data.asleep_seconds).toBe(1000);
    expect(second.data.supplied_fields).toEqual({ duration: 3600 });

    const nap = byKey.get(key("oura", S_JORA, "sleep", "jn-nap-1"))!;
    expect(nap.record_id).not.toBe(night.record_id);
    expect(nap.data.asleep_seconds).toBe(800);
    expect(nap.data.supplied_fields).toEqual({ duration: 900, type: "acknowledged_nap" });

    const apple = byKey.get(key("apple_health_kit", S_JAPPLE, "sleep", "jn-sleep-apple"))!;
    expect(apple.data.asleep_seconds).toBe(3000);
    expect(apple.data.supplied_fields).toEqual({ duration: 3600, average_hrv: 33 });
    expect(apple.data.input_end_timestamp.timestamp_text).toBe("2026-08-11T03:00:00.000Z");
    expect(sourceOf(sources, apple.data.source_id)).toMatchObject({
      data_writer: "apple_health_kit",
      data_route: "junction",
      sensor_type: S_JAPPLE,
    });
    expect([...byKey.keys()].some((matchKey) => matchKey.startsWith(`${key("apple_health_kit", S_JAPPLE, "sleep", "jn-sleep-apple")}:`))).toBe(false);

    expect(result.left_out).toContain("activity: no mapping section");
    expect([...byKey.values()].some((record) => JSON.stringify(record.data).includes("8432"))).toBe(false);
    expect(sources.some((source) => source.data_writer === "oura" && source.data_route === "junction" && source.sensor_type === S_JORA)).toBe(true);
  }, 60_000);

  it("average_hrv_with_basis_can_become_rmssd from the shipped line", async () => {
    const dir = emptyDatabase();
    mapped(dir, JUNCTION, "junction");
    const { byKey } = await stored(dir);
    const hrv = byKey.get(key("oura", S_JORA, "sleep", "jn-sleep-1", "point", "hrv_rmssd"))!;
    expect(hrv.data.quantity).toBe("hrv_rmssd");
    expect(readFileSync(join(SHIPPED, "junction.md"), "utf8")).toContain(
      "average_hrv: hrv_rmssd ms ms basis hrv_mean_rmssd when-writer oura,whoop_v2,garmin,fitbit,polar",
    );
  }, 60_000);
});

describe("mapped imports and the rest of the file", () => {
  it("mapped_imports_pass_ath_check", () => {
    const dir = emptyDatabase();
    mapped(dir, join(OW, "summaries.json"), "open-wearables");
    mapped(dir, JUNCTION, "junction");
    const check = ath(["check"], dir);
    expect(check.code, check.stdout + check.stderr).toBe(0);
  }, 60_000);

  it("garmin_file_and_mapping_stay_two_sources", async () => {
    const dir = emptyDatabase();
    const garmin = ath(["import", GARMIN, "--yes"], dir);
    expect(garmin.code, garmin.stderr).toBe(0);
    mapped(dir, join(OW, "summaries.json"), "open-wearables");
    const client = createClient({ url: pathToFileURL(join(dir, "athlete.ath.sqlite")).href });
    const workouts = await client.execute(
      "SELECT data_json FROM workout_sessions JOIN records USING (record_id) WHERE records.is_deleted = 0",
    );
    expect(workouts.rows).toHaveLength(2);
    const garminSources = await client.execute(
      "SELECT data_route, COALESCE(sensor_type, '') AS sensor FROM sources WHERE data_writer = 'garmin' ORDER BY data_route",
    );
    expect(garminSources.rows.map((row) => [row.data_route, row.sensor])).toEqual([
      ["garmin_account_export", ""],
      ["open_wearables", S_WATCH],
    ]);
    const bodies = workouts.rows.map((row) => JSON.parse(String(row.data_json)));
    const fileWorkout = bodies.find((body) => body.supplied_fields?.Feeling !== undefined)!;
    const mappedWorkout = bodies.find((body) => body.supplied_fields?.start_latitude !== undefined)!;
    expect(fileWorkout.supplied_fields.Feeling).toBe("smooth");
    expect(mappedWorkout.supplied_fields.start_latitude).toBe(12.345);
    const peaks = await client.execute(
      "SELECT source_id, standard_value FROM provider_summaries WHERE summary_type = 'max_heart_rate'",
    );
    const peakFor = (sourceId: string) =>
      peaks.rows.find((row) => row.source_id === sourceId)?.standard_value;
    expect(peakFor(fileWorkout.source_id)).toBe(200);
    expect(peakFor(mappedWorkout.source_id)).toBe(180);
    client.close();
  }, 60_000);

  it("ath apply sleep-efficiency reads only the two durations", async () => {
    const dir = emptyDatabase();
    mapped(dir, join(OW, "summaries.json"), "open-wearables");
    mapped(dir, JUNCTION, "junction");
    const { byKey } = await stored(dir);
    const owNight = byKey.get(key("oura", S_RING, "sleep", "ow-sleep-1"))!;
    const owApply = ath(["apply", "sleep-efficiency", "--record", owNight.record_id, "--json"], dir);
    expect(owApply.code, owApply.stderr).toBe(0);
    expect(JSON.parse(owApply.stdout)).toEqual({
      calculation: "sleep-efficiency",
      record_id: owNight.record_id,
      fields_read: [
        { field: "asleep_seconds", present: true },
        { field: "in_bed_seconds", present: true },
      ],
      missing: [],
      result: { efficiency: 0.8571428571428571 },
    });
    const jnNight = byKey.get(key("oura", S_JORA, "sleep", "jn-sleep-1"))!;
    const jnApply = ath(["apply", "sleep-efficiency", "--record", jnNight.record_id, "--json"], dir);
    expect(JSON.parse(jnApply.stdout)).toEqual({
      calculation: "sleep-efficiency",
      record_id: jnNight.record_id,
      fields_read: [
        { field: "asleep_seconds", present: true },
        { field: "in_bed_seconds", present: false },
      ],
      missing: ["in_bed_seconds"],
      result: null,
    });
  }, 60_000);
});

describe("refusals", () => {
  it("personal_report_cannot_become_a_measured_record", async () => {
    const dir = emptyDatabase();
    const note = ath(["log", "--yes", "rough night"], dir);
    expect(note.code, note.stderr).toBe(0);
    expect((await stored(dir)).recordCount).toBe(1);
    const input = join(dir, "report.json");
    writeFileSync(
      input,
      JSON.stringify({
        sleep: [{ id: "report-1", report_type: "sleep_quality", reported_via: "text", rating: 2, note: "rough night" }],
      }),
    );
    const refused = ath(["import", input, "--mapping", "open-wearables", "--yes"], dir);
    expect(refused.code).toBe(1);
    expect(refused.stderr.trim()).toBe(`ath: ${PERSONAL_REFUSAL}`);
    expect((await stored(dir)).recordCount).toBe(1);
    const fromCore = core([
      "import-mapped",
      "--database",
      join(dir, "athlete.ath.sqlite"),
      "--mapping-file",
      join(SHIPPED, "open-wearables.md"),
      "--input",
      input,
    ]);
    expect(fromCore.code).toBe(1);
    expect(fromCore.stderr.trim()).toBe(PERSONAL_REFUSAL);
  }, 60_000);

  it("a mapping section or field for a personal report is refused", () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-mapping-refuse-"));
    const section = writeMapping(dir, "report-section", "name: r\nroute: r_route\nwriter_field: source.provider\n[athlete_report]\nrating: rating score score");
    const field = writeMapping(dir, "report-field", `${SLEEP_HEADER}quality: rating score score`);
    for (const path of [section, field]) {
      const refused = core(["import-mapped", "--database", "x", "--mapping-file", path, "--input", "x"]);
      expect(refused.stderr.trim()).toBe(PERSONAL_REFUSAL);
    }
  });

  it("last_night_avg_cannot_become_rmssd and last_night_avg_cannot_become_sdnn", async () => {
    const dir = emptyDatabase();
    const rmssd = ath(["import", join(OW, "summaries.json"), "--mapping", join(MAPPINGS, "last-night-rmssd.md"), "--yes"], dir);
    expect(rmssd.code).toBe(1);
    expect(rmssd.stderr.trim()).toBe(
      "ath: refusing to map LastNightAvg onto hrv_rmssd; the field does not name the statistic",
    );
    const sdnn = core(["import-mapped", "--database", join(dir, "athlete.ath.sqlite"), "--mapping-file", join(MAPPINGS, "last-night-sdnn.md"), "--input", join(OW, "summaries.json")]);
    expect(sdnn.code).toBe(1);
    expect(sdnn.stderr.trim()).toBe("refusing to map LastNightAvg onto hrv_sdnn; the field does not name the statistic");
    expect((await stored(dir)).recordCount).toBe(0);
  }, 60_000);

  it("average_hrv_without_basis_cannot_become_rmssd and average_hrv_cannot_become_sdnn", () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-mapping-refuse-"));
    const cases: Array<[string, string]> = [
      ["average_hrv: hrv_rmssd ms ms", "refusing to map average_hrv onto hrv_rmssd; the field does not name the statistic"],
      ["average_hrv: hrv_sdnn ms ms basis hrv_mean_rmssd", "refusing to map average_hrv onto hrv_sdnn; the field does not name the statistic"],
      ["average_hrv: hrv_rmssd ms ms basis hrv_mean_rmssd", "refusing to map average_hrv onto hrv_rmssd; the field does not name the statistic"],
    ];
    for (const [line, message] of cases) {
      const path = writeMapping(dir, "hrv", `${SLEEP_HEADER}${line}`);
      const refused = core(["import-mapped", "--database", "x", "--mapping-file", path, "--input", "x"]);
      expect(refused.code).toBe(1);
      expect(refused.stderr.trim()).toBe(message);
    }
  });

  it("efficiency_percent_cannot_map_onto_efficiency", () => {
    const refused = core(["import-mapped", "--database", "x", "--mapping-file", join(MAPPINGS, "efficiency.md"), "--input", "x"]);
    expect(refused.code).toBe(1);
    expect(refused.stderr.trim()).toBe(
      "refusing to map efficiency_percent onto efficiency; efficiency is computed from asleep_seconds and in_bed_seconds",
    );
  });

  it("max_heart_rate_cannot_become_the_athlete_maximum and zones_max_hr_cannot_become_the_athlete_maximum", () => {
    const refused = core(["import-mapped", "--database", "x", "--mapping-file", join(MAPPINGS, "athlete-maximum.md"), "--input", "x"]);
    expect(refused.stderr.trim()).toBe(
      "refusing to map max_heart_rate_bpm onto maximum_heart_rate_bpm; that field is the athlete's known maximum",
    );
    const dir = mkdtempSync(join(tmpdir(), "ath-mapping-refuse-"));
    const zones = writeMapping(
      dir,
      "zones",
      "name: z\nroute: z_route\nwriter_field: source.provider\n[workout]\nid_field: id\nstart_field: start_time\nend_field: end_time\nhr_zones.max_hr: maximum_heart_rate_bpm bpm bpm",
    );
    const zoneRefusal = core(["import-mapped", "--database", "x", "--mapping-file", zones, "--input", "x"]);
    expect(zoneRefusal.stderr.trim()).toBe(
      "refusing to map hr_zones.max_hr onto maximum_heart_rate_bpm; that field is the athlete's known maximum",
    );
  });

  it("builtin_route_is_rejected", () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-mapping-refuse-"));
    const route = writeMapping(dir, "garmin-route", SLEEP_HEADER.replace("invented_route", "garmin_account_export") + "sleep_duration_seconds: asleep_seconds s s");
    expect(core(["import-mapped", "--database", "x", "--mapping-file", route, "--input", "x"]).stderr.trim()).toBe(
      "route garmin_account_export is already used by a built-in importer",
    );
    const method = writeMapping(dir, "garmin-method", SLEEP_HEADER.replace("invented_route", "garmin_account") + "sleep_duration_seconds: asleep_seconds s s");
    expect(core(["import-mapped", "--database", "x", "--mapping-file", method, "--input", "x"]).stderr.trim()).toBe(
      "match key method garmin_account/v1 is already used by a built-in importer",
    );
  });

  it("other check failures name the line", () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-mapping-refuse-"));
    const cases: Array<[string, string]> = [
      [`${SLEEP_HEADER}sleep_duration_seconds asleep_seconds`, "could not read 'sleep_duration_seconds asleep_seconds'"],
      [`${SLEEP_HEADER}x: steps count count`, "unknown field steps"],
      [`${SLEEP_HEADER}x: asleep_seconds s s when-absent\ny: asleep_seconds s s when-absent`, "two when-absent lines write asleep_seconds"],
      [`${SLEEP_HEADER}x: asleep_seconds s s basis provider`, "basis is only used for hrv_rmssd, hrv_sdnn, and heart-rate summaries"],
      [`${SLEEP_HEADER}x: asleep_seconds s s scale 1-100`, "scale is only used for provider_score"],
      [`${SLEEP_HEADER}x: provider_score.x score score`, "provider_score x needs a scale"],
      [`${SLEEP_HEADER}x: oxygen_saturation percent percent`, "oxygen_saturation uses %"],
      [`${SLEEP_HEADER}x: provider_summary.y ms s`, "cannot convert ms to s for x"],
      [`${SLEEP_HEADER}[activity]\nsteps: provider_summary.steps count count`, "unknown mapping section activity"],
    ];
    for (const [body, message] of cases) {
      const path = writeMapping(dir, "case", body);
      const refused = core(["import-mapped", "--database", "x", "--mapping-file", path, "--input", "x"]);
      expect(refused.code, body).toBe(1);
      expect(refused.stderr.trim(), body).toBe(message);
    }
    const noFence = join(dir, "no-fence.md");
    writeFileSync(noFence, "# nothing\n");
    expect(core(["import-mapped", "--database", "x", "--mapping-file", noFence, "--input", "x"]).stderr.trim()).toBe(
      "the mapping file has no ath-mapping block",
    );
  });
});

describe("mapping files a person adds", () => {
  it("installed_mapping_file_is_the_one_that_runs", () => {
    const dir = emptyDatabase();
    const installed = join(dir, ".agents", "mappings", "open-wearables.md");
    mkdirSync(dirname(installed), { recursive: true });
    const edited = readFileSync(join(SHIPPED, "open-wearables.md"), "utf8").replace(
      "type.heart_rate_variability_rmssd: hrv_rmssd ms ms",
      "type.LastNightAvg: hrv_rmssd ms ms",
    );
    writeFileSync(installed, edited);
    const refused = ath(["import", join(OW, "summaries.json"), "--mapping", "open-wearables", "--yes"], dir);
    expect(refused.code).toBe(1);
    expect(refused.stderr.trim()).toBe(
      "ath: refusing to map LastNightAvg onto hrv_rmssd; the field does not name the statistic",
    );
    installMappings(dir);
    expect(readFileSync(installed, "utf8")).toBe(edited);
  }, 60_000);

  it("person_adds_my_source", async () => {
    const dir = emptyDatabase();
    const file = join(dir, ".agents", "mappings", "my-source.md");
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(
      file,
      "# My source\n\n```ath-mapping\nname: my-source\nroute: my_source\nwriter_field: maker\n\n[sleep]\nid_field: night\nstart_field: from\nend_field: to\nasleep: asleep_seconds s s\n```\n",
    );
    const input = join(dir, "nights.json");
    writeFileSync(
      input,
      JSON.stringify({ sleep: [{ night: "n-1", from: "2026-08-09T01:00:00.000Z", to: "2026-08-09T07:00:00.000Z", maker: "invented-maker", asleep: 20000, mood_word: "fine" }] }),
    );
    const imported = ath(["import", input, "--mapping", "my-source", "--yes"], dir);
    expect(imported.code, imported.stderr).toBe(0);
    const { byKey, sources } = await stored(dir);
    const night = byKey.get(key("invented-maker", "-", "sleep", "n-1"))!;
    expect(night.data.asleep_seconds).toBe(20000);
    expect(night.data.supplied_fields).toEqual({ mood_word: "fine" });
    expect(sourceOf(sources, night.data.source_id).data_route).toBe("my_source");
    const skills = ath(["skill", "--json"], dir);
    expect(skills.stdout).not.toContain("my-source");
    const apply = ath(["apply", "my-source", "--record", night.record_id], dir);
    expect(apply.code).toBe(1);
    expect(apply.stderr).toContain("no calculation skill named my-source");
  }, 60_000);

  it("init_no_skill_skips_mapping_files", () => {
    const without = emptyDatabase();
    expect(existsSync(join(without, ".agents", "mappings"))).toBe(false);
    const withSkill = emptyDatabase({ skill: true });
    expect(existsSync(join(withSkill, ".agents", "mappings", "open-wearables.md"))).toBe(true);
    expect(existsSync(join(withSkill, ".agents", "mappings", "junction.md"))).toBe(true);
  }, 60_000);

  it("ath skill install copies a missing mapping file and leaves an edited one", () => {
    const dir = emptyDatabase();
    const installed = join(dir, ".agents", "mappings", "junction.md");
    mkdirSync(dirname(installed), { recursive: true });
    writeFileSync(installed, "edited by a person\n");
    const install = ath(["skill", "install", "--json"], dir);
    expect(install.code, install.stderr).toBe(0);
    expect(JSON.parse(install.stdout).mappings_installed).toEqual([
      join(dir, ".agents", "mappings", "open-wearables.md"),
    ]);
    expect(readFileSync(installed, "utf8")).toBe("edited by a person\n");
  }, 60_000);
});

describe("the mapping engine's unit table", () => {
  it("matches POINT_MEASUREMENT_UNITS", () => {
    const source = readFileSync(resolve(here, "../crates/ath-core/src/mapping.rs"), "utf8");
    const table = /pub const POINT_UNITS: &\[\(&str, &str\)\] = &\[([\s\S]*?)\];/.exec(source)![1]!;
    const pairs = [...table.matchAll(/\("([^"]+)", "([^"]+)"\)/g)].map((match) => [match[1], match[2]]);
    expect(Object.fromEntries(pairs)).toEqual(POINT_MEASUREMENT_UNITS);
  });
});
