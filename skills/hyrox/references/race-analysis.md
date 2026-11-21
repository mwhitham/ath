# Race analysis

Read this file when the athlete has a race or simulation result and asks where the
time went. The aim is to divide the finish time into runs, stations, Roxzone, and
penalties without counting any time twice, and to say what the numbers can and
cannot show.

The full post-race review, with competing explanations and tests, is in
[post-race-review.md](post-race-review.md).

## Step 1. Get the result and its split layout

Ask the athlete for:

1. The official finish time.
2. Every split the timing provider shows, copied exactly, with its name.
3. Any penalties listed, and whether 48 hours have passed. "Penalties may be added
   or amendments made for up to 48 hours after each event ends." (Singles 26/27
   p. 10)
4. Format, division, category, age group, and the event (race or simulation).

The 2026/27 rulebooks do not define where the Roxzone starts or ends, and they list
no official split types (see [rules-2026-27.md](rules-2026-27.md#roxzone-and-timing)).
Split fields come from the timing provider. Layouts differ between providers and
venues. Identify the layout before you map anything:

| Layout | What the provider shows | How to label it |
|---|---|---|
| A | 8 runs, 8 stations, and one Roxzone total | `run_1`…`run_8`, the 8 station labels, one `roxzone` |
| B | 8 runs, 8 stations, and separate in and out transitions | the runs, the stations, `roxzone_1`…`roxzone_16` in order |
| C | 8 runs and 8 stations only | the runs and the stations; no Roxzone segment |
| D | Only some splits, for example a partial simulation | only the segments you have |

Rules for mapping:

- Copy the provider's numbers. Do not move time between parts to "fix" them.
- If a provider's run time already includes Roxzone time, do not add a Roxzone
  segment too. That would count it twice. Say in `split_layout` that runs include
  Roxzone.
- If you are not sure what a provider's field contains, ask the athlete or label
  the layout as uncertain. Do not guess.
- Write the layout in the extension's `split_layout` key, in plain words.
- Penalties go in `penalties_s` or one `penalty` segment, not both.

Then log the result as shown in [records.md](records.md#a-complete-race-result).

## Step 2. Run the calculation

```sh
ath apply hyrox-race-splits --record <benchmark-result-id> --json
```

The output has `result`, `missing`, and sometimes `exclusions`.

### If `result` is null

The calculation refused the record. `missing` says why. Fix the record with
`ath correct` after the athlete approves, then run it again.

| Message in `missing` | What it means | What to do |
|---|---|---|
| a segment "is not a HYROX split label" | An unknown label | Rename it to a label in [records.md](records.md#segment-labels) |
| a segment "appears twice" | A duplicate label | Keep one; check the source for a typing error |
| "either one roxzone total or numbered roxzone_N transitions" | Both Roxzone forms | Keep the one the provider gave |
| "penalties as a segment or as penalties_s" | Both penalty forms | Keep one |
| "parts that add up to no more than the finish time" | The parts exceed the finish time | Some time is counted twice, or a split was typed wrong. Recheck the source |
| "score.duration_s" | No finish time, or a segment without a time | Add the missing time |
| "segments labelled run_1 to run_8…" | No segments | Ask the athlete for the splits |

### Reading `result`

| Field | Meaning |
|---|---|
| `total_seconds` | The official finish time. |
| `runs_seconds` | All recorded runs added. |
| `stations_seconds` | All recorded stations added. |
| `roxzone_seconds` | The Roxzone total, or the transitions added. |
| `penalties_seconds` | Penalty time from the segment or the extension. |
| `unaccounted_seconds` | Finish time not covered by any part. |
| `share_percent.runs`, `.stations`, `.roxzone`, `.penalties`, `.unaccounted` | Each part as a percentage of the finish time, to 0.1. |
| `runs` | Each run, in order: `label` and `seconds`. |
| `run_summary.count` | How many runs were recorded. |
| `run_summary.mean_seconds` | The mean run. |
| `run_summary.fastest`, `.slowest` | The fastest and slowest runs, with labels. |
| `run_summary.spread_seconds` | Slowest minus fastest. |
| `run_summary.first_run_seconds` | Run 1, which follows no station. |
| `run_summary.later_runs_mean_seconds` | The mean of runs 2–8, each of which follows a station. |
| `stations` | Each station in race order: `label`, `seconds`, and `share_percent` of the finish time. |
| `season`, `format`, `division` | Copied from the extension. |

`exclusions` lists what was not there, such as "4 of 8 runs recorded" or "no
Roxzone time recorded; it is part of the unaccounted time".

### Reading `unaccounted_seconds`

- A few seconds: most likely rounding in the provider's splits. Say "most likely",
  because the record cannot prove it.
- Many seconds with no Roxzone segment: most likely untimed Roxzone and transitions.
  Do not call it Roxzone time unless the provider said so.
- Many seconds with a Roxzone segment: ask what else the provider left out, for
  example time at the start or in a penalty box.

## Step 3. Describe what the numbers show

Lead with observed facts, then evidence, then hypotheses. Keep them visibly apart.

**Observed** (from this result only):

- The finish time and the share of runs, stations, Roxzone, and penalties.
- Run 1 against the mean of runs 2–8, and the slowest run.
- The station with the largest share, and the next two.
- Any penalties, and whether the 48-hour window has passed.

**Evidence** (label its strength, see [evidence.md](evidence.md)):

- Run speed usually falls across the race. Run 8 slows most (Research).
- Slower athletes lose relatively more time at strength stations (Research).
- Fade size was not clearly linked to finish time among top athletes (Research).

**Hypothesis** (for this athlete): name more than one explanation for anything that
looks weak. The post-race review gives the full list and a test for each.

## Comparing results

Compare like with like:

| Compare | Allowed? | Note |
|---|---|---|
| Two races, same format, division, and split layout | Yes | Venues still differ: floor friction, lap layout, Roxzone length |
| Two races, different venues | With care | Compare machine and bodyweight stations before runs and Roxzone, which depend more on the venue. Sled times also depend on the venue's floor friction (see [stations.md](stations.md#2-sled-push-4--125-m)) |
| A race and a simulation | Segment by segment only | Name every substitution; never compare totals as the same test |
| Doubles or Relay with Singles | No | Team splits include shared or other members' work |
| Different divisions | No | Loads differ |
| The athlete with population percentiles | Only to locate a weak segment | The large tables stop at 2023/24 (Fernández-Navarrete et al. 2026). Unverified inference: they may now flatter, because times kept falling |
| Different seasons | With care | Check whether any load or standard changed |

One race is one observation. Do not call a trend from one race.

## What a good and a bad answer look like

**Bad:** "Your Roxzone was 8 minutes and your runs were 44 minutes, so you spent
52 minutes running." This adds two parts that may overlap and calls Roxzone
running.

**Bad:** "You were slow on wall balls compared with the average athlete, so do more
wall balls." This uses a population average as the only evidence and gives one
explanation.

**Good:** "Observed: your runs took 44:11 (45.6% of 1:36:49), stations 44:45
(46.2%), and the Roxzone total 7:50 (8.1%). The provider's layout gave one Roxzone
total, and 3 seconds are unaccounted, most likely rounding. Run 1 took 4:10; runs 2–8
averaged 5:43. Wall balls took 11:30, the largest single part at 11.9%. Evidence:
some slowing after run 1 is normal in HYROX races (Research). Hypothesis: the gap
here is large enough that run 1 pacing is one of several possible causes; the review
names the others and how to test them."
