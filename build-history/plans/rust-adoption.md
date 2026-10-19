# Rust adoption plan (proposed)

**Status:** proposed. Recorded draft decisions: D130–D132 in
`build-history/v1.0.0/decisions.md`. Supporting tables:
[`rust-capability-inventory.md`](rust-capability-inventory.md),
[`rust-calculation-coverage.md`](rust-calculation-coverage.md).

**Goal:** One Rust core (`ath-core`) holds every rule about athlete records. Node,
iOS, and later hosts use thin bindings. Keep the npm package name
`athleticstandard` and the `ath` command. Do not add a local HTTP service or MCP
server (D7 stands).

This plan supersedes earlier draft text in this file that described a TypeScript
fallback flag or a long-term dual engine. There is one cutover, not a permanent
switch.

---

## 1. Starting point (repository today)

Reviewed on branch `cursor/rust-adoption-plan-0ce5` after merging `main`.

| Check | Baseline result |
|---|---|
| `pnpm check:public-data` | pass |
| `pnpm typecheck` | pass |
| `pnpm test` | pass (31 files, 327 tests) |
| `pnpm build` | pass |

**What exists:** v1 SQLite via libSQL in TypeScript (`src/db/`), Zod normative
records in `src/v1/schema.ts`, published SQL under `spec/sql/` (still generated
from TypeScript migration strings), provider importers through a legacy 0.x hop,
calculation operations in `src/db/calculate.ts`, contribution foundation CR1–CR3
done (D115–D129), CR5 conformance vectors still pending.

**Open pull requests to coordinate with (do not duplicate):**

- [#20](https://github.com/mwhitham/ath/pull/20) — this plan and decisions.
- [#15](https://github.com/mwhitham/ath/pull/15) — contribution-readiness plan text;
  prerequisite work lands on `main` with decisions already partially recorded (D129).
- [#25](https://github.com/mwhitham/ath/pull/25) — migrate flow, import check,
  structured logging, docs; port behavior to Rust after it stabilizes in TypeScript.

**Grokbot verification:** No Grokbot-named code, scenarios, or browser verifier
live in this repository today. Reuse existing Vitest coverage (`tests/v1-cli-writes.test.ts`,
`tests/v1-export-import.test.ts`, `tests/v1-direct-sql.test.ts`, provider import tests)
as the behavioral catalog until Grokbot publishes shared scenario IDs. During transition,
any browser-based verifier Grokbot maintains can run against the same synthetic fixtures
under `spec/conformance/`. **Owner:** Grokbot owns verification evidence; Rust work
owns core, bindings, and packaging.

**SwiftUI verification app (single location):** `apps/verification-ios/` — small
SwiftUI app depending on the UniFFI Swift package from `crates/ath-uniffi`. It opens
a snapshot `.ath.sqlite` produced by the installed CLI, applies one correction, and
writes history. Do not add a second demo app elsewhere in the repo.

---

## 2. Architecture: one core, thin consumers

| Component | Responsibility |
|---|---|
| **ath-core** | Records, validation, storage, history, conflicts, provider parsing,
normalization, calculations, package import/export, attachments, deletion, erasure |
| **ath-node** | napi-rs bindings; blocking work off the Node event loop |
| **ath-uniffi** | UniFFI bindings; exercised through Swift first |
| **TypeScript CLI** | Arguments, help, presentation, Skill, credentials, external model calls |
| **Swift app** | `apps/verification-ios/` — screens, file access, permissions, lifecycle |

Internal Rust modules stay inside `ath-core` unless a dependency forces a separate
crate. Storage starts with **rusqlite** and bundled SQLite, matching `spec/sql/`
pragmas and triggers.

Platform code may read HealthKit or file exports. ATH interpretation, normalization,
and write decisions live in Rust.

---

## 3. Remove duplicate rules and scaffolding

End state:

- One executable record model.
- One implementation of domain behavior.
- One supported write interface: **domain operations on the core**.

Remove, after parity proof at each gate:

- Legacy 0.x record model (`src/schema.ts`) from import paths.
- Hand-maintained TypeScript validation and calculations that duplicate Rust.
- Duplicate SQL definitions (TypeScript string migrations once SQL files are canonical).
- Legacy writers and superseded engine code.

**Not allowed at cutover:** engine-selection flags, automatic TypeScript fallback, or
compatibility facades around removed APIs.

Temporary TypeScript may remain **only** until a named stage passes tests. Every
temporary path has a removal milestone in
[`rust-capability-inventory.md`](rust-capability-inventory.md). Do not reorganize the
TypeScript engine into a new architecture that Rust will immediately replace.

---

## 4. Rust as the executable definition of records

Rust types and explicit semantic validators define valid ATH records. **Generate**
JSON Schema and TypeScript data types from that model. Binding types map mechanically
to the same definitions.

Inventory of Zod refinements and relationships:
[`rust-capability-inventory.md`](rust-capability-inventory.md) (preserve, correct, or
remove, with reason).

Generated JSON Schema must express every rule it can represent. Remaining semantic
rules stay documented and enforced only through `ath-core` validation. No second
handwritten validator in a host language.

Specify and test across Rust, Node, Swift, and portable packages:

- timestamps and time zones
- units and numeric limits
- optional vs null vs missing
- unknown fields and extension envelopes
- deterministic serialization (stable ordering where the format requires it)

---

## 5. One SQL source and a small public API

**Canonical storage definition:** plain SQL in `spec/sql/migrations/`. Rust embeds
those files. Published SQL in releases comes from the same files. TypeScript loads
the same files at runtime (prerequisite PR); the generator becomes verify-only, then
retires.

Keep foreign keys, constraints, and integrity triggers. Do not redesign storage without
a concrete correctness or simplicity gain.

**Writes:** official supported writes go through domain operations in `ath-core`. SQLite
stays open and documented for inspection. D116 still allows direct SQLite access with
required pragmas and trigger enforcement, but **D132** retires the promise that a
third party can recreate the full revision protocol using only documented direct SQL.
Stable views remain for reads (D125); narrow direct-write tables may stay for simple
cases, not full lifecycle parity.

Expose typed operations and errors. No arbitrary SQL callback surface and no opaque
`execute(JSON)` API.

**Concurrency:** connection ownership, serialized writes, stale-state checks, and handle
closure are defined in the core. Blocking work runs off the Node event loop and off the
main iOS UI thread (async boundary in bindings).

---

## 6. Capabilities the core must keep

All current v1 behaviors in the capability inventory, including:

- two-tier wall between measured records and athlete reports
- source identity; imported vs reported vs calculated values
- stable IDs, corrections, history, stale-edit rejection
- provider timestamps, reporting periods, units
- repeatable imports without duplicates; deliberate provider corrections
- complete export and restore (history, extensions, dense samples, attachments,
  import identity, erasure markers)
- safe conflict and transaction failure behavior
- distinct deletion vs privacy erasure
- reproducible calculations with explained inputs

Attachment staging must commit or roll back with the database transaction. Failed
operations must not leave records pointing at missing files.

Reuse proven parsers, SQL, algorithms, and fixtures. Fix bugs; do not encode bugs as
compatibility.

---

## 7. Calculation coverage

See [`rust-calculation-coverage.md`](rust-calculation-coverage.md). Broad library work
is explicit batches after the foundation proof — not a prerequisite to prove architecture.

Use large existing workloads (`performance/`) only to detect **material** regressions or
memory problems. Do not benchmark for speed targets or justify Rust by performance.

---

## 8–9. Methods and explainability

Calculation families (HRV, heart rate, sleep, training, activity, recovery) are listed
in the coverage table with stages **Existing calculations** vs **Expanded calculations**.

Rules:

- RMSSD and SDNN stay separate; never merge baselines or substitute silently.
- Imported branded scores stay `provider_result`; generic ATH methods use their own names.
- Ordinary modules plus a small method registry — no plugin framework.
- Each result carries method, version, parameters, inputs, exclusions, and references.
- Insufficient inputs → clear `unavailable` reason.
- Tests use independent reference answers.

Establish the **calculation contract** during prerequisites (documented in coverage file).

---

## 10. Import completeness

Parsers normalize into the canonical Rust model. No route through `src/schema.ts`.

Field-level mapping checks, not row counts alone. Import **receipt** required (see
coverage doc). Video findings (Oura readiness, sleep scores, contributors, SpO₂,
invalid records after upgrade/reimport) stay open investigation targets until reproduced
with synthetic regressions fixed in Rust.

---

## 11. Prerequisites (concrete, no scope creep)

| Prerequisite | Delivered by |
|---|---|
| Capability and dependency inventory | `rust-capability-inventory.md` + this plan |
| Reconcile plan, decisions, contribution-readiness | D130–D132; `AGENTS.md` update; progress note |
| Canonical SQL files | `spec/sql/` runtime load PR on `main` |
| Language-neutral behavior fixtures | `spec/conformance/` PR on `main` |
| Rust workspace, ath-node, ath-uniffi, focused CI | `crates/*` PR on `main` |
| Calculation contract and coverage table | `rust-calculation-coverage.md` |
| Dead code and temporary milestones | deletion checklist below + importExport removal PR |

Defer: community catalog infrastructure, speculative DB backends, TypeScript workspace
split (D115), generic extension frameworks beyond the v1 envelope.

---

## 12. Foundation proof (CLI + iPhone)

**Required proof:**

1. Installed CLI creates a database and measurement.
2. iPhone app opens the file and corrects the measurement.
3. Installed CLI reads the correction and retained history.

Also prove: reopen, valid source attribution, invalid input rejection, stale-edit
rejection, unsupported version rejection, unchanged data after refused operations.

Snapshots transfer closed and consistent between environments; compare logical data and
history.

**Simulator gate:** must run the iOS target in the **iPhone Simulator**. Browser-only or
macOS compile-only does **not** satisfy the gate.

**Environment status (Linux CI agent):** **BLOCKED** — missing Xcode, iOS SDK, and
iPhone Simulator. Cannot be reported as PASS from this environment.

Reuse Grokbot/browser verification during transition if available externally. Permanent
setup: `apps/verification-ios/`, CLI, and automated tests.

**Do not start the broad feature port until foundation proof passes.**

---

## 13. Migration stages

| Stage | Completion requirement |
|---|---|
| **Foundation** | Core, Node and Swift packaging, shared model, basic storage, verified round trip |
| **Records and lifecycle** | Full validation, queries, history, corrections, deletion, erasure, safe failures |
| **Provider imports** | Canonical parsing, field reconciliation, repeat imports, corrections, time handling |
| **Portable packages** | Preview/apply, conflicts, export, restore, extensions, dense samples, attachments |
| **Existing calculations** | Rust implementations, independent reference tests, explanations |
| **Expanded calculations** | Named batches from coverage table |
| **Cutover and distribution** | Consumers on Rust; obsolete TS engine deleted; release artifacts verified |

Each stage lists modules, dependencies, test IDs, acceptance criteria, code to delete,
and blockers in the implementing PR.

Migration completion is separate from expanded calculation batches; both stay scheduled.

---

## 14. Data and formats

No general legacy compatibility system for external users. Preserve owner files through
explicit, tested conversion on copies when needed.

Do not silently convert on open. Unsupported storage versions fail clearly. Keep record
format version and SQL schema version independent.

---

## 15. Packaging and verification

- Node: prebuilt natives per declared platform matrix; consumers do not compile Rust.
- Swift: builds on declared iOS targets; test packed artifacts in clean projects.
- Shared `spec/conformance/` fixtures run in Rust, Node, and Swift tests.
- Report PASS / FAIL / BLOCKED / NOT IMPLEMENTED. Data loss or corruption blocks completion.

Android, Python, browser storage, Watch, and hosted sync are follow-on — they do not
block the first Node + iOS release.

---

## 16. Deletion checklist

Delete when the stage gate passes (not before):

| Code / artifact | Gate |
|---|---|
| `src/schema.ts` and 0.x import routing | Provider imports |
| `src/import/index.ts#importExport` | Prerequisite (unused) |
| TypeScript `INITIAL_SCHEMA_SQL` strings as source | SQL canonical verify-only |
| Hand-maintained `src/v1/schema.ts` as normative | Foundation + codegen live |
| Duplicate validation in `src/validate.ts` for v1 | Records and lifecycle |
| `src/db/calculate.ts` engine | Existing calculations parity |
| Provider merge via `ImportPayload` 0.x | Provider imports |
| TypeScript-only migration runner logic replaced by Rust | Cutover |
| Engine fallback or feature flags (if any added during dev) | Cutover |

---

## Next implementation task

On `main`, merge the prerequisite PR chain (canonical SQL runtime load → conformance
fixtures → Rust workspace skeleton with CI → remove `importExport`). Then implement
`ath-core` open/create database using embedded SQL and wire `ath-node` for a single
read path used by one CLI command behind an internal dev hook — still without public
cutover until the iPhone simulator round trip passes on macOS hardware.

---

## Industry reference (tooling)

| Host | Binding |
|---|---|
| Node / npm | napi-rs |
| Swift / Kotlin | UniFFI |
| Web (later) | wasm-bindgen |

See prior research table in git history at commit `5610f02` if needed; this plan does
not treat web or Android as first-release blockers.
