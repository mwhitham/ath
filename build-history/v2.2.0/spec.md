# Athletic Standard 2.2.0

Status: **plan only. The maintainer has not approved it. Not approved for build.**

`ath pull openwearables` reads one person's stored health data from an Open
Wearables server they run. When `--vendor-workouts` is passed, it also
reads the live workout list and detail. It writes the stored reads with
the mapping this release ships. It writes a live original only when that
flag is on and the person confirms the write. The proposals are D227 through D244 in
[decisions.md](decisions.md). They are not adopted. The evidence is
[appendix/open-wearables-api.md](appendix/open-wearables-api.md), checked on
6, 7, 8, and 14 November 2026.

This pull request is the plan. The build after approval is one later pull
request. This plan does not change a version number, a schema, a fixture, or
the program.

Version 2.1.1 on `main` `0de71bbd6474916af0408a4090bc2c74a1a3aed2` is the
software this plan is read against. That commit is the merge of pull request
44. An earlier reading used version 2.1.0 at
`8db90b42ac654767e9e9570a121bbbd4fde3348a`, the merge of pull request 39.

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

`--vendor-workouts` is off unless the person passes it. Without it, the
command does not call the connections route or either live workout route.
With it, those calls run, including on a preview and on `--json`. The
server may then refresh and save vendor tokens, revoke a connection, and
wait about 105 seconds on a rate limit. The help text and every run with
the flag say so (section 2).

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
page is previewed against the database as it was before this command. A
preview writes nothing. A match key that appears on two pages, or a server that changes
between the preview and `--yes`, can make the estimate differ from the
write. The estimate is not a promise of the final new, changed, and
unchanged split. Exact equality would need a command-wide overlay. This
proposal does not add one, and it does not add that work to `mapping.rs`.

Preview text prints:

- one line that a date-only bound is sent as typed, and Open Wearables decides
  what that day means
- `preview only; nothing stored in this database`
- `reads finished` or `reads incomplete`
- one line each for the twelve reads in section 2: `completed`, `failed`,
  or `not_attempted`
- one line each for the live workout providers: `completed`, `incomplete`,
  or `not_attempted`, the connection status when it is not `active`,
  whether that limit covers the requested range, whether more data exists,
  and the detail and left-out counts in section 2
- when `--vendor-workouts` is on, before the first live request: the three
  sentences about server changes in section 2
- when `--vendor-workouts` is on: `vendor requests were made`
- when `--vendor-workouts` is on: `vendor workout originals` with new,
  changed, and unchanged counts
- when a credential-shaped value was withheld: the count and up to 20
  paths, as section 4 says
- when `--vendor-workouts` is on, this sentence: a workout can appear twice,
  once as the vendor original and once as the stored session. The original
  is the source report whose subject is `vendor_workout`. It is never a
  session and never a calculation input
- when `--vendor-workouts` is on: `These originals keep the vendor's account ids, user URLs, and full routes.`
- for body, on a preview: `body snapshot would store` or `body snapshot
  would be empty`. On a confirmed write: `body snapshot stored`,
  `body snapshot empty`, or `body snapshot incomplete`
- estimated counts, by record type, of sleep sessions, workouts, points,
  measurement samples, summaries, scores, and source reports
- committed counts of zero on a preview
- the coverage lines in section 2
- `left_out` as a total, a count for each reason, and at most 20 example
  lines. When there are more lines, it says the list was shortened, as
  2.1.1 previews do with `MAX_PREVIEW_RECORD_IDS`
- `nothing written to this database; pass --yes after reviewing this preview`.
  The shared `NOTHING_WRITTEN` line in `src/cli/commands/routes.rs` stays
  as it is for the other commands

`--yes` without `--json` commits each successful page. That commit is one
`ProviderPreview::apply` transaction (D189). It is not a completed pull. The text
prints actual committed counts for pages kept. It prints `requested range
<from> through <to> is complete` only when every stored ranged read in
section 2 reached that read's terminal condition. A live workout line does
not change that sentence. Body is not one of those ranged reads. The
success sentence `pulled openwearables from <from> through <to>` is
printed only when that range is complete, body and the personal record
completed, and every live provider is `completed` or `not_attempted`.
An `incomplete` live provider prints `vendor workouts incomplete:` and the
provider name. The exit is then nonzero. The success sentence is not
printed. The stored range sentence can still say the range is complete. A finished range with an incomplete body snapshot does not
print that sentence. The body line still says the snapshot is incomplete.

If a later page fails, the command keeps a typed outcome and then exits
nonzero. It prints that outcome as text, then uses the exit status. There
is no child process. Version 2.1.1 removed that split. When a ranged read failed, the text says the
requested range is incomplete. When every ranged read completed and body
or the personal record failed, the text says the requested range is
complete and names the snapshot that failed. It names any read that is
`not_attempted`, and it prints the retained counts. The exit is nonzero
in both cases. The success sentence is not printed. A failed read says to
run the same command again. An identity refusal uses the sentence in
section 3, which says a rerun cannot
repair the device id. It does not print the success sentence. A refused
page is not applied, so it makes no database changes.
Records already stored stay, including their revisions. An archive, when
section 2 allows one, is a later pull-only preview and apply. It does not print
the success sentence. `written` is true when any protected transaction
in this command created or revised a counted record. An archive does not
make `written` true. `database_changed` is true when any protected
transaction or any archive transaction in this command committed a
change, including the first insert of the archive source. A vendor page
in section 2 is a protected transaction. Its originals are `source_report`
records, so they add to `committed.source_report` as well as to
`vendor_workouts.originals` and that provider's counts. A committed vendor
page that creates or revises an original makes `written` true and
`database_changed` true, even when every stored page was unchanged. A
first `vendor_workout` source counts as a database change. A later page
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
every stored read completed. `range_complete` ignores the live workout
lines. The read status token is `not_attempted`, in this object and in
the text. The object has no key, server, user id, or request path.
`vendor_workouts` is always present.

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
  "left_out": { "total": 0, "by_reason": {}, "examples": [], "shortened": false },
  "withheld": { "total": 0, "paths": [], "shortened": false },
  "owner_corrections_replaced": 0,
  "vendor_workouts": {
    "requested": false,
    "requests_made": false,
    "notice": [],
    "originals": { "new": 0, "changed": 0, "unchanged": 0 },
    "providers": {
      "garmin": {
        "status": "not_attempted",
        "connection_status": null,
        "covers_requested_range": false,
        "more": false,
        "limit": "not requested",
        "owner_corrections_replaced": 0,
        "details": {
          "stored": 0,
          "kept": 0,
          "deferred": 0,
          "failed": 0,
          "mismatch": 0,
          "thinner": 0,
          "not_available": 0
        },
        "left_out": { "total": 0, "by_reason": {} }
      }
    }
  }
}
```

`notice` is empty without the flag. With it, `notice` holds the three
sentences about server changes in section 2, on every run, including a
preview. `connection_status` is `active`, `revoked`, `expired`,
`unknown status`, or `null` when the server listed no connection for that
provider.

The top-level `owner_corrections_replaced` counts owner corrections of
stored records that the stored pages replace, by the owner-correction rule
in section 2. Each provider's `owner_corrections_replaced` counts its
vendor originals. In a preview both are estimates. When a provider's count
is above zero, the preview prints `would replace N owner corrections;
delete an original to keep it` on that provider's line. When the top-level
count is above zero, it prints `would replace N owner corrections of
stored records because the server's data changed`. After `--yes` the same
lines say `replaced N owner corrections`, and that each correction stays
in the record's history. The text uses the singular when N is 1.

The same record types are present for workouts, points, summaries, scores,
and source reports. The example above is shortened. Polar, Suunto, Whoop,
Fitbit, Oura, Strava, Withings, and Sensorbio use the same fields as
`garmin`. Those names are the public `--json` keys. It is the preview
outcome. A preview does not say a snapshot was stored.

The command returns a different outcome for `--yes` without `--json`. That
outcome has the actual `committed` counts, the actual `archive_committed`
counts, the actual `owner_corrections_replaced` counts,
`database_changed`, `protected_write`, `archive`,
`range_complete`, and `snapshots_complete`. `committed` counts protected
records only. `archive_committed` counts archive records only, including
a created archive source, counted once. A record is in one of those
maps. `new`, `changed`, and `unchanged` are the created, revised, and
unchanged counts in each map. `range_complete` is true when the stored ranged reads
finished and no protected page in those reads was refused. A live workout
line does not change it. `snapshots_complete`
is true when the personal record and the body snapshot finished. `written`
is true when any protected transaction in this command created or revised
a counted record. A command that only repeats unchanged protected pages,
and that commits no archive change, has `written` false,
`database_changed` false, both completion flags true, and unchanged
protected counts. An earlier change keeps both flags true. A failed ranged page has `range_complete` false. A
failed snapshot after a finished range has `range_complete` true and
`snapshots_complete` false. The command prints sentences from this outcome.
It does not print the outcome as public JSON. Public `--json`, with or
without `--yes`, applies neither the protected preview nor the archive
preview.

`--date` stays required only for `garmin`. In 2.1.1, `PULL` in
`src/cli/spec.rs` marks `--date` mandatory, so the parser refuses any pull
without it: `error: required option '--date <day>' not specified`. The
build drops that mark. `pull` in `src/cli/commands/routes.rs` then checks
`--date` for `garmin` and prints that same text, so `ath pull garmin`
without a date fails as it does today. `--from` and `--to` are required
only for `openwearables`, and they are flags, not environment variables.
`--base-url` and `--user` are required for `openwearables` unless the
environment named above supplies them. `--base-url` already exists for
`garmin`, where it names a test server's API root. For `openwearables` it
names the server origin, as D229 says. `ath pull garmin --date YYYY-MM-DD`
is otherwise unchanged, and it still writes without a preview.
`ath pull` with any other name still fails. The failure text
`pull currently reads garmin` becomes `pull reads garmin or openwearables`.

The flag's help in `src/cli/spec.rs` says: also read live vendor workouts;
the server may refresh and save vendor tokens, revoke a connection, and
wait about 105 seconds on a rate limit. `src/cli/examples/pull.txt` repeats
those three effects and says that "nothing stored" means this database
only.

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

D237 is the storage rule. D232 keeps these values in this pull. D233
corrected source, time, and score direction. D234 corrects units, day
windows, sleep bounds, identity encoding, reported records, and child
withdrawal. D235 corrects the root record type, numberless calculation
inputs, recovery scales, required ids, workout provenance, hydration,
and the archive of a refused original. D236 corrects the migration
dependencies, the command counts, the archive capsule, and null-source
report routing. D237 corrects when the archive reads the database, and
the failed-init sentence. D238 stores a workout the person typed as a
workout session. D239 keeps a body skin-temperature time unknown when the
response does not say it is an instant. D240 keeps a finite
`hr_recovery`, including zero and a negative number. D241 keeps the
current manual-session calculation rule. D242 stores the live
workout list and detail as originals. D243 is the live-read procedure.
D244 keeps each vendor object whole. An earlier draft
left day values, the heart-rate stream, summaries, and workout segments for
a later version. That draft is superseded. File imports stay. The Garmin pull stays. A Junction pull is proposed as
2.3.0. Automatic sync, retries, token refresh, and backfill are proposed as
2.4.0. None of those numbers is adopted. This release does not add its
own token refresh. A live read can still make the server refresh one, as
section 2 says. This release adds one module for
this server. It does not add a connector interface.

This pull stores the representations Open Wearables 0.11.0 returns for one
user. A ranged read stores that range. Body does not. A returned number
keeps the server's unit. A returned null stays absent and is not stored as
zero. A returned zero stays zero, except a point quantity whose rule still
requires a positive number. `hr_recovery` is not one of those quantities.
A finite zero and a finite negative `hr_recovery` stay. `temperature_deviation`
stays signed. Other points stay positive. A row the server did not return is not
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

A page over the response cap, `MAX_PULL_RESPONSE_BYTES` (10 MiB), is not
mapped. On a cursor or offset read, the same
cursor or the same offset is sent again with `limit` halved, down to 1.
The discarded response writes nothing and does not advance the cursor or
the offset. A transport failure is not retried. Day `limit` counts periods.
A `limit` of 1 can still hold many rows, one for each source and series in
that period. The server also floors that period count at 1. If the minimum
page is still over the cap, the read is incomplete. The message names the
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

`entry_source` `automatic` is a workout session on the device source, as
the paragraph above says. `entry_source` `manual` is a workout session
on the source whose `source_type` is `manual`. `field_basis.value` is
`entered_by_hand`. It is not a device measurement. A `device_id` the
server sent stays on the original object. It does not set
`device_binding`, and the workout is not stored on the watch's source.
The server workout id is the match key. A later pull of that same
workout updates the session. Name, type, start, end, distance, heart
rate, elevation, zones, and segments are stored the same way they are
stored on an automatic workout. `field_basis.activity_type` is
`aggregator_normalized`, because Open Wearables normalized the sport
name. The numbers on this session do not enter the eight range
calculations or `provider-period-summary`. `workout-drift` may use the
session's start and end as its window when the caller names that session.
The samples in that window still exclude `source_type` `manual`.
`query_measurements`, `query_series`, and `query_sessions` in
`src/store/query.rs` omit manual sources unless `include_manual` is true.
This pull does not change those three functions. Other record listings are not that switch. The session does not enter a device
baseline. `unknown`, JSON
null, and a missing `entry_source` are source reports of kind
`unknown_workout`. They are not a workout session, and they are not
manual. The original keeps null and absence distinct. Unknown is not
labeled manual, and it is not labeled automatic because a model or a
device id is present. A repeated server workout id that arrives as a
different kind or sensor refuses the protected page. The stored record
stays. The new interpretation is not written beside it. The text says
the classification changed and a rerun cannot choose. `ath correct`
remains the owner's way to replace the record.

Sleep, workout, meal, cycle, and health-score objects need a `id` that is
a UUID string. Missing, null, blank, and any other shape refuse the
protected page before a source, a row, or hardware changes. The row is not
`left_out`. No empty or time-based id is invented. A valid neighbor on
that page is not stored. Each page of the command is its own preview and
apply, so only the failed page is refused, and earlier committed pages
stay.

A new sensor is the prefix `ow1:` plus a JSON array. The array has no
spaces. A missing part is JSON `null`. An empty string and the text `-`
stay strings. JSON escaping keeps a colon, a quote, and a backslash
inside one part. Two different tuples therefore stay different sensors.
The legacy device sensor is not rewritten into this form. D181 still
escapes `:` and `\` when the sensor is placed in a match key.

The array shapes are:

- Summary or body: `["summary", family, provider, device, device_type, source, device_id]`
- Meal, cycle, or unknown workout: `["event", family, provider, device, device_type, source, device_id]`
- Reported series, including a row whose `source` is null: `["report", resolution, provider, device, device_type, source, device_id]`. `resolution` is `raw` or `1day`.
- Health score: `["score", provider]`. `provider` is the top-level author,
  or JSON `null` when that field is null. The input `source` is not in
  the array.
- Measured series with `source: null`: `["unattributed", "timeseries"]`. A reported series does not use this sensor.
- Archive originals: `["archive"]`
- Personal record: `["personal_record"]`
- Vendor originals: `["vendor_workout", provider]`. `provider` is the
  provider name in the route path, such as `garmin` or `suunto`.

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
protected page. The stored normalized row stays. The text says the
rows are ambiguous and a rerun cannot choose the normalized row. `ath
correct` remains the owner's way to replace that row. A preview page
writes nothing, and the command memory holds device-binding facts only.
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
| Manual workout | `{ "workout": data }` | `source.provider` | not the device sensor | `workout_session` on the manual source, server id | same typed fields, zones, and segments as an automatic workout |
| Unknown, null, or omitted `entry_source` | `{ "workout": data }` | `source.provider` | `ow1` event | `source_report` `unknown_workout`, server id | none. The original object is the record |
| Measured raw series with `device_id` | timeseries object, `resolution` `raw` | `source.provider` | legacy device | the raw class in the appendix, D181 time key | none |
| Measured raw or day series, source present, no `device_id` | timeseries object | `source.provider` | legacy device, then the protected page is refused | no measured row. Section 2 may archive the original | none |
| Reported series, source present or null | timeseries object | `source.provider`, or `open_wearables` when that string is missing | `ow1` report only | `source_report`, appendix subject | none. The amount is the reported pair |
| Measured series, `source` null | timeseries object | `open_wearables` | `ow1` unattributed only | the raw class, full match key | none. An unequal payload refuses the protected page and may be archived |
| Day series | timeseries object, `resolution` `1day` | same as the raw row | same as the raw row | window form in the appendix. Key adds `1day` and `period_end` | none |
| Sleep, activity, or recovery summary | `{ "sleep_summary": data }`, `{ "activity_summary": data }`, or `{ "recovery_summary": data }` | `source.provider` | `ow1` summary | `source_report` subject `object`, date | one measured child per numeric field, field path in the key |
| Body | the object | umbrella `source.provider`, or `open_wearables` when it is missing | `ow1` summary, family `body` | `source_report` subject `object` | slow-changing weight, height, fat, and muscle are time-unknown provider summaries; averaged heart rate and HRV use the server window; body temperature and blood pressure use their own times; skin temperature is time-unknown; age and BMI are source reports |
| Health score | `{ "health_score": data }` | top-level `provider`, or `open_wearables` | `ow1` score | `provider_score` when the scale table has a finite range, otherwise `source_report` | none. Components stay on the record |
| Meal | `{ "meal": data }` | `source.provider` | `ow1` event | `source_report` subject `object`, meal id | one `source_report` per numeric field path |
| Cycle | `{ "cycle": data }` | `source.provider` | `ow1` event | `source_report` subject `object`, cycle id | one `source_report` per numeric reported length |
| Personal record | the object after `user_id` is removed | `open_wearables` | `ow1` personal | `source_report` subject `personal_record` | none |
| Vendor workout | the vendor envelope in the vendor section below | `open_wearables` | `ow1:["vendor_workout", provider]` | `source_report` kind `original`, subject `vendor_workout`. The key ends with the provider and the vendor id | none. The whole envelope is the record |

`source_report` is not one of the seven measured record types in SPEC
section 5.4. It has a source, so it is not an athlete report and it is
not a nutrition entry. Nutrition entries have no source. D161 still says
a mapping cannot turn a personal report into a measured record. A source
report stores that report without making it a measurement. The eight
range calculations in `src/calculate/range.rs`, `provider-period-summary`
among them, do not read it. Step 6 creates the `source_reports` table. Value and unit are both present or both absent.
`report_kind` is one of `intake`, `account`, `predicted`, `code`, `text`,
`unknown_workout`, `index`, or `original`. Gender uses
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
  "input_timestamp": {
    "timestamp_text": "the server timestamp, as sent",
    "utc_offset_text": "the offset at the end of that text",
    "utc_offset_meaning": "meaning_not_documented",
    "timestamp_basis": "source_offset_timestamp"
  },
  "provider_reporting_periods": [],
  "amount": {
    "input_value": 250,
    "input_unit": "mL",
    "standard_value": 250,
    "standard_unit": "mL"
  },
  "received_at": "when this program first stored it",
  "received_at_basis": "import_time",
  "supplied_fields": {}
}
```

`amount` is optional. When it is present, both input fields and both
standard fields are present. It is a reported pair. It is not a measured
`SourceValue`, and `field_basis` is absent. A day report omits
`reported_at` and uses `open_wearables_reporting_window` for the server's
timestamp and `period_end`. Kind `original` omits `amount` and omits
reporting periods. `input_timestamp` is the receipt that `input_receipt`
in `src/import/mapping.rs` writes for a timestamp text. A `Z` text is
`source_instant` with `documented_recording_offset`. Any other offset is
`source_offset_timestamp` with `meaning_not_documented`. When the server
gave no time, it is `missing_timestamp_receipt` in
`src/import/candidate.rs`. `received_at_basis` is `import_time`, as every
2.1.1 mapped import writes. A vendor original's start follows the rule in
the vendor section below. The record is registered on `RecordType`, the
current and revision unions, the package schema, the table description in
`src/store/encode.rs`, repair, and the root SQL check. It is not added to the seven measured types or to
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
bpm. A finite value stays, including zero and a negative number. The
mapping's positive check and the v1 point check gain that exception
beside `temperature_deviation`. The exception is the point check in
`src/model/data.rs`. `packages/npm/src/schema.ts` is frozen compatibility
code and is not where this check is added. No table
is rebuilt for the check. The server series name stays in `supplied_fields`. The 2.1.0 import
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
slow-changing fields. Body temperature and blood pressure use their own
measured-at times when the server sends those times. Skin temperature
does not. The body service can put a day row's `period_start` into
`skin_temperature_measured_at` when no sample is in the latest window,
and the response has no field that says which one it did. The amount,
the unit, and that timestamp stay on the original object. The skin-temperature
child is time-unknown. It has no `recorded_at`. The program does not
decide by comparing timestamps, and it does not invent a window from
that timestamp. A raw skin-temperature sample and a day skin-temperature
window stay the series records they already are. A null time on body
temperature or blood pressure uses the time-unknown receipt. A null group
withdraws that group's existing children. It does not delete them because
a different request omitted the snapshot.

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
Segments and zones stay under the server's names. A manual workout is
the workout session in the table. It is not a source report, a point, or
a device receipt.

The same returned object can change a child from a number to null, or
remove a nutrient. The mapper emits that child again with value and unit
both absent. The record id, the revisions, and the first `received_at`
stay. D182 copies the stored `received_at` onto the new revision.
`became_known_at` is when that revision was written. A later number
returns on the same child. A range that does not return the object does
not withdraw its children. A refused page writes nothing, so its
withdrawals are not written either.

A page is one 2.1.1 import. `preview_route` in `src/import/provider.rs`
stages the page and matches it against the database in one read
transaction, at one database revision. It writes nothing. With `--yes`,
`ProviderPreview::apply` writes that plan in one write transaction. The
apply refuses the plan when the database revision moved after the
preview, when a corrected record's revision moved, or when a match key
now names another record. A protected refusal is found by the preview,
so the page is not applied. It does not become a successful pull. The
archive is pull-only. `ath import --mapping` uses the same protected
envelopes and the same refusal, and it does not build an archive. A
refused file import keeps no original.

Before the preview, the pull checks the page itself. The order of
the rows does not matter. The privacy scan is first. A hit builds
no preview. A missing or malformed required UUID, a supplied
field whose name is not allowed, and any other decoded-field failure are
the same kind of check. They use the returned JSON. They do not use the
database. One such failure builds no preview. A timeseries row
has no required UUID. A query-bucket `left_out` row is not one of these
failures, and it is not archived.

The database facts are read in the page's matching step, inside the read
transaction above. The read covers every row on the page, including a row
the mapping would leave out. For each named source it reads the receipt,
the binding, deletion, and history, and history includes tombstones. For
each named record and import key it reads deletion, the current revision,
and, for a workout, the stored kind. For each null-source full key it
reads the stored payload. That set of facts is the safety snapshot. It
belongs to the database revision the preview read. The apply refuses a
moved revision, so a page is never written against facts it did not read.

One database veto anywhere in that snapshot refuses the protected page.
The vetoes are a conflicting device id, unbound history, a malformed
receipt, and a workout kind that differs from the stored record. The
archive is not attempted after a veto. A missing device id on a
timeseries row is not this veto.

A deleted source or a deleted record is not a veto. A row whose source the
person deleted is left out, and the line says so (D190). A row whose
match key names a record the person deleted is left out, and the line
says so (D182). The rest of the page is written. These are the 2.1.1
keyed-import rules, and a deleted row is never recreated. A deleted source
or a deleted record named anywhere on the page still forbids the archive
of that page.

An owner correction is kept until the server's data changes. This rule
is new in 2.2.0. It covers every record on route `open_wearables`, from a
pull or from `ath import --mapping open-wearables`. Every other route
keeps D182 as it is. An import writes each revision with basis
`provider_import`. A revision with any other basis is an owner correction
here: `ath correct`, a protocol write, and an accepted package merge write
other bases. A native restore copies each revision's basis, so a restored
database keeps the same answer. When a page's candidate names a stored
record by its import key, and that record is not deleted:

1. A candidate equal to the current record writes nothing, as in 2.1.1.
2. When the current revision has basis `provider_import`, a different
   candidate replaces it, as in 2.1.1 (D182).
3. When the current revision is an owner correction, the candidate is
   compared with the newest revision whose basis is `provider_import`.
   Equal means the server has not changed that record since an import last
   wrote it. Nothing is written, and the correction stays current.
   Different means the server's data changed. The candidate replaces the
   correction, and that counts as one `owner_corrections_replaced`. The
   correction stays in the record's history. A record with no
   `provider_import` revision is replaced and counted the same way.

The comparison is the 2.1.1 comparison, with the compared revision's
`received_at`. Section 4 withholds credential values before the candidate
is built, so a withheld value is the same marker on both sides. A library
write that chooses basis `provider_import` counts as an import revision.

The archive is allowed only when that snapshot has no veto, names no
deleted source or record, and has one of these eligible facts: a
device-targeted series row whose source has no device id, or a
null-source series row, measured or reported, whose full match key has a
different payload in this page or already stored. An eligible row does not
authorize the archive when a veto is also on the page. A confirmed command
keeps the snapshot's database revision for the archive. A preview never
applies an archive.

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
is already stored and the stored capsule differs, the archive fails before
it writes and does not revise the old capsule.

The archive source uses writer `open_wearables`, sensor `ow1:["archive"]`,
and `source_type` `connector`. The receipt operations are not called, and
`device_binding` is not written.

The archive is its own preview and apply, after the protected refusal. Its
preview stages one capsule per returned series row. It matches them
against the archive source and each target capsule, including a deleted
one. It goes ahead only when its database revision is still the one the
protected preview read. Otherwise `archive` is `not_attempted` and nothing
is written. Its apply refuses a moved revision, as every 2.1.1 apply does,
and `archive` is then `not_attempted` too.

The archive does not read the safety facts a second time. Every write to a
database moves its revision: `Store::apply` in `src/store/write.rs`,
`ProviderPreview::apply`, repair in `src/store/repair.rs`, restore in
`src/package/restore.rs`, and erase in `src/package/erase.rs` all advance
it. So an unchanged revision means the binding, the history, the
tombstones, and the vetoes are the ones the protected preview read. The
fetched payload is not stored under the archive source, and it is not
given a new identity.

A deleted archive source leaves every capsule out and is not recreated
(D190). A deleted capsule is left out and is not replaced (D182). The
archive refuses its own plan when that plan would revise a stored capsule,
so a stored capsule is never revised. If the pull cannot name this bounded
set for a row, it does not archive the page. There is no retry. A page that
committed an archive still sees that archive. A deletion after this archive
commits is a later command.

`protected_write` is `committed` or `refused`. `archive` is
`not_attempted`, `stored`, `unchanged`, or `failed`. `committed` is the
protected counts. `archive_committed` is the archive counts. A record is
not in both. `written` follows section 1 and is not set by the archive
alone. `database_changed` is true when this command's archive apply
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
archive apply fails, its transaction writes nothing, `archive` is `failed`,
the protected refusal remains, earlier commits remain, the success
sentence is not printed, and the exit is nonzero. The text is `Protected
write refused; archive failed; database unchanged; pull incomplete.`
After an earlier change it says `database changed`. It does not print a
partial archive count. When the archive is `not_attempted` after a
protected refusal, the text is `Protected write refused; archive not
attempted; database unchanged; pull incomplete.` After an earlier change
it says `database changed`. That result adds no archive count. A
preview, including `--json` and `--json --yes`, applies neither the
protected plan nor the archive. `estimated_archive` comes from the
archive's own preview and is labeled estimated.

A page of only reported series, with no device id, is an ordinary source
report write. It is not an archive, and it does not refuse the device
check. A normalized row is not revised from an archive.

A provider summary or provider score may omit `value` only together with
its unit. One without the other is refused. Step 6 rebuilds both typed
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
- Accounts, sync, OAuth, webhooks, API keys, config, uploads, and every
  `DELETE`. They are not health readings. The API key, the server address,
  and the user id are not written into the athlete record. `/sync` and
  `/sync/historical` are not called. The connections list is called only
  when `--vendor-workouts` is on.

### Live vendor workouts

`--vendor-workouts` is off by default. Off, the command makes no request
to `/api/v1/users/{user_id}/connections` and no request to either live
workout route. Preview and `--json` without the flag make those zero
requests. On, the same preview and `--json` do make them, write nothing
in this database, and print `vendor requests were made`. "Nothing stored"
and "nothing written" always mean this database. The server can still
change, as the next paragraph says.

A live call can change the server. `api_client.py` at
the pinned commit can refresh a vendor token and save it, revoke a
connection when refresh is missing or auth has failed for a day, and emit
`connection.revoked`. It writes the Redis keys `token_refresh_lock` and
`provider_auth_failing_since`. On HTTP 429 it sleeps 15, then 30, then 60
seconds, up to three retries, so one request can wait about 105 seconds.
This pull does not add its own retry. It does not add a vendor token or a
new scope. The server may still do the work above while answering the GET.

Every run with the flag says this in three sentences, before its first
live request. The server may refresh and save vendor tokens. It may revoke
a connection. It may wait about 105 seconds on a rate limit. A preview and
a `--yes` run print the sentences. A `--json` run puts them in
`vendor_workouts.notice`, so its output stays one JSON value. The flag's
help text says the same.

With the flag on, the pull reads the connections list first. It checks
that each item's `user_id` is the selected user. Any other value refuses
the list, because it is not this user's list. The pull then keeps only
`provider` and `status` from each item and drops every other field,
including `linked_user_ids`, which holds other users' ids. The list is
never stored. Nothing from it is printed except a provider name from the
table below and a status. So the privacy scan in section 4 does not run on
this list, and the selected user id inside it does not refuse it. A
refused list makes every provider `not_attempted`, and the vendor read is
`incomplete`.

A provider is called only when its item has `status` `active`. The
pinned `ConnectionStatus` also has `revoked` and `expired`. Those
providers are `not_attempted`, and their line shows that status. They are
never called, because a call could make the server try a refresh or a
revoke. A provider with no item is `not_attempted`, and its status is
`null`. A status the pin does not define is not active, and it is printed
as `unknown status`.

The order is fixed: Garmin, Polar, Suunto, Whoop, Fitbit, Oura, Strava,
Withings, Sensorbio. An active provider with no live list method is
`not_attempted`. That is not a failure. One provider's failure leaves its
line `incomplete`, and the other providers still run. Apple, Samsung,
Health Connect, Ultrahuman, and Google Health have no live list method.
Their stored workouts still come from `/events/workouts`.

Live requests go one at a time. Each starts at least one second after the
previous live request started. The connect timeout is 30 seconds. The read
timeout is 360 seconds. The server's own provider timeout,
`provider_request_timeout_seconds`, defaults to 30 seconds. So one GET can
take four attempts and three waits, about 225 seconds, plus any token
refresh under the server's lock. The longer read timeout lets the server
finish those retries first. If a live request still times out, the server
may still be working on it. The pull then sends no other request to that
provider in this command, so two requests to one provider never overlap.
That provider is `incomplete`. The next provider still runs, and it uses
a different vendor account and limit. An HTTP 429 from a live request
stops that provider the same way. The server has already waited and
retried three times, so the vendor's limit is spent. No later list call,
window, or detail call goes to that provider in this command. The detail
call that got 429 counts as `failed`. Every listed id that had not had its
detail call yet counts as `deferred`, and a later command details it by
the order below. What the provider already returned is still written as that
provider's page: the list objects, and the details already fetched. A
later Garmin window after a 429 or a timeout is not sent, unlike a window
that failed another way. A server whose provider timeout is
raised can outlast this timeout. The same rule covers that case. The
stored reads keep 30 seconds to connect and 120 seconds to read, the values
`src/runtime/garmin_pull.rs` uses.

Budgets keep one command inside the providers' limits:

- The pace above keeps this command at 60 live requests a minute or
  fewer. The review cited about 100 a minute for Garmin. This study did
  not find that number in Garmin's public docs.
- Garmin is read in windows of 82,800 seconds, 23 hours, or less, one
  call each. Every window is sent. A one-year range is 381 windows, so it
  takes more than six minutes before any detail call.
- A command makes at most 100 detail calls to one provider, and at most 20
  to Strava. Ids past the budget are `deferred`. The next paragraphs say
  which ids go first.
- Strava's limit is per application, so the deployment's own sync shares
  it. Strava's rate-limit page, fetched 15 November 2026, gives a default
  of 100 read requests per 15 minutes and 1,000 a day. Strava calls them
  non-upload requests. One list call and 20 detail calls can become 84
  requests when the server retries each one three times. That leaves 16
  for the deployment's own sync in the same 15 minutes. The Strava line
  says that its limit is shared.

The routes are
`GET /api/v1/providers/{provider}/users/{user_id}/workouts` and
`GET /api/v1/providers/{provider}/users/{user_id}/workouts/{workout_id}`.
At the pinned commit both return the provider's raw response. The server
does not reshape it. The table below is the whole provider logic. Every
live request has the 30-second connect timeout and the 360-second read
timeout above.

| Provider | List unwrap | Id | Start | Detail key | Detail unwrap | Detail fields | Budget | Read timeout |
|---|---|---|---|---|---|---|---|---|
| Garmin | the JSON array | `activityId` | `startTimeInSeconds`, Unix seconds | `activityId` | the object | none named | every window; 100 details | 360 s |
| Polar | the JSON array | `id` | `start_time`, text with no offset | `id` | the object | `samples`, `zones`, `route` | 1 list; 100 details | 360 s |
| Suunto | `payload` | `workoutId` | `startTime`, Unix milliseconds | `workoutKey`, else `workoutId` | `payload` | none named | list pages of 100; 100 details | 360 s |
| Whoop | `records` | `id` | `start`, text with offset | `id` | the object | none named | 1 list; 100 details | 360 s |
| Fitbit | the JSON array. The method reads `activities` | `logId` | `startTime`, text with offset | no detail call | none | none | 1 list | 360 s |
| Oura | `data` | `id` | `start_datetime`, text with offset | `id` | the object | none named | 1 list; 100 details | 360 s |
| Strava | the JSON array | `id` | `start_date`, text with `Z` | `id` | the object | `laps`, `splits_metric`, `splits_standard`, `segment_efforts`, `best_efforts`, `map` | 1 list; 20 details | 360 s |
| Withings | the JSON array. The method reads `series` | `id` | `startdate`, Unix seconds | no detail call | none | none | 1 list | 360 s |
| Sensorbio | `data[].activities[]`. The parent is kept as `list_parent` | `id` on the nested activity | `start_time`, Unix milliseconds | no detail call | none | none | 1 list | 360 s |

- Garmin. The pull splits the range into windows and sends
  `summary_start_time` and `summary_end_time` as integer Unix seconds. The
  server puts those values in `uploadStartTimeInSeconds` and
  `uploadEndTimeInSeconds`, so a window selects by upload time, not by
  start. Each window ends where the next begins. The pinned server splits
  a window it sees as longer than 24 hours, and it drops a failed piece
  without saying so. It measures the window in naive local time, so on a
  server whose clock moves forward, a true 24-hour window can look 25
  hours long. A 23-hour window cannot look longer than 24 hours.
  A failed window makes Garmin `incomplete`, and later windows still run.
  The pull calls Garmin detail at the pinned path,
  `/wellness-api/rest/activities/{activity_id}`. That path is not in
  Garmin's public docs. This study looked and did not find it, so the path
  may not exist. For a Garmin detail only, HTTP 404 or HTTP 501 counts as
  `not_available`. The list object is stored whole, with `detail_status`
  `not_available`, and Garmin stays `completed`. Any other failed Garmin
  detail is `failed`, and Garmin is `incomplete`. A 404 or 501 on a Garmin
  list window is not this rule. It fails that window. Every other
  provider's detail 404 is `failed`. The pinned client passes a vendor's
  404 through unchanged, and the route answers 501 only for a provider it
  has no workout method for, so this study cannot tell a missing path
  from a missing activity.
- Polar. The list sends `samples`, `zones`, and `route` false, the route's
  default, so the list stays small. Detail sends all three true. A list
  over the response cap fails that list, because the route has no smaller
  page.
- Suunto. The list sends `since` as the requested start in Unix
  milliseconds, `filter_by_modification_time=false`, and `limit` 100, and
  pages `offset` until a short page. False means creation time. A page
  over the response cap is asked again with a smaller `limit`, down to 1.
  The record key is `workoutId`. Detail is called at `workoutKey` when the
  list object has one, and at `workoutId` otherwise. Detail unwraps
  `payload`. A detail matches when its `workoutId` or its `workoutKey`
  equals the list object's. The pinned `WorkoutJSON` model has no
  `workoutKey`. The pinned tests and Suunto's own API use it, so the list
  object may carry it. Detail sends no `extensions` parameter.
- Whoop. One call. The method caps `limit` at 25. The response's
  next-page field is `next_token`. The request parameter is `nextToken`,
  and the route does not send it. `more` is true when the response has
  `next_token`.
- Oura. One call with no date and no `next_token`. `more` is true when the
  response has `next_token`.
- Strava. One list call at page 1. `per_page` is the server setting
  `strava_events_per_page`, default 200. `more` is true when the page
  length equals `per_page`. The detail object also carries `embed_token`.
  Section 4 withholds that value.
- Fitbit. The list uses the last 30 days when the route sends no dates.
  Detail is not implemented and is not called.
- Withings. The list uses `pull_sync_lookback` or 30 days, widens each
  edge by one local day, and pages inside the method. Detail is not called.
- Sensorbio. The method sends `last-timestamp` 0 and `limit` at most 50,
  and it does not follow `links.next`. So it reads only the oldest 50
  entries. `more` is true when `links.next` is present. Detail is not
  called. Each nested activity is one object. Its parent entry, without
  the `activities` array, is kept beside it as `list_parent`, so the
  parent's `timestamp` and `name` are not lost.

Every returned original is stored, including one whose start is outside
`--from` and `--to`. The same rule is used for every provider. The count
is `vendor workout originals`. `range_complete` uses only the stored
ranged reads. Each live line names its limit, says whether that limit
covers the requested range, and sets `more` as the notes above say. Those
next-page values are not followed. The published route does not accept
them as the next request. `covers_requested_range` is false for every
provider in this pin. None of the live methods filter by the workout's
start inside `--from` and `--to`.

The pull converts the range once for Garmin and Suunto, because their
parameters are numbers. A date `--from` is midnight UTC that day. A date
`--to` ends at midnight UTC after that day. That is the rule the Open
Wearables API introduction gives for a date-only bound. A timestamp bound
converts exactly. A fraction of a second rounds down for the start and up
for the end, so the windows cover the whole range. The stored reads still
send `--from` and `--to` unchanged.

A vendor id is text. A JSON string is used as it is, unless it is empty.
A JSON integer is written in decimal. So `123` and `"123"` are one id and
one record. An integer above 9007199254740991 is refused. D213 stores every
number the way JavaScript does, so its digits would change. Any other
number, and any other JSON type, is refused as an id. An object that holds
an integer above that size anywhere inside it is refused for the same
reason. A refused object refuses only itself, and the count says why. Two
different objects with one id in one provider's response refuse both,
because the pull cannot tell which is right. Two equal objects are one.

Garmin is read in windows, and one activity can be in two windows when
Garmin received it again. A window selects by upload time, so the later
window holds the later copy. For Garmin only, an object in a later window
replaces a different object with the same id from an earlier window. The
earlier copy counts as `superseded` in `left_out`, and the later copy goes
through the detail rules. Equal copies in two windows are one object, and
nothing is counted. Inside one window, two different objects with one id
still refuse both as `duplicate_id`.

The start column names one field and how to read it:

- Text with an offset or `Z`. The receipt is the one `input_receipt` in
  `src/import/mapping.rs` writes. `reported_at` is that instant in UTC.
- Text with no offset, such as Polar's `start_time`. The receipt is
  `offset_timestamp_receipt` in `src/import/candidate.rs` with
  `meaning_not_documented`. It keeps the text and says the source gave no
  offset. There is no `reported_at`. Polar's separate
  `start_time_utc_offset` stays in `list`. This pull does not join the two.
- Unix seconds or milliseconds. The receipt keeps the digits as
  `timestamp_text`, with `timestamp_basis` `source_instant` and
  `utc_offset_missing_reason` `not_applicable`. `reported_at` is that
  instant in UTC, with milliseconds. A new helper beside
  `offset_timestamp_receipt` writes this receipt. It passes the 2.1.1
  receipt checks: a text has a basis, and the missing offset has a reason.
- Missing, or any other value. The receipt is `missing_timestamp_receipt`.
  There is no `reported_at`.

The start is stored and is not a filter.

The pull withholds credential values, by the rule in section 4, in each
list object and each detail before it compares anything. The detail
order, the `kept` check, the thinner check, and the owner-correction rule
all compare withheld copies. So a Garmin list object whose
`userAccessToken` changed between pulls still equals its stored copy, and
it gets no detail call.

For each listed id, the pull first reads the stored original, if there is
one. That read is crate-private and lives in `src/import/provider.rs`. It
returns whether the original exists, whether it is deleted, whether it
holds a detail, and its stored `list`. It reads the newest revision with
basis `provider_import`, not an owner correction, so a person's edit does
not look like a vendor change and does not spend a detail call.

The pull calls detail only when the provider has a detail call and its
budget allows. It spends the budget in this order. First come ids with no
stored detail that no earlier command marked `not_available`. Then come
ids whose list object differs from the stored `list`. Then come ids an
earlier command marked `not_available`. Inside each group the oldest start
goes first. An id without a start instant goes last in its group, in list
order. So a Garmin path that answers 404 for every id still lets new ids
go first, and it costs at most the provider's budget in one command. An id whose original holds a
detail and whose list object is unchanged gets no detail call. A deleted
original gets no detail call. A detail is fetched again only when the list
object changed. A provider that changes a detail without changing the
list object is not seen, and the line does not claim it.

The detail must name the same id. A detail that names another id is a
`mismatch`, and it is not stored.

What is written for one id depends on what this command got:

- A new detail that is not thinner than the stored one. The original is
  written with this list object and this detail. The id counts as
  `stored`.
- A new detail that is thinner. Nothing is written for that id, and the
  stored original stays. The id counts as `thinner`. A detail is thinner
  when one of the provider's detail fields in the table is a nonempty value
  in the stored detail and is missing, null, or empty in the new one. A
  provider with no named detail field has no thinner detail.
- No new detail call, because the stored detail is current. The pull sends
  the envelope with the stored detail, so the original counts as
  `unchanged` and nothing is written. The id counts as `kept`.
- No new detail, and none stored. The original is written with the list
  object alone. `detail_status` says why: `deferred`, `failed`,
  `mismatch`, or `not_available`. The id counts under the same word.
  `not_available` means the provider has no detail call, or Garmin
  answered its detail call with 404 or 501.
- No new detail, but one is stored, and the list object changed. Nothing
  is written for that id, and no revision is added. The stored detail
  stays. The id counts as `deferred`, `failed`, or `mismatch`.

A list-only object therefore never replaces a stored detail. A `failed`
or `mismatch` detail makes the provider `incomplete`, and the exit is
nonzero. A `not_available` detail leaves the provider `completed`. A
`deferred` detail leaves the provider `completed`, with the
deferred count on its line, and the next command details those ids first.
A deferred id that the provider stops listing is never detailed by a later
command. The deferred count shows how many are waiting.

Each original is one envelope. The pull builds it, and a saved file holds
the same object:

```json
{
  "provider": "suunto",
  "vendor_id": "1001",
  "start": 1767254400000,
  "start_kind": "unix_milliseconds",
  "list": { "workoutId": 1001, "workoutKey": "abc", "startTime": 1767254400000 },
  "detail": { "workoutId": 1001, "workoutKey": "abc", "startTime": 1767254400000 },
  "detail_status": "stored"
}
```

- `provider` is the provider name in the route path.
- `vendor_id` is the id from the table's id column, as text.
- `start` is the value at the table's start column, copied unchanged. It
  is absent when the object has none. `start_kind` is `text`,
  `unix_seconds`, or `unix_milliseconds`, from the table. It is absent
  when `start` is.
- `list` is the list object. `list_parent` is present only for Sensorbio.
- `detail` is the detail object after the table's detail unwrap. It is
  absent when no detail is stored.
- `detail_status` is `stored`, `deferred`, `failed`, `mismatch`, or
  `not_available`.
- A value that section 4 withholds is replaced in place by its marker.
  Nothing else in `list`, `list_parent`, or `detail` is changed.

The record is a `source_report` of kind `original` and subject
`vendor_workout`. Its writer is `open_wearables`, its sensor is
`ow1:["vendor_workout", provider]`, and its `source_type` is `connector`.
`provider_record_id` is `vendor_id`. `input_timestamp` and `reported_at`
follow the start rule above. `received_at_basis` is `import_time`. The
whole envelope is kept in `supplied_fields`, including the fields the
mapping reads. The match key is the writer, the sensor, the resource
`vendor_workout`, the provider, and the vendor id, escaped by D181. So the
Suunto record in section 7 has a key that ends `suunto|1001`.

Vendor originals have a total size bound. `MAX_VENDOR_ORIGINAL_BYTES` in
`src/input_policy.rs` is 128 MiB. It counts the bytes of `record_json` in
every revision of every vendor original, including a deleted one. A native
package writes these records in its main document, `athlete.ath.json`,
with every other record that is not a measurement sample. The current
record is under `records`, and each revision is under `record_revisions`.
Only measurement samples go to the `series/` files. So each original is in
that one document at least twice. Version 2.1.1 reads a package document
only up to `MAX_PACKAGE_DOCUMENT_BYTES`, which is 512 MiB, and a mapped
import file only up to `MAX_MAPPED_INPUT_BYTES`, also 512 MiB. The bound
keeps the originals near 256 MiB of the main document, and the rest is
left for every other record. A saved file of current envelopes is at most
128 MiB plus its JSON wrapping, so it fits the mapped-import cap.

The matching step of a page with `vendor_workout` rows adds the page's
new and changed envelopes to the stored total. A page that would pass the
bound is refused. It writes nothing, its objects count as `size_cap` in
`left_out`, and that provider is `incomplete`. The line says the stored
vendor originals would pass 128 MiB, so a package holding them could not
be read back. `ath import --mapping open-wearables` uses the same bound.
A Polar detail with samples and a route for a one-hour outdoor workout is
a few hundred kilobytes, so the bound holds several hundred such workouts
with one revision each. Deleting an original does not lower the total,
because its revisions stay. `ath erase` removes the record and its
revisions, so it does. Storing originals in their own package files is
later work, in section 9.

`ath export` in 2.1.1 writes the main document and the series files with
no size check, so it can publish a package that `src/package/load.rs`
refuses to read. The build adds that check to `src/package/export.rs`.
Export counts the bytes of each document it writes into its staging
folder. When one passes `MAX_PACKAGE_DOCUMENT_BYTES`, export fails before
the rename, removes the staging folder, publishes nothing, and names the
document that was too large.

The pull writes one provider's originals as one page, after that
provider's list and detail calls finish. The page is one preview at one
database revision. With `--yes` and without `--json`, it is then one
`ProviderPreview::apply`, which refuses a moved revision as every 2.1.1
apply does. A stored page and an archive never hold a vendor original, and
a vendor original is never a capsule. A vendor page that fails or is
refused leaves that provider `incomplete`. It does not change a stored
page or an archive in the same command, and a refused stored page does not
change the vendor page.

The vendor page uses the 2.1.1 keyed rules. If the person deleted an
original, a later pull leaves that object out, says so on its line, and
does not recreate it (D182). That happens on every later pull. If the
person deleted a provider's `vendor_workout` source, every object from
that provider is left out (D190). If the person corrected an original with
`ath correct`, the owner-correction rule above applies. A later pull
whose envelope equals the newest `provider_import` revision adds nothing,
and the correction stays current. A later pull whose envelope differs
replaces the correction and counts it in `owner_corrections_replaced`.
The person's revision stays in the history. To keep a pull from changing
an original at all, the person deletes it. The deleted original then
keeps the person's version in its history, and every later pull leaves
that object out. A workout fix belongs on the stored session or in a
note, not on the vendor original. The skill says so (section 6).

Each provider's line, and its object in `--json`, counts:

- `originals`: new, changed, and unchanged records.
- `owner_corrections_replaced`: owner corrections that this page replaces,
  by the owner-correction rule above. Each is also counted as a changed
  original.
- `details`: `stored`, `kept`, `deferred`, `failed`, `mismatch`,
  `thinner`, and `not_available`.
- `left_out`: objects not written, by reason: `missing_id`,
  `unusable_id`, `unsafe_number`, `duplicate_id`, `superseded`,
  `deleted_record`, `deleted_source`, `response_refused`, and `size_cap`.
- `withheld`: values that section 4 withheld, with up to 20 paths.

Each listed object is in exactly one detail count or one left-out count.
A list response that fails or that the privacy scan refuses stores nothing
from that response. Its objects count as `response_refused` when they
could be read, and the provider is `incomplete`.

The original is not a workout session. The stored session stays matched
by the Open Wearables workout id. The public stored workout has no
`external_id`, so the two records are not joined by time, distance, or
device. A device id inside the vendor object does not set
`device_binding`. Nested samples stay inside the original. The eight
range calculations do not read it.

The rewritten `mappings/open-wearables.md` gains a section named
`vendor_workout`. It declares the record type `source_report`, kind
`original`, and subject `vendor_workout`. It names the writer
`open_wearables`, the sensor from `provider`, the key from `provider` and
`vendor_id`, and the start from `start` and `start_kind`. It keeps the
whole envelope in `supplied_fields`. A saved file of envelopes, under
`vendor_workout`, imports through that section with `ath import --mapping
open-wearables`. File import does not apply the detail rules above. It
writes each envelope as the file holds it, as D182 does for every mapped
import. Export and import keep one original id.

The provider table, the detail rules, and the thinner check live in
`src/runtime/vendor_workouts.rs`. They run before the mapping engine sees
an envelope, so the mapping engine has no vendor exception.
`open_wearables.rs` calls that module. It does not grow a provider branch.

Day rows are one selected row per source, series, and local day. The
repository prefers `computed_by: provider`, then the later provider row.
With the sample fallback on, a missing day can be computed while reading.
The returned `timestamp` and `period_end` come from `CalendarPeriod`, not
from the provider interval that was stored. `stats` and `computed_by` are
kept. They are the server's window receipt, not the original provider
period.

## 3. How a response becomes records

The pull resolves `open-wearables` through `resolve_mapping` in
`src/assets/mod.rs`. It parses and
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

Everything after that is the mapping engine that 2.1.1 carries over from
2.1.0: match keys (D181), replace on change (D182) except for the owner
corrections section 2 keeps, source and device
(D183), one write transaction per page (D189), a deleted source left out
(D190), and field names refused when they are empty or `__proto__` (D191).
A Garmin file import remains a different `data_route`, so both copies of
one workout stay (D161). A second pull of the same Open Wearables object,
or an `ath import --mapping open-wearables` of a saved copy of that object,
keeps the same rows. An identical vendor original changes no row. A
changed object replaces the stored one. The mapping engine has no vendor
exception. Section 2's detail rules decide which vendor envelopes reach
it.

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

Both `ath import --mapping` and `ath pull` go through `preview_route` and
`ProviderPreview::apply` in `src/import/provider.rs`. The check lives in the
matching step there, for route `open_wearables` only. It reads its facts in
the read transaction, at the preview's database revision, and decides
before a staged plan can change a device source or a timeseries record on
that source. The apply refuses a moved revision, so the write sees the
facts the check read. Summary, event, score, and unattributed sensors do
not enter it. Other routes are not checked. A pull-only check would leave
import-after-pull able to replace the point. The check reads every
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
  row may repeat unchanged or correct the stored value under D182 and the
  owner-correction rule in section 2. The binding stays. Hardware is not replaced with a different id. Other fields
  already on the source stay.
- The source has an established binding, and the incoming id is different,
  missing, null, or blank. The call is refused before any mutation. Equal
  point values with a different id are still refused. A new timestamp does
  not make the new id acceptable. The text says the replacement device is
  blocked, including when the sample time is new, and that running the
  command again cannot change the id.
- The source exists, the source key is absent, and any history exists.
  History includes a deleted record and a deleted source device, not only
  live rows. `existing_source` in `src/import/provider.rs` keeps
  `records.is_deleted = 0`, so it is not
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
  that accepts no rows creates no binding. A refused call is not applied,
  so it creates no source and binds none.

A second device is refused rather than merged. That refusal is not
archived. On `--yes`, an earlier committed page stays, the failed page
makes no database changes, the range is not complete, and the exit is
nonzero. File import refuses the whole call and writes nothing.
`--json` and `--json --yes` make no database changes. Device reads
that have `filter_by_priority` send `false`. Summary routes have no such
switch. Section 2 states that limit.

Preview keeps two facts in memory for this command, for each source sensor:
the binding it would have stored, and an unbound source it would have
created once that source has accepted history. A summary sensor does not
supply a binding for the device sensor. The pull passes those facts to the
next preview page's matching step, including a later page that adds an id
after an id-less sleep page. The memory is discarded with the process. It
does not hold null-source payloads, so a preview does not use it to judge
those conflicts. It does not change the estimated record counts. A second
command's matching reads the committed source.

Owner and library writes store a whole source record and do not run this
device-id guard. In 2.1.1 they are `ath correct` in
`src/cli/commands/edit.rs`, the protocol operations `records.write`,
`records.write_changes`, and `records.correct` in
`src/protocol/ops/records.rs`, native package restore, and a package merge
the person accepts, both in `src/package/restore.rs`. `ath correct` and the
protocol operations store the source `data` the caller supplied, after the
schema check. Native package restore copies the package's records. A
package merge the person accepts for a changed source stores that source
record. A 2.2.0 record keeps `device_binding` when the
record has it. A 2.1.0 record does not gain the key, and a supplied field
of the same name stays a supplied field. The next mapped pull or mapped
import then uses the source key that those operations left behind. A 2.2.0
export and restore keeps the key and the import keys, so the same id can
correct a point and a different id is refused. No signature is added. A
person who replaces the database file is outside this rule.

`Store::apply` in `src/store/write.rs` writes a whole source record. There
is no partial `revise`. In 2.1.1, every `Store::apply` caller that can
write a source is one of the owner and library writes above. The one
automatic source writer is `ProviderPreview::apply`. A keyed import there
only creates sources. It never revises one. So the binding rule is
enforced in `ProviderPreview::apply`. A staged source for route
`open_wearables` may carry `device_binding` only as the checked rule above
set it. The first bind of an existing source is a source correction in
that plan, with the source's expected revision. No other staged change may
set, change, or clear `device_binding`. Such a plan is refused, and the
source stays unchanged. Other record types are unchanged.

The store connection is private (D223). `Run::source` in
`src/import/mapping.rs` asks `CandidateSink::route_source` in
`src/import/provider.rs` for a source by writer, route, and sensor. That
call takes no receipt. The mapping receives a provisional source id. It
does not hold the source JSON, and it does not read the database. The
facts come from the matching step. A later automatic write cannot fill
the receipt in, because `ProviderPreview::apply` refuses any staged change
to `device_binding` that the checked rule did not make.

`src/import/provider.rs` therefore gains these source reads and writes,
besides the typed writes in section 6. It gains nothing beyond those:

- In matching, read the committed receipt, and whether any record or
  source device for that source exists, including a deleted row.
- In matching, read the sources, records, import keys, and archive keys
  named by one page, including tombstones and the current revision. This is
  the safety snapshot in section 2. It does not scan the database.
- In the staged plan, create a source with the checked receipt already in
  its JSON.
- In the staged plan, write the checked receipt on an existing source only
  when the receipt is absent and the same matching read shows no accepted
  or deleted record and no source device. Every other source field stays.
  The write is a correction with the source's expected revision. Writer,
  route, sensor, import keys, and baselines stay.
- In `ProviderPreview::apply`, refuse any other staged change to
  `device_binding`.
- In the sink, stage a `connector` source. `route_source` stages only an
  `export_file` source that names a vendor, and it reuses only a staged
  `export_file` source. The writer-`open_wearables` sensors in section 2,
  the archive source, and each `vendor_workout` source are `connector`
  sources with no vendor. So a new `connector_source(writer, sensor)`
  goes beside `route_source`. Matching finds its stored source by writer,
  route, and sensor, including a deleted one, as `keyed_existing_source`
  already does. A device source still comes from `route_source`.
- In the preview, count the staged plan by record type and action.
  `Staging::count` in `src/import/staging.rs` counts by action only. A count
  by record type goes beside it, and `ProviderPreview` exposes it. Each
  page's counts in section 1 come from that count.
- A crate-private read of a stored vendor original, for the detail rules
  in section 2.
- In matching, for route `open_wearables` only, the owner-correction rule
  in section 2. `KeyedPolicy` gains a flag that only that route sets.
  `record_by_import_key` also returns the current revision's basis. For an
  owner correction, matching reads the newest `provider_import` revision of
  that record and compares the candidate with it. The page's report counts
  each replaced correction.
- In matching, for a page with `vendor_workout` rows, the size bound in
  section 2. It sums the stored vendor-original revisions and refuses the
  page when its new and changed envelopes would pass
  `MAX_VENDOR_ORIGINAL_BYTES`.

The pull reads the revision a preview matched at from
`ProviderPreview::destination_database_revision`, which is already public.
No accessor is added for it.

The reads run in the matching step's read transaction. The writes run in
the page's one apply transaction. A refused call writes none of them.
Other routes do not call them, and `route_source` is unchanged for them.
It does not add a table or a trait. The
binding check and the safety snapshot go in one new file,
`src/import/binding.rs`, because `src/import/provider.rs` is already 1297
lines. The automatic path does not replace `detail`, `vendor`, or
`supplied_fields`.

`zone_offset` is not copied into `start_at`. It remains in `supplied_fields`
when the server sends it. The timestamp text is the server's `start_time`,
`end_time`, or `timestamp`. A timestamp with no offset is left out by the
mapping. The pull does not add `Z`.

`computed_by`, `window_seconds`, `is_nap`, `intensity`, and `efficiency_percent`
stay under those names when they are present. `computed_by` sets the basis
in section 2. It does not change the writer. Raw samples on 0.11.0 are
built without that field. Day rows carry it, and those rows are stored.

## 4. Credentials, privacy, and the network

D229 is the rule. The key never enters the athlete database, the export, the
preview, or an error line. The pull reads the paths in section 2. Connections and the two live
workout routes are included only when `--vendor-workouts` is on.
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
the key cannot be sent to a second host. One attempt per request, then
stop. The stored reads use 30 seconds to connect and 120 seconds to read,
as `src/runtime/garmin_pull.rs` does. A live vendor request uses 30
seconds to connect and 360 seconds to read, for the reason in section 2.
The timeouts are named constants in `src/runtime/open_wearables.rs`. One
response may be at most `MAX_PULL_RESPONSE_BYTES`, which is 10 MiB. One
read may be at most 1000 pages. Those limits are named constants in
`src/input_policy.rs`, where 2.1.1 keeps every limit on untrusted input.
In 2.1.1 the comment on `MAX_PULL_RESPONSE_BYTES` says it is the bytes of
one Garmin personal connection response. The route list at the top of
that file's route limits names only the Garmin pull. The build rewrites
that comment to name both pulls, and it adds the Open Wearables pull and
its new limits to that list. The value stays 10 MiB.

The body is read inside that cap and parsed before the page reaches
`MappedPage`. The guard is in `open_wearables.rs`. It walks every decoded
object key and every decoded string. A value that contains the API key,
the selected origin, or the selected user id rejects the whole page, even
under a credential-shaped key.

A credential-shaped key does not reject the page. Its value is withheld.
A key is credential-shaped when its letters and digits, with case ignored
and every other character dropped, end in `token` or `secret`, or are
`apikey`, `authorization`, or `password`. That covers `userAccessToken`,
`access_token`, `refresh-token`, `id_token`, `session_token`, `token`,
`embed_token`, `client_secret`, and `apiKey`. The value is replaced by the
text `[withheld by ath]`, whatever type it had. A string that is an
absolute URL keeps its address. Each query parameter in it whose name is
credential-shaped has its value replaced by the same text. The pull
records the JSON path of each withheld value and counts them. It prints
the count and up to 20 paths, and it says when that list was shortened.
Nothing else in the object changes. The rule applies to every page,
stored reads and vendor reads alike. Garmin's summary objects can carry
an optional `userAccessToken`. The pinned server declares it on its Garmin
wellness models, and the live route returns Garmin's objects unchanged.
Refusing the page would refuse every Garmin page that carries it.
`polar_user` and a route coordinate are not credential-shaped.
D244 keeps the rest of the vendor object, including account ids, Polar
user URLs, and full routes. The matching personal-record `user_id` is the
only field of a stored read that is dropped before this walk. Any other
copy of the user id still rejects the page. The connections list is not
walked, as section 2 says.
A raw personal-record `user_id` that is absent, null, the wrong type, or
not the selected user refuses the page before the drop. The page is not
rewritten and it is not mapped. A Unicode escape such as `\u0073` is visible
only after parsing, so a scan of the raw text is not enough. The raw-text
scan still runs on a body that did not parse, and the command still refuses
to print the key. Those checks are defense in depth. They do not repair a
page that has already committed.

Printed failure text is a fixed sentence plus the HTTP status. A server
problem `code` is printed only when it is a single token of letters, digits,
`_`, or `-`, at most 64 characters, and it does not contain the key, the
origin, or the user id. Any other code is withheld. A transport error or a
parser error is replaced with a fixed sentence. The sentence has no URL, no
path, and no user id. The result keeps a `left_out` total, a count for
each reason, and at most 20 example lines. It does not keep every id or
every source string. The command prints from this process, as every 2.1.1
command does. The 20 example lines are the bound.

There is no Athletic Standard telemetry on this command. The server the person
named is the only host contacted.

A deployment API key can read every user on that server. The command cannot
shrink that key. It can avoid using the extra power: one user id, the
reads in section 2, no user list.

## 5. Version, migration, and what stays compatible

The build sets `2.2.0` on the package, the record, and the
`athleticstandard` crate, by the same constants 2.1.1 uses.
`ATHLETIC_STANDARD_VERSION` is in `src/model/package.rs`. `SQL_SCHEMA_VERSION` is in
`src/store/migrate.rs`. Migration step 6 is a new SQL file and a narrow
runner arm. Step 5 is already
`spec/sql/migrations/005-version-2-1-1.sql`. Step 6 updates
`athleticstandard_version` to `2.2.0` and `sql_schema_version` to 6. No
column is added to an existing table. The contract adds
`source_report` to the `records.record_type` check, creates
`source_reports`, and rebuilds `provider_summaries` and
`provider_scores` as section 2 describes. Files `001` through `005` stay
byte-identical. The check is not removed, and ordinary store writes do
not turn foreign keys off.

SQLite cannot change that check in place. `records` and
`record_revisions` reference each other. The link from a record to its
current revision is deferred. The link from a revision back to the
record, and the links from typed rows, import keys, deletions,
attachments, reporting periods, and the other child tables, use
`ON DELETE CASCADE` and are not deferred. Triggers and views name
`records`. `PRAGMA foreign_keys` does not change inside a transaction.
`configure` in `src/store/mod.rs` requires foreign keys on before
`install_schema` runs. A `DROP TABLE records` inside that transaction
would fail or cascade through the children. Step 6 does not do that.
`write_transaction` in the same file refuses to write when foreign keys
are off. `Store::transaction` uses it. `ath migrate` and
`apply_pending_without_backup` run steps 2 through 5 through
`Store::transaction`. A new database does not. `Store::create` opens a
connection, runs `configure` and `enable_wal`, and calls `install_schema`
on that connection before any `Store` exists. `install_schema` opens its
own transaction with `rusqlite::Transaction::new_unchecked`.

Today `install_schema` runs the whole chain, steps 1 through 5, in that
one transaction. It sets `user_version` to `SQL_SCHEMA_VERSION`. Then it
refuses unless the metadata reads `SQL_SCHEMA_VERSION` and
`ATHLETIC_STANDARD_VERSION`, in `src/store/mod.rs` lines 625 through 629
at `0de71bb`. `apply_steps` in `src/store/migrate.rs` has the same kind of
final check, in lines 321 through 326. It refuses unless the last step it
ran reached `SQL_SCHEMA_VERSION`. When the constant becomes 6 and steps 1
through 5 commit at schema 5, both checks would fail. So the build changes
both. The first `install_schema` transaction stops after step 5, sets
`user_version` 5, checks for schema 5 and `2.1.1`, and commits.
`apply_steps` checks that it reached the last step it was given. The
step-6 function makes the final check before it commits: schema 6,
`2.2.0`, and `user_version` 6.

Step 6 is one function in `src/store/migrate.rs`. It takes the
connection and opens its own transaction. At init it runs on the
connection `Store::create` opened. On migrate it runs on the store's
connection. It does not use `Store::transaction`. Before `BEGIN IMMEDIATE`, it sets
`PRAGMA foreign_keys=OFF` and reads the pragma back. The value must be 0. Turning foreign keys off does not turn off the schema
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
9. Set the version rows and `user_version` 6.

`sql_v1_database_info`, `sql_v1_record_revisions`, and
`sql_v1_record_deletions` do not name `records`, so they stay through the
gap. Then the transaction commits. After commit, and after a rollback or
an error, the runner sets `PRAGMA foreign_keys=ON` and reads it back. The
value must be 1. If it is not, the command fails and closes the store.
`write_transaction` would refuse a later write on that connection anyway,
and the next open turns foreign keys on in `configure`. A failure before
commit restores the views, the
cross-table triggers, the old check, the rows, and the version. The D124
backup is already on disk. Device receipts stay in the copied source
JSON. They are not rewritten.

A new database commits migration steps 1 through 5, then runs step 6.
Item 9 in the list above sets `user_version`. If step 6 fails during init, that transaction
rolls back to the committed step 5 schema before the existing failed-init
cleanup runs. Init reports failure. `Store::create` still calls
`remove_partial_database`, which deletes the new file, the wal file, and
the shm file. D231 leaves that cleanup in place, and this build does not
change it. A file that cleanup does not delete is not left with the views
dropped. A migrate of a database that already existed is a separate case.
`apply_after_backup` and `apply_pending_without_backup` in
`src/store/migrate.rs` run the steps up to 5 in `Store::transaction`, as
2.1.1 does, and commit. They then run step 6 in its own transaction. For a
2.1.1 database, a step 6 rollback leaves the file at schema 5 with foreign
keys on. For an older database, the earlier steps stay committed, so the
file is now a 2.1.1 database. The message says which schema the file is
at and names the verified backup. Running `ath migrate` again runs only
step 6. `apply_step` keeps its signature for steps 2 through 5. Step 6 is
a separate function in that file.
`MIGRATION_CHAIN` in that file and `spec/sql/contract.json` gain the step.
The native tests check that the contract matches the embedded SQL.
`packages/npm/package.json` exports and `packages/npm/test/public-api.test.ts`
point at `schema/2.2.0/`.

`READABLE_DATABASE_VERSIONS` and `ACCEPTED_RECORD_VERSIONS` become `1.0.0`,
`2.0.0`, `2.1.0`, `2.1.1`, and `2.2.0`. Both sets already contain `1.0.0`.
Dropping it would make a 1.0.0 database unloadable, so migrate could not
open it. `IMPORTABLE_PACKAGE_VERSIONS` is `2.0.0`, `2.1.0`, and `2.1.1`
today. It gains `2.2.0` and keeps those three. `Store::open` rejects a
`2.1.1` database until `ath migrate`, with the refusal it already uses for
an older database. A `2.1.1` database will not open until that migrate.

The build generates `schema/2.2.0/` and does not rewrite `schema/2.1.1/`,
`schema/2.1.0/`, or `schema/2.0.0/`. The 2.2.0 source object gains optional `device_binding`.
`ProviderReportingPeriodType` gains `open_wearables_reporting_window`.
`ValueBasis` gains `aggregator`. Series quantities gain the sample series
whose units are in the appendix table. Intake, index, age, and effort
series do not gain quantities. The record type `source_report` is added to `RecordType` in
`src/model/`, the current and revision unions, the package schema, and the
table description in `src/store/encode.rs`. `src/store/check.rs` compares
stored rows with that description. It is not a measured type. Its table has `record_id`, `source_id`,
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
regenerated because the stored version is inside it. The demo is
`examples/demo-athlete/athlete.ath.sqlite`. The preserved 2.1.0 demo in
`tests/fixtures/version-2-1-0/` is not regenerated. The 0.4.3 file in
`tests/fixtures/version-0-4-3/` is not regenerated. `tests/fixtures/version-2-0-0/` is not
regenerated.

`build.rs` compiles every file under `mappings/` into the program, and
`src/assets/mod.rs` reads the compiled copy. No package list changes. The
build rewrites `mappings/open-wearables.md` so it implements section 2. Other mapping
files stay as they are. `ATH_WRITE_GENERATED=1 cargo test --test generated`
runs because help text, the schema, and the demo change (D80, and the 2.1.1
generator decision). `AGENTS.md`, `build-history/README.md`, and `SPEC.md` are
updated in the build so they name 2.2.0 as the software. `docs/connections.md`
keeps the file imports and is updated to the coverage in section 2, including
that `RecoverySummary` at the pinned commit has no skin-temperature field.
This
plan's edits to `AGENTS.md` and `build-history/README.md` only point at the
plan.

A database from 2.1.1 needs `ath migrate` before the pull. The migrate
command still previews, backs up, and asks (D124). `Store::open` only
refuses. A noninteractive open and an open with `--json` do not migrate. An
interactive open can preview, back up, ask, and then migrate.

## 6. Where the code goes

| Piece | Place |
|---|---|
| HTTP, timeouts, page loop, envelope split, privacy walk, and withheld values | `src/runtime/open_wearables.rs` |
| Live provider table, unwrap, windows, pacing, detail budget and rules, and the vendor envelope | `src/runtime/vendor_workouts.rs` |
| Command `pull openwearables`, and the `--date` check for `garmin` | `src/cli/spec.rs` and `src/cli/commands/routes.rs` |
| Flags, key, preview text, refusal if the key appears | `src/cli/`, `src/cli/help.rs`, `src/runtime/keyring.rs`, and `key_clear` in `src/cli/commands/model.rs` |
| Reader for one decoded page | `MappedPage`, beside `MappedReader` in `src/import/mapping.rs` |
| Mapping names and field rules, including the `vendor_workout` section | the existing file `mappings/open-wearables.md` |
| Stage, match, and write each page | `preview_route` and `ProviderPreview::apply` in `src/import/provider.rs`. The revision a preview read is the public `destination_database_revision` |
| A `connector` source with no vendor | new `connector_source` beside `route_source` in `src/import/provider.rs` |
| Page counts by record type | a count beside `Staging::count` in `src/import/staging.rs`, exposed by `ProviderPreview` |
| Owner-correction rule and its count | `match_keyed` and `record_by_import_key` in `src/import/provider.rs`, behind a `KeyedPolicy` flag that only route `open_wearables` sets |
| Where a workout fix belongs | one paragraph in `skills/athletic-standard/SKILL.md`, under "Stable IDs and revisions" |
| Total size bound for vendor originals | `MAX_VENDOR_ORIGINAL_BYTES` in `src/input_policy.rs`, checked in the matching step in `src/import/provider.rs` |
| Export refuses a document over the package read cap | `src/package/export.rs`, before the staging folder is renamed |
| Device-binding facts and the safety snapshot | new `src/import/binding.rs`, called from the matching step in `src/import/provider.rs` for route `open_wearables` |
| Create or first-bind with the checked receipt, and refuse any other `device_binding` change | the staged plan and `ProviderPreview::apply` in `src/import/provider.rs` |
| Stored-original read for the vendor detail rules | a crate-private read in `src/import/provider.rs` |
| Response cap and page cap, and the comment and route list that now name both pulls | named constants in `src/input_policy.rs` |
| Receipt for a start in Unix seconds or milliseconds | a helper beside `offset_timestamp_receipt` in `src/import/candidate.rs` |
| Measurement samples, source reports, paired null value and unit, and child withdrawal | `write_current_state` and `typed_columns` in `src/store/encode.rs`. Package restore, repair, and the record check use that encoder |
| Step 6 with foreign keys off | its own transaction in `src/store/migrate.rs`, and `install_schema` in `src/store/mod.rs` |
| Whole source record from the owner or a library caller | existing `ath correct`, protocol `records.write`, `records.write_changes`, and `records.correct`, package restore, and an accepted package merge |

`src/store/`, `src/import/mapping.rs`, and `src/runtime/garmin_pull.rs` are
not the home for this client. `garmin_pull.rs` remains the Garmin pull.
The two clients may each call `ureq`.
A shared trait is the later connector work, and this release does not add it.

The build adds this paragraph to `skills/athletic-standard/SKILL.md`,
under "Stable IDs and revisions": "Fix an Open Wearables workout on its
workout session, or write a note. Do not correct the vendor original. It
is the vendor's own copy. A later pull keeps your correction until the
server's data for that record changes. Then the pull replaces it, counts
it, and keeps your correction in the record's history." This plan pull
request does not edit the skill, because the skill describes the
released version.

`mapping.rs` changes for `MappedPage` and for the preservation writes in
section 2: the measurement-sample target, the reporting-window write, the
segment copy, and the `vendor_workout` section. HTTP stays out of
`mapping.rs`. The mapped-import report from `report_value` in
`src/import/provider.rs` keeps its `outcome` field. The pull counts each
page by record type from its staged plan. For route `open_wearables` only,
the matching step runs the device-binding check in section 3 before a
staged plan can change a source device or a timeseries record. That check
is the shared mapped-import and pull enforcement. It is not a new match
key and it does not apply to any other route. The matching step also
reads the page's safety snapshot: the sources, records, import keys, and
archive keys named by that page, including tombstones and the current
revision. It does not scan the database. `ProviderPreview::apply` refuses
any `device_binding` change the checked rule did not make. The store
inserts and updates `measurement_sample` and `source_report`, and it
accepts a provider
summary or provider score whose `value` and unit are both present or both
absent. A half-null pair is refused. When an anchor in this response no
longer emits a child, that child is revised to the absent pair in the same
transaction. `write_current_state` in `src/store/encode.rs` writes that
pair and the new table. Package restore, repair, and the record check use
that encoder. `RecordType` in `src/model/` gains the `source_report` entry.
The eight range calculations, `provider-period-summary` among them, do not
read `source_reports`. `read_inputs` and
`read_period` also exclude `source_type` `manual`, and they have no
`include_manual` switch. `workout-drift` loads the named session first,
including a manual session, and uses that session's start and end as the
window. Its samples then go through `read_inputs`. A manual source's
numbers stay out. `query_measurements`, `query_series`, and
`query_sessions` in `src/store/query.rs` omit manual sources unless
`include_manual` is true. The JavaScript names in
`packages/npm/src/database.ts` call those functions.
This proposal does not change that code. A generic record listing is
not those three functions. In
`read_inputs` and `read_period`, a provider summary or provider score
whose `standard_value` or `standard_unit` is null is excluded in the SQL,
before the limit, the count, the grouping, and the numeric decode. A null
is not stored as zero and is not replaced by an older revision. The
session-summary branch of `read_period` uses the same filter on the
extracted number and unit. Formulas and source grouping stay as they are.
The archive is the pull-only preview and apply in section 2. A mapped file
import does not build it. It goes ahead only at the database revision the
protected preview read, and its apply refuses a moved revision. It does
not call the receipt operations. No other store behavior changes. The
owner and library writes in section 3 stay whole-record writes. They are
named there so the binding survives a 2.2.0 round trip. They are not a
new importer. The pull sums only `sleep_session`, `workout_session`,
`point_measurement`, `measurement_sample`, `provider_summary`,
`provider_score`, and `source_report`. A committed protected page adds
those counts to `committed`. A committed archive page adds its counts
only to `archive_committed`. A preview page adds its protected estimates
to `estimated` and its archive estimates to `estimated_archive`. It does
not add `outcome` across pages. `outcome` counts the source-device row in
the same group. Filtering that row out of the totals does not make a
preview equal a write. A preview page writes nothing, so page two does not
see page one. A vendor page is a protected page. Its originals add to
`committed.source_report` and to the vendor counts.

The command reads the saved key and `OPEN_WEARABLES_API_KEY` in this
process, in `src/runtime/keyring.rs`. It has no `--key` flag. The
decoded-key walk in section 4 runs before `MappedPage`. The command
checks its own text before printing.

The command returns the preview outcome from section 1 when the mode is a
preview, including public `--json`. It returns the confirmed outcome when
the mode is `--yes` without `--json`. On a partial failure it keeps that
confirmed outcome and then exits nonzero. It prints text from the confirmed
outcome and public JSON from the preview outcome. The success sentence is
printed only after `--yes` when `range_complete` and `snapshots_complete`
are both true. There is no child process.

The program runs on Linux, macOS, and Windows with the same command. The
fixture server listens on `127.0.0.1` and an ephemeral port, which is the
pattern `src/runtime/garmin_pull.rs` already tests. Mapping paths stay
`resolve_mapping` in `src/assets/mod.rs`. No new installer and no new
system library.

## 7. Tests

CI has no Open Wearables account, no API key, and no Docker Compose. The
acceptance tests are the fixture server and the synthetic files already in
`tests/fixtures/exports/open-wearables/`. Those files are on the public-data
list. A new file, such as a two-page response or the two-window Garmin
fixture below, is synthetic,
invented, and added to `tools/public-data-allowlist.json` with its path
and SHA-256. `tests/public_data.rs` enforces that list. No real
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
  that is still over the response cap. Page 1 remains. Page 2 adds no rows and no revisions. No later read
  runs. The exit is nonzero. The success sentence is absent. The text gives
  the retained counts. The same command run again stores no second copy
- the same failure sequence with no `--yes`, and again with `--json --yes`,
  makes no database changes. Rows that were already stored stay. A
  successful `--json --yes` of a readable range is also a preview and makes
  no database changes
- an installed mapping that does not implement section 2 fails before any
  request. The server sees no request. Raw and day timeseries requests omit
  `types`. A comma-joined type string is not sent
- a cursor or offset page fails before `MappedPage` when the object is
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
  `hr_recovery` in bpm. A finite zero and a finite negative value are
  stored. Import, a repeat pull, and the package export and import keep
  the quantity, the unit, the key, and the value. A heart-rate point of
  zero is still left out. A negative `temperature_deviation` is still
  stored. A 2.1.0 mapped file and this pull, in either
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
  A 2.1.0 raw point and a
  pull of that same raw point stay one record. A heart-rate sample from
  device B does not replace device A's sample. A negative latitude and a
  negative longitude are stored
- a response larger than the response cap on a cursor or offset read is
  requested again with a smaller limit, the same cursor or offset, and the
  rows are stored. A day page at `limit` 1 that holds many small rows and is
  still over the cap leaves that read incomplete and names the read and
  `resolution=1day`. It does not name a series type. An oversized body
  response names `body` and writes nothing. The export after a pull, and a
  native package import, keep period bounds, units, `computed_by`,
  segments, score components, meal nutrients, and a pregnancy snapshot
- a summary shaped like `summaries_service.py`, with no `device_id`, is
  stored after a bound raw page. The device receipt and the device's
  points stay. The same summary stored first does not block a later raw
  page from binding its device source. File import then pull, pull then
  file import, and import then import keep one summary id. A refused
  device page refuses only that page. Two meals with one timestamp and
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
  and is not archived. The next page is still refused on its own failure.
  A measured timeseries row with
  `source: null` is stored on the unattributed sensor when the payload
  matches. A reported row with `source: null` uses the report sensor
  only. A different payload with the same full key, including
  resolution and `period_end`, refuses the protected write and archives
  the new original. A mapped file import of that changed payload writes
  nothing. A raw midnight row and a day window that starts then
  stay two records. A preview of those two pages does not claim the
  conflict, because the first preview page wrote nothing. A confirmed
  second page does.
- a missing device id beside a conflicting device id, in either row order,
  refuses the protected page and stores no capsule and no revision. The
  same veto holds for an unbound legacy history and a missing workout UUID
  on the same page as an eligible row. A deleted source on a page with an
  eligible row leaves that source's rows out with the D190 line. The
  eligible row still refuses the protected page, and the deleted source
  stops the archive, so no capsule is stored. The API key, the server
  origin, or the selected user id on a neighboring object builds no
  preview. A deleted archive source is not recreated. An existing device
  receipt is unchanged. On an empty
  database, the first eligible archive sets
  `written` false, `database_changed` true, `archive` `stored`, and the
  text `Protected write refused; N originals archived; database changed;
  pull incomplete.` A repeat on that empty database sets `archive`
  `unchanged`, `written` false, `database_changed` false, and the text
  `Protected write refused; 0 originals archived; database unchanged; pull
  incomplete.` A third payload adds one archive count and
  leaves the earlier capsule. An archive apply failure on that
  empty database sets `archive` `failed`, keeps the refusal, adds no
  archive row, leaves `written` false and `database_changed` false, and
  uses the text `Protected write refused; archive failed; database
  unchanged; pull incomplete.` The same four results after an earlier
  committed sleep page keep that page's `written` true, its `committed`
  counts, and `database_changed` true. The failure text then says
  `database changed`.
  Preview and `--json --yes` apply neither plan
- two connections, both row orders. After the protected preview and before
  its apply, the other connection commits a binding of device C on source
  S. The page has a series for S with device B. The protected apply
  refuses because the database revision moved. `archive` is
  `not_attempted`. No capsule, source revision, or receipt change is
  written. The same refusal holds when the other connection commits a
  tombstone or history on S instead of the binding. A page whose safety
  snapshot has no veto and whose protected preview refused it, then a
  deletion of S before the archive, leaves `archive` `not_attempted` and
  does not archive S's payload. The same holds when the other connection
  deletes the normalized record or its import key, deletes the archive
  source or an existing capsule, or makes an owner correction, between
  the protected preview and the archive apply. Nothing is recreated, and
  no second identity is stored. An already deleted neighbor, in either
  row order, is left out with its line, and the page is not archived. The
  API key on the page builds no preview. An earlier committed sleep page
  keeps `written` true, its `committed` counts, and `database_changed`
  true. The text is `Protected write refused; archive not attempted;
  database changed; pull incomplete.`
- every athlete-data write moves the database revision: `Store::apply`,
  `ProviderPreview::apply`, repair, erase, and a package merge. A restore
  into an empty database sets the package's revision and database ID, and
  the apply refuses another database ID. A test commits each of these
  between a protected preview and an archive apply, and each one makes the
  archive `not_attempted`
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
  A latest value with a null time is stored the same way. A body object
  whose `skin_temperature_measured_at` is a raw-looking instant, and one
  whose timestamp is a day `period_start`, both keep the amount, the unit,
  and the timestamp on the original. The skin-temperature child has no
  `recorded_at`. Import, pull, and the native package roundtrip keep that
  child out of an instant calculation. A raw skin-temperature sample and a
  day skin-temperature window keep their own classes. Age is a source
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
- step 6 keeps an old summary's id, number, unit, `data_json`, import key,
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
- a mapped page never revises a stored source except by the checked first
  bind. A staged plan that would set, change, or clear `device_binding`
  any other way is refused by `ProviderPreview::apply`, and the source is
  unchanged. `ath correct` with a complete source object replaces that
  object, and protocol `records.correct` does the same. After that
  correction, a mapped write follows the source key the correction stored
- export a 2.2.0 database that has an established binding, import that
  package, and the source key and the import keys are still there. The same
  id can correct the point. A different id is refused, with no new point
  revision, source revision, or hardware revision. A stored source key that
  is not a nonempty string refuses the automatic write and is left unchanged
- `page-sleep.json` becomes a sleep session. A manual workout and its
  automatic neighbor from one response stay two records. The automatic
  workout is a session on the device source. The manual workout is a
  workout session on the manual source, with `field_basis.value`
  `entered_by_hand` and no device receipt. A `device_id` on that object
  stays on the original. A workout with `entry_source` `unknown`, null,
  or omitted is an `unknown_workout` report. It is not a session and not
  a manual workout. The same server id arriving later as `automatic`
  refuses the page, and the stored session stays. A later pull of the
  same manual workout updates that session. Export and import keep the
  pair that was accepted. `workout-drift` named on that manual session
  uses its start and end. A wearable sample inside the window is an input.
  A manual sample and the session's own heart rate and distance are not.
  `query_measurements`, `query_series`, and `query_sessions` omit the manual
  session unless `include_manual` is true. That switch is not claimed for
  every record listing.
  `page-raw.json` is stored.
  RMSSD and SDNN stay different quantities
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
- `left_out` in the result is a total, a count for each reason, and at
  most 20 example lines. With 21 lines it says the list was shortened
- a synthetic Polar list without samples, zones, and route, then one
  detail with all three true, stores one original with `detail_status`
  `stored`. The nested samples are not measurement samples. A second
  identical pull makes no detail call for that id, counts it `kept`,
  changes no row, and reports no database change for that original. A
  changed list object whose new detail has an empty `route` writes
  nothing for that id, counts it `thinner`, and keeps the stored original.
  The stored workout with a different Open Wearables id stays a separate
  session. A device id on the vendor object does not set `device_binding`.
  The result text does not print a route coordinate. It does say the
  originals keep account ids, user URLs, and full routes
- a Strava detail with `laps`, `splits_metric`, `segment_efforts`, and
  `map` is stored. A later changed list object whose detail has no
  `segment_efforts` is `thinner` and writes nothing. A Whoop detail is
  never `thinner`, because the table names no Whoop detail field
- a failed Polar detail for a new id stores the list object with
  `detail_status` `failed`. Polar is `incomplete`, and the command exits
  nonzero. The same failure for an id that has a stored detail and a
  changed list object writes nothing and adds no revision. A detail that
  names another id is `mismatch`, is not stored, and also makes Polar
  `incomplete`
- 45 Strava ids with no stored detail. The first pull details the 20
  oldest starts and stores the other 25 with `detail_status` `deferred`.
  Strava stays `completed`, its line counts 25 deferred, and the exit is
  zero when nothing else failed. A second pull details ids 21 to 40 in
  start order. A third pull details the last five. A deferred id whose
  original already has a detail, and whose list object changed, keeps
  that detail, writes nothing, and adds no revision
- an owner corrects a stored Polar original with `ath correct`. The next
  pull with the same server data writes nothing for it, the correction
  stays current, and Polar's `owner_corrections_replaced` is 0. That pull
  makes no detail call for the id, because the stored-original read uses
  the newest `provider_import` revision. After the server's list object
  changes, the preview prints `would replace 1 owner correction; delete an
  original to keep it`, and `--json` has Polar's
  `owner_corrections_replaced` 1. `--yes` replaces the correction, counts
  it as a changed original and as 1 replaced, and keeps the owner's
  revision in the history. A deleted original is left out instead, and
  the count stays 0
- an owner corrects a stored sleep session. A pull with the same server
  data keeps the correction, and the top-level `owner_corrections_replaced`
  is 0. A pull whose session changed replaces it, and the count is 1 in
  the preview, in `--json`, and after `--yes`. The preview says `would
  replace 1 owner correction of stored records because the server's data
  changed`. `ath import --mapping open-wearables` of a file that holds the
  same data also keeps the correction. Another route keeps its 2.1.1 rule
- a record whose current revision came from an accepted package merge is
  an owner correction. A native export and restore keeps every revision's
  basis, so the same pull reports the same counts before and after the
  restore
- without `--vendor-workouts`, preview and `--json` make zero requests to
  the connections route and to both workout routes. `requests_made` is
  false, `notice` is empty, and every live line is `not_attempted`. With
  the flag, preview and `--json` still write nothing, `requests_made` is
  true, and the text says vendor requests were made
- every run with the flag, as a preview, under `--yes`, and with `--json`,
  says before its first live request that the server may refresh and save
  vendor tokens, may revoke a connection, and may wait about 105 seconds
  on a rate limit. `--json` holds those three sentences in
  `vendor_workouts.notice` and is still one JSON value. The help text of
  `--vendor-workouts` and `src/cli/examples/pull.txt` say the same, and
  the generated `skills/athletic-standard/references/cli.md` matches them
- a connections list for the selected user whose items carry `user_id`,
  `linked_user_ids`, `id`, `provider`, `status`, and other fields passes.
  It is not stored, and no other user's id is printed. One item whose
  `user_id` is another user refuses the list, and every provider is
  `not_attempted`
- an active Garmin connection and a revoked Polar connection call only
  Garmin. Polar is `not_attempted` with `connection_status` `revoked`, and
  the fixture server sees no Polar request. `expired` is the same. A
  status the pin does not define prints `unknown status` and is not
  called
- a connections list with only Garmin completes Garmin and marks Polar,
  Suunto, Whoop, Fitbit, Oura, Strava, Withings, and Sensorbio
  `not_attempted`. A failed Polar list does not stop Garmin
- a Suunto list object `{"workoutId": 1001, "workoutKey": "abc"}` calls
  detail at `abc`. The detail's `payload` is unwrapped. One record is
  stored, and its key ends `suunto|1001`. A detail that names 1001 or
  `abc` matches. A list object with no `workoutKey` calls detail at 1001
- a Garmin `activityId` of `123` and a later `"123"` are one record. An
  integer above 9007199254740991 refuses only that object and counts it
  as `unsafe_number`. A fractional id refuses only that object and counts
  it as `unusable_id`. Two different objects with one id are both
  refused as `duplicate_id`
- a synthetic database whose vendor originals, with their revisions, total
  just under `MAX_VENDOR_ORIGINAL_BYTES` exports with `ath export`. The
  main document is under 512 MiB. `ath import` of that package into a
  database made by `ath init --empty` keeps every record id, revision,
  deletion, and import key.
  The restored database passes `ath check`. A vendor page that would pass
  the bound is refused, writes nothing, counts its objects as `size_cap`,
  and makes that provider `incomplete`. A mapped import of the same
  envelopes is refused the same way. An export whose main document would
  pass the read cap, with that cap lowered for the test, fails before the
  rename and publishes nothing
- a synthetic Garmin fixture returns activity 555 in the first window with
  `durationInSeconds` 1800 and in the second window with 1860. One original
  is stored, and its `list` is the second window's object. Garmin's
  `left_out` counts 1 `superseded`, and Garmin is `completed`. The same id
  with equal objects in both windows stores one original and counts
  nothing. The same id twice in one window, with different objects,
  refuses both as `duplicate_id`
- a three-day Garmin range sends four windows, three of 82,800 seconds and
  one of 10,800 seconds. Each window sends `summary_start_time` and
  `summary_end_time` as integer Unix seconds, and each starts where the
  last one ended. A date `--from` starts at midnight UTC. One failed
  window makes Garmin `incomplete` and never `completed`. A later window
  still runs
- a Garmin detail that answers HTTP 404 stores the list object whole with
  `detail_status` `not_available`. Garmin is `completed`, its line counts
  1 `not_available`, and the command exits 0. HTTP 501 is the same. A
  Garmin detail that answers HTTP 500 is `failed`, Garmin is
  `incomplete`, and the exit is nonzero. A Polar detail that answers 404
  is `failed`. A Garmin list window that answers 404 fails that window.
  On the next command, a new Garmin id is detailed before the id that was
  `not_available`
- live requests go one at a time, at least one second apart by the
  fixture server's clock. A live request that times out, with the read
  timeout shortened for the test, stops every later request to that
  provider in the command. The next provider still runs. The 360-second
  value is checked as a named constant
- five Strava ids with no stored detail, where the third detail call
  answers HTTP 429. The fixture server sees no later Strava request. Ids
  one and two store their details. Id three is `failed`. Ids four and five
  are `deferred`. All five list objects are stored. Strava is
  `incomplete`, the next provider still runs, and the exit is nonzero. A
  429 on a Garmin window sends no later window
- a Garmin list object with `userAccessToken` is pulled twice, with a
  different token value the second time. The second pull counts it
  `kept`, makes no detail call, and writes nothing for it
- an empty database is pulled once without `--vendor-workouts`, which
  stores the personal record. A second pull of the same stored data with
  the flag, whose only new rows are two Garmin originals, reports
  `written` true and `database_changed` true, `committed.source_report`
  new 2, and Garmin `originals` new 2. A third identical pull reports both
  flags false
- a Garmin list object with `userAccessToken` stores that original with
  the value `[withheld by ath]`. The record, the export, the text, and
  `--json` do not hold the token. `withheld` counts 1 and names the path.
  The same holds for `access_token`, `refresh-token`, `id_token`,
  `session_token`, `token`, `embed_token`, `client_secret`, `apiKey`,
  `authorization`, and `password`, and for a `token` query parameter in a
  URL string. The URL keeps its address and its other parameters. The API
  key under a credential-shaped key still refuses the page. A `polar_user`
  URL and a route coordinate are stored unchanged
- a Sensorbio body stores `data[].activities[].id`, with the parent entry
  as `list_parent`. A missing id refuses that object, counts it as
  `missing_id`, and keeps the others. File import of the saved original,
  then export, keeps one record id. Sensorbio's line says it reads only
  the oldest 50 entries
- a Polar exercise whose start is outside `--from` and `--to` is stored
  and counted under `vendor workout originals`. `range_complete` still
  follows the stored ranged reads. A Polar `start_time` with no offset has
  no `reported_at`, and its receipt says the source gave no offset. A
  Suunto start in Unix milliseconds keeps the digits and stores
  `reported_at` with milliseconds. Both receipts pass `ath check`
- a Whoop response with `next_token`, an Oura response with
  `next_token`, a Sensorbio response with `links.next`, and a Strava page
  whose length equals `per_page` set `more` true. The fixture server sees
  no `nextToken` sent to Whoop. `covers_requested_range` is false
- a vendor original that the person deleted is left out on every later
  pull, counted as `deleted_record`, and not recreated. A deleted
  `vendor_workout` source leaves out every object from that provider as
  `deleted_source`. An original that the person corrected gains a
  revision with the server's envelope when a later envelope differs, and
  the correction stays in its history. An equal envelope adds nothing
- with the flag on, a run whose protected page is refused and archived
  still writes the vendor page and keeps its counts apart. A refused
  vendor page leaves the archive and the stored pages as they were
- a vendor list response that the privacy scan refuses stores nothing
  from that response, counts its readable objects as `response_refused`,
  and makes the provider `incomplete`. In every vendor test, each listed
  object is in exactly one detail count or one left-out count
- `--json` includes `vendor_workouts.requested`, `requests_made`,
  `notice`, `originals`, and each provider's `status`,
  `connection_status`, `covers_requested_range`, `more`, `limit`,
  `details`, and `left_out`

Migration tests, in the build gate:

- a synthetic 2.1.1 database makes `Store::open` refuse before any network
  call. `ath migrate` previews without writing. It still previews, backs up,
  and asks, as D124 requires. The target it names is `2.2.0`
- a confirmed migrate backs up, then sets the record version to `2.2.0`,
  SQL schema 6, and `user_version` 6. Record ids, revisions, deletions,
  attachments, import keys, reporting periods, and device receipts are
  unchanged. `PRAGMA foreign_key_check` is empty. A deleted row and its
  history are still there. The upgraded database is populated, including
  a tombstone. `sql_v1_current_records` and
  `sql_v1_active_records` still return rows. An insert that would make a
  revision type disagree with its record is still rejected. A second
  migrate does nothing. A failure injected before the step 6 commit
  leaves the old views, the two cross-table triggers, the rows, the
  version, and `foreign_keys` read back as on. A successful migrate
  leaves `foreign_keys` on. A source report can then be created, revised,
  checked, exported, restored, and repaired from `data_json` without
  becoming a measured type
- `ath init` creates a database that is already `2.2.0`, SQL schema 6, and
  `user_version` 6, and `ath check` exits 0. The fresh database accepts a
  source report, the same two views are queryable, and a mismatched
  revision type is rejected. A failed init rolls step 6 back and then the
  existing cleanup removes the new file. A failed step 6 migrate of a
  populated 2.1.1 database leaves that file, schema 5, the views, and
  foreign-key enforcement. Step 5 still
  writes `2.1.1` and schema 5, and `install_schema` commits that
  transaction before step 6. Its check accepts schema 5 and `2.1.1` there.
  A step 6 that is made to stop before it sets the version rows fails the
  step-6 check for schema 6 and `2.2.0`, and init fails. Init runs on the
  connection `Store::create` opened and never calls `Store::transaction`
- `apply_steps` given steps 2 through 5 passes its final check at schema 5.
  Given a list that ends before 5, it still refuses when the database
  stops short of the last step in that list
- a 2.0.0 database whose step 6 is made to fail after steps 3 to 5
  committed is a 2.1.1 database at schema 5. The message says so and names
  the verified backup. A second `ath migrate` runs only step 6 and
  finishes. `Store::transaction` still refuses a write while foreign keys
  are off, and step 6 does not use it
- `ath migrate` re-checks the file, the database ID, the schema, and the
  revision its earlier steps left before step 6. A change made between the
  two transactions stops step 6 and changes nothing more
- databases at `1.0.0`, `2.0.0`, and `2.1.0` still open for migrate. Packages at
  `2.0.0`, `2.1.0`, and `2.1.1` still import. `spec/sql/migrations/004-version-2-1.sql`,
  `spec/sql/migrations/005-version-2-1-1.sql`, and the folders `schema/2.0.0/`,
  `schema/2.1.0/`, and `schema/2.1.1/` stay byte-identical

Rust tests use `MemorySecretStore` in `src/runtime/keyring.rs` and do not
open the real password store. They cover:

- the missing-key sentence
- `ath key set openwearables`
- `ath key clear` with no name, when an Open Wearables key is saved, clears
  `vercel` and `openrouter` only, exits successfully, and prints that the
  Open Wearables key remains saved, with the command `ath key clear
  openwearables`
- `ath key clear` with no name, when no Open Wearables key is saved, keeps
  today's gateway sentences
- `ath key clear vercel` does not remove a saved Open Wearables key
- `ath key clear garmin` is still refused, as in 2.1.1
- `OPEN_WEARABLES_API_KEY` is used before the saved key, as
  `GARMIN_PERSONAL_API_KEY` is for Garmin
- `ath key clear openwearables` removes the saved key and, when
  `OPEN_WEARABLES_API_KEY` is set, says that variable is still available. The
  variable is still set afterward
- `ath key --json` keeps `saved` and `environment` as gateway lists and adds
  `openwearables.saved` and `openwearables.environment`
- a preview `--json` has `written: false`, protected and archive
  committed zeros, `estimated` counts, and `range_complete: false`
- `--json --yes` is a preview. After a later-page failure, and after a
  readable range, neither preview is applied and the database is
  unchanged
- `ath pull garmin` with no `--date` fails with `error: required option
  '--date <day>' not specified`, as in 2.1.1. `ath pull openwearables`
  needs no `--date`. `ath pull other` fails with `pull reads garmin or
  openwearables`

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
`cargo test` pass. In `packages/npm`, `npm ci`, `npm run typecheck`, and
`npm test` pass. That is the gate before the build is called done.

## 8. You test this (Laptop)

The card uses the fixture server the tests use. It does not use a real
account or a real export. The fixture user has a Garmin connection and no
other connection. The card does not pass `--vendor-workouts`, so it makes
no live vendor request. The build starts that server on `127.0.0.1` and an
ephemeral port, prints the port, and uses the fixture key and the fixture
user id. The range is `--from 2026-01-01 --to 2026-01-07`. The person pastes
the fixture key once into `ath key set openwearables`.

1. Setup. Success: `ath init --yes --empty` creates the database, the key
   command saves the fixture key, and the key is not on screen. Failure:
   init fails, or the key is printed.
2. Preview. Success: the pull without `--yes` says nothing is stored, shows
   estimated counts that include a heart-rate sample and a day summary, shows
   a body snapshot line, makes no vendor request, and `ath check` still
   passes. Failure: a row is written, a vendor request is made, or the key
   appears.
3. Write. Success: the same command with `--yes` says the requested range is
   complete, shows the body snapshot line, shows committed counts, and
   `ath check` exits 0. The stored range can be complete with only Garmin
   connected. Failure: the success sentence appears while the
   range or the body snapshot is incomplete, or `ath check` fails.
4. Repeat. Success: `--yes` again reports the stored rows unchanged.
   Failure: a second copy of a sleep session or a point appears.
5. Compatibility. Success: one synthetic Apple, WHOOP, or Oura import still
   passes `ath check`, and `ath pull garmin` with no date still fails with
   `error: required option '--date <day>' not specified`. Failure: that
   import or the Garmin command breaks.

The automated cases in section 7 are not extra card steps.

The build's proof is one phone message. That message contains a
screenshot of each step inline and one video that plays in the conversation.
A video that plays only on the pull request page is not that packet. The
images are not committed. The numbers on screen are the fixture's invented
values. The release workflow on Linux, macOS, and Windows is the
cross-platform proof. The video is one system.

This plan pull request has no screenshots and no video.

## 9. Separate follow-ups

D231 records the check. None of these is a prerequisite for the pull, and none
is in the build pull request.

- Failed `ath init` and failed `Store::create` still delete the database
  file in the same process that closed it, without the abandoned-directory
  cleanup `make_owned_temporary_directory` uses. On Windows that delete can
  fail with `EBUSY`. Version 2.1.0 spec section 12 left it out
  and named no later version. Version 2.1.1 did not take it. A failed step 6
  rolls back before that cleanup. This build does not replace the cleanup.
- `src/import/mapping.rs` is 1703 lines, `src/import/provider.rs` is 1297,
  and `src/store/check.rs` is 1044. The pull does not add HTTP to them.
  `mapping.rs` gains `MappedPage`, the `vendor_workout` section, and the
  preservation writes in section 2. `provider.rs` gains the staged first
  bind, the `device_binding` refusal in `ProviderPreview::apply`, the
  `connector` source call, and the stored-original read. `staging.rs` gains
  the count by record type. The binding check and the safety snapshot go in
  the new `src/import/binding.rs`. `src/store/encode.rs` gains the sample,
  source-report, withdrawal, and paired-null writes in section 6, and
  `src/calculate/range.rs` gains the null filter. Splitting `mapping.rs`
  stays out.
- `ath key clear garmin` is refused in 2.1.1, because `key_clear` in
  `src/cli/commands/model.rs` accepts only gateway names. `ath key set
  garmin` works. This pull leaves that as it is. Making the Garmin key
  removable is separate work.
- `persist.rs` and `ath-core persist` are gone. A name check outside import
  is separate work. The pull does not add one.
- The mapping path rule is already `is_mapping_path` in `src/assets/mod.rs`.
  There is no duplicate to delete.
- Vendor originals live in the package's main document, under its 512 MiB
  read cap. Storing them in their own package files, as samples are, would
  lift the 128 MiB bound in section 2. That is a package format change,
  and this release does not make it.

No maintainability note under another name was found in the 2.1.0 or
2.1.1 decision logs. The sizes above are the code at 2.1.1, not a
recorded D-number.

## 10. Risks and the approval

Cloud-agent time for the build is uncertain. The published history has no
measured run for this pull.

Risks:

- A server newer than 0.11.0 can add a default that hides a second device.
  Current `main` does not, when the queries in D230 are sent. The client is
  tested against the stored fixture. That fixture is not a live response, and
  a live timestamp without an offset is left out. A breaking envelope needs a
  new fixture before the pull trusts it.
- A raw range can be large. A page over the response cap, 10 MiB, is read
  again with a smaller limit. A 429 or another transport failure still
  stops that read, keeps earlier pages, and withholds the success line. The
  study did not read the rate-limit middleware.
- A failed step 6 on a database older than 2.1.1 leaves it at 2.1.1,
  because the earlier steps committed first. The message says so and names
  the verified backup, and a second `ath migrate` finishes. Step 6 needs
  foreign keys off, and the earlier steps run with them on, so they cannot
  share one transaction.
- A live read can change the server. It can refresh a token, revoke a
  connection, or wait on a rate limit. The command says so before its
  first live request. It cannot stop the server from doing it.
- Strava's limit is shared with the deployment's own sync. One command can
  use most of a 15-minute window.
- The pinned Garmin detail path and the cited Garmin rate were not found
  in Garmin's public docs. If the path does not exist, each Garmin detail
  answers 404 or 501. Those count as `not_available`, Garmin stays
  `completed`, and each Garmin original holds its list object only. Each
  command still spends up to 100 Garmin detail calls on ids that were
  `not_available`, after new ids. A deployment where that path works fills
  those details in on later commands.
- A later pull replaces an owner correction when the server's data for
  that record changed since an import last wrote it. The preview and the
  result count each one. The correction stays in the history. Deleting
  the record stops later pulls from changing it, and it also takes the
  record out of current data. This rule covers every record on route
  `open_wearables`, not only vendor originals. Other routes keep D182.
- A deferred id that the provider stops listing is never detailed. Its
  list object stays stored, and the deferred count shows that it waits.
- Vendor originals share the main package document and its 512 MiB read
  cap. The 128 MiB bound refuses a vendor page before a package would
  become unreadable. A person with several hundred Polar workouts that
  have routes can reach it. That provider then stays `incomplete` until
  originals are erased, or a later version stores them in their own files.
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
schema and step 6: the root `record_type` check, the period type, the
`aggregator` basis, the sample series quantities in the appendix table,
the `source_report` type and table, the paired null value and unit, and
the time-unknown summary. The point check in `src/model/data.rs` and the
mapping's positive check accept a finite `hr_recovery`, including zero and
a negative number. `packages/npm/src/schema.ts` is not where that check is
added. No table is rebuilt for that check. Then the store, the
calculation null filter, the staging and apply changes in
`src/import/provider.rs` and `src/import/binding.rs`, the archive, and the
record paths in section 6. Then the mapping engine, `MappedPage`, and
`mappings/open-wearables.md`. Then the stored reads, the vendor read, the
cycle filter, the smaller-page split, and the tests in section 7. The
laptop card and the phone proof are part of that same pull request. This
plan pull request has no media.

The maintainer approves or rejects this plan before any build. D227 through
D244 are the proposal. They are not adopted. The stored workout and the
live original stay separate records, because the public stored
workout has no vendor id. That split is part of this proposal. D244 is the
privacy choice, and the maintainer can change it before approval. Reviews of earlier heads were
incorporated here. None of those reviews approves this revision. The next
head needs re-review. An earlier review accepted a
migration with no new column, under D152 and D184. That acceptance does not
adopt D227. The
optional source key adds no column.

1. One number, 2.2.0. Readable sets keep `1.0.0`. Step 6 runs on create and
   on migrate. Step 5 stays the published 2.1.1 migration. The device
   receipt adds no column. Step 6 extends the
   root check with `source_report` by the copy in section 5, creates
   `source_reports`, and rebuilds `provider_summaries` and
   `provider_scores` so a numberless object can omit `value` and its unit
   together. Existing ids and rows are copied. Before the root rename,
   step 6 drops and later recreates `sql_v1_current_records`,
   `sql_v1_active_records`, and the two `record_revisions` triggers that
   read `records`. Foreign keys stay on for ordinary writes. Step 6 turns
   them off on the store's own connection, only before its transaction,
   and reads them back on after every exit. A
   `2.1.1` database does not open until `ath migrate`. This release does
   not schedule a pull, add its own token refresh, backfill, or store a
   cursor (D227). A live read can still make the server refresh a vendor
   token (D243).
2. Store the returned representations in section 2. Device receipts stay
   on device sources. Summaries without a device id use their own sensors.
   Null stays null. A missing response is not called provider-unavailable.
   The omissions in section 2 are the ones with a reason (D232, D235).
3. The key, the selected origin, and the selected user id are rejected in
   decoded JSON before a page is mapped. Failure text uses a fixed sentence.
   `ath key clear` with no name still clears only the gateway keys and says
   when the Open Wearables key remains. Named clear removes the saved key
   only. It does not unset the environment and it does not revoke the server
   key (D229). A credential-shaped value is withheld and counted. It does
   not refuse the page (D244).
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
   left in place. `ath correct`, the protocol record writes, and a native
   2.2.0 package restore are the owner's whole source record (D230, D231).
5. The Windows cleanup, the Garmin key removal, and the maintainability
   notes stay separate. `src/import/mapping.rs` gains `MappedPage`, the
   `vendor_workout` section, and the preservation writes in section 2.
   `src/import/provider.rs` gains the staged first bind, the
   `device_binding` refusal in `ProviderPreview::apply`, the `connector`
   source call, and the stored-original read. `src/import/staging.rs` gains
   the count by record type. The new `src/import/binding.rs` holds the binding
   check and the bounded safety snapshot. `src/store/encode.rs` gains the
   sample, source-report, withdrawal, and paired-null writes in section 6,
   and `src/calculate/range.rs` gains the null filter. The pull command
   builds the archive preview and applies it only at the snapshot's
   revision. `mapping.rs` does not start it. Failed-init cleanup stays as
   it is (D231, D237).
6. The live vendor read is off by default and warns before its first
   request. It calls only active connections and never stores the
   connections list. It keeps every stored detail, counts every object it
   does not write, and withholds credential-shaped values. The stored
   workout and the vendor original stay two records. On route
   `open_wearables`, an owner correction is kept until the server's data
   for that record changes, and each correction a page replaces is counted
   (D242, D243, D244).
