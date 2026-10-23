# Athletic Standard v1.1.0 — Decision Log

Decisions for this package release continue after D129 in
[v1.0.0](../v1.0.0/decisions.md). The record format version stays `1.0.0`. Rejected
alternatives are recorded because they are the part everyone forgets.

## D130. Ship npm 1.1.0 without bumping the 1.0.0 record format

`athleticstandard@1.1.0` publishes migrate, import-check, structured-log, docs, and
default-model fixes. `athleticstandard_version`, `sql_schema_version`, and generated
JSON Schema paths under `schema/1.0.0/` stay on `1.0.0` / `1`.

`ATH_PACKAGE_VERSION` in `src/version.ts` is the single source for the CLI
`--version`, Skill metadata, and `tool_version` on tool actors. `ATHLETIC_STANDARD_VERSION`
in `src/v1/schema.ts` remains what `ath check` reports for the athlete record format.

Rejected alternatives:

- **Bump the record format to 1.1.0 for a tooling release.** Rejected because none of
  the shipped changes alter normative Zod, SQL encoding, or portable JSON layout.
- **Hold npm at 1.0.0 after publish.** Rejected because users already have `1.0.0` on
  npm and need a distinct package version for these fixes.

## D131. Provider import parses `CurrentRecord` before typed-row and revision writes

`checkDatabase` compares typed `data_json` to the current revision's `record_json` as
exact strings. Provider import previously wrote importer key order in one store and
`RecordRevision` normalization in the other, so a clean Apple Health, WHOOP, or Oura
import could still fail `ath check`.

New and changed provider rows now call `CurrentRecord.parse` in `insertNewRecord` and
`insertChangedRecord` before `insertRevision` and `upsertTypedRecord`, matching the
library write path.

Rejected alternatives:

- **Relax `checkDatabase` to compare parsed objects instead of strings.** Rejected
  because byte-identical copies are an explicit integrity rule (D103).
- **Normalize only the revision copy.** Rejected because the typed table would still
  diverge from the revision athletes and exports rely on.

## D132. Structured `ath log` uses a closed envelope with validated extensions

`planStructured` accepts only `record_type`, `record_id`, `created_at`,
`current_revision`, `is_deleted`, `data`, and `extensions`. Any other top-level field
errors with the field names listed. Valid `extensions` are stored on write. Invalid
extensions, including forbidden `source_id` on athlete reports, error instead of
silently dropping data.

Rejected alternatives:

- **Ignore unknown envelope fields.** Rejected because agents could believe data was
  stored when it was discarded.
- **Accept extensions without schema validation.** Rejected because invalid extension
  shapes would corrupt the extension envelope contract (D118).

## D133. Public docs match the implemented schema and export layout without a format bump

SPEC.md, README.md, and `skill/references/format.md` now state **24** record types and
chunked series files named
`<source-id>-<quantity>-000001.ath-series.json`. The schema and exporter already
behaved that way; only prose was wrong.

`tests/v1-docs.test.ts` guards against regressions to 23 types or unchunked series
names.

Rejected alternatives:

- **Change export to match the old doc string.** Rejected because deterministic export
  and existing packages already use numbered chunks.
- **Leave docs wrong until a format version bump.** Rejected because misleading public
  reference text is worse than a doc-only fix on a patch package.

## D134. The default model file is `athlete.ath.model` beside the database, with refused unsafe targets

`defaultModelPath` maps `*.ath.sqlite` to `*.ath.model` in the same directory. The
previous `.json` suffix swap could resolve to the live database path for typical
layouts.

`writeDefaultModel` refuses paths equal to the database, ending in `.ath.sqlite` or
`.ath.json`, symbolic links, or a realpath equal to the database realpath. It writes
through a private temp file in the same directory and `rename`s into place so the
write does not follow a symlink at the destination.

Rejected alternatives:

- **Keep the `.json` swap with a runtime warning.** Rejected because a mistaken default
  overwrites the athlete record.
- **Write directly to the model path.** Rejected because replacing an existing symlink
  could follow it to the database.

## D135. Migration backups escape sqlite3 paths and reuse the previewed backup location

`sqlite3BackupDotCommand` double-quotes the backup path and applies sqlite3 backslash
escapes for `\`, `"`, and newlines so `.backup` writes to the intended file when paths
contain quotes or backslashes.

`ath migrate` passes `preview.planned_backup_path` into
`executeConfirmedDatabaseMigration` as `backupPath` so the file created matches the
preview. Backup files are created exclusively at mode `0600`; symlinks and existing
paths are refused; partial files are removed on failure.

Rejected alternatives:

- **Pass unescaped paths to sqlite3.** Rejected because special characters break or
  redirect the backup command.
- **Generate a new backup path at execution time.** Rejected because the user confirms
  a specific path from the preview (D124).

## D136. Interactive migration asks before creating a backup

On `ath migrate` with `--yes`, and on interactive database open when migration is
required, the tool asks for confirmation before `createVerifiedSqliteBackup` runs.
Declining leaves no backup file and does not modify the database.

Non-interactive migration still requires `--yes` and `--confirm-migration <database-id>`
after reviewing the JSON preview. Non-interactive open still refuses to migrate and
points to `ath migrate`.

Rejected alternatives:

- **Create the backup before the prompt.** Rejected because declining would leave backup
  files the user did not want.
- **Skip the prompt when `--yes` is set.** Rejected because `--yes` must not bypass the
  migration-specific confirmation (D68 applies to write previews, not silent schema
  upgrades).
