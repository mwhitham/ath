/**
 * Upgrade a database to software version 2.1.0.
 *
 * No column is added. Records written before this step stay as they are.
 */
export const VERSION_2_1_UPGRADE_SQL = `
UPDATE database_metadata
SET sql_schema_version = 4,
    athleticstandard_version = '2.1.0'
WHERE singleton_id = 1;
`.trim();
