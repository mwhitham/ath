# Athletic Standard 2.2.0

Status: **plan only. The maintainer has not approved it. Not approved for build.**

`ath pull openwearables` reads one person's sleep, workouts, and raw samples
from an Open Wearables server they run, and writes them with the mapping file
that version 2.1.0 already ships. The decisions are D198 through D202 in
[decisions.md](decisions.md). The evidence is
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

The first run writes nothing. It prints:

- one line that a date-only bound is sent as typed, and Open Wearables decides
  what that day means
- counts, by record type, of sleep sessions, workouts, points, summaries, and
  scores that would be new, changed, or unchanged
- two fixed lines: this pull asks for raw samples only, so a nightly
  heart-rate variability, resting heart rate, blood oxygen, or VO2 max that
  the server keeps as a day value is not in this pull; and the heart-rate
  stream is not stored, while a workout's average and maximum heart rate are
  stored when the server sent them
- how many rows were left out, then up to 20 left-out lines
- `nothing written; pass --yes after reviewing this preview`

`--yes` writes, then prints those same two limit lines. It prints
`pulled openwearables from <from> through <to>` only when every read ended
because `has_more` was false. `--json` prints the structured result and does
not write, including when `--yes` is also present. That is the same rule as
`ath import --mapping`. `ath pull garmin --json` still writes, which is the
Garmin command as it is today. `--file` chooses a database, as on the other
commands.

A failed page names the read (`sleep`, `workouts`, or `timeseries`). The
command exits with a failure and does not print the success line. When
`--yes` had already written a page, the message says those pages are kept.
It says to run the same command again. An empty range prints zero counts and
is a success, and it still prints the two limit lines.

`--date` stays required only for `garmin`. `--from` and `--to` are required
only for `openwearables`, and they are flags, not environment variables.
`--base-url` and `--user` are required for `openwearables` unless the
environment named above supplies them. `ath pull garmin --date YYYY-MM-DD` is
otherwise unchanged. `ath pull` with any other name still fails. The failure
text becomes `pull reads garmin or openwearables`.

The saved key can read every user on that server. `ath key` shows whether it
is saved, without printing it. `ath key` with no further word does not remove
it. `ath key clear` with no name leaves it. `ath key clear openwearables`
removes it.

## 2. What this release leaves for later

D198 names the boundary. File imports stay. The Garmin pull stays. Junction is
2.3.0. Automatic sync, retries, token refresh, and backfill are 2.4.0.
Storing day-level heart-rate variability, resting heart rate, blood oxygen,
and VO2 max is 2.5.0, and so is the heart-rate stream. That version is not a
decision, made here, to add an Open Wearables period type. A shared connector
interface is later than that.

This release also leaves out:

- creating users, OAuth, webhooks, SDK uploads, and Apple Health XML upload
  through Open Wearables
- `DELETE` on the server, and any local delete inferred from a missing row
- health scores, meals, menstrual cycles, body summaries, and the activity,
  sleep, and recovery summary routes
- timeseries resolutions other than `raw`
- the heart-rate stream, steps, and the other series the mapping file does not
  name
- storing the server address, the user id, the API key, or a resume cursor

On Open Wearables 0.11.0, day and session values are no longer raw samples.
A raw pull can return an empty timeseries for resting heart rate, nightly
heart-rate variability, blood oxygen, and VO2 max even when the server holds
them as day rows. Sleep sessions and workouts still return. The release does
not invent a midnight timestamp for those day rows (D88, D110, D199).

## 3. How a response becomes records

The pull resolves `open-wearables` through `resolveMappingFile`. It parses and
checks the file before the first request. A bad file fails with no network
call.

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
`source.device`, and `source.source`. Two devices that share those three
strings share one source and one baseline. `source.device_id` is stored as
hardware on the source-device row. It does not split the source. That is the
mapping file version 2.1.0 already ships (D183). The queries in D201 ask for
every device.

`zone_offset` is not copied into `start_at`. It remains in `supplied_fields`
when the server sends it. The timestamp text is the server's `start_time`,
`end_time`, or `timestamp`. A timestamp with no offset is left out by the
mapping. The pull does not add `Z`.

`computed_by`, `window_seconds`, `is_nap`, `intensity`, and `efficiency_percent`
stay under those names when they are present. The basis on a mapped value
stays the basis the file already states. This release does not read
`computed_by` as a new basis. Raw samples on 0.11.0 are built without that
field. Aggregates, which do carry it, are not requested.

## 4. Credentials, privacy, and the network

D200 is the rule. The key never enters the athlete database, the export, the
preview, or an error line. The pull reads only the three paths in D201, plus
no others. It does not list users and does not read the personal record.

The process refuses a base URL that could carry the key in the URL itself.
It refuses plain `http` except to loopback. It does not follow a redirect, so
the key cannot be sent to a second host. One attempt, then stop. Timeouts are
10 seconds to connect and 30 seconds to read. One response may be at most
8 MiB. One read may be at most 1000 pages. Before a page is written, the
response text is checked for the key. A body that contains the key is not
written. Printed failure text is the status and, when the body is problem
JSON, the `code`. It omits the request path, which contains the user id.

There is no Athletic Standard telemetry on this command. The server the person
named is the only host contacted.

A deployment API key can read every user on that server. The command cannot
shrink that key. It can avoid using the extra power: one user id, three
reads, no user list.

## 5. Version, migration, and what stays compatible

The build sets `2.2.0` on the package, the record, and `ath-core`, by the
same constants 2.1.0 moved. Migration step 5 is a new SQL file. It updates
`athleticstandard_version` to `2.2.0` and `sql_schema_version` to 5. No column
is added. Step 4 and file `004` are not edited (D165).

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
`schema/2.0.0/`. The JSON schema gains no field. The folder exists so the
package version and the schema path stay aligned. The demo athlete is
regenerated because the stored version is inside it. `examples/demo-athlete/athlete.ath.json`
stays the legacy 0.4.3 file. `tests/fixtures/version-2-0-0/` is not
regenerated.

`package.json` `files` already includes `mappings`. The mapping files are not
rewritten by this release. `pnpm generate:skill-cli` runs because help text
changes (D80). `AGENTS.md`, `build-history/README.md`, and `SPEC.md` are
updated in the build so they name 2.2.0 as the software. `docs/connections.md`
keeps the file imports and states the same limit as section 1: a raw pull
does not store day-level resting heart rate, nightly heart-rate variability,
blood oxygen, VO2 max, or the heart-rate stream. This plan's edits to
`AGENTS.md` and `build-history/README.md` only point at the plan.

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

`store.rs`, `persist.rs`, and `personal.rs` are not the home for this client.
`personal.rs` remains the Garmin pull. The two clients may each call `ureq`.
A shared trait is the later connector work, and this release does not add it.

`mapping.rs` changes in one place. `apply_mapping` keeps its `outcome` field
and adds counts by record type. The pull sums only `sleep_session`,
`workout_session`, `point_measurement`, `provider_summary`, and
`provider_score`. It does not add `outcome` across pages. `outcome` counts
the source-device row in the same group, and a preview rolls each page back,
so that total would not match the write. The three printed record families,
plus summaries and scores, do match, because each row's match key is looked
up on its own.

The Rust command reads the key from the environment only. It has no `--key`
flag. The child checks the response body for the key before `apply_mapping`.
The parent checks that stdout and stderr do not contain the key before
printing, and the child does the same before it returns JSON.

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

Rust tests, run by the existing `cargo test` job on Linux, macOS, and Windows:

- two pages, the second `next_cursor`, then `has_more: false`, and the
  success line is printed
- a repeated cursor fails that read, writes nothing for that page, and does
  not print the success line
- `has_more: true` with a null cursor fails the page and writes nothing for it
- a 401, a 429, a timeout, and a non-JSON body fail the page
- a redirect is not followed, and the key is not sent to the second host
- `http` to a non-loopback host is refused before a socket opens
- a base URL with a user name, a query, or a path is refused
- the request header is `X-Open-Wearables-API-Key`, and the key is not in the
  result or the error
- `filter_by_priority=false` and `resolution=raw` are present, and each
  mapping type is one repeated `types` query parameter, with duplicates
  removed (`skin_temperature` is named twice in the file and sent once)
- the date strings are the ones the caller passed
- `page-sleep.json`, served as the sleep response, becomes a sleep session
- a workout page with one `entry_source` of `manual` and one automatic
  neighbor leaves the manual row out and keeps the neighbor
- a user id that is not a UUID is refused before a socket opens
- `page-raw.json` stores the mapped sample, and `page-5min.json` is left out
- RMSSD and SDNN stay different quantities
- a second run reports unchanged
- a changed end time keeps the record id and increases the revision
- a `__proto__` supplied field fails the page and leaves that page unwritten
- a deleted mapped source is left out
- a Garmin file import of another workout remains a second source
- the 1000-page cap and the 8 MiB cap fail that read, write nothing for the
  failed page, and do not print the success line; the page cap is tested with
  a small limit passed into the loop, not by looping 1000 times
- a timestamp with no offset is left out; the stored fixture's timestamps
  include `Z`, and the test does not add `Z` to a bare timestamp

TypeScript tests cover the missing-key sentence, `ath key set` and
`ath key clear` for `openwearables` against the memory store, the preview
sentence, and `--json` not writing. They do not open the real password store.

`pnpm check:public-data`, `pnpm typecheck`, `pnpm test`, and `pnpm build`
pass, and `cargo test` passes, before the build is called done.

## 8. Laptop card and the proof

The card uses the fixture server or Open Wearables' own synthetic seed. It
does not use a real account or a real export.

1. Create an empty database with `ath init --yes --empty` in a temporary folder.
2. Save the fixture key with `ath key set openwearables`.
3. Run the pull for `2026-01-01` through `2026-01-07` without `--yes`.
4. Read the preview. Confirm the two limit lines are on screen, a left-out
   line is visible, and the key is not.
5. Run the same command with `--yes`. `ath check` exits 0.
6. Run it with `--yes` again. The counts say unchanged.
7. Run `ath pull garmin` with no date and confirm it still asks for `--date`.
8. Import one existing synthetic Apple, WHOOP, or Oura fixture and confirm
   `ath check` still exits 0.

The build pull request's description carries screenshots or a short recording
of steps 3 through 6. The images are not committed. The numbers on screen are
the fixture's invented values. The release workflow on Linux, macOS, and
Windows is the cross-platform proof. The recording is one system.

This plan pull request has no recording.

## 9. Separate follow-ups

D202 records the check. None of these is a prerequisite for the pull, and none
is in the build pull request.

- Failed `ath init` and failed `createDatabase` still delete the database file
  in the same process that closed it, without the garbage collection
  `removeTemporaryDirectory` uses. On Windows that delete can fail with
  `EBUSY`. Version 2.1.0 spec section 12 left it out and named no later
  version.
- `store.rs` and `mapping.rs` are past a thousand lines. The pull does not
  add HTTP to them. The only `mapping.rs` change is the count by record type
  in section 6. Splitting the files stays out.
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
- A raw range of mapped series can be large. The 1000-page cap and the 8 MiB
  cap fail that read, write nothing for the failed page, and withhold the
  success line. The person narrows `--from` and `--to` and runs again. An
  unchecked server limit, including a 429, can stop a later page after earlier
  pages have been kept. The study did not read the rate-limit middleware.
- Date-only bounds mean whatever Open Wearables decides. The docs and the
  0.11.0 parser disagree about UTC. This release does not correct that server.
- Day-level recovery values are absent after a successful pull, because they
  are not requested. The two printed lines say so. Workout average and maximum
  heart rate can still be stored.
- Fields on a sleep or workout object that the mapping does not consume are
  stored under the server's names. The shipped mapping does not request
  latitude or longitude. A replaced mapping file can name more series, and
  those series are then requested when they map onto an existing field.
- The administrator key is more powerful than this command. A leaked key from
  the password store exposes every user on that deployment. The command's job
  is to avoid copying the key into the record.

The maintainer approves or rejects this plan before any build. The proposal
to approve is D198 through D202, in particular:

1. One number, 2.2.0. Readable sets keep `1.0.0`. Step 5 runs on create and
   on migrate. No new column. A `2.1.0` database does not open until
   `ath migrate`. This release does not schedule, retry, refresh a token,
   backfill, or store a cursor (D198).
2. The mapping file writes the records. A 0.11.0 raw pull stores workout
   average and maximum heart rate when the server sent them, and it can store
   zero points for the day values. The heart-rate stream is not requested.
   `ProviderReportingPeriodType` is unchanged. Storing the day values is
   2.5.0, and that is not a decision to add a period type (D199).
3. The key, the server, and the user id stay out of the record. Failure text
   omits the request path. `ath key clear` leaves this key.
   `ath key clear openwearables` removes it (D200).
4. Preview is the default. A failed later page keeps earlier pages, the
   command fails, and it does not print the success line. A missing row is
   not deleted. Date-only bounds are sent unchanged, and the server decides
   the day. The 1000-page cap and the 8 MiB cap fail that read (D201).
5. The Windows cleanup and the maintainability notes stay separate. The count
   by record type in `mapping.rs` is the only change in that file (D202).
