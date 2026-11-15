# Native 2.1.1 performance report

This report covers spec 2.1.1 section 14. It compares the 2.1.0 reference with the native 2.1.1 executable on the same generated inputs, on one machine. The tools and exact commands are in [README.md](README.md) and `tools/performance/`. [`limits.json`](limits.json) is unchanged; it is the 2.1.0 historical evidence.

The measurements were made in two rounds. The first round measured every operation and found two missed import targets. The import changes in "Native changes made during this work" followed. The second round measured every import again, the whole 100,000-record bulk table, the 100k library latency of both versions, the native 3M library latency, and the very large record. Each table names the round it comes from.

## Result

| Target | Result |
|---|---|
| All nine existing limits | Pass. Native library p95 is under every limit at 100,000 and 3 million records. |
| 3M initial import at most half the old median | Pass. Native median 408.7 s; old median 878.7 s. Ratio 0.465 (0.468, 0.461, and 0.473 in the three pairs). |
| 3M database check at most half the old median | Pass, with a caveat. The 2.1.0 check ran out of Node heap in all three runs. With a 16 GiB heap it took 419.6 s; native median 80.1 s. |
| Export no slower than the old one | Pass at both sizes. |
| At most 512 MiB peak memory for import, export, and check | Pass. Import peak is at most 185.0 MiB at 3M and 324.2 MiB at 1M. Export is at most 40.1 MiB and check at most 15.2 MiB. |
| Bounded working memory as counts grow | Pass. Export and check stay flat. Import memory follows the size of the largest series file. It stops growing once files reach their 50,000-sample cap at 1M. |
| One very large valid record | Measured and accepted. |
| Cold `--help`, `--version`, `stats`, latest value | Measured. No other runtime starts. |

## Machine and software

- Apple M4 Pro (10 performance and 4 efficiency cores), 24 GiB memory, internal SSD (APFS).
- macOS 26.6.2 (25G83), Darwin 25.6.0 arm64.
- The machine was shared: other build jobs ran during the measurements. The 1-minute load average is recorded before every run.
- **2.1.0 build:** `node dist/cli.js` from a reference checkout. Its `src/`, `crates/`, `scripts/performance/`, and `examples/demo-athlete/` equal commit c6218e8. A fresh `tsup` build of that source matched `dist/` apart from chunk file names.
- **2.1.0 software:** Node v25.6.1, `@libsql/client` 0.18.0 (SQLite 3.45.1), and the release `ath-core` helper (rusqlite 0.32, libsqlite3-sys 0.30.1, bundled SQLite).
- **Native build:** 2.1.1 release profile (thin LTO, 4 codegen units), rustc 1.100.0 (0ef1bc386 2026-11-09), rusqlite 0.42 with libsqlite3-sys 0.40.0 (bundled SQLite 3.54.1).
- **Measured native executables:**
  - **Round 1:** sha256 721058bb9d095ab4b244f6e18a2791a23682b69b412c9194b59254151bddc97a (11,948,560 bytes), built from 45aef60. Its rows are the 3M export and check, the short commands at both sizes, the 3M query plans, and the export and check rows at 300k and 1M.
  - **Round 2:** sha256 1ecdf261995c17b4e42385d00a7120ed80769460ceaf3d7266b872e6be728c2d (12,627,088 bytes), built from 6e7f459. Its rows are every import, the 100k bulk table, the 100k library table, the native 3M library run, and the very large record.
  - **Later commits:** 5c07374 changes only what a read-only open does after two kinds of error, so it runs on no measured path (see "Limits of this report"). The cutover moved the code to the repository root without changing it.
  - The commits between the two builds change import, restore, and merge. They also add the JavaScript client worker (`ath __client`), change the human text of `ath calculate`, and change Windows path handling. None of them changes the export, check, or read paths that only round 1 measured. The 100k export and check, measured in both rounds, agree within their spread.
- **SQLite settings, both versions:** WAL journal, 4,096-byte pages, `cache_size` −2000 (2,000 KiB), `mmap_size` 0, `synchronous` FULL.
- **Native restore settings:** the restore transaction sets `temp_store` to MEMORY. A restore into a new, empty database also uses a rollback journal (`journal_mode` DELETE) and grows the file 8 MiB at a time, then returns the file to WAL mode before the command ends. `synchronous` stays FULL. See "Restore into a new database".

## Inputs

Both versions read the same inputs.

- **Databases** come from the unchanged 2.1.0 generator (`scripts/performance/generate-database.ts` in the reference checkout, seed `ath-v1-performance-seed-2`).
  - The native commands read a copy moved to SQL schema step 5 by `ath migrate`. That step adds one internal table and changes no record.
  - The data passes the native check with no issue at every size, and the 2.1.0 check at 100k and (with a larger heap) at 3M. So no corrected generator was needed.
  - Round 2 generated every size again with the same command. Record counts, byte counts, and hashes equal round 1's.
- **The import input** is the package that 2.1.0 `ath export` writes from the generated database. It is byte-identical to the package the 2.1.0 runner exports and imports itself.
- **The one-record package** for the incremental import is written by the 2.1.0 library, as in the 2.1.0 runner. Its sha256 is 64572306…b9150.

| Size | Records | Samples | Revisions | 2.1.0 database bytes | Package files / bytes | Package hash |
|---|---|---|---|---|---|---|
| 100k | 100,000 (1 deleted) | 99,974 | 100,001 | 194,125,824 | 21 / 195,366,345 | b73e3e5f… |
| 300k | 300,000 | – | – | – | – / 586,105,301 | – |
| 1M | 1,000,000 | – | – | – | – / 1,953,710,081 | – |
| 3M | 3,000,000 (1 deleted) | 2,999,974 | 3,000,001 | 5,818,683,392 | 75 / 5,861,136,493 | 56d84bae… |

## Method

- **End to end.** Every measurement is a new process, so the 2.1.0 command includes Node startup. Rust compilation is never measured.
- **Memory.** Each run is wrapped in `/usr/bin/time -l` for peak resident memory and peak footprint. Peak resident memory includes memory the allocator keeps after it is freed. A `ps` sampler adds up the whole process tree, every 250 ms for bulk runs and 5 ms for separate short-command memory runs. That is how the 2.1.0 `ath-core` helper is counted with its parent.
- **Disk.** For imports, the database file plus its WAL and shared-memory file is sampled. The rollback journal of a native restore is not sampled; it stays about 350 KB (see "Restore into a new database").
- **Repetitions.**
  - Bulk operations: three complete runs per size. Runs alternate which version goes first.
  - Round 2 measured import alone at 3M: three paired runs, the 2.1.0 import first in runs 1 and 3 and the native import first in run 2. Each import run is followed by the one-record merge.
  - Short commands: one first run, kept apart; then 50 measured runs; then 5 memory runs.
  - Library latency: one discarded warmup, then 50 repetitions per read.
    - 100k: three runs each, through the unchanged 2.1.0 runner and the native `performance-library` program.
    - 3M: one run each. The 2.1.0 runner (round 1) used a 16 GiB Node heap so its check could finish.
- **Cache.** The OS file cache was not cleared, because `purge` needs administrator rights. No SQLite cache survives between commands, since every command is a new process.
- **The 3M database check, supplementary run.** Under Node's default heap, the 2.1.0 check stopped with "Reached heap limit … JavaScript heap out of memory" in every run. One extra run with `--max-old-space-size=16384` shows its completion time.

## Results at 100,000 records

### Bulk, end to end (round 2)

| Operation | Version | Runs | Median | Min | Max | Spread | Native/old | Peak RSS | Peak footprint | Tree peak | Peak disk | Output bytes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| export | old | 3 | 8,560.7 ms | 8,500.9 | 8,988.5 | 487.6 ms | | 465.0 MiB | 448.8 MiB | 455.5 MiB | – | 126 (package 195,366,345) |
| export | native | 3 | 2,753.4 ms | 2,666.4 | 3,317.6 | 651.2 ms | 0.322 | 27.3 MiB | 19.6 MiB | 27.0 MiB | – | 129 (package 195,366,345) |
| check | old | 3 | 9,305.1 ms | 9,178.4 | 10.6 s | 1,460.9 ms | | 1,292.7 MiB | 1,288.1 MiB | 1,292.6 MiB | – | 390 |
| check | native | 3 | 1,942.6 ms | 1,936.1 | 1,952.7 | 16.6 ms | 0.209 | 15.2 MiB | 7.1 MiB | 15.1 MiB | – | 393 |
| initial import | old | 3 | 19.6 s | 19.5 s | 19.8 s | 337.5 ms | | 900.0 MiB | 756.8 MiB | 899.9 MiB | 369.3 MiB | 412 |
| initial import | native | 3 | 6,712.0 ms | 6,690.2 | 6,935.1 | 244.8 ms | 0.343 | 30.2 MiB | 21.7 MiB | 30.1 MiB | 184.0 MiB | 412 |
| incremental import | old | 3 | 138.9 ms | 133.1 | 152.8 | 19.7 ms | | 84.5 MiB | 47.4 MiB | 76.5 MiB | – | 356 |
| incremental import | native | 3 | 7.668 ms | 7.564 | 10.6 | 2.999 ms | 0.055 | 12.4 MiB | 4.2 MiB | – | – | 356 |
| `init --empty` | old / native | 3 / 3 | 126.1 / 10.9 ms | | | | 0.087 | 82.1 / 11.7 MiB | | | | 177 / 180 |

In round 1 the native initial import took a median 6,651.4 ms with 163.0 MiB peak. Before fixes 1–3, the unmodified native build (d3f44bf) took a median 14.6 s, with 223.8 MiB peak.

### Short commands, end to end (round 1)

| Command | Version | First run | p50 | p95 | p99 | Peak RSS | Tree peak | Output bytes | Output SHA-256 |
|---|---|---|---|---|---|---|---|---|---|
| `--help` | old | 113.0 ms | 111.6 | 115.0 | 120.9 | 77.5 MiB | 65.5 MiB | 3,766 | 8dafa9a31458 |
| `--help` | native | 5.7 ms | 5.5 | 5.8 | 6.4 | 7.7 MiB | | 3,766 | 8dafa9a31458 |
| `--version` | old | 112.1 ms | 110.7 | 112.9 | 113.9 | 77.5 MiB | | 6 | c89b87c262e9 |
| `--version` | native | 5.5 ms | 5.5 | 5.8 | 6.4 | 7.4 MiB | | 6 | 7b3ff271f137 |
| `stats --json` | old | 329.0 ms | 328.2 | 338.1 | 340.2 | 112.3 MiB | | 7,488 | a49ce86bf2a4 |
| `stats --json` | native | 219.6 ms | 214.0 | 220.3 | 230.0 | 20.0 MiB | | 7,488 | cae3dcfbc435 |
| latest value | old | 126.2 ms | 120.7 | 124.6 | 127.3 | 81.4 MiB | | 1,088 | 5209e980275f |
| latest value | native | 7.6 ms | 6.7 | 7.0 | 7.6 | 11.2 MiB | | 1,088 | 5209e980275f |
| evidence | old | 126.9 ms | 123.7 | 130.4 | 134.3 | 83.5 MiB | | 7,778 | 0abd6b63092c |
| evidence | native | 7.3 ms | 7.2 | 7.6 | 9.7 | 11.9 MiB | | 7,778 | 0abd6b63092c |
| baseline 90 d | old | 142.9 ms | 135.8 | 137.7 | 139.3 | 81.6 MiB | | 100,777 | 0c614f22ed60 |
| baseline 90 d | native | 17.6 ms | 17.6 | 18.0 | 18.4 | 15.6 MiB | | 100,777 | 0c614f22ed60 |
| workout drift | old | 125.9 ms | 126.8 | 131.6 | 134.4 | 81.8 MiB | | 100,966 | eaaabbde168a |
| workout drift | native | 9.0 ms | 8.9 | 9.1 | 9.2 | 14.4 MiB | | 100,966 | eaaabbde168a |
| source comparison | old | 148.1 ms | 146.8 | 151.8 | 152.8 | 81.5 MiB | 92.8 MiB (node + ath-core) | 101,149 | c690fbee1467 |
| source comparison | native | 28.2 ms | 28.0 | 28.6 | 28.9 | 19.9 MiB | 13.2 MiB | 101,149 | c690fbee1467 |

How each command was run:

| Command | Exact form |
|---|---|
| latest value | `ath series hrv_sdnn --raw --source <A> --from <last time> --to <last time> --json` |
| evidence | `ath predict fran --json --as-of 2026-10-13T23:59:59.999Z`, with the command's default 90-day window |
| baseline, drift, source comparison | `ath calculate … --json`, with the 2.1.0 runner's parameters |

### Library latency (in process, warm connection; round 2)

| Operation | old p50 / p95 | native p50 / p95 | Samples |
|---|---|---|---|
| latest value | 0.067 / 0.120 ms | 0.035 / 0.070 ms | 150 |
| evidence, 28 d | 0.208 / 0.343 ms | 0.119 / 0.184 ms | 150 |
| baseline 90 d | 16.2 / 16.9 ms | 9.706 / 10.1 ms | 150 |
| workout drift | 6.548 / 7.346 ms | 1.526 / 1.579 ms | 150 |
| source comparison | 26.9 / 27.2 ms | 19.5 / 20.0 ms | 150 |
| export | 8,519.6 / 8,521.6 ms | 2,717.9 / 2,917.3 ms | 3 |
| check | 8,946.6 / 9,051.9 ms | 1,913.8 / 1,915.2 ms | 3 |
| initial import | 19.5 / 19.5 s | 6,707.8 / 6,724.2 ms | 3 |
| incremental import | 3.829 / 4.002 ms | 1.251 / 4.918 ms | 3 |

## Results at 3 million records

### Bulk, end to end

Export and check are from round 1. Import is from round 2.

| Operation | Version | Runs | Median | Min | Max | Spread | Native/old | Peak RSS | Peak footprint | Peak disk | Output bytes |
|---|---|---|---|---|---|---|---|---|---|---|---|
| export | old | 3 | 438.3 s | 412.3 s | 593.9 s | 181.7 s | | 1,666.5 MiB | 1,645.1 MiB | – | 124 (package 5,861,136,493) |
| export | native | 3 | 184.6 s | 112.6 s | 279.1 s | 166.5 s | 0.421 | 40.1 MiB | 32.5 MiB | – | 127 (package 5,861,136,493) |
| check | old | 3, all failed | – (out of heap after 286.6, 268.3, 265.3 s) | | | | | 4,489.0 MiB | 4,725.8 MiB | – | 0 |
| check | old, 16 GiB heap | 1 | 419.6 s | | | | | 7,503.5 MiB | | – | 389 |
| check | native | 3 | 80.1 s | 72.9 s | 84.6 s | 11.8 s | 0.19 vs the 16 GiB run | 15.0 MiB | 7.1 MiB | – | 392 |
| initial import | old | 3 | 878.7 s | 850.3 s | 886.4 s | 36.1 s | | 5,134.8 MiB | 5,018.7 MiB | 11,081.8 MiB | 413 |
| initial import | native | 3 | 408.7 s | 402.5 s | 411.2 s | 8.7 s | **0.465** | 185.0 MiB (185.0, 170.2, 126.0 per run) | 148.4 MiB | 5,520.0 MiB | 413 |
| incremental import | old | 3 | 898.3 ms | 535.4 | 1,066.4 | 531.0 ms | | 85.1 MiB | 48.0 MiB | – | 356 |
| incremental import | native | 3 | 9.590 ms | 7.521 | 9.633 | 2.112 ms | 0.011 | 12.5 MiB | 4.3 MiB | – | 356 |

The three import pairs:

| Run | Order | 1-minute load before old / native | Old | Native | Native/old |
|---|---|---|---|---|---|
| 1 | old first | 4.57 / 3.51 | 878.7 s | 411.2 s | 0.468 |
| 2 | native first | 4.93 / 3.35 | 886.4 s | 408.7 s | 0.461 |
| 3 | old first | 3.87 / 3.72 | 850.3 s | 402.5 s | 0.473 |

- Half the old median is 439.3 s. The slowest native run is 28.1 s under it.
- In round 1 the native import median was 469.7 s against an old median of 921.6 s (ratio 0.510).
- The native import used 194–196 s of user time and 195–203 s of system time per run. The old import used 651–654 s of user time.
- Peak disk is the final database file. In round 1 it was 10,970.4 MiB, about twice the database, because every page went to the WAL first.
- Round 1 measured the incremental import before commit 475f7c8; its native median was then 2,576.4 ms.

### Short commands, end to end (round 1)

| Command | old p50 / p95 | native p50 / p95 | Output hash equal |
|---|---|---|---|
| `--help` | 112.7 / 126.6 ms | 5.544 / 6.448 ms | yes |
| `--version` | 111.3 / 121.4 ms | 5.520 / 7.085 ms | no (version text) |
| `stats --json` | 8,168.5 ms / 14.6 s (918.5 MiB) | 6,941.7 ms / 14.8 s (20.8 MiB) | no (7,531 bytes each) |
| latest value | 121.8 / 153.9 ms | 6.901 / 15.1 ms | yes |
| evidence | 125.0 / 148.6 ms | 7.290 / 10.9 ms | yes |
| baseline 90 d | 222.1 / 626.6 ms | 97.9 / 506.3 ms | yes |
| workout drift | 127.8 / 142.4 ms | 9.105 / 15.9 ms | yes |
| source comparison | 319.0 / 345.6 ms (tree 119.6 MiB with `ath-core`) | 188.4 / 236.1 ms (tree 37.5 MiB) | yes |

### Library latency, one run each

The 2.1.0 runner is from round 1, with the 16 GiB heap. The native program is from round 2.

| Operation | old p95 | native p95 |
|---|---|---|
| latest value | 0.123 ms | 0.048 ms |
| evidence, 28 d | 0.390 ms | 0.153 ms |
| baseline 90 d | 100.8 ms | 90.3 ms |
| workout drift | 7.013 ms | 1.651 ms |
| source comparison | 197.0 ms | 177.1 ms |
| export | 393.5 s | 194.3 s |
| check | 331.9 s | 59.7 s |
| initial import | 981.7 s (memory growth 9.2 GB) | 406.1 s |
| incremental import | 1,081.3 ms | 1.583 ms |

The native library export took 194.3 s here and 92.1 s in round 1, with the same export code. End to end, the round-1 native export ranged from 112.6 to 279.1 s. At this size, export time depends on the state of the machine.

## The nine existing limits

The limits in `limits.json` come from in-process library calls on a warm connection. The native library numbers use the same method. End-to-end numbers include process start, so the 2.1.0 command would exceed several limits that way too.

| Operation | 100k limit | native library p95 | 3M limit | native library p95 | Result |
|---|---|---|---|---|---|
| latest value | 5 ms | 0.070 ms | 5 ms | 0.048 ms | pass |
| evidence, 28 d | 10 ms | 0.184 ms | 10 ms | 0.153 ms | pass |
| baseline 90 d | 100 ms | 10.1 ms | 1,000 ms | 90.3 ms | pass |
| workout drift | 50 ms | 1.579 ms | 50 ms | 1.651 ms | pass |
| source comparison | 150 ms | 20.0 ms | 1,500 ms | 177.1 ms | pass |
| export | 20,000 ms | 2,917.3 ms | 900,000 ms | 194.3 s (end to end, round 1, max 279.1 s) | pass |
| check | 15,000 ms | 1,915.2 ms | 600,000 ms | 59.7 s (end to end, round 1, max 84.6 s) | pass |
| initial import | 45,000 ms | 6,724.2 ms | 1,800,000 ms | 406.1 s (end to end, max 411.2 s) | pass |
| incremental import | 100 ms | 4.918 ms | 5,000 ms | 1.583 ms (end to end, max 9.633 ms) | pass |
| peak memory growth | 1 GiB | 30.2 MiB | 6 GiB | 185.0 MiB | pass |

## Working memory as the record count grows (native)

Every row is three runs, and each run uses the same record sizes. Export and check are from round 1, except the 100k row. Import is from round 2.

| Size | Export peak RSS | Check peak RSS | Import peak RSS | Import median | Import peak disk |
|---|---|---|---|---|---|
| 100k | 27.3 MiB | 15.2 MiB | 30.2 MiB | 6.71 s | 184.0 MiB |
| 300k | 30.5 MiB | 14.8 MiB | 46.8 MiB | 20.3 s | 552.0 MiB |
| 1M | 37.9 MiB | 14.8 MiB | 307.7–324.2 MiB | 69.1 s | 1,840.0 MiB |
| 3M | 40.1 MiB | 15.0 MiB | 126.0–185.0 MiB | 408.7 s | 5,520.0 MiB |

- **Export and check use bounded memory.** It does not depend on the record count.
- **Import memory follows the size of the largest series file.** A file holds up to 50,000 samples, a cap first reached at 1M. Import memory grows from 100k to 1M and stops growing after that. In round 1 import peaked at 777–1,422 MiB at 1M and 774–1,625 MiB at 3M.
- **Most of the 1M peak is memory the allocator keeps after it is freed.** The peak comes during the import preview, before anything is written. `vmmap` at that point showed a physical footprint of 285.0 MiB, of which 249.3 MiB was freed large blocks ("MALLOC_LARGE (empty)"). The default malloc zone held 3.3 MB of live allocations. At 3M the same retention was smaller in every run. The peak stays under the target, so the allocator was not changed.
- **Import disk use is about the size of the final database.** In round 1 it was about twice that size, because every page went to the WAL first.

## One very large valid record (round 2)

The test record is one workout session with 200,000 route points and 200,000 heart-rate samples: 25,843,445 bytes of record JSON. It was added to the 2.1.0 demo package. Both versions accept it.

| Operation | old median (3 runs) | native median (3 runs) | old peak RSS | native peak RSS |
|---|---|---|---|---|
| initial import | 4,199.8 ms | 1,609.2 ms | 1,826.5 MiB | 974.7 MiB |
| check | 1,104.8 ms | 657.3 ms | 727.5 MiB | 585.1 MiB |
| export | 1,133.8 ms | 644.1 ms | 983.3 MiB | 355.9 MiB |

The record is a single item of the main package document, not of a series file, so its import holds the whole record at once. Spec section 14 measures it apart from the ordinary-fixture memory target, and it is accepted rather than refused.

## Output equivalence

- **Exports, first run of each version.** At 100k (21 files, both rounds) and 3M (75 files, round 1) the two exports are equal file by file after replacing the version text and the file hashes. Byte counts are equal; "2.1.0" and "2.1.1" have the same length. The 2.1.0 export equals the shared import package.
- **Restored databases (round 2).** After the restore and the one-record merge, both versions have identical hashes of records, revision keys, revision JSON, and typed sample columns. At 100k: 100,001 records, 100,002 revisions, 99,974 samples. At 3M: 3,000,001 records, 3,000,002 revisions, 2,999,974 samples. Database revision 6 at both sizes. The round-2 hashes equal round 1's.
- **Short commands (round 1).** Output is byte-identical for `--help`, latest value, evidence, baseline, workout drift, and source comparison. `--version` and `stats` differ by version text.

## Query plans

No statistics tables exist (no `ANALYZE`), so plans do not depend on size. Native plans were captured from the exact statements each library operation ran, with their first bound values.

| Operation | Native plan |
|---|---|
| latest value | `SEARCH measurement_samples USING INDEX measurement_samples_lookup_idx (quantity=? AND source_id=?)`, keyed searches on `records`, `record_revisions`, and `sources`, then `USE TEMP B-TREE FOR LAST TERM OF ORDER BY` |
| baseline, source comparison, workout drift | A `UNION ALL` of keyed searches on `point_measurements_lookup_idx` and `measurement_samples_lookup_idx` with a time range. No stored JSON is read. |
| evidence | `SEARCH candidate_records USING COVERING INDEX records_record_type_idx (record_type=?)` with a correlated keyed search on the `record_revisions` primary key |
| check | Two ordered passes over the primary-key indexes of `records` and `record_revisions`. Every per-record read is a keyed `SEARCH`. |
| export | Chunk queries use `measurement_samples_lookup_idx` with a time range. `LIMIT` stops early: on the 3M database, 1 row took 0.001 s and 50,000 rows took 0.153 s. |

The 2.1.0 runner's plans:

| Operation | 2.1.0 plan |
|---|---|
| latest value | `SEARCH measurement_samples USING INDEX measurement_samples_lookup_idx (quantity=? AND source_id=?)`, then `USE TEMP B-TREE FOR RIGHT PART OF ORDER BY` |
| baseline | The same, with a time range |
| evidence | `SEARCH candidate_records USING COVERING INDEX records_record_type_idx`, then `USE TEMP B-TREE FOR ORDER BY` |

## Restore into a new database

A restore into a new, empty database runs its one transaction with a rollback journal, then returns the file to WAL mode.

- **When it applies.** The destination has no record, and SQLite can leave WAL mode. SQLite cannot leave WAL mode while another connection has the file open; the restore then runs in WAL mode, as before. A merge into a database that has records always runs in WAL mode.
- **Why.** In WAL mode every new page is written to the WAL, and the checkpoint then reads it back and writes it into the database. A rollback journal saves only the earlier content of pages that already existed. A new database has almost none, so its journal stayed about 350 KB at 3M, and each page is written once.
- **File growth.** During the restore the file grows 8 MiB at a time (`SQLITE_FCNTL_CHUNK_SIZE`). Growing it page by page left the process waiting on the file system. The finished file can be up to one chunk longer than its pages: at 3M, 5,788,139,520 bytes for 5,787,496,448 bytes of pages. The extra bytes are zeros that SQLite does not read. Later writes use them, and `VACUUM` removes them.
- **Atomicity.** The restore is still one transaction, with `synchronous` FULL. A failed restore rolls back and leaves the empty database as it was, in WAL mode. If the process stops during the restore, SQLite rolls the journal back the next time the file is opened. A read-only open then also returns the file to WAL mode; otherwise the next write does.
- **Tests.** The store tests check that a restore runs with a rollback journal and ends in WAL mode; that a failed restore leaves the empty database unchanged and in WAL mode; that a second open connection keeps the restore in WAL mode; that a chunked file passes `integrity_check`, accepts later WAL writes, and reopens; and that a read-only open during a rollback-journal write reports the lock. The package tests check WAL mode and the absence of a journal file after a failed, stopped, or completed restore. One more package test kills a child process during a rollback-journal write that has already written pages to the file; a read-only open then reads the empty database, in WAL mode, with no journal left.
- **A killed restore, checked by hand at 100k.** The restore was stopped with SIGKILL after 4 s. The file had grown to 92,274,688 bytes beside a 350,208-byte journal.
  - With 5c07374, `ath stats` and `ath check` then reported the empty database: revision 0, valid, no records. The file was back to 466,944 bytes in WAL mode. Running the import again restored 100,000 records, and `integrity_check` returned `ok`.
  - Before 5c07374, both read-only commands said "File is not an Athletic Standard database: metadata table is missing" until a command that writes opened the file. With the round-1 WAL restore, they reported the empty database. The fix is described under "Native changes made during this work".
- **A reader during the restore.** A rollback journal lets no other connection read the file while the restore writes. `ath stats` started 20 s into a 1M restore waited 5 s (the busy timeout) and stopped with "database is locked". Under WAL, the same reader would have seen the empty database. Decision D221 accepts this.

### Measured effect

One experiment build had a switch that kept the restore in WAL mode with the same 8 MiB growth. It was run at 3M in alternating order.

| Journal during the restore | Runs | Elapsed | User / system time | Peak disk |
|---|---|---|---|---|
| WAL | 2 | 452.3 s, 490.8 s | 203 / 206–208 s | 11,620,757,312 bytes |
| rollback | 2 | 400.2 s, 401.2 s | 192 / 195–197 s | 5,788,502,040 bytes |

A short manual test overlapped the first WAL run for about 8 s. Earlier pairs on other experiment builds, both with a rollback journal, took 495.7 and 448.5 s without the 8 MiB growth, and 427.3 and 421.2 s with it.

## Native changes made during this work

Each change keeps every check and result, and has its own commit and tests.

| Commit | Rule | Cause | Measured effect |
|---|---|---|---|
| d3cd922 | A package item that its file's schema already checked is decoded without a second check. | Decoding ran the full check again; for revisions that meant the 24-variant union, about 38% of a 100k restore. | 100k import: median 14.57 s before, 9.79 s after (1 run). |
| 728676b | The restore transaction keeps SQLite statement journals in memory. The earlier setting returns afterwards. | Once the journal spilled to a temp file, each inserted row cost several write calls, about 30% of the restore. | 100k import 9.79 → 7.16 s; system time 3.19 → 0.55 s; peak memory unchanged at about 234 MB. |
| 45aef60 | A series file is parsed one item at a time. The history rules run over extracted facts with the same code as the schema. A file with any problem is re-read whole, so issues do not change. | Each 50,000-sample file (about 78 MB of JSON) became a JSON tree of about a gigabyte. | 3M import peak memory 2.40 → 1.33 GB (1 run each). |
| 475f7c8 | Restore vs merge is decided by whether any record exists, not by counting records. | Counting read the whole records index. | One-record merge into a 3M clone: 1.68–1.73 s → 0.04 s (3 runs each). |
| dca08bb | A restore into a new, empty database writes through a rollback journal and grows the file 8 MiB at a time, then returns to WAL mode. | Every page was written twice, to the WAL and then to the database, and file growth page by page left the process waiting. | See "Restore into a new database". |
| bcca0be | The history rule that a current record equals its latest revision compares SHA-256 digests of the two data texts. | Each item's data text was kept until the end of its file. | 32 bytes per item instead of the text. Results are unchanged: equal texts give equal digests. |
| afef521 | A restore reads each series file from disk one item at a time and writes 500 records at a time. Records are read in a first pass over the file and revisions in a second. A file the streamed reading cannot follow, or one with any problem, is read whole, so issues do not change. | The restore read each file whole and wrote it as one batch, so memory followed the size of one file. Two passes write each parent row before the rows that refer to it, so SQLite does not search for waiting child rows as each parent arrives. | 1M import peak memory: 777–1,422 MiB (round 1) → 307.7–324.2 MiB (round 2). 3M: 774–1,625 MiB → 126.0–185.0 MiB. |
| 6e7f459 | The streamed reading finds where a value ends a run of bytes at a time, and applies the nesting limit there. | Each byte was examined twice per pass, and a restore makes three passes over each file. | 100k import: 7.3 → 6.7 s (3 runs each). |
| 5c07374 | A read-only open that meets the journal of a stopped write opens the file for writing once, so SQLite undoes the write, and returns the file to WAL mode. A reader blocked by a restore reports that the database is locked. | A killed restore into a new database left a journal that a read-only connection cannot undo, and `ath stats` and `ath check` then called the file "not an Athletic Standard database". | Behavior only; see "Restore into a new database". |

- **Paired 3M import in round 1, one run each:** the unmodified d3f44bf build took 695.8 s with 2,405 MB peak; the round-1 build took 448.4 s with 1,985 MB peak.
- **Not kept: WAL mode with the 8 MiB file growth.** It took 452.3 and 490.8 s against 400.2 and 401.2 s with a rollback journal, and used twice the disk.
- **Not kept: a larger page cache for the restore.**
  - In WAL mode, 64 MiB took 508 s; 256 MiB took 443.0 s against 421.7 s for the default in one A/B pair. System time stayed at about 196 s.
  - With a rollback journal, 128 MiB took 492.2 s and 64 MiB 502.9 s (one run each, without the 8 MiB file growth).

## Maintainability exercises

Both changes were committed, measured, and reverted. Neither is shipped. Paths are as they were before the cutover moved `native/src/` to `src/`.

### (a) An invented optional field (de6bc45, reverted by d7136ba)

The exercise added `exercise_mood_label` to athlete reports.

- **Modules touched:**
  - `model/data.rs`: one line declares the field.
  - `log.rs` and `import/whoop.rs`: one line each. They build an `AthleteReportData` with a Rust struct literal, which must name every field.
  - `tests/exercise_optional_field.rs`: new.
- **Not touched:** the encoder, SQL, package code, check, and CLI. The field travels in `record_json` and the typed row's `data_json`.
- **Round trip:** the new test writes a report with the field, checks the database, exports it, restores it into a new database, and checks again. The field survives every step.
- **Generated contract:** the JSON Schema generated from the model (`model::json_schema::document`) gained only the new field, in the athlete-report current-record and revision shapes.
- **Tests:** the comparison against the published 2.1.0 schema failed, as it should for a format change. All other 576 tests passed.
- **Note:** the generation test (`tests/generated.rs`) that the cutover added now regenerates every generated file in one run.

### (b) An invented provider reader (2c7bbb1, reverted by a62f230)

"Pulse Band" reads a CSV of daily resting heart rate and HRV as point measurements through the shared import operation.

- **Modules touched:**
  - `import/pulseband.rs`: 67 lines, new.
  - `import/detect.rs`: 8 lines (a format, three names, and a folder marker).
  - `import/provider.rs`: one line registers the reader.
  - `import/mod.rs`: one line.
  - `tests/import_pulseband.rs`: 49 lines.
- **No SQL writer and no command code.** Nothing in `store/` or `cli/` changed.
- **End to end:** `ath import <folder>` showed the preview, and `--yes` imported 5 new records. A second import matched all 5. `ath check` reported the database valid. All 577 tests passed.

## Limits of this report

- **Two builds.** Rows that only round 1 measured come from the round-1 executable. The later commits do not change those code paths, and the 100k export and check agree across rounds.
- **The 2.1.0 check could not finish at 3M** under its published configuration. The check verdict uses its time to failure and one larger-heap run.
- **Shared machine, uncleared OS cache.** Results vary by run: in round 1 the 3M export spread was 166.5 s for native and 181.7 s for 2.1.0.
- **3M library-level results** are one run each. The 2.1.0 3M library run is from round 1.
- **The measured round-2 executable is 6e7f459.** The later commit 5c07374 changes only what a read-only open does after an error: it finishes a stopped rollback-journal write, and it reports a locked file as locked. No measured command takes those paths. Two 100k imports with each build took 6.71 and 6.67 s (6e7f459) and 6.66 and 6.67 s (5c07374).
- **One machine.** Every number comes from one macOS arm64 machine. The speed workflow on GitHub, run by hand, runs the 100k comparison on a hosted macOS runner, and the 3M comparison on a self-hosted runner when one is set up (D225); see [README.md](README.md).
- **Readers during a restore into a new database** wait up to 5 s and then report that the database is locked. Under WAL they read the empty database.
