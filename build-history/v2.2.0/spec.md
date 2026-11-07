# Athletic Standard 2.2.0

Status: **plan only. The maintainer has not approved it. Not approved for build.**

`ath pull openwearables` reads one person's stored health data from an Open
Wearables server they run, and writes it with the mapping this release
ships. The proposals are D198 through D204 in
[decisions.md](decisions.md). They are not adopted. The evidence is
[appendix/open-wearables-api.md](appendix/open-wearables-api.md), checked on
6 and 7 November 2026.

This pull request is the plan. The build after approval is one later pull
request. This plan does not change a version number, a schema, a fixture, or
the program.

Version 2.1.0 on `main` `8db90b42ac654767e9e9570a121bbbd4fde3348a` is the
software this plan was read against. That commit is the merge of pull request
39. The pull request head was `72b9db9`.

## 1. What a person does

They already run Open Wearables and have already connected their devices in
that server's own portal. This command does not sign them in to Garmin, WHOOP,
Oura, Apple, or anyone else. It does not start a sync.

They save the key once:

```text
ath key set openwearables
```

The program asks them to paste the key and does not print it. A script may set
`OPEN_WEARABLES_API_KEY` instead.

They name the server, the user, and the range:

```text
ath pull openwearables \
  --base-url http://127.0.0.1:8000 \
  --user 00000000-0000-4000-8000-000000000001 \
  --from 2026-01-01 \
  --to 2026-01-07
```

The same three values may come from `OPEN_WEARABLES_BASE_URL`,
`OPEN_WEARABLES_USER_ID`, and the key variable above. A flag wins over the
environment when both are set.

The first run writes nothing. `--json` writes nothing, including when
`--yes` is also present. That is the same rule as `ath import --mapping`.
`ath pull garmin --json` still writes. `--file` chooses a database, as on
the other commands.

A review of the previous head asked for the committed result in JSON as well
as in text. This proposal does not add a JSON mode that writes. Section 7
requires `--json` and `--json --yes` to leave the database unchanged. The
committed result is the text of `--yes` without `--json`. A writing JSON
report would be a different choice. It is not this proposal.

Preview text, and every `--json` result, labels counts `estimated`. Each
page is counted against the database as it was before this command, then
rolled back. A match key that appears on two pages, or a server that changes
between the preview and `--yes`, can make the estimate differ from the
write. The estimate is not a promise of the final new, changed, and
unchanged split. Exact equality would need a command-wide overlay. This
proposal does not add one, and it does not add that work to `mapping.rs`.

Preview text prints:

- one line that a date-only bound is sent as typed, and Open Wearables decides
  what that day means
- `preview only; nothing stored`
- `reads finished` or `reads incomplete`
- one line each for the twelve reads in section 2: `completed`, `failed`,
  or `not_attempted`
- for body, one extra line: `body snapshot stored`, `body snapshot empty`,
  or `body snapshot incomplete`
- estimated counts, by record type, of sleep sessions, workouts, points,
  measurement samples, summaries, and scores
- committed counts of zero
- the coverage lines in section 2
- `left_out` as a total plus at most 20 example lines
- `nothing written; pass --yes after reviewing this preview`

`--yes` without `--json` commits each successful page. That commit is one
`apply_mapping` transaction (D189). It is not a completed pull. The text
prints actual committed counts for pages kept. It prints `requested range
<from> through <to> is complete` only when every ranged read in section 2
reached that read's terminal condition. Body is not one of those reads.
The success sentence `pulled openwearables from <from> through <to>` is
printed only when the range is complete and body and the personal record
also completed. A finished range with an incomplete body snapshot does not
print that sentence. The body line still says the snapshot is incomplete.

If a later page fails, the child returns a typed outcome to the parent
before the process exits nonzero. The parent renders that outcome as text,
then uses the exit status. The text says the requested range is incomplete,
names the failed read, names any read that is `not_attempted`, and prints
the retained counts. A failed read says to run the same command again. An
identity refusal uses the sentence in section 3, which says a rerun cannot
repair the device id. It does not print the success sentence. The failed page makes no database changes. Records
already stored stay, including their revisions. An empty range whose reads
finished says the range is complete. `written` is false when the confirmed
command changed nothing. The database is unchanged. That is not a claim that
the database has no rows.

Public `--json`, with or without `--yes`, is only the preview outcome below.
`written` is false. `committed` is zero. `estimated` holds the page-local
estimates. `range_complete` is false. `reads_finished` is true only when
every read completed. The read status token is
`not_attempted`, in this object and in the text. The object has no key,
server, user id, or request path.

```json
{
  "from": "2026-01-01",
  "to": "2026-01-07",
  "written": false,
  "range_complete": false,
  "reads_finished": false,
  "reads": {
    "sleep": "completed",
    "workouts": "failed",
    "timeseries_raw": "not_attempted",
    "timeseries_day": "not_attempted"
  },
  "estimated": {
    "sleep_session": { "new": 1, "changed": 0, "unchanged": 0 }
  },
  "committed": {
    "sleep_session": { "new": 0, "changed": 0, "unchanged": 0 }
  },
  "left_out": { "total": 0, "examples": [] }
}
```

The same record types are present for workouts, points, summaries, and
scores. The example above is shortened. It is the preview outcome.

The child returns a different outcome for `--yes` without `--json`. That
outcome has the actual `committed` counts, `database_changed`, and
`range_complete`. `written` is true only when that confirmed command
committed at least one new or revised record. A successful repeat has
`written` false, `database_changed` false, `range_complete` true, and
unchanged counts. A later-page failure has `range_complete` false and the
retained counts. The parent prints sentences from this outcome. It does not
print the outcome as public JSON.

`--date` stays required only for `garmin`. `--from` and `--to` are required
only for `openwearables`, and they are flags, not environment variables.
`--base-url` and `--user` are required for `openwearables` unless the
environment named above supplies them. `ath pull garmin --date YYYY-MM-DD` is
otherwise unchanged. `ath pull` with any other name still fails. The failure
text becomes `pull reads garmin or openwearables`.

The saved key can read every user on that server. `ath key` shows whether a
key is saved in the password store and whether `OPEN_WEARABLES_API_KEY` is
set, without printing either value. Those are different facts. `ath key`
with no further word does not remove the saved key.

`ath key clear` with no name still clears only the gateway keys, `vercel`
and `openrouter`, and it still exits successfully. Open Wearables is not a
gateway name, so this command does not delete that saved key. When a saved
Open Wearables key remains, the text keeps the gateway sentence and adds
`The Open Wearables key remains saved.` and `Remove it with ath key clear
openwearables.` When no saved Open Wearables key remains, the gateway
sentences stay as they are today.

`ath key clear openwearables` removes the saved key from the password store.
It does not change `OPEN_WEARABLES_API_KEY`, and it does not revoke the key
on the server. When that variable is set, the text says the saved key was
removed and the environment key is still available, so a pull can still use
it. `ath key` still reports the environment key after the saved key is gone.
When the variable is unset, the text says only that the saved key was
removed. `ath key clear vercel` and `ath key clear openrouter` stay the
gateway commands they are today.

## 2. What this pull stores

D204 is the storage rule. D203 keeps these values in this pull. D204
corrects how they are identified, timed, and classified. An earlier draft
left day values, the heart-rate stream, summaries, and workout segments for
a later version. That draft is superseded. File imports stay. The Garmin pull stays. A Junction pull is proposed as
2.3.0. Automatic sync, retries, token refresh, and backfill are proposed as
2.4.0. None of those numbers is adopted. This release adds one module for
this server. It does not add a connector interface.

This pull stores the representations Open Wearables 0.11.0 returns for one
user. A ranged read stores that range. Body does not. A returned number
keeps the server's unit. A returned null stays absent and is not stored as
zero. A returned zero stays zero. A row the server did not return is not
labeled provider-unavailable. Priority selection, a day-row pick, and a
rollup can omit a stored row from the response. That omission is unknown
here. It is not a null, and it is not a refusal. `left_out` counts only a
row the server returned and this program refused.

The promise is the returned object. It is not a copy of the Open Wearables
database, and it is not every device on a route that selects one source.
Sleep events, workouts, and raw and day timeseries send
`filter_by_priority=false` where that parameter exists. Sleep, activity,
and recovery summaries have no such parameter. Their services select a
source. This pull does not send a priority switch and does not change the
server's priority. Activity summaries also add together data sources that
share provider, writer, model, and type
(`series_aggregate_repository.py`, `daily_activity`). The stored source is
the source object on the response. It is not a receipt that every
contributor was that device.

The twelve reads, in order, use these paths under
`/api/v1/users/{user_id}`:

| Read | Path and query | Response and end | What the time means |
|---|---|---|---|
| `sleep` | `/events/sleep`. `start_date`, `end_date`, `limit`, `cursor`, `filter_by_priority=false`, `include=stages` | Cursor page. Ends at `has_more: false` | Session start and end |
| `workouts` | `/events/workouts`. dates, `limit`, `cursor`, `include=zones`, `include=segments`. No priority parameter | Cursor page. Ends at `has_more: false` | Session start and end |
| `timeseries_raw` | `/timeseries`. `start_time`, `end_time`, `limit`, `cursor`, `resolution=raw`, `filter_by_priority=false`. `types` omitted | Cursor page. Ends at `has_more: false` | Sample `timestamp` |
| `timeseries_day` | `/timeseries`. Same, with `resolution=1day` | Cursor page. `limit` counts periods, not rows. Ends at `has_more: false` | Server calendar window, not the provider's original interval |
| `sleep_summary` | `/summaries/sleep`. dates, `limit`, `cursor`. No priority parameter | Cursor page. Ends at `has_more: false` | `date`, plus main-sleep bounds only on the fields named below |
| `activity_summary` | `/summaries/activity`. dates, `limit`, `cursor`. No priority parameter | Cursor page. Ends at `has_more: false` | `date` only |
| `recovery_summary` | `/summaries/recovery`. dates, `limit`, `cursor`. No priority parameter | Cursor page. Ends at `has_more: false` | `date` only |
| `health_scores` | `/health-scores`. dates, `limit`, `offset`. No category or provider filter | Offset page with `pagination.has_more`. Ends at `has_more: false` | Score `recorded_at` |
| `meals` | `/events/meals`. dates, `limit`, `cursor` | Cursor page. Ends at `has_more: false` | Meal `timestamp` |
| `cycles` | `/events/menstrual-cycles`. dates, `limit`, `cursor`. The service clears `end_datetime` | Cursor page, then the local start rule below. Ends at `has_more: false` | Cycle `start_time` and `end_time` |
| `personal_record` | `/personal-record`. No dates | One object. The top-level `user_id` is removed before the privacy scan and is not stored | No measurement time |
| `body` | `/summaries/body`. No dates, no limit. `average_period` and `latest_window_hours` omitted, so the server uses 7 days and 4 hours | One `BodySummary`, or JSON `null`. `null` is an empty snapshot, not a bad page | Current snapshot. Not the requested range |

A cursor page is a JSON object whose `data` is an array and whose
`pagination` is an object with boolean `has_more`, checked before mapping.
An offset page is the same, and it has no `next_cursor` requirement. Body
and the personal record are not those envelopes. A cursor rule applied to
them is a failed read. Health-score `metadata` is not a record.

The menstrual-cycle service sets `end_datetime` to null so a predicted
future end is not dropped. This program still sends the requested dates.
Before mapping, it keeps a cycle whose start falls in the requested range
and keeps that cycle's end as sent, including an end after the range. For
date bounds, the date is the `YYYY-MM-DD` at the start of `start_time`,
with no timezone conversion, and the dates are inclusive. For timestamp
bounds, the start instant is greater than or equal to `--from` and less
than `--to`. A start that cannot be compared, because it has no offset, is
`left_out`. It is not given `Z`. A start after the range is not stored and
is not `left_out`. Paging continues until `has_more` is false. The server
still applies its own start bound, so a cycle that started before `--from`
is not returned. That is the server's filter.

A page over 8 MiB is not mapped. On a cursor or offset read, the same
cursor or the same offset is sent again with `limit` halved, down to 1.
The discarded response writes nothing and does not advance the cursor or
the offset. A transport failure is not retried. Day `limit` counts periods.
A `limit` of 1 can still hold many rows, one for each source and series in
that period. The server also floors that period count at 1. If the minimum
page is still over 8 MiB, the read is incomplete. The message names the
read and, for timeseries, the resolution. It does not name a series type,
and it does not say one row was too large. Body and the personal record
have no limit. An oversized response fails that read and names the read.
Earlier committed pages stay. The pull does not drop a series, a segment,
or a location to stay under the cap.

Device rows and other rows do not share a source. A device source uses the
2.1.0 sensor, `device_type|device|source`. Its receipt rules in section 3
are unchanged. A row enters that source only when its `source` object has a
nonempty `device_id`, or when it is an id-less sleep or workout on a new
source with no history. A raw or day sample whose `source` object exists
and whose `device_id` is missing, null, or blank is `left_out`. The reason
is the missing device id. No id is copied from another row.

A summary built without `device_id` uses a different sensor,
`summary:{family}:{provider}:{device}:{device_type}:{source}`. A missing
part is `-`. Sleep, activity, recovery, and body summaries use that sensor.
The builders in `summaries_service.py` omit `device_id`. Body's `source`
is the newest slow-changing measurement's provider, writer, model, and
type. The service discards provider and device on the latest readings.
That object is the response's umbrella source. It is not the author of
every field, and it is not a device receipt. A meal or cycle with no
`device_id` uses `event:{family}:{provider}:{device}:{device_type}:{source}`.
A meal or cycle that has a `device_id` uses the device source. A timeseries
or score with `source: null` uses
`unattributed:{resource}:{author}`. A health score's sensor is
`score:{author}`, where `{author}` is the top-level `provider`, or `-`
when that field is null. The score's `source` is the input object. It is
stored on the record and does not choose the sensor.

These sensors do not establish `device_binding`. Their history is not
history of the device source. A summary stored first does not block a
later raw page from binding its own device source. A bound device source
does not reject a later summary. Preview memory is kept per sensor. A
refused device page rolls back only that page.

Match keys stay D181 for the existing raw points, the raw `vo2_max`
summary, and the raw `garmin_stress_level` and `garmin_body_battery`
scores. A day row adds `1day` and `period_end` to its key, so it does not
replace the raw row. A summary field's key is the writer, the summary
sensor, the family, the `date`, and the field path. A meal, cycle, or
score uses the server id. Two meals at one timestamp stay two records.
Two metrics on one date stay two records. A body field's key is the
writer, the body sensor, the group, and the field path. It has no
requested date. A later snapshot revises that record. `received_at` stays
the time this program stored the revision. It is not the measurement time.
The personal record's key is the writer, its sensor, and `personal_record`.
The athlete profile is not changed. Birth date, gender, and sex do not fit
that profile: it has a birth year, a two-value sex, and no source.

The original server object is stored on one anchor record, in
`supplied_fields`, including nulls, text, booleans, codes, and nested
objects. A normalized number is a second record. Its `supplied_fields`
hold the leaf it came from, including that leaf's unit. Transport through
this API is not by itself evidence the value was measured (D161). The
classes are:

- A raw point the 2.1.0 file already stores stays that point. The lines
  are RMSSD, SDNN, resting heart rate, respiratory rate, skin temperature,
  blood oxygen, and one-minute heart-rate recovery. Apple skin temperature
  stays `wrist_temperature_sleeping` when the writer is `apple`.
- Raw `vo2_max` stays `provider_summary.vo2_max`. Raw
  `garmin_stress_level` stays a provider score on scale `1-100`. Raw
  `garmin_body_battery` stays a provider score on scale `0-100`. Body
  battery is not a sample.
- These raw series are points because Athletic Standard already has one
  quantity for each, and the original series name stays in
  `supplied_fields`: `walking_heart_rate`, `body_temperature`, `height`,
  `weight` as `body_weight`, `body_fat_percentage`, `lean_body_mass`,
  `blood_pressure_systolic`, and `blood_pressure_diastolic`.
- The other raw series readings are measurement samples. The appendix
  lists all 71. The number is unchanged. The unit is the server unit,
  using the existing conversions `percent` to `%`, `celsius` to `°C`,
  `degrees` to `deg`, and `brpm`, `bpm`, `ms`, and `m` unchanged. A new
  series quantity uses the server's unit string when no conversion exists.
  Latitude and longitude are samples. A negative coordinate stays negative.
  `heart_rate` is a sample. It is not a point. Integer `window_seconds`
  sets `duration_ms` to that count times 1000, and the original field
  stays.
- Dietary series, and `number_of_alcoholic_beverages`, are reported
  intake. Each is a provider summary at the sample time. The summary type
  is the series name. They are not nutrition entries and not samples.
- These raw series are provider summaries, not samples:
  `peripheral_perfusion_index`, `breathing_disturbance_index`,
  `body_mass_index`, `cardiovascular_age`, `workout_effort_score`,
  `estimated_workout_effort_score`, `garmin_fitness_age`,
  `withings_metabolic_age`, and `nike_fuel`. The estimated effort score
  keeps that name. Its basis is omitted. It is not relabeled as measured.
- A day row uses the same class, with the window below. The two Garmin
  scores stay scores. A day row of a point series is a provider summary,
  not a point. `vo2_max` on a day row is a window summary. Its key is not
  the raw `vo2_max` key.

`computed_by: provider` sets basis `provider`. `computed_by:
open_wearables` sets basis `aggregator`. `computed_by: null` omits the
basis and keeps the null. A raw row has no `computed_by`. Its basis stays
`provider_or_aggregator`. A health score with top-level `provider:
internal` has basis `aggregator`. That score was computed by Open
Wearables. A vendor name in that field has basis `provider`. A null
`provider` omits the basis. The writer field on the device sensor is not
this author.

A health score with a finite range in `constants/health_scores.py` is a
provider score. The scale text is `{min}-{max}`. The unit is `score` when
the number is present. Polar readiness is `0-10`. Polar recovery is `1-6`.
WHOOP strain is `0-21`. Sleep, and WHOOP recovery, are `0-100` only for
the provider entries in that table. Polar strain has no finite maximum, so
it is not given a scale. A category or provider missing from the table is
not given `0-100`. That score is a provider summary with no numeric
`value`. The number, qualifier, components, and `event_record_id` stay on
the anchor. A null score value uses the same kind as a present value for
that provider and category, with `value` and unit both absent. Components
are not turned into extra scores. RMSSD and SDNN stay separate names.
`RecoverySummary.recovery_score` is a deprecated 0 to 100 field. A present
number is a provider score on scale `0-100`. A null is only the anchor's
original object.

WHOOP recovery components can include `skin_temp_celsius`. The health-score
object keeps that component. `RecoverySummary` has no skin-temperature
field. Its class ends at `recovery_score`. Its `avg_hrv_sdnn_ms` text says
current rows are null because they come from WHOOP and that HRV is RMSSD.
A non-null SDNN is stored. RMSSD is not copied into it. The body snapshot
does not fill historical temperature.

Sleep summary `start_time` and `end_time` are the longest main sleep when
one exists, and otherwise the service's minimum start and maximum end.
Those bounds are the period `open_wearables_main_sleep` only for
`duration_minutes`, `time_in_bed_minutes`, `efficiency_percent`, the stage
minutes, and `interruptions_count`. `total_duration_minutes`, nap count,
nap duration, the session list, and the heart-rate, HRV, respiration, and
blood-oxygen averages stay on `provider_date` alone. A nap outside the
main sleep does not inherit the main-sleep bounds. The session list stays
on the anchor. It does not become extra sleep sessions. The service can
add a missing main duration as zero when it builds
`total_duration_minutes`. This program stores the returned number and the
original. It does not turn a null into zero, and it does not reverse the
server's sum.

Activity fields are date-only. `floors_climbed` is described as calculated
from elevation. `total_calories_kcal` can be the provider total or the sum
of active and basal energy. The response does not say which. The basis
stays `provider_or_aggregator`. Intensity minutes can be returned as zero
where the service fills a missing key with zero. A returned zero stays
zero.

Body is one current snapshot. `range_complete` does not treat it as the
requested dates. `slow_changing` has weight, height, fat, muscle, BMI, and
age, and no measurement time. Those values use
`input_timestamp.timestamp_missing_reason: not_provided_by_source`. They
have no `recorded_at` and no reporting period. The average window is not
borrowed, and the pull time is not written as the measurement time. Age is
basis `aggregator` because the service calculates it from the birth date
and the server clock. BMI is not called measured. The response does not
say whether that BMI was stored or calculated, so its basis is omitted.
`averaged` uses the server's `period_start` and `period_end`. Those bounds
are the current averaging window, computed from `datetime.now` in the
service. They are not applied to `slow_changing` or `latest`. Each latest
field uses its own time: `body_temperature_measured_at`,
`skin_temperature_measured_at`, or `blood_pressure_measured_at`. A value
with a null time uses the same time-unknown receipt. The three groups can
be stored together or alone. A null group stores no field records. The
anchor still holds the object.

A meal's calories, macros, water, and `nutrients` entries are provider
summaries at the meal timestamp. A nutrient uses the value and unit on
that entry. The name, meal type, and full nutrient map stay on the anchor.
A meal with no numbers is the anchor only. A cycle's reported lengths are
provider summaries on the cycle's start and end. `current_phase` is a
code. `predicted_cycle_length` and `is_predicted_cycle` stay on the
anchor, as do booleans, text, and `pregnancy_snapshot`. They are not
turned into measurements. Workout numbers that are not already typed stay
on the workout as source-reported summaries, and the original fields stay.
`intensity` stays under that name. It is not a sample. Segments stay under
`segments`. A segment with a label is also a typed segment. A missing
label is not filled in. Zones stay under the server's names. A workout
with `entry_source` `manual` is left out.

A provider summary or provider score may omit `value` only together with
its unit. One without the other is refused. Step 5 rebuilds both typed
tables so the pair can be null, copies every existing row, and keeps
record ids, `data_json`, import keys, revisions, tombstones, and reporting
periods. Scale on a provider score stays required. No number is invented.
The time-unknown summary is valid only with the missing-timestamp reason
above. A summary that has a server time uses that time.

The following are not requested. Each line is the reason.

- Resolutions `1min`, `5min`, `15min`, and `1hour`. They are buckets built
  at query time from raw samples. The raw samples are stored.
- Resolutions `1week` and `1month`. They are built from day rows. The day
  rows are stored.
- Sleep totals, workout totals, and activity totals. They add up rows the
  lists return.
- The data summary and the data timeline. They count stored rows.
- The provider workout list and detail. They call the vendor through the
  server's live connection. This pull keeps the stored workout.
- Accounts, connections, sync, OAuth, webhooks, API keys, config, uploads,
  and every `DELETE`. They are not health readings. The API key, the
  server address, and the user id are not written into the athlete record.

Day rows are one selected row per source, series, and local day. The
repository prefers `computed_by: provider`, then the later provider row.
With the sample fallback on, a missing day can be computed while reading.
The returned `timestamp` and `period_end` come from `CalendarPeriod`, not
from the provider interval that was stored. `stats` and `computed_by` are
kept. They are the server's window receipt, not the original provider
period.

## 3. How a response becomes records

The pull resolves `open-wearables` through `resolveMappingFile`. It parses and
checks the file before the first request. A bad file fails with no network
call. A resolved file that does not implement section 2 also fails before
any request. An installed 2.1.0 copy still wins under D179, and this check
stops it from quietly dropping the rows section 2 stores.

File import and pull use the same envelopes. A cursor or offset page passes
its `data` array to the section named in the mapping file. A timeseries
page passes the response object, because `metadata.resolution` is required.
Body passes the object. JSON `null` is not mapped. The personal record
passes the object after `user_id` is removed. Pagination and health-score
`metadata` are not records.

Everything after that is the mapping engine from 2.1.0: match keys (D181),
replace on change (D182), source and device (D183), one transaction per call
(D189), a deleted source left out (D190), and field names refused when they
are empty or `__proto__` (D191). A Garmin file import remains a different
`data_route`, so both copies of one workout stay (D161). A second pull of the
same Open Wearables object, or an `ath import --mapping open-wearables` of a
saved copy of that object, updates the same rows.

On a device source, the writer is `source.provider`. The sensor string is
`source.device_type`, `source.device`, and `source.source`. Section 2's
other sensors are different sources. They do not enter the device check.
Two devices with different triples are
two sources and two baselines. Two devices that share the triple share one
source and one baseline. `source.device_id` is copied to `source_device.hardware`.
It does not split the source, and it is not part of the match key (D181,
D183). Route, writer, and sensor are the source identity. The mapping
`name` is a label. Changing `name` alone does not change the source or the
match key. File import and pull of the same mapped object, in either order,
keep one record id when route, writer, and sensor are the same.

Open Wearables uses `device_id` to tell same-model devices apart when a
provider sends it. The pinned field is optional (`str | None`) in
`backend/app/schemas/utils/metadata.py` at
`183e8cc4f6de897dbd288c93d49a691f2cf721ae`. This study does not treat that
id as stable or unique for every provider. A timeseries sample has no
sample id. Its match key is the writer, the sensor, the kind, the type, and
the UTC time. Two `device_id` values therefore share one key. The mapping
removes `source.device_id` from the point because that path is hardware, so
the point and its revisions do not carry it. `source_devices.hardware` is
one mutable column. A later write can revise it. It is not a binding.
D45 records hardware without splitting a source. D181 and D182 then replace
one key with a different payload. None of those decisions says a second
device's observation may replace the first.

The recommended rule keeps the match key, the sensor, and the baseline.
Putting `device_id` into the sensor list would split sources and baselines,
change keys, and drop the promise that an old triple-key row and a new pull
are one record. The sensor join uses `|`, so a `device_id` that contains
`|` is not a safe extra part unless values containing `|` are refused or
the join is encoded. That identity change is not this proposal. Old rows
are not rewritten, moved, or deleted.

Both `ath import --mapping` and `ath pull` call `apply_mapping`. The check
lives there, for route `open_wearables` only, inside the same transaction
as the write and before `insert_group` changes a device source or a
timeseries record on that source. Summary, event, score, and unattributed
sensors do not enter it. Other routes are not checked. A pull-only check would
leave import-after-pull able to replace the point. The check reads every
accepted row, including a row after the first source device.

The receipt is the source object's own key `device_binding`. The 2.2.0
source schema makes it an optional nonempty string. D185 keeps
`supplied_fields` for names the source sent, so the receipt is a sibling of
that object, as `field_basis` is a sibling on a measured record. The
receipt adds no column and copies no field onto old sources. A source migrated
from 2.1.0 has no `device_binding` key. A string already stored at
`supplied_fields.device_binding` stays in `supplied_fields`. Migration does
not treat it as the receipt. Renaming that supplied name would still leave
an old free-form value able to authorize a write, so this proposal does not
do that.

An established binding is that source key holding a nonempty string. The
automatic path reads only that key. The same name anywhere in a raw payload,
a mapped file, or nested `supplied_fields` is source data. The mapping may
keep it on the measured record. It is not copied onto the source, and it
cannot replace or erase the source key.

The value the automatic path compares is the row's `source.device_id`. The
mapping `name` does not choose it. The shipped file points `device_hardware`
at `source.device_id`. A resolved mapping that points `device_hardware`
somewhere else fails before any write, because that file cannot satisfy this
contract. The source already stored stays unchanged.

The check runs on accepted rows. A row the mapping has already classified
as `left_out` does not establish a binding and does not supply the id. The
call is one of these states:

- The source is deleted. D190 leaves its rows out. The call does not revive
  the source and does not set a binding.
- The source has an established binding, and the incoming id equals it. The
  row may repeat unchanged or correct the stored value under D182. The
  binding stays. Hardware is not replaced with a different id. Other fields
  already on the source stay.
- The source has an established binding, and the incoming id is different,
  missing, null, or blank. The call is refused before any mutation. Equal
  point values with a different id are still refused. A new timestamp does
  not make the new id acceptable. The text says the replacement device is
  blocked, including when the sample time is new, and that running the
  command again cannot change the id.
- The source exists, the source key is absent, and any history exists.
  History includes a deleted record and a deleted source device, not only
  live rows. `same_source_records` keeps `is_deleted = 0`, so it is not
  this test. The store read below includes the deleted rows. The call is
  refused. Hardware is not promoted. The text
  says this Open Wearables history cannot be refreshed until a separate
  reconciliation exists, and that running the command again cannot create
  the binding. This is the block for 2.1.0 history. It is not the coverage
  choice in section 2.
- The source key is present and is not a nonempty string. The call is
  refused. The stored value is left as it is. The text says this command
  cannot use that binding, and a rerun cannot repair it.
- The source is new for this call, or it already exists with the source key
  absent and with no history. Accepted rows that are only sleep and
  workouts, and that all lack an id, are stored. The source has no
  `device_binding`. The result says the device id is unknown. An accepted
  timeseries row with no id is refused. Accepted rows that mix a nonempty
  id with a missing id are refused. Accepted rows that share one nonempty
  `source.device_id` store that string on the source in the same
  transaction as the first accepted write. That source may be created in
  this call, or it may already exist with no receipt and no history. A call
  that accepts no rows creates no binding. A source created or first-bound
  inside a refused call is rolled back with the call.

A second device is refused rather than merged. On `--yes`, an earlier
committed page stays, the failed page makes no database changes, the range
is not complete, and the exit is nonzero. File import rolls back the whole
call. `--json` and `--json --yes` make no database changes. Device reads
that have `filter_by_priority` send `false`. Summary routes have no such
switch. Section 2 states that limit.

Preview keeps two facts in memory for this command, for each source sensor:
the binding it would have stored, and an unbound source it would have
created once that source has accepted history. A summary sensor does not
supply a binding for the device sensor. A later preview page uses those facts, including a
later page that adds an id after an id-less sleep page. The memory is
discarded with the process. It does not change the estimated record counts.
A second command reads the committed source inside its own `BEGIN IMMEDIATE`
transaction.

Three owner operations store a whole source record and do not run this
device-id guard. `ath correct` stores the source `data` the person
supplied, after the schema check. Native package restore copies the
package's records. A package merge the person accepts for a changed source
stores that source record. A 2.2.0 record keeps `device_binding` when the
record has it. A 2.1.0 record does not gain the key, and a supplied field
of the same name stays a supplied field. The next mapped pull or mapped
import then uses the source key that those operations left behind. A 2.2.0
export and restore keeps the key and the import keys, so the same id can
correct a point and a different id is refused. No signature is added. A
person who replaces the database file is outside this rule.

`Store::revise` on a source is a partial update. It applies the other keys
in the patch and keeps the current `device_binding`, including when the key
is absent. A patch that contains `device_binding` is refused. The source
stays unchanged. `revise` cannot set, change, or clear the receipt. Other
record types are unchanged.

The store connection is private. `ensure_mapped_source` takes the writer,
the route, the vendor, and the sensor. An existing source returns before a
data object is built. A new source's JSON is built inside that function,
and that JSON has no receipt argument today. The mapping receives a source
id. It cannot add the receipt to an object it does not hold, and it cannot
read deleted rows through `same_source_records` or through the private
connection. A later `revise` cannot fill the receipt in, because that patch
is refused.

`store.rs` therefore gains three source operations besides the `revise`
rule above, and the typed writes in section 6. It gains nothing beyond those:

- Read the committed receipt, and whether any record or source device for
  that source exists, including a deleted row.
- Create a source with the checked receipt already in its JSON.
- Write the checked receipt on an existing source only when the receipt is
  absent and that same read shows no accepted or deleted record and no
  source device. Every other source field stays. The write uses the normal
  revision and `data_json` bookkeeping. Writer, route, sensor, import keys,
  and baselines stay.

The three operations run inside the caller's `BEGIN IMMEDIATE` transaction.
A refused call rolls them back with the call. Other routes do not call
them. The build may extend `ensure_mapped_source` or add a narrow operation
beside it. It does not add a table, a trait, or a file split. The automatic
path does not replace `detail`, `vendor`, or `supplied_fields`.

`zone_offset` is not copied into `start_at`. It remains in `supplied_fields`
when the server sends it. The timestamp text is the server's `start_time`,
`end_time`, or `timestamp`. A timestamp with no offset is left out by the
mapping. The pull does not add `Z`.

`computed_by`, `window_seconds`, `is_nap`, `intensity`, and `efficiency_percent`
stay under those names when they are present. `computed_by` sets the basis
in section 2. It does not change the writer. Raw samples on 0.11.0 are
built without that field. Day rows carry it, and those rows are stored.

## 4. Credentials, privacy, and the network

D200 is the rule. The key never enters the athlete database, the export, the
preview, or an error line. The pull reads only the paths in section 2.
It does not list users. The personal-record object includes `user_id`.
That field is removed before the scan below and is not stored. Birth date,
gender, and sex are stored. They are account health data, not the API key.

The process refuses a base URL that could carry the key in the URL itself.
It refuses plain `http` except to loopback. It does not follow a redirect, so
the key cannot be sent to a second host. One attempt, then stop. Timeouts are
10 seconds to connect and 30 seconds to read. One response may be at most
8 MiB. One read may be at most 1000 pages.

The body is read inside that cap and parsed before any `apply_mapping` call.
The guard is in `open_wearables.rs`. It walks every decoded object key and
every decoded string. A value that contains the API key, the selected
origin, or the selected user id rejects the whole page. The removed
personal-record `user_id` is the only field that is dropped before this
walk. Any other copy of the user id still rejects the page. The page is not
rewritten and it is not mapped. A Unicode escape such as `\u0073` is visible
only after parsing, so a scan of the raw text is not enough. The raw-text
scan still runs on a body that did not parse, and the parent still refuses
to print the key. Those checks are defense in depth. They do not repair a
page that has already committed.

Printed failure text is a fixed sentence plus the HTTP status. A server
problem `code` is printed only when it is a single token of letters, digits,
`_`, or `-`, at most 64 characters, and it does not contain the key, the
origin, or the user id. Any other code is withheld. A transport error or a
parser error is replaced with a fixed sentence. The sentence has no URL, no
path, and no user id. The result keeps a `left_out` total and at most 20
example lines. It does not keep every id or every source string. The child
output has to stay inside the 64 MiB buffer in `src/ath-core.ts`.

There is no Athletic Standard telemetry on this command. The server the person
named is the only host contacted.

A deployment API key can read every user on that server. The command cannot
shrink that key. It can avoid using the extra power: one user id, the
reads in section 2, no user list.

## 5. Version, migration, and what stays compatible

The build sets `2.2.0` on the package, the record, and `ath-core`, by the
same constants 2.1.0 moved. Migration step 5 is a new SQL file. It updates
`athleticstandard_version` to `2.2.0` and `sql_schema_version` to 5. No column
is added. The same file rebuilds `provider_summaries` and `provider_scores`
as section 2 describes and copies existing rows, including deleted rows.
Record ids, revisions, import keys, and reporting-period rows stay. Step 4
and file `004` are not edited (D165).

A new database runs that step 5 SQL after step 4 and before the `user_version`
pragma, inside `installInitialSchema`. `applyMigrationStep` gains an arm for
target 5. `MIGRATION_CHAIN` in `src/db/sql-contract.ts`,
`scripts/generate-sql-contract.ts`, and `scripts/check-schema-drift.ts` gain
the step. `package.json` exports and `tests/v1-package-exports.test.ts` point
at `schema/2.2.0/`.

`READABLE_DATABASE_VERSIONS` and `ACCEPTED_RECORD_VERSIONS` become `1.0.0`,
`2.0.0`, `2.1.0`, and `2.2.0`. Both sets already contain `1.0.0`. Dropping it
would make a 1.0.0 database unloadable, so migrate could not open it.
`IMPORTABLE_PACKAGE_VERSIONS` is only `2.0.0` and `2.1.0` today. It gains
`2.2.0` and keeps those two. `Store::open` rejects a `2.1.0` database until
`ath migrate`, with the refusal it already uses for an older database. A
`2.1.0` database will not open in `ath-core` until that migrate.

The build generates `schema/2.2.0/` and does not rewrite `schema/2.1.0/` or
`schema/2.0.0/`. The 2.2.0 source object gains optional `device_binding`.
`ProviderReportingPeriodType` gains `open_wearables_reporting_window` and
`open_wearables_main_sleep`. `ValueBasis` gains `aggregator`. Series
quantities gain the sample series in section 2, not the intake, index, age,
or effort series. `provider_summary.value` and `provider_score.value` may
be omitted together with the unit, as section 2 says. A provider summary
may also omit `recorded_at` and reporting periods when its input timestamp
uses `not_provided_by_source`. No other field is added. The folder exists
so the package version and the schema path
stay aligned. The demo athlete is
regenerated because the stored version is inside it. `examples/demo-athlete/athlete.ath.json`
stays the legacy 0.4.3 file. `tests/fixtures/version-2-0-0/` is not
regenerated.

`package.json` `files` already includes `mappings`. The build rewrites
`mappings/open-wearables.md` so it implements section 2. Other mapping
files stay as they are. `pnpm generate:skill-cli` runs because help text
changes (D80). `AGENTS.md`, `build-history/README.md`, and `SPEC.md` are
updated in the build so they name 2.2.0 as the software. `docs/connections.md`
keeps the file imports and is updated to the coverage in section 2, including
that `RecoverySummary` at the pinned commit has no skin-temperature field.
This
plan's edits to `AGENTS.md` and `build-history/README.md` only point at the
plan.

A database from 2.1.0 needs `ath migrate` before the pull. The migrate
command still previews, backs up, and asks (D124). `Store::open` only
refuses. A noninteractive open and an open with `--json` do not migrate. An
interactive open can preview, back up, ask, and then migrate.

## 6. Where the code goes

| Piece | Place |
|---|---|
| HTTP, page loop, envelope split, call `apply_mapping` | `crates/ath-core/src/open_wearables.rs` |
| Command `pull-openwearables` | `crates/ath-core/src/main.rs` |
| Flags, key, preview text, refusal if the key appears | `src/program.ts`, `src/help.ts`, `src/keyring.ts` |
| Mapping names and field rules | the existing file `mappings/open-wearables.md` |
| Stored records | the existing store and mapping engine |
| Keep `device_binding` on a partial source update | `Store::revise` in `crates/ath-core/src/store.rs`, sources only |
| Read receipt and tombstone history; create or first-bind with the checked receipt | three source operations in `store.rs`, inside the caller's transaction |
| Measurement samples; paired null value and unit on summaries and scores | `insert_typed`, `update_typed`, and `typed_table` in `store.rs`; the same pair in `src/db/records.ts`, `src/db/import-package.ts`, `src/db/records-view.ts`, and the repair copy from `data_json` |
| Whole source record from the owner | existing `ath correct`, package restore, and an accepted package merge |

`store.rs`, `persist.rs`, and `personal.rs` are not the home for this client.
`personal.rs` remains the Garmin pull. The two clients may each call `ureq`.
A shared trait is the later connector work, and this release does not add it.

`mapping.rs` changes for the counts, the device check, and the preservation
writes in section 2. `apply_mapping` keeps its `outcome` field and adds
counts by record type.
For route `open_wearables` only, it also runs the device-binding check in
section 3 before it mutates a source device or a timeseries record. That
check is the shared mapped-import and pull enforcement. It is not a new
match key and it does not apply to any other route. It also gains the
measurement-sample target, the reporting-window write, and the segment
copy from section 2. HTTP stays out of `mapping.rs`. `store.rs` keeps the
`revise` refusal and adds the three source operations in section 3: the
receipt and tombstone read, creation with the checked receipt, and a
first bind on an existing source that has no history. It also inserts and
updates `measurement_sample`, and it accepts a provider summary or provider
score whose `value` and unit are both present or both absent. A half-null
pair is refused. `src/db/records.ts`, `src/db/import-package.ts`,
`src/db/records-view.ts`, and the repair copy from `data_json` follow that
pair. No other store behavior changes. `ath correct`,
package restore, and an accepted package merge stay whole-record owner
writes. They are named in section 3 so the binding survives a 2.2.0 round
trip. They are not a new importer. The pull sums only `sleep_session`, `workout_session`,
`point_measurement`, `measurement_sample`, `provider_summary`, and
`provider_score`. A committed page adds those counts to the actual committed
result. A rolled-back preview page adds them to `estimated`. It does not add
`outcome` across pages. `outcome` counts the source-device row in the same
group. Filtering that row out of the totals does not make a preview equal a
write. Each preview page is rolled back, so page two does not see page one.

The Rust command reads the key from the environment only. It has no `--key`
flag. The decoded-key walk in section 4 runs before `apply_mapping`. The
parent still checks stdout and stderr before printing.

The child returns the preview outcome from section 1 when the mode is a
preview, including public `--json`. It returns the confirmed outcome when
the mode is `--yes` without `--json`. On a partial failure it returns that
confirmed outcome to the parent and then exits nonzero. The parent renders
text from the confirmed outcome and public JSON from the preview outcome.
The success sentence is parent text, and only after `--yes` when
`range_complete` is true.

The program runs on Linux, macOS, and Windows with the same command. The
fixture server listens on `127.0.0.1` and an ephemeral port, which is the
pattern `personal.rs` already tests. Paths stay the TypeScript resolver's
job. No new installer and no new system library.

## 7. Tests

CI has no Open Wearables account, no API key, and no Docker Compose. The
acceptance tests are the fixture server and the synthetic files already in
`tests/fixtures/exports/open-wearables/`. Those files are on the public-data
list. A new file, if one is required for a two-page response, is synthetic,
invented, and added to `scripts/check-public-data.ts` with its hash. No real
export is copied.

Before the first request, `--from` and `--to` are both dates or both
timestamps with an offset, `--from` is not after `--to`, and a calendar date
is a real date. A number is refused here even though Open Wearables accepts
Unix seconds. A bad bound does not open a socket.

Rust tests, run by the existing `cargo test` job on Linux, macOS, and Windows:

- two pages, the second `next_cursor`, then `has_more: false`. `--yes` sets
  `range_complete` in the text and prints the success sentence
- the same session id on two pages, identical payload: the preview estimate
  counts two new rows, and `--yes` counts one new and one unchanged
- the same session id on two pages, changed payload: the preview estimate
  counts two new rows, and `--yes` counts one new and one changed
- a valid page, then a failing page, under `--yes`. The failure is one of a
  401, a 429, a repeated or null cursor, an invalid row, or a minimum page
  that is still over 8 MiB. Page 1 remains. Page 2 adds no rows and no revisions. No later read
  runs. The exit is nonzero. The success sentence is absent. The text gives
  the retained counts. The same command run again stores no second copy
- the same failure sequence with no `--yes`, and again with `--json --yes`,
  makes no database changes. Rows that were already stored stay. A
  successful `--json --yes` of a readable range is also a preview and makes
  no database changes
- an installed mapping that does not implement section 2 fails before any
  request. The server sees no request. Raw and day timeseries requests omit
  `types`. A comma-joined type string is not sent
- a cursor or offset page fails before `apply_mapping` when the object is
  missing, `data` is not an array, `pagination` is missing, or `has_more`
  is not a boolean. That failure is not an empty success. A body response
  of JSON `null` is a completed empty snapshot and writes nothing. A body
  object is mapped. A health-score page ends on `has_more: false`, and a
  smaller retry uses the same offset
- `filter_by_priority=false` is present on the reads that have the flag.
  Raw timeseries sends `resolution=raw`. Day timeseries sends
  `resolution=1day`. Sleep sends `include=stages`. Workouts send
  `include=zones` and `include=segments`. A fixture with stages omitted has
  null intervals and still has stage totals. A fixture with `include=stages`
  keeps the intervals under the server's names. A fixture with
  `include=zones` keeps the nested heart-rate and power zones under the
  server's names. A fixture with segments keeps the original objects, and a
  segment that has a label is also a typed segment
- a raw `heart_rate` row becomes a measurement sample in bpm at the
  server's timestamp. Export and import keep that timestamp, unit, and
  value. A day row for resting heart rate, RMSSD, SDNN, and blood oxygen
  becomes a provider summary with `open_wearables_reporting_window`, no
  `recorded_at`, and the server's `timestamp` and `period_end`. Raw
  `vo2_max` stays `provider_summary.vo2_max` on its 2.1.0 key. A day
  `vo2_max` row is a different window summary. Raw `garmin_stress_level`
  and `garmin_body_battery` stay provider scores on their existing scales
  and keys. They do not become samples. A null RMSSD beside a present SDNN
  stores only the SDNN. A sleep-summary respiratory rate is date-only. The
  sleep session from the same night still has no respiration field. Nap
  totals do not receive the main-sleep bounds. Skin temperature is stored
  from a series row or from body latest when that time is present. A
  recovery object does not gain a skin field. A WHOOP health-score
  component named `skin_temp_celsius` stays on that object.
  `page-5min.json` is left out as a query bucket. A 2.1.0 raw point and a
  pull of that same raw point stay one record. A heart-rate sample from
  device B does not replace device A's sample. A negative latitude and a
  negative longitude are stored
- a response larger than 8 MiB on a cursor or offset read is requested
  again with a smaller limit, the same cursor or offset, and the rows are
  stored. A day page at `limit` 1 that holds many small rows and is still
  over 8 MiB leaves that read incomplete and names the read and
  `resolution=1day`. It does not name a series type. An oversized body
  response names `body` and writes nothing. The export after a pull, and a
  native package import, keep period bounds, units, `computed_by`,
  segments, score components, meal nutrients, and a pregnancy snapshot
- a summary shaped like `summaries_service.py`, with no `device_id`, is
  stored after a bound raw page. The device receipt and the device's
  points stay. The same summary stored first does not block a later raw
  page from binding its device source. File import then pull, pull then
  file import, and import then import keep one summary id. A refused
  device page rolls back only that page. Two meals with one timestamp and
  different ids stay two records. Two summary fields on one date stay two
  records. A timeseries row with a source and no `device_id` is `left_out`.
  A timeseries row with `source: null` is stored on the unattributed sensor
- an internal health score has basis `aggregator`. A vendor score has
  basis `provider`. A null `provider` omits the basis. Polar readiness uses
  scale `0-10`, Polar recovery uses `1-6`, and WHOOP strain uses `0-21`.
  A score with no finite range keeps its number on the anchor and does not
  use scale `0-100`. A null value and a later number revise the same
  record. `computed_by: null` omits the basis and keeps the null
- body fetched during a historical range is the current object. Weight
  with no time has no `recorded_at` and does not use the average window.
  A latest value with a null time is stored the same way. Age has basis
  `aggregator`. BMI's basis is omitted. The three groups can arrive
  together or alone. JSON null and a returned zero and a missing field
  stay three different results
- a dietary series is a provider summary, not a sample and not a nutrition
  entry. A cycle code, a predicted length, and a pregnancy snapshot stay
  on the anchor. A cycle that starts in range and ends later keeps that
  end. A cycle that starts later is not stored, and the read can still
  complete. Birth date, gender, and sex are stored. `user_id` is not.
  The athlete profile is unchanged
- step 5 keeps an old summary's id, number, unit, `data_json`, import key,
  revisions, and tombstone. A numeric summary can be corrected to
  numberless and back. A half-null value and unit is refused. A sample
  repeats and corrects. `ath check`, export, native import, and restore
  accept the numberless summary and the sample
- two distinct sensor triples become two sources. Device A is stored, then
  a separate operation contains only device B at the same triple, type, and
  instant, with a different value. The operation fails. The current point,
  its revisions, the source, and hardware are unchanged, and the range is
  not complete. The pairs are pull then pull, pull then pull in reverse,
  file import then pull, and pull then file import. The same collision
  inside one page, and split across two pages, refuses the failed page
  instead of keeping one device and dropping the other. On `--yes`, an
  earlier committed page stays. Preview and `--json --yes` make no database
  changes. Equal values with different ids still fail. B at a different
  timestamp fails before hardware changes, and a later B row does not
  replace A's older point
- the same `device_id` with the same payload is unchanged. The same
  `device_id` with a changed value revises that record and does not change
  the binding. Import then pull, and pull then import, of that one device
  keep one record id. Mapped file import of device A, then a second mapped
  file import of only device B, fails the same way. A missing, null, or
  blank id does not replace a bound source. A 2.1.0 source that already has
  history and has no source key `device_binding` is refused, including when
  the only history is a deleted record or a deleted source device, and
  including when `supplied_fields.device_binding` is already the string for
  device B. The same misleading field in a 2.1.0 package is still a supplied
  field after import. Device B does not replace A's point. The supplied
  field and the hardware stay. No source key is added. A sample keeps basis
  `provider_or_aggregator`. A provider score keeps basis `provider`
- a raw page and a mapped file that carry `device_binding` on the row, on a
  nested source object, and inside `supplied_fields` do not set, replace, or
  erase the source key. A mapping whose `name` changes and whose
  `device_hardware` stays `source.device_id` keeps the same binding. A
  mapping whose `device_hardware` points elsewhere is refused before any
  write
- an existing open-wearables source has no receipt, no record, and no
  source device. The first accepted write stores the checked receipt on
  that source. Its other fields, its writer, its route, its sensor, and
  its baselines stay. The revision and `data_json` record that receipt. A
  source whose only history is a deleted record or a deleted source device
  is still refused, and that refusal writes no receipt
- a new source of only id-less sleep is stored with no source key. A later
  timeseries row with an id is refused, and the sleep row stays. One call
  that accepts both an id and a missing id is refused. A `left_out` row does
  not create a binding. The first preview page is id-less sleep and the
  second preview page has an id for that same source: the preview refuses
  and writes nothing
- a partial source update changes `detail` and leaves `device_binding`
  unchanged. A partial update that includes `device_binding` changes
  nothing. `ath correct` with a complete source object replaces that object.
  After that correction, a mapped write follows the source key the
  correction stored
- export a 2.2.0 database that has an established binding, import that
  package, and the source key and the import keys are still there. The same
  id can correct the point. A different id is refused, with no new point
  revision, source revision, or hardware revision. A stored source key that
  is not a nonempty string refuses the automatic write and is left unchanged
- `page-sleep.json` becomes a sleep session. A manual workout is left out
  and its automatic neighbor is kept. `page-raw.json` is stored.
  `page-5min.json` is left out. RMSSD and SDNN stay different quantities
- a changed end time keeps the record id and increases the revision
- a `__proto__` supplied field fails the page
- a literal key, a Unicode-escaped key in a nested unconsumed field, the
  key used as an object key, the selected origin, and the selected user id
  each reject the page before mapping. The database, its revisions, and the
  export do not contain them. The error text does not contain them. A page
  that committed earlier remains. A problem `code` or a transport error that
  contains those values is withheld
- a redirect is not followed. `http` to a non-loopback host is refused. A
  base URL with a user name, a query, or a path is refused. A user id that
  is not a UUID is refused before a socket opens
- the 1000-page cap is tested with a small limit passed into the loop
- a timestamp with no offset is left out. The stored fixture's timestamps
  include `Z`. The test does not add `Z`
- a deleted mapped source is left out. A Garmin file import of a different
  workout remains a second source
- `left_out` in the result is a total and at most 20 examples

Migration tests, in the build gate:

- a synthetic 2.1.0 database makes `Store::open` refuse before any network
  call. `ath migrate` previews without writing. It still previews, backs up,
  and asks, as D124 requires. The target it names is `2.2.0`
- a confirmed migrate backs up, then sets the record version to `2.2.0`,
  SQL schema 5, and `user_version` 5. Record ids, revisions, deletions, and
  attachments are unchanged. A second migrate does nothing
- `ath init` creates a database that is already `2.2.0`, SQL schema 5, and
  `user_version` 5, and `ath check` exits 0. Step 4 still writes `2.1.0` and
  schema 4, and the create path runs step 5 after that
- databases at `1.0.0` and `2.0.0` still open for migrate. Packages at
  `2.0.0` and `2.1.0` still import. `spec/sql/migrations/004-version-2-1.sql`
  and the folders `schema/2.0.0/` and `schema/2.1.0/` stay byte-identical

TypeScript tests use the memory store and do not open the real password
store. They cover:

- the missing-key sentence
- `ath key set openwearables`
- `ath key clear` with no name, when an Open Wearables key is saved, clears
  `vercel` and `openrouter` only, exits successfully, and prints that the
  Open Wearables key remains saved, with the command `ath key clear
  openwearables`
- `ath key clear` with no name, when no Open Wearables key is saved, keeps
  today's gateway sentences
- `ath key clear vercel` does not remove a saved Open Wearables key
- `ath key clear openwearables` removes the saved key and, when
  `OPEN_WEARABLES_API_KEY` is set, says that variable is still available. The
  variable is still set afterward
- `ath key --json` keeps `saved` and `environment` as gateway lists and adds
  `openwearables.saved` and `openwearables.environment`
- a preview `--json` has `written: false`, committed zeros, `estimated`
  counts, and `range_complete: false`
- `--json --yes` is a preview. After a later-page failure, and after a
  readable range, the database is unchanged

`pnpm check:public-data`, `pnpm typecheck`, `pnpm test`, and `pnpm build`
pass, and `cargo test` passes, before the build is called done.

## 8. You test this (Laptop)

The card uses the fixture server the tests use. It does not use a real
account or a real export. The build starts that server on `127.0.0.1` and an
ephemeral port, prints the port, and uses the fixture key and the fixture
user id. The range is `--from 2026-01-01 --to 2026-01-07`. The person pastes
the fixture key once into `ath key set openwearables`.

1. Setup. Success: `ath init --yes --empty` creates the database, the key
   command saves the fixture key, and the key is not on screen. Failure:
   init fails, or the key is printed.
2. Preview. Success: the pull without `--yes` says nothing is stored, shows
   estimated counts that include a heart-rate sample and a day summary, shows
   a body snapshot line, and `ath check` still passes. Failure: a row is
   written, or the key appears.
3. Write. Success: the same command with `--yes` says the requested range is
   complete, shows the body snapshot line, shows committed counts, and
   `ath check` exits 0. Failure: the success sentence appears while the
   range is incomplete, or `ath check` fails.
4. Repeat. Success: `--yes` again reports the stored rows unchanged.
   Failure: a second copy of a sleep session or a point appears.
5. Compatibility. Success: one synthetic Apple, WHOOP, or Oura import still
   passes `ath check`, and `ath pull garmin` with no date still asks for
   `--date`. Failure: that import or the Garmin command breaks.

The automated cases in section 7 are not extra card steps.

The build's proof is one phone message. That message contains a
screenshot of each step inline and one video that plays in the conversation.
A video that plays only on the pull request page is not that packet. The
images are not committed. The numbers on screen are the fixture's invented
values. The release workflow on Linux, macOS, and Windows is the
cross-platform proof. The video is one system.

This plan pull request has no screenshots and no video.

## 9. Separate follow-ups

D202 records the check. None of these is a prerequisite for the pull, and none
is in the build pull request.

- Failed `ath init` and failed `createDatabase` still delete the database file
  in the same process that closed it, without the garbage collection
  `removeTemporaryDirectory` uses. On Windows that delete can fail with
  `EBUSY`. Version 2.1.0 spec section 12 left it out and named no later
  version.
- `store.rs` and `mapping.rs` are past a thousand lines. The pull does not
  add HTTP to them. `mapping.rs` gains the counts, the open-wearables
  binding check, and the preservation writes in section 2. `store.rs` gains the
  partial-update refusal, the three source operations in section 3, and the
  sample and paired-null writes in section 6. Splitting the files stays out.
- `persist.rs` can still store a supplied field named `__proto__` if a caller
  uses `ath-core persist` directly. The pull does not use that path.
- The mapping path rule is already one function. There is no duplicate to
  delete.

No maintainability note under another name was found in the 2.1.0 decision
log. The sizes and the persist gap are the current code, not a recorded
D-number.

## 10. Risks and the approval

Cloud-agent time for the build is uncertain. The published history has no
measured run for this pull.

Risks:

- A server newer than 0.11.0 can add a default that hides a second device.
  Current `main` does not, when the queries in D201 are sent. The client is
  tested against the stored fixture. That fixture is not a live response, and
  a live timestamp without an offset is left out. A breaking envelope needs a
  new fixture before the pull trusts it.
- A raw range can be large. A page over 8 MiB is read again with a smaller
  limit. A 429 or another transport failure still stops that read, keeps
  earlier pages, and withholds the success line. The study did not read the
  rate-limit middleware.
- Date-only bounds mean whatever Open Wearables decides. The docs and the
  0.11.0 parser disagree about UTC. This release does not correct that server.
- Section 2 stores returned periods, series, and snapshots. It does not
  invent a measurement time. A null stays null. A row the server did not
  return stays unknown. The device receipt is a separate rule.
- Fields on a sleep or workout object that the mapping does not consume are
  stored under the server's names. Latitude and longitude are stored when
  the server sends them.
- The administrator key is more powerful than this command. A leaked key from
  the password store exposes every user on that deployment. The command's job
  is to avoid copying the key into the record.

The build, after approval, is one pull request in this order. First the
schema and step 5: the period types, the `aggregator` basis, the sample
series quantities, the paired null value and unit, and the time-unknown
summary. Then the store and record paths in section 6. Then the mapping
engine and `mappings/open-wearables.md`. Then the reads, the cycle filter,
the smaller-page split, and the tests in section 7. The laptop card and the phone proof are
part of that same pull request. This plan pull request has no media.

The maintainer approves or rejects this plan before any build. D198 through
D204 are the proposal. They are not adopted. Reviews of earlier heads were
incorporated here. None of those reviews approves this revision. The next
head needs re-review. An earlier review accepted a
migration with no new column, under D152 and D184. That acceptance does not
adopt D198. The
optional source key adds no column.

1. One number, 2.2.0. Readable sets keep `1.0.0`. Step 5 runs on create and
   on migrate. The device receipt adds no column. Step 5 does rebuild
   `provider_summaries` and `provider_scores` so a numberless object can omit
   `value` and its unit together. Existing ids and rows are copied. A
   `2.1.0` database does not open until `ath migrate`. This release does
   not schedule, refresh a token, backfill, or store a cursor (D198, D204).
2. Store the returned representations in section 2. Device receipts stay
   on device sources. Summaries without a device id use their own sensors.
   Null stays null. A missing response is not called provider-unavailable.
   The omissions in section 2 are the ones with a reason (D203, D204).
3. The key, the selected origin, and the selected user id are rejected in
   decoded JSON before a page is mapped. Failure text uses a fixed sentence.
   `ath key clear` with no name still clears only the gateway keys and says
   when the Open Wearables key remains. Named clear removes the saved key
   only. It does not unset the environment and it does not revoke the server
   key (D200).
4. Preview is the default and its counts are estimates. `--json` does not
   write. `--yes` commits each successful page under D189, and a committed
   page is not a completed pull. The text of `--yes` shows retained counts
   and whether the requested range is complete. A missing row is not deleted.
   Raw and day reads omit `types` and store every series. The same sensor
   triple keeps one
   source. A second `device_id` does not replace or suppress the first, in
   a later command or in a mapped file import, including import then import.
   The receipt is the source key `device_binding`. A 2.1.0 supplied field of
   that name does not become the receipt. Unbound history is refused and
   left in place. `ath correct` and a native 2.2.0 package restore are the
   owner's whole source record (D201).
5. The Windows cleanup and the maintainability notes stay separate.
   `mapping.rs` gains counts by record type, the open-wearables binding
   check, and the preservation writes in section 2. `store.rs` gains the
   partial-update refusal, the receipt and tombstone read, creation with
   the checked receipt, and the first bind of an existing source that has
   no history, and the sample and paired-null writes in section 6.
   Nothing else is added to either file (D202, D204).
