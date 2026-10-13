# Logging v1 records

`ath log` is the general record-creation command. It is the only command that enters
a benchmark result.

It accepts either:

- one plain-text entry for a common manual record; or
- one v1 `{ "record_type": "...", "data": { ... } }` object; or
- an array of those v1 objects, written in one transaction.

## Preview before writing

For an agent, preview with `--json`:

```sh
ath log RMSSD 61 ms --json
```

`--json` never acts as confirmation. Show the returned interpretation and records.
After approval, repeat the command without `--json` and pass `--yes`:

```sh
ath log RMSSD 61 ms --yes
```

Without `--yes`, an interactive terminal asks before writing.

## Plain text

The plain-text parser is intentionally narrow. It supports:

- a known point quantity, number, and exact standard unit;
- a time-scored benchmark result such as `Fran in 4:41 rx`;
- a rating such as `sore quads 4/5`;
- any other text as an athlete note.

Examples:

```sh
ath log RMSSD 61 ms
ath log resting heart rate 52 bpm
ath log sore quads 4/5
ath log Fran in 4:41 rx
ath log Easy session, no soreness
```

Say `RMSSD` or `SDNN`. Plain `HRV` is refused because those statistics must not
share a baseline.

A point measurement entered as text uses the database's `manual` source. It is
never treated as device evidence.

Use `--at 2026-10-13T04:00:00.000Z` for an exact earlier event time. Normalized
times use UTC with fixed millisecond precision.

Use `--benchmark <key>` to name a time-scored result when the text does not contain
the intended benchmark name. If the key does not exist, the plain path creates a
custom time-scored benchmark definition in the same transaction.

Use structured input for rep scores, load scores, plans, predictions, nutrition,
and other records outside the narrow text parser.

## Structured envelope

Each item has:

| Top-level field | Required | Meaning |
|---|---|---|
| `record_type` | yes | A v1 record type except the tool-managed prediction-result link and grade |
| `data` | yes | Complete typed v1 data object |
| `record_id` | no | Lowercase UUIDv7; generated when omitted |
| `created_at` | no | UTC millisecond timestamp; current time when omitted |

Example athlete report:

```json
{
  "record_type": "athlete_report",
  "data": {
    "report_type": "soreness",
    "reported_at": "2026-10-13T04:00:00.000Z",
    "input_reported_at": {
      "timestamp_text": "2026-10-13T04:00:00.000Z",
      "utc_offset_text": "Z",
      "utc_offset_meaning": "documented_recording_offset",
      "timestamp_basis": "source_instant"
    },
    "reported_via": "text",
    "rating": 4,
    "rating_scale": "1-5",
    "body_region": "quads",
    "note": "Quads sore after intervals, 4/5."
  }
}
```

Run `ath log`, paste the object, and press Ctrl-D. A shell-quoted JSON object also
works.

An array uses the same item envelope:

```json
[
  {
    "record_type": "athlete_report",
    "data": {
      "report_type": "energy",
      "reported_at": "2026-10-13T04:00:00.000Z",
      "input_reported_at": {
        "timestamp_text": "2026-10-13T04:00:00.000Z",
        "utc_offset_text": "Z",
        "utc_offset_meaning": "documented_recording_offset",
        "timestamp_basis": "source_instant"
      },
      "reported_via": "text",
      "rating": 3,
      "rating_scale": "1-5",
      "note": "Energy 3/5."
    }
  },
  {
    "record_type": "goal",
    "data": {
      "authorship": {
        "author": {
          "author_type": "agent",
          "agent": "agent-harness",
          "model": "model-id"
        },
        "authored_at": "2026-10-13T04:00:00.000Z",
        "evidence": [],
        "content_version": 1,
        "status": "active"
      },
      "title": "Repeat the current benchmark"
    }
  }
]
```

Every item is validated before the batch writes. One invalid item means nothing is
written.

Structured logging cannot create `prediction_result_link` or `prediction_grade`
records. Those records must come from automatic result handling or `ath grade`, so
the tool can validate the exact prediction and result revisions and calculate the
grade.

`--agent <name>` names the actor making the structured database change. It does not
fill `data.authorship`. An authored record must still identify its athlete, agent,
or tool author inside `data`.

## Structured measured records

Structured measured data needs an existing source ID. Find sources first:

```sh
ath records list --type source --json
```

Do not claim a device source for a number supplied by the athlete. Use the manual
source ID for hand-entered measurements and results.

Example point measurement:

```json
{
  "record_type": "point_measurement",
  "data": {
    "source_id": "01890f3e-e7d3-7cc0-88c8-000000000001",
    "received_at": "2026-10-13T04:00:00.000Z",
    "received_at_basis": "local_write_time",
    "quantity": "hrv_rmssd",
    "recorded_at": "2026-10-13T04:00:00.000Z",
    "input_timestamp": {
      "timestamp_text": "2026-10-13T04:00:00.000Z",
      "utc_offset_text": "Z",
      "utc_offset_meaning": "documented_recording_offset",
      "timestamp_basis": "source_instant"
    },
    "value": {
      "standard_value": 61,
      "standard_unit": "ms",
      "input_value": 61,
      "input_unit": "ms"
    }
  }
}
```

Every measured record requires `source_id`, `received_at`, and
`received_at_basis`. Source values keep both the standard value and source receipt.
See [format.md](format.md) for other measured fields and units.

Athlete reports and nutrition entries have no `source_id`. Unknown fields are
rejected by their v1 data schemas.

## Authored records

Predictions, grades, error analyses, plans, planned sessions, coaching notes, saved
reviews, goals, benchmark definitions, and nutrition interpretations carry
`authorship`.

Agent authorship requires:

```json
{
  "author": {
    "author_type": "agent",
    "agent": "agent-harness",
    "model": "model-id"
  },
  "authored_at": "2026-10-13T04:00:00.000Z",
  "evidence": [
    {
      "record_id": "01890f3e-e7d3-7cc0-88c8-000000000010",
      "revision": 1
    }
  ],
  "content_version": 1,
  "status": "active"
}
```

Evidence always names an exact record ID and revision.

## Benchmark results and automatic grading

A structured benchmark result names `benchmark_definition_id`, not a benchmark key.
Find it with:

```sh
ath records list --type benchmark_definition --json
```

When a new result has exactly one matching open prediction, the same log transaction
adds a `prediction_result_link` and a tool-authored `prediction_grade`. The preview
shows all records.

No open prediction means only the result is written. Several matching predictions
mean no link or grade is guessed. Use explicit grading afterwards:

```sh
ath grade <prediction-id> --result <result-id> --json
```

## Corrections are not new logs

Use `ath correct` to replace the complete data of an existing record while keeping
its ID and prior revisions:

```sh
ath records show <record-id> --history --json
ath correct <record-id> --revision <current-revision> \
  --data '<complete-data-object>' --json
```

Use `ath link` to add or change a benchmark result's workout-session link. Do not
log a duplicate result to correct a value or link.
