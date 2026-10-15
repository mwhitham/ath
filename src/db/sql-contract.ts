import { ENFORCEMENT_TRIGGERS_SQL } from "./migrations/enforcement-triggers.js";
import { INITIAL_SCHEMA_SQL } from "./migrations/001-initial.js";
import { SQL_SCHEMA_VERSION } from "./client.js";

export const MINIMUM_SQLITE_VERSION = "3.45.0";

export const REQUIRED_PRAGMAS = [
  { name: "foreign_keys", value: "ON" },
  { name: "busy_timeout", value: "5000" },
] as const;

export interface StableViewDefinition {
  name: string;
  sql_schema_version: number;
  columns: readonly string[];
  description: string;
}

export const STABLE_VIEWS: readonly StableViewDefinition[] = [
  {
    name: "sql_v1_database_info",
    sql_schema_version: 1,
    columns: [
      "application_marker",
      "sql_schema_version",
      "athleticstandard_version",
      "database_id",
      "created_at",
      "database_revision",
    ],
    description: "Database identity, format versions, and monotonic database revision.",
  },
  {
    name: "sql_v1_current_records",
    sql_schema_version: 1,
    columns: [
      "record_id",
      "record_type",
      "current_revision",
      "created_at",
      "is_deleted",
      "became_known_at",
      "became_known_at_basis",
      "changed_by_json",
      "previous_revision",
      "record_json",
      "extensions_json",
    ],
    description: "Every record at its current revision, including soft-deleted rows.",
  },
  {
    name: "sql_v1_active_records",
    sql_schema_version: 1,
    columns: [
      "record_id",
      "record_type",
      "current_revision",
      "created_at",
      "became_known_at",
      "became_known_at_basis",
      "changed_by_json",
      "previous_revision",
      "record_json",
      "extensions_json",
    ],
    description: "Current revisions for records that are not soft-deleted.",
  },
  {
    name: "sql_v1_record_revisions",
    sql_schema_version: 1,
    columns: [
      "record_id",
      "revision",
      "record_type",
      "became_known_at",
      "became_known_at_basis",
      "changed_by_json",
      "previous_revision",
      "record_json",
      "extensions_json",
    ],
    description: "Full revision history for every record.",
  },
  {
    name: "sql_v1_record_deletions",
    sql_schema_version: 1,
    columns: ["record_id", "record_revision", "deleted_at", "deleted_by_json"],
    description: "Soft-deletion records that hide a record from active views.",
  },
];

export const STABLE_WRITABLE_TABLES = [
  "records",
  "record_revisions",
  "record_deletions",
  "database_state",
  "athlete_profile",
  "sources",
  "source_devices",
  "point_measurements",
  "point_measurement_calculations",
  "point_measurement_calculation_inputs",
  "measurement_samples",
  "sleep_sessions",
  "workout_sessions",
  "workout_session_segments",
  "benchmark_results",
  "benchmark_result_segments",
  "provider_scores",
  "provider_summaries",
  "provider_reporting_periods",
  "athlete_reports",
  "nutrition_entries",
  "reported_nutrition_items",
  "attachments",
  "record_attachment_links",
  "benchmark_definitions",
  "predictions",
  "prediction_result_links",
  "prediction_grades",
  "prediction_error_analyses",
  "prediction_error_causes",
  "training_plans",
  "planned_sessions",
  "planned_session_blocks",
  "coaching_notes",
  "saved_training_reviews",
  "goals",
  "nutrition_model_interpretations",
  "authored_record_evidence",
] as const;

export const INTERNAL_TABLES = [
  "database_metadata",
  "record_erasure_markers",
  "record_import_keys",
] as const;

export const TRANSACTION_RECIPES = {
  connection: [
    "PRAGMA foreign_keys = ON;",
    "PRAGMA busy_timeout = 5000;",
    "PRAGMA defer_foreign_keys = ON;",
  ],
  create_record: [
    "INSERT INTO records (record_id, record_type, current_revision, created_at, is_deleted)",
    "INSERT INTO record_revisions (...)",
    "INSERT or UPSERT into the typed table for record_type and any ordered child rows",
    "UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1",
  ],
  correct_record: [
    "INSERT INTO record_revisions (...) with revision = current_revision + 1",
    "UPDATE records SET current_revision = ? WHERE record_id = ?",
    "UPSERT typed table and child rows for the new revision payload",
    "UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1",
  ],
  soft_delete: [
    "INSERT INTO record_deletions (...)",
    "UPDATE records SET is_deleted = 1 WHERE record_id = ?",
    "UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1",
  ],
} as const;

export const MIGRATION_CHAIN = [
  {
    version: 1,
    file: "migrations/001-initial.sql",
    description: "Initial Athletic Standard v1 schema, enforcement triggers, and stable read views.",
  },
] as const;

export function publishedMigrationSql(): string {
  return INITIAL_SCHEMA_SQL.trim();
}

export function enforcementTriggerNames(): string[] {
  const names: string[] = [];
  const pattern = /CREATE TRIGGER\s+(\w+)/g;
  for (const match of ENFORCEMENT_TRIGGERS_SQL.matchAll(pattern)) {
    const name = match[1];
    if (name !== undefined) {
      names.push(name);
    }
  }
  return names;
}

export function buildSqlContractDocument(): Record<string, unknown> {
  return {
    contract: "athleticstandard-sql",
    sql_schema_version: SQL_SCHEMA_VERSION,
    minimum_sqlite_version: MINIMUM_SQLITE_VERSION,
    user_version_mirror: {
      description:
        "On local SQLite files, PRAGMA user_version equals database_metadata.sql_schema_version after creation.",
      pragma: "user_version",
    },
    required_pragmas: REQUIRED_PRAGMAS,
    migrations: MIGRATION_CHAIN,
    metadata_insert: {
      description:
        "After 001-initial.sql runs, the migration runner inserts one database_metadata row and one database_state row. database_id and created_at are supplied by the runner. They are not fixed in the SQL file.",
      tables: ["database_metadata", "database_state"],
    },
    stable_views: STABLE_VIEWS,
    stable_writable_tables: STABLE_WRITABLE_TABLES,
    internal_tables: INTERNAL_TABLES,
    transaction_recipes: TRANSACTION_RECIPES,
    enforcement_triggers: enforcementTriggerNames(),
  };
}
