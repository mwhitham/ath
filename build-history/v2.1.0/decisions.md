# v2.1.0 decisions

Decisions continue after D176. Earlier decisions stay as they are. The plan is
[spec.md](spec.md).

## D177. A mapping file is an `ath-mapping` block

Changed by D193 and D196 before 2.1.0 was published: the page and row field names
below are now settings in each mapping file, and a `when-absent` line is tried after
every other line.

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

Changed by D189 before 2.1.0 was published: the whole mapped import is now one
transaction, and each source object's records are written in a savepoint inside it.

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

## D184. 2.1.0 ships as option A: the program and the record stay one number

2.1.0 ships as option A. `ath --version` and
`database_metadata.athleticstandard_version` are both `2.1.0`. Option A is
[spec.md](spec.md) section 8.

The maintainer chose option A on 2026-10-31 at 8:52 AM CT. Asked whether saved data should also say 2.1.0, with a field_basis note on each value, the maintainer answered "yes".

D152 made the program version and the record version one number, so a change does not
have to pick which number moves. This release keeps that rule. The mapping adds one optional field, `field_basis`, and no
required field. That is D185. Minor versions may add optional fields, which is the
rule in [v0.1.0/spec.md](../v0.1.0/spec.md) at the `athleticstandard_version` line,
and the rule D96 records. The number moves because the program moved and because
`field_basis` is new.

Migration step 4 stores `2.1.0` and SQL schema `4`. It adds no column and changes
no record. A new database runs step 3 and then step 4 when it is created. Step 3 is
not edited (D165). A `2.0.0` package still imports, because a `2.0.0` record is
valid `2.1.0` data. `schema/2.0.0/` is not rewritten. `schema/2.1.0/` is new.

Rejected alternatives:

- Option B: leave the record at 2.0.0 while the program becomes 2.1.0. Rejected
  because D152 and D164 already rejected a split, and a later change would again
  have to pick which number moves.
- Refuse a 2.0.0 package once the program is 2.1.0. Rejected because its records
  are valid 2.1.0 records, and the person would lose the way back from a backup.

## D185. A basis we assign is not a supplied field

Changed by D192 before 2.1.0 was published: `provider_or_open_wearables` is now
`provider_or_aggregator`, and `open_wearables_normalized` is now
`aggregator_normalized`.

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

Changed by D192 before 2.1.0 was published: `provider_or_open_wearables` and
`provider_or_junction` are now one value, `provider_or_aggregator`. D195 limits the
bases a mapping line may claim.

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

## D188. Every pull request runs the release workflow

`.github/workflows/release-cross-platform.yml` runs on every pull request, as well
as by hand and on a `v*` tag. Its test job runs `cargo test`, the TypeScript tests,
and the release build on Linux, macOS, and Windows. Its package job packs the
tarball and runs the installed-package smoke test. Only a `v*` tag publishes,
because the publish step checks that the ref is a tag.

The pull-request workflow ran the TypeScript tests on Linux only. On macOS and
Windows it built and tested the Rust crate, and ran one test file on Windows. The
2.0.0 release run was the first time the TypeScript tests met macOS and Windows,
and it failed after the tag was pushed. A pull request now meets the same jobs
before anything is tagged.

The Windows test job installs `sqlite3`, because `ath migrate` and `ath repair` need
it for a verified backup (D137). Every test job prints `sqlite3 --version`, so a
missing tool stops the job before the tests run.

Rejected alternatives:

- Copy the release test matrix into `ci.yml`. Rejected because two copies drift
  apart, and the release workflow is the one that has to pass before a tag.
- Run the release workflow by hand before tagging. Rejected because a hand step can
  be skipped, and a failure would still show up only at release time.
- Call the release workflow from `ci.yml`. Rejected because both workflows upload
  `ath-core-*` artifacts under the same names, and a called workflow shares the
  caller's run.

## D189. One mapped import is one transaction, and the preview runs it

`ath import --mapping` writes inside one SQLite transaction. It begins before the first
row is read and ends after the last one. Each source object's records are written in a
savepoint inside it. Any error rolls back the whole import. A failure partway through
leaves nothing written, including the sources and devices made earlier in the run.

The preview is the same run, rolled back at the end. It reads the same stored records
and makes the same checks. So it shows the same `left_out` lines, the same counts, and
the same error that the write would. Only `record_ids` differs. It is empty, because
nothing was kept.

The result gains `outcome`: how many records are new, changed, and unchanged. The text
output prints it as `outcome: 22 new, 0 changed, 0 unchanged`.

The transaction starts with `BEGIN IMMEDIATE`. It holds the write lock from the first
read, so another writer cannot change a row between the lookup and the write.

The final review found the problem. A bad row late in a file stopped the import after
the earlier rows were committed. The old preview skipped every write. It could not see a
deleted source, a deleted record, or a row the store would refuse.

This changes D182, which said each source object had its own transaction. D182 was
added in this release, which is not published. It now starts with a short note that
points here. The rest of its text is unchanged.

Rejected alternatives:

- One transaction for each source object, as D182 first said. Rejected because a
  failure in a later object left the earlier objects written.
- A preview that only reads. Rejected because a row's outcome can depend on a row
  written earlier in the same run, such as a new source or a new key.
- Check every row in a first pass, then write in a second. Rejected because the store's
  own checks run only when a record is written, so the two passes could disagree.

## D190. A deleted mapped source is left out

When a person deletes the source a mapped import writes under, a later import of the
same file leaves out that source's rows. Each row adds `{label}: its source {id} is
deleted` to `left_out`. Nothing is written under the source, and no second source is
made. Rows under other sources are written as usual.

`ensure_mapped_source` finds the source by writer, route, and sensor, whether or not it
is deleted. A deleted source keeps its row, and the unique index on those three columns
still covers it. Before this, the lookup skipped deleted sources. The import then tried
to add a second source with the same three values, and SQLite stopped it with a UNIQUE
error.

An erased source is removed with every record that depends on it and their import keys.
A later import makes the source again, with new records.

This follows D182. A deleted record is not revised, and its object is left out. A
deleted source is the same rule for the source.

Rejected alternatives:

- Make a new source beside the deleted one. Rejected because the unique index refuses
  it. Dropping the index would let one device become two sources.
- Undelete the source. Rejected because a person chose to hide it, and an import should
  not undo that.
- Refuse the whole import. Rejected because the rows under other sources are still
  good. D182 already leaves out only the deleted record's object.

## D191. A supplied field needs a name every reader keeps

The store refuses a record whose `supplied_fields` has an empty name, or a name
`__proto__` at any depth. The check covers the record's own `supplied_fields` and the
`supplied_fields` of each route point. It runs in `ath-core` before a record is
inserted or revised, so it covers every importer that writes through the store. The
message names the record and the reason:

```text
sleep_session bad-night: a supplied field has an empty name, and the format needs a name for every field
sleep_session bad-night: a supplied field is named __proto__, which JavaScript readers drop
```

Under D189 the whole mapped import then stops, and nothing is written.

The record schema already requires a name of at least one character (`SuppliedFields`
in `src/v1/schema.ts`). The Rust writers do not run that schema. A mapped row with a
`""` key was stored. Then `ath check` said the database was invalid, and `ath export`
failed. A `__proto__` key was stored too. `ath check` passed, but `ath export` dropped
the key without a warning, at the top level and inside nested objects. JavaScript
treats that name as the object's prototype, not as a field.

Other names are kept. A name of spaces, a name with control characters, and an empty
name inside a nested value all pass `ath check` and come back from `ath export`
unchanged.

The TypeScript package import already refuses an empty name, through the schema. It
still drops a `__proto__` name in a package that another program wrote. This release
does not change that reader.

Rejected alternatives:

- Drop the bad field and store the rest. Rejected because D153 and D161 keep what the
  source sent, and the loss would be silent.
- Rename the field, for example to `_empty`. Rejected because the stored name would no
  longer be the name the source sent.
- Check only in the mapping engine. Rejected because the Garmin, Strava, and
  personal-pull writers also copy source names into `supplied_fields`.

## D192. A basis names no product

`field_basis.value` `provider_or_aggregator` replaces `provider_or_open_wearables` and
`provider_or_junction`. It means the provider or the service that passed the data on
computed the number, and the record does not say which. `field_basis.activity_type`
`aggregator_normalized` replaces `open_wearables_normalized`. It means that service
chose the activity word. The other values are unchanged.

The Open Wearables file and the Junction file both state `value_basis:
provider_or_aggregator`. The Open Wearables workout line is `type: activity_type basis
aggregator_normalized`. The source's route, `open_wearables` or `junction`, still names
the service. A reader who needs the service reads the source.

The final review asked for this. A product name in the format means each new service
needs a new value and a new schema. With one value, a mapping file for another service
needs no format change.

`schema/2.1.0/` is regenerated. 2.1.0 is not published, so no stored record or package
has the old values. A test keeps the engine's lists equal to the schema's (`names the
same bases as the record schema` in `tests/mapping-cli.test.ts`).

This changes D185 and D187, which were added in this release. Each now starts with a
short note that points here. The rest of their text is unchanged.

Rejected alternatives:

- One value for each service. Rejected because each new service would change the
  record format.
- Use `provider` for data a service passed on. Rejected because the service may have
  computed the number, and `provider` would claim more than the file says.
- Keep the old values beside the new one. Rejected because no released record uses
  them, and two names for one meaning need a rule for which one to write.

## D193. A mapping file names its own layout

The engine, `crates/ath-core/src/mapping.rs`, names no source. Every field name that
belongs to one source's layout is a setting in that source's mapping file. These are
new header keys:

- `personal_when: {field} {value}`. A row whose field has that value is a personal
  report. It is left out as `{field} {value} is a personal report`. The Open Wearables
  file says `personal_when: entry_source manual`.
- `page_rows`, `page_marker`, and `page_section`. An input whose root has the
  `page_rows` array and the `page_marker` key is one page. Its rows map with the
  section `page_section`. The Open Wearables file says `data`, `metadata`, and
  `timeseries`.
- `page_resolution` and `raw_resolution`, which go together. When a page has a value
  at `page_resolution` and it is not `raw_resolution`, every row is left out. The Open
  Wearables file says `metadata.resolution` and `raw`.

A section now names the fields of its rows. `[sleep]` and `[workout]` need `id_field`,
`start_field`, and `end_field`. `[timeseries]` needs `time_field`, `type_field`,
`value_field`, and `unit_field`. It may also name `aggregate_field`, whose presence
marks an aggregate row, and `window_field`, which cannot be confirmed raw on a page
with no resolution. The Open Wearables file says `timestamp`, `type`, `value`, `unit`,
`period_end`, and `window_seconds`. A timeseries line reads `{type_field}.{type}`.

The check refuses a section without its settings, a setting the section does not use, a
timeseries line that does not start with the type field, and a page that is only partly
described.

The shipped files write the same records and the same `left_out` lines as before, with
two changes. A page given to a file that does not describe pages is read as plain
arrays, so its rows are `data: no mapping section`, not `timeseries: no mapping
section`. The envelope line is built from the settings: `envelope is not a
{page_section} page: pass its {page_rows} array under workout or sleep`. For the Open
Wearables file it is the same line as before.

The final review found Open Wearables and Junction names in the engine: `entry_source`,
`metadata.resolution`, `window_seconds`, `period_end`, the sample's `type` and `unit`,
and the heart-rate variability field names (D194). A person who adds a mapping for
another source could not change them.

This changes D177, which was added in this release. It now starts with a short note
that points here. The rest of its text is unchanged.

Rejected alternatives:

- Keep the names in the engine as defaults. Rejected because a default is still one
  source's layout, and a file that left out a setting would get it without knowing.
- A separate settings file for each source. Rejected because the mapping file is
  already the one file a person reads and edits for that source.

## D194. The heart-rate variability check reads names, in every record a line writes

A line whose target names RMSSD or SDNN must read a field that names the same
statistic. The target names it when `rmssd` or `sdnn` is in the point quantity, the
summary type, or the score metric, in any letter case. So `hrv_rmssd`,
`provider_summary.hrv_rmssd`, and `provider_score.sdnn_balance` are all checked. The
field names it when its name contains `rmssd` or `sdnn`, and not both. For a timeseries
line, the name is the type after `{type_field}.`.

A field that names neither statistic may write an RMSSD target only with basis
`hrv_mean_rmssd` or `provider_documented_rmssd`, and only with a `when-writer` list.
That keeps the shipped Junction line `average_hrv: hrv_rmssd ms ms basis hrv_mean_rmssd
when-writer oura,whoop_v2,garmin,fitbit,polar`. Every other case is refused with D179's
message: `refusing to map {their_field} onto {our_field}; the field does not name the
statistic`.

This replaces two fixed lists of Open Wearables and Junction names in the engine:
`heart_rate_variability_rmssd` for RMSSD, and `heart_rate_variability_sdnn` and
`hrv_mean_sdnn` for SDNN, plus the exception for `average_hrv`. The rule is still
D161's: a value is stored as RMSSD or SDNN only when the source says which. `LastNightAvg`,
`hrv`, and `HRV` are still refused for both.

The final review also found that the check covered only point records. A line could
put a field with no statistic in its name into `provider_summary.hrv_rmssd`. That is
now refused too. A provider score whose metric names neither statistic is not checked,
because it is that provider's own score (D27, D159).

Rejected alternatives:

- Keep the fixed lists and add each new source's names. Rejected because each new
  source would need an engine change, which D193 removes.
- Check only point records. Rejected because a summary named `hrv_rmssd` makes the same
  claim.
- Let any basis stand in for the name. Rejected because only an RMSSD basis says which
  statistic a field holds, and only for the writers the file lists.

## D195. A mapping line claims only a basis a mapping can know

A line's `basis` may be `provider`, `provider_or_aggregator`, `hrv_mean_rmssd`, or
`provider_documented_rmssd`. The two RMSSD bases are for `hrv_rmssd` only. An
`activity_type` line uses `aggregator_normalized` or `provider_label`. A file's
`value_basis` is `provider` or `provider_or_aggregator` (D187, D192).

A line that claims another value fails the check with `a mapping line cannot claim
basis {token}; use one of provider, provider_or_aggregator, hrv_mean_rmssd,
provider_documented_rmssd`. An RMSSD basis on another field fails with `basis {token}
is only used for hrv_rmssd`.

`calculated_on_import` means this software calculated the value. A mapping copies
values and calculates none. `entered_by_hand` means a person typed the value. A mapped
file comes from a device or a service. Before this, a line could claim either one, and
the stored basis would be false.

Rejected alternatives:

- Allow every `field_basis.value`. Rejected because two of them describe work a mapping
  never does.
- Allow `entered_by_hand` in a mapping file a person wrote. Rejected because the person
  wrote the mapping, not the values. The values still come from the input file.

## D196. The first line that writes a field wins

When two lines of one section write the same our-field for one row, the first line in
the file that has a value to write writes it. A `when-absent` line is tried after every
other line, wherever it sits in the file. The fields of the lines that did not write
stay in `supplied_fields` under their names.

Before this, both lines wrote. A session field kept the last value. A point, summary,
or score got two records with one match key, so the second revised the first. Every
later import of the same file revised the record twice more, with no change in the
file.

The check does not refuse the second line. A file may list a fallback on purpose. The
Junction file reads `stage_asleep_second`, then `total` with `when-absent`.

D177 said a `when-absent` line looks only at the lines before it. The short note at the
top of D177 points here too.

Rejected alternatives:

- Refuse two lines that write one field. Rejected because a fallback field is a real
  need, and `when-absent` already marks one.
- Let the last line win. Rejected because D181 already stores the first of two objects
  with one key, and one rule for both is easier to follow.

## D197. The Windows release job retries Chocolatey, then uses a pinned archive

The Windows test job in `release-cross-platform.yml` runs `choco install sqlite` up to
three times. A try counts only when `sqlite3` is on the path afterwards. Chocolatey's
exit code is not trusted. The job waits 20 seconds after the first failed try and 40
after the second. If `sqlite3` never appears, the job downloads
`sqlite-tools-win-x64-3540100.zip` from sqlite.org, checks its SHA-256, and adds the
folder to `PATH`. A different hash stops the job. The step ends with `exit 0`.
Without it, the runner would end the step with Chocolatey's exit code. Every test job
still prints `sqlite3 --version` before the tests (D188).

The SHA-256 is `fb20997ade5583a5fbf1e36db7bc3c82c9416b3f314f16961e1572aa4e49487d`. The
file's SHA3-256 matches the value sqlite.org lists for it. The archive holds
`sqlite3.exe` at its top level.

In release run 41375453717 on this pull request, Chocolatey answered HTTP 503 for the
sqlite package. It installed 0 of 0 packages and still exited 0, so the install step
passed. The job then failed at `sqlite3 --version`. Nothing in the code was wrong.

The first version of this step trusted Chocolatey's exit code. In that run it would have
passed after one try and never used the archive. The final review found this. The step
was run locally in PowerShell 7.4.6 with a `choco` that exits 0 and installs nothing. It
made three tries and then used the archive.

Rejected alternatives:

- Trust Chocolatey's exit code. Rejected because Chocolatey exited 0 after the 503,
  with nothing installed.
- Download from sqlite.org only. Rejected because then an outage at that one server
  stops every run.
- Retry Chocolatey with no fallback. Rejected because a longer outage would still stop
  the release.
- Download without a hash check. Rejected because the job would then run whatever file
  the server returned.
