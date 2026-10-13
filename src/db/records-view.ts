import type { CurrentRecordT, RecordRevisionT } from "../v1/schema.js";
import { databaseClient, type AthleticDatabase } from "./client.js";

export interface RecordLink {
  direction: "outgoing" | "incoming";
  relation: string;
  record_id: string;
  revision?: number;
}

const linkColumns = [
  ["source_devices", "record_id", "source_id", "source"],
  ["point_measurements", "record_id", "source_id", "source"],
  ["measurement_samples", "record_id", "source_id", "source"],
  ["sleep_sessions", "record_id", "source_id", "source"],
  ["workout_sessions", "record_id", "source_id", "source"],
  ["benchmark_results", "record_id", "source_id", "source"],
  ["benchmark_results", "record_id", "benchmark_definition_id", "benchmark_definition"],
  ["benchmark_results", "record_id", "workout_session_id", "workout_session"],
  ["provider_summaries", "record_id", "source_id", "source"],
  ["provider_scores", "record_id", "source_id", "source"],
  ["record_attachment_links", "owner_record_id", "attachment_record_id", "attachment"],
  ["point_measurement_calculation_inputs", "point_measurement_id", "evidence_record_id", "calculation_input"],
  ["predictions", "record_id", "benchmark_definition_id", "benchmark_definition"],
  ["prediction_result_links", "record_id", "prediction_id", "prediction"],
  ["prediction_result_links", "record_id", "benchmark_result_id", "benchmark_result"],
  ["prediction_grades", "record_id", "prediction_result_link_id", "prediction_result_link"],
  ["prediction_error_analyses", "record_id", "prediction_grade_id", "prediction_grade"],
  ["prediction_error_causes", "prediction_error_analysis_id", "evidence_record_id", "error_cause_evidence"],
  ["planned_sessions", "record_id", "training_plan_id", "training_plan"],
  ["planned_sessions", "record_id", "completed_workout_session_id", "completed_workout_session"],
  ["goals", "record_id", "target_benchmark_definition_id", "target_benchmark_definition"],
  ["nutrition_model_interpretations", "record_id", "nutrition_entry_id", "nutrition_entry"],
  ["authored_record_evidence", "authored_record_id", "evidence_record_id", "authored_evidence"],
] as const;

const revisionColumns = new Map<string, string>([
  ["point_measurement_calculation_inputs:calculation_input", "evidence_revision"],
  ["prediction_result_links:prediction", "prediction_revision"],
  ["prediction_result_links:benchmark_result", "benchmark_result_revision"],
  ["prediction_grades:prediction_result_link", "prediction_result_link_revision"],
  ["prediction_error_analyses:prediction_grade", "prediction_grade_revision"],
  ["prediction_error_causes:error_cause_evidence", "evidence_revision"],
  ["authored_record_evidence:authored_evidence", "evidence_revision"],
]);

export async function readRecordLinks(
  database: AthleticDatabase,
  recordId: string,
): Promise<RecordLink[]> {
  const links: RecordLink[] = [];
  for (const [table, ownerColumn, targetColumn, relation] of linkColumns) {
    const revisionColumn = revisionColumns.get(`${table}:${relation}`);
    const outgoing = await databaseClient(database).execute({
      sql: `
        SELECT ${targetColumn} AS linked_record_id${
          revisionColumn === undefined ? "" : `, ${revisionColumn} AS linked_revision`
        }
        FROM ${table}
        WHERE ${ownerColumn} = ? AND ${targetColumn} IS NOT NULL
      `,
      args: [recordId],
    });
    for (const row of outgoing.rows) {
      links.push({
        direction: "outgoing",
        relation,
        record_id: String(row.linked_record_id),
        ...(row.linked_revision === undefined
          ? {}
          : { revision: Number(row.linked_revision) }),
      });
    }

    const incoming = await databaseClient(database).execute({
      sql: `
        SELECT ${ownerColumn} AS linked_record_id${
          revisionColumn === undefined ? "" : `, ${revisionColumn} AS linked_revision`
        }
        FROM ${table}
        WHERE ${targetColumn} = ?
      `,
      args: [recordId],
    });
    for (const row of incoming.rows) {
      links.push({
        direction: "incoming",
        relation,
        record_id: String(row.linked_record_id),
        ...(row.linked_revision === undefined
          ? {}
          : { revision: Number(row.linked_revision) }),
      });
    }
  }
  return links.sort(
    (left, right) =>
      left.direction.localeCompare(right.direction) ||
      left.relation.localeCompare(right.relation) ||
      left.record_id.localeCompare(right.record_id) ||
      (left.revision ?? 0) - (right.revision ?? 0),
  );
}

function recordLabel(record: CurrentRecordT): string {
  switch (record.record_type) {
    case "athlete_profile":
      return record.data.name ?? "athlete profile";
    case "source":
      return `${record.data.data_writer} via ${record.data.data_route}`;
    case "point_measurement":
      return `${record.data.quantity} ${record.data.value.standard_value}${record.data.value.standard_unit}`;
    case "measurement_sample":
      return `${record.data.quantity} ${record.data.value.standard_value}${record.data.value.standard_unit}`;
    case "provider_summary":
      return `${record.data.summary_type} ${record.data.value.standard_value}${record.data.value.standard_unit}`;
    case "provider_score":
      return `${record.data.metric} ${record.data.value.standard_value} ${record.data.scale}`;
    case "benchmark_definition":
      return record.data.benchmark_key;
    case "athlete_report":
      return record.data.report_type;
    case "nutrition_entry":
      return record.data.athlete_description;
    case "training_plan":
    case "planned_session":
    case "goal":
      return record.data.title;
    default:
      return "";
  }
}

export function renderRecordList(records: readonly CurrentRecordT[]): string {
  if (records.length === 0) {
    return "no records matched";
  }
  const lines = ["record ID                             revision  type                         summary"];
  for (const record of records) {
    lines.push(
      `${record.record_id}  ${record.current_revision.toString().padStart(8)}  ` +
        `${record.record_type.padEnd(27)}  ${record.is_deleted ? "[deleted] " : ""}` +
        recordLabel(record),
    );
  }
  return lines.join("\n");
}

export function renderRecord(
  record: CurrentRecordT,
  options: {
    revision?: RecordRevisionT;
    history?: readonly RecordRevisionT[];
    links?: readonly RecordLink[];
  } = {},
): string {
  const shown = options.revision;
  const lines = [
    `${record.record_id}`,
    `type: ${record.record_type}`,
    `created: ${record.created_at}`,
    `current revision: ${record.current_revision}`,
    `deleted: ${record.is_deleted ? "yes" : "no"}`,
  ];
  if (shown !== undefined) {
    lines.push(
      `showing revision: ${shown.revision}`,
      `became known: ${shown.became_known_at}`,
      `changed by: ${JSON.stringify(shown.changed_by)}`,
      "",
      JSON.stringify(shown.data, null, 2),
    );
  } else {
    lines.push("", JSON.stringify(record.data, null, 2));
  }

  if (options.history !== undefined) {
    lines.push("", "history:");
    for (const revision of options.history) {
      lines.push(
        `  revision ${revision.revision} — ${revision.became_known_at} ` +
          `(${revision.became_known_at_basis})`,
      );
    }
  }
  if (options.links !== undefined) {
    lines.push("", "links:");
    if (options.links.length === 0) {
      lines.push("  none");
    }
    for (const link of options.links) {
      lines.push(
        `  ${link.direction} ${link.relation}: ${link.record_id}` +
          (link.revision === undefined ? "" : `@${link.revision}`),
      );
    }
  }
  return lines.join("\n");
}
