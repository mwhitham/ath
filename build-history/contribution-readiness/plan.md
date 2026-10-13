# Contribution readiness plan

Status: proposal only. This document does not approve a format, package, or
governance change.

## 1. Goal

Make the public Athletic Standard repository safe and practical to build on.
A Swift fitness app should be able to read the SQLite record without running
Node. A TypeScript app should have stable package entry points. Contributors
should have clear ways to add skills, provider importers, and calculations.

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

## 2. Why this plan is outside a version folder

`build-history/README.md` names version folders after the format version they
produce. This work has open questions that may produce no format change, an
additive format release, or a breaking release. Giving it a version now would
answer those questions before they are reviewed.

Once the decisions in section 7 are made:

1. Append the approved decisions after D114 in the decisions file for the
   release that will carry them.
2. Put any approved format work in a folder named for that format version.
3. Add its build steps to that version's `progress.md`.
4. Keep this proposal as the record of the work considered before approval.

## 3. Verified starting point

This review uses `main` after pull request
[#14](https://github.com/mwhitham/ath/pull/14) merged.

| Area | What exists now | Readiness gap |
|---|---|---|
| SQLite versions | `database_metadata.sql_schema_version` and `PRAGMA user_version` are both checked in `src/db/migrate.ts` and `src/db/check.ts`. | `src/db/migrations/001-initial.ts` contains the only schema as a TypeScript string. `migrateDatabase` throws for every version other than 1. There is no standalone SQL contract or forward migration runner. |
| Record schemas | `src/v1/schema.ts` is the public record definition. Generated JSON Schemas live in `schema/1.0.0/`. | The SQL schema and JSON Schema can drift without one check that compares all generated public artifacts. |
| Cross-language use | A standard SQLite file and complete JSON package exist. | There are no language-neutral query examples, conformance vectors, or Swift examples. Direct writers outside Node have no tested transaction recipe. |
| Pure logic | `src/v1/schema.ts` and some calculation helpers are independent of file access. | Database calculations live in `src/db/calculate.ts`. Import, series, model, key, file, and package paths use Node file APIs. The published package requires Node 20. |
| Local API | The CLI exposes named operations. | There is no local HTTP API, OpenAPI document, or stdio MCP server. D7 chose CLI plus Skill over MCP, so MCP requires a new decision rather than being assumed work. |
| Package API | `package.json` already has `.`, `./database`, `./schema`, JSON Schema, and Skill entry points. `tests/v1-package-exports.test.ts` checks them. | `src/index.ts` uses `export *`. There is no public API policy, API reference, deprecation rule, or importer entry point. |
| Importers | Apple Health, WHOOP, and Oura parsing feeds the common `ImportPayload` in `src/import/merge.ts`. | `src/import/index.ts` uses `VENDOR_LABELS` and nested format branches. `ImportPayload` and `mergePayload` are not public. There is no importer interface or registry. |
| Skills | `skill/` contains one versioned Skill. `src/skill.ts` installs and updates that one directory. | There is no selected multi-skill loader. D41 expressly deferred a registry and community distribution, so a new decision is required. |
| CLI and services | Tests can replace model completion and model catalog functions. | `src/program.ts` writes to the console and process state throughout. `src/update.ts`, `src/keyring.ts`, and `src/gateway.ts` hold separate package, keyring, URL, and site constants. |
| Extensions | Strict Zod objects and refusal of unknown package fields prevent silent data loss. | Records have no namespaced extension field. Adding one without tier and preservation rules could weaken the two-tier wall. |
| Contribution checks | Release-only cross-platform and performance workflows exist. | There is no pull-request check on `main`. Pull request #5 adds it, public-data checks, and a first `CONTRIBUTING.md`. It should land before this plan is executed. |
| Repository guidance | `AGENTS.md`, `README.md`, `SPEC.md`, and the build history explain the current release. | There is no contributor-facing architecture guide, extension guide, public API guide, or Swift build-on example. There are no issue templates, pull request template, or `CODEOWNERS`. |
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
- The public database handle is intentionally opaque under D104. A direct
  Swift writer therefore needs a separately approved write contract. Reading
  SQLite does not imply that third-party direct writes are safe.
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
  openapi/
  conformance/
packages/
  core/          # @ath/core: schemas, values, pure transforms, API version
  store/         # @ath/store: SQLite operations and file adapters
  importers/     # @ath/importers: interface, registry, built-in importers
  server/        # @ath/server: optional loopback HTTP service
src/             # athleticstandard compatibility facade and ath CLI
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
the contract and version it tests. Record, SQL, core API, and local API vectors
do not share one version.

### 4.2 Swift and other non-JavaScript clients

The first supported route is direct SQLite reading:

- Publish ordered `.sql` migration files.
- Publish the required SQLite version and compile options.
- Publish stable read views. Views give clients one versioned read contract
  inside SQLite. Example queries alone do not.
- Publish JSON decoding fixtures for `record_revisions.record_json`.
- Add a small Swift example using one library chosen in section 7.

Direct SQLite writing is a separate promise. Until its transaction, revision,
link, erasure, and migration rules pass cross-language tests, the guide must
label it unsupported. Non-JavaScript writers can use the local HTTP API if
that API is approved.

### 4.3 Community code and hosted promotion

The catalog describes reviewed source in this repository. It is not a remote
plugin store. Catalog generation never downloads or runs an entry.

Skills remain passive instructions. Importers and calculations are compiled
as part of a reviewed package release. The default runtime never loads
arbitrary JavaScript from a community directory. Promotion into the hosted
product follows this order:

1. Merge the public contribution with synthetic tests.
2. Release an exact public package version.
3. Update the hosted product's exact package pin.
4. Run the hosted product's own checks.

This keeps the public package as the source reviewed by both products. It also
avoids copying an untracked private variant.

## 5. Pull request sequence

Every pull request below is independently mergeable. Each one must preserve
the current `athleticstandard` imports and `ath` commands unless its breaking
change is expressly approved.

### PR 0 — Land the health-data safeguard first

Dependency, not duplicate work.

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

### PR 1 — Add contribution governance

Scope:

- Expand #5's `CONTRIBUTING.md` rather than replacing it.
- Add `.github/PULL_REQUEST_TEMPLATE.md`,
  `.github/ISSUE_TEMPLATE/bug.yml`,
  `.github/ISSUE_TEMPLATE/importer.yml`,
  `.github/ISSUE_TEMPLATE/calculation.yml`, and
  `.github/ISSUE_TEMPLATE/skill.yml`.
- Add `.github/CODEOWNERS` after owners are confirmed.
- Add DCO sign-off instructions and a sign-off check if section 7 approves
  DCO.

Files touched:

- `CONTRIBUTING.md`
- `.github/**`
- documentation tests

Acceptance:

- Each issue form asks for synthetic data only.
- The pull request template requires fixture safety and generated-file checks.
- DCO and owner checks match the decisions in section 7.
- No private repository name or private process is required by a public
  contributor.

Risk: governance files can block all contributions when owner names or DCO
rules are wrong. Land required checks only after a test pull request passes.

### PR 1A — Add architecture and build-on guides

Scope:

- Add `ARCHITECTURE.md`, `docs/build-on-ath.md`, and
  `docs/extensions.md`.
- Document the current package, record, store, importer, CLI, and service
  boundaries before moving files.
- Change `package.json#homepage` and the gateway referrer in
  `src/gateway.ts` to `https://ath.fit`.

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
- The build-on guide labels direct SQLite writes unsupported.
- Public links use `ath.fit`.
- No private repository is required to follow the guide.

Risk: an architecture guide can promise the target before it exists. Mark
current behavior and planned behavior separately.

### PR 2 — Publish the SQLite contract as plain SQL

Scope:

- Create `spec/sql/migrations/001-initial.sql` from
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

Files touched:

- `spec/sql/README.md`
- `spec/sql/migrations/001-initial.sql`
- `src/db/migrations/**`
- `src/db/migrate.ts`
- `src/db/check.ts`
- `scripts/check-schema-drift.ts`
- `tests/v1-db.test.ts`
- `package.json`

Acceptance:

- `sqlite3` can create the schema from published SQL without TypeScript.
- A database made by plain SQL has the same schema as one made by
  `createDatabase`.
- The Node runner can migrate from each retained schema version.
- A changed table without a migration or drift update fails CI.
- Metadata and `PRAGMA user_version` agree after each migration.

Risk: the initial migration currently relies on parameterized metadata inserts
after table creation. The SQL contract must state which values a runner
supplies. It must not embed a fixed database ID or creation time.

### PR 3 — Add language-neutral conformance fixtures

Scope:

- Add `spec/conformance/` with small JSON, SQL, and expected-result fixtures.
- Cover timestamps, UUIDv7 IDs, record revisions, deletions, source links,
  dense samples, package order, calculations, and migration results.
- Generate fixtures from fixed inputs. Do not hand-edit the generated outputs.
- Add one runner that checks fixture hashes and expected results.
- Add CI on Linux, macOS, and Windows for the language-neutral contract. Reuse
  the release platform matrix rather than creating a conflicting matrix.

Files touched:

- `spec/conformance/**`
- `scripts/generate-conformance.ts`
- `scripts/check-conformance.ts`
- `tests/conformance.test.ts`
- `.github/workflows/ci.yml`
- `.github/workflows/release-cross-platform.yml`
- `package.json`

Acceptance:

- Another language can implement the tests without importing npm code.
- The fixture generator is deterministic.
- SDNN and RMSSD have separate vectors and expected values.
- Measured and athlete-reported fixtures prove the two-tier wall.
- CI fails on changed generated output.

Risk: fixtures can accidentally describe current implementation bugs as the
contract. Each vector needs a short rule citation from `SPEC.md` or an approved
decision.

### PR 4 — Define and document the public core API

Scope:

- Add `CORE_API_VERSION` as an exact semantic version.
- Replace root `export *` statements with an explicit reviewed export list
  while preserving all currently supported names.
- Add stability markers: stable, experimental, and internal.
- Define stable tool IDs, data-group IDs, and the `community.<id>` grammar for
  later manifests.
- Generate an API reference from the actual exported types.
- Add an API report test that requires review for additions and removals.
- Apply the deprecation and support policy chosen in section 7.

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

### PR 5 — Establish the workspace and extract `@ath/core`

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
- Algorithm fixtures can be consumed from Swift without npm.

Risk: package splitting can introduce duplicate Zod copies or declaration
paths that only work in the workspace. Test packed tarballs, not only source
imports.

### PR 6 — Extract SQLite storage without behavior changes

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

### PR 6A — Add value and platform adapters

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

### PR 6B — Publish stable SQLite read views

Scope:

- Add versioned SQL views for supported cross-language reads.
- Document each view's columns, JSON fields, ordering, and version policy.
- Add drift and query-plan tests against the demo database.
- Keep direct third-party writes unsupported.

Files touched:

- `spec/sql/migrations/**`
- `spec/sql/README.md`
- `packages/store/src/migrations/**`
- `tests/v1-db.test.ts`
- `spec/conformance/sql/**`
- `docs/build-on-ath.md`

Acceptance:

- Plain `sqlite3` can run every documented read against the demo database.
- View changes require a SQL migration and contract-vector update.
- Raw storage tables remain documented as internal encoding details.
- The views expose source IDs for every measured value.

Risk: a stable view becomes a long-term contract. Keep the first set small and
limited to real app read needs.

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

### PR 10 — Decide and add namespaced record extensions

Do not start this pull request until the extension decision in section 7 is
approved.

Scope:

- Add an optional extension envelope keyed by validated community IDs.
- Set JSON value depth, byte, and collection limits.
- Make an extension inherit the tier and source rules of its parent record.
- Preserve unknown extension values through SQLite and JSON export and import.
  Deterministic export may normalize JSON bytes while preserving the value.
- Require a published extension schema for entries accepted into this
  repository.
- Refuse an extension that attempts to replace core measured quantities,
  evidence links, source fields, revisions, or deletion state.
- Add migration and old-reader behavior for the chosen format version.

Files touched:

- `packages/core/src/schema/**`
- `src/v1/schema.ts`
- `schema/<approved-version>/**`
- `spec/json-schema/**`
- `spec/sql/migrations/**` if storage changes
- `src/db/records.ts`
- `src/db/export.ts`
- `src/db/import-package.ts`
- schema, round-trip, and safety tests
- `SPEC.md`

Acceptance:

- Strict validation remains in force outside the extension envelope.
- Measured records still require a source. Athlete reports still cannot claim
  one.
- Unknown extension values survive a complete round trip or cause an explicit
  refusal. They are never silently dropped.
- Limits prevent deeply nested or oversized extension data.
- An older reader's behavior is documented and tested.

Risk: arbitrary JSON can become a second ungoverned record model. The envelope
must not be approved until its tier, size, naming, and promotion rules are
clear.

### PR 11 — Add the community layout and manifest schema

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

Risk: `coreRange` can claim compatibility that tests do not prove. Catalog
generation must include only versions that pass the contract runner.

### PR 12 — Add isolated community contract checks

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
- Entries with blocked tools, direct-write claims, real data, network use, or
  failed fixtures are excluded and fail CI.
- Executable checks cannot read repository secrets, write outside their
  temporary directory, or reach the network.

Risk: text scanning cannot prove a Skill is safe. Treat it as one check, not a
security boundary. Human review and a narrow runtime permission model remain
required.

### PR 12A — Generate the release community catalog

Scope:

- Generate `community/community-catalog.json` from validated manifests and
  passing contract results.
- Check catalog drift on pull requests.
- Regenerate and verify the catalog during release.
- Record item version, core, format, and CLI ranges, license, deprecation, and
  fixture result.

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

Risk: a generated catalog can be mistaken for approval. Include the
compatibility-only meaning in the file metadata and public docs.

### PR 13 — Support selected multiple skills

Do not start this pull request until D41 is superseded by an approved decision.

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

### PR 14 — Add an optional read-only local HTTP API and OpenAPI

Do not start this pull request until the API decision in section 7 is approved.

Scope:

- Add `@ath/server` over the same application services used by the CLI.
- Publish `spec/openapi/ath-local-v1.yaml`.
- Bind to loopback only by default.
- Require a random session token, deny broad cross-origin access, and emit no
  telemetry.
- Expose read routes only.
- Version routes independently from npm package paths.
- Add request limits and shutdown behavior.

Files touched:

- `packages/server/**`
- `spec/openapi/ath-local-v1.yaml`
- `src/application/**`
- `package.json`
- `tsup.config.ts`
- server contract and security tests
- `docs/build-on-ath.md`

Acceptance:

- OpenAPI validation passes and generated clients call every documented route.
- The server cannot bind publicly without an explicit unsafe-development
  option.
- Tests prove no health data enters logs.
- A generated test client can read the demo database.

Risk: a loopback health-data server is still reachable by local processes and
browser attacks. Token, origin, logging, and bind tests are release gates.

### PR 14A — Add local HTTP writes after user-presence approval

Do not start this pull request until section 7 chooses a user-presence rule.

Scope:

- Add preview and confirmed write routes over the application service.
- Bind a confirmation to the exact preview, database revision, caller
  session, expiry time, and approved user-presence mechanism.
- Preserve one database transaction and current conflict behavior.
- Add replay, stale-preview, cross-session, and concurrent-write tests.

Files touched:

- `packages/server/**`
- `spec/openapi/ath-local-v1.yaml`
- `src/application/**`
- server write and security tests
- `docs/build-on-ath.md`

Acceptance:

- Calling preview and confirm in sequence is not itself proof of a person.
- A stale, changed, replayed, or cross-session confirmation is refused.
- The approved user-presence mechanism is tested at the boundary that supplies
  it.
- Writes have the same result as the matching CLI operation.

Risk: any client with a bearer token can automate both calls unless a separate
user-presence signal exists. Keep the server read-only until that signal is
approved.

### PR 15 — Consider stdio MCP separately

This is optional. HTTP and OpenAPI do not require it.

Scope if approved:

- Record a decision that supersedes D7 for a narrow local use.
- Implement stdio MCP as a small adapter over the application service.
- Expose only stable tool IDs already used by community manifests.
- Keep writes as preview or proposal unless a person confirms through the
  invoking host.
- Add transcript fixtures for tool listing, reads, proposals, refusals, and
  malformed input.

Files touched:

- `packages/server/src/mcp/**` or a separate `packages/mcp/**`
- MCP contract fixtures
- `docs/build-on-ath.md`
- package exports and tests

Acceptance:

- No network listener starts for stdio use.
- Standard output contains protocol frames only.
- Tools do not bypass CLI or HTTP write rules.
- Tool descriptions remain small enough to justify reversing part of D7.

Risk: MCP adds another public API and can use much more agent context than a
Skill plus CLI. Skip this pull request unless a concrete host integration
needs it.

### PR 16 — Publish the Swift reader example

Scope:

- Add a minimal Swift package under `examples/swift-reader/`.
- Use the approved SQLite library and decode at least sources, point
  measurements, sleep sessions, and workout sessions.
- Run the read-contract subset of the language-neutral fixtures from Swift.

Files touched:

- `examples/swift-reader/**`
- `docs/build-on-ath.md`
- `spec/conformance/**`
- `.github/workflows/ci.yml`

Acceptance:

- The example opens a copied demo database read-only and performs no migration.
- It passes on macOS and Linux Swift runners.
- It does not depend on Node or private services.

Risk: GRDB and SQLite.swift have different licensing, package, and API tradeoffs.
Choose one before writing the example. Keep raw SQL examples available so the
contract does not depend on that library.

### PR 16A — Complete package and release checks

Scope:

- Add packed-tarball tests for every published JavaScript entry point.
- Verify each promised spec, migration, conformance, Skill, and community file
  is present in the package that claims to ship it.
- Add a release-note template that maps changed npm, core API, SQL, format, and
  local API versions.
- Align pull-request and release workflow runtime versions.

Files touched:

- package tarball tests
- `package.json`
- `.github/workflows/ci.yml`
- release workflows
- release-note template

Acceptance:

- Tests install packed artifacts in an empty project with no workspace links.
- Published tarballs contain every promised public file.
- Release notes contain the compatibility table from section 6.
- Pull-request and release jobs use the approved Node and package-manager
  versions.

Risk: source tests can pass while packed declarations or workspace dependencies
are missing. The release gate must test only tarballs, not the checkout.

## 6. Compatibility and migration

| Change | Breaks existing users? | Migration |
|---|---|---|
| CI, templates, guides, `ath.fit` metadata | No runtime break. New contribution checks can block a pull request. | Rebase and satisfy the documented checks. |
| Plain SQL migration files | No, if generated Node behavior remains identical. | Existing databases stay at SQL version 1 until a later migration exists. |
| Explicit export list | It can break users only if a current public name is missed. | Preserve the built export list. Deprecate before removal. |
| Workspace package split | No for supported `athleticstandard` imports and `ath` commands. | Root compatibility entry points re-export the new packages. New apps may adopt `@ath/*` after publishing is approved. |
| Importer v1 payload | It can break undocumented deep imports. No importer API is currently public. | Publish the new subpath and a mapping guide. Keep one temporary adapter if an actual user is found. |
| CLI output separation | It can break scripts if text, JSON, or exit codes change. | Lock current behavior with stream and status tests. Announce any intentional correction. |
| Optional extension envelope | Yes at the format boundary. Current readers use strict schemas and exact version checks. | Release under the approved format version. Old readers refuse safely. Update `ath`, migrate if SQL changes, then import or open. Never make old readers discard extensions. |
| Community manifests and catalog | No. They are additive and passive. | Existing bundled Skill remains the default. |
| Multi-skill install | No when current defaults are preserved. | Community installs are explicit. Existing installed copies continue to report their own versions. |
| HTTP and OpenAPI | No. It is an optional new process and package. | Clients pin the local API version and regenerate on a documented breaking version. |
| stdio MCP | No when optional. | Hosts opt in to the MCP adapter. CLI plus Skill remains supported. |
| Direct third-party SQLite writes | Yes if declared supported without a full write contract. | Keep writes unsupported until another implementation passes revision, link, erasure, migration, concurrency, and round-trip fixtures. |

No pull request may silently rewrite an athlete database merely because the npm
package was updated. The approved trigger must be one of: an explicit
`ath migrate` command, or a confirmed migration during database open. Both
need preview, backup guidance, and rollback tests.

## 7. Decisions Mike must make

These choices change public promises. Record each approved choice as a new
D-number before its implementation pull request.

1. **Package split:** publish `@ath/core`, `@ath/store`,
   `@ath/importers`, and `@ath/server`, or keep them private workspace
   packages behind `athleticstandard` at first?
   - Recommendation: use private workspace packages first. Publish after
     packed-package and release checks are stable.

2. **Core API version:** should `CORE_API_VERSION` use semantic versions and
   start at `1.0.0`?
   - Recommendation: yes. `coreRange` then uses normal semantic version
     ranges and does not depend on unrelated CLI patch releases.

3. **SQLite support:** is direct third-party access read-only, or are direct
   writes part of the public contract?
   - Recommendation: promise reads first. Add writes only after a second
     implementation passes the full write suite.

4. **Swift library:** should the maintained example use GRDB or SQLite.swift?
   - Recommendation: use SQLite.swift if Linux CI remains required. GRDB's
     Linux support is community-maintained. Keep the SQL contract
     library-neutral either way.

5. **Extensions:** should extensions live on every record, only selected
   records, or in separate linked records?
   - Recommendation: use one optional namespaced envelope on selected record
     bases only if it can inherit the parent's tier. Do not add arbitrary
     top-level records.

6. **Extension release version:** does the approved envelope ship before the
   first public v1 release, as an additive later v1 release, or as v2?
   - Recommendation: if v1.0.0 has not been published, settle it before that
     release. Otherwise use the existing public version rules and test old
     readers' refusal.

7. **Community execution:** are importers and calculations compiled into a
   reviewed release, or can the runtime load third-party code?
   - Recommendation: compile reviewed source into releases. Do not add
     arbitrary runtime code loading.

8. **Community endorsement:** does catalog inclusion mean compatibility only,
   or project approval?
   - Recommendation: state that inclusion means the manifest and tests pass.
     It is not coaching, medical review, or a recommendation.

9. **Multi-skill distribution:** should D41 be superseded to permit explicit
   local installation from this repository's catalog?
   - Recommendation: yes, but never install by default and never fetch remote
     entries automatically.

10. **Local API:** is the non-JavaScript write route direct SQLite, local HTTP,
    or both?
    - Recommendation: direct SQLite reads plus local HTTP writes. Defer direct
      writes until cross-language proof exists.

11. **MCP:** is there a concrete local host that needs stdio MCP enough to
    supersede part of D7?
    - Recommendation: defer it. OpenAPI serves non-JavaScript apps, while the
      current Skill plus CLI serves agents.

12. **Contribution attestation:** require DCO sign-off, a contributor license
    agreement, or neither?
    - Recommendation: DCO sign-off with the existing MIT license. Do not add a
      separate contributor license agreement without legal need.

13. **Ownership:** who belongs in `.github/CODEOWNERS`, and which areas require
    review?
    - Recommendation: require owner review for `spec/`, migrations, schemas,
      release workflows, and community policy. Confirm GitHub handles before
      making the check required.

14. **Hosted promotion:** do accepted community items ship inside
    `athleticstandard`, as separate public packages, or both?
    - Recommendation: built-ins ship in the exact-pinned package. Larger or
      optional items may use separately exact-pinned public packages after
      publishing policy exists.

15. **SQL migration trigger:** should an old database migrate only through
    `ath migrate`, or through a confirmed database-open flow too?
    - Recommendation: start with explicit `ath migrate --dry-run` followed by
      confirmation. A library caller uses the same preview and apply API.

16. **SQLite read contract:** are versioned SQL views stable, or are example
    queries the only promise?
    - Recommendation: make a small set of views stable. Raw tables remain the
      versioned encoding and may change through migrations.

17. **API support policy:** how long must a deprecated stable TypeScript name
    remain available?
    - Recommendation: keep it through the next major npm release and document
      the replacement in the release where deprecation starts.

18. **HTTP user presence:** what proves a person approved a local HTTP write?
    - Recommendation: keep HTTP read-only until an app-owned confirmation
      channel can bind approval to one exact preview. Two API calls are not
      proof of a person.

## 8. Release gates

The contribution-ready milestone is complete only when:

- #5 is merged and its required check is enabled.
- Current imports and CLI behavior have compatibility tests.
- SQL migrations and language-neutral conformance fixtures are public.
- A Swift reader passes the shared fixtures without Node.
- Public APIs have explicit exports, stability labels, and generated docs.
- Importers and calculations use registries with deterministic synthetic
  contract tests.
- Community manifests are strict and the catalog is generated.
- Community entries never run merely because they are cataloged.
- Extension behavior preserves the two-tier wall and unknown data.
- Every published package is tested from its packed tarball.
- Release notes state package, core API, SQL, format, and local API versions
  separately.

The local HTTP API, direct third-party writes, and stdio MCP are not all needed
to call the foundation contribution-ready. Their decisions and support levels
must be explicit.
