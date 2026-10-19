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
| 10 | Run functional, cross-platform, and measured performance suites | partial — local functional and measured 100,000- and 3-million-record runs pass; **cross-platform workflow not run yet** |
| 11 | Release a prerelease for v1 package and database testing | pending — no npm prerelease; 1.0.0 shipped from `main` at 7cd6a09 |
| 12 | Release v1.0.0 after round-trip and cross-platform tests pass | ✅ shipped — before the cross-platform tests ran (step 10); `athleticstandard@1.0.0` on npm (2026-10-18, `main` 7cd6a09) |

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

## Contribution foundation (CR4–CR9 after 1.0.0)

D115–D128 and `spec.md` §19 listed CR4–CR9 as pre-publish work. **CR4–CR9 slipped past
the 1.0.0 npm release**; they were not deferred on purpose. The long-form sequence is
in [`appendix/contribution-readiness.md`](appendix/contribution-readiness.md).

| Step | Deliverable | Status |
|---|---|---|
| CR0 | Record the approved contribution-foundation decisions | ✅ D115–D128 recorded |
| CR1 | Add the v1 extension envelope and its generated schemas and fixture | ✅ done — D129; implementation started before #15 merged with owner approval |
| CR2 | Enforce the two-tier wall, revisions, deletion, erasure, JSON shape, and protected links with SQLite constraints and triggers | ✅ done — D116; direct-SQL tests in `tests/v1-direct-sql.test.ts` |
| CR3 | Publish plain SQL migrations, stable read views, and the stable writable-table contract | ✅ done |
| CR4 | Add `ath migrate` preview, verified backup, explicit confirmation, and interactive prompt-on-open | pending |
| CR5 | Add direct-SQL and language-neutral conformance vectors | pending |
| CR6 | Publish machine-readable contract metadata, update `AGENTS.md`, and add the build-on-ATH Skill | pending |
| CR7 | Add contribution governance, community manifests, isolated checks, catalog generation, and explicit named Skill installation | pending |
| CR8 | Add internal workspace boundaries, `CORE_API_VERSION` 1.0.0, explicit public exports, and API deprecation checks | pending |
| CR9 | Verify the packed v1 tarball contains every promised schema, SQL, conformance, community, and Skill file | pending |

CR1–CR3 landed before athleticstandard@1.0.0 shipped. CR4–CR9 remain open. Step 10’s
cross-platform workflow is still pending.

## Rust adoption (proposed)

Tracked in [`appendix/rust-adoption.md`](appendix/rust-adoption.md). Draft decisions
D130–D132. `athleticstandard@1.0.0` is already on npm; this work targets a later npm
major (**2.0.0**) for dropping Zod-based exports while the record format can stay
1.0.0. Land [#25](https://github.com/mwhitham/ath/pull/25) (npm 1.1.0) before the
Rust build PR.

| Step | Deliverable | Status |
|---|---|---|
| R0 | Plan, inventory, coverage, decisions, `AGENTS.md` | in review — PR #20 |
| R1 | Approved plan → one build PR on `main` (SQL packaging, conformance, Rust workspace, dead-code milestones) | pending approval |
| R2 | iPhone simulator CLI ↔ app round trip | **BLOCKED** — requires Xcode and iOS Simulator on macOS |
