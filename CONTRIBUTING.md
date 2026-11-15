# Contributing

Use a feature branch and open a pull request against `main`. Keep each commit to one logical change.

## Protect health data

Never commit a real health export or athlete file. This rule applies to public and private repositories because clones, logs, and old commits retain deleted data.

Keep real validation data in encrypted storage outside Git. Public bug reports, tests, and decisions must use the smallest invented example that proves the behavior. Remove exact workout dates, locations, birth dates, travel clues, device inventories, and sample values.

Files under `tests/fixtures/` and `examples/demo-athlete/` are synthetic. Adding a health-shaped file requires an entry with its exact path and SHA-256 in `tools/public-data-allowlist.json`. Health-data files also require an explicit exception in `.gitignore`. `tests/public_data.rs` enforces both.

Do not upload real exports to public continuous integration. Run private validation locally and publish only a sanitized conclusion.

## Build

Install the Rust toolchain named in `rust-toolchain.toml`, then build from the repository root:

```sh
cargo build
```

The npm package in `packages/npm/` has its own dependencies. Install them with `npm ci` in that folder. Its tests run `target/debug/ath`, or the executable named by `ATH_NATIVE_EXECUTABLE`.

## Verify a change

Run:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cd packages/npm
npm ci
npm run typecheck
npm test
```

`cargo test` also checks public data and every generated file.

## What GitHub checks

- **Every pull request push, and every push to `main`:** the quick check. Linux
  runs formatting, Clippy, every Rust test, and the npm tests on Node 24.
  Windows runs Clippy and every Rust test. The `checks passed` job must pass
  before a merge. A new push cancels the run for the push before it.
- **A version tag, such as `v2.1.1`:** the full check on Linux, macOS, and
  Windows, with release builds, archive and npm install checks, and Node 22
  and 24. If everything passes, it publishes to npm and creates the GitHub
  release. To run the full check on a branch without publishing, use Actions >
  Release cross-platform > Run workflow.
- **The speed comparison with 2.1.0** runs only by hand: Actions > Release
  performance > Run workflow.

## Generated files

JSON Schema, the npm protocol module, the skill's command reference, and the demo database are generated from the Rust sources. After changing the record model, a command or its help, or a protocol operation, regenerate them and commit the result:

```sh
ATH_WRITE_GENERATED=1 cargo test --test generated
```

Do not edit a generated file by hand. Published historical schemas, the SQL files in `spec/sql/`, and the fixtures in `tests/fixtures/` are never regenerated.
