# Should Athletic Standard use Rust?

**Recommendation (interim):** **No — not now.** Keep the core in TypeScript and
SQLite and ship v1 on the current stack. D121 keeps the public OSS base thin; D7
keeps the agent surface as CLI + Skill, not MCP or a local HTTP service.

This note lives in `build-history/plans/` as a proposed plan. It is not
approved and does not add a D-number to `decisions.md` yet.

**Status:** Mike asked to drop performance benchmarking from this plan and to
**pause** further rewriting until he sets what the plan should focus on. The
sections below are background only, not a finished proposal.

---

## SQLite and native code today

Athletic Standard uses **`@libsql/client`**, which already downloads a **native
libSQL binary** per platform (similar in distribution terms to `better-sqlite3`).
Adding a Rust application core would not remove native code from `npm install`;
it would add another native layer on top of the existing SQLite client.

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
boundary. ATH is closer to Prisma (data + queries + validation) than to SWC
(compile loop).

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
large SQLite-backed import/export paths. Neither removes the SQLite native
dependency.

---

## Draft decision for owner (not recorded until approved)

### D130 (draft). Defer a Rust core unless a later plan approves it

Athletic Standard does **not** add a Rust core for v1.0.0. D121’s thin public
base stays: file, spec, vectors, CLI, Skills — no maintained Rust port until
a future owner-approved plan supersedes that deferral.

If Rust is approved later, it may ship **inside** `athleticstandard` behind
existing commands, with conformance parity and without changing D7’s CLI + Skill
surface.

Rejected for now:

- **Rust core without an approved plan and named need.** Extra native binaries
  and maintainers under D121.
- **WASM as the default CLI engine.** Poor fit for large SQLite-backed I/O.
- **Replacing libSQL with better-sqlite3 just to “add Rust.”** ATH already uses
  a native SQLite stack; the question is application logic, not the driver brand.

---

**Placement note:** `build-history/README.md` does not name a folder for
unapproved plans. This file stays at `build-history/plans/rust-adoption.md`
until approval; then it should be linked from a recorded D-number in the
appropriate version folder (likely `v1.0.0/appendix/` if the work does not
change the format version).
