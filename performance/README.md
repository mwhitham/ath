# Release performance proof

This folder holds the performance evidence for releases.

- [`limits.json`](limits.json) records the 2.1.0 reference measurements and the
  review limits from its named reference runner. It is historical evidence. The
  tools below never change it.
- [`report-2.1.1.md`](report-2.1.1.md) is the 2.1.1 report: the native program
  compared with 2.1.0 on the same generated databases (spec 2.1.1 section 14).

Large databases are generated locally or in the release workflow. They are not
committed. Keep them outside Git, in a temporary folder.

## The nine release measurements

The required sizes are 100,000 and 3 million current records. The generator is
the 2.1.0 one, `scripts/performance/generate-database.ts` in a checkout of the
reference commit, with seed `ath-v1-performance-seed-2`. Each database includes
three sources that record the same quantities, dense all-day and workout
samples, summaries, scores, sessions, corrections, deletions, athlete reports,
nutrition, plans, and predictions. The 3-million-record database is about 5.8 GB.

The measurements are:

- Initial package import.
- Incremental package import.
- Latest-value lookup.
- A 28-day evidence request.
- A 90-day baseline.
- A within-workout calculation.
- A two-source comparison.
- Complete JSON export.
- A complete database check.

Reports include database counts and sizes, hardware, operating system, SQLite and
compiler versions, settings, repetitions, percentiles, peak process memory, output
bytes and hashes, and query plans.

Larger generated databases are optional investigation tools. They are not a
release gate.

## How the 2.1.1 comparison measures

- `tools/performance/compare.ts` runs complete commands end to end. Each run is a new process,
  wrapped in `/usr/bin/time -l` for its peak resident memory. A sampler reads
  `ps` for the whole process tree every 250 ms (5 ms for the separate short-query
  memory runs), so a helper process such as the 2.1.0 `ath-core` counts together
  with its parent. Every run is one JSON line in `<root>/<label>/results/`.
- `tools/performance/library.rs` is the native counterpart of
  `scripts/performance/run-benchmarks.ts`. It calls the library directly and
  reports library latency without process startup. With `--plans`, it records the
  query plan of every distinct read statement each operation runs. `cargo test`
  runs its own test, which measures all nine operations once on the demo
  database with invented series and requires the evidence request to read
  records through `records_record_type_idx`.
- The 2.1.0 library latency comes from the unchanged
  `scripts/performance/run-benchmarks.ts` in the reference checkout.

The old command is `node <reference>/dist/cli.js`, the published entry point, so
Node startup is part of every old measurement. The native command is the release
`ath` executable. Rust compilation is never part of a measurement.

### Inputs

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

The generated data passes the native check with no issue at every size, and the
2.1.0 check at 100,000 records. At 3 million records the 2.1.0 check stops at
Node's default heap limit; with a 16 GiB heap it also reports no issue. So no
corrected copy of the generator was needed.

`prepare` refuses a generated database that does not hold all 24 record types,
corrections, deletions, and samples from three sources, or whose migrated copy
the native check rejects. These are the checks the 2.1.0 generator test made.

### Requirements

- macOS (the tools use `/usr/bin/time -l`, `ps`, `cp -c`, and `sqlite3`).
- Node 22.18 or later, which runs the TypeScript file directly.
- A checkout of the 2.1.0 reference with `node_modules`, a built `dist/`, and
  the release `crates/ath-core` binary. Set `ATH_REFERENCE` to its path.
- `/usr/bin/time -l` reads a system clock setting. Run `compare.ts` outside any
  sandbox that blocks `sysctl`.

### Commands

Run from the repository root. `PERF_ROOT` is any folder outside Git with room for
about 45 GB at the 3-million-record size.

```sh
export ATH_REFERENCE=/path/to/2.1.0/checkout
export PERF_ROOT=/private/tmp/ath-perf
TOOLS=tools/performance

# Native release builds. Compilation happens here, never during a measurement.
cargo build --release --offline
cargo build --release --offline --example performance-library

# Record the machine, operating system, and versions.
node $TOOLS/compare.ts environment

# Generate each size with the 2.1.0 generator, make the migrated native copy,
# export the shared import package, and write the one-record package.
node $TOOLS/compare.ts prepare --label 100k --records 100000
node $TOOLS/compare.ts prepare --label 300k --records 300000
node $TOOLS/compare.ts prepare --label 1m --records 1000000
node $TOOLS/compare.ts prepare --label 3m --records 3000000

# Export, check, initial import, and incremental import: three complete runs,
# alternating which implementation goes first. A 3-million-record run takes
# about 45 minutes, so each run is its own invocation there.
node $TOOLS/compare.ts bulk --label 100k --runs 3
node $TOOLS/compare.ts bulk --label 3m --runs 1 --first-run 1
node $TOOLS/compare.ts bulk --label 3m --runs 1 --first-run 2
node $TOOLS/compare.ts bulk --label 3m --runs 1 --first-run 3

# After a change to import, measure import alone the same way: three paired
# runs at 3 million records, alternating order, and three native runs at
# 300,000 and 1 million records. Each import run also makes the one-record
# merge.
node $TOOLS/compare.ts bulk --label 3m --runs 1 --first-run 1 --operations import
node $TOOLS/compare.ts bulk --label 3m --runs 1 --first-run 2 --operations import
node $TOOLS/compare.ts bulk --label 3m --runs 1 --first-run 3 --operations import
node $TOOLS/compare.ts bulk --label 300k --runs 3 --only native --operations import
node $TOOLS/compare.ts bulk --label 1m --runs 3 --only native --operations import

# The 2.1.0 check stops at Node's default heap limit at 3 million records.
# One supplementary run with a 16 GiB heap shows how long it takes to finish.
node $TOOLS/compare.ts bulk-old-heap --label 3m --runs 1 --only old \
  --operations check --old-heap-mib 16384

# Short commands: --help, --version, stats, latest value, evidence, baseline,
# workout drift, and source comparison. One first run each, then 50 runs, then
# 5 runs with the fast memory sampler.
node $TOOLS/compare.ts queries --label 100k --runs 50 --memory-runs 5
node $TOOLS/compare.ts queries --label 3m --runs 50 --memory-runs 5

# Library latency: the 2.1.0 runner and the native library program, 50
# repetitions of each read. At 3 million records one run each, and the 2.1.0
# runner gets the 16 GiB heap so its database check can finish.
node $TOOLS/compare.ts library --label 100k --runs 3 --repetitions 50
node $TOOLS/compare.ts library --label 3m --runs 1 --repetitions 50 --no-plans \
  --old-heap-mib 16384

# Working memory at increasing sizes with the same record sizes (native only).
node $TOOLS/compare.ts scaling --label 300k --runs 3 --only native
node $TOOLS/compare.ts scaling --label 1m --runs 3 --only native

# One very large valid record: a workout session with 200,000 route points and
# 200,000 heart-rate samples, added to the demo package.
node $TOOLS/compare.ts large-record --label large --points 200000 --runs 3

# Markdown tables (medians, spread, percentiles, peak memory, output bytes and
# hashes) and a JSON summary for each size.
node $TOOLS/compare.ts tables --label 3m
node $TOOLS/compare.ts summary --label 3m
```

`bulk` keeps the first export of each implementation until both exist, then
compares them file by file after replacing the version text (`2.1.0` or `2.1.1`)
and the file hashes that depend on it. It also hashes the restored records of
each implementation. The comparisons are written to
`<root>/<label>/results/bulk-content.jsonl`.

### Cache conditions

The tools do not clear the operating system's file cache: `purge` needs
administrator rights. Every implementation runs under the same conditions. Each
command starts a new process, so no SQLite page cache survives between runs.
Each short command has one first run, reported apart from the measured runs.
Bulk runs alternate which implementation goes first.
