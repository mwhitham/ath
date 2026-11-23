# v2.1.3 progress

Status: **in progress; not released**. The plan is [spec.md](spec.md).

| Step | What | Status |
|---|---|---|
| 1 | Plan, decisions D254–D267, storage study, hosting experiments | done |
| 2 | Version 2.1.3, migration 007, coach authorship | done |
| 3 | Changeset capture in the library (D268) | done |
| 4 | libSQL remote backend: working copies, guarded commits, replication log, qualification suite | not started |
| 5 | `ath-host` native directory mode: connect, read, write, attachments, refresh, restore | not started |
| 6 | Sign-in and grants | not started |
| 7 | Remote mode with object storage and several host copies | not started |
| 8 | Moves and backups | not started |
| 9 | Clients, starter, `docs/hosting.md`, container and setup guide | not started |
| 10 | Scale and fault runs, release evidence | not started |

The experiments in step 1 ran on 2.1.2 with synthetic data on one machine
([hosting-experiments.md](appendix/hosting-experiments.md)). No live provider, region,
or network latency has been measured yet. Steps 4 and 7 do that.

Step 2 notes:

- An erasure removes that record's earlier rows from `internal_change_log` and leaves one
  `erasure` row with no values, so the log never outlives an erasure.
- The change-log triggers are listed under a new `internal_triggers` key in
  `spec/sql/contract.json`, because they enforce nothing.
- Records carry no version, so the 2.1.3 gate on coach authors runs on the package
  document, in Rust and in Zod.
- A 2.1.2 program refuses a step-7 database by its version check ("Unsupported Athletic
  Standard database version: 2.1.3"). This was checked by hand with a 2.1.2 build.

D268 changes how the libSQL backend writes. Steps 3 and 4 follow it.

Step 3 notes:

- Changes are recorded with temporary triggers on the writing connection, not with
  SQLite's session extension. The session extension needs `bindgen` and `libclang` in
  every build, including the release builds for each platform. Temporary triggers need
  nothing extra, keep the exact order of the writes, and roll back with the transaction.
- A changeset is JSON. No replicated column holds a binary value, so JSON carries every
  value exactly.
- `tests/replica.rs` runs with `--features changesets`. Step 4 adds the host to the
  workspace, which turns the feature on for the default `cargo test`.
- Migration 007 gained `internal_replication_log`, with the revision each changeset
  started from, and a trigger that empties it when a record is erased.
