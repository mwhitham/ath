# Athletic Standard SQL contract

Language-neutral SQLite schema for Athletic Standard v1. **Runtime and Rust embed the
plain SQL files in this directory** (`migrations/*.sql`). The TypeScript modules in
`src/db/migrations/` remain the authoring source until the Rust Records stage; run
`pnpm generate:sql-contract` after editing those modules so published SQL stays in sync.
`pnpm check:schema-drift` fails when the files here diverge from a freshly created database.

## Versions

| Field | Meaning |
|---|---|
| `athleticstandard_version` | Record and JSON package format (see `src/v1/schema.ts`). |
| `sql_schema_version` | SQLite encoding in `database_metadata.sql_schema_version`. |
| `PRAGMA user_version` | Local mirror of `sql_schema_version` on file-backed databases. |

These versions are independent. See `contract.json` for the current `sql_schema_version`.

## Minimum SQLite

Use SQLite **3.45.0** or newer with `STRICT` table support.

## Required connection pragmas

Every client must enable:

```sql
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
```

## File header pragmas

After the initial migration and metadata insert on a **local file**, the runner must
set:

```sql
PRAGMA application_id = 0x41544831;
PRAGMA user_version = 1;
```

`application_id` marks the file as Athletic Standard (`ATH1`). `user_version` must
match `database_metadata.sql_schema_version`. See `contract.json` for the current
version number.

## Write transactions

SQLite resets `defer_foreign_keys` after each `COMMIT` or `ROLLBACK`. Do not set it
once per connection and assume it persists.

At the start of **every** write transaction:

```sql
BEGIN IMMEDIATE;
PRAGMA foreign_keys = ON;
PRAGMA defer_foreign_keys = ON;
```

Then run the record steps below and `COMMIT`. On failure, `ROLLBACK`.

## Migrations

| Order | File | `sql_schema_version` after apply |
|---|---|---|
| 1 | `migrations/001-initial.sql` | 1 |

Rules:

- Run each migration in order inside a single transaction when possible.
- `database_metadata` is the version table. Do not add a second version source.
- After `001-initial.sql`, the runner inserts one `database_metadata` row and one
  `database_state` row, then sets the file header pragmas above. The runner supplies
  `database_id` and `created_at`. They are not embedded in the SQL file.
- Roll back the transaction on failure. A failed migration must leave the prior file
  usable.

Forward upgrades from earlier published SQL versions will be added here when they
exist. v1.0.0 ships only schema version 1.

## Stable read views

View names use the `sql_v1_` prefix for `sql_schema_version` 1. Column lists and
descriptions are in `contract.json`. Prefer these views over ad hoc joins.

| View | Role |
|---|---|
| `sql_v1_database_info` | Identity, format versions, monotonic `database_revision`. |
| `sql_v1_current_records` | Current revision for every record, including soft-deleted. |
| `sql_v1_active_records` | Current revision for records with `is_deleted = 0`. |
| `sql_v1_record_revisions` | Full revision history. |
| `sql_v1_record_deletions` | Soft-deletion audit rows. |

New SQL schema versions may add parallel `sql_vN_*` views. Retiring a view name
requires a documented migration policy change.

## Stable writable tables

Direct writers may insert and update only the tables listed as
`stable_writable_tables` in `contract.json`. Every other table is internal encoding.
Internal tables may change in a future ordered migration without a format-version
bump when they are not listed as stable.

### New record (inside one write transaction)

1. `write_transaction_start` (see above)
2. `INSERT INTO records`
3. `INSERT INTO record_revisions`
4. Insert or upsert the typed table for `record_type`
5. Insert child collection rows from the full payload
6. `UPDATE database_state SET current_revision = current_revision + 1`
7. `COMMIT`

### Correction

1. `write_transaction_start`
2. `INSERT INTO record_revisions` with `revision = current_revision + 1`
3. `UPDATE records SET current_revision`
4. Upsert the typed table for the new payload
5. **Replace child collections** (see below)
6. `UPDATE database_state SET current_revision = current_revision + 1`
7. `COMMIT`

### Child collection replacement

On every create and correction, ordered child tables and link tables are **replaced**
from the full logical payload:

1. `DELETE` all child rows whose parent column equals the record's `record_id`.
2. `INSERT` the child rows in order from the payload.

If the payload omits a child that existed before, the `DELETE` removes it. An empty
collection in the payload means delete all prior child rows and insert none.

Direct writers may use these `DELETE` statements during a correction even though
other deletes are uncommon. The tables and parent columns are listed in
`contract.json` under `collection_replacement`.

Examples: `provider_reporting_periods` for summaries with provider periods,
`authored_record_evidence` for authored records, `record_attachment_links` for
reports and nutrition entries.

### Soft delete

1. `write_transaction_start`
2. `INSERT INTO record_deletions`
3. `UPDATE records SET is_deleted = 1`
4. `UPDATE database_state SET current_revision = current_revision + 1`
5. `COMMIT`

`database_metadata` is written once at creation. `record_import_keys` and
`record_erasure_markers` are reserved for official import and privacy-erasure flows.

## Enforcement

Constraints, foreign keys, and triggers in the migration enforce the two-tier wall,
revision order, deletion state, and protected links (D116). Trigger names are listed
in `contract.json`. Invalid direct writes must fail in SQLite even when the Node
library is not used.

## Drift check

`pnpm check:schema-drift` verifies that:

- Generated files match the TypeScript migration source.
- A database from `createDatabase()` matches a database built with `sqlite3` and
  `migrations/001-initial.sql`, including `sqlite_master` and file header pragmas.
