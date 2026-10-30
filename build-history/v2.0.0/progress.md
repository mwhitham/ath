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
| 11 | Apple Health `export.xml` in `ath-core`, matching the previous importer | done |

Already done before this version: the safe upgrade command from pull request 21,
Apple Health, and the contribution notes that were already merged. Pull request
15's index links are in `build-history/README.md`. WHOOP and Oura already had
readers. Those readers now run in `ath-core` (D162, D163, D166). The
published 1.0.0 migration file is unchanged. A new database applies the
2.0.0 columns at creation (D165). An older tool skill is replaced (D164).
The packed binary is marked executable before it runs (D168). A FIT
position that means "not present" is left out, and a GPX point with no time
is left out (D169). The release workflow stages the binaries, packs the tarball with
`npm pack`, checks that file, and publishes it with
`npm publish athleticstandard-2.0.0.tgz` (D170, D171). A correction keeps a
17-digit number the same in the revision and the typed rows.
`maximum_heart_rate_bpm` is the athlete's known maximum. Garmin `maxHR` is
a `max_heart_rate` provider summary (D172). Apple Health `export.xml` is
read in `ath-core` (D173). The command-line front end stays in TypeScript
(D174). An inline `WorkoutRoute` is stored as the workout's route, and a
`FileReference` GPX file is that route too. Each point keeps its UTC
offset. The route element's own fields stay in `supplied_fields`. A
location that cannot be stored is named in the preview (D175). An Apple
`export.zip` whose entry is named `export.xml` is still an Apple import.
A referenced GPX file has to sit in that export's `workout-routes/` folder
and overlap the workout, or the preview names it and the points are not
stored (D176).

Later, with the reason:

- iPhone app. This build cannot run it.
- Strava's online connection. The bulk download is the path in this version,
  and the permissions question is still open.
- Android app. This version has the command-line tool.
- Package import, checks, the upgrade, export, and erasure. D155 said those
  pieces move after the same tests pass in `ath-core`. D173 says that wait is
  no longer required. The tests still have to pass. The command-line front
  end stays in TypeScript (D174).
- A pull from Open Wearables or Junction, and the mapping file a new source
  uses. D161 records the rule. The study is
  [appendix/source-formats.md](appendix/source-formats.md). The pull is not
  built yet.
