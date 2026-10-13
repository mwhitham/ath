# v1.0.0 Build Progress

Tracks the build order from `spec.md`. Update when a step lands.

| Step | Deliverable | Status |
|---|---|---|
| 0 | Confirm the next release is v1.0.0 after published v0.4.3 | ✅ done |
| 1 | Approve the SQLite build plan and record D82 onward | ✅ D82–D114 recorded |
| 2 | Define stable IDs, revisions, time and provider reporting periods, nutrition, plans, and authored records in Zod | ✅ done |
| 3 | Add SQL migrations and the local libSQL adapter | ✅ done |
| 4 | Generate the SQLite fixture and database test helpers | ✅ done |
| 5 | Move read commands to SQLite | ✅ done |
| 6 | Move writes, result grading, prediction, backtest, deletion, and privacy erasure to SQLite transactions | ✅ done |
| 7 | Add v1 JSON package import and deterministic JSON export | ✅ done |
| 8 | Migrate provider importers and dense samples using `appendix/time-and-provider-periods.md` | ✅ done |
| 9 | Update checks, help, the Skill, SPEC, README, and package exports | ✅ done |
| 10 | Run functional, cross-platform, and measured performance suites | in progress — local functional and measured 100,000- and 3-million-record runs pass; cross-platform workflow pending |
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

Step 7 has deterministic package export, empty-database restore, same-database
merge checks, dense sample files, attachment checks, and explicit conflict
resolution.

Step 8 detects Apple Health, WHOOP, and Oura exports. It maps their records and
dense samples into SQLite, writes route-specific import keys, matches unchanged
reimports, and stores provider corrections as revisions in one transaction.

D114 replaced the required 30-million-record release run with focused query-bound
tests. The required measured runs now use 100,000 and 3 million records. Both pass
all nine operations on the reference runner. Their measurements and review limits
are recorded in `performance/limits.json`.
