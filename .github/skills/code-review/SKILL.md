---
name: code-review
description: Reviews Athletic Standard pull requests against repository rules from AGENTS.md, CONTRIBUTING.md, build-history, and the v1 spec. Use for GitHub Copilot code review on any PR to this repo.
---

# Athletic Standard pull request review

Review the diff against **only** the sources below. Cite a path (and D-number when one exists). Do not invent policy.

## Priority

1. **High:** data corruption, silent data loss, broken public contracts, real health data in Git, migration or backup mistakes, two-tier wall violations, unresolved import/merge writes.
2. **Medium:** missing required regeneration, missing D-number for a new decision, DCO sign-off missing on commits, changes to protected paths without appropriate scrutiny.
3. **Low:** style, naming, and prose — only after higher tiers are clean.

## Authoritative sources

| Topic | Read first |
| --- | --- |
| Agent rules and verification | `AGENTS.md` |
| Health data and local checks | `CONTRIBUTING.md` |
| Decision logging layout | `build-history/README.md` |
| v1 SQLite, migrations, SQL contract | `build-history/v1.0.0/spec.md` (especially §19.3–19.4) |
| Recorded choices | `build-history/*/decisions.md` |
| Field-level JSON / v1 records | `SPEC.md` (normative Zod in `src/v1/schema.ts`; legacy 0.x in `src/schema.ts`) |
| Public fixture allowlist | `scripts/check-public-data.ts` |

When sources appear to disagree, apply recorded precedence first (`AGENTS.md`: for v1 storage and schema names, D82–D105 supersede the listed older decisions; explicit supersession notes in `build-history/*/decisions.md`, e.g. D116 supersedes any reading of D104 that forbids direct SQL writers). Report only disagreements that remain after that. Do not invent policy.

## High-severity checks

### Two-tier wall (`AGENTS.md`; enforced in v1 spec and D116)

Measured records and athlete reports never mix. Measured records require a `source` reference; athlete reports must not claim one. Flag any change that weakens validation, triggers, or types that enforce this.

### Health data (`AGENTS.md`, `CONTRIBUTING.md`)

Never commit real health exports or identifying sample values. Public tests and fixtures must stay synthetic and allowlisted in `scripts/check-public-data.ts`. Flag exact workout dates, routes, birth dates, travel clues, device inventories, or real sample values in committed content or decision evidence.

### Migrations never silent (D124; `build-history/v1.0.0/spec.md` §19.4)

`ath migrate` previews, then confirms. Noninteractive open must not migrate. Every route creates and verifies a SQLite-safe backup before the first write. A package update alone must not rewrite a database. Reject silent migrate-on-open, unsafe backup, or migration without explicit confirmation.

### Stable SQL contract (D125; `build-history/v1.0.0/spec.md` §19.3)

Versioned SQL views are the stable **read** contract. A small documented table set is the stable **write** contract. Other tables are internal and may change via ordered migration. Flag breaking stable view or writable-table columns without a recorded compatibility plan, or treating internal tables as public API.

### Schema and generated artifacts (`SPEC.md`, D80)

**v1 (current):** `src/v1/schema.ts` is normative (`SPEC.md`). After changes, run `pnpm generate:json-schema` and `pnpm generate:v1-fixture`, and commit regenerated JSON Schema and the SQLite fixture under `examples/demo-athlete/`.

**Legacy 0.x:** `src/schema.ts` defines released 0.x JSON. After changes there, run `pnpm generate:json-schema` and `pnpm generate:fixture` for `examples/demo-athlete/athlete.ath.json`.

When CLI or help changes (`src/program.ts`, `src/help.ts`), run `pnpm generate:skill-cli` and commit `skill/references/cli.md` (D80). Flag drift between sources and generated outputs.

### SQL safety (D93; v1 spec)

Migrations and database code use parameterized SQL. Flag string-concatenated SQL with record or user text.

### Import, merge, and deletion

Unresolved import or merge conflicts must refuse and write nothing (v1 spec). Deletions and erasure must not hide dependents silently (see D124 alternatives and contribution-readiness themes). Flag best-effort merges or silent drops.

### HRV statistics (`AGENTS.md`)

`hrv_sdnn` and `hrv_rmssd` are different statistics. Never combine them in one baseline.

### Thin public surface (D121, D7)

The approved agent interface is CLI plus Skill — not a local HTTP or MCP server added without a new recorded decision and concrete need. Flag new maintained ports, services, or heavy dependencies that expand scope before demand is documented.

### build-history integrity (`AGENTS.md`, `build-history/README.md`)

`build-history/` is append-only. New decisions get new D-numbers in the current version’s `decisions.md`; do not rewrite earlier entries or add addendum files. Long research stays in `appendix/` and links from the D-number.

## Medium-severity checks

### New decisions

If the PR changes format, governance, migration behavior, public SQL/TS contracts, or other recorded policy, it should append a D-number with rejected alternatives per `build-history/README.md`. Flag policy encoded only in code or comments.

### Impersonal repository (`AGENTS.md`, D123)

No personal objectives, audience framing, or individual names in committed content. **Exception:** `@mwhitham` in `CODEOWNERS` and in decision or plan text that explains `CODEOWNERS` (D123). Flag other personal names or handles.

### Protected contracts (D123)

Changes under `spec/`, `src/db/migrations/`, `src/v1/schema.ts`, `schema/`, `.github/workflows/release-*.yml`, or `community/policy.json` need explicit owner review per D123. Call this out even when automated checks pass.

### DCO (D122)

Contributions require Developer Certificate of Origin sign-off (`Signed-off-by` on commits). Flag commits without it.

### Verification (`AGENTS.md`, `CONTRIBUTING.md`)

Before merge, `pnpm check:public-data && pnpm typecheck && pnpm test && pnpm build` must pass. Note if the PR likely fails these (e.g. hand-edited `examples/demo-athlete/`).

## Low-severity / defer

Prose in `build-history/` should follow D24 (rule then reason; plain words). Package naming: Athletic Standard / `athleticstandard` / `ath` / `AthleticStandard` per `AGENTS.md`. Do not block merge on taste when contracts and data safety are sound.

## Review output

- Tie each important comment to a source path or D-number.
- Prefer concrete failure modes (“this path could write on conflict”) over generic advice.
- Suggest fixes only when they follow the cited rule.
