# v2.1.1 rebuild progress

Status: **implementation in progress on `build/native-2-1-1`**.

The release target is 2.1.1. The complete specification is [spec.md](spec.md).
[D208–D211](decisions.md) record the target and compatibility requirements.

Completed:

- Recorded 2.1.1 as the selected version for the complete rebuild.
- Retained the full implementation and verification requirements.
- Removed the earlier draft's proposed public API removals from the selected plan.
- Preserved the unapproved 3.0.0 draft as history.

Built so far (all under `native/`, which moves to the root at the cutover):

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

Remaining:

- Oura, Garmin, FIT, GPX, Strava, mapped, and Apple Health imports.
- Connecting every library operation to its command.
- The npm launcher, the JavaScript client, and the Zod compatibility facade.
- Performance measurements (spec section 14) and the maintainability exercises.
- Linux and Windows runs, the cutover (spec section 15, step 8), and the
  architecture document.
- Record the release evidence listed in spec section 16 before creating a release.

The installed implementation remains 2.1.0 until the rebuilt 2.1.1 is complete.
No package was published, and no release or tag was created.
