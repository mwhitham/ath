# v2.1.0 decisions

Decisions continue after D176. Earlier decisions stay as they are. The plan is
[spec.md](spec.md).

## D177. A mapping file is an `ath-mapping` block

A mapping file is markdown. The program reads the first fenced block tagged
`ath-mapping`. Text outside that block is for a person. A trailing `\r` is
stripped from each line, so a file with Windows line endings reads the same.

These header keys are read only before the first `[section]`: `name`, `route`,
`writer_field`, `sensor`, `device_name`, `device_model`, and `device_hardware`.
The last four take a comma-separated list of their fields. They name the device
(D183). A later `name:` line, such as `name: name` under `[workout]`, is a mapping
line: their field `name` to our field `name`.

A section is `[sleep]`, `[workout]`, or `[timeseries]`. `[activity]` is not a
section. Settings `id_field`, `start_field`, `end_field`, `time_field`, and
`value_field` locate the record. Every other line is:

```text
their_field: our_field [their_unit our_unit] [basis TOKEN] [scale TOKEN] [when-absent] [when-writer a,b,c]
```

A dotted their-field reads that path. `type.<name>` matches a timeseries sample's
`type`. A numeric target needs a unit pair. `activity_type`, `name`,
`description`, and `text_result` take none.

`when-absent` uses the line only when every earlier line that writes the same
our-field had no number. The unused their-field stays in `supplied_fields`. At
most one `when-absent` line writes a given our-field.

`when-writer` is a comma-separated list with no spaces. The line is applied only
when `data_writer` is in the list. Otherwise their field stays in
`supplied_fields` and the import continues. When two lines name the same
their-field, the line whose `when-writer` matches is the one that writes. The line
without `when-writer` is the fallback. That is how an Apple `skin_temperature`
sample becomes `wrist_temperature_sleeping` and every other writer keeps
`skin_temperature`. `apple.rs` `point_mapping` maps
`AppleSleepingWristTemperature` to `wrist_temperature_sleeping`. The unit is `°C`.

A timeseries line maps a raw sample. The import should be given the full
timeseries response, envelope included. If `metadata.resolution` is `raw`, a
`window_seconds` value stays in `supplied_fields`. If `metadata.resolution` is any
other value, every row is left out. If it is absent or null, a row with a
non-null `window_seconds` is left out. A sample with `period_end` is left out.
An envelope whose `data` is empty writes nothing and adds no line. Against Open
Wearables 0.10.0 and current main (`8adb8d0`), `computed_by` appears only on
aggregate rows, which are left out, so this release does not store a
connector-computed value under a provider's name.

Rejected alternatives:

- A JSON file. Rejected because a person already edits a calculation as a fenced
  block.
- A CSV of two columns. Rejected because a row cannot name the record, the unit
  pair, the basis, and the writer list without a second header language.
- Reuse the `ath-calculation` tag. Rejected because `ath apply` would run a
  mapping as a calculation.
- Treat `name` inside a section as the file name. Rejected because the workout
  field `name` is their field.
- Use `window_seconds` alone to decide that a row is raw. Rejected because raw rows
  on current Open Wearables main carry `window_seconds` too. The envelope's
  `metadata.resolution` is the check when the file has it.
- Store bare rows that carry `window_seconds`. Rejected because a bucket row would
  then be stored as a raw sample.
- Hard-code the device fields for `open_wearables` and `junction` in the engine.
  Rejected because a mapping file a person adds could not then keep two devices
  apart, which D183 requires.

## D178. Shipped mappings live in `mappings/`, and a person's copy lives in `.agents/mappings/`

The shipped files are `mappings/open-wearables.md` and `mappings/junction.md`. A
person's file is `.agents/mappings/<name>.md`. `ath init`, `ath skill install`, and
`ath --update` copy a shipped file only when that destination is missing. They do
not replace it. `ath init --no-skill` skips the mapping copy along with the skill.

An installed file wins over a later shipped file. That is accepted. A mapping is
not the tool skill, so D164's version replace does not apply. Mapping files are not
`skills/<name>/SKILL.md`, so `ath skill` does not list them and `ath apply` does
not run them.

Rejected alternatives:

- `skills/open-wearables/SKILL.md`. Rejected because `skillNames` treats every
  `SKILL.md` as a skill.
- Replace the installed file when the shipped file changes. Rejected because the
  person may have edited it, which is the reason D157 leaves a calculation in place.
- Give mappings a version and replace older copies. Rejected for this release. The
  installed file is the one the person added.

## D179. The installed file is read first, and it is checked before a write

`ath import <path> --mapping <name>` is the entry point. `--mapping` is read before
import detection. A value with `/`, `\`, or a drive prefix such as `C:` is a path.
Otherwise the installed file is used, then the shipped file, then the import fails
with `no mapping file named <name>`. D174 is unchanged: the flag stays in the
TypeScript command, and Rust only reads the file it is given.

The file is checked before anything is written. The four refusals are:

1. A personal report turned into a measured record. D161. Message:
   `refusing to turn a personal report into a measured record`. It covers an
   `athlete_report` or `nutrition_entry` section, an our-field of `source_id`,
   `report_type`, `reported_via`, or `rating`, and an input object that carries
   `report_type` or `reported_via`.
2. A generic heart-rate variability field renamed to `hrv_rmssd` or `hrv_sdnn`.
   D161. Message: `refusing to map {their_field} onto {our_field}; the field does
   not name the statistic`.
3. Their efficiency percentage written onto the computed efficiency.
   `appendix/source-formats.md` lines 29 and 39. Message: `refusing to map
   {their_field} onto {our_field}; efficiency is computed from asleep_seconds and
   in_bed_seconds`.
4. A workout peak written onto `maximum_heart_rate_bpm`. D172. The peak includes
   `max_heart_rate_bpm` and `hr_zones.max_hr`. Message: `refusing to map
   {their_field} onto maximum_heart_rate_bpm; that field is the athlete's known
   maximum`.

The check also rejects a `route`, or a method `{route}/v1`, that a built-in
importer already uses: `garmin_account_export`, `garmin_personal_api`,
`strava_bulk`, `apple_health`, `whoop_csv`, `oura_csv`, `manual_entry`,
`unknown_import`, and the methods `garmin_account/v1`, `garmin_personal/v1`,
`strava_bulk/v1`, `apple_health/v1`, `whoop_csv/v1`, `oura_csv/v1`,
`manual_entry/v1`, and `unknown_import/v1`. A mapping that reused those routes
would attach rows to a built-in source.

`ath check` does not read mapping files. After a mapped import, `ath check` still
passes, because the Rust writer produces records the schema accepts.

Rejected alternatives:

- Read only the shipped file. Rejected because D161 says a person can add a file.
- Check the mapping inside `ath check` after the write. Rejected because a bad file
  would already have stored a measured record.
- Guess the mapping from the JSON keys, with no `--mapping` flag. Rejected because
  a Garmin file and a mapped file are both JSON, and the Garmin reader has to keep
  running when the flag is absent.
- Let a mapping use `garmin_account_export`. Rejected because that route is the
  Garmin file source.

## D180. Units convert by a fixed pair

A unit pair converts only by this table. Any other pair fails with `cannot convert
{their_unit} to {our_unit} for {their_field}`.

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
| `percent` | `%` | the same number. SpO2 |
| `celsius` | `°C` | the same number. Skin temperature |
| `ml_kg_min` | `ml/kg/min` | the same number. VO2 max |
| `percent` | `score` | the same number. Body battery, scale 0–100 |

The result is a JSON number built in binary64. A whole result inside the safe
integer range is written as an integer, so 600 seconds is `600`, not `600.0`.
100 seconds is `1.6666666666666667` minutes. The number is not rounded again.

Our unit must be the field's own unit. A point quantity uses its
`POINT_MEASUREMENT_UNITS` value. `asleep_seconds` and `in_bed_seconds` use `s`.
`stage_minutes` uses `min`. `interruptions` uses `count`. A workout distance or
elevation gain uses `m`, and energy uses `kcal`. Any other unit fails with
`{our_field} uses {unit}`.

A `SourceValue` keeps their number and their unit as `input_value` and
`input_unit`, and ours as `standard_value` and `standard_unit`. A timeseries
sample whose `unit` differs from the line's their-unit is left out.

Rejected alternatives:

- Store their number unchanged and ignore units. Rejected because Junction stage
  fields are seconds and `stage_minutes` is minutes.
- A formula on the line. Rejected because a formula can divide asleep by time in
  bed, or relabel a heart-rate variability field.
- Store SpO2's standard unit as `percent`. Rejected because `oxygen_saturation`
  uses `%`.
- Round the binary64 result to a fixed number of decimals. Rejected because the
  test then depends on a second rounding rule.

## D181. The match key is the writer, the sensor, their id, and the kind

Their record id is the match key, as D161 says. The method is `{route}/v1`. A
session's key is `{writer}:{sensor}:{section}:{id}`. A child adds `point`,
`summary`, or `score`, then the quantity, summary type, or metric. A timeseries
sample has no id. Its key is `{writer}:{sensor}:timeseries:{kind}:{type}:{utc}`,
where `{utc}` is the time in UTC with milliseconds. A score uses its metric in
place of the type. The source device row is `{writer}:{sensor}:source_device`. An
empty sensor is written as `-`.

Every part is escaped before the parts are joined with `:`. A `\` becomes `\\` and
a `:` becomes `\:`. That includes the colons inside the UTC time. Two objects in one
file with the same key do not both write. The first is stored. The second is left
out as `duplicate match key {key}`.

A second device from the same provider has a different sensor string, so it has a
different key.

Rejected alternatives:

- Use their id alone. Rejected because a sleep id and a point taken from that sleep
  would update one row, and two devices that reuse an id would update one row.
- Use the original offset text in a timeseries key. Rejected because `06:12Z` and
  `02:12-04:00` are one instant.
- Drop a sample that has no id. Rejected because the Open Wearables timeseries
  object has no id, and the timestamp plus the type is the sample.

## D182. A re-run updates the same row

A re-run looks up the import key inside one transaction for each source object. The
stored `received_at` is copied onto the new record before the compare. An equal
record is not written again. A changed record keeps its id and its first
`received_at`, gets one new revision, and replaces the whole stored object. It is
not merged. The typed columns are set from the new object, and a column whose key
is absent becomes NULL.

This replace is new code (`Store::insert_group`). The built-in importers keep
`insert_record`, which merges through `revise`. `revise` stays a merge for
`ath-core revise --patch`. It now also refreshes the typed columns of a sleep
session, a point, and a provider score, not only a workout and a provider summary.

When the key names a record that was deleted, the record is not revised. The
object is left out as `the stored record {id} is deleted`.

Rejected alternatives:

- Insert a second row on every run. Rejected because D161 says their id is the
  match key so a re-run updates the same record.
- Treat a new `received_at` as a change. Rejected because the clock would revise
  every row on every run.
- Merge the new object into the stored one. Rejected because a field the source
  dropped would stay on the record.
- Revise a deleted record. Rejected because its deletion row names the revision it
  hid, and `ath check` would then fail.

## D183. The source names the device and the path

D161 says the source names the device and the path. The path is `data_route`. The
device is `sensor_type` on the source row, plus a `source_device` record.

The mapping file names the device fields. `sensor` lists the fields joined with
`|`, empty parts skipped. `device_name`, `device_model`, and `device_hardware` fill
the `source_device` record, first present field first. The manufacturer is the
writer. Open Wearables uses `source.device_type`, `source.device`, and
`source.source` for the sensor. Junction uses `source.type`, `source.device_id`,
and `source.app_id`. The lookup is writer, route, and sensor together, so an Apple
Watch, an iPhone, and a third-party app are three sources and three baselines.

Every stored score's `data_writer` is `source.provider`, whether or not
`computed_by` is present. A provider score stays that provider's score (D27,
D159). A stored row keeps `computed_by` verbatim in `supplied_fields`, and that
string does not change the writer. There is no `junction` writer value.

`ensure_source` is not changed. Mapped imports call `ensure_mapped_source`.

Rejected alternatives:

- One source per provider, with the model only in `supplied_fields`. Rejected
  because a watch and a phone would then share a baseline.
- Put the device only on `source_device` and keep one `source_id`. Rejected because
  baselines group by `source_id`, so the rows would still share a baseline.
- Use the empty-sensor lookup in `ensure_source`. Rejected because a mapped Garmin
  row would attach to the Garmin file source when the sensor is empty.

## D184. The program and the record stay one number, 2.1.0

`ath --version` and `database_metadata.athleticstandard_version` are `2.1.0`. D152
made them one number so a change does not have to pick which number moves. This
release keeps that rule. The mapping adds one optional field, `field_basis`, and no
required field. That is D185. Minor versions may add optional fields, which is the
rule in [v0.1.0/spec.md](../v0.1.0/spec.md) at the `athleticstandard_version` line,
and the rule D96 records. The number moves because the program moved and because
`field_basis` is new.

Migration step 4 stores `2.1.0` and SQL schema `4`. It adds no column and changes
no record. A new database runs step 3 and then step 4 when it is created. Step 3 is
not edited (D165). A `2.0.0` package still imports, because a `2.0.0` record is
valid `2.1.0` data. `schema/2.0.0/` is not rewritten. `schema/2.1.0/` is new.

Rejected alternatives:

- Leave the record at 2.0.0 while the program becomes 2.1.0. Rejected because D152
  and D164 already rejected a split, and a later change would again have to pick
  which number moves.
- Refuse a 2.0.0 package once the program is 2.1.0. Rejected because its records
  are valid 2.1.0 records, and the person would lose the way back from a backup.

## D185. A basis we assign is not a supplied field

`supplied_fields` holds names the source sent (D153, D161). `intensity` is one of
those names. It stays there verbatim. It is not a measured record.

`received_at_basis` is already an ATH-owned field on every measured record.
`ReceivedAtBasis` already allows `import_time`. Mapped imports set that. It is not
copied into `supplied_fields`. No new enum value is added.

The activity-type origin and the value basis are ours. They do not go in
`supplied_fields`. They go in one new optional object, `field_basis`, on the
record:

```ts
field_basis: z.strictObject({
  activity_type: z.literal("open_wearables_normalized").optional(),
  value: z.enum([
    "provider_or_open_wearables",
    "provider",
    "hrv_mean_rmssd",
  ]).optional(),
}).optional()
```

`WorkoutSessionData`, `ProviderSummaryData`, `ProviderScoreData`, and
`PointMeasurementData` gain the optional key. The objects are strict, so a 2.0.0
reader rejects the key. That is why the record version moves (D184).

A mapping line states the basis with `basis`. The Open Wearables workout line
`type: activity_type basis open_wearables_normalized` stores
`field_basis.activity_type` `open_wearables_normalized`, because Open Wearables
normalizes that word. A heart-rate summary line with `basis
provider_or_open_wearables` stores `field_basis.value` `provider_or_open_wearables`,
because a workout object has no flag that says who computed the number. The
Junction `average_hrv` line stores `field_basis.value` `hrv_mean_rmssd` on the
`hrv_rmssd` point. Every stored score stores `field_basis.value` `provider`. The
writer is `source.provider`. The enum has no `junction` value.

D161 said a new source is a field mapping, not a new record format. This release
still keeps the Athletic Standard record. It adds one optional field, in minor
version 2.1.0. That is the case D96 allows. It is not the Open Wearables object used
as the record, which is what D161 rejected.

Rejected alternatives:

- Put `activity_type_basis` and `value_basis` in `supplied_fields`. Rejected
  because D153 and D161 keep that object for names the source sent.
- Add a new `received_at_basis` value for mappings. Rejected because `import_time`
  already names an import.
- Add `field_basis` and also rewrite `schema/2.0.0/`. Rejected because that folder
  is the published 2.0.0 schema.
- Leave the basis off the record. Rejected because a later reader could not tell a
  normalized activity type from a name the athlete wrote.

## D186. `intensity` stays in `supplied_fields`

Open Wearables `intensity` is kept in `supplied_fields` under that name. It is not
a measured record. Garmin's `Feeling` is the same kind of field, and the Garmin
import already keeps it there.

No calculation may read `supplied:intensity` or `supplied:Feeling`. The ban covers
only those two names. Other names in `supplied_fields` are not covered.
`parse_skill` in `crates/ath-core/src/calc.rs` refuses either value before the
skill runs, with `a calculation cannot read supplied:intensity` or `a calculation
cannot read supplied:Feeling`. D103 says a source-provided label stays in its named
field and is not a normalized fact.

The tool skill says an agent may quote `intensity` and `Feeling` only as "the
source said", never as evidence. This is stricter than D70. D70 lets a vendor score
corroborate a claim but not be its basis.

Rejected alternatives:

- Name it in `left_out` and drop it. Rejected because D153 rejected dropping a
  column the person supplied, and D161 keeps any other field under the name it
  arrived with.
- Turn it into a linked athlete report. Rejected because the workout object is a
  measured record, and a second record would invent a personal report the file did
  not contain.
- Give it a typed field on the workout. Rejected because the record has no
  intensity field, and this release does not add one.
- Let a calculation read `supplied:intensity` or `supplied:Feeling`. Rejected
  because D103 keeps a source label out of the normalized facts a calculation uses.

## D187. Every stored value states its basis

Every measured record that an import, a mapping file, or `ath log` writes from
2.1.0 on has `field_basis.value`. A workout with an activity type also has
`field_basis.activity_type`. D185 put the object on four record types for mapped
imports only. This decision extends it to all seven measured record types and to
every importer. D185 is not rewritten.

The object gains these values, and one key:

- `value: "provider_or_junction"`. The provider or Junction computed the number.
  Junction calculates some summary numbers from samples, and the summary does not
  say which ones.
- `value: "provider_documented_rmssd"`. The column says heart-rate variability
  without naming the statistic. The provider documents it as RMSSD. D22 and D26
  record that WHOOP and Oura report RMSSD.
- `value: "calculated_on_import"`. This software calculated the value from the
  source's own samples.
- `value: "entered_by_hand"`. A person entered the value, or an agent entered it
  for them through `ath log`. It was not read from a device file or service.
- `activity_type: "provider_label"`. The writer named the activity. The software may
  drop Apple's `HKWorkoutActivityType` prefix and change the letter case. That does
  not change who chose the word.
- `route_summaries: "calculated_on_import"`. This software calculated an Apple
  workout's elevation gain or kilometre splits from its route file. The workout's
  own totals stay `value: "provider"`. One value could not describe both.

Each importer states what its source says:

| Importer | Records | `field_basis` |
|---|---|---|
| Apple Health | points, series samples, workouts, and activity summaries from a writer | `value` `provider` |
| Apple Health | a value whose writer is `Health` or `Manual` | `value` `entered_by_hand` |
| Apple Health | sleep sessions, which are sums of the writer's sleep-stage samples | `value` `calculated_on_import` |
| Apple Health | RMSSD from a beat list or an ECG, and ECG beat intervals | `value` `calculated_on_import` |
| Apple Health | a workout | `activity_type` `provider_label`, and `route_summaries` when the route file supplied splits or elevation gain |
| WHOOP | `Heart rate variability (ms)` as `hrv_rmssd` | `value` `provider_documented_rmssd` |
| WHOOP | every other point, score, sleep session, and workout | `value` `provider`; a workout also `activity_type` `provider_label` |
| Oura | the HRV column as `hrv_rmssd` | `value` `provider_documented_rmssd` |
| Oura | every other point, score, and sleep session | `value` `provider` |
| Garmin | account JSON workouts, GPX and FIT workouts, the `max_heart_rate` summary, the personal pull's workouts and `LastNightAvg` | `value` `provider`; a workout with an activity type also `activity_type` `provider_label` |
| Strava | workouts | `value` `provider`; `activity_type` `provider_label` |
| `ath log` | a plain-text measurement or benchmark result, and a structured measured record with no `field_basis.value` | `value` `entered_by_hand` |

A mapping file states its basis. The header key `value_basis` is required. It is
`provider`, `provider_or_open_wearables`, or `provider_or_junction`, and it applies
to every value the file writes. A line's own `basis` replaces it for that record.
Every provider score is `provider`. An `activity_type` line must state its basis.
The Open Wearables file says `provider_or_open_wearables`. The Junction file says
`provider_or_junction`. A file that does not state a basis is refused before
anything is written, so no basis is guessed.

`ath correct` stores the whole record the person gives, including its
`field_basis`. The tool skill says to set `entered_by_hand` when a correction
replaces a measured value with one a person typed.

Migration step 4 does not backfill. A record written before 2.1.0 keeps no
`field_basis`, and it stays valid because the key is optional. A restored 2.0.0
package keeps its records as they were. The stored rows do not say which
importer version wrote them, which writer was hand-entered at the time, or whether
an Apple route file was read. A basis guessed from the source row could be wrong,
and a wrong basis is worse than none. A later import of the same export states the
basis in a new revision, because the content then differs.

Rejected alternatives:

- Backfill old records in migration step 4. Rejected because the basis would be
  guessed, and every measured record would gain a revision the person did not make.
- Write `field_basis` only for mapped imports. Rejected because a reader could not
  tell an Apple sleep total this software summed from a WHOOP total WHOOP sent.
- One basis for every record an importer writes. Rejected because an Apple export
  holds writer values, hand-entered values, and values this software calculated.
- A basis for each field name. Rejected because one value per record, plus the
  activity type and the Apple route summaries, already describes every importer.
- Default a mapping file's basis to `provider`. Rejected because a connector may
  have computed the value, and the file's author is the one who knows.
- Refuse a structured `ath log` record that has no `field_basis`. Rejected because
  a value entered through `ath log` is entered by hand by definition, and existing
  agents would fail.
