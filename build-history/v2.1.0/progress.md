# v2.1.0 progress

| Step | What | Status |
|---|---|---|
| 1 | The import baseline from code that matches main `d418a8f`, and a copy of the 2.0.0 database and a 2.0.0 package | done |
| 2 | The mapping engine in `ath-core`, `ath import --mapping`, and the shipped `mappings/` files (D177–D183) | done |
| 3 | The four refusals, and no calculation reads `supplied:intensity` or `supplied:Feeling` (D179, D186) | done |
| 4 | Version 2.1.0 for the program, the record, and the database, migration step 4, and `schema/2.1.0/` (D184) | done |
| 5 | `field_basis` on mapped values (D185) | done |
| 6 | `field_basis` on every stored value, from every importer and `ath log` (D187) | done |
| 7 | `cargo test` on Linux, macOS, and Windows, a Windows path test, and the pack checks in CI | done |
| 8 | The regenerated demo athlete, and a migrated copy of the 2.0.0 database that passes `ath check` | done |

Mike approved the plan in [spec.md](spec.md) and changed it in three ways. The build is
one pull request, not three. The version is option A: one number, 2.1.0. `field_basis`
goes on every stored value, not only mapped values. That last change is D187. The spec
says what was built where it differs from the plan.

The baseline found one mistake in the plan. Its package row named
`examples/demo-athlete/athlete.ath.json`, which is the legacy 0.4.3 file that `ath import`
refuses. The row now restores a complete 2.0.0 package exported from the 2.0.0 database
copy. Under 2.1.0 that package still imports. The package loader had accepted only the
current version, so it now accepts `2.0.0` and `2.1.0` (D184).

Baseline row 04 passed only in UTC. One GPX time in its fixture has no UTC offset, and
the importer reads it in the machine's time zone. The test now runs that one import
with `TZ=UTC`. The importer and the assertions are unchanged (spec section 6).

Every importer could state a basis from what its source says. None had to be invented.
Migration step 4 does not backfill `field_basis` on older records (D187).

Mike asked to remove the outdated "Later, with the reason:" heading from
[v2.0.0/progress.md](../v2.0.0/progress.md). The mapping study is
[v2.0.0/appendix/source-formats.md](../v2.0.0/appendix/source-formats.md) (D161).

Nothing is published. There is no npm publish, crate publish, release, or tag.
