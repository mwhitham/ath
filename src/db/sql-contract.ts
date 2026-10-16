import { ENFORCEMENT_TRIGGERS_SQL } from "./migrations/enforcement-triggers.js";
import { INITIAL_SCHEMA_SQL } from "./migrations/001-initial.js";
import { SQL_SCHEMA_VERSION, SQLITE_APPLICATION_ID } from "./client.js";

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

export const COLLECTION_REPLACEMENT = {
  description:
    "On every create or correction, ordered child collections and link tables are replaced from the full payload. Delete existing child rows for the parent record_id, then insert the payload order. Omitted children are removed. An empty collection deletes all prior child rows.",
  tables: [
    {
      table: "provider_reporting_periods",
      parent_column: "owner_record_id",
      applies_when: "record_json includes provider_reporting_periods",
    },
    {
      table: "authored_record_evidence",
      parent_column: "authored_record_id",
      applies_when: "record_json includes authorship.evidence",
    },
    {
      table: "record_attachment_links",
      parent_column: "owner_record_id",
      applies_when: "athlete_report or nutrition_entry record_json",
    },
    {
      table: "reported_nutrition_items",
      parent_column: "nutrition_entry_id",
      applies_when: "nutrition_entry corrections",
    },
    {
      table: "point_measurement_calculations",
      parent_column: "point_measurement_id",
      applies_when: "point_measurement corrections",
    },
    {
      table: "point_measurement_calculation_inputs",
      parent_column: "point_measurement_id",
      applies_when: "point_measurement corrections",
    },
    {
      table: "workout_session_segments",
      parent_column: "workout_session_id",
      applies_when: "workout_session corrections",
    },
    {
      table: "benchmark_result_segments",
      parent_column: "benchmark_result_id",
      applies_when: "benchmark_result corrections",
    },
    {
      table: "prediction_error_causes",
      parent_column: "prediction_error_analysis_id",
      applies_when: "prediction_error_analysis corrections",
    },
    {
      table: "planned_session_blocks",
      parent_column: "planned_session_id",
      applies_when: "planned_session corrections",
    },
  ],
} as const;

export const TRANSACTION_RECIPES = {
  connection: ["PRAGMA foreign_keys = ON;", "PRAGMA busy_timeout = 5000;"],
  write_transaction_start: [
    "BEGIN IMMEDIATE;",
    "PRAGMA foreign_keys = ON;",
    "PRAGMA defer_foreign_keys = ON;",
  ],
  write_transaction_note:
    "SQLite resets defer_foreign_keys after each COMMIT or ROLLBACK. Run write_transaction_start at the beginning of every write transaction. Do not rely on a connection-level defer_foreign_keys setting.",
  create_record: [
    "write_transaction_start",
    "INSERT INTO records (record_id, record_type, current_revision, created_at, is_deleted)",
    "INSERT INTO record_revisions (...)",
    "INSERT or UPSERT into the typed table for record_type",
    "Replace child collections from the full payload (see collection_replacement)",
    "UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1",
    "COMMIT;",
  ],
  correct_record: [
    "write_transaction_start",
    "INSERT INTO record_revisions (...) with revision = current_revision + 1",
    "UPDATE records SET current_revision = ? WHERE record_id = ?",
    "UPSERT typed table for the new revision payload",
    "DELETE and reinsert each child collection for the parent record_id (see collection_replacement)",
    "UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1",
    "COMMIT;",
  ],
  soft_delete: [
    "write_transaction_start",
    "INSERT INTO record_deletions (...)",
    "UPDATE records SET is_deleted = 1 WHERE record_id = ?",
    "UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1",
    "COMMIT;",
  ],
} as const;

export const FILE_HEADER_PRAGMAS = {
  application_id: {
    value: SQLITE_APPLICATION_ID,
    literal: "0x41544831",
    description: "Athletic Standard file marker (ATH1). Set on local database creation.",
  },
  user_version: {
    matches: "database_metadata.sql_schema_version",
    description: "Local mirror of sql_schema_version on file-backed databases.",
  },
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
        "After 001-initial.sql runs, the migration runner inserts one database_metadata row and one database_state row, then sets file header pragmas on local files. database_id and created_at are supplied by the runner. They are not fixed in the SQL file.",
      tables: ["database_metadata", "database_state"],
      header_pragmas: [
        `PRAGMA application_id = ${SQLITE_APPLICATION_ID};`,
        `PRAGMA user_version = ${SQL_SCHEMA_VERSION};`,
      ],
    },
    file_header_pragmas: FILE_HEADER_PRAGMAS,
    stable_views: STABLE_VIEWS,
    stable_writable_tables: STABLE_WRITABLE_TABLES,
    internal_tables: INTERNAL_TABLES,
    collection_replacement: COLLECTION_REPLACEMENT,
    transaction_recipes: TRANSACTION_RECIPES,
    enforcement_triggers: enforcementTriggerNames(),
  };
}

export interface DatabaseFileHeader {
  application_id: number;
  user_version: number;
}

function pragmaNumber(
  rows: Array<Record<string, unknown>>,
  column: string,
): number {
  const row = rows[0];
  if (row === undefined) {
    throw new Error(`Missing PRAGMA ${column} result`);
  }
  const value = row[column];
  if (typeof value === "number" || typeof value === "bigint") {
    return Number(value);
  }
  throw new Error(`PRAGMA ${column} did not return a number`);
}

export async function readDatabaseFileHeader(
  execute: (sql: string) => Promise<{ rows: Array<Record<string, unknown>> }>,
): Promise<DatabaseFileHeader> {
  const applicationId = await execute("PRAGMA application_id");
  const userVersion = await execute("PRAGMA user_version");
  return {
    application_id: pragmaNumber(applicationId.rows, "application_id"),
    user_version: pragmaNumber(userVersion.rows, "user_version"),
  };
}
