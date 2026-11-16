# v2.1.1 rebuild decisions

The complete native rebuild targets version 2.1.1. The specification is
[spec.md](spec.md). Numbers continue after the unapproved D198–D207 proposals in
[the earlier draft](appendix/draft-2.1.1/decisions.md). That draft remains in build history.

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

## D221. A restore into a new database writes through a rollback journal

A restore into a new, empty database runs its one transaction with a rollback
journal (`journal_mode` DELETE). The file grows 8 MiB at a time during the
restore. Before the command ends, the file returns to WAL mode. A merge into a
database that has records, and a restore while another connection has the file
open, stay in WAL mode, because SQLite cannot leave WAL mode then.

The reason is speed and disk use. In WAL mode every new page is written to the
WAL, and the checkpoint reads it back and writes it again into the database. A
rollback journal saves only the old content of pages that already existed, and
a new database has almost none. At 3 million records the restore took 400.2
and 401.2 s with a rollback journal, against 452.3 and 490.8 s in WAL mode, and
peak disk fell from about twice the database to its size. Without this change
the 3-million-record import missed the spec section 14 target of half the
2.1.0 time. The measurements are in
[performance/report-2.1.1.md](../../performance/report-2.1.1.md).

The restore is still one atomic transaction with `synchronous` FULL. A failed
restore rolls back and leaves the empty database in WAL mode. A stopped
restore is undone the next time the file is opened (D222).

The cost is for readers. While the restore writes, no other connection can
read the file. A reader waits for the busy timeout of 5 s and then reports
"database is locked". In 2.1.0 a reader saw the empty database. This is
accepted because the database is new and empty: a reader learns nothing from
it during the restore, and the restore is usually run by the same person.
The finished file can also be up to one chunk longer than its pages. The extra
bytes are zeros that SQLite does not read; later writes use them, and `VACUUM`
removes them.

Rejected alternatives:

- Keep WAL mode and add the 8 MiB file growth. Rejected because it was slower
  in every paired run and used twice the disk.
- A larger page cache for the restore. Rejected because 64, 128, and 256 MiB
  caches were no faster in paired runs.
- `journal_mode` OFF or MEMORY. Rejected because a stopped process could then
  leave a half-written database.
- `synchronous` OFF or NORMAL for the restore. Rejected because a power loss
  could then lose or damage a restore that reported success.

## D222. A read-only open undoes a write that stopped partway

When a read-only open finds the journal of a rollback-journal write that
stopped partway, it opens the file for writing once. SQLite undoes the stopped
write, the file returns to WAL mode, and the read-only open continues on the
file as it was before the write. Undoing needs write access to the file and
its folder. Without it, the open is refused with a message that says why, and
nothing is changed.

This is an exception to the spec section 7 rule that read-only operations do
not change file state. The reason is that a journal left by a stopped write
can only be read by undoing it. A read-only SQLite connection cannot undo it,
and without this step `ath stats` and `ath check` called the file "not an
Athletic Standard database". The undo changes no committed data. It removes
only pages that were never committed, which is what the next write would do
anyway. A database in WAL mode never leaves such a journal. In 2.1.1 the
usual cause is a stopped restore into a new database (D221). A file kept in
rollback-journal mode, such as a verified backup, can leave one when another
program's write to it stops. Such a file is also left in WAL mode, as any
2.1.1 write leaves it.

Rejected alternatives:

- Refuse the read and ask the person to run a write command first. Rejected
  because the read cannot succeed until the write is undone, the undo has one
  correct result, and `ath check` is the command a person runs to find out
  what is wrong.
- Read the file as immutable and ignore the journal. Rejected because it would
  read half-written pages as if they were committed.
- Keep the old message. Rejected because it named the wrong problem.

## D223. The library keeps its SQLite connection private

`Store` keeps its SQLite connection and its transactions inside the library.
`Store::connection`, `Store::transaction`, `Store::bulk_transaction`, and
`Store::fresh_restore_transaction` are crate-private. No public function takes
or returns a rusqlite `Connection` or `Transaction`, or takes a closure over
one. Public reads and calculations take a `Store`. Examples are
`prediction::build_evidence`, `prediction::run_backtest`,
`log::plan_log_entry`, `view::calendar::resolve_calendar_range`, and
`store::contract::verify_live_schema`. `Store::pending_file_operations` is the
public way to read pending file work. Helpers that run inside a write
transaction, such as `store::write::apply_in` and `store::migrate::apply_step`,
are crate-private. Where one calculation reads through the same connection
several times, a crate-private `_in` form takes that connection, such as
`view::calendar::resolve_calendar_range_in`. The command line and the protocol
worker are part of the library, so they may use crate-private items.

Code outside the library that checks stored rows opens its own SQLite
connection to the database file, as a direct-SQL client does. The tests do
this through small helpers in their support modules. The performance runner
traces the statements an operation runs. It learns the handle of the store's
connection from a SQLite auto-extension when SQLite opens it, and it reads
counts and query plans through its own read-only connection.

A `compile_fail` doc test on `Store` calls each of the four methods from
outside the library. `cargo test` fails if any of them becomes public again.

The reason is spec sections 5 and 7. The store owns the connection and
transaction lifecycle, and public methods describe domain operations without
arbitrary SQL callbacks. A public connection let any caller run any SQL,
including a write that skips record validation, the revision rules, and the
database revision. SQL clients have the published direct-SQL contract in
`spec/sql/`, which applies to their own connection. This change alters
signatures and visibility only. Outputs and stored rows are the same.

Rejected alternatives:

- A `test-support` Cargo feature that re-exports the raw connection to tests.
  Rejected because any caller can turn on a feature, so the access would stay
  public in practice. Tests can open their own connection instead.
- Leave the methods public and document that callers should not use them.
  Rejected because a comment does not stop a write that skips the store's
  checks, and the spec asks for no such access.

## D224. An unapproved draft lives in the appendix of the version that replaced it

A version folder holds only a plan for a version that was approved. A draft that
was replaced before approval moves, unchanged, into the `appendix/` of the version
that replaced it, under the name `draft-<version>/`. Its proposed D-numbers stay
reserved and are never reused.

The first draft of the rebuild plan was written as version 3.0.0 and proposed
D198–D207. It was never approved, and version 2.1.1 replaced it (D208, D209). Its
three files moved from `build-history/v3.0.0/` to
[`appendix/draft-2.1.1/`](appendix/draft-2.1.1/spec.md) with no change to their text.
Their titles still say 3.0.0, because that is what the draft proposed. Their
relative links are as they were at commit 9048078, where they resolve; several name
files that the 2.1.1 cutover removed.

The reason is that folders are named for the format version they describe
(`build-history/README.md`). No version 3.0.0 exists, so a `v3.0.0/` folder read as
a planned future version. `build-history/README.md` now lists the status of each
version folder and states this rule.

This is an exception to the rule that earlier version folders are not changed. It
applies only to a draft that was never approved. A released version's folder never
moves.

Rejected alternatives:

- Delete the draft. Rejected because build history keeps rejected plans, and the
  draft explains what 2.1.1 chose not to do.
- Leave `v3.0.0/` in place with a warning. Rejected because the folder name itself
  is what misleads.
- Name the folder `draft-3.0.0/`. Rejected because it still names a version that
  does not exist; the draft is the first plan for 2.1.1.
- Edit the draft's text or links. Rejected because build history keeps what was
  written.

## D225. Pull requests run a quick check; releases run the full check

Every pull request push and every push to `main` runs the quick check in
`.github/workflows/ci.yml`. Linux runs formatting, Clippy, every Rust test, and
the npm tests on Node 24. Windows runs Clippy, every Rust test, and the real
Credential Manager test. A summary job, `checks passed`, passes only when both
pass, and it is the one status check the `main` branch rules require. A new push
to a pull request cancels the run for the push before it. Rust builds are cached.

A version tag runs the full check in `.github/workflows/validate.yml` through
`release-cross-platform.yml`. It covers every Rust test on Linux, macOS, and
Windows, the release builds, the archive and npm install checks on the three
systems, and the npm tests on Node 22 and 24. Then it publishes the checked
files. The same workflow runs by hand on any branch, and publishes only from a
version tag with `publish` set.

The speed comparison with 2.1.0 (`release-performance.yml`) runs only by hand.
A release no longer waits for its results. The 3-million-record job runs only
when asked for, because it needs a self-hosted runner.

This supersedes two parts of spec section 15: pull requests running the release
workflow's validation jobs, and the release job refusing to publish without
performance results for the same commit.

The reasons:

- A pull request ran about 35 jobs on each push: CI's 16, the release
  workflow's same 16, and a 20-minute speed comparison. For a pull request,
  GitHub matches the path filters against the whole pull request, so on a large
  one the release and speed workflows ran on every push.
- The release could not succeed. It looked for a finished speed comparison as
  soon as it started, while the comparison started at the same moment and took
  20 minutes or more. The 3-million-record job needed a self-hosted runner, and
  none is registered, so it would have waited forever.
- The `main` rules required seven check names from the 2.1.0 workflows. The
  cutover removed those jobs, so no pull request could merge. One stable
  summary check keeps the rules valid when jobs are renamed or added.
- Every platform bug found during the rebuild appeared on Windows, so Windows
  stays in the quick check. macOS is the development machine, where the tests
  run before each commit.
- Packaging and macOS problems are now found when a release is tagged, not when
  a pull request merges. Packaging changes rarely, and the full check can run by
  hand on a pull request that touches it.

Rejected alternatives:

- Run the full check on every pull request push. Rejected because it ran about
  35 jobs per push, most of them for packaging that a push rarely changes.
- Run the full check after every merge to `main`. Rejected because nothing on
  `main` reaches users until a release, and a release runs the full check.
- Keep the speed comparison as a release gate. Rejected because it cannot finish
  before the release workflow checks for it, and its 3-million-record job has
  no runner.
- Drop Windows from the quick check. Rejected because Windows is where the
  platform bugs appeared.

## D226. macOS joins the quick check

The quick check in `.github/workflows/ci.yml` also runs Clippy and every Rust
test on macOS. `checks passed` requires the Linux, macOS, and Windows jobs. This
supersedes the part of D225 that left macOS to the release check.

The reason is that D225 counted on development happening on a Mac, where the
tests run before each commit. A change written on Linux or Windows would then
reach macOS only when a release is tagged. The macOS job runs alongside the
others and finishes before the Windows job, so it adds no waiting.

Rejected alternatives:

- Keep macOS in the release check only. Rejected because it depends on where
  each change is written.
