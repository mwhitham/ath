import { RecordExtensions, type RecordExtensionsT } from "../v1/extensions.js";

export function parseExtensionsJson(
  value: unknown,
): RecordExtensionsT | undefined {
  if (value === null || value === undefined) {
    return undefined;
  }
  if (typeof value !== "string") {
    throw new Error("extensions_json must be text");
  }
  const parsed = JSON.parse(value) as unknown;
  if (parsed === null) {
    return undefined;
  }
  return RecordExtensions.parse(parsed);
}

export function serializeExtensionsJson(
  extensions: RecordExtensionsT | undefined,
): string | null {
  if (extensions === undefined) {
    return null;
  }
  return JSON.stringify(extensions);
}

export function currentRecordPayloadFromRow(
  row: Record<string, unknown>,
): Record<string, unknown> {
  const extensions = parseExtensionsJson(row.extensions_json ?? null);
  return {
    record_id: row.record_id,
    record_type: row.record_type,
    current_revision: Number(row.current_revision),
    created_at: row.created_at,
    is_deleted: Number(row.is_deleted) === 1,
    data: JSON.parse(String(row.record_json)) as unknown,
    ...(extensions === undefined ? {} : { extensions }),
  };
}

export function recordRevisionPayloadFromRow(
  row: Record<string, unknown>,
): Record<string, unknown> {
  const extensions = parseExtensionsJson(row.extensions_json ?? null);
  return {
    record_id: row.record_id,
    record_type: row.record_type,
    revision: Number(row.revision),
    previous_revision:
      row.previous_revision === null ? null : Number(row.previous_revision),
    became_known_at: row.became_known_at,
    became_known_at_basis: row.became_known_at_basis,
    changed_by: JSON.parse(String(row.changed_by_json)) as unknown,
    data: JSON.parse(String(row.record_json)) as unknown,
    ...(extensions === undefined ? {} : { extensions }),
  };
}
