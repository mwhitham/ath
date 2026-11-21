# Reading and saving records

This file shows how the CrossFit skill reads the record and how it saves goals,
plans, notes, reviews, and benchmark results. The `athletic-standard` tool skill
covers every other command.

Every ID in these examples is invented. Replace each one with a real ID from
`ath records list`. Replace `example-agent` and `example-model` with your own agent
and model names.

## Reading

```sh
ath stats --json
ath records list --type <record-type> --limit 10000 --json
ath records show <record-id> --history --links --json
```

`ath records list` returns records oldest first, and only 100 by default. Always
pass `--limit 10000`, or the most recent records can be missing.

Record types this skill reads most: `workout_session`, `benchmark_result`,
`benchmark_definition`, `athlete_report`, `goal`, `training_plan`,
`planned_session`, `coaching_note`, `saved_training_review`, `point_measurement`
(nightly RMSSD and resting heart rate), `measurement_sample` (Apple Watch SDNN),
and `sleep_session`. Use whichever HRV statistic the device records, `hrv_rmssd` or
`hrv_sdnn`, and never both in one result.

Evidence names a record and an exact revision. Read the revision from
`current_revision` in `ath records show`.

## The three classes of record

| Class | Examples | Has a source? | Who writes it |
|---|---|---|---|
| Measured | `workout_session`, `benchmark_result`, `point_measurement`, `sleep_session` | Yes. A typed result uses the `manual` source. | A device import, or the athlete through `ath log` |
| Athlete report | `athlete_report` (`session_effort`, `soreness`, `note`) | Never | The athlete, in their own words |
| Authored | `goal`, `training_plan`, `planned_session`, `coaching_note`, `saved_training_review`, `benchmark_definition` | No. It names an author and evidence. | You, the coach, or the athlete |

Coaching advice is always authored. Never write advice as a measurement or as an
athlete report. Never put an athlete's rating into an authored record as if it were
yours.

## Approval before every write

1. Build the JSON: one object, or an array for a batch.
2. Preview: `ath log --json` with the JSON on standard input. `--json` never writes.
3. Show the athlete the preview: what will be saved and why.
4. Wait for a clear yes.
5. Write: `ath log --yes` with the same input.

A batch is written in one transaction. One invalid item means nothing is written.
In a batch, give new records a `record_id` so other items can point at them, such
as a planned session at its plan.

To change an existing record:

```sh
ath records show <record-id> --history --json
ath correct <record-id> --revision <current-revision> --data '<complete data object>' --json
ath correct <record-id> --revision <current-revision> --data '<complete data object>' --yes
```

`--data` replaces the complete data object. Copy the old data, change what you
need, raise `authorship.content_version` by one, update `authored_at`, and update
`evidence`.

## Authorship

```json
"authorship": {
  "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" },
  "authored_at": "2026-11-15T20:00:00.000Z",
  "evidence": [ { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000201", "revision": 1 } ],
  "content_version": 1,
  "status": "active"
}
```

- `author`: an agent with `agent` and `model`, or `{ "author_type": "athlete" }`
  when the athlete wrote it. `agent_version` must be a version number such as
  `1.0.0` if given. `model_version` is optional text.
- `evidence`: every record the advice relied on, with its exact revision. Empty only
  when nothing in the record informed it. Then say so in the text.
- `content_version`: 1 when created. Add one on each change.
- `status`:

| Status | Use it when |
|---|---|
| `draft` | Proposed and not yet agreed. |
| `active` | Agreed and in use. |
| `completed` | A plan or goal that has run its course. |
| `superseded` | Replaced by a newer record. |
| `cancelled` | Dropped before it ran. |

## Goals and a custom benchmark

A goal that is a benchmark names the definition and the target score. A score is
`{ "duration_s": ... }`, `{ "reps": ... }`, or `{ "weight_kg": ... }`.

For a test that `ath init` does not seed, create a `benchmark_definition` first.
`benchmark_key` is lowercase letters, digits, `-`, and `_`. `benchmark_type` is
`named_wod`, `run`, `hyrox`, `lift`, or `custom`. `score_type` is `time`, `reps`, or
`load`.

```json ath-record-example
[
  {
    "record_type": "benchmark_definition",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000401",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" },
        "authored_at": "2026-11-15T20:00:00.000Z",
        "evidence": [],
        "content_version": 1,
        "status": "active"
      },
      "benchmark_key": "cindy",
      "benchmark_type": "named_wod",
      "score_type": "reps",
      "definition": "As many rounds and reps as possible in 20 minutes: 5 pull-ups, 10 push-ups, 15 air squats. Score is total reps (one round = 30 reps). Source: crossfit.com/benchmark/cindy.",
      "tags": ["long", "gymnastics"]
    }
  },
  {
    "record_type": "goal",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" },
        "authored_at": "2026-11-15T20:00:00.000Z",
        "evidence": [ { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000210", "revision": 1 } ],
        "content_version": 1,
        "status": "active"
      },
      "title": "Cindy as prescribed: 15 rounds (450 reps)",
      "description": "Last Cindy was 12 rounds and 4 reps with all pull-ups kipping. Goal agreed with the athlete. The target is a coaching hypothesis, not a prediction.",
      "target_date": "2027-01-30",
      "target_benchmark_definition_id": "01890f3e-e7d3-7cc0-88c8-000000000401",
      "target_score": { "reps": 450 }
    }
  }
]
```

The evidence list above is empty for the definition because nothing in the record
informed it. Its source is the crossfit.com page named in the text.

## A plan and its sessions

```json ath-record-example
[
  {
    "record_type": "training_plan",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000501",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" },
        "authored_at": "2026-11-15T20:00:00.000Z",
        "evidence": [ { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000210", "revision": 1 } ],
        "content_version": 1,
        "status": "active"
      },
      "title": "Pull-up endurance block",
      "objective": "Raise repeatable kipping pull-up sets from 3 to 6 so Cindy can be done as prescribed at a steady pace.",
      "start_date": "2026-11-16",
      "end_date": "2026-12-27"
    }
  },
  {
    "record_type": "planned_session",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" },
        "authored_at": "2026-11-15T20:00:00.000Z",
        "evidence": [ { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000210", "revision": 1 } ],
        "content_version": 1,
        "status": "active"
      },
      "training_plan_id": "01890f3e-e7d3-7cc0-88c8-000000000501",
      "title": "Gymnastics: kipping pull-up sets",
      "scheduled_date": "2026-11-17",
      "activity_type": "crossfit",
      "blocks": [
        { "block_type": "warm_up", "title": "Shoulders and hollow", "instructions": "Arm circles, band pull-aparts 2 x 15, hollow hold 3 x 20 s.", "planned_duration_s": 480 },
        { "block_type": "skill", "title": "Kipping pull-ups, every minute for 10 minutes", "instructions": "Purpose: repeatable sets. 3 kipping pull-ups at the start of each minute. Stop a set if the swing loses timing; do 2 next minute.", "planned_duration_s": 600, "planned_reps": 30 }
      ],
      "completion_status": "planned"
    }
  }
]
```

## A coaching note

Use a note for standing rules, such as how to scale class workouts or how the
athlete rates session effort.

```json ath-record-example
[
  {
    "record_type": "coaching_note",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" },
        "authored_at": "2026-11-15T20:00:00.000Z",
        "evidence": [],
        "content_version": 1,
        "status": "active"
      },
      "title": "How to rate session effort",
      "note": "Rate each whole session on the 0-10 scale, for the whole session's minutes, warm-up to cool-down (Foster et al., 2001). Give the rating about 30 minutes after the session ends, as later papers describe Foster's method. Keep this method for every session so weekly loads can be compared. Nothing in the record informed this rule; it sets the method from now on.",
      "applies_from": "2026-11-16"
    }
  }
]
```

## A saved review

```json ath-record-example
[
  {
    "record_type": "saved_training_review",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" },
        "authored_at": "2026-11-22T19:00:00.000Z",
        "evidence": [
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000611", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000612", "revision": 1 }
        ],
        "content_version": 1,
        "status": "active"
      },
      "period_start": "2026-11-16",
      "period_end": "2026-11-22",
      "summary": "Observed: 5 of 5 planned sessions done. Both kipping sessions hit every set. Evidence: CrossFit advises adding intensity only after mechanics and consistency (L1 p. 77, official). Hypothesis: the athlete can move to 4 reps per minute.",
      "recommendations": [ "Kipping sets rise from 3 to 4 per minute on 24 November. Check: every set completed with the swing in time." ]
    }
  }
]
```

## Session effort: the athlete's report

The athlete gives the rating. You only help them record it, with their approval.
It has no source and no authorship.

```json ath-record-example
[
  {
    "record_type": "athlete_report",
    "data": {
      "report_type": "session_effort",
      "reported_at": "2026-11-16T19:30:00.000Z",
      "input_reported_at": { "timestamp_text": "2026-11-16T19:30:00.000Z", "utc_offset_text": "Z", "utc_offset_meaning": "documented_recording_offset", "timestamp_basis": "source_instant" },
      "reported_via": "text",
      "rating": 7,
      "rating_scale": "0-10",
      "duration_seconds": 3600,
      "workout_session_id": "01890f3e-e7d3-7cc0-88c8-000000000701"
    }
  }
]
```

Keep one athlete on one scale: `0-10`, `1-10`, or `6-20`. `duration_seconds` is the
span the rating covers. Then run `ath apply training-load-effort --record <id> --json`.

## Benchmark results and the `org.athleticstandard/crossfit-score` extension

A benchmark result is a measured record. When the athlete types it, it uses the
`manual` source, `received_at_basis: local_write_time`, and
`field_basis.value: entered_by_hand`. Find the manual source with
`ath records list --type source --limit 10000 --json`.

A plain prescribed time can be logged as text: `ath log Fran in 4:41 rx`. Use the
structured form for anything else: a scaled or modified version, reps, a cap, or a
competition score.

The extension sits beside `data`, under `extensions`. Its fields:

| Field | Required | Meaning |
|---|---|---|
| `variant` | Yes | `rx`: as the definition prescribes. `scaled`: a published scaled version, such as an Open Scaled tier or a crossfit.com intermediate or beginner version; name it in `standards`. `foundations`: the Open Foundations tier. `modified`: any other change, by the gym, the coach, or the athlete. |
| `time_cap_s` | No | The cap in seconds. |
| `capped` | No | `true` when the cap ended the attempt before the work was done. Then `score.reps` holds the reps completed. |
| `tiebreak_s` | No | Seconds from the start to the workout's tiebreak point. |
| `loads` | No | Each external load: `movement`, `weight`, `weight_unit` (`kg` or `lb`), in the unit used. |
| `substitutions` | No | Each changed movement: `prescribed`, `performed`, `reason`. |
| `standards` | No | Free text: the movement standards used, such as pull-up style or target height. |
| `competition` | No | The event and workout, such as "2026 CrossFit Open 26.1". |
| `judged` | No | `true` when a judge or video review validated the score. |

Set the record's own `scaling` field to `rx` only when `variant` is `rx`. Set it to
`scaled` for every other variant. The extension holds the detail.

A zero-rep attempt cannot be a benchmark score (D249). The athlete logs it as a
note in their own words. Say in your reply, or in a `coaching_note`, that it is not
a benchmark score.

### A finished, modified attempt

```json ath-record-example
[
  {
    "record_type": "benchmark_result",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000201",
    "data": {
      "source_id": "01890f3e-e7d3-7cc0-88c8-000000000001",
      "received_at": "2026-10-17T11:20:00.000Z",
      "received_at_basis": "local_write_time",
      "field_basis": { "value": "entered_by_hand" },
      "benchmark_definition_id": "01890f3e-e7d3-7cc0-88c8-000000000103",
      "recorded_at": "2026-10-17T10:15:00.000Z",
      "input_timestamp": { "timestamp_text": "2026-10-17T10:15:00.000Z", "utc_offset_text": "Z", "utc_offset_meaning": "documented_recording_offset", "timestamp_basis": "source_instant" },
      "score": { "duration_s": 768 },
      "input_score_text": "12:48",
      "scaling": "scaled"
    },
    "extensions": {
      "org.athleticstandard/crossfit-score": {
        "variant": "modified",
        "loads": [ { "movement": "kettlebell swing", "weight": 16, "weight_unit": "kg" } ],
        "substitutions": [ { "prescribed": "pull-up", "performed": "band-assisted kipping pull-up, thin band", "reason": "building pull-up capacity" } ],
        "standards": "Kettlebell swings to overhead. Pull-ups chin over bar. 400 m run on the gym's usual loop.",
        "judged": false
      }
    }
  }
]
```

### A capped attempt

The cap ended Fran at 10:00 with 78 of 90 reps done. The score is reps. There is no
time.

```json ath-record-example
[
  {
    "record_type": "benchmark_result",
    "data": {
      "source_id": "01890f3e-e7d3-7cc0-88c8-000000000001",
      "received_at": "2026-11-20T19:10:00.000Z",
      "received_at_basis": "local_write_time",
      "field_basis": { "value": "entered_by_hand" },
      "benchmark_definition_id": "01890f3e-e7d3-7cc0-88c8-000000000101",
      "recorded_at": "2026-11-20T18:40:00.000Z",
      "input_timestamp": { "timestamp_text": "2026-11-20T18:40:00.000Z", "utc_offset_text": "Z", "utc_offset_meaning": "documented_recording_offset", "timestamp_basis": "source_instant" },
      "score": { "reps": 78 },
      "input_score_text": "capped at 10:00, 78 reps",
      "scaling": "rx"
    },
    "extensions": {
      "org.athleticstandard/crossfit-score": {
        "variant": "rx",
        "time_cap_s": 600,
        "capped": true,
        "loads": [ { "movement": "thruster", "weight": 29, "weight_unit": "kg" } ],
        "standards": "Kipping pull-ups, chin over bar. 10-minute cap set by the class.",
        "judged": false
      }
    }
  }
]
```

### A competition score

A capped Open score with a tiebreak. The benchmark definition holds the workout as
written on the official scorecard.

```json ath-record-example
[
  {
    "record_type": "benchmark_result",
    "data": {
      "source_id": "01890f3e-e7d3-7cc0-88c8-000000000001",
      "received_at": "2026-02-27T20:30:00.000Z",
      "received_at_basis": "local_write_time",
      "field_basis": { "value": "entered_by_hand" },
      "benchmark_definition_id": "01890f3e-e7d3-7cc0-88c8-000000000301",
      "recorded_at": "2026-02-27T18:00:00.000Z",
      "input_timestamp": { "timestamp_text": "2026-02-27T18:00:00.000Z", "utc_offset_text": "Z", "utc_offset_meaning": "documented_recording_offset", "timestamp_basis": "source_instant" },
      "score": { "reps": 180 },
      "input_score_text": "180 reps, tiebreak 7:11",
      "scaling": "rx"
    },
    "extensions": {
      "org.athleticstandard/crossfit-score": {
        "variant": "rx",
        "time_cap_s": 720,
        "capped": true,
        "tiebreak_s": 431,
        "loads": [ { "movement": "wall-ball shot", "weight": 20, "weight_unit": "lb" } ],
        "standards": "Men 16-54 Rx: 20 lb ball to a 10 ft target. Other standards as on the 26.1 scorecard.",
        "competition": "2026 CrossFit Open 26.1",
        "judged": true
      }
    }
  }
]
```

### A zero-rep attempt

The athlete's note, in their words. Your classification ("not a benchmark score")
goes in your reply, not in their note.

```json ath-record-example
[
  {
    "record_type": "athlete_report",
    "data": {
      "report_type": "note",
      "reported_at": "2026-11-21T11:00:00.000Z",
      "input_reported_at": { "timestamp_text": "2026-11-21T11:00:00.000Z", "utc_offset_text": "Z", "utc_offset_meaning": "documented_recording_offset", "timestamp_basis": "source_instant" },
      "reported_via": "text",
      "note": "Tried Amanda as written today. Couldn't get a single muscle-up, stopped at 6 minutes."
    }
  }
]
```

## Revising a planned session

Mark a session done, or change a future session, with `ath correct`. Send the
complete data. This example marks a session completed and links the workout.

The block below is a data object for `ath correct`. Do not send it to `ath log`:
the record already exists. Pass only its `data` object.

```json ath-record-example
[
  {
    "record_type": "planned_session",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000502",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" },
        "authored_at": "2026-11-17T20:00:00.000Z",
        "evidence": [
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000210", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000702", "revision": 1 }
        ],
        "content_version": 2,
        "status": "completed"
      },
      "training_plan_id": "01890f3e-e7d3-7cc0-88c8-000000000501",
      "title": "Gymnastics: kipping pull-up sets",
      "scheduled_date": "2026-11-17",
      "activity_type": "crossfit",
      "blocks": [
        { "block_type": "warm_up", "title": "Shoulders and hollow", "instructions": "Arm circles, band pull-aparts 2 x 15, hollow hold 3 x 20 s.", "planned_duration_s": 480 },
        { "block_type": "skill", "title": "Kipping pull-ups, every minute for 10 minutes", "instructions": "Purpose: repeatable sets. 3 kipping pull-ups at the start of each minute. Stop a set if the swing loses timing; do 2 next minute.", "planned_duration_s": 600, "planned_reps": 30 }
      ],
      "completion_status": "completed",
      "completed_workout_session_id": "01890f3e-e7d3-7cc0-88c8-000000000702"
    }
  }
]
```

The command passes only the `data` object:

```sh
ath correct 01890f3e-e7d3-7cc0-88c8-000000000502 --revision 1 --data '<the data object>' --json
```

## Strength sets on a workout

When the athlete logs lifts, each set names its movement and load basis (D239):
`{ "reps": 5, "weight": 60, "weight_unit": "kg", "movement": "back squat",
"load_basis": "total" }`. Two 22.5 kg dumbbells are `"weight": 22.5,
"load_basis": "per_implement", "implement_count": 2`. Logging workouts belongs to
the `athletic-standard` skill.
