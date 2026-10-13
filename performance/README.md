# Release performance proof

Large databases are generated locally or in the release workflow. They are not
committed.

Generate one required size:

```sh
pnpm performance:generate -- --records 100000 \
  --output performance/generated/100k
```

Run all nine release measurements:

```sh
pnpm performance:run -- \
  --database performance/generated/100k/athlete.ath.sqlite \
  --output performance/results/local/100k.json \
  --measured-at 2026-10-13T06:00:00.000Z \
  --repetitions 3
```

The required sizes are 100,000, 3 million, and 30 million current records. The
generator uses `ath-v1-performance-seed-2`. Each database includes three sources
that record the same quantities, dense all-day and workout samples, summaries,
scores, sessions, corrections, deletions, athlete reports, nutrition, plans, and
predictions.

The runner measures:

- Initial package import.
- Incremental package import.
- Latest-value lookup.
- A 28-day evidence request.
- A 90-day baseline.
- A within-workout calculation.
- A two-source comparison.
- Complete JSON export.
- A complete database check.

Reports include database counts and sizes, hardware, operating system, Node and
libSQL versions, SQLite settings, repetitions, percentiles, peak process memory
growth, and query plans.

Read measurements use one open connection after one discarded warmup. This does not
clear SQLite's page cache or the operating system's file cache. Import, export, and
database check run once because each processes the complete fixture.

`limits.json` is filled only from a successful measured run on the named reference
runner. It is not a claim about other hardware.
