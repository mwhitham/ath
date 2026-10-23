# Athletic Standard v1.1.0 — Package Release Specification

Status: shipped in [pull request #25](https://github.com/mwhitham/ath/pull/25) (npm
`athleticstandard@1.1.0`, merged 2026-10-23).

This plan does not change `athleticstandard_version`, `sql_schema_version`, or the
generated JSON Schema under `schema/1.0.0/`. It ships tooling and documentation fixes
on top of the approved [v1.0.0 SQLite build plan](../v1.0.0/spec.md).

Decisions for this release are D130 onward in [decisions.md](decisions.md). D124 in
v1.0.0 already required preview, backup, and confirmation for SQL migrations; this
release implements contribution-readiness step CR4.

## 0. Why this version exists

v1.0.0 landed the SQLite record, provider importers, and the contribution-readiness
plan. Before the remaining CR steps, four verification gaps and one safety bug
needed a published package fix without a format bump:

1. **CR4 migrate** — SQL schema upgrades need a verified backup and explicit
   confirmation; non-interactive opens must not migrate silently.
2. **Provider import vs `ath check`** — Apple Health, WHOOP, and Oura imports could
   pass the importer yet fail integrity check because typed-row JSON and revision JSON
   used different key order.
3. **Structured `ath log`** — Unknown envelope fields were dropped and `extensions`
   never reached storage.
4. **Public docs** — README, SPEC, and the Skill format reference still described 23
   record types and unchunked series file names while the schema and exporter already
   used 24 types and numbered chunks.
5. **Default model path** — `ath models --default` could target the live database when
   the default path was derived by swapping a `.json` suffix.

npm `1.0.0` was already published. The CLI, Skill metadata, and `tool_version` fields
now follow the package version (`src/version.ts` → `ATH_PACKAGE_VERSION` `1.1.0`).
`ath check` still reports Athletic Standard `1.0.0` for the record format
(`ATHLETIC_STANDARD_VERSION` in `src/v1/schema.ts`).

## 1. What ships in this version

| Area | Deliverable |
|---|---|
| Migrate | `ath migrate` with JSON preview, `--dry-run`, `--yes`, `--confirm-migration <database-id>`, and verified SQLite backup via `sqlite3` `.backup` |
| Open | Interactive TTY open prompts for migration after preview; non-interactive open throws `MigrationRequiredError` and points to `ath migrate` |
| Backup safety | Backup files at mode `0600`; refuse symlinks and existing paths; cleanup on failure; `sqlite3BackupDotCommand` escapes quotes and backslashes in paths |
| Provider import | `CurrentRecord.parse` before typed-row and revision writes so `checkDatabase` byte comparison succeeds |
| Structured log | Closed envelope; refuse unknown top-level keys; store valid `extensions`; reject invalid extensions |
| Docs | 24 record types; chunked series path `<source-id>-<quantity>-000001.ath-series.json` in SPEC, README, and `skill/references/format.md` |
| Default model | `defaultModelPath` maps `athlete.ath.sqlite` → `athlete.ath.model`; `writeDefaultModel` refuses database and portable JSON paths, symlinks, and realpaths equal to the database; write via temp file and `rename` |
| Tests | `tests/v1-migrate.test.ts`, `tests/v1-docs.test.ts`, `tests/models.test.ts`, provider-import check coverage, structured-log tests |
| Package | `package.json` version `1.1.0`; exports `ATH_PACKAGE_VERSION` from `src/index.ts` |

Record format, SQL schema version (`SQL_SCHEMA_VERSION` `1`), and portable JSON layout
are unchanged from v1.0.0.

## 2. `ath migrate` behavior

### 2.1 Preview

`inspectDatabase` opens read-only without enabling WAL (`deferWal`). The preview
includes:

- `source_sql_schema_version` and `target_sql_schema_version` (target is
  `SQL_SCHEMA_VERSION` from `src/db/client.ts`, currently `1`)
- `database_id`
- `planned_steps` from `MIGRATION_CHAIN` in `src/db/sql-contract.ts`
- `planned_backup_path` — `defaultMigrationBackupPath` →
  `<database>.migration-backup.<timestamp>.sqlite`
- `compatibility_effect` text

`--json` and `--dry-run` never apply a migration. Without `--yes`, the command prints
the preview and exits without writing.

### 2.2 Confirmation and backup order

Interactive flows ask whether to proceed **before** creating a backup:

- `ath migrate` with `--yes` calls `confirmWrite` with
  `Create a verified backup and apply this migration?` before
  `executeConfirmedDatabaseMigration`.
- Interactive open uses `Apply this SQL schema migration?` before
  `createVerifiedSqliteBackup`.

Answering no prints `nothing migrated` (migrate) or fails with a message to run
`ath migrate` (open). No backup file is created.

Non-interactive migration after `--yes` requires a second confirmation: pass
`--confirm-migration` with the exact `database_id`, or type it at the prompt when
stdin and stdout are TTYs.

### 2.3 Verified backup and apply

`createVerifiedSqliteBackup` in `src/db/backup.ts`:

- Requires the `sqlite3` CLI (SQLite 3.45+ per `MINIMUM_SQLITE_VERSION`)
- Creates the backup path exclusively (`wx`, mode `0600`)
- Runs `.backup` with `sqlite3BackupDotCommand` quoting
- Verifies record count and metadata against the source

Confirmed execution passes the previewed `planned_backup_path` into
`executeConfirmedDatabaseMigration` so the on-disk backup matches what the user
reviewed. WAL is enabled after backup and before `applyPendingDatabaseMigrations`.

On this release, `MIGRATION_CHAIN` contains only the initial schema (version `1`).
Databases already at `sql_schema_version` `1` report `migration_needed: false`. The
command surface exists so later SQL migrations can ship without another CLI redesign.

### 2.4 Non-interactive open

`openDatabase` with default options refuses outdated schema and throws
`MigrationRequiredError` whose message includes `Run ath migrate`. Agents and scripts
must call `ath migrate` explicitly with `--yes` and `--confirm-migration`.

## 3. Provider import and integrity check

`checkDatabase` compares each typed table's `data_json` to the current revision's
`record_json` as strings (`src/db/check.ts`).

Provider import now builds each new or changed record with `CurrentRecord.parse` in
`insertNewRecord` and `insertChangedRecord` (`src/db/provider-import.ts`) before
`insertRevision` and `upsertTypedRecord`, matching the library write path.

Fixture imports for Apple Health, WHOOP, and Oura pass `checkDatabase` in
`tests/v1-provider-import.test.ts`.

## 4. Structured `ath log`

`planStructured` in `src/db/log-entry.ts` accepts only:

`record_type`, `record_id`, `created_at`, `current_revision`, `is_deleted`, `data`,
`extensions`.

Any other top-level key produces an error listing the unknown fields. Valid
`extensions` are stored; invalid extensions (including `source_id` on an athlete
report) error.

`tool_version` on tool actors uses `ATH_PACKAGE_VERSION`.

## 5. Documentation alignment

No export format change. The exporter already wrote chunked series files; docs now
state:

- **24** record types (`RecordType` in `src/v1/schema.ts`)
- Series chunk file name pattern `<source-id>-<quantity>-000001.ath-series.json`

`tests/v1-docs.test.ts` guards SPEC, README, and `skill/references/format.md` against
regression to 23 types or unchunked series names.

## 6. Default model file

`MODEL_SUFFIX` is `.ath.model` (`src/models.ts`).

`defaultModelPath("athlete.ath.sqlite")` → `athlete.ath.model` (same directory stem,
not a `.json` swap).

`writeDefaultModel` refuses when the target path is the database path, ends with
`.ath.sqlite` or `.ath.json`, is a symlink, or `realpathSync` equals the database
realpath. Contents are written to a pid-random temp file beside the model, then
`renameSync` replaces the destination without following a symlink.

`readDefaultModel` reads the first line of the model file if present.

## 7. Version fields

| Field | Value on 1.1.0 | Where |
|---|---|---|
| npm / CLI | `1.1.0` | `package.json`, `ATH_PACKAGE_VERSION`, `ath --version` |
| Record format | `1.0.0` | `ATHLETIC_STANDARD_VERSION`, `ath check` metadata |
| SQL schema | `1` | `database_metadata.sql_schema_version`, `PRAGMA user_version` |
| `tool_version` / prediction agent version | `1.1.0` | provider import, structured log, grade, predict paths using `ATH_PACKAGE_VERSION` |

`CORE_API_VERSION` remains decided (D128) but not implemented (CR8 pending).

## 8. Known open items (not fixed in 1.1.0)

### 8.1 Oura member-portal CSV layout ([#26](https://github.com/mwhitham/ath/issues/26))

The importer matches Oura fixtures with flattened comma columns (`readiness_score`,
`contributor_*`). Oura's current membership-hub export uses semicolon CSVs per API
model, a `score` column, and JSON object cells for contributors and SpO2. Readiness,
sleep score, contributors, and nightly SpO2 from that export can be silently omitted
(rows have a `day` so they are not counted as skips). Fixing detection, parsing, or
skip reporting is future work.

### 8.2 Databases provider-imported with npm 1.0.0 may stay INVALID after `ath migrate`

The 1.0.0 package could write mismatched `data_json` and `record_json` on provider
import. `ath migrate` only upgrades SQL schema; it does not rewrite existing record
payloads. `ath check` can still report INVALID after a successful no-op or future SQL
migration.

Documented repair:

1. `ath export <directory>` — complete portable package
2. `ath init --empty` (with `-y` when non-interactive) in a new or empty folder
3. `ath import <directory>` with preview, then `--yes`

Re-import applies the 1.1.0 canonical write path. This path is not automated.

## 9. Build verification

Pull request #25 required:

`pnpm check:public-data && pnpm typecheck && pnpm test && pnpm build && pnpm check:schema-drift`

Docs regression: `tests/v1-docs.test.ts`.

Contribution-readiness steps CR5–CR9 and v1.0.0 release steps 10–12 remain as tracked
in [v1.0.0/progress.md](../v1.0.0/progress.md).
