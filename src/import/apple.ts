/**
 * Apple Health export.xml.
 *
 * ath-core reads the XML. This file asks for a source id, keeps a hand-entered
 * writer on the manual source, and puts the signals back into the maps the rest
 * of the import uses. SDNN stays a series. RMSSD is the figure computed from the
 * beats under that series.
 */
import { existsSync, lstatSync } from "node:fs";
import { basename, join } from "node:path";
import { runAthCore } from "../ath-core.js";
import type { DeviceT, HardSignalT } from "../schema.js";
import { buildSeries, type Sample } from "../series.js";
import { silentProgress, type Progress } from "../progress.js";
import { rmssdFromIntervals } from "../hrv.js";
import {
  countSkip,
  countSkipWithExample,
  deviceIndexKey,
  emptyPayload,
  observeDevice,
  writerSlug,
  type ImportPayload,
  type SourceBook,
} from "./merge.js";
import { beatsFromEcg, parseEcgCsv } from "./ecg.js";
import { parseGpx, summarizeRoute, type RouteSummary } from "./routes.js";
import { isEcgEntry, isRouteEntry, readEntries, type DetectedExport } from "./detect.js";
import { MAX_ZIP_ENTRY_BYTES, openZip, zipEntryNames } from "./zip.js";

function attribute(payload: ImportPayload, signal: HardSignalT, writer: Writer): void {
  if (writer.deviceKey) payload.deviceOf.set(signal, writer.deviceKey);
}

/**
 * Apple writes `2026-08-09 06:12:00 -0700`. Turn that into ISO 8601 keeping the
 * offset, because a naive local time is not a valid timestamp in this format.
 */
export function parseAppleDate(raw: string): string | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2}):(\d{2})\s*([+-]\d{2}):?(\d{2})?$/.exec(
    raw.trim(),
  );
  if (m) {
    const [, y, mo, d, h, mi, s, offH, offM = "00"] = m;
    return `${y}-${mo}-${d}T${h}:${mi}:${s}${offH}:${offM}`;
  }
  const iso = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/;
  if (iso.test(raw.trim())) return raw.trim();
  return null;
}

/** Fields Apple writes inside a `device` attribute, in the order they appear. */
const DEVICE_FIELDS = [
  "name",
  "manufacturer",
  "model",
  "hardware",
  "software",
  "firmware",
  "localIdentifier",
  "UDI",
] as const;

/**
 * Read the stable parts of Apple's device string.
 *
 * `<<HKDevice: 0x2809b6800>, name:Apple Watch, manufacturer:Apple Inc., model:Watch,
 * hardware:Watch6,2, software:10.2>`. The address is different every time the phone
 * runs and the software changes every update, so neither is kept. Values can hold a
 * comma — `Watch6,2` — so the string is cut at the field names, not at commas.
 */
export function parseAppleDevice(raw: string | undefined): DeviceT | undefined {
  if (!raw) return undefined;
  const body = raw.replace(/^<<HKDevice:[^>]*>,?\s*/, "").replace(/>\s*$/, "");

  const fieldAt = new RegExp(`(?:^|,\\s*)(${DEVICE_FIELDS.join("|")}):`, "g");
  const hits: { key: string; valueStart: number; matchStart: number }[] = [];
  for (const m of body.matchAll(fieldAt)) {
    hits.push({ key: m[1]!, valueStart: m.index! + m[0].length, matchStart: m.index! });
  }
  if (hits.length === 0) return undefined;

  const fields: Record<string, string> = {};
  hits.forEach((hit, i) => {
    const end = i + 1 < hits.length ? hits[i + 1]!.matchStart : body.length;
    const value = body.slice(hit.valueStart, end).trim();
    if (value) fields[hit.key] = value;
  });

  const device: DeviceT = {
    ...(fields.name ? { name: fields.name } : {}),
    ...(fields.manufacturer ? { manufacturer: fields.manufacturer } : {}),
    ...(fields.model ? { model: fields.model } : {}),
    ...(fields.hardware ? { hardware: fields.hardware } : {}),
  };
  return Object.keys(device).length > 0 ? device : undefined;
}

/** Writers that are the Health app itself: the person typed the number in. */
const HAND_ENTERED_WRITERS = new Set(["health", "manual"]);

/**
 * Vendor slug for a writer. The manufacturer settles it when the device names one;
 * otherwise Apple's own devices are recognised by name and anything else is taken
 * from the first word of the writer.
 */
export function vendorOf(writer: string, device?: DeviceT): string {
  if (device?.manufacturer) return writerSlug(device.manufacturer).split("-")[0] ?? "unknown";
  const slug = writerSlug(writer);
  if (/^(iphone|ipad|apple)(-|$)/.test(slug)) return "apple";
  return slug.split("-")[0] ?? "unknown";
}

interface Writer {
  source: string;
  deviceKey?: string;
}

/**
 * Source ids for the writers met while parsing, resolved once per distinct
 * (name, device) pair.
 */
class WriterCache {
  private readonly writers = new Map<string, Writer>();
  private readonly devices = new Map<string, DeviceT | undefined>();

  constructor(
    private readonly sources: SourceBook,
    private readonly payload: ImportPayload,
  ) {}

  resolve(writerName: string, deviceRaw: string, onDay: string): Writer {
    const writer = writerName.trim() || "Unknown";
    const key = `${writer}\u0000${deviceRaw}`;
    let hit = this.writers.get(key);

    if (!hit) {
      if (HAND_ENTERED_WRITERS.has(writer.toLowerCase())) {
        hit = { source: this.sources.manual() };
      } else {
        let device = this.devices.get(deviceRaw);
        if (!this.devices.has(deviceRaw)) {
          device = parseAppleDevice(deviceRaw);
          this.devices.set(deviceRaw, device);
        }
        const source = this.sources.forWriter({
          writer,
          vendor: vendorOf(writer, device),
          ...(device ? { device } : {}),
        });
        hit = { source, ...(device ? { deviceKey: deviceIndexKey(source, device) } : {}) };
      }
      this.writers.set(key, hit);
    }

    if (hit.deviceKey) {
      const device = this.devices.get(deviceRaw);
      if (device && /^\d{4}-\d{2}-\d{2}$/.test(onDay)) {
        observeDevice(this.payload, hit.source, device, onDay);
      }
    }
    return hit;
  }
}

const DAY_MS = 86_400_000;
const HOUR_MS = 3_600_000;
const BEAT_WINDOW_TOLERANCE_MS = 5 * 60 * 1000;
const LONGEST_PLACEABLE_WINDOW_MS = 55 * 60 * 1000;

function clockMs(timestamp: string): number | null {
  const m = /T(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,3}))?/.exec(timestamp);
  if (!m) return null;
  const [, h, mi, s, frac = "0"] = m;
  return (Number(h) * 3600 + Number(mi) * 60 + Number(s)) * 1000 + Number(frac.padEnd(3, "0"));
}

/**
 * Where a beat falls inside its record's window, in milliseconds from the start.
 * The hour is not read. A whole-hour disagreement between the phone clock and the
 * record cannot move a beat out of the minute it belongs to.
 */
export function beatOffsetMs(
  recordedAt: string,
  endsAt: string | null,
  timeOfDay: string,
): number | null {
  const m = /\d{1,2}:(\d{2}):(\d{2})(?:[.,](\d{1,3}))?/.exec(timeOfDay);
  if (!m) return null;
  const [, mi, s, frac = "0"] = m;
  const beatWithinHour = Number(mi) * 60_000 + Number(s) * 1000 + Number(frac.padEnd(3, "0"));

  const startMs = clockMs(recordedAt);
  if (startMs === null) return null;
  const endMs = endsAt === null ? null : clockMs(endsAt);
  const durationMs = endMs === null ? 0 : (endMs - startMs + DAY_MS) % DAY_MS;
  if (durationMs > LONGEST_PLACEABLE_WINDOW_MS) return null;

  const base = beatWithinHour - (startMs % HOUR_MS);

  let best: number | null = null;
  for (const offset of [base, base + HOUR_MS, base - HOUR_MS]) {
    if (offset < -BEAT_WINDOW_TOLERANCE_MS) continue;
    if (offset > durationMs + BEAT_WINDOW_TOLERANCE_MS) continue;
    if (best === null || Math.abs(offset) < Math.abs(best)) best = offset;
  }
  return best;
}

interface AppleCore {
  events: AppleEvent[];
  sleep_sessions: SleepSession[];
  skipped: Record<string, number>;
  skip_examples: Record<string, string>;
}

interface AppleEvent {
  kind: "point" | "series" | "sleep_fragment" | "workout" | "activity";
  writer: string;
  device_raw: string;
  on_day: string;
  type?: string;
  value?: number;
  unit?: string;
  recorded_at?: string;
  receipt?: ImportPayload["inputReceipts"] extends Map<unknown, infer Value> ? Value : never;
  quantity?: string;
  sample?: Sample;
  beat_samples?: Sample[];
  derived?: {
    signal: Omit<HardSignalT, "source"> & { type: "hrv_rmssd" };
    receipt: ImportPayload["inputReceipts"] extends Map<unknown, infer Value> ? Value : never;
  };
  start?: string;
  end?: string;
  aggregates?: Record<string, unknown>;
  segments?: Array<{ label: string; duration_s?: number; distance_m?: number }>;
  route?: RoutePointImport[];
  file_references?: string[];
  route_fields?: Array<Record<string, unknown>>;
  summaries?: Array<Omit<ImportPayload["providerSummaries"][number], "source">>;
}

interface RoutePointImport {
  recorded_at: string;
  latitude_deg: number;
  longitude_deg: number;
  input_timestamp?: {
    timestamp_text: string;
    timestamp_basis: "source_offset_timestamp";
    utc_offset_text?: string;
    utc_offset_meaning?: "meaning_not_documented";
    utc_offset_missing_reason?: "not_provided_by_source";
  };
  supplied_fields?: Record<string, string>;
}

interface SleepSession {
  writer: string;
  device_raw: string;
  start: string;
  end: string;
  aggregates: Record<string, unknown>;
  receipt: ImportPayload["inputReceipts"] extends Map<unknown, infer Value> ? Value : never;
}

interface SeriesBucket {
  quantity: NonNullable<AppleEvent["quantity"]>;
  source: string;
  byDay: Map<string, Sample[]>;
}

async function locateAppleXml(
  detected: DetectedExport,
): Promise<{ xmlPath: string; zipEntry?: string; size: number }> {
  const openLocal = (path: string) => {
    const stat = lstatSync(path);
    if (stat.isSymbolicLink() || !stat.isFile()) {
      throw new Error(`refusing Apple Health XML that is not a regular file: ${path}`);
    }
    if (stat.size > MAX_ZIP_ENTRY_BYTES) {
      throw new Error(`Apple Health XML exceeds ${MAX_ZIP_ENTRY_BYTES} bytes: ${path}`);
    }
    return { xmlPath: path, size: stat.size };
  };
  if (detected.container === "file") return openLocal(detected.path);

  if (detected.container === "directory") {
    const candidates = [
      join(detected.path, "export.xml"),
      join(detected.path, "apple_health_export", "export.xml"),
    ];
    const found = candidates.find((candidate) => existsSync(candidate));
    if (!found) throw new Error(`no export.xml under ${detected.path}`);
    return openLocal(found);
  }

  const zip = await openZip(detected.path);
  const entry = zipEntryNames(zip).find((name) => name.toLowerCase().endsWith("export.xml"));
  if (!entry) throw new Error(`no export.xml inside ${detected.path}`);
  return { xmlPath: detected.path, zipEntry: entry, size: lstatSync(detected.path).size };
}

export async function importAppleHealth(
  detected: DetectedExport,
  sources: SourceBook,
  progress: Progress = silentProgress,
): Promise<ImportPayload> {
  const payload = emptyPayload("apple", `Apple Health export (${detected.container})`);
  const located = await locateAppleXml(detected);
  progress.start("reading export.xml", located.size, "bytes");
  const raw = runAthCore(
    ["import-apple"],
    process.env,
    JSON.stringify({ xml_path: located.xmlPath, zip_entry: located.zipEntry ?? "" }),
  ) as AppleCore;
  progress.update(located.size);
  progress.finish();

  for (const [reason, count] of Object.entries(raw.skipped)) payload.skipped.set(reason, count);
  for (const [reason, example] of Object.entries(raw.skip_examples)) {
    payload.skipExamples.set(reason, example);
  }

  const writers = new WriterCache(sources, payload);
  const points: HardSignalT[] = [];
  const workouts: HardSignalT[] = [];
  const series = new Map<string, SeriesBucket>();
  const beatsByDay = new Map<string, { source: string; day: string; samples: Sample[] }>();
  const derived: HardSignalT[] = [];
  let shortWindows = 0;

  for (const event of raw.events) {
    if (event.kind === "sleep_fragment") {
      writers.resolve(event.writer, event.device_raw, event.on_day);
      continue;
    }
    if (event.kind === "point") {
      const writer = writers.resolve(event.writer, event.device_raw, event.on_day);
      const reading = {
        type: event.type,
        value: event.value,
        unit: event.unit,
        recorded_at: event.recorded_at,
        source: writer.source,
      } as HardSignalT;
      attribute(payload, reading, writer);
      if (event.receipt) payload.inputReceipts.set(reading, event.receipt);
      points.push(reading);
      continue;
    }
    if (event.kind === "series" && event.sample && event.quantity) {
      const writer = writers.resolve(event.writer, event.device_raw, event.on_day);
      const key = `${event.quantity}|${writer.source}`;
      const bucket = series.get(key) ?? {
        quantity: event.quantity,
        source: writer.source,
        byDay: new Map<string, Sample[]>(),
      };
      const day = event.sample.at.slice(0, 10);
      const samples = bucket.byDay.get(day) ?? [];
      samples.push(event.sample);
      bucket.byDay.set(day, samples);
      series.set(key, bucket);
      if (event.beat_samples && event.beat_samples.length > 0) {
        const beatKey = `${writer.source}|${day}`;
        const beatBucket = beatsByDay.get(beatKey) ?? { source: writer.source, day, samples: [] };
        beatBucket.samples.push(...event.beat_samples);
        beatsByDay.set(beatKey, beatBucket);
        if (event.derived) {
          const derivedReading = { ...event.derived.signal, source: writer.source } as HardSignalT;
          payload.inputReceipts.set(derivedReading, event.derived.receipt);
          derived.push(derivedReading);
        } else {
          shortWindows += 1;
        }
      }
      continue;
    }
    if (event.kind === "workout" && event.start && event.end && event.aggregates && event.receipt) {
      const writer = writers.resolve(event.writer, event.device_raw, event.on_day);
      const session = {
        type: "workout_session",
        start: event.start,
        end: event.end,
        source: writer.source,
        aggregates: event.aggregates,
        ...(event.segments && event.segments.length > 0 ? { segments: event.segments } : {}),
        ...(event.route && event.route.length > 0 ? { route: event.route } : {}),
        ...(event.file_references && event.file_references.length > 0
          ? { file_references: event.file_references }
          : {}),
        ...(event.route_fields && event.route_fields.length > 0
          ? { supplied_fields: { WorkoutRoute: event.route_fields } }
          : {}),
      } as HardSignalT;
      attribute(payload, session, writer);
      payload.inputReceipts.set(session, event.receipt);
      workouts.push(session);
      continue;
    }
    if (event.kind === "activity") {
      const source = writers.resolve(event.writer, event.device_raw, event.on_day).source;
      for (const summary of event.summaries ?? []) {
        payload.providerSummaries.push({ ...summary, source });
      }
    }
  }

  payload.hardSignals.push(...points);
  await attachReferencedRoutes(detected, workouts, payload);
  await attachRouteSplits(detected, workouts, payload, progress);
  payload.hardSignals.push(...workouts);
  for (const session of raw.sleep_sessions) {
    const writer = writers.resolve(session.writer, session.device_raw, "");
    const night = {
      type: "sleep_session",
      start: session.start,
      end: session.end,
      source: writer.source,
      aggregates: session.aggregates,
    } as HardSignalT;
    const device = parseAppleDevice(session.device_raw);
    if (device) payload.deviceOf.set(night, deviceIndexKey(writer.source, device));
    payload.inputReceipts.set(night, session.receipt);
    payload.hardSignals.push(night);
  }

  await importEcgs(detected, sources, payload, progress);
  for (let index = 0; index < shortWindows; index += 1) {
    countSkip(payload, "HRV beat windows too short or sparse for RMSSD");
  }
  payload.hardSignals.push(...derived);

  for (const { source, day, samples } of beatsByDay.values()) {
    const built = buildSeries("hrv_beats", source, day, samples);
    if (built) payload.series.push(built);
  }
  for (const bucket of series.values()) {
    for (const [day, samples] of bucket.byDay) {
      const built = buildSeries(bucket.quantity as Parameters<typeof buildSeries>[0], bucket.source, day, samples);
      if (built) payload.series.push(built);
    }
  }
  return payload;
}

/**
 * Give each workout the splits from its GPS route, where one exists.
 *
 * Routes are matched to workouts by overlapping time rather than by the date in the
 * filename, since filenames are rounded to the minute and a wearer can start two
 * activities close together.
 *
 * A workout that already has laps from the watch keeps them: the wearer pressed the
 * button deliberately, and that division means more to them than an even kilometre.
 */
/**
 * A WorkoutRoute that names a GPX file has no inline points. The file is the route.
 * The path on the FileReference is how the workout and the file are matched.
 */
async function attachReferencedRoutes(
  detected: DetectedExport,
  workouts: HardSignalT[],
  payload: ImportPayload,
): Promise<void> {
  const files = await readEntries(detected, isRouteEntry);
  if (files.size === 0) return;
  for (const workout of workouts) {
    if (workout.type !== "workout_session") continue;
    const holder = workout as HardSignalT & {
      route?: RoutePointImport[];
      file_references?: string[];
    };
    if ((holder.route?.length ?? 0) > 0) continue;
    for (const reference of holder.file_references ?? []) {
      const text = gpxText(files, reference);
      if (text === undefined) {
        countSkipWithExample(
          payload,
          "workout route file named by a WorkoutRoute was not in the export",
          reference,
        );
        continue;
      }
      const built = routeFromGpx(text);
      for (const example of built.skipped) {
        countSkipWithExample(payload, "GPX route points we could not store", example);
      }
      if (built.points.length > 0) holder.route = [...(holder.route ?? []), ...built.points];
    }
  }
}

function gpxText(files: Map<string, string>, reference: string): string | undefined {
  const wanted = reference.replaceAll("\\", "/").replace(/^\/+/, "").toLowerCase();
  for (const [name, text] of files) {
    const entry = name.replaceAll("\\", "/").replace(/^\/+/, "").toLowerCase();
    if (entry === wanted || entry.endsWith(`/${wanted}`) || wanted.endsWith(`/${entry}`)) {
      return text;
    }
  }
  return undefined;
}

function routeFromGpx(text: string): { points: RoutePointImport[]; skipped: string[] } {
  const points: RoutePointImport[] = [];
  const skipped: string[] = [];
  const pattern = /<trkpt\b([^>]*)>([\s\S]*?)<\/trkpt>|<trkpt\b([^>]*)\/>/g;
  let match: RegExpExecArray | null;
  let index = 0;
  while ((match = pattern.exec(text)) !== null) {
    index += 1;
    const attrs = match[1] ?? match[3] ?? "";
    const body = match[2] ?? "";
    const latitude = Number(/\blat\s*=\s*"([^"]+)"/.exec(attrs)?.[1]);
    const longitude = Number(/\blon\s*=\s*"([^"]+)"/.exec(attrs)?.[1]);
    const at = /<time>([^<]+)<\/time>/.exec(body)?.[1]?.trim();
    if (!at || Number.isNaN(Date.parse(at))) {
      skipped.push(`track point ${index} had no usable time`);
      continue;
    }
    if (!Number.isFinite(latitude) || latitude < -90 || latitude > 90) {
      skipped.push(`track point ${index} latitude "${/\blat\s*=\s*"([^"]+)"/.exec(attrs)?.[1] ?? "(none)"}"`);
      continue;
    }
    if (!Number.isFinite(longitude) || longitude < -180 || longitude > 180) {
      skipped.push(`track point ${index} longitude "${/\blon\s*=\s*"([^"]+)"/.exec(attrs)?.[1] ?? "(none)"}"`);
      continue;
    }
    const elevation = /<ele>([^<]+)<\/ele>/.exec(body)?.[1]?.trim();
    points.push({
      ...routeInstant(at),
      latitude_deg: latitude,
      longitude_deg: longitude,
      ...(elevation === undefined || elevation === "" ? {} : { supplied_fields: { ele: elevation } }),
    });
  }
  return { points, skipped };
}

function routeInstant(raw: string): Pick<RoutePointImport, "recorded_at" | "input_timestamp"> {
  const offset = /(Z|[+-]\d{2}:?\d{2})$/i.exec(raw.trim())?.[1];
  return {
    recorded_at: new Date(Date.parse(raw)).toISOString(),
    input_timestamp: {
      timestamp_text: raw,
      timestamp_basis: "source_offset_timestamp",
      ...(offset === undefined
        ? { utc_offset_missing_reason: "not_provided_by_source" as const }
        : {
            utc_offset_text: offset,
            utc_offset_meaning: "meaning_not_documented" as const,
          }),
    },
  };
}

async function attachRouteSplits(
  detected: DetectedExport,
  workouts: HardSignalT[],
  payload: ImportPayload,
  progress: Progress,
): Promise<void> {
  const files = await readEntries(detected, isRouteEntry, (done, total) => {
    if (done === 1) progress.start("reading workout routes", total);
    progress.update(done);
  });
  progress.finish();
  if (files.size === 0) return;

  const summaries: RouteSummary[] = [];
  for (const [name, text] of files) {
    const summary = summarizeRoute(parseGpx(text));
    if (!summary) {
      countSkipWithExample(payload, "workout routes with too few track points", name);
      continue;
    }
    summaries.push(summary);
  }

  let matched = 0;
  for (const workout of workouts) {
    if (workout.type !== "workout_session") continue;
    const workoutStart = Date.parse(workout.start);
    const workoutEnd = Date.parse(workout.end);

    const route = summaries.find((item) => {
      const routeStart = Date.parse(item.start);
      const routeEnd = Date.parse(item.end);
      return routeStart < workoutEnd && routeEnd > workoutStart;
    });
    if (!route) continue;

    matched++;
    if (route.elevationGainM > 0) {
      (workout.aggregates as Record<string, number>).elevation_gain_m = route.elevationGainM;
    }
    if (!workout.segments || workout.segments.length === 0) {
      if (route.splits.length > 0) workout.segments = route.splits;
    }
  }

  const unmatched = summaries.length - matched;
  if (unmatched > 0) {
    countSkip(payload, "workout routes with no workout in the same window", unmatched);
  }
}

/**
 * Read the ECG recordings and derive an RMSSD from each.
 *
 * These land under their own source. The watch measures beats optically all day
 * and electrically only during an ECG, and those two figures are not pooled.
 */
async function importEcgs(
  detected: DetectedExport,
  sources: SourceBook,
  payload: ImportPayload,
  progress: Progress,
): Promise<void> {
  const files = await readEntries(detected, isEcgEntry, (done, total) => {
    if (done === 1) progress.start("reading ECG recordings", total);
    progress.update(done);
  });
  progress.finish();
  if (files.size === 0) return;

  const ecgSourceId = sources.forWriter({ writer: "Apple Watch", vendor: "apple", sensor: "ecg" });
  const beatsByDay = new Map<string, Sample[]>();

  for (const [name, text] of files) {
    const parsed = parseEcgCsv(text);
    if ("kind" in parsed) {
      countSkipWithExample(payload, "ECG recordings we could not read", `${basename(name)}: ${parsed.detail}`);
      continue;
    }

    const beats = beatsFromEcg(parsed);
    if ("kind" in beats) {
      const reason =
        beats.kind === "not_sinus"
          ? "ECG recordings not in sinus rhythm, where variability does not describe recovery"
          : "ECG recordings with too few detectable beats";
      countSkipWithExample(payload, reason, `${basename(name)}: ${beats.detail}`);
      continue;
    }

    const day = beats.recordedAt.slice(0, 10);
    const samples = beatsByDay.get(day) ?? [];
    const startMs = Date.parse(beats.recordedAt);
    for (let i = 1; i < beats.offsetsMs.length; i++) {
      samples.push({
        at: atOffsetOf(beats.recordedAt, startMs + beats.offsetsMs[i]!),
        value: Math.round(beats.intervalsMs[i - 1]! * 10) / 10,
      });
    }
    beatsByDay.set(day, samples);

    const rmssd = rmssdFromIntervals(beats.intervalsMs);
    if (!rmssd) {
      countSkipWithExample(
        payload,
        "ECG recordings with too few usable intervals for RMSSD",
        basename(name),
      );
      continue;
    }

    const derivedReading = {
      type: "hrv_rmssd",
      value: rmssd.rmssd_ms,
      unit: "ms",
      recorded_at: beats.recordedAt,
      source: ecgSourceId,
      derived: {
        from: "ecg_beats",
        method: "rmssd",
        window_s: rmssd.window_s,
        n_beats: rmssd.n_beats,
        ...(rmssd.n_dropped > 0 ? { n_dropped: rmssd.n_dropped } : {}),
        ...(beats.quality > 0 ? { signal_quality: beats.quality } : {}),
      },
    } as HardSignalT;
    payload.inputReceipts.set(derivedReading, {
      input_timestamp: {
        timestamp_text: beats.recordedAt,
        timestamp_basis: "source_offset_timestamp",
        ...(/(Z|[+-]\d{2}:?\d{2})$/i.exec(beats.recordedAt.trim())?.[1] === undefined
          ? { utc_offset_missing_reason: "not_provided_by_source" as const }
          : {
              utc_offset_text: /(Z|[+-]\d{2}:?\d{2})$/i.exec(beats.recordedAt.trim())![1],
              utc_offset_meaning: "meaning_not_documented" as const,
            }),
      },
      input_value_missing_reason: "not_applicable",
      input_unit_missing_reason: "not_applicable",
    });
    payload.hardSignals.push(derivedReading);
  }

  for (const [day, samples] of beatsByDay) {
    const built = buildSeries("ecg_beats", ecgSourceId, day, samples);
    if (built) payload.series.push(built);
  }
}

function atOffsetOf(reference: string, instantMs: number): string {
  const rounded = Math.round(instantMs);
  const offset = /([+-]\d{2}:\d{2})$/.exec(reference)?.[1];
  if (!offset) return new Date(rounded).toISOString();

  const sign = offset.startsWith("-") ? -1 : 1;
  const [oh, om] = offset.slice(1).split(":").map(Number);
  const offsetMs = sign * ((oh! * 60 + om!) * 60_000);
  const shifted = new Date(rounded + offsetMs).toISOString();
  return `${shifted.slice(0, 23)}${offset}`;
}
