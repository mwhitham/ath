# Architecture

Athletic Standard is one Rust library and one `ath` executable. This document
names who owns each job and follows two ordinary operations from input to the
database file. It describes the current code. Read `build-history/` for why a
choice was made; read this to find where to change something.

Paths below are relative to the repository root.

## Owners

| Job | Owner | Depends on |
|---|---|---|
| Record shapes, validation, JSON Schema | `src/model/` | `src/json.rs` |
| JSON text with JavaScript's rules | `src/json.rs` | nothing |
| JavaScript text rules (`trim`, `Number()`) | `src/text.rs` | nothing |
| Private files, staging, durable writes, attachment files | `src/files/` | `src/error.rs` |
| SQLite connections, the one record encoder, writes, reads, migrations, backup, check, repair, stats | `src/store/` | model, files |
| Provider imports: detection, readers, matching, staging, preview, apply | `src/import/` | model, store, files |
| Complete JSON packages, conflicts, attachments, erasure | `src/package/` | model, store, files |
| Calculations and their receipts | `src/calculate/` | model, store |
| Series and record views, calendar ranges | `src/view/` | model, store |
| Prediction evidence, grading, backtest | `src/prediction/` | model, store, calculate |
| `ath log` planning | `src/log.rs` | model, store |
| Optional network and system services: password store, model gateways, updates, the Garmin pull | `src/runtime/` | model, import |
| Shipped skills and mappings built into the program | `src/assets/` | files |
| Limits on untrusted input | `src/input_policy.rs` | nothing |
| Arguments, help, prompts, rendering | `src/cli/` | everything above |
| The npm launcher, JavaScript client, and Zod compatibility facade | `packages/npm/` | the `ath` executable |

Rules that keep this table true:

- `src/model/` has no database, terminal, network, or process dependency.
- `src/store/` never prompts and never calls a model.
- Readers in `src/import/` produce typed candidates. They never write SQL.
- `src/cli/` holds no record-to-table mapping, matching rule, grading
  arithmetic, or history rule. It parses, calls one library operation, and
  renders the result.
- Local record commands never start a network client or a credential store.

## The one record encoder

A record's current state lives in three places that must agree: the complete
JSON in `record_revisions`, a typed row in the record type's table, and ordered
child rows for its collections. `src/store/encode.rs` is the only code that maps
a record onto those rows. `typed_columns` and `child_rows` describe the mapping
as data. `write_current_state` writes it. `ath check` compares the same
description with what is stored, and `ath repair` rebuilds rows from it. A new
field is mapped once, in `encode.rs`, or not at all if it lives only in the JSON.

## One correction, from input to disk

`ath correct <record-id> --revision 3 --data '{...}' --yes`

1. `src/cli/parse.rs` reads the arguments from the command table in
   `src/cli/spec.rs`. `src/cli/commands/edit.rs` finds the database
   (`Context::find_database`) and opens it for writing
   (`Context::open_for_write`), which refuses an outdated database or offers
   `ath migrate` in a terminal, and finishes any pending attachment cleanup.
2. The command reads the current record (`Store::get_record`) and parses
   `--data` with the record type's shape: `RecordData::decode` in
   `src/model/record.rs`. A bad field is refused here with the published
   message, such as `data.end_at: Workout session end cannot precede its start`.
3. The command renders the preview and asks for confirmation, unless `--yes`.
   `--json` prints the preview and writes nothing.
4. The command builds one `Correction` and hands it to
   `package::apply_with_attachments`, which calls `Store::apply`
   (`src/store/write.rs`) directly when no `--attachment` was given. With
   attachments, it first writes and synchronizes the files and records them as
   pending work, so the transaction never names a missing file. In one
   immediate SQLite transaction the store checks
   that the record is still at revision 3, validates the complete new current
   record, inserts revision 4 into `record_revisions`, writes the typed row and
   child rows through `encode::write_current_state`, moves `records` to
   revision 4, checks prediction links and grades
   (`validate_relationships`), and advances the database revision once.
   Foreign keys are checked at commit. SQLite triggers still refuse a broken
   revision chain or a source claim on an athlete report even if this code
   were bypassed.
5. The file stays owner-only, and the command prints the result.

## One provider import, from input to disk

`ath import ./whoop-export --yes`

1. `src/cli/commands/write.rs` opens the database for writing and calls
   `import::preview` (`src/import/provider.rs`). No write lock is held while
   the input is read.
2. `src/import/detect.rs` classifies the input: a package, a provider export,
   or a single file. `src/import/input.rs` reads folders and zip archives under
   the limits in `src/input_policy.rs`, measuring expanded bytes as it reads.
3. The registered reader, here `src/import/whoop.rs`, streams rows and sends a
   typed `Candidate` for each record to the `CandidateSink`: complete record
   data with its time and value receipts, a match key, and a section. It names
   sources through the sink, so a measured record always has a source. It
   reports each skipped row with a reason and an example.
4. The sink validates each candidate as a complete current record and stages
   it in a private temporary SQLite file (`src/import/staging.rs`), so a large
   export never sits in memory.
5. Matching compares each staged candidate with the live database by import
   key: new, matching, changed, or conflict. A changed record in an ambiguous
   group is a conflict, never a guess. Two devices that describe one workout
   stay two source records.
6. The preview records the database revision it was built from. The command
   renders it, or prints it with `--json`.
7. With confirmation, `ProviderPreview::apply` opens one write transaction,
   refuses a stale plan if the database revision moved, writes new sources,
   new records, and corrections through `store::write::write_in` in bounded
   batches, inserts import keys, and advances the database revision once. A
   plan with nothing to write leaves the revision unchanged. The staging file
   is removed when the preview is dropped.

## Packages and files

`src/package/` exports one consistent read snapshot to a private staging folder
and publishes it by rename, with the manifest written last. Restore and merge
write through the same encoder. SQLite cannot commit file changes, so
attachment writes and erasure record their file work in the internal table
`internal_pending_file_operations` (migration 005). A later write command
finishes or cleans up that work; `ath check` reports it.

## Contracts outside the code

- `spec/sql/migrations/*.sql` are the executable migrations, embedded unchanged.
  `spec/sql/contract.json` describes what direct-SQL clients may rely on.
- `schema/<version>/` holds the published JSON Schema files, generated from
  `src/model/` (`src/model/json_schema.rs`).
- `skills/` and `mappings/` are built into the program by `build.rs`.
- `tests/vectors/` and `tests/golden/` hold the recorded 2.1.0 behavior that
  the native code is compared with. `tools/reference/` holds the generators
  that recorded them from the reference program.
- `tests/generated.rs` is the one generation entry point. It builds the JSON
  Schema files, the npm protocol module, the skill's command reference, and the
  demo database in `examples/demo-athlete/`, and fails when a committed copy
  differs. `ATH_WRITE_GENERATED=1 cargo test --test generated` writes them.

## Adding things

- A new optional field on a record: declare it in `src/model/data.rs`, add a
  typed column in `src/store/encode.rs` only if direct SQL needs it, then run
  `ATH_WRITE_GENERATED=1 cargo test --test generated`. Nothing else maps fields.
- A new provider: write a reader that implements `ProviderReader` and a
  detection rule (`src/import/README.md`). It needs no SQL and no command code.
- A new command: add it to the table in `src/cli/spec.rs` and a handler in
  `src/cli/commands/` that calls a library operation.
