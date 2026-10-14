# Contribution readiness plan

Status: direction approved. Section 7 records the approved choices. Each
format choice still gets a D-number in `build-history/v1.0.0/decisions.md`
when its implementation pull request starts.

## Plain-English summary

Athletic Standard stays small.
The product is one local SQLite file, a public specification, and the existing CLI and Skill.
The file enforces its important rules even when another program writes it.
Builders may read and write it directly with ordinary SQLite tools.
They do not need a Node service or an Athletic Standard language library.
Agents get machine-readable rules, examples, test vectors, and a build-on-ATH Skill.
The first change adds a safe place for namespaced extension data.
The same change moves critical record rules into SQLite constraints and triggers.
That schema work must land before v1.0.0 is published to npm.
Next come plain SQL migrations, stable read views, and documented writable tables.
Conformance tests then show any app or agent whether its implementation is correct.
The existing npm package gains clear internal boundaries without breaking its public name.
Reviewed skills, importers, and calculations may ship in the main package.
Catalog inclusion means tests passed; it is not an endorsement or hosted approval.
HTTP, MCP, Rust, Swift libraries, and other ports wait until a real builder needs them.

## 1. Goal

Make the public Athletic Standard repository safe and practical to build on.
A builder should be able to read and write the SQLite record without running
Node or waiting for a maintained language library. A TypeScript app should
have stable package entry points. Contributors should have clear ways to add
skills, provider importers, and calculations.

Accepted public work may later ship in `athleticstandard`. A private hosted
product can then adopt it by updating its exact package pin. The public
repository must not depend on private code or private checks.

This plan preserves the current product boundary:

- SQLite is the live local record.
- JSON is the complete portable package.
- The two-tier wall remains structural. Measured records require a source.
  Athlete reports cannot claim one.
- The public package is not an app, a coach, or medical advice.
- Community entries are not fetched or run merely because they appear in a
  catalog.
- Forks may add other runtimes or loading models. Official Athletic Standard
  and the hosted product run only reviewed code compiled into releases.
- The open-source base stays thin: the file, specification, conformance
  vectors, CLI, and Skills.

## 2. Why this plan is outside a version folder

`build-history/README.md` names version folders after the format version they
produce. This plan covers repository work as well as the remaining v1 format
work. The approved extension envelope and database-enforced rules belong to
v1.0.0 because npm still publishes 0.4.3.

As implementation begins:

1. Append the approved format decisions after D114 in
   `build-history/v1.0.0/decisions.md`.
2. Update `build-history/v1.0.0/spec.md` and `progress.md` before the matching
   schema implementation lands.
3. Put later format changes in the folder named for the version they produce.
4. Keep this proposal as the record of the work considered before approval.

## 3. Verified starting point

This review uses `main` after pull request
[#14](https://github.com/mwhitham/ath/pull/14) merged.

| Area | What exists now | Readiness gap |
|---|---|---|
| SQLite versions | `database_metadata.sql_schema_version` and `PRAGMA user_version` are both checked in `src/db/migrate.ts` and `src/db/check.ts`. | `src/db/migrations/001-initial.ts` contains the only schema as a TypeScript string. `migrateDatabase` throws for every version other than 1. There is no standalone SQL contract or forward migration runner. |
| Record schemas | `src/v1/schema.ts` is the public record definition. Generated JSON Schemas live in `schema/1.0.0/`. | The SQL schema and JSON Schema can drift without one check that compares all generated public artifacts. |
| Cross-language use | A standard SQLite file and complete JSON package exist. | There are no language-neutral query examples, conformance vectors, stable writable-table contract, or tested direct-write transaction recipe. |
| Pure logic | `src/v1/schema.ts` and some calculation helpers are independent of file access. | Database calculations live in `src/db/calculate.ts`. Import, series, model, key, file, and package paths use Node file APIs. The published package requires Node 20. |
| Local API | The CLI and Skill expose named operations. | No local HTTP or MCP service is planned. Direct SQLite, the CLI, and Skills are the approved thin public interfaces. |
| Package API | `package.json` already has `.`, `./database`, `./schema`, JSON Schema, and Skill entry points. `tests/v1-package-exports.test.ts` checks them. | `src/index.ts` uses `export *`. There is no public API policy, API reference, deprecation rule, or importer entry point. |
| Importers | Apple Health, WHOOP, and Oura parsing feeds the common `ImportPayload` in `src/import/merge.ts`. | `src/import/index.ts` uses `VENDOR_LABELS` and nested format branches. `ImportPayload` and `mergePayload` are not public. There is no importer interface or registry. |
| Skills | `skill/` contains one versioned Skill. `src/skill.ts` installs and updates that one directory. | There is no selected multi-skill loader or build-on-ATH Skill. D41 is superseded by the approved explicit local-install rule. |
| CLI and services | Tests can replace model completion and model catalog functions. | `src/program.ts` writes to the console and process state throughout. `src/update.ts`, `src/keyring.ts`, and `src/gateway.ts` hold separate package, keyring, URL, and site constants. |
| Extensions | Strict Zod objects and refusal of unknown package fields prevent silent data loss. | Records have no namespaced extension field. Adding one without tier and preservation rules could weaken the two-tier wall. |
| Contribution checks | Release-only cross-platform and performance workflows exist. | There is no pull-request check on `main`. Pull request #5 adds it, public-data checks, and a first `CONTRIBUTING.md`. It should land before this plan is executed. |
| Repository guidance | `AGENTS.md`, `README.md`, `SPEC.md`, and the build history explain the current release. | There is no contributor-facing architecture guide, extension guide, public API guide, build-on-ATH Skill, issue template, pull request template, or `CODEOWNERS`. |
| Site metadata | Repository and issue links are present in `package.json`. | The homepage is `https://athleticstandard.ai`; the intended public site is `https://ath.fit`. |

The earlier review was wrong in two places:

- A schema-version table is not wholly missing. `database_metadata` already
  carries `sql_schema_version`, with `PRAGMA user_version` as a local mirror.
  The missing part is a public SQL migration chain.
- Subpath exports are not wholly missing. The missing part is a stated and
  documented stability boundary.

Additional gaps found during review:

- Provider import still passes through the old model in `src/schema.ts` before
  `src/db/provider-import.ts` maps it into v1 records. This leaves two domain
  models in active maintenance.
- `src/import/index.ts#importExport` is an unused 0.x JSON write path. It makes
  the supported importer contract unclear.
- `src/models.ts#defaultModelPath` only replaces `.json`, even though the live
  file now ends in `.ath.sqlite`.
- The public JavaScript database handle is opaque under D104, but direct
  SQLite writes are now approved. Critical correctness rules therefore need
  to move from Node-only checks into SQL constraints and triggers.
- The release workflows and the proposed pull-request workflow in #5 use
  different Node and action versions. They should share one supported version
  policy after #5 lands.

## 4. Recommended end state

Use a workspace with small packages, but reach it through compatibility
entry points. Do not make the repository move and public package split one
large pull request.

```text
spec/
  README.md
  json-schema/
  sql/
    migrations/
  conformance/
packages/
  core/          # @ath/core: schemas, values, pure transforms, API version
  store/         # @ath/store: SQLite operations and file adapters
  importers/     # @ath/importers: interface, registry, built-in importers
src/             # athleticstandard compatibility facade and ath CLI
skills/
  build-on-ath/  # agent-facing guide for direct SQLite and extension work
community/
  schema/
  skill/<id>/
  importer/<id>/
  calculation/<id>/
  community-catalog.json
```

The existing `athleticstandard` npm name and `ath` command remain supported.
The root package re-exports the supported APIs from the workspace packages.
The split packages should start as workspace packages. Publishing them under
the `@ath` scope happens only after package ownership and release policy are
confirmed. While they are private workspaces, the root build must bundle their
code into `athleticstandard`; a published package cannot refer to an
unpublished workspace dependency.

The `spec/` directory is language-neutral. Generated files may also remain at
their current package paths for compatibility. One generator writes both
locations and one drift check verifies them.

### 4.1 Contract boundaries

There are four different contracts. Their versions must not be conflated.

1. `athleticstandard_version` versions athlete-owned record content.
2. `sql_schema_version` versions the SQLite encoding and migrations.
3. `CORE_API_VERSION` versions the public extension interfaces.
4. The npm package version versions shipped TypeScript and CLI behavior.

An npm release may change without changing the record format. An additive SQL
index may change the SQL version without changing record content. A breaking
importer interface changes `CORE_API_VERSION`. Each conformance vector names
the contract and version it tests. Record, SQL, and core API vectors do not
share one version.

### 4.2 Direct SQLite builders

Direct SQLite reads and writes are public interfaces:

- Publish ordered `.sql` migration files.
- Publish the required SQLite version and compile options.
- Publish stable read views. Views give clients one versioned read contract
  inside SQLite. Example queries alone do not.
- Publish a small set of stable writable tables. Every other table is
  internal.
- Require every client connection to run `PRAGMA foreign_keys = ON`.
- Back critical cross-table rules with triggers because foreign-key
  enforcement is set per connection.
- Enforce field shape with `STRICT`, `CHECK`, `NOT NULL`, `UNIQUE`, foreign
  keys, and SQLite JSON functions.
- Enforce the two-tier wall, revision order, current-revision pointers,
  deletion state, and protected links in the database.
- Publish JSON decoding fixtures for `record_revisions.record_json`.
- Publish transaction recipes and conformance tests for direct writers.

No hand-maintained Swift, Rust, or other language library is required for
v1. Agents and builders can generate one from the machine-readable contract
when needed.

### 4.3 Community code and hosted promotion

The catalog describes reviewed source in this repository. It is not a remote
plugin store. Catalog generation never downloads or runs an entry.

Skills remain passive instructions. Official Athletic Standard and the hosted
product run only reviewed importers and calculations compiled into the main
`athleticstandard` package. The default runtime never loads arbitrary code
from a community directory. Forks are free to choose another model. Promotion
into the hosted product follows this order:

1. Merge the public contribution with synthetic tests.
2. Release an exact public package version.
3. Update the hosted product's exact package pin.
4. Run the hosted product's own checks.

Catalog inclusion records passed public tests only. It is not endorsement and
does not approve hosted promotion. This keeps the public package as the source
reviewed by both products without copying an untracked private variant.

## 5. Pull request sequence

Every pull request below is independently mergeable. Each one must preserve
the current `athleticstandard` imports and `ath` commands unless its breaking
change is expressly approved.

### PR 0 — Land the health-data safeguard first

Dependency, not product implementation. It lands first because every later
pull request needs its health-data and CI checks. PR 1 remains the first
product and schema change.

Scope:

- Merge [#5](https://github.com/mwhitham/ath/pull/5) after it is rebased.
- Treat its `.github/workflows/ci.yml`, `CONTRIBUTING.md`,
  `scripts/check-public-data.ts`, `.gitignore`, `AGENTS.md`, and package script
  as the starting point.
- Rebase each later pull request after #5.
- Treat [#13](https://github.com/mwhitham/ath/pull/13) as superseded by merged
  #14. It has no remaining change to carry into this plan.

Files touched: only the files already owned by #5.

Acceptance:

- Pull requests run public-data, type, test, and build checks.
- Branch rules require the check after it exists on `main`.
- Synthetic fixtures remain allowlisted.

Risk: a stale rebase could drop the v1 fixture exceptions added by #14.
Review the combined `.gitignore` and public-data allowlist.

### PR 1 — Finish the v1 schema before npm publish

This pull request is the first implementation step. It must merge before
v1.0.0 is published to npm.

Scope:

- Add an optional, namespaced, size-limited `extensions` envelope to records.
- Make extension values inherit the parent record's measured or
  athlete-reported tier.
- Preserve unknown extension values through SQLite and complete JSON package
  round trips.
- Add `STRICT`, `CHECK`, `NOT NULL`, `UNIQUE`, and foreign-key constraints
  wherever SQLite can enforce the current v1 rules.
- Add triggers for critical cross-table rules, including:
  - measured records require a valid source;
  - athlete-reported records cannot claim a source;
  - record and revision types agree;
  - revision numbers and previous-revision links are valid;
  - current-revision pointers identify the current retained revision;
  - deletion and erasure state cannot contradict current records;
  - protected evidence and result links keep valid record types.
- Validate stored JSON shape with SQLite JSON functions.
- Add direct-SQL tests that bypass Node operations and prove invalid writes
  fail in SQLite.
- Append the approved v1 decisions after D114 and update the v1 spec and
  progress before implementation.

Files touched:

- `build-history/v1.0.0/decisions.md`
- `build-history/v1.0.0/spec.md`
- `build-history/v1.0.0/progress.md`
- `src/v1/schema.ts`
- `schema/1.0.0/**`
- `src/db/migrations/**`
- `src/db/records.ts`
- `src/db/import-package.ts`
- `src/db/export.ts`
- `SPEC.md`
- schema, direct-SQL, package round-trip, and safety tests

Acceptance:

- npm still reports 0.4.3 when this work starts.
- The v1 schema includes the extension envelope before v1.0.0 is published.
- Unknown extension values survive database and package round trips.
- Extension limits reject oversized or deeply nested values.
- Direct SQL cannot bypass the two-tier wall or critical revision, deletion,
  erasure, and link rules.
- Every measured write still identifies its source.
- Every athlete-reported write is structurally unable to claim a source.
- Generated JSON Schemas and fixtures are current.

Risk: triggers can reject valid bulk, restore, or migration work when their
transaction ordering differs from normal writes. Test creation, correction,
package restore, provider import, deletion, erasure, and migration through
both Node and direct SQL.

### PR 1A — Add contribution governance

Scope:

- Expand #5's `CONTRIBUTING.md` rather than replacing it.
- Add `.github/PULL_REQUEST_TEMPLATE.md`,
  `.github/ISSUE_TEMPLATE/bug.yml`,
  `.github/ISSUE_TEMPLATE/importer.yml`,
  `.github/ISSUE_TEMPLATE/calculation.yml`, and
  `.github/ISSUE_TEMPLATE/skill.yml`.
- Add `.github/CODEOWNERS` with `@mwhitham` for `spec/`, migrations, schemas,
  release workflows, and community policy. Other areas may be reviewed by any
  maintainer.
- Use these exact owner patterns:
  - `/spec/ @mwhitham`
  - `/src/db/migrations/ @mwhitham`
  - `/src/v1/schema.ts @mwhitham`
  - `/schema/ @mwhitham`
  - `/.github/workflows/ @mwhitham`
  - `/community/policy.json @mwhitham`
- Do not add a catch-all owner. Normal repository permissions let any
  maintainer review every other path.
- Add DCO sign-off instructions and a required sign-off check.
- Keep MIT and do not add a contributor license agreement.

Files touched:

- `CONTRIBUTING.md`
- `.github/**`
- documentation tests

Acceptance:

- Each issue form asks for synthetic data only.
- The pull request template requires fixture safety and generated-file checks.
- DCO sign-off is required, MIT remains the license, and there is no
  contributor license agreement.
- `@mwhitham` owns the approved protected areas. Other paths do not require
  that owner specifically.
- No private repository name or private process is required by a public
  contributor.

Risk: governance files can block all contributions when owner names or DCO
rules are wrong. Land required checks only after a test pull request passes.

### PR 1B — Add architecture and build-on guides

Scope:

- Add `ARCHITECTURE.md`, `docs/build-on-ath.md`, and
  `docs/extensions.md`.
- Document the current package, record, store, importer, and CLI
  boundaries before moving files.
- Change `package.json#homepage` and the gateway referrer in
  `src/gateway.ts` to `https://ath.fit`.
- Document extension promotion. An extension becomes an official field only
  after repeated real use, stable meaning across builders, clear tier rules,
  and a separately approved format change.

Files touched:

- `ARCHITECTURE.md`
- `docs/build-on-ath.md`
- `docs/extensions.md`
- `README.md`
- `package.json`
- `src/gateway.ts`
- documentation tests

Acceptance:

- A new contributor can identify each current boundary and its planned
  package.
- The build-on guide states that direct SQLite reads and writes are supported.
- It tells clients to enable `PRAGMA foreign_keys` and names the constraints
  and triggers that remain authoritative.
- Public links use `ath.fit`.
- `docs/extensions.md` explains that linked extension records may be added
  later and that popular extensions require a new format decision before
  promotion to official fields.
- No private repository is required to follow the guide.

Risk: an architecture guide can promise the target before it exists. Mark
current behavior and planned behavior separately.

### PR 2 — Publish the SQLite contract as plain SQL

Scope:

- Create `spec/sql/migrations/001-initial.sql` from the completed v1 schema in
  `src/db/migrations/001-initial.ts`.
- Keep `database_metadata.sql_schema_version` as the version table. Do not add
  a second source of truth.
- Define ordered migration names, transaction rules, rollback behavior,
  minimum SQLite version, and the relation to `PRAGMA user_version`.
- Change the Node migration runner to read or import generated SQL rather than
  owning a separate string.
- Add a migration ledger test that creates every supported schema from zero
  and upgrades every retained prior version.
- Add a drift check between migration files, a newly created database, and the
  expected table, index, trigger, and view list.
- Add stable SQL read views and document their columns and version policy.
- Name the small set of stable writable tables and their required transaction
  order. Mark every other table internal.
- Publish the constraint and trigger contract in machine-readable form.
- Add `ath migrate` with preview, backup, and explicit confirmation for
  agents and scripts.
- On interactive database open, preview the migration and ask the person to
  confirm. Never migrate in a noninteractive open.
- Create and verify a backup before applying any migration.

Files touched:

- `spec/sql/README.md`
- `spec/sql/migrations/001-initial.sql`
- `src/db/migrations/**`
- `src/db/migrate.ts`
- `src/db/check.ts`
- `src/program.ts`
- `src/help.ts`
- `scripts/check-schema-drift.ts`
- `tests/v1-db.test.ts`
- direct-write and migration tests
- `skill/references/cli.md`
- `package.json`

Acceptance:

- `sqlite3` can create the schema from published SQL without TypeScript.
- A database made by plain SQL has the same schema as one made by
  `createDatabase`.
- The Node runner can migrate from each retained schema version.
- A changed table without a migration or drift update fails CI.
- Metadata and `PRAGMA user_version` agree after each migration.
- Plain `sqlite3` can use every stable read view and writable table.
- Direct clients are told to enable `PRAGMA foreign_keys`.
- Critical rules still fail through triggers when a test connection omits that
  pragma.
- `ath migrate` previews, backs up, and confirms before writing.
- Interactive open prompts after a backup is ready. Noninteractive open
  refuses and points to `ath migrate`.
- A failed migration leaves the original database usable and the backup
  verified.

Risk: the initial migration currently relies on parameterized metadata inserts
after table creation. The SQL contract must state which values a runner
supplies. It must not embed a fixed database ID or creation time. Backup and
prompt code must also work for WAL files and interrupted migrations.

### PR 3 — Add language-neutral conformance fixtures

Scope:

- Add `spec/conformance/` with small JSON, SQL, and expected-result fixtures.
- Cover timestamps, UUIDv7 IDs, record revisions, deletions, source links,
  dense samples, package order, calculations, direct writes, constraints,
  triggers, stable views, writable tables, and migration results.
- Generate fixtures from fixed inputs. Do not hand-edit the generated outputs.
- Add one runner that checks fixture hashes and expected results.
- Add CI on Linux, macOS, and Windows for the language-neutral contract. Reuse
  the release platform matrix rather than creating a conflicting matrix.
- Add machine-readable contract metadata for required pragmas, stable views,
  writable tables, internal tables, constraints, triggers, and transaction
  recipes.
- Update `AGENTS.md` with the direct-write and generated-artifact rules.
- Add `skills/build-on-ath/SKILL.md`. It teaches agents to inspect the
  machine-readable contract, generate only the client code a project needs,
  and run conformance before writing athlete data.
- Package the build-on-ATH Skill without changing the existing Skill path.

Files touched:

- `spec/conformance/**`
- `scripts/generate-conformance.ts`
- `scripts/check-conformance.ts`
- `tests/conformance.test.ts`
- `AGENTS.md`
- `skills/build-on-ath/**`
- `docs/build-on-ath.md`
- `.github/workflows/ci.yml`
- `.github/workflows/release-cross-platform.yml`
- `package.json`

Acceptance:

- Another language can implement the tests without importing npm code.
- The fixture generator is deterministic.
- SDNN and RMSSD have separate vectors and expected values.
- Measured and athlete-reported fixtures prove the two-tier wall.
- Direct-write vectors pass without importing npm code.
- The build-on-ATH Skill contains no hand-maintained language client.
- An agent can find required pragmas, stable reads, stable writes, and tests
  from the machine-readable contract.
- CI fails on changed generated output.

Risk: fixtures can accidentally describe current implementation bugs as the
contract. Each vector needs a short rule citation from `SPEC.md` or an approved
decision.

### PR 4 — Lock the v1.0.0 npm publish gate

Scope:

- Add packed-tarball tests for every published JavaScript and specification
  entry point.
- Verify the v1 extension schema, SQL enforcement, migrations, stable views,
  writable tables, conformance files, and both official Skills ship in the
  tarball.
- Add a release check that refuses v1.0.0 when any v1 schema or conformance
  artifact is stale.
- Align pull-request and release workflow runtime versions.
- Add release notes for direct writes, required pragmas, extensions, and
  explicit migrations.

Files touched:

- package tarball tests
- `package.json`
- `.github/workflows/ci.yml`
- release workflows
- release-note template

Acceptance:

- `npm view athleticstandard version` is still 0.4.3 before this gate lands.
- Tests install the packed artifact in an empty project with no workspace
  links.
- The packed v1 package contains the extension envelope and database
  enforcement from PR 1.
- It contains the SQL contract, migration commands, conformance vectors, and
  build-on-ATH Skill from PRs 2 and 3.
- A stale or missing v1 artifact blocks publish.
- All cross-platform and measured v1 release checks pass before npm publish.

Risk: the repository already declares package version 1.0.0. A release command
could publish it before the schema gate is complete. Keep publishing manual
and blocked by this required check until PRs 1 through 4 land.

### PR 5 — Define and document the public core API

Scope:

- Add `CORE_API_VERSION = "1.0.0"`.
- Replace root `export *` statements with an explicit reviewed export list
  while preserving all currently supported names.
- Add stability markers: stable, experimental, and internal.
- Define stable tool IDs, data-group IDs, and the `community.<id>` grammar for
  later manifests.
- Generate an API reference from the actual exported types.
- Add an API report test that requires review for additions and removals.
- Keep deprecated stable APIs working with warnings until the next major npm
  release. That major release may remove them.

Files touched:

- `src/index.ts`
- `src/v1/schema.ts`
- `src/db/index.ts`
- `src/core-api.ts`
- `package.json`
- `tests/v1-package-exports.test.ts`
- `docs/api/**`
- API report configuration and checked output

Acceptance:

- Existing documented imports compile unchanged.
- Each public export has a stability label and reference entry.
- Deep imports outside `package.json#exports` remain unsupported.
- A public removal or type break fails the API report check.
- `CORE_API_VERSION` is available to manifests and runtime checks.
- Stable tool IDs and data-group IDs have one generated public list.

Risk: an explicit list can accidentally remove a schema export that a user
already imports. Compare the built package before and after and keep aliases
for supported names.

### PR 6 — Establish the workspace and extract `@ath/core`

Scope:

- Add `pnpm-workspace.yaml` and shared TypeScript/build settings.
- Move schemas, plain values, deterministic JSON helpers, and other no-I/O
  logic into `packages/core/`.
- Enforce a no-Node-built-ins rule in `@ath/core`.
- Keep root `athleticstandard` and `athleticstandard/schema` exports as
  compatibility wrappers.
- Move algorithm vectors from implementation tests into
  `spec/conformance/algorithms/`.

Files touched:

- `pnpm-workspace.yaml`
- `packages/core/**`
- `src/index.ts`
- `src/v1/schema.ts`
- `tsup.config.ts`
- `tsconfig*.json`
- `package.json`
- `pnpm-lock.yaml`
- affected tests

Acceptance:

- `@ath/core` builds for a browser-like environment with no Node built-ins.
- The current root and schema imports return the same names and behavior.
- The CLI and all tests pass without path aliases that consumers cannot use.
- Algorithm fixtures can be consumed without npm.

Risk: package splitting can introduce duplicate Zod copies or declaration
paths that only work in the workspace. Test packed tarballs, not only source
imports.

### PR 6A — Extract SQLite storage without behavior changes

Scope:

- Move database operations to `packages/store/`.
- Keep the public database handle opaque for JavaScript callers.
- Keep `athleticstandard/database` as a compatibility wrapper.

Files touched:

- `packages/store/**`
- `src/db/**`
- `src/index.ts`
- `tsup.config.ts`
- package export tests

Acceptance:

- Node file, zip, and keyring code is absent from `@ath/core`.
- Existing database imports and CLI behavior remain unchanged.
- Packed `athleticstandard` contains the store code even while `@ath/store`
  remains an unpublished workspace.

Risk: moving modules can change declaration paths or package initialization.
Compare packed exports and database behavior before and after the move.

### PR 6B — Add value and platform adapters

Scope:

- Define clock, random-ID, database-client, file, archive, and secret-store
  adapters.
- Move Node file, zip, and keyring work behind Node adapters.
- Keep local file security checks mandatory in the Node file adapter.
- Inject clock and ID sources into store domain operations.

Files touched:

- `packages/store/src/adapters/**`
- `packages/store/src/**`
- `src/file.ts`
- `src/series.ts`
- `src/import/zip.ts`
- `src/keyring.ts`
- adapter and platform tests

Acceptance:

- Store operations can be tested with fixed clock and ID sources.
- Pure packages contain no Node built-ins.
- File permission and symbolic-link protections pass on supported platforms.
- A missing security capability produces a refusal or explicit warning.

Risk: an adapter can make a required security check look optional. Keep the
safe Node adapter as the CLI default and test every refusal.

### PR 7 — Define the importer interface and registry

Scope:

- Define `Importer`, `ImporterDetection`, `ImportContext`, and
  `ImportPayload` in `packages/importers/`.
- Export payload validation and merge operations through a supported subpath.
- Replace `VENDOR_LABELS` and nested format branches in
  `src/import/index.ts` with a deterministic registry.
- Register Apple Health, WHOOP, and Oura as built-ins.
- Make detection report ambiguity instead of picking the first match.
- Version each importer's matching recipe, preserving D108.

Files touched:

- `packages/importers/**`
- `src/import/**`
- `src/db/provider-import.ts`
- `src/schema.ts`
- `package.json`
- `tsup.config.ts`
- `tests/import*.test.ts`
- `tests/v1-provider-import.test.ts`
- provider fixtures under `tests/fixtures/exports/`

Acceptance:

- A synthetic importer can be registered in a test without editing a switch.
- Duplicate IDs, ambiguous detection, unsupported versions, and invalid
  payloads fail with clear errors.
- Existing Apple, WHOOP, and Oura fixture results do not change.
- No importer writes directly to SQLite.

Risk: changing detection order can change which importer receives an
ambiguous file. Add ambiguity fixtures before replacing the branches.

### PR 7A — Remove the 0.x importer model

Scope:

- Replace `HardSignalT` and `SoftSignalT` in `ImportPayload` with v1 draft
  records.
- Move matching-key creation into the versioned importer result.
- Retire or isolate the unused `src/import/index.ts#importExport` path.
- Remove old model dependencies from `src/db/provider-import.ts`.

Files touched:

- `packages/importers/**`
- `src/import/**`
- `src/db/provider-import.ts`
- `src/schema.ts`
- importer conformance fixtures and tests

Acceptance:

- `ImportPayload` has one v1 model.
- Existing provider record IDs, matching keys, source facts, and receipts do
  not change.
- The supported v1 CLI does not call the old JSON merge path.

Risk: moving off the 0.x intermediate model can change IDs or matching keys.
Lock current results with conformance fixtures before changing the model.

### PR 8 — Define calculation extensions and portable vectors

Scope:

- Separate pure calculation functions from queries in
  `src/db/calculate.ts`.
- Define a calculation descriptor with ID, version, accepted input groups,
  output record type, and deterministic run function.
- Add a registry for built-in calculations.
- Publish source-separated vectors for summaries, baselines, trends,
  correlations, and workout drift.
- Preserve exact input evidence references on every stored result.

Files touched:

- `packages/core/src/calculations/**`
- `packages/store/src/calculate.ts`
- `src/db/calculate.ts`
- `spec/conformance/calculations/**`
- `tests/v1-calculate.test.ts`
- `tests/v1-schema.test.ts`
- package exports and API docs

Acceptance:

- Pure calculations accept values and return values without a database or
  filesystem.
- Query selection remains in the store package.
- Every result can identify exact input record revisions.
- Provider sources never combine in a baseline.
- SDNN and RMSSD remain separate operations and fixtures.

Risk: a generic calculation interface may hide operation-specific safety
limits. Keep per-operation limits and evidence rules in the descriptor and
tests.

### PR 9 — Centralize runtime configuration and fix model paths

Scope:

- Centralize package name, command name, Skill name, keyring service, public
  site, and model gateway settings.
- Fix `src/models.ts#defaultModelPath` for `.ath.sqlite`.
- Keep gateway and update code out of core and store packages.

Files touched:

- `src/program.ts`
- `src/config.ts`
- `src/update.ts`
- `src/keyring.ts`
- `src/gateway.ts`
- `src/models.ts`
- command tests and generated `skill/references/cli.md`

Acceptance:

- Branding and service constants have one reviewed source.
- Model settings are placed correctly beside `.ath.sqlite`.
- Existing command output and gateway behavior do not change.

Risk: changing keyring service names would orphan saved keys. Keep the current
service value as the default and treat any rename as a separate migration.

### PR 9A — Separate the CLI application from output and process state

Scope:

- Introduce an application service that returns typed results and errors.
- Inject output, error output, exit status, clock, configuration, and prompts
  into the CLI layer.
- Remove direct `console.*` and `process.exit*` use from reusable operations.

Files touched:

- `src/program.ts`
- `src/cli.ts`
- `src/application/**`
- command tests
- generated `skill/references/cli.md`

Acceptance:

- Commands can run in tests without changing global process state.
- JSON command output contains no warnings or progress text on standard output.
- Existing command names, options, exit codes, and text remain covered.

Risk: changing output plumbing can break scripts that parse JSON or rely on
exit codes. Snapshot both standard output streams and status for each command.

### PR 10 — Add the community layout and manifest schema

Scope:

- Add `community/<kind>/<id>/` for `skill`, `importer`, and `calculation`.
- Add a strict `community/schema/ath.manifest.schema.json`.
- Require these fields:
  - `id`, in the form `community.<id>`
  - `kind`
  - `version`, as one exact semantic version
  - `coreRange`, as a supported `CORE_API_VERSION` range
  - `formatRange`, as the supported `athleticstandard_version` range
  - `cliRange` for Skills that call `ath`
  - `tools`, limited to stable tool IDs
  - `writeMode`, limited to `read` or `propose`
  - `dataGroups`, limited to documented groups
  - `requiresRawSeries`
  - `license`, as a valid SPDX expression
  - `deprecated`
  - `replacedBy`, required only when there is a replacement
- Also require `name`, `description`, and a kind-specific `entrypoint`.
  Without an entry point, a catalog cannot locate the reviewed source.
- Add one minimal synthetic example for each kind.
- Document that a manifest grants no runtime authority.
- Document that reviewed items approved for official use are compiled into the
  main `athleticstandard` package. There is no third-party runtime loader.

Files touched:

- `community/schema/ath.manifest.schema.json`
- `community/README.md`
- `community/skill/example/**`
- `community/importer/example/**`
- `community/calculation/example/**`
- `docs/extensions.md`
- manifest validation tests

Acceptance:

- Directory kind and ID match the manifest.
- IDs and versions are unique.
- `replacedBy` names an existing compatible item and cannot form a cycle.
- Tool and data-group values come from generated allowlists.
- Every entry has an SPDX-compatible license.
- Examples use only synthetic data and cannot write without confirmation.
- Skills that call the CLI declare both `coreRange` and `cliRange`.
- A manifest cannot make an entry loadable at runtime.

Risk: `coreRange` can claim compatibility that tests do not prove. Catalog
generation must include only versions that pass the contract runner.

### PR 11 — Add isolated community contract checks

Scope:

- Add one contract runner with kind-specific suites.
- Skill checks cover passive language, allowed tool names, `read|propose`
  behavior, and fixture prompts.
- Importer checks cover detection, malformed input, resource limits,
  deterministic payloads, and no direct writes.
- Calculation checks cover deterministic vectors, source separation,
  evidence, and resource limits.
- Add a machine-readable public policy file for blocked capabilities.
- Extend #5's public-data check instead of creating a second health-data
  scanner.
- Run executable importer and calculation checks in a process with no secrets,
  filtered environment variables, network denial, a read-only checkout, one
  temporary writable directory, memory limits, and time limits.

Files touched:

- `community/policy.json`
- `scripts/check-community.ts`
- `tests/community/**`
- `package.json`
- `.github/workflows/ci.yml`

Acceptance:

- A manifest alone cannot cause code execution.
- Entries with blocked tools, undeclared writes, real data, network use, or
  failed fixtures are excluded and fail CI.
- Executable checks cannot read repository secrets, write outside their
  temporary directory, or reach the network.

Risk: text scanning cannot prove a Skill is safe. Treat it as one check, not a
security boundary. Human review and a narrow runtime permission model remain
required.

### PR 11A — Generate the release community catalog

Scope:

- Generate `community/community-catalog.json` from validated manifests and
  passing contract results.
- Check catalog drift on pull requests.
- Regenerate and verify the catalog during release.
- Record item version, core, format, and CLI ranges, license, deprecation, and
  fixture result.
- Mark the catalog as test results only. Catalog presence is not endorsement
  or approval for hosted promotion.

Files touched:

- `community/community-catalog.json`
- `scripts/generate-community-catalog.ts`
- catalog tests
- `package.json`
- `.github/workflows/ci.yml`
- release workflows

Acceptance:

- Catalog order and bytes are deterministic.
- A release fails when the checked catalog is stale.
- Failed or blocked entries never appear in the catalog.
- Catalog generation reads results but never runs entry code.
- Hosted promotion remains a separate review outside catalog generation.

Risk: a generated catalog can be mistaken for approval. Include the
compatibility-only meaning in the file metadata and public docs.

### PR 12 — Support selected multiple skills

D41 is superseded for this narrow case. A community Skill may be installed
only when it is explicitly named.

Scope:

- Generalize `src/skill.ts` from one shipped directory to a list of selected
  skill descriptors.
- Keep `ath skill install` installing only the official bundled Skill by
  default.
- Add explicit commands to list and install a named local community Skill.
- Show source, version, permissions, and destination before installation.
- Never fetch a Skill from the network or install every catalog entry.
- Preserve stale-version reporting per installed Skill.

Files touched:

- `src/skill.ts`
- `src/program.ts`
- `src/help.ts`
- `skill/**`
- `tests/skill.test.ts`
- `tests/cli.test.ts`
- generated `skill/references/cli.md`

Acceptance:

- Current single-Skill behavior is unchanged without new options.
- Installing one Skill does not delete another.
- No read command installs or refreshes a Skill.
- A Skill with an incompatible `coreRange` is refused.
- The user must name each community Skill to install.

Risk: listing community work can look like endorsement. The command and docs
must distinguish compatibility checks from review or recommendation.

## Later, only on demand

Do not build these now:

- A local HTTP or OpenAPI service.
- A local stdio MCP server.
- A maintained Swift library or example app.
- A Rust core or maintained Rust library.
- Any other hand-maintained language port.

A real builder request must name the missing capability before one of these
becomes an active plan. The machine-readable specification and test vectors
let an agent generate a narrow client without waiting for this repository.

A future Rust core remains compatible with this direction. It may ship inside
the npm package and sit behind existing public APIs. The same conformance
vectors must keep its results aligned with other implementations.

## 6. Compatibility and migration

| Change | Breaks existing users? | Migration |
|---|---|---|
| CI, templates, guides, `ath.fit` metadata | No runtime break. New contribution checks can block a pull request. | Rebase and satisfy the documented checks. |
| Plain SQL migration files and database enforcement | No published v1 user exists yet. Invalid direct writes that previously reached internal tables will now fail. | Regenerate repository fixtures. Future v1 databases use `ath migrate` or confirmed interactive open after backup. Published 0.4.3 files remain outside the v1 migration promise under D101. |
| Explicit export list | It can break users only if a current public name is missed. | Preserve the built export list. A deprecated stable API warns and remains available until the next major release. |
| `CORE_API_VERSION` | No. It adds a separate compatibility version for extensions. | Start at `1.0.0`. Increase it by semantic-version rules when the public extension interface changes. |
| Workspace package split | No for supported `athleticstandard` imports and `ath` commands. | Root compatibility entry points re-export the new packages. New apps may adopt `@ath/*` after publishing is approved. |
| Importer v1 payload | It can break undocumented deep imports. No importer API is currently public. | Publish the new subpath and a mapping guide. Keep one temporary adapter if an actual user is found. |
| CLI output separation | It can break scripts if text, JSON, or exit codes change. | Lock current behavior with stream and status tests. Announce any intentional correction. |
| Optional extension envelope | No published v1 user exists yet because npm latest is 0.4.3. It changes the current unreleased v1 schema. | Land it in v1.0.0 before npm publish. Regenerate schemas and fixtures. Every v1 reader preserves unknown extension values. |
| Community manifests and catalog | No. They are additive and passive. | Existing bundled Skill remains the default. |
| Multi-skill install | No when current defaults are preserved. | Community installs are explicit. Existing installed copies continue to report their own versions. |
| Reviewed community code in `athleticstandard` | No for existing exports. New built-ins are additive. | Update the exact package version. Catalog presence alone does not add runtime code. |
| Direct third-party SQLite writes | This is a new supported interface before the first v1 npm publish. | Enable `PRAGMA foreign_keys`, use only stable writable tables, follow documented transactions, and pass conformance tests. SQLite constraints and triggers reject invalid state. |

No pull request may silently rewrite an athlete database merely because the npm
package was updated. Agents and scripts use explicit `ath migrate` with preview
and confirmation. Interactive open may preview and ask a person to confirm.
Both routes create and verify a backup first. Noninteractive open never
migrates.

## 7. Approved decisions

These choices are approved. Record the format and runtime choices as new
D-numbers before their implementation pull requests.

1. **Keep the open-source base thin.** It contains the SQLite file, public
   specification, conformance vectors, existing CLI, and official Skills.
   Builders should not need a service or maintained language library.

2. **Split internal packages first.** Start with private workspace packages
   behind `athleticstandard`. Publish `@ath/*` packages only after a real need
   and stable packed-package checks.

3. **Use semantic core API versions.** `CORE_API_VERSION` starts at `1.0.0`.
   Community `coreRange` values use normal semantic version ranges.

4. **Support direct SQLite writes.** Correctness belongs in the database.
   Use `STRICT`, `CHECK`, `NOT NULL`, `UNIQUE`, foreign keys, JSON checks, and
   triggers. Clients must enable `PRAGMA foreign_keys`. Triggers back critical
   rules that must survive a connection that fails to enable it.

5. **Ship the extension envelope in v1.0.0.** Records receive an optional,
   namespaced, size-limited `extensions` envelope. It inherits the parent
   record's tier and survives every round trip. A minimal pass-through version
   is enough for v1. Separate linked extension records may arrive later as an
   additive change. Popular extensions should become official fields.

6. **Compile reviewed official code into releases.** Official Athletic
   Standard and the hosted product do not load third-party code at runtime.
   Forks remain free to add other loading models.

7. **Treat catalog inclusion as test status only.** It is not endorsement and
   does not approve hosted promotion. Hosted promotion is a separate review.

8. **Supersede D41 for explicit local Skill installation.** A community Skill
   installs only when named. Nothing auto-installs, and the installer never
   fetches a remote Skill.

9. **Use DCO with MIT and no contributor license agreement.** Every
   contribution carries DCO sign-off.

10. **Set protected owners.** `@mwhitham` owns `spec/`, migrations, schemas,
    release workflows, and community policy. Any maintainer may review other
    areas.

11. **Never migrate silently.** Agents and scripts use `ath migrate` with
    preview and confirmation. Interactive open may prompt a person. Every
    route creates and verifies a backup before writing.

12. **Publish stable SQL reads and writes.** Stable views are the read
    contract. A small documented set of tables is writable. All other tables
    are internal.

13. **Keep deprecated stable APIs until the next major release.** Emit
    warnings and document replacements. That major release may remove them.

14. **Ship approved community items in `athleticstandard`.** Separate public
    packages may come later if a concrete need appears.

15. **Defer extra runtimes and services.** Do not build local HTTP, local MCP,
    Swift libraries or examples, Rust, or other maintained ports now. A future
    Rust core may ship through npm behind existing APIs. Shared vectors keep
    any future port honest.

## 8. Release gates

The contribution-ready milestone is complete only when:

- #5 is merged and its required check is enabled.
- The extension envelope and database-enforced critical rules land before
  v1.0.0 is published to npm.
- Current imports and CLI behavior have compatibility tests.
- Plain SQL migrations, stable read views, stable writable tables, and
  language-neutral conformance fixtures are public.
- Direct writes that bypass Node still cannot violate the two-tier wall,
  revision, deletion, erasure, or critical link rules.
- `ath migrate` and interactive migration always preview, confirm, and verify
  a backup. Noninteractive open never migrates.
- `AGENTS.md` and the build-on-ATH Skill point agents to the machine-readable
  contract and conformance runner.
- `CORE_API_VERSION` is `1.0.0`.
- Deprecated stable APIs warn and remain available until the next major
  release.
- Public APIs have explicit exports, stability labels, and generated docs.
- Importers and calculations use registries with deterministic synthetic
  contract tests.
- Community manifests are strict and the catalog is generated.
- Community entries never run merely because they are cataloged.
- Extension behavior preserves the two-tier wall and unknown data.
- The main `athleticstandard` tarball contains every approved official item
  and all promised specification files.
- Release notes state package, core API, SQL, and format versions separately.

HTTP, MCP, Rust, Swift, and other maintained ports are not release gates. They
remain deferred until a real builder asks for a capability the thin base does
not provide.
