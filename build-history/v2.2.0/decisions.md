# Decisions — v2.2.0

D-numbers continue from [v2.1.0](../v2.1.0/decisions.md). D198 follows D197.

These are proposals in [spec.md](spec.md). The maintainer has not adopted
them. This pull request does not build them. A change of mind before
approval is an edit of the proposal. After approval, a change is a new
D-number.

## D198. Version 2.2.0 is one manual pull, and the program and the record share that number

`ath pull openwearables` reads the stored measurements D203 lists from an
Open Wearables server the person runs, for one user and one range they name. The
command is manual. The person runs it. The program does not schedule it,
retry a failed request, or sign the person in to a device maker.

The program, the record, and the database become `2.2.0` together. D152 made
those one number. D184 kept that rule. Migration step 5 stores `2.2.0` and
SQL schema 5. It adds no column. It rebuilds `provider_summaries` as D203
describes and copies existing rows with their numbers. Step 4 is not edited
(D165). A new database runs step 5 after step 4 and before `user_version` is
set. `applyMigrationStep` handles target 5. The migration chain, the SQL
contract generator, and the drift check gain that step. The package exports
point at `schema/2.2.0/`. `READABLE_DATABASE_VERSIONS` and
`ACCEPTED_RECORD_VERSIONS` keep `1.0.0`, `2.0.0`, and `2.1.0`, and gain
`2.2.0`. `IMPORTABLE_PACKAGE_VERSIONS` gains `2.2.0` and keeps `2.0.0` and
`2.1.0`. A `2.1.0` package still imports. A `2.1.0` database does not open
until `ath migrate`.

The build is one pull request after this plan is approved. It is not split
into a version change, a client, and a mapping change.

The version numbers below are proposals. They are not specs, and they are
not adopted.

- 2.2.0 is this pull. D203 says which stored measurements it keeps.
- 2.3.0 is a proposed Junction pull. This release does not call Junction.
- 2.4.0 is proposed automatic incremental sync, saved sign-in refresh,
  retries, and backfill. This release has none of those. A smaller page
  after a local 8 MiB cap, described in D203, is not that retry.
- An earlier draft of this plan left day aggregates and the heart-rate
  stream out and suggested 2.5.0 for them. D203 supersedes that draft.
  D88 still forbids a fabricated instant. It does not forbid keeping a
  bound the server sent.
- A connector interface that every future source would implement is not
  part of this release. This release adds one module for this server.

File imports stay: Apple Health, WHOOP, Oura, Garmin, and Strava. `ath pull
garmin` stays as it is. Mapped import for every route other than
`open_wearables` stays as it is. Open Wearables mapped import gains the
shared binding refusal in the specification. It no longer accepts a second
device id, or a refresh of history that has no established binding.

Evidence: [appendix/open-wearables-api.md](appendix/open-wearables-api.md).

Rejected alternatives:

- Leave the record at 2.1.0 while the program becomes 2.2.0. Rejected because
  D152 and D184 already rejected a split.
- Ship the pull as three pull requests. Rejected because the maintainer asked
  for one versioned build.
- Pull Junction, start sync jobs, or add a connector trait in this release.
  Rejected because this release has to make one manual read reliable first.
  The later version numbers are proposals, not adopted assignments.
- Leave day aggregates and the heart-rate stream for a later version, or
  request them while still calling that later version their home. Superseded
  by D203. The current proposal stores them in this pull and says so.

## D199. The pull fetches pages and the shipped mapping writes the records

New Rust module `crates/ath-core/src/open_wearables.rs` requests the reads
in D203 and calls `apply_mapping`. The command-line front end stays in
TypeScript (D174). New imports stay in `ath-core` (D155). The HTTP client is
the `ureq` 2.12 dependency the Garmin pull already uses. No new crate.

The mapping file is resolved with `resolveMappingFile` in `src/skill.ts`,
name `open-wearables`. An installed copy in `.agents/mappings/` wins, which
is D179. Rust receives the absolute path. It does not grow a second rule for
what a path looks like.

The mapping file chooses every stored field. The pull does not rename a
field, choose a unit, or decide that a value is RMSSD or SDNN. A field the
file does not name stays in `supplied_fields` under the name it arrived with
(D153, D161). `field_basis` stays whatever the file already states (D185,
D187, D192).

Sleep and workout responses are envelopes with `data` and `metadata`. The
shipped file's page section is `timeseries`, so `apply_mapping` would leave
that envelope out. The pull passes sleep rows as `{ "sleep": [ ... ] }` and
workout rows as `{ "workout": [ ... ] }`. It passes each timeseries response
through unchanged, so `metadata.resolution` still reaches the mapping. The
pull does not drop one row and keep its neighbors. A credential, a reflected
origin or user id, a same-key `device_id` collision, or a bad envelope
rejects the whole page first. A manual workout stays the mapping's
`left_out`, and the other rows in that page are still written.

The timeseries query's `types` list is the `type.<name>` lines in the
resolved mapping file, read when the command runs. The prefix is that
section's `type_field`. Each name is one repeated `types` query parameter.
A name that appears twice is sent once. `skin_temperature` is that case. A
comma-joined string is not a member of the `SeriesType` enum. This study did
not send that request, so it does not claim a live 422. The Rust source has
no second copy of those names.

On Open Wearables 0.11.0 the route defaults `types` to an empty list, and
the repository applies a type filter only when the list is nonempty
(`data_point_series_repository.py`). An empty list therefore returns every
series. D203 omits `types` on the raw read and the day read for that
reason. A mapping that cannot store those series fails before any network
call. A comma-joined type string is still not sent.

`heart_rate` is a series quantity. D203 stores the raw stream as
measurement samples. Workout `avg_heart_rate_bpm` and
`max_heart_rate_bpm` stay as the file already maps them.

`apply_mapping` adds counts by record type and keeps the existing `outcome`
field. The pull adds those counts into the committed result for a page that
committed and into `estimated` for a page that was rolled back. It does not
add `outcome` across pages. Estimated counts are against the database before
this command. They are not equal to the write when the same match key appears
on two pages, or when the server changes between runs. The write still
follows D182. This proposal does not keep a command-wide preview overlay.

The read order is the order in D203. A failure in a later read
leaves the earlier read's pages committed and the reads after the failure
`not_attempted`. The result says so. See D201.

D203 is the coverage rule. An earlier draft of this decision left the day
rows, the summaries, and the heart-rate stream unrequested. That draft is
superseded. D88 still forbids inventing a midnight. The period type in
D203 stores the bound the server sent. An Open Wearables window is not
labeled as a Whoop cycle or an Oura day.

Sleep requests `include=stages`. Workouts request `include=zones` and
`include=segments`. Stage minute totals return either way. The timeline,
the zones, and the segments are null when the expansion is omitted, so
omitting it would drop them before `supplied_fields` can keep them. A page
over 8 MiB is handled as D203 says.

Rejected alternatives:

- Teach the pull the Open Wearables field names in Rust. Rejected because
  D161 put those names in the mapping file so a second source does not become
  a second record format.
- Pass the sleep envelope straight to `apply_mapping`. Rejected because the
  current page rule would leave the page out. The check is `page_rows` and
  `row_fits` in `crates/ath-core/src/mapping.rs`.
- Request every series type and let `left_out` drop the ones the mapping
  cannot store. Rejected. D203 stores them. `left_out` remains a row the
  server returned and this program refused for a named reason.
- Map `heart_rate` samples as points, or map a day summary onto midnight.
  Rejected. `heart_rate` is stored as a measurement sample. A day row is
  stored as a reporting window (D88, D110, D203).
- Refuse an empty `types` list in order to avoid heart rate and location.
  Superseded by D203. Omitting `types` is how 0.11.0 returns every series,
  and those series are stored. A comma-joined string is still not a member
  of the enum.
- Put `device_id` on the sensor string, and therefore on the match key, for
  both import and pull. Not this proposal. It needs no new column, and the
  mapping engine already joins configured sensor paths, but it changes
  source, baseline, and key identity. Old triple-key rows would stay where
  they are, and dedupe against them cannot be promised. A `device_id` that
  contains `|` collides with the join unless those values are refused or
  the join is encoded. The shared binding in the specification keeps the
  current keys and refuses the unsafe write instead.
- Guard the pull only, or treat `source_devices.hardware` as the binding.
  Rejected. Import after pull would still replace the point, and hardware
  is one mutable column. The point's revisions do not contain `device_id`
  today, because that path is removed as hardware.
- Put the receipt in `supplied_fields`, under `device_binding` or another
  name. Rejected. D185 keeps that object for names the source sent. A 2.1.0
  source may already hold an arbitrary value at that name, and step 5 must
  not turn that value into authority. The receipt is the source object's own
  key `device_binding`.
- Treat a nonempty string at the new key as proof no matter who wrote it,
  or require a signature so the owner cannot change it. Rejected. The
  automatic path writes the key only by the checked rule. `ath correct`,
  package restore, and an accepted package merge store the whole source the
  owner supplied. A hand-edited database file is outside this proposal.

## D200. The key, the server, and the user id stay out of the record

The API key is an administrator key for every user on that deployment. The
pull sends it only as the header `X-Open-Wearables-API-Key`. It is saved with
`ath key set openwearables` in the operating system password store, the same
store D75 uses. The environment name for a script is `OPEN_WEARABLES_API_KEY`.
The child process receives it as `ATH_CORE_OPEN_WEARABLES_KEY`. The key is
not a command argument. If the password store is missing, the command refuses
and does not write a file.

Open Wearables is not added to the gateway list. `GATEWAYS` stays `vercel`
and `openrouter`. `ath key clear` with no name still clears only those
gateway keys, and it still exits successfully when that is what it did.
When a saved Open Wearables key remains, the text adds two sentences: the
Open Wearables key remains saved, and `ath key clear openwearables` removes
it. When no saved Open Wearables key remains, the gateway sentences stay as
they are today. `ath key clear vercel` and `ath key clear openrouter` do not
remove the Open Wearables key and do not gain that sentence.

`ath key clear openwearables` deletes the password-store key only. It does
not unset `OPEN_WEARABLES_API_KEY`. The text says the saved key was removed.
When the environment variable is set, the text also says that key is still
available and a pull can still use it. When the variable is unset, the text
does not claim an environment key remains.

`ath key` keeps its gateway lines. It adds separate lines for a saved Open
Wearables key and for `OPEN_WEARABLES_API_KEY`, and it prints neither value.
`ath key --json` keeps `saved` and `environment` as the gateway lists they
are today, and adds `openwearables.saved` and `openwearables.environment`.
Garmin's key command is unchanged.

The server is `--base-url` or `OPEN_WEARABLES_BASE_URL`. The user id is
`--user` or `OPEN_WEARABLES_USER_ID`. Neither is written into the database,
the export, a config file, or a log line. The pull never calls `GET /users`
and never calls the personal-record route. One command reads one user.

`--base-url` is an origin: scheme, host, and optional port. No user name, no
password, no path other than empty or `/`, no query, and no fragment. `http`
is allowed only for `localhost`, `127.0.0.1`, and `::1`. Any other host
requires `https`. The pull appends `/api/v1` itself. Redirects are not
followed. A timeout, a non-JSON body, or an HTTP error becomes the page
failure in D201. The printed error is a fixed sentence and the HTTP status. A problem `code`
is printed only when it is one short token and does not contain the key,
the origin, or the user id. Otherwise it is withheld. A transport error or
a parser error is replaced with a fixed sentence that has no URL, no path,
and no user id.

After the body is read inside the size cap and parsed, and before
`apply_mapping`, `open_wearables.rs` walks every object key and every string.
A value that contains the API key, the selected origin, or the selected user
id rejects the whole page. The row is not rewritten. A scan of the raw text
misses a Unicode escape. The raw-text scan and the check before printing
stay as defense in depth. They are not what keeps the key out of a committed
page. Named clear of the saved key does not revoke it on the server.

The shell history can contain the server and the user id when they are
passed as flags. The key is not a flag. `ath key set openwearables`, or
`OPEN_WEARABLES_API_KEY` already held by the shell, keeps the key off the
command line. Putting the key on the same line as the command would store
it in shell history. Do not do that.

Rejected alternatives:

- Store the server and the user id on the source row so the next pull can
  omit them. Rejected because the export would carry them, and agents read
  the export.
- Remember them in a file beside the database. Rejected because that file is
  easy to commit and easy to copy into a backup by mistake.
- Accept a bearer token or a user-scoped SDK token for these reads. Rejected
  because the 0.11.0 server does not accept the API key as a bearer token,
  and the SDK token is limited to SDK routes. See the appendix.
- List users and pull every user the key can see. Rejected because the key
  sees the whole deployment, and this record is one athlete.

## D201. A page is atomic, a missing row is not a deletion, and the same command is safe to repeat

Each response page is one `apply_mapping` call. D189 still makes that call
one transaction: a failure rolls the page back, and a preview rolls the page
back after counting. The pull itself is a series of those calls. The proposed
order is sleep pages, then workout pages, then timeseries pages. A committed
page follows D189. A committed page does not mean the pull is complete.

The command previews unless `--yes` is present. `--json` does not write,
with or without `--yes`, matching `ath import --mapping`. `--yes` without
`--json` writes each page as that page succeeds.

The child returns one bounded outcome for the mode it ran. A preview,
including public `--json`, has `written` false, committed zeros, estimated
counts, and `range_complete` false. A confirmed `--yes` has the actual
committed counts. `written` is true only when that command committed a new
or revised record. A successful repeat has `written` false and
`database_changed` false. The database is unchanged. The parent renders the
confirmed outcome as text. It renders the preview outcome as public JSON.
On a partial failure the child returns the confirmed outcome and then exits
nonzero. The parent renders the text before it uses that status.

Each of `sleep`, `workouts`, and `timeseries` is `completed`, `failed`,
`skipped`, or `not_attempted`. The token is `not_attempted` in the outcome
and in the text. `skipped` is the timeseries read when the mapping named no
types. `range_complete` is true only on a `--yes` write whose reads finished
without a failed page. The success sentence is printed only then. A retained
count with an incomplete range is a partial pull. The message names the
failed read, names any `not_attempted` read, and gives the retained counts.
A failed read says to run the same command again. An identity refusal uses
the sentence in the specification's section 3: a replacement id is blocked
even at a new time, a 2.1.0 Open Wearables history cannot be refreshed until
reconciliation, and
a rerun cannot repair either one. A preview that fails makes no database
changes, including for pages that had already been counted.

Before `apply_mapping`, the page must be a JSON object, `data` must be an
array, `pagination` must be an object, and `has_more` must be a boolean.
Anything else fails the page. It is not an empty success. The result keeps
a `left_out` total and at most 20 examples.

An empty range whose reads completed is a successful pull with
committed counts of zero and `range_complete` true. A repeat uses the match
key from D181 and the replace from D182, so an unchanged row is not written
again. A rerun after a partial write leaves the committed rows in place,
runs the failed read again, and runs any read that was `not_attempted`.

`has_more: false` is the only successful end of a read. A repeated cursor,
a null cursor while `has_more` is true, and the 1000-page cap fail that
read. A body over 8 MiB is handled by D203: the same query is sent with a
smaller limit, and one row that is still over 8 MiB fails that read. The
failed page is not written. Later reads are `not_attempted`. The success
line is not printed. A cursor with the `prev_` prefix is not sent.
`limit` starts at 1000. One attempt is made per request, except the smaller
page D203 sends after a local size cap. A transport error is not retried.
The connect timeout is 10 seconds and the read timeout is 30 seconds.

A row that this range did not return is left where it is. The pull does not
call `DELETE`. There is no deletion feed on these reads. Absence can be
a short range, a failed page, or a server filter. D190 still applies: a
source the person has deleted stays deleted, and its rows are left out. A
record the person has deleted stays deleted, and that object is left out
(D182).

`--from` and `--to` are sent unchanged. Both are dates `YYYY-MM-DD`, or both
are timestamps with an offset. A mix is refused. The command prints one line
that a date-only bound is interpreted by Open Wearables, and that Athletic
Standard did not convert it to the athlete's timezone (D88). The appendix
records the disagreement between the docs and the 0.11.0 parser.

Queries that are sent:

- Sleep: `start_date`, `end_date`, `limit`, `cursor` after the first page,
  `filter_by_priority=false`, `include=stages`. `is_nap` is omitted, so naps
  and main sleep both return.
- Workouts: `start_date`, `end_date`, `limit`, `cursor` after the first page,
  `include=zones`, and `include=segments`. No priority flag, because 0.11.0
  has none and current `main` already returns every device when the flag
  is absent.
- Timeseries, twice: `start_time`, `end_time`, `limit`,
  `filter_by_priority=false`, `cursor` after the first page, and
  `resolution=raw` on one read and `resolution=1day` on the other. `types`
  is omitted on both.
- Sleep summary, activity summary, and recovery summary: `start_date`,
  `end_date`, `limit`, and `cursor` after the first page.
- Body summary: one request. `average_period` and `latest_window_hours`
  are omitted, so the server's defaults apply.
- Health scores: `start_date`, `end_date`, `limit`, and `offset` until a
  short page. This route has no cursor.
- Meals and menstrual cycles: `start_date`, `end_date`, `limit`, and
  `cursor` after the first page.

`source_selection` is not sent. On 0.11.0 the parameter does not exist. On
current `main`, omitting it returns every device when the priority flag is
false.

Rejected alternatives:

- One transaction for the whole range. Rejected because a raw range can be
  large, and a failure at the end would discard pages that had already
  succeeded. Each page stays atomic under D189. The pull's result still says
  the requested range is incomplete when a later read failed or was not
  attempted.
- Treat a committed page as a finished pull. Rejected because the person and
  an agent would read a partial database as the whole requested range.
- Store a resume cursor in the database. Rejected because the same command
  finds the same rows by their match keys, so a stored cursor would be a
  second copy of progress.
- Delete local rows missing from the response. Rejected because the read has
  no tombstone, and a partial range would hide history the server still has.
- Retry, refresh a token, or start a historical sync. Rejected because those
  are 2.4.0. A 401 or a 429 fails the page and writes nothing for it.
- Convert a date-only bound into the athlete profile timezone. Rejected
  because D88 forbids rewriting a source bound into a timezone the source
  did not use.

## D202. The Windows cleanup and the maintainability notes are not part of this build

Checked on `main` at `8db90b42ac654767e9e9570a121bbbd4fde3348a`, which is the
merge of pull request 39.

The failed-init cleanup is still the code in `src/program.ts` (the `init`
catch closes the database and deletes the file, the wal file, and the shm
file with `rm`). `createDatabase` still deletes a half-created file in
`removePartialLocalDatabase` (`src/db/migrate.ts`) by unlinking it. Neither
path collects garbage first. `removeTemporaryDirectory` in `src/db/client.ts`
does, and tests use it. Version 2.1.0 spec section 12 left the production
paths out of that release and did not assign them a version. They are still
open. They do not run on a pull. This build does not change them.

`crates/ath-core/src/store.rs` is 1034 lines. `mapping.rs` is 1443.
`persist.rs` is 1051. The pull is a new module so those files do not grow
for HTTP. `mapping.rs` may add the count by record type that D199 names, and the
open-wearables device-binding check in the specification. That check is
shared by mapped import and pull, and it is limited to route
`open_wearables`. D203 also adds the measurement-sample target, the
reporting-window write, and the segment copy. Nothing beyond that is added
to `mapping.rs`. `store.rs` may
preserve `device_binding` inside `revise` when the record is a source, and
refuse a partial patch that contains that key. `revise` cannot set, change,
or clear the receipt. The connection is private, `ensure_mapped_source`
has no receipt argument, and `same_source_records` omits deleted rows, so
`store.rs` may also read the receipt and tombstone-inclusive history,
create a source with the checked receipt, and write that receipt on an
existing unbound source only when the same transaction shows no accepted
or deleted record and no source device. Those writes keep every other
source field and the normal revision and `data_json` bookkeeping. They
join the caller's `BEGIN IMMEDIATE` transaction. Nothing else is added to
`store.rs`. Other routes do not call them. `ath correct`, package restore,
and an accepted package merge remain the owner's whole source record.
Splitting the files is separate work. It is not required to make the pull
correct.

`check_supplied_fields` runs in `store.rs` before insert and revise (D191).
`persist.rs` `insert_record` writes `data_json` and does not call that check.
The pull writes through `apply_mapping` and the store, so the check covers
it. Closing the persist gap is separate work.

`isMappingPath` exists once, in `src/skill.ts`. Rust has no copy. The pull
uses the TypeScript function. There is no second path rule to remove.

Rejected alternatives:

- Fix the Windows cleanup, split the Rust files, and guard `persist.rs`
  inside the 2.2.0 pull request. Rejected because none of them changes what
  the pull writes, and the maintainer asked not to widen this build.
- Block the pull on those fixes. Rejected for the same reason. A later
  change can take any one of them on its own.

## D203. Stored Open Wearables measurements are kept in this pull

This supersedes the earlier draft, inside D198 and D199, that left day
values, the heart-rate stream, sleep and recovery summaries, meals, and
workout segments out of this pull. Those drafts are not deleted above.
They are not the current proposal. The specification's section 2 is the
list of reads, the storage rule, and the omissions that remain.

A raw series that 2.1.0 already stores as a point stays a point, on the
same match key. A raw `heart_rate` series is a measurement sample because
that quantity already exists and it is not a point. Every other pinned
series type that has no quantity yet gains one. The unit and the number
come from the server. A row with `period_end` is a provider summary with
period type `open_wearables_reporting_window`. `recorded_at` is unset.
`start_at` and `end_at` are the server's `timestamp` and `period_end`.

`ProviderReportingPeriodType` gains that one value. `ValueBasis` gains
`aggregator`, used when `computed_by` is `open_wearables`. Series
quantities gain the missing pinned types. `quantity` is already an
unconstrained text column, so those names add no SQL column. Step 5
rebuilds `provider_summaries` so `standard_value` and `standard_unit` can
be null together. Existing summary rows are copied with their numbers.
The device receipt still adds no column. The receipt, the transaction,
the preview counts, and the public non-writing `--json` rule are
unchanged.

The mapping file `mappings/open-wearables.md` is rewritten in the build
so a raw point, a raw sample, a day window, and a segment follow section
2. A resolved file that does not do that fails before any request. D179
still lets an installed file win. This check stops the 2.1.0 file from
dropping rows the pull can store.

Rejected alternatives:

- Keep the earlier draft and store these values in a later version.
  Rejected. The server already returns them, and a later version was
  being used as a reason to leave them out.
- Store a day row as a point at the period start. Rejected. That would
  make a reporting window look like one instant (D88, D110).
- Drop latitude, longitude, segments, or a large series to keep a
  response under 8 MiB. Rejected. The smaller page in the specification
  stores the rows. One row larger than 8 MiB is a real limit and is
  named.
- Call the personal-record route, the live provider workout proxy, or
  the totals routes so the pull can say it hit every path. Rejected.
  The personal record is birth date, gender, and sex. The provider
  workout routes read the vendor through the server's connection. The
  totals routes add up rows this pull already stores.
- Invent a midnight, a zero, or an SDNN value from an RMSSD value.
  Rejected. A null stays absent. Skin temperature is stored only from
  a response that has the field. `RecoverySummary` at the pinned commit
  does not have one.
