-- Athletic Standard upgrade to software version 2.1.1 (migration step 5).
-- This file is the executable migration. The ath program embeds it unchanged.
--
-- See build-history/v2.1.1/spec.md section 9.2 and decision D211.
-- No athlete-owned table or column changes. Records written before this step
-- stay as they are.
--
-- internal_pending_file_operations holds recovery state for file changes that
-- a SQLite transaction cannot include: attachment files written before their
-- records commit, and attachment files that an erasure must still remove. Rows
-- hold only operation IDs, kinds, relative paths, and times. They hold no
-- health values and are not part of an exported package.

CREATE TABLE internal_pending_file_operations (
  operation_id TEXT NOT NULL,
  operation_kind TEXT NOT NULL CHECK (operation_kind IN (
    'attachment_write',
    'attachment_removal'
  )),
  relative_path TEXT NOT NULL CHECK (length(relative_path) > 0),
  created_at TEXT NOT NULL,
  PRIMARY KEY (operation_id, relative_path)
) STRICT;

UPDATE database_metadata
SET sql_schema_version = 5,
    athleticstandard_version = '2.1.1'
WHERE singleton_id = 1;
