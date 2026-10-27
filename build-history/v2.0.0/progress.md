# v2.0.0 progress

| Step | What | Status |
|---|---|---|
| 1 | Version 2.0.0 on the program, the record, and the database | done |
| 2 | Workout names, text, routes, heart-rate samples, and original field names | done |
| 3 | Upgrade through `ath migrate` | done |
| 4 | Rust library and a shared-file test with the command-line tool | done |
| 5 | Garmin account files, FIT points, GPX, and the personal connection | done |
| 6 | Strava bulk export | done |
| 7 | Calculation skill files and the zone worked example | done |
| 8 | Linux checks: `pnpm` checks and `cargo test` | done |

Already done before this version: the safe upgrade command from pull request 21,
Apple Health, WHOOP, and Oura imports, and the contribution notes that were
already merged. Pull request 15's index links are in `build-history/README.md`.

Later, with the reason:

- iPhone app. This build cannot run it.
- Strava's online connection. The bulk download is the path in this version,
  and the permissions question is still open.
- Android app. This version has the command-line tool.
- Record writes, corrections, deletions, provider-import writes, the eight
  range calculations, and WHOOP CSV reading run in `ath-core` (D162). Apple
  Health, Oura, package import, checks, the upgrade, export, and erasure are
  still TypeScript until the same tests pass there.
- A pull from Open Wearables or Junction, and the mapping file a new source
  uses. D161 records the rule. The study is
  [appendix/source-formats.md](appendix/source-formats.md). The pull is not
  built yet.
