/**
 * SQL schema version 2 metadata bump. Typed-table byte repair runs in the migration
 * runner through repairProviderEncodingRows(), which copies record_json into data_json
 * only when semanticJsonEqual says the values match.
 */
export const PROVIDER_ENCODING_REPAIR_SQL = `
UPDATE database_metadata SET sql_schema_version = 2 WHERE singleton_id = 1;
`.trim();
