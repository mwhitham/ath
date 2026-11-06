# Decisions — v2.2.0

D-numbers continue from [v2.1.0](../v2.1.0/decisions.md). D198 follows D197.

These decisions are the proposal in [spec.md](spec.md). The maintainer has not
approved them. This pull request does not build them. A change of mind before
approval is an edit of the proposal. After approval, a change is a new
D-number.

## D198. Version 2.2.0 is one manual pull, and the program and the record share that number

`ath pull openwearables` reads sleep, workouts, and raw samples from an Open
Wearables server the person runs, for one user and one range they name. The
command is manual. The person runs it. The program does not schedule it, retry
it, or sign the person in to a device maker.

The program, the record, and the database become `2.2.0` together. D152 made
those one number. D184 kept that rule. Migration step 5 stores `2.2.0` and
SQL schema 5. It adds no column and changes no record. Step 4 is not edited
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

The versions named here are boundaries for this plan. They are not specs.

- 2.2.0 is this pull.
- 2.3.0 is a Junction pull. This release does not call Junction.
- 2.4.0 is automatic incremental sync, saved sign-in refresh, retries, and
  backfill. This release has none of those.
- 2.5.0 is the work of storing day aggregates and the heart-rate stream.
  This release does not store them, and it does not decide to add an Open
  Wearables period type. `ProviderReportingPeriodType` stays the Whoop, Oura,
  and Apple values it has now.
- A connector interface that every future source would implement is later
  than 2.5.0. This release adds one module for this server.

File imports stay: Apple Health, WHOOP, Oura, Garmin, and Strava. `ath pull
garmin` stays as it is. `ath import --mapping` stays as it is.

Evidence: [appendix/open-wearables-api.md](appendix/open-wearables-api.md).

Rejected alternatives:

- Leave the record at 2.1.0 while the program becomes 2.2.0. Rejected because
  D152 and D184 already rejected a split.
- Ship the pull as three pull requests. Rejected because the maintainer asked
  for one versioned build.
- Pull Junction, start sync jobs, or add a connector trait in this release.
  Rejected because each of those is a later version, and this release has to
  make one manual read reliable first.

## D199. The pull fetches pages and the shipped mapping writes the records

New Rust module `crates/ath-core/src/open_wearables.rs` requests the three
reads and calls `apply_mapping`. The command-line front end stays in
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
pull does not drop a row before the mapping sees it. A manual workout stays
the mapping's `left_out`, and the other rows in that page are still written.

The timeseries query's `types` list is the `type.<name>` lines in the
resolved mapping file, read when the command runs. Each name is one repeated
`types` query parameter. A name that appears twice is sent once.
`skin_temperature` is that case. A comma-joined value is not a series type
on the 0.11.0 server and returns 422. The Rust source has no second copy of
those names. `heart_rate` is not one of those lines, and it is not a point
quantity the mapping can write. This release does not add the line and does
not store the heart-rate stream. Workout `avg_heart_rate_bpm` and
`max_heart_rate_bpm` stay as the file already maps them.

`apply_mapping` adds counts by record type and keeps the existing `outcome`
field. The pull prints the sleep, workout, point, summary, and score counts.
It does not add `outcome` across pages. Those printed counts match the write
when each object id appears on one page. A server that repeats an id on a
later page can make the preview say new where the write says unchanged. The
write still follows D182.

Day aggregates, recovery summaries, health scores, meals, and body summaries
are not requested. On Open Wearables 0.11.0 those day values are no longer
raw samples. Storing a date as midnight would break D88. This release leaves
`ProviderReportingPeriodType` unchanged. Whether a later version adds an Open
Wearables period is a later decision. Storing the day values is named as
2.5.0 work, and that name is a boundary, not that decision.

Rejected alternatives:

- Teach the pull the Open Wearables field names in Rust. Rejected because
  D161 put those names in the mapping file so a second source does not become
  a second record format.
- Pass the sleep envelope straight to `apply_mapping`. Rejected because the
  current page rule would leave the page out. The check is `page_rows` and
  `row_fits` in `crates/ath-core/src/mapping.rs`.
- Request every series type and let `left_out` drop the rest. Rejected
  because the type list includes latitude, longitude, and other series the
  file does not store. The pull would download them for a result that cannot
  keep them.
- Map `heart_rate` samples as points, or map recovery summaries onto
  midnight. Rejected because one invents a point quantity and the other
  invents an instant (D88, D110).

## D200. The key, the server, and the user id stay out of the record

The API key is an administrator key for every user on that deployment. The
pull sends it only as the header `X-Open-Wearables-API-Key`. It is saved with
`ath key set openwearables` in the operating system password store, the same
store D75 uses. The environment name for a script is `OPEN_WEARABLES_API_KEY`.
The child process receives it as `ATH_CORE_OPEN_WEARABLES_KEY`. The key is
not a command argument. If the password store is missing, the command refuses
and does not write a file.

`ath key clear openwearables` deletes that saved key. `ath key clear` with
no name still clears only the gateway keys. The Open Wearables key is cleared
only when it is named. `ath key` keeps its gateway lines and adds one line
that says whether an Open Wearables key is saved or set in the environment.
The line does not include the key. Garmin's key command is unchanged.

The server is `--base-url` or `OPEN_WEARABLES_BASE_URL`. The user id is
`--user` or `OPEN_WEARABLES_USER_ID`. Neither is written into the database,
the export, a config file, or a log line. The pull never calls `GET /users`
and never calls the personal-record route. One command reads one user.

`--base-url` is an origin: scheme, host, and optional port. No user name, no
password, no path other than empty or `/`, no query, and no fragment. `http`
is allowed only for `localhost`, `127.0.0.1`, and `::1`. Any other host
requires `https`. The pull appends `/api/v1` itself. Redirects are not
followed. A timeout, a non-JSON body, or an HTTP error becomes the page
failure in D201. The printed error is the status and, when the body is
problem JSON, the `code`. It does not include the request path. The path
contains the user id. The response body is checked for the key before
`apply_mapping`. A body that contains the key is not written. The key is
removed from any text the command prints. A result that still contains the
key is replaced with a fixed refusal, as the Garmin pull already does for
its own key.

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
back after counting. The pull itself is a series of those calls. Sleep pages
finish before workout pages, then timeseries pages.

The command previews unless `--yes` is present. `--json` is a preview and
does not write, matching `ath import --mapping`. `--yes` writes each page as
that page succeeds. If a later page fails, pages already written stay. The
message names the read. When at least one page was written, it says those
pages are kept. It says to run the same command again. An empty range is a
successful pull with zero records. A repeat uses the
match key from D181 and the replace from D182, so an unchanged row is not
written again.

`has_more: false` is the only successful end of a read. A repeated cursor,
a null cursor while `has_more` is true, the 1000-page cap, and a body over
8 MiB fail that read. The failed page is not written. The success line is
not printed. A cursor with the `prev_` prefix is not sent.
`limit` is 1000. One attempt is made per request. There is no retry. The
connect timeout is 10 seconds and the read timeout is 30 seconds.

A row that this range did not return is left where it is. The pull does not
call `DELETE`. There is no deletion feed on the three reads. Absence can be
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
  `filter_by_priority=false`. `is_nap` is omitted, so naps and main sleep
  both return. `include` is omitted.
- Workouts: `start_date`, `end_date`, `limit`, `cursor` after the first page.
  No priority flag, because 0.11.0 has none and current `main` already
  returns every device when the flag is absent.
- Timeseries: `start_time`, `end_time`, `limit`, `resolution=raw`,
  `filter_by_priority=false`, `types` from the mapping file, and `cursor`
  after the first page.

`source_selection` is not sent. On 0.11.0 the parameter does not exist. On
current `main`, omitting it returns every device when the priority flag is
false.

Rejected alternatives:

- One transaction for the whole range. Rejected because a raw range can be
  large, and a failure at the end would discard pages that had already
  succeeded. Each page is still atomic.
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
for HTTP. `mapping.rs` may add the count by record type that D199 names, and
nothing else. Splitting the files is separate work. It is not required to
make the pull correct.

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
