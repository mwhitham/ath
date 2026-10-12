# v1.0.0 Build Progress

Tracks the build order from `spec.md`. Update when a step lands.

| Step | Deliverable | Status |
|---|---|---|
| 0 | Confirm the next release is v1.0.0 after published v0.4.3 | ✅ done |
| 1 | Approve the SQLite build plan and record D82 onward | ✅ D82–D101 recorded |
| 2 | Define stable IDs, revisions, time and provider reporting periods, nutrition, plans, and authored records in Zod | pending |
| 3 | Add SQL migrations and the local libSQL adapter | pending |
| 4 | Generate the SQLite fixture and database test helpers | pending |
| 5 | Move read commands to SQLite | pending |
| 6 | Move writes, result grading, prediction, and backtest to SQLite transactions | pending |
| 7 | Add v1 JSON package import and deterministic JSON export | pending |
| 8 | Migrate provider importers and dense samples using `appendix/time-and-provider-periods.md` | pending |
| 9 | Update checks, help, the Skill, SPEC, README, and package exports | pending |
| 10 | Run functional, cross-platform, and measured performance suites | pending |
| 11 | Release a prerelease for v1 package and database testing | pending |
| 12 | Release v1.0.0 after round-trip and cross-platform tests pass | pending |

D101 removed the prerelease-file migration gate. Runtime implementation begins at
step 2 with the v1 domain schemas.
