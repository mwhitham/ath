# v1.0.0 Build Progress

Tracks the build order from `spec.md`. Update when a step lands.

| Step | Deliverable | Status |
|---|---|---|
| 0 | Confirm the next release is v1.0.0 after published v0.4.3 | ✅ done |
| 1 | Approve the SQLite build plan and record D82 onward | ✅ D82–D98 recorded |
| 2 | Review and approve the field-by-field v0.4.3 migration appendix | review required |
| 3 | Define stable IDs, revisions, time and provider periods, nutrition, plans, and authored records in Zod | pending |
| 4 | Add SQL migrations and the local libSQL adapter | pending |
| 5 | Generate the SQLite fixture and database test helpers | pending |
| 6 | Move read commands to SQLite | pending |
| 7 | Move writes, result grading, prediction, and backtest to SQLite transactions | pending |
| 8 | Add legacy JSON import and deterministic JSON export | pending |
| 9 | Migrate provider importers and dense samples using `appendix/time-and-provider-periods.md` | pending |
| 10 | Update checks, help, the Skill, SPEC, README, and package exports | pending |
| 11 | Run functional, cross-platform, and measured performance suites | pending |
| 12 | Release a prerelease for migration testing | pending |
| 13 | Release v1.0.0 after round-trip and cross-platform tests pass | pending |

Runtime implementation is intentionally paused at step 2. The approved plan requires
the migration appendix to be reviewed before schemas or database code change.
