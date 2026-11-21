# Reading and saving records

Read this file before you read the athlete's data, log a race or simulation, or save
a plan, review, note, or goal. The general logging rules are in the
`athletic-standard` tool skill (`references/logging.md`). This file adds what HYROX
needs.

## Reading the athlete's data

```sh
ath stats --json                                                    # counts, sources, coverage
ath records list --type benchmark_result --limit 10000 --json       # races, simulations, run tests
ath records list --type benchmark_definition --limit 10000 --json   # definition IDs and keys
ath records list --type workout_session --limit 10000 --json        # training sessions
ath records list --type athlete_report --limit 10000 --json         # effort ratings, notes, soreness
ath records list --type training_plan --limit 10000 --json          # also planned_session, goal,
                                                                    # coaching_note, saved_training_review
ath records show <record-id> --history --links --json               # one record, its revisions and links
ath apply hyrox-race-splits --record <benchmark-result-id> --json
ath apply training-load-effort --record <athlete-report-id> --json
ath calculate summary <quantity> --group week --timezone <zone> --json
```

`ath records list` returns records oldest first and only 100 by default, so always
pass `--limit 10000` (the largest allowed). Without it, recent records can be
missing.

Write down the `record_id` and `current_revision` of every record you rely on. They
become the `evidence` list of anything you save.

Keep the two kinds of input apart:

- **Measured records** have a `source_id`. A race result the athlete types in uses
  the database's `manual` source. It is still a measured record, because the timing
  system measured it.
- **Athlete reports** have no source. What the athlete felt or said (an effort
  rating, "my legs were gone", "I got no-reps") is an athlete report. Never put it in
  a measured record or claim a source for it.

## Benchmark definitions for HYROX

Use one benchmark definition per format, division, and category, so that only like
races are compared and graded together. The `athletic-standard` logging guide asks
for the same thing.

| Event | Suggested `benchmark_key` |
|---|---|
| Singles, Women (Open) | `hyrox-singles-open-women` |
| Singles, Men Pro | `hyrox-singles-pro-men` |
| Doubles Mixed | `hyrox-doubles-open-mixed` |
| Relay Women | `hyrox-relay-women` |
| Adaptive (one confirmed division) | `hyrox-adaptive-<division>-<category>`, for example `hyrox-adaptive-lower-limb-minor-men` |
| A full simulation with gym equipment | `hyrox-sim-<place>`, for example `hyrox-sim-home-gym` |
| A half simulation | `hyrox-half-sim-<place>-<runs>`, for example `hyrox-half-sim-home-gym-runs-5-8` |

- Check `ath records list --type benchmark_definition --limit 10000 --json` first. Reuse a
  definition that already matches.
- Write the definition text from the 2026/27 rulebook loads, and name the rulebook
  and page.
- If a later season changes a load, distance, or rep count, create a new
  definition. If nothing changed, keep the definition and record the season in each
  result's extension.
- The seeded `hyrox-full` definition names the course but not the division, and its
  text says "wall balls 100/75 reps". That is not the 2026/27 rule: every Singles
  division does 100 reps. Do not use its text as a rule. If older results were
  logged under `hyrox-full`, leave them; read their extension for the division.
- A simulation with substitutions is a different test from a race. Give it its own
  definition and `"event": "simulation"`. Never compare its total with a race total
  as if they were the same test.
- A half simulation is a different test from a full one. Log it under its own
  definition that names the runs and stations it covered, so half and full times are
  never mixed.

## The `org.athleticstandard/hyrox-race` extension

Sport detail goes in this extension on the `benchmark_result`, not in new fields
(decisions D118 and D245).

| Key | Meaning | Values |
|---|---|---|
| `season` | The rulebook season | `"2026/27"` |
| `rulebook` | The rulebook that applied | `"HYROX Rulebook Singles 26/27"`, `"HYROX Rulebook Doubles 26/27"`, `"HYROX Rulebook Relay 26/27"`, `"HYROX Rulebook Adaptive 26/27"` |
| `format` | Race format | `singles`, `doubles`, `relay` |
| `division` | Division | `open`, `pro`, `adaptive` |
| `category` | Category | `women`, `men`, `mixed` |
| `event` | A real race or a simulation | `race`, `simulation` |
| `age_group` | Age group on the results | for example `"30-34"` |
| `penalties_s` | Total time penalties, in seconds | `0` when none |
| `adaptive_category` | Only for Adaptive: the division the athlete says HYROX confirmed | for example `"Lower Limb Minor, confirmed by HYROX"` |
| `split_layout` | Where the splits came from and how they are divided | for example `"official results page: 8 runs, 8 stations, one Roxzone total"` |

`season`, `format`, `division`, and `event` are required by convention. The others
are optional. `split_layout` is a proposal of this skill: the 2026/27 rulebooks do
not define Roxzone timing or official splits, so a result must say which layout it
came from.

## Segment labels

Each segment has a `label` and a `score` with `duration_s` in seconds.

| Part | Labels |
|---|---|
| Runs | `run_1` … `run_8` |
| Stations, in race order | `ski_erg`, `sled_push`, `sled_pull`, `burpee_broad_jumps`, `row`, `farmers_carry`, `sandbag_lunges`, `wall_balls` |
| Roxzone | one `roxzone` total, or numbered `roxzone_1` … `roxzone_16`, never both |
| Penalties | an optional `penalty` segment, or `penalties_s` in the extension, never both |

- Use each label once.
- A duration must be more than zero. Leave a segment out when there is no time for
  it (for example no penalties, or no Roxzone split). Do not write a zero.
- `score.duration_s` on the result is the official finish time, which includes time
  penalties.

## A complete race result

The invented Athlete B, a man aged 34, raced Singles Men (Open) on 24 October 2026.
He copied his splits from the official results page, which showed eight runs, eight
stations, and one Roxzone total. No penalties were listed 48 hours after the event.
This batch creates the benchmark definition and the result in one transaction. The
definition's evidence list is empty because it was written from the rulebook, not
from the athlete's records.

```json ath-record-example
[
  {
    "record_type": "benchmark_definition",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b01",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "agent-harness", "model": "model-id" },
        "authored_at": "2026-10-26T19:00:00.000Z",
        "evidence": [],
        "content_version": 1,
        "status": "active"
      },
      "benchmark_key": "hyrox-singles-open-men",
      "benchmark_type": "hyrox",
      "score_type": "time",
      "definition": "HYROX Singles, Men (Open), from the HYROX Rulebook Singles 26/27, p. 8: 8 x (1 km run + station), in this order: SkiErg 1000 m; Sled Push 4 x 12.5 m at 152 kg including the sled; Sled Pull 4 x 12.5 m at 103 kg including the sled; Burpee Broad Jumps 80 m; Rowing 1000 m; Farmers Carry 200 m with 2 x 24 kg; Sandbag Lunges 100 m with 20 kg; Wall Balls 100 reps with 6 kg to a 3.00 m target. Score: official finish time, including time penalties.",
      "tags": ["hyrox", "singles", "open", "men", "2026/27"]
    }
  },
  {
    "record_type": "benchmark_result",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b02",
    "data": {
      "source_id": "01890f3e-e7d3-7cc0-88c8-000000000001",
      "received_at": "2026-10-26T19:00:00.000Z",
      "received_at_basis": "local_write_time",
      "benchmark_definition_id": "01890f3e-e7d3-7cc0-88c8-000000000b01",
      "recorded_at": "2026-10-24T10:40:00.000Z",
      "input_timestamp": {
        "timestamp_text": "2026-10-24T10:40:00.000Z",
        "utc_offset_text": "Z",
        "utc_offset_meaning": "documented_recording_offset",
        "timestamp_basis": "source_instant"
      },
      "score": { "duration_s": 5809 },
      "input_score_text": "1:36:49",
      "segments": [
        { "label": "run_1", "score": { "duration_s": 250 } },
        { "label": "ski_erg", "score": { "duration_s": 285 } },
        { "label": "run_2", "score": { "duration_s": 318 } },
        { "label": "sled_push", "score": { "duration_s": 225 } },
        { "label": "run_3", "score": { "duration_s": 341 } },
        { "label": "sled_pull", "score": { "duration_s": 330 } },
        { "label": "run_4", "score": { "duration_s": 332 } },
        { "label": "burpee_broad_jumps", "score": { "duration_s": 380 } },
        { "label": "run_5", "score": { "duration_s": 352 } },
        { "label": "row", "score": { "duration_s": 300 } },
        { "label": "run_6", "score": { "duration_s": 330 } },
        { "label": "farmers_carry", "score": { "duration_s": 140 } },
        { "label": "run_7", "score": { "duration_s": 356 } },
        { "label": "sandbag_lunges", "score": { "duration_s": 335 } },
        { "label": "run_8", "score": { "duration_s": 372 } },
        { "label": "wall_balls", "score": { "duration_s": 690 } },
        { "label": "roxzone", "score": { "duration_s": 470 } }
      ]
    },
    "extensions": {
      "org.athleticstandard/hyrox-race": {
        "season": "2026/27",
        "rulebook": "HYROX Rulebook Singles 26/27",
        "format": "singles",
        "division": "open",
        "category": "men",
        "event": "race",
        "age_group": "30-34",
        "penalties_s": 0,
        "split_layout": "official results page, copied by the athlete: 8 runs, 8 stations, one Roxzone total"
      }
    }
  }
]
```

How to log it:

1. Find the manual source ID with `ath records list --type source --limit 10000 --json`. Use it as
   `source_id`. The ID above is a placeholder.
2. Preview: `ath log --json` with the batch on standard input. Show the athlete
   every record in the preview.
3. After the athlete approves, run `ath log --yes` with the same input.
4. If the watch recorded the race as a workout, link them:
   `ath link <result-id> <session-id> --revision <n> --json`, then `--yes` after
   approval.
5. Run `ath apply hyrox-race-splits --record <result-id> --json`. See
   [race-analysis.md](race-analysis.md).

If a split was typed wrong, correct the record with `ath correct`. Do not log a
second result.

## What the athlete said about the race

Effort and comments are athlete reports. They have no source.

```json ath-record-example
[
  {
    "record_type": "athlete_report",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b03",
    "data": {
      "report_type": "note",
      "reported_at": "2026-10-24T14:30:00.000Z",
      "input_reported_at": {
        "timestamp_text": "2026-10-24T14:30:00.000Z",
        "utc_offset_text": "Z",
        "utc_offset_meaning": "documented_recording_offset",
        "timestamp_basis": "source_instant"
      },
      "reported_via": "text",
      "note": "Run 1 felt easy. Legs were gone after the lunges. At wall balls the judge called several no-reps for depth and I lost count."
    }
  },
  {
    "record_type": "athlete_report",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b06",
    "data": {
      "report_type": "session_effort",
      "reported_at": "2026-10-24T13:00:00.000Z",
      "input_reported_at": {
        "timestamp_text": "2026-10-24T13:00:00.000Z",
        "utc_offset_text": "Z",
        "utc_offset_meaning": "documented_recording_offset",
        "timestamp_basis": "source_instant"
      },
      "reported_via": "text",
      "rating": 10,
      "rating_scale": "0-10",
      "duration_seconds": 5809,
      "workout_session_id": "01890f3e-e7d3-7cc0-88c8-000000000b05"
    }
  }
]
```

## A goal

A goal names its target, the definition it is measured against, and the evidence
behind it. A time target is a hypothesis. Set it from the athlete's own races or
simulations, never from population averages alone.

```json ath-record-example
[
  {
    "record_type": "goal",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b12",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "agent-harness", "model": "model-id" },
        "authored_at": "2026-10-27T18:00:00.000Z",
        "evidence": [
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b02", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b10", "revision": 1 }
        ],
        "content_version": 1,
        "status": "active"
      },
      "title": "HYROX Singles Men (Open) under 1:32:00",
      "description": "Hypothesis: about 5 minutes can come from a steadier start, faster wall balls, and faster runs 6-8, based on the 24 October 2026 race (1:36:49) and its review. Re-check this target after the 5 December simulation.",
      "target_date": "2027-03-20",
      "target_benchmark_definition_id": "01890f3e-e7d3-7cc0-88c8-000000000b01",
      "target_score": { "duration_s": 5520 }
    }
  }
]
```

To have a finish-time estimate graded automatically when the result is logged, save
it as a `prediction` with the tool skill's `references/predicting.md`.

## Plans, sessions, reviews, and notes

- `training_plan`, `planned_session`, and a `goal`: see the 12-week worked example in
  [race-preparation.md](race-preparation.md).
- `coaching_note` for a race budget: see [pacing.md](pacing.md).
- `saved_training_review` and `coaching_note` after a race: see the worked example
  in [post-race-review.md](post-race-review.md).

Rules for every authored record:

- `authorship.author` names the agent and model, or `{ "author_type": "athlete" }`
  when the athlete wrote it.
- `evidence` lists the exact record IDs and revisions you used. Leave it empty only
  when no record informed the advice, and say so in the text.
- Use `"status": "draft"` while the athlete is still deciding, `"active"` once they
  accept it, and `"superseded"` when a newer version replaces it.
- In the text, keep **Observed**, **Evidence**, and **Hypothesis** apart, and name
  the season and rulebook behind any rule.
- Always preview with `ath log --json`, show the athlete, and write with
  `ath log --yes` only after they approve.
