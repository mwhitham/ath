import { typedTables } from "../check-tables.js";

function alignTypedTableSql(table: string, recordType: string): string {
  return `
UPDATE ${table}
SET data_json = (
  SELECT record_revisions.record_json
  FROM records
  JOIN record_revisions
    ON record_revisions.record_id = records.record_id
   AND record_revisions.revision = records.current_revision
  WHERE records.record_id = ${table}.record_id
)
WHERE record_id IN (
  SELECT records.record_id
  FROM records
  JOIN record_revisions
    ON record_revisions.record_id = records.record_id
   AND record_revisions.revision = records.current_revision
  JOIN ${table} ON ${table}.record_id = records.record_id
  WHERE records.record_type = '${recordType}'
    AND ${table}.data_json <> record_revisions.record_json
);`;
}

const alignStatements = Object.entries(typedTables)
  .map(([recordType, table]) => alignTypedTableSql(table, recordType))
  .join("\n");

export const PROVIDER_ENCODING_ALIGN_SQL = alignStatements.trim();

export const PROVIDER_ENCODING_REPAIR_SQL = `${PROVIDER_ENCODING_ALIGN_SQL}
UPDATE database_metadata SET sql_schema_version = 2 WHERE singleton_id = 1;
`.trim();
