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
| 9 | The release workflow on every pull request, passing on Linux, macOS, and Windows (D188) | done |
| 10 | The fixes from the final review: one transaction for a mapped import, deleted sources, supplied field names, bases that name no product, each source's layout in its mapping file, and the heart-rate variability name check (D189–D194) | done |
| 11 | The smaller fixes from the final review: the bases a line may claim, the first line that writes a field, and the Windows `sqlite3` install (D195–D197) | done |

The maintainer approved the plan in [spec.md](spec.md). Three changes override it where
they conflict. The build is one pull request, not three. 2.1.0 ships as option A: one
number, 2.1.0. D184 says when the maintainer chose it and quotes the answer.
`field_basis` goes on every stored value, not only mapped values. That last change is
D187. The spec says what was built where it differs from the plan.

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

The 2.0.0 release run failed on macOS and Windows, and nothing was published. The
pull-request workflow had run the TypeScript tests on Linux only, so the failures first
showed at release time. The release workflow now runs on every pull request (D188).
Spec section 6 lists each fix. None removes or loosens a test. One backup test case
uses a file name that Windows does not allow, so it runs on Linux and macOS only.

The final review found three problems before merge, and the maintainer approved fixing
them. A mapped import could stop halfway with its earlier rows written, and deleting a
mapped source made the next import fail. A supplied field with an empty name was stored,
and then `ath check` and `ath export` failed. The engine held Open Wearables and Junction
names, and two basis values named those products. D189 through D194 fix them. The
review also listed four smaller items. Three are D195 through D197. The fourth, the
heart-rate variability check outside point records, is part of D194. No test or check
was removed or loosened.

The maintainer asked to remove the outdated "Later, with the reason:" heading from
[v2.0.0/progress.md](../v2.0.0/progress.md). The mapping study is
[v2.0.0/appendix/source-formats.md](../v2.0.0/appendix/source-formats.md) (D161).

Nothing is published. There is no npm publish, crate publish, release, or tag.

The engineering verification work in spec section 14 adds a repository skill,
scoped instructions, PR templates, and checks for recorded decisions and generated
outputs. The [audit](../../docs/engineering-audit.md) separates Cursor-origin work
from unconfirmed GrokBot attribution. The
[workflow](../../docs/engineering-workflow.md) records review setup and its remaining
account-setting requirement. No existing D-number is changed or added.
