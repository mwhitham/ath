# Capability and dependency inventory (Rust adoption)

Maps today’s TypeScript code to planned `ath-core` modules and thin hosts.

## Host shells (stay thin)

| Area | Today | After cutover |
|---|---|---|
| CLI args, help, presentation | `src/program.ts`, `src/help.ts`, `src/cli.ts` | TypeScript CLI |
| Skill | `src/skill.ts`, `skill/` | TypeScript CLI |
| Credentials, models | `src/keyring.ts`, `src/gateway.ts`, `src/predict.ts` | TypeScript CLI |
| Progress, share | `src/progress.ts`, `src/share.ts` | TypeScript CLI |
| iOS verification app | not in repo | `apps/verification-ios/` when built |

## ath-core

| Capability | Today | Rust module (planned) | Notes |
|---|---|---|---|
| v1 records | `src/v1/schema.ts`, `schema/1.0.0/` | `model`, `validate` | Zod normative until npm 2.0.0; then generated from Rust |
| Legacy 0.x | `src/schema.ts` | none | Remove from import paths |
| SQLite open, migrate | `src/db/migrate.ts`, `src/db/client.ts` (D93) | `storage` | Embed SQL; packaging must ship bytes (see plan §5) |
| Writes, revisions | `src/db/records.ts`, `revision-encoding.ts` | `records` | Official write path (D132) |
| Queries | `src/db/query.ts`, views | `query` | |
| Conflicts, transactions | `src/db/conflicts.ts`, `transaction.ts` | `storage` | |
| Deletion, erasure | `src/db/erase.ts` | `lifecycle` | |
| Packages | `src/db/import-package.ts`, `export.ts` | `package` | |
| Attachments | `src/db/attachments.ts`, `attachment-ingest.ts` | `attachments` | |
| Provider import | `src/import/*`, `src/db/provider-import.ts` | `import` | Drop 0.x hop |
| Series | `src/series.ts`, `src/db/series-view.ts` | `series` | |
| Calculations | `src/db/calculate.ts`, `src/hrv.ts`, `src/stats.ts` | `calc` | |
| Check, drift | `src/db/check.ts`, `scripts/check-schema-drift.ts` | `check` | |
| Grading storage | `src/db/evidence.ts`, `prediction-grade.ts` | `grading` | Models stay in CLI |
| IDs | `src/db/uuid.ts` | `ids` | |
| SQL contract | `src/db/sql-contract.ts`, `spec/sql/` | `contract` | |

## SQL generator timeline (plan and inventory agree)

| Phase | SQL source |
|---|---|
| Today | TypeScript strings in `src/db/migrations/`; `pnpm generate:sql-contract` writes `spec/sql/` |
| Records and lifecycle step | Edit `spec/sql/migrations/*.sql` directly; generator removed; drift check uses SQL files |
| Rust build | `ath-core` embeds the same SQL files (or post-packaging copy in Node) |

## Generated contracts

| Artifact | Today | After Rust cutover (npm 2.0.0 for TS exports) |
|---|---|---|
| JSON Schema | from Zod | from Rust model |
| TypeScript types | Zod inference | generated |
| `spec/sql/` | generated from TS | canonical on disk |
| Behavior fixtures | CR5 not started | `spec/conformance/` in build PR |

## Open pull requests

| PR | Role |
|---|---|
| [#20](https://github.com/mwhitham/ath/pull/20) | This plan + D130–D132 |
| [#25](https://github.com/mwhitham/ath/pull/25) | Land before Rust build; npm 1.1.0 |
| [#15](https://github.com/mwhitham/ath/pull/15) | Small README link updates only |

Implementation is **one build PR** after plan approval (not separate prerequisite PRs).

## Temporary paths and removal

| Path | Remove when |
|---|---|
| TypeScript engine `src/db/*` | Cutover |
| `src/schema.ts` on imports | Provider imports step |
| `pnpm generate:sql-contract` | Records and lifecycle step |
| Hand-maintained Zod as normative | Foundation + npm 2.0.0 export change |
| `src/import/index.ts#importExport` | Start of build PR |
