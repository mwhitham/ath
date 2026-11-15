# v2.1.1 rebuild progress

Status: **cutover complete on `build/native-2-1-1`; release evidence pending**.

The release target is 2.1.1. The complete specification is [spec.md](spec.md).
[D208–D211](decisions.md) record the target and compatibility requirements.

Completed:

- Recorded 2.1.1 as the selected version for the complete rebuild.
- Retained the full implementation and verification requirements.
- Removed the earlier draft's proposed public API removals from the selected plan.
- Preserved the unapproved 3.0.0 draft as history.

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
  keep its promises. Tests were added where a promise had no remaining check.
  51 of 52 files were removed; the result and the one kept file are at the end
  of the [test disposition](appendix/test-disposition.md).
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

Remaining:

- The performance report, `performance/report-2.1.1.md`.
- Runs of the new validation workflow on Linux, macOS, and Windows, with the
  archive and npm smoke tests, on the final commit. The jobs are defined in
  `.github/workflows/validate.yml` and have not run yet.
- A native test that reads the plan of the evidence query, which would replace
  the last kept 2.1.0 test file.
- Record the release evidence listed in spec section 16 before creating a release.

The installed implementation remains 2.1.0 until the rebuilt 2.1.1 is complete.
No package was published, and no release or tag was created.
