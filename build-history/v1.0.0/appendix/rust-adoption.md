# Rust adoption plan (proposed)

**Status:** proposed. Draft decisions D130–D132 live in
`build-history/v1.0.0/decisions.md`. Tables:
[`rust-capability-inventory.md`](rust-capability-inventory.md),
[`rust-calculation-coverage.md`](rust-calculation-coverage.md).

**Goal:** Put all athlete-record rules in one Rust library (`ath-core`). Node and iOS
use thin bindings. Keep the npm name `athleticstandard` and the `ath` command. Do not
add a local HTTP service (D121). Do not add MCP (D7).

There is one cutover to Rust. No engine switch, no TypeScript fallback, and no
compatibility layer that keeps two rule sets alive.

---

## 1. Where the repo stands today

Checks on `main` (representative):

| Check | Result |
|---|---|
| `pnpm check:public-data` | pass |
| `pnpm typecheck` | pass |
| `pnpm test` | pass |
| `pnpm build` | pass |

**In code today:** v1 SQLite through TypeScript and `@libsql/client` (D93), Zod in
`src/v1/schema.ts` as the normative record model (D8, D83), SQL under `spec/sql/`
produced by `pnpm generate:sql-contract` from TypeScript migration strings, provider
parsers that still touch the legacy 0.x shape in places, and calculations in
`src/db/calculate.ts`.

**Pull requests to coordinate with:**

- [#20](https://github.com/mwhitham/ath/pull/20) — this plan and D130–D132 only.
- [#25](https://github.com/mwhitham/ath/pull/25) — migrate, import checks, structured
  logging, docs. **Land #25 before the Rust build PR.** It targets npm **1.1.0**.
- [#15](https://github.com/mwhitham/ath/pull/15) — about seven README link lines only;
  decisions and appendix text are already on `main` (D129).

**After the owner approves this plan:** one implementation PR on `main`. That PR carries
prerequisites and the start of `ath-core`.

**Verification today:** reuse Vitest (`tests/v1-cli-writes.test.ts`,
`tests/v1-export-import.test.ts`, `tests/v1-direct-sql.test.ts`, provider import tests).
CR5 calls for shared behavior fixtures under `spec/conformance/`; that directory is not
in the repo yet and will be added in the build PR.

**iPhone proof app:** when built, it should live at `apps/verification-ios/` and use the
Swift package from `ath-uniffi`. One app, one place.

---

## 2. One core, thin shells

| Piece | Job |
|---|---|
| **ath-core** | Records, validation, storage, history, conflicts, imports, calculations, packages, attachments, deletion, erasure |
| **ath-node** | napi-rs bindings; heavy work off the Node event loop |
| **ath-uniffi** | UniFFI bindings; Swift first |
| **TypeScript CLI** | Args, help, Skill, credentials, model calls |
| **Swift app** | UI, files, permissions (verification app above) |

Use **rusqlite** with bundled SQLite inside `ath-core`. Match pragmas and triggers from
the SQL contract.

---

## 3. One rule set

End state:

- One record model.
- One implementation of behavior.
- Supported writes through **typed operations on the core**.

Remove legacy pieces only after tests prove parity for that migration step:

- 0.x model (`src/schema.ts`) on import paths.
- Hand-maintained TypeScript validation and calculations that duplicate Rust.
- TypeScript string migrations as the SQL authoring path (see §5).
- Old writers and the TypeScript engine.

No engine-selection flags and no automatic fallback to TypeScript.

TypeScript may remain temporarily during the build. Each temporary path has a removal
step in the inventory table.

---

## 4. Rust owns the record model

Rust types and validators define valid records. JSON Schema and TypeScript types are
**generated** from that model.

The inventory lists each Zod rule and whether Rust keeps, fixes, or drops it.

JSON Schema must express every rule it can. Rules JSON Schema cannot express stay in
Rust and in docs. Host languages do not keep a second handwritten validator.

Test timestamps, units, optional vs null, extensions, and serialization across Rust,
Node, Swift, and portable packages.

**npm 1.1.0** ([#25](https://github.com/mwhitham/ath/pull/25)) still uses Zod. A later
**1.x** release deprecates the Zod-based normative exports with a warning and names the
generated replacement (D126). **npm 2.0.0** removes them (D131). **athleticstandard_version**
stays **1.0.0** for the record format unless a separate format change is approved.
**sql_schema_version** stays **1** unless a new SQL migration is added. Those numbers are
independent of the npm major version.

---

## 5. SQL files and the public API

**Today:** `src/db/migrations/*.ts` is the authoring source. `pnpm generate:sql-contract`
writes `spec/sql/migrations/*.sql`. The npm `files` list does **not** include `spec/`,
so shipping SQL to Node at runtime requires a **packaging change** (for example include
`spec/sql` in the tarball or embed SQL only in native binaries).

**Target:** plain SQL in `spec/sql/migrations/` is the canonical definition. Rust embeds
the same bytes. Node either embeds them or reads them from shipped files after packaging
is fixed.

Retire `pnpm generate:sql-contract` at the **Records and lifecycle** migration step,
when edits go to SQL files and drift checks compare SQL to a fresh database (see
inventory).

Keep foreign keys, constraints, and triggers unless a concrete change improves
correctness.

**Writes (D132):** official products use core operations. SQLite stays readable. Triggers
still enforce the two-tier wall. Third parties may still open the file with SQLite (D116),
but the project no longer promises that someone can recreate the **full revision protocol**
using only the old direct-write recipes in `spec/sql/README.md` (**D132** narrows **D125**).

No opaque `execute(JSON)` API.

Blocking work must not run on the Node event loop or the iOS main thread; bindings call
into Rust on a worker thread.

---

## 6. Capabilities to keep

Everything in the inventory, including the two-tier wall, sources, revisions, imports,
export/restore, attachments, erasure, conflicts, and explained calculations.

Attachments must commit or roll back with the database.

Fix bugs when found. Do not treat old bugs as compatibility.

---

## 7. Calculations

See [`rust-calculation-coverage.md`](rust-calculation-coverage.md). The wide calculation
library comes **after** the foundation proof.

Use `performance/` workloads only to spot serious regressions or memory problems. Do not
sell Rust on speed.

RMSSD and SDNN stay separate. Vendor scores stay labeled as vendor results.

Each result should identify its method, inputs, and missing-data behavior (coverage
appendix).

---

## 8. Import completeness

Parsers should land in the canonical Rust model without routing through `src/schema.ts`.

Imports need a **receipt**: imported, unchanged, corrected, skipped, invalid, and missing
fields, with reasons.

**Owner-reported import issues** (Oura readiness, sleep scores, contributors, SpO₂,
invalid rows after reimport) are not reproduced in public tests yet. Treat them as targets
for synthetic regressions in the build.

---

## 9. Work in the approved build PR

| Item | Notes |
|---|---|
| Inventory and coverage tables | This appendix |
| D130–D132 and guidance | `decisions.md`, `AGENTS.md`, `progress.md` |
| SQL canonical path + packaging | Ship or embed SQL; align with §5 |
| Behavior fixtures | `spec/conformance/` (CR5) |
| Rust workspace | `ath-core`, `ath-node`, `ath-uniffi`, CI |
| Calculation contract | Coverage appendix |
| Dead code | e.g. unused `importExport`; milestones in inventory |

Defer: catalog infrastructure, extra database backends, D115 internal workspace split, generic
extension frameworks beyond the v1 envelope.

**CORE_API_VERSION** is decided (D128) but not implemented (CR8 pending). The Rust plan
does not change what D128 covers.

---

## 10. Foundation proof (CLI + iPhone)

1. Installed CLI creates a database and a measurement.
2. iPhone app opens it and corrects the measurement.
3. CLI reads the correction and history.

Also prove reopen, source rules, bad input rejection, stale edits, unsupported versions,
and no change after refused writes.

Must run in the **iPhone Simulator** on macOS with Xcode. Linux CI cannot pass this gate.

**Status:** **BLOCKED** here (no Xcode / iOS Simulator).

Do not start the wide port until this proof passes.

---

## 11. Migration steps

| Step | Done when |
|---|---|
| Foundation | Core, bindings, shared model, basic storage, iPhone round trip |
| Records and lifecycle | Validation, queries, history, corrections, deletion, erasure |
| Provider imports | Parsing, field checks, repeat imports, corrections |
| Portable packages | Import/export, conflicts, attachments, samples |
| Existing calculations | Rust parity + reference tests |
| Expanded calculations | Coverage table batches |
| Cutover | TypeScript engine removed; release artifacts verified |

Record-format version and SQL schema version stay separate from npm versioning (see §4).

---

## 12. Data

No broad legacy system for external users. Use explicit, tested conversion on copies when
needed. Do not silently upgrade a database on open.

---

## 13. Packaging and checks

Prebuilt natives for declared Node platforms. Swift builds on declared iOS targets.

Shared behavior fixtures (once added) run in Rust, Node, and Swift tests.

Report PASS, FAIL, BLOCKED, or NOT IMPLEMENTED.

---

## 14. Deletion checklist

Remove only after the step above passes:

| Artifact | Step |
|---|---|
| `src/schema.ts` on import paths | Provider imports |
| Hand-maintained `src/v1/schema.ts` as normative | Foundation complete; generated types at npm 2.0.0 |
| `src/validate.ts` (0.x only, imports `src/schema.ts`) | Provider imports / 0.x removal |
| TypeScript `INITIAL_SCHEMA_SQL` as authoring source | Records and lifecycle |
| `pnpm generate:sql-contract` | Records and lifecycle |
| `src/db/calculate.ts` engine | Existing calculations |
| 0.x `ImportPayload` routing | Provider imports |
| TypeScript migration runner replaced by Rust | Cutover |
| `src/import/index.ts#importExport` (unused) | Start of build PR |

---

## Next task

Get this plan and D130–D132 approved. Land **#25**. Then open **one build PR** that
starts `ath-core` (open database, embed SQL, first binding surface) and the prerequisite
items in §9. Public cutover waits on the iPhone simulator proof on macOS.

---

## Tooling reference

| Host | Binding |
|---|---|
| Node | napi-rs |
| Apple | UniFFI |
| Web (later) | wasm-bindgen |

Web and Android are not blockers for the first Node + iOS release.
