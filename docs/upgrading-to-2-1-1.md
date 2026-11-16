# Upgrading to 2.1.1

Version 2.1.1 replaces the program, not the record. No record field changes. The
`ath` command is one native executable, built from one Rust library. This page
lists what changes for a database, the command line, an npm installation, and
JavaScript callers. Every deliberate behavior difference from 2.1.0 is listed, with
the test that proves it, in
[deliberate changes](../build-history/v2.1.1/appendix/deliberate-changes.md) and
[command-line changes](../build-history/v2.1.1/appendix/cli-changes.md).

## Database

A 2.1.0 database is at SQL schema step 4. The 2.1.1 program writes step 5, which
records version 2.1.1 and adds one internal table, `internal_pending_file_operations`.
The table holds recovery state for attachment files. Step 5 changes no athlete-owned
table, record, or revision, and adds no `field_basis` to older records.

Run the migration once:

```sh
ath migrate --json          # preview the plan
ath migrate                 # confirm in a terminal
```

Without a terminal, pass `--yes --confirm-migration <database-id>` after reviewing the
preview. Read-only commands open a 2.1.0 database unchanged. A command that writes
refuses an outdated database and points to `ath migrate`.

The migration makes a verified backup with SQLite's own backup interface first. It no
longer needs a `sqlite3` program. All steps run in one transaction, so a failure
leaves the database as it was and reports the backup's location.

To go back to 2.1.0, restore the verified backup and keep the migrated file
separately. The backup does not contain work recorded after it was made. Version
2.1.0 refuses a database at step 5; it never writes to it.

Databases from 1.0.0 and 2.0.0 upgrade the same way. Packages from 2.0.0 and 2.1.0
still import.

Restoring a package into a new, empty database is faster and uses about half the disk.
While that restore runs, other commands cannot read the file. They wait up to 5 seconds,
then report `database is locked`. Run them again when the import finishes. If a restore
is stopped partway, the next command that opens the file undoes it, and the database is
empty again.

## Command line

Commands, arguments, flags, defaults, confirmations, exit statuses, and JSON fields
keep their 2.1.0 meaning. The visible differences are:

- `--version` prints `2.1.1`, and new records name tool version 2.1.1.
- A refusal is printed once.
- `ath init` without a terminal refuses instead of stopping with status 0.
- `ath migrate --help` no longer asks for `sqlite3`.
- `ath share` finds the report `ath backtest` writes.

## Installation

The npm package `athleticstandard` holds a prebuilt `ath` for Linux x64, macOS
arm64, and Windows x64. Installing it compiles and downloads nothing. On another
system the launcher stops with an installation error that names the system.

`ath --update` still updates an npm, pnpm, or bun global installation through the
same package manager. The native program returns the checked install plan, and the
launcher runs it after the native process exits. On Windows this lets the package
manager replace the executable.

The native executable is also published as a release archive. An archive copy needs
no Node.js. `ath --update` from an archive copy prints the steps to replace it.

## JavaScript

The root, `/database`, `/schema`, `/json-schema`, the manifest and series schemas,
and `/skill` exports keep every 2.1.0 name and signature.

- The `/schema` exports are still Zod schemas. Parsing, composition, and inference
  run in the calling process, as before. They are frozen 2.1.x compatibility code,
  tested against the native validator.
- The `/database` functions call the native executable for each operation, through
  a versioned interface. There is no JavaScript database implementation and no raw
  SQL export.
- A remote libSQL target, such as `libsql://…`, is still parsed as remote and is
  refused as 2.1.0 refused it. 2.1.1 opens no network connection.
  [Remote targets](../packages/npm/REMOTE-TARGETS.md) lists each export's result.
- Validation errors are still `ZodError` objects with the 2.1.0 paths and messages.

The npm-client items in the deliberate changes list (62 onward) describe the smaller
differences, such as when a security warning is reported.
