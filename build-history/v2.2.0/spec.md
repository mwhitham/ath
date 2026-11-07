# Athletic Standard 2.2.0

Status: **plan only. The maintainer has not approved it. Not approved for build.**

`ath pull openwearables` reads one person's stored health data from an Open
Wearables server they run, and writes it with the mapping this release
ships. The proposals are D198 through D208 in
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
- for body, on a preview: `body snapshot would store` or `body snapshot
  would be empty`. On a confirmed write: `body snapshot stored`,
  `body snapshot empty`, or `body snapshot incomplete`
- estimated counts, by record type, of sleep sessions, workouts, points,
  measurement samples, summaries, scores, and source reports
- committed counts of zero on a preview
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
then uses the exit status. When a ranged read failed, the text says the
requested range is incomplete. When every ranged read completed and body
or the personal record failed, the text says the requested range is
complete and names the snapshot that failed. It names any read that is
`not_attempted`, and it prints the retained counts. The exit is nonzero
in both cases. The success sentence is not printed. A failed read says to
run the same command again. An identity refusal uses the sentence in
section 3, which says a rerun cannot
repair the device id. It does not print the success sentence. The
protected transaction of a refused page makes no database changes.
Records already stored stay, including their revisions. An archive, when
section 2 allows one, is a later pull-only transaction. It does not print
the success sentence. `written` is true when any protected transaction
in this command created or revised a counted record. An archive does not
make `written` true. `database_changed` is true when any protected
transaction or any archive transaction in this command committed a
change, including the first insert of the archive source. A later page
does not reset either flag or the counts from an earlier page. An empty
ranged result can still store the personal record or the body snapshot.
Those writes are counted as protected rows. They are not ranged rows.
`written` is false and `database_changed` is false when the confirmed
command committed no protected change and no archive change. Rows stored by an earlier
command can still be there.

Public `--json`, with or without `--yes`, is only the preview outcome below.
`written` is false. `database_changed` is false. `committed` and
`archive_committed` are zero. `estimated` holds the protected
page-local estimates. `estimated_archive` holds the archive estimates.
`range_complete` is false. `reads_finished` is true only when
every read completed. The read status token is
`not_attempted`, in this object and in the text. The object has no key,
server, user id, or request path.

```json
{
  "from": "2026-01-01",
  "to": "2026-01-07",
  "written": false,
  "database_changed": false,
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
  "estimated_archive": {
    "source_report": { "new": 0, "changed": 0, "unchanged": 0 }
  },
  "archive_committed": {
    "source_report": { "new": 0, "changed": 0, "unchanged": 0 }
  },
  "left_out": { "total": 0, "examples": [] }
}
```

The same record types are present for workouts, points, summaries, scores,
and source reports. The example above is shortened. It is the preview
outcome. A preview does not say a snapshot was stored.

The child returns a different outcome for `--yes` without `--json`. That
outcome has the actual `committed` counts, the actual `archive_committed`
counts, `database_changed`, `protected_write`, `archive`,
`range_complete`, and `snapshots_complete`. `committed` counts protected
records only. `archive_committed` counts archive records only, including
a created archive source, counted once. A record is in one of those
maps. `new`, `changed`, and `unchanged` are the created, revised, and
unchanged counts in each map. `range_complete` is true when the ranged reads
finished and no protected page in those reads was refused. `snapshots_complete`
is true when the personal record and the body snapshot finished. `written`
is true when any protected transaction in this command created or revised
a counted record. A command that only repeats unchanged protected pages,
and that commits no archive change, has `written` false,
`database_changed` false, both completion flags true, and unchanged
protected counts. An earlier change keeps both flags true. A failed ranged page has `range_complete` false. A
failed snapshot after a finished range has `range_complete` true and
`snapshots_complete` false. The parent prints sentences from this outcome.
It does not print the outcome as public JSON. Public `--json`, with or
without `--yes`, commits neither the protected transaction nor the
archive transaction.

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

D208 is the storage rule. D203 keeps these values in this pull. D204
corrected source, time, and score direction. D205 corrects units, day
windows, sleep bounds, identity encoding, reported records, and child
withdrawal. D206 corrects the root record type, numberless calculation
inputs, recovery scales, required ids, workout provenance, hydration,
and the archive of a refused original. D207 corrects the migration
dependencies, the command counts, the archive capsule, and null-source
report routing. D208 corrects when the archive reads the database, and
the failed-init sentence. An earlier draft
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

A device source uses the 2.1.0 sensor, `device_type|device|source`. Its
receipt rules in section 3 are unchanged. A sleep session or a workout
with `entry_source` `automatic` enters that source when it has a nonempty
`device_id`, or when it is id-less on a new source with no history. A raw
or day row enters that check only when the appendix class is a point, a
measurement sample, `vo2_max`, or one of the two Garmin scores. A missing,
null, or blank `device_id` on that row refuses the protected transaction
before any measured write. It is not classified `left_out` first. A valid
row beside it on the same page is not stored as a measurement. No id is
copied from another row. Intake, index, age, effort, and `hydration` do
not enter that check. A device id on those rows is original data. It is
not a receipt and it is not evidence the value was measured.

`entry_source` `automatic` is the only workout class that becomes a
workout session. `manual` is a source report of kind `manual_workout`.
`unknown`, JSON null, and a missing `entry_source` are source reports of
kind `unknown_workout`. The original keeps null and absence distinct.
Unknown is not labeled manual, and it is not labeled automatic because a
model or a device id is present. A repeated server workout id that arrives
as a different kind or sensor refuses the protected page. The stored
record stays. The new interpretation is not written beside it. The text
says the classification changed and a rerun cannot choose. `ath correct`
remains the owner's way to replace the record.

Sleep, workout, meal, cycle, and health-score objects need a `id` that is
a UUID string. Missing, null, blank, and any other shape refuse the
protected page before a source, a row, or hardware changes. The row is not
`left_out`. No empty or time-based id is invented. A valid neighbor on
that page is not stored. A later page of the same command still rolls
back only the failed page, and earlier committed pages stay.

A new sensor is the prefix `ow1:` plus a JSON array. The array has no
spaces. A missing part is JSON `null`. An empty string and the text `-`
stay strings. JSON escaping keeps a colon, a quote, and a backslash
inside one part. Two different tuples therefore stay different sensors.
The legacy device sensor is not rewritten into this form. D181 still
escapes `:` and `\` when the sensor is placed in a match key.

The array shapes are:

- Summary or body: `["summary", family, provider, device, device_type, source, device_id]`
- Meal, cycle, manual workout, or unknown workout: `["event", family, provider, device, device_type, source, device_id]`
- Reported series, including a row whose `source` is null: `["report", resolution, provider, device, device_type, source, device_id]`. `resolution` is `raw` or `1day`.
- Health score: `["score", provider]`. `provider` is the top-level author,
  or JSON `null` when that field is null. The input `source` is not in
  the array.
- Measured series with `source: null`: `["unattributed", "timeseries"]`. A reported series does not use this sensor.
- Archive originals: `["archive"]`
- Personal record: `["personal_record"]`

The writer is `source.provider` when that string is nonempty. A health
score uses its top-level `provider` when that string is nonempty. The
personal record, a null-provider score, a measured series with
`source: null`, and a reported series whose provider is missing use the
writer `open_wearables`. That word names this route. It is not a device
maker and it is not an invented author. The vendor is omitted.
`source_type` is `connector`. These sensors do not establish
`device_binding`. Their history is not history of a device source.

A health score never uses the unattributed timeseries sensor. A change
of its input `source` from null to an object, or the reverse, revises
the same score. The writer, the sensor, and the score id stay put.

A null-source timeseries row has no sample id. A measured row uses the
unattributed sensor. A reported row uses the report sensor and does not
also get an unattributed record. The comparison is that one full match
key: writer, sensor, resource, resolution, series type, UTC timestamp,
and, for a day row, `period_end`. A raw row at midnight and a day window
that starts at that midnight are different keys. Two rows with one key
are one normalized record only when the whole returned row is equal. A
different value, in the same page or a later confirmed call, refuses the
protected transaction. The stored normalized row stays. The text says the
rows are ambiguous and a rerun cannot choose the normalized row. `ath
correct` remains the owner's way to replace that row. Preview
pages roll back, and the command memory holds device-binding facts only.
A preview therefore does not claim to see a null-source conflict from an
earlier preview page. A confirmed `--yes` page does see an earlier
committed page.

File import and pull use one envelope table. The pull passes these
objects. A saved file imports the same object. A mapping that sends a
device row to another sensor, or that changes a series class in the
appendix table, fails before any request.

| Resource | Envelope | Writer | Sensor | Anchor | Child |
|---|---|---|---|---|---|
| Sleep session | `{ "sleep": data }` | `source.provider` | legacy device | `sleep_session`, D181 id key | stage and duration fields on that session |
| Automatic workout | `{ "workout": data }` | `source.provider` | legacy device | `workout_session`, D181 id key | existing typed fields; untyped numbers are source reports |
| Manual workout | `{ "workout": data }` | `source.provider` | `ow1` event | `source_report` `manual_workout`, server id | none. The original object is the record |
| Unknown, null, or omitted `entry_source` | `{ "workout": data }` | `source.provider` | `ow1` event | `source_report` `unknown_workout`, server id | none. The original object is the record |
| Measured raw series with `device_id` | timeseries object, `resolution` `raw` | `source.provider` | legacy device | the raw class in the appendix, D181 time key | none |
| Measured raw or day series, source present, no `device_id` | timeseries object | `source.provider` | legacy device, then the protected transaction is refused | no measured row. Section 2 may archive the original | none |
| Reported series, source present or null | timeseries object | `source.provider`, or `open_wearables` when that string is missing | `ow1` report only | `source_report`, appendix subject | none. The amount is the reported pair |
| Measured series, `source` null | timeseries object | `open_wearables` | `ow1` unattributed only | the raw class, full match key | none. An unequal payload refuses the protected transaction and may be archived |
| Day series | timeseries object, `resolution` `1day` | same as the raw row | same as the raw row | window form in the appendix. Key adds `1day` and `period_end` | none |
| Sleep, activity, or recovery summary | `{ "sleep_summary": data }`, `{ "activity_summary": data }`, or `{ "recovery_summary": data }` | `source.provider` | `ow1` summary | `source_report` subject `object`, date | one measured child per numeric field, field path in the key |
| Body | the object | umbrella `source.provider`, or `open_wearables` when it is missing | `ow1` summary, family `body` | `source_report` subject `object` | slow-changing weight, height, fat, and muscle are time-unknown provider summaries; averaged heart rate and HRV use the server window; latest temperature and blood pressure use their own times; age and BMI are source reports |
| Health score | `{ "health_score": data }` | top-level `provider`, or `open_wearables` | `ow1` score | `provider_score` when the scale table has a finite range, otherwise `source_report` | none. Components stay on the record |
| Meal | `{ "meal": data }` | `source.provider` | `ow1` event | `source_report` subject `object`, meal id | one `source_report` per numeric field path |
| Cycle | `{ "cycle": data }` | `source.provider` | `ow1` event | `source_report` subject `object`, cycle id | one `source_report` per numeric reported length |
| Personal record | the object after `user_id` is removed | `open_wearables` | `ow1` personal | `source_report` subject `personal_record` | none |

`source_report` is not one of the seven measured record types in SPEC
section 5.4. It has a source, so it is not an athlete report and it is
not a nutrition entry. Nutrition entries have no source. D161 still says
a mapping cannot turn a personal report into a measured record. A source
report stores that report without making it a measurement. The eight range calculations and
`provider-period-summary` do not read it. Step 5 creates the
`source_reports` table. Value and unit are both present or both absent.
`report_kind` is one of `intake`, `account`, `predicted`, `code`, `text`,
`manual_workout`, `unknown_workout`, `index`, or `original`. Gender uses
`account`. It is not copied into `sex` or the athlete profile. The
receipt still adds no column.

One source report has this JSON shape. Extra fields are refused.

```json
{
  "source_id": "record id of the source",
  "report_kind": "intake",
  "subject": "hydration",
  "provider_record_id": "optional server id",
  "reported_at": "optional UTC timestamp",
  "input_timestamp": { "timestamp_text": "the server timestamp" },
  "provider_reporting_periods": [],
  "amount": {
    "input_value": 250,
    "input_unit": "mL",
    "standard_value": 250,
    "standard_unit": "mL"
  },
  "received_at": "when this program first stored it",
  "received_at_basis": "provider_import",
  "supplied_fields": {}
}
```

`amount` is optional. When it is present, both input fields and both
standard fields are present. It is a reported pair. It is not a measured
`SourceValue`, and `field_basis` is absent. A day report omits
`reported_at` and uses `open_wearables_reporting_window` for the server's
timestamp and `period_end`. Kind `original` omits `amount` and omits
reporting periods. `input_timestamp` is either a timestamp text or
`timestamp_missing_reason`. The record is registered on `RecordType`, the
current and revision unions, the package schema, `typedTables`, repair,
and the root SQL check. It is not added to the seven measured types or to
the point, sample, or session readers.

Match keys for the new records are the writer, the sensor, the resource,
the server id or the date or the body group, and the subject. The anchor
subject is `object`. A child subject is the field path, one key part, so
`nutrients.dietary_sugar` and `calories_kcal` do not collide. Two meals
at one timestamp stay two records because the meal id differs. Legacy
device keys are unchanged.

The original server object is stored on the anchor, in `supplied_fields`,
including nulls, text, booleans, codes, and nested objects. A normalized
number is a child. Its `input_value` and `input_unit` are the server's
number and unit. `supplied_fields` hold that leaf. The appendix table is
the mapping for all 141 series, raw and day. `standard_value` equals
`input_value` except for `walking_step_length` and `running_stride_length`.
Those quantities are metres. The server sends centimetres, so
`standard_value` is `input_value / 100`. A 70 cm step is 0.70 m. A 120 cm
stride is 1.20 m. The other aliases change the unit text and not the
number: `percent` to `%`, `celsius` to `°C`, `degrees` to `deg`, `meters`
to `m`, `m_per_s` to `m/s`, `watts` to `W`, `met` to `MET`, `minutes` to
`min`, and `ml_kg_min` to `ml/kg/min`. A new quantity uses that alias, or
the server's unit token when it has no alias. Existing quantity units are
not edited to match the server token.

No day row is a measurement sample. The day `timestamp` is
`CalendarPeriod.of_date(...).start`. `period_end` is the next boundary.
`window_seconds` is null. Those bounds are the reporting window
`open_wearables_reporting_window` on a provider summary, a provider
score, or a source report. `recorded_at` is unset. `duration_ms` is unset.
The program does not turn the two bounds into a duration, so a 23-hour or
25-hour local day stays the server's two timestamps. The window is not
called the measurement instant. Integer `window_seconds` on a raw sample
sets `duration_ms` to that count times 1000. The original field stays. Raw `heart_rate` stays a sample at the
server timestamp. Day `heart_rate`, day steps, and day speed are window
summaries. The two Garmin day scores stay scores on their existing
scales, with the window and without `recorded_at`. Raw points, raw
`vo2_max`, and the two raw Garmin scores keep their kinds and keys. Raw
`heart_rate_recovery_one_minute` is the point quantity `hr_recovery`, in
bpm. The server series name stays in `supplied_fields`. The 2.1.0 import
key stays. A day row of that series is a window summary whose summary
type is also `hr_recovery`. Its key adds `1day` and `period_end`, so it
does not replace the raw point. A mapped file and a pull, in either
order, keep one record id.

`computed_by: provider` sets basis `provider`. `computed_by:
open_wearables` sets basis `aggregator`. `computed_by: null` omits the
basis and keeps the null. A raw row that omits `computed_by` keeps the
2.1.0 basis `provider_or_aggregator`. That omission is not the JSON null.
The raw Garmin scores keep basis `provider` from the existing lines.
A health score with top-level `provider: internal` has basis
`aggregator`. A vendor name there has basis `provider`. A null provider
omits the basis. The device writer is not this author.

A finite range in `constants/health_scores.py` is the only scale. The
text is `{min}-{max}`. The unit is `score` when the number is present.
Sleep is `0-100` for internal, WHOOP, and Samsung, and `1-100` for Oura,
Garmin, and Polar. Oura readiness is `1-100`. Polar readiness is `0-10`.
Oura activity is `1-100`. Garmin stress and Garmin body battery are
`0-100`. WHOOP and Suunto recovery are `0-100`. Polar recovery is `1-6`.
WHOOP strain is `0-21`. Polar strain has no finite maximum. A missing
category or provider is not given a scale. That score is a
`source_report`. The number stays in the original object. The raw series
`garmin_stress_level` and `garmin_body_battery` stay the existing score
lines. They are not these health-score rows. A null value uses the same kind as a present value for
that provider and category. Components are not extra scores. RMSSD and
SDNN stay separate. `RecoverySummary.recovery_score` uses the same range
table, with category recovery and the provider copied onto the summary's
`source`. Polar is `1-6`. WHOOP and Suunto are `0-100`. A provider that
is missing or absent from that table does not get `0-100`. That child is
a source report, and the number stays in the original object. The schema
prose that says the field is WHOOP-only and 0 to 100 is not the scale.
The number is not rescaled. A null uses the same kind as a present value
for that provider, and the child is withdrawn as section 2 says. The
anchor keeps the null.

WHOOP recovery components can include `skin_temp_celsius`. The score
object keeps that component. `RecoverySummary` has no skin-temperature
field. A non-null SDNN is stored. RMSSD is not copied into it. The body
snapshot does not fill historical temperature.

Sleep summary numbers are sums or weighted values across every non-nap
session in the selected source and date. `start_time` and `end_time` are
only the longest main sleep, or the service's minimum start and maximum
end when no main sleep exists. Those bounds stay inside the anchor's
original object. They are not a reporting period on `duration_minutes`,
time in bed, efficiency, stage minutes, or interruptions. Two main sleeps,
00:00–04:00 and 06:00–08:00, can sum to 360 minutes. That 360 stays on
`provider_date`. It is not placed inside the four-hour sleep. This
program does not re-sum the sessions and does not invent an interval.
Nap totals and the heart-rate, HRV, respiration, and blood-oxygen
averages are also date-only. The session list stays on the anchor. The
service can add a missing part as zero when it builds
`total_duration_minutes`. A returned zero stays zero. A null stays null.

Activity fields are date-only provider summaries. `floors_climbed` is
described as calculated from elevation. `total_calories_kcal` can be the
provider total or a sum. The response does not say which, so the basis
stays `provider_or_aggregator`. Intensity minutes can be returned as zero
where the service fills a missing key with zero.

Body is the current snapshot. It is not the requested range. The anchor
is a `source_report`. Weight, height, fat, and muscle are provider
summaries with `timestamp_missing_reason: not_provided_by_source`, no
`recorded_at`, and no borrowed window. Age and BMI are `source_report`
children. Age is calculated by the service. The child is a source report
and has no `field_basis`. BMI is not labeled measured. The averaged
heart-rate and HRV values are provider summaries on the server's
`period_start` and `period_end`. Those bounds are not copied onto the
slow-changing fields. Each latest field uses its own measured-at time
when the server sends one. A value with a null time uses the time-unknown
receipt. A null group withdraws that group's existing children. It does
not delete them because a different request omitted the snapshot.

Meal numbers, nutrient numbers, dietary series, alcoholic-beverage
counts, `hydration`, and the index, age, and effort series are
`source_report` records. `hydration` is intake. The server's amount and
millilitre sum are not a sample and not a day measurement. A device id
on that row does not make them measured. They are not provider summaries
and not nutrition entries. A
nutrient keeps the value and unit on its leaf. Cycle codes, predicted
lengths, booleans, text, and `pregnancy_snapshot` stay on the cycle
anchor. A numeric cycle length is also a `source_report` child, not a
measurement. On an automatic workout, a number that is not already a
typed session field stays in `supplied_fields` and is a `source_report`
child. It is not a provider summary. `intensity` stays under that name.
Segments and zones stay under the server's names. A
manual workout is the `source_report` in the table. It does not become a
workout session, a point, or a device receipt.

The same returned object can change a child from a number to null, or
remove a nutrient. The mapper emits that child again with value and unit
both absent. The record id, the revisions, and the first `received_at`
stay. D182 copies the stored `received_at` onto the new revision.
`became_known_at` is when that revision was written. A later number
returns on the same child. A range that does not return the object does
not withdraw its children. A refused page rolls the withdrawals back
with the page.

A protected refusal rolls that transaction back. It does not become a
successful pull. The archive is pull-only. `ath import --mapping` uses
the same protected envelopes and the same refusal, and it does not start
the archive transaction. A refused file import keeps no original.

Before either transaction, the pull checks the page itself. The order of
the rows does not matter. The privacy scan is first. A hit starts
neither transaction. A missing or malformed required UUID, a supplied
field whose name is not allowed, and any other decoded-field failure are
the same kind of check. They use the returned JSON. They do not use the
database. One such failure starts neither transaction. A timeseries row
has no required UUID. A query-bucket `left_out` row is not one of these
failures, and it is not archived.

The database facts are read later, inside the protected
`BEGIN IMMEDIATE`, before any insert, update, or delete. The read covers
every row on the page, including a row the mapping would leave out. For
each named source it reads the receipt, the binding, deletion, and
history, and history includes tombstones. For each named record and
import key it reads deletion, the current revision, and, for a workout,
the stored kind. For each null-source full key it reads the stored
payload. That set of facts is the safety snapshot. An unlocked read
taken before this transaction is not the snapshot.

One database veto anywhere in that snapshot refuses the protected page
before any mutation. The vetoes are a conflicting device id, a deleted
source, unbound history, a malformed receipt, a workout kind that
differs from the stored record, and a deleted record or import key the
page would copy. A deleted-source row that the mapping would leave out
is still a veto. The protected transaction rolls back. The archive is
not attempted. The veto stands even when an earlier unlocked read saw
no veto. A missing device id on a timeseries row is not this veto.

The archive is allowed only when that snapshot has no veto and it has
one of these eligible facts: a device-targeted series row whose source
has no device id, or a null-source series row, measured or reported,
whose full match key has a different payload in this page or already
stored. An eligible row does not authorize the archive when a veto is
also on the page. A confirmed command keeps this snapshot for the
archive transaction. It does not keep a lock after the protected
rollback. A preview discards the snapshot with that rollback.

When the archive revalidation below matches the safety snapshot, the
archive stores one capsule per returned series row on that page, not a
query-bucket row and not a non-series row. A workout or a meal on that page is not a capsule. A
later page that does not refuse can store it. The capsule is:

```json
{
  "resource": "timeseries",
  "resolution": "raw",
  "row": {}
}
```

`resolution` is the response's `metadata.resolution`, copied. It is not
inferred from `window_seconds` or `period_end`. `row` is the original
series object, unchanged. The capsule is the same object for a pull and
for a later native export and import. Kind is `original`. There is no
`amount` and no reporting period. The number stays inside `row`. The
match key is the writer, the archive sensor, and the SHA-256 of the
canonical capsule, with keys sorted and no extra whitespace. The same
capsule is one record. A raw capsule and a day capsule are different
even when the row bodies match. The hash is not a sample id. This plan
does not claim that two different capsules cannot hash equal. If the key
is already stored and the stored capsule differs, the archive transaction
fails before any insert and does not revise the old capsule.

The archive source uses writer `open_wearables`, sensor `ow1:["archive"]`,
and `source_type` `connector`. The receipt operations are not called, and
`device_binding` is not written.

The archive transaction starts with its own `BEGIN IMMEDIATE`, after the
protected rollback. The re-read and any archive writes are that same
transaction, so another writer cannot commit between them. Before it
inserts a source or a capsule, it reads
the same safety facts again, including a `left_out` neighbor. It also
reads the archive source and each target capsule, including a tombstone.
It compares those facts with the safety snapshot. A new veto, a
different binding, a new or missing history row, a source or record
tombstone, or a different safety revision means the snapshot is stale.
`archive` is then `not_attempted`. The transaction rolls back with no
writes. The fetched payload is not stored under the archive source, and
it is not given a new identity. A deleted archive source is not
recreated. A deleted capsule is not replaced. If the pull cannot name
this bounded set for a row, it does not archive the page. There is no
retry. A page that committed an archive still sees that archive. A
deletion after this archive commits is a later command.

`protected_write` is `committed` or `refused`. `archive` is
`not_attempted`, `stored`, `unchanged`, or `failed`. `committed` is the
protected counts. `archive_committed` is the archive counts. A record is
not in both. `written` follows section 1 and is not set by the archive
alone. `database_changed` is true when this command's archive transaction
committed a new source or a new capsule, and it stays true if an earlier
page already changed the database. A later refusal does not clear the
earlier counts. When the archive stores N new capsules and the command
has no protected change, the text is `Protected write refused; N
originals archived; database changed; pull incomplete.` A repeat that
stores nothing says `0 originals archived` and says the database is
unchanged only when no earlier page changed it. The unchanged text is
`Protected write refused; 0 originals archived; database unchanged; pull
incomplete.` After an earlier change it is `Protected write refused; 0
originals archived; database changed; pull incomplete.` A third payload
adds its own new count and does not replace the earlier capsule. If the
archive transaction fails, it rolls back entirely, `archive` is `failed`,
the protected refusal remains, earlier commits remain, the success
sentence is not printed, and the exit is nonzero. The text is `Protected
write refused; archive failed; database unchanged; pull incomplete.`
After an earlier change it says `database changed`. It does not print a
partial archive count. When the archive is `not_attempted` after a
protected refusal, the text is `Protected write refused; archive not
attempted; database unchanged; pull incomplete.` After an earlier change
it says `database changed`. That result adds no archive count. A
preview, including `--json` and `--json --yes`, rolls the protected
transaction back and does not start the archive transaction.
`estimated_archive` comes from that rolled-back read and is labeled
estimated.

A page of only reported series, with no device id, is an ordinary source
report write. It is not an archive, and it does not refuse the device
check. A normalized row is not revised from an archive.

A provider summary or provider score may omit `value` only together with
its unit. One without the other is refused. Step 5 rebuilds both typed
tables so the pair can be null, copies every existing row, and keeps
record ids, `data_json`, import keys, revisions, tombstones, and reporting
periods. Scale on a provider score stays required. No number is invented.
The time-unknown summary is valid only with the missing-timestamp reason
above.

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

The check runs on device-targeted rows. That includes a measured raw or
day row whose `source` object has no `device_id`. That row is not marked
`left_out` before the check. A reported series does not enter this check.
A query-bucket row that is already `left_out` does not establish a
binding and does not supply the id. Summary, event, score, report,
archive, unattributed, and personal-record rows do not enter this check.
The call is one of these states:

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
  device-targeted timeseries row with no id is refused. Accepted rows that
  mix a nonempty id with a missing id are refused. A reported series is
  not one of these rows. Accepted rows that share one nonempty
  `source.device_id` store that string on the source in the same
  transaction as the first accepted write. That source may be created in
  this call, or it may already exist with no receipt and no history. A call
  that accepts no rows creates no binding. A source created or first-bound
  inside a refused call is rolled back with the call.

A second device is refused rather than merged. That refusal is not
archived. On `--yes`, an earlier committed page stays, the failed page
makes no database changes, the range is not complete, and the exit is
nonzero. File import rolls back the whole
call. `--json` and `--json --yes` make no database changes. Device reads
that have `filter_by_priority` send `false`. Summary routes have no such
switch. Section 2 states that limit.

Preview keeps two facts in memory for this command, for each source sensor:
the binding it would have stored, and an unbound source it would have
created once that source has accepted history. A summary sensor does not
supply a binding for the device sensor. A later preview page uses those facts, including a
later page that adds an id after an id-less sleep page. The memory is
discarded with the process. It does not hold null-source payloads, so a
preview does not use it to judge those conflicts. It does not change the
estimated record counts.
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
rule above, the typed writes in section 6, and the bounded archive
safety read in section 2. It gains nothing beyond those:

- Read the committed receipt, and whether any record or source device for
  that source exists, including a deleted row.
- Create a source with the checked receipt already in its JSON.
- Write the checked receipt on an existing source only when the receipt is
  absent and that same read shows no accepted or deleted record and no
  source device. Every other source field stays. The write uses the normal
  revision and `data_json` bookkeeping. Writer, route, sensor, import keys,
  and baselines stay.
- Read the sources, records, import keys, and archive keys named by one
  page, including tombstones and the current revision. This is the safety
  read in section 2. It does not scan the database.

The three receipt operations run inside the caller's `BEGIN IMMEDIATE`
transaction. The safety read runs in that same transaction, before any
mutation, and again at the start of the archive transaction.
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
It does not list users. A raw HTTP personal-record response includes `user_id`. That field must
be a string UUID equal to the selected user id. Absence, null, a
non-string, and any other string refuse the page before redaction and
before mapping. A matching string is removed before the scan below and
is not stored. The canonical import envelope is that object after the
removal. File import of the envelope does not put `user_id` back. An
envelope that still has `user_id` is not that envelope, and the scan
rejects it. Birth date,
gender, and sex are stored. They are account health data, not the API key.

The process refuses a base URL that could carry the key in the URL itself.
It refuses plain `http` except to loopback. It does not follow a redirect, so
the key cannot be sent to a second host. One attempt, then stop. Timeouts are
10 seconds to connect and 30 seconds to read. One response may be at most
8 MiB. One read may be at most 1000 pages.

The body is read inside that cap and parsed before any `apply_mapping` call.
The guard is in `open_wearables.rs`. It walks every decoded object key and
every decoded string. A value that contains the API key, the selected
origin, or the selected user id rejects the whole page. The matching personal-record `user_id` is the only field that is dropped
before this walk. Any other copy of the user id still rejects the page.
A raw personal-record `user_id` that is absent, null, the wrong type, or
not the selected user refuses the page before the drop. The page is not
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
same constants 2.1.0 moved. Migration step 5 is a new SQL file and a narrow runner arm. It updates
`athleticstandard_version` to `2.2.0` and `sql_schema_version` to 5. No
column is added to an existing table. The generated contract adds
`source_report` to the `records.record_type` check, creates
`source_reports`, and rebuilds `provider_summaries` and
`provider_scores` as section 2 describes. Files `001` through `004` stay
byte-identical. The check is not removed, and ordinary store writes do
not turn foreign keys off.

SQLite cannot change that check in place. `records` and
`record_revisions` reference each other. The link from a record to its
current revision is deferred. The link from a revision back to the
record, and the links from typed rows, import keys, deletions,
attachments, reporting periods, and the other child tables, use
`ON DELETE CASCADE` and are not deferred. Triggers and views name
`records`. `PRAGMA foreign_keys` does not change inside a transaction.
`installInitialSchema` and `withWriteTransaction` already require it to
be on. A `DROP TABLE records` there would fail or cascade through the
children. Step 5 does not do that.

The create path finishes steps 1 through 4 and closes that transaction
before step 5. `installInitialSchema` does not run step 5 inside the
transaction that requires foreign keys to be on. Target 5 does not use
`withWriteTransaction`.

Step 5 uses one connection that ordinary writes do not reuse. Before
`BEGIN`, it sets `PRAGMA foreign_keys=OFF` and reads the pragma back. The
value must be 0. Turning foreign keys off does not turn off the schema
check SQLite runs during `ALTER TABLE`. `sql_v1_current_records` and
`sql_v1_active_records` select from `records`. The triggers
`record_revisions_record_type_matches_parent_insert` and
`record_revisions_record_type_matches_parent_update` are defined on
`record_revisions` and also read `records`. Dropping `records` does not
drop them. A rename while they still point at a missing `records` fails.
`writable_schema` is not used.

Inside that one transaction the order is fixed.

1. Drop `sql_v1_current_records` and `sql_v1_active_records`.
2. Drop the two cross-table triggers named above.
3. Create `records_next` with the old columns and the check extended by
   `source_report`. Copy every root row. Drop `records`. Rename
   `records_next` to `records`.
4. Rebuild `provider_summaries` and `provider_scores` the same way.
   Copy deleted rows. Keep record ids, `data_json`, import keys,
   revisions, and periods. Value and unit may be null together.
   Recreate `provider_summaries_lookup_idx` and
   `provider_scores_lookup_idx` from the text in `001`. No view and no
   trigger in `001` names those two tables. A generated object whose
   text names one of them is dropped before that rebuild and recreated
   from the same text afterward. There is no search of the whole schema.
5. Recreate `records_record_type_idx` and the triggers defined on
   `records`, from the text in `001`:
   `records_current_revision_must_exist`,
   `records_deleted_flag_must_match_deletion_row`,
   `record_erasure_blocks_current_record_insert`, and
   `record_erasure_blocks_current_record_update`.
6. Recreate the two views and the two cross-table triggers from that
   same text, unchanged.
7. Create `source_reports` and its type trigger.
8. `PRAGMA foreign_key_check` is empty and `PRAGMA integrity_check` is
   `ok`.
9. Set the version rows and `user_version` 5.

`sql_v1_database_info`, `sql_v1_record_revisions`, and
`sql_v1_record_deletions` do not name `records`, so they stay through the
gap. Then the transaction commits. After commit, and after a rollback or
a thrown error, the connection sets `PRAGMA foreign_keys=ON` and reads it
back. The value must be 1. If that read fails, the connection is closed
and is not reused. A later ordinary open still refuses to write while
foreign keys are off. A failure before commit restores the views, the
cross-table triggers, the old check, the rows, and the version. The D124
backup is already on disk. Device receipts stay in the copied source
JSON. They are not rewritten.

A new database closes the step 4 transaction, then runs this step 5 path.
Step 9 sets `user_version`. If step 5 fails during init, that transaction
rolls back to the closed step 4 schema before the existing failed-init
cleanup runs. Init reports failure. `createDatabase` still closes the
database and `removePartialLocalDatabase` still deletes the new file.
D202 leaves those paths in place, and this build does not change them.
A file that cleanup does not delete is not left with the views dropped.
A migrate of a database that already existed is a separate case. Its
step 5 rollback leaves that file in place, at the old schema, with
foreign keys on. `applyMigrationStep` gains this arm for target 5
and does not use `withWriteTransaction` for it. `MIGRATION_CHAIN` in `src/db/sql-contract.ts`,
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
`ProviderReportingPeriodType` gains `open_wearables_reporting_window`.
`ValueBasis` gains `aggregator`. Series quantities gain the sample series
whose units are in the appendix table. Intake, index, age, and effort
series do not gain quantities. The record type `source_report` is added to `RecordType`, the current
and revision unions, the package schema, and `typedTables` in
`src/db/check-tables.ts`. It is not a measured type. Its table has `record_id`, `source_id`,
`report_kind`, `subject`, optional `reported_at`, optional
`standard_value` and `standard_unit` together, optional
`provider_record_id`, `received_at`, and `data_json`. It has no required
measured value. `provider_summary.value` and
`provider_score.value` may be omitted together with the unit, as section 2
says. A provider summary may also omit `recorded_at` and reporting periods
when its input timestamp uses `not_provided_by_source`. No other field is
added. The folder exists
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
| Measurement samples, source reports, paired null value and unit, and child withdrawal | `insert_typed`, `update_typed`, and `typed_table` in `store.rs`; the same pair and table in `src/db/records.ts`, `src/db/import-package.ts`, `src/db/records-view.ts`, and the repair copy from `data_json` |
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
first bind on an existing source that has no history. The archive
transaction adds a bounded read of the sources, records, import keys,
and archive keys named by that page, including tombstones and the
current revision. It does not scan the database. It also inserts and
updates `measurement_sample` and `source_report`, and it accepts a provider
summary or provider score whose `value` and unit are both present or both
absent. A half-null pair is refused. When an anchor in this response no
longer emits a child, that child is revised to the absent pair in the same
transaction. `src/db/records.ts`, `src/db/import-package.ts`,
`src/db/records-view.ts`, and the repair copy from `data_json` follow that
pair and the new table. `src/db/check-tables.ts` gains the
`source_report` entry. The eight range calculations and
`provider-period-summary` do not read `source_reports`. In
`read_inputs` and `read_period`, a provider summary or provider score
whose `standard_value` or `standard_unit` is null is excluded in the SQL,
before the limit, the count, the grouping, and the numeric decode. A null
is not stored as zero and is not replaced by an older revision. The
session-summary branch of `read_period` uses the same filter on the
extracted number and unit. Formulas and source grouping stay as they are.
The archive insert is the pull-only transaction in section 2. It is not
inside `apply_mapping`, and a mapped file import does not call it. It
revalidates the safety snapshot before any insert. It does
not call the receipt operations. No other store behavior changes. `ath correct`,
package restore, and an accepted package merge stay whole-record owner
writes. They are named in section 3 so the binding survives a 2.2.0 round
trip. They are not a new importer. The pull sums only `sleep_session`, `workout_session`,
`point_measurement`, `measurement_sample`, `provider_summary`,
`provider_score`, and `source_report`. A committed protected page adds
those counts to `committed`. A committed archive page adds its counts
only to `archive_committed`. A rolled-back preview page adds its
protected estimates to `estimated` and its archive estimates to
`estimated_archive`. It does not add
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
`range_complete` and `snapshots_complete` are both true.

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

- two pages, the second `next_cursor`, then `has_more: false`. `--yes`
  with a finished body and personal record prints the success sentence.
  A finished range whose body read fails says the range is complete, names
  the body snapshot, does not print the success sentence, and exits
  nonzero. A preview says the snapshot would be stored. An empty ranged
  result can still store the personal record
- the same session id on two pages, identical payload: the preview estimate
  counts two new rows, and `--yes` counts one new and one unchanged
- the same session id on two pages, changed payload: the preview estimate
  counts two new rows, and `--yes` counts one new and one changed
- a valid page, then a failing page, under `--yes`. The failure is one of a
  401, a 429, a repeated or null cursor, a vetoed page, or a minimum page
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
  value. A day row for heart rate, steps, running speed, resting heart
  rate, RMSSD, SDNN, and blood oxygen becomes a provider summary with
  `open_wearables_reporting_window`, no `recorded_at`, and the server's
  `timestamp` and `period_end`. Check, export, and import keep that window
  and do not invent an instant. A step length of 70 cm is stored as 0.70 m.
  A stride of 120 cm is stored as 1.20 m. The input number and `cm` stay
  on the record. Raw `heart_rate_recovery_one_minute` is quantity
  `hr_recovery` in bpm. A 2.1.0 mapped file and this pull, in either
  order, keep one record id and the old import key. Every row in the appendix table has one raw fixture and
  one day fixture. Check, export, and import keep the input pair, the
  standard pair, and the day window. Raw
  `vo2_max` stays `provider_summary.vo2_max` on its 2.1.0 key. A day
  `vo2_max` row is a different window summary. Raw `garmin_stress_level`
  and `garmin_body_battery` stay provider scores on their existing scales
  and keys. They do not become samples. A null RMSSD beside a present SDNN
  stores only the SDNN. A sleep-summary respiratory rate is date-only. The
  sleep session from the same night still has no respiration field. Nap
  totals do not receive main-sleep bounds. Two main sleeps of 240 and 120
  minutes store duration 360 on `provider_date` only. Skin temperature is stored
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
  records. A page with one bound heart-rate row and one same-triple row
  whose `device_id` is missing refuses the protected page. Neither row
  is stored as a measurement. The pull archive stores both series rows
  as capsules, with `resolution` copied from the response and no device
  receipt. A mapped file import of that page refuses and stores no
  capsule. A later pull page that has only the valid row stores that
  measurement. The capsule remains a different record. Export and native
  import keep the resolution. A raw capsule and a day capsule of the same
  row body stay two records. A sleep id that
  is missing, beside a valid workout, refuses the page before any write
  and is not archived. The next page still rolls back on its own failure.
  A measured timeseries row with
  `source: null` is stored on the unattributed sensor when the payload
  matches. A reported row with `source: null` uses the report sensor
  only. A different payload with the same full key, including
  resolution and `period_end`, refuses the protected write and archives
  the new original. A mapped file import of that changed payload writes
  nothing. A raw midnight row and a day window that starts then
  stay two records. A preview of those two pages does not claim the
  conflict, because the first preview page was rolled back. A confirmed
  second page does.
- a missing device id beside a conflicting device id, in either row order,
  refuses the protected page and stores no capsule and no revision. The
  same veto holds for a deleted source, an unbound legacy history, and a
  missing workout UUID on the same page as an eligible row, including when
  the deleted-source row would otherwise be `left_out`. A credential on a
  neighboring object starts neither transaction. A deleted archive source
  is not recreated. An existing device receipt is unchanged. On an empty
  database, the first eligible archive sets
  `written` false, `database_changed` true, `archive` `stored`, and the
  text `Protected write refused; N originals archived; database changed;
  pull incomplete.` A repeat on that empty database sets `archive`
  `unchanged`, `written` false, `database_changed` false, and the text
  `Protected write refused; 0 originals archived; database unchanged; pull
  incomplete.` A third payload adds one archive count and
  leaves the earlier capsule. An archive-transaction failure on that
  empty database sets `archive` `failed`, keeps the refusal, adds no
  archive row, leaves `written` false and `database_changed` false, and
  uses the text `Protected write refused; archive failed; database
  unchanged; pull incomplete.` The same four results after an earlier
  committed sleep page keep that page's `written` true, its `committed`
  counts, and `database_changed` true. The failure text then says
  `database changed`.
  Preview and `--json --yes` commit neither transaction
- two connections, both row orders. Before the protected `BEGIN IMMEDIATE`,
  the other connection commits a binding of device C on source S. The page
  has a series for S with device B and a missing-id neighbor. The
  protected transaction refuses. `archive` is `not_attempted`. No capsule,
  source revision, or receipt change is written. The same refusal holds
  when the other connection commits a tombstone or history on S instead
  of the binding. A page whose safety snapshot has no veto, then a
  deletion of S after the protected rollback, does not archive S's
  payload. A deletion of the normalized record or its import key between
  the two transactions does not create a capsule. A deletion of the
  archive source, or of an existing capsule, does not recreate it and
  does not store a second identity. An owner correction that changes the
  binding, the history, or the safety revision after the protected
  rollback leaves `archive` `not_attempted` and writes nothing. An
  already deleted neighbor, in either row order, is a protected veto and
  is not archived. A credential on the page starts neither transaction.
  An earlier committed sleep page keeps `written` true, its `committed`
  counts, and `database_changed` true. The text is `Protected write
  refused; archive not attempted; database changed; pull incomplete.`
- sensors `ow1:["summary","sleep",null]` and a colon inside a provider
  name stay different from a shifted tuple. The text `-` is not the same
  sensor as a missing part. A score whose input source changes from null
  to an object keeps its id. A personal record repeats. A personal record
  with no birth date uses the time-unknown receipt. A meal with calories
  and two nutrients stays three children plus the anchor
- an internal health score has basis `aggregator`. A vendor score has
  basis `provider`. A null `provider` omits the basis. Polar readiness uses
  scale `0-10`, Polar recovery uses `1-6`, and WHOOP strain uses `0-21`.
  A score with no finite range keeps its number on the anchor and does not
  use scale `0-100`. An Oura sleep score uses `1-100`. A Polar recovery
  summary value of 3 and the Polar health score use scale `1-6`. A WHOOP
  recovery summary uses `0-100`. An author missing from the range table
  is a source report and is not given `0-100`. A null value and a
  later number revise the same record. `computed_by: null` omits the basis
  and keeps the null. A raw row that omits `computed_by` keeps
  `provider_or_aggregator`
- body fetched during a historical range is the current object. Weight
  with no time has no `recorded_at` and does not use the average window.
  A latest value with a null time is stored the same way. Age is a source
  report. The service calculates it, and the record has no `field_basis`.
  BMI's basis is omitted. The three groups can arrive
  together or alone. JSON null and a returned zero and a missing field
  stay three different results
- a dietary series, `number_of_alcoholic_beverages`, and `hydration` are
  source reports, not samples, not provider summaries, and not nutrition
  entries. Raw and day hydration stay out of the eight range calculations
  and out of `provider-period-summary`. A device id on the hydration row
  does not create a receipt. A null-source hydration,
  `dietary_energy_consumed`, or index row, raw and day, uses only the
  report sensor. File import then
  pull, pull then file import, and a repeat keep one record and no
  receipt. A changed payload refuses the protected write. A pull with no
  veto archives that new capsule and leaves the stored report. A mapped
  file import writes nothing. The row is not a calculation input. A cycle
  code, a predicted length, and a pregnancy snapshot stay on the anchor.
  A cycle that starts in range and ends later keeps that end. A cycle that
  starts later is not stored, and the read can still complete. Birth date,
  gender, and sex are source reports. Gender is not copied into the
  athlete profile. A matching `user_id` is not stored. A different,
  absent, null, or non-string `user_id` on the HTTP object refuses the
  page. The canonical envelope without `user_id` imports. The athlete
  profile is unchanged
- step 5 keeps an old summary's id, number, unit, `data_json`, import key,
  revisions, and tombstone. A numeric summary can be corrected to
  numberless and back. A half-null value and unit is refused. A sample
  repeats and corrects. `ath check`, export, native import, and restore
  accept the numberless summary, the numberless score, and the sample. A
  meal nutrient that changes from a number to null, then back, keeps one
  child id. A null body group withdraws that group's children. A refused
  page restores the withdrawn child. A range calculation that sees a
  numeric summary and a numberless summary uses only the numeric row.
  The same is true for a score, on the instant reader and the period
  reader. An all-numberless set adds no inputs and does not fail the
  calculation. After value, then null, then value, the calculation sees
  the current number and not the withdrawn one
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
  `provider_or_aggregator`. A raw Garmin score keeps basis `provider`. An
  internal score keeps basis `aggregator`
- a raw page and a mapped file that carry `device_binding` on the row, on a
  nested source object, and inside `supplied_fields` do not set, replace, or
  erase the source key. A mapping whose `name` changes and whose
  `device_hardware` stays `source.device_id` keeps the same binding. A
  mapping whose `device_hardware` points elsewhere is refused before any
  write. A mapping that sends a device row to an `ow1` sensor, or that
  changes a series class in the appendix table, is refused before any
  request
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
- `page-sleep.json` becomes a sleep session. A manual workout and its
  automatic neighbor from one response stay two records. The automatic
  workout is a session on the device source. The manual workout is a
  source report with the original object and no device receipt. A workout
  with `entry_source` `unknown`, null, or omitted is an `unknown_workout`
  report. It is not a session and not a manual report. The same server id
  arriving later as `automatic` refuses the page, and the stored report
  stays. Export and import keep the pair that was accepted.
  `page-raw.json` is stored.
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
  SQL schema 5, and `user_version` 5. Record ids, revisions, deletions,
  attachments, import keys, reporting periods, and device receipts are
  unchanged. `PRAGMA foreign_key_check` is empty. A deleted row and its
  history are still there. The upgraded database is populated, including
  a tombstone. `sql_v1_current_records` and
  `sql_v1_active_records` still return rows. An insert that would make a
  revision type disagree with its record is still rejected. A second
  migrate does nothing. A failure injected before the step 5 commit
  leaves the old views, the two cross-table triggers, the rows, the
  version, and `foreign_keys` read back as on. A successful migrate
  leaves `foreign_keys` on. A source report can then be created, revised,
  checked, exported, restored, and repaired from `data_json` without
  becoming a measured type
- `ath init` creates a database that is already `2.2.0`, SQL schema 5, and
  `user_version` 5, and `ath check` exits 0. The fresh database accepts a
  source report, the same two views are queryable, and a mismatched
  revision type is rejected. A failed init rolls step 5 back and then the
  existing cleanup removes the new file. A failed migrate of a populated
  database leaves that file, the old schema, the views, and foreign-key
  enforcement. Step 4 still
  writes `2.1.0` and schema 4, and the create path closes that
  transaction before step 5
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
- a preview `--json` has `written: false`, protected and archive
  committed zeros, `estimated` counts, and `range_complete: false`
- `--json --yes` is a preview. After a later-page failure, and after a
  readable range, neither transaction commits and the database is
  unchanged

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
   range or the body snapshot is incomplete, or `ath check` fails.
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
  version. A failed step 5 rolls back before that cleanup. This build
  does not replace the cleanup.
- `store.rs` and `mapping.rs` are past a thousand lines. The pull does not
  add HTTP to them. `mapping.rs` gains the counts, the open-wearables
  binding check, and the preservation writes in section 2. `store.rs` gains the
  partial-update refusal, the three source operations in section 3, and the
  sample, source-report, withdrawal, null-filter, archive, and paired-null
  writes in section 6. Splitting the files stays out.
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
schema and step 5: the root `record_type` check, the period type, the
`aggregator` basis, the sample series quantities in the appendix table,
the `source_report` type and table, the paired null value and unit, and
the time-unknown summary. Then the store, the calculation null filter,
the archive transaction, and the record paths in section 6. Then the
mapping engine and `mappings/open-wearables.md`. Then the reads, the
cycle filter, the smaller-page split, and the tests in section 7. The laptop card and the phone proof are
part of that same pull request. This plan pull request has no media.

The maintainer approves or rejects this plan before any build. D198 through
D208 are the proposal. They are not adopted. Reviews of earlier heads were
incorporated here. None of those reviews approves this revision. The next
head needs re-review. An earlier review accepted a
migration with no new column, under D152 and D184. That acceptance does not
adopt D198. The
optional source key adds no column.

1. One number, 2.2.0. Readable sets keep `1.0.0`. Step 5 runs on create and
   on migrate. The device receipt adds no column. Step 5 extends the
   root check with `source_report` by the copy in section 5, creates
   `source_reports`, and rebuilds `provider_summaries` and
   `provider_scores` so a numberless object can omit `value` and its unit
   together. Existing ids and rows are copied. Before the root rename,
   step 5 drops and later recreates `sql_v1_current_records`,
   `sql_v1_active_records`, and the two `record_revisions` triggers that
   read `records`. Foreign keys stay on for ordinary writes. The step 5
   connection turns them off only before its transaction and reads them
   back on after every exit. A
   `2.1.0` database does not open until `ath migrate`. This release does
   not schedule, refresh a token, backfill, or store a cursor (D198, D208).
2. Store the returned representations in section 2. Device receipts stay
   on device sources. Summaries without a device id use their own sensors.
   Null stays null. A missing response is not called provider-unavailable.
   The omissions in section 2 are the ones with a reason (D203, D208).
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
   no history, and the sample, source-report, withdrawal, and paired-null
   writes in section 6, including the null filter in the calculation
   readers and the bounded archive safety read. The pull command owns the
   archive transaction and calls the store. `mapping.rs` does not start
   it. Failed-init cleanup stays as it is (D202, D208).
