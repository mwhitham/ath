# v2.1.3 progress

Status: **built; not released**. Evidence: [release-evidence.md](appendix/release-evidence.md). The plan is [spec.md](spec.md).

| Step | What | Status |
|---|---|---|
| 1 | Plan, decisions D254–D267, storage study, hosting experiments | done |
| 2 | Version 2.1.3, migration 007, coach authorship | done |
| 3 | Changeset capture in the library (D268) | done |
| 4 | libSQL remote backend: working copies, guarded commits, replication log, qualification suite | built; live run pending |
| 5 | `ath-host` native directory mode: connect, read, write, attachments, refresh, restore | done |
| 6 | Sign-in and grants | done; provider recipes in step 9 |
| 7 | Remote mode with object storage and several host copies | done with a stand-in server; live run pending |
| 8 | Moves and backups | done; Litestream recipe untested |
| 9 | Clients, starter, `docs/hosting.md`, container and setup guide | done; iOS and container not run |
| 10 | Scale and fault runs, release evidence | done |

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

Steps 4 to 7 notes:

- `ath-host` is a second package in the workspace (`host/`). The `ath` CLI does not
  depend on it. The libSQL client lives only in the host.
- Remote mode follows D268: the library runs every operation on a working copy, and
  the host commits the recorded rows to the server in one guarded transaction. The
  same commit code runs against a local SQLite file standing in for the server
  (`local_server` storage), which is how the tests run without a network.
- Host copies that share athletes' databases also share the host directory. In remote
  mode it is a libSQL database; in the stand-in mode it is a file in the shared folder.
- `host/tests/host_api.rs` covers, in both modes: a report with a photo saved and
  opened on another device; retries and request-ID conflicts; a coach who sees and
  adds only what was granted; a program; a revocation that blocks a waiting write; and
  two host copies sharing one record, including catching up from the replication log
  and rebuilding after an erasure.
- Not yet run: the remote backend against a real libSQL server, and sign-in through a
  real provider. Tokens in the tests are signed by a fixed test-only key.

Step 8 notes:

- A move is driven by the owner's app: `move/freeze` on the source, the package
  uploaded to an `incoming` record on the destination, `move/retire` on the source,
  then `move/activate` on the destination, which asks the source to confirm the
  single-use code for its own address and the digest of the package it holds (D267).
- `ath-host backup` writes each athlete's package as a zip after a trial restore and
  check. `ath-host restore` replaces one athlete's record in directory mode, then
  applies every erasure recorded since and raises the access epoch. In remote mode, use
  the provider's restore for that athlete's database.
- Every erasure is noted in the host directory before it is applied. The host finishes
  unfinished erasures when it starts (D264).
- The Litestream recipe is documented but not run: no Litestream binary was run in
  this build.
- `host_api.rs` has an ignored qualification test that runs every scenario against a
  real libSQL server with two host copies and prints timings. It needs a server
  address and token in `ATH_QUALIFY_*` variables.

Steps 9 and 10 notes:

- The npm package exports `connectHost` and `HostError` (`packages/npm/src/host.ts`).
  `examples/host-starter`, `examples/host-mcp`, and `examples/host-swift` are not
  published.
- `docs/hosting.md` is the guide for operators and app developers; `host/Dockerfile`
  builds the host image.
- Building the clients found a security bug, fixed before this step closed: coach
  routes read storage before looking the athlete up, and athlete IDs reached storage
  unchecked. IDs are now checked everywhere, and an unknown ID answers like an unshared
  one.
- The [release evidence](appendix/release-evidence.md) lists what passed, the timings,
  what was not verified, and what 2.1.3 does not build.

After step 10:

- An athlete can disconnect one app from a record and connect it again from another
  (D270). The host keeps each record's apps in its directory. A disconnection raises
  the access epoch, and owner writes now carry the epoch guard that coach writes use.
  A guarded write in remote mode now retries after a revision conflict; the retry
  checks the epoch again first.
