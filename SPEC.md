# Athletic Standard Format Specification v1.0.0

**Status:** public v1 format
**Live local file:** `athlete.ath.sqlite`
**Portable package:** `athlete.ath.json` plus generated files
**Normative schemas:** [`src/v1/schema.ts`](src/v1/schema.ts)
**Generated JSON Schemas:** [`schema/1.0.0/`](schema/1.0.0/)

Athletic Standard is an open, local-first record for one athlete's training and
recovery data. SQLite is the sole live local record. JSON is an explicit, complete
package for export, import, inspection, and backup.

Local use needs no account, network connection, subscription, or hosted service.

## 1. One logical model, two encodings

The Zod schemas in `src/v1/schema.ts` define athlete-owned records. SQLite and the
JSON package encode the same logical records.

| Encoding | Use |
|---|---|
| SQLite | Live local record read and written by `ath` |
| JSON package | Complete portable snapshot used by explicit export and import |

An exported package does not update when the database changes. Editing a package
does not change the database. A package changes SQLite only through a confirmed
import.

Dense samples live in SQLite. They use generated JSON series files only inside an
exported package.

## 2. Record IDs, revisions, and database revisions

Every logical record has a stable lowercase UUIDv7 `record_id`. New records and new
database IDs use UUIDv7. A UUIDv7 time component helps database insertion order. It
is not an event timestamp.

A correction keeps the `record_id` and creates the next positive integer record
revision. Revision 1 has `previous_revision: null`. Every later revision points to
the previous integer.

The database also has a nonnegative `database_revision`. One committed athlete-data
transaction increments it once, even when that transaction writes several records.

Links to a current logical record use its UUIDv7 ID. Evidence and historical links
use both `record_id` and `revision`:

```json
{
  "record_id": "01890f3e-e7d3-7cc0-88c8-000000000010",
  "revision": 2
}
```

## 3. Time and dates

Every normalized instant is UTC text with fixed millisecond precision:

```text
YYYY-MM-DDTHH:mm:ss.SSSZ
```

Fixed-width UTC timestamps sort in instant order in SQLite. Importers may round a
source instant to the nearest millisecond for the normalized field. They preserve
the original timestamp text separately.

A calendar date is `YYYY-MM-DD`. A date-only provider record remains date-only.
Import must not invent midnight or any other instant.

Calendar calculations use an explicit IANA timezone such as `America/Chicago`, or
the saved `athlete_profile.default_time_zone`. They refuse when a timezone is needed
and neither is available. A profile timezone is a calculation choice. It is not
evidence that all historical records happened there.

Provider reporting periods remain distinct from calendar dates. See
[Provider reporting periods](#53-provider-reporting-periods).

## 4. Record lifecycle envelopes

### 4.1 Current record

Every current record has these fields:

| Field | Type | Meaning |
|---|---|---|
| `record_id` | lowercase UUIDv7 | Stable logical record ID |
| `record_type` | one of 23 values | Selects the typed `data` object |
| `created_at` | UTC timestamp | Time the Athletic Standard record was created |
| `current_revision` | positive integer | Latest retained revision |
| `is_deleted` | boolean | Whether normal deletion hides the record from current views |
| `data` | typed object | Complete current logical state |

A normally deleted record remains a current envelope with `is_deleted: true`. Its
data and revisions remain until a retention rule or privacy erasure removes them.

### 4.2 Retained revision

Every retained revision has:

| Field | Type | Meaning |
|---|---|---|
| `record_id` | lowercase UUIDv7 | Owning record |
| `record_type` | record type | Must match the current record |
| `revision` | positive integer | This revision number |
| `previous_revision` | integer or `null` | `null` for revision 1, otherwise `revision - 1` |
| `became_known_at` | UTC timestamp | When this state became available to the database |
| `became_known_at_basis` | enum | `local_write`, `provider_import`, `package_restore`, or `package_merge` |
| `changed_by` | record actor | Actor that made the change |
| `data` | typed object | Complete state at this revision |

A correction cannot use a `became_known_at` earlier than the prior revision.
Historical prediction and backtest queries use revision knowledge time, not only the
event time inside `data`.

### 4.3 Record actors

`changed_by`, `deleted_by`, and link creators use one of:

```json
{ "author_type": "athlete" }
```

```json
{
  "author_type": "agent",
  "agent": "agent-name",
  "agent_version": "1.2.3"
}
```

```json
{
  "author_type": "tool",
  "tool": "ath",
  "tool_version": "1.0.0"
}
```

`agent_version` is optional. Tool versions are semantic versions.

### 4.4 Normal deletion

A deletion has:

| Field | Type | Meaning |
|---|---|---|
| `record_id` | UUIDv7 | Deleted record |
| `record_revision` | positive integer | Revision that was current when deleted |
| `deleted_at` | UTC timestamp | Deletion time |
| `deleted_by` | record actor | Actor that deleted it |

The deletion names an existing retained revision. It hides the record from current
queries without removing its values or history.

### 4.5 Privacy erasure marker

Privacy erasure may leave only:

| Field | Type | Meaning |
|---|---|---|
| `record_id` | UUIDv7 | Stable ID that was erased |
| `erased_at` | UTC timestamp | Erasure time |
| `reason_code` | enum | `athlete_request`, `legal_requirement`, `recorded_in_error`, or `other` |

The marker has no foreign key to `records`, because the parent may be gone. It
contains no record type, health value, event time, source detail, free text,
revision, or attachment hash.

### 4.6 Provider import key

Provider imports match future rows through:

| Field | Type | Meaning |
|---|---|---|
| `record_id` | UUIDv7 | Existing Athletic Standard record |
| `match_key_method` | nonempty string | Versioned route recipe |
| `match_key` | nonempty string | Stable route-specific key |
| `imported_at` | UTC timestamp | Time the key was recorded |

The current methods are `apple_health/v1`, `whoop_csv/v1`, and `oura_csv/v1`.
A provider record ID is used when available. Otherwise the route uses stable source
fields and a collision position for otherwise indistinguishable rows.

The same key and content is a match. The same key with changed content creates a
revision. An unresolved duplicate or conflicting correction is refused rather than
matched by guess.

## 5. Shared logical objects

### 5.1 Authorship

Authored records use one `authorship` object:

| Field | Type | Meaning |
|---|---|---|
| `author` | authored-record author | Who wrote the claim |
| `authored_at` | UTC timestamp | When it was written |
| `evidence` | evidence reference array | Ordered exact record revisions used |
| `content_version` | positive integer | Version of the authored content |
| `status` | enum | `draft`, `active`, `completed`, `superseded`, or `cancelled` |

An athlete author is `{ "author_type": "athlete" }`.

An agent author requires `author_type: "agent"`, `agent`, and `model`. It may also
have `agent_version` and `model_version`.

A tool author requires `author_type: "tool"`, `tool`, `tool_version`, and
`calculation_version`.

### 5.2 Input timestamp receipts

Source and report times preserve source text in an input receipt:

| Field | Required | Type |
|---|---|---|
| `timestamp_text` | when supplied | nonempty string |
| `timestamp_missing_reason` | when text is absent | missing-reason enum |
| `timestamp_basis` | with timestamp text | `source_instant`, `source_offset_timestamp`, `source_local_timestamp_with_documented_offset`, or `reconstructed_from_series_start` |
| `utc_offset_text` | when supplied | nonempty string |
| `utc_offset_missing_reason` | when offset text is absent | missing-reason enum |
| `utc_offset_meaning` | with offset text | `documented_recording_offset`, `provider_cycle_offset`, `route_rendering_offset`, or `meaning_not_documented` |

Missing reasons are `not_provided_by_source`, `not_exposed_by_route`, and
`not_applicable`. A supplied value cannot also carry its missing reason.

### 5.3 Provider reporting periods

A provider reporting period has:

| Field | Required | Type |
|---|---|---|
| `period_type` | yes | provider-period enum |
| `provider_period_id` | no | nonempty string |
| `provider_date` | no | calendar date |
| `start_at`, `end_at` | no | normalized UTC timestamps |
| `input_start_timestamp`, `input_end_timestamp` | no | input timestamp receipts |
| `timezone_offset_text` | no | preserved offset text |
| `timezone_offset_meaning` | with offset text | offset-meaning enum |
| `timezone_offset_missing_reason` | no | missing-reason enum |

At least one of `provider_period_id`, `provider_date`, or `start_at` is required.
An end cannot precede a start.

The period types are:

- `whoop_physiological_cycle`
- `oura_activity_day`
- `oura_sleep_day`
- `oura_calendar_day`
- `oura_day_unspecified`
- `apple_activity_summary_day`

These values must not be renamed to a generic day or silently combined with
midnight-to-midnight calendar totals.

### 5.4 Measured record fields

The six measured record types are `point_measurement`, `measurement_sample`,
`sleep_session`, `workout_session`, `benchmark_result`, and `provider_score`.
Their `data` objects all include:

| Field | Required | Type |
|---|---|---|
| `source_id` | yes | source record UUIDv7 |
| `provider_record_id` | no | nonempty source record ID |
| `provider_reporting_periods` | no | provider reporting period array |
| `received_at` | yes | normalized UTC timestamp |
| `received_at_basis` | yes | `provider_supplied`, `connector_received_time`, `file_modified_time`, `import_time`, or `local_write_time` |

The source foreign key is required. A source of type `manual` marks a hand-entered
measurement or benchmark result. It does not turn that record into device evidence.

### 5.5 Source values and summaries

A source value has:

| Field | Required | Type |
|---|---|---|
| `standard_value` | yes | number |
| `standard_unit` | yes | nonempty string |
| `input_value` | when supplied by source | number |
| `input_value_missing_reason` | when input value is absent | missing-reason enum |
| `input_unit` | when supplied by source | nonempty string |
| `input_unit_missing_reason` | when input unit is absent | missing-reason enum |

Exactly one of each input value and its missing reason is present. The same rule
applies to the input unit.

A source-reported summary adds a required nonempty `summary_type`. Sleep sessions,
workout sessions, and workout segments use arrays of these summaries. A provider
summary remains a source fact even when samples also exist.

### 5.6 Score

A score is an object with at least one of:

| Field | Type |
|---|---|
| `duration_s` | positive number |
| `reps` | positive integer |
| `weight_kg` | positive number |

## 6. The 23 record types

### 6.1 `athlete_profile`

| Data field | Required | Type |
|---|---|---|
| `name` | no | nonempty string |
| `birth_year` | no | integer, 1900 through 2100 |
| `sex` | no | `male` or `female` |
| `display_units` | no | `metric` or `imperial` |
| `default_time_zone` | no | valid IANA timezone |

Stored measurement units do not change with `display_units`.

### 6.2 `source`

| Data field | Required | Type |
|---|---|---|
| `source_type` | yes | `wearable`, `export_file`, `connector`, or `manual` |
| `data_writer` | yes | nonempty string |
| `data_route` | yes | nonempty string |
| `vendor` | no | nonempty string |
| `sensor_type` | no | nonempty string |
| `detail` | no | nonempty string |

SQLite treats `data_writer`, `data_route`, and the optional `sensor_type` as the
source key. Two routes remain separate even when the writer is the same.

### 6.3 `source_device`

| Data field | Required | Type |
|---|---|---|
| `source_id` | yes | source UUIDv7 |
| `name`, `manufacturer`, `model`, `hardware`, `serial_number` | no | nonempty strings |
| `coverage_start_date`, `coverage_end_date` | no | calendar dates |
| `accepted_record_count` | no | nonnegative integer |

A source device is independently revisioned. Descriptive hardware does not split a
source when writer, route, and sensor type remain the same.

### 6.4 `point_measurement`

In addition to the measured fields:

| Data field | Required | Type |
|---|---|---|
| `quantity` | yes | point quantity |
| `recorded_at` | no | normalized UTC timestamp |
| `input_timestamp` | yes | input timestamp receipt |
| `value` | yes | source value |
| `calculations` | no | point calculation array |

A point calculation has required `from_quantity`, `method`,
`calculation_version`, and `input_records`. Optional fields are `window_s`,
`usable_sample_count`, `dropped_sample_count`, and `signal_quality`.

| Quantity | Standard unit |
|---|---|
| `hrv_rmssd`, `hrv_sdnn` | `ms` |
| `resting_heart_rate`, `hr_recovery`, `walking_heart_rate` | `bpm` |
| `body_weight`, `lean_body_mass` | `kg` |
| `respiratory_rate` | `brpm` |
| `vo2_max` | `ml/kg/min` |
| `oxygen_saturation`, `body_fat_percentage` | `%` |
| `body_temperature`, `skin_temperature`, `wrist_temperature_sleeping`, `temperature_deviation` | `°C` |
| `height` | `cm` |
| `blood_pressure_systolic`, `blood_pressure_diastolic` | `mmHg` |

`temperature_deviation` may be zero or negative. Other point values must be
positive. RMSSD and SDNN are different quantities and never share a baseline.

### 6.5 `measurement_sample`

In addition to the measured fields:

| Data field | Required | Type |
|---|---|---|
| `quantity` | yes | sample quantity |
| `recorded_at` | yes | normalized UTC timestamp |
| `input_timestamp` | yes | input timestamp receipt |
| `value` | yes | source value |
| `duration_ms` | no | nonnegative integer |
| `timestamp_base_text` | for reconstructed samples | nonempty string |
| `timestamp_reconstruction_method` | for reconstructed samples | nonempty string |

The reconstruction fields are both present only when
`input_timestamp.timestamp_basis` is `reconstructed_from_series_start`.

| Quantity | Standard unit |
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

Distances remain separate by activity. Samples from separate sources remain
separate.

### 6.6 `sleep_session`

In addition to the measured fields:

| Data field | Required | Type |
|---|---|---|
| `start_at`, `end_at` | yes | normalized UTC timestamps |
| `input_start_timestamp`, `input_end_timestamp` | yes | input timestamp receipts |
| `source_reported_summaries` | yes | source summary array |

`end_at` cannot precede `start_at`.

### 6.7 `workout_session`

In addition to the measured fields:

| Data field | Required | Type |
|---|---|---|
| `start_at`, `end_at` | yes | normalized UTC timestamps |
| `input_start_timestamp`, `input_end_timestamp` | yes | input timestamp receipts |
| `activity_type` | no | nonempty string |
| `source_reported_summaries` | yes | source summary array |
| `segments` | no | workout segment array |

A segment requires `label` and `source_reported_summaries`. It may have
`start_offset_ms` and `end_offset_ms`.

### 6.8 `benchmark_result`

In addition to the measured fields:

| Data field | Required | Type |
|---|---|---|
| `benchmark_definition_id` | yes | benchmark definition UUIDv7 |
| `recorded_at` | yes | normalized UTC timestamp |
| `input_timestamp` | yes | input timestamp receipt |
| `score` | yes | score object |
| `input_score_text` | no | nonempty source text |
| `scaling` | no | `rx` or `scaled` |
| `workout_session_id` | no | workout session UUIDv7 |
| `segments` | no | ordered result segments |

A result segment requires `label` and may contain a `score`. The optional workout
link is a stable UUID.

### 6.9 `provider_score`

In addition to the measured fields:

| Data field | Required | Type |
|---|---|---|
| `metric` | yes | nonempty provider metric |
| `value` | yes | source value |
| `scale` | yes | nonempty provider scale |
| `recorded_at` | no | normalized UTC timestamp |
| `input_timestamp` | yes | input timestamp receipt |

A provider score needs either `recorded_at` or at least one provider reporting
period. It stores a source-reported composite such as recovery, strain, readiness,
or sleep score. It is not a sensor measurement.

### 6.10 `athlete_report`

| Data field | Required | Type |
|---|---|---|
| `report_type` | yes | `sleep_quality`, `soreness`, `stress`, `mood`, `energy`, or `note` |
| `reported_at` | yes | normalized UTC timestamp |
| `input_reported_at` | yes | input timestamp receipt |
| `reported_via` | yes | `text`, `voice`, or `photo` |
| `rating` | no | number |
| `rating_scale` | with `rating` | nonempty string |
| `body_region` | no | nonempty string |
| `note` | no | nonempty string |
| `attachment_ids` | no | attachment UUIDv7 array |

This record has no source field. Unknown fields are rejected.

### 6.11 `nutrition_entry`

| Data field | Required | Type |
|---|---|---|
| `reported_at` | yes | normalized UTC timestamp |
| `input_reported_at` | yes | input timestamp receipt |
| `reported_via` | yes | `text`, `voice`, or `photo` |
| `athlete_description` | yes | nonempty string |
| `reported_items` | yes | reported nutrition item array |
| `attachment_ids` | no | attachment UUIDv7 array |

Each item requires `food_description`. It may have `amount`, `amount_unit`, and a
nutrient array. Each nutrient requires `nutrient_name`, numeric `amount`, and
`unit`. This record has no source field.

Model-inferred nutrition belongs in a separate
`nutrition_model_interpretation` record.

### 6.12 `attachment`

| Data field | Required | Type |
|---|---|---|
| `file_name` | yes | nonempty original name |
| `media_type` | yes | nonempty media type |
| `byte_count` | yes | nonnegative integer |
| `sha256` | yes | lowercase 64-character SHA-256 |
| `description` | no | nonempty string |

Binary bytes are sibling files. They are not embedded in record JSON.

### 6.13 `benchmark_definition`

This is an authored record.

| Data field | Required | Type |
|---|---|---|
| `authorship` | yes | authorship object |
| `benchmark_key` | yes | lowercase letters, digits, hyphens, and underscores |
| `benchmark_type` | yes | `named_wod`, `run`, `hyrox`, `lift`, or `custom` |
| `score_type` | yes | `time`, `reps`, or `load` |
| `definition` | yes | nonempty string |
| `tags` | no | nonempty string array |

### 6.14 `prediction`

This is an authored record.

| Data field | Required | Type |
|---|---|---|
| `authorship` | yes | authorship object |
| `benchmark_definition_id` | yes | benchmark definition UUIDv7 |
| `predicted_score` | yes | score |
| `predicted_range` | no | `{ "low": score, "high": score }` |
| `confidence` | yes | `low`, `moderate`, or `high` |
| `reasoning` | yes | nonempty string |

The prediction is a separate record from its result, link, grade, and later error
analysis.

### 6.15 `prediction_result_link`

| Data field | Required | Type |
|---|---|---|
| `prediction` | yes | exact evidence reference |
| `benchmark_result` | yes | exact evidence reference |
| `created_by` | yes | record actor |
| `linked_at` | yes | normalized UTC timestamp |
| `content_version` | yes | positive integer |
| `status` | yes | authored-record status |

The prediction and result must use the same benchmark definition.
A corrected result creates a new revision of this link. Existing grades keep their
earlier exact link revision.

### 6.16 `prediction_grade`

This is a tool-authored record.

| Data field | Required | Type |
|---|---|---|
| `authorship` | yes | authorship object |
| `prediction_result_link` | yes | exact link revision |
| `signed_error` | yes | number |
| `absolute_error_percentage` | yes | nonnegative number |
| `result_in_predicted_range` | yes | boolean |
| `predicted_score` | yes | copied exact predicted score |
| `actual_score` | yes | copied exact result score |

The stored scores and arithmetic must match the exact linked revisions and the
named calculation version.

### 6.17 `prediction_error_analysis`

This is an authored record.

| Data field | Required | Type |
|---|---|---|
| `authorship` | yes | authorship object |
| `prediction_grade` | yes | exact grade revision |
| `error_direction` | yes | `faster`, `slower`, `higher`, or `lower` |
| `error_severity` | yes | `minor`, `significant`, or `severe` |
| `causes` | yes | ordered cause array |
| `unexplained` | yes | boolean |
| `lesson` | no | nonempty string |

Each cause has an exact `evidence` reference and nonempty `explanation`. An analysis
with no causes must set `unexplained: true`.

### 6.18 `training_plan`

This is an authored record.

| Data field | Required | Type |
|---|---|---|
| `authorship` | yes | authorship object |
| `title` | yes | nonempty string |
| `objective` | no | nonempty string |
| `start_date`, `end_date` | no | calendar dates |

Storing a plan does not authorize the CLI or an agent to prescribe training.

### 6.19 `planned_session`

This is an authored record.

| Data field | Required | Type |
|---|---|---|
| `authorship` | yes | authorship object |
| `training_plan_id` | no | training plan UUIDv7 |
| `title` | yes | nonempty string |
| `scheduled_date` | conditionally | calendar date |
| `scheduled_start_at` | conditionally | normalized UTC timestamp |
| `activity_type` | no | nonempty string |
| `blocks` | yes | planned block array |
| `completion_status` | yes | `planned`, `completed`, `skipped`, or `cancelled` |
| `completed_workout_session_id` | no | workout session UUIDv7 |

At least one of `scheduled_date` and `scheduled_start_at` is required.

A block requires `block_type` and `title`. It may contain `instructions`,
`planned_duration_s`, `planned_distance_m`, `planned_reps`, and
`planned_weight_kg`.

The planned session owns the optional completion link. The measured workout does
not store a second link back.

### 6.20 `coaching_note`

This is an authored record.

| Data field | Required | Type |
|---|---|---|
| `authorship` | yes | authorship object |
| `title` | no | nonempty string |
| `note` | yes | nonempty string |
| `applies_from`, `applies_to` | no | calendar dates |

### 6.21 `saved_training_review`

This is an authored record.

| Data field | Required | Type |
|---|---|---|
| `authorship` | yes | authorship object |
| `period_start`, `period_end` | yes | calendar dates |
| `summary` | yes | nonempty string |
| `recommendations` | no | nonempty string array |

### 6.22 `goal`

This is an authored record.

| Data field | Required | Type |
|---|---|---|
| `authorship` | yes | authorship object |
| `title` | yes | nonempty string |
| `description` | no | nonempty string |
| `target_date` | no | calendar date |
| `target_benchmark_definition_id` | no | benchmark definition UUIDv7 |
| `target_score` | no | score |

### 6.23 `nutrition_model_interpretation`

This is an authored record.

| Data field | Required | Type |
|---|---|---|
| `authorship` | yes | authorship object |
| `nutrition_entry_id` | yes | nutrition entry UUIDv7 |
| `confidence` | yes | number from 0 through 1 |
| `interpreted_items` | yes | reported nutrition item array |
| `explanation` | no | nonempty string |

The athlete's original nutrition claim remains in the linked source-free
`nutrition_entry`.

## 7. SQLite encoding

### 7.1 Database metadata

`database_metadata` is the sole format metadata table:

| Column | Meaning |
|---|---|
| `application_marker` | `ATH1` |
| `sql_schema_version` | Positive SQL migration version |
| `athleticstandard_version` | `1.0.0` |
| `database_id` | Stable UUIDv7 database ID |
| `created_at` | Database creation timestamp |

`database_state.current_revision` stores the database-wide revision.

A local file uses SQLite application ID `0x41544831` and mirrors the SQL schema
version into `PRAGMA user_version`. `database_metadata` remains authoritative for
adapters that cannot use local file header values.

Every local connection enables:

```sql
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
```

Local files use WAL mode. Every athlete-data write uses one transaction.

### 7.2 Generic record tables

`records` stores `record_id`, `record_type`, `current_revision`, `created_at`, and
`is_deleted`.

`record_revisions` stores every complete revision as `record_json`, with its type,
number, prior revision, knowledge time, knowledge-time basis, and change actor.

`record_deletions`, `record_erasure_markers`, and `record_import_keys` encode the
lifecycle objects from section 4.

The current typed tables are:

| Record type | SQLite table |
|---|---|
| `athlete_profile` | `athlete_profile` |
| `source` | `sources` |
| `source_device` | `source_devices` |
| `point_measurement` | `point_measurements` |
| `measurement_sample` | `measurement_samples` |
| `sleep_session` | `sleep_sessions` |
| `workout_session` | `workout_sessions` |
| `benchmark_result` | `benchmark_results` |
| `provider_score` | `provider_scores` |
| `athlete_report` | `athlete_reports` |
| `nutrition_entry` | `nutrition_entries` |
| `attachment` | `attachments` |
| `benchmark_definition` | `benchmark_definitions` |
| `prediction` | `predictions` |
| `prediction_result_link` | `prediction_result_links` |
| `prediction_grade` | `prediction_grades` |
| `prediction_error_analysis` | `prediction_error_analyses` |
| `training_plan` | `training_plans` |
| `planned_session` | `planned_sessions` |
| `coaching_note` | `coaching_notes` |
| `saved_training_review` | `saved_training_reviews` |
| `goal` | `goals` |
| `nutrition_model_interpretation` | `nutrition_model_interpretations` |

Each typed table stores indexed fields in named columns and the complete current
logical `data` object in `data_json`. The typed row and latest revision are updated
in the same transaction.

Ordered nested values use child tables:

- `point_measurement_calculations`
- `point_measurement_calculation_inputs`
- `workout_session_segments`
- `benchmark_result_segments`
- `provider_reporting_periods`
- `reported_nutrition_items`
- `record_attachment_links`
- `authored_record_evidence`
- `prediction_error_causes`
- `planned_session_blocks`

These child rows belong to their parent record state. They do not get independent
record IDs. Correcting one creates a revision of the parent.

Foreign keys cover sources, exact evidence revisions, benchmark definitions,
prediction-result links, grades, attachments, plans, nutrition interpretations,
and workout completion links. Measured rows require a source foreign key.
Athlete-report and nutrition tables have no source column.

## 8. Portable JSON package

An export directory may contain:

```text
athlete.ath.json
series/
  <source-id>-<quantity>.ath-series.json
attachments/
  <stored attachment files>
export-manifest.json
```

The `series/` and `attachments/` directories are present only when needed.

### 8.1 `athlete.ath.json`

The main document has:

| Field | Type |
|---|---|
| `athleticstandard_version` | literal `1.0.0` |
| `database_id` | UUIDv7 |
| `database_revision` | nonnegative integer |
| `records` | every current non-sample record envelope |
| `record_revisions` | every retained non-sample revision |
| `record_deletions` | deletion objects for non-sample records |
| `record_erasure_markers` | all value-free erasure markers |
| `record_import_keys` | import keys for non-sample records |
| `measurement_series_files` | generated series file references |

A generated series file reference has:

| Field | Type |
|---|---|
| `file` | safe relative package path |
| `sha256` | lowercase SHA-256 |
| `source_id` | source UUIDv7 |
| `quantity` | sample quantity |
| `start_at`, `end_at` | normalized UTC bounds |
| `sample_count` | nonnegative integer |
| `revision_count` | nonnegative integer |
| `deletion_count` | nonnegative integer |
| `import_key_count` | nonnegative integer |

### 8.2 Generated measurement-series files

Each series file covers one current source and quantity. It has:

| Field | Type |
|---|---|
| `athleticstandard_version` | literal `1.0.0` |
| `source_id` | source UUIDv7 |
| `quantity` | sample quantity |
| `start_at`, `end_at` | normalized UTC bounds |
| `records` | current `measurement_sample` envelopes |
| `record_revisions` | all retained revisions for those samples |
| `record_deletions` | their deletion objects |
| `record_import_keys` | their provider import keys |

All revisions of a sample stay in the file chosen by its current source, quantity,
and time. Erasure markers stay in the main document because an erased sample no
longer has source, quantity, or time values.

### 8.3 Attachments

Every current non-deleted attachment record must have matching bytes in the
package. Export fails when the byte count or SHA-256 does not match. Attachment
files are listed in the manifest.

### 8.4 `export-manifest.json`

The manifest has:

| Field | Type |
|---|---|
| `athleticstandard_version` | literal `1.0.0` |
| `source_database_id` | UUIDv7 |
| `database_revision` | nonnegative integer |
| `exported_at` | optional normalized UTC timestamp |
| `files` | file entries |
| `record_counts` | one count entry per record type |
| `erasure_marker_count` | nonnegative integer |

Each file entry contains `file`, `sha256`, and `byte_count`.

Each record count contains `record_type`, `current_record_count`,
`revision_count`, `deletion_count`, and `import_key_count`.

The manifest is written last. Its `files` array lists every other exported package
file, not the manifest itself. `exported_at` is absent by default. The reference
writer uses UTF-8, two-space indentation, sorted object keys, stable array ordering,
and one final newline. Two exports of the same database revision with the same
explicit options are byte-identical.

### 8.5 Complete history rules

A complete package includes every revision from 1 through each current record's
`current_revision`. The latest revision data equals the current record data.

Validation refuses:

- missing or duplicate current records;
- missing, duplicate, or broken revision chains;
- a revision whose type differs from its current record;
- a deletion without its retained current record;
- disagreement between `is_deleted` and the deletion object;
- an import key without its current record;
- one import key resolving to two records;
- an erasure marker whose record still exists;
- duplicate or unsafe package paths.

The generated JSON Schemas give exhaustive object validation:

- [`schema/1.0.0/athleticstandard.schema.json`](schema/1.0.0/athleticstandard.schema.json)
- [`schema/1.0.0/measurement-series.schema.json`](schema/1.0.0/measurement-series.schema.json)
- [`schema/1.0.0/export-manifest.schema.json`](schema/1.0.0/export-manifest.schema.json)

## 9. Source separation and the two-tier wall

The two-tier wall is structural. Measured records require `source_id`.
`athlete_report` and `nutrition_entry` cannot contain it.

Authored records are a third class. They name an author and exact evidence. They are
not measurements and are not athlete reports.

Calculations remain separate by quantity and source unless the operation explicitly
compares sources. Device baselines exclude `manual` sources. RMSSD and SDNN never
share a calculation. Temperature quantities and activity-specific distances also
remain separate.

Provider scores and source-reported summaries are retained as facts reported by a
source. They do not replace the samples that support another question.

## 10. Import, export, and restore behavior

Provider and package imports preview before writing. `--yes` applies a
conflict-free preview in one transaction. `--json` may return the preview, but it
never acts as confirmation.

A v1 package import:

- preserves record IDs, creation times, revisions, knowledge times, import keys,
  and erasure markers;
- refuses unknown fields and records it cannot represent;
- does not delete a local record merely because the package omits it;
- refuses prerelease Athletic Standard versions below `1.0.0`.

An empty destination at database revision zero adopts the package database ID and
exact database revision. A nonempty destination may merge only a package with the
same database ID. A different database ID is refused.

One stable read transaction captures an export database revision. The package
contains athlete-owned records and permitted attachments. It excludes gateway keys,
connector grants, model preferences, unsaved backtest reports, accounts, billing,
and service operations.

## 11. Delete and erase

`ath delete` is normal deletion. It writes a deletion object and keeps record values
and revisions.

`ath erase` is privacy erasure. It previews the named record, linked dependents, and
attachments. Non-interactive use requires `--yes` and
`--confirm-erasure <record-id>`.

Erasure removes current values, value-bearing revisions, samples, dependent derived
records, and attachment files. It may leave only value-free erasure markers.

Erasure cannot promise removal from an earlier export, backup, filesystem snapshot,
SQLite journal, or storage-device history.

## 12. Security and integrity

The SQLite database and attachments contain sensitive health and training data.
Athletic Standard does not provide application-level encryption. Use
operating-system disk encryption. Anyone who can read the files can read the
athlete record.

A conforming implementation must:

- create databases, attachments, temporary files, and exports with owner-only
  permissions where supported;
- refuse to write health data through symbolic links;
- warn about existing paths readable by other local users;
- refuse a database directory writable by other local users;
- treat packages, provider exports, archives, attachments, photos, and existing
  databases as untrusted input;
- reject archive traversal, absolute archive paths, links, mismatched file types,
  unsupported images, and size-limit violations;
- use parameterized SQL and transactions;
- check SQLite integrity, foreign keys, schema versions, record rules, the
  two-tier wall, exact evidence, and attachment hashes.

The reference CLI applies these input limits:

- 512 MiB and 128 nested object or array levels for one package JSON file;
- 10,000 generated series files and 50 million records in one package;
- 100,000 entries, 16 GiB for one expanded entry, and 64 GiB expanded in one
  provider archive;
- a 1,000:1 maximum archive expansion ratio;
- 64 MiB for a provider CSV, route, or ECG file read into memory;
- 25 MiB for one directly logged attachment.

Large Apple Health XML remains streamed. A limit refusal writes no athlete data.

Do not copy an open WAL database as a backup. Use `ath export` or SQLite's online
backup operation. A closed, checkpointed database may be copied.

## 13. Version behavior

The format and package version are `1.0.0`. The portable document, generated series
files, export manifest, and live database metadata all carry that version.

Normative package and record data objects are strict. Unknown fields are rejected.
The v1 importer refuses Athletic Standard `0.x` documents without modifying them.
