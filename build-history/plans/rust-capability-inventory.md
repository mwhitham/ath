# Capability and dependency inventory (Rust adoption)

Maps today’s TypeScript implementation to the planned Rust core (`ath-core`) and thin
host shells. Status is **current** unless noted **planned** or **remove**.

## Host shells (stay thin)

| Area | Today | Owner after cutover |
|---|---|---|
| CLI argument parsing, help, presentation | `src/program.ts`, `src/help.ts`, `src/cli.ts` | TypeScript CLI |
| Skill install and references | `src/skill.ts`, `skill/` | TypeScript CLI |
| Credentials and model gateway | `src/keyring.ts`, `src/gateway.ts`, `src/predict.ts` | TypeScript CLI |
| Progress and share UX | `src/progress.ts`, `src/share.ts` | TypeScript CLI |
| SwiftUI verification app | not in repo yet | Swift app under `apps/verification-ios/` (see rust-adoption plan) |

## ath-core (single executable domain)

| Capability | Current location | Rust module (planned) | Notes |
|---|---|---|---|
| v1 record shapes and JSON rules | `src/v1/schema.ts`, generated `schema/1.0.0/` | `model`, `validate` | Zod refinements move to Rust validators; JSON Schema and TS types generated from Rust |
| Legacy 0.x shapes | `src/schema.ts` | none | Remove after cutover; importers stop routing through 0.x |
| SQLite open, pragmas, migrate | `src/db/migrate.ts`, `src/db/client.ts` | `storage` | Reads embedded `spec/sql/migrations/*.sql` |
| Record write, revision, history | `src/db/records.ts`, `revision-encoding.ts` | `records` | Official write path; replaces ad hoc direct-SQL revision recipes |
| Queries and views | `src/db/query.ts`, `records-view.ts`, `series-view.ts` | `query` | |
| Conflicts and transactions | `src/db/conflicts.ts`, `transaction.ts` | `storage` | |
| Deletion and erasure | `src/db/erase.ts` | `lifecycle` | |
| Package import and export | `src/db/import-package.ts`, `export.ts` | `package` | |
| Attachments | `src/db/attachments.ts`, `attachment-ingest.ts` | `attachments` | Staging must roll back with failed transactions |
| Provider import | `src/import/*`, `src/db/provider-import.ts` | `import` | Normalize straight into canonical model; drop `ImportPayload` via 0.x |
| Dense series encode/decode | `src/series.ts`, `src/db/series-view.ts` | `series` | |
| Calculations | `src/db/calculate.ts`, `src/hrv.ts`, `src/stats.ts` | `calc` + method registry | Expand per coverage table |
| Check and drift | `src/db/check.ts`, `scripts/check-schema-drift.ts` | `check` | |
| Evidence and grading hooks | `src/db/evidence.ts`, `prediction-grade.ts` | `grading` (host calls models; core stores results) | |
| UUID and IDs | `src/db/uuid.ts` | `ids` | |
| SQL contract metadata | `src/db/sql-contract.ts`, `spec/sql/` | embed + `contract` | Plain SQL files are canonical |

## Generated public contracts

| Artifact | Today | After Rust foundation |
|---|---|---|
| JSON Schema | from `src/v1/schema.ts` | from Rust model |
| TypeScript types | Zod inference | generated from JSON Schema or Rust export |
| `spec/sql/` | generated from TS migration strings | canonical source; TS and Rust embed the same files |
| Conformance vectors | missing (`CR5` pending) | `spec/conformance/`; shared across Rust, Node, Swift |

## Zod and extension inventory (validation migration)

| Rule area | In Zod today | Rust plan |
|---|---|---|
| Two-tier wall (source required on measured records) | `src/v1/schema.ts` + SQL triggers | Both; triggers stay in SQL |
| Athlete reports cannot claim `source` | schema + triggers | same |
| Record type unions and required fields | Zod objects | Rust enums + validators |
| `extensions` envelope size and namespacing | `src/v1/extensions.ts` | Rust validator |
| Portable package unknown fields | strict package schemas | Rust import validator |
| Cross-record links and protected references | partial in Zod; more in SQL | full semantic checks in core |
| Provider period and timezone fields | `src/v1/schema.ts` | preserve |
| RMSSD vs SDNN quantities | separate fields | never merge baselines |
| Legacy 0.x-only rules | `src/schema.ts` | remove with 0.x paths |

## Dependencies between open work

| Pull request | Overlap with Rust prerequisites |
|---|---|
| [#15](https://github.com/mwhitham/ath/pull/15) (contribution foundation plan) | CR5 conformance, CR6 guidance, CR3 SQL — implement prerequisites on `main`; do not duplicate #15 doc-only deltas |
| [#25](https://github.com/mwhitham/ath/pull/25) (migrate, import check, logging) | `ath migrate` and import receipts belong in core eventually; land #25 behaviors in TS first, port to Rust in Provider imports stage |
| [#20](https://github.com/mwhitham/ath/pull/20) (this plan) | Docs and decisions only |

## Temporary dual paths (must have removal milestones)

| Temporary path | Milestone to remove |
|---|---|
| TypeScript engine in `src/db/*` | Cutover stage: delete after foundation proof and stage gates |
| `src/schema.ts` 0.x model in import pipeline | Provider imports stage |
| `generate:sql-contract` from TS strings | Records stage: SQL files edited directly; generator becomes verify-only or removed |
| Hand-maintained Zod as normative | Foundation stage: Rust owns validation; Zod becomes generated |
| `src/import/index.ts#importExport` | Prerequisite PR: delete unused 0.x export writer |
