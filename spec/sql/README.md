# Athletic Standard SQL contract

Language-neutral SQLite schema for Athletic Standard v1. The TypeScript migration in
`src/db/migrations/` is the source of truth. `pnpm generate:sql-contract` writes the
files in this directory.

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

Write transactions should also use `PRAGMA defer_foreign_keys = ON` when inserting
linked rows in one transaction.

## Migrations

| Order | File | `sql_schema_version` after apply |
|---|---|---|
| 1 | `migrations/001-initial.sql` | 1 |

Rules:

- Run each migration in order inside a single transaction when possible.
- `database_metadata` is the version table. Do not add a second version source.
- After `001-initial.sql`, the runner inserts one `database_metadata` row and one
  `database_state` row. The runner supplies `database_id` and `created_at`. They are
  not embedded in the SQL file.
- On local files, set `PRAGMA user_version` to match `sql_schema_version`.
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

Typical new-record order inside one transaction:

1. `INSERT INTO records`
2. `INSERT INTO record_revisions`
3. Insert or upsert the typed table for `record_type` and any ordered child rows
4. `UPDATE database_state SET current_revision = current_revision + 1`

Corrections insert a new `record_revisions` row, update `records.current_revision`,
refresh typed rows, then bump `database_state`. Soft delete inserts
`record_deletions`, sets `records.is_deleted = 1`, then bumps `database_state`.

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
  `migrations/001-initial.sql`.
