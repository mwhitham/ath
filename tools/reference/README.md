# Reference generators

These scripts recorded the behavior of the 2.1.0 program that the native 2.1.1
program is compared with (spec 2.1.1 section 13.1). Their outputs are committed:

| Script | Output |
|---|---|
| `command-inventory.ts` | `build-history/v2.1.1/appendix/command-inventory.json` |
| `public-api-inventory.ts` | `build-history/v2.1.1/appendix/public-api.json` |
| `generate-model-vectors.ts` | `tests/vectors/model/`, `tests/vectors/model-index.json`, `tests/vectors/json/` |
| `cli-golden.ts` | `tests/golden/cli/` |
| `calculation-golden.ts` | `tests/golden/{calculate,view,prediction,log}/` |
| `import-golden.ts`, `import-golden-apple.ts` | `tests/golden/import/` |
| `route-golden.ts` | `tests/golden/routes/` |
| `package-golden.ts` | `tests/golden/package/` |
| `maintenance-golden.ts` | `tests/golden/maintenance/` |

The Rust tests read these files. They never run the scripts. Rerun a script
only to record a new invented case from the 2.1.0 program.

## Running a script

This repository no longer holds the 2.1.0 program. The scripts load it from a
separate checkout of the reference commit
`c6218e88c035708bdd15b3a20888d19713e1df29`:

```sh
git worktree add ../ath-2.1.0 c6218e88c035708bdd15b3a20888d19713e1df29
(cd ../ath-2.1.0 && pnpm install --frozen-lockfile && \
  cargo build --release --manifest-path crates/ath-core/Cargo.toml)

export ATH_REFERENCE_ROOT="$(cd ../ath-2.1.0 && pwd)"
node --import "$ATH_REFERENCE_ROOT/node_modules/tsx/dist/esm/index.mjs" \
  tools/reference/command-inventory.ts
```

`ATH_REFERENCE_ROOT` is required. Each script reads the old source, its
packages, and the synthetic inputs from that checkout, and writes its output
into this repository. `reference.ts` holds the shared lookup. The import, route,
and maintenance scripts also need the release `ath-core` helper built in the
checkout. `maintenance-golden.ts` also needs a `sqlite3` program on the path,
because the 2.1.0 backup used it. `route-golden.ts` starts a local HTTP server.

All inputs are invented. A script that records a new case must keep it
synthetic, and a new health-shaped file must pass `tests/public_data.rs`.
