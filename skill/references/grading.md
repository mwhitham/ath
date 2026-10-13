# Grading an existing prediction

`ath grade` grades one existing prediction against one existing benchmark result.
It never records a result.

Enter a result through `ath log`. Then find both stable IDs:

```sh
ath records list --type prediction --json
ath records list --type benchmark_result --json
```

Preview the grade:

```sh
ath grade <prediction-id> --result <result-id> --json
```

The preview shows:

- the exact prediction ID and revision;
- the exact result ID and revision;
- whether a prediction-result link will be created or reused;
- the complete tool-authored grade data;
- `write_confirmed: false`.

`--json` never acts as confirmation. After approval, repeat without `--json`:

```sh
ath grade <prediction-id> --result <result-id> --yes
```

The prediction and result must use the same benchmark definition. Both must be
current and not normally deleted.

## What grading writes

If the exact prediction and result revisions are not linked, grading creates a
UUIDv7 `prediction_result_link`. If the same result record was corrected, grading
revises its existing link to name the new result revision. Older grades keep
pointing to the older link revision.

It then creates a UUIDv7 `prediction_grade` that points to the exact link revision.
The grade stores:

- the tool authorship and calculation version;
- the exact link revision as evidence;
- `signed_error`;
- `absolute_error_percentage`;
- `result_in_predicted_range`;
- the exact predicted and actual score objects.

The default calculation version is `prediction-grade-v1`. A later deterministic
implementation may be named explicitly:

```sh
ath grade <prediction-id> --result <result-id> \
  --calculation-version prediction-grade-v2 --json
```

Several calculation versions may grade the same exact link without changing the
prediction or result.

## Arithmetic

The benchmark definition selects `duration_s`, `reps`, or `weight_kg`.

```text
signed_error = predicted value - actual value
absolute_error_percentage = abs(signed_error) / actual value * 100
```

The reference command rounds signed error to two decimal places and absolute error
percentage to one decimal place.

`result_in_predicted_range` is true only when the prediction has a range and the
actual value is between its low and high values, inclusive. A prediction without a
range is not a range hit.

The arithmetic is performed by code. Do not replace it with a model judgment.

## Result logging may grade automatically

`ath log` remains the only result-entry command.

When a new result matches exactly one open prediction, its transaction can also
create the prediction-result link and grade. No open prediction writes only the
result. Several possible predictions produce no guessed link or grade.

Use explicit `ath grade <prediction-id> --result <result-id>` for:

- an imported result;
- an ambiguous result;
- a corrected result;
- a later grade calculation version.

## Prediction error analysis

A prediction error analysis is a separate authored record. It is not a grade option.
Write it through structured `ath log` after reading the exact grade and supporting
records:

```sh
ath records show <grade-id> --links --json
ath records show <evidence-id> --revision <revision> --json
```

The structured object is:

```json
{
  "record_type": "prediction_error_analysis",
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
          "record_id": "01890f3e-e7d3-7cc0-88c8-000000000030",
          "revision": 1
        }
      ],
      "content_version": 1,
      "status": "active"
    },
    "prediction_grade": {
      "record_id": "01890f3e-e7d3-7cc0-88c8-000000000030",
      "revision": 1
    },
    "error_direction": "slower",
    "error_severity": "significant",
    "causes": [
      {
        "evidence": {
          "record_id": "01890f3e-e7d3-7cc0-88c8-000000000020",
          "revision": 2
        },
        "explanation": "The cited sleep-session revision reports less sleep than the comparison period."
      }
    ],
    "unexplained": false,
    "lesson": "State the evidence limit more clearly in the next prediction."
  }
}
```

`error_direction` is `faster`, `slower`, `higher`, or `lower`.
`error_severity` is `minor`, `significant`, or `severe`.

Every claimed cause must name an existing exact record revision. The explanation
must say what that record supports. If no recorded evidence supports a cause, use an
empty `causes` array and set `unexplained: true`.

Do not infer that association proves cause. Keep the analysis tied to stored
evidence and state uncertainty.

Preview, obtain approval, and then write:

```sh
ath log '<analysis-object>' --json
ath log '<analysis-object>' --yes
```
