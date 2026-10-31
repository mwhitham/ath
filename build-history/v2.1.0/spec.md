# Athletic Standard 2.1.0

Status: **pending Mike's OK**. Not approved for build.

This release builds the source mapping [D161](../v2.0.0/decisions.md) describes. A source whose reader already produces named fields is imported by adding a mapping file. The file lists their field and our field. A mapped field becomes the typed value. Any other field stays in `supplied_fields` under the name it arrived with.

`ath pull openwearables` is not this release. This release does not call Open Wearables or Junction.

The field table is [v2.0.0/appendix/source-formats.md](../v2.0.0/appendix/source-formats.md). Id, start, end, and nested stage paths were read from those public pages on 31 October 2026. The study's word `source.slug` is the description Junction gives `source.provider`. The shipped Junction file reads `source.provider`.

This pull request is the plan. The build after Mike's OK is three later pull requests, in this order.

## How 2.1.0 is delivered

One release, 2.1.0. Three pull requests. Each one merges before the next starts. Version options A and B stay side by side until Mike picks. The pick is written in the third pull request.

| Pull request | What lands | Decisions |
|---|---|---|
| (a) Baseline | The comparison harness and the 2.0.0 database copy, taken from main `d418a8f653f7074ed9e08208c4eb555c391bcbc7` with no other edit | none |
| (b) Mapping | The engine, `ath import --mapping`, the fixtures, the pack checks | D177–D183 |
| (c) Version | Option A or option B, then the progress-note cleanup in section 9 | D184 |

Rough engineering time, for a reviewer planning the work. These are not a promise.

- (a) is about 2 engineering days. It adds `tests/import-baseline.test.ts`, the comparison JSON files under `tests/fixtures/import-baseline/`, and `tests/fixtures/version-2-0-0/database.ath.sqlite`. It edits `scripts/check-public-data.ts` and `.gitignore`. It does not change a reader or a version constant.
- (b) is about 8 engineering days. It adds `crates/ath-core/src/mapping.rs`, the two files under `mappings/`, the two invented fixtures, the mapping tests, and `field_basis` (D185). It edits `store.rs`, `main.rs`, `lib.rs`, `src/v1/schema.ts`, `src/skill.ts`, `src/program.ts`, `src/help.ts`, `src/update.ts`, the generated `cli.md`, `package.json`, `scripts/smoke-installed-package.mjs`, and `.github/workflows/ci.yml`. It adds `build-history/v2.1.0/decisions.md` through D183 and D185. The tool skill `metadata.version` stays `2.0.0`, equal to `ATH_PACKAGE_VERSION`, because `tests/skill.test.ts` line 105 requires that. It does not change `athleticstandard_version`.
- (c) is about 2 engineering days. Option A adds migration 4, the generator and drift checks for that file, and regenerates the schema folder and the demo athlete. Option B changes the program version and does not edit `build-history/README.md`. Both set the tool skill version equal to the package version, write D184, delete the heading in section 9, and write `build-history/v2.1.0/progress.md`.

## 1. The shipped mapping files

A mapping file is markdown. The program reads the first fenced block whose info string is `ath-mapping`. Text outside that block is for a person. The grammar is D177. The build writes these two files in pull request (b).

Shipped paths:

- `mappings/open-wearables.md`
- `mappings/junction.md`

`package.json` `files` gains `"mappings"`.

### `mappings/open-wearables.md`

~~~~markdown
# Open Wearables

Sleep sessions, workouts, and timeseries samples. The provider is `source.provider`. The device is `source.device_type`, `source.device`, and `source.source`. Stage minutes are under `stages`. `efficiency_percent` stays under that name. `entry_source` `manual` is left out. `intensity` stays under that name. A timeseries type is stored only when a line below names it.

```ath-mapping
name: open-wearables
route: open_wearables
writer_field: source.provider

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
type: activity_type
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
type.skin_temperature: skin_temperature °C °C
type.skin_temperature: wrist_temperature_sleeping °C °C when-writer apple
type.oxygen_saturation: oxygen_saturation % %
type.heart_rate_recovery_one_minute: hr_recovery bpm bpm
type.vo2_max: provider_summary.vo2_max mL/kg/min ml/kg/min
type.garmin_stress_level: provider_score.garmin_stress_level score score scale 1-100
type.garmin_body_battery: provider_score.garmin_body_battery score score scale 0-100
```
~~~~

`duration_seconds` stays in `supplied_fields`. `intensity` stays in `supplied_fields` under that name (D153, D161). It is not a measured record. `name` is stored as the exact string. `activity_type` is stored as sent. The workout's `field_basis.activity_type` is `open_wearables_normalized`, because Open Wearables normalizes that word (D185). `field_basis` is not inside `supplied_fields`.

`avg_heart_rate_bpm` and `max_heart_rate_bpm` each become a `provider_summary`. The summary types are `average_heart_rate` and `max_heart_rate`. The shipped basis is `provider_or_open_wearables`, because a workout object has no `computed_by` flag, so the file does not claim the number is the provider's alone. `field_basis.value` holds that basis (D185). A later object may send `{their_field}_computed_by`. That sibling is the upgrade path: if it equals `open_wearables`, it is consumed and `field_basis.value` becomes `open_wearables`. The shipped fixture does not send it. `maximum_heart_rate_bpm` is never written (D172).

`hr_zones` is not a line. The object stays nested in `supplied_fields`. A line that aims `hr_zones.max_hr` at `maximum_heart_rate_bpm` is the D172 refusal in section 4.

A timeseries type with no line is `left_out`. It is not a measured record. That includes `dietary_*`, `workout_effort_score`, `estimated_workout_effort_score`, and the temperature-deviation types. Those deviations wait until each vendor's own baseline is checked. Section 12 carries that forward. `vo2_max` is a `provider_summary`, not the point quantity `vo2_max`.

`garmin_stress_level` and `garmin_body_battery` are `provider_score` records. The scale is on the line. When `computed_by` is missing, the writer is the connector, `open_wearables`, not the device maker. When `computed_by` is `provider`, the writer stays `source.provider`. When `computed_by` is `open_wearables`, the writer is `open_wearables`. The device maker stays on the `source_device` row either way (D183). Test `missing_computed_by_uses_the_connector`.

Open Wearables `/health-scores` is not in this file. Section 12 carries it.

### `mappings/junction.md`

~~~~markdown
# Junction

Sleep summaries. The provider is `source.provider`. The device is `source.type`, `source.device_id`, and `source.app_id`. `average_hrv` maps to `hrv_rmssd` only with basis `hrv_mean_rmssd`, and only when the writer is oura, whoop, garmin, fitbit, or polar. `duration` stays under `duration`. `efficiency` stays under `efficiency`. There is no activity section.

```ath-mapping
name: junction
route: junction
writer_field: source.provider

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
average_hrv: hrv_rmssd ms ms basis hrv_mean_rmssd when-writer oura,whoop,garmin,fitbit,polar
score: provider_score.sleep.score score score scale 1-100
recovery_readiness_score: provider_score.recovery_readiness_score score score scale 0-100
```
~~~~

There is no `[activity]` section. Junction activity summaries are dated at UTC midnight. [D88](../v1.0.0/decisions.md) says a date-only record does not receive an invented midnight timestamp. [D110](../v1.0.0/decisions.md) says a day total is not a measurement at one instant. An `activity` array in the input is `left_out`. It is not stored.

`total` writes `asleep_seconds` only when `stage_asleep_second` is missing or null. Otherwise `total` stays in `supplied_fields`.

The older stage fields `awake`, `light`, `rem`, and `deep` are not lines. A `0` there stays in `supplied_fields`. A null `stage_*_second` is omitted.

`score` is stored as `provider_score` metric `sleep.score`. `recovery_readiness_score` keeps that metric name.

`hrv_mean_sdnn` is not on the summary object. This file has no line for it. A field literally named `hrv_mean_sdnn` may map to `hrv_sdnn`, because the name says SDNN.

`average_hrv` from any other writer stays in `supplied_fields` under `average_hrv`. Junction's provider slug for Apple Health is `apple_health_kit`. Health Connect's slug is `health_connect`. Samsung's slug is `samsung`. None of those is on the `when-writer` list. No `hrv_rmssd` point is written for them. The five names on the list are Junction `source.provider` slugs for providers whose summary this release treats as RMSSD. The `hrv_mean_rmssd` basis is the query-layer name in the study. A line that aims `average_hrv` at `hrv_sdnn` is refused (section 4).

## 2. How a mapping runs

The runner is new Rust in `crates/ath-core`, pull request (b). D155 put new imports there. D173 says the tests still have to pass. The Apple Health, WHOOP, Oura, Garmin, and Strava readers stay on their current functions.

### Types and functions

New module `crates/ath-core/src/mapping.rs`, declared from `lib.rs`.

```rust
pub struct MappingFile { /* name, route, writer_field, sections */ }
pub struct MapLine { /* their_field, our_field, units, basis, scale, when_absent, when_writer */ }

pub fn parse_mapping(text: &str) -> Result<MappingFile, String>
pub fn check_mapping(file: &MappingFile) -> Result<(), String>
pub fn apply_mapping(store: &mut Store, file: &MappingFile, input: &Value, write: bool) -> Result<Value, String>
pub fn input_receipt(text: &str) -> Value
pub fn convert_unit(value: f64, their_unit: &str, our_unit: &str) -> Result<f64, String>
pub fn is_mapping_path(name: &str) -> bool
```

`parse_mapping` strips a trailing `\r` from each line, so a CRLF file parses the same as a LF file. Test `crlf_mapping_file_parses`.

`is_mapping_path` in Rust is true when the string contains `/` or `\`, or matches a drive prefix such as `C:`. The TypeScript command does not call it. `resolveMappingFile` in `src/skill.ts` resolves the name to an absolute path and passes that path as `--mapping-file`. Rust then only reads the file. The same path rule lives in TypeScript as `isMappingPath`, next to `resolveMappingFile`, so a Windows string can be tested where the TypeScript tests run. `tests/mapping-path.test.ts` asserts `isMappingPath("C:\\mappings\\mine.md")` and `isMappingPath("mappings\\mine.md")` on ubuntu. `windows_path_is_a_file_path` in `mapping.rs` asserts the Rust function on the same strings, and that test also runs on `macos-latest` and `windows-latest`.

`main.rs` gains `import-mapped` with `--database`, `--mapping-file`, `--input`, and `--preview`.

`Store::ensure_mapped_source(writer, route, vendor, sensor_type)` looks up `data_writer`, `data_route`, and `sensor_type` together. It does not call the empty-sensor query in `ensure_source` (`store.rs` around line 175). Built-in importers keep calling `ensure_source`.

`Store::insert_group` writes every record from one source object in one transaction. Today `insert_record` opens a transaction and commits it (`store.rs` lines 229 and 259), and `revise` opens another and commits it (lines 94 and 169). `insert_group` does not call those wrappers. It opens one transaction and runs the same match lookup those lines already use: `store.rs` lines 204–221 look up `record_import_keys`, compare with `json_equal`, and either return the existing id or replace the row. `apply_mapping` copies the stored `received_at` onto the new JSON before that compare, so an unchanged re-run is equal and does not write. This is D182.

The update inside that transaction replaces the record. It does not merge. `revise` merges a patch into the current object (`store.rs` lines 82–84) and only refreshes typed columns for `workout_session` and `provider_summary` (lines 119–163). A mapping update must not use that merge, or a key that the new object omits would survive. The replace writes the new object as the whole `record_json`. It refreshes typed columns for `workout_session`, `provider_summary`, `point_measurement`, `provider_score`, `source`, and `source_device`. A column whose key is absent is set to NULL. `revise` stays a merge for `ath-core revise --patch`, and it gains the same column refresh for `point_measurement` and `provider_score` so a patch of those records updates the columns, not only `data_json`.

`typed_table` in `store.rs` gains `"provider_score" => Ok("provider_scores")` and `"source_device" => Ok("source_devices")`. `insert_typed` gains both arms. The `provider_score` columns are already in `src/db/migrations/001-initial.ts`. The `source_device` columns are `record_id`, `record_type`, `source_id`, `name`, `manufacturer`, `model`, `hardware`, `serial_number`, `coverage_start_date`, `coverage_end_date`, `accepted_record_count`, and `data_json`. The source insert at `store.rs` lines 315–316 writes `sensor_type` as SQL `NULL` and ignores the JSON. `ath check` compares that column with `record.data.sensor_type` through `typedMainRowFields` (`src/db/records.ts` line 253) and `mainRowMismatch` (`src/db/typed-denormalized.ts` lines 54–59). The source insert must bind `sensor_type` from the JSON, or `mapped_imports_pass_ath_check` fails. No new table and no new column.

### What a line writes

`check_mapping` allows:

- On a sleep session: `asleep_seconds`, `in_bed_seconds`, `interruptions`, `stage_minutes.awake`, `stage_minutes.light`, `stage_minutes.deep`, `stage_minutes.rem`.
- On a workout: `activity_type`, `name`, `description`, `text_result`, and `source_reported_summaries.distance`, `source_reported_summaries.elevation_gain`, `source_reported_summaries.energy`. Sleep summary types such as `sleep_duration` and `deep_sleep` are not workout targets.
- A point quantity in `POINT_MEASUREMENT_UNITS`. `hrv_rmssd` and `hrv_sdnn` also pass section 4. The our-unit must equal that quantity's unit in `POINT_MEASUREMENT_UNITS`.
- `provider_summary.` plus a non-empty summary type.
- `provider_score.` plus a metric. The metric may contain dots (`sleep.score`). The line includes `scale`.

A numeric target needs a unit pair. `activity_type`, `name`, `description`, and `text_result` take none.

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

A Junction child of `jn-sleep-1` stores `timestamp_text` `2026-08-09T10:00:00.000-04:00`, `utc_offset_text` `-04:00`, `utc_offset_meaning` `meaning_not_documented`, and `timestamp_basis` `source_offset_timestamp`. `field_basis` on the child holds the value basis when the line has one (D185). The child does not repeat the session's leftovers. `field_basis` is not inside `supplied_fields`.

A timeseries sample uses `time_field` and `value_field`. The sample's `type` selects `type.<that string>`. If the sample's `unit` differs from the line's their-unit, the sample is `left_out` and nothing is written for it:

```text
timeseries heart_rate_variability_rmssd 2026-08-09T07:00:00.000Z: unit bpm does not match ms
```

The id in that string is the UTC timestamp when the sample has no id.

### `supplied_fields`

Nested objects stay nested. A leaf that was used is removed. An object that becomes empty is dropped. Used means a mapping line consumed the leaf, or a section setting was read (`id_field`, `start_field`, `end_field`, `time_field`, `value_field`), or `writer_field` was read, or a device field was read for D183 (`source.source`, `source.device`, `source.device_type`, `source.device_id`, `source.device_name`, `source.type`, `source.app_id`).

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

Each part is escaped before the parts are joined with `:`. A `\` in a part becomes `\\`. A `:` in a part becomes `\:`. The sensor string can contain `|`. It must not contain a raw `:`. Two objects in one file that produce the same match key do not both write. The first is stored. The second is `left_out` as `duplicate match key {key}`. A later import of a new file still updates that row (D182).

`provider_record_id` is their id when the object has one. A timeseries sample has no id.

### Re-run

D182. A second run uses the same method and key. `insert_record` already looks up that row, compares with `json_equal`, and replaces it when the JSON differs (`store.rs` lines 204–221). `insert_group` runs that lookup in its one transaction and replaces the whole object. It does not call `revise`, which merges (`store.rs` lines 82–84). Before the compare, `apply_mapping` sets `received_at` to the stored value. An unchanged import does not write a revision. A changed mapped value keeps the record id, increases `current_revision` by one, and keeps the first `received_at`.

### The source row

D183. `ensure_mapped_source(writer, route, vendor, sensor_type)` stores `source_type` `export_file`, `data_writer`, `data_route`, `vendor`, and `sensor_type`. The lookup is those three: writer, route, and sensor. A different sensor is a different source row and a different baseline, because baselines read `source_id`.

Open Wearables sensor string, empty parts skipped, joined with `|`: `source.device_type`, then `source.device`, then `source.source`.

Junction sensor string, the same way: `source.type`, then `source.device_id`, then `source.app_id`.

A `source_device` record is written for that source:

| | Open Wearables | Junction |
|---|---|---|
| `name` | `source.device_name`, else `source.device` | `source.type` |
| `model` | `source.device` | `source.device_id` |
| `hardware` | `source.device_id` | `source.app_id` |
| `manufacturer` | `source.provider` | `source.provider` |

An Apple Watch, an iPhone, and a third-party app are three sensor strings, three source rows, and three baselines.

When a score's `computed_by` is `open_wearables` or `junction`, `data_writer` and `vendor` are that route name. `source_device.manufacturer` stays `source.provider`, so the device maker is still named. The score's baseline is the connector's source, not the device maker's source.

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

`src/help.ts` `EXAMPLES.import` shows `--mapping open-wearables`. `EXAMPLES.skill` says mapping files are copied into `.agents/mappings` when that copy is missing. Then `pnpm generate:skill-cli` rewrites `skills/athletic-standard/references/cli.md` (D80). The tool skill `metadata.version` stays `2.0.0` in pull request (b). `tests/skill.test.ts` line 105 requires it to equal `ATH_PACKAGE_VERSION`. Pull request (c) sets both to `2.1.0`. Calculation skill files stay at `2.0.0`.

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

`left_out` reasons:

- `missing source.provider`
- `missing start_time`
- `missing end_time`
- `end is before the start`
- `value is not a number`
- `unit {sample unit} does not match {their_unit}`
- `unmapped type {type}`
- `entry_source manual is a personal report`
- `duplicate match key {key}`
- `activity: no mapping section`

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

Test `person_adds_my_source`: write `.agents/mappings/my-source.md` with route `my_source`, one invented sleep object, `ath import <file> --mapping my-source --yes`. The sleep record exists. The source `data_route` is `my_source`. `ath skill` does not list `my-source`.

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

`intensity` is not a refusal and not a `left_out` line. Test `intensity_stays_in_supplied_fields` uses `ow-workout-1`. `supplied_fields.intensity` is `moderate`. No measured field is named `intensity`.

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
- A bad line: `could not read '{line}'`
- Unknown section: `unknown mapping section {name}`
- Unknown our-field: `unknown field {our_field}`
- Two `when-absent` lines for one our-field: `two when-absent lines write {our_field}`
- `basis` on a field other than the two heart-rate variability quantities or a `provider_summary` heart-rate line: `basis is only used for hrv_rmssd, hrv_sdnn, and heart-rate summaries`
- `scale` on anything except `provider_score`: `scale is only used for provider_score`
- `provider_score` without `scale`: `provider_score {their_field} needs a scale`
- A point our-unit that is not that quantity's `POINT_MEASUREMENT_UNITS` value: `{quantity} uses {unit}`
- A unit pair outside D180: `cannot convert {their_unit} to {our_unit} for {their_field}`
- A built-in route: `route {route} is already used by a built-in importer`
- A built-in method: `match key method {route}/v1 is already used by a built-in importer`

Built-in routes: `garmin_account_export`, `garmin_personal_api`, `strava_bulk`, `apple_health`, `whoop_csv`, `oura_csv`, `manual_entry`, `unknown_import`. `manual_entry` and `unknown_import` are the routes `sourceData` writes in `src/db/provider-import.ts` lines 261–268.

Built-in methods: `garmin_account/v1`, `garmin_personal/v1`, `strava_bulk/v1`, `apple_health/v1`, `whoop_csv/v1`, `oura_csv/v1`.

Test `builtin_route_is_rejected` uses route `garmin_account_export`.

## 5. Proposed decisions

Numbers continue from D176. D152 through D176 are not rewritten. Pull request (b) writes D177–D183 and D185. Pull request (c) writes D184 after Mike picks A or B, and places it before D185. The previous draft called the version decision D181. That number is now the match key, so the list has no gap.

### D177. A mapping file is an `ath-mapping` block

A mapping file is markdown. The program reads the first fenced block tagged `ath-mapping`. `name`, `route`, and `writer_field` are read only before the first `[section]`. A later `name:` line, such as `name: name` under `[workout]`, is a mapping line: their field `name` to our field `name`.

A section is `[sleep]`, `[workout]`, or `[timeseries]`. `[activity]` is not a shipped section. Settings `id_field`, `start_field`, `end_field`, `time_field`, and `value_field` locate the record. Every other line is:

```text
their_field: our_field [their_unit our_unit] [basis TOKEN] [scale TOKEN] [when-absent] [when-writer a,b,c]
```

A dotted their-field reads that path. `type.<name>` matches a timeseries sample's `type`. A numeric target needs a unit pair. `activity_type`, `name`, `description`, and `text_result` take none.

`when-absent` uses the line only when every earlier line that writes the same our-field had no number. The unused their-field stays in `supplied_fields`. At most one `when-absent` line writes a given our-field.

`when-writer` is a comma-separated list with no spaces. The line is applied only when `data_writer` is in the list. Otherwise their field stays in `supplied_fields` and the import continues. When two lines name the same their-field, the line whose `when-writer` matches is the one that writes. The line without `when-writer` is the fallback. That is how Apple `skin_temperature` becomes `wrist_temperature_sleeping` and every other writer keeps `skin_temperature`. `apple.rs` `point_mapping` maps `AppleSleepingWristTemperature` to `wrist_temperature_sleeping` (line 1122). The unit is `°C`.

A trailing `\r` is stripped. Text outside the fence is not read.

Rejected alternatives:

- A JSON file. Rejected because a person already edits a calculation as a fenced block.
- A CSV of two columns. Rejected because a row cannot name the record, the unit pair, the basis, and the writer list without a second header language.
- Reuse the `ath-calculation` tag. Rejected because `ath apply` would run a mapping as a calculation.
- Treat `name` inside a section as the file name. Rejected because the workout field `name` is their field.

### D178. Shipped mappings live in `mappings/`, and a person's copy lives in `.agents/mappings/`

The shipped files are `mappings/open-wearables.md` and `mappings/junction.md`. A person's file is `.agents/mappings/<name>.md`. `ath init`, `ath skill install`, and `ath --update` copy a shipped file only when that destination is missing. They do not replace it. `ath init --no-skill` skips `installMappings` along with `installSkill`.

An installed file wins over a later shipped file. That is accepted. A mapping is not the tool skill, so D164's version replace does not apply.

Mapping files are not `skills/<name>/SKILL.md`.

Rejected alternatives:

- `skills/open-wearables/SKILL.md`. Rejected because `skillNames` treats every `SKILL.md` as a skill.
- Replace the installed file when the shipped file changes. Rejected because the person may have edited it, which is the reason D157 leaves a calculation in place.
- Give mappings a version and replace older copies. Rejected for this release. The installed file is the one the person added.

### D179. The installed file is read first, and it is checked before a write

`resolveMappingFile` uses the installed file, then the shipped file, then `no mapping file named <name>`. The entry point is `ath import <path> --mapping <name>`. `--mapping` is read before import detection. D174 is unchanged: the flag stays in the TypeScript command.

`check_mapping` accepts the file or returns one message. The four refusals are:

1. A personal report turned into a measured record. D161. Message: `refusing to turn a personal report into a measured record`.
2. A generic heart-rate variability field renamed to `hrv_rmssd` or `hrv_sdnn`. D161. Message: `refusing to map {their_field} onto {our_field}; the field does not name the statistic`.
3. Their efficiency percentage written onto the computed efficiency. `appendix/source-formats.md` lines 29 and 39. Message: `refusing to map {their_field} onto {our_field}; efficiency is computed from asleep_seconds and in_bed_seconds`.
4. A workout peak written onto `maximum_heart_rate_bpm`. D172. The peak includes `max_heart_rate_bpm` and `hr_zones.max_hr`. Message: `refusing to map {their_field} onto maximum_heart_rate_bpm; that field is the athlete's known maximum`.

`check_mapping` also rejects a `route`, or a method `{route}/v1`, that a built-in importer already uses: `garmin_account_export`, `garmin_personal_api`, `strava_bulk`, `apple_health`, `whoop_csv`, `oura_csv`, `manual_entry`, `unknown_import`, and the methods `garmin_account/v1`, `garmin_personal/v1`, `strava_bulk/v1`, `apple_health/v1`, `whoop_csv/v1`, `oura_csv/v1`, `manual_entry/v1`, `unknown_import/v1`. `ensure_source` looks up writer plus route with an empty sensor. A mapping that reused those routes would attach rows to the built-in source. `ensure_mapped_source` is a separate lookup, and this check stops the route string from matching.

`ath check` does not read mapping files. After a mapped import, `ath check` is still run (section 6), because the Rust writer does not call the TypeScript schema parse.

Rejected alternatives:

- Read only the shipped file. Rejected because D161 says a person can add a file.
- Check the mapping inside `ath check` after the write. Rejected because a bad file would already have stored a measured record.
- Guess the mapping from the JSON keys, with no `--mapping` flag. Rejected because a Garmin file and a mapped file are both JSON, and the Garmin reader has to keep running when the flag is absent.
- Let a mapping use `garmin_account_export`. Rejected because that route is the Garmin file source.

### D180. Units convert by a fixed pair

`convert_unit` accepts only these pairs. Any other pair fails with `cannot convert {their_unit} to {our_unit} for {their_field}`.

| Their unit | Our unit | Result |
|---|---|---|
| `s` | `s` | the same number |
| `min` | `min` | the same number |
| `s` | `min` | divide by 60 in binary64 |
| `ms` | `ms` | the same number |
| `bpm` | `bpm` | the same number |
| `m` | `m` | the same number |
| `kcal` | `kcal` | the same number |
| `count` | `count` | the same number |
| `score` | `score` | the same number |
| `brpm` | `brpm` | the same number |
| `%` | `%` | the same number |
| `°C` | `°C` | the same number |
| `mL/kg/min` | `ml/kg/min` | the same number, stored lowercase |

`convert_unit` returns a binary64 float. The stored number is that IEEE-754 result. When the value is a whole number inside the safe integer range, JSON writes it as an integer, so 600 seconds stored as seconds is `600`, not `600.0`. Any other value uses serde_json's default float format, which round-trips through binary64. 100 seconds converted to minutes is the JSON number `1.6666666666666667`. Test `one_hundred_seconds_is_the_binary64_minute` asserts that number. Test `whole_seconds_serialize_as_an_integer` asserts that 600 seconds is the JSON number `600`. `600/700` is the JSON number `0.8571428571428571`.

When the target is a point quantity, the our-unit must equal `POINT_MEASUREMENT_UNITS` for that quantity. `oxygen_saturation` uses `%`. `skin_temperature` uses `°C`. `respiratory_rate` uses `brpm`. `hr_recovery` uses `bpm`.

When the target is a `SourceValue`, `input_value` and `input_unit` keep their number and their unit, and `standard_value` and `standard_unit` hold ours. `asleep_seconds`, `in_bed_seconds`, and `stage_minutes` are plain numbers, so the stored number is the converted one. The original field is not also copied into `supplied_fields`.

A sample whose `unit` differs from the line's their-unit is `left_out` (section 2). Test `sample_unit_mismatch_is_left_out`.

Rejected alternatives:

- Store their number unchanged and ignore units. Rejected because Junction stage fields are seconds and `stage_minutes` is minutes.
- A formula on the line. Rejected because a formula can divide asleep by time in bed, or relabel a heart-rate variability field.
- Call the percent unit `percent`. Rejected because the schema unit is `%`.
- Round the binary64 result to a fixed number of decimals. Rejected because the test then depends on a second rounding rule.

### D181. The match key is the writer, the sensor, their id, and the kind

Their record id is the match key, as D161 says. The method is `{route}/v1`. The key is `{writer}:{sensor}:{section}:{id}` for a session. A child adds `:point:`, `:summary:`, or `:score:` and then the quantity, summary type, or metric. A timeseries sample has no id. Its key uses the type and the timestamp normalized to UTC milliseconds, with the same kind prefix. The table is in section 2.

A second device from the same provider has a different sensor string, so it has a different key.

Rejected alternatives:

- Use their id alone. Rejected because a sleep id and a point taken from that sleep would update one row, and two devices that reuse an id would update one row.
- Use the original offset text in a timeseries key. Rejected because `06:12Z` and `02:12-04:00` are one instant.
- Drop a sample that has no id. Rejected because the Open Wearables timeseries object has no id, and the timestamp plus the type is the sample.

### D182. A re-run updates the same row

`insert_record` already looks up the match key, compares the stored JSON with `json_equal`, and replaces the row when they differ (`store.rs` lines 204–221). `insert_group` runs that lookup inside its one transaction. It does not call `revise`, because `revise` merges (`store.rs` lines 82–84) and commits its own transaction (line 169). The mapping update replaces the whole object, so a key that the new object omits does not survive. It sets `received_at` from the stored row before the compare. An unchanged re-run does not write a revision. A changed value keeps the record id and the first `received_at`, and the revision increases by one.

Rejected alternatives:

- Insert a second row on every run. Rejected because D161 says their id is the match key so a re-run updates the same record.
- Treat a new `received_at` as a change. Rejected because the clock would revise every row on every run.
- Add a second compare that ignores `insert_record`. Rejected because the lookup and `json_equal` are already there.

### D183. The source names the device and the path

D161 says the source names the device and the path. The path is `data_route` (`open_wearables` or `junction`). The device is `sensor_type` on the source row, plus a `source_device` record. The strings are in section 2. Open Wearables uses `source.device_type`, `source.device`, and `source.source`. Junction uses `source.type`, `source.device_id`, and `source.app_id`.

An Apple Watch, an iPhone, and a third-party app do not share a source row. They do not share a baseline.

A score Open Wearables or Junction computed itself uses the connector as `data_writer`, not the device maker. `computed_by` `open_wearables` or `junction` selects that. `computed_by` `provider` keeps `source.provider`. `source_device.manufacturer` stays the provider string either way.

`ensure_source` is not changed. Mapped imports call `ensure_mapped_source`.

Rejected alternatives:

- One source per provider, with the model only in `supplied_fields`. Rejected because a watch and a phone would then share a baseline. That reading of "device" is narrower than D161.
- Put the device only on `source_device` and keep one `source_id`. Rejected because baselines group by `source_id`, so the rows would still share a baseline.
- Use the empty-sensor lookup in `ensure_source`. Rejected because a mapped Garmin row would attach to the Garmin file source when the sensor is empty, and a non-empty sensor would not be found by that query.

### D184. The version pick

Written in pull request (c), using the option A text or the option B text in section 8. Mike picks before that pull request starts. Both texts are in this plan. Neither is copied into `decisions.md` until he picks. Pull request (b) writes D185 before D184 exists. Pull request (c) inserts D184 immediately before D185, so the finished file is D177 through D185 with no gap.

### D185. A basis we assign is not a supplied field

`supplied_fields` holds names the source sent (D153, D161). `intensity` is one of those names. It stays there verbatim. It is not a measured record.

`received_at_basis` is already an ATH-owned field on every measured record. `ReceivedAtBasis` in `src/v1/schema.ts` lines 229–235 already allows `import_time`. Mapped imports set that. It is not copied into `supplied_fields`. No new enum value is added.

The activity-type origin and the value basis are ours. They do not go in `supplied_fields`. They go in one new optional object, `field_basis`, on the record:

```ts
field_basis: z.strictObject({
  activity_type: z.literal("open_wearables_normalized").optional(),
  value: z.enum([
    "provider_or_open_wearables",
    "provider",
    "open_wearables",
    "junction",
  ]).optional(),
}).optional()
```

`WorkoutSessionData`, `ProviderSummaryData`, and `ProviderScoreData` in `src/v1/schema.ts` gain that optional key. The objects are strict, so `ath check` rejects the key until the schema has it. Pull request (b) adds it and runs `pnpm generate:json-schema`. The version constant is still `2.0.0` in that pull request, so the regenerated files are under `schema/2.0.0/`. Option A moves them to `schema/2.1.0` in pull request (c). Option B leaves them in `schema/2.0.0/`.

A workout stores `field_basis.activity_type` as `open_wearables_normalized`. A heart-rate summary from the shipped Open Wearables file stores `field_basis.value` as `provider_or_open_wearables`. A score stores `provider`, `open_wearables`, or `junction` from `computed_by`, and `open_wearables` or `junction` when `computed_by` is missing.

Rejected alternatives:

- Put `activity_type_basis` and `value_basis` in `supplied_fields`. Rejected because D153 and D161 keep that object for names the source sent.
- Add a new `received_at_basis` value for mappings. Rejected because `import_time` already names an import.
- Leave the basis off the record. Rejected because a later reader cannot tell a normalized activity type from a name the athlete wrote.

## 6. Acceptance tests

Pull request (a) runs:

```text
pnpm check:public-data
pnpm typecheck
pnpm test
pnpm build
cargo test --manifest-path crates/ath-core/Cargo.toml
```

Pull requests (b) and (c) run those, plus `pnpm check:schema-drift`, and:

```text
npm pack
node scripts/assert-packed-binary.mjs athleticstandard-<version>.tgz
node scripts/smoke-installed-package.mjs
```

`<version>` is `2.0.0` in pull request (b), because the program version has not moved yet. It is `2.1.0` in pull request (c). The smoke script already packs a tarball. It gains two checks: the tarball lists `package/mappings/open-wearables.md` and `package/mappings/junction.md`, and the installed `ath` runs `ath import <Open Wearables fixture> --mapping open-wearables --yes` against the fixture in the repo.

Those pack commands are not steps in `.github/workflows/ci.yml` today. `pnpm check:schema-drift` is not a step there either. Pull request (b) adds them to the `package-smoke` job, after `pnpm build`:

```yaml
- run: pnpm check:schema-drift
- run: npm pack
- run: node scripts/assert-packed-binary.mjs athleticstandard-2.0.0.tgz
- run: node scripts/smoke-installed-package.mjs
```

Pull request (c) changes the tarball name in that step to `athleticstandard-2.1.0.tgz`. The job stays `package-smoke`.

`cargo test --manifest-path crates/ath-core/Cargo.toml` is added to the `ath-core-binary` job, which already runs on `ubuntu-latest`, `macos-latest`, and `windows-latest`. The CRLF test and the Windows path test run on those three.

The baseline test passes unchanged under option A and option B. It does not read the program version.

### Baseline from `d418a8f`

Pull request (a) is branched from `d418a8f653f7074ed9e08208c4eb555c391bcbc7` and does not edit a reader. `tests/import-baseline.test.ts` contains:

```text
BASELINE_FROM = d418a8f653f7074ed9e08208c4eb555c391bcbc7
```

The test runs each importer, builds the comparison string, and asserts that the string is byte-for-byte the committed file. It does not write the file. The committed files are not regenerated in this release. Changing `BASELINE_FROM` fails the test.

The comparison string is UTF-8 JSON with object keys sorted. The walk is:

1. Read every record, every route point in `point_order`, every attachment, every attachment link, and every record link.
2. Delete every key named `received_at` or `imported_at` at any depth, including `received_at` inside record JSON such as a Garmin `max_heart_rate` summary.
3. Delete `created_at` and the database id.
4. Sort records that have a match key by `match_key_method`, then `match_key`, then `record_type`.
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
| `16-package.json` | `examples/demo-athlete/athlete.ath.json` as it is at `d418a8f` |
| `17-apple-zip.json` | `tests/fixtures/exports/apple` zipped at test time under `apple_health_export/` |
| `18-apple-bare-xml.json` | `tests/fixtures/exports/apple/export.xml` as a file |

The personal-day server follows the local server in `personal.rs`. The HRV body includes `LastNightAvg`. The comparison asserts `summary_type` `LastNightAvg`. File names do not contain `export`, `athlete`, `garmin`, `oura`, `whoop`, `healthkit`, or `apple-health`.

### The 2.0.0 database

Copy `examples/demo-athlete/athlete.ath.sqlite` at `d418a8f` to `tests/fixtures/version-2-0-0/database.ath.sqlite`. The sha256 is `442e9a966711660c2a03d701b5de2c55479a78330702083a3a67a182992a7856`. Register that path and hash in `scripts/check-public-data.ts`. Add this line to `.gitignore`:

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

Under option A, the second command applies step 4. The copy then has `athleticstandard_version` `2.1.0` and `sql_schema_version` `4`. The five counts stay 25, 26, 1, 1, and 1. `ath check` exits 0.

Under option B, the second command prints `No migration is required. Direct-read views and writable tables match this ath version.` The version stays `2.0.0`, the schema stays `3`, the five counts stay, and `ath check` exits 0.

Opening the file does not migrate it.

### New fixtures

Both files are invented. Dates are in 2026. Coordinates are invented. Pull request (b) writes them, then adds each path and its sha256 to `approvedSyntheticFixtures`. Add a `!` gitignore line only if that check asks for one.

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
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "skin_temperature", "value": 33.2, "unit": "°C", "source": { "provider": "oura", "source": "invented-app", "device": "invented-ring", "device_type": "ring", "device_id": "ring-1", "device_name": "Invented Ring" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "oxygen_saturation", "value": 98, "unit": "%", "source": { "provider": "oura", "source": "invented-app", "device": "invented-ring", "device_type": "ring", "device_id": "ring-1", "device_name": "Invented Ring" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "heart_rate_recovery_one_minute", "value": 22, "unit": "bpm", "source": { "provider": "oura", "source": "invented-app", "device": "invented-ring", "device_type": "ring", "device_id": "ring-1", "device_name": "Invented Ring" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "vo2_max", "value": 45, "unit": "mL/kg/min", "source": { "provider": "oura", "source": "invented-app", "device": "invented-ring", "device_type": "ring", "device_id": "ring-1", "device_name": "Invented Ring" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "skin_temperature", "value": 33.4, "unit": "°C", "source": { "provider": "apple", "source": "Health", "device": "Watch6,1", "device_type": "watch", "device_id": "watch-a", "device_name": "Invented Watch" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "garmin_stress_level", "value": 15, "unit": "score", "computed_by": "provider", "source": { "provider": "garmin", "source": "invented-app", "device": "invented-watch", "device_type": "watch", "device_id": "watch-1", "device_name": "Invented Watch" } },
    { "timestamp": "2026-08-09T06:13:00.000Z", "type": "garmin_body_battery", "value": 70, "unit": "score", "computed_by": "open_wearables", "source": { "provider": "garmin", "source": "invented-app", "device": "invented-watch", "device_type": "watch", "device_id": "watch-1", "device_name": "Invented Watch" } },
    { "timestamp": "2026-08-09T08:00:00.000Z", "type": "resting_heart_rate", "value": 61, "unit": "bpm", "source": { "provider": "apple", "source": "Health", "device": "iPhone15,2", "device_type": "phone", "device_id": "phone-1", "device_name": "Invented Phone" } },
    { "timestamp": "2026-08-09T08:00:00.000Z", "type": "resting_heart_rate", "value": 52, "unit": "bpm", "source": { "provider": "apple", "source": "Health", "device": "Watch6,1", "device_type": "watch", "device_id": "watch-a", "device_name": "Invented Watch" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "dietary_caffeine", "value": 40, "unit": "mg", "source": { "provider": "oura", "device_type": "ring", "device": "invented-ring", "source": "invented-app" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "workout_effort_score", "value": 7, "unit": "score", "source": { "provider": "oura", "device_type": "ring", "device": "invented-ring", "source": "invented-app" } },
    { "timestamp": "2026-08-09T06:12:00.000Z", "type": "skin_temperature_deviation", "value": 0.2, "unit": "°C", "source": { "provider": "oura", "device_type": "ring", "device": "invented-ring", "source": "invented-app" } },
    { "timestamp": "2026-08-09T07:00:00.000Z", "type": "heart_rate_variability_rmssd", "value": 40, "unit": "bpm", "source": { "provider": "oura", "device_type": "ring", "device": "invented-ring", "source": "invented-app" } }
  ]
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

Test `open_wearables_fixture_maps_fields`. Sensor `ring|invented-ring|invented-app` is `S-ring`. Sensor `watch|invented-watch|invented-app` is `S-watch`. Sensor `phone|iPhone15,2|Health` is `S-phone`. Sensor `watch|Watch6,1|Health` is `S-awatch`.

- `ow-sleep-1` is its own `sleep_session`. `start_at` is `2026-08-09T06:00:00.000Z`. `end_at` is `2026-08-09T14:00:00.000Z`. The start receipt's `timestamp_text` is `2026-08-09T02:00:00.000-04:00`, `utc_offset_text` is `-04:00`, `utc_offset_meaning` is `meaning_not_documented`, and `timestamp_basis` is `source_offset_timestamp`.
- `asleep_seconds` is `600`. `in_bed_seconds` is `700`. `stage_minutes` is `{ "awake": 2, "light": 3, "deep": 4 }` with no `rem` key.
- `supplied_fields` is `{ "zone_offset": "-04:00", "duration_seconds": 800, "efficiency_percent": 91, "is_nap": false }`.
- Match key `oura:S-ring:sleep:ow-sleep-1`, method `open_wearables/v1`.
- The source is writer `oura`, route `open_wearables`, vendor `oura`, `source_type` `export_file`, `sensor_type` `S-ring`. The `source_device` name is `Invented Ring`, model `invented-ring`, hardware `ring-1`, manufacturer `oura`.
- `ow-nap-1` is a second `sleep_session`. It is not merged into `ow-sleep-1`. `supplied_fields` is `{ "efficiency_percent": 80, "is_nap": true }`. `asleep_seconds` is `1000`. `in_bed_seconds` is `1200`. Same source as the night.
- `ow-workout-1` source is writer `garmin`, route `open_wearables`, sensor `S-watch`. `activity_type` is `running`. `name` is `Invented loop`. `source_reported_summaries` holds distance `12` `m` and elevation gain `4` `m`. There is no `maximum_heart_rate_bpm`.
- `supplied_fields` on that workout is `{ "duration_seconds": 3, "intensity": "moderate", "start_latitude": 12.345, "start_longitude": -45.678, "calories_kcal": 1, "hr_zones": { "max_hr": 190 } }`. `field_basis` is `{ "activity_type": "open_wearables_normalized" }`.
- A `provider_summary` `average_heart_rate` is `140` `bpm`, match key `garmin:S-watch:workout:ow-workout-1:summary:average_heart_rate`, `field_basis` `{ "value": "provider_or_open_wearables" }`. A `provider_summary` `max_heart_rate` is `180` `bpm`, key suffix `:summary:max_heart_rate`, with the same `field_basis`. Neither summary has `supplied_fields`.
- `ow-workout-manual` writes no record. `left_out` contains `workout ow-workout-manual: entry_source manual is a personal report`.
- Points at `2026-08-09T06:12:00.000Z` on the oura ring source: `hrv_rmssd` `42.5`, `hrv_sdnn` `61.2`, `resting_heart_rate` `48`, `respiratory_rate` `16` unit `brpm`, `skin_temperature` `33.2` unit `°C`, `oxygen_saturation` `98` unit `%`, `hr_recovery` `22` unit `bpm`. Keys look like `oura:S-ring:timeseries:point:heart_rate_variability_rmssd:2026-08-09T06:12:00.000Z`. Those samples have no `supplied_fields`. Each point's `input_timestamp` is the `Z` receipt in section 2. `asleep_seconds` `600` is the JSON integer `600`.
- The Apple watch sample at the same timestamp is `wrist_temperature_sleeping` `33.4` unit `°C`, writer `apple`, sensor `S-awatch`. It is not `skin_temperature`.
- `vo2_max` is a `provider_summary` summary type `vo2_max`, value `45`, unit `ml/kg/min`, key suffix `:summary:vo2_max`. The sample's unit was `mL/kg/min`. It is not a point quantity `vo2_max`.
- `garmin_stress_level` is a `provider_score` metric `garmin_stress_level`, value `15`, scale `1-100`, writer `garmin`, sensor `S-watch`, `supplied_fields` `{ "computed_by": "provider" }`, `field_basis` `{ "value": "provider" }`.
- `garmin_body_battery` is a `provider_score` metric `garmin_body_battery`, value `70`, scale `0-100`, writer `open_wearables`, vendor `open_wearables`, sensor `S-watch`. `source_device.manufacturer` is `garmin`. `supplied_fields` is `{ "computed_by": "open_wearables" }`. `field_basis` is `{ "value": "open_wearables" }`.
- The phone sample and the watch sample at `2026-08-09T08:00:00.000Z` are two `resting_heart_rate` points, `61` and `52`, on two sources. Sensors are `S-phone` and `S-awatch`. Both writers are `apple`. Match keys differ by that sensor.
- `left_out` contains `timeseries dietary_caffeine 2026-08-09T06:12:00.000Z: unmapped type dietary_caffeine`, the same shape for `workout_effort_score` and `skin_temperature_deviation`, and `timeseries heart_rate_variability_rmssd 2026-08-09T07:00:00.000Z: unit bpm does not match ms`. No record exists for those four samples.

Test `junction_fixture_maps_fields`. Junction oura sensor `ring|ring-j1|oura-app` is `S-jora`. Junction Apple Health Kit sensor `watch|watch-a|health` is `S-japple`. The writer is the slug `apple_health_kit`.

- `jn-sleep-1` `asleep_seconds` is `600`. `supplied_fields` is `{ "duration": 700, "total": 500, "awake": 0, "light": 0, "rem": 0, "deep": 0, "efficiency": 0.88, "LastNightAvg": 40, "type": "long_sleep" }`. There is no `in_bed_seconds`.
- `stage_minutes` is `{ "awake": 2, "light": 3, "deep": 4 }` with no `rem` key.
- A point `hrv_rmssd` is `55.5`, key `oura:S-jora:sleep:jn-sleep-1:point:hrv_rmssd`. A point `resting_heart_rate` is `47`. A point `respiratory_rate` is `14` unit `brpm`. There is no `hrv_sdnn` record.
- `provider_score` metric `sleep.score`, value `80`, scale `1-100`, key suffix `:score:sleep.score`. `provider_score` metric `recovery_readiness_score`, value `70`, scale `0-100`.
- `jn-sleep-2` `asleep_seconds` is `1000`. `supplied_fields` is `{ "duration": 3600 }`.
- `jn-nap-1` is its own `sleep_session`. `supplied_fields` is `{ "duration": 900, "type": "acknowledged_nap" }`. `asleep_seconds` is `800`. It is not merged into `jn-sleep-1`.
- `jn-sleep-apple` has `asleep_seconds` `3000`. `supplied_fields` is `{ "duration": 3600, "average_hrv": 33 }`. The writer is `apple_health_kit`. The import writes no `hrv_rmssd` point for that id. Test name `apple_average_hrv_stays_unmapped`. The child's `input_timestamp.timestamp_text` is `2026-08-11T03:00:00.000Z`.
- `left_out` contains `activity: no mapping section`. No `steps` record is written.
- Sources include writer `oura` route `junction` sensor `S-jora`, and writer `apple_health_kit` route `junction` sensor `S-japple`.

Test `mapped_imports_pass_ath_check`: import both fixtures with `--yes` into a new database, then `ath check` exits 0.

Test `rerun_does_not_add_a_record`: import the Open Wearables fixture twice. One sleep row has `provider_record_id` `ow-sleep-1`. `current_revision` is `1`. One import-key row exists for that match key. Then import a copy whose `sleep_duration_seconds` is `601`. The same record id remains, `current_revision` is `2`, `asleep_seconds` is `601`, and `received_at` is unchanged.

Test `garmin_file_and_mapping_stay_two_sources`: `ath import` of `tests/fixtures/exports/garmin/park-loop.json` with `--yes` and no `--mapping`, then the Open Wearables fixture with `--mapping open-wearables --yes`. Two workout sessions. Writer `garmin` appears on two sources: `data_route` `garmin_account_export` with an empty sensor, and `data_route` `open_wearables` with sensor `S-watch`. The file import still has `supplied_fields.Feeling` `smooth` and a `max_heart_rate` summary of `200`. The mapped workout has `start_latitude` `12.345` and a `max_heart_rate` summary of `180`.

Test `computed_by_open_wearables_sets_field_basis`: a workout JSON that adds the upgrade-path sibling `avg_heart_rate_bpm_computed_by` with value `open_wearables` stores `field_basis.value` `open_wearables` and does not keep the sibling in `supplied_fields`. The shipped fixture has no such sibling, so its summaries stay `provider_or_open_wearables`.

## 7. What this release does not change

- The records, `supplied_fields`, sources, route points, attachments, links, and import receipts produced from the section 6 baseline, including the package, the Apple zip, the bare `export.xml`, and `ath pull garmin`. Those readers are not moved onto mapping files.
- Required record fields. No new column.
- The separation of measured readings and athlete reports. `hrv_rmssd` and `hrv_sdnn` stay two quantities. `LastNightAvg` on the Garmin personal pull stays `LastNightAvg`.
- The TypeScript command front end, except the `--mapping` flag, the help sentences, the generated `cli.md`, and `installMappings`. The front end is not moved to Rust (D174).
- Published migrations `001`, `002`, and `003` (D165).
- Earlier D-numbers and earlier `build-history/` folders, except the one edit in section 9.
- Network pulls, an Open Wearables client, a Junction client, sync, and any hosted service. `import-mapped` has no URL flag.

`src/v1/schema.ts` gains the optional `field_basis` object (D185). It gains no required field and no SQL column. `pnpm generate:json-schema` runs in pull request (b) for that object. `pnpm generate:v1-fixture` runs under option A, as section 8 says.

## 8. Version

Mike has not chosen whether the program version and the record version stay one number. D152 made them one number for 2.0.0. D164 says that choice still holds from 2.0.0 on. Neither decision is rewritten here. The choice is D184, written in pull request (c).

The code uses two constants either way.

- `src/version.ts` exports `ATH_PACKAGE_VERSION`. `ath --version` prints it.
- `src/v1/schema.ts` exports `ATHLETIC_STANDARD_VERSION`. New databases store it. `ACCEPTED_RECORD_VERSIONS` lists the record versions this program can read.
- `crates/ath-core/src/lib.rs` exports `APP_VERSION` and `RECORD_VERSION`. `Store::open` compares `database_metadata.athleticstandard_version` to `RECORD_VERSION`. The error stays `database version is {version}; run ath migrate before using ath-core`. `changed_by` uses `APP_VERSION`. The single `SOFTWARE_VERSION` constant is removed.

Today those strings are all `2.0.0`. Splitting the names is the small change. Setting the strings is D184.

### Option A. One number, 2.1.0

`ath --version` prints `2.1.0`. A new database stores `2.1.0`. `APP_VERSION` and `RECORD_VERSION` are both `2.1.0`.

D184 text if this option is picked:

> D184. The program and the record stay one number, 2.1.0.
>
> `ath --version` and `database_metadata.athleticstandard_version` are `2.1.0`. D152 made them one number so a change does not have to pick which number moves. This release keeps that rule. The mapping does not add a required field. Minor versions may add optional fields, which is the rule in [v0.1.0/spec.md](../v0.1.0/spec.md) at the `athleticstandard_version` line. The optional sleep fields this mapping fills were already on the 2.0.0 record. The number still moves, because the program moved.
>
> Rejected alternatives:
>
> - Leave the record at 2.0.0 while the program becomes 2.1.0. Rejected because D152 and D164 already rejected a split, and a later change would again have to pick which number moves.

What changes:

- `package.json` `version` becomes `2.1.0`. The release workflow packs `athleticstandard-$(version).tgz`. The tarball name becomes `athleticstandard-2.1.0.tgz`. D171's `2.0.0` filename is the 2.0.0 case of that rule. It is not rewritten.
- `ATH_PACKAGE_VERSION`, `ATHLETIC_STANDARD_VERSION`, `APP_VERSION`, and `RECORD_VERSION` become `2.1.0`.
- `ACCEPTED_RECORD_VERSIONS` becomes `["1.0.0", "2.0.0", "2.1.0"]`. `READABLE_DATABASE_VERSIONS` in `src/db/migrate.ts` includes `"1.0.0"`, `"2.0.0"`, and `"2.1.0"`. If option A drops `"2.0.0"` from that set, migrate cannot open the database it is supposed to upgrade.
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
- Rust tests that insert `'2.0.0'` and then call `Store::open` use `RECORD_VERSION`. Inserts that build a 1.0.0 or 2.0.0 file on purpose stay hardcoded. `crates/ath-core/src/persist.rs` `persist_inserts_a_source_and_bumps_the_revision` (line 1039) passes `tool_version` `2.0.0` inside `changed_by_json`. That test does not call `Store::open`. `apply` stores the string it is given. Under option A and option B the tool version is `2.1.0`, so that literal becomes `2.1.0`. The helper `database()` at line 1019 still inserts `athleticstandard_version` `2.0.0`, because it builds a pre-migration file.
- `pnpm generate:json-schema` writes `schema/2.1.0/`. `schema/2.0.0/` stays. `package.json` exports and `tests/v1-package-exports.test.ts` point at `schema/2.1.0/`. Both version constants are asserted equal to `2.1.0`.
- `pnpm generate:v1-fixture` regenerates `examples/demo-athlete/` if the stored version is inside it. The new hashes replace the demo-athlete lines in `scripts/check-public-data.ts`. The fixture is not hand-edited. `tests/fixtures/version-2-0-0/database.ath.sqlite` is not regenerated.
- `README.md` and `SPEC.md` gain `schema/2.1.0/` beside `schema/2.0.0/`. The field list in `SPEC.md` gains no field.
- `build-history/README.md` "Current records" gains the 2.1.0 links and the sentence: version 2.1.0 is the software now. The program, the record, and the database use that number.
- Calculation skill files stay on metadata version `2.0.0`. The tool skill `metadata.version` becomes `2.1.0` in this pull request, in the same commit as `package.json`, so `tests/skill.test.ts` line 105 still sees equal versions.
- Search the tree for `2.0.0` before the last commit of pull request (c). Change a hit only when it is this program's current version. Do not change `003`, historical decisions, the 2.0.0 schema folder, the baseline files, or the version-2-0-0 database copy.

What this asks of a 2.0.0 database: `ath migrate` runs, and no column is added. `user_version` becomes 4. D152 says that integer is the upgrade step. The software version is the `2.1.0` stored beside it.

### Option B. The program is 2.1.0 and the record stays 2.0.0

`ath --version` prints `2.1.0`. A new database still stores `2.0.0`. `APP_VERSION` is `2.1.0`. `RECORD_VERSION` is `2.0.0`. `SQL_SCHEMA_VERSION` stays `3`. There is no migration 004. `applyMigrationStep` gains no arm. `001` and `003` are not edited.

D184 text if this option is picked:

> D184. The program is 2.1.0. The record stays 2.0.0.
>
> `ath --version` prints `2.1.0`. `database_metadata.athleticstandard_version` stays `2.0.0`. This release adds a way to read named fields. It does not add a record field, a column, or a required value. This supersedes D164's rejection of two numbers, and D152's one number, from 2.1.0 on. D152 and D164 are not rewritten.
>
> Rejected alternatives:
>
> - Move both numbers to 2.1.0. Rejected for this release because a migrate step with no new column would be the only effect on a 2.0.0 database.
> - Hide the program version and keep printing 2.0.0 from `ath --version`. Rejected because the mapping command is new and the person needs to see that they are running it.

What changes:

- `package.json` `version`, `ATH_PACKAGE_VERSION`, and `APP_VERSION` become `2.1.0`. The tarball name becomes `athleticstandard-2.1.0.tgz`.
- `ATHLETIC_STANDARD_VERSION` and `RECORD_VERSION` stay `2.0.0`. `ACCEPTED_RECORD_VERSIONS` stays `["1.0.0", "2.0.0"]`. `READABLE_DATABASE_VERSIONS` stays `"1.0.0"` plus the current record version.
- No file under `src/db/migrations/` or `spec/sql/migrations/` is added or edited. `pnpm generate:sql-contract` does not run for a version bump.
- `schema/2.0.0/` already contains `field_basis` from pull request (b). This pull request does not move that folder. The demo athlete fixture is not regenerated. `tests/fixtures/version-2-0-0/database.ath.sqlite` stays the `d418a8f` bytes.
- `tests/v1-package-exports.test.ts` expects package `2.1.0` and record `2.0.0`.
- `ath migrate` on the temp copy prints the existing no-migration sentence. `ath check` passes with schema `3`. `Store::open` accepts `2.0.0`.
- `crates/ath-core/src/persist.rs` line 1039 changes `tool_version` from `2.0.0` to `2.1.0`, because that string is the tool version and `APP_VERSION` is `2.1.0`. The `database()` helper keeps `athleticstandard_version` `2.0.0`.
- The tool skill `metadata.version` becomes `2.1.0` here, with `package.json`. Calculation skills stay `2.0.0`.
- `build-history/README.md` is not edited. The only edit to an earlier `build-history` folder is the heading removal in section 9. The note that the program is 2.1.0 and the record stays 2.0.0 is written in `build-history/v2.1.0/progress.md`, not in the README. That progress file also says a program release that does not change the record still uses the folder named in its plan.
- `README.md` and `SPEC.md` keep their schema links on `schema/2.0.0/`.
- `src/db/backtest.ts` stays on `2.0.0`.

What this asks of a 2.0.0 database: `ath migrate` reports that no migration is required. The person sees `2.1.0` from `ath --version` and `2.0.0` inside the database. D184 says that this supersedes D152 and D164's rejection of two numbers. Those two decisions are not rewritten.

### What is shared

Both options add the two constants and point `Store::open` at `RECORD_VERSION`. Both leave required fields, migrations `001`–`003`, and the existing importers alone. Both run the migrate commands in section 6 on a copy of the named 2.0.0 database. Switching from B to A later is migration 4 plus the string `RECORD_VERSION`. Switching from A to B is not available once a database has been written at `2.1.0`.

## 9. The 2.0.0 progress note

The build deletes the section whose heading is `Later, with the reason:` from `build-history/v2.0.0/progress.md`. The edit finds that heading. It does not find a line number. Mike asked for the deletion. The list is out of date. Pull request (c) makes the edit. This plan pull request does not.

The heading and the bullets under it are removed. The paragraphs above the heading stay.

No file links to that heading. The one link inside the section points at `appendix/source-formats.md`. These still open the study after that section is removed:

- [D161](../v2.0.0/decisions.md), the relative link `appendix/source-formats.md`
- `docs/connections.md`, the link `../build-history/v2.0.0/appendix/source-formats.md`

`docs/connections.md` says an Open Wearables pull is not built. That sentence stays true. `docs/connections.md` is not edited.

`build-history/v2.1.0/progress.md`, written in pull request (c), says Mike asked for the deletion, and it links the study:

```markdown
Mike asked to remove the outdated "Later, with the reason:" heading from
[v2.0.0/progress.md](../v2.0.0/progress.md). The mapping study is
[v2.0.0/appendix/source-formats.md](../v2.0.0/appendix/source-formats.md) (D161).
```

## 10. Notes and proof

Pull request (b) starts `build-history/v2.1.0/decisions.md` with D177–D183. Pull request (c) appends D184 and writes `progress.md`. `build-history/README.md` "Current records" links `spec.md`, `decisions.md`, and `progress.md` in pull request (c). The version sentence is the one from the option Mike picked.

The proof is a screenshot or a playable mp4 on the pull request (b) body, and another on pull request (c) if the version pick changes what `ath --version` prints. The recording is not committed. Pull request (b) shows:

1. `ath import` of the Open Wearables fixture with `--mapping open-wearables --yes`.
2. The `hrv_rmssd` point at `42.5` and the `hrv_sdnn` point at `61.2`.
3. `efficiency_percent` still named `efficiency_percent`, and `start_latitude` still named `start_latitude`.
4. The same import again, with one sleep record and revision 1.
5. The Garmin file import and the mapped workout as two sources.
6. The `LastNightAvg` refusal on stderr, with nothing written.
7. `ath check` exiting 0 after both fixtures.

This plan pull request has no recording.

## 11. Risks, the remaining question, and the order

### Risks

- `typed_table` and `insert_typed` grow a `provider_score` arm. `insert_record`'s match logic stays. The baseline files are the check that the existing readers did not move.
- A mapping route equal to `garmin_account_export` would reuse the built-in source through the empty-sensor lookup. D179 rejects that route before a write.
- `received_at` is inside record JSON. The baseline walk deletes it at any depth. A walk that only deletes the column would fail the Garmin `max_heart_rate` summary.
- Records with no match key need the sort in section 6. A sort that drops them would hide package links.
- Pull request (b) must not bump `ATHLETIC_STANDARD_VERSION`. If it does, the baseline no longer matches `d418a8f`.
- If option A drops `"2.0.0"` from `READABLE_DATABASE_VERSIONS`, migrate cannot open the version-2-0-0 copy.
- Option A runs `003`, which sets version `2.0.0`, and then step 4. Editing `003` instead would violate D165. `applyMigrationStep` must gain the `targetVersion === 4` arm or the step throws.
- Junction `average_hrv` from `apple_health_kit`, `health_connect`, or `samsung` stays in `supplied_fields`. A writer missing from the allow-list is left unmapped, not refused.
- The npm package omits `mappings/` unless `package.json` `files` lists it. The smoke test asserts the tarball paths.

### Question for Mike

1. Do you want option A or option B in section 8?
   - Option A. Recommendation: this one. D152 and D164 already use one number.
   - Option B. The record format gained no field. A 2.0.0 database stays on schema 3. D184 would supersede D152 and D164's rejection of two numbers, and those two decisions would not be rewritten.

2. A workout with `entry_source` `manual` is left out of the measured import. The other objects in the file are still written. The alternative is to refuse the whole file with `refusing to turn a personal report into a measured record`.
   - Leave the object out. Recommendation: this one. One hand-entered workout does not throw away the measured rows in the same file. The test is `ow-workout-manual`.
   - Refuse the whole file.

`--mapping` is in this release. D174 does not need a new D-number. The fourth refusal stays. It is D179 item 4, from D172.

### Order

1. Mike says the plan is OK, and he answers the two questions.
2. Pull request (a), from `d418a8f`, with no other change: the baseline files, the test, and `tests/fixtures/version-2-0-0/database.ath.sqlite`.
3. Pull request (b): D177–D183, the engine, the flag, the fixtures, the pack checks, `cargo test` on the three operating systems.
4. Pull request (c): D184 for the option he picked, the migrate commands on a copy of the named database, and section 9.

## 12. Not in 2.1.0

These are outside this release. This section does not assign them to a later version.

- Real paged fixtures from Open Wearables and Junction, instead of one invented page.
- Sleep reads that pass `include=stages` and `filter_by_priority=false`. Timeseries reads at `raw` or `1day`.
- A check for an old self-hosted Open Wearables server that still has Oura RMSSD stored under the SDNN type. The study names that relabel. This release trusts the type string the server sends.
- A mapping-file version stored on each record the mapping writes.
- Open Wearables `/health-scores` is not read.
- Temperature-deviation types are `left_out`. This release does not check a vendor baseline for them.
- Junction activity summaries are `left_out`. This release does not store a UTC midnight as a measurement (D88, D110).
