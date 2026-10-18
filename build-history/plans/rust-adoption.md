# Should Athletic Standard use Rust?

**Recommendation:** **No — not now.** Keep the core in TypeScript and SQLite.
Ship v1 on the current stack. Revisit Rust only if release performance
limits fail **after** the SQL and batching improvements already listed in the
v1 build spec, or if a builder proves they cannot ship without a supported
native library.

This note lives in `build-history/plans/` as a proposed plan. It is not
approved and does not add a D-number to `decisions.md` yet.

---

## What we measured (2026-10-18, Linux x64, Node 22, 4 vCPUs)

Commands: `pnpm build`, `pnpm performance:generate`, `pnpm performance:run`
(see `performance/README.md`). Seed: `ath-v1-performance-seed-2`.

| Operation | 10k records (p95) | 100k records (p95) | Release limit at 100k |
|---|---:|---:|---:|
| Latest value lookup | 0.4 ms | 0.3 ms | 5 ms |
| 28-day evidence | 1.0 ms | 0.9 ms | 10 ms |
| 90-day baseline | 2.2 ms | 36 ms | 100 ms |
| Within-workout series math | 9.0 ms | 8.0 ms | 50 ms |
| Cross-source comparison | 2.0 ms | 68 ms | 150 ms |
| Incremental package import | 5.4 ms | 8.5 ms | 100 ms |
| Full JSON export | 1.4 s | 12.8 s | 20 s |
| Database check | 1.1 s | 10.3 s | 15 s |
| Initial package import | 3.3 s | **31.0 s** | **45 s** |

At **100k current records** (~185 MB database, ~100k measurement samples), every
operation sits **inside** the checked limits in `performance/limits.json`.
Nothing in daily agent use (evidence, baselines, series math) is slow. The heavy
work is **whole-file**: import, export, and check walk the full dataset.

`ath --help` cold start (8 runs): **~170 ms** median — mostly loading the bundled
JavaScript, not SQLite.

`pnpm test` (327 tests): **~90 s** wall time on this runner.

**Where it hurts at scale:** the repo’s reference **3 million** record run (not
re-run here; numbers are in `performance/limits.json`) shows initial import
~17 minutes, full export ~8.8 minutes, and check ~5.7 minutes. That is I/O and
volume, not a missing Rust parser.

**SQLite today:** Athletic Standard uses **`@libsql/client`**, which already
downloads a **native libSQL binary** per platform (same class of problem as
`better-sqlite3`). Moving logic to Rust does not remove native code from
`npm install`; it would add **another** native layer on top.

---

## What Rust would and would not fix

**Would help (in theory):** tight loops on measurement samples, faster XML/CSV
parsing for huge vendor exports, lower CPU on export serialization.

**Would not help much:** SQLite read/write and trigger enforcement (already in
the database engine), CLI startup (Node bundle load), or keeping two
implementations in sync with Zod schemas and conformance tests.

**Cost:** a Rust core means prebuilt binaries per OS/CPU/libc, release CI matrix,
Rust-capable maintainers, and duplicate behavior tests. The public OSS base is
meant to stay thin (recorded as D121). The agent surface stays CLI + Skill (D7);
Rust is not an excuse to add MCP or a local HTTP server.

**Effort (rough):** one bounded napi-rs module (e.g. series decode only):
several weeks to ship safely with fallbacks. A full “Rust engine” behind `ath`:
many months and ongoing parity work. Not justified while 100k limits still pass.

---

## What other npm tools did with Rust

These tools adopted Rust for **hot, repetitive CPU work** (parse, transform,
minify). They pay for it with **large optional platform packages** and CI
that builds every target.

| Project | Rust role | Install / ops lesson |
|---|---|---|
| [SWC](https://swc.rs/) | JS/TS compiler | Per-platform `@swc/core-*` packages; Linux x64 install ~27 MB ([comparison table](https://github.com/bluwy/npm-prebuilt-binaries-comparison)). |
| [Biome](https://biomejs.dev/) | Linter + formatter | Single meta-package; ~43–63 MB install per OS ([same comparison](https://github.com/bluwy/npm-prebuilt-binaries-comparison)). |
| [Rolldown](https://rolldown.rs/) / [Oxc](https://oxc.rs/) | Bundler / parser | Optional `@rolldown/binding-*`; WASM fallback exists but native is default for speed. |
| [Lightning CSS](https://lightningcss.dev/) | CSS transform | Smaller binaries (~8–10 MB install) because scope is narrow. |
| [Prisma ORM](https://www.prisma.io/blog/from-rust-to-typescript-a-new-chapter-for-prisma-orm) | **Moved away** from Rust engine (2025–2026) | Cross-language copies hurt; **TypeScript + WASM query compiler** replaced multi‑MB Rust binaries; simpler deploys and more contributors. |

**Lesson for Athletic Standard:** Rust in npm works when the win is huge and the
API surface is stable (SWC, esbuild). Prisma’s reversal is relevant: ATH already
crosses JS↔native for SQLite; adding Rust business logic increases that
boundary unless the speedup is large and measured. ATH is closer to Prisma
(data + queries + validation) than to SWC (compile loop).

---

## napi-rs vs WASM for `athleticstandard`

| | **napi-rs** (native addon) | **WASM** |
|---|---|---|
| Fit for `ath` CLI on Node 20+ | Good — same model as libSQL today | Possible but awkward for large file I/O |
| `npm install` | +1 optional package per platform (~8–30 MB typical; see comparison above) | One portable blob; often 3–15 MB |
| Platforms | Must build/test linux/macOS/windows × arm64/x64 (+ musl) ([napi-rs docs](https://napi.rs/docs/introduction/getting-started)) | One build; slower on hot paths |
| CI | Multi-target release workflow | Simpler, but still test all vectors |
| ATH SQLite | Still need libSQL native client | Still need libSQL; WASM does not replace it |

**Conclusion:** If ATH ever needs native code for **compute**, **napi-rs** matches
the existing libSQL distribution story. **WASM** is a poor default for
multi‑gigabyte import/export paths. Neither removes the SQLite native dependency.

---

## What we do now

1. **Stay TypeScript.** Finish v1 publish work, SQL contract, conformance vectors,
   and `ath migrate` (in flight elsewhere).
2. **Optimize where the timers point:** batching, indexes, export/check streaming,
   and series storage per v1 spec §13 — all before any Rust spike.
3. **Keep `performance/` as the gate.** Limits at 100k and 3M are already encoded.

**Revisit Rust when any of these happen:**

- **100k initial import** p95 **exceeds 45 s**, or **full JSON export** p95
  **exceeds 20 s**, on the reference runner **after** §13 optimizations land; or
- **3M** initial import p95 **exceeds 30 minutes** after the same optimizations; or
- A **documented builder** needs a supported native library and cannot use direct
  SQLite + public vectors within their timeline.

If a trigger fires, start with **one** narrow native module (series math or import
parse), TypeScript fallback, and shared conformance fixtures — not a full rewrite.

---

## Draft decision for owner (not recorded until approved)

### D130 (draft). Defer a Rust core until measured limits or a builder force it

Athletic Standard does **not** add a Rust core for v1.0.0. D121’s thin public
base stays: file, spec, vectors, CLI, Skills — no maintained Rust port until
the revisit triggers in this plan are met.

If a trigger is met, Rust may ship **inside** `athleticstandard` behind existing
commands, with conformance parity and without changing D7’s CLI + Skill surface.

Rejected for now:

- **Rust core without failed limits or a named builder.** Extra native binaries
  and maintainers without a measured win.
- **WASM as the default CLI engine.** Poor fit for large SQLite-backed I/O.
- **Replacing libSQL with better-sqlite3 just to “add Rust.”** ATH already uses
  a native SQLite stack; the question is application logic, not the driver brand.

---

**Placement note:** `build-history/README.md` does not name a folder for
unapproved plans. This file stays at `build-history/plans/rust-adoption.md`
until approval; then it should be linked from a recorded D-number in the
appropriate version folder (likely `v1.0.0/appendix/` if the work does not
change the format version).

**Evidence artifacts (local only, not committed):**
`performance/generated/{10k,100k}/`, `performance/results/local/*-rust-plan.json`.
