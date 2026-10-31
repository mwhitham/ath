import { createClient } from "@libsql/client";
import { pathToFileURL } from "node:url";

/** The main commit whose importers produced the committed comparison files. */
export const BASELINE_FROM = "d418a8f653f7074ed9e08208c4eb555c391bcbc7";

const TIME_KEYS = new Set(["received_at", "imported_at"]);
const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g;

const LINK_COLUMNS = [
  ["source_devices", "record_id", "source_id", "source", null],
  ["point_measurements", "record_id", "source_id", "source", null],
  ["measurement_samples", "record_id", "source_id", "source", null],
  ["sleep_sessions", "record_id", "source_id", "source", null],
  ["workout_sessions", "record_id", "source_id", "source", null],
  ["benchmark_results", "record_id", "source_id", "source", null],
  ["benchmark_results", "record_id", "benchmark_definition_id", "benchmark_definition", null],
  ["benchmark_results", "record_id", "workout_session_id", "workout_session", null],
  ["provider_summaries", "record_id", "source_id", "source", null],
  ["provider_scores", "record_id", "source_id", "source", null],
  ["point_measurement_calculation_inputs", "point_measurement_id", "evidence_record_id", "calculation_input", "evidence_revision"],
  ["predictions", "record_id", "benchmark_definition_id", "benchmark_definition", null],
  ["prediction_result_links", "record_id", "prediction_id", "prediction", "prediction_revision"],
  ["prediction_result_links", "record_id", "benchmark_result_id", "benchmark_result", "benchmark_result_revision"],
  ["prediction_grades", "record_id", "prediction_result_link_id", "prediction_result_link", "prediction_result_link_revision"],
  ["prediction_error_analyses", "record_id", "prediction_grade_id", "prediction_grade", "prediction_grade_revision"],
  ["prediction_error_causes", "prediction_error_analysis_id", "evidence_record_id", "error_cause_evidence", "evidence_revision"],
  ["planned_sessions", "record_id", "training_plan_id", "training_plan", null],
  ["planned_sessions", "record_id", "completed_workout_session_id", "completed_workout_session", null],
  ["goals", "record_id", "target_benchmark_definition_id", "target_benchmark_definition", null],
  ["nutrition_model_interpretations", "record_id", "nutrition_entry_id", "nutrition_entry", null],
  ["authored_record_evidence", "authored_record_id", "evidence_record_id", "authored_evidence", "evidence_revision"],
] as const;

interface StoredRecord {
  record_id: string;
  record_type: string;
  current_revision: number;
  is_deleted: boolean;
  data: unknown;
  extensions: unknown;
}

interface ImportKey {
  method: string;
  key: string;
}

export interface ImportSnapshot {
  records: StoredRecord[];
  keys: Map<string, ImportKey[]>;
  routePoints: Array<{ workout: string; order: number; point: unknown }>;
  attachments: Array<{
    record_id: string;
    file_name: string;
    media_type: string;
    byte_count: number;
    sha256: string;
  }>;
  attachmentLinks: Array<{ owner: string; order: number; attachment: string }>;
  links: Array<{ table: string; relation: string; owner: string; target: string; revision?: number }>;
}

function withoutTimes(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(withoutTimes);
  if (typeof value !== "object" || value === null) return value;
  const out: Record<string, unknown> = {};
  for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
    if (TIME_KEYS.has(key)) continue;
    out[key] = withoutTimes(child);
  }
  return out;
}

export function canonical(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonical);
  if (typeof value !== "object" || value === null) return value;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0))
      .map(([key, child]) => [key, canonical(child)]),
  );
}

function replaceIds(value: unknown, replace: (id: string) => string): unknown {
  if (typeof value === "string") return value.replace(UUID, (match) => replace(match));
  if (Array.isArray(value)) return value.map((child) => replaceIds(child, replace));
  if (typeof value !== "object" || value === null) return value;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>).map(([key, child]) => [
      key,
      replaceIds(child, replace),
    ]),
  );
}

function parseJson(value: unknown): unknown {
  return typeof value === "string" ? (JSON.parse(value) as unknown) : null;
}

/** Read every current record and the rows that hang off it. */
export async function readImportSnapshot(databasePath: string): Promise<ImportSnapshot> {
  const client = createClient({ url: pathToFileURL(databasePath).href });
  try {
    const recordRows = await client.execute(`
      SELECT records.record_id, records.record_type, records.current_revision,
             records.is_deleted, record_revisions.record_json, record_revisions.extensions_json
      FROM records
      JOIN record_revisions
        ON record_revisions.record_id = records.record_id
       AND record_revisions.revision = records.current_revision
    `);
    const records = recordRows.rows.map((row) => ({
      record_id: String(row.record_id),
      record_type: String(row.record_type),
      current_revision: Number(row.current_revision),
      is_deleted: Number(row.is_deleted) === 1,
      data: parseJson(row.record_json),
      extensions: parseJson(row.extensions_json),
    }));
    const keyRows = await client.execute(
      "SELECT record_id, match_key_method, match_key FROM record_import_keys",
    );
    const keys = new Map<string, ImportKey[]>();
    for (const row of keyRows.rows) {
      const list = keys.get(String(row.record_id)) ?? [];
      list.push({ method: String(row.match_key_method), key: String(row.match_key) });
      keys.set(String(row.record_id), list);
    }
    const pointRows = await client.execute(
      "SELECT workout_session_id, point_order, point_json FROM workout_route_points",
    );
    const routePoints = pointRows.rows.map((row) => ({
      workout: String(row.workout_session_id),
      order: Number(row.point_order),
      point: parseJson(row.point_json),
    }));
    const attachmentRows = await client.execute(
      "SELECT record_id, file_name, media_type, byte_count, sha256 FROM attachments",
    );
    const attachments = attachmentRows.rows.map((row) => ({
      record_id: String(row.record_id),
      file_name: String(row.file_name),
      media_type: String(row.media_type),
      byte_count: Number(row.byte_count),
      sha256: String(row.sha256),
    }));
    const attachmentLinkRows = await client.execute(
      "SELECT owner_record_id, attachment_order, attachment_record_id FROM record_attachment_links",
    );
    const attachmentLinks = attachmentLinkRows.rows.map((row) => ({
      owner: String(row.owner_record_id),
      order: Number(row.attachment_order),
      attachment: String(row.attachment_record_id),
    }));
    const links: ImportSnapshot["links"] = [];
    for (const [table, ownerColumn, targetColumn, relation, revisionColumn] of LINK_COLUMNS) {
      const result = await client.execute(
        `SELECT ${ownerColumn} AS owner, ${targetColumn} AS target${
          revisionColumn === null ? "" : `, ${revisionColumn} AS revision`
        } FROM ${table} WHERE ${targetColumn} IS NOT NULL`,
      );
      for (const row of result.rows) {
        links.push({
          table,
          relation,
          owner: String(row.owner),
          target: String(row.target),
          ...(revisionColumn === null ? {} : { revision: Number(row.revision) }),
        });
      }
    }
    return { records, keys, routePoints, attachments, attachmentLinks, links };
  } finally {
    client.close();
  }
}

function compareText(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

/**
 * Give each record a label that does not depend on random ids.
 *
 * A record with an import key sorts by method, then key, then type. A key can
 * contain the id of its source, so that id is first replaced by the source's
 * own type and key. A record with no key sorts after those, by type and then
 * by its JSON with every record id blanked.
 */
export function labelRecords(snapshot: ImportSnapshot): Map<string, string> {
  const byId = new Map(snapshot.records.map((record) => [record.record_id, record]));
  const ownIdentity = (id: string): string => {
    const record = byId.get(id);
    if (record === undefined) return id;
    const keys = (snapshot.keys.get(id) ?? [])
      .map((key) => `${key.method}\u0001${key.key.replace(UUID, "?")}`)
      .sort(compareText);
    return `${record.record_type}\u0002${keys.join("\u0003")}`;
  };
  const substitute = (text: string): string =>
    text.replace(UUID, (match) => (byId.has(match) ? `<${ownIdentity(match)}>` : match));

  const keyed: Array<{ id: string; sort: string[] }> = [];
  const keyless: Array<{ id: string; sort: string[] }> = [];
  for (const record of snapshot.records) {
    const keys = (snapshot.keys.get(record.record_id) ?? [])
      .map((key) => `${key.method}\u0000${substitute(key.key)}`)
      .sort(compareText);
    if (keys.length > 0) {
      keyed.push({ id: record.record_id, sort: [...keys, record.record_type] });
    } else {
      const blanked = replaceIds(withoutTimes(record.data), (match) =>
        byId.has(match) ? "?" : match,
      );
      keyless.push({
        id: record.record_id,
        sort: [record.record_type, JSON.stringify(canonical(blanked))],
      });
    }
  }
  const order = (left: { sort: string[] }, right: { sort: string[] }): number => {
    for (let index = 0; index < Math.max(left.sort.length, right.sort.length); index += 1) {
      const diff = compareText(left.sort[index] ?? "", right.sort[index] ?? "");
      if (diff !== 0) return diff;
    }
    return 0;
  };
  keyed.sort(order);
  keyless.sort(order);
  const all = [...keyed, ...keyless];
  for (let index = 1; index < all.length; index += 1) {
    if (order(all[index - 1]!, all[index]!) === 0) {
      throw new Error(`two records sort the same way: ${all[index]!.sort.join(" | ")}`);
    }
  }
  return new Map(all.map((entry, index) => [entry.id, `record:${index + 1}`]));
}

/** The comparison string for one import. Keys are sorted at every depth. */
export function comparisonString(
  snapshot: ImportSnapshot,
  options: { withoutKey?: string } = {},
): string {
  const labels = labelRecords(snapshot);
  const label = (id: string): string => labels.get(id) ?? id;
  const relabel = (value: unknown): unknown => replaceIds(value, label);
  const strip = (data: unknown): unknown => {
    if (options.withoutKey === undefined || typeof data !== "object" || data === null) return data;
    const { [options.withoutKey]: _removed, ...rest } = data as Record<string, unknown>;
    return rest;
  };
  const records = snapshot.records
    .map((record) => ({
      label: label(record.record_id),
      record_type: record.record_type,
      current_revision: record.current_revision,
      is_deleted: record.is_deleted,
      import_keys: (snapshot.keys.get(record.record_id) ?? [])
        .map((key) => ({ method: key.method, key: relabel(key.key) as string }))
        .sort((left, right) => compareText(left.method, right.method) || compareText(left.key, right.key)),
      data: relabel(strip(withoutTimes(record.data))),
      ...(record.extensions === null ? {} : { extensions: relabel(withoutTimes(record.extensions)) }),
    }))
    .sort((left, right) => Number(left.label.slice(7)) - Number(right.label.slice(7)));
  const routePoints = snapshot.routePoints
    .map((point) => ({
      workout: label(point.workout),
      order: point.order,
      point: relabel(withoutTimes(point.point)),
    }))
    .sort(
      (left, right) =>
        Number(left.workout.slice(7)) - Number(right.workout.slice(7)) || left.order - right.order,
    );
  const attachments = snapshot.attachments
    .map((row) => ({ ...row, record_id: label(row.record_id) }))
    .sort((left, right) => compareText(left.sha256, right.sha256));
  const attachmentLinks = snapshot.attachmentLinks
    .map((link) => ({ owner: label(link.owner), order: link.order, attachment: label(link.attachment) }))
    .sort((left, right) => compareText(left.owner, right.owner) || left.order - right.order);
  const links = snapshot.links
    .map((link) => ({ ...link, owner: label(link.owner), target: label(link.target) }))
    .sort(
      (left, right) =>
        compareText(left.table, right.table) ||
        compareText(left.relation, right.relation) ||
        compareText(left.owner, right.owner) ||
        compareText(left.target, right.target) ||
        (left.revision ?? 0) - (right.revision ?? 0),
    );
  return `${JSON.stringify(
    canonical({
      baseline_from: BASELINE_FROM,
      records,
      route_points: routePoints,
      attachments,
      attachment_links: attachmentLinks,
      links,
    }),
    null,
    2,
  )}\n`;
}
