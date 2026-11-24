-- Athletic Standard upgrade to software version 2.1.3 (migration step 7).
-- This file is the executable migration. The ath program embeds it unchanged.
--
-- See build-history/v2.1.3/spec.md section 4.2 and decisions D254 and D261
-- to D263. Three internal tables are added. No athlete-owned table, column,
-- or row changes. Records written before this step stay as they are. None of
-- the three tables is part of an exported package, and none holds a health
-- value.
--
-- internal_change_log has one row for each record revision, deletion, and
-- erasure, with the database revision that committed it (D263). Triggers
-- fill it in the same transaction as the change, so direct SQL writers that
-- follow the published recipes are included. A write raises
-- database_state.current_revision once, at the end of its transaction, so a
-- trigger records the current value plus one. The rows hold only record IDs
-- and numbers, and no foreign keys. An erasure removes the record's earlier
-- rows and leaves one erasure row, which names no revision
-- (record_revision 0), so the log keeps no more than the erasure marker
-- does. A restore into an empty database starts a new log.
--
-- internal_write_receipts has one row for each write request a host accepted
-- (D261): who sent it, through which app, the request ID, a keyed digest of
-- the request, and the resulting IDs and revisions. It holds no request body.
--
-- internal_access_state holds the access epoch (D262). Revoking a grant
-- raises it. A local database that never connects to a host keeps it at 0.

CREATE TABLE internal_change_log (
  database_revision INTEGER NOT NULL CHECK (database_revision >= 1),
  record_id TEXT NOT NULL,
  record_revision INTEGER NOT NULL CHECK (record_revision >= 0),
  change_kind TEXT NOT NULL CHECK (change_kind IN (
    'revision',
    'deletion',
    'erasure'
  )),
  PRIMARY KEY (database_revision, record_id, change_kind, record_revision)
) STRICT, WITHOUT ROWID;

CREATE INDEX internal_change_log_record_idx
  ON internal_change_log (record_id);

CREATE TRIGGER internal_change_log_revision_insert
AFTER INSERT ON record_revisions
BEGIN
  INSERT OR IGNORE INTO internal_change_log (database_revision, record_id, record_revision, change_kind)
  VALUES (
    (SELECT current_revision + 1 FROM database_state WHERE singleton_id = 1),
    NEW.record_id,
    NEW.revision,
    'revision'
  );
END;

CREATE TRIGGER internal_change_log_deletion_insert
AFTER INSERT ON record_deletions
BEGIN
  INSERT OR IGNORE INTO internal_change_log (database_revision, record_id, record_revision, change_kind)
  VALUES (
    (SELECT current_revision + 1 FROM database_state WHERE singleton_id = 1),
    NEW.record_id,
    NEW.record_revision,
    'deletion'
  );
END;

CREATE TRIGGER internal_change_log_erasure_insert
AFTER INSERT ON record_erasure_markers
BEGIN
  DELETE FROM internal_change_log
  WHERE record_id = NEW.record_id AND change_kind <> 'erasure';
  INSERT OR IGNORE INTO internal_change_log (database_revision, record_id, record_revision, change_kind)
  VALUES (
    (SELECT current_revision + 1 FROM database_state WHERE singleton_id = 1),
    NEW.record_id,
    0,
    'erasure'
  );
END;

-- internal_replication_log is used only in a database that a host keeps on a
-- remote libSQL server (D268). Each row is the changeset of one committed
-- write: the row changes it made, in order, as JSON, so another host copy can bring its working copy up to date.
-- It holds earlier row values, so an erasure deletes the whole log in the same
-- transaction. On a local file it stays empty.
CREATE TABLE internal_replication_log (
  database_revision INTEGER PRIMARY KEY CHECK (database_revision >= 1),
  base_revision INTEGER NOT NULL CHECK (base_revision >= 0 AND base_revision < database_revision),
  changeset TEXT NOT NULL CHECK (json_valid(changeset)),
  committed_at TEXT NOT NULL
) STRICT;

CREATE TRIGGER internal_replication_log_erasure
AFTER INSERT ON record_erasure_markers
BEGIN
  DELETE FROM internal_replication_log;
END;

CREATE TABLE internal_write_receipts (
  principal_id TEXT NOT NULL,
  client_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  operation TEXT NOT NULL,
  request_digest TEXT NOT NULL,
  result_json TEXT NOT NULL CHECK (json_valid(result_json)),
  database_revision INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (principal_id, client_id, request_id)
) STRICT;

CREATE TABLE internal_access_state (
  singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
  access_epoch INTEGER NOT NULL CHECK (access_epoch >= 0)
) STRICT;

INSERT INTO internal_access_state (singleton_id, access_epoch) VALUES (1, 0);

UPDATE database_metadata
SET sql_schema_version = 7,
    athleticstandard_version = '2.1.3'
WHERE singleton_id = 1;
