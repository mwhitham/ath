# v1.0.0 Build Progress

Tracks the build order from `spec.md`. Update when a step lands.

| Step | Deliverable | Status |
|---|---|---|
| 0 | Confirm the next release is v1.0.0 after published v0.4.3 | ✅ done |
| 1 | Approve the SQLite build plan and record D82 onward | ✅ D82–D109 recorded |
| 2 | Define stable IDs, revisions, time and provider reporting periods, nutrition, plans, and authored records in Zod | ✅ done |
| 3 | Add SQL migrations and the local libSQL adapter | ✅ done |
| 4 | Generate the SQLite fixture and database test helpers | ✅ done |
| 5 | Move read commands to SQLite | ✅ done |
| 6 | Move writes, result grading, prediction, backtest, deletion, and privacy erasure to SQLite transactions | ✅ done |
| 7 | Add v1 JSON package import and deterministic JSON export | in progress — package restore and export work; conflict commands remain |
| 8 | Migrate provider importers and dense samples using `appendix/time-and-provider-periods.md` | in progress — v1 transactional import and repeat matching work; raw source receipts need final audit |
| 9 | Update checks, help, the Skill, SPEC, README, and package exports | pending |
| 10 | Run functional, cross-platform, and measured performance suites | pending |
| 11 | Release a prerelease for v1 package and database testing | pending |
| 12 | Release v1.0.0 after round-trip and cross-platform tests pass | pending |

D101 removed the prerelease-file migration gate. Runtime implementation begins at
step 2 with the v1 domain schemas. Step 2 added the typed record, revision, portable
package, measurement-series, and export-manifest schemas.

Steps 3 and 4 added the raw SQL schema, opaque libSQL database handle, record and
historical query operations, deterministic SQLite fixture, and storage integrity
tests. Steps 5 and 6 moved the read and write command flows to SQLite. The write
commands now include stable-ID correction, linking, deletion, grading, and
dependency-aware privacy erasure.

Runtime reads now include named source-separated calculations for summaries,
baselines, trends, calendar coverage, source comparisons, correlations, and workout
drift.

Step 7 now has deterministic package export, empty-database restore, same-database
merge checks, dense sample files, and attachment checks. The temporary conflict
workflow remains before that step is complete.

Step 8 now detects Apple Health, WHOOP, and Oura exports. It maps their records and
dense samples into SQLite, writes route-specific import keys, matches unchanged
reimports, and stores provider corrections as revisions in one transaction.
