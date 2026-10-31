# Athletic Standard 2.1.0

Status: plan, for review. Not approved for build.

This release builds the source mapping [D161](../v2.0.0/decisions.md) describes. A source whose reader already produces named fields is imported by adding a mapping file. The file lists their field and our field. A mapped field becomes the typed value. Any other field stays in `supplied_fields` under the name it arrived with.

`ath pull openwearables` is a later release. This release does not call Open Wearables or Junction, and it does not add a sync.

This pull request adds only this file. The build that follows Mike's approval adds the code, the decisions, and the progress notes named below.

The field table this mapping follows is [v2.0.0/appendix/source-formats.md](../v2.0.0/appendix/source-formats.md), checked for that study on 27 October 2026. Id, start, end, and nested stage paths below were read from the same public pages on 31 October 2026, because the study names the measurements and not those structural fields. Where a 31 October name differs from the study, section 11 asks Mike before the build treats it as decided.

## 1. The two shipped mapping files

A mapping file is markdown. The program reads the first fenced block whose info string is `ath-mapping`. Text outside that block is for a person. The grammar, the folder, and the checks are D177–D180 below. The build writes these two files byte for byte.

Shipped paths:

- `mappings/open-wearables.md`
- `mappings/junction.md`

`package.json` `files` gains `"mappings"` so a packed install contains them. `tests/v1-package-exports.test.ts` asserts that entry.

### `mappings/open-wearables.md`

~~~~markdown
# Open Wearables

Sleep sessions, workouts, and timeseries samples. The provider is `source.provider`. Stage minutes are under `stages`. `efficiency_percent` stays under that name. A timeseries type maps when the type name says RMSSD or SDNN, or when the type is resting heart rate.

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
avg_heart_rate_bpm: source_reported_summaries.average_heart_rate bpm bpm
max_heart_rate_bpm: provider_summary.max_heart_rate bpm bpm

[timeseries]
time_field: timestamp
value_field: value
type.heart_rate_variability_rmssd: hrv_rmssd ms ms
type.heart_rate_variability_sdnn: hrv_sdnn ms ms
type.resting_heart_rate: resting_heart_rate bpm bpm
```
~~~~

`duration_seconds` on a sleep or a workout is not a line. The start and end instants are. Their duration number stays in `supplied_fields`. `max_heart_rate_bpm` becomes a `provider_summary` with `summary_type` `max_heart_rate`. It does not become `maximum_heart_rate_bpm`. That field is the athlete's known maximum (D172).

Heart-rate zones, power zones, and lap segments are not lines. When a reader includes them, they stay in `supplied_fields` under the names they arrived with.

Open Wearables health scores (`recovery`, `strain`, `readiness`, `body_battery`) are a separate object. This file does not guess that object's keys. Section 11 asks whether they join this file before the build.

A timeseries sample whose `type` has no line is still stored. `summary_type` is that type string. The number is the value. The sample's `unit` is the unit. Other keys on that sample stay in `supplied_fields`. `garmin_stress_level` is the fixture for this. It is not renamed to a point quantity.

### `mappings/junction.md`

~~~~markdown
# Junction

Sleep summaries and activity summaries. The summary object names the provider `source.provider` and describes that string as the provider slug. `average_hrv` maps to `hrv_rmssd` only with basis `hrv_mean_rmssd`, which is the name Junction's query layer gives that column. The summary field is still called `average_hrv`. `duration` is the length of the sleep period. It stays under `duration`. `efficiency` stays under `efficiency`.

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
average_hrv: hrv_rmssd ms ms basis hrv_mean_rmssd
score: provider_score score score scale 1-100
recovery_readiness_score: provider_score score score scale 0-100

[activity]
id_field: id
time_field: date
steps: provider_summary.steps count count
calories_active: provider_summary.calories_active kcal kcal
calories_total: provider_summary.calories_total kcal kcal
heart_rate.resting_bpm: resting_heart_rate bpm bpm
```
~~~~

`hrv_mean_sdnn` is a query-layer column. The sleep summary does not return it under that name, so this file has no line for it. A later file may map a field that is literally named `hrv_mean_sdnn` to `hrv_sdnn`, because that name says SDNN. `average_hrv` is not that field.

The older stage fields `awake`, `light`, `rem`, and `deep` are not lines. Junction's docs say those older fields returned `0` when the provider sent no stage. A `0` there stays in `supplied_fields` under that name. A null `stage_rem_second` is omitted. It is not stored as zero minutes.

`total` writes `asleep_seconds` only when `stage_asleep_second` is missing or null. When both are present, `stage_asleep_second` is the typed value and `total` stays in `supplied_fields`.

`score` and `recovery_readiness_score` each become a `provider_score`. The metric is their field name. The scale is the scale written on the line (`1-100` or `0-100`). The unit is `score`. They stay that provider's scores.

Junction workouts are not in this file. This release maps sleep and activity.

## 2. How a mapping runs

The runner is new Rust in `crates/ath-core`. D155 put new imports there. D173 says a move does not wait on a hand re-run, and the tests still have to pass. The existing Apple Health, WHOOP, Oura, Garmin, and Strava readers stay on their current functions. They are not rewritten as mapping files.

### Types and functions

New module `crates/ath-core/src/mapping.rs`, declared in `crates/ath-core/src/lib.rs`.

```rust
pub struct MappingFile {
    pub name: String,
    pub route: String,
    pub writer_field: String,
    pub sections: BTreeMap<String, Section>,
}

pub struct Section {
    pub kind: SectionKind,
    pub id_field: Option<String>,
    pub start_field: Option<String>,
    pub end_field: Option<String>,
    pub time_field: Option<String>,
    pub value_field: Option<String>,
    pub lines: Vec<MapLine>,
}

pub enum SectionKind {
    Sleep,
    Workout,
    Timeseries,
    Activity,
}

pub struct MapLine {
    pub their_field: String,
    pub our_field: String,
    pub their_unit: Option<String>,
    pub our_unit: Option<String>,
    pub basis: Option<String>,
    pub scale: Option<String>,
    pub when_absent: bool,
}

pub fn parse_mapping(text: &str) -> Result<MappingFile, String>
pub fn check_mapping(file: &MappingFile) -> Result<(), String>
pub fn apply_mapping(store: &mut Store, file: &MappingFile, input: &Value, write: bool) -> Result<Value, String>
pub fn input_receipt(text: &str) -> Value
pub fn convert_unit(value: f64, their_unit: &str, our_unit: &str) -> Result<f64, String>
```

`crates/ath-core/src/main.rs` gains command `import-mapped`. Flags: `--database`, `--mapping-file`, `--input`. `--preview` builds the result and writes nothing. The unknown-command string in `main` lists `import-mapped` with the commands it already lists.

`apply_mapping` reads a JSON object. Each key that matches a section name is an array of objects. Any other top-level key is named in `left_out` and is not written. One source object is written in one transaction, by a new `Store::insert_group` in `crates/ath-core/src/store.rs`. `insert_record` is not changed. Garmin, Strava, and `pull_garmin` keep calling `insert_record`.

`insert_typed` gains one arm, `provider_score`, writing the columns already created in `src/db/migrations/001-initial.ts`: `record_id`, `record_type`, `source_id`, `metric`, `recorded_at`, `standard_value`, `standard_unit`, `scale`, `provider_record_id`, `received_at`, `data_json`. No new table and no new column. Published migrations stay as they are (D165).

### What a line writes

`check_mapping` allows these `our_field` values:

- On a sleep session: `asleep_seconds`, `in_bed_seconds`, `interruptions`, `stage_minutes.awake`, `stage_minutes.light`, `stage_minutes.deep`, `stage_minutes.rem`.
- On a workout: `activity_type`, `name`, `description`, `text_result`, and `source_reported_summaries.` plus one of `distance`, `average_heart_rate`, `max_heart_rate`, `energy`, `elevation_gain`, `sleep_duration`, `time_in_bed`, `deep_sleep`, `rem_sleep`, `light_sleep`, `awake`, `sleep_interruptions`. Those summary type strings are the ones `summaryNames` already uses in `src/db/provider-import.ts`. `sleep_efficiency` is left out of this list. The shipped workout line uses `provider_summary.max_heart_rate` for the peak, which is the same summary type Garmin already stores.
- A point quantity in `POINT_MEASUREMENT_UNITS` in `src/v1/schema.ts`. `hrv_rmssd` and `hrv_sdnn` also have to pass the name rule in section 4.
- `provider_summary.` plus a non-empty summary type. The summary type is kept as written. `provider_summary.max_heart_rate` stays `max_heart_rate`.
- `provider_score`. The metric stored is their field name. The line must include `scale`.

`start_field`, `end_field`, `time_field`, `id_field`, and `value_field` are section settings. They are not mapping lines. They locate the record.

A sleep or workout object is written when both times parse and the end is not before the start. `start_at` and `end_at` are that instant in UTC with millisecond precision, using `crate::time::format_datetime`. `input_start_timestamp` and `input_end_timestamp` keep the original text, from `input_receipt`. A `Z` suffix uses the same receipt `timestamp_receipt` in `store.rs` already builds: offset text `Z`, meaning `documented_recording_offset`, basis `source_instant`. Any other numeric offset uses offset text as written, meaning `meaning_not_documented`, basis `source_offset_timestamp`. No new offset meaning is added to `src/v1/schema.ts`.

`source_reported_summaries` is always an array. It is empty when no line wrote one. Each written summary is a `SourceValue`: `standard_value`, `standard_unit`, `input_value`, `input_unit`. `received_at_basis` is `import_time`.

A line whose `our_field` is a point quantity, `provider_summary.…`, or `provider_score` writes a second record. The session does not also store that number. `hr_resting` on a Junction sleep object becomes a `point_measurement` with `quantity` `resting_heart_rate` and unit `bpm`. `average_hrv` becomes `quantity` `hrv_rmssd` and unit `ms`. The recorded instant for those children is the session end.

An activity object has no session. Each mapped line is its own record. `time_field` is the recorded instant. `calendar_date` is not a line, so it stays in `supplied_fields`.

A timeseries sample uses `time_field` and `value_field`. The sample's `type` selects the line `type.<that string>`. `heart_rate_variability_rmssd` and `heart_rate_variability_sdnn` become two point quantities. They are never one series.

A null mapped value is omitted. It is not written as zero, and it is not copied into `supplied_fields`. A mapped field that was used is not copied there either.

Fields that remain are `supplied_fields` on the session, for sleep and workout. Child records from that session do not repeat them. An activity object has no session, so the leftover object is stored on each record that object produced. A timeseries sample's leftovers are stored on that sample's record. The `source` object is leftover. Reading `source.provider` for the writer does not remove it.

`understood` lists the our-field names that were written. `preserved` lists the their-field names that went into `supplied_fields`. Both are sorted, and duplicates are dropped, the same way `import_garmin` does it. The result JSON is:

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

An object that cannot be stored is one `left_out` string, and the other objects in the file are still written. The string is `{section} {id}: {reason}`. A timeseries sample with no id uses the type and the timestamp in place of the id. Reasons used by this release:

- `missing source.provider` (or whatever `writer_field` the file names)
- `missing start_time`
- `missing end_time`
- `end is before the start`
- `value is not a number`

A refusal in section 4 is not a `left_out` line. The command exits 1 and writes nothing.

### Match key

Their id is the match key, as D161 says. The method column keeps the path, the same way Garmin already uses `garmin_account/v1` and `garmin_personal/v1`.

| Record | `match_key_method` | `match_key` |
|---|---|---|
| Sleep or workout session | `{route}/v1` | `{writer}:{section}:{id}` |
| Point, summary, or score taken from that session | `{route}/v1` | `{writer}:{section}:{id}:{suffix}` |
| Activity field | `{route}/v1` | `{writer}:activity:{id}:{suffix}` |
| Timeseries sample | `{route}/v1` | `{writer}:timeseries:{type}:{timestamp}` |

`{route}` is `open_wearables` or `junction`. `{writer}` is the provider string. `{suffix}` is the point quantity, the summary type, or the score metric. `provider_record_id` is their id when the object has one. A timeseries sample from the current Open Wearables schema has no id. The type and the timestamp are the match key. Section 11 asks Mike to confirm that.

A second run looks up that pair. The same record is updated. A second row is not inserted. `insert_group` copies the stored `received_at` onto the new data before it compares. If the rest is the same, it does not write a new revision. If a mapped value changed, the revision increases by one, the record id stays, and `received_at` stays the first import time. `insert_record` is not taught this comparison. Garmin's current re-import behavior stays what it is.

### The source row

The source is `Store::ensure_source` in `crates/ath-core/src/store.rs`, which is what `pull_garmin` calls:

```rust
store.ensure_source("garmin", "garmin_personal_api", "garmin")
```

For a mapped object the call is `ensure_source(writer, route, writer)`. `writer` is the string at `writer_field`. `route` is the mapping file's `route`. `ensure_source` stores `source_type` `export_file`, `data_writer`, `data_route`, and `vendor`, and it leaves `sensor_type` empty. The lookup is writer plus route plus an empty sensor, which is the lookup that function already has. The device model stays in `supplied_fields` under `source`. It is not a second source.

A Garmin file already uses writer `garmin` and route `garmin_account_export`. Garmin through this mapping uses writer `garmin` and route `open_wearables` or `junction`. Those are two source rows. D161's rule is that the path is part of the source.

### The TypeScript command

D174 stays. The command parser, help, skill install, key storage, and language-model calls stay in TypeScript. They are not moved into `ath-core`.

One flag is added to the existing `import` command in `src/program.ts`. `ath pull` is unchanged. `ath pull openwearables` still fails with the message that command already prints: `pull currently reads garmin`.

```
ath import <path> --mapping <name>
ath import <path> --mapping <name> --yes
ath import <path> --mapping <name> --json
```

`--mapping` is read before `detectSharedImport`. Without the flag, detection and the Apple, WHOOP, Oura, Garmin, and Strava branches run as they do now. A Garmin JSON file is not sent through a mapping because it contains `startTimeGMT`. With the flag, the path is not sent through those readers.

`resolveMappingFile` in `src/skill.ts` returns a path. `runAthCore` in `src/ath-core.ts` is called with `import-mapped`, `--database`, `--mapping-file`, `--input`, and `--preview` unless `--yes` was passed. `--json` is a preview, the same rule the import command already uses. The preview text is `renderMappedImport` in `src/shared-import.ts`, with the same four lines `renderSharedImport` already prints: understood, kept under original names, left out, records.

`src/cli/runtime.ts` `fail` already prints `ath: ` in front of every message. The Rust process prints the message alone on stderr. The tests in section 4 assert both.

`src/help.ts` `EXAMPLES.import` gains two lines showing `--mapping open-wearables` on a JSON file of named fields. The import command description names that flag. `EXAMPLES.skill` gains one sentence: mapping files are copied into `.agents/mappings` when that copy is missing, and an existing mapping file is left as it is. Then `pnpm generate:skill-cli` rewrites `skills/athletic-standard/references/cli.md` (D80). The tool skill frontmatter `metadata.version` becomes `2.1.0` so an older installed tool skill is replaced (D164). Calculation skill files stay at `2.0.0`. Their text is not edited.

That is the whole TypeScript change. There is no new top-level command.

## 3. A mapping file a person adds

D157 reads an installed calculation from `.agents/skills/<name>/SKILL.md` and otherwise reads the shipped file. A file that is already installed is not replaced. A mapping is found the same way, in its own folder, so `ath apply` does not try to run it. `skillNames` in `src/skill.ts` only lists folders that contain `SKILL.md`. Mapping files are not named `SKILL.md`.

`resolveMappingFile(name, cwd)`:

1. If `name` contains a path separator or ends in `.md`, it is a file path. This is how the refusal tests point at a bad file. A missing path is `no mapping file at <path>`.
2. Otherwise, if `.agents/mappings/<name>.md` exists in `cwd`, that file is used.
3. Otherwise `mappings/<name>.md` next to `skills/` is used. `mappingRoot()` is the same parent `skillsRoot()` already uses, with `mappings` in place of `skills`.
4. Otherwise the error is `no mapping file named <name>`.

`installMappings(cwd)` copies each shipped file into `.agents/mappings/` only when the destination is missing. It does not replace a file. `ath init` calls it just after `installSkill` (`src/program.ts`, the init action). `ath skill install` calls it just after `installSkill`. It does not copy into `.claude`, `.cursor`, or the other harness folders. Those copies are for skills.

A person edits `.agents/mappings/open-wearables.md`. The next import reads that file. The shipped file is not used while the installed file exists. The test puts a `LastNightAvg` line into the installed file and expects the refusal in section 4, which the shipped file does not contain.

## 4. Refusals

`check_mapping` runs before the database is written. `apply_mapping` runs the personal-report check before `insert_group`. On any refusal the process exits 1, stderr is the message below, and the record count is unchanged. `ath` prints the same message with the existing `ath: ` prefix.

The shipped files do not contain these lines. The tests use fixture files under `tests/fixtures/mappings/`. Those files are not health-data names, so `scripts/check-public-data.ts` does not need a hash for them. If the check disagrees, add the hash. Do not put them under `tests/fixtures/exports/`.

### A personal report

A measured record has a source. An athlete report has no `source_id`. The triggers in `src/db/migrations/enforcement-triggers.ts` already abort when one is present. This check runs first, and the person sees the mapping message. Those triggers stay as published.

The message is:

```text
refusing to turn a personal report into a measured record
```

`check_mapping` prints it when a section name is `athlete_report` or `nutrition_entry`, or when `our_field` is `source_id`, `report_type`, `reported_via`, or `rating`.

`apply_mapping` prints it when an input object contains `report_type` or `reported_via`, even if the mapping file is the shipped Open Wearables file. The whole file is refused. Nothing is written.

Test `personal_report_cannot_become_a_measured_record` in `mapping.rs`: input object `{ "id": "report-1", "report_type": "sleep_quality", "reported_via": "text", "rating": 2, "note": "rough night" }` inside a `sleep` array, mapped with the shipped file. Exit 1. Stderr is that sentence. A database that already holds one record still holds one record.

Test `tests/mapping-cli.test.ts` runs the same input through `ath import … --mapping open-wearables --yes` and expects stderr `ath: refusing to turn a personal report into a measured record`.

### An unlabeled HRV field

These their-field names may map to `hrv_rmssd` with no basis: `heart_rate_variability_rmssd`.

These may map to `hrv_sdnn` with no basis: `heart_rate_variability_sdnn`, `hrv_mean_sdnn`.

`average_hrv` may map to `hrv_rmssd` only when the line says `basis hrv_mean_rmssd`. The same line with `hrv_sdnn`, or with no basis, or with any other basis, is refused. Any other their-field, including `LastNightAvg`, `hrv`, and `HRV`, is refused for both quantities.

The message is:

```text
refusing to map LastNightAvg onto hrv_rmssd; the field does not name the statistic
```

and:

```text
refusing to map LastNightAvg onto hrv_sdnn; the field does not name the statistic
```

The template is `refusing to map {their_field} onto {our_field}; the field does not name the statistic`. `{their_field}` is the leaf name (`LastNightAvg`, not `type.LastNightAvg`).

Tests in `mapping.rs`:

- `last_night_avg_cannot_become_rmssd`, file `tests/fixtures/mappings/last-night-rmssd.md`, line `type.LastNightAvg: hrv_rmssd ms ms`.
- `last_night_avg_cannot_become_sdnn`, file `tests/fixtures/mappings/last-night-sdnn.md`, line `type.LastNightAvg: hrv_sdnn ms ms`.
- `average_hrv_without_basis_cannot_become_rmssd`, line `average_hrv: hrv_rmssd ms ms`, message `refusing to map average_hrv onto hrv_rmssd; the field does not name the statistic`.

`average_hrv_with_basis_can_become_rmssd` parses the shipped Junction line and `check_mapping` returns ok.

`pull_garmin` still stores `LastNightAvg` as a provider summary of that name. This release does not put that call on a mapping file.

### Their efficiency percentage

`asleep_seconds` divided by `in_bed_seconds` is the `sleep-efficiency` skill (`skills/sleep-efficiency/SKILL.md`). A percentage they sent is not that division. The study says their efficiency stays under their field name.

`our_field` `efficiency`, `efficiency_pct`, `sleep_efficiency`, `sleep-efficiency`, or `source_reported_summaries.sleep_efficiency` is refused. The message for the planned test is:

```text
refusing to map efficiency_percent onto efficiency; efficiency is computed from asleep_seconds and in_bed_seconds
```

The template fills in the two names from the line. Test `efficiency_percent_cannot_map_onto_efficiency`, file `tests/fixtures/mappings/efficiency.md`:

```ath-mapping
name: bad-efficiency
route: open_wearables
writer_field: source.provider

[sleep]
id_field: id
start_field: start_time
end_field: end_time
efficiency_percent: efficiency
```

The shipped Open Wearables fixture keeps `efficiency_percent` of `91` in `supplied_fields`. `ath apply sleep-efficiency` on that session reads `600` and `700` and returns `600/700`. It does not return `91`.

The shipped Junction fixture keeps `efficiency` of `0.97` in `supplied_fields`. That session has no `in_bed_seconds`. `ath apply sleep-efficiency` reports `in_bed_seconds` missing. `duration` is not used.

### The athlete's known maximum

D172 says `maximum_heart_rate_bpm` is the athlete's known maximum. A mapping line aimed at that field is refused. The message is:

```text
refusing to map max_heart_rate_bpm onto maximum_heart_rate_bpm; that field is the athlete's known maximum
```

Test `max_heart_rate_cannot_become_the_athlete_maximum`, file `tests/fixtures/mappings/athlete-maximum.md`, line `max_heart_rate_bpm: maximum_heart_rate_bpm bpm bpm`. The shipped workout line writes `provider_summary.max_heart_rate` instead, and the workout JSON has no `maximum_heart_rate_bpm`.

### Other check failures

These are not the three wall refusals. They still fail the file before a write:

- No fence: `the mapping file has no ath-mapping block`
- A bad line: `could not read '{line}'`
- Unknown section: `unknown mapping section {name}`
- Unknown our-field: `unknown field {our_field}`
- Two `when-absent` lines for one our-field: `two when-absent lines write {our_field}`
- `basis` on a field that is not the two HRV quantities: `basis is only used for hrv_rmssd and hrv_sdnn`
- `scale` on anything except `provider_score`: `scale is only used for provider_score`
- `provider_score` without `scale`: `provider_score {their_field} needs a scale`
- A unit pair outside the table in D180: `cannot convert {their_unit} to {our_unit} for {their_field}`

## 5. Proposed decisions

D161 leaves the file syntax, the folder, the lookup, and unit conversion open. These four are the proposals. The build copies the approved text into `build-history/v2.1.0/decisions.md`. It does not rewrite D152–D176. D181 is written only after Mike picks a version option in section 8. Both texts are drafted there.

### D177. A mapping file is an `ath-mapping` block

A mapping file is markdown. The program reads the first fenced block tagged `ath-mapping`. Inside it, `name`, `route`, and `writer_field` come first. A section is `[sleep]`, `[workout]`, `[timeseries]`, or `[activity]`. Settings `id_field`, `start_field`, `end_field`, `time_field`, and `value_field` locate the record. Every other line is `their_field: our_field`, then an optional unit pair, then optional `basis <token>`, `scale <token>`, and `when-absent`. A dotted their-field reads that path. `type.<name>` matches a timeseries sample's `type`. Text outside the fence is not read.

This is the same kind of block `parse_skill` already reads from a calculation file, with a different tag so a calculation is not applied as a mapping.

Rejected alternatives:

- A JSON file. Rejected because a person already edits calculation skills as a fenced block, and a second syntax means a second parser.
- A CSV of two columns. Rejected because a row cannot name the record, the unit pair, or the `hrv_mean_rmssd` basis without more columns and a second header language.
- Reuse the `ath-calculation` tag. Rejected because `ath apply` would then try to run a mapping as a calculation.

### D178. Shipped mappings live in `mappings/`, and a person's copy lives in `.agents/mappings/`

The shipped files are `mappings/open-wearables.md` and `mappings/junction.md`. A person's file is `.agents/mappings/<name>.md`. `ath init` and `ath skill install` copy a shipped file only when that destination is missing. They do not replace it. Mapping files are not `skills/<name>/SKILL.md`.

The calculation folder is for calculations. A mapping in that folder would be installed as a skill and offered to `ath apply`.

Rejected alternatives:

- `skills/open-wearables/SKILL.md`. Rejected because `skillNames` treats every `SKILL.md` as a skill, and `ath apply open-wearables` would look for a calculation block.
- One file, `mappings/all.md`. Rejected because a person adding a source adds one file, which is the rule D161 states.
- Replace the installed file when the shipped file changes. Rejected because D157 already refuses that for a calculation the person may have edited. The same reason applies here.

### D179. The installed file is read first, and it is checked before a write

`resolveMappingFile` uses the installed file, then the shipped file, then `no mapping file named <name>`. A path argument is used as a path, so a test can point at a bad file. `check_mapping` accepts the file or returns one of the messages in section 4. `apply_mapping` refuses a personal-report object before `insert_group`. `ath check` does not read mapping files. The records it checks are already stored.

The program has to know which file won, and it has to reject a bad file with nothing written. Doing that at import matches the calculation lookup, which also happens when the command runs.

Rejected alternatives:

- Read only the shipped file. Rejected because D161 says a person can add a file, and D157 says the installed calculation is the one that runs.
- Check the mapping inside `ath check` after the write. Rejected because a bad file would already have stored a measured record.
- Guess the mapping from the JSON keys, with no `--mapping` flag. Rejected because a Garmin file and a mapped file are both JSON, and the Garmin reader has to keep running when the flag is absent.

### D180. Units convert by a fixed pair, or the line is refused

`convert_unit` accepts only these pairs. Any other pair fails with `cannot convert {their_unit} to {our_unit} for {their_field}`.

| Their unit | Our unit | Result |
|---|---|---|
| `s` | `s` | the same number |
| `min` | `min` | the same number |
| `s` | `min` | the number divided by 60 |
| `ms` | `ms` | the same number |
| `bpm` | `bpm` | the same number |
| `m` | `m` | the same number |
| `kcal` | `kcal` | the same number |
| `count` | `count` | the same number |
| `score` | `score` | the same number |
| `brpm` | `brpm` | the same number |
| `percent` | `percent` | the same number |

Division by 60 is exact. `180` seconds becomes `3` minutes. `90` seconds becomes `1.5` minutes. The result is not rounded. When the target is a `SourceValue`, `input_value` and `input_unit` keep their number and their unit, and `standard_value` and `standard_unit` hold ours. `asleep_seconds`, `in_bed_seconds`, and `stage_minutes` are plain numbers, so the stored number is the converted one. The original field is not also copied into `supplied_fields`.

A percent is in the table so a line can name it. It is not a target for efficiency. Section 4 refuses that target before the unit pair is applied.

Rejected alternatives:

- Store their number unchanged and ignore units. Rejected because Junction stage fields are seconds and `stage_minutes` is minutes. The minutes would be sixty times too large.
- A formula on the line. Rejected because a formula can divide asleep by time in bed and store that as efficiency, or relabel a heart-rate variability field. The line names a pair. It does not compute a new measurement.
- Convert `percent` to a ratio, or a ratio to `percent`. Rejected because Junction's sleep example shows `efficiency` of `0.97` while the description calls it a percentage. This release does not pick a scale. The number stays under their field name.

## 6. Acceptance tests

Every change runs:

```text
pnpm check:public-data
pnpm typecheck
pnpm test
pnpm build
cargo test --manifest-path crates/ath-core/Cargo.toml
```

### Existing imports stay byte for byte

The first build step, before `mapping.rs` exists, runs each current importer once and writes a comparison file. Later steps fail if that text changes. The text is UTF-8 JSON with sorted keys. Record ids, `created_at`, `received_at`, and the database id are left out, because a new run mints a new id and a new import time. Everything else listed here is compared as bytes.

Included, per record, in this order: `record_type`, the data object, `supplied_fields`, every input timestamp receipt, the source as `data_writer`, `data_route`, `vendor`, and `source_type`, and each `record_import_keys` pair `match_key_method` and `match_key`. Source ids inside the data are replaced with `writer|route|vendor` before the text is written. Records are sorted by match key.

The comparison files live in `tests/fixtures/import-baseline/`. They are not under `tests/fixtures/exports/`. Their names do not contain `export`, `athlete`, `garmin`, `oura`, `whoop`, `healthkit`, or `apple-health`, so `scripts/check-public-data.ts` does not treat them as health files. If a name is flagged, register the hash. Do not rename them into `tests/fixtures/exports/`.

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

The personal-day server follows the local server in `crates/ath-core/src/personal.rs`. The activity body is one invented activity. The HRV body includes `LastNightAvg` and no other statistic name. The comparison asserts that summary's `summary_type` is `LastNightAvg`. The test is `tests/import-baseline.test.ts`. It calls the same commands the importers use now: `ath import` for the files, and `ath pull garmin --date 2026-08-09 --base-url <local>` for the personal day.

`tests/apple-rust-parity.test.ts`, `tests/v1-provider-import.test.ts`, `tests/v2-core.test.ts`, and the Rust tests in `apple.rs`, `whoop.rs`, `oura.rs`, `garmin.rs`, `strava.rs`, and `personal.rs` stay. The baseline file is the check that the mapping work did not move those readers.

### New fixtures

Both files are invented. Dates are in 2026. The coordinates are not a place from a real export. The build writes them, then adds each path and its sha256 to `approvedSyntheticFixtures` in `scripts/check-public-data.ts`. They sit under `tests/fixtures/exports/`, so the allowlist is required. Add a `!` gitignore line only if that check asks for one.

`tests/fixtures/exports/open-wearables/summaries.json`:

```json
{
  "sleep": [
    {
      "id": "ow-sleep-1",
      "start_time": "2026-08-09T02:00:00.000-04:00",
      "end_time": "2026-08-09T10:00:00.000-04:00",
      "zone_offset": "-04:00",
      "source": { "provider": "oura", "device": "invented-ring" },
      "duration_seconds": 800,
      "sleep_duration_seconds": 600,
      "time_in_bed_seconds": 700,
      "efficiency_percent": 91,
      "stages": {
        "awake_minutes": 2,
        "light_minutes": 3,
        "deep_minutes": 4,
        "rem_minutes": null
      },
      "is_nap": false
    }
  ],
  "workout": [
    {
      "id": "ow-workout-1",
      "type": "running",
      "name": "Invented loop",
      "start_time": "2026-01-02T15:00:00.000Z",
      "end_time": "2026-01-02T15:00:03.000Z",
      "source": { "provider": "garmin", "device": "invented-watch" },
      "duration_seconds": 3,
      "distance_meters": 12,
      "avg_heart_rate_bpm": 140,
      "max_heart_rate_bpm": 180,
      "start_latitude": 12.345,
      "start_longitude": -45.678,
      "calories_kcal": 1
    }
  ],
  "timeseries": [
    {
      "timestamp": "2026-08-09T06:12:00.000Z",
      "type": "heart_rate_variability_rmssd",
      "value": 42.5,
      "unit": "ms",
      "source": { "provider": "oura" }
    },
    {
      "timestamp": "2026-08-09T06:12:00.000Z",
      "type": "heart_rate_variability_sdnn",
      "value": 61.2,
      "unit": "ms",
      "source": { "provider": "oura" }
    },
    {
      "timestamp": "2026-08-09T06:12:00.000Z",
      "type": "resting_heart_rate",
      "value": 48,
      "unit": "bpm",
      "source": { "provider": "oura" }
    },
    {
      "timestamp": "2026-08-09T06:12:00.000Z",
      "type": "garmin_stress_level",
      "value": 15,
      "unit": "score",
      "source": { "provider": "garmin" },
      "computed_by": "provider"
    }
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
      "efficiency": 0.97,
      "hr_resting": 47,
      "average_hrv": 55.5,
      "LastNightAvg": 40,
      "respiratory_rate": 14,
      "score": 80,
      "recovery_readiness_score": 70,
      "source": { "provider": "oura", "type": "ring" }
    },
    {
      "id": "jn-sleep-2",
      "bedtime_start": "2026-08-10T02:00:00.000Z",
      "bedtime_stop": "2026-08-10T03:00:00.000Z",
      "duration": 3600,
      "total": 1000,
      "stage_asleep_second": null,
      "source": { "provider": "oura" }
    }
  ],
  "activity": [
    {
      "id": "jn-activity-1",
      "date": "2026-08-09T00:00:00.000Z",
      "calendar_date": "2026-08-09",
      "steps": 8432,
      "calories_active": 410,
      "calories_total": 2100,
      "floors_climbed": 3,
      "heart_rate": { "resting_bpm": 49, "avg_bpm": 72 },
      "source": { "provider": "garmin", "type": "watch" }
    }
  ]
}
```

The numbers are invented. They do not need to add up to a night. Do not "fix" them into `efficiency_percent` or into `600/700`.

Test `open_wearables_fixture_maps_fields`:

- Sleep `ow-sleep-1` is a `sleep_session`. `start_at` is `2026-08-09T06:00:00.000Z`. `end_at` is `2026-08-09T14:00:00.000Z`. The start receipt's `timestamp_text` is `2026-08-09T02:00:00.000-04:00`, `utc_offset_text` is `-04:00`, `utc_offset_meaning` is `meaning_not_documented`, and `timestamp_basis` is `source_offset_timestamp`.
- `asleep_seconds` is `600`. `in_bed_seconds` is `700`. `stage_minutes` is `{ "awake": 2, "light": 3, "deep": 4 }` with no `rem` key.
- `supplied_fields` holds `efficiency_percent` `91`, `duration_seconds` `800`, `is_nap` false, `zone_offset` `-04:00`, and `source.provider` `oura`. It does not hold `sleep_duration_seconds`, `time_in_bed_seconds`, or `stages`.
- The source is writer `oura`, route `open_wearables`, vendor `oura`, `source_type` `export_file`, empty sensor.
- Match method `open_wearables/v1`, match key `oura:sleep:ow-sleep-1`.
- The workout source is writer `garmin`, route `open_wearables`. `activity_type` is `running`, `name` is `Invented loop`. `source_reported_summaries` holds distance `12` `m` and average heart rate `140` `bpm`. There is no `maximum_heart_rate_bpm`. A `provider_summary` `max_heart_rate` has standard value `180` and unit `bpm`, match key `garmin:workout:ow-workout-1:max_heart_rate`.
- `supplied_fields` on that workout holds `start_latitude` `12.345`, `start_longitude` `-45.678`, `calories_kcal` `1`, and `duration_seconds` `3`.
- Two point measurements at `2026-08-09T06:12:00.000Z`: `hrv_rmssd` `42.5` and `hrv_sdnn` `61.2`, plus `resting_heart_rate` `48`. Match keys `oura:timeseries:heart_rate_variability_rmssd:2026-08-09T06:12:00.000Z` and the same with `heart_rate_variability_sdnn` and `resting_heart_rate`.
- `garmin_stress_level` is a `provider_summary` with `summary_type` `garmin_stress_level`, value `15`, unit `score`. `supplied_fields.computed_by` is `provider`. It is not `hrv_rmssd` or `hrv_sdnn`. Its source route is `open_wearables` and its writer is `garmin`.

Test `junction_fixture_maps_fields`:

- `jn-sleep-1` has `asleep_seconds` `600`, from `stage_asleep_second`, not from `total`. `supplied_fields.total` is `500`. `supplied_fields.duration` is `700`. There is no `in_bed_seconds`.
- `stage_minutes` is `{ "awake": 2, "light": 3, "deep": 4 }` (`120/60`, `180/60`, `240/60`). No `rem` key.
- `supplied_fields` also holds `efficiency` `0.97`, `LastNightAvg` `40`, `respiratory_rate` `14`, `awake` `0`, `light` `0`, `rem` `0`, `deep` `0`.
- A point `hrv_rmssd` is `55.5`, not `40`. A point `resting_heart_rate` is `47`. Match keys `oura:sleep:jn-sleep-1:hrv_rmssd` and `oura:sleep:jn-sleep-1:resting_heart_rate`. There is no `hrv_sdnn` record.
- Two `provider_score` records: metric `score`, value `80`, scale `1-100`; metric `recovery_readiness_score`, value `70`, scale `0-100`.
- `jn-sleep-2` has `asleep_seconds` `1000` and no `total` in `supplied_fields`. `duration` `3600` is still there.
- Activity: `provider_summary` `steps` `8432` unit `count`, `calories_active` `410` unit `kcal`, `calories_total` `2100` unit `kcal`. A `resting_heart_rate` point is `49`. `supplied_fields` holds `floors_climbed` `3`, `calendar_date` `2026-08-09`, and `heart_rate.avg_bpm` `72`. `heart_rate.resting_bpm` is not in `supplied_fields`.
- Sources are writer `oura` route `junction`, and writer `garmin` route `junction`. Neither row's route is `open_wearables` or `garmin_account_export`.

Test `rerun_does_not_add_a_record`: import the Open Wearables fixture with `--yes`, then import it again. One sleep row has `provider_record_id` `ow-sleep-1`. Its `current_revision` is `1`. One `record_import_keys` row exists for `open_wearables/v1` and `oura:sleep:ow-sleep-1`. Then import a copy whose `sleep_duration_seconds` is `601`. The same record id remains, `current_revision` is `2`, `asleep_seconds` is `601`, and `received_at` is unchanged.

Test `garmin_file_and_mapping_stay_two_sources`: `ath import` of `tests/fixtures/exports/garmin/park-loop.json` with `--yes` and no `--mapping`, then `ath import` of the Open Wearables fixture with `--mapping open-wearables --yes`. Two workout sessions exist. Two sources have writer `garmin`: one `data_route` is `garmin_account_export`, the other is `open_wearables`. The file import's workout still has `supplied_fields.Feeling` `smooth` and a `max_heart_rate` summary of `200`. The mapped workout has `start_latitude` `12.345` and a `max_heart_rate` summary of `180`.

Test `installed_mapping_file_is_the_one_that_runs`: in a temp directory, write `.agents/mappings/open-wearables.md` with a `LastNightAvg` line aimed at `hrv_rmssd`. Import the Open Wearables fixture. The command fails with the section 4 message. Then restore a valid installed file, run `installMappings` again, and assert the edited bytes are unchanged.

`tests/mapping-cli.test.ts` also asserts that `ath import` of `park-loop.json` without `--mapping` still reports the Garmin preview, and that `ath pull openwearables` still prints `pull currently reads garmin`.

### A 2.0.0 database

Take a database whose `athleticstandard_version` is `2.0.0` and whose `sql_schema_version` is `3`. The published 1.0.0 fixture is not that database. Use a database created by version 2.0.0, or the fixture already stored for that version if one is committed. Run `ath migrate`, then `ath check`. Check passes. History, links, attachments, and deletion records are still there.

What migrate does depends on section 8. Under option A it applies migration 004 and the version becomes `2.1.0`. Under option B it prints `No migration is required. Direct-read views and writable tables match this ath version.` and the stored version stays `2.0.0`. Either way `ath check` exits 0. Opening the file does not migrate it. That rule is unchanged.

## 7. What this release does not change

- The records, `supplied_fields`, sources, and import receipts produced from the Apple Health, WHOOP, Oura, Garmin, and Strava fixtures, including `ath pull garmin`. Those readers are not moved onto mapping files.
- Required record fields. `asleep_seconds`, `in_bed_seconds`, and `stage_minutes` are already optional on `SleepSessionData` in `src/v1/schema.ts`. This release fills them from a mapping. It does not add a required field, and it does not add a column.
- The separation of measured readings and athlete reports. A measured record has a source. An athlete report has no `source_id`. `hrv_rmssd` and `hrv_sdnn` stay two quantities. `LastNightAvg` stays `LastNightAvg` on the Garmin personal pull.
- The TypeScript command front end, except the `--mapping` flag, the two help sentences, the generated `cli.md`, and `installMappings` called from init and skill install. The front end is not moved to Rust (D174).
- Published migrations `001`, `002`, and `003`, including the `2.0.0` string inside `src/db/migrations/003-version-2.ts` and `spec/sql/migrations/003-version-2.sql` (D165).
- Earlier D-numbers and earlier `build-history/` folders, except the one edit in section 9.
- Network pulls, an Open Wearables client, a Junction client, sync, and any hosted service. `import-mapped` has no URL flag and does not call `ureq`.

`src/v1/schema.ts` is not given new fields for this mapping. `pnpm generate:json-schema` and `pnpm generate:v1-fixture` run only if option A changes the version enum, as section 8 says. They do not run because of a new field.

## 8. Version

Mike has not chosen whether the program version and the record version stay one number. D152 made them one number for 2.0.0. D164 says that choice still holds from 2.0.0 on. Neither decision is rewritten. The choice for this release is D181, written when Mike picks, using one of the texts below.

The code uses two constants either way, so the other choice is a later edit of those strings.

- `src/version.ts` exports `ATH_PACKAGE_VERSION`. `ath --version` prints it. `buildProgram` already passes it to `.version`.
- `src/v1/schema.ts` exports `ATHLETIC_STANDARD_VERSION`. New databases store it. `ACCEPTED_RECORD_VERSIONS` lists the record versions this program can read.
- `crates/ath-core/src/lib.rs` exports `APP_VERSION` and `RECORD_VERSION`. `Store::open` compares `database_metadata.athleticstandard_version` to `RECORD_VERSION`. The error stays `database version is {version}; run ath migrate before using ath-core`. `changed_by` uses `APP_VERSION` as `tool_version`. The single `SOFTWARE_VERSION` constant is removed, and every current use is pointed at one of the two names.

Today those strings are all `2.0.0`, via `SOFTWARE_VERSION`, `ATH_PACKAGE_VERSION`, and `ATHLETIC_STANDARD_VERSION`. Splitting the names is the small change. Setting the strings is the decision.

### Option A. One number, 2.1.0

`ath --version` prints `2.1.0`. `athleticstandard_version` on a new database is `2.1.0`. `APP_VERSION` and `RECORD_VERSION` are both `2.1.0`.

D181 text if this option is picked:

> D181. The program and the record stay one number, 2.1.0.
>
> `ath --version` and `database_metadata.athleticstandard_version` are `2.1.0`. D152 made them one number so a change does not have to pick which number moves. This release keeps that rule. The mapping does not add a required field. Minor versions may add optional fields, which is the rule in [v0.1.0/spec.md](../v0.1.0/spec.md) at the `athleticstandard_version` line. The optional sleep fields this mapping fills were already on the 2.0.0 record. The number still moves, because the program moved.
>
> Rejected alternatives:
>
> - Leave the record at 2.0.0 while the program becomes 2.1.0. Rejected because D152 and D164 already rejected a split, and a later change would again have to pick which number moves.

What changes:

- `package.json` `version` becomes `2.1.0`. The release workflow already packs `athleticstandard-$(version).tgz` from that field (`.github/workflows/release-cross-platform.yml`). The tarball name becomes `athleticstandard-2.1.0.tgz`. D171's `2.0.0` filename is the 2.0.0 case of that rule. It is not rewritten.
- `ATH_PACKAGE_VERSION`, `ATHLETIC_STANDARD_VERSION`, `APP_VERSION`, and `RECORD_VERSION` become `2.1.0`.
- `ACCEPTED_RECORD_VERSIONS` becomes `["1.0.0", "2.0.0", "2.1.0"]`. A 2.0.0 database still opens in TypeScript so `ath migrate` can read it. `READABLE_DATABASE_VERSIONS` in `src/db/migrate.ts` must include `"1.0.0"`, `"2.0.0"`, and `"2.1.0"`. If `"2.0.0"` is dropped, migrate cannot open the database it is supposed to upgrade.
- New migration step 4, and only that. Files: `src/db/migrations/004-version-2-1.ts` and `spec/sql/migrations/004-version-2-1.sql`, generated through `pnpm generate:sql-contract`. SQL:

```sql
UPDATE database_metadata
SET sql_schema_version = 4,
    athleticstandard_version = '2.1.0'
WHERE singleton_id = 1;
```

- No column is added. `003` still sets `2.0.0` during its own statement. Creation in `installInitialSchema` runs `VERSION_2_UPGRADE_SQL`, then the new step, then the `user_version` pragma. A new database ends at schema 4 and version `2.1.0`. `003` is not edited to skip its update.
- `SQL_SCHEMA_VERSION` in `src/db/client.ts` becomes `4`. `MIGRATION_CHAIN` in `src/db/sql-contract.ts` gains version 4. The description says the step stores software version 2.1.0 and adds no column.
- `ath check` already compares `sql_schema_version` to `SQL_SCHEMA_VERSION`. A 2.0.0 database fails that check until migrate. After migrate it passes. The check message stays the one in `src/db/check.ts`.
- `Store::open` rejects `2.0.0` until migrate, with the existing sentence.
- `src/db/backtest.ts` stops hardcoding `"2.0.0"` and uses `ATHLETIC_STANDARD_VERSION`.
- Rust tests that insert `athleticstandard_version` `'2.0.0'` into a database they then open with `Store::open` use `RECORD_VERSION`. The inserts that deliberately build a 1.0.0 or 2.0.0 file, including the migrate test, stay hardcoded.
- `pnpm generate:json-schema` writes `schema/2.1.0/`. `schema/2.0.0/` stays. `package.json` exports and `tests/v1-package-exports.test.ts` point at `schema/2.1.0/`. Both version constants are asserted equal to `2.1.0`.
- `pnpm generate:v1-fixture` regenerates `examples/demo-athlete/` if the stored version is inside it. The new hashes replace the three demo-athlete lines in `scripts/check-public-data.ts`. The fixture is not hand-edited.
- `README.md` and `SPEC.md` sentences that say the current schema folder is `schema/2.0.0/` gain the `schema/2.1.0/` folder. The field list in `SPEC.md` does not gain fields.
- `build-history/README.md` "Current records" gains the 2.1.0 links and the sentence: version 2.1.0 is the software now. The program, the record, and the database use that number.
- Calculation skill files stay on metadata version `2.0.0`. The tool skill metadata version becomes `2.1.0` because `cli.md` changes (section 2).
- Search the tree for `2.0.0` before the last commit. Change a hit only when it is this program's current version. Do not change `003`, historical decisions, the 2.0.0 schema folder, or a test that is supposed to open an old database.

What this asks of a 2.0.0 database: `ath migrate` runs, and no column is added. `user_version` becomes 4. D152 says that integer is the upgrade step. It is the step count, and the software version is the `2.1.0` stored beside it.

### Option B. The program is 2.1.0 and the record stays 2.0.0

`ath --version` prints `2.1.0`. A new database still stores `athleticstandard_version` `2.0.0`. `APP_VERSION` is `2.1.0`. `RECORD_VERSION` is `2.0.0`. `SQL_SCHEMA_VERSION` stays `3`. There is no migration 004. `001` and `003` are not edited.

D181 text if this option is picked:

> D181. The program is 2.1.0. The record stays 2.0.0.
>
> `ath --version` prints `2.1.0`. `database_metadata.athleticstandard_version` stays `2.0.0`. This release adds a way to read named fields. It does not add a record field, a column, or a required value. D152 kept one number for 2.0.0 because a change then had to pick which number moved. This release is that pick: the program moves, and the record does not, because the record format did not change. A later release that adds an optional field moves the record number.
>
> Rejected alternatives:
>
> - Move both numbers to 2.1.0. Rejected for this release because a migrate step with no new column would be the only effect on a 2.0.0 database.
> - Hide the program version and keep printing 2.0.0 from `ath --version`. Rejected because the mapping command is new and the person needs to see that they are running it.

What changes:

- `package.json` `version` and `ATH_PACKAGE_VERSION` and `APP_VERSION` become `2.1.0`. The packed tarball name follows `package.json`, so it becomes `athleticstandard-2.1.0.tgz` with no workflow edit.
- `ATHLETIC_STANDARD_VERSION` and `RECORD_VERSION` stay `2.0.0`. `ACCEPTED_RECORD_VERSIONS` stays `["1.0.0", "2.0.0"]`. `READABLE_DATABASE_VERSIONS` stays `"1.0.0"` plus the current record version.
- No file under `src/db/migrations/` or `spec/sql/migrations/` is added or edited. `pnpm generate:sql-contract` does not run for a version bump.
- `schema/2.0.0/` stays the schema folder. JSON Schema is not regenerated. The demo athlete fixture is not regenerated. Its hashes stay.
- `tests/v1-package-exports.test.ts` currently expects the package version and `ATHLETIC_STANDARD_VERSION` to be `2.0.0` and equal. It expects package `2.1.0` and record `2.0.0`.
- `ath migrate` on a 2.0.0 database prints the existing no-migration sentence. `ath check` passes with `sql_schema_version` `3`. `Store::open` accepts `2.0.0`.
- The tool skill metadata version becomes `2.1.0` because the help text changed, not because the record version changed. Calculation skills stay `2.0.0`.
- `build-history/README.md` "Current records" gains the 2.1.0 links and two sentences: version 2.1.0 is the program. The record and the database stay at 2.0.0. The folder rule at the top of that file says a folder is named for the record version. One sentence is added under that rule: a program release that does not change the record still gets the folder named in its plan. The rest of that rule is not rewritten.
- `README.md` and `SPEC.md` do not move their schema links off `schema/2.0.0/`.
- `src/db/backtest.ts` stays on `2.0.0`.

Trade-off: a 2.0.0 database needs no migrate. The person sees `2.1.0` from `ath --version` and `2.0.0` inside the database. D152 treated that split as the thing to avoid. Picking B means saying so in D181, not by editing D152.

### What is shared

Both options add the two constants and point `Store::open` at `RECORD_VERSION`. Both options leave required fields, published migrations `001`–`003`, and the existing importers alone. Both options run the 2.0.0 migrate-then-check test in section 6. Switching from B to A later is migration 004 plus the string `RECORD_VERSION`. Switching from A to B later is not available once a database has been written at `2.1.0`, which is a reason to pick before the build writes a database.

## 9. The 2.0.0 progress note

The build deletes the "Later, with the reason:" section from `build-history/v2.0.0/progress.md`. Mike asked for that deletion. The list is out of date. This pull request does not make the edit.

The section is lines 42–56, 15 lines. It starts at the heading `Later, with the reason:` and ends at the blank line after the last bullet. The paragraph above it, which ends with the bare `export.xml` sentence, stays. The file then ends.

Those lines are five bullets: the iPhone app, Strava's online connection, an Android app, moving package import and the other TypeScript pieces, and the Open Wearables or Junction pull plus the mapping file. D173 and D174 already hold the TypeScript sentences. D161 already holds the mapping rule. The progress file does not need a second copy.

No file links to that heading. A search for `Later, with the reason` and for `v2.0.0/progress` finds the heading only in that file. The one link inside the section points at `appendix/source-formats.md`. These still open the study after that section is removed:

- [D161](../v2.0.0/decisions.md), the relative link `appendix/source-formats.md`
- `docs/connections.md`, the link `../build-history/v2.0.0/appendix/source-formats.md`

`docs/connections.md` also says an Open Wearables pull is not built. That sentence stays true, because this release does not build the pull. `docs/connections.md` is not edited.

`build-history/v2.1.0/progress.md`, written with the code and not in this pull request, says the section was removed from `build-history/v2.0.0/progress.md` at Mike's request because the list was out of date, and it links the study:

```markdown
The outdated "Later, with the reason:" list was removed from
[v2.0.0/progress.md](../v2.0.0/progress.md). The mapping study is
[v2.0.0/appendix/source-formats.md](../v2.0.0/appendix/source-formats.md) (D161).
```

## 10. Notes and proof

The build commits `build-history/v2.1.0/` with the code:

| File | What it holds |
|---|---|
| `spec.md` | This plan, updated only if Mike changes it before the build |
| `decisions.md` | D177–D180 as approved, then D181 for the version he picked |
| `progress.md` | The steps in section 11, marked done as they land, plus the section 9 sentence |

`build-history/README.md` "Current records" links all three. That README edit is part of the build, not part of this pull request. The version sentence in that list is the one from the chosen option in section 8.

The proof is a screenshot or a playable mp4 attached to the build pull request. It is not committed. It shows, in one pass:

1. `ath import` of the Open Wearables fixture with `--mapping open-wearables --yes`.
2. The `hrv_rmssd` point at `42.5` and the `hrv_sdnn` point at `61.2` as two records.
3. `efficiency_percent` still named `efficiency_percent` on the sleep session, and `start_latitude` still named `start_latitude` on the workout.
4. The same import again, with one sleep record and revision 1.
5. The Garmin file import and the mapped workout as two sources.
6. The `LastNightAvg` refusal on stderr, with nothing written.

This planning pull request has no recording. There is no program to run.

## 11. Risks, questions, and the build order

### Risks

- `insert_typed` grows a `provider_score` arm. `insert_record` stays. The baseline files in section 6 are the check that Garmin, Strava, Apple, WHOOP, and Oura did not move.
- A re-run writes a second row if the match key omits the writer or the section. The keys in section 2 include both. The re-run test covers the sleep row.
- Timeseries samples have no id. Two samples with the same provider, type, and timestamp update one row. The source is also one row per provider and path, because `ensure_source` leaves `sensor_type` empty. A second physical device from the same provider shares that source. That matches the Garmin file source, which is also one row per writer and path.
- `--mapping` has to be handled before `detectSharedImport`. If it is handled after, a JSON file that contains `startTimeGMT` never reaches the mapping. The CLI test imports `park-loop.json` both ways.
- Option A drops `"2.0.0"` from the readable set, and then `ath migrate` cannot open the database. Section 8 lists the set for that reason.
- Option A runs `003`, which sets version `2.0.0`, and then step 4. Editing `003` instead would violate D165.
- Junction's study says `source.slug`. The summary schema on 31 October 2026 has `source.provider` and calls that string the provider slug. A file that reads `source.slug` would leave out every object. The shipped file reads `source.provider`. Question 2 asks Mike to confirm.
- Junction's `efficiency` example is `0.97` and the description says percentage. The number stays under `efficiency`. No scale is applied.
- Open Wearables stage minutes are under `stages` in the 31 October sleep schema. The study lists the leaf names. The shipped lines use `stages.awake_minutes` and the other three. A reader that flattens those leaves would miss the lines and leave them in `supplied_fields`. The fixture uses the nested object.
- The npm package omits `mappings/` unless `package.json` `files` lists it. The export test asserts the list entry.

### Questions for Mike

1. Do you want option A or option B in section 8?
   - Option A. Recommendation: this one. D152 and D164 already use one number, and a later change then keeps a single number to move.
   - Option B. The record format gained no field. A 2.0.0 database would stay on schema 3.

2. Junction's writer field is `source.provider` in this plan. The study's table says `source.slug`. The summary object uses `source.provider` and describes it as the provider slug. Do you want the shipped line to read `source.provider`?
   - `source.provider`. Recommendation: this one. It is the field on the summary object, so the source row gets the device name.
   - `source.slug`. The objects checked on 31 October 2026 have no such field. Every record would be left out until a reader renamed it.

3. Should the shipped Open Wearables file wait to map `/health-scores` until that object's keys are written down, or should the build add them now?
   - Wait. Recommendation: this one. This release's file is sleep, workouts, and timeseries. The study does not give the health-score object's paths.
   - Add them now. The build reads the health-score schema first and adds only keys that schema returns.

4. A timeseries sample has no id. The match key is the provider, the type, and the timestamp. Is that the key you want?
   - Yes. Recommendation: this one. The sample has no id. A second import of the same sample updates one row.
   - No. Refuse a sample with no id, and name it in `left_out`.

5. The TypeScript front end gains `--mapping` and two help sentences, then `cli.md` is regenerated. D174 stays in force. Is that flag part of this release?
   - Yes. Recommendation: this one. `ath import` can run the file. `ath pull` stays Garmin-only.
   - No. Tests call `ath-core import-mapped` only. The flag arrives with `ath pull openwearables` in the next release.

### Build order

The build starts after this plan is approved and after question 1 is answered. Questions 2–5 have a recommended pick above. A different answer changes the named line, the score section, the timeseries key, or the CLI flag, and it does not change the rest of the order.

1. Record D177–D180 and D181 in `build-history/v2.1.0/decisions.md`. Add the version constants as two names, still equal to `2.0.0`, and point `Store::open` at `RECORD_VERSION`. Then apply option A or option B. Run the 2.0.0 migrate-then-check test.
2. Write the import-baseline comparison files from the importers as they are today. Land the test that reads them. Do this before `mapping.rs` exists.
3. Add `parse_mapping`, `check_mapping`, and the refusal tests. No database writes yet.
4. Add `apply_mapping`, `insert_group`, the `provider_score` arm, and the two fixtures. Land the map, re-run, and two-source tests.
5. Add `resolveMappingFile`, `installMappings`, and the `--mapping` flag if question 5 stays yes. Regenerate `cli.md`. Bump the tool skill version to `2.1.0`. Leave calculation skills at `2.0.0`.
6. Delete lines 42–56 of `build-history/v2.0.0/progress.md`. Write `build-history/v2.1.0/progress.md`, including the sentence in section 9. Link the three v2.1.0 files from `build-history/README.md`.
7. Run the acceptance commands in section 6. Put the proof recording on the build pull request. Do not commit the recording.
