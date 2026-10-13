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

Not an app, not a coach, not medical advice.

## Security

The database contains sensitive health and training data. Athletic Standard creates
local files with owner-only permissions where the operating system supports them,
but it does not encrypt the database itself. Use operating-system disk encryption.
Anyone who can read the database file can read the athlete record.

## Install

Athletic Standard needs Node.js 20 or newer.

```sh
npm install -g athleticstandard
```

This installs the `ath` command. `npx athleticstandard` can be used instead of a
global install.

Run `ath --update` to update the package. Run `ath skill install` to refresh the
local agent skill without updating the package.

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

The command installs the agent skill into `.agents/skills/` and supported agent
folders already present. Pass `--no-skill` to skip that step.

Use an empty database only as the destination for a complete v1 restore:

```sh
ath init --empty -y
ath import ./saved-package
ath import ./saved-package --yes
```

Athletic Standard v1 does not import prerelease `0.x` Athletic Standard documents.
The old npm package remains available for reading those files.

## Import provider data

`ath import` reads Apple Health, WHOOP, Oura, and complete Athletic Standard v1
packages. The first run is always a preview:

```sh
ath import ~/Downloads/apple_health_export.zip
```

Provider previews report new, matching, changed, skipped, and conflicting records.
Package previews report new, matching, changed, conflicting, unrepresentable, and
unknown records. Apply a conflict-free preview with a second command:

```sh
ath import ~/Downloads/apple_health_export.zip --yes
```

Provider rows use route-specific import keys. Importing unchanged data reports a
match. Changed data with the same key creates a new revision of the existing
record. It does not create a second record.

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
```

The operations are `summary`, `baseline`, `trend`, `coverage`,
`compare-sources`, `correlation`, and `workout-drift`. Results name their method,
sources, input revisions, exclusions, range, and timezone when one was used.
A receipt lists at most 1,000 input revisions and says when that list was shortened.
Arithmetic refuses more than 100,000 inputs until the request is narrowed. It never
calculates from a silently shortened input set. Sources are not pooled. Manual
sources are excluded.

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
    <source-id>-<quantity>.ath-series.json
  attachments/
    <stored attachment files>
  export-manifest.json
```

`athlete.ath.json` contains every current non-sample record, every retained
non-sample revision, normal deletions, erasure markers, provider import keys, and
references to generated sample files. Each generated sample file contains current
sample records and their history. The manifest lists file hashes, byte counts,
record counts, the source database ID, and the database revision.

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

The format defines 23 record types for profile data, sources, measurements,
sessions, athlete reports, nutrition, attachments, benchmarks, predictions, plans,
goals, and saved authored work. [`SPEC.md`](SPEC.md) is the public field reference.
The generated v1 JSON Schemas are in [`schema/1.0.0/`](schema/1.0.0/).

Provider data research is in
[`docs/connections.md`](docs/connections.md). The installed agent instructions are
in [`skill/`](skill/).

## License

MIT License. See [LICENSE](LICENSE). Copyright 2026 Athletic Standard contributors.
