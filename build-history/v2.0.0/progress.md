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
| 9 | Record writes, the eight range calculations, and WHOOP and Oura CSV reading in `ath-core` | done |
| 10 | The npm package includes `ath-core`, and the installed package can init, log, import, and apply | done |

Already done before this version: the safe upgrade command from pull request 21,
Apple Health, and the contribution notes that were already merged. Pull request
15's index links are in `build-history/README.md`. WHOOP and Oura already had
readers. Those readers now run in `ath-core` (D162, D163, D166). The
published 1.0.0 migration file is unchanged. A new database applies the
2.0.0 columns at creation (D165). An older tool skill is replaced (D164).

Later, with the reason:

- iPhone app. This build cannot run it.
- Strava's online connection. The bulk download is the path in this version,
  and the permissions question is still open.
- Android app. This version has the command-line tool.
- Apple Health, package import, checks, the upgrade, export, and erasure.
  They stay in TypeScript until the same tests pass in `ath-core` (D155, D162).
- A pull from Open Wearables or Junction, and the mapping file a new source
  uses. D161 records the rule. The study is
  [appendix/source-formats.md](appendix/source-formats.md). The pull is not
  built yet.
