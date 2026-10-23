# v1.1.0 Build Progress

Tracks the work shipped in [pull request #25](https://github.com/mwhitham/ath/pull/25).
The v1.0.0 format and SQL schema are unchanged; this is an npm package release on top
of the plan in [spec.md](spec.md).

| Step | Deliverable | Status |
|---|---|---|
| 1 | Record D130–D136 and this spec | ✅ done (retroactive build-history in follow-up PR) |
| 2 | `ath migrate` with preview, verified backup, `--confirm-migration`, interactive open | ✅ done — implements v1.0.0 CR4 / D124 |
| 3 | sqlite3 backup path escaping and previewed `planned_backup_path` handoff | ✅ done — D135 |
| 4 | Interactive migrate/open confirm before backup; no backup on decline | ✅ done — D136 |
| 5 | Provider import `CurrentRecord.parse` before typed row and revision | ✅ done — D131 |
| 6 | Structured `ath log` closed envelope and extensions | ✅ done — D132 |
| 7 | Docs: 24 record types and chunked series names | ✅ done — D133; `tests/v1-docs.test.ts` |
| 8 | Default model path `.ath.model` and safe `writeDefaultModel` | ✅ done — D134; `tests/models.test.ts` |
| 9 | npm `1.1.0`; CLI and `tool_version` use package version; format stays `1.0.0` | ✅ done — D130 |
| 10 | `pnpm check:public-data`, typecheck, test, build, `check:schema-drift` on merge | ✅ done per PR #25 |

## Relation to v1.0.0 contribution readiness

CR4 in [v1.0.0/progress.md](../v1.0.0/progress.md) is marked done. CR5–CR9 and release
steps 10–12 of the v1.0.0 plan remain open on later package versions of the same
format.

## Known follow-ups (not in 1.1.0)

- [Issue #26](https://github.com/mwhitham/ath/issues/26) — Oura member-portal CSV
  layout drops readiness, scores, contributors, and SpO2 without skip lines.
- Databases provider-imported with npm `1.0.0` may remain INVALID after `ath migrate`;
  repair is export, then `ath init --empty`, then import (see spec §8.2).
