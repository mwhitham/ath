# Athletic Standard

Athletic Standard is an open, local-first record for training and recovery data.
The live record is one SQLite database:

```text
athlete.ath.sqlite
```

The `ath` command reads and writes that database. Local use needs no account,
network connection, subscription, or hosted service.

Athletic Standard keeps measured records separate from athlete reports. A measured
record must name its source. An athlete report cannot name one. Sources also remain
separate when two devices measure the same quantity.

Not an app, not a coach, not medical advice. The record and its tool skill describe
what the data shows. Training advice comes only from an optional coaching skill that
the athlete installs, and it is saved as an authored record.

## Agent skills

The `ath` executable carries three kinds of [Agent Skills](https://agentskills.io):

| Kind | Skills | What it does |
|---|---|---|
| Tool | `athletic-standard` | Finds, reads, logs, imports, and exports records. |
| Calculation | 24 skills, such as `hrv-change`, `sleep-regularity`, `training-load-effort`, and `hyrox-race-splits` | Runs one reproducible calculation and states its method and limits. |
| Coaching | `crossfit`, `hyrox` | Assesses, plans, adapts, and reviews training for one sport. |

`ath init` installs the tool and calculation skills into `.agents/skills/`.
Coaching skills are installed only when named: `ath skill install hyrox`.
[`docs/skills.md`](docs/skills.md) lists every skill and what was verified.

Every skill runs `ath`. A skill installed another way, such as with the skills CLI,
needs `ath` installed too (see [Install](#install)). Each skill tells the agent how to
check for `ath`, and to ask before installing it.

## Security

The database contains sensitive health and training data. Athletic Standard creates
local files with owner-only permissions where the operating system supports them,
but it does not encrypt the database itself. Use operating-system disk encryption.
Anyone who can read the database file can read the athlete record.

## Install

`ath` is one native executable. It runs on Linux x64, macOS arm64, and Windows x64.
Local use needs no Node.js, compiler, `sqlite3` program, service, or account.

With Node.js 22 or newer, install it from npm:

```sh
npm install -g athleticstandard
```

The package holds the prebuilt executable for each supported system. Installing it
downloads and compiles nothing. `npx athleticstandard` can be used instead of a
global install.

Without Node.js, download the archive for your system from the
[releases page](https://github.com/mwhitham/ath/releases), and put the `ath`
executable on your `PATH`. The executable carries the agent skills and mapping files.

Run `ath --update` to update. An npm, pnpm, or bun global install updates through the
same package manager. An archive install prints the steps to replace the executable.
Versions before 0.4.3 have no `--update`; run `npm install -g athleticstandard@latest`.
After updating from 0.x or 1.x, run `ath skill install` in each folder with agent
skills ([upgrading to 2.1.2](docs/upgrading-to-2-1-2.md)).
Run `ath skill install` to refresh the local agent skills without updating `ath`. It
updates a calculation skill you never edited and keeps one you edited.

To build from source, install the Rust toolchain named in `rust-toolchain.toml` and
run `cargo build --release`. The executable is `target/release/ath`.

TypeScript and JavaScript callers can import the public parts directly:

```ts
import { AthleticStandardPackage } from "athleticstandard/schema";
import { openDatabase, queryMeasurements } from "athleticstandard/database";
```

The JSON schemas are exported as `athleticstandard/json-schema`,
`athleticstandard/json-schema/export-manifest`, and
`athleticstandard/json-schema/measurement-series`. The Agent Skill is exported as
`athleticstandard/skill`. The retired prerelease JSON runtime is not part of the v1
package API.

A program that works with a record on an `ath-host` server uses `connectHost` from
`athleticstandard`. It takes the host's address and the signed-in person's access
token. The host checks that person's grants on every request. The program never
receives a database credential, and `openDatabase` still refuses database URLs.
[`examples/host-starter/`](examples/host-starter/) runs a coach's steps with it.

## Upgrade a database

A database from version 1.0.0, 2.0.0, 2.1.0, 2.1.1, or 2.1.2 upgrades with `ath migrate`. The
command shows a preview first. It makes a verified backup with SQLite's own backup
interface before the first schema write. Opening a database never migrates it without
confirmation. To go back, restore the backup; it does not contain work recorded after
it was made. [`docs/upgrading-to-2-1-2.md`](docs/upgrading-to-2-1-2.md) lists what
2.1.2 changes, including the repaired calculation methods.
[`docs/upgrading-to-2-1-1.md`](docs/upgrading-to-2-1-1.md) covers the 2.1.1 rebuild.

When typed-table columns disagree with the current revision in meaning (for example
after direct SQL edits), run `ath repair -y --from-revision
--confirm-revision-align <database-id>` after reviewing the preview. Each attempt
writes a new `.repair-backup.*.sqlite` file beside the database; failed attempts
leave that file in place for restore.

## Create a database

```sh
mkdir training-record
cd training-record
ath init
```

`ath init` asks for optional profile fields and creates
`athlete.ath.sqlite`. It also adds a manual source and the standard benchmark
definitions. Pass `--timezone America/Chicago` to set the default IANA timezone for
calendar calculations.

The command installs the tool and calculation skills into `.agents/skills/` and
supported agent folders already present. Pass `--no-skill` to skip that step.

Use an empty database only as the destination for a complete v1 restore:

```sh
ath init --empty -y
ath import ./saved-package
ath import ./saved-package --yes
```

Athletic Standard v1 does not import prerelease `0.x` Athletic Standard documents.
The old npm package remains available for reading those files.

## Import provider data

`ath import` reads Apple Health, WHOOP, Oura, Garmin, and Strava exports, GPX and FIT
files, and complete Athletic Standard v1 packages. The first run is always a preview:

```sh
ath import ~/Downloads/apple_health_export.zip
```

Provider previews report new, matching, changed, skipped, and conflicting records.
Package previews report new, matching, changed, conflicting, unrepresentable, and
unknown records. Unknown or unrepresentable content blocks the write. No option
silently discards it. Apply a conflict-free preview with a second command:

```sh
ath import ~/Downloads/apple_health_export.zip --yes
```

Provider rows use route-specific import keys. Importing unchanged data reports a
match. Changed data with the same key creates a new revision of the existing
record. It does not create a second record.

A JSON file from Open Wearables or Junction is read through a mapping file:

```sh
ath import ~/Downloads/open-wearables.json --mapping open-wearables
ath import ~/Downloads/open-wearables.json --mapping open-wearables --yes
```

The mapping file lists their field and our field. A mapped field becomes the typed
value. Every other field stays under the name it arrived with. The result lists
each object that was left out and why. The shipped files are in
[`mappings/`](mappings/). A person can add one in `.agents/mappings/`.

When a v1 package conflicts with local history, the preview creates a temporary
conflict set:

```sh
ath conflicts show
ath conflicts resolve <record-id> --choice local
ath conflicts resolve <record-id> --choice imported
ath conflicts resolve <record-id> --choice corrected --data '<complete-data-json>'
ath import ./saved-package --yes
```

The conflict commands do not change athlete data. The final import writes the
non-conflicting records and every chosen resolution in one transaction. A changed
package or database makes the choices stale.

Apple Health, WHOOP, and Oura routes remain separate sources. Records from two
devices also remain separate. A manual source never enters a device baseline.

## Log records

`ath log` is the general creation command. It is also the only command that enters a
benchmark result.

Plain text covers common entries:

```sh
ath log RMSSD 61 ms
ath log sore quads 4/5
ath log Fran in 4:41 rx
ath log Meal photo --attachment ./receipt.png
```

Say `RMSSD` or `SDNN`. They are different statistics and never share a baseline.

Structured input is one v1 object or an array of v1 objects. Every item has
`record_type` and `data`. Prediction-result links and grades are tool-managed and
cannot be supplied this way:

```sh
ath log '{
  "record_type": "athlete_report",
  "data": {
    "report_type": "note",
    "reported_at": "2026-10-13T04:00:00.000Z",
    "input_reported_at": {
      "timestamp_text": "2026-10-13T04:00:00.000Z",
      "utc_offset_text": "Z",
      "utc_offset_meaning": "documented_recording_offset",
      "timestamp_basis": "source_instant"
    },
    "reported_via": "text",
    "note": "Easy session; no soreness."
  }
}'
```

`record_id` and `created_at` may be supplied beside `record_type`. When omitted,
`ath` creates a UUIDv7 record ID and uses the current time. A structured array is
validated and written in one transaction.

`--attachment` accepts PNG, JPEG, WebP, and PDF files up to 25 MiB. Pass it more
than once for several files. The log batch must contain exactly one athlete report
or nutrition entry to own them. The file hashes, attachment records, links, and
owner-only stored files are written together after confirmation.

The command shows a preview before writing. `--json` returns the preview and never
acts as confirmation. After review, run the command without `--json` and confirm in
the terminal, or pass `--yes`.

When a new benchmark result matches exactly one open prediction, the same
transaction also creates a prediction-result link and a grade. It does not guess
when no prediction or several predictions match.

## Read and change records

Use the fixed overview first:

```sh
ath stats
ath check
```

Use stable record IDs for revisions and links:

```sh
ath records list
ath records list --type prediction
ath records show <record-id> --history --links
```

`ath records list` shows current records in stable order. `ath records show` can
show one retained revision, the full revision history, and incoming or outgoing
links.

Corrections keep the record ID and retain the prior complete state:

```sh
ath correct <record-id> --revision 2 --data '<complete-data-json>' --json
ath correct <record-id> --revision 2 --data '<complete-data-json>' --yes
```

`--revision` prevents an old preview from overwriting a newer change.

`ath link <result-id> <session-id> --revision <current-revision>` links a benchmark
result to a workout session. The link is stored as a new revision of the benchmark
result. A stale revision is refused.

Normal deletion and privacy erasure are different operations:

```sh
ath delete <record-id> --revision 2 --json
ath erase <record-id> --revision 2 --json
```

`ath delete` hides the current record and keeps its revisions. `ath erase` removes
the record's values, value-bearing revisions, dependent records, and attachment
files. The preview records the database revision and complete dependency set. The
write is refused if either changes before confirmation. Interactive use asks for
normal confirmation and then requires the exact record ID. Non-interactive use
needs both flags:

```sh
ath erase <record-id> --revision 2 --reason athlete_request --yes \
  --confirm-erasure <record-id>
```

`--json` never performs erasure. Erasure cannot remove copies already present in
exports, backups, filesystem
snapshots, SQLite journals, or storage-device history.

## Read samples and calculate results

Dense samples live in SQLite. There are no separate live sample files.

```sh
ath series heart_rate
ath series heart_rate --raw --from 2026-10-01 --to 2026-10-07
```

`ath calculate` runs named operations without writing athlete data:

```sh
ath calculate baseline hrv_rmssd --from 2026-07-01 --to 2026-09-30
ath calculate summary steps --group day --timezone America/Chicago
ath calculate compare-sources resting_heart_rate \
  --source <source-id> --compare-source <other-source-id>
ath calculate baseline recovery --record-kind provider-score
ath calculate provider-period-summary readiness \
  --record-kind provider-score --period-type oura_day_unspecified
```

The operations are `summary`, `baseline`, `trend`, `coverage`,
`compare-sources`, `correlation`, `workout-drift`, and
`provider-period-summary`. Record kinds are `measurement`, `provider-score`,
`provider-summary`, and `session-summary`. Measurements are the default. Naming the
kind prevents a provider score or reported total from being mixed with a sensor
measurement that has a similar name.

`provider-period-summary` requires `--period-type`. It groups by the exact Oura day,
WHOOP cycle, or Apple activity-summary day stored on each record. It never relabels
that period as a midnight-to-midnight calendar day.

Results name their method, sources, input revisions, exclusions, range, and timezone
or provider period when one was used. Receipts are `ath-runtime-v2`: a baseline adds
a daily-mean summary when a time zone is known, a trend adds its standard error, and a
correlation needs at least three paired days.

`ath apply <calculation> --record <id>` runs one calculation skill on one record,
such as `hrv-change` or `sleep-regularity`. Each skill's `SKILL.md` states its
method and holds a worked example that the test suite runs. A receipt lists at most 1,000 input revisions
and says when that list was shortened. Arithmetic refuses more than 100,000 inputs
until the request is narrowed. It never calculates from a silently shortened input
set. Sources are not pooled. Manual sources are excluded.

## Export a complete package

```sh
ath export ./athlete-backup
```

SQLite remains the sole live local record. The export is a complete portable
snapshot. It does not stay synchronized with the database.

An exported directory contains:

```text
athlete-backup/
  athlete.ath.json
  series/
    <source-id>-<quantity>-000001.ath-series.json
  attachments/
    <stored attachment files>
  export-manifest.json
```

`athlete.ath.json` contains every current non-sample record, every retained
non-sample revision, normal deletions, erasure markers, provider import keys, and
references to generated sample files. Each generated sample file contains current
sample records and their history. A large source-and-quantity group is split
across numbered files. The main document lists them in `measurement_series_files`.
The manifest lists file hashes, byte counts, record counts, the source database
ID, and the database revision.

The export time is absent by default. Two exports from the same database revision
with the same options are byte-identical. Pass a fixed `--exported-at` value only
when the manifest needs one.

Importing an exported package is explicit and uses the same preview-then-`--yes`
workflow as a provider import.

## Predict and grade

Inside an agent harness, ask the database for prediction evidence:

```sh
ath predict fran --json
```

This returns evidence, not a prediction. The agent writes a `prediction` record
through structured `ath log`.

In a terminal, bare `ath predict fran` can call a selected model and save its
prediction after confirmation. That optional model call needs a Vercel or OpenRouter
key in the operating-system password store. Local database use does not need that
key or a network connection.

Results are entered with `ath log`. Explicit grading only joins two records that
already exist:

```sh
ath records list --type prediction
ath records list --type benchmark_result
ath grade <prediction-id> --result <result-id> --json
ath grade <prediction-id> --result <result-id> --yes
```

`ath grade` never logs a result. It creates a UUIDv7 prediction-result link when one
does not exist. A corrected result revises the existing link, so older grades retain
their exact link revision. The command saves deterministic grade arithmetic against
the current link revision.

A prediction error analysis is a separate authored record written through
structured `ath log`.

`ath backtest` replays historical results and writes a report beside the database.
It does not change athlete data.

## Record model

Every logical record has a stable UUIDv7 ID and numbered revisions. A correction
keeps the ID. Evidence names both the record ID and revision.

The format defines 24 record types for profile data, sources, measurements,
sessions, athlete reports, nutrition, attachments, benchmarks, predictions, plans,
goals, and saved authored work. [`SPEC.md`](SPEC.md) is the public field reference.
The generated JSON Schemas are in [`schema/2.1.3/`](schema/2.1.3/). The published
2.0.0, 2.1.0, 2.1.1, and 2.1.2 schemas stay in [`schema/2.0.0/`](schema/2.0.0/),
[`schema/2.1.0/`](schema/2.1.0/), [`schema/2.1.1/`](schema/2.1.1/), and
[`schema/2.1.2/`](schema/2.1.2/).

How the code is organized, and the path of one import and one correction to disk, is
in [`docs/architecture.md`](docs/architecture.md).

Provider data research is in
[`docs/connections.md`](docs/connections.md). The installed agent instructions are
in [`skills/`](skills/).

## License

MIT License. See [LICENSE](LICENSE). Copyright 2026 Athletic Standard contributors.
