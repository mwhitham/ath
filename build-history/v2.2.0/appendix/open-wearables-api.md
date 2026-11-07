# Open Wearables, as checked on 6 November 2026

This study is the evidence for the 2.2.0 plan. The rules are in
[the specification](../spec.md) and [the decisions](../decisions.md). Provider
software changes. A fact below is tied to the page or the file it came from.
Nothing here was read from a live server, and no key was used.

## What the product is

Open Wearables is the program in the public repository
`the-momentum/open-wearables`. The repository describes it as a self-hosted
platform that puts wearable data behind one API. The license file reported by
GitHub is MIT. The repository was created on 30 October 2025. GitHub reported
the default branch `main` pushed at 13:43 UTC on 6 November 2026.

The newest published release on that date was `0.11.0`, published at 16:10 UTC
on 2 November 2026, at commit `183e8cc4f6de897dbd288c93d49a691f2cf721ae`.
Its title is "0.11 - Daily
aggregates & faster summaries". The commit on `main` that day,
`7ca121dd4e48`, is "chore: update OW to 1.0 (#2065)". There was no GitHub
release named 1.0.0.

The GitHub homepage field is `https://docs.openwearables.io`. This study did
not fetch that host. The pages it did fetch are on
`https://openwearables.io/docs`. The API introduction there says the reference
is generated from `docs/openapi.json` on every change, so the website describes
current `main`, and a self-hosted copy should be read from that copy's own
`/openapi.json`.

Checked:

- Repository: `https://github.com/the-momentum/open-wearables`
- Release `0.11.0`: `https://github.com/the-momentum/open-wearables/releases/tag/0.11.0`
- API introduction, fetched 6 November 2026: `https://openwearables.io/docs/api-reference/introduction`
- Integration guide, fetched the same day: `https://openwearables.io/docs/dev-guides/integration-guide`
- Doc index: `https://openwearables.io/docs/llms.txt`
- OpenAPI at the `0.11.0` tag: `docs/openapi.json`, `info.version` `0.11.0`
- OpenAPI on `main` the same day: `docs/openapi.json`, `info.version` `1.0.0`

The integration guide says "self-hosted or cloud". In the same guide the cloud
case is a deployment the reader runs. This study did not find a separate
hosted service, in the repository description, the `0.11.0` notes, or the API
introduction, that stores data for people who do not run the server.

Open Wearables is not Junction. Junction remains the other shipped mapping
file, and this plan does not call it.

## The reads this plan uses

D203 names every read. The paths below are the ones this study opened
first. Each is under `/api/v1` and takes a user id in the path. The user
id is an Open Wearables UUID. Dates are required on the paged reads.

| Read | Path on `0.11.0` | Page shape |
|---|---|---|
| Sleep | `GET /api/v1/users/{user_id}/events/sleep` | `data`, `pagination`, `metadata` |
| Workouts | `GET /api/v1/users/{user_id}/events/workouts` | `data`, `pagination`, `metadata` |
| Timeseries | `GET /api/v1/users/{user_id}/timeseries` | `data`, `pagination`, `metadata` |

`pagination.has_more` is required in the OpenAPI schema. `next_cursor` is
null when there is no next page. The cursor code in `0.11.0` is a URL-safe
base64 of a timestamp, an id, and an optional `prev_` prefix
(`backend/app/utils/pagination.py`). `limit` defaults to 50 and the maximum
is 1000.

Sleep and timeseries take `filter_by_priority`, default `false`. The `0.11.0`
sleep handler passes that flag into the query. The `0.11.0` timeseries handler
does the same. On `main` (OpenAPI `1.0.0`, not a release), an omitted
`source_selection` means all devices when `filter_by_priority` is false. The
`main` workout list uses all devices when `source_selection` is omitted. The
`0.11.0` workout list has no priority flag. It returns the workouts in range.

Timeseries `resolution` defaults to `raw`. The enum is `raw`, `1min`, `5min`,
`15min`, `1hour`, `1day`, `1week`, `1month`. The `0.11.0` service copies that
value into `metadata.resolution` (`timeseries_service.py` around the
`TimeseriesMetadata` construction).

The `0.11.0` OpenAPI parameter `types` is an array of `SeriesType`. It omits
`style` and `explode`. For a query parameter, OpenAPI's default style is
`form`, and the default explode for `form` is true, which is one query
parameter per value. A comma-joined string is not a member of that enum.
This study did not send the request. The plan sends one parameter per type.

The `0.11.0` release notes say day and session values are no longer samples.
They are read with `resolution=1day` or from the sleep or workout. The notes
say a sample row can carry `window_seconds`, and an aggregate can carry
`computed_by` of `provider` or `open_wearables`. The raw sample builder in
`timeseries_service.py` (the `_to_sample` call in the raw return) does not
pass `computed_by`. The field exists on the schema and can be null.

`ComputedBy` is `provider` or `open_wearables`. The description says who
computed an aggregate.

## Fields the mapping already names

Sleep, from the `0.11.0` schema `SleepSession`: `id`, `start_time`,
`end_time`, `zone_offset`, `source`, `duration_seconds`,
`sleep_duration_seconds`, `time_in_bed_seconds`, `efficiency_percent`,
`stages`, `sleep_stage_intervals`, `is_nap`. Stages have `awake_minutes`,
`light_minutes`, `deep_minutes`, `rem_minutes`. The session object does not
carry resting heart rate, heart-rate variability, blood oxygen, skin
temperature, or respiration. `sleep_stage_intervals` is null unless the
query sends `include=stages` (`event_record_service.py` on this commit).
Stage minute totals are returned either way. Workout zones need
`include=zones`. Workout segments need `include=segments`. `source` has
`provider`, `source`, `device`, `device_type`, `device_id`, and read-only
`device_name`. `metadata.py` on this commit says `device_id` tells
same-model devices apart. `provider` is the integration, and `source` is
the writer inside it.

The timeseries route defaults `types` to an empty list. The repository
filters by type only when that list is nonempty. An empty list is every
series.

Workouts, from `Workout`: `id`, `type`, `name`, `start_time`, `end_time`,
`zone_offset`, `source`, `entry_source`, `intensity`, the distance, heart-rate,
and elevation fields the mapping already names, plus fields it does not name.
`entry_source` is `manual`, `automatic`, or `unknown`, and the response
allows null. The SDK maps an unsupported method to `unknown` and a
missing method to null. The schema text says `manual` is a self-reported
entry with no device measurement behind it. `unknown` and null are not
that entry, and they are not `automatic`.

Timeseries, from `TimeSeriesSample`: `timestamp`, `zone_offset`, `type`,
`value`, `unit`, `source`, `is_daily_total`, `window_seconds`, `period_end`,
`computed_by`, `stats`.

`start_time` and `timestamp` are schema format `date-time`. This study did not
capture a live JSON body, so it does not claim the wire format beyond that
schema. `zone_offset` is a separate string, `+01:00` or `-05:30`, or null.

The 2.1.0 mapping file's timeseries lines do not name `heart_rate`. In
Athletic Standard, `heart_rate` is a series quantity
(`SERIES_QUANTITY_UNITS` in `src/v1/schema.ts`). It is not one of the point
quantities the 2.1.0 mapping engine writes. D203 stores that stream as
measurement samples. The 2.1.0 file, by itself, still has nowhere to put it.

## Time bounds

The API introduction says a date-only start is midnight UTC and a date-only
end covers that whole UTC day.

The `0.11.0` parser does not say UTC. `parse_query_datetime` in
`backend/app/utils/dates.py` accepts a numeric Unix timestamp in seconds
before it tries `datetime.fromisoformat`. A date-only string becomes midnight
with no timezone. `parse_query_end_datetime` turns a date-only end into the
next midnight, also with no timezone, because the comparison is `<`. This
study did not follow those naive values into the database session, so it does
not claim which timezone PostgreSQL applies. Athletic Standard's own command
still refuses a numeric bound. That is this program's rule, not the server's.

A full timestamp is sent through as parsed. The docs say a value with an
explicit time is used as given, and `end_time` at midnight includes nothing
later that day.

## Credentials

Data routes depend on `ApiKeyDep` (`backend/app/services/api_key_service.py`
on `0.11.0`). That dependency accepts a developer login JWT or the header
`X-Open-Wearables-API-Key`. A missing or unknown key is HTTP 401, detail
"Authentication required: provide JWT token or API key" or "Invalid or
missing API key".

The integration guide says the key is not a bearer token, and
`Authorization: Bearer` with the API key returns 401. The OpenAPI operation
security lists `OAuth2PasswordBearer` and marks the API-key header optional.
Both can be true: the header is optional in the schema because a developer
JWT is the other way in. The header is how an integration authenticates.

`POST /api/v1/users/{user_id}/token` returns a token the `0.11.0` route text
limits to SDK endpoints. `get_current_developer` in `backend/app/utils/auth.py`
rejects a token whose scope is `sdk` on developer routes. There is no
user-scoped token for the sleep, workout, and timeseries reads. The API key
is an administrator key for the whole deployment. The integration guide and
the credentials page say that.

`GET /api/v1/users` lists every user and uses page/limit pagination, not the
cursor used by the three data reads. `GET /api/v1/users/{user_id}/personal-record`
returns birth date, gender, and sex.

`DELETE` exists for one sleep, one workout, one provider connection, and one
provider's stored data. Those are writes against the server. None of the
three read routes takes `updated_since` or returns a deletion list. Sync
routes (`/sync`, `/sync/historical`, `/sync/runs`, `/sync/stream`) report or
start provider sync. They are not a feed of changed records.

`GET /api/v1/config` returns feature flags and requires a developer JWT. It
has no version field. The docs point at `/openapi.json` for the running
version. That file is the full specification (about 600 KB at `0.11.0` and
about 700 KB on `main` the day of this study).

Health scores use `limit` and `offset`. The `0.11.0` notes say Open Wearables
stops computing its own scores on 6 November 2026. Recovery summaries are
daily objects with a `date` and no instant. They include `avg_hrv_rmssd_ms`
and `avg_hrv_sdnn_ms` as separate fields.

No rate-limit field was in the API introduction or in the three `0.11.0`
route files. A limit may still exist in middleware this study did not open.

## Coverage audit on 7 November 2026

The same commit, `183e8cc4f6de897dbd288c93d49a691f2cf721ae`, was read again
for every user data route. The OpenAPI file is `docs/openapi.json`,
`info.version` `0.11.0`. The storage rule is D208. This section records
what the source shows.

`SeriesType` is the enum in `backend/app/schemas/enums/series_types.py`.
Each entry in `SERIES_TYPE_DEFINITIONS` is an id, a name, and a unit.
`heart_rate` is `bpm`. `latitude` and `longitude` are `degrees`. The raw
path in `timeseries_service.py` copies `recorded_at`, `zone_offset`, the
type, the value, and `window_seconds`. It does not set `period_end` or
`computed_by`. The response `metadata.resolution` is the raw or day
label. A series row does not carry that label. The archive capsule
copies it. D208 writes that capsule only after the locked safety read.

`_aggregated_timeseries` handles `1min`, `5min`, `15min`, and `1hour`. It
sets `window_seconds` from `BUCKET_SIZES` and does not set `period_end`.
Those rows are buckets of raw samples, built when the query runs.

`_period_timeseries` handles `1day`, `1week`, and `1month`. The timestamp
is `CalendarPeriod.of_date(period, zone_offset).start`. `period_end` is
the start of the next period. For a day, `computed_by` is the stored row's
value. For a week or a month, the code sets `computed_by` to
`open_wearables`. The comment says weeks start on Monday and months on
the first, from the local day. One response row is one period, one
source, and one series type. `stats` is the combined object.

`SleepSession` has no resting heart rate, heart-rate variability, blood
oxygen, respiration, or temperature. `SleepSummary` has
`avg_heart_rate_bpm`, `avg_hrv_sdnn_ms`, `avg_hrv_rmssd_ms`,
`avg_respiratory_rate`, and `avg_spo2_percent`, plus a `date` and optional
`start_time` and `end_time`. `RecoverySummary` has `resting_heart_rate_bpm`,
`avg_hrv_rmssd_ms`, `avg_hrv_sdnn_ms`, `avg_spo2_percent`, and
`recovery_score`. It has no temperature field. The SDNN field text says
the value is null for current rows because they come from WHOOP and
WHOOP's HRV there is RMSSD. The class prose calls `recovery_score`
deprecated, WHOOP-only, and 0 to 100. The recovery query selects by
category, not by WHOOP, and copies `provider` and `value` unchanged.
Polar writes recovery scores on the 1 to 6 range. D206 uses that range
table and the copied provider. It does not use the prose as the scale.
`BodyLatest` has `skin_temperature_celsius`
and `skin_temperature_measured_at`. `BodyAveraged` has `period_start`,
`period_end`, and separate RMSSD and SDNN fields.

Workout `segments` is an array of objects. `event_record_service.py`
fills it only when `include=segments`. Zones are filled only when
`include=zones`. The provider workout routes describe a live call to
Suunto, Polar, or Garmin. They are not the stored workout object.

Sleep totals, workout totals, and activity totals describe sums of the
listed rows. The personal-record response fields are `birth_date`,
`gender`, and `sex`.

## Contract read on 7 November 2026

The same commit was read again for the summary, score, meal, cycle, body,
and personal-record contracts. D208 is the storage rule. This section
records the source facts that rule uses. `SERIES_TYPE_DEFINITIONS` has 141
entries.

Raw rows keep the 2.1.0 kind when the mapping already names one. Those
series are `heart_rate_variability_rmssd`, `heart_rate_variability_sdnn`,
`resting_heart_rate`, `respiratory_rate`, `skin_temperature`,
`oxygen_saturation`, `heart_rate_recovery_one_minute`, `vo2_max`,
`garmin_stress_level`, and `garmin_body_battery`.

The table below is the mapping for all 141 series. D208 is the storage rule.
`input_value` and the input unit are the server's number and unit.
`standard_value` equals `input_value` except where the conversion is
`divide by 100`. A day row uses the day record in the last column. It is
never a measurement sample. Apple skin temperature stays
`wrist_temperature_sleeping` when the writer is `apple`. `weight` is stored
as `body_weight`. The series name stays in `supplied_fields`.

Sleep summary numbers in `event_record_repository.py` sum every non-nap
session in the source and date. The summary service sets `start_time` and
`end_time` from the longest main sleep. Those bounds stay on the original
object. They are not the period of the summed fields.

| Series | Input unit | Raw class | Target | Standard unit | Conversion | Day record |
|---|---|---|---|---|---|---|
| `heart_rate` | `bpm` | measurement_sample | `heart_rate` | `bpm` | unchanged | provider_summary window |
| `resting_heart_rate` | `bpm` | point | `resting_heart_rate` | `bpm` | unchanged | provider_summary window |
| `heart_rate_variability_sdnn` | `ms` | point | `hrv_sdnn` | `ms` | unchanged | provider_summary window |
| `heart_rate_recovery_one_minute` | `bpm` | point | `hr_recovery` | `bpm` | unchanged | provider_summary window, summary type `hr_recovery` |
| `walking_heart_rate` | `bpm` | point | `walking_heart_rate` | `bpm` | unchanged | provider_summary window |
| `heart_rate_variability_rmssd` | `ms` | point | `hrv_rmssd` | `ms` | unchanged | provider_summary window |
| `oxygen_saturation` | `percent` | point | `oxygen_saturation` | `%` | alias | provider_summary window |
| `blood_glucose` | `mg_dl` | measurement_sample | `blood_glucose` | `mg_dl` | unchanged | provider_summary window |
| `blood_pressure_systolic` | `mmHg` | point | `blood_pressure_systolic` | `mmHg` | unchanged | provider_summary window |
| `blood_pressure_diastolic` | `mmHg` | point | `blood_pressure_diastolic` | `mmHg` | unchanged | provider_summary window |
| `respiratory_rate` | `brpm` | point | `respiratory_rate` | `brpm` | unchanged | provider_summary window |
| `sleeping_breathing_disturbances` | `count` | measurement_sample | `sleeping_breathing_disturbances` | `count` | unchanged | provider_summary window |
| `blood_alcohol_content` | `mg_dl` | measurement_sample | `blood_alcohol_content` | `mg_dl` | unchanged | provider_summary window |
| `peripheral_perfusion_index` | `score` | source_report | `peripheral_perfusion_index` | `score` | unchanged | source_report window |
| `forced_vital_capacity` | `liters` | measurement_sample | `forced_vital_capacity` | `liters` | unchanged | provider_summary window |
| `forced_expiratory_volume_1` | `liters` | measurement_sample | `forced_expiratory_volume_1` | `liters` | unchanged | provider_summary window |
| `peak_expiratory_flow_rate` | `L/min` | measurement_sample | `peak_expiratory_flow_rate` | `L/min` | unchanged | provider_summary window |
| `breathing_disturbance_index` | `score` | source_report | `breathing_disturbance_index` | `score` | unchanged | source_report window |
| `height` | `cm` | point | `height` | `cm` | unchanged | provider_summary window |
| `weight` | `kg` | point | `body_weight` | `kg` | unchanged | provider_summary window |
| `body_fat_percentage` | `percent` | point | `body_fat_percentage` | `%` | alias | provider_summary window |
| `body_mass_index` | `kg_m2` | source_report | `body_mass_index` | `kg_m2` | unchanged | source_report window |
| `lean_body_mass` | `kg` | point | `lean_body_mass` | `kg` | unchanged | provider_summary window |
| `body_temperature` | `celsius` | point | `body_temperature` | `°C` | alias | provider_summary window |
| `skin_temperature` | `celsius` | point | `skin_temperature` | `°C` | alias | provider_summary window |
| `waist_circumference` | `cm` | measurement_sample | `waist_circumference` | `cm` | unchanged | provider_summary window |
| `body_fat_mass` | `kg` | measurement_sample | `body_fat_mass` | `kg` | unchanged | provider_summary window |
| `skeletal_muscle_mass` | `kg` | measurement_sample | `skeletal_muscle_mass` | `kg` | unchanged | provider_summary window |
| `skin_temperature_deviation` | `celsius` | measurement_sample | `skin_temperature_deviation` | `°C` | alias | provider_summary window |
| `skin_temperature_trend_deviation` | `celsius` | measurement_sample | `skin_temperature_trend_deviation` | `°C` | alias | provider_summary window |
| `bone_mass` | `kg` | measurement_sample | `bone_mass` | `kg` | unchanged | provider_summary window |
| `body_water_mass` | `kg` | measurement_sample | `body_water_mass` | `kg` | unchanged | provider_summary window |
| `vo2_max` | `ml_kg_min` | provider_summary | `vo2_max` | `ml/kg/min` | alias | provider_summary window |
| `six_minute_walk_test_distance` | `meters` | measurement_sample | `six_minute_walk_test_distance` | `m` | alias | provider_summary window |
| `cardiovascular_age` | `years` | source_report | `cardiovascular_age` | `years` | unchanged | source_report window |
| `steps` | `count` | measurement_sample | `steps` | `count` | unchanged | provider_summary window |
| `active_energy` | `kcal` | measurement_sample | `active_energy` | `kcal` | unchanged | provider_summary window |
| `basal_energy` | `kcal` | measurement_sample | `basal_energy` | `kcal` | unchanged | provider_summary window |
| `stand_time` | `minutes` | measurement_sample | `stand_time` | `min` | alias | provider_summary window |
| `exercise_time` | `minutes` | measurement_sample | `exercise_time` | `min` | alias | provider_summary window |
| `physical_effort` | `met` | measurement_sample | `physical_effort` | `MET` | alias | provider_summary window |
| `flights_climbed` | `count` | measurement_sample | `flights_climbed` | `count` | unchanged | provider_summary window |
| `active_time` | `minutes` | measurement_sample | `active_time` | `min` | alias | provider_summary window |
| `total_energy` | `kcal` | measurement_sample | `total_energy` | `kcal` | unchanged | provider_summary window |
| `distance_walking_running` | `meters` | measurement_sample | `distance_walking_running` | `m` | alias | provider_summary window |
| `distance_cycling` | `meters` | measurement_sample | `distance_cycling` | `m` | alias | provider_summary window |
| `distance_swimming` | `meters` | measurement_sample | `distance_swimming` | `m` | alias | provider_summary window |
| `distance_downhill_snow_sports` | `meters` | measurement_sample | `distance_downhill_snow_sports` | `m` | alias | provider_summary window |
| `distance_other` | `meters` | measurement_sample | `distance_other` | `m` | alias | provider_summary window |
| `walking_step_length` | `cm` | measurement_sample | `walking_step_length` | `m` | divide by 100 | provider_summary window |
| `walking_speed` | `m_per_s` | measurement_sample | `walking_speed` | `m/s` | alias | provider_summary window |
| `walking_double_support_percentage` | `percent` | measurement_sample | `walking_double_support_percentage` | `%` | alias | provider_summary window |
| `walking_asymmetry_percentage` | `percent` | measurement_sample | `walking_asymmetry_percentage` | `%` | alias | provider_summary window |
| `walking_steadiness` | `percent` | measurement_sample | `walking_steadiness` | `%` | alias | provider_summary window |
| `stair_descent_speed` | `m_per_s` | measurement_sample | `stair_descent_speed` | `m/s` | alias | provider_summary window |
| `stair_ascent_speed` | `m_per_s` | measurement_sample | `stair_ascent_speed` | `m/s` | alias | provider_summary window |
| `running_power` | `watts` | measurement_sample | `running_power` | `W` | alias | provider_summary window |
| `running_speed` | `m_per_s` | measurement_sample | `running_speed` | `m/s` | alias | provider_summary window |
| `running_vertical_oscillation` | `cm` | measurement_sample | `running_vertical_oscillation` | `cm` | unchanged | provider_summary window |
| `running_ground_contact_time` | `ms` | measurement_sample | `running_ground_contact_time` | `ms` | unchanged | provider_summary window |
| `running_stride_length` | `cm` | measurement_sample | `running_stride_length` | `m` | divide by 100 | provider_summary window |
| `running_vertical_ratio` | `percent` | measurement_sample | `running_vertical_ratio` | `%` | alias | provider_summary window |
| `running_stance_time_balance` | `percent` | measurement_sample | `running_stance_time_balance` | `%` | alias | provider_summary window |
| `swimming_stroke_count` | `count` | measurement_sample | `swimming_stroke_count` | `count` | unchanged | provider_summary window |
| `underwater_depth` | `meters` | measurement_sample | `underwater_depth` | `m` | alias | provider_summary window |
| `cadence` | `rpm` | measurement_sample | `cadence` | `rpm` | unchanged | provider_summary window |
| `power` | `watts` | measurement_sample | `power` | `W` | alias | provider_summary window |
| `speed` | `m_per_s` | measurement_sample | `speed` | `m/s` | alias | provider_summary window |
| `workout_effort_score` | `score` | source_report | `workout_effort_score` | `score` | unchanged | source_report window |
| `estimated_workout_effort_score` | `score` | source_report | `estimated_workout_effort_score` | `score` | unchanged | source_report window |
| `distance` | `meters` | measurement_sample | `distance` | `m` | alias | provider_summary window |
| `elevation_gain` | `meters` | measurement_sample | `elevation_gain` | `m` | alias | provider_summary window |
| `environmental_audio_exposure` | `dB` | measurement_sample | `environmental_audio_exposure` | `dB` | unchanged | provider_summary window |
| `headphone_audio_exposure` | `dB` | measurement_sample | `headphone_audio_exposure` | `dB` | unchanged | provider_summary window |
| `environmental_sound_reduction` | `dB` | measurement_sample | `environmental_sound_reduction` | `dB` | unchanged | provider_summary window |
| `time_in_daylight` | `minutes` | measurement_sample | `time_in_daylight` | `min` | alias | provider_summary window |
| `water_temperature` | `celsius` | measurement_sample | `water_temperature` | `°C` | alias | provider_summary window |
| `uv_exposure` | `count` | measurement_sample | `uv_exposure` | `count` | unchanged | provider_summary window |
| `inhaler_usage` | `count` | measurement_sample | `inhaler_usage` | `count` | unchanged | provider_summary window |
| `weather_temperature` | `celsius` | measurement_sample | `weather_temperature` | `°C` | alias | provider_summary window |
| `weather_humidity` | `percent` | measurement_sample | `weather_humidity` | `%` | alias | provider_summary window |
| `elevation` | `meters` | measurement_sample | `elevation` | `m` | alias | provider_summary window |
| `latitude` | `degrees` | measurement_sample | `latitude` | `deg` | alias | provider_summary window |
| `longitude` | `degrees` | measurement_sample | `longitude` | `deg` | alias | provider_summary window |
| `air_temperature` | `celsius` | measurement_sample | `air_temperature` | `°C` | alias | provider_summary window |
| `garmin_stress_level` | `score` | provider_score | `garmin_stress_level` | `score` | unchanged; scale 1-100 | provider_score window |
| `garmin_skin_temperature` | `celsius` | measurement_sample | `garmin_skin_temperature` | `°C` | alias | provider_summary window |
| `garmin_fitness_age` | `years` | source_report | `garmin_fitness_age` | `years` | unchanged | source_report window |
| `garmin_body_battery` | `percent` | provider_score | `garmin_body_battery` | `score` | unchanged; input percent; scale 0-100 | provider_score window |
| `withings_pulse_wave_velocity` | `m_per_s` | measurement_sample | `withings_pulse_wave_velocity` | `m/s` | alias | provider_summary window |
| `withings_metabolic_age` | `years` | source_report | `withings_metabolic_age` | `years` | unchanged | source_report window |
| `dietary_energy_consumed` | `kcal` | source_report | `dietary_energy_consumed` | `kcal` | unchanged | source_report window |
| `dietary_carbohydrates` | `g` | source_report | `dietary_carbohydrates` | `g` | unchanged | source_report window |
| `dietary_fiber` | `g` | source_report | `dietary_fiber` | `g` | unchanged | source_report window |
| `dietary_sugar` | `g` | source_report | `dietary_sugar` | `g` | unchanged | source_report window |
| `dietary_fat_total` | `g` | source_report | `dietary_fat_total` | `g` | unchanged | source_report window |
| `dietary_fat_saturated` | `g` | source_report | `dietary_fat_saturated` | `g` | unchanged | source_report window |
| `dietary_fat_monounsaturated` | `g` | source_report | `dietary_fat_monounsaturated` | `g` | unchanged | source_report window |
| `dietary_fat_polyunsaturated` | `g` | source_report | `dietary_fat_polyunsaturated` | `g` | unchanged | source_report window |
| `dietary_cholesterol` | `mg` | source_report | `dietary_cholesterol` | `mg` | unchanged | source_report window |
| `dietary_protein` | `g` | source_report | `dietary_protein` | `g` | unchanged | source_report window |
| `dietary_sodium` | `mg` | source_report | `dietary_sodium` | `mg` | unchanged | source_report window |
| `dietary_potassium` | `mg` | source_report | `dietary_potassium` | `mg` | unchanged | source_report window |
| `dietary_calcium` | `mg` | source_report | `dietary_calcium` | `mg` | unchanged | source_report window |
| `dietary_iron` | `mg` | source_report | `dietary_iron` | `mg` | unchanged | source_report window |
| `dietary_magnesium` | `mg` | source_report | `dietary_magnesium` | `mg` | unchanged | source_report window |
| `dietary_phosphorus` | `mg` | source_report | `dietary_phosphorus` | `mg` | unchanged | source_report window |
| `dietary_zinc` | `mg` | source_report | `dietary_zinc` | `mg` | unchanged | source_report window |
| `dietary_copper` | `mg` | source_report | `dietary_copper` | `mg` | unchanged | source_report window |
| `dietary_manganese` | `mg` | source_report | `dietary_manganese` | `mg` | unchanged | source_report window |
| `dietary_selenium` | `mcg` | source_report | `dietary_selenium` | `mcg` | unchanged | source_report window |
| `dietary_chromium` | `mcg` | source_report | `dietary_chromium` | `mcg` | unchanged | source_report window |
| `dietary_molybdenum` | `mcg` | source_report | `dietary_molybdenum` | `mcg` | unchanged | source_report window |
| `dietary_iodine` | `mcg` | source_report | `dietary_iodine` | `mcg` | unchanged | source_report window |
| `dietary_chloride` | `mg` | source_report | `dietary_chloride` | `mg` | unchanged | source_report window |
| `dietary_vitamin_a` | `mcg` | source_report | `dietary_vitamin_a` | `mcg` | unchanged | source_report window |
| `dietary_vitamin_b6` | `mg` | source_report | `dietary_vitamin_b6` | `mg` | unchanged | source_report window |
| `dietary_vitamin_b12` | `mcg` | source_report | `dietary_vitamin_b12` | `mcg` | unchanged | source_report window |
| `dietary_vitamin_c` | `mg` | source_report | `dietary_vitamin_c` | `mg` | unchanged | source_report window |
| `dietary_vitamin_d` | `mcg` | source_report | `dietary_vitamin_d` | `mcg` | unchanged | source_report window |
| `dietary_vitamin_e` | `mg` | source_report | `dietary_vitamin_e` | `mg` | unchanged | source_report window |
| `dietary_vitamin_k` | `mcg` | source_report | `dietary_vitamin_k` | `mcg` | unchanged | source_report window |
| `dietary_thiamin` | `mg` | source_report | `dietary_thiamin` | `mg` | unchanged | source_report window |
| `dietary_riboflavin` | `mg` | source_report | `dietary_riboflavin` | `mg` | unchanged | source_report window |
| `dietary_niacin` | `mg` | source_report | `dietary_niacin` | `mg` | unchanged | source_report window |
| `dietary_folate` | `mcg` | source_report | `dietary_folate` | `mcg` | unchanged | source_report window |
| `dietary_pantothenic_acid` | `mg` | source_report | `dietary_pantothenic_acid` | `mg` | unchanged | source_report window |
| `dietary_biotin` | `mcg` | source_report | `dietary_biotin` | `mcg` | unchanged | source_report window |
| `dietary_caffeine` | `mg` | source_report | `dietary_caffeine` | `mg` | unchanged | source_report window |
| `dietary_fat_trans` | `g` | source_report | `dietary_fat_trans` | `g` | unchanged | source_report window |
| `dietary_energy_from_fat` | `kcal` | source_report | `dietary_energy_from_fat` | `kcal` | unchanged | source_report window |
| `dietary_fat_unsaturated` | `g` | source_report | `dietary_fat_unsaturated` | `g` | unchanged | source_report window |
| `dietary_folic_acid` | `mcg` | source_report | `dietary_folic_acid` | `mcg` | unchanged | source_report window |
| `electrodermal_activity` | `count` | measurement_sample | `electrodermal_activity` | `count` | unchanged | provider_summary window |
| `push_count` | `count` | measurement_sample | `push_count` | `count` | unchanged | provider_summary window |
| `atrial_fibrillation_burden` | `count` | measurement_sample | `atrial_fibrillation_burden` | `count` | unchanged | provider_summary window |
| `insulin_delivery` | `count` | measurement_sample | `insulin_delivery` | `count` | unchanged | provider_summary window |
| `number_of_times_fallen` | `count` | measurement_sample | `number_of_times_fallen` | `count` | unchanged | provider_summary window |
| `number_of_alcoholic_beverages` | `count` | source_report | `number_of_alcoholic_beverages` | `count` | unchanged | source_report window |
| `nike_fuel` | `count` | source_report | `nike_fuel` | `count` | unchanged | source_report window |
| `hydration` | `mL` | source_report | `hydration` | `mL` | unchanged | source_report window |

`get_day_rows` keeps one row per data source, series, and local day. It
orders provider-computed rows first, then the later `period_start`.
`_period_timeseries` rebuilds the response bounds with `CalendarPeriod`.
A week or month forces `computed_by` to `open_wearables`. A day page's
`limit` counts periods. The service floors that count at 1, and one period
can contain one row per source and series.

`HEALTH_SCORE_RANGES` sets the finite scales. Sleep is 0 to 100 for
internal, WHOOP, and Samsung, and 1 to 100 for Oura, Garmin, and Polar.
Oura readiness and Oura activity are 1 to 100. Polar readiness is 0 to 10.
Garmin stress and Garmin body battery are 0 to 100. WHOOP and Suunto
recovery are 0 to 100. Polar recovery is 1 to 6. WHOOP strain is 0 to 21.
Polar strain has no finite maximum. The route text says `provider: internal` scores were
computed by Open Wearables and are still returned. WHOOP recovery
components include `skin_temp_celsius` when the provider sent it.

`get_menstrual_cycles` sets `end_datetime` to null. `get_body_summary`
uses `datetime.now`, omits `device_id`, and does not put a time on
`BodySlowChanging`. The personal-record response is `birth_date`,
`gender`, `sex`, and `user_id`. A field that was never set is null.

## What was not checked

- A running server's JSON, including the exact `start_time` text.
- Whether Docker tag `latest` matches release `0.11.0` or `main`.
- The database timezone applied to a date-only bound.
- Middleware rate limits.
- The host `https://docs.openwearables.io`.
