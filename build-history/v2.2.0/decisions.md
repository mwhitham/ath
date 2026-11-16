# Decisions — v2.2.0

These are proposals in [spec.md](spec.md). The maintainer has not adopted
them. This pull request does not build them. A change of mind before
approval is an edit of the proposal. After approval, a change is a new
D-number.

## Numbering

These proposals are D227 through D244. They were first written as D198
through D215, when D197 was the last decision on `main`. Version 2.1.1 then
used D208 through D226. D224 keeps D198 through D207 reserved for the
unapproved 2.1.1 draft in
[v2.1.1/appendix/draft-2.1.1/](../v2.1.1/appendix/draft-2.1.1/spec.md), so
they are never reused. On 15 November 2026 each proposal moved to the next
free number, in the same order. In this folder, D208 through D226 now mean
the 2.1.1 decisions. Reviews written before that day use the old numbers.

| Old | New | Subject |
|---|---|---|
| D198 | D227 | Version 2.2.0 and one manual pull |
| D199 | D228 | Pages, and the shipped mapping writes the records |
| D200 | D229 | The key, the server, and the user id |
| D201 | D230 | A page is atomic and a repeat is safe |
| D202 | D231 | Work left out of this build |
| D203 | D232 | Stored measurements are kept |
| D204 | D233 | Meaning, source, and time |
| D205 | D234 | Units, windows, reported records, and identities |
| D206 | D235 | The new record type and refused originals |
| D207 | D236 | The migration rename, the counts, and the archive |
| D208 | D237 | The archive's safety facts |
| D209 | D238 | A typed workout is a session |
| D210 | D239 | Skin-temperature times |
| D211 | D240 | Heart-rate recovery of zero |
| D212 | D241 | A manual session window |
| D213 | D242 | A live workout original |
| D214 | D243 | The live vendor read |
| D215 | D244 | Vendor originals and credentials |

## Changes on 15 November 2026

The plan was read again against `main` at `0de71bb`, the merge of 2.1.1.
These edits follow from that reading:

- D230 and D237. A page is one 2.1.1 import preview and one apply. The
  preview never writes. The apply refuses the page when the database
  revision moved after the preview. The earlier text counted a page inside
  a write transaction, rolled the count back, and read the safety facts a
  second time under the write lock. Version 2.1.1 has no such step.
- D236 and D237. A deleted source or a deleted record leaves its rows out,
  as D182 and D190 say. It no longer refuses the page. It still stops the
  archive of that page.
- D227 and D236. Step 6 runs on the store's own connection. Foreign keys
  are turned off before its transaction starts, in `src/store/migrate.rs`.
- D228. The pull gives each page to a reader that holds the page in
  memory. `MappedReader` in 2.1.1 reads a file path.
- D229. The Open Wearables key is a third `KeyAccount`, beside the gateway
  keys and the Garmin token. `ath key clear garmin` stays refused, as it is
  in 2.1.1.
- D230, D232, D233, and D243. Limits and timeouts are named constants, as
  2.1.1 does. One response may be at most `MAX_PULL_RESPONSE_BYTES`, which
  is 10 MiB. The earlier text said 8 MiB.
- D231. The source rules go in `src/import/provider.rs` and a new
  `src/import/binding.rs`, where 2.1.1 stages and applies an import. They
  no longer go in `src/store/write.rs`. The protocol's record writes join
  the owner writes that store a whole source record.
- D232, D235, and D236. Sentences that called the new migration step 5 now
  say step 6. Step 5 is the published 2.1.1 migration.

These edits answer the thirteen findings of the review at `e145fa6`:

- D242. The stored original's JSON layout, its key, its counts, and owner
  deletion and correction.
- D243. Connections, statuses, Suunto keys, detail rules, timeouts, request
  budgets, Garmin windows, id spelling, start times, and the count of every
  object left out. Garmin windows are 23 hours, not 24, because the pinned
  server measures a window in naive local time.
- D244. A credential-shaped value is withheld and counted. It no longer
  refuses the page.

## Changes after the review at `d636d7a`

These edits answer the review of this plan at `d636d7a`:

- D227. A new database runs its migrations on the connection
  `Store::create` opened, not through `Store::transaction`. The two 2.1.1
  checks that expect one transaction to reach the last schema are named,
  with the check that replaces each.
- D230. The 2.1.1 comment on the 10 MiB response cap names only Garmin.
  The build rewrites it and the route list to name both pulls.
- D231. The `src/import/provider.rs` list is complete. It adds a
  `connector` source call, because `route_source` stages only
  `export_file` sources, and a count by record type in
  `src/import/staging.rs`. The revision a preview read is already public.
- D243. A Garmin detail that answers 404 or 501 is `not_available`, not
  `failed`. Garmin stays `completed`, and the list object is stored whole.
  The pinned detail path is not in Garmin's public docs.
- D242. On route `open_wearables`, an owner correction is replaced only
  when the server's data differs from the newest revision an import wrote.
  Each replaced correction is counted per provider and for stored records,
  in the text, in `--json`, and in the preview. The skill says where a
  workout fix belongs.
- D243. An HTTP 429 stops that provider for the command, as a timeout
  does. Credential values are withheld before any comparison.
- D230 and D236. A committed vendor page is a protected write. It sets
  `written` and `database_changed`, and its originals add to
  `committed.source_report`.
- D243. When one Garmin activity id is in two windows with different
  objects, the later window's copy is kept and the earlier is counted as
  `superseded`.
- D242. Vendor originals have a 128 MiB total bound, because a package
  holds them in its main document under the 512 MiB read cap. `ath export`
  refuses to publish a document over that cap.
- D243. A Suunto list object with no `workoutKey` gets no detail call and
  is `not_available`. A detail over the response cap is `too_large`, and a
  detail the privacy scan refuses is `refused`. Both make the provider
  `incomplete`.
- D243. A preview reads each list and makes no detail call, so a preview
  then `--yes` reads each list twice and makes each detail call once. One
  command sends at most 100 Suunto list requests. A detail can come
  minutes after its list object, and the plan says so.
- D244. The page cursors `next_token` and `nextToken` are not
  credential-shaped. A user name or password in a URL, and a
  credential-shaped pair in a URL fragment, are withheld too.
- D243. The daylight-saving sentence names both directions. A forward
  change makes 24 real hours read as 25 local hours, and a backward change
  makes them read as 23. A zone that moves its clock by two hours is not
  covered by the 23-hour window.
- D231. The `__proto__` citation names `check_supplied_fields`, which
  still refuses the key at any depth on import. D214 in 2.1.1 changed only
  the native reader.
- D243. For Garmin, Suunto, Whoop, and Oura, which name no detail field, a
  new detail with fewer fields replaces the stored one, and the stored
  detail stays in the record's history.
- D230. Each of the four page refusals names the owner's fix, or says
  2.2 has none.

## Changes after the review at `3c3f660`

These edits answer the reviews of this plan at `3c3f660`. No decision
number is added:

- D243. The opening of the specification says what the flag reads on a
  preview and on `--json`: the connections list and each active
  provider's list, with no detail call. The help text, the examples, and
  their test say the same.

## D227. Version 2.2.0 is one manual pull, and the program and the record share that number

`ath pull openwearables` reads the stored representations D232 lists from an
Open Wearables server the person runs, for one user and one range they name. The
command is manual. The person runs it. The program does not schedule it,
retry a failed request, or sign the person in to a device maker.

The program, the record, and the database become `2.2.0` together. D152 made
those one number. D184 kept that rule. Version 2.1.1 kept it too. Migration
step 5 is already published. It stores `2.1.1` and SQL schema 5 in
`spec/sql/migrations/005-version-2-1-1.sql`. This pull adds step 6. Step 6
stores `2.2.0` and SQL schema 6. It adds no column to an existing table. It
rebuilds `provider_summaries` and `provider_scores`, and it creates
`source_reports`, as D234 describes. D236 drops the record views and
the two cross-table triggers around the root rename. Existing rows keep
their numbers. Steps 1 through 5 are not edited (D165).

In 2.1.1, `install_schema` in `src/store/mod.rs` runs the whole chain in one
transaction with foreign keys on. `Store::create` calls it on a new
connection before any `Store` exists, and it opens its own transaction.
`ath migrate` and `apply_pending_without_backup` run their steps inside
`Store::transaction`, which refuses to write when foreign keys are off.
Step 6 needs them off while it rebuilds tables that other tables point to.
So `install_schema` commits steps 1 through 5 first, and step 6 runs in its
own transaction on the same connection. Step 6 is one function in
`src/store/migrate.rs` that takes the connection, so init and migrate both
call it. It turns
foreign keys off before that transaction begins, because SQLite ignores the
setting inside a transaction, and reads the setting back. The step runs
`PRAGMA foreign_key_check` and refuses any row it returns. It sets
`user_version` 6 and commits. Foreign keys are turned on again and read
back, whether the step committed or not.

Two 2.1.1 checks expect one transaction to reach the last schema.
`install_schema` refuses unless the metadata reads `SQL_SCHEMA_VERSION`
and `ATHLETIC_STANDARD_VERSION` (`src/store/mod.rs`, lines 625 through
629 at `0de71bb`). `apply_steps` refuses unless its last step reached
`SQL_SCHEMA_VERSION` (`src/store/migrate.rs`, lines 321 through 326). With
the constant at 6, both would fail once steps 1 through 5 commit at schema
5. So the first transaction checks for schema 5 and `2.1.1`. `apply_steps`
checks for the last step it was given. The step-6 function checks for
schema 6, `2.2.0`, and `user_version` 6 before it commits.

`ath migrate` keeps its verified
backup. Inside the step-6 transaction it checks again that the file, the
database ID, the schema, and the database revision are the ones its earlier
steps left. `MIGRATION_CHAIN` in that file and `spec/sql/contract.json` gain
the step.
`packages/npm/package.json` exports point at `schema/2.2.0/`.
`READABLE_DATABASE_VERSIONS` and `ACCEPTED_RECORD_VERSIONS` keep `1.0.0`,
`2.0.0`, `2.1.0`, and `2.1.1`, and gain `2.2.0`. `IMPORTABLE_PACKAGE_VERSIONS`
gains `2.2.0` and keeps `2.0.0`, `2.1.0`, and `2.1.1`. A `2.1.1` package
still imports. A `2.1.1` database does not open until `ath migrate`.

The build is one pull request after this plan is approved. It is not split
into a version change, a client, and a mapping change.

The version numbers below are proposals. They are not specs, and they are
not adopted.

- 2.2.0 is this pull. D232 says which stored measurements it keeps.
- 2.3.0 is a proposed Junction pull. This release does not call Junction.
- 2.4.0 is proposed automatic incremental sync, saved sign-in refresh,
  retries, and backfill. This release has none of those. A smaller page
  after the local response cap, described in D232, is not that retry.
- An earlier draft of this plan left day aggregates and the heart-rate
  stream out and suggested 2.5.0 for them. D232 supersedes that draft.
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
  by D232. The current proposal stores them in this pull and says so.

## D228. The pull fetches pages and the shipped mapping writes the records

New module `src/runtime/open_wearables.rs` requests the reads
in D232 and gives each page to the mapping engine. In 2.1.1, `MappedReader`
in `src/import/mapping.rs` reads a JSON file from a path. The pull adds a
second reader beside it, `MappedPage`, that holds one decoded page in memory.
It runs the same checks as `MappedReader::read` and the same
`Run::map_input`. `preview_route` in `src/import/provider.rs` stages and
matches the page, and `ProviderPreview::apply` writes it. The command is
added in `src/cli/`,
beside `ath pull garmin` in `src/cli/commands/routes.rs`. Version 2.1.1
superseded D174's TypeScript front end and the separate `ath-core` library
in D155 and D162. This pull does not bring either back. The library is the
`athleticstandard` crate at the repository root. The HTTP client is
the `ureq` 2.12 dependency the Garmin pull already uses. No new crate.

The mapping file is resolved with `resolve_mapping` in `src/assets/mod.rs`,
name `open-wearables`. An installed copy in `.agents/mappings/` wins, which
is D179. The pull does not grow a second rule for what a path looks like.
`is_mapping_path` in that file is the one rule.

The mapping file chooses every stored field. The pull does not rename a
field, choose a unit, or decide that a value is RMSSD or SDNN. A field the
file does not name stays in `supplied_fields` under the name it arrived with
(D153, D161). `field_basis` stays whatever the file already states (D185,
D187, D192).

Sleep and workout responses are envelopes with `data` and `metadata`. The
shipped file's page section is `timeseries`, so the mapping engine would
leave that envelope out. The pull passes sleep rows as `{ "sleep": [ ... ] }` and
workout rows as `{ "workout": [ ... ] }`. It passes each timeseries response
through unchanged, so `metadata.resolution` still reaches the mapping. The
pull does not drop one row and keep its neighbors. The API key, the
server's origin or the selected user id echoed in a response, a same-key
`device_id` collision, or a bad envelope rejects the whole page first. A
value under a credential-shaped key is withheld and counted instead, as
D244 says. A manual workout is written. D238 stores
it as a workout session. The other rows in that page are still written.

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
series. D232 omits `types` on the raw read and the day read for that
reason. A mapping that cannot store those series fails before any network
call. A comma-joined type string is still not sent.

`heart_rate` is a series quantity. D232 stores the raw stream as
measurement samples. Workout `avg_heart_rate_bpm` and
`max_heart_rate_bpm` stay as the file already maps them.

The pull counts each page by record type from its staged plan, which
`ProviderPreview::planned` already lists. The mapped-import report from
`report_value` in `src/import/provider.rs` keeps its `outcome` field. The
pull adds the counts into the committed result for a page it applied, and
into `estimated` for a page it only previewed. It does not add `outcome`
across pages. A preview writes nothing, so estimated counts are against the
database as it was before this command. They are not equal to the write
when the same match key appears on two pages, or when the server changes
between runs. The write still follows D182. This proposal does not keep a
command-wide preview overlay.

The read order is the order in D232. A failure in a later read
leaves the earlier read's pages committed and the reads after the failure
`not_attempted`. The result says so. See D230.

D232 is the coverage rule. An earlier draft of this decision left the day
rows, the summaries, and the heart-rate stream unrequested. That draft is
superseded. D88 still forbids inventing a midnight. The period type in
D232 stores the bound the server sent. An Open Wearables window is not
labeled as a Whoop cycle or an Oura day.

Sleep requests `include=stages`. Workouts request `include=zones` and
`include=segments`. Stage minute totals return either way. The timeline,
the zones, and the segments are null when the expansion is omitted, so
omitting it would drop them before `supplied_fields` can keep them. A page
over the response cap is handled as D232 says.

Rejected alternatives:

- Teach the pull the Open Wearables field names in Rust. Rejected because
  D161 put those names in the mapping file so a second source does not become
  a second record format.
- Pass the sleep envelope straight to the mapping engine. Rejected because
  the current page rule would leave the page out. The check is `page_rows` and
  `row_fits` in `src/import/mapping.rs`.
- Request every series type and let `left_out` drop the ones the mapping
  cannot store. Rejected. D232 stores them. `left_out` remains a row the
  server returned and this program refused for a named reason.
- Map `heart_rate` samples as points, or map a day summary onto midnight.
  Rejected. `heart_rate` is stored as a measurement sample. A day row is
  stored as a reporting window (D88, D110, D232).
- Refuse an empty `types` list in order to avoid heart rate and location.
  Superseded by D232. Omitting `types` is how 0.11.0 returns every series,
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
  source may already hold an arbitrary value at that name, and step 6 must
  not turn that value into authority. The receipt is the source object's own
  key `device_binding`.
- Treat a nonempty string at the new key as proof no matter who wrote it,
  or require a signature so the owner cannot change it. Rejected. The
  automatic path writes the key only by the checked rule. `ath correct`,
  package restore, and an accepted package merge store the whole source the
  owner supplied. A hand-edited database file is outside this proposal.

## D229. The key, the server, and the user id stay out of the record

The API key is an administrator key for every user on that deployment. The
pull sends it only as the header `X-Open-Wearables-API-Key`. It is saved with
`ath key set openwearables` in the operating system password store, the same
store D75 uses. The environment name for a script is `OPEN_WEARABLES_API_KEY`.
The command reads that variable and the password store in this process,
in `src/runtime/keyring.rs`. There is no child process and no
`ATH_CORE_OPEN_WEARABLES_KEY`. `ath pull garmin` already reads
`GARMIN_PERSONAL_API_KEY` the same way. The key is
not a command argument. If the password store is missing, the command refuses
and does not write a file.

Open Wearables is not a gateway. `Gateway::ALL` in `src/runtime/keyring.rs`
stays `vercel` and `openrouter`. `KeyAccount` in that file gains
`OpenWearables`, saved under the account `openwearables`, the way 2.1.1
added `Garmin`. `account_secret` reads `OPEN_WEARABLES_API_KEY` before the
saved key, as it reads `GARMIN_PERSONAL_API_KEY` for Garmin. `key_clear` in
`src/cli/commands/model.rs` accepts `openwearables` before it parses a
gateway name. `ath key clear` with no name still clears only the gateway
keys, and it still exits successfully when that is what it did.
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
Garmin's key command is unchanged. In 2.1.1, `ath key clear garmin` is
refused because `garmin` is not a gateway name. This pull keeps that, and
the specification's section 9 lists it as a separate follow-up.

The server is `--base-url` or `OPEN_WEARABLES_BASE_URL`. The user id is
`--user` or `OPEN_WEARABLES_USER_ID`. Neither is written into the database,
the export, a config file, or a log line. The pull never calls `GET /users`.
D233 reads the personal record and drops its `user_id` before the scan.
Birth date, gender, and sex are stored. One command reads one user.

`--base-url` is an origin: scheme, host, and optional port. No user name, no
password, no path other than empty or `/`, no query, and no fragment. `http`
is allowed only for `localhost`, `127.0.0.1`, and `::1`. Any other host
requires `https`. The pull appends `/api/v1` itself. Redirects are not
followed. A timeout, a non-JSON body, or an HTTP error becomes the page
failure in D230. The printed error is a fixed sentence and the HTTP status. A problem `code`
is printed only when it is one short token and does not contain the key,
the origin, or the user id. Otherwise it is withheld. A transport error or
a parser error is replaced with a fixed sentence that has no URL, no path,
and no user id.

After the body is read inside the size cap and parsed, and before the page
reaches `MappedPage`, `open_wearables.rs` walks every object key and every
string. A value that contains the API key, the selected origin, or the
selected user id rejects the whole page. The row is not rewritten. A value
under a credential-shaped key is a different case. D244 withholds that one
value and counts it. A scan of the raw text
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

## D230. A page is atomic, a missing row is not a deletion, and the same command is safe to repeat

Each response page is one 2.1.1 import preview and, under `--yes`, one
apply. `preview_route` in `src/import/provider.rs` stages and matches the
page in one read transaction, at one database revision, and never writes.
`ProviderPreview::apply` writes that plan in one transaction, as D189
requires. It refuses the plan when the database revision moved after the
preview. A failure writes nothing for that page. The pull itself is a
series of those pages. The order and the terminal condition of each read
are the table in the specification's section 2. A committed page follows
D189. A committed page does not mean the pull is complete.

The command previews unless `--yes` is present. `--json` does not write,
with or without `--yes`, matching `ath import --mapping`. `--yes` without
`--json` writes each protected page as that page succeeds. A refused
page is not a success. The pull-only archive in D237 is a separate
preview and apply. It goes ahead only when the database revision has not
moved since the protected preview.

The command returns one bounded outcome for the mode it ran. A preview,
including public `--json`, has `written` false, protected and archive
committed zeros, estimated counts, and `range_complete` false. A
confirmed `--yes` has the actual protected and archive counts. `written` is
true when a protected transaction in the command created or revised a
counted record. An archive does not make it true. `database_changed` is
true when a protected transaction or an archive transaction committed a
change. A vendor page (D242) is a protected transaction, so a committed
vendor original makes both flags true even when every stored page was
unchanged. A successful repeat of an unchanged command has both flags false.
The command prints the
confirmed outcome as text. It prints the preview outcome as public JSON.
On a partial failure it keeps the confirmed outcome and then exits
nonzero. It prints the text before it uses that status. There is no
separate child process. Version 2.1.1 removed that split.

Each read in that table is `completed`, `failed`, or `not_attempted`. The
token is `not_attempted` in the outcome and in the text. There is no
`skipped` read. `range_complete` is true only on a `--yes` write whose
ranged reads reached their terminal conditions. The success sentence also
requires the personal record and the body snapshot to have completed. A retained
count with an incomplete range is a partial pull. The message names the
failed read, names any `not_attempted` read, and gives the retained counts.
A failed read says to run the same command again. An identity refusal uses
the sentence in the specification's section 3: a replacement id is blocked
even at a new time, a 2.1.0 Open Wearables history cannot be refreshed until
reconciliation, and
a rerun cannot repair either one. Each page refusal also names the
owner's fix where 2.2 has one. `ath correct` on the source can set a new
`device_binding`, bind history that one device wrote, or replace a
malformed receipt. A changed workout kind has no fix that keeps the
record. `ath erase` removes it, and the next pull stores the new kind. A
bound source that receives rows with no id, and history from more than
one device, have no fix in 2.2. A preview that fails makes no database
changes, including for pages that had already been counted.

Before `MappedPage`, a cursor page and an offset page must be a JSON
object, `data` must be an array, `pagination` must be an object, and
`has_more` must be a boolean. Anything else fails that page. It is not an
empty success. Body may be one object or JSON `null`. `null` is a completed
empty snapshot. The personal record is one object. The result keeps a
`left_out` total, a count for each reason, and at most 20 examples.

An empty ranged result whose ranged reads completed has `range_complete`
true. The personal record and the body snapshot can still be written.
Those writes are counted. They are not ranged rows. A repeat uses the match
key from D181 and the replace from D182, so an unchanged row is not written
again. An owner correction on this route is kept until the server's data
for that record changes (D242). A rerun after a partial write leaves the committed rows in place,
runs the failed read again, and runs any read that was `not_attempted`.

`has_more: false` is the successful end of a cursor read and of the offset
read. A repeated cursor, a null cursor while `has_more` is true, and the
1000-page cap fail that read. An offset retry uses the same offset. A body
over the response cap, `MAX_PULL_RESPONSE_BYTES` in `src/input_policy.rs`,
is handled by D233. The failed page is not written. Later reads are
`not_attempted`. The success line is not printed. A cursor with the
`prev_` prefix is not sent. `limit` starts at 1000. One attempt is made per
request, except the smaller page D233 sends after the response cap. A
transport error is not retried. The stored reads use a 30-second connect
timeout and a 120-second read timeout, the values
`src/runtime/garmin_pull.rs` uses. Live vendor requests use 30 and 360
seconds, for the reason in D243. Each timeout is a named constant. In
2.1.1 the comment on `MAX_PULL_RESPONSE_BYTES` calls it one Garmin
personal connection response, and the route list above it names only the
Garmin pull. The build rewrites both to name the Open Wearables pull too.
The value stays 10 MiB.

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

The queries are the table in the specification's section 2. An earlier
draft of this proposal listed three reads and treated every page as a
cursor envelope. D233 supersedes that draft. `source_selection` is not
sent. On 0.11.0 the parameter does not exist. On current `main`, omitting
it returns every device when the priority flag is false. Summary routes
still select a source. That selection is not a client filter.

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

## D231. The Windows cleanup and the maintainability notes are not part of this build

Checked again on version 2.1.1 at `0de71bbd6474916af0408a4090bc2c74a1a3aed2`,
the merge of pull request 44. The earlier check was version 2.1.0 at
`8db90b42ac654767e9e9570a121bbbd4fde3348a`, the merge of pull request 39.

Failed `ath init` still deletes the new file, the wal file, and the shm
file. `Store::create` in `src/store/mod.rs` calls `remove_partial_database`,
which unlinks those three paths. The `init` command in
`src/cli/commands/init.rs` unlinks the same three paths if creation fails
after that. Neither path collects abandoned temporary directories first.
`make_owned_temporary_directory` in `src/files/staging.rs` does, and import
working data uses it. The database file is not one of those directories.
Version 2.1.0 spec section 12 left the production database cleanup out and
named no later version. Version 2.1.1 did not take that work. The cleanup
does not run on a pull. This build does not change it. D237 rolls a failed
step 6 back before this cleanup runs.

`src/import/mapping.rs` is 1703 lines and `src/import/provider.rs` is
1297. The store is already several files. `src/store/mod.rs` is 848
lines, `src/store/write.rs` is 508, `src/store/encode.rs` is 549, and
`src/store/check.rs` is 1044. `persist.rs` is gone. The pull is a new
module so `mapping.rs` and the store files do not grow for HTTP.
`mapping.rs` may add `MappedPage`, which D228 names, and the
`vendor_workout` section, which D242 names. D232 also adds the
measurement-sample target, the reporting-window write, and the segment
copy. Nothing beyond that is added to `mapping.rs`.

Version 2.1.1 stages, matches, and applies a keyed import in
`src/import/provider.rs`. A keyed import there only creates sources. It
never revises one. So the source rules go there, not in
`src/store/write.rs`. The matching step runs the open-wearables
device-binding check and reads the page's safety snapshot. Both live in a
new file, `src/import/binding.rs`, so `provider.rs` grows little. That
check is shared by mapped import and pull, and it is limited to route
`open_wearables`. The staged plan may create a source with the checked
receipt. It may write that receipt on an existing unbound source only when
the snapshot shows no accepted or deleted record and no source device.
That first bind is a source correction with the source's expected
revision. `ProviderPreview::apply` refuses any other staged change to
`device_binding`. Those writes keep every other source field and the
normal revision and `data_json` bookkeeping. `provider.rs` also gains the
crate-private read of a stored vendor original that D243 uses. It gains
`connector_source` beside `route_source`, because `route_source` stages
only an `export_file` source that names a vendor. The writer
`open_wearables` sources, the archive source, and each `vendor_workout`
source are `connector` sources with no vendor. `Staging::count` in
`src/import/staging.rs` counts by action only, so a count by record type
goes beside it for the page counts. The revision a preview read is
already public as `ProviderPreview::destination_database_revision`.

`Store::apply` in `src/store/write.rs` writes a whole record. There is no
`Store::revise`, and a correction carries the whole record. The owner and
library writes store a whole source record: `ath correct`, the protocol
operations `records.write`, `records.write_changes`, and `records.correct`,
native package restore, and an accepted package merge. They keep the
receipt the caller supplied. `Store` keeps its connection private (D223).
D233 also allows `measurement_sample` insert and update, and a null value
and unit pair on provider summaries and provider scores. A half-null pair
is refused. `write_current_state` in `src/store/encode.rs` is the only
record-to-table mapping. Package restore, repair, and the record check use
it. There is no TypeScript writer to update. D234 also allows
`source_report` insert and update, and withdrawal of a child whose anchor
is in this response. Nothing beyond that is added to the store. Other
routes do not call the receipt operations. Splitting `mapping.rs` is
separate work. It is not required to make the pull correct.

`ath key clear garmin` is refused in 2.1.1, because `key_clear` in
`src/cli/commands/model.rs` accepts only gateway names. D229 keeps that.
Making the Garmin key removable is separate work.

`check_supplied_fields` is in `src/import/shared.rs`. A keyed import calls
it before it builds a candidate (D191). It refuses an empty name and a
`__proto__` key at any depth. Version 2.1.1's D214 changed only the
native reader, which keeps such a key and checks it, so the import
refusal still applies. There is no `persist.rs` and no
`ath-core persist` command. The pull writes through `MappedPage` and the
2.1.1 import, so the import check covers it. A name check outside import
is separate work.

`is_mapping_path` exists once, in `src/assets/mod.rs`. The pull uses
`resolve_mapping` in that file. There is no second path rule to remove.

Rejected alternatives:

- Fix the Windows cleanup, split `mapping.rs`, and add a name check outside
  import inside the 2.2.0 pull request. Rejected because none of them changes
  what the pull writes, and the maintainer asked not to widen this build.
- Block the pull on those fixes. Rejected for the same reason. A later
  change can take any one of them on its own.
- Put the source rules in `Store::apply`. Rejected. In 2.1.1 every
  `Store::apply` caller that can write a source is an owner or library
  write, and the one automatic source writer is `ProviderPreview::apply`.
  A rule in `Store::apply` would also block the owner's own correction.

## D232. Stored Open Wearables measurements are kept in this pull

D233 supersedes the classification, source identity, time, score, endpoint,
and size sentences in this proposal. D234 supersedes D233 where they
disagree. The decision to keep the returned health data in this pull
stands. Where a paragraph below disagrees with D237, D237 is the proposal.

This supersedes the earlier draft, inside D227 and D228, that left day
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
unconstrained text column, so those names add no SQL column. Step 6
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
  response under the response cap. Rejected. The smaller page in the specification
  stores the rows. D233 states the minimum-page limit without naming a
  series type.
- Call the live provider workout proxy or the totals routes so the pull
  can say it hit every path. Rejected. Those routes are not the stored
  measurements. D233 does read the personal record. An earlier sentence
  in this list left it out. That sentence is superseded.
- Invent a midnight, a zero, or an SDNN value from an RMSSD value.
  Rejected. A null stays absent. `RecoverySummary` at the pinned commit
  has no skin-temperature field. A health-score component can still carry
  one. D233 says so.

## D233. Returned values keep their meaning, source, and time

D234 supersedes the sentences below that call dietary or index series
provider summaries, that store a day row as the same class as its raw
sample, that build a sensor by joining parts with `:` and `-`, that put
main-sleep bounds on a summed field, that leave a missing device id as
`left_out`, that omit a manual workout, and that treat `received_at` as
the time a revision was written. The values stay in this pull. The
device-receipt rules stay. Where this proposal disagrees with D234, D234
is the proposal.

This corrects D232. The corrections that D234 does not replace are the
contracts in the specification's section 2.

A device source is still the 2.1.0 sensor, and its receipt rules are
unchanged. A summary the server builds without `device_id`, a meal or
cycle without one, a null timeseries source, and a health score use the
separate sensors in that section. They do not borrow a device id, and
their history does not block a later device binding. A raw or day sample
that has a source and no device id is still refused.

A number is classified before it is normalized. Existing raw points, raw
`vo2_max`, and the two raw Garmin scores keep their kinds and keys. Dietary
series and the named indexes, ages, and effort scores are provider
summaries. The other raw series readings are measurement samples. Day rows
use the same classes on the server's calendar window. That window is not
the provider's original interval.

Health-score `provider` is the author. `source` is the input and may be
null. `provider: internal` is basis `aggregator`. A finite range in
`constants/health_scores.py` is the scale. An unknown or unbounded range
is not given `0-100`. The original object, including components and
`event_record_id`, is kept.

Sleep main-sleep bounds apply only to the fields section 2 names. Body is
the server's current snapshot, not the requested range. Fields with no
time use `not_provided_by_source` and no invented instant. Age is
calculated. BMI is not labeled measured. The personal record's birth date,
gender, and sex are stored. Its `user_id` is not. The athlete profile is
not used for them, because that record cannot hold a full birth date, the
gender values, or a source.

Cursor pages, offset pages, and the two one-shot reads have the terminal
conditions in the table. A smaller page repeats the same uncommitted
cursor or offset. A minimum page or an unpaginated response that is still
over the response cap, 10 MiB, is an incomplete read. The message names the read. It does not
name a series type. Menstrual cycles keep an in-range start and a future
end. A later start is not stored. The page is still followed to the end.

Step 6 rebuilds `provider_summaries` and `provider_scores` so value and
unit are both null or both present. It copies existing rows and keeps
their ids, revisions, import keys, and tombstones. `src/store/write.rs`
may insert and update samples and those paired-null rows.
`write_current_state` in `src/store/encode.rs` writes the same pair.
Package restore, repair, and the record check use that encoder. Half-null
is refused. No other store change is included.

Rejected alternatives:

- Put a summary with no device id on the bound device source. Rejected.
  The summary would be refused, or the missing id would be invented.
- Leave birth date, gender, and sex out because they are sensitive.
  Rejected. They are health data. The user id is the part that stays out.
- Use the athlete profile for those three fields. Rejected. The profile
  has no source, no full birth date, and no room for every gender value.
- Call every numeric series a measurement sample. Rejected. Intake,
  indexes, ages, effort scores, and the existing summary and score lines
  would change meaning or identity.
- Treat body as a read of `--from` and `--to`, or date the undated fields
  with the average window or the pull time. Rejected. The route has no
  date parameters, and those times would be invented.
- Give every health score the scale `0-100` or the basis `provider`.
  Rejected. The pinned range table and `provider: internal` say otherwise.
- Stop a day read at `limit` 1 and report one oversized series. Rejected.
  That limit counts periods, and one period can hold many rows.

## D234. Units, windows, reported records, and identities stay distinct

D235 supersedes the sentences below that put every source-present series
through the device check, that compare null-source rows by type and
timestamp alone, that leave a refused original unstored in every case,
that map `heart_rate_recovery_one_minute` to a quantity of that same
name, and that call `hydration` a measurement sample. The units, day
windows, sleep bounds, and reported-record direction stay. D236
supersedes D235 where they disagree. D237 supersedes D236 where they
disagree. Where this proposal disagrees with D237, D237 is the proposal,
except the sentences that store a manual workout as a source report.
D238 supersedes those sentences.

This corrects D233. The returned health data stays in this pull. The
specification's section 2 and the appendix series table are the contract.

Every series has an input number, an input unit, a standard number, and a
standard unit. The input pair is the server's pair. The standard number
equals the input number except for `walking_step_length` and
`running_stride_length`. Those quantities are metres in Athletic Standard
and centimetres on the server, so the standard number is the input number
divided by 100. A 70 cm step is 0.70 m. The aliases change the unit text
and not the number: `percent` to `%`, `celsius` to `°C`, `degrees` to
`deg`, `meters` to `m`, `m_per_s` to `m/s`, `watts` to `W`, `met` to
`MET`, `minutes` to `min`, and `ml_kg_min` to `ml/kg/min`. A new quantity
uses that alias, or the server unit token when it has no alias. Existing
quantity units are not edited.

A raw row keeps the class in the appendix table. A day row is never a
measurement sample. Its `timestamp` is the start of a calendar period, and
`window_seconds` is null. The stored period is
`open_wearables_reporting_window`, from that timestamp to `period_end`,
with `recorded_at` unset. The window is not called the measurement
instant. Raw points, raw `vo2_max`, and the two raw Garmin scores keep
their kinds and keys. Day heart rate, day steps, and day speed are window
summaries. Day Garmin scores stay scores on the window.

Sleep summary numbers cover every non-nap session in the selected source
and date. The longest session supplies `start_time` and `end_time` on the
original object only. Those bounds are not a period on the summed fields.
A 360 minute total from two main sleeps stays on `provider_date`. This
program does not re-sum the sessions and does not invent the missing
interval.

`source_report` is a new record type. It has a source, so it is not an
athlete report and not a nutrition entry. It is not one of the seven
measured types. D161 still forbids turning a personal report into a
measured record. Dietary series, alcoholic-beverage counts, meal and
nutrient numbers, index, age, and effort series, personal-record fields,
predicted and coded cycle content, BMI, calculated age, and an unbounded
health score use this type. The eight range calculations and
`provider-period-summary` do not read it. A manual workout is a workout
session. D238 is that rule. An automatic workout stays a measured
session on the device source.

A new sensor is `ow1:` plus a JSON array with no spaces. JSON `null`, an
empty string, and `-` stay different. Legacy device sensors are not
rewritten. A health score's sensor is `["score", provider]`, using the
top-level author. The input source is not in that array, so a change from
null to a known input revises the same score. A null-source timeseries
uses `["unattributed", "timeseries"]` and the writer `open_wearables`.
Two unequal payloads at the same type and time refuse the call. The
personal record uses `["personal_record"]` and that same writer. A meal
or cycle uses the event array even when `device_id` is present. That id
stays in the array and in the original object. It does not establish
`device_binding`. A manual workout does not use that array. D238 puts it
on the manual source.

A raw or day row whose `source` object has no `device_id` is an incoming
device row. The whole call is refused, including a valid row on the same
page. It is not marked `left_out` before the check. Null-source rows do
not enter that check.

The same returned object can change a child from a number to null or
remove a nutrient. That child is revised to an absent value and unit.
The id and the first `received_at` stay. D182 copies that `received_at`
onto the revision. `became_known_at` is when the revision was written. A
later number returns on the same child. A range that does not return the
object does not withdraw its children. A refused page rolls the
withdrawal back.

A finished range with a failed body or personal-record read says the
range is complete, names the failed snapshot, and exits nonzero. It does
not print the success sentence. A preview says a snapshot would be
stored. It does not say it was stored. An empty ranged result does not
mean the command wrote nothing.

`computed_by` omitted on a raw row keeps basis `provider_or_aggregator`.
`computed_by: null` omits the basis. The raw Garmin scores keep basis
`provider`. `provider: internal` keeps basis `aggregator`. The scale text
comes only from `HEALTH_SCORE_RANGES`, including the `1-100` rows.

Rejected alternatives:

- Store a day heart-rate, step, or speed row as a measurement sample at
  the calendar start. Rejected. That start is not a measurement instant,
  and a sample requires `recorded_at`.
- Add an interval-sample table so a day row can carry a duration.
  Rejected for this release. The reporting window already holds the
  bounds the server returned, and a day row does not carry
  `window_seconds`.
- Put the longest sleep's bounds on the summed duration. Rejected. The
  sum covers every non-nap session, and the bounds cover one session.
- Keep colon-joined sensors with `-` for a missing part. Rejected. A
  colon in a part and a literal hyphen collide with that encoding.
- Treat a second null-source payload as a correction. Rejected. There is
  no sample id, so the program cannot tell a correction from a second
  observation.
- Call dietary intake a provider summary or a workout session. Rejected.
  SPEC section 5.4 and D161 keep those measured types for measured
  records. A source report preserves the object without making it a
  measurement.
- Call a manual workout a provider summary. Rejected. D238 stores it as
  a workout session. It is not a provider summary.
- Leave a removed nutrient current because the store compares emitted
  keys. Rejected. The same object no longer contains that number.

## D235. The new record type, numberless inputs, and refused originals stay separable

D236 supersedes the sentences below that leave the record views in place
during the rename, that set `written` false for an archive without
defining `database_changed`, that archive a page from the first error,
that hash the series row without its resolution, and that give a
null-source reported series both the report sensor and the unattributed
sensor. The root check, the numberless readers, and the reported-record
direction stay. D237 supersedes D236 where they disagree. Where this
proposal disagrees with D237, D237 is the proposal.

This corrects D234. The returned health data stays in this pull. The
specification is the contract.

`records.record_type` is a closed check. `source_report` is not in it.
Creating `source_reports` does not make a root row insertable. Step 6
adds `source_report` to that check in the generated contract. It does
not edit migrations 001 through 005, and it does not drop the check.
Ordinary writes keep foreign keys on. The migration connection may turn
them off only before its own transaction, rebuild `records` and the two
nullable typed tables by copy, rename, and recreate, then require
`foreign_key_check` and `integrity_check` before commit. D236 drops the
two record views and the two cross-table triggers before that rename and
recreates them unchanged. A drop of
`records` inside the ordinary foreign-key transaction is not the
procedure. Fresh init and a populated, deleted, history-bearing upgrade
both have to accept a source report afterward.

A numberless provider summary or provider score is excluded by the
instant reader and the period reader before the limit, the count, and
the numeric decode. It is not zero, and it is not the previous revision.
Source reports stay out of those readers. The formulas stay.

`RecoverySummary.recovery_score` takes its scale from
`HEALTH_SCORE_RANGES` and the provider the builder copied. Polar recovery
is 1 to 6. A provider the table does not name is a source report. The
WHOOP-only prose is not the scale.

Raw `heart_rate_recovery_one_minute` is quantity `hr_recovery`. The
server name and the old key stay. `hydration` is a source report. A
device id does not make that intake a sample.

Sleep, workout, meal, cycle, and score ids must be UUIDs. A raw HTTP
personal record must have a `user_id` UUID equal to the selected user
before that field is removed. The canonical envelope does not contain
it. A missing id refuses the protected page. It is not `left_out`, and
it is not archived.

`entry_source` `unknown`, null, and omission are `unknown_workout`
reports. They are not sessions and not manual reports. The same server
id with a different kind refuses the page.

A refused protected page is not applied, so it writes nothing. For a
missing device id on a device-targeted series row, and for unequal
null-source payloads of one full match key, a later archive preview and
apply stores kind `original` with no amount and no device receipt. D237
is when that archive may write. D236 is the capsule and the command counts. `written` stays false when only the archive
changed. The command
exits nonzero. Credential failures, invalid ids, conflicting device ids,
legacy history, and kind changes are not archived. Those originals are
not kept, because storing them would invent an id or keep a payload the
guard rejected. Preview does not see a cross-page null-source conflict.
A raw midnight row and a day window that starts then are different keys.

Rejected alternatives:

- Rebuild `records` with `DROP TABLE` inside the foreign-key write
  transaction. Rejected. The child links cascade or the drop fails, and
  the history would not survive.
- Leave foreign keys off after migrate. Rejected. Ordinary writes keep
  the check.
- Decode a null summary as zero or as its previous number. Rejected. The
  current row says the number is absent.
- Use 0 to 100 for every recovery summary because the class prose says
  so. Rejected. Polar writes 1 to 6, and the query copies that value.
- Add a point quantity named `heart_rate_recovery_one_minute`. Rejected.
  `hr_recovery` is the existing quantity and the existing key.
- Treat unknown provenance as automatic when a device id is present.
  Rejected. The id is not a measurement label.
- Archive every refused page, or write the archive inside the refused
  page's apply. Rejected. The first keeps rejected secrets and rejected
  devices. The second stores the refused write.
- Drop the originals of a missing device id so the refusal stays one
  transaction. Rejected for the two reasons above. The archive is a
  second transaction with a failed protected-write status. The other
  refusal reasons stay unstored.

## D236. The migration rename, the command counts, and the archive decision stay consistent

D237 supersedes the sentences below that classify database facts before
either transaction, that leave the archive with only an archive-source
check, and that leave a failed init file in place. The migration graph,
the command counts, the capsule, and the report routing stay. Where this
proposal disagrees with D237, D237 is the proposal.

This corrects D235. The returned health data stays in this pull. The
specification is the contract.

Step 6 drops `sql_v1_current_records`, `sql_v1_active_records`, and the
two `record_revisions` triggers that read `records` before it drops
`records`. It recreates them from the `001` text after the new root
exists, in the same transaction. Foreign keys are turned off only before
that transaction, on the store's own connection, in
`src/store/migrate.rs`. No ordinary write can run while they are off,
because `write_transaction` refuses one. After every exit the connection
reads `foreign_keys` back as on, or the command fails and closes the
store. A failure before commit restores the old schema, the old rows,
and the old version. `writable_schema` is not used. Files 001 through
005 stay unchanged.

`written` counts protected creates and revisions across the command,
including vendor originals. Those are `source_report` records, so they
also add to `committed.source_report`. An
archive-only change leaves it false and sets `database_changed` true.
Archive counts are separate from protected counts. A later page does not
clear either. A failed archive apply keeps the refusal, writes nothing,
and leaves earlier commits. The pull text for a new
archive-only change is `Protected write refused; N originals archived;
database changed; pull incomplete.`

D237 reads database-dependent vetoes in the protected preview, at one
database revision. The archive goes ahead only at that same revision. A
veto there, including on a `left_out` row, forbids the archive. A deleted
source or a deleted record leaves its rows out (D182, D190) and also
forbids the archive. The privacy scan still comes
first. The archive is pull-only. A mapped file import does not gain it.
A deleted archive source is not recreated, and the archive writes no
device receipt.

Each archived series row is a capsule of `resource`, the response
`resolution`, and the unchanged row. The hash is of that capsule. A raw
capsule and a day capsule are different records. A stored capsule that
hashes equal but differs is an archive failure, not a merge.

A reported series uses only the report sensor, including when `source`
is null. A measured series with `source: null` uses only the unattributed
sensor. Age is a source report with no `field_basis`.

Rejected alternatives:

- Leave the views in place until commit. Rejected. SQLite checks them
  when the replacement table is renamed, and the missing `records` name
  fails that check.
- Count an archive as `written`. Rejected. The protected import failed.
  The database change is reported by `database_changed` and the archive
  counts.
- Archive whichever error the mapper returns first. Rejected. A later
  conflicting device id would then be stored.
- Give a mapped file import the same second transaction. Rejected. The
  file has no selected-server privacy context, and the shared mapper
  would gain a write the import did not ask for.
- Hash the series row and infer the resolution later. Rejected. The
  resolution is on the response, not on the row.

## D237. The archive goes ahead only at the revision where the safety facts were read

This corrects D236. The returned health data stays in this pull. The
specification is the contract. The first version of this decision read
the safety facts under one write lock and read them again under a second
one. Version 2.1.1 has no write-locked preview, so this version uses the
2.1.1 revision check instead.

Privacy, required ids, and supplied-field names are checked on the
returned page before either preview. A failure builds neither preview.

Receipts, bindings, history, source deletion, stored records, import
keys, tombstones, null-source payloads, and workout kinds are read in the
protected preview. `preview_route` in `src/import/provider.rs` reads
them in one read transaction, at one database revision. The read
includes a row the mapping would leave out. That read is the safety
snapshot. A veto in it refuses the protected page, and the archive is
not attempted. A deleted source or a deleted record leaves its rows out
(D182, D190), and it also stops the archive of that page. A read from
before that preview is not the snapshot.

The archive is a second preview and apply. Its preview reads the archive
source and the target capsules, including tombstones. It is built only
when the database revision still equals the snapshot's revision. Its
apply refuses a moved revision, as every 2.1.1 apply does. Every
athlete-data write in 2.1.1 moves that revision. A restore into an empty
database instead sets a new database ID, and the apply refuses another
database ID. So a binding, a deletion, or an owner correction made after
the snapshot sets `archive` to `not_attempted`, and the archive writes
nothing. A deleted source or a deleted record is not copied onto a new
archive identity. A deleted archive source is not recreated. A deleted
capsule is not replaced. If the page names a row the bounded read cannot
cover, the page is not archived. There is no retry.

A failed step 6 during init rolls back to the committed step 5 schema
before the existing init cleanup runs. Init fails. D231's cleanup is
unchanged. A migrate of an existing 2.1.1 database keeps that file and
schema 5 when step 6 rolls back. An older database keeps the steps that
committed before step 6, as D227 says.

Rejected alternatives:

- Read the safety facts again under a second write lock, as the first
  version of this decision did. Rejected. The revision check already says
  whether anything changed after the snapshot. A second read would repeat
  that check in new code.
- Use a read from before the protected preview as the archive decision.
  Rejected. Another command can bind the source before that preview, and
  the protected veto would then be stored.
- Recheck only the archive source before the archive. Rejected. The owner
  can delete the original source or record in between, and the payload
  would be stored under a new identity.
- Leave a failed init file at step 5. Rejected. The existing caller
  removes that file, and this build does not change that cleanup.

## D238. A workout the person typed is a workout session

This supersedes only the sentences in D227 and D234 that store a manual
workout as a source report or leave it out. D227 through D237 otherwise
stay as written. The specification is the contract.

`entry_source` `manual` is a `workout_session`. Its source type is
`manual`. `field_basis.value` is `entered_by_hand`. It is not a device
measurement. `device_id`, if the server sent one, stays on the original
object. It does not set `device_binding`, and it does not put the
workout on the watch's source. The server workout id is the match key,
so a later pull updates the same workout. Name, type, start, end,
distance, heart rate, elevation, zones, and segments are stored the same
way they are stored on an automatic workout. `activity_type` still
records that Open Wearables normalized the sport name. D241 corrects the
next rule. The numbers on this session do not enter the eight range
calculations or `provider-period-summary`. `workout-drift` may use the
session's start and end when the caller names that session. The samples
in that window still exclude a manual source. The session does not enter
a device baseline.

`unknown`, JSON null, and a missing `entry_source` stay source reports of
kind `unknown_workout`. They are not a workout session, and they are not
manual.

Rejected alternatives:

- Store a manual workout as a source report. Rejected. The workout object
  says a manual entry is self-reported and has no device measurement
  behind it. The record is a workout, and the source is the person.
- Call a manual workout a provider summary. Rejected. It is a session.
- Treat `unknown`, JSON null, or a missing `entry_source` as manual.
  Rejected. Those values are not the self-reported entry.

## D239. A body skin-temperature time is not an instant by itself

This corrects the sentence that treats a non-null
`skin_temperature_measured_at` as a measurement instant. The other body
rules stay.

The body service reads a skin-temperature sample in the latest window.
When that sample is missing, it reads the newest day value and uses that
day's `period_start` as the same timestamp. The response has no field
that says which read it used. The amount, the unit, and the timestamp
stay on the original object. The skin-temperature child is time-unknown.
It has no `recorded_at`. The program does not choose by comparing
timestamps, and it does not invent bounds. Body temperature and blood
pressure still use their own times. A raw skin-temperature sample and a
day skin-temperature window keep the series rules they already have.

Rejected alternatives:

- Treat every non-null body timestamp as an instant. Rejected. The day
  fallback writes `period_start` into the same field.
- Drop the timestamp. Rejected. The original object keeps what the server
  sent.

## D240. A finite heart-rate recovery stays, including zero

This corrects the positive point gate for one quantity. The quantity,
the unit, and the import key stay `hr_recovery` and `bpm`.

Open Wearables stores `heart_rate_recovery_one_minute` as a number in
bpm. The sample value has no positive minimum. The mapping and the v1
point check currently reject a point at or below zero, except
`temperature_deviation`. A finite `hr_recovery`, including zero and a
negative number, passes that check. A non-finite value does not. Other
points stay positive. `temperature_deviation` stays signed.
The exception is the point check in `src/model/data.rs`.
`packages/npm/src/schema.ts` is frozen compatibility code and is not where
this check is added. No table is rebuilt for this check.

Rejected alternatives:

- Keep the positive gate and drop a recovery of zero. Rejected. The
  server can return that number, and this pull said a returned zero stays
  zero for this quantity.
- Allow zero for every point. Rejected. The other positive checks stay.

## D241. A manual session can set a window, and its numbers stay out

This corrects the D238 sentence that says the calculations do not read a
manual workout. It does not change the calculation code.

`read_inputs` and `read_period` exclude `source_type` `manual`. They have
no `include_manual` switch. `workout-drift` loads the session the caller
names, including a manual session, and uses its start and end as the
window. The samples then come from `read_inputs`, so a manual source's
numbers are not inputs. The session's own heart rate and distance are not
inputs. `query_measurements`, `query_series`, and `query_sessions` in
`src/store/query.rs` omit manual sources unless `include_manual` is true.
The JavaScript names `queryMeasurements`, `querySeries`, and
`querySessions` in `packages/npm/src/database.ts` call those functions.
A generic record listing
is not those three functions. A typed workout must not enter a device
baseline.

Rejected alternatives:

- Refuse a manual session as the drift window. Rejected. The current
  command already accepts that window and then reads the other sources'
  samples. This proposal does not take that away.
- Let a manual heart rate enter a baseline when `include_manual` is set.
  Rejected. The calculation SQL has no such switch.

## D242. A provider-live workout is kept as an original

D243 is the live-read procedure. D244 is the privacy choice. Where this
proposal disagrees with either, those decisions are the proposal.

The stored workout list is the normalized session. The provider route
handlers return the vendor object and do not insert a workout row.
Defaults leave workout samples, FIT bytes, and raw payloads unstored
unless that deployment turned the flag on. This pull can store the vendor
object when the person passes `--vendor-workouts`.

The original is a `source_report` of kind `original`, subject
`vendor_workout`, writer `open_wearables`, sensor
`ow1:["vendor_workout", provider]`. The match key is the provider plus
the vendor workout id. It is not a second workout session, and it is not
joined to the stored session by time. The public stored workout has no
`external_id`. Device names, device ids, routes, and nested samples stay
inside the original. They do not set `device_binding`, and they are not
measurement samples. Calculations do not read the original.

Each original is one envelope. It holds `provider`, `vendor_id` as text,
`start` and `start_kind` when the object has a start, `list`, `detail`
when a detail is stored, and `detail_status`. Sensorbio adds
`list_parent`. The specification's section 2 shows the JSON. The whole
envelope is kept in `supplied_fields`, and `provider_record_id` is
`vendor_id`. The match key is the writer, the sensor, the resource
`vendor_workout`, the provider, and the vendor id, escaped by D181.

The rewritten `mappings/open-wearables.md` gains a section named
`vendor_workout`. It declares the record type, the kind, the subject, the
writer, the sensor, the key, and the start. A saved file of envelopes
imports through that section. File import writes each envelope as the
file holds it, as D182 does for every mapped import. The detail rules in
D243 run before the mapping engine sees an envelope, so the mapping
engine has no vendor exception.

The pull writes one provider's originals as one page, after that
provider's list and detail calls finish. That page is one preview and,
under `--yes`, one apply, as D230 says for every page. A vendor page
never holds a stored row or an archive capsule. A failed or refused
vendor page leaves that provider `incomplete`. It does not change the
stored pages or the archive.

The vendor page uses the 2.1.1 keyed rules. An original the person
deleted is left out on every later pull and is not recreated (D182). A
deleted `vendor_workout` source leaves out every object from that
provider (D190).

An owner correction is kept until the server's data changes. On route
`open_wearables` only, a pull or a mapped import compares a candidate for
an owner-corrected record with the newest revision an import wrote, basis
`provider_import`. It does not compare with the owner's revision. Equal
writes nothing, and the correction stays current. Different replaces the
correction, and the count `owner_corrections_replaced` shows it on that
provider's line, in `--json`, and in the preview, which says `would
replace N owner corrections; delete an original to keep it`. The
correction stays in the history. The rule covers every record on that
route, so a workout fix on the stored session also survives a pull of
unchanged data. For that route it supersedes the D182 sentence that a
mapped import replaces a changed object. Other routes keep D182. The
stored-original read in D243 uses the same import revision, so an owner's
edit does not trigger a detail call. The skill says that a workout fix
belongs on the session or in a note, not on the vendor original.
Deleting an original keeps the person's version in its history and stops
every later pull from changing it.

Each provider's line counts originals as new, changed, and unchanged. It
counts owner corrections replaced, details by outcome, objects left out
by reason, and withheld values.

Vendor originals have a total size bound, `MAX_VENDOR_ORIGINAL_BYTES`,
128 MiB, counted over every revision of every original. A native package
writes them in its main document, `athlete.ath.json`, with every record
that is not a measurement sample: the current record under `records` and
each revision under `record_revisions`. Only samples go to `series/`
files. Version 2.1.1 reads that document only up to 512 MiB, and a mapped
import file only up to 512 MiB. The bound keeps the originals near half of
the main document. A page that would pass it is refused, its objects
count as `size_cap`, and that provider is `incomplete`. A mapped import
uses the same bound. `ath export` gains a check that refuses to publish a
document over the 512 MiB read cap, because 2.1.1 can write a package it
cannot read back. Each listed object is in exactly one detail count or one left-out
count, so no object is dropped without a count.

The pull uses the published query parameters and does not add its own
retry. It does not call sync or historical sync. A completed provider
line means that provider's stated limit was reached. It does not mean the
vendor returned lifetime history. The same API key is used. No new vendor
permission is added.

Rejected alternatives:

- Leave the live routes unread. Rejected. The vendor object is available
  from the server and is not the stored workout.
- Make the live object a second workout session. Rejected. The stored
  session already exists, and the public workout has no vendor id to join
  them.
- Call sync or historical sync to fill the gap. Rejected. Those routes
  are writes this pull does not call.
- Store the list object and the detail as two records. Rejected. They
  describe one workout, and nothing in either object joins them except
  the id the envelope already uses.
- Store vendor originals with no size bound. Rejected. They share the
  package's main document, and an export past its read cap could not be
  restored.
- Store only the list object once the bound is near. Rejected. The pull
  would drop details while it reported success.
- Store originals in their own package files now. Not chosen for this
  release. It changes the package format. Section 9 of the specification
  lists it.
- Replace an owner correction whenever the candidate differs from the
  current record, as D182 does and as the first version of this decision
  did. Rejected. A pull repeats the same server data, so every pull would
  undo every correction even when the server had not changed.
- Never replace an owner correction. Rejected. A later fix by the vendor
  would be lost, and nothing would say so.
- Limit the rule to vendor originals. Rejected. The skill sends workout
  fixes to the stored session, and the D182 rule would undo those on every
  pull. The maintainer can narrow it before approval.
- Use the rule on every mapped route. Not chosen for this release. Other
  routes import a file the person chose, and D182 stays their rule.

## D243. A live vendor read is opt-in, and one failure stays on that provider

This supersedes the D242 sentences that said a live GET does not write on
the server, that a failed provider stops the later providers, that Polar's
list asks for samples, zones, and route, that a start outside the range
is dropped, and that the server splits a Garmin range.

On 15 November 2026 this decision was revised for the review at
`e145fa6` and for 2.1.1. The specification's section 2 is the full
procedure.

`--vendor-workouts` is off unless the person passes it. Off, preview and
`--json` call neither the connections route nor a live workout route.
On, a preview reads the connections list and each active provider's list,
and it makes no detail call. Its detail counts are estimates. The preview
writes nothing in this database and prints `vendor requests were made`.
"Nothing stored" and "nothing written" always mean this database. A
preview and then `--yes` read each list twice, and the preview and the
help say so. Running `--yes` alone reads each list once.

The server, while answering the GET, can save a refreshed vendor token,
revoke a connection, emit `connection.revoked`, write `token_refresh_lock`
and `provider_auth_failing_since`, and on HTTP 429 sleep 15, then 30,
then 60 seconds, up to three retries. This program does not add a retry
of its own. Every run with the flag says, before its first live request,
that the server may refresh and save vendor tokens, may revoke a
connection, and may wait about 105 seconds on a rate limit. That holds
for a preview, for `--yes`, and for `--json`, which puts the three
sentences in `vendor_workouts.notice`. The flag's help text says the
same.

The pull reads `GET /users/{id}/connections` first. Each item's `user_id`
must be the selected user, or the list is refused. The pull keeps only
`provider` and `status` from each item. It drops the rest, including
`linked_user_ids`, which holds other users' ids. The list is never
stored, so the privacy scan does not run on it. A provider is called only
when its status is `active` and it has a live list method. `revoked`,
`expired`, a missing item, and an unknown status are `not_attempted`, and
the line shows the status. Those providers are never called, because a
call could make the server try a refresh or a revoke. The order is
Garmin, Polar, Suunto, Whoop, Fitbit, Oura, Strava, Withings, Sensorbio.
One failure leaves that line `incomplete`. The others still run.

Garmin is split by this program into windows of 82,800 seconds, 23
hours, or less, one call each. Each window sends integer Unix seconds.
The pinned server splits a window it sees as longer than 24 hours and
drops a failed piece without saying so. It measures in naive local time.
When the server's clock moves forward an hour, 24 real hours read as 25
local hours, and the server would split the window. When the clock moves
back, they read as 23. A 23-hour window reads as 24 at most where the
clock moves by one hour. A zone that moves it by two hours, such as
`Antarctica/Troll`, is not covered. Any failed window makes Garmin
`incomplete`. The pull keeps calling Garmin detail at the pinned path,
`/wellness-api/rest/activities/{activity_id}`. That path is not in
Garmin's public docs, and this study did not find it there. For a Garmin
detail only, HTTP 404 or 501 counts as `not_available`: the list object
is stored whole, and Garmin stays `completed`. Any other Garmin detail
error is `failed`. An id that was `not_available` is detailed again after
the new and changed ids, inside the same budget. The maintainer confirms
this rule before approval. Polar's list omits samples, zones, and route, and detail
asks for all three. Suunto's detail is called at `workoutKey`. A list
object with no `workoutKey` gets no detail call and is `not_available`,
because nothing this study read shows that Suunto's detail accepts a
`workoutId`. Its `payload` is unwrapped, and the record key is
`workoutId`. One command sends at most 100 Suunto list requests,
`MAX_VENDOR_LIST_PAGES`. A cap that stops a list after a full page makes
Suunto `incomplete`, and the objects already read are still written. Whoop's response names its
next page `next_token`. The request name, `nextToken`, is not sent.
Sensorbio reads only the oldest 50 entries, and its line says so.

A stored detail is never lost. A detail is fetched for an id with no
stored detail, and again only when the list object changed. Credential
values (D244) are withheld before any comparison, so a token that changed
between pulls does not make a list object look changed. A list-only
object never replaces a stored detail. A new detail that is thinner than
the stored one writes nothing. Thinner means a detail field the
specification's table names for that provider is filled in the stored
detail and empty in the new one. Garmin, Suunto, Whoop, and Oura name no
detail field, so a new detail from them replaces the stored one even when
it has fewer fields. The stored detail stays in the record's history as
the earlier revision. A deferred or failed detail keeps the
stored detail and adds no revision. A detail over the 10 MiB response cap
is `too_large`, because the detail route has no smaller page. A detail
the privacy scan refuses is `refused`. Neither is stored. A failed,
mismatched, too large, or refused detail makes the provider `incomplete`
and the exit nonzero, because a limit may refuse work but must not report
it as complete. A too large or refused id is retried after the new and
changed ids. A deferred detail leaves the provider `completed`, with a
separate deferred count. A detail is read after the provider's whole
list, so it can come minutes after its list object, and the vendor can
change the workout in between. The envelope does not record when each
part was read.

Live requests go one at a time, at least one second apart. The read
timeout is 360 seconds. The server's own retries can take about 225
seconds, so the client waits for them. A live request that still times
out ends that provider for the command, so two requests to one provider
never overlap. An HTTP 429 ends that provider the same way, because the
server has already spent its three retries on the vendor's limit. The
detail call that got 429 is `failed`. Ids whose detail was not yet called
are `deferred`. What the provider already returned is still written, and
the provider is `incomplete`. A command makes at most 100 detail calls to one provider
and at most 20 to Strava. Ids with no stored detail go first, oldest
start first. Strava's limit belongs to the whole application, so the
deployment's own sync shares it. The review cited about 100 requests a
minute for Garmin. This study did not find that number in Garmin's public
docs. The one-second pace keeps this command at 60 a minute or fewer.

A vendor id is text. A JSON integer is written in decimal, so `123` and
`"123"` are one id. An integer above 9007199254740991 is refused, because
D213 would change its digits. When one Garmin activity id is in two
windows with different objects, the later window's copy is kept, because
windows select by upload time and the later window holds the later
upload. The earlier copy counts as `superseded`. Inside one window, two
different objects with one id are both refused. The start becomes `input_timestamp` by the
rules in the specification: text keeps its offset or says it has none,
and Unix seconds or milliseconds keep their digits. The start is stored
and is not a filter.

Every returned original is stored, including a start outside `--from` and
`--to`. The count is `vendor workout originals`. `range_complete` uses
only the stored ranged reads. Each line says whether its limit covers the
requested range and whether more data exists. An object the pull does not
write is counted by reason.

The unwrap, the id, the start field, the detail key, the detail unwrap,
the detail fields, the budget, and the read timeout for each provider are
the table in the spec. File import uses the same envelope. The provider
logic lives in `src/runtime/vendor_workouts.rs`, driven by one table.
`src/runtime/open_wearables.rs` calls that module.

Rejected alternatives:

- Call every provider the server lists. Rejected. A call to a revoked or
  expired connection can make the server try a refresh or a revoke.
- Store the connections list. Rejected. It holds other users' ids, and it
  is not health data.
- Write the list object over a stored detail when the new detail fails or
  is deferred. Rejected. The person would lose data they already have.
- Count a Garmin detail 404 as `failed`, as the earlier text did.
  Rejected. If the path does not exist, Garmin would be `incomplete` on
  every command, and the person could not fix it.
- Drop the Garmin detail call because the path is not in Garmin's public
  docs. Rejected. The pinned server calls it, and a deployment where it
  works would lose every Garmin detail.
- Use a 30-second read timeout. Rejected. The server's own retries take
  longer. The client would give up, and a rerun could send a second
  request while the first still runs.
- Retry a live request that timed out. Rejected for the same reason.
- Keep calling a provider after a 429. Rejected. The server has already
  waited about 105 seconds and retried three times, and more calls would
  spend a limit the deployment's own sync shares.

- Leave live reads on by default. Rejected. The server can change a token
  and revoke a connection while answering the GET.
- Stop the later providers after one failure. Rejected. A person connected
  only to Garmin must still finish when another provider is absent, and a
  failed provider must not hide a finished one.
- Let the server split a Garmin range. Rejected. A failed window must be
  visible as incomplete.
- Call Suunto detail at `workoutId` when the list object has no
  `workoutKey`, as the earlier text did. Rejected. No source this study
  read shows that call works, and a wrong call would count as `failed` on
  every command.
- Make detail calls in a preview. Rejected. A preview and then `--yes`
  would make every detail call twice, and Strava's limit is shared.
- Count a too large detail as `not_available`. Rejected. The vendor has
  the detail, and this program's own cap refused it, so the provider did
  not finish.
- Refuse both copies when one Garmin id is in two windows with different
  objects, as inside one response. Rejected. A re-upload is an ordinary
  Garmin event, and the later window holds the later copy.
- Drop an original whose start is outside the requested range. Rejected.
  The live methods do not filter by workout start, so a drop would hide a
  returned object.

## D244. Vendor originals stay whole. The maintainer can change this

This is a labeled choice. The default in this proposal is to keep each
vendor object whole, including account ids, Polar user URLs, and full GPS
routes. The reason is a complete, portable copy of what the vendor
returned. The output and the spec say those fields are included. Only a
credential-shaped value is withheld, as the next paragraphs say.

The scan still refuses the API key, the selected origin, and the selected
user id, wherever they appear. That includes a value under a
credential-shaped key.

A credential-shaped key does not refuse the page. Its value is withheld.
The value is replaced by the text `[withheld by ath]`, and the pull
records the JSON path and counts it. A key is credential-shaped when its
letters and digits, with case ignored and every other character dropped,
end in `token` or `secret`, or are `apikey`, `authorization`, or
`password`. The page cursors `next_token` and `nextToken`, matched
exactly, are not credential-shaped. Whoop and Oura name their next page
`next_token`, and withholding it would turn a null into text. A query
parameter or a fragment pair with a credential-shaped name inside a URL
string is withheld the same way, and so is a user name or password before
the `@`. The URL keeps its scheme, host, port, and path. `polar_user` and a
coordinate are not credential-shaped. The rule covers stored reads and
vendor reads alike.

Garmin's summary objects can carry an optional `userAccessToken`. The
pinned server declares it on its Garmin wellness models, and the live
route returns Garmin's objects unchanged. Refusing the page would refuse
every Garmin page that carries it, and the workouts on it would be lost. Withholding keeps the workout and keeps the vendor's token
out of the record. The stored original then differs from the vendor's
object only at the withheld paths, and the count says how many there
are. The maintainer can change the key list or the marker before
approval.

Rejected alternatives:

- Strip account ids, user URLs, and routes before storage. Not chosen.
  The stored object would no longer be the object the vendor returned.
  The maintainer can replace this decision before approval.
- Refuse the whole page when a credential-shaped key appears, as the
  first version of this decision did. Rejected. One optional Garmin field
  would block every Garmin page, and the person would lose those workouts.
- Drop a credential-shaped field without a marker. Rejected. The stored
  original would not show that a value was removed.
