import type { LogPlan } from "../db/index.js";

export function renderImportPreview(preview: {
  source_database_id: string;
  source_database_revision: number;
  destination_database_id: string;
  destination_database_revision: number;
  restore: boolean;
  new_records: readonly string[];
  new_record_count: number;
  matching_records: readonly string[];
  matching_record_count: number;
  changed_records: readonly string[];
  changed_record_count: number;
  record_lists_truncated: boolean;
  conflicts: readonly { record_id?: string; message: string }[];
  unrepresentable_records: readonly string[];
  unknown_fields: readonly string[];
}): string {
  const lines = [
    `${preview.restore ? "restore" : "merge"} preview`,
    `  source database:      ${preview.source_database_id} revision ${preview.source_database_revision}`,
    `  destination database: ${preview.destination_database_id} revision ${preview.destination_database_revision}`,
    `  new records:          ${preview.new_record_count}`,
    `  matching records:     ${preview.matching_record_count}`,
    `  changed records:      ${preview.changed_record_count}`,
    `  conflicts:            ${preview.conflicts.length}`,
    `  unrepresentable:       ${preview.unrepresentable_records.length}`,
    `  unknown fields:        ${preview.unknown_fields.length}`,
  ];
  if (preview.record_lists_truncated) {
    lines.push("  record ID lists:      first 1,000 per category");
  }
  for (const conflict of preview.conflicts) {
    lines.push(
      `    ${conflict.record_id === undefined ? "" : `${conflict.record_id}: `}` +
        conflict.message,
    );
  }
  return lines.join("\n");
}

export function renderProviderImportPreview(preview: {
  imported: string;
  destination_database_revision: number;
  new_records: readonly { record_id: string; record_type: string }[];
  matching_records: readonly { record_id: string; record_type: string }[];
  changed_records: readonly {
    record_id: string;
    record_type: string;
    current_revision: number;
  }[];
  conflicts: readonly { record_id?: string; message: string }[];
  skipped: Readonly<Record<string, { count: number; example?: string }>>;
  route_points_stored?: number;
}): string {
  const skippedCount = Object.values(preview.skipped).reduce(
    (total, skipped) => total + skipped.count,
    0,
  );
  const lines = [
    `${preview.imported} preview`,
    `  destination revision: ${preview.destination_database_revision}`,
    `  new records:         ${preview.new_records.length}`,
    `  matching records:    ${preview.matching_records.length}`,
    `  changed records:     ${preview.changed_records.length}`,
    `  conflicts:           ${preview.conflicts.length}`,
    `  skipped rows:        ${skippedCount}`,
  ];
  if ((preview.route_points_stored ?? 0) > 0) {
    const count = preview.route_points_stored ?? 0;
    lines.push(`  route points stored: ${count}`);
  }
  for (const conflict of preview.conflicts) {
    lines.push(
      `    ${conflict.record_id === undefined ? "" : `${conflict.record_id}: `}` +
        conflict.message,
    );
  }
  for (const [reason, skipped] of Object.entries(preview.skipped)) {
    lines.push(
      `    skipped ${skipped.count} — ${reason}` +
        `${skipped.example === undefined ? "" : ` (example: ${skipped.example})`}`,
    );
  }
  return lines.join("\n");
}

export function renderLogPlan(plan: LogPlan): string {
  const lines = [
    `understood: ${plan.interpretation}`,
    `records to write: ${plan.inputs.length}`,
  ];
  for (const input of plan.inputs) {
    lines.push(`  ${input.record_id}  ${input.record_type}`);
    lines.push(
      ...JSON.stringify(input.data, null, 2)
        .split("\n")
        .map((line) => `    ${line}`),
    );
    if (input.extensions !== undefined) {
      lines.push("    extensions");
      lines.push(
        ...JSON.stringify(input.extensions, null, 2)
          .split("\n")
          .map((line) => `    ${line}`),
      );
    }
  }
  return lines.join("\n");
}
