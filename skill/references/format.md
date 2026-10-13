# Athletic Standard v1 format

The live record is `athlete.ath.sqlite`. SQLite holds dense samples, current
records, complete revision history, normal deletions, value-free erasure markers,
and provider import keys.

JSON exists only as an explicit complete export or import package. It is not a live
copy and does not stay synchronized.

The full public field reference is `SPEC.md`. Exhaustive generated schemas are in
`schema/1.0.0/`.

## Common record fields

A current logical record is:

```json
{
  "record_id": "<lowercase UUIDv7>",
  "record_type": "<one of 23 types>",
  "created_at": "2026-10-13T04:00:00.000Z",
  "current_revision": 1,
  "is_deleted": false,
  "data": {}
}
```

A retained revision has `record_id`, `record_type`, `revision`,
`previous_revision`, `became_known_at`, `became_known_at_basis`, `changed_by`, and
the complete typed `data` object.

Evidence uses an exact record revision:

```json
{ "record_id": "<UUIDv7>", "revision": 1 }
```

Corrections keep the stable ID and add a revision. Links use stable IDs, not
timestamps.

## Record classes

Measured records require `source_id`:

- `point_measurement`
- `measurement_sample`
- `sleep_session`
- `workout_session`
- `benchmark_result`
- `provider_summary`
- `provider_score`

A `manual` source labels a hand-entered measurement or result. It does not count as
device evidence.

Athlete reports have no source field:

- `athlete_report`
- `nutrition_entry`

Authored records carry `authorship`, with author, time, ordered evidence,
`content_version`, and status:

- `benchmark_definition`
- `prediction`
- `prediction_grade`
- `prediction_error_analysis`
- `training_plan`
- `planned_session`
- `coaching_note`
- `saved_training_review`
- `goal`
- `nutrition_model_interpretation`

`prediction_result_link` carries its own creator, exact prediction and result
revisions, link time, content version, and status.

The remaining records are `athlete_profile`, `source`, `source_device`, and
`attachment`.

## The 23 record types

| Type | Main `data` fields |
|---|---|
| `athlete_profile` | `name`, `birth_year`, `sex`, `display_units`, `default_time_zone` |
| `source` | `source_type`, `data_writer`, `data_route`, optional vendor, sensor, detail |
| `source_device` | `source_id`, device description, coverage dates, accepted count |
| `point_measurement` | measured fields, `quantity`, optional `recorded_at`, input timestamp receipt, source value, optional calculations |
| `measurement_sample` | measured fields, `quantity`, `recorded_at`, input timestamp receipt, source value, optional duration and reconstruction receipt |
| `sleep_session` | measured fields, start/end and input receipts, source summaries |
| `workout_session` | measured fields, start/end and input receipts, activity, source summaries, optional segments |
| `benchmark_result` | measured fields, benchmark ID, time receipt, score, scaling, optional workout ID and segments |
| `provider_summary` | measured fields, summary type, source value, instant or provider period |
| `provider_score` | measured fields, metric, source value, scale, instant or provider period |
| `athlete_report` | report type, report time receipt, text/voice/photo route, optional rating, region, note, attachments |
| `nutrition_entry` | report time receipt, route, athlete description, reported items, attachments |
| `attachment` | file name, media type, byte count, SHA-256, optional description |
| `benchmark_definition` | authorship, key, kind, score type, definition, optional tags |
| `prediction` | authorship, benchmark ID, predicted score and range, confidence, reasoning |
| `prediction_result_link` | exact prediction and result revisions, creator, link time, version, status |
| `prediction_grade` | authorship, exact link revision, error values, range result, copied scores |
| `prediction_error_analysis` | authorship, exact grade revision, direction, severity, evidence-backed causes, unexplained flag, lesson |
| `training_plan` | authorship, title, objective, dates |
| `planned_session` | authorship, optional plan ID, schedule, activity, blocks, completion, optional completed workout ID |
| `coaching_note` | authorship, title, note, applicable dates |
| `saved_training_review` | authorship, period, summary, optional recommendations |
| `goal` | authorship, title, description, date, optional benchmark target and score |
| `nutrition_model_interpretation` | authorship, nutrition entry ID, confidence, interpreted items, explanation |

## Measured value receipts

Every measured record has:

- `source_id`
- optional `provider_record_id`
- optional `provider_reporting_periods`
- `received_at`
- `received_at_basis`

A source value has `standard_value` and `standard_unit`. It also has either
`input_value` or `input_value_missing_reason`, and either `input_unit` or
`input_unit_missing_reason`.

An input timestamp receipt preserves source timestamp and offset text. It also says
what the route documents those values to mean. Normalized instants always use fixed
UTC millisecond text: `YYYY-MM-DDTHH:mm:ss.SSSZ`.

Provider periods remain distinct:

- `whoop_physiological_cycle`
- `oura_activity_day`
- `oura_sleep_day`
- `oura_calendar_day`
- `oura_day_unspecified`
- `apple_activity_summary_day`

A date-only source row stays date-only.

## Measurement units

Point quantities:

| Quantity | Unit |
|---|---|
| `hrv_rmssd`, `hrv_sdnn` | `ms` |
| `resting_heart_rate`, `walking_heart_rate`, `hr_recovery` | `bpm` |
| `respiratory_rate` | `brpm` |
| `oxygen_saturation`, `body_fat_percentage` | `%` |
| `body_weight`, `lean_body_mass` | `kg` |
| `height` | `cm` |
| `vo2_max` | `ml/kg/min` |
| `body_temperature`, `skin_temperature`, `wrist_temperature_sleeping`, `temperature_deviation` | `°C` |
| `blood_pressure_systolic`, `blood_pressure_diastolic` | `mmHg` |

Sample quantities:

| Quantity | Unit |
|---|---|
| `heart_rate` | `bpm` |
| `hrv_beats`, `ecg_beats`, `hrv_sdnn` | `ms` |
| `respiratory_rate` | `brpm` |
| `oxygen_saturation` | `%` |
| `steps`, `flights_climbed` | `count` |
| `active_energy`, `basal_energy` | `kcal` |
| `distance_walking_running`, `distance_cycling`, `distance_swimming` | `m` |
| `running_speed`, `walking_speed` | `m/s` |
| `running_power` | `W` |
| `running_stride_length`, `walking_step_length` | `m` |
| `running_vertical_oscillation` | `cm` |
| `running_ground_contact_time` | `ms` |
| `physical_effort` | `MET` |
| `exercise_time`, `time_in_daylight` | `min` |
| `walking_asymmetry_percentage` | `%` |

RMSSD and SDNN are different statistics. Never combine them. Temperature quantities,
distance activities, and sources also remain separate.

## Delete and erase

Normal deletion writes `record_id`, `record_revision`, `deleted_at`, and
`deleted_by`. The values and revisions remain.

Privacy erasure removes value-bearing state and stored dependents. It may leave only
`record_id`, `erased_at`, and a fixed `reason_code`. It does not leave health values,
text, source detail, revisions, or attachment hashes.

## Complete package

```text
athlete.ath.json
series/<source-id>-<quantity>.ath-series.json
attachments/<stored files>
export-manifest.json
```

`athlete.ath.json` contains:

- `athleticstandard_version`
- `database_id`
- `database_revision`
- current non-sample `records`
- non-sample `record_revisions`
- non-sample `record_deletions`
- all `record_erasure_markers`
- non-sample `record_import_keys`
- `measurement_series_files` with paths, hashes, bounds, and counts

Each generated series file contains one source and quantity, current sample records,
their complete revisions, normal deletions, and import keys.

The manifest names the source database and revision. It lists each other package
file's hash and byte count, counts by record type, and the erasure-marker count.
`exported_at` is optional and absent by default.

Exports are deterministic for the same database revision and options. Every
referenced attachment must be present and match its record.

## Security

The database contains sensitive health data. `ath` does not encrypt it. Recommend
operating-system disk encryption. Anyone who can read the database or a package can
read the athlete record.
