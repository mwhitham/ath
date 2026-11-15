# v2.1.1 rebuild decisions

The complete native rebuild targets version 2.1.1. The specification is
[spec.md](spec.md). Numbers continue after the unapproved D198–D207 proposals in
[the earlier draft](../v3.0.0/decisions.md). That draft remains in build history.

## D208. The complete native rebuild ships as 2.1.1

The program, npm package, record package, and database metadata use 2.1.1 for this
rebuild. The selected version replaces the unapproved 3.0.0 proposal. D152's single
version rule remains in force.

The release still requires the complete Rust library and native command. It must
replace every working command before acceptance. It must remove duplicate local
writers, legacy import intermediates, and the TypeScript command implementation.
The compatibility, recovery, cross-platform, and performance requirements remain.

This supersedes D174's TypeScript front end and the remaining implementation split
described in D155 and D162. The implemented overlap is the new evidence, documented
in spec section 2. The implementation is replaced without redesigning the public
athlete record.

Rejected alternatives:

- Keep the proposed 3.0.0 version. Rejected because 2.1.1 is the selected release.
- Reduce the rebuild to a smaller cleanup. Rejected because the complete native
  replacement is still the selected goal.
- Ship an incomplete native command and finish the rest later. Rejected because
  current workflows require a complete replacement.

## D209. The patch preserves existing public contracts

Version 2.1.1 retains public commands, arguments, confirmations, JavaScript exports,
Zod schema behavior, record fields, and the direct-SQL contract. The draft's
proposed API removals and calculation-receipt additions are not selected.

Preserve existing remote-target behavior and existing unsupported-operation
refusals. The rebuild adds no remote capability. Every current public export maps
to its retained implementation and a compatibility test.

The npm package retains genuine in-process Zod objects as versioned compatibility
code. Their existing composition, inference, parsing, and error behavior remain.
The native implementation validates in Rust and never calls this compatibility
code. Tests compare the frozen public schema behavior with the Rust checks. This
preserves existing callers without a JavaScript implementation of local storage.

Rejected alternatives:

- Apply the proposed major-version API removals under a patch number. Rejected
  because existing callers must continue to work.
- Replace synchronous schema parsing with a child-process call. Rejected because
  that would change both behavior and ordinary validation performance.
- Retain the TypeScript local database implementation for compatibility. Rejected
  because public adapters can call the new library without duplicating its rules.

## D210. Rust owns the native record lifecycle

Rust types and semantic checks own native validation. One encoder maintains typed
rows, complete payloads, child collections, and relationships. Creation, correction,
imports, restore, and repair share it while keeping their distinct history rules.
Bundled SQLite through rusqlite owns local operations and verified backups.

This replaces D83's native dependence on Zod and D93's local libSQL adapter.
The npm compatibility requirement in D209 is explicit. D85, D103, D104, D116,
D118, D124, and D125's record and database protections remain in force.

The native executable needs no Node runtime, external SQLite executable, compiler,
or service for local use. npm continues to install the same tested executable and
provides adapters for existing JavaScript callers.

Rejected alternatives:

- Keep several encoders with the same field knowledge. Rejected because the
  rebuild must remove the implemented duplication.
- Replace public typed tables with one generic JSON table. Rejected because that
  would break direct writers and the indexed query contract.
- Remove source separation or revision checks to simplify the port. Rejected
  because the record guarantees remain required.

## D211. Migration changes metadata and internal state without changing athlete fields

Published migrations 001–004 remain byte-identical. Migration 005 records 2.1.1
and any internal file-operation recovery state required by the specification.
It does not change existing athlete-owned fields, record IDs, or revisions.
It does not backfill field_basis or collapse sources.

Keep preview, verified backups, explicit confirmation, and rollback behavior.
Use the in-process SQLite backup API. New creation runs the same ordered migration
chain. The published SQL files become the executable migration source.

This retains D165's protection of published migrations. It does not change the
meaning of the integer SQL migration step. Retained revisions keep their original
contents and authorship, and existing supported packages remain importable.

Rejected alternatives:

- Rewrite earlier SQL files. Rejected because they are published contracts.
- Skip migration safety because this release has a patch number. Rejected because
  metadata and internal storage changes still need a verified backup.
- Redesign athlete-owned fields during the rebuild. Rejected because the selected
  work is an implementation replacement with compatible public behavior.

## D212. Each record shape is declared once and matches the published validator

Each record type, part, and enum is declared once in `native/src/model/`. That
one declaration produces the Rust type, the structural validator, and the JSON
Schema. Refinements that JSON Schema cannot express run as checks attached to
the same declaration. Validation reports every issue with the path, code, and
wording the 2.1.0 validator used, in the same order.

Errors are part of what a person and an agent read. Matching them means a
correction that worked against 2.1.0 still works. The 7,399 recorded 2.1.0
validation cases are the test: all 7,285 comparable cases match exactly. The
generated JSON Schema files match the published 2.1.0 files byte for byte apart
from the version list and schema IDs, which shows the structure is unchanged.

Rejected alternatives:

- Derive validation from Serde alone. Rejected because Serde stops at the first
  error, accepts `null` for optional fields, and words errors differently.
- Generate JSON Schema with Schemars. Rejected because its layout differs from the
  published files, so agreement could only be argued, not compared byte for byte.
- Keep a separate hand-written schema next to the Rust types. Rejected because
  it repeats every field in two places (spec section 3, item 1).

## D213. Stored and exported JSON follows JavaScript's rules

The library writes JSON the way JavaScript's `JSON.stringify` does. Numbers
print as `Number.prototype.toString` prints them. Keys that look like array
indexes come first. Deterministic package files sort the other keys by UTF-16
code units. Replaying the demo history through the native writer reproduces
every row in all 40 athlete tables byte for byte.

Stored record JSON and package files were written by 2.1.0. Keeping the same
bytes keeps package hashes, direct-SQL comparisons, and conflict detection
stable across the rebuild.

Rejected alternatives:

- Use the Rust JSON library's own writer. Rejected because it prints `100.0`
  for 100 and keeps insertion order for index-like keys, which changes bytes.

## D214. A `__proto__` key is kept and checked

The native reader keeps a `__proto__` key in supplied fields and extensions like
any other key, and every rule applies to it. The 2.1.0 validator dropped such a
key and everything under it. That hid a nested `source_id` from the rule that
athlete-reported records cannot claim a source (the two-tier wall).

Rejected alternatives:

- Drop the key as 2.1.0 did. Rejected because a JavaScript object quirk must not
  open a gap in the two-tier wall.

## D215. Backtest replays cut off before the result was first known

A backtest replay now stops one millisecond before the earlier of the result's
event time and the time the database first learned of it. The same cutoff
filters event times and knowledge times. In 2.1.0 the cutoff followed the
result's latest revision, so a corrected result could show the model its own
first revision, and a result logged late let the model see later events.

This is the rule spec section 10 states. Regressions cover a corrected outcome
and a late import.

Rejected alternatives:

- Keep the 2.1.0 cutoff to match old reports. Rejected because a replay that
  can see its answer does not measure prediction.

## D216. The step-5 SQL contract waits until the cutover

The updated `spec/sql/contract.json` and `spec/sql/README.md` for SQL schema step
5 live in `native/pending-publication/` until the cutover. The 2.1.0 test suite
compares the published contract with its own generator. That suite must pass
on every commit while the old implementation remains (AGENTS.md). Migration
`005-version-2-1-1.sql` is new and does not disturb that comparison, so it is
in `spec/sql/migrations/` already.

Rejected alternatives:

- Update the published contract now and let one old test fail. Rejected because
  every commit must pass the existing checks.
- Teach the 2.1.0 generator about step 5. Rejected because the 2.1.0 program
  cannot run step 5.

## D217. The rebuild runs as parallel tracks merged into one branch

Each part of the rebuild was built on its own branch from the shared model and
store, then merged into `build/native-2-1-1`. Every merge runs the full native
suite and the 2.1.0 suite. Behavior differences are listed one by one in
`native/reference/deliberate-changes.md` with the test that proves each.

Rejected alternatives:

- Build every part in one working tree. Rejected because one part that does not
  compile would stop work on every other part.

## D218. The reference evidence stays in build history

The inventories and comparison notes made from the 2.1.0 reference move from
`native/reference/` into [appendix/](appendix/) at the cutover. They are the
evidence for D212, D215, D217, and spec section 13. Tests that compare the
native program with 2.1.0 read them from there.

- [command-inventory.json](appendix/command-inventory.json): every 2.1.0
  command, option, default, help text, and parse-only probe.
- [command-behavior.md](appendix/command-behavior.md): what each command does
  after parsing, with the rule IDs the native code cites.
- [cli-changes.md](appendix/cli-changes.md): the deliberate command-line
  differences from 2.1.0.
- [deliberate-changes.md](appendix/deliberate-changes.md): every deliberate
  behavior difference, with the test that proves it (D217).
- [semantic-rules.md](appendix/semantic-rules.md): the record rules JSON Schema
  cannot express, which the Rust validator runs (D212).
- [public-api.json](appendix/public-api.json): every 2.1.0 JavaScript export
  that the npm package keeps (D209).
- [test-disposition.md](appendix/test-disposition.md): where each 2.1.0 test
  file's promises are checked after the rebuild (spec section 13.3).

Rejected alternatives:

- Keep the files under `native/`. Rejected because the cutover removes that
  folder (spec section 5).
- Delete them once the comparison passes. Rejected because they record why the
  native behavior is what it is.

## D219. One Cargo test generates and checks every generated file

`tests/generated.rs` builds the JSON Schema files, the npm protocol module, the
skill's command reference, and the demo database from the Rust sources.
`cargo test` fails when a committed copy differs. With
`ATH_WRITE_GENERATED=1` the same test writes the files. It refuses to write a
published historical schema, `spec/sql/`, build history, or `tests/fixtures/`.

The command reference is the help that `ath <command> --help` prints, rendered
from the command table. The demo database is written by the native store from
the fixed IDs, times, and records of the 2.1.0 demo, and a test requires every
athlete table to equal the 2.1.0 demo's. The 2.1.0 demo stays unchanged in
`tests/fixtures/version-2-1-0/`, because the comparison tests replay 2.1.0
behavior on it. A database is compared by its schema and rows when its bytes
differ, so another SQLite build's page layout does not fail the check.

Rejected alternatives:

- A separate generator program beside a check program. Rejected because two
  entry points can disagree about which files are generated.
- Render the reference from Clap's own help. Rejected because the program
  prints Commander-style help from the command table, and the reference must
  be the text a person sees.
- Regenerate the 2.1.0 demo in place. Rejected because tests that compare with
  2.1.0 need the database 2.1.0 wrote.

## D220. The npm package keeps its own npm lockfile

`packages/npm/` installs its dependencies with `npm ci` from its own
`package-lock.json`. Its type check and tests run from that folder. Release
packing copies the repository's committed schema and skill into the tarball,
so the repository holds one copy of each.

Rejected alternatives:

- Keep a root pnpm workspace. Rejected because the repository root is now the
  Rust package, and a Node workspace there would serve one folder.
- Use pnpm inside the package. Rejected because npm ships with Node, and the
  release jobs already use it to pack and install the tarball.
