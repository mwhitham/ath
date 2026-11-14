/**
 * Generate language-neutral validation vectors from the 2.1.0 Zod schemas.
 *
 * Every verdict, parsed output, and issue list is observed by running the real
 * schema from src/v1/schema.ts and src/v1/extensions.ts. Nothing here predicts a
 * result. The native validator must reproduce these files exactly.
 *
 * Output:
 *   native/tests/vectors/model/<ExportName>.json   one file per exported schema
 *   native/tests/vectors/model-index.json           file list, counts, rule index
 *   native/tests/vectors/json/serialization.json    JavaScript JSON behavior
 *
 * Run from the repository root:
 *   node --import tsx native/tools/reference/generate-model-vectors.ts
 *
 * The output is deterministic. All inputs are invented.
 */
import { mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { z } from "zod";
import * as Extensions from "../../../src/v1/extensions.js";
import * as Schema from "../../../src/v1/schema.js";
import { stableValue } from "../../../src/db/stable-json.js";

// Date.parse reads a timestamp without a zone as local time. Fix the zone so the
// observed verdicts do not depend on the machine.
process.env.TZ = "UTC";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const vectorDirectory = join(root, "native", "tests", "vectors");
const modelDirectory = join(vectorDirectory, "model");
const jsonDirectory = join(root, "native", "tests", "vectors", "json");

// ---------------------------------------------------------------------------
// Case model
// ---------------------------------------------------------------------------

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
type JsonObject = { [key: string]: Json };
type Expect = "trigger" | "satisfy";
type RuleTag = [string, Expect];

/** One input. `text` is raw JSON source; use it when JSON.parse changes the text. */
interface CaseDef {
  name: string;
  input?: Json;
  text?: string;
  rules?: RuleTag[];
}

interface Mutation {
  label: string;
  value?: Json;
  text?: string;
  remove?: true;
}

const trig = (...ids: string[]): RuleTag[] => ids.map((id) => [id, "trigger"]);
const sat = (...ids: string[]): RuleTag[] => ids.map((id) => [id, "satisfy"]);
const both = (trigger: string[], satisfy: string[]): RuleTag[] => [
  ...trig(...trigger),
  ...sat(...satisfy),
];

function c(name: string, input: Json, rules?: RuleTag[]): CaseDef {
  return rules === undefined ? { name, input } : { name, input, rules };
}

function t(name: string, text: string, rules?: RuleTag[]): CaseDef {
  return rules === undefined ? { name, text } : { name, text, rules };
}

function fail(message: string): never {
  throw new Error(message);
}

// ---------------------------------------------------------------------------
// Semantic rules (see native/reference/semantic-rules.md)
// ---------------------------------------------------------------------------

const RULE_MESSAGES: Record<string, string | RegExp> = {
  R1: "A missing input timestamp needs a reason",
  R2: "An available input timestamp cannot also be marked missing",
  R3: "An input timestamp needs its documented basis",
  R4: "A missing input UTC offset needs a reason",
  R5: "An available input UTC offset cannot also be marked missing",
  R6: "An input UTC offset needs its documented meaning",
  R7: "UTC offset meaning requires UTC offset text",
  R8: "A source timezone and its documented meaning must be provided together",
  R9: "A provider reporting period needs an ID, date, or start instant",
  R10: "Provider reporting period end cannot precede its start",
  R11: "A provider UTC offset needs its documented meaning",
  R12: "An available provider UTC offset cannot also be marked missing",
  R13: "Provide the input value or one missing reason",
  R14: "Provide the input unit or one missing reason",
  R15: "IANA timezone",
  R16: /^[a-z0-9_]+ uses .+$/,
  R17: /^[a-z0-9_]+ must be positive$/,
  R18: /^[a-z0-9_]+ uses .+$/,
  R19: "A reconstructed timestamp needs its base and method, and other timestamps cannot claim them",
  R20: "Sleep session end cannot precede its start",
  R21: "Workout session end cannot precede its start",
  R22: "A score needs duration_s, reps, or weight_kg",
  R23: "A provider summary needs an instant or a provider reporting period",
  R24: "A provider score needs an instant or a provider reporting period",
  R25: "A rating needs its scale",
  R26: "An analysis without causes must mark the error unexplained",
  R27: "A planned session needs a date or start instant",
  R28: "previous_revision must be null for revision 1 or the prior revision number",
  R29: "extensions cannot contain source_id",
  R30: "extension key must be a namespaced extension id",
  R31: "extensions cannot contain more than 32 keys",
  R32: "extensions exceed 65536 encoded bytes",
  R33: "extension value exceeds 16384 encoded bytes",
  R34: "extension value exceeds depth 8",
  R35: "extension string exceeds 4096 characters",
  R36: "extension array exceeds 256 elements",
  R37: "extension object exceeds 64 keys",
  R38: "A package cannot contain the same current record twice",
  R39: "A package cannot contain the same record revision twice",
  R40: "A retained revision needs its current record",
  R41: "A revision must keep its record type",
  R42: "A deletion record needs its retained current record",
  R43: "A record can have only one current deletion record",
  R44: "A complete package needs every revision through current_revision",
  R45: "Record revision numbers must be complete and consecutive",
  R46: "The current record must equal its current retained revision",
  R47: "A deleted current record needs its deletion record",
  R48: "A current deletion record requires is_deleted",
  R49: "A deletion record must name the current retained revision",
  R50: "A provider import key needs its current record",
  R51: "One provider import key cannot identify two records",
  R52: "Series file end cannot precede its start",
  R53: "Every sample must match the series source and quantity",
  R54: "Every current sample must fall inside the series time bounds",
  R55: "A package cannot reference the same measurement-series file twice",
  R56: "An erased record cannot remain in current package records",
};

// ---------------------------------------------------------------------------
// Zod introspection (Zod 4 internals; used only to choose inputs)
// ---------------------------------------------------------------------------

type ZodDef = {
  type: string;
  [key: string]: unknown;
};

function zdef(schema: unknown): ZodDef {
  return (schema as { _zod: { def: ZodDef } })._zod.def;
}

function checksOf(schema: unknown): Array<Record<string, unknown>> {
  const checks = (zdef(schema).checks ?? []) as Array<{ _zod: { def: Record<string, unknown> } }>;
  return checks.map((check) => check._zod.def);
}

function unwrap(schema: unknown): { schema: unknown; optional: boolean; nullable: boolean } {
  let current = schema;
  let optional = false;
  let nullable = false;
  for (;;) {
    const definition = zdef(current);
    if (definition.type === "optional") {
      optional = true;
      current = definition.innerType;
    } else if (definition.type === "nullable") {
      nullable = true;
      current = definition.innerType;
    } else {
      return { schema: current, optional, nullable };
    }
  }
}

function shapeOf(schema: unknown): Record<string, unknown> {
  const definition = zdef(schema);
  if (definition.type !== "object") fail("not an object schema");
  return definition.shape as Record<string, unknown>;
}

function stringFormat(schema: unknown): string | undefined {
  const definition = zdef(schema);
  if (typeof definition.format === "string") return definition.format;
  const check = checksOf(schema).find((item) => item.check === "string_format");
  return check?.format as string | undefined;
}

function stringMutations(schema: unknown, base: Json | undefined): Mutation[] {
  const mutations: Mutation[] = [{ label: "wrong_type=number", value: 1 }];
  const checks = checksOf(schema);
  const format = stringFormat(schema);
  mutations.push({ label: "empty_string", value: "" });
  for (const check of checks) {
    if (check.check === "max_length") {
      const maximum = check.maximum as number;
      mutations.push({ label: `length=${maximum}`, value: "a".repeat(maximum) });
      mutations.push({ label: `length=${maximum + 1}`, value: "a".repeat(maximum + 1) });
    }
    if (check.check === "custom") {
      mutations.push({ label: "refine_invalid", value: "Not/AZone" });
    }
  }
  if (format === "datetime") {
    mutations.push({ label: "format=no_milliseconds", value: "2026-01-02T03:04:05Z" });
    mutations.push({ label: "format=offset", value: "2026-01-02T03:04:05.678+00:00" });
  } else if (format === "date") {
    mutations.push({ label: "format=february_30", value: "2026-02-30" });
    mutations.push({ label: "format=datetime", value: "2026-01-02T03:04:05.678Z" });
  } else if (format === "regex") {
    mutations.push({ label: "format=not_valid", value: "not valid" });
    if (typeof base === "string" && base.toUpperCase() !== base) {
      mutations.push({ label: "format=uppercase", value: base.toUpperCase() });
    }
  }
  return mutations;
}

function numberMutations(schema: unknown, base: Json | undefined): Mutation[] {
  const checks = checksOf(schema);
  const isInteger = checks.some((check) => check.check === "number_format");
  const lower = checks.filter((check) => check.check === "greater_than");
  const upper = checks.filter((check) => check.check === "less_than");
  const values: number[] = [];
  for (const bound of [...lower, ...upper]) {
    const value = bound.value as number;
    values.push(value - 1, value, value + 1);
    if (!isInteger) values.push(value - 0.5, value + 0.5);
    if (!isInteger && value === 0) values.push(5e-324, -5e-324);
  }
  if (isInteger) {
    values.push(
      1.5,
      Number.MAX_SAFE_INTEGER,
      2 ** 53,
      -Number.MAX_SAFE_INTEGER,
      -(2 ** 53),
      1e21,
    );
  } else {
    if (upper.length === 0) values.push(1e308);
    if (lower.length === 0) values.push(-1e308);
    if (lower.length === 0 && upper.length === 0) values.push(-1.5, 0, 5e-324);
  }
  const seen = new Set<string>();
  const mutations: Mutation[] = [{ label: "wrong_type=string", value: "1" }];
  for (const value of values) {
    const text = JSON.stringify(value);
    if (seen.has(text)) continue;
    seen.add(text);
    mutations.push({ label: `number=${text}`, value });
  }
  const texts: string[] = ["-0"];
  if (isInteger) {
    const valid = typeof base === "number" ? base : 1;
    texts.push(`${valid}.0`, `${valid}E0`, "9007199254740993");
  } else {
    texts.push("1e400");
  }
  for (const text of texts) mutations.push({ label: `number_text=${text}`, text });
  return mutations;
}

function enumOptions(schema: unknown): string[] {
  return Object.values(zdef(schema).entries as Record<string, string>);
}

function valueMutations(schema: unknown, base: Json | undefined): Mutation[] {
  const definition = zdef(schema);
  switch (definition.type) {
    case "string":
      return stringMutations(schema, base);
    case "number":
      return numberMutations(schema, base);
    case "boolean":
      return [
        { label: "wrong_type=string", value: "true" },
        { label: "wrong_type=number", value: 0 },
      ];
    case "enum": {
      const options = enumOptions(schema);
      const mutations: Mutation[] = [
        { label: "wrong_type=number", value: 1 },
        { label: "enum_invalid", value: "__not_an_option__" },
        { label: "empty_string", value: "" },
      ];
      const first = options[0] ?? "";
      if (first.toUpperCase() !== first) {
        mutations.push({ label: "enum_uppercase", value: first.toUpperCase() });
      }
      return mutations;
    }
    case "literal":
      return [
        { label: "literal_other", value: "__other__" },
        { label: "wrong_type=number", value: 1 },
      ];
    case "object":
      return [
        { label: "wrong_type=array", value: [] },
        { label: "wrong_type=string", value: "x" },
        { label: "empty_object", value: {} },
        {
          label: "nested_unknown_key",
          value: { ...((base ?? {}) as JsonObject), unknown_nested: true },
        },
      ];
    case "array": {
      const element = definition.element;
      const sample = Array.isArray(base) && base.length > 0 ? base[0] : undefined;
      const elementMutations = valueMutations(element, sample)
        .filter((mutation) => mutation.text === undefined && mutation.remove === undefined)
        .slice(0, 2);
      const mutations: Mutation[] = [
        { label: "wrong_type=object", value: {} },
        { label: "empty_array", value: [] },
      ];
      for (const mutation of elementMutations) {
        mutations.push({ label: `element[0]:${mutation.label}`, value: [mutation.value as Json] });
      }
      const firstElement = elementMutations[0];
      if (sample !== undefined && firstElement !== undefined) {
        mutations.push({
          label: `element[1]:${firstElement.label}`,
          value: [sample, firstElement.value as Json],
        });
      }
      return mutations;
    }
    case "record": {
      const keyHasMinimum = checksOf(definition.keyType).some(
        (check) => check.check === "min_length",
      );
      const sampleEntry =
        base !== undefined && base !== null && typeof base === "object" && !Array.isArray(base)
          ? Object.entries(base)[0]
          : undefined;
      const sampleValue = sampleEntry?.[1] ?? 1;
      const mutations: Mutation[] = [
        { label: "wrong_type=array", value: [] },
        { label: "wrong_type=string", value: "x" },
        { label: "empty_object", value: {} },
      ];
      if (keyHasMinimum) mutations.push({ label: "empty_key", value: { "": sampleValue } });
      const entryMutations = valueMutations(definition.valueType, sampleValue)
        .filter((mutation) => mutation.text === undefined && mutation.remove === undefined)
        .slice(0, 2);
      for (const mutation of entryMutations) {
        mutations.push({ label: `entry:${mutation.label}`, value: { k: mutation.value as Json } });
      }
      return mutations;
    }
    case "union": {
      const mutations: Mutation[] = [
        { label: "wrong_type=array", value: [] },
        { label: "empty_object", value: {} },
      ];
      const discriminator = definition.discriminator;
      if (typeof discriminator === "string") {
        mutations.push({ label: "discriminator_invalid", value: { [discriminator]: "__other__" } });
      }
      return mutations;
    }
    case "lazy":
      return [];
    default:
      return fail(`no mutations for Zod type ${definition.type}`);
  }
}

const RAW_PLACEHOLDER = "\u0000raw-json\u0000";

function withKey(base: JsonObject, key: string, mutation: Mutation): { input?: Json; text?: string } {
  if (mutation.remove === true) {
    const copy = { ...base };
    delete copy[key];
    return { input: copy };
  }
  if (mutation.text !== undefined) {
    const marked = JSON.stringify({ ...base, [key]: RAW_PLACEHOLDER });
    return { text: marked.replace(JSON.stringify(RAW_PLACEHOLDER), mutation.text) };
  }
  return { input: { ...base, [key]: mutation.value as Json } };
}

interface ObjectSpec {
  /** Named valid examples. The first is minimal; the second is maximal. */
  valid: Array<[string, JsonObject]>;
  /** Optional keys that cannot coexist with the maximal example. */
  maxExempt?: string[];
  extras?: CaseDef[];
}

function objectCases(name: string, schema: unknown, spec: ObjectSpec): CaseDef[] {
  const shape = shapeOf(schema);
  const keys = Object.keys(shape);
  const [minimalEntry, maximalEntry] = spec.valid;
  if (minimalEntry === undefined) fail(`${name}: no valid example`);
  const minimal = minimalEntry[1];
  const maximal = (maximalEntry ?? minimalEntry)[1];
  for (const key of keys) {
    const { optional } = unwrap(shape[key]);
    if (!optional && !(key in minimal)) fail(`${name}: minimal example lacks required ${key}`);
    if (!(key in maximal) && !(spec.maxExempt ?? []).includes(key)) {
      fail(`${name}: maximal example lacks ${key}`);
    }
    if (!spec.valid.some(([, example]) => key in example)) {
      fail(`${name}: no valid example contains ${key}`);
    }
  }
  const cases: CaseDef[] = spec.valid.map(([label, example]) => c(`valid:${label}`, example));
  cases.push(c("root:wrong_type=null", null));
  cases.push(c("root:wrong_type=array", []));
  cases.push(c("root:wrong_type=string", "x"));
  cases.push(c("unknown_key", { ...maximal, unknown_field: true }));
  for (const key of keys) {
    const { schema: inner, optional, nullable } = unwrap(shape[key]);
    const base = optional
      ? (spec.valid.find(([, example]) => key in example)?.[1] ?? fail(`${name}: ${key}`))
      : minimal;
    const baseValue = base[key];
    const mutations: Mutation[] = [];
    if (!optional) mutations.push({ label: "missing", remove: true });
    if (!nullable) mutations.push({ label: "null", value: null });
    mutations.push(...valueMutations(inner, baseValue));
    for (const mutation of mutations) {
      cases.push({ name: `${key}:${mutation.label}`, ...withKey(base, key, mutation) });
    }
  }
  return [...cases, ...(spec.extras ?? [])];
}

function enumCases(schema: unknown): CaseDef[] {
  const options = enumOptions(schema);
  const first = options[0] ?? fail("empty enum");
  const cases = options.map((option) => c(`valid:${option}`, option));
  cases.push(
    c("invalid:unknown", "__not_an_option__"),
    c("invalid:empty", ""),
    c("invalid:uppercase", first.toUpperCase()),
    c("invalid:leading_space", ` ${first}`),
    c("invalid:trailing_space", `${first} `),
    c("invalid:number", 1),
    c("invalid:null", null),
    c("invalid:array", [first]),
    c("invalid:object", {}),
  );
  return cases;
}

function stringListCases(valid: Array<[string, string]>, invalid: Array<[string, Json]>): CaseDef[] {
  return [
    ...valid.map(([label, value]) => c(label, value)),
    ...invalid.map(([label, value]) => c(label, value)),
    c("wrong_type:null", null),
    c("wrong_type:number", 1),
    c("wrong_type:boolean", true),
    c("wrong_type:array", []),
    c("wrong_type:object", {}),
  ];
}

// ---------------------------------------------------------------------------
// Invented building blocks
// ---------------------------------------------------------------------------

const id = (sequence: number): string =>
  `01890f3e-e7d3-7cc0-88c8-${sequence.toString(16).padStart(12, "0")}`;
const T0 = "2026-01-02T03:04:05.678Z";
const T1 = "2026-01-02T04:04:05.678Z";
const T_BEFORE = "2026-01-02T03:04:05.677Z";
const T_AFTER = "2026-01-02T04:04:05.679Z";
const DAY = "2026-01-02";
const SHA = "0123456789abcdef".repeat(4);
const EXTENSION_KEY = "org.example/vectors";

const itrMin: JsonObject = {
  timestamp_missing_reason: "not_provided_by_source",
  utc_offset_missing_reason: "not_provided_by_source",
};
const itrMax: JsonObject = {
  timestamp_text: "2026-01-01T21:04:05.678-06:00",
  utc_offset_text: "-06:00",
  utc_offset_meaning: "documented_recording_offset",
  source_time_zone: "America/Chicago",
  source_time_zone_meaning: "apple_health_object_creation_time_zone",
  timestamp_basis: "source_offset_timestamp",
};
const svMin: JsonObject = {
  standard_value: 42,
  standard_unit: "ms",
  input_value_missing_reason: "not_provided_by_source",
  input_unit_missing_reason: "not_provided_by_source",
};
const svMax: JsonObject = {
  standard_value: 42.5,
  standard_unit: "ms",
  input_value: 42.5,
  input_unit: "ms",
};
const srsMin: JsonObject = {
  standard_value: 91,
  standard_unit: "%",
  input_value_missing_reason: "not_exposed_by_route",
  input_unit_missing_reason: "not_exposed_by_route",
  summary_type: "sleep_efficiency",
};
const srsMax: JsonObject = {
  standard_value: 91,
  standard_unit: "%",
  input_value: 0.91,
  input_unit: "ratio",
  summary_type: "sleep_efficiency",
};
const prpMin: JsonObject = { period_type: "oura_sleep_day", provider_date: DAY };
const prpMax: JsonObject = {
  period_type: "whoop_physiological_cycle",
  provider_period_id: "cycle-0001",
  provider_date: DAY,
  start_at: T0,
  end_at: T1,
  input_start_timestamp: itrMax,
  input_end_timestamp: itrMax,
  timezone_offset_text: "-06:00",
  timezone_offset_meaning: "provider_cycle_offset",
};
const suppliedMax: JsonObject = {
  "Raw Value": "72",
  nested: { list: [1, 2.5, null, true, "x"] },
};
const fieldBasisMax: JsonObject = {
  activity_type: "provider_label",
  value: "provider",
  route_summaries: "calculated_on_import",
};
const measuredMin: JsonObject = {
  source_id: id(3),
  received_at: T0,
  received_at_basis: "import_time",
};
const measuredMax: JsonObject = {
  source_id: id(3),
  provider_record_id: "provider-0001",
  provider_reporting_periods: [prpMax],
  received_at: T0,
  received_at_basis: "provider_supplied",
  supplied_fields: suppliedMax,
  field_basis: fieldBasisMax,
};
const evidence = (sequence: number, revision = 1): JsonObject => ({
  record_id: id(sequence),
  revision,
});
const athleteActor: JsonObject = { author_type: "athlete" };
const agentActorMax: JsonObject = {
  author_type: "agent",
  agent: "reference-agent",
  agent_version: "2.1.0",
};
const toolActor: JsonObject = { author_type: "tool", tool: "ath", tool_version: "2.1.0" };
const agentAuthorMax: JsonObject = {
  ...agentActorMax,
  model: "example-model",
  model_version: "2026-01",
};
const toolAuthor: JsonObject = { ...toolActor, calculation_version: "1" };
const authMin: JsonObject = {
  author: athleteActor,
  authored_at: T0,
  evidence: [],
  content_version: 1,
  status: "active",
};
const authMax: JsonObject = {
  author: agentAuthorMax,
  authored_at: T0,
  evidence: [evidence(7)],
  content_version: 2,
  status: "completed",
};
const scoreMax: JsonObject = { duration_s: 300.5, reps: 21, weight_kg: 43 };
const pmcMin: JsonObject = {
  from_quantity: "hrv_beats",
  method: "rmssd",
  calculation_version: "1.0.0",
  input_records: [],
};
const pmcMax: JsonObject = {
  ...pmcMin,
  window_s: 300,
  usable_sample_count: 280,
  dropped_sample_count: 3,
  signal_quality: 0.95,
  input_records: [evidence(8)],
};
const routeMin: JsonObject = { recorded_at: T0, latitude_deg: 10.5, longitude_deg: -20.25 };
const routeMax: JsonObject = {
  ...routeMin,
  input_timestamp: itrMax,
  supplied_fields: { altitude_m: 12.5 },
};
const segmentMin: JsonObject = { label: "warm-up", source_reported_summaries: [] };
const segmentMax: JsonObject = {
  label: "warm-up",
  start_offset_ms: 0,
  end_offset_ms: 600000,
  source_reported_summaries: [srsMax],
};
const nutrientMax: JsonObject = { nutrient_name: "protein", amount: 10.5, unit: "g" };
const itemMin: JsonObject = { food_description: "oatmeal" };
const itemMax: JsonObject = {
  food_description: "oatmeal",
  amount: 80,
  amount_unit: "g",
  nutrients: [nutrientMax],
};
const blockMin: JsonObject = { block_type: "main", title: "6 x 400 m" };
const blockMax: JsonObject = {
  ...blockMin,
  instructions: "Even splits",
  planned_duration_s: 1800,
  planned_distance_m: 2400,
  planned_reps: 6,
  planned_weight_kg: 20,
};
const extensionsMax: JsonObject = {
  [EXTENSION_KEY]: { label: "invented", count: 2, flags: [true, false], nested: { value: null } },
};

const sampleDataMin: JsonObject = {
  ...measuredMin,
  quantity: "heart_rate",
  recorded_at: T0,
  input_timestamp: itrMin,
  value: { ...svMin, standard_value: 61, standard_unit: "bpm" },
};

/** Minimal and maximal valid data for every record type. */
const DATA: Record<string, { min: JsonObject; max: JsonObject; others?: Array<[string, JsonObject]> }> = {
  athlete_profile: {
    min: {},
    max: {
      name: "Sample Athlete",
      birth_year: 1990,
      sex: "female",
      display_units: "metric",
      default_time_zone: "America/Chicago",
    },
  },
  source: {
    min: { source_type: "wearable", data_writer: "Example Band", data_route: "provider_export" },
    max: {
      source_type: "connector",
      data_writer: "Example Band",
      data_route: "provider_export",
      vendor: "Example Vendor",
      sensor_type: "optical",
      detail: "Invented source for vectors",
      supplied_fields: suppliedMax,
    },
  },
  source_device: {
    min: { source_id: id(3) },
    max: {
      source_id: id(3),
      name: "Example Band 2",
      manufacturer: "Example Vendor",
      model: "EB-2",
      hardware: "rev-b",
      serial_number: "SN-0000",
      coverage_start_date: "2026-01-01",
      coverage_end_date: "2026-01-31",
      accepted_record_count: 10,
    },
  },
  point_measurement: {
    min: { ...measuredMin, quantity: "hrv_rmssd", input_timestamp: itrMin, value: svMin },
    max: {
      ...measuredMax,
      quantity: "hrv_rmssd",
      recorded_at: T0,
      input_timestamp: itrMax,
      value: svMax,
      calculations: [pmcMax],
    },
  },
  measurement_sample: {
    min: sampleDataMin,
    max: {
      ...measuredMax,
      quantity: "heart_rate",
      recorded_at: T0,
      input_timestamp: { ...itrMax, timestamp_basis: "reconstructed_from_series_start" },
      value: { standard_value: 61, standard_unit: "bpm", input_value: 61, input_unit: "count/min" },
      duration_ms: 1000,
      timestamp_base_text: "2026-01-01T21:00:00.000-06:00",
      timestamp_reconstruction_method: "series_start_plus_offset",
    },
  },
  sleep_session: {
    min: {
      ...measuredMin,
      start_at: T0,
      end_at: T1,
      input_start_timestamp: itrMin,
      input_end_timestamp: itrMin,
      source_reported_summaries: [],
    },
    max: {
      ...measuredMax,
      start_at: T0,
      end_at: T1,
      input_start_timestamp: itrMax,
      input_end_timestamp: itrMax,
      source_reported_summaries: [srsMax],
      asleep_seconds: 3300,
      in_bed_seconds: 3600,
      interruptions: 1,
      stage_minutes: { awake: 5, light: 30, deep: 15, rem: 10 },
    },
  },
  workout_session: {
    min: {
      ...measuredMin,
      start_at: T0,
      end_at: T1,
      input_start_timestamp: itrMin,
      input_end_timestamp: itrMin,
      source_reported_summaries: [],
    },
    max: {
      ...measuredMax,
      start_at: T0,
      end_at: T1,
      input_start_timestamp: itrMax,
      input_end_timestamp: itrMax,
      activity_type: "running",
      name: "Morning run",
      description: "Easy pace",
      text_result: "5 km",
      maximum_heart_rate_bpm: 171,
      resting_heart_rate_bpm: 52,
      heart_rate_samples: [{ offset_ms: 0, bpm: 120 }],
      sets: [{ reps: 5, weight: 60, weight_unit: "kg" }],
      route: [routeMax],
      source_reported_summaries: [srsMax],
      segments: [segmentMax],
    },
  },
  benchmark_result: {
    min: {
      ...measuredMin,
      benchmark_definition_id: id(6),
      recorded_at: T0,
      input_timestamp: itrMin,
      score: { duration_s: 300 },
    },
    max: {
      ...measuredMax,
      benchmark_definition_id: id(6),
      recorded_at: T0,
      input_timestamp: itrMax,
      score: scoreMax,
      input_score_text: "5:00",
      scaling: "rx",
      workout_session_id: id(10),
      segments: [{ label: "round 1", score: { reps: 21 } }],
    },
  },
  provider_summary: {
    min: {
      ...measuredMin,
      summary_type: "daily_steps",
      value: { ...svMin, standard_value: 8000, standard_unit: "count" },
      recorded_at: T0,
      input_timestamp: itrMin,
    },
    max: {
      ...measuredMax,
      summary_type: "daily_steps",
      value: { standard_value: 8000, standard_unit: "count", input_value: 8000, input_unit: "steps" },
      recorded_at: T0,
      input_timestamp: itrMax,
    },
    others: [
      [
        "period_without_instant",
        {
          ...measuredMin,
          provider_reporting_periods: [prpMin],
          summary_type: "daily_steps",
          value: { ...svMin, standard_value: 8000, standard_unit: "count" },
          input_timestamp: itrMin,
        },
      ],
    ],
  },
  provider_score: {
    min: {
      ...measuredMin,
      metric: "recovery",
      value: { ...svMin, standard_value: 66, standard_unit: "%" },
      scale: "0-100",
      recorded_at: T0,
      input_timestamp: itrMin,
    },
    max: {
      ...measuredMax,
      metric: "recovery",
      value: { standard_value: 66, standard_unit: "%", input_value: 66, input_unit: "%" },
      scale: "0-100",
      recorded_at: T0,
      input_timestamp: itrMax,
    },
    others: [
      [
        "period_without_instant",
        {
          ...measuredMin,
          provider_reporting_periods: [prpMin],
          metric: "recovery",
          value: { ...svMin, standard_value: 66, standard_unit: "%" },
          scale: "0-100",
          input_timestamp: itrMin,
        },
      ],
    ],
  },
  athlete_report: {
    min: { report_type: "note", reported_at: T0, input_reported_at: itrMin, reported_via: "text" },
    max: {
      report_type: "soreness",
      reported_at: T0,
      input_reported_at: itrMax,
      reported_via: "voice",
      rating: 3,
      rating_scale: "0-10",
      body_region: "left calf",
      note: "Mild soreness after intervals.",
      duration_seconds: 45,
      attachment_ids: [id(14)],
    },
  },
  nutrition_entry: {
    min: {
      reported_at: T0,
      input_reported_at: itrMin,
      reported_via: "text",
      athlete_description: "Oatmeal with berries",
      reported_items: [],
    },
    max: {
      reported_at: T0,
      input_reported_at: itrMax,
      reported_via: "photo",
      athlete_description: "Oatmeal with berries",
      reported_items: [itemMax],
      attachment_ids: [id(14)],
    },
  },
  attachment: {
    min: { file_name: "meal.png", media_type: "image/png", byte_count: 1024, sha256: SHA },
    max: {
      file_name: "meal.png",
      media_type: "image/png",
      byte_count: 1024,
      sha256: SHA,
      description: "Meal photo",
    },
  },
  benchmark_definition: {
    min: {
      authorship: authMin,
      benchmark_key: "fran",
      benchmark_type: "named_wod",
      score_type: "time",
      definition: "21-15-9 thrusters and pull-ups",
    },
    max: {
      authorship: authMax,
      benchmark_key: "fran",
      benchmark_type: "named_wod",
      score_type: "time",
      definition: "21-15-9 thrusters and pull-ups",
      tags: ["couplet", "classic"],
    },
  },
  prediction: {
    min: {
      authorship: authMin,
      benchmark_definition_id: id(6),
      predicted_score: { duration_s: 300 },
      confidence: "moderate",
      reasoning: "Recent intervals trend faster.",
    },
    max: {
      authorship: authMax,
      benchmark_definition_id: id(6),
      predicted_score: { duration_s: 300 },
      predicted_range: { low: { duration_s: 280 }, high: { duration_s: 320 } },
      confidence: "high",
      reasoning: "Recent intervals trend faster.",
    },
  },
  prediction_result_link: {
    min: {
      prediction: evidence(16),
      benchmark_result: evidence(11),
      created_by: toolActor,
      linked_at: T0,
      content_version: 1,
      status: "active",
    },
    max: {
      prediction: evidence(16, 2),
      benchmark_result: evidence(11, 3),
      created_by: agentActorMax,
      linked_at: T0,
      content_version: 2,
      status: "superseded",
    },
  },
  prediction_grade: {
    min: {
      authorship: authMin,
      prediction_result_link: evidence(17),
      signed_error: -5,
      absolute_error_percentage: 1.6,
      result_in_predicted_range: true,
      predicted_score: { duration_s: 300 },
      actual_score: { duration_s: 295 },
    },
    max: {
      authorship: { ...authMax, author: toolAuthor },
      prediction_result_link: evidence(17),
      signed_error: 12.5,
      absolute_error_percentage: 0,
      result_in_predicted_range: false,
      predicted_score: scoreMax,
      actual_score: scoreMax,
    },
  },
  prediction_error_analysis: {
    min: {
      authorship: authMin,
      prediction_grade: evidence(18),
      error_direction: "faster",
      error_severity: "minor",
      causes: [],
      unexplained: true,
    },
    max: {
      authorship: authMax,
      prediction_grade: evidence(18),
      error_direction: "slower",
      error_severity: "significant",
      causes: [{ evidence: evidence(9), explanation: "Slept longer than usual." }],
      unexplained: false,
      lesson: "Weight sleep more.",
    },
  },
  training_plan: {
    min: { authorship: authMin, title: "Base block" },
    max: {
      authorship: authMax,
      title: "Base block",
      objective: "Build aerobic base",
      start_date: "2026-01-05",
      end_date: "2026-02-01",
    },
  },
  planned_session: {
    min: {
      authorship: authMin,
      title: "Intervals",
      scheduled_date: DAY,
      blocks: [],
      completion_status: "planned",
    },
    max: {
      authorship: authMax,
      training_plan_id: id(20),
      title: "Intervals",
      scheduled_date: DAY,
      scheduled_start_at: T0,
      activity_type: "running",
      blocks: [blockMax],
      completion_status: "completed",
      completed_workout_session_id: id(10),
    },
  },
  coaching_note: {
    min: { authorship: authMin, note: "Keep easy days easy." },
    max: {
      authorship: authMax,
      title: "Recovery",
      note: "Keep easy days easy.",
      applies_from: "2026-01-01",
      applies_to: "2026-03-31",
    },
  },
  saved_training_review: {
    min: {
      authorship: authMin,
      period_start: "2026-01-01",
      period_end: "2026-01-07",
      summary: "Consistent week.",
    },
    max: {
      authorship: authMax,
      period_start: "2026-01-01",
      period_end: "2026-01-07",
      summary: "Consistent week.",
      recommendations: ["Add one rest day."],
    },
  },
  goal: {
    min: { authorship: authMin, title: "Sub-5 Fran" },
    max: {
      authorship: authMax,
      title: "Sub-5 Fran",
      description: "Finish under five minutes",
      target_date: "2026-06-01",
      target_benchmark_definition_id: id(6),
      target_score: { duration_s: 299 },
    },
  },
  nutrition_model_interpretation: {
    min: {
      authorship: authMin,
      nutrition_entry_id: id(15),
      confidence: 0.5,
      interpreted_items: [],
    },
    max: {
      authorship: authMax,
      nutrition_entry_id: id(15),
      confidence: 0.75,
      interpreted_items: [itemMax],
      explanation: "Portion estimated from the photo.",
    },
  },
};

const RECORD_TYPES = Schema.RecordType.options as readonly string[];
const ATHLETE_REPORTED = new Set(["athlete_report", "nutrition_entry", "attachment", "athlete_profile"]);
const pascal = (value: string): string =>
  value
    .split("_")
    .map((part) => `${part.slice(0, 1).toUpperCase()}${part.slice(1)}`)
    .join("");

function dataFor(type: string): { min: JsonObject; max: JsonObject; others?: Array<[string, JsonObject]> } {
  return DATA[type] ?? fail(`no data example for ${type}`);
}

function currentRecord(type: string, recordId: string, data: JsonObject, over: JsonObject = {}): JsonObject {
  return {
    record_id: recordId,
    record_type: type,
    created_at: T0,
    current_revision: 1,
    is_deleted: false,
    data,
    ...over,
  };
}

function recordRevision(
  type: string,
  recordId: string,
  revision: number,
  data: JsonObject,
  over: JsonObject = {},
): JsonObject {
  return {
    record_id: recordId,
    record_type: type,
    revision,
    previous_revision: revision === 1 ? null : revision - 1,
    became_known_at: T0,
    became_known_at_basis: "local_write",
    changed_by: athleteActor,
    data,
    ...over,
  };
}

const deletion = (recordId: string, revision: number): JsonObject => ({
  record_id: recordId,
  record_revision: revision,
  deleted_at: T1,
  deleted_by: athleteActor,
});

const importKey = (recordId: string, method: string, key: string): JsonObject => ({
  record_id: recordId,
  match_key_method: method,
  match_key: key,
  imported_at: T0,
});

// ---------------------------------------------------------------------------
// Extension builders
// ---------------------------------------------------------------------------

function encodedBytes(value: unknown): number {
  return Buffer.byteLength(JSON.stringify(value), "utf8");
}

/** An array of ASCII strings whose JSON encoding is exactly `target` bytes. */
function stringArrayOfEncodedSize(target: number): string[] {
  const parts: string[] = [];
  let size = 2;
  for (;;) {
    const separator = parts.length === 0 ? 0 : 1;
    const room = target - size - separator - 2;
    if (room < 0) fail(`cannot build ${target} bytes`);
    if (room <= 4000) {
      parts.push("a".repeat(room));
      break;
    }
    parts.push("a".repeat(4000));
    size += separator + 2 + 4000;
  }
  if (encodedBytes(parts) !== target) fail(`array size ${encodedBytes(parts)} != ${target}`);
  return parts;
}

/** Extensions whose JSON encoding is exactly `target` bytes, each value under 16,000 bytes. */
function extensionsOfEncodedSize(target: number): JsonObject {
  const extensions: JsonObject = {};
  for (let index = 0; ; index += 1) {
    const key = `org.example/v${String(index).padStart(2, "0")}`;
    const overhead = JSON.stringify(key).length + 1 + (index === 0 ? 0 : 1);
    const room = target - encodedBytes(extensions) - overhead;
    if (room <= 16000) {
      extensions[key] = stringArrayOfEncodedSize(room);
      break;
    }
    extensions[key] = stringArrayOfEncodedSize(16000);
  }
  if (encodedBytes(extensions) !== target) fail("extensions size mismatch");
  return extensions;
}

function nestArrays(levels: number, inner: Json): Json {
  let value = inner;
  for (let level = 0; level < levels; level += 1) value = [value];
  return value;
}

function nestObjects(levels: number, inner: Json): Json {
  let value = inner;
  for (let level = 0; level < levels; level += 1) value = { k: value };
  return value;
}

function keyedObject(count: number): JsonObject {
  return Object.fromEntries(
    Array.from({ length: count }, (_, index) => [`k${String(index).padStart(2, "0")}`, index]),
  );
}

function manyExtensionKeys(count: number, value: Json = 1): JsonObject {
  return Object.fromEntries(
    Array.from({ length: count }, (_, index) => [`org.example/v${String(index).padStart(2, "0")}`, value]),
  );
}

// ---------------------------------------------------------------------------
// Specifications for every exported schema
// ---------------------------------------------------------------------------

const exported: Record<string, unknown> = { ...Schema, ...Extensions };
const specs = new Map<string, () => CaseDef[]>();

function target(name: string): z.ZodType {
  const value = exported[name];
  if (!(value instanceof z.ZodType)) fail(`${name} is not an exported Zod schema`);
  return value;
}

function objectFile(name: string, spec: ObjectSpec): void {
  specs.set(name, () => objectCases(name, target(name), spec));
}

// --- Formats ---------------------------------------------------------------

specs.set("UtcTimestamp", () =>
  stringListCases(
    [
      ["valid:milliseconds_z", "2026-01-02T03:04:05.678Z"],
      ["valid:leap_day_2024", "2024-02-29T00:00:00.000Z"],
      ["valid:leap_day_2000", "2000-02-29T12:00:00.000Z"],
      ["valid:year_0000_leap_day", "0000-02-29T00:00:00.000Z"],
      ["valid:year_9999", "9999-12-31T23:59:59.999Z"],
      ["valid:day_31_in_31_day_month", "2026-01-31T00:00:00.000Z"],
      ["valid:day_30_in_30_day_month", "2026-04-30T00:00:00.000Z"],
      ["valid:hour_23_second_59", "2026-01-02T23:59:59.000Z"],
    ],
    [
      ["invalid:missing_milliseconds", "2026-01-02T03:04:05Z"],
      ["invalid:one_fractional_digit", "2026-01-02T03:04:05.6Z"],
      ["invalid:two_fractional_digits", "2026-01-02T03:04:05.67Z"],
      ["invalid:four_fractional_digits", "2026-01-02T03:04:05.6789Z"],
      ["invalid:six_fractional_digits", "2026-01-02T03:04:05.678901Z"],
      ["invalid:nine_fractional_digits", "2026-01-02T03:04:05.678901234Z"],
      ["invalid:offset_plus_01_00", "2026-01-02T03:04:05.678+01:00"],
      ["invalid:offset_plus_00_00", "2026-01-02T03:04:05.678+00:00"],
      ["invalid:offset_minus_05_00", "2026-01-02T03:04:05.678-05:00"],
      ["invalid:offset_without_colon", "2026-01-02T03:04:05.678+0100"],
      ["invalid:no_zone", "2026-01-02T03:04:05.678"],
      ["invalid:lowercase_z", "2026-01-02T03:04:05.678z"],
      ["invalid:lowercase_t", "2026-01-02t03:04:05.678Z"],
      ["invalid:space_separator", "2026-01-02 03:04:05.678Z"],
      ["invalid:no_seconds", "2026-01-02T03:04Z"],
      ["invalid:comma_fraction", "2026-01-02T03:04:05,678Z"],
      ["invalid:feb_29_non_leap_2023", "2023-02-29T00:00:00.000Z"],
      ["invalid:feb_29_non_leap_1900", "1900-02-29T00:00:00.000Z"],
      ["invalid:feb_29_non_leap_2100", "2100-02-29T00:00:00.000Z"],
      ["invalid:feb_30", "2024-02-30T00:00:00.000Z"],
      ["invalid:month_13", "2026-13-02T03:04:05.678Z"],
      ["invalid:month_00", "2026-00-02T03:04:05.678Z"],
      ["invalid:day_00", "2026-01-00T03:04:05.678Z"],
      ["invalid:day_32", "2026-01-32T03:04:05.678Z"],
      ["invalid:april_31", "2026-04-31T00:00:00.000Z"],
      ["invalid:june_31", "2026-06-31T00:00:00.000Z"],
      ["invalid:september_31", "2026-09-31T00:00:00.000Z"],
      ["invalid:november_31", "2026-11-31T00:00:00.000Z"],
      ["invalid:hour_24", "2026-01-02T24:00:00.000Z"],
      ["invalid:minute_60", "2026-01-02T03:60:00.000Z"],
      ["invalid:second_60", "2026-12-31T23:59:60.000Z"],
      ["invalid:unpadded_month", "2026-1-02T03:04:05.678Z"],
      ["invalid:five_digit_year", "12026-01-02T03:04:05.678Z"],
      ["invalid:expanded_year", "+002026-01-02T03:04:05.678Z"],
      ["invalid:negative_year", "-0001-01-02T03:04:05.678Z"],
      ["invalid:leading_space", " 2026-01-02T03:04:05.678Z"],
      ["invalid:trailing_space", "2026-01-02T03:04:05.678Z "],
      ["invalid:trailing_newline", "2026-01-02T03:04:05.678Z\n"],
      ["invalid:arabic_indic_digits", "\u0662\u0660\u0662\u0666-01-02T03:04:05.678Z"],
      ["invalid:fullwidth_digits", "\uff12\uff10\uff12\uff16-01-02T03:04:05.678Z"],
      ["invalid:date_only", "2026-01-02"],
      ["invalid:empty", ""],
    ],
  ),
);

specs.set("CalendarDate", () =>
  stringListCases(
    [
      ["valid:ordinary", "2026-01-02"],
      ["valid:leap_day_2024", "2024-02-29"],
      ["valid:leap_day_2000", "2000-02-29"],
      ["valid:leap_day_0000", "0000-02-29"],
      ["valid:year_9999", "9999-12-31"],
      ["valid:january_31", "2026-01-31"],
      ["valid:april_30", "2026-04-30"],
    ],
    [
      ["invalid:feb_29_non_leap_2023", "2023-02-29"],
      ["invalid:feb_29_non_leap_1900", "1900-02-29"],
      ["invalid:feb_29_non_leap_2100", "2100-02-29"],
      ["invalid:feb_30", "2024-02-30"],
      ["invalid:month_13", "2026-13-01"],
      ["invalid:month_00", "2026-00-10"],
      ["invalid:day_00", "2026-01-00"],
      ["invalid:day_32", "2026-01-32"],
      ["invalid:april_31", "2026-04-31"],
      ["invalid:june_31", "2026-06-31"],
      ["invalid:september_31", "2026-09-31"],
      ["invalid:november_31", "2026-11-31"],
      ["invalid:unpadded", "2026-1-2"],
      ["invalid:compact", "20260102"],
      ["invalid:with_time", "2026-01-02T00:00:00.000Z"],
      ["invalid:slashes", "2026/01/02"],
      ["invalid:five_digit_year", "12026-01-02"],
      ["invalid:leading_space", " 2026-01-02"],
      ["invalid:trailing_newline", "2026-01-02\n"],
      ["invalid:arabic_indic_digits", "\u0662\u0660\u0662\u0666-01-02"],
      ["invalid:empty", ""],
    ],
  ),
);

const recordIdCases = (): CaseDef[] =>
  stringListCases(
    [
      ["valid:lowercase_v7", id(1)],
      ["valid:variant_9", "01890f3e-e7d3-7cc0-98c8-000000000001"],
      ["valid:variant_a", "01890f3e-e7d3-7cc0-a8c8-000000000001"],
      ["valid:variant_b", "01890f3e-e7d3-7cc0-b8c8-000000000001"],
      ["valid:max_random_bits", "ffffffff-ffff-7fff-bfff-ffffffffffff"],
    ],
    [
      ["invalid:uppercase_hex", "01890F3E-E7D3-7CC0-88C8-00000000000A"],
      ["invalid:uppercase_single_letter", "01890f3e-e7d3-7cc0-88c8-00000000000A"],
      ["invalid:version_4", "01890f3e-e7d3-4cc0-88c8-000000000001"],
      ["invalid:version_8", "01890f3e-e7d3-8cc0-88c8-000000000001"],
      ["invalid:variant_0", "01890f3e-e7d3-7cc0-08c8-000000000001"],
      ["invalid:variant_7", "01890f3e-e7d3-7cc0-78c8-000000000001"],
      ["invalid:variant_c", "01890f3e-e7d3-7cc0-c8c8-000000000001"],
      ["invalid:variant_f", "01890f3e-e7d3-7cc0-f8c8-000000000001"],
      ["invalid:nil", "00000000-0000-0000-0000-000000000000"],
      ["invalid:no_hyphens", "01890f3ee7d37cc088c8000000000001"],
      ["invalid:braces", "{01890f3e-e7d3-7cc0-88c8-000000000001}"],
      ["invalid:urn", "urn:uuid:01890f3e-e7d3-7cc0-88c8-000000000001"],
      ["invalid:short", "01890f3e-e7d3-7cc0-88c8-00000000001"],
      ["invalid:long", "01890f3e-e7d3-7cc0-88c8-0000000000001"],
      ["invalid:non_hex", "01890f3e-e7d3-7cc0-88c8-00000000000g"],
      ["invalid:trailing_newline", `${id(1)}\n`],
      ["invalid:leading_space", ` ${id(1)}`],
      ["invalid:empty", ""],
    ],
  );
specs.set("RecordId", recordIdCases);
specs.set("DatabaseId", recordIdCases);

specs.set("Sha256", () =>
  stringListCases(
    [
      ["valid:lowercase", SHA],
      ["valid:all_zero", "0".repeat(64)],
    ],
    [
      ["invalid:uppercase", SHA.toUpperCase()],
      ["invalid:63_chars", SHA.slice(0, 63)],
      ["invalid:65_chars", `${SHA}0`],
      ["invalid:non_hex", `${SHA.slice(0, 63)}g`],
      ["invalid:prefixed", `sha256:${SHA}`],
      ["invalid:trailing_newline", `${SHA}\n`],
      ["invalid:empty", ""],
    ],
  ),
);

specs.set("Semver", () =>
  stringListCases(
    [
      ["valid:plain", "1.0.0"],
      ["valid:zeros", "0.0.0"],
      ["valid:leading_zero_accepted", "01.02.03"],
      ["valid:prerelease", "1.0.0-alpha.1"],
      ["valid:build", "1.0.0+build.5"],
      ["valid:prerelease_and_build", "1.0.0-rc-1+sha.abc"],
      ["valid:large_numbers", "123456789012345678901.0.0"],
    ],
    [
      ["invalid:two_parts", "1.0"],
      ["invalid:four_parts", "1.0.0.0"],
      ["invalid:v_prefix", "v1.0.0"],
      ["invalid:empty_prerelease", "1.0.0-"],
      ["invalid:empty_build", "1.0.0+"],
      ["invalid:space_in_prerelease", "1.0.0-al pha"],
      ["invalid:non_ascii_prerelease", "1.0.0-\u03b1"],
      ["invalid:underscore_prerelease", "1.0.0-alpha_1"],
      ["invalid:arabic_indic_digits", "\u0661.0.0"],
      ["invalid:trailing_newline", "1.0.0\n"],
      ["invalid:empty", ""],
    ],
  ),
);

specs.set("PackageRelativePath", () =>
  stringListCases(
    [
      ["valid:single", "a"],
      ["valid:nested", "series/a.ath-series.json"],
      ["valid:dot_segment", "./a"],
      ["valid:inner_dot_segment", "a/./b"],
      ["valid:double_slash", "a//b"],
      ["valid:dotdot_prefix_name", "..a"],
      ["valid:dotdot_suffix_name", "a.."],
      ["valid:three_dots", "..."],
      ["valid:dotdot_inside_name", "a/..b/c"],
      ["valid:encoded_dotdot", "%2e%2e/a"],
      ["valid:drive_letter", "C:/x"],
      ["valid:space", " "],
      ["valid:unicode", "s\u00e9ries/\ud83d\ude00.json"],
    ],
    [
      ["invalid:absolute", "/a"],
      ["invalid:backslash", "a\\b"],
      ["invalid:windows_absolute", "C:\\x"],
      ["invalid:parent", ".."],
      ["invalid:parent_prefix", "../a"],
      ["invalid:parent_inner", "a/../b"],
      ["invalid:parent_suffix", "a/.."],
      ["invalid:newline", "a\nb"],
      ["invalid:carriage_return", "a\rb"],
      ["invalid:line_separator", "a\u2028b"],
      ["invalid:paragraph_separator", "a\u2029b"],
      ["invalid:empty", ""],
    ],
  ),
);

specs.set("ExtensionId", () =>
  stringListCases(
    [
      ["valid:two_part_owner", EXTENSION_KEY],
      ["valid:three_part_owner", "org.example.team/vectors"],
      ["valid:digits", "org2.example3/v1"],
      ["valid:name_punctuation", "org.example/vectors.v2_x-y"],
      ["valid:length_200", `org.example/${"a".repeat(188)}`],
    ],
    [
      ["invalid:no_dot_in_owner", "example/vectors"],
      ["invalid:uppercase", "Org.example/vectors"],
      ["invalid:one_char_name", "org.example/v"],
      ["invalid:second_slash", "org.example/demo/widget"],
      ["invalid:hyphen_in_owner", "org.ex-ample/vectors"],
      ["invalid:digit_first_owner", "1org.example/vectors"],
      ["invalid:digit_after_dot", "org.1example/vectors"],
      ["invalid:name_starts_with_digit", "org.example/1v"],
      ["invalid:name_second_char_punctuation", "org.example/v.x"],
      ["invalid:trailing_dot_owner", "org.example./vectors"],
      ["invalid:length_201", `org.example/${"a".repeat(189)}`],
      ["invalid:source_id", "source_id"],
      ["invalid:trailing_newline", `${EXTENSION_KEY}\n`],
      ["invalid:empty", ""],
    ],
  ),
);

specs.set("RecordRevisionNumber", () => [
  c("valid:1", 1),
  c("valid:2", 2),
  c("valid:max_safe_integer", Number.MAX_SAFE_INTEGER),
  c("valid:integer_valued_exponent", 1e15),
  c("invalid:0", 0),
  c("invalid:-1", -1),
  c("invalid:1.5", 1.5),
  c("invalid:2^53", 2 ** 53),
  c("invalid:1e21", 1e21),
  c("invalid:-2^53", -(2 ** 53)),
  c("invalid:5e-324", 5e-324),
  t("text:1.0", "1.0"),
  t("text:1E0", "1E0"),
  t("text:10e-1", "10e-1"),
  t("text:2.0000000000000001_rounds_to_2", "2.0000000000000001"),
  t("text:-0", "-0"),
  t("text:9007199254740993", "9007199254740993"),
  t("text:1e400", "1e400"),
  c("wrong_type:string", "1"),
  c("wrong_type:null", null),
  c("wrong_type:boolean", true),
  c("wrong_type:array", [1]),
  c("wrong_type:object", {}),
]);

// --- Enums -----------------------------------------------------------------

for (const name of [
  "RecordFormatVersion",
  "RecordType",
  "AuthoredRecordStatus",
  "MissingReason",
  "InputTimestampBasis",
  "UtcOffsetMeaning",
  "SourceTimeZoneMeaning",
  "ReceivedAtBasis",
  "ProviderReportingPeriodType",
  "ValueBasis",
  "ActivityTypeBasis",
  "SourceType",
  "PointMeasurementTypeSchema",
  "SeriesQuantitySchema",
  "AthleteReportType",
  "Confidence",
  "BecameKnownAtBasis",
  "ErasureReasonCode",
]) {
  specs.set(name, () => enumCases(target(name)));
}

// --- Small objects ---------------------------------------------------------

objectFile("EvidenceReference", { valid: [["minimal", evidence(7)]] });
objectFile("AthleteActor", { valid: [["minimal", athleteActor]] });
objectFile("AgentActor", {
  valid: [
    ["minimal", { author_type: "agent", agent: "reference-agent" }],
    ["maximal", agentActorMax],
  ],
  extras: [c("extra:model_not_allowed_on_actor", { ...agentActorMax, model: "example-model" })],
});
objectFile("ToolActor", { valid: [["minimal", toolActor]] });
objectFile("AgentAuthor", {
  valid: [
    ["minimal", { author_type: "agent", agent: "reference-agent", model: "example-model" }],
    ["maximal", agentAuthorMax],
  ],
});
objectFile("ToolAuthor", { valid: [["minimal", toolAuthor]] });
objectFile("AuthoredRecordMetadata", {
  valid: [
    ["minimal", authMin],
    ["maximal", authMax],
    ["tool_author", { ...authMin, author: toolAuthor }],
  ],
});
objectFile("FieldBasis", { valid: [["minimal", {}], ["maximal", fieldBasisMax]] });
objectFile("HeartRateSample", { valid: [["minimal", { offset_ms: 0, bpm: 120 }]] });
objectFile("StrengthSet", { valid: [["minimal", { reps: 5, weight: 60, weight_unit: "kg" }]] });
objectFile("RoutePoint", {
  valid: [
    ["minimal", routeMin],
    ["maximal", routeMax],
  ],
  extras: [
    c("bounds:north_pole", { ...routeMin, latitude_deg: 90, longitude_deg: 180 }),
    c("bounds:south_pole", { ...routeMin, latitude_deg: -90, longitude_deg: -180 }),
  ],
});
objectFile("PointMeasurementCalculation", {
  valid: [
    ["minimal", pmcMin],
    ["maximal", pmcMax],
  ],
});
objectFile("WorkoutSessionSegment", {
  valid: [
    ["minimal", segmentMin],
    ["maximal", segmentMax],
  ],
  extras: [
    c("no_rule:end_offset_before_start_offset", {
      ...segmentMin,
      start_offset_ms: 1000,
      end_offset_ms: 10,
    }),
  ],
});
objectFile("Score", {
  valid: [
    ["reps_only", { reps: 1 }],
    ["maximal", scoreMax],
    ["duration_only", { duration_s: 0.001 }],
    ["weight_only", { weight_kg: 1 }],
  ],
  extras: [
    c("rule:empty", {}, trig("R22")),
    c("rule:reps", { reps: 21 }, sat("R22")),
    c("rule:duration", { duration_s: 300 }, sat("R22")),
    c("rule:weight", { weight_kg: 100 }, sat("R22")),
    c("order:zero_reps_counts_as_present", { reps: 0 }, sat("R22")),
    c("order:fractional_reps_skips_refinement", { reps: 1.5 }, sat("R22")),
    c("order:null_duration_skips_refinement", { duration_s: null }, sat("R22")),
    c("order:unknown_key_still_runs_refinement", { seconds: 300 }, trig("R22")),
  ],
});
objectFile("BenchmarkResultSegment", {
  valid: [
    ["minimal", { label: "round 1" }],
    ["maximal", { label: "round 1", score: { reps: 21 } }],
  ],
  extras: [c("rule:empty_score", { label: "round 1", score: {} }, trig("R22"))],
});
objectFile("ReportedNutrient", { valid: [["minimal", nutrientMax]] });
objectFile("ReportedNutritionItem", {
  valid: [
    ["minimal", itemMin],
    ["maximal", itemMax],
  ],
});
objectFile("PredictionErrorCause", {
  valid: [["minimal", { evidence: evidence(9), explanation: "Slept longer than usual." }]],
});
objectFile("PlannedSessionBlock", {
  valid: [
    ["minimal", blockMin],
    ["maximal", blockMax],
  ],
});
objectFile("RecordDeletion", {
  valid: [
    ["minimal", deletion(id(13), 1)],
    ["tool_actor", { ...deletion(id(13), 2), deleted_by: toolActor }],
  ],
});
objectFile("RecordErasureMarker", {
  valid: [["minimal", { record_id: id(99), erased_at: T0, reason_code: "athlete_request" }]],
});
objectFile("RecordImportKey", {
  valid: [["minimal", importKey(id(7), "provider_record_id", "provider-0007")]],
});
const seriesFileReference: JsonObject = {
  file: `series/${id(3)}-heart_rate-000001.ath-series.json`,
  sha256: SHA,
  source_id: id(3),
  quantity: "heart_rate",
  start_at: T0,
  end_at: T1,
  sample_count: 1,
  revision_count: 1,
  deletion_count: 0,
  import_key_count: 0,
};
objectFile("MeasurementSeriesFileReference", {
  valid: [["minimal", seriesFileReference]],
  extras: [
    c("no_rule:end_before_start", { ...seriesFileReference, start_at: T1, end_at: T0 }),
    c("no_rule:counts_unchecked", { ...seriesFileReference, sample_count: 0, revision_count: 7 }),
  ],
});
const manifestFile: JsonObject = { file: "athlete.ath.json", sha256: SHA, byte_count: 2048 };
const manifestCount: JsonObject = {
  record_type: "athlete_profile",
  current_record_count: 1,
  revision_count: 2,
  deletion_count: 0,
  import_key_count: 0,
};
objectFile("ManifestFile", { valid: [["minimal", manifestFile]] });
objectFile("ManifestRecordCount", { valid: [["minimal", manifestCount]] });
objectFile("ExportManifest", {
  valid: [
    [
      "minimal",
      {
        athleticstandard_version: "2.1.0",
        source_database_id: id(1),
        database_revision: 0,
        files: [],
        record_counts: [],
        erasure_marker_count: 0,
      },
    ],
    [
      "maximal",
      {
        athleticstandard_version: "2.1.0",
        source_database_id: id(1),
        database_revision: 4,
        exported_at: T1,
        files: [manifestFile],
        record_counts: [manifestCount],
        erasure_marker_count: 1,
      },
    ],
  ],
});

// --- Input timestamps, reporting periods, source values ----------------------

objectFile("InputTimestampReceipt", {
  valid: [
    ["minimal", itrMin],
    ["maximal", itrMax],
    ["other_reasons", { timestamp_missing_reason: "not_exposed_by_route", utc_offset_missing_reason: "not_applicable" }],
  ],
  maxExempt: ["timestamp_missing_reason", "utc_offset_missing_reason"],
  extras: [
    c("rule:empty_object", {}, trig("R1", "R4")),
    c("rule:no_timestamp_no_reason", { utc_offset_missing_reason: "not_applicable" }, trig("R1")),
    c(
      "rule:timestamp_and_reason",
      { ...itrMax, timestamp_missing_reason: "not_applicable" },
      trig("R2"),
    ),
    c(
      "rule:timestamp_without_basis",
      { timestamp_text: "2026-01-02T03:04:05", utc_offset_missing_reason: "not_provided_by_source" },
      trig("R3"),
    ),
    c(
      "rule:basis_without_timestamp",
      { ...itrMin, timestamp_basis: "source_instant" },
      sat("R1", "R2", "R3"),
    ),
    c("rule:no_offset_no_reason", { timestamp_missing_reason: "not_applicable" }, trig("R4")),
    c(
      "rule:offset_and_reason",
      { ...itrMax, utc_offset_missing_reason: "not_applicable" },
      trig("R5"),
    ),
    c(
      "rule:offset_without_meaning",
      { timestamp_missing_reason: "not_applicable", utc_offset_text: "+01:00" },
      trig("R6"),
    ),
    c(
      "rule:meaning_without_offset",
      { ...itrMin, utc_offset_meaning: "meaning_not_documented" },
      trig("R7"),
    ),
    c(
      "rule:time_zone_without_meaning",
      { ...itrMin, source_time_zone: "America/Chicago" },
      trig("R8"),
    ),
    c(
      "rule:time_zone_meaning_without_zone",
      { ...itrMin, source_time_zone_meaning: "apple_health_object_creation_time_zone" },
      trig("R8"),
    ),
    c(
      "rule:time_zone_not_checked_as_iana",
      { ...itrMin, source_time_zone: "Not/AZone", source_time_zone_meaning: "apple_health_object_creation_time_zone" },
      sat("R8"),
    ),
    c("rule:all_satisfied_minimal", itrMin, sat("R1", "R2", "R3", "R4", "R5", "R6", "R7", "R8")),
    c("rule:all_satisfied_maximal", itrMax, sat("R1", "R2", "R3", "R4", "R5", "R6", "R7", "R8")),
    c(
      "rule:every_conflict",
      {
        timestamp_text: "x",
        timestamp_missing_reason: "not_applicable",
        utc_offset_text: "x",
        utc_offset_missing_reason: "not_applicable",
        source_time_zone: "UTC",
      },
      trig("R2", "R3", "R5", "R6", "R8"),
    ),
    c(
      "order:empty_text_still_runs_refinements",
      { timestamp_text: "", timestamp_missing_reason: "not_applicable", utc_offset_missing_reason: "not_applicable" },
      trig("R2", "R3"),
    ),
    c("order:wrong_type_skips_refinements", { timestamp_text: 1 }, sat("R1", "R3", "R4")),
    c("order:unknown_key_still_runs_refinements", { extra: 1 }, trig("R1", "R4")),
    c("order:bad_enum_skips_refinements", { timestamp_missing_reason: "x" }, sat("R4")),
  ],
});

objectFile("ProviderReportingPeriod", {
  valid: [
    ["minimal", prpMin],
    ["maximal", prpMax],
    ["offset_missing", { period_type: "oura_activity_day", provider_date: DAY, timezone_offset_missing_reason: "not_provided_by_source" }],
  ],
  maxExempt: ["timezone_offset_missing_reason"],
  extras: [
    c("rule:no_identifier", { period_type: "oura_sleep_day" }, trig("R9")),
    c("rule:id_only", { period_type: "whoop_physiological_cycle", provider_period_id: "c-1" }, sat("R9")),
    c("rule:date_only", prpMin, sat("R9")),
    c("rule:start_only", { period_type: "oura_day_unspecified", start_at: T0 }, sat("R9")),
    c("rule:end_only_is_not_an_identifier", { period_type: "oura_day_unspecified", end_at: T1 }, trig("R9")),
    c(
      "rule:invalid_start_counts_as_present",
      { period_type: "oura_day_unspecified", start_at: "2026-01-02T03:04:05Z" },
      sat("R9"),
    ),
    c("rule:end_before_start", { ...prpMin, start_at: T1, end_at: T0 }, trig("R10")),
    c("rule:end_one_ms_before_start", { ...prpMin, start_at: T0, end_at: T_BEFORE }, trig("R10")),
    c("rule:end_equals_start", { ...prpMin, start_at: T0, end_at: T0 }, sat("R10")),
    c("rule:end_without_start", { ...prpMin, end_at: T0 }, sat("R10")),
    c(
      "date_parse:unparseable_end_passes",
      { ...prpMin, start_at: T1, end_at: "not a date" },
      sat("R10"),
    ),
    c(
      "date_parse:date_only_end_before_start",
      { ...prpMin, start_at: T1, end_at: "2026-01-01" },
      trig("R10"),
    ),
    c("rule:offset_without_meaning", { ...prpMin, timezone_offset_text: "+09:00" }, trig("R11")),
    c(
      "rule:meaning_without_offset",
      { ...prpMin, timezone_offset_meaning: "route_rendering_offset" },
      sat("R11"),
    ),
    c(
      "rule:offset_and_missing_reason",
      {
        ...prpMin,
        timezone_offset_text: "+09:00",
        timezone_offset_meaning: "provider_cycle_offset",
        timezone_offset_missing_reason: "not_applicable",
      },
      trig("R12"),
    ),
    c(
      "rule:offset_neither_text_nor_reason",
      prpMin,
      sat("R11", "R12"),
    ),
    c(
      "rule:every_conflict",
      { period_type: "oura_calendar_day", end_at: T0, timezone_offset_text: "x", timezone_offset_missing_reason: "not_applicable" },
      trig("R9", "R11", "R12"),
    ),
    c(
      "order:nested_receipt_issue_still_runs_refinements",
      { period_type: "oura_sleep_day", input_start_timestamp: {} },
      trig("R1", "R4", "R9"),
    ),
  ],
});

objectFile("SourceValue", {
  valid: [
    ["minimal", svMin],
    ["maximal", svMax],
  ],
  maxExempt: ["input_value_missing_reason", "input_unit_missing_reason"],
  extras: [
    c("rule:input_value_and_reason", { ...svMax, input_value_missing_reason: "not_applicable" }, trig("R13")),
    c(
      "rule:input_value_neither",
      { standard_value: 1, standard_unit: "ms", input_unit: "ms" },
      trig("R13"),
    ),
    c("rule:input_unit_and_reason", { ...svMax, input_unit_missing_reason: "not_applicable" }, trig("R14")),
    c(
      "rule:input_unit_neither",
      { standard_value: 1, standard_unit: "ms", input_value: 1 },
      trig("R14"),
    ),
    c("rule:both_neither", { standard_value: 1, standard_unit: "ms" }, trig("R13", "R14")),
    c(
      "rule:mixed_value_present_unit_missing",
      { standard_value: 1, standard_unit: "ms", input_value: 0, input_unit_missing_reason: "not_exposed_by_route" },
      sat("R13", "R14"),
    ),
    c(
      "order:empty_input_unit_still_runs_refinements",
      { ...svMax, input_unit: "", input_unit_missing_reason: "not_applicable" },
      trig("R14"),
    ),
    c("no_rule:negative_standard_value", { ...svMin, standard_value: -3.25 }),
    c("no_rule:any_unit", { ...svMin, standard_unit: "furlongs" }),
  ],
});

objectFile("SourceReportedSummary", {
  valid: [
    ["minimal", srsMin],
    ["maximal", srsMax],
  ],
  maxExempt: ["input_value_missing_reason", "input_unit_missing_reason"],
  extras: [
    c("rule:both_neither", { standard_value: 1, standard_unit: "%", summary_type: "x" }, trig("R13", "R14")),
    c("rule:value_and_reason", { ...srsMax, input_value_missing_reason: "not_applicable" }, trig("R13")),
    c("rule:satisfied", srsMax, sat("R13", "R14")),
  ],
});

// --- Records with free-form JSON ---------------------------------------------

specs.set("SuppliedFields", () => [
  c("valid:empty", {}),
  c("valid:scalars", { s: "x", n: 1.5, b: false, z: null }),
  c("valid:nested", { outer: { inner: [1, { deep: [true, null, "y"] }] } }),
  c("valid:value_is_array", { list: [] }),
  c("valid:whitespace_key", { " ": 1 }),
  t("keys:integer_like_order", '{"b":1,"10":2,"2":3,"01":4,"-1":5,"1.5":6,"0":7}'),
  t("keys:array_index_limit", '{"z":0,"4294967295":1,"4294967294":2,"9007199254740993":3}'),
  t("keys:unicode", '{"\ufb01":1,"\ud83d\ude00":2,"\u00e9":3,"e\u0301":4,"a":5}'),
  t("keys:duplicate_last_wins", '{"a":1,"b":2,"a":3}'),
  t("keys:proto_object_value_dropped", '{"a":1,"__proto__":{"x":1},"b":2}'),
  t("keys:proto_scalar_value", '{"a":1,"__proto__":5}'),
  t("keys:constructor", '{"constructor":1,"toString":2}'),
  c("keys:empty_invalid", { "": 1 }),
  c("keys:empty_and_valid", { a: 1, "": 2 }),
  t("values:big_integer", '{"v":123456789012345678901}'),
  t("values:unsafe_integer", '{"v":9007199254740993}'),
  t("values:negative_zero", '{"v":-0}'),
  t("values:1e21", '{"v":1e21}'),
  t("values:1e-7", '{"v":1e-7}'),
  t("values:min_subnormal", '{"v":5e-324}'),
  t("values:max_double", '{"v":1.7976931348623157e308}'),
  t("values:integer_valued_float", '{"v":100.0}'),
  t("values:uppercase_exponent", '{"v":1E2}'),
  t("values:overflow_to_infinity", '{"v":1e400}'),
  t("values:lone_surrogate_string", '{"v":"\\ud800"}'),
  c("values:deep_nesting_64", { v: nestArrays(64, 1) }),
  c("root:wrong_type=array", []),
  c("root:wrong_type=string", "x"),
  c("root:wrong_type=null", null),
]);

specs.set("SleepStageMinutes", () => [
  c("valid:empty", {}),
  c("valid:stages", { awake: 5, light: 30.5, deep: 0, rem: 10 }),
  c("valid:free_stage_names", { "N3 (deep)": 15 }),
  t("valid:integer_like_keys", '{"rem":1,"3":2,"1":3}'),
  t("valid:negative_zero", '{"deep":-0}'),
  c("invalid:negative", { deep: -1 }),
  c("invalid:string_value", { deep: "1" }),
  c("invalid:null_value", { deep: null }),
  c("invalid:empty_key", { "": 1 }),
  t("invalid:infinity", '{"deep":1e400}'),
  c("root:wrong_type=array", []),
  c("root:wrong_type=null", null),
]);

// --- Unions of actors ----------------------------------------------------------

specs.set("RecordActor", () => [
  c("valid:athlete", athleteActor),
  c("valid:agent_minimal", { author_type: "agent", agent: "reference-agent" }),
  c("valid:agent_maximal", agentActorMax),
  c("valid:tool", toolActor),
  c("invalid:missing_discriminator", {}),
  c("invalid:unknown_discriminator", { author_type: "robot" }),
  c("invalid:discriminator_wrong_type", { author_type: 1 }),
  c("invalid:athlete_extra_key", { author_type: "athlete", agent: "x" }),
  c("invalid:agent_missing_agent", { author_type: "agent" }),
  c("invalid:agent_empty_agent", { author_type: "agent", agent: "" }),
  c("invalid:agent_with_model", { ...agentActorMax, model: "example-model" }),
  c("invalid:agent_bad_version", { ...agentActorMax, agent_version: "v2" }),
  c("invalid:tool_missing_version", { author_type: "tool", tool: "ath" }),
  c("invalid:tool_bad_version", { ...toolActor, tool_version: "2.1" }),
  c("invalid:tool_with_calculation_version", { ...toolActor, calculation_version: "1" }),
  c("root:wrong_type=null", null),
  c("root:wrong_type=array", []),
  c("root:wrong_type=string", "athlete"),
]);

specs.set("AuthoredRecordAuthor", () => [
  c("valid:athlete", athleteActor),
  c("valid:agent_minimal", { author_type: "agent", agent: "reference-agent", model: "example-model" }),
  c("valid:agent_maximal", agentAuthorMax),
  c("valid:tool", toolAuthor),
  c("invalid:missing_discriminator", {}),
  c("invalid:unknown_discriminator", { author_type: "robot" }),
  c("invalid:agent_missing_model", agentActorMax),
  c("invalid:agent_empty_model_version", { ...agentAuthorMax, model_version: "" }),
  c("invalid:tool_missing_calculation_version", toolActor),
  c("invalid:tool_empty_calculation_version", { ...toolAuthor, calculation_version: "" }),
  c("invalid:athlete_extra_key", { author_type: "athlete", model: "x" }),
  c("root:wrong_type=null", null),
  c("root:wrong_type=array", []),
]);

// --- Record data -----------------------------------------------------------------

const UNIT_CONFUSABLES: Record<string, string> = {
  ms: "s",
  bpm: "BPM",
  kg: "lb",
  brpm: "rpm",
  "ml/kg/min": "mL/kg/min",
  "%": "percent",
  "\u00b0C": "C",
  cm: "m",
  mmHg: "mmhg",
  count: "steps",
  kcal: "Cal",
  m: "km",
  "m/s": "km/h",
  W: "w",
  MET: "met",
  min: "minutes",
};

function confusableUnit(unit: string): string {
  return UNIT_CONFUSABLES[unit] ?? fail(`no confusable unit for ${unit}`);
}

/** Zones whose verdict does not depend on the ICU build. Others are observed only. */
const TIME_ZONE_TAGS: Record<string, RuleTag[]> = {
  UTC: sat("R15"),
  "America/Chicago": sat("R15"),
  "Not/AZone": trig("R15"),
  "Etc/Unknown": trig("R15"),
};

const twoTierExtras = (type: string): CaseDef[] =>
  ATHLETE_REPORTED.has(type)
    ? [c("two_tier:source_id_key_rejected", { ...dataFor(type).min, source_id: id(3) })]
    : [];

const dataExtras: Record<string, CaseDef[]> = {
  athlete_profile: [
    ...[
      "UTC",
      "utc",
      "Etc/UTC",
      "GMT",
      "UCT",
      "Zulu",
      "Z",
      "America/Chicago",
      "america/chicago",
      "AMERICA/CHICAGO",
      "EST5EDT",
      "EST",
      "Etc/GMT+5",
      "Etc/GMT-14",
      "Etc/GMT+13",
      "US/Eastern",
      "Asia/Calcutta",
      "Asia/Kolkata",
      "Europe/Kyiv",
      "Europe/Kiev",
      "America/Ciudad_Juarez",
      "America/Argentina/Buenos_Aires",
      "Antarctica/Troll",
      "Factory",
      "+05:00",
      "-08:00",
      "+0530",
      "+05",
      "+14:00",
      "+24:00",
      "Not/AZone",
      "Etc/Unknown",
      "local",
      " UTC",
      "UTC ",
      "America/Chicago\n",
    ].map((zone) =>
      c(
        `time_zone:${JSON.stringify(zone).slice(1, -1)}`,
        { default_time_zone: zone },
        TIME_ZONE_TAGS[zone],
      ),
    ),
    c("time_zone:empty", { default_time_zone: "" }),
  ],
  point_measurement: [
    ...Object.entries(Schema.POINT_MEASUREMENT_UNITS).flatMap(([quantity, unit]) => [
      c(
        `rule:unit:${quantity}:matches`,
        { ...dataFor("point_measurement").min, quantity, value: { ...svMin, standard_unit: unit } },
        sat("R16", "R17"),
      ),
      c(
        `rule:unit:${quantity}:wrong`,
        {
          ...dataFor("point_measurement").min,
          quantity,
          value: { ...svMin, standard_unit: confusableUnit(unit) },
        },
        trig("R16"),
      ),
    ]),
    c(
      "rule:unit:celsius_sign_character",
      { ...dataFor("point_measurement").min, quantity: "body_temperature", value: { ...svMin, standard_value: 36.6, standard_unit: "\u2103" } },
      trig("R16"),
    ),
    c(
      "rule:unit:trailing_space",
      { ...dataFor("point_measurement").min, value: { ...svMin, standard_unit: "ms " } },
      trig("R16"),
    ),
    c(
      "rule:positive:zero",
      { ...dataFor("point_measurement").min, value: { ...svMin, standard_value: 0 } },
      trig("R17"),
    ),
    t(
      "rule:positive:negative_zero",
      JSON.stringify({ ...dataFor("point_measurement").min, value: { ...svMin, standard_value: RAW_PLACEHOLDER } }).replace(
        JSON.stringify(RAW_PLACEHOLDER),
        "-0",
      ),
      trig("R17"),
    ),
    c(
      "rule:positive:negative",
      { ...dataFor("point_measurement").min, value: { ...svMin, standard_value: -1 } },
      trig("R17"),
    ),
    c(
      "rule:positive:smallest_positive",
      { ...dataFor("point_measurement").min, value: { ...svMin, standard_value: 5e-324 } },
      sat("R17"),
    ),
    c(
      "rule:positive:temperature_deviation_zero",
      { ...dataFor("point_measurement").min, quantity: "temperature_deviation", value: { ...svMin, standard_value: 0, standard_unit: "\u00b0C" } },
      sat("R17"),
    ),
    c(
      "rule:positive:temperature_deviation_negative",
      { ...dataFor("point_measurement").min, quantity: "temperature_deviation", value: { ...svMin, standard_value: -0.4, standard_unit: "\u00b0C" } },
      sat("R17"),
    ),
    c(
      "rule:unit_and_positive_together",
      { ...dataFor("point_measurement").min, quantity: "body_weight", value: { ...svMin, standard_value: 0, standard_unit: "lb" } },
      trig("R16", "R17"),
    ),
    c(
      "order:bad_quantity_skips_refinements",
      { ...dataFor("point_measurement").min, quantity: "hrv", value: { ...svMin, standard_value: 0, standard_unit: "x" } },
      sat("R16", "R17"),
    ),
    c(
      "order:nested_value_refinement_does_not_block",
      {
        ...dataFor("point_measurement").min,
        value: { standard_value: 0, standard_unit: "s" },
      },
      trig("R13", "R14", "R16", "R17"),
    ),
    c(
      "order:nested_type_error_skips_parent_refinement",
      {
        ...dataFor("point_measurement").min,
        value: { ...svMin, standard_value: 0, standard_unit: "s", input_value: "1" },
      },
      sat("R16", "R17"),
    ),
  ],
  measurement_sample: [
    ...Object.entries(Schema.SERIES_QUANTITY_UNITS).flatMap(([quantity, unit]) => [
      c(
        `rule:unit:${quantity}:matches`,
        { ...sampleDataMin, quantity, value: { ...svMin, standard_unit: unit } },
        sat("R18"),
      ),
      c(
        `rule:unit:${quantity}:wrong`,
        { ...sampleDataMin, quantity, value: { ...svMin, standard_unit: confusableUnit(unit) } },
        trig("R18"),
      ),
    ]),
    c("no_rule:negative_value", { ...sampleDataMin, value: { ...svMin, standard_value: -5, standard_unit: "bpm" } }),
    c("no_rule:zero_value", { ...sampleDataMin, value: { ...svMin, standard_value: 0, standard_unit: "bpm" } }),
    c(
      "rule:reconstructed:base_and_method",
      {
        ...sampleDataMin,
        input_timestamp: { ...itrMin, timestamp_basis: "reconstructed_from_series_start" },
        timestamp_base_text: "2026-01-02T03:00:00",
        timestamp_reconstruction_method: "start_plus_offset",
      },
      sat("R19"),
    ),
    c(
      "rule:reconstructed:missing_base",
      {
        ...sampleDataMin,
        input_timestamp: { ...itrMin, timestamp_basis: "reconstructed_from_series_start" },
        timestamp_reconstruction_method: "start_plus_offset",
      },
      trig("R19"),
    ),
    c(
      "rule:reconstructed:missing_method",
      {
        ...sampleDataMin,
        input_timestamp: { ...itrMin, timestamp_basis: "reconstructed_from_series_start" },
        timestamp_base_text: "2026-01-02T03:00:00",
      },
      trig("R19"),
    ),
    c(
      "rule:reconstructed:missing_both",
      { ...sampleDataMin, input_timestamp: { ...itrMin, timestamp_basis: "reconstructed_from_series_start" } },
      trig("R19"),
    ),
    c(
      "rule:not_reconstructed:both_claimed",
      {
        ...sampleDataMin,
        input_timestamp: itrMax,
        timestamp_base_text: "2026-01-02T03:00:00",
        timestamp_reconstruction_method: "start_plus_offset",
      },
      trig("R19"),
    ),
    c(
      "rule:not_reconstructed:base_only_allowed",
      { ...sampleDataMin, timestamp_base_text: "2026-01-02T03:00:00" },
      sat("R19"),
    ),
    c(
      "rule:not_reconstructed:method_only_allowed",
      { ...sampleDataMin, timestamp_reconstruction_method: "start_plus_offset" },
      sat("R19"),
    ),
    c("rule:not_reconstructed:neither", sampleDataMin, sat("R18", "R19")),
    c(
      "rule:unit_and_reconstruction_together",
      {
        ...sampleDataMin,
        value: { ...svMin, standard_unit: "ms" },
        input_timestamp: { ...itrMin, timestamp_basis: "reconstructed_from_series_start" },
      },
      trig("R18", "R19"),
    ),
  ],
  sleep_session: [
    c("rule:end_before_start", { ...dataFor("sleep_session").min, start_at: T1, end_at: T0 }, trig("R20")),
    c("rule:end_one_ms_before_start", { ...dataFor("sleep_session").min, start_at: T0, end_at: T_BEFORE }, trig("R20")),
    c("rule:end_equals_start", { ...dataFor("sleep_session").min, start_at: T0, end_at: T0 }, sat("R20")),
    c("rule:end_after_start", dataFor("sleep_session").min, sat("R20")),
    c(
      "order:min_length_issue_still_runs_refinement",
      { ...dataFor("sleep_session").min, provider_record_id: "", start_at: T1, end_at: T0 },
      trig("R20"),
    ),
    c(
      "order:invalid_end_format_still_runs_refinement",
      { ...dataFor("sleep_session").min, start_at: T1, end_at: "2026-01-02T03:04:05Z" },
      trig("R20"),
    ),
    c(
      "date_parse:unparseable_end",
      { ...dataFor("sleep_session").min, end_at: "not a date" },
      trig("R20"),
    ),
    c(
      "date_parse:unparseable_start",
      { ...dataFor("sleep_session").min, start_at: "not a date" },
      trig("R20"),
    ),
    c(
      "date_parse:bad_format_later_end",
      { ...dataFor("sleep_session").min, end_at: "2026-01-02T05:00:00Z" },
      sat("R20"),
    ),
    c(
      "date_parse:offset_end_compared_as_instant",
      { ...dataFor("sleep_session").min, end_at: "2026-01-02T04:00:00.000+02:00" },
      trig("R20"),
    ),
    c(
      "date_parse:date_only_end",
      { ...dataFor("sleep_session").min, end_at: "2026-01-02" },
      trig("R20"),
    ),
    c(
      "date_parse:wrong_type_end_skips_refinement",
      { ...dataFor("sleep_session").min, end_at: 1 },
      sat("R20"),
    ),
    c("no_rule:asleep_exceeds_in_bed", { ...dataFor("sleep_session").min, asleep_seconds: 9000, in_bed_seconds: 10 }),
  ],
  workout_session: [
    c("rule:end_before_start", { ...dataFor("workout_session").min, start_at: T1, end_at: T0 }, trig("R21")),
    c("rule:end_equals_start", { ...dataFor("workout_session").min, start_at: T0, end_at: T0 }, sat("R21")),
    c("rule:end_after_start", dataFor("workout_session").min, sat("R21")),
    c(
      "date_parse:unparseable_end",
      { ...dataFor("workout_session").min, end_at: "not a date" },
      trig("R21"),
    ),
    c(
      "no_rule:route_point_outside_session",
      { ...dataFor("workout_session").min, route: [{ ...routeMin, recorded_at: "2025-01-01T00:00:00.000Z" }] },
    ),
  ],
  benchmark_result: [
    c("rule:empty_score", { ...dataFor("benchmark_result").min, score: {} }, trig("R22")),
    c("rule:reps_score", { ...dataFor("benchmark_result").min, score: { reps: 1 } }, sat("R22")),
    c("rule:segment_empty_score", { ...dataFor("benchmark_result").min, segments: [{ label: "a", score: {} }] }, trig("R22")),
  ],
  provider_summary: [
    c(
      "rule:no_instant_no_period",
      withoutKey(dataFor("provider_summary").min, "recorded_at"),
      trig("R23"),
    ),
    c(
      "rule:empty_period_list",
      { ...withoutKey(dataFor("provider_summary").min, "recorded_at"), provider_reporting_periods: [] },
      trig("R23"),
    ),
    c(
      "rule:period_without_instant",
      { ...withoutKey(dataFor("provider_summary").min, "recorded_at"), provider_reporting_periods: [prpMin] },
      sat("R23"),
    ),
    c("rule:instant_without_period", dataFor("provider_summary").min, sat("R23")),
    c(
      "rule:invalid_period_counts_as_present",
      { ...withoutKey(dataFor("provider_summary").min, "recorded_at"), provider_reporting_periods: [{ period_type: "oura_sleep_day" }] },
      both(["R9"], ["R23"]),
    ),
  ],
  provider_score: [
    c(
      "rule:no_instant_no_period",
      withoutKey(dataFor("provider_score").min, "recorded_at"),
      trig("R24"),
    ),
    c(
      "rule:empty_period_list",
      { ...withoutKey(dataFor("provider_score").min, "recorded_at"), provider_reporting_periods: [] },
      trig("R24"),
    ),
    c(
      "rule:period_without_instant",
      { ...withoutKey(dataFor("provider_score").min, "recorded_at"), provider_reporting_periods: [prpMin] },
      sat("R24"),
    ),
    c("rule:instant_without_period", dataFor("provider_score").min, sat("R24")),
  ],
  athlete_report: [
    c("rule:rating_without_scale", { ...dataFor("athlete_report").min, rating: 3 }, trig("R25")),
    c("rule:rating_with_scale", { ...dataFor("athlete_report").min, rating: 3, rating_scale: "0-10" }, sat("R25")),
    c("rule:scale_without_rating", { ...dataFor("athlete_report").min, rating_scale: "0-10" }, sat("R25")),
    c("rule:negative_rating_allowed", { ...dataFor("athlete_report").min, rating: -2.5, rating_scale: "custom" }, sat("R25")),
    c(
      "order:empty_scale_still_runs_refinement",
      { ...dataFor("athlete_report").min, rating: 3, rating_scale: "" },
      sat("R25"),
    ),
  ],
  prediction: [
    c("no_rule:range_low_above_high", {
      ...dataFor("prediction").min,
      predicted_range: { low: { duration_s: 400 }, high: { duration_s: 200 } },
    }),
    c("rule:empty_predicted_score", { ...dataFor("prediction").min, predicted_score: {} }, trig("R22")),
    c(
      "rule:empty_range_scores",
      { ...dataFor("prediction").min, predicted_range: { low: {}, high: {} } },
      trig("R22"),
    ),
  ],
  prediction_error_analysis: [
    c(
      "rule:no_causes_not_unexplained",
      { ...dataFor("prediction_error_analysis").min, unexplained: false },
      trig("R26"),
    ),
    c("rule:no_causes_unexplained", dataFor("prediction_error_analysis").min, sat("R26")),
    c(
      "rule:causes_and_unexplained",
      { ...dataFor("prediction_error_analysis").max, unexplained: true },
      sat("R26"),
    ),
  ],
  planned_session: [
    c(
      "rule:no_date_no_start",
      withoutKey(dataFor("planned_session").min, "scheduled_date"),
      trig("R27"),
    ),
    c(
      "rule:start_only",
      { ...withoutKey(dataFor("planned_session").min, "scheduled_date"), scheduled_start_at: T0 },
      sat("R27"),
    ),
    c("rule:date_only", dataFor("planned_session").min, sat("R27")),
  ],
  benchmark_definition: [
    ...["Fran", "-fran", "_fran", "fran 2", "fr\u00e1n", "\ufb01"].map((key) =>
      c(`benchmark_key:${JSON.stringify(key).slice(1, -1)}`, { ...dataFor("benchmark_definition").min, benchmark_key: key }),
    ),
    ...["f", "0", "fran-2", "fran_2", "fran--"].map((key) =>
      c(`benchmark_key:${key}`, { ...dataFor("benchmark_definition").min, benchmark_key: key }),
    ),
  ],
  training_plan: [
    c("no_rule:end_before_start", { ...dataFor("training_plan").min, start_date: "2026-02-01", end_date: "2026-01-01" }),
  ],
  saved_training_review: [
    c("no_rule:end_before_start", { ...dataFor("saved_training_review").min, period_start: "2026-02-01", period_end: "2026-01-01" }),
  ],
  goal: [c("rule:empty_target_score", { ...dataFor("goal").min, target_score: {} }, trig("R22"))],
  prediction_grade: [
    c("rule:empty_actual_score", { ...dataFor("prediction_grade").min, actual_score: {} }, trig("R22")),
  ],
};

function withoutKey(value: JsonObject, key: string): JsonObject {
  const copy = { ...value };
  delete copy[key];
  return copy;
}

for (const type of RECORD_TYPES) {
  const data = dataFor(type);
  const name = `${pascal(type)}Data`;
  objectFile(name, {
    valid: [["minimal", data.min], ["maximal", data.max], ...(data.others ?? [])],
    extras: [...twoTierExtras(type), ...(dataExtras[type] ?? [])],
  });
}

// --- Current records and revisions ------------------------------------------------

const RECORD_ID = id(100);

function recordMin(type: string): JsonObject {
  return currentRecord(type, RECORD_ID, dataFor(type).min);
}

function recordMax(type: string): JsonObject {
  return {
    record_id: RECORD_ID,
    record_type: type,
    created_at: T0,
    current_revision: 3,
    is_deleted: true,
    extensions: extensionsMax,
    data: dataFor(type).max,
  };
}

function revisionMin(type: string): JsonObject {
  return recordRevision(type, RECORD_ID, 1, dataFor(type).min);
}

function revisionMax(type: string): JsonObject {
  return {
    record_id: RECORD_ID,
    record_type: type,
    revision: 2,
    previous_revision: 1,
    became_known_at: T0,
    became_known_at_basis: "package_merge",
    changed_by: agentActorMax,
    data: dataFor(type).max,
    extensions: extensionsMax,
  };
}

const otherType = (type: string): string => (type === "goal" ? "coaching_note" : "goal");

function extensionWallCases(base: JsonObject, type: string): CaseDef[] {
  const expect: Expect = ATHLETE_REPORTED.has(type) ? "trigger" : "satisfy";
  const tag: RuleTag[] = [["R29", expect]];
  return [
    c("two_tier:extension_nested_source_id", { ...base, extensions: { [EXTENSION_KEY]: { source_id: id(3) } } }, tag),
    c(
      "two_tier:extension_deep_source_id_in_array",
      { ...base, extensions: { [EXTENSION_KEY]: { items: [{ ok: 1 }, { nested: { source_id: 1 } }] } } },
      tag,
    ),
    c(
      "two_tier:extension_source_id_as_value_only",
      { ...base, extensions: { [EXTENSION_KEY]: { note: "source_id", list: ["source_id"] } } },
      sat("R29"),
    ),
    c(
      "two_tier:extension_source_id_as_extension_key",
      { ...base, extensions: { source_id: id(3) } },
      ATHLETE_REPORTED.has(type) ? trig("R29", "R30") : both(["R30"], ["R29"]),
    ),
    t(
      "two_tier:extension_source_id_hidden_under_proto",
      JSON.stringify({ ...base, extensions: RAW_PLACEHOLDER }).replace(
        JSON.stringify(RAW_PLACEHOLDER),
        `{"${EXTENSION_KEY}":{"__proto__":{"source_id":"${id(3)}"}}}`,
      ),
      sat("R29"),
    ),
    c("extensions:bad_key", { ...base, extensions: { "Bad Key": 1 } }, trig("R30")),
  ];
}

for (const type of RECORD_TYPES) {
  const name = `${pascal(type)}Record`;
  const minimal = recordMin(type);
  objectFile(name, {
    valid: [
      ["minimal", minimal],
      ["maximal", recordMax(type)],
    ],
    extras: [
      c("data:unknown_key", { ...minimal, data: { ...dataFor(type).min, unknown_data_key: 1 } }),
      c("data:other_record_type_data", { ...minimal, data: dataFor(otherType(type)).min }),
      c("record_type:other_valid_type", { ...minimal, record_type: otherType(type) }),
      ...extensionWallCases(minimal, type),
    ],
  });
}

for (const type of RECORD_TYPES) {
  const name = `${pascal(type)}Revision`;
  const minimal = revisionMin(type);
  const second = recordRevision(type, RECORD_ID, 2, dataFor(type).min);
  objectFile(name, {
    valid: [
      ["minimal", minimal],
      ["maximal", revisionMax(type)],
    ],
    extras: [
      c("rule:revision_1_previous_null", minimal, sat("R28")),
      c("rule:revision_2_previous_1", second, sat("R28")),
      c("rule:revision_1_previous_1", { ...minimal, previous_revision: 1 }, trig("R28")),
      c("rule:revision_2_previous_null", { ...second, previous_revision: null }, trig("R28")),
      c("rule:revision_2_previous_2", { ...second, previous_revision: 2 }, trig("R28")),
      c("rule:revision_3_previous_1", { ...second, revision: 3, previous_revision: 1 }, trig("R28")),
      c(
        "order:bad_previous_type_skips_chain_rule",
        { ...second, previous_revision: "1" },
        sat("R28"),
      ),
      c("data:unknown_key", { ...minimal, data: { ...dataFor(type).min, unknown_data_key: 1 } }),
      c("data:other_record_type_data", { ...minimal, data: dataFor(otherType(type)).min }),
      ...extensionWallCases(minimal, type),
    ],
  });
}

function unionRecordCases(build: (type: string) => JsonObject, includeSample: boolean): CaseDef[] {
  const cases: CaseDef[] = [];
  for (const type of RECORD_TYPES) {
    if (type === "measurement_sample" && !includeSample) continue;
    cases.push(c(`valid:${type}`, build(type)));
  }
  const goal = build("goal");
  if (!includeSample) cases.push(c("invalid:measurement_sample", build("measurement_sample")));
  cases.push(
    c("invalid:unknown_record_type", { ...goal, record_type: "workout" }),
    c("invalid:missing_record_type", withoutKey(goal, "record_type")),
    c("invalid:record_type_null", { ...goal, record_type: null }),
    c("invalid:record_type_number", { ...goal, record_type: 1 }),
    c("invalid:data_for_other_type", { ...goal, data: dataFor("sleep_session").min }),
    c("invalid:soft_issue_only", { ...goal, data: { ...dataFor("goal").min, title: "" } }),
    c("invalid:missing_data", withoutKey(goal, "data")),
    c("invalid:athlete_extension_source_id", {
      ...build("athlete_report"),
      extensions: { [EXTENSION_KEY]: { source_id: id(3) } },
    }, trig("R29")),
    c("root:wrong_type=null", null),
    c("root:wrong_type=array", []),
  );
  return cases;
}

specs.set("CurrentRecord", () => unionRecordCases(recordMin, true));
specs.set("NonSampleRecord", () => unionRecordCases(recordMin, false));
specs.set("RecordRevision", () => [
  ...unionRecordCases(revisionMin, true),
  c("rule:previous_revision_chain", { ...revisionMin("goal"), previous_revision: 1 }, trig("R28")),
]);
specs.set("NonSampleRecordRevision", () => [
  ...unionRecordCases(revisionMin, false),
  c("rule:previous_revision_chain", { ...revisionMin("goal"), previous_revision: 1 }, trig("R28")),
]);

// --- Extensions ----------------------------------------------------------------------

specs.set("RecordExtensions", () => {
  const lone = "\\ud800";
  return [
    c("valid:empty", {}),
    c("valid:one_scalar", { [EXTENSION_KEY]: 1 }),
    c("valid:every_json_type", { [EXTENSION_KEY]: { s: "x", n: -1.5, b: true, z: null, a: [], o: {} } }),
    c("key:valid", { "org.example/v1": 1 }, sat("R30")),
    c("key:no_dot", { "example/vectors": 1 }, trig("R30")),
    c("key:uppercase", { "Org.example/vectors": 1 }, trig("R30")),
    c("key:one_char_name", { "org.example/v": 1 }, trig("R30")),
    c("key:second_slash", { "org.example/demo/widget": 1 }, trig("R30")),
    c("key:length_200", { [`org.example/${"a".repeat(188)}`]: 1 }, sat("R30")),
    c("key:length_201", { [`org.example/${"a".repeat(189)}`]: 1 }, trig("R30")),
    c("key:empty", { "": 1 }, trig("R30")),
    c("key:two_bad_keys_in_order", { "b/x": 1, [EXTENSION_KEY]: 2, "a/y": 3 }, trig("R30")),
    t("key:proto_dropped", '{"__proto__":{"x":1},"org.example/vectors":1}'),
    c("count:32_keys", manyExtensionKeys(32), sat("R31")),
    c("count:33_keys", manyExtensionKeys(33), trig("R31")),
    c(
      "count:33_keys_with_bad_key",
      { ...manyExtensionKeys(32), "Bad/Key": 1 },
      trig("R30", "R31"),
    ),
    c(
      "count:33_keys_skips_later_checks",
      manyExtensionKeys(33, keyedObject(65)),
      both(["R31"], ["R37"]),
    ),
    c("depth:8_arrays_scalar_at_depth_8", { [EXTENSION_KEY]: nestArrays(7, 1) }, sat("R34")),
    c("depth:9_arrays_scalar_at_depth_9", { [EXTENSION_KEY]: nestArrays(8, 1) }, trig("R34")),
    c("depth:8_empty_arrays", { [EXTENSION_KEY]: nestArrays(7, []) }, sat("R34")),
    c("depth:9_empty_arrays", { [EXTENSION_KEY]: nestArrays(8, []) }, trig("R34")),
    c("depth:8_objects_scalar_at_depth_8", { [EXTENSION_KEY]: nestObjects(7, 1) }, sat("R34")),
    c("depth:9_objects_scalar_at_depth_9", { [EXTENSION_KEY]: nestObjects(8, 1) }, trig("R34")),
    c("depth:8_empty_objects", { [EXTENSION_KEY]: nestObjects(7, {}) }, sat("R34")),
    c("depth:9_empty_objects", { [EXTENSION_KEY]: nestObjects(8, {}) }, trig("R34")),
    c("depth:9_null_leaf", { [EXTENSION_KEY]: nestArrays(8, null) }, trig("R34")),
    c("depth:10_reports_once", { [EXTENSION_KEY]: nestArrays(9, 1) }, trig("R34")),
    c("string:4096_ascii", { [EXTENSION_KEY]: "a".repeat(4096) }, sat("R35")),
    c("string:4097_ascii", { [EXTENSION_KEY]: "a".repeat(4097) }, trig("R35")),
    c("string:4096_utf16_units_with_emoji", { [EXTENSION_KEY]: `${"a".repeat(4094)}\ud83d\ude00` }, sat("R35")),
    c("string:4097_utf16_units_with_emoji", { [EXTENSION_KEY]: `${"a".repeat(4095)}\ud83d\ude00` }, trig("R35")),
    c("string:2048_emoji_is_4096_units", { [EXTENSION_KEY]: "\ud83d\ude00".repeat(2048) }, sat("R35")),
    c("string:2049_emoji_is_4098_units_2049_code_points", { [EXTENSION_KEY]: "\ud83d\ude00".repeat(2049) }, trig("R35")),
    c("string:4097_code_points_with_emoji", { [EXTENSION_KEY]: `${"a".repeat(4096)}\ud83d\ude00` }, trig("R35")),
    t("string:4096_lone_surrogates", `{"${EXTENSION_KEY}":"${lone.repeat(4096)}"}`, sat("R35")),
    t("string:4097_lone_surrogates", `{"${EXTENSION_KEY}":"${lone.repeat(4097)}"}`, trig("R35")),
    c("string:4096_two_byte_chars", { [EXTENSION_KEY]: "\u00e9".repeat(4096) }, sat("R35")),
    c("string:4097_nested", { [EXTENSION_KEY]: { a: ["x", "a".repeat(4097)] } }, trig("R35")),
    c("array:256", { [EXTENSION_KEY]: Array(256).fill(0) }, sat("R36")),
    c("array:257", { [EXTENSION_KEY]: Array(257).fill(0) }, trig("R36")),
    c("array:257_nested", { [EXTENSION_KEY]: { list: Array(257).fill(0) } }, trig("R36")),
    c("object:64_keys", { [EXTENSION_KEY]: keyedObject(64) }, sat("R37")),
    c("object:65_keys", { [EXTENSION_KEY]: keyedObject(65) }, trig("R37")),
    c("object:65_keys_nested", { [EXTENSION_KEY]: { inner: keyedObject(65) } }, trig("R37")),
    c("bytes:total_65536", extensionsOfEncodedSize(65536), sat("R32", "R33")),
    c("bytes:total_65537", extensionsOfEncodedSize(65537), both(["R32"], ["R33"])),
    c(
      "bytes:total_over_with_two_byte_chars",
      manyExtensionKeys(5, Array(4).fill("\u00e9".repeat(2000))),
      both(["R32"], ["R33", "R35"]),
    ),
    c("bytes:value_16384", { [EXTENSION_KEY]: stringArrayOfEncodedSize(16384) }, sat("R33")),
    c("bytes:value_16385", { [EXTENSION_KEY]: stringArrayOfEncodedSize(16385) }, trig("R33")),
    c(
      "bytes:value_over_by_escaped_control_characters",
      { [EXTENSION_KEY]: ["\u0001".repeat(2731)] },
      both(["R33"], ["R35"]),
    ),
    t(
      "bytes:value_over_by_escaped_lone_surrogates",
      `{"${EXTENSION_KEY}":["${lone.repeat(2731)}"]}`,
      both(["R33"], ["R35"]),
    ),
    c(
      "bytes:value_with_raw_line_separators",
      { [EXTENSION_KEY]: ["\u2028".repeat(4096), "\u2028".repeat(1363)] },
      sat("R33"),
    ),
    c(
      "combined:many_issues_in_order",
      {
        "Bad/Key": nestArrays(9, "a".repeat(4097)),
        [EXTENSION_KEY]: { list: Array(257).fill(0), obj: keyedObject(65) },
      },
      trig("R30", "R34", "R36", "R37"),
    ),
    c("path:dot_in_extension_id_splits_path", { "org.example.team/vectors": { a: { b: "a".repeat(4097) } } }, trig("R35")),
    c("path:dot_in_nested_key_splits_path", { [EXTENSION_KEY]: { "x.y": "a".repeat(4097), "[z]": keyedObject(65) } }, trig("R35", "R37")),
    c("wrong_type:value_undefined_like", { [EXTENSION_KEY]: "undefined" }),
    t("wrong_type:infinity", `{"${EXTENSION_KEY}":1e400}`),
    c("root:wrong_type=array", []),
    c("root:wrong_type=null", null),
    c("root:wrong_type=string", "x"),
  ];
});

// --- Series files and packages ------------------------------------------------------

const PROFILE_ID = id(2);
const SOURCE_ID = id(3);

function sample(recordedAt: string, over: JsonObject = {}): JsonObject {
  return { ...sampleDataMin, recorded_at: recordedAt, ...over };
}

function sampleRecord(sequence: number, recordedAt: string, over: JsonObject = {}, dataOver: JsonObject = {}): JsonObject {
  return currentRecord("measurement_sample", id(200 + sequence), sample(recordedAt, dataOver), over);
}

function sampleRevision(sequence: number, revision: number, recordedAt: string, dataOver: JsonObject = {}): JsonObject {
  return recordRevision("measurement_sample", id(200 + sequence), revision, sample(recordedAt, dataOver));
}

function series(over: JsonObject): JsonObject {
  return {
    athleticstandard_version: "2.1.0",
    source_id: SOURCE_ID,
    quantity: "heart_rate",
    start_at: T0,
    end_at: T1,
    records: [],
    record_revisions: [],
    record_deletions: [],
    record_import_keys: [],
    ...over,
  };
}

const seriesOne = series({ records: [sampleRecord(1, T0)], record_revisions: [sampleRevision(1, 1, T0)] });
const seriesMax = series({
  records: [
    sampleRecord(1, T0, { current_revision: 2 }, { value: { ...svMin, standard_value: 62, standard_unit: "bpm" } }),
    sampleRecord(2, T1, { is_deleted: true }),
  ],
  record_revisions: [
    sampleRevision(1, 1, T0),
    sampleRevision(1, 2, T0, { value: { ...svMin, standard_value: 62, standard_unit: "bpm" } }),
    sampleRevision(2, 1, T1),
  ],
  record_deletions: [deletion(id(202), 1)],
  record_import_keys: [importKey(id(201), "provider_record_id", "hr-0001")],
});

objectFile("MeasurementSeriesFile", {
  valid: [
    ["minimal", series({})],
    ["maximal", seriesMax],
    ["one_sample", seriesOne],
  ],
  extras: [
    c("version:1.0.0_accepted", { ...seriesOne, athleticstandard_version: "1.0.0" }),
    c("version:2.1.1_rejected", { ...seriesOne, athleticstandard_version: "2.1.1" }),
    c("rule:end_before_start", series({ start_at: T1, end_at: T0 }), trig("R52")),
    c("rule:end_equals_start", series({ start_at: T0, end_at: T0 }), sat("R52")),
    c("date_parse:unparseable_end_passes", series({ end_at: "not a date" }), sat("R52")),
    c(
      "date_parse:unparseable_sample_time_passes_bounds",
      series({ records: [sampleRecord(1, "not a date")], record_revisions: [sampleRevision(1, 1, "not a date")] }),
      sat("R54"),
    ),
    c(
      "rule:sample_source_mismatch",
      series({
        records: [sampleRecord(1, T0, {}, { source_id: id(4) })],
        record_revisions: [recordRevision("measurement_sample", id(201), 1, sample(T0, { source_id: id(4) }))],
      }),
      trig("R53"),
    ),
    c(
      "rule:sample_quantity_mismatch",
      series({
        records: [sampleRecord(1, T0, {}, { quantity: "steps", value: { ...svMin, standard_unit: "count" } })],
        record_revisions: [
          recordRevision("measurement_sample", id(201), 1, sample(T0, { quantity: "steps", value: { ...svMin, standard_unit: "count" } })),
        ],
      }),
      trig("R53"),
    ),
    c("rule:sample_matches", seriesOne, sat("R53", "R54")),
    c(
      "rule:sample_before_start",
      series({ records: [sampleRecord(1, T_BEFORE)], record_revisions: [sampleRevision(1, 1, T_BEFORE)] }),
      trig("R54"),
    ),
    c(
      "rule:sample_after_end",
      series({ records: [sampleRecord(1, T_AFTER)], record_revisions: [sampleRevision(1, 1, T_AFTER)] }),
      trig("R54"),
    ),
    c(
      "rule:sample_at_end",
      series({ records: [sampleRecord(1, T1)], record_revisions: [sampleRevision(1, 1, T1)] }),
      sat("R54"),
    ),
    c(
      "rule:deleted_sample_outside_bounds",
      series({
        records: [sampleRecord(1, T_AFTER, { is_deleted: true })],
        record_revisions: [sampleRevision(1, 1, T_AFTER)],
        record_deletions: [deletion(id(201), 1)],
      }),
      trig("R54"),
    ),
    c(
      "rule:older_revision_outside_bounds_allowed",
      series({
        records: [sampleRecord(1, T0, { current_revision: 2 })],
        record_revisions: [sampleRevision(1, 1, T_AFTER), sampleRevision(1, 2, T0)],
      }),
      sat("R54"),
    ),
    c(
      "rule:mismatch_and_outside_together",
      series({
        records: [sampleRecord(1, T_AFTER, {}, { source_id: id(4) })],
        record_revisions: [recordRevision("measurement_sample", id(201), 1, sample(T_AFTER, { source_id: id(4) }))],
      }),
      trig("R53", "R54"),
    ),
    c(
      "history:duplicate_current_record",
      series({ records: [sampleRecord(1, T0), sampleRecord(1, T0)], record_revisions: [sampleRevision(1, 1, T0)] }),
      trig("R38"),
    ),
    c(
      "history:import_key_two_records",
      series({
        records: [sampleRecord(1, T0), sampleRecord(2, T0)],
        record_revisions: [sampleRevision(1, 1, T0), sampleRevision(2, 1, T0)],
        record_import_keys: [
          importKey(id(201), "provider_record_id", "same"),
          importKey(id(202), "provider_record_id", "same"),
        ],
      }),
      trig("R51"),
    ),
    c(
      "history:revision_without_record",
      series({ record_revisions: [sampleRevision(1, 1, T0)] }),
      trig("R40"),
    ),
    c(
      "structure:non_sample_record",
      series({ records: [recordMin("goal")], record_revisions: [revisionMin("goal")] }),
    ),
  ],
});

function pkg(over: JsonObject): JsonObject {
  return {
    athleticstandard_version: "2.1.0",
    database_id: id(1),
    database_revision: 1,
    records: [],
    record_revisions: [],
    record_deletions: [],
    record_erasure_markers: [],
    record_import_keys: [],
    measurement_series_files: [],
    ...over,
  };
}

const profile = (over: JsonObject = {}, data: JsonObject = {}): JsonObject =>
  currentRecord("athlete_profile", PROFILE_ID, data, over);
const profileRevision = (revision: number, data: JsonObject = {}, over: JsonObject = {}): JsonObject =>
  recordRevision("athlete_profile", PROFILE_ID, revision, data, over);
const named = { name: "Sample Athlete" };
const pkgOne = pkg({ records: [profile()], record_revisions: [profileRevision(1)] });
const sourceData = dataFor("source").min;
const pointData = dataFor("point_measurement").min;
const pkgMax = pkg({
  database_revision: 9,
  records: [
    profile({ current_revision: 2 }, named),
    currentRecord("source", SOURCE_ID, sourceData),
    currentRecord("point_measurement", id(7), pointData),
    currentRecord("athlete_report", id(13), dataFor("athlete_report").min, { is_deleted: true }),
  ],
  record_revisions: [
    profileRevision(1),
    profileRevision(2, named),
    recordRevision("source", SOURCE_ID, 1, sourceData),
    recordRevision("point_measurement", id(7), 1, pointData),
    recordRevision("athlete_report", id(13), 1, dataFor("athlete_report").min),
  ],
  record_deletions: [deletion(id(13), 1)],
  record_erasure_markers: [{ record_id: id(99), erased_at: T1, reason_code: "recorded_in_error" }],
  record_import_keys: [importKey(id(7), "provider_record_id", "hrv-0007")],
  measurement_series_files: [seriesFileReference],
});

const ALL_HISTORY = [
  "R38", "R39", "R40", "R41", "R42", "R43", "R44", "R45", "R46", "R47", "R48", "R49", "R50", "R51", "R55", "R56",
];

function textWithRaw(value: JsonObject, replacements: string[]): string {
  let text = JSON.stringify(value);
  for (const replacement of replacements) {
    text = text.replace(JSON.stringify(RAW_PLACEHOLDER), replacement);
  }
  return text;
}

objectFile("AthleticStandardPackage", {
  valid: [
    ["minimal", pkg({ database_revision: 0 })],
    ["maximal", pkgMax],
    ["one_record", pkgOne],
  ],
  extras: [
    c("history:all_satisfied", pkgMax, sat(...ALL_HISTORY)),
    c("version:1.0.0_accepted", { ...pkgOne, athleticstandard_version: "1.0.0" }),
    c("version:2.0.0_accepted", { ...pkgOne, athleticstandard_version: "2.0.0" }),
    c(
      "history:duplicate_current_record",
      pkg({ records: [profile(), profile()], record_revisions: [profileRevision(1)] }),
      trig("R38"),
    ),
    c(
      "history:duplicate_revision",
      pkg({ records: [profile()], record_revisions: [profileRevision(1), profileRevision(1)] }),
      trig("R39"),
    ),
    c(
      "history:revision_without_current_record",
      pkg({
        records: [profile()],
        record_revisions: [profileRevision(1), recordRevision("goal", id(50), 1, dataFor("goal").min)],
      }),
      trig("R40"),
    ),
    c(
      "history:revision_changes_record_type",
      pkg({
        records: [profile({ current_revision: 2 })],
        record_revisions: [profileRevision(1), recordRevision("goal", PROFILE_ID, 2, dataFor("goal").min)],
      }),
      trig("R41"),
    ),
    c(
      "history:deletion_without_current_record",
      pkg({ ...pkgOne, record_deletions: [deletion(id(50), 1)] }),
      trig("R42"),
    ),
    c(
      "history:two_deletions_for_one_record",
      pkg({
        records: [profile({ is_deleted: true })],
        record_revisions: [profileRevision(1)],
        record_deletions: [deletion(PROFILE_ID, 1), deletion(PROFILE_ID, 1)],
      }),
      trig("R43"),
    ),
    c(
      "history:missing_revision",
      pkg({ records: [profile({ current_revision: 2 })], record_revisions: [profileRevision(1)] }),
      trig("R44"),
    ),
    c(
      "history:no_revisions_at_all",
      pkg({ records: [profile()] }),
      trig("R44"),
    ),
    c(
      "history:revision_gap",
      pkg({
        records: [profile({ current_revision: 2 }, named)],
        record_revisions: [profileRevision(1), profileRevision(3, named)],
      }),
      trig("R45"),
    ),
    c(
      "history:revisions_listed_out_of_order",
      pkg({
        records: [profile({ current_revision: 2 }, named)],
        record_revisions: [profileRevision(2, named), profileRevision(1)],
      }),
      sat("R44", "R45", "R46"),
    ),
    c(
      "history:current_data_differs",
      pkg({ records: [profile({}, { name: "A" })], record_revisions: [profileRevision(1, { name: "B" })] }),
      trig("R46"),
    ),
    c(
      "history:current_data_key_order_differs",
      pkg({
        records: [currentRecord("source", SOURCE_ID, { ...sourceData, supplied_fields: { a: 1, b: 2 } })],
        record_revisions: [
          recordRevision("source", SOURCE_ID, 1, { ...sourceData, supplied_fields: { b: 2, a: 1 } }),
        ],
      }),
      trig("R46"),
    ),
    t(
      "history:integer_like_key_order_normalized",
      textWithRaw(
        pkg({
          records: [currentRecord("source", SOURCE_ID, { ...sourceData, supplied_fields: RAW_PLACEHOLDER })],
          record_revisions: [recordRevision("source", SOURCE_ID, 1, { ...sourceData, supplied_fields: RAW_PLACEHOLDER })],
        }),
        ['{"10":1,"2":2}', '{"2":2,"10":1}'],
      ),
      sat("R46"),
    ),
    t(
      "history:number_spelling_normalized",
      textWithRaw(
        pkg({
          records: [profile({}, { birth_year: RAW_PLACEHOLDER })],
          record_revisions: [profileRevision(1, { birth_year: RAW_PLACEHOLDER })],
        }),
        ["1990", "1.99e3"],
      ),
      sat("R46"),
    ),
    c(
      "history:shape_key_order_normalized",
      pkg({
        records: [profile({}, { sex: "female", name: "A" })],
        record_revisions: [profileRevision(1, { name: "A", sex: "female" })],
      }),
      sat("R46"),
    ),
    c(
      "history:extensions_not_compared",
      pkg({
        records: [profile({ extensions: { [EXTENSION_KEY]: 1 } })],
        record_revisions: [profileRevision(1, {}, { extensions: { [EXTENSION_KEY]: 2 } })],
      }),
      sat("R46"),
    ),
    c(
      "history:deleted_without_deletion",
      pkg({ records: [profile({ is_deleted: true })], record_revisions: [profileRevision(1)] }),
      trig("R47"),
    ),
    c(
      "history:deletion_without_is_deleted",
      pkg({ ...pkgOne, record_deletions: [deletion(PROFILE_ID, 1)] }),
      trig("R48"),
    ),
    c(
      "history:deletion_names_old_revision",
      pkg({
        records: [profile({ current_revision: 2, is_deleted: true }, named)],
        record_revisions: [profileRevision(1), profileRevision(2, named)],
        record_deletions: [deletion(PROFILE_ID, 1)],
      }),
      trig("R49"),
    ),
    c(
      "history:import_key_without_record",
      pkg({ ...pkgOne, record_import_keys: [importKey(id(50), "provider_record_id", "x")] }),
      trig("R50"),
    ),
    c(
      "history:import_key_two_records",
      pkg({
        records: [profile(), currentRecord("source", SOURCE_ID, sourceData)],
        record_revisions: [profileRevision(1), recordRevision("source", SOURCE_ID, 1, sourceData)],
        record_import_keys: [
          importKey(PROFILE_ID, "provider_record_id", "same"),
          importKey(SOURCE_ID, "provider_record_id", "same"),
        ],
      }),
      trig("R51"),
    ),
    c(
      "history:import_key_repeated_for_same_record",
      pkg({
        ...pkgOne,
        record_import_keys: [
          importKey(PROFILE_ID, "provider_record_id", "same"),
          importKey(PROFILE_ID, "provider_record_id", "same"),
        ],
      }),
      sat("R50", "R51"),
    ),
    c(
      "history:import_key_same_text_other_method",
      pkg({
        records: [profile(), currentRecord("source", SOURCE_ID, sourceData)],
        record_revisions: [profileRevision(1), recordRevision("source", SOURCE_ID, 1, sourceData)],
        record_import_keys: [
          importKey(PROFILE_ID, "method_a", "same"),
          importKey(SOURCE_ID, "method_b", "same"),
        ],
      }),
      sat("R51"),
    ),
    c(
      "history:import_key_nul_separator_collision",
      pkg({
        records: [profile(), currentRecord("source", SOURCE_ID, sourceData)],
        record_revisions: [profileRevision(1), recordRevision("source", SOURCE_ID, 1, sourceData)],
        record_import_keys: [
          importKey(PROFILE_ID, "a\u0000b", "c"),
          importKey(SOURCE_ID, "a", "b\u0000c"),
        ],
      }),
      trig("R51"),
    ),
    c(
      "package:duplicate_series_file",
      pkg({ measurement_series_files: [seriesFileReference, { ...seriesFileReference, sample_count: 2 }] }),
      trig("R55"),
    ),
    c(
      "package:two_series_files",
      pkg({
        measurement_series_files: [
          seriesFileReference,
          { ...seriesFileReference, file: `series/${id(3)}-heart_rate-000002.ath-series.json` },
        ],
      }),
      sat("R55"),
    ),
    c(
      "package:series_file_path_case_differs",
      pkg({
        measurement_series_files: [seriesFileReference, { ...seriesFileReference, file: seriesFileReference.file!.toString().toUpperCase() }],
      }),
      sat("R55"),
    ),
    c(
      "package:erasure_marker_matches_current_record",
      pkg({
        ...pkgOne,
        record_erasure_markers: [{ record_id: PROFILE_ID, erased_at: T1, reason_code: "athlete_request" }],
      }),
      trig("R56"),
    ),
    c(
      "package:erasure_marker_for_absent_record",
      pkg({
        ...pkgOne,
        record_erasure_markers: [{ record_id: id(99), erased_at: T1, reason_code: "other" }],
      }),
      sat("R56"),
    ),
    c(
      "package:duplicate_erasure_markers_allowed",
      pkg({
        record_erasure_markers: [
          { record_id: id(99), erased_at: T1, reason_code: "other" },
          { record_id: id(99), erased_at: T0, reason_code: "legal_requirement" },
        ],
      }),
    ),
    c(
      "order:structural_error_skips_history",
      pkg({
        records: [profile(), { ...profile(), record_type: "nope" }],
        record_revisions: [profileRevision(1)],
      }),
      sat("R38"),
    ),
    c(
      "order:soft_error_still_runs_history",
      pkg({
        database_revision: -1,
        records: [profile(), profile()],
        record_revisions: [profileRevision(1)],
      }),
      trig("R38"),
    ),
    c(
      "structure:measurement_sample_in_package_records",
      pkg({ records: [sampleRecord(1, T0)], record_revisions: [sampleRevision(1, 1, T0)] }),
    ),
    c(
      "history:every_issue_in_order",
      pkg({
        records: [
          profile({ current_revision: 2, is_deleted: true }, named),
          profile(),
          currentRecord("source", SOURCE_ID, sourceData, { current_revision: 3 }),
        ],
        record_revisions: [
          profileRevision(1),
          profileRevision(1),
          recordRevision("goal", PROFILE_ID, 3, dataFor("goal").min),
          recordRevision("goal", id(50), 1, dataFor("goal").min),
          recordRevision("source", SOURCE_ID, 1, sourceData),
        ],
        record_deletions: [deletion(PROFILE_ID, 1), deletion(PROFILE_ID, 1), deletion(id(51), 1), deletion(SOURCE_ID, 1)],
        record_erasure_markers: [{ record_id: SOURCE_ID, erased_at: T1, reason_code: "other" }],
        record_import_keys: [
          importKey(id(52), "m", "k"),
          importKey(PROFILE_ID, "m", "k"),
        ],
        measurement_series_files: [seriesFileReference, seriesFileReference],
      }),
      trig("R38", "R39", "R40", "R41", "R42", "R43", "R44", "R45", "R46", "R48", "R49", "R50", "R51", "R55", "R56"),
    ),
  ],
});

// ---------------------------------------------------------------------------
// Running cases
// ---------------------------------------------------------------------------

interface IssueOut {
  path: Array<string | number>;
  code: string;
  message: string;
  [key: string]: unknown;
}

function normalizeIssue(issue: Record<string, unknown>): IssueOut {
  const out: IssueOut = {
    path: (issue.path as Array<string | number>) ?? [],
    code: String(issue.code),
    message: String(issue.message),
  };
  const rest = Object.keys(issue)
    .filter((key) => !["path", "code", "message", "input", "inst", "continue"].includes(key))
    .sort();
  for (const key of rest) {
    const value = issue[key];
    if (key === "errors" && Array.isArray(value)) {
      out.errors = value.map((branch: Array<Record<string, unknown>>) => branch.map(normalizeIssue));
    } else if (key === "issues" && Array.isArray(value)) {
      out.issues = (value as Array<Record<string, unknown>>).map(normalizeIssue);
    } else if (value instanceof RegExp) {
      out[key] = value.toString();
    } else {
      out[key] = value;
    }
  }
  return out;
}

function allMessages(issues: IssueOut[]): string[] {
  const messages: string[] = [];
  for (const issue of issues) {
    messages.push(issue.message);
    if (Array.isArray(issue.errors)) {
      for (const branch of issue.errors as IssueOut[][]) messages.push(...allMessages(branch));
    }
    if (Array.isArray(issue.issues)) messages.push(...allMessages(issue.issues as IssueOut[]));
  }
  return messages;
}

function wellFormed(value: unknown): boolean {
  if (typeof value === "string") return value.isWellFormed();
  if (Array.isArray(value)) return value.every(wellFormed);
  if (value !== null && typeof value === "object") {
    return Object.entries(value).every(([key, child]) => key.isWellFormed() && wellFormed(child));
  }
  return true;
}

const ruleIndex = new Map<string, { trigger: string[]; satisfy: string[] }>();

function runCase(schemaName: string, schema: z.ZodType, definition: CaseDef): Record<string, unknown> {
  if (definition.text === undefined && definition.input === undefined) {
    fail(`${schemaName}/${definition.name}: no input`);
  }
  const text = definition.text;
  const value: unknown =
    text !== undefined ? JSON.parse(text) : JSON.parse(JSON.stringify(definition.input));
  const result = schema.safeParse(value);
  const issues = result.success
    ? []
    : result.error.issues.map((issue) => normalizeIssue(issue as unknown as Record<string, unknown>));
  const out: Record<string, unknown> = { name: definition.name };
  if (text !== undefined) out.input_text = text;
  if (wellFormed(value)) out.input = value;
  else if (text === undefined) out.input_text = JSON.stringify(value);
  out.valid = result.success;
  if (result.success) {
    const output = JSON.parse(JSON.stringify(result.data)) as unknown;
    if (wellFormed(output)) out.output = output;
    else out.output_text = JSON.stringify(result.data);
  }
  out.issues = issues;
  if (definition.rules !== undefined) {
    const messages = allMessages(issues);
    for (const [rule, expect] of definition.rules) {
      const pattern = RULE_MESSAGES[rule] ?? fail(`unknown rule ${rule}`);
      const matched = messages.some((message) =>
        typeof pattern === "string" ? message === pattern : pattern.test(message),
      );
      if ((expect === "trigger") !== matched) {
        fail(
          `${schemaName}/${definition.name}: expected ${rule} to ${expect}; issues: ${JSON.stringify(issues).slice(0, 2000)}`,
        );
      }
      const entry = ruleIndex.get(rule) ?? { trigger: [], satisfy: [] };
      entry[expect].push(`${schemaName}#${definition.name}`);
      ruleIndex.set(rule, entry);
    }
    out.rules = definition.rules.map(([rule, expect]) => ({ rule, expect }));
  }
  return out;
}

function writeJson(path: string, value: unknown): void {
  const text = `${JSON.stringify(value, null, 2)}\n`;
  if (!text.isWellFormed()) fail(`${path}: text is not well-formed Unicode`);
  if (/\\u[dD][89abAB][0-9a-fA-F]{2}/.test(text.replace(/\\\\/g, ""))) {
    fail(`${path}: contains a surrogate escape outside a raw-text field`);
  }
  writeFileSync(path, text);
}

// ---------------------------------------------------------------------------
// JSON serialization vectors
// ---------------------------------------------------------------------------

const serializationTexts: Array<[string, string]> = [
  ["object:integer_like_keys", '{"b":1,"10":2,"2":3,"01":4,"-1":5,"1.5":6,"0":7}'],
  ["object:array_index_limit_keys", '{"z":0,"4294967295":1,"4294967294":2,"9007199254740993":3,"a":4}'],
  ["object:utf16_vs_code_point_order", '{"\ufb01":1,"\ud83d\ude00":2,"a":3,"\u00e9":4,"Z":5,"\uffff":6}'],
  ["object:escaped_key_order", '{"\\ufb01":1,"\\ud83d\\ude00":2,"\\uffff":3}'],
  ["object:case_and_punctuation_order", '{"b":1,"B":2,"a":3,"A":4,"_":5,"~":6," ":7}'],
  ["object:combining_vs_precomposed", '{"e\u0301":1,"\u00e9":2}'],
  ["object:empty_key", '{"":1,"a":2}'],
  ["object:duplicate_keys", '{"a":1,"b":2,"a":3}'],
  ["object:duplicate_keys_nested", '{"a":{"x":1},"b":0,"a":{"y":2}}'],
  ["object:proto_key", '{"__proto__":{"x":1},"a":1}'],
  ["object:nested_empty", '{"a":{},"b":[],"c":[{}],"d":[[]],"e":{"f":{}}}'],
  ["object:whitespace", ' \t\n\r{ "a" : [ 1 , 2 ] ,\n "b" : { } }\r\n'],
  ["string:control_characters", '["\\u0000\\u0001\\u001f","\\b\\f\\n\\r\\t","\\u000b"]'],
  ["string:del_and_c1", '["\\u007f","\\u0080","\\u009f"]'],
  ["string:line_and_paragraph_separators", '["\\u2028","\\u2029","\u2028"]'],
  ["string:no_break_space", '["\\u00a0","\u00a0"]'],
  ["string:escaped_slash", '["\\/","a\\/b"]'],
  ["string:quote_and_backslash", '["\\"","\\\\"]'],
  ["string:escaped_letters", '["\\u0041\\u00e9\\u00E9"]'],
  ["string:raw_non_ascii", '["\u00e9\u4e2d\ud83d\ude00"]'],
  ["string:surrogate_pair_escape", '["\\ud83d\\ude00"]'],
  ["string:lone_high_surrogate", '["\\ud800"]'],
  ["string:lone_low_surrogate", '["\\udc00"]'],
  ["string:reversed_surrogates", '["\\ude00\\ud83d"]'],
  ["string:lone_surrogate_key", '{"\\ud800":1,"a":2,"\\udc00":3}'],
  ["string:noncharacter", '["\\uffff","\\ufffe"]'],
  ["number:-0", "-0"],
  ["number:-0.0", "-0.0"],
  ["number:0.1", "0.1"],
  ["number:0.30000000000000004", "0.30000000000000004"],
  ["number:1e21", "1e21"],
  ["number:1e20", "1e20"],
  ["number:123456789012345678901", "123456789012345678901"],
  ["number:1e-7", "1e-7"],
  ["number:1e-6", "1e-6"],
  ["number:1.5e-7", "1.5e-7"],
  ["number:-1e-7", "-1e-7"],
  ["number:123e-20", "123e-20"],
  ["number:5e-324", "5e-324"],
  ["number:2.4703282292062328e-324_rounds_up", "2.4703282292062328e-324"],
  ["number:1e-400_underflows", "1e-400"],
  ["number:1.7976931348623157e308", "1.7976931348623157e308"],
  ["number:1e400_overflows", "1e400"],
  ["number:-1e400_overflows", "-1e400"],
  ["number:100.0", "100.0"],
  ["number:1E2", "1E2"],
  ["number:1e+2", "1e+2"],
  ["number:0.1e1", "0.1e1"],
  ["number:9007199254740993", "9007199254740993"],
  ["number:9007199254740992.5", "9007199254740992.5"],
  ["number:4.35", "4.35"],
  ["number:1.0000000000000002", "1.0000000000000002"],
  ["number:in_array", "[-0,0.1,1e21,1e-7,123456789012345678901,5e-324,1.7976931348623157e308,100.0,1E2]"],
  ["literal:true_false_null", "[true,false,null]"],
  ["scalar:string", '"x"'],
  ["nesting:depth_64", `${"[".repeat(64)}1${"]".repeat(64)}`],
];

const invalidJsonTexts: Array<[string, string]> = [
  ["invalid:nan", "NaN"],
  ["invalid:infinity", "Infinity"],
  ["invalid:trailing_comma_array", "[1,]"],
  ["invalid:trailing_comma_object", '{"a":1,}'],
  ["invalid:leading_zero", "01"],
  ["invalid:trailing_dot", "1."],
  ["invalid:leading_dot", ".5"],
  ["invalid:plus_sign", "+1"],
  ["invalid:single_quotes", "'a'"],
  ["invalid:bad_escape", '"\\x"'],
  ["invalid:short_unicode_escape", '"\\u12"'],
  ["invalid:raw_tab_in_string", '"a\tb"'],
  ["invalid:raw_newline_in_string", '"a\nb"'],
  ["invalid:byte_order_mark", "\ufeff{}"],
  ["invalid:empty", ""],
  ["invalid:whitespace_only", " "],
  ["invalid:two_values", "[1] [2]"],
  ["invalid:bare_minus", "-"],
  ["invalid:bare_exponent", "1e"],
  ["invalid:unquoted_key", "{a:1}"],
  ["invalid:comment", "[1/*x*/]"],
  ["invalid:non_breaking_space_whitespace", "[\u00a01]"],
];

function serializationVectors(): unknown {
  const cases: unknown[] = [];
  for (const [name, text] of serializationTexts) {
    const parsed = JSON.parse(text) as unknown;
    cases.push({
      name,
      input_text: text,
      parses: true,
      compact: JSON.stringify(parsed),
      stable_compact: JSON.stringify(stableValue(parsed)),
      pretty: JSON.stringify(stableValue(parsed), null, 2),
    });
  }
  for (const [name, text] of invalidJsonTexts) {
    let parses = true;
    try {
      JSON.parse(text);
    } catch {
      parses = false;
    }
    if (parses) fail(`${name} unexpectedly parses`);
    cases.push({ name, input_text: text, parses: false });
  }
  return {
    description:
      "JavaScript JSON behavior. compact = JSON.stringify(JSON.parse(input_text)); " +
      "stable_compact = JSON.stringify(stableValue(JSON.parse(input_text))); " +
      "pretty = JSON.stringify(stableValue(JSON.parse(input_text)), null, 2). " +
      "stableValue is src/db/stable-json.ts. deterministicJson writes pretty plus one newline.",
    cases,
  };
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main(): void {
  const schemaNames = Object.keys(exported)
    .filter((name) => exported[name] instanceof z.ZodType)
    .sort();
  const missing = schemaNames.filter((name) => !specs.has(name));
  if (missing.length > 0) fail(`no vectors for exported schemas: ${missing.join(", ")}`);
  const extra = [...specs.keys()].filter((name) => !schemaNames.includes(name));
  if (extra.length > 0) fail(`vectors for unknown schemas: ${extra.join(", ")}`);

  rmSync(modelDirectory, { recursive: true, force: true });
  mkdirSync(modelDirectory, { recursive: true });
  mkdirSync(jsonDirectory, { recursive: true });

  const files: Array<{ schema: string; file: string; cases: number; valid: number; invalid: number }> = [];
  for (const name of schemaNames) {
    const schema = target(name);
    const definitions = (specs.get(name) ?? fail(name))();
    const names = new Set<string>();
    for (const definition of definitions) {
      if (names.has(definition.name)) fail(`${name}: duplicate case name ${definition.name}`);
      names.add(definition.name);
    }
    const cases = definitions.map((definition) => runCase(name, schema, definition));
    const file = `${name}.json`;
    writeJson(join(modelDirectory, file), { schema: name, cases });
    const valid = cases.filter((item) => item.valid === true).length;
    files.push({ schema: name, file, cases: cases.length, valid, invalid: cases.length - valid });
  }

  for (const rule of Object.keys(RULE_MESSAGES)) {
    const entry = ruleIndex.get(rule);
    if (entry === undefined || entry.trigger.length === 0 || entry.satisfy.length === 0) {
      fail(`rule ${rule} needs at least one trigger and one satisfy case`);
    }
  }

  const zodVersion = (
    JSON.parse(readFileSync(join(root, "node_modules", "zod", "package.json"), "utf8")) as {
      version: string;
    }
  ).version;
  const rules = Object.fromEntries(
    Object.keys(RULE_MESSAGES).map((rule) => {
      const pattern = RULE_MESSAGES[rule];
      return [
        rule,
        {
          message: typeof pattern === "string" ? pattern : pattern?.toString(),
          ...(ruleIndex.get(rule) ?? { trigger: [], satisfy: [] }),
        },
      ];
    }),
  );
  writeJson(join(vectorDirectory, "model-index.json"), {
    description:
      "Validation vectors observed from the 2.1.0 Zod schemas. Each case has input (the value after JSON parsing) " +
      "and, when the raw JSON text matters or the value holds a lone surrogate, input_text. When input_text is " +
      "present it is the authoritative input. output is JSON.parse(JSON.stringify(parsed)) for valid cases; " +
      "output_text replaces it when the output holds a lone surrogate. issues lists every Zod issue in order with " +
      "path, code, message, then the remaining Zod fields in key order. rules names semantic rules a case triggers " +
      "or satisfies (native/reference/semantic-rules.md).",
    generator: "native/tools/reference/generate-model-vectors.ts",
    zod_version: zodVersion,
    time_zone_data: `ICU ${process.versions.icu ?? "unknown"}, tz ${process.versions.tz ?? "unknown"}`,
    files,
    total_cases: files.reduce((sum, item) => sum + item.cases, 0),
    rules,
  });

  writeJson(join(jsonDirectory, "serialization.json"), serializationVectors());

  const written = readdirSync(modelDirectory).length;
  console.log(
    `Wrote ${written} schema files, ${files.reduce((sum, item) => sum + item.cases, 0)} cases, ` +
      `${Object.keys(RULE_MESSAGES).length} rules.`,
  );
}

main();
