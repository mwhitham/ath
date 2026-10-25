/**
 * Upgrade a version 1.0.0 database to software version 2.0.0.
 *
 * Adds workout name, description, text result, and route points.
 * Record history, links, attachments, and deletion rows are left in place.
 */
export const VERSION_2_UPGRADE_SQL = `
ALTER TABLE workout_sessions ADD COLUMN name TEXT;
ALTER TABLE workout_sessions ADD COLUMN description TEXT;
ALTER TABLE workout_sessions ADD COLUMN text_result TEXT;

CREATE TABLE workout_route_points (
  workout_session_id TEXT NOT NULL,
  point_order INTEGER NOT NULL CHECK (point_order >= 0),
  point_json TEXT NOT NULL CHECK (json_valid(point_json)),
  PRIMARY KEY (workout_session_id, point_order),
  FOREIGN KEY (workout_session_id)
    REFERENCES workout_sessions (record_id)
    ON DELETE CASCADE
) STRICT;

UPDATE database_metadata
SET sql_schema_version = 3,
    athleticstandard_version = '2.0.0'
WHERE singleton_id = 1;
`.trim();
