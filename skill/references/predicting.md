# Making a prediction

Inside an agent harness, the harness model makes the prediction. No gateway key is
needed.

Run:

```sh
ath predict <benchmark> --json
```

This command returns evidence. It does not return or save a prediction. Its response
starts with:

```json
{
  "purpose": "prediction_evidence",
  "is_prediction": false
}
```

Reason over the evidence, then save a v1 `prediction` record through structured
`ath log`.

## Evidence response

The v1 response has:

| Field | Meaning |
|---|---|
| `purpose` | Literal `prediction_evidence` |
| `is_prediction` | Literal `false` |
| `as_of` | Exact UTC knowledge cutoff |
| `calendar_time_zone` | Optional IANA timezone used to resolve a calendar `--as-of` |
| `benchmark` | Current benchmark-definition envelope at the cutoff |
| `prior_results` | Earlier results for that benchmark |
| `device_measurements` | Non-manual point measurements from the prior 90 days |
| `athlete_reports` | Athlete reports from the prior 90 days |
| `sleep_sessions` | Sleep sessions from the prior 90 days |
| `workout_sessions` | Workout sessions from the prior 90 days |
| `prior_predictions` | Earlier predictions for that benchmark |
| `gaps` | Missing evidence and query-limit warnings |

Each item is a current record envelope with `record_id`, `current_revision`, and
typed `data`. Use those values for exact evidence references.

The evidence response does not silently pool sources. Each measured item still names
its `source_id`. Run `ath records show <source-id> --json` when the source details
matter.

`device_measurements` excludes the manual source. Athlete reports remain separate
and have no source.

The response is a bounded evidence view. Use `ath stats --json`,
`ath calculate ... --json`, `ath series ... --json`, and
`ath records show ... --json` when the question needs coverage, baselines, samples,
links, grades, or revisions not present in this response.

## Historical cutoff

```sh
ath predict <benchmark> --as-of 2026-10-01 --timezone America/Chicago --json
```

A calendar date needs an explicit or saved IANA timezone. An exact UTC timestamp
does not.

Historical visibility uses each revision's `became_known_at`. A record with an
earlier event time is not visible before the database learned it. The response also
excludes an event whose own event time is after the cutoff.

Do not look at later records before making a historical prediction.

## Read the evidence

1. Start with `prior_results`. Check count, dates, score type, and direction of
   change. A small number of results does not establish a trend.
2. Read recent device measurements by source. Do not combine two devices into one
   baseline.
3. Keep RMSSD and SDNN separate. Keep temperature quantities and activity-specific
   distances separate.
4. Read sleep and workout sessions as source records. A missing session means no
   recording, not proof that nothing happened.
5. Read athlete reports as the athlete's claims. They may change confidence or
   explain uncertainty. Do not present them as device measurements.
6. Read prior predictions. Follow links with `ath records show <id> --links --json`
   when earlier grades or analyses matter.
7. Carry every item in `gaps` into the confidence and reasoning.

Provider scores are source-reported composites. If separately inspected, they may
support a claim. Do not treat them as raw sensor measurements.

## Structured prediction record

Use the benchmark definition's UUIDv7 `record_id`, not only its key. Include exact
evidence revisions actually used.

```json
{
  "record_type": "prediction",
  "data": {
    "authorship": {
      "author": {
        "author_type": "agent",
        "agent": "agent-harness",
        "model": "model-id"
      },
      "authored_at": "2026-10-13T04:00:00.000Z",
      "evidence": [
        {
          "record_id": "01890f3e-e7d3-7cc0-88c8-000000000010",
          "revision": 2
        }
      ],
      "content_version": 1,
      "status": "active"
    },
    "benchmark_definition_id": "01890f3e-e7d3-7cc0-88c8-000000000001",
    "predicted_score": {
      "duration_s": 275
    },
    "predicted_range": {
      "low": {
        "duration_s": 265
      },
      "high": {
        "duration_s": 290
      }
    },
    "confidence": "moderate",
    "reasoning": "The reasoning cites exact dated results and measurements, separates sources, and states the missing coverage."
  }
}
```

`confidence` is `low`, `moderate`, or `high`.

The score field must match the benchmark:

- `duration_s` for time;
- `reps` for reps;
- `weight_kg` for load.

`predicted_range` is optional in the schema. Include it when a useful uncertainty
range can be stated. Grading can only mark `result_in_predicted_range: true` when a
range exists.

The agent author requires both `agent` and `model`. `agent_version` and
`model_version` are optional. Do not put change-actor fields beside the authorship
object.

Reasoning should cite dates, values, source IDs when relevant, and evidence limits.
Do not replace exact evidence with vague trend language.

## Save only after approval

Preview:

```sh
ath log '<prediction-object>' --agent agent-harness --json
```

Show the complete record. `--json` never acts as confirmation. After approval:

```sh
ath log '<prediction-object>' --agent agent-harness --yes
```

The tool creates a UUIDv7 `record_id` when one was not supplied.

## Bare terminal behavior

Bare `ath predict <benchmark>` is different. It calls the selected gateway model,
prints a preview, and may save that model's prediction after confirmation. It needs
a Vercel or OpenRouter key stored outside athlete data.

Inside a harness, always use `--json` and write the harness model's prediction
through `ath log`.

## Backtesting

`ath backtest` replays eligible historical results with gateway models. It uses
knowledge-time cutoffs and writes a report beside the database. It does not change
athlete data.
