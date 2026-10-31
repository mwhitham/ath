# Athletic Standard 2.1.0

Status: **approved by Mike and built in one pull request**. The decisions are in
[decisions.md](decisions.md), D177 through D187. What was built is in
[progress.md](progress.md).

This release builds the source mapping [D161](../v2.0.0/decisions.md) describes. A source whose reader already produces named fields is imported by adding a mapping file. The file lists their field and our field. A mapped field becomes the typed value. Any other field stays in `supplied_fields` under the name it arrived with.

`ath pull openwearables` is not this release. This release does not call Open Wearables or Junction.

The field table is [v2.0.0/appendix/source-formats.md](../v2.0.0/appendix/source-formats.md). Id, start, end, and nested stage paths were read from those public pages on 31 October 2026. The study's word `source.slug` is the description Junction gives `source.provider`. The shipped Junction file reads `source.provider`.

This plan was written in pull request 38. The build shipped as one later pull request.

## How 2.1.0 was delivered

One release, 2.1.0, in one pull request. Mike changed the plan's three pull requests to one. The plan's work for each part is still in that pull request, as separate commits, in this order:

| Commit | What lands | Decisions |
|---|---|---|
| Baseline | The comparison harness, the 2.0.0 database copy, and a 2.0.0 package exported from that copy, all from code that matches main `d418a8f653f7074ed9e08208c4eb555c391bcbc7` | none |
| Mapping | The engine, `ath import --mapping`, the fixtures, the pack checks, the calculation refusal | D177–D183, D186 |
| Version | Option A from section 8 | D184, D185 |
| Every value's basis | `field_basis` on every stored value, from every importer | D187 |
| Notes | The nits from the last review, the progress note in section 9, and this spec | none |

Nothing is published. Mike picked option A in section 8. Option B was not chosen. The tool skill `metadata.version` moves with the package version, to `2.1.0`, so `tests/skill.test.ts` still sees equal versions.

Mike also asked for three changes that override the plan where they conflict: one pull request, option A, and `field_basis` on every stored value, not only mapped values. The third change is D187 and section 13.

## 1. The shipped mapping files

A mapping file is markdown. The program reads the first fenced block whose info string is `ath-mapping`. Text outside that block is for a person. The grammar is D177. These are the two shipped files, exactly as built.

The header has more keys than the plan first listed. `sensor`, `device_name`, `device_model`, and `device_hardware` name the device fields (D183). They live in the file, so a mapping a person adds can keep two devices apart. `value_basis` gives the basis of every value the file writes (D187). The `type` line states the activity type's basis.

Shipped paths:

- `mappings/open-wearables.md`
- `mappings/junction.md`

`package.json` `files` gains `"mappings"`.

### `mappings/open-wearables.md`

~~~~markdown
# Open Wearables

Sleep sessions, workouts, and timeseries samples. The provider is `source.provider`. The device is `source.device_type`, `source.device`, and `source.source`. A value's basis is `provider_or_open_wearables`, because an Open Wearables object does not say whether the provider or Open Wearables computed it. A provider score's basis is `provider`. Stage minutes are under `stages`. `efficiency_percent` stays under that name. `entry_source` `manual` is left out. `intensity` stays under that name. A timeseries type is stored only when a line below names it.

```ath-mapping
name: open-wearables
route: open_wearables
writer_field: source.provider
value_basis: provider_or_open_wearables
sensor: source.device_type, source.device, source.source
device_name: source.device_name, source.device
device_model: source.device
device_hardware: source.device_id

[sleep]
id_field: id
start_field: start_time
end_field: end_time
sleep_duration_seconds: asleep_seconds s s
time_in_bed_seconds: in_bed_seconds s s
stages.awake_minutes: stage_minutes.awake min min
stages.light_minutes: stage_minutes.light min min
stages.deep_minutes: stage_minutes.deep min min
stages.rem_minutes: stage_minutes.rem min min

[workout]
id_field: id
start_field: start_time
end_field: end_time
type: activity_type basis open_wearables_normalized
name: name
distance_meters: source_reported_summaries.distance m m
elevation_gain_meters: source_reported_summaries.elevation_gain m m
avg_heart_rate_bpm: provider_summary.average_heart_rate bpm bpm basis provider_or_open_wearables
max_heart_rate_bpm: provider_summary.max_heart_rate bpm bpm basis provider_or_open_wearables

[timeseries]
time_field: timestamp
value_field: value
type.heart_rate_variability_rmssd: hrv_rmssd ms ms
type.heart_rate_variability_sdnn: hrv_sdnn ms ms
type.resting_heart_rate: resting_heart_rate bpm bpm
type.respiratory_rate: respiratory_rate brpm brpm
type.skin_temperature: skin_temperature celsius °C
type.skin_temperature: wrist_temperature_sleeping celsius °C when-writer apple
type.oxygen_saturation: oxygen_saturation percent %
type.heart_rate_recovery_one_minute: hr_recovery bpm bpm
type.vo2_max: provider_summary.vo2_max ml_kg_min ml/kg/min
type.garmin_stress_level: provider_score.garmin_stress_level score score scale 1-100
type.garmin_body_battery: provider_score.garmin_body_battery percent score scale 0-100
```
~~~~

`duration_seconds` stays in `supplied_fields`. `intensity` stays in `supplied_fields` under that name (D186). It is not a measured record. `name` is stored as the exact string. `activity_type` is stored as sent. The workout's `field_basis.activity_type` is `open_wearables_normalized`, because Open Wearables normalizes that word (D185). Its `field_basis.value` is `provider_or_open_wearables` (D187).

`avg_heart_rate_bpm` and `max_heart_rate_bpm` each become a `provider_summary`. The summary types are `average_heart_rate` and `max_heart_rate`. The basis stays `provider_or_open_wearables`, because a workout object has no flag that says who computed the number. Open Wearables does not send `{their_field}_computed_by`. That sibling rule is not part of this release. `field_basis.value` holds `provider_or_open_wearables`. `maximum_heart_rate_bpm` is never written (D172).

`hr_zones` is not a line. The object stays nested in `supplied_fields`. A line that aims `hr_zones.max_hr` at `maximum_heart_rate_bpm` is the D172 refusal in section 4.

A timeseries type with no line is `left_out`. It is not a measured record. That includes `dietary_*`, `workout_effort_score`, `estimated_workout_effort_score`, and the temperature-deviation types. This release does not check a vendor baseline for those deviations. `vo2_max` is a `provider_summary`, not the point quantity `vo2_max`.

`garmin_stress_level` and `garmin_body_battery` are `provider_score` records. The scale is on the line. Every stored score's writer is `source.provider`, whether or not `computed_by` is present. When a stored row has `computed_by`, that string stays verbatim in `supplied_fields`, and it never changes the writer. The field table says a provider score stays that provider's score. The `garmin_stress_level` sample has no `computed_by`. Its writer is `garmin`. Test `missing_computed_by_uses_the_provider`. The raw `garmin_body_battery` sample has no `computed_by`. Its writer is `garmin`. Aggregate rows are `left_out` (section 2). The device maker stays on the `source_device` row (D183).

Open Wearables `/health-scores` is not in this file. This release does not read that path.

### `mappings/junction.md`

~~~~markdown
# Junction

Sleep summaries. The provider is `source.provider`. The device is `source.type`, `source.device_id`, and `source.app_id`. A value's basis is `provider_or_junction`, because Junction calculates some summary numbers from samples. A provider score's basis is `provider`. `average_hrv` maps to `hrv_rmssd` only with basis `hrv_mean_rmssd`, and only when the writer is oura, whoop_v2, garmin, fitbit, or polar. `duration` stays under `duration`. `efficiency` stays under `efficiency`. There is no activity section.

```ath-mapping
name: junction
route: junction
writer_field: source.provider
value_basis: provider_or_junction
sensor: source.type, source.device_id, source.app_id
device_name: source.type
device_model: source.device_id
device_hardware: source.app_id

[sleep]
id_field: id
start_field: bedtime_start
end_field: bedtime_stop
stage_asleep_second: asleep_seconds s s
total: asleep_seconds s s when-absent
stage_awake_second: stage_minutes.awake s min
stage_light_second: stage_minutes.light s min
stage_rem_second: stage_minutes.rem s min
stage_deep_second: stage_minutes.deep s min
hr_resting: resting_heart_rate bpm bpm
respiratory_rate: respiratory_rate brpm brpm
average_hrv: hrv_rmssd ms ms basis hrv_mean_rmssd when-writer oura,whoop_v2,garmin,fitbit,polar
score: provider_score.sleep.score score score scale 1-100
recovery_readiness_score: provider_score.recovery_readiness_score score score scale 0-100
```
~~~~

There is no `[activity]` section. Junction activity summaries are dated at UTC midnight. [D88](../v1.0.0/decisions.md) says a date-only record does not receive an invented midnight timestamp. [D110](../v1.0.0/decisions.md) says a day total is not a measurement at one instant. An `activity` array in the input is `left_out`. It is not stored.

`total` writes `asleep_seconds` only when `stage_asleep_second` is missing or null. Otherwise `total` stays in `supplied_fields`.

The older stage fields `awake`, `light`, `rem`, and `deep` are not lines. A `0` there stays in `supplied_fields`. A null `stage_*_second` is omitted.

`score` is stored as `provider_score` metric `sleep.score`. `recovery_readiness_score` keeps that metric name. Junction has no `computed_by`. The writer is `source.provider`, the same rule as every other score. The field table says a provider score stays that provider's score. D27 keeps a vendor score as that vendor's composite. D159 says a score a device maker computed stays that maker's score. There is no `junction` writer value.

`hrv_mean_sdnn` is not on the summary object. This file has no line for it. A field literally named `hrv_mean_sdnn` may map to `hrv_sdnn`, because the name says SDNN.

`average_hrv` from any other writer stays in `supplied_fields` under `average_hrv`. Junction's provider slug for Apple Health is `apple_health_kit`. Health Connect's slug is `health_connect`. Samsung's slug is `samsung_health`. None of those is on the `when-writer` list. No `hrv_rmssd` point is written for them. The five names on the list are Junction `source.provider` slugs for providers whose summary this release treats as RMSSD. The `hrv_mean_rmssd` basis is the query-layer name in the study. A line that aims `average_hrv` at `hrv_sdnn` is refused (section 4).

## 2. How a mapping runs

The runner is new Rust in `crates/ath-core`. D155 put new imports there. D173 says the tests still have to pass. The Apple Health, WHOOP, Oura, Garmin, and Strava readers stay on their current functions.

### Types and functions

New module `crates/ath-core/src/mapping.rs`, declared from `lib.rs`.

```rust
pub struct MappingFile { /* name, route, writer_field, value_basis, sensor, device_name, device_model, device_hardware, sections */ }
pub struct MapLine { /* their_field, our_field, units, basis, scale, when_absent, when_writer */ }

pub fn parse_mapping(text: &str) -> Result<MappingFile, String>
pub fn check_mapping(file: &MappingFile) -> Result<(), String>
pub fn apply_mapping(store: &mut Store, file: &MappingFile, input: &Value, write: bool) -> Result<Value, String>
pub fn input_receipt(text: &str) -> Value
pub fn convert_unit(value: f64, their_unit: &str, our_unit: &str) -> Result<Value, String>
```

`parse_mapping` strips a trailing `\r` from each line, so a CRLF file parses the same as a LF file. Test `crlf_mapping_file_parses`.

There is no Rust `is_mapping_path`. `resolveMappingFile` in `src/skill.ts` resolves the name to an absolute path and passes that path as `--mapping-file`. Rust only reads the file. The path rule lives in TypeScript as `isMappingPath`, next to `resolveMappingFile`. It is true when the string contains `/` or `\`, or matches a drive prefix such as `C:`. `tests/mapping-path.test.ts` asserts `isMappingPath("C:\\mappings\\mine.md")` and `isMappingPath("mappings\\mine.md")`. The build adds a `windows-latest` job. The existing `verify` job stays on `ubuntu-latest`. The new job copies `verify`'s setup steps from `.github/workflows/ci.yml` lines 20–28, then runs the one test file:

```yaml
mapping-path:
  runs-on: windows-latest
  steps:
    - uses: actions/checkout@v5
    - uses: pnpm/action-setup@v4
      with:
        version: 10.33.3
    - uses: actions/setup-node@v4
      with:
        node-version: 22
        cache: pnpm
    - run: pnpm install --frozen-lockfile
    - run: pnpm exec vitest run tests/mapping-path.test.ts
```

`verify` uses a node matrix of 22 and 24. This job uses 22, the first of those two.

`main.rs` gains `import-mapped` with `--database`, `--mapping-file`, `--input`, and `--preview`.

`Store::ensure_mapped_source(writer, route, vendor, sensor_type)` looks up `data_writer`, `data_route`, and `sensor_type` together. It does not call the empty-sensor query in `ensure_source` (`store.rs` around line 175). Built-in importers keep calling `ensure_source`.

`Store::insert_group` writes every record from one source object in one transaction. Today `insert_record` opens a transaction and commits it (`store.rs` lines 229 and 259), and `revise` opens another and commits it (lines 94 and 169). `insert_group` does not call those wrappers. It opens one transaction, looks up `record_import_keys` the way `insert_record` does at lines 204–221, and compares with `json_equal`. `apply_mapping` copies the stored `received_at` onto the new JSON before that compare, so an unchanged re-run is equal and does not write. This is D182.

Today that lookup does not replace. At lines 218–219 `insert_record` calls `revise` with the whole new object as the patch. `revise` merges the patch into the current object (`store.rs` lines 82–84). A key the new object omits stays. The replace is a code change. `insert_group` writes the new object as the whole `record_json` and does not call `revise`. It refreshes typed columns for `sleep_session`, `workout_session`, `provider_summary`, `point_measurement`, `provider_score`, `source`, and `source_device`. `ath check` compares a sleep row's `start_at`, `end_at`, `provider_record_id`, and `received_at` (`src/db/records.ts` lines 288–294). A column whose key is absent is set to NULL. `revise` stays a merge for `ath-core revise --patch`. It also gains that column refresh for `sleep_session`, `point_measurement`, and `provider_score`, so a patch of those records updates the columns and not only `data_json`.

`typed_table` in `store.rs` gains `"provider_score" => Ok("provider_scores")` and `"source_device" => Ok("source_devices")`. `insert_typed` gains both arms. The `provider_score` columns are already in `src/db/migrations/001-initial.ts`. The `source_device` columns are `record_id`, `record_type`, `source_id`, `name`, `manufacturer`, `model`, `hardware`, `serial_number`, `coverage_start_date`, `coverage_end_date`, `accepted_record_count`, and `data_json`. The source insert at `store.rs` lines 315–316 writes `sensor_type` as SQL `NULL` and ignores the JSON. `ath check` compares that column with `record.data.sensor_type` through `typedMainRowFields` (`src/db/records.ts` line 253) and `mainRowMismatch` (`src/db/typed-denormalized.ts` lines 54–59). The source insert must bind `sensor_type` from the JSON, or `mapped_imports_pass_ath_check` fails. No new table and no new column.

### What a line writes

`check_mapping` allows:

- On a sleep session: `asleep_seconds`, `in_bed_seconds`, `interruptions`, `stage_minutes.awake`, `stage_minutes.light`, `stage_minutes.deep`, `stage_minutes.rem`.
- On a workout: `activity_type`, `name`, `description`, `text_result`, and `source_reported_summaries.distance`, `source_reported_summaries.elevation_gain`, `source_reported_summaries.energy`. Sleep summary types such as `sleep_duration` and `deep_sleep` are not workout targets.
- A point quantity in `POINT_MEASUREMENT_UNITS`. `hrv_rmssd` and `hrv_sdnn` also pass section 4. The our-unit must equal that quantity's unit in `POINT_MEASUREMENT_UNITS`.
- `provider_summary.` plus a non-empty summary type.
- `provider_score.` plus a metric. The metric may contain dots (`sleep.score`). The line includes `scale`.

A numeric target needs a unit pair. `activity_type`, `name`, `description`, and `text_result` take none. Our unit must be the field's own unit (D180): `s` for `asleep_seconds` and `in_bed_seconds`, `min` for `stage_minutes`, `count` for `interruptions`, `m` for a workout distance or elevation gain, `kcal` for energy, and the `POINT_MEASUREMENT_UNITS` value for a point.

`basis` is allowed on an `activity_type` line, on `hrv_rmssd` and `hrv_sdnn`, and on a `provider_summary` heart-rate line whose our unit is `bpm`. An `activity_type` line must have one: `open_wearables_normalized` or `provider_label` (D187). A value basis must be one of the `field_basis.value` values.

`start_at` and `end_at` are UTC with millisecond precision, via `crate::time::format_datetime`. `input_receipt` keeps the original text. A `Z` suffix uses offset text `Z`, meaning `documented_recording_offset`, basis `source_instant`. Any other numeric offset keeps that text, meaning `meaning_not_documented`, basis `source_offset_timestamp`. No new offset meaning is added.

`source_reported_summaries` is always an array on a sleep or workout. It is empty when no line wrote one. Each summary is a `SourceValue`. `received_at_basis` is `import_time`.

A point quantity, a `provider_summary`, or a `provider_score` from a session is a second record. The recorded instant for a child of a session is the session end in UTC. `received_at_basis` is `import_time`. That value is already in `ReceivedAtBasis` (`src/v1/schema.ts` lines 229–235). No new enum value is added. The child's `input_timestamp` is `input_receipt` of the session end's original text, the same object as the session's `input_end_timestamp`. A timeseries child's `input_timestamp` is `input_receipt` of the sample's `timestamp` string. A `Z` sample therefore stores:

```json
{
  "timestamp_text": "2026-08-09T06:12:00.000Z",
  "utc_offset_text": "Z",
  "utc_offset_meaning": "documented_recording_offset",
  "timestamp_basis": "source_instant"
}
```

A Junction child of `jn-sleep-1` stores `timestamp_text` `2026-08-09T10:00:00.000-04:00`, `utc_offset_text` `-04:00`, `utc_offset_meaning` `meaning_not_documented`, and `timestamp_basis` `source_offset_timestamp`. The child does not repeat the session's leftovers. The child stores `field_basis.value`: the line's `basis` when the line has one (D185), `provider` for a provider score, and otherwise the file's `value_basis` (D187). The session stores the file's `value_basis` too. `field_basis` is not inside `supplied_fields`.

A timeseries sample uses `time_field` and `value_field`. The sample's `type` selects `type.<that string>`. If the sample's `unit` differs from the line's their-unit, the sample is `left_out` and nothing is written for it:

```text
timeseries heart_rate_variability_rmssd 2026-08-09T07:00:00.000Z: unit bpm does not match ms
```

The import should be given the full timeseries response, envelope included. Two shapes are accepted.

An envelope is a JSON object with `data` (the sample array) and `metadata`. `pagination` may be present and is not mapped. An envelope's `data` is read as the timeseries section only. A `metadata.resolution` of null is treated as absent. At tag 0.10.0, commit `d18304f`, `PaginatedResponse` requires `data`, `pagination`, and `metadata` (`docs/openapi.json`, schema `PaginatedResponse[TimeSeriesSample]`). `TimeseriesMetadata.resolution` is the `Resolution` enum or null. `_page` sets `resolution=params.resolution` (`timeseries_service.py` lines 106–107). The sample object at that tag has no `window_seconds`. On current main, commit `8adb8d0`, `TimeseriesMetadata` is `resolution: Resolution | None` (`backend/app/schemas/utils/metadata.py` lines 28–29). It is the `metadata` field on the page (`backend/app/schemas/utils/pagination.py` line 35). `_page` fills it from `params.resolution` (`timeseries_service.py` lines 179–180). A sample on that page still has no `resolution` field (`data_point_responses.py` lines 22–37). The import does not invent one. Workout and sleep use that same page type: `list_workouts` returns `PaginatedResponse[Workout]` and `list_sleep_sessions` returns `PaginatedResponse[SleepSession]` (`backend/app/api/routes/v1/events.py` lines 59 and 134, on `8adb8d0` and on tag 0.10.0). Their metadata is `TimeseriesMetadata` with `sample_count`, `start_time`, and `end_time`, and `resolution` left null. At 0.10.0 that is `event_record_service.py` lines 953–957 for workouts and lines 1076–1080 for sleep. On main, workouts are lines 997–1001. Those pages are not timeseries pages. They are not read as envelopes. The person passes the `data` array under `workout` or `sleep` in the bare shape.

Bare rows are the other shape. That is the invented `summaries.json` object, with `sleep`, `workout`, and `timeseries` arrays, and any file whose root has no `metadata.resolution`.

If `metadata.resolution` is present, rows are accepted only when the value is `raw`. A null `metadata.resolution` is treated as absent. Any other value leaves every row in `data` out. The reason is `resolution {value}: not raw samples`. Nothing is written. Under a `raw` envelope, `window_seconds` is the sample interval. The row is stored, and `window_seconds` stays verbatim in `supplied_fields`.

The check reads the first row of the envelope's `data`. An Open Wearables page comes from one endpoint, so its rows share one shape. The page is a timeseries page when that row has `timestamp`, `type`, `value`, and `unit`, and has no `start_time`. That is `TimeSeriesSample` at tag 0.10.0 (`backend/app/schemas/responses/activity/data_point_responses.py` lines 11–20). `list_sleep_sessions` is `GET /users/{user_id}/events/sleep` (`events.py` lines 118–134 at 0.10.0). A `SleepSession` (`backend/app/schemas/responses/activity/events.py` lines 118–130 at 0.10.0, commit `d18304f8076a7710c48a4b279f8e8abd5a16887e`) has `start_time`, `end_time`, `duration_seconds`, `stages`, and `is_nap`. It has no `bedtime_start` and no `type`. Sleep and workouts both use `start_time`, so the first row is not labeled by guessing those names. Otherwise the whole envelope is refused. Nothing is written. `left_out` is exactly one line: `envelope is not a timeseries page: pass its data array under workout or sleep`. Inside a timeseries page, later rows still get the usual per-row reasons. An envelope whose `data` is empty writes nothing and adds no line, because there is nothing to leave out. Open Wearables returns an empty page for an empty window. Test `an empty envelope writes nothing and adds no left_out line` uses `tests/fixtures/exports/open-wearables/page-empty.json`.

If `metadata.resolution` is absent, a row with `window_seconds` set to any non-null value is `left_out`. The reason is `window_seconds present without resolution: cannot confirm raw`. Nothing is written for that row. That is the default refusal when resolution is missing. At 0.10.0 it does not fire: a sample has no `window_seconds`, and `_page` already sets `metadata.resolution` (`timeseries_service.py` lines 106–107). It catches envelope-stripped rows from current main, where a real raw row also carries `window_seconds`. Pass the full response.

On current main the bucket path (`timeseries_service.py` lines 344–355) sets `window_seconds` to the bucket width and does not pass `period_end` or `stats`. A 5-minute page from that server has `metadata.resolution` `5min`, so every row is left out. The column was added in `6a42ac4d` (`window_seconds: Mapped[int | None]` on the model). `4ff8f48` adds the ingest field. `cd45e77` puts it on the response. `4ff8f48`'s parent is `8e371cb`, and that commit's parent is `6a42ac4d`.

A sample with `period_end` is an aggregate. It is `left_out` in every case, envelope or not. The reason is `aggregate row, not a raw sample`. Nothing is written. On current main, `stats` is passed in the same call as `period_end` (`timeseries_service.py` lines 456–458), so a period row is already caught. `is_daily_total` is set only for a summed series (`daily_total_flag` in `aggregation_method.py` lines 178–191). None of the mapped types is a summed series, so that flag is not the check. If `stats` or `is_daily_total` is present on a row that is stored, it stays verbatim in `supplied_fields`. A stored score's writer is `source.provider`. When a stored row has `computed_by`, that string stays verbatim in `supplied_fields` and does not change the writer.

The id in that string is the UTC timestamp when the sample has no id.

### `supplied_fields`

Nested objects stay nested. A leaf that was used is removed. An object that becomes empty is dropped. The removal keeps the order of the remaining keys. Used means a mapping line consumed the leaf, or a section setting was read (`id_field`, `start_field`, `end_field`, `time_field`, `value_field`), or `writer_field` was read, or a device field named in the header was read for D183. In the shipped files those are `source.source`, `source.device`, `source.device_type`, `source.device_id`, `source.device_name`, `source.type`, and `source.app_id`.

`id`, `start_time`, `end_time`, `bedtime_start`, `bedtime_stop`, `timestamp`, `type`, `value`, `unit`, and `date` are not kept when they located the record or selected the timeseries line. `type` on a Junction sleep object is not that selector. It stays in `supplied_fields`.

A null mapped value is removed. It is not stored as zero, and it is not copied into `supplied_fields`.

`intensity` stays in `supplied_fields` verbatim. It is not a measured record and it is not a `left_out` line (D153, D161). `entry_source` `manual` drops the whole object (section 4).

Session leftovers sit on the session only. A timeseries sample's leftovers sit on that sample's record. The exact JSON for each fixture object is in section 6.

### Match key

D181. Their id is the match key. The method is `{route}/v1`. The key also carries the writer and the sensor string from D183, so two devices do not update one row.

`sensor_type` empty is written in the key as `-`.

| Record | `match_key` |
|---|---|
| Sleep or workout session | `{writer}:{sensor}:{section}:{id}` |
| Child point | `{writer}:{sensor}:{section}:{id}:point:{quantity}` |
| Child summary | `{writer}:{sensor}:{section}:{id}:summary:{summary_type}` |
| Child score | `{writer}:{sensor}:{section}:{id}:score:{metric}` |
| Timeseries point | `{writer}:{sensor}:timeseries:point:{type}:{utc}` |
| Timeseries summary | `{writer}:{sensor}:timeseries:summary:{type}:{utc}` |
| Timeseries score | `{writer}:{sensor}:timeseries:score:{metric}:{utc}` |
| Source device row | `{writer}:{sensor}:source_device` |

`{utc}` is the timestamp normalized to UTC with millisecond precision (`2026-08-09T06:12:00.000Z`), not the original offset text. `{metric}` for Junction's sleep score is `sleep.score`.

Each part is escaped before the parts are joined with `:`. A `\` in a part becomes `\\`. A `:` in a part becomes `\:`. That includes the colons inside `{utc}`. The sensor string can contain `|`. A stored timeseries key therefore looks like `oura:ring|invented-ring|invented-app:timeseries:point:heart_rate_variability_rmssd:2026-08-09T06\:12\:00.000Z`. Two objects in one file that produce the same match key do not both write. The first is stored. The second is `left_out` as `duplicate match key {key}`. A later import of a new file still updates that row (D182).

`provider_record_id` is their id when the object has one. A timeseries sample has no id.

### Re-run

D182. A second run uses the same method and key. Today `insert_record` looks up that row and compares with `json_equal` (`store.rs` lines 204–221). When the JSON differs it calls `revise` (lines 218–219). `revise` merges (`store.rs` lines 82–84), so a key the new object omits survives. The replace is a code change. `insert_group` does that lookup inside one transaction and writes the new object as the whole record. It does not call `revise`. Before the compare, `apply_mapping` sets `received_at` to the stored value. An unchanged import does not write a revision. A changed mapped value keeps the record id, increases `current_revision` by one, and keeps the first `received_at`. The sleep re-run test changes `end_at`, which `ath check` compares (`src/db/records.ts` lines 288–294), not only `asleep_seconds`.

### The source row

D183. `ensure_mapped_source(writer, route, vendor, sensor_type)` stores `source_type` `export_file`, `data_writer`, `data_route`, `vendor`, and `sensor_type`. The lookup is those three: writer, route, and sensor. A different sensor is a different source row and a different baseline, because baselines read `source_id`.

The header key `sensor` names the sensor fields. Empty parts are skipped, and the rest are joined with `|`. The Open Wearables file says `sensor: source.device_type, source.device, source.source`. The Junction file says `sensor: source.type, source.device_id, source.app_id`. A file with no `sensor` key has an empty sensor.

A `source_device` record is written once per source in one run, from the first object of that source. `device_name`, `device_model`, and `device_hardware` in the header fill it. The first present field in each list is used. The shipped files give:

| | Open Wearables | Junction |
|---|---|---|
| `name` | `source.device_name`, else `source.device` | `source.type` |
| `model` | `source.device` | `source.device_id` |
| `hardware` | `source.device_id` | `source.app_id` |
| `manufacturer` | `source.provider` | `source.provider` |

An Apple Watch, an iPhone, and a third-party app are three sensor strings, three source rows, and three baselines.

Every stored score's `data_writer` and `vendor` are `source.provider`. That holds whether or not `computed_by` is present. A stored row keeps `computed_by` verbatim in `supplied_fields`, and that string does not change the writer. Junction `sleep.score` and `recovery_readiness_score` have no `computed_by`. Their writer is `source.provider`. There is no `junction` writer value. The field table says a provider score stays that provider's score. `source_device.manufacturer` stays `source.provider`.

`ensure_source` is unchanged. Garmin's file import still uses writer `garmin` and route `garmin_account_export` with an empty sensor. A mapped Garmin watch uses route `open_wearables` or `junction` and a non-empty sensor. `check_mapping` rejects the built-in route names so a mapping cannot reuse those rows (D179).

### The TypeScript command

D174 stays. The command, its help, skill install, key storage, and language-model calls stay in TypeScript. They are not moved into `ath-core`. D174 says where the front end lives. Adding a flag in TypeScript does not need a new D-number.

```text
ath import <path> --mapping <name>
ath import <path> --mapping <name> --yes
ath import <path> --mapping <name> --json
```

`--mapping` is read before `detectSharedImport`. Without the flag, the package branch and the Apple, WHOOP, Oura, Garmin, and Strava branches run as they do now. With the flag, the path is not sent through those readers.

`ath pull` is unchanged. `ath pull openwearables` still fails with `pull currently reads garmin`.

`resolveMappingFile` returns a path. `runAthCore` is called with `import-mapped`. `--json` and a missing `--yes` pass `--preview`. `renderMappedImport` in `src/shared-import.ts` prints understood, kept under original names, left out, and records.

`fail` in `src/cli/runtime.ts` prints `ath: ` in front of the message. Rust prints the message alone. The tests assert both.

`src/help.ts` `EXAMPLES.import` shows `--mapping open-wearables`. `EXAMPLES.skill` says mapping files are copied into `.agents/mappings` when that copy is missing. Then `pnpm generate:skill-cli` rewrites `skills/athletic-standard/references/cli.md` (D80). The tool skill `metadata.version` and `ATH_PACKAGE_VERSION` are both `2.1.0`, because `tests/skill.test.ts` line 105 requires them to be equal. Calculation skill files stay at `2.0.0`.

`ath init` and `ath skill install --json` report the copied mapping files as `mappings_installed`.

The result JSON is:

```json
{
  "mapping": "open-wearables",
  "record_ids": [],
  "understood": [],
  "preserved": [],
  "left_out": []
}
```

On `--preview`, `record_ids` is empty and the database is unchanged.

An object that cannot be stored is one `left_out` string. Other objects are still written. A section 4 refusal exits 1 and writes nothing.

`left_out` reasons. Each line is `{section} {id}: {reason}`, or `timeseries {type} {utc}: {reason}` for a sample:

- `missing {writer_field}`, such as `missing source.provider`
- `missing {id_field}`, such as `missing id`
- `missing start_time` and `missing end_time`, or the section's own field names
- `{start_field} is not a timestamp with an offset`
- `end is before the start`
- `value is not a number`
- `unit {sample unit} does not match {their_unit}`
- `unmapped type {type}`
- `{quantity} must be positive`
- `entry_source manual is a personal report`
- `duplicate match key {key}`
- `the stored record {id} is deleted` (D182)
- `{key}: no mapping section`, such as `activity: no mapping section`, for an array at the root that the file has no section for
- `aggregate row, not a raw sample`
- `resolution {value}: not raw samples`
- `window_seconds present without resolution: cannot confirm raw`
- `envelope is not a timeseries page: pass its data array under workout or sleep`. Only an envelope whose first row is a timeseries sample is read. Other pages, such as Open Wearables workout and sleep pages, are not mapped as envelopes; pass their `data` array under `workout` or `sleep`
- `timeseries: no mapping section`, for a timeseries page when the file has no `[timeseries]` section

A root that is not a JSON object fails with `the input must be a JSON object`. A field whose value has the wrong type for its target, such as text for a number, is not mapped. It stays in `supplied_fields` under its name, and the object is still stored.

## 3. A mapping file a person adds

D178. `resolveMappingFile(name, cwd)`:

1. If `isMappingPath(name)` is true, it is a file path. A missing path is `no mapping file at <path>`. Rust is not asked to resolve the name.
2. Otherwise, if `.agents/mappings/<name>.md` exists, that file is used.
3. Otherwise `mappings/<name>.md` from `mappingRoot()` is used. `mappingRoot()` is the parent `skillsRoot()` already uses, with `mappings` in place of `skills`.
4. Otherwise `no mapping file named <name>`.

`installMappings(cwd)` copies each shipped file into `.agents/mappings/` only when the destination is missing. It does not replace a file. Callers:

- `ath init`, in the same branch as `installSkill` (`src/program.ts`). `ath init --no-skill` sets `opts.skill` false and skips `installSkill`. It skips `installMappings` too. Test `init_no_skill_skips_mapping_files`: `ath init -y --empty --no-skill` leaves `.agents/mappings` absent.
- `ath skill install`, just after `installSkill`.
- `ath --update`, in `src/update.ts`, where `installSkill` is already called. The same copy-if-missing rule. An existing mapping file is left as it is.

Mapping files are not `SKILL.md`. `skillNames` does not list them. `ath apply` does not offer them.

An installed file wins over the shipped file, including after a later shipped fix. That is accepted. Mappings do not get the tool-skill version replace from D164. D178 records that.

Test `installed_mapping_file_is_the_one_that_runs`: `.agents/mappings/open-wearables.md` with a `LastNightAvg` line aimed at `hrv_rmssd` makes the import fail with the section 4 message. A second `installMappings` does not change those bytes.

Test `person_adds_my_source`: write `.agents/mappings/my-source.md` with route `my_source` and `value_basis: provider`, one invented sleep object, `ath import <file> --mapping my-source --yes`. The sleep record exists, and its `field_basis` is `{ "value": "provider" }`. The source `data_route` is `my_source`. `ath skill` does not list `my-source`, and `ath apply my-source` fails with `no calculation skill named my-source`.

## 4. Refusals

`check_mapping` runs before a write. On a refusal the process exits 1, stderr is the message, and the record count is unchanged. `ath` prints `ath: ` plus the message.

There are four refusals. D179 lists them.

### A personal report

A measured record has a source. An athlete report has no `source_id`. The triggers in `src/db/migrations/enforcement-triggers.ts` already abort when one is present. This check runs first. Those triggers stay as published.

```text
refusing to turn a personal report into a measured record
```

`check_mapping` prints it when a section name is `athlete_report` or `nutrition_entry`, or when `our_field` is `source_id`, `report_type`, `reported_via`, or `rating`.

`apply_mapping` prints it when an input object contains `report_type` or `reported_via`. The whole file is refused.

Test `personal_report_cannot_become_a_measured_record`: object `{ "id": "report-1", "report_type": "sleep_quality", "reported_via": "text", "rating": 2, "note": "rough night" }` in a `sleep` array, shipped file. Exit 1. Stderr is that sentence. A database that already holds one record still holds one.

`tests/mapping-cli.test.ts` expects `ath: refusing to turn a personal report into a measured record`.

`entry_source` `manual` is not this refusal. The default is `left_out` for that object only. Section 11 asks Mike if the whole import should refuse instead. Test `manual_entry_source_is_left_out` uses `ow-workout-manual`. The object writes no record. `left_out` contains `workout ow-workout-manual: entry_source manual is a personal report`.

`intensity` is not a refusal and not a `left_out` line (D186). Test `intensity_stays_in_supplied_fields` uses `ow-workout-1`. `supplied_fields.intensity` is `moderate`. No measured field is named `intensity`. The precedent is Garmin `Feeling`, which `tests/v2-core.test.ts` line 54 keeps in `preserved`.

### An unlabeled heart-rate variability field

These their-field names may map to `hrv_rmssd` with no basis: `heart_rate_variability_rmssd`.

These may map to `hrv_sdnn` with no basis: `heart_rate_variability_sdnn`, `hrv_mean_sdnn`.

`average_hrv` may map to `hrv_rmssd` only with `basis hrv_mean_rmssd` and only when `when-writer` includes the writer. Any other their-field, including `LastNightAvg`, `hrv`, and `HRV`, is refused for both quantities.

```text
refusing to map LastNightAvg onto hrv_rmssd; the field does not name the statistic
```

```text
refusing to map LastNightAvg onto hrv_sdnn; the field does not name the statistic
```

The template is `refusing to map {their_field} onto {our_field}; the field does not name the statistic`.

Tests:

- `last_night_avg_cannot_become_rmssd`, file `tests/fixtures/mappings/last-night-rmssd.md`, line `type.LastNightAvg: hrv_rmssd ms ms`.
- `last_night_avg_cannot_become_sdnn`, file `tests/fixtures/mappings/last-night-sdnn.md`, line `type.LastNightAvg: hrv_sdnn ms ms`.
- `average_hrv_without_basis_cannot_become_rmssd`, line `average_hrv: hrv_rmssd ms ms`, message `refusing to map average_hrv onto hrv_rmssd; the field does not name the statistic`.
- `average_hrv_cannot_become_sdnn`, line `average_hrv: hrv_sdnn ms ms basis hrv_mean_rmssd`, message `refusing to map average_hrv onto hrv_sdnn; the field does not name the statistic`. The basis names RMSSD, so the line cannot write `hrv_sdnn`.

`average_hrv_with_basis_can_become_rmssd` accepts the shipped Junction line.

`pull_garmin` still stores `LastNightAvg` as a provider summary of that name.

### Their efficiency percentage

`asleep_seconds` divided by `in_bed_seconds` is the `sleep-efficiency` skill. A percentage they sent stays under their field name. The study says so at `appendix/source-formats.md` lines 29 and 39.

`our_field` `efficiency`, `efficiency_pct`, `sleep_efficiency`, `sleep-efficiency`, or `source_reported_summaries.sleep_efficiency` is refused.

```text
refusing to map efficiency_percent onto efficiency; efficiency is computed from asleep_seconds and in_bed_seconds
```

Test `efficiency_percent_cannot_map_onto_efficiency`, file `tests/fixtures/mappings/efficiency.md`, line `efficiency_percent: efficiency`.

On the Open Wearables night, `ath apply sleep-efficiency --record <ow-sleep-1> --json` returns this JSON. `record_id` is the imported session id. `600/700` in binary64 is `0.8571428571428571` (D180).

```json
{
  "calculation": "sleep-efficiency",
  "record_id": "<ow-sleep-1 record id>",
  "fields_read": [
    { "field": "asleep_seconds", "present": true },
    { "field": "in_bed_seconds", "present": true }
  ],
  "missing": [],
  "result": { "efficiency": 0.8571428571428571 }
}
```

On Junction `jn-sleep-1`, which has `asleep_seconds` and no `in_bed_seconds`, the same command returns:

```json
{
  "calculation": "sleep-efficiency",
  "record_id": "<jn-sleep-1 record id>",
  "fields_read": [
    { "field": "asleep_seconds", "present": true },
    { "field": "in_bed_seconds", "present": false }
  ],
  "missing": ["in_bed_seconds"],
  "result": null
}
```

`duration` is not read. `efficiency` in `supplied_fields` is not read.

### The athlete's known maximum

D172. `maximum_heart_rate_bpm` is the athlete's known maximum. A line aimed at it is refused.

```text
refusing to map max_heart_rate_bpm onto maximum_heart_rate_bpm; that field is the athlete's known maximum
```

Test `max_heart_rate_cannot_become_the_athlete_maximum`, file `tests/fixtures/mappings/athlete-maximum.md`, line `max_heart_rate_bpm: maximum_heart_rate_bpm bpm bpm`.

The same check refuses a zone peak:

```text
refusing to map hr_zones.max_hr onto maximum_heart_rate_bpm; that field is the athlete's known maximum
```

Test `zones_max_hr_cannot_become_the_athlete_maximum`, line `hr_zones.max_hr: maximum_heart_rate_bpm bpm bpm`.

### Other check failures

These are not the four refusals. They still fail the file before a write, except the `left_out` rows, which skip one object and continue.

- No fence: `the mapping file has no ath-mapping block`
- A fence that never closes: `the ath-mapping block is not closed`
- A bad line, including an unknown header key: `could not read '{line}'`
- A missing header key: `the mapping file needs {key}`, for `name`, `route`, `writer_field`, or `value_basis`
- A `value_basis` outside `provider`, `provider_or_open_wearables`, and `provider_or_junction`: `value_basis {token} is not one of provider, provider_or_open_wearables, provider_or_junction`
- Unknown section: `unknown mapping section {name}`
- Unknown our-field: `unknown field {our_field}`
- Two `when-absent` lines for one our-field: `two when-absent lines write {our_field}`
- Two lines without `when-writer` for one their-field: `two lines read {their_field} without when-writer`
- `basis` on any other field: `basis is only used for activity_type, hrv_rmssd, hrv_sdnn, and heart-rate summaries`
- A `basis` token that is not a `field_basis` value: `unknown basis {token}`
- An `activity_type` line with no basis: `activity_type needs a basis: one of open_wearables_normalized, provider_label`
- A unit on a text field: `{our_field} takes no unit`
- A numeric field with no unit pair: `{our_field} needs a unit pair`
- `scale` on anything except `provider_score`: `scale is only used for provider_score`
- `provider_score` without `scale`: `provider_score {their_field} needs a scale`
- An our-unit that is not the field's own unit: `{our_field} uses {unit}`, such as `oxygen_saturation uses %`
- A unit pair outside D180: `cannot convert {their_unit} to {our_unit} for {their_field}`
- A built-in route: `route {route} is already used by a built-in importer`
- A built-in method: `match key method {route}/v1 is already used by a built-in importer`

Built-in routes: `garmin_account_export`, `garmin_personal_api`, `strava_bulk`, `apple_health`, `whoop_csv`, `oura_csv`, `manual_entry`, `unknown_import`. `manual_entry` and `unknown_import` are the routes `sourceData` writes in `src/db/provider-import.ts` lines 261–268.

Built-in methods: `garmin_account/v1`, `garmin_personal/v1`, `strava_bulk/v1`, `apple_health/v1`, `whoop_csv/v1`, `oura_csv/v1`.

Test `builtin_route_is_rejected` uses route `garmin_account_export`.

## 5. Decisions

The decisions as adopted are in [decisions.md](decisions.md). Numbers continue from D176. D152 through D176 are not rewritten. The plan's drafts of D177 through D186 were copied there and then corrected to match the build. That file is the record. This section only lists them.

- D177. A mapping file is an `ath-mapping` block. Its header names the device fields, and from D187 the value basis.
- D178. Shipped mappings live in `mappings/`. A person's copy lives in `.agents/mappings/` and is never replaced.
- D179. The installed file is read first, and it is checked before a write. The four refusals.
- D180. Units convert by a fixed pair, and our unit is the field's own unit.
- D181. The match key is the writer, the sensor, their id, and the kind. Every part is escaped.
- D182. A re-run updates the same row by replacing the stored object. A deleted record is not revised.
- D183. The source names the device and the path.
- D184. The program and the record stay one number, 2.1.0. Option A.
- D185. A basis we assign is not a supplied field. `field_basis` for mapped values.
- D186. `intensity` stays in `supplied_fields`, and no calculation reads it or `Feeling`.
- D187. Every stored value states its basis, from every importer, mapping file, and `ath log`. Migration step 4 does not backfill.

D184 picks option A. Option B, two numbers, was not chosen, so no D185 stub was written.

## 6. Acceptance tests

Every commit in the pull request runs:

```text
pnpm check:public-data
pnpm typecheck
pnpm test
pnpm build
cargo test --manifest-path crates/ath-core/Cargo.toml
```

CI also runs `pnpm check:schema-drift`, and:

```text
npm pack
node scripts/assert-packed-binary.mjs athleticstandard-<version>.tgz
node scripts/smoke-installed-package.mjs
```

`<version>` is the version in `package.json`, `2.1.0` at the end of the pull request. The smoke script already packs a tarball. It gains two checks: the tarball lists `package/mappings/open-wearables.md` and `package/mappings/junction.md`, and the installed `ath` runs `ath import <Open Wearables fixture> --mapping open-wearables --yes` against the fixture in the repo.

Those pack commands were not steps in `.github/workflows/ci.yml`. `pnpm check:schema-drift` was not a step there either. The build adds them to the `package-smoke` job, after `pnpm build`. `npm pack` runs after the step that marks the binaries executable:

```yaml
- run: pnpm check:schema-drift
- run: npm pack
- run: node scripts/assert-packed-binary.mjs "athleticstandard-$(node -p "require('./package.json').version").tgz"
```

The `package-smoke` job already runs `node scripts/smoke-installed-package.mjs` (`.github/workflows/ci.yml`). The build does not add that step again. The tarball name is read from `package.json`. It is `athleticstandard-2.1.0.tgz`, with no hardcoded version in the workflow.

`cargo test --manifest-path crates/ath-core/Cargo.toml` is added to the `ath-core-binary` job, which already runs on `ubuntu-latest`, `macos-latest`, and `windows-latest`. The CRLF parse test runs there. The Windows path test is the TypeScript job in section 2, not a Rust helper.

The baseline test does not read the program version. After D187 it compares each import with `field_basis` removed, so the d418a8f files stay byte for byte, and it compares `field_basis` with a second set of committed files (below).

### Baseline from `d418a8f`

The first commit of the pull request has the same code as `d418a8f653f7074ed9e08208c4eb555c391bcbc7`. It adds only the baseline. It does not edit a reader. `tests/import-baseline.test.ts` contains:

```text
BASELINE_FROM = d418a8f653f7074ed9e08208c4eb555c391bcbc7
```

The test runs each importer, builds the comparison string, and asserts that the string is byte-for-byte the committed file. It does not write the file. The committed files are not regenerated in this release. Each file records `baseline_from`, so changing `BASELINE_FROM` fails the test.

The comparison string is UTF-8 JSON with object keys sorted. The walk is:

1. Read every record, every route point in `point_order`, every attachment, every attachment link, and every record link.
2. Delete every key named `received_at` or `imported_at` at any depth, including `received_at` inside record JSON such as a Garmin `max_heart_rate` summary.
3. Delete `created_at` and the database id.
4. Sort records that have a match key by `match_key_method`, then `match_key`, then `record_type`. A provider import key contains its source's random record id. Before the sort, each record id inside a key is replaced by that record's type and its own keys, so the order does not depend on random ids.
5. Sort records with no match key after those, by `record_type`, then by the JSON of the record after ids and the deleted time fields are removed.
6. Assign `record:1`, `record:2`, and so on, in that order. Replace every record id at any depth, including `source_id`, link endpoints, attachment ids, and route-point workout ids.
7. Sort attachments by `sha256`. Sort links by the normalized endpoint labels.

| Comparison file | What is imported |
|---|---|
| `01-apple-dir.json` | `tests/fixtures/exports/apple` |
| `02-apple-inline.json` | `tests/fixtures/exports/apple-inline-route` |
| `03-apple-file.json` | `tests/fixtures/exports/apple-file-reference` |
| `04-apple-mismatch.json` | `tests/fixtures/exports/apple-route-mismatch` |
| `05-cycles.json` | `tests/fixtures/exports/whoop` |
| `06-ring.json` | `tests/fixtures/exports/oura` |
| `07-ring-portal.json` | `tests/fixtures/exports/oura-member-portal` |
| `08-ring-offset.json` | `tests/fixtures/exports/oura-offset/sleep.csv` |
| `09-ring-daily.json` | `tests/fixtures/exports/oura-portal` |
| `10-ring-spo2.json` | `tests/fixtures/exports/oura-spo2` |
| `11-park-loop.json` | `tests/fixtures/exports/garmin/park-loop.json` |
| `12-namespaced-hr.json` | `tests/fixtures/exports/garmin/namespaced-hr.gpx` |
| `13-position.json` | `tests/fixtures/exports/garmin/synthetic-position.fit` |
| `14-bulk.json` | `tests/fixtures/exports/strava` |
| `15-personal-day.json` | `ath pull garmin` against a local server |
| `16-package.json` | `tests/fixtures/version-2-0-0/package`, the complete package `ath export` wrote from the 2.0.0 database copy at `d418a8f` |
| `17-apple-zip.json` | `tests/fixtures/exports/apple` zipped at test time under `apple_health_export/` |
| `18-apple-bare-xml.json` | `tests/fixtures/exports/apple/export.xml` as a file |

The personal-day server follows the local server in `personal.rs`. The HRV body includes `LastNightAvg`. The comparison asserts `summary_type` `LastNightAvg`. File names do not contain `export`, `athlete`, `garmin`, `oura`, `whoop`, `healthkit`, or `apple-health`.

Row 16 changed during the build. The plan named `examples/demo-athlete/athlete.ath.json`, but that file is the legacy 0.4.3 document. `ath import` refuses it with `Athletic Standard 0.4.3 is a prerelease JSON format`, so it never reaches the package restore. The row instead imports a complete 2.0.0 package: `athlete.ath.json`, `export-manifest.json`, the attachment, and the series file that `ath export` wrote from a copy of the 2.0.0 database, using code that matches `d418a8f`. Those files are in `tests/fixtures/version-2-0-0/package/` and are registered in `scripts/check-public-data.ts`. Under 2.1.0 the same package still imports (D184).

### The field basis files

D187 adds `field_basis` to every measured record an importer writes. The comparison above removes that one key before it compares. A second file for each row, `tests/fixtures/import-baseline/field-basis/<same name>`, lists every record that has `field_basis`, under the same labels, with its exact object. The test asserts that string byte for byte too. It also asserts that every measured record has `field_basis.value`, that no other record has `field_basis`, and that the restored 2.0.0 package (row 16) has none.

### The 2.0.0 database

Copy `examples/demo-athlete/athlete.ath.sqlite` at `d418a8f` to `tests/fixtures/version-2-0-0/database.ath.sqlite`. The sha256 is `442e9a966711660c2a03d701b5de2c55479a78330702083a3a67a182992a7856`. Register that path and hash in `scripts/check-public-data.ts`. Add this line to `.gitignore`, and the same for the package's `athlete.ath.json` and `export-manifest.json`:

```text
!tests/fixtures/version-2-0-0/database.ath.sqlite
```

The committed file stays those bytes. Tests copy it to a temp directory and migrate the copy.

The database id is `01890f3e-e7d3-7cc0-88c8-000000000001`. At `d418a8f` it has `athleticstandard_version` `2.0.0`, `sql_schema_version` `3`, 25 `records`, 26 `record_revisions`, 1 `record_attachment_links`, 1 `attachments`, and 1 `record_deletions`.

The acceptance commands, on the temp copy:

```text
ath migrate --file <copy> --json
ath migrate --yes --confirm-migration 01890f3e-e7d3-7cc0-88c8-000000000001 --file <copy>
ath check --file <copy>
```

The second command applies step 4. The copy then has `athleticstandard_version` `2.1.0` and `sql_schema_version` `4`. The five counts stay 25, 26, 1, 1, and 1. No record gains `field_basis` (D187). `ath check` exits 0. Test `migrate_version_2_database_to_schema_4` in `tests/version-2-1.test.ts` runs these commands on a copy, with the attachment copied beside it, and checks that the committed file keeps its hash.

Opening the file does not migrate it. Before the migration, `ath-core` refuses the copy with `database version is 2.0.0; run ath migrate before using ath-core`.

### New fixtures

All the files are invented. Dates are in 2026. Coordinates are invented. The build writes them, then adds each path and its sha256 to `approvedSyntheticFixtures`. The check did not ask for a `!` gitignore line for any of them.

`tests/fixtures/exports/open-wearables/summaries.json`:

```json
{
  "sleep": [
    {
      "id": "ow-sleep-1",
      "start_time": "2026-08-09T02:00:00.000-04:00",
      "end_time": "2026-08-09T10:00:00.000-04:00",
      "zone_offset": "-04:00",
      "source": {
        "provider": "oura",
        "source": "invented-app",
        "device": "invented-ring",
        "device_type": "ring",
        "device_id": "ring-1",
        "device_name": "Invented Ring"
      },
      "duration_seconds": 800,
      "sleep_duration_seconds": 600,
      "time_in_bed_seconds": 700,
      "efficiency_percent": 91,
      "stages": { "awake_minutes": 2, "light_minutes": 3, "deep_minutes": 4, "rem_minutes": null },
      "is_nap": false
    },
    {
      "id": "ow-nap-1",
      "start_time": "2026-08-09T18:00:00.000Z",
      "end_time": "2026-08-09T18:20:00.000Z",
      "source": {
        "provider": "oura",
        "source": "invented-app",
        "device": "invented-ring",
        "device_type": "ring",
        "device_id": "ring-1",
        "device_name": "Invented Ring"
      },
      "sleep_duration_seconds": 1000,
      "time_in_bed_seconds": 1200,
      "efficiency_percent": 80,
      "is_nap": true
    }
  ],
  "workout": [
    {
      "id": "ow-workout-1",
      "type": "running",
      "name": "Invented loop",
      "start_time": "2026-01-02T15:00:00.000Z",
      "end_time": "2026-01-02T15:00:03.000Z",
      "source": {
        "provider": "garmin",
        "source": "invented-app",
        "device": "invented-watch",
        "device_type": "watch",
        "device_id": "watch-1",
        "device_name": "Invented Watch"
      },
      "duration_seconds": 3,
      "distance_meters": 12,
      "elevation_gain_meters": 4,
      "avg_heart_rate_bpm": 140,
      "max_heart_rate_bpm": 180,
      "intensity": "moderate",
      "start_latitude": 12.345,
      "start_longitude": -45.678,
      "calories_kcal": 1,
      "hr_zones": { "max_hr": 190 }
    },
    {
      "id": "ow-workout-manual",
      "type": "running",
      "name": "Typed by hand",
      "entry_source": "manual",
      "start_time": "2026-08-09T12:00:00.000Z",
      "end_time": "2026-08-09T12:10:00.000Z",
      "source": { "provider": "garmin", "device_type": "watch", "device": "invented-watch", "source": "invented-app" }
    }
  ],
  "timeseries": [
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "heart_rate_variability_rmssd", "value": 42.5, "unit": "ms", "source": { "provider": "oura", "source": "invented-app", "device": "invented-ring", "device_type": "ring", "device_id": "ring-1", "device_name": "Invented Ring" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "heart_rate_variability_sdnn", "value": 61.2, "unit": "ms", "source": { "provider": "oura", "source": "invented-app", "device": "invented-ring", "device_type": "ring", "device_id": "ring-1", "device_name": "Invented Ring" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "resting_heart_rate", "value": 48, "unit": "bpm", "source": { "provider": "oura", "source": "invented-app", "device": "invented-ring", "device_type": "ring", "device_id": "ring-1", "device_name": "Invented Ring" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "respiratory_rate", "value": 16, "unit": "brpm", "source": { "provider": "oura", "source": "invented-app", "device": "invented-ring", "device_type": "ring", "device_id": "ring-1", "device_name": "Invented Ring" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "skin_temperature", "value": 33.2, "unit": "celsius", "source": { "provider": "oura", "source": "invented-app", "device": "invented-ring", "device_type": "ring", "device_id": "ring-1", "device_name": "Invented Ring" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "oxygen_saturation", "value": 98, "unit": "percent", "source": { "provider": "oura", "source": "invented-app", "device": "invented-ring", "device_type": "ring", "device_id": "ring-1", "device_name": "Invented Ring" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "heart_rate_recovery_one_minute", "value": 22, "unit": "bpm", "source": { "provider": "oura", "source": "invented-app", "device": "invented-ring", "device_type": "ring", "device_id": "ring-1", "device_name": "Invented Ring" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "vo2_max", "value": 45, "unit": "ml_kg_min", "source": { "provider": "oura", "source": "invented-app", "device": "invented-ring", "device_type": "ring", "device_id": "ring-1", "device_name": "Invented Ring" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "skin_temperature", "value": 33.4, "unit": "celsius", "source": { "provider": "apple", "source": "Health", "device": "Watch6,1", "device_type": "watch", "device_id": "watch-a", "device_name": "Invented Watch" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "garmin_stress_level", "value": 15, "unit": "score", "source": { "provider": "garmin", "source": "invented-app", "device": "invented-watch", "device_type": "watch", "device_id": "watch-1", "device_name": "Invented Watch" } },
    { "timestamp": "2026-08-09T06:13:00.000Z", "type": "garmin_body_battery", "value": 70, "unit": "percent", "source": { "provider": "garmin", "source": "invented-app", "device": "invented-watch", "device_type": "watch", "device_id": "watch-1", "device_name": "Invented Watch" } },
    { "timestamp": "2026-08-09T06:14:00.000Z", "type": "garmin_stress_level", "value": 18, "unit": "score", "window_seconds": 60, "source": { "provider": "garmin", "source": "invented-app", "device": "invented-watch", "device_type": "watch", "device_id": "watch-1", "device_name": "Invented Watch" } },
    { "timestamp": "2026-08-09T06:15:00.000Z", "type": "garmin_stress_level", "value": 20, "unit": "score", "period_end": "2026-08-09T07:15:00.000Z", "source": { "provider": "garmin", "source": "invented-app", "device": "invented-watch", "device_type": "watch", "device_id": "watch-1", "device_name": "Invented Watch" } },
    { "timestamp": "2026-08-09T08:00:00.000Z", "type": "resting_heart_rate", "value": 61, "unit": "bpm", "source": { "provider": "apple", "source": "Health", "device": "iPhone15,2", "device_type": "phone", "device_id": "phone-1", "device_name": "Invented Phone" } },
    { "timestamp": "2026-08-09T08:00:00.000Z", "type": "resting_heart_rate", "value": 52, "unit": "bpm", "source": { "provider": "apple", "source": "Health", "device": "Watch6,1", "device_type": "watch", "device_id": "watch-a", "device_name": "Invented Watch" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "dietary_caffeine", "value": 40, "unit": "mg", "source": { "provider": "oura", "device_type": "ring", "device": "invented-ring", "source": "invented-app" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "workout_effort_score", "value": 7, "unit": "score", "source": { "provider": "oura", "device_type": "ring", "device": "invented-ring", "source": "invented-app" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "skin_temperature_deviation", "value": 0.2, "unit": "celsius", "source": { "provider": "oura", "device_type": "ring", "device": "invented-ring", "source": "invented-app" } },
    { "timestamp": "2026-08-09T07:00:00.000Z", "type": "heart_rate_variability_rmssd", "value": 40, "unit": "bpm", "source": { "provider": "oura", "device_type": "ring", "device": "invented-ring", "source": "invented-app" } }
  ]
}
```

`tests/fixtures/exports/open-wearables/page-raw.json` is one page. `metadata.resolution` is `raw`. The one sample is `garmin_stress_level` at `2026-08-09T06:14:00.000Z`, value `18`, unit `score`, `window_seconds` `60`, on the same invented Garmin watch as `summaries.json`.

```json
{
  "data": [
    { "timestamp": "2026-08-09T06:14:00.000Z", "type": "garmin_stress_level", "value": 18, "unit": "score", "window_seconds": 60, "source": { "provider": "garmin", "source": "invented-app", "device": "invented-watch", "device_type": "watch", "device_id": "watch-1", "device_name": "Invented Watch" } }
  ],
  "pagination": { "has_more": false },
  "metadata": { "resolution": "raw" }
}
```

`tests/fixtures/exports/open-wearables/page-workouts.json` is an invented workouts page. `metadata.resolution` is null. The one row is a workout, not a timeseries sample.

```json
{
  "data": [
    { "id": "018f0000-0000-7000-8000-0000000000a1", "type": "running", "name": "Invented page loop", "start_time": "2026-08-09T15:00:00.000Z", "end_time": "2026-08-09T15:10:00.000Z", "source": { "provider": "garmin", "source": "invented-app", "device": "invented-watch", "device_type": "watch", "device_id": "watch-1", "device_name": "Invented Watch" } }
  ],
  "pagination": { "has_more": false },
  "metadata": { "resolution": null, "sample_count": 1, "start_time": "2026-08-09T15:00:00.000Z", "end_time": "2026-08-09T15:10:00.000Z" }
}
```

`tests/fixtures/exports/open-wearables/page-sleep.json` is an Open Wearables 0.10.0 sleep page. The row has `start_time`, `end_time`, `duration_seconds`, `stages`, and `is_nap`. It has no `bedtime_start` and no `type`. `metadata.resolution` is null.

```json
{
  "data": [
    { "id": "018f0000-0000-7000-8000-0000000000b2", "start_time": "2026-08-09T06:00:00.000Z", "end_time": "2026-08-09T14:00:00.000Z", "duration_seconds": 28800, "stages": { "awake_minutes": 2, "light_minutes": 3, "deep_minutes": 4 }, "is_nap": false, "source": { "provider": "garmin", "source": "invented-app", "device": "invented-watch", "device_type": "watch", "device_id": "watch-1", "device_name": "Invented Watch" } }
  ],
  "pagination": { "has_more": false },
  "metadata": { "resolution": null, "sample_count": 1, "start_time": "2026-08-09T06:00:00.000Z", "end_time": "2026-08-09T14:00:00.000Z" }
}
```

`tests/fixtures/exports/open-wearables/page-null-resolution.json` is a timeseries page with `metadata.resolution` null. The first row has `window_seconds` `60`. The second row does not.

```json
{
  "data": [
    { "timestamp": "2026-08-09T06:14:00.000Z", "type": "garmin_stress_level", "value": 18, "unit": "score", "window_seconds": 60, "source": { "provider": "garmin", "source": "invented-app", "device": "invented-watch", "device_type": "watch", "device_id": "watch-1", "device_name": "Invented Watch" } },
    { "timestamp": "2026-08-09T06:17:00.000Z", "type": "garmin_stress_level", "value": 12, "unit": "score", "source": { "provider": "garmin", "source": "invented-app", "device": "invented-watch", "device_type": "watch", "device_id": "watch-1", "device_name": "Invented Watch" } }
  ],
  "pagination": { "has_more": false },
  "metadata": { "resolution": null }
}
```

`tests/fixtures/exports/open-wearables/page-empty.json` is an empty `raw` page: `{ "data": [], "pagination": { "has_more": false }, "metadata": { "resolution": "raw" } }`. The import writes nothing and `left_out` is empty.

`tests/fixtures/exports/open-wearables/page-5min.json` is a 5-minute bucket. The row starts at `2026-08-09T06:10:00.000Z` and `window_seconds` is `300`, which is the width that path sets. `metadata.resolution` is `5min`.

```json
{
  "data": [
    { "timestamp": "2026-08-09T06:10:00.000Z", "type": "garmin_stress_level", "value": 18, "unit": "score", "window_seconds": 300, "source": { "provider": "garmin", "source": "invented-app", "device": "invented-watch", "device_type": "watch", "device_id": "watch-1", "device_name": "Invented Watch" } }
  ],
  "pagination": { "has_more": false },
  "metadata": { "resolution": "5min" }
}
```

`tests/fixtures/exports/junction/summaries.json`:

```json
{
  "sleep": [
    {
      "id": "jn-sleep-1",
      "bedtime_start": "2026-08-09T02:00:00.000-04:00",
      "bedtime_stop": "2026-08-09T10:00:00.000-04:00",
      "duration": 700,
      "total": 500,
      "stage_asleep_second": 600,
      "stage_awake_second": 120,
      "stage_light_second": 180,
      "stage_rem_second": null,
      "stage_deep_second": 240,
      "awake": 0,
      "light": 0,
      "rem": 0,
      "deep": 0,
      "efficiency": 0.88,
      "hr_resting": 47,
      "average_hrv": 55.5,
      "LastNightAvg": 40,
      "respiratory_rate": 14,
      "score": 80,
      "recovery_readiness_score": 70,
      "type": "long_sleep",
      "source": { "provider": "oura", "type": "ring", "device_id": "ring-j1", "app_id": "oura-app" }
    },
    {
      "id": "jn-sleep-2",
      "bedtime_start": "2026-08-10T02:00:00.000Z",
      "bedtime_stop": "2026-08-10T03:00:00.000Z",
      "duration": 3600,
      "total": 1000,
      "stage_asleep_second": null,
      "source": { "provider": "oura", "type": "ring", "device_id": "ring-j1", "app_id": "oura-app" }
    },
    {
      "id": "jn-nap-1",
      "bedtime_start": "2026-08-09T19:00:00.000Z",
      "bedtime_stop": "2026-08-09T19:15:00.000Z",
      "duration": 900,
      "total": 800,
      "stage_asleep_second": null,
      "type": "acknowledged_nap",
      "source": { "provider": "oura", "type": "ring", "device_id": "ring-j1", "app_id": "oura-app" }
    },
    {
      "id": "jn-sleep-apple",
      "bedtime_start": "2026-08-11T02:00:00.000Z",
      "bedtime_stop": "2026-08-11T03:00:00.000Z",
      "duration": 3600,
      "stage_asleep_second": 3000,
      "average_hrv": 33,
      "source": { "provider": "apple_health_kit", "type": "watch", "device_id": "watch-a", "app_id": "health" }
    }
  ],
  "activity": [
    {
      "id": "jn-activity-1",
      "date": "2026-08-09T00:00:00.000Z",
      "steps": 8432,
      "source": { "provider": "garmin" }
    }
  ]
}
```

`0.88` is an invented efficiency. It is not Junction's documentation example.

The bullets under the two fixture tests were written before `field_basis`. The `field_basis` checks follow them.

Test `open_wearables_fixture_maps_fields`. Sensor `ring|invented-ring|invented-app` is `S-ring`. Sensor `watch|invented-watch|invented-app` is `S-watch`. Sensor `phone|iPhone15,2|Health` is `S-phone`. Sensor `watch|Watch6,1|Health` is `S-awatch`.

- `ow-sleep-1` is its own `sleep_session`. `start_at` is `2026-08-09T06:00:00.000Z`. `end_at` is `2026-08-09T14:00:00.000Z`. The start receipt's `timestamp_text` is `2026-08-09T02:00:00.000-04:00`, `utc_offset_text` is `-04:00`, `utc_offset_meaning` is `meaning_not_documented`, and `timestamp_basis` is `source_offset_timestamp`.
- `asleep_seconds` is `600`. `in_bed_seconds` is `700`. `stage_minutes` is `{ "awake": 2, "light": 3, "deep": 4 }` with no `rem` key.
- `supplied_fields` is `{ "zone_offset": "-04:00", "duration_seconds": 800, "efficiency_percent": 91, "is_nap": false }`.
- Match key `oura:S-ring:sleep:ow-sleep-1`, method `open_wearables/v1`.
- The source is writer `oura`, route `open_wearables`, vendor `oura`, `source_type` `export_file`, `sensor_type` `S-ring`. The `source_device` name is `Invented Ring`, model `invented-ring`, hardware `ring-1`, manufacturer `oura`.
- `ow-nap-1` is a second `sleep_session`. It is not merged into `ow-sleep-1`. `supplied_fields` is `{ "efficiency_percent": 80, "is_nap": true }`. `asleep_seconds` is `1000`. `in_bed_seconds` is `1200`. Same source as the night.
- `ow-workout-1` source is writer `garmin`, route `open_wearables`, sensor `S-watch`. `activity_type` is `running`. `name` is `Invented loop`. `source_reported_summaries` holds distance `12` `m` and elevation gain `4` `m`. There is no `maximum_heart_rate_bpm`.
- `supplied_fields` on that workout is `{ "duration_seconds": 3, "intensity": "moderate", "start_latitude": 12.345, "start_longitude": -45.678, "calories_kcal": 1, "hr_zones": { "max_hr": 190 } }`.
- A `provider_summary` `average_heart_rate` is `140` `bpm`, match key `garmin:S-watch:workout:ow-workout-1:summary:average_heart_rate`. A `provider_summary` `max_heart_rate` is `180` `bpm`, key suffix `:summary:max_heart_rate`. Neither summary has `supplied_fields`.
- `ow-workout-manual` writes no record. `left_out` contains `workout ow-workout-manual: entry_source manual is a personal report`.
- Points at `2026-08-09T06:12:00.000Z` on the oura ring source: `hrv_rmssd` `42.5`, `hrv_sdnn` `61.2`, `resting_heart_rate` `48`, `respiratory_rate` `16` unit `brpm`, `skin_temperature` `33.2` unit `°C`, `oxygen_saturation` `98` unit `%`, `hr_recovery` `22` unit `bpm`. The skin-temperature sample's unit was `celsius`. The oxygen-saturation sample's unit was `percent`. Keys look like `oura:S-ring:timeseries:point:heart_rate_variability_rmssd:2026-08-09T06\:12\:00.000Z`, with every part escaped (section 2). Those samples have no `supplied_fields`. Each point's `input_timestamp` is the `Z` receipt in section 2. `asleep_seconds` `600` is the JSON integer `600`.
- The Apple watch sample at the same timestamp is `wrist_temperature_sleeping` `33.4` unit `°C`, writer `apple`, sensor `S-awatch`. The sample's unit was `celsius`. It is not `skin_temperature`.
- `vo2_max` is a `provider_summary` summary type `vo2_max`, value `45`, unit `ml/kg/min`, key suffix `:summary:vo2_max`. The sample's unit was `ml_kg_min`. It is not a point quantity `vo2_max`.
- `garmin_stress_level` is a `provider_score` metric `garmin_stress_level`, value `15`, scale `1-100`, writer `garmin`, sensor `S-watch`. The sample has no `computed_by`. `supplied_fields` does not contain that key. Test `missing_computed_by_uses_the_provider`. The writer is `source.provider` whether or not `computed_by` is present.
- `garmin_body_battery` is a `provider_score` metric `garmin_body_battery`, value `70`, scale `0-100`, writer `garmin`, vendor `garmin`, sensor `S-watch`. The sample's unit was `percent`. The stored unit is `score`. The sample has no `computed_by`. `source_device.manufacturer` is `garmin`.
- Test `raw_envelope_keeps_window_seconds`. Import `page-raw.json` with `--mapping open-wearables --yes`. `metadata.resolution` is `raw`. The `garmin_stress_level` sample is a `provider_score`, value `18`, scale `1-100`, writer `garmin`. `supplied_fields` is `{ "window_seconds": 60 }`. `left_out` has no line for that timestamp.
- Test `workout_page_is_not_a_timeseries_envelope`. Import `page-workouts.json` with `--mapping open-wearables --yes`. Nothing is written. `left_out` is exactly one line, `envelope is not a timeseries page: pass its data array under workout or sleep`.
- Test `sleep_page_is_not_a_timeseries_envelope`. Import `page-sleep.json` with `--mapping open-wearables --yes`. Nothing is written. `left_out` is exactly that same line.
- Test `null_resolution_is_treated_as_absent`. Import `page-null-resolution.json` with `--mapping open-wearables --yes`. The row at `2026-08-09T06:14:00.000Z` is left out: `timeseries garmin_stress_level 2026-08-09T06:14:00.000Z: window_seconds present without resolution: cannot confirm raw`. No record exists for that timestamp. The row at `2026-08-09T06:17:00.000Z` is a `provider_score`, value `12`, scale `1-100`, writer `garmin`. It has no `window_seconds` and no `supplied_fields`.
- Test `five_minute_envelope_is_left_out`. Import `page-5min.json` with `--mapping open-wearables --yes`. `left_out` contains `timeseries garmin_stress_level 2026-08-09T06:10:00.000Z: resolution 5min: not raw samples`. No record is written.
- Test `window_seconds_without_resolution_is_left_out`. `summaries.json` has no `metadata.resolution`. The `garmin_stress_level` sample at `2026-08-09T06:14:00.000Z` has `window_seconds` `60`. `left_out` contains `timeseries garmin_stress_level 2026-08-09T06:14:00.000Z: window_seconds present without resolution: cannot confirm raw`. No record exists for that timestamp.
- Test `period_end_is_an_aggregate`. The `garmin_stress_level` sample at `2026-08-09T06:15:00.000Z` has `period_end` `2026-08-09T07:15:00.000Z`. `left_out` contains `timeseries garmin_stress_level 2026-08-09T06:15:00.000Z: aggregate row, not a raw sample`. No record exists for that timestamp.
- The phone sample and the watch sample at `2026-08-09T08:00:00.000Z` are two `resting_heart_rate` points, `61` and `52`, on two sources. Sensors are `S-phone` and `S-awatch`. Both writers are `apple`. Match keys differ by that sensor.
- `left_out` contains `timeseries dietary_caffeine 2026-08-09T06:12:00.000Z: unmapped type dietary_caffeine`, the same shape for `workout_effort_score` and `skin_temperature_deviation`, and `timeseries heart_rate_variability_rmssd 2026-08-09T07:00:00.000Z: unit bpm does not match ms`. No record exists for those four samples.

Test `junction_fixture_maps_fields`. Junction oura sensor `ring|ring-j1|oura-app` is `S-jora`. Junction Apple Health Kit sensor `watch|watch-a|health` is `S-japple`. The writer is the slug `apple_health_kit`.

- `jn-sleep-1` `asleep_seconds` is `600`. `supplied_fields` is `{ "duration": 700, "total": 500, "awake": 0, "light": 0, "rem": 0, "deep": 0, "efficiency": 0.88, "LastNightAvg": 40, "type": "long_sleep" }`. There is no `in_bed_seconds`.
- `stage_minutes` is `{ "awake": 2, "light": 3, "deep": 4 }` with no `rem` key.
- A point `hrv_rmssd` is `55.5`, key `oura:S-jora:sleep:jn-sleep-1:point:hrv_rmssd`. A point `resting_heart_rate` is `47`. A point `respiratory_rate` is `14` unit `brpm`. There is no `hrv_sdnn` record.
- `provider_score` metric `sleep.score`, value `80`, scale `1-100`, writer `oura`, key suffix `:score:sleep.score`. `provider_score` metric `recovery_readiness_score`, value `70`, scale `0-100`, writer `oura`. Neither object has `computed_by`. The writer is `source.provider` whether or not `computed_by` is present.
- `jn-sleep-2` `asleep_seconds` is `1000`. `supplied_fields` is `{ "duration": 3600 }`.
- `jn-nap-1` is its own `sleep_session`. `supplied_fields` is `{ "duration": 900, "type": "acknowledged_nap" }`. `asleep_seconds` is `800`. It is not merged into `jn-sleep-1`.
- `jn-sleep-apple` has `asleep_seconds` `3000`. `supplied_fields` is `{ "duration": 3600, "average_hrv": 33 }`. The writer is `apple_health_kit`. The import writes no `hrv_rmssd` point for that id, so that session has no child. Test name `apple_average_hrv_stays_unmapped`. The session's `input_end_timestamp.timestamp_text` is `2026-08-11T03:00:00.000Z`.
- `left_out` contains `activity: no mapping section`. No `steps` record is written.
- Sources include writer `oura` route `junction` sensor `S-jora`, and writer `apple_health_kit` route `junction` sensor `S-japple`.

Test `mapped_imports_pass_ath_check`: import both fixtures with `--yes` into a new database, then `ath check` exits 0.

Test `rerun_does_not_add_a_record`: import the Open Wearables fixture twice. One sleep row has `provider_record_id` `ow-sleep-1`. `current_revision` is `1`. One import-key row exists for that match key. Then import a copy whose `end_time` is `2026-08-09T10:05:00.000-04:00`. The same record id remains, `current_revision` is `2`, `end_at` is `2026-08-09T14:05:00.000Z`, and `received_at` is unchanged. `end_at` is a column `ath check` compares (`src/db/records.ts` lines 288–294). `asleep_seconds` is not that column.

Test `garmin_file_and_mapping_stay_two_sources`: `ath import` of `tests/fixtures/exports/garmin/park-loop.json` with `--yes` and no `--mapping`, then the Open Wearables fixture with `--mapping open-wearables --yes`. Two workout sessions. Writer `garmin` appears on two sources: `data_route` `garmin_account_export` with an empty sensor, and `data_route` `open_wearables` with sensor `S-watch`. The file import still has `supplied_fields.Feeling` `smooth` and a `max_heart_rate` summary of `200`. The mapped workout has `start_latitude` `12.345` and a `max_heart_rate` summary of `180`.

### `field_basis` checks

The `field_basis` writer is in `crates/ath-core/src/mapping.rs`. These checks hold under D185 and D187. There is no `{their_field}_computed_by` test. Open Wearables does not send that field.

- `ow-workout-1` `field_basis` is `{ "value": "provider_or_open_wearables", "activity_type": "open_wearables_normalized" }`.
- The `average_heart_rate` and `max_heart_rate` summaries have `field_basis` `{ "value": "provider_or_open_wearables" }`.
- `garmin_stress_level` has `field_basis` `{ "value": "provider" }`. The sample has no `computed_by`.
- `garmin_body_battery` has `field_basis` `{ "value": "provider" }`. The writer is `garmin`.
- The Junction `hrv_rmssd` point has `field_basis` `{ "value": "hrv_mean_rmssd" }`.
- `sleep.score` and `recovery_readiness_score` have `field_basis` `{ "value": "provider" }`.
- Every other mapped record states the file's `value_basis` (D187): `ow-sleep-1` is `{ "value": "provider_or_open_wearables" }`, every Open Wearables timeseries point and the `vo2_max` summary are `provider_or_open_wearables`, and `jn-sleep-1` and its resting heart rate and respiratory rate points are `provider_or_junction`. Test `every mapped value states its basis (D187)`. A `source_device` record has no `field_basis`.

## 7. What this release does not change

- The records, `supplied_fields`, sources, route points, attachments, links, and import receipts produced from the section 6 baseline, including the package, the Apple zip, the bare `export.xml`, and `ath pull garmin`, apart from the one new key `field_basis` (D187). Those readers are not moved onto mapping files.
- Required record fields. No new column.
- The separation of measured readings and athlete reports. `hrv_rmssd` and `hrv_sdnn` stay two quantities. `LastNightAvg` on the Garmin personal pull stays `LastNightAvg`.
- The TypeScript command front end, except the `--mapping` flag, the help sentences, the generated `cli.md`, and `installMappings`. The front end is not moved to Rust (D174).
- Published migrations `001`, `002`, and `003` (D165). Step `004` is new.
- Earlier D-numbers and earlier `build-history/` folders, except the one edit in section 9.
- Network pulls, an Open Wearables client, a Junction client, sync, and any hosted service. `import-mapped` has no URL flag.

The build adds the optional `field_basis` object to `src/v1/schema.ts` (D185, D187), and `pnpm generate:json-schema` writes `schema/2.1.0/`. `schema/2.0.0/` is not changed. `pnpm generate:v1-fixture` regenerates the demo athlete, as section 8 says. No required field and no SQL column are added.

## 8. Version

Mike chose option A: the program version and the record version stay one number, 2.1.0. D152 made them one number for 2.0.0. D164 says that choice still holds from 2.0.0 on. Neither decision is rewritten here. The choice is D184.

The code uses two constants.

- `src/version.ts` exports `ATH_PACKAGE_VERSION`. `ath --version` prints it.
- `src/v1/schema.ts` exports `ATHLETIC_STANDARD_VERSION`. New databases store it. `ACCEPTED_RECORD_VERSIONS` lists the record versions this program can read.
- `crates/ath-core/src/lib.rs` exports `APP_VERSION` and `RECORD_VERSION`. `Store::open` compares `database_metadata.athleticstandard_version` to `RECORD_VERSION`. The error stays `database version is {version}; run ath migrate before using ath-core`. `changed_by` uses `APP_VERSION`. The single `SOFTWARE_VERSION` constant is removed.

Those strings were all `2.0.0`. Splitting the names was the small change. Setting the strings is D184.

### Option A. One number, 2.1.0. Chosen and built

`ath --version` prints `2.1.0`. A new database stores `2.1.0`. `APP_VERSION` and `RECORD_VERSION` are both `2.1.0`.

The plan's D184 text was:

> D184. The program and the record stay one number, 2.1.0.
>
> `ath --version` and `database_metadata.athleticstandard_version` are `2.1.0`. D152 made them one number so a change does not have to pick which number moves. This release keeps that rule. The mapping adds one optional field, `field_basis`, and no required field. That is D185. Minor versions may add optional fields, which is the rule in [v0.1.0/spec.md](../v0.1.0/spec.md) at the `athleticstandard_version` line, and the rule D96 records. The optional sleep fields this mapping fills were already on the 2.0.0 record. The number still moves, because the program moved and because `field_basis` is new.
>
> Rejected alternatives:
>
> - Leave the record at 2.0.0 while the program becomes 2.1.0. Rejected because D152 and D164 already rejected a split, and a later change would again have to pick which number moves.

The adopted D184 in [decisions.md](decisions.md) adds two sentences. A 2.0.0 package still imports, because its records are valid 2.1.0 data. The build found that the package loader accepted only the current version, so a 2.1.0 program would have refused a 2.0.0 package. `IMPORTABLE_PACKAGE_VERSIONS` in `src/v1/schema.ts` is `2.0.0` and `2.1.0`.

What changed:

- `package.json` `version` becomes `2.1.0`. The release workflow packs `athleticstandard-$(version).tgz`. The tarball name becomes `athleticstandard-2.1.0.tgz`. D171's `2.0.0` filename is the 2.0.0 case of that rule. It is not rewritten.
- `ATH_PACKAGE_VERSION`, `ATHLETIC_STANDARD_VERSION`, `APP_VERSION`, and `RECORD_VERSION` become `2.1.0`.
- `ACCEPTED_RECORD_VERSIONS` becomes `["1.0.0", "2.0.0", "2.1.0"]`. `READABLE_DATABASE_VERSIONS` in `src/db/migrate.ts` includes `"1.0.0"`, `"2.0.0"`, and `"2.1.0"`. If `"2.0.0"` were dropped from that set, migrate could not open the database it is supposed to upgrade.
- New migration step 4 only. Files: `src/db/migrations/004-version-2-1.ts` and `spec/sql/migrations/004-version-2-1.sql`, through `pnpm generate:sql-contract`. SQL:

```sql
UPDATE database_metadata
SET sql_schema_version = 4,
    athleticstandard_version = '2.1.0'
WHERE singleton_id = 1;
```

- No column is added. `003` still sets `2.0.0` during its own statement. Creation in `installInitialSchema` runs `VERSION_2_UPGRADE_SQL`, then the new step, then the `user_version` pragma. A new database ends at schema 4 and version `2.1.0`. `003` is not edited.
- The 004 file header is a new constant `MIGRATION_004_FILE_HEADER` in `src/db/sql-contract.ts`:

```text
-- Athletic Standard upgrade to software version 2.1.0 (migration step 4).
-- Generated by pnpm generate:sql-contract. Do not edit by hand.
--
-- See build-history/v2.1.0/spec.md.
-- No column is added. The step stores athleticstandard_version 2.1.0
-- and sql_schema_version 4.
```

- `scripts/generate-sql-contract.ts` today writes only `001-initial.sql`, `002-provider-encoding-repair.sql`, and `003-version-2.sql` (lines 19–40). It gains a write of `spec/sql/migrations/004-version-2-1.sql` from `MIGRATION_004_FILE_HEADER` plus the step 4 SQL.
- `scripts/check-schema-drift.ts` today compares only those three files (`expectedMigrationFileContents`, `expectedMigration002FileContents`, `expectedMigration003FileContents`, lines 152–203). It gains `expectedMigration004FileContents` and fails when `004-version-2-1.sql` is stale. `MIGRATION_CHAIN` in `src/db/sql-contract.ts` gains version 4, or the drift check and the migrate runner disagree.
- `applyMigrationStep` in `src/db/migrate.ts` gains `targetVersion === 4`. Today any target other than 2 or 3 throws `Migration ${step.file} is not implemented in this ath build`. The new arm runs the step 4 SQL and sets `PRAGMA user_version` to 4 on a local file, then returns. Test `migrate_version_2_database_to_schema_4` uses the temp copy of `tests/fixtures/version-2-0-0/database.ath.sqlite` and the commands in section 6.
- `SQL_SCHEMA_VERSION` in `src/db/client.ts` becomes `4`. `MIGRATION_CHAIN` gains version 4. The description says the step stores software version 2.1.0 and adds no column.
- `ath check` already compares `sql_schema_version` to `SQL_SCHEMA_VERSION`. A 2.0.0 database fails that check until migrate. After migrate it passes.
- `Store::open` rejects `2.0.0` until migrate, with the existing sentence.
- `src/db/backtest.ts` uses `ATHLETIC_STANDARD_VERSION` instead of a hardcoded `"2.0.0"`.
- The Rust test that built a `'2.0.0'` file and then called `Store::open` now runs the published `003` and `004` SQL files on a `1.0.0` file and asserts the stored version equals `RECORD_VERSION`. Inserts that build a 1.0.0 or 2.0.0 file on purpose stay hardcoded. `crates/ath-core/src/persist.rs` `persist_inserts_a_source_and_bumps_the_revision` (line 1039) passes `tool_version` inside `changed_by_json`. That test does not call `Store::open`. `apply` stores the string it is given. The tool version is `2.1.0`, so that literal is `2.1.0`. The helper `database()` at line 1019 still inserts `athleticstandard_version` `2.0.0`, because it builds a pre-migration file.
- `crates/ath-core/Cargo.toml` `version` is `2.1.0`.
- `pnpm generate:json-schema` writes `schema/2.1.0/` and does not rewrite `schema/2.0.0/`. `package.json` exports and `tests/v1-package-exports.test.ts` point at `schema/2.1.0/`. Both version constants are asserted equal to `2.1.0`.
- `pnpm generate:v1-fixture` regenerates `examples/demo-athlete/athlete.ath.sqlite`, because the stored version is inside it. From D187 its measured records also state their basis. The new hash replaces the demo-athlete line in `scripts/check-public-data.ts`. The fixture is not hand-edited. `examples/demo-athlete/athlete.ath.json` is the legacy 0.4.3 file and does not change. `tests/fixtures/version-2-0-0/database.ath.sqlite` is not regenerated.
- `README.md` links `schema/2.1.0/` beside the existing `schema/2.0.0/` link. `schema/2.0.0/` is not rewritten. `SPEC.md` documents the optional `field_basis` object. Its title, metadata table, package rule, and version section now say `2.1.0`. They still said `1.0.0` in two places, which was out of date since 2.0.0.
- `AGENTS.md` lists this spec and names `2.1.0` as the version of the normative Zod.
- `build-history/README.md` "Current records" gains the 2.1.0 links and the sentence: version 2.1.0 is the software now. The program, the record, and the database use that number.
- Calculation skill files stay on metadata version `2.0.0`. The tool skill `metadata.version` becomes `2.1.0` in the same commit as `package.json`, so `tests/skill.test.ts` line 105 still sees equal versions.
- Search the tree for `2.0.0` before the last commit. Change a hit only when it is this program's current version. Do not change `003`, historical decisions, the 2.0.0 schema folder, the baseline files, or the version-2-0-0 database copy.

What this asks of a 2.0.0 database: `ath migrate` runs, and no column is added. `user_version` becomes 4. D152 says that integer is the upgrade step. The software version is the `2.1.0` stored beside it.

### Option B. Not chosen

Option B kept the record at 2.0.0 and the program at 2.1.0, with no migration and no `field_basis` on the record. Mike did not choose it. Its text is not copied into the decisions, and no D185 stub was written.

## 9. The 2.0.0 progress note

The build deletes the section whose heading is `Later, with the reason:` from `build-history/v2.0.0/progress.md`. The edit finds that heading. It does not find a line number. Mike asked for the deletion. The list is out of date. The build pull request makes the edit. The plan pull request did not.

The heading and the bullets under it are removed. The paragraphs above the heading stay.

No file links to that heading. The one link inside the section points at `appendix/source-formats.md`. These still open the study after that section is removed:

- [D161](../v2.0.0/decisions.md), the relative link `appendix/source-formats.md`
- `docs/connections.md`, the link `../build-history/v2.0.0/appendix/source-formats.md`

`docs/connections.md` says an Open Wearables pull is not built. That sentence stays true. `docs/connections.md` is not edited.

`build-history/v2.1.0/progress.md` says Mike asked for the deletion, and it links the study:

```markdown
Mike asked to remove the outdated "Later, with the reason:" heading from
[v2.0.0/progress.md](../v2.0.0/progress.md). The mapping study is
[v2.0.0/appendix/source-formats.md](../v2.0.0/appendix/source-formats.md) (D161).
```

## 10. Notes and proof

`build-history/v2.1.0/decisions.md` holds D177 through D187. D184 and D185 were inserted before D186, and D187 follows D186. `build-history/README.md` "Current records" links the spec, the decisions, and the progress note, and says version 2.1.0 is the software now.

The proof is an image in the build pull request's description. It is not committed. Mike asked for `ath --version` showing `2.1.0`, an `ath import --mapping open-wearables` run with its `left_out` lines, and `ath check` passing on a migrated copy of the 2.0.0 database. The plan's list also asked for:

1. `ath import` of the Open Wearables fixture with `--mapping open-wearables --yes`.
2. The `hrv_rmssd` point at `42.5` and the `hrv_sdnn` point at `61.2`.
3. `efficiency_percent` still named `efficiency_percent`, and `start_latitude` still named `start_latitude`.
4. The same import again, with one sleep record and revision 1.
5. The Garmin file import and the mapped workout as two sources.
6. The `LastNightAvg` refusal on stderr, with nothing written.
7. `ath check` exiting 0 after both fixtures.

The plan pull request had no recording.

## 11. Risks, the remaining question, and the order

### Risks

- `typed_table` and `insert_typed` grow a `provider_score` arm. `insert_record`'s match logic stays. The baseline files are the check that the existing readers did not move.
- A mapping route equal to `garmin_account_export` would reuse the built-in source through the empty-sensor lookup. D179 rejects that route before a write.
- `received_at` is inside record JSON. The baseline walk deletes it at any depth. A walk that only deletes the column would fail the Garmin `max_heart_rate` summary.
- Records with no match key need the sort in section 6. A sort that drops them would hide package links.
- The baseline commit must not bump `ATHLETIC_STANDARD_VERSION`. If it does, the baseline no longer matches `d418a8f`.
- If `"2.0.0"` is dropped from `READABLE_DATABASE_VERSIONS`, migrate cannot open the version-2-0-0 copy.
- A new database runs `003`, which sets version `2.0.0`, and then step 4. Editing `003` instead would violate D165. `applyMigrationStep` must gain the `targetVersion === 4` arm or the step throws.
- Junction `average_hrv` from `apple_health_kit`, `health_connect`, or `samsung_health` stays in `supplied_fields`. A writer missing from the allow-list is left unmapped, not refused. `whoop_v2` is on the list. `google_health` is not. Fitbit's HRV method through that slug is not confirmed.
- The npm package omits `mappings/` unless `package.json` `files` lists it. The smoke test asserts the tarball paths.
- A re-import of a 2.0.0 export under 2.1.0 adds `field_basis` in a new revision of each record it matches (D187). A large Apple export gains one revision per record the first time.

### Answers from Mike

1. Option A or option B in section 8: option A. One number, 2.1.0, with `field_basis`, `schema/2.1.0/`, and migration step 4.
2. A workout with `entry_source` `manual`: Mike approved the plan with its recommendation. The object is left out of the measured import, and the other objects in the file are still written. The test is `ow-workout-manual`.
3. Mike added one more change: `field_basis` goes on every stored value, not only mapped values. That is D187 and section 13.

`--mapping` is in this release. D174 does not need a new D-number. The fourth refusal stays. It is D179 item 4, from D172.

### Order

The build was one pull request, in the commit order at the top of this file: the baseline from code that matches `d418a8f`, then the mapping engine and its front end, then the version, then `field_basis` on every stored value, then the notes. Nothing is published.

## 12. Not in 2.1.0

These are outside this release. This section does not assign them to a later version.

- Real paged fixtures from Open Wearables and Junction, instead of one invented page.
- Sleep reads that pass `include=stages` and `filter_by_priority=false`. Timeseries reads at `raw` or `1day`.
- This mapping targets Open Wearables 0.10.0 raw output. Newer Open Wearables docs say resting heart rate, nightly HRV, SpO2, and VO2 max exist only at `1day` and coarser. Against a newer server those lines are left out as aggregates. Without the envelope, files from Open Wearables servers newer than 29 October leave out every timeseries row that carries `window_seconds`, each listed in `left_out`. Rows without it are still stored. An empty or short import there does not mean no data.
- A check for an old self-hosted Open Wearables server that still has Oura RMSSD stored under the SDNN type. The study names that relabel. This release trusts the type string the server sends.
- A mapping-file version stored on each record the mapping writes.
- Open Wearables `/health-scores` is not read.
- Temperature-deviation types are `left_out`. This release does not check a vendor baseline for them.
- Junction activity summaries are `left_out`. This release does not store a UTC midnight as a measurement (D88, D110).
- Open Wearables day, week, and month aggregate rows are not mapped. The import should be given the full response. If `metadata.resolution` is present and is not `raw`, every row is `left_out`. If it is absent, a row with `window_seconds` is `left_out`. A row with `period_end` is `left_out`. On current main the bucket path sets `window_seconds` and does not set `period_end` or `stats` (`timeseries_service.py` lines 344–355). A full page at 0.10.0 already includes `metadata.resolution`. The sample at that tag has no `window_seconds` and no `computed_by`.
- `google_health` is not on the `average_hrv` allow-list. Fitbit's HRV method through that slug is not confirmed.
- A `field_basis` on records written before 2.1.0. Migration step 4 does not backfill (D187).
- A basis for each field name. One value per record, plus the activity type and the Apple route summaries, is the whole object.

## 13. Every stored value's basis

Mike extended `field_basis` from mapped values to every stored value. This section is that change. The decision is D187.

### The object

`field_basis` moves into the fields every measured record shares, so all seven measured record types may carry it: `point_measurement`, `measurement_sample`, `sleep_session`, `workout_session`, `benchmark_result`, `provider_summary`, and `provider_score`. `source`, `source_device`, athlete reports, and authored records do not. An athlete report's basis is already its record type.

```ts
field_basis: z.strictObject({
  activity_type: z.enum(["open_wearables_normalized", "provider_label"]).optional(),
  value: z.enum([
    "provider_or_open_wearables",
    "provider",
    "hrv_mean_rmssd",
    "provider_or_junction",
    "provider_documented_rmssd",
    "calculated_on_import",
    "entered_by_hand",
  ]).optional(),
  route_summaries: z.literal("calculated_on_import").optional(),
}).optional()
```

The first three `value` values and `open_wearables_normalized` are D185. The rest are D187. `schema/2.1.0/` is generated from this. `schema/2.0.0/` is not touched.

### What each importer states

| Importer | Records | `field_basis` |
|---|---|---|
| Apple Health | points, series samples, workouts, and activity summaries from a writer | `value` `provider` |
| Apple Health | a value whose writer is `Health` or `Manual`, which stays on the manual source (D173) | `value` `entered_by_hand` |
| Apple Health | sleep sessions, which this software sums from the writer's sleep-stage samples | `value` `calculated_on_import` |
| Apple Health | RMSSD from a beat list or an ECG, which already carries a `calculations` block, and ECG beat intervals | `value` `calculated_on_import` |
| Apple Health | a workout | `activity_type` `provider_label`; `route_summaries` `calculated_on_import` when a route file supplied its elevation gain or kilometre splits |
| WHOOP | `Heart rate variability (ms)` stored as `hrv_rmssd` | `value` `provider_documented_rmssd` |
| WHOOP | every other point, score, sleep session, and workout | `value` `provider`; a workout also `activity_type` `provider_label` |
| Oura | the HRV column stored as `hrv_rmssd` | `value` `provider_documented_rmssd` |
| Oura | every other point, score, and sleep session | `value` `provider` |
| Garmin | account JSON, GPX, and FIT workouts; the `max_heart_rate` summary; the personal pull's workouts and `LastNightAvg` | `value` `provider`; a workout with an activity type also `activity_type` `provider_label` |
| Strava | workouts | `value` `provider`; `activity_type` `provider_label` |
| `ath log` | a plain-text measurement or benchmark result; a structured measured record whose `field_basis.value` is missing | `value` `entered_by_hand` |
| Open Wearables mapping | sleep, workouts, points, and summaries | `value` `provider_or_open_wearables`; a workout also `activity_type` `open_wearables_normalized` |
| Junction mapping | sleep and points | `value` `provider_or_junction`; the `average_hrv` point `hrv_mean_rmssd` |
| Any mapping | provider scores | `value` `provider` |

Every importer could state a basis from what its source says. None had to be invented. Apple's hand-entered writers, its calculated sleep totals, and its route summaries are the three places where one record type from one source has more than one basis. Each is stated separately.

The TypeScript provider import decides the basis in `valueBasis` in `src/db/provider-import.ts`. The Apple importer marks a workout whose splits or elevation gain it calculated, in `routeSummariesCalculated`. The Rust importers use `provider_basis` in `crates/ath-core/src/garmin.rs`.

### Mapping files

The header key `value_basis` is required. The value is `provider`, `provider_or_open_wearables`, or `provider_or_junction`. It applies to every record the file writes. A line's `basis` replaces it for that line's record. Every provider score is `provider`. An `activity_type` line must state its basis. The shipped Open Wearables file has `value_basis: provider_or_open_wearables` and `type: activity_type basis open_wearables_normalized`. The Junction file has `value_basis: provider_or_junction`. A `source_device` record has no `field_basis`.

### Old records and corrections

Migration step 4 does not backfill. A record written before 2.1.0 has no `field_basis`, and it stays valid. A restored 2.0.0 package keeps its records as they were. A re-import of the same export under 2.1.0 states the basis in a new revision, because the content then differs.

`ath correct` stores the whole record the person gives. The tool skill says to set `field_basis.value` to `entered_by_hand` when a correction replaces a measured value with one a person typed.

### Tests

- The import baseline compares each import with `field_basis` removed against the d418a8f files, byte for byte. It compares the basis of every record against `tests/fixtures/import-baseline/field-basis/`, byte for byte. It asserts that every measured record has a basis value, that no other record has one, and that the restored 2.0.0 package has none.
- `tests/mapping-cli.test.ts` asserts the basis of every mapped record, the `value_basis` and `activity_type` check failures, and a person's own file with `value_basis: provider`.
- `tests/v2-core.test.ts` asserts the Garmin workout, the Garmin `max_heart_rate` summary, and the Strava workout.
- `tests/v1-cli-writes.test.ts` asserts `entered_by_hand` on a plain-text measurement, a plain-text benchmark result, and a structured point, and that a structured point keeps a basis it already states.
- `every_garmin_workout_states_its_basis` in `crates/ath-core/src/garmin.rs` covers JSON, GPX, and the activity type.
- The demo athlete is regenerated. Its WHOOP-style HRV point is `provider_documented_rmssd`, its benchmark result is `entered_by_hand`, its workout states `provider_label`, and its other measured records are `provider`.
