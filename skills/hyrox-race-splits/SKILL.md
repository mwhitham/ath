---
name: hyrox-race-splits
description: Divides one HYROX race or simulation result into its eight runs, eight stations, Roxzone, and penalties, refusing any layout that would count time twice, and summarises run pacing and each station's share of the finish time. Use when someone asks where their HYROX time went, how their runs held up after stations, which station cost the most, or how much time Roxzone took. Needs a benchmark result with labelled segments. Not for training plans or race strategy (use the hyrox coaching skill).
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.1.2 database. Runs with ath apply hyrox-race-splits --record <benchmark-result-id>.
metadata:
  version: "2.1.2"
  kind: calculation
---

# HYROX race splits

A HYROX race is eight runs of about 1 km, each followed by one of eight stations.
This calculation adds a recorded result's parts and checks that they fit inside the
finish time.

## Run it

```sh
ath records list --type benchmark_result --limit 10000 --json
ath apply hyrox-race-splits --record <benchmark-result-id> --json
```

## Log the result so it can be split

A result is a `benchmark_result` against a benchmark definition for its format,
division, and category, such as `hyrox-singles-open-women`, so that only comparable
races share a definition. Its `score.duration_s` is the finish time. Each of its
`segments` has a label from the table below and a `score.duration_s`:

| Part | Labels |
|---|---|
| Runs | `run_1` … `run_8` |
| Stations, in race order | `ski_erg`, `sled_push`, `sled_pull`, `burpee_broad_jumps`, `row`, `farmers_carry`, `sandbag_lunges`, `wall_balls` |
| Roxzone | one `roxzone` total, or numbered `roxzone_1` … `roxzone_16` transitions |
| Penalties | an optional `penalty` segment |

The `org.athleticstandard/hyrox-race` extension on the record names the season,
rulebook, format, division, category, whether it was a race or a simulation, and
any `penalties_s` (D245). The `hyrox` coaching skill shows a complete record.

The 2026/27 rulebooks do not define where Roxzone timing starts or ends, and timing
providers split it differently. Record the split layout you were given, for example
in the extension's `split_layout` text, and say which it is.

## Method

Operation `hyrox_splits` (D245).

1. Every segment label must be one of the labels above, used once. An unknown label,
   a repeated label, or both a `roxzone` total and numbered transitions stops the
   calculation, because time would be counted twice or not at all.
2. Penalties come from a `penalty` segment or the extension's `penalties_s`, not
   both.
3. Runs, stations, Roxzone, and penalties are added. If they add up to more than
   the finish time, beyond half a second for each part plus half a second for
   rounding, the calculation stops: something is counted twice.
4. `unaccounted_seconds` is the finish time not covered by any part, such as
   Roxzone time that was not recorded.
5. Shares are percentages of the finish time, to 0.1.
6. `run_summary` gives the mean run, the fastest and slowest runs, their spread,
   run 1 (which follows no station), and the mean of runs 2–8 (each follows a
   station).

## Reading the result

| Field | Meaning |
|---|---|
| `total_seconds` | The finish time. |
| `runs_seconds`, `stations_seconds`, `roxzone_seconds`, `penalties_seconds`, `unaccounted_seconds` | The parts. |
| `share_percent` | Each part's share of the finish time. |
| `runs`, `stations` | Each split, in order; stations with their share. |
| `run_summary` | Run pacing. |
| `season`, `format`, `division` | From the extension. |

How to describe it:

- Lead with the largest controllable parts. Running is usually about half of the
  race, so a few seconds per kilometre across eight runs adds up.
- `later_runs_mean_seconds` well above `first_run_seconds` means the runs slowed
  after stations. Several explanations fit: starting too fast, station fatigue,
  or limited running fitness. Name them; the splits alone cannot choose.
- Compare a station with the same athlete's earlier races in the same format and
  division only. Loads differ between divisions, and Doubles splits are team
  splits.
- A run "km" can be laid out as laps of uneven length at a venue. Compare run
  splits within one race more safely than across venues.

## Limits

- The calculation trusts the splits it is given. A timing mat in an unexpected
  place moves time between a run and Roxzone.
- It reads one result. Use the coaching skill to compare races.

## Worked example

A women's Open Singles result of 4 800 s (1:20:00). Runs from 285 s to 320 s add to
2 420 s (50.4 %); stations add to 1 950 s; Roxzone is 420 s; 10 s are unaccounted.
Run 1 took 285 s and runs 2–8 averaged 305 s: about 20 s slower per run after
stations. Wall balls took 330 s, 6.9 % of the race.

```json ath-example
{
  "records": [
    { "id": "race", "record_type": "benchmark_result", "benchmark": "hyrox-singles-open-women", "benchmark_type": "hyrox",
      "at": "2026-11-01T10:00:00.000Z", "score": { "duration_s": 4800 },
      "segments": [
        { "label": "run_1", "score": { "duration_s": 285 } }, { "label": "ski_erg", "score": { "duration_s": 270 } },
        { "label": "run_2", "score": { "duration_s": 290 } }, { "label": "sled_push", "score": { "duration_s": 180 } },
        { "label": "run_3", "score": { "duration_s": 295 } }, { "label": "sled_pull", "score": { "duration_s": 240 } },
        { "label": "run_4", "score": { "duration_s": 300 } }, { "label": "burpee_broad_jumps", "score": { "duration_s": 270 } },
        { "label": "run_5", "score": { "duration_s": 305 } }, { "label": "row", "score": { "duration_s": 280 } },
        { "label": "run_6", "score": { "duration_s": 310 } }, { "label": "farmers_carry", "score": { "duration_s": 120 } },
        { "label": "run_7", "score": { "duration_s": 315 } }, { "label": "sandbag_lunges", "score": { "duration_s": 260 } },
        { "label": "run_8", "score": { "duration_s": 320 } }, { "label": "wall_balls", "score": { "duration_s": 330 } },
        { "label": "roxzone", "score": { "duration_s": 420 } }],
      "extensions": { "org.athleticstandard/hyrox-race": {
        "season": "2026/27", "rulebook": "HYROX Rulebook Singles 26/27", "format": "singles", "division": "open",
        "category": "women", "event": "race", "penalties_s": 0 } } }
  ],
  "apply": "race",
  "result": {
    "total_seconds": 4800, "runs_seconds": 2420, "stations_seconds": 1950, "roxzone_seconds": 420,
    "penalties_seconds": 0, "unaccounted_seconds": 10,
    "share_percent": { "runs": 50.4, "stations": 40.6, "roxzone": 8.8, "penalties": 0, "unaccounted": 0.2 },
    "runs": [
      { "label": "run_1", "seconds": 285 }, { "label": "run_2", "seconds": 290 }, { "label": "run_3", "seconds": 295 },
      { "label": "run_4", "seconds": 300 }, { "label": "run_5", "seconds": 305 }, { "label": "run_6", "seconds": 310 },
      { "label": "run_7", "seconds": 315 }, { "label": "run_8", "seconds": 320 }],
    "run_summary": { "count": 8, "mean_seconds": 302.5, "fastest": { "label": "run_1", "seconds": 285 },
      "slowest": { "label": "run_8", "seconds": 320 }, "spread_seconds": 35, "first_run_seconds": 285, "later_runs_mean_seconds": 305 },
    "stations": [
      { "label": "ski_erg", "seconds": 270, "share_percent": 5.6 }, { "label": "sled_push", "seconds": 180, "share_percent": 3.8 },
      { "label": "sled_pull", "seconds": 240, "share_percent": 5 }, { "label": "burpee_broad_jumps", "seconds": 270, "share_percent": 5.6 },
      { "label": "row", "seconds": 280, "share_percent": 5.8 }, { "label": "farmers_carry", "seconds": 120, "share_percent": 2.5 },
      { "label": "sandbag_lunges", "seconds": 260, "share_percent": 5.4 }, { "label": "wall_balls", "seconds": 330, "share_percent": 6.9 }],
    "season": "2026/27", "format": "singles", "division": "open"
  }
}
```

A layout that records both a Roxzone total and a transition is refused:

```json ath-example
{
  "records": [
    { "id": "race", "record_type": "benchmark_result", "benchmark": "hyrox-singles-open-women", "benchmark_type": "hyrox",
      "at": "2026-11-01T10:00:00.000Z", "score": { "duration_s": 4800 },
      "segments": [
        { "label": "run_1", "score": { "duration_s": 285 } },
        { "label": "roxzone", "score": { "duration_s": 420 } },
        { "label": "roxzone_1", "score": { "duration_s": 30 } }] }
  ],
  "apply": "race",
  "result": null,
  "missing": ["either one roxzone total or numbered roxzone_N transitions; both would count Roxzone twice"]
}
```

## Related

The `hyrox` coaching skill plans training and reviews races from these splits.
`heart-rate-zones` and `training-load-heart-rate` read the race's workout record.

```ath-calculation
name: hyrox-race-splits
use: hyrox_splits
```
