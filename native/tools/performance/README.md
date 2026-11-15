# Native performance comparison

These tools compare the 2.1.0 reference implementation with the native 2.1.1
executable on the same generated databases. The results for the release are in
[`native/performance/report-2.1.1.md`](../../performance/report-2.1.1.md).

Generated databases and packages are large. Keep them outside Git, in a
temporary folder. `performance/limits.json` stays as the 2.1.0 historical
evidence. These tools do not change it.

## What is measured

- `compare.ts` runs complete commands end to end. Each run is a new process,
  wrapped in `/usr/bin/time -l` for its peak resident memory. A sampler reads
  `ps` for the whole process tree every 250 ms (5 ms for the separate short-query
  memory runs), so a helper process such as the 2.1.0 `ath-core` counts together
  with its parent. Every run is one JSON line in `<root>/<label>/results/`.
- `library.rs` is the native counterpart of
  `scripts/performance/run-benchmarks.ts`. It calls the library directly and
  reports library latency without process startup. With `--plans`, it records the
  query plan of every distinct read statement each operation runs.
- The 2.1.0 library latency comes from the unchanged
  `scripts/performance/run-benchmarks.ts` in the reference checkout.

The old command is `node <reference>/dist/cli.js`, the published entry point, so
Node startup is part of every old measurement. The native command is the release
`ath` executable. Rust compilation is never part of a measurement.

## Inputs

Both implementations read the same inputs:

- The database is made by the unchanged 2.1.0 generator
  (`scripts/performance/generate-database.ts`, seed `ath-v1-performance-seed-2`).
  The old commands read it as generated. The native commands read a copy that
  `ath migrate` moved to SQL schema step 5; that step adds one internal table and
  changes no record.
- The import input is the package that the 2.1.0 `ath export` writes from the
  generated database. It is byte-identical to the package that the 2.1.0 runner
  exports and imports itself.
- The incremental package is the 2.1.0 runner's one-record package, written by
  the 2.1.0 library.

The generated data passes both the 2.1.0 check and the native check with no
issue, so no corrected copy of the generator was needed.

## Requirements

- macOS (the tools use `/usr/bin/time -l`, `ps`, `cp -c`, and `sqlite3`).
- Node 22.18 or later, which runs the TypeScript file directly.
- A checkout of the 2.1.0 reference with `node_modules`, a built `dist/`, and
  the release `crates/ath-core` binary. Set `ATH_REFERENCE` to its path.
- `/usr/bin/time -l` reads a system clock setting. Run `compare.ts` outside any
  sandbox that blocks `sysctl`.

## Commands

Run from the repository root. `PERF_ROOT` is any folder outside Git with room for
about 45 GB at the 3-million-record size.

```sh
export ATH_REFERENCE=/path/to/2.1.0/checkout
export PERF_ROOT=/private/tmp/ath-perf
TOOLS=native/tools/performance

# Native release builds. Compilation happens here, never during a measurement.
(cd native && cargo build --release --offline && \
  cargo build --release --offline --example performance-library)

# Record the machine, operating system, and versions.
node $TOOLS/compare.ts environment

# Generate each size with the 2.1.0 generator, make the migrated native copy,
# export the shared import package, and write the one-record package.
node $TOOLS/compare.ts prepare --label 100k --records 100000
node $TOOLS/compare.ts prepare --label 3m --records 3000000

# Export, check, initial import, and incremental import: three complete runs,
# alternating which implementation goes first.
node $TOOLS/compare.ts bulk --label 100k --runs 3
node $TOOLS/compare.ts bulk --label 3m --runs 3

# Short commands: --help, --version, stats, latest value, evidence, baseline,
# workout drift, and source comparison. One first run each, then 50 runs, then
# 5 runs with the fast memory sampler.
node $TOOLS/compare.ts queries --label 100k --runs 50 --memory-runs 5
node $TOOLS/compare.ts queries --label 3m --runs 50 --memory-runs 5

# Library latency: three runs of the 2.1.0 runner and of the native library
# program, 50 repetitions of each read.
node $TOOLS/compare.ts library --label 100k --runs 3 --repetitions 50
node $TOOLS/compare.ts library --label 3m --runs 3 --repetitions 50

# Working memory at increasing sizes with the same record sizes (native only).
node $TOOLS/compare.ts prepare --label 300k --records 300000
node $TOOLS/compare.ts prepare --label 1m --records 1000000
node $TOOLS/compare.ts scaling --label 300k --runs 1 --only native
node $TOOLS/compare.ts scaling --label 1m --runs 1 --only native

# One very large valid record: a workout session with 200,000 route points and
# 200,000 heart-rate samples, added to the demo package.
node $TOOLS/compare.ts large-record --label large --points 200000 --runs 3

# Medians, percentiles, spread, peak memory, output bytes, and hashes.
node $TOOLS/compare.ts summary --label 100k
node $TOOLS/compare.ts summary --label 3m
```

`bulk` keeps the first export of each implementation until both exist, then
compares them file by file after replacing the version text (`2.1.0` or `2.1.1`)
and the file hashes that depend on it. It also hashes the restored records of
each implementation. The comparisons are written to
`<root>/<label>/results/bulk-content.jsonl`.

## Cache conditions

The tools do not clear the operating system's file cache: `purge` needs
administrator rights. Every implementation runs under the same conditions. Each
command starts a new process, so no SQLite page cache survives between runs.
Each short command has one first run, reported apart from the measured runs.
Bulk runs alternate which implementation goes first.
