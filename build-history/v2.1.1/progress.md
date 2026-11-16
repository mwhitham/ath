# v2.1.1 rebuild progress

Status: **cutover complete on `build/native-2-1-1`; release evidence recorded; not released**.

The release target is 2.1.1. The complete specification is [spec.md](spec.md).
[D208–D211](decisions.md) record the target and compatibility requirements.

Completed:

- Recorded 2.1.1 as the selected version for the complete rebuild.
- Retained the full implementation and verification requirements.
- Removed the earlier draft's proposed public API removals from the selected plan.
- Preserved the unapproved first draft, written as 3.0.0, as history. It is in
  [appendix/draft-2.1.1/](appendix/draft-2.1.1/spec.md) (D224).

Built before the cutover (under `native/` at the time):

- Reference inventories of every command, option, help text, public JavaScript
  export, and existing test, now in [appendix/](appendix/) (D218).
- The record model, validator, and JSON Schema generator (D212, D213). All
  7,285 comparable 2.1.0 validation cases match.
- The one record encoder and the SQLite store. Replaying the demo history
  reproduces every athlete row byte for byte. Migration step 5 (D211).
- Backups through SQLite's own backup interface, confirmed migration, check,
  repair, and stats, compared with 2.1.0 results. The old program refuses a
  migrated 2.1.1 database.
- The import operation with staging and stale-plan refusal, and the WHOOP route.
- Complete packages: streaming export, restore, merge with conflicts,
  attachments with recovery state, and privacy erasure, compared byte for byte
  with 2.1.0 exports.
- Calculations, queries, series and records views, prediction evidence,
  grading, backtest (D215), and log planning, compared with 2.1.0 outputs.
- Runtime services: password store, model gateways, saved models, update plans,
  and the embedded skills and mappings.
- The command shell: every help text and parse error matches 2.1.0.
- The Oura, Garmin, FIT, GPX, Strava, mapped, and Apple Health imports, each
  command connected to its library operation, the npm launcher, the JavaScript
  client and Zod compatibility facade, the performance tools, and the two
  maintainability exercises.

The cutover (spec section 15, step 8):

- The Rust package moved to the repository root in the layout of spec section
  5: `Cargo.toml`, `src/`, `tests/` beside the synthetic fixtures, and `tools/`.
  `native/` is gone.
- The 2.1.0 implementation is removed: the TypeScript database implementation
  and command tree, the legacy 0.x runtime schema and signal conversion, the
  `ath-core` bridge and its string action interpreter, the modification-time
  rebuilds, the external `sqlite3` backup calls, the TypeScript SQL generator,
  the old build and publish scripts, and the root Node package files.
- The reference evidence moved into [appendix/](appendix/) (D218). The
  reference generators in `tools/reference/` load the 2.1.0 program from a
  separate checkout named by `ATH_REFERENCE_ROOT`.
- The SQL step 5 contract and `docs/architecture.md` were published from
  `native/pending-publication/` (D216).
- Each 2.1.0 TypeScript test file was matched with the native or npm tests that
  keep its promises. Tests were added where a promise had no remaining check,
  including the three that 2.1.0 never had. All 52 files were removed; the
  result is at the end of the [test disposition](appendix/test-disposition.md).
- The public-data check is a Rust test with the reviewed list in
  `tools/public-data-allowlist.json`.
- One generation entry point, `tests/generated.rs` (D219), checks and writes the JSON
  Schema, the npm protocol module, the skill's command reference, and the
  2.1.1 demo database. The 2.1.0 demo is kept in
  `tests/fixtures/version-2-1-0/`, and the 0.4.3 JSON file in
  `tests/fixtures/version-0-4-3/` for the prerelease refusal test.
- The npm package installs its own dependencies from its own lockfile (D220) and packs
  the repository's committed schema and skill.
- README, SPEC, AGENTS, CONTRIBUTING, the skills, and
  [`docs/upgrading-to-2-1-1.md`](../../docs/upgrading-to-2-1-1.md) describe
  2.1.1. `ath migrate --help` no longer asks for `sqlite3`
  ([command-line change 6](appendix/cli-changes.md)).
- CI runs one reusable validation workflow; the release workflow reuses it and
  publishes only the checked tarball and archives.

Performance (spec section 14), reported in
[`performance/report-2.1.1.md`](../../performance/report-2.1.1.md):

- Every target passes on the measuring machine. The 3-million-record initial
  import takes 0.465 of the 2.1.0 median (0.468, 0.461, and 0.473 in three
  paired runs). The 3-million-record check takes about 0.19 of the 2.1.0 time,
  which needed a larger Node heap to finish at all. Export is faster at both
  sizes. All nine existing limits pass.
- Peak memory stays under 512 MiB: at most 185.0 MiB for the 3-million-record
  import, 324.2 MiB at 1 million, 40.1 MiB for export, and 15.2 MiB for check.
- The report lists the nine changes made during this work. Each keeps every
  check and result, and has its own commit and tests. D221 records the rollback-journal
  restore into a new database, and D222 records how a read-only open undoes a
  restore that stopped partway. A write command no longer waits for a reader
  before it exits ([change 77](appendix/deliberate-changes.md)).

The release review (spec section 16):

- A review mapped every spec rule to the tests that check it and found rules
  with no test. The missing tests were added: direct-SQL tests for the
  remaining triggers and uniqueness rules, stopped and failed migrations,
  imports, and repairs, prompts through a pseudoterminal, no write lock during
  a prompt or a model call, every record type through its whole life, pinned
  published files, and others.
- Two of the new tests found bugs, now fixed. A stopped import left its
  working files in the temporary folder; a later import now removes them. The
  JavaScript client wrote a provider export whose files changed after the
  preview when its counts did not; it now compares the files
  ([change 78](appendix/deliberate-changes.md)).
- The Rust library no longer hands out its SQLite connection (D223).
- The CI jobs on Linux, macOS, and Windows needed small fixes: the macOS 14
  runners are retired, two test imports were Unix-only, and a Windows test
  process needs `SystemRoot` to open a socket.
- The evidence is in [appendix/release-evidence.md](appendix/release-evidence.md).

Remaining:

- A test import of a complete Garmin account export, once a synthetic fixture
  with that layout is reviewed and approved.
- The 3-million-record comparison on the dedicated performance runner.
- Merging, tagging, and publishing need approval. Nothing is published.

The installed implementation remains 2.1.0 until 2.1.1 is released.
No package was published, and no release or tag was created.

The engineering loop from [2.1.0 spec section 14](../v2.1.0/spec.md) landed after
the cutover. The decision check is `tests/decision_history.rs`. Generated files
remain on `tests/generated.rs` (D219). The Linux job in the quick check runs the
decision check for a pull request. No new D-number. The record format is unchanged.
