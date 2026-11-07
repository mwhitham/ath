# Athletic Standard 2.2.0

Status: **plan only. The maintainer has not approved it. Not approved for build.**

`ath pull openwearables` reads one person's stored health data from an Open
Wearables server they run, and writes it with the mapping this release
ships. The proposals are D198 through D203 in
[decisions.md](decisions.md). They are not adopted. The evidence is
[appendix/open-wearables-api.md](appendix/open-wearables-api.md), checked on
6 November 2026.

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
- one line each for `sleep`, `workouts`, `timeseries_raw`,
  `timeseries_day`, `sleep_summary`, `activity_summary`,
  `recovery_summary`, `body`, `health_scores`, `meals`, and `cycles`:
  `completed`, `failed`, `skipped`, or `not_attempted`
- estimated counts, by record type, of sleep sessions, workouts, points,
  measurement samples, summaries, and scores
- committed counts of zero
- the coverage lines in section 2
- `left_out` as a total plus at most 20 example lines
- `nothing written; pass --yes after reviewing this preview`

`--yes` without `--json` commits each successful page. That commit is one
`apply_mapping` transaction (D189). It is not a completed pull. The text
prints actual committed counts for pages kept. It prints `requested range
<from> through <to> is complete` only when every read ended with `has_more`
false. Only then does it print `pulled openwearables from <from> through <to>`.

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
every read completed or was skipped. The read status token is
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

D203 is the rule. An earlier draft of this plan left day values, the
heart-rate stream, sleep and recovery summaries, and workout segments for a
later version. That draft is superseded. Those values are part of this pull.
File imports stay. The Garmin pull stays. A Junction pull is proposed as
2.3.0. Automatic sync, retries, token refresh, and backfill are proposed as
2.4.0. None of those numbers is adopted. This release adds one module for
this server. It does not add a connector interface.

The pull stores the measurements Open Wearables 0.11.0 can return for one
user and one range. A number is stored with the unit the server sent. A
period is stored as the period the server sent. A null field is absent. It
is not stored as zero. A value the provider never sent is absent. That
absence belongs to the provider. This plan did not remove the field.

Sleep sessions are requested with `include=stages`. Workouts are requested
with `include=zones` and `include=segments`. Raw timeseries and day
timeseries each omit `types`, so the server returns every series it holds
for that resolution. The same dates are also read from the sleep, activity,
and recovery summaries, the body summary, health scores, meals, and
menstrual cycles. Reads that accept `filter_by_priority` send `false`, so
every device is included. Section 3 still applies to those rows.

A raw row with no `period_end`, whose type is already a point quantity,
stays a point at the server's timestamp. Those 2.1.0 lines keep their match
keys, so a file import and a pull of that raw point stay one record. A raw
`heart_rate` row is a measurement sample. `heart_rate` is already a series
quantity. It is not stored as a point. When that row has `window_seconds`,
`duration_ms` is that count times 1000, and `window_seconds` also stays in
`supplied_fields`. Any other raw series that is not
already a point is a measurement sample. The build adds a quantity for each
pinned series type that does not have one. The unit is the unit in
`SERIES_TYPE_DEFINITIONS` at the pinned commit, and the number is the
server's number. Latitude and longitude are in that list. The stored record
then contains the locations the server sent.

A row with `period_end`, including every `resolution=1day` row, is a
provider summary. `recorded_at` is left unset. The period type is
`open_wearables_reporting_window`. `start_at` and `end_at` are the server's
`timestamp` and `period_end`. On 0.11.0 those instants come from
`CalendarPeriod.of_date` in `timeseries_service.py`. This program stores the
strings the server sent. It does not calculate the boundary, and it does
not label the window as a Whoop cycle or an Oura day. `stats` and
`computed_by` stay under those names. Basis is `provider` when
`computed_by` is `provider`, `aggregator` when it is `open_wearables`, and
`provider_or_aggregator` when `computed_by` is absent.

`SleepSession` has durations and stage totals. It has no resting heart
rate, heart-rate variability, blood oxygen, respiration, or skin
temperature. Those numbers are stored from the summary or the series that
actually carries them. Each non-null number on a sleep, activity, or
recovery summary is its own provider summary. `provider_date` is the
server's `date`. When a sleep summary also sends `start_time` and
`end_time`, those are the period bounds. A date by itself does not become
midnight. RMSSD and SDNN stay separate fields. The body summary uses
`measured_at` for a latest reading and `period_start` with `period_end`
for an average, when the server sent them. `age` is stored as the
calculated number the server sent. This pull does not call the
personal-record route.

A health score that has a number is a provider score. The metric is the
category. The scale is `0-100` only for a field the server documents that
way. `RecoverySummary.recovery_score` is documented as 0 to 100 and marked
deprecated. The number is still stored, and the deprecation text stays in
`supplied_fields`. A score with a null value keeps the server object,
including its qualifier, on a provider summary that has no numeric value.
Meal numbers are provider summaries at the meal timestamp. Cycle numbers
use the cycle's start and end. The original object stays in
`supplied_fields` under the server's names. Workout numbers that are not
already typed stay there too, and each number is also a source-reported
summary on the workout. Segment objects stay under `segments`. A segment
with a label is also a typed segment. A missing label is not filled in.
Zones stay under the server's names.

A provider summary may omit `value` when the server object has no number.
`supplied_fields` holds that object, and the record has an instant or a
reporting period. Step 5 rebuilds `provider_summaries` so `standard_value`
and `standard_unit` may be null together. Existing rows are copied with
their numbers. No number is created.

The following are not requested. Each line is the reason.

- Resolutions `1min`, `5min`, `15min`, and `1hour`. `_aggregated_timeseries`
  builds those buckets from raw samples when the query runs. The raw
  samples are stored.
- Resolutions `1week` and `1month`. `_period_timeseries` builds them from
  day rows and sets `computed_by` to `open_wearables`. The day rows are
  stored.
- Sleep totals, workout totals, and activity totals. The route text says
  they add up the same rows the corresponding list returns.
- The data summary and the data timeline. They count stored rows.
- The personal-record route. Its fields are birth date, gender, and sex.
- The provider workout list and the provider workout detail. Those routes
  read the vendor through the server's live connection. This pull keeps
  the stored workout, including segments.
- Accounts, connections, sync, OAuth, webhooks, API keys, config, uploads,
  and every `DELETE`. They are not measurements. The API key, the server
  address, and the user id are not written into the athlete record.
- A workout with `entry_source` `manual`. The server describes it as a
  self-reported entry with no device measurement. The measured record does
  not take it.

`RecoverySummary` at this commit has no skin-temperature field. The class
ends at `recovery_score`. Skin temperature is stored from a timeseries row
or from `BodyLatest.skin_temperature_celsius` when the server sends one.
The recovery field `avg_hrv_sdnn_ms` is in the schema. Its text says the
value is null for the rows that endpoint returns today, because those rows
come from WHOOP and WHOOP's HRV on that object is RMSSD. A non-null SDNN
is stored. RMSSD is not written into the SDNN field.

A raw page and a day page are separate reads. Each page is one transaction
(D201). The body cap stays 8 MiB. A larger response is not mapped. The
same query is sent again with `limit` cut in half, down to 1. A transport
failure is not retried. One row that is itself larger than 8 MiB cannot
be stored. That read stays incomplete, and the message names the series
type. The pull does not drop a series to stay under the cap.

The result separates four facts. The server returned no row. The server
sent null. The row was stored. The server returned a row and the mapping
refused it. `left_out` counts only the last fact. A missing series is not
reported as zero.

`source.provider` is the Open Wearables integration name. `source.source` is
the writer inside that integration. The mapping's `writer_field` is a
convention. It is not proof that the device vendor computed the value. The
basis is `provider_or_aggregator` when the server does not say who computed
the number, `provider` when `computed_by` is `provider` or the record is a
provider score, and `aggregator` when `computed_by` is `open_wearables`.
A raw sample on 0.11.0 is built without `computed_by`. That absence stays
`provider_or_aggregator`. Workout fields may be filled by Open Wearables
and have no per-field `computed_by` on the workout object. The original
`computed_by` string, when the server sent it, stays in `supplied_fields`.

## 3. How a response becomes records

The pull resolves `open-wearables` through `resolveMappingFile`. It parses and
checks the file before the first request. A bad file fails with no network
call. A resolved file that does not implement section 2 also fails before
any request. An installed 2.1.0 copy still wins under D179, and this check
stops it from quietly dropping the rows section 2 stores.

For each page of sleep it calls `apply_mapping` with
`{ "sleep": <data array> }`. For each page of workouts it uses `workout`.
For each timeseries page it passes the response object unchanged. Pagination
is not a record. Sleep and workout `metadata` is not a record. Timeseries
`metadata.resolution` stays, because the mapping file requires it.

Everything after that is the mapping engine from 2.1.0: match keys (D181),
replace on change (D182), source and device (D183), one transaction per call
(D189), a deleted source left out (D190), and field names refused when they
are empty or `__proto__` (D191). A Garmin file import remains a different
`data_route`, so both copies of one workout stay (D161). A second pull of the
same Open Wearables object, or an `ath import --mapping open-wearables` of a
saved copy of that object, updates the same rows.

The writer is `source.provider`. The sensor string is `source.device_type`,
`source.device`, and `source.source`. Two devices with different triples are
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
as the write and before `insert_group` changes a source device or a
timeseries record, including a measurement sample and a window summary.
Other routes are not checked. A pull-only check would
leave import-after-pull able to replace the point. The check reads every
accepted row, including a row after the first source device.

The receipt is the source object's own key `device_binding`. The 2.2.0
source schema makes it an optional nonempty string. D185 keeps
`supplied_fields` for names the source sent, so the receipt is a sibling of
that object, as `field_basis` is a sibling on a measured record. Step 5
adds no column, copies no field, and rewrites no record. A source migrated
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
call. `--json` and `--json --yes` make no database changes. The queries in
D201 still ask for every device.

Preview keeps two facts in memory for this command: the binding it would
have stored, and an unbound source it would have created once that source
has accepted history. A later preview page uses those facts, including a
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
rule above. It gains nothing else:

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
preview, or an error line. The pull reads only the paths in D201 and D203.
It does not list users and does not read the personal record.

The process refuses a base URL that could carry the key in the URL itself.
It refuses plain `http` except to loopback. It does not follow a redirect, so
the key cannot be sent to a second host. One attempt, then stop. Timeouts are
10 seconds to connect and 30 seconds to read. One response may be at most
8 MiB. One read may be at most 1000 pages.

The body is read inside that cap and parsed before any `apply_mapping` call.
The guard is in `open_wearables.rs`. It walks every decoded object key and
every decoded string. A value that contains the API key, the selected
origin, or the selected user id rejects the whole page. The page is not
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
is added. The same file rebuilds `provider_summaries` as section 2 describes
and copies existing rows. Step 4 and file `004` are not edited (D165).

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
`ProviderReportingPeriodType` gains `open_wearables_reporting_window`.
`ValueBasis` gains `aggregator`. Series quantities gain one entry for each
pinned series type that does not already have a point or series quantity.
`provider_summary.value` may be omitted, as section 2 says. No other field
is added. The folder exists so the package version and the schema path
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
first bind on an existing source that has no history. `ath correct`,
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
  401, a 429, a repeated or null cursor, an invalid row, or a body over
  8 MiB. Page 1 remains. Page 2 adds no rows and no revisions. No later read
  runs. The exit is nonzero. The success sentence is absent. The text gives
  the retained counts. The same command run again stores no second copy
- the same failure sequence with no `--yes`, and again with `--json --yes`,
  makes no database changes. Rows that were already stored stay. A
  successful `--json --yes` of a readable range is also a preview and makes
  no database changes
- an installed mapping that does not implement section 2 fails before any
  request. The server sees no request. Raw and day timeseries requests omit
  `types`. A comma-joined type string is not sent
- a missing object, a `data` value that is not an array, a missing
  pagination object, or a `has_more` value that is not a boolean fails the
  page before `apply_mapping`. It is not an empty success
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
  value. A day row for resting heart rate, RMSSD, SDNN, blood oxygen, and
  VO2 max becomes a provider summary with
  `open_wearables_reporting_window`, no `recorded_at`, and the server's
  `timestamp` and `period_end`. A null RMSSD beside a present SDNN stores
  only the SDNN. A sleep summary respiratory rate is stored, and the sleep
  session from the same night still has no respiration field. Skin
  temperature is stored from a series row or from body latest. A recovery
  object with no skin-temperature field does not gain one. `page-5min.json`
  is left out, and the reason says it is a query bucket. RMSSD and SDNN
  stay different quantities. A 2.1.0 raw point and a pull of that same raw
  point through the new mapping stay one record. A heart-rate sample from
  device B does not replace device A's sample
- a response larger than 8 MiB is requested again with a smaller limit, and
  the rows are stored. One row larger than 8 MiB leaves that read
  incomplete and names the series type. The personal-record route is not
  requested. The export after a pull and a package import contains the
  period bounds, the units, `computed_by`, and the segment objects
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
   estimated counts that include a heart-rate sample and a day summary, and
   `ath check` still passes. Failure: a row is written, or the key appears.
3. Write. Success: the same command with `--yes` says the requested range is
   complete, shows committed counts, and `ath check` exits 0. Failure: the
   success sentence appears while the range is incomplete, or `ath check`
   fails.
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
  partial-update refusal and the three source operations in section 3.
  Splitting the files stays out.
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
- Section 2 stores the server's periods and series. It does not invent a
  midnight. A provider that never sent a value still has a gap. The device
  receipt is a separate rule.
- Fields on a sleep or workout object that the mapping does not consume are
  stored under the server's names. Latitude and longitude are stored when
  the server sends them.
- The administrator key is more powerful than this command. A leaked key from
  the password store exposes every user on that deployment. The command's job
  is to avoid copying the key into the record.

The build, after approval, is one pull request in this order. First the
schema and step 5: the period type, the `aggregator` basis, the series
quantities, and the nullable provider-summary value. Then the mapping
engine and `mappings/open-wearables.md`. Then the reads, the smaller-page
split, and the tests in section 7. The laptop card and the phone proof are
part of that same pull request. This plan pull request has no media.

The maintainer approves or rejects this plan before any build. D198 through
D203 are the proposal. They are not adopted. Reviews of commit `d1fdde1`
were incorporated here. The re-reviews of `1124e56`, `da006a6`, and
`c9ca93a` are incorporated here. None of those reviews approves this
revision. The next head needs re-review. An earlier review accepted a
migration with no new column, under D152 and D184. That acceptance does not
adopt D198. The
optional source key adds no column.

1. One number, 2.2.0. Readable sets keep `1.0.0`. Step 5 runs on create and
   on migrate. The device receipt adds no column. Step 5 does rebuild
   `provider_summaries` so a summary of an object with no number can omit
   `value`. A `2.1.0` database does not open until `ath migrate`. This
   release does not schedule, refresh a token, backfill, or store a cursor
   (D198, D203).
2. Store the Open Wearables measurements in section 2. Periods keep the
   server's bounds. Series keep the server's numbers and units. Null stays
   null. The omissions in section 2 are the ones with a reason (D203).
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
   no history. Nothing else is added to either file (D202, D203).
