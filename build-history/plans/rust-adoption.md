# One Rust engine for Athletic Standard

**What we are doing:** Build **one** Athletic Standard engine in Rust. Every
platform links that same library directly. There is no local HTTP service, no
MCP server, and no second implementation in TypeScript, Swift, or Kotlin. The
existing npm package **`athleticstandard`** and the **`ath`** CLI stay the public
install path (`npm i athleticstandard`); they become a thin Node wrapper around
the Rust engine, same as an iOS or Python app would use.

**Why:** Builders want to read and write the athlete file from watches, phones,
desktop apps, Python notebooks, and the web without re‑implementing rules that
already live in SQL triggers, Zod schemas, and migration logic. One engine
means one place to fix bugs and one set of conformance tests for every language.

This plan is **proposed** (not approved). It lives at
`build-history/plans/rust-adoption.md` until the owner records draft decisions
below in `decisions.md`.

---

## What others learned shipping Rust to many languages

| Project | Pattern | What worked | What hurt |
|---|---|---|---|
| [Mozilla application-services](https://github.com/mozilla/application-services/) + [UniFFI](https://github.com/mozilla/uniffi-rs) | Rust components; Kotlin/Swift/Python from UniFFI; iOS “megazord” XCFramework | Same API shape on mobile; Glean moved bindings to UniFFI and cut glue code ([talk slides](https://fnordig.de/talks/2022/dataclub/glean-uniffi-migration.pdf)) | Large CI matrix; Rust + mobile toolchains; cross-team ownership |
| [Signal libsignal](https://github.com/signalapp/libsignal) | Custom `#[bridge_fn]` macros; C FFI + JNI + Node; Maven/CocoaPods with prebuilt natives | Tight control over crypto APIs; published artifacts per platform ([bridge README](https://github.com/signalapp/libsignal/blob/main/rust/bridge/README.md)) | Heavy custom tooling (`cbindgen`, `gen_java_decl.py`); few casual contributors |
| [Matrix Rust SDK](https://github.com/matrix-org/matrix-rust-sdk/blob/main/ARCHITECTURE.md) | Core crates + `matrix-sdk-ffi` (UniFFI) for Swift/Kotlin; separate `wasm-bindgen` for web crypto | Clear split: Rust core, thin FFI, platform packages ([bindings](https://github.com/matrix-org/matrix-rust-sdk/tree/main/bindings)) | Android/iOS release profiles tuned for binary size; many targets to build |
| [Automerge](https://github.com/automerge/automerge) | Rust core; JS via WASM; C for other languages | One CRDT implementation everywhere ([2.0 blog](https://automerge.org/blog/automerge-2/)) | WASM loading differs per bundler ([init docs](https://automerge.org/docs/reference/library-initialization/)); React Native needed C, not WASM |
| [Polars](https://github.com/pola-rs/polars) | Rust query engine; Python via PyO3/maturin; Node via [napi-rs](https://github.com/pola-rs/nodejs-polars) | Each host gets idiomatic API; Rust stays internal | Multiple binding stacks (PyO3 vs napi-rs); compile farms for releases |

**Lessons for ATH:** Use **one FFI strategy per host family** (UniFFI for
Apple/Android/Python, napi-rs for Node, wasm-bindgen for browser). Keep the
**public contract** in `spec/` (SQL files, JSON schemas, conformance vectors) so
bindings do not become a second spec. Expect **release engineering** to be a real
job: prebuilt libraries per OS/CPU, not just Rust source. Plan for **fewer
casual contributors** in the core; most app developers only touch thin wrappers.

---

## Recommended tooling

| Layer | Choice | Reason |
|---|---|---|
| SQLite in Rust | [`rusqlite`](https://github.com/rusqlite/rusqlite) (bundled SQLite, same pragmas as `spec/sql/README.md`) | One engine owns migrations and writes; matches published SQL contract |
| Node / `athleticstandard` | [napi-rs](https://napi.rs/docs/introduction/getting-started) | Same distribution model as today’s native deps; optional per-platform npm packages |
| Swift, Kotlin, Python | [UniFFI](https://mozilla.github.io/uniffi-rs/) | Proven for mobile + Python; generates bindings from Rust API surface |
| Web | [wasm-bindgen](https://rustwasm.github.io/wasm-bindgen/) (+ `wasm-pack` or npm WASM asset) | Direct file access in browser is limited; WASM fits sandboxed embedders |
| C API (optional later) | UniFFI’s C scaffolding or a thin `extern "C"` layer | Escape hatch for watch OS or embedders UniFFI does not cover yet |

**Not recommended:** A separate local service (violates product shape and D7).
**Not in scope for step 1:** Replacing the Skill or CLI UX—only the engine
underneath.

---

## Rust core vs thin platform shells

**Inside the Rust engine (`ath-core` or similar workspace crate):**

- Open, migrate, and check `athlete.ath.sqlite` using the ordered SQL in
  `spec/sql/migrations/` and metadata rules in `spec/sql/README.md`.
- Enforce athlete-record rules in code **and** rely on existing SQL triggers for
  the two-tier wall, extension envelope limits, and other database-enforced
  rules.
- Validation of record payloads against the v1 format (aligned with
  `src/v1/schema.ts` / generated JSON Schema until a single schema source is
  chosen).
- Measurement-series encode/decode and shared calculations (baselines, stats,
  within-workout math).
- JSON package import/export parity with today’s portable format.
- Migration preview/apply logic used by `ath migrate` (behavior must match CR4;
  implementation moves to Rust without changing the UX contract).

**Stays thin per platform:**

- **Node:** Commander CLI, Skill packaging, help text generation, LLM gateway
  hooks for `predict` / `backtest` (D7: still CLI + Skill, not MCP).
- **Swift / Kotlin:** App UI, permissions, background tasks, file pickers.
- **Python:** Notebooks and scripts call UniFFI bindings.
- **Web:** WASM module + JS loader; host app handles storage APIs.

**Same results everywhere:** `spec/conformance/` vectors (CR5) and existing
`pnpm test` scenarios become the cross-language gate. Each binding runs the
same fixtures; Rust is the reference implementation. Record content version
(`athleticstandard_version`) and SQL schema version (`sql_schema_version`) stay
independent, as today.

---

## Step-by-step path (each step = its own PR)

1. **Rust core + Node wrapper, tests green.** Add `crates/ath-core`, wire
   napi-rs into `athleticstandard`, call Rust for database open/migrate/check
   and one read path. Keep TypeScript implementations behind a flag as fallback
   until vectors pass on all CI platforms. Do not change `package.json` name,
   `bin`, or `engines`.

2. **Move write paths and import/export.** Port package import, export, and
   validation into the engine; delete duplicate TS logic once parity tests pass.
   Regenerate nothing by hand—`pnpm generate:*` stays the source for published
   `spec/` artifacts.

3. **Swift package (UniFFI).** XCFramework or SwiftPM wrapper; document how an
   iOS or watch app opens the same SQLite file. No separate “ATH service.”

4. **Kotlin / Android (UniFFI).** Same FFI crate as Swift; publish AAR or Maven
   coordinates as a follow-on decision.

5. **Python (UniFFI).** Wheel with extension module for agents and science
   stacks.

6. **Web (wasm-bindgen).** Crate subset for validation and read-only paths where
   full SQLite in WASM is acceptable; document limits for large files.

7. **Retire the TypeScript engine.** Remove fallback once Node, Swift, and
   conformance CI all use Rust only.

Steps 3–6 can reorder if a builder need appears first; steps 1–2 block
everything else.

---

## Risks and costs

- **Maintainers:** Need Rust plus release tooling for mobile and npm prebuilds.
- **CI:** Multi-target builds (see Matrix “small-release” profiles for size).
- **Schema drift:** Until one generator owns types, Rust and Zod must both run
  conformance checks on every PR.
- **D121 “thin OSS”:** This plan **widens** the core deliberately; it does not
  add MCP, HTTP, or extra maintained *ports*—it replaces ports with bindings.

---

## Draft decisions for owner (not recorded until approved)

### D130 (draft). Single Rust engine with generated language bindings

Athletic Standard implements file logic **once** in Rust. The npm package
`athleticstandard` and the `ath` CLI remain the Node distribution and wrap that
engine via napi-rs. Swift, Kotlin, Python, and web embedders use UniFFI or
wasm-bindgen rather than maintained hand-written ports.

This **supersedes D121** only for “no Rust core”: Rust is approved as the core,
not as an optional speed-up. D121’s rejection of a local HTTP service, MCP
server, and unrelated language ports **remains**. D7 **remains**: agents use CLI
+ Skill; Rust does not add a conversational server.

Rejected alternatives:

- **Separate TypeScript engine long term.** Two implementations will diverge on
  triggers, migrations, and vectors.
- **Local API service for mobile.** Extra process and deployment; bindings only.
- **Per-app copy-paste of SQL rules.** Already rejected by the public SQL
  contract; engine must execute that contract.

### D131 (draft). Public spec and vectors outrank binding APIs

The normative contracts are `spec/sql/`, generated JSON Schema, and
`spec/conformance/`. Generated binding APIs may change between releases if the
spec version does not. Breaking record or SQL schema versions follow the
existing format versioning rules.

---

**Placement:** `build-history/README.md` does not name a folder for unapproved
plans. This file remains here until approval, then moves or links from
`build-history/v1.0.0/appendix/` and a recorded D-number (format version
unchanged if only implementation moves).
