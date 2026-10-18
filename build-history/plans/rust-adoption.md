# Rust adoption plan (proposed)

Status: **proposed — awaiting owner approval.** This file is not an approved
decision. No new D-number is recorded in any `decisions.md` until the owner
accepts this plan and the draft decision below.

Placement follows the rule that plans stay **outside** a version folder until
their decisions are approved. After approval, link this plan from a new
D-number in the folder for the format version the work produces (likely
`build-history/v1.0.0/` if the npm package stays at 1.x and does not change
record content).

## 1. Core goal

Keep Athletic Standard easy to install (`npm i athleticstandard`, `ath` on Node
20+) and faithful to the public contracts (`spec/sql`, JSON schemas, two-tier
wall, extension envelope). If Rust is added, it must sit **behind** the same
npm package and CLI surface, not replace them.

The question is not “should we like Rust?” It is whether a **concrete need**
justifies the maintenance cost of a second implementation language, given D121.

## 2. What is locked today

| Decision | What it says for Rust |
|---|---|
| **D7** | Agent interface is CLI + Skill, not MCP. Rust does not change this. Any Rust work must not add a local HTTP or MCP server as the primary interface. |
| **D8** | TypeScript was chosen for distribution (`npx`/`pnpm dlx`), Zod-first schemas, and a workload that did not need a systems language. A Rust **core** does not repeal D8 for the **npm shell**: the published package remains Node-first for install and CLI entry. |
| **D121** | No Rust core, Swift library, MCP, or HTTP service **now**. A future Rust core may ship inside npm behind existing APIs after a **concrete builder need** and the same conformance vectors. |
| **Contribution readiness** | Direct SQLite + language-neutral `spec/` are the cross-language path. Rust is explicitly “later, only on demand.” |
| **v1 spec §13** | Before a second database engine, exhaust indexes, batching, fewer text copies, chunked series, and compressed blocks — then measure again. |

This plan does **not** assume Rust is approved. It defines when approval would
be honest and how to execute if the owner says yes.

## 3. What must not break

Any Rust adoption path must preserve:

- **Published SQL contract** — `spec/sql/` (generated from `src/db/migrations/`),
  triggers enforcing the two-tier wall, extension rules, and migration ordering.
- **SQLite as live record** — same file format, pragmas, and transaction recipe
  documented in `spec/sql/README.md`.
- **`ath migrate`** — preview, backup, confirmation, and interactive prompt-on-open
  (CR4, in progress elsewhere). This plan does not edit migrate implementation
  files; Rust must call the same behavior, not fork it silently.
- **Conformance** — CR5 vectors (when landed) and existing automated tests
  remain the parity gate. Rust code is wrong if vectors fail.
- **npm identity** — package name `athleticstandard`, binary `ath`, Node `>=20`,
  same `package.json#exports` unless a separate semver-major API decision says
  otherwise.
- **OSS thin** — no maintained Swift port, no MCP server, no “build every client
  surface” unless a later, separate plan approves it.

## 4. What would justify Rust (honest need)

D121 blocks Rust **without demonstrated need**. Any proposal to adopt Rust should
name at least one of these **measured or reported** triggers. Vague “performance”
or “Rust is nicer” is not enough.

1. **Release performance limits are missed after §13 remedies** — The 100k-record
   CI benchmark or 3M large job fails agreed limits after indexes, batching, and
   series storage options are tried in TypeScript/SQLite, and profiling shows CPU
   in hot loops (import merge, series decode, baseline windows, export), not
   I/O or SQLite alone.

2. **A real builder is blocked without a native library** — Someone shipping a
   non-Node app (mobile, desktop, embedded agent runtime) needs a **supported**
   shared library with the same semantics as `ath`, and cannot use direct SQLite
   + vectors alone within their timeline. The request must be specific (which
   commands or calculations, which platform).

3. **Security or supply-chain pressure on parsing** — A maintained importer or
   decode path handles untrusted input at scale, and memory-safe native parsing
   with fuzzing is judged lower risk than growing TypeScript parsers. This still
   requires vectors and does not skip SQL triggers.

4. **Contributor capacity** — At least one maintainer can own Rust toolchains,
   cross-platform prebuilds, and CI for the next 12 months. Without that, defer.

If **none** of the above is true today, the correct action is **Option A** (stay
TypeScript) and keep improving SQL and TypeScript hot paths per v1 spec §13.

## 5. Options

### Option A — Stay TypeScript; optimize SQLite and TypeScript only

Keep all logic in the current repo layout. Finish CR4–CR9, land conformance
vectors, and use `performance/` benchmarks as the gate. Revisit Rust only when
§4 triggers fire.

**Pros**

- Matches D121 as written; no new decision required.
- One language, one toolchain, aligns with D8 distribution story.
- Zod remains normative for records; no schema drift between Rust and TS.
- Lowest CI and release complexity.

**Cons**

- Very large imports or series may stay slower than a tuned native core.
- Non-Node builders rely on direct SQLite + vectors, not a shared `.so`/`.dylib`.
- CPU-heavy calculations stay in Node unless moved to SQLite (views, triggers).

### Option B — Rust library as the engine; Node CLI stays the façade

Introduce a `crates/ath-core` (name TBD) Rust crate compiled to native addons
via **napi-rs** (or similar) and loaded from the existing `athleticstandard`
package. TypeScript keeps CLI, Skill packaging, LLM gateway hooks, and thin
wrappers. Heavy work (import merge, series math, export assembly, optional
`check` walks) moves to Rust behind stable internal FFI.

**Pros**

- Single npm install with prebuilt binaries per platform (napi-rs pattern).
- Strong speed and memory wins possible on hot paths without changing user
  commands.
- Public API stays Node; D8 distribution story largely intact.

**Cons**

- Two implementations until parity is proven; ongoing sync on behavior changes.
- Release matrix: linux/macOS/windows × arm64/x64, plus musl where needed.
- Zod/JSON Schema stay in TS; Rust needs generated types or shared fixtures —
  drift risk unless conformance vectors are strict.
- Debugging spans Node + native; contributors need Rust toolchain.

### Option C — Rust only for narrow, high-value modules

Same packaging as B, but scope Rust to **one** bounded area first, for example:

- measurement-series decode/encode only, or
- import merge for one vendor, or
- pure calculation helpers used by `ath stats` / baselines.

Everything else stays TypeScript. Expand only when vectors pass and §4 triggers
justify the next module.

**Pros**

- Smallest step that amends D121; proves CI and prebuild pipeline before a big
  rewrite.
- Easier rollback: disable native addon and fall back to TS for that module.
- Lessons apply before committing to a full core port.

**Cons**

- Boundary design work up front (what is “pure” vs what touches SQLite).
- Two languages in production forever unless later phases expand Rust.
- Some overhead from FFI per call if modules are too granular.

## 6. Recommendation

**Default: Option A** until a §4 trigger is documented with evidence (benchmark
JSON, builder issue, or security review). D121 is still the right default for a
thin OSS base finishing v1 publish.

**If the owner accepts a trigger now: start with Option C**, not a full rewrite.
First candidate module (pending owner decision §8): **measurement-series
sample decode and within-workout calculations**, because they are CPU-bound,
mostly pure, and already have clear inputs/outputs for vectors. Keep all SQLite
writes and migrations in TypeScript (or shared SQL) until CR5 vectors exist.

**Do not choose Option B (full core)** until Option C has shipped, vectors pass,
and maintainers agree the remaining TS hot paths still fail limits.

Option B remains the long-term shape **if** multiple modules move and maintaining
split brains in TS and Rust costs more than consolidation.

## 7. Phased steps (only after owner approves a D-number superseding D121)

Phase 0 — **No Rust** (current)

- Land CR4–CR5, SQL contract drift checks, and performance baselines.
- Document current p95/p99 from `performance/results/` for the nine scenarios.

Phase 1 — **Spike behind a flag** (Option C)

- Add `crates/` with one crate, napi-rs wiring, and `optionalDependencies` or
  prebuild download script; **no** default enable in release until vectors pass.
- Implement one module; TS retains fallback implementation.
- Extend conformance fixtures for that module (input bytes/JSON → expected
  outputs). Same tests run against TS fallback and Rust path.

Phase 2 — **Enable by default when parity proven**

- CI runs vectors on all platforms in the release matrix.
- Turn on Rust path when prebuild exists; TS fallback remains for unsupported
  platforms until prebuilds exist or owner drops support.

Phase 3 — **Expand or consolidate** (optional)

- Move additional hot paths per §4 evidence.
- If most logic is Rust, collapse duplicate TS implementations and document
  internal API. Public npm exports unchanged.

Phase 4 — **Standalone Rust library** (only on builder request)

- Publish `ath-core` crate to crates.io for non-Node consumers.
- Not required for npm users; separate decision if API surface differs from FFI.

**Explicit non-goals for early phases**

- Rewriting SQL migrations in Rust.
- Replacing Zod with Rust as normative record schema.
- WASM in the browser unless a builder needs it (WASM is slower for file I/O
  heavy work; napi-rs is the default for Node CLI).

## 8. Risks

| Risk | Mitigation |
|---|---|
| Behavior drift between TS and Rust | CR5 conformance vectors; property tests; TS fallback until green matrix |
| Broken `npm i` on exotic platforms | TS fallback; document supported platforms; fail with clear message |
| Schema changes require two edits | TS remains normative; Rust reads fixtures; codegen later only if justified |
| `ath migrate` divergence | Migrate stays in TS/SQL; Rust never applies migrations without shared code path |
| Violating two-tier wall in native code | DB triggers remain authoritative; Rust uses same SQL write recipes |
| Team overload | Phase 1 time-box; revert if no maintainer owns Rust CI in 90 days |
| Contradicting D121 silently | Owner must approve draft D130 (below) before Phase 1 merges |

## 9. How existing and planned tests prove parity

**Today**

- `pnpm test` — unit and integration tests for import, check, context, grading,
  package exports, SQL migration from empty DB.
- `pnpm check:public-data` — no real health data in repo.
- `performance/` — nine scenarios on 100k (CI) and 3M (large job); compare
  before/after Rust with same seed and hardware notes.

**After CR5 (planned)**

- `spec/conformance/` — language-neutral fixtures; `scripts/check-conformance.ts`
  runs in CI. Rust napi module runs the same vectors as TypeScript.

**Parity rule**

- For any function moved to Rust: every vector that passed in TS must pass in
  Rust on all release platforms. No release if native prebuild missing on a
  tier-1 platform without TS fallback.

**Regression for migrate**

- Do not change CR4 files in Rust PRs until CR4 lands; then add integration
  tests only via existing migrate test suite, not a second migrate implementation.

## 10. npm packaging sketch (if B or C approved)

- Root package `athleticstandard` remains the install target.
- `napi-rs` (or `@napi-rs/cli`) builds platform triples in release CI; artifacts
  attached or published via optional dependency package pattern used by other
  native npm modules.
- `engines.node >=20` unchanged.
- `package.json#exports` unchanged for public paths; optional subpath
  `athleticstandard/native` only if needed for advanced embedders — owner
  decision.
- WASM: reserve for a future embedder need; not the default CLI path.

## 11. Draft decision for owner approval (not recorded yet)

When the owner accepts Rust work, append something like the following to
`build-history/v1.0.0/decisions.md` as **D130** (number may shift if other
decisions land first). Wording is draft only.

---

### D130. Rust core modules only after demonstrated need (draft)

Athletic Standard adopts Rust **only** for bounded, measured needs inside the
existing `athleticstandard` npm package. D121’s “no Rust core now” is
superseded **to the extent** the owner documents a §4 trigger and approves a
phase-1 module list. D121’s thin-OSS principle remains: no MCP, no HTTP
service, no maintained Swift port, and no full language port without vectors.

D7 remains in force. D8 remains in force for npm install and CLI entry; TypeScript
keeps Zod normative schemas and the `ath` command surface.

Rust code must pass the same conformance vectors as TypeScript, keep SQLite
triggers as the enforcement layer for the two-tier wall, and not fork `ath
migrate` behavior.

Rejected alternatives:

- **Full Rust rewrite before vectors exist.** Rejected because parity cannot be
  proven and D121’s intent would be bypassed.
- **Rust as a separate product without npm integration.** Rejected because
  distribution and the `ath` CLI must stay `npm i athleticstandard`.
- **Adopt Rust without a named maintainer.** Rejected because native prebuilds
  require ongoing CI ownership.

---

Until D130 (or equivalent) is appended and approved, **do not merge Rust code**.

## 12. Open decisions (owner — answer in order)

1. **Is there a documented §4 trigger today?** If no, stop at Option A and
   close this plan until evidence exists. If yes, name which trigger (1–4) and
   attach data (benchmark file, issue link, or security note).

2. **Which option?** A (stay TS), B (full Rust core behind npm), or C (one
   module first). Recommendation: A unless (1) is yes; then C.

3. **First Rust module (only if C or B):** measurement-series math, import
   merge, baselines/stats, export assembly, or other — pick one for phase 1.

4. **Binding technology:** napi-rs native prebuilds (recommended for CLI) vs
   WASM vs both. Recommendation: napi-rs for Node; defer WASM.

5. **Platform tier-1 list for native prebuilds:** e.g. `linux-x64-gnu`,
   `linux-arm64-gnu`, `darwin-arm64`, `darwin-x64`, `win32-x64-msvc` — confirm
   or shrink.

6. **TS fallback policy:** require fallback implementation until all tier-1
   prebuilds ship (recommended) vs hard-require native and fail install
   elsewhere.

7. **When to append D130:** approve this plan as-is, approve with edits, or
   reject and keep D121 with no Rust timeline.

## 13. Relationship to in-flight work

| Work | Relationship |
|---|---|
| CR4 `ath migrate` | Do not edit migrate sources in Rust planning PRs; Rust must use the same UX and SQL outcomes when it touches DBs. |
| CR5 conformance vectors | Prerequisite before enabling Rust by default; Rust spike may add vectors for its module in the same PR series. |
| CR8 workspace split | Compatible: `@ath/core` pure logic is the first TS boundary; Rust module replaces or accelerates the same pure functions, not `store` migrations first. |
| v1.0.0 npm publish | Rust is not a release gate for v1.0.0 unless the owner explicitly promotes it. |

## 14. Approval checklist

- [ ] Owner answered open decisions 1–7.
- [ ] Draft D130 text accepted or edited.
- [ ] Plan moved or linked from `build-history/v1.0.0/decisions.md` per
      repository process.
- [ ] No Rust implementation merged before D130 is recorded.
