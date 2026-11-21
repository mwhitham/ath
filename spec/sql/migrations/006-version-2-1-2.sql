-- Athletic Standard upgrade to software version 2.1.2 (migration step 6).
-- This file is the executable migration. The ath program embeds it unchanged.
--
-- See build-history/v2.1.2/spec.md section 2 and decisions D227 and D238.
-- One nullable column is added. No existing row changes. Records written
-- before this step stay as they are.
--
-- athlete_reports.workout_session_id names the workout an athlete report
-- describes, such as the session a session-effort rating covers. The link
-- points from the report to the measured workout. The report still has no
-- source. SQLite enforces the link like every other logical link (D104).

ALTER TABLE athlete_reports
  ADD COLUMN workout_session_id TEXT
  REFERENCES workout_sessions (record_id);

CREATE INDEX athlete_reports_workout_session_idx
  ON athlete_reports (workout_session_id)
  WHERE workout_session_id IS NOT NULL;

UPDATE database_metadata
SET sql_schema_version = 6,
    athleticstandard_version = '2.1.2'
WHERE singleton_id = 1;
