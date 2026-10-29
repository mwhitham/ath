# Athletic Standard 2.0.0

The software version is 2.0.0. The program, the saved record, and
`database_metadata.athleticstandard_version` use that number. When any of them
changes, that number changes with it.

A version 1.0.0 database upgrades with `ath migrate`. The upgrade shows the
change, saves a copy, and asks before it writes. History, links, attachments,
and deletion records stay. If the upgrade fails, the original file is unchanged.
Opening a file does not upgrade it.

## What the record holds

A workout can store a name, a description, a text result, heart-rate samples,
a maximum heart rate, a resting heart rate, strength sets, and a route of timed
points. A sleep record can store time asleep, time in bed, interruptions, and
stage minutes. A field the software understands is stored as one of those
values. A field it does not understand stays in `supplied_fields` under the
name it arrived with. The import result lists both groups.

The original import file is not copied into the record. Files that are not
training or recovery, such as orders or follower lists, are named in the import
result and left out.

Measured readings and personal reports stay separate. Training load from
reported effort reads a personal report. Training load from heart rate reads a
workout.

## Imports

Garmin import reads an account directory or zip, a JSON activity, a GPX file,
and FIT record points. A FIT value that means "not present", including a
latitude of `0x7FFFFFFF`, is left out. A GPX heart-rate element is read with
or without a namespace prefix. A GPX point with no time is left out, and
the import result counts those points. The workout is checked against the
field rules before it is written (D169). `maximum_heart_rate_bpm` on a
workout is the athlete's known maximum. FIT, GPX, and Garmin JSON do not
set it. The athlete sets it with `ath correct`. Garmin JSON `maxHR` is that
activity's peak and is stored as a `max_heart_rate` provider summary (D172).
`ath pull garmin --date YYYY-MM-DD` reads Garmin's personal connection for
that day. The personal key stays in the password store or
`GARMIN_PERSONAL_API_KEY`. It is not written into the database.

Strava import reads a bulk export that contains `activities.csv` and the
activity files named there. The question of whether Strava's terms allow that
use is not decided. The pages are listed in
[appendix/garmin-and-strava.md](appendix/garmin-and-strava.md).

When two devices describe the same activity, both records are kept. A
calculation uses one record. Run it again with the other record to see which
fields it read. Do not add the two records together.

## Calculations

Calculation steps ship as skills under `skills/<name>/SKILL.md`. The program
reads the installed file and runs it. `ath apply <name> --record <id>` runs one
skill on one record. Editing that file changes the result. An older copy of
the tool skill is replaced, so the agent reads the instructions that ship
with this program (D164). A calculation skill that is already installed is
not replaced. A new calculation is a new skill folder.

The shipped files are heart-rate zones, heart-rate recovery, resting heart
rate, training load from heart rate, training load from reported effort, sleep
duration, efficiency, interruptions, timing, stages, and regularity, strength
volume, and the separate changes in heart-rate variability, resting heart rate,
and sleep. There is no combined recovery score. RMSSD and SDNN stay separate.
Garmin's `LastNightAvg` stays under that name.

Worked numbers are in [appendix/calculations.md](appendix/calculations.md).

## What stays in TypeScript

The command-line tool still reads commands, prints help, installs the skill,
stores keys, and calls a language model. The published field list stays in
`src/v1/schema.ts`.

Record writes, corrections, deletions, and provider-import writes run in
`ath-core` (D162). WHOOP and Oura CSV reading run there too (D162, D163). The
eight range calculations run there, from skill files. Garmin and Strava imports
and one-record calculation files run there. Checks, the upgrade command, package
import and export, and erasure still run in TypeScript. Apple Health still runs
in TypeScript. D155 said those pieces move after the same tests pass in Rust.
That wait is not finished for the pieces still in TypeScript.

The iPhone app is not in this version. Nothing in this build can run it. Two
programs share one file: `ath` and `ath-core`. The npm package includes a
built `ath-core` binary for linux-x64, darwin-arm64, and win32-x64 (D167).
A checkout of this repository rebuilds that binary when it is older than the
Rust source, and refuses to run the old binary if the rebuild fails. `ath`
marks the binary executable before it runs (D168).

Strava's online connection and an Android app wait. They are not needed for
these imports and calculations.

## Checks

On every change, run:

```
pnpm check:public-data
pnpm typecheck
pnpm test
pnpm build
cargo test --manifest-path crates/ath-core/Cargo.toml
```

Publishing is not one of those commands. The release workflow packs with
`npm pack`, checks that file, and publishes it:

```
node scripts/assert-packed-binary.mjs athleticstandard-2.0.0.tgz
npm publish athleticstandard-2.0.0.tgz
```

The `2.0.0` in the filename is the version in `package.json` (D171).
`npm publish` and `pnpm publish` with no file argument are refused.

`pnpm test` includes the shared-file example. Rust tests check the zone
arithmetic, a FIT record point, the Garmin token request against a local
server, GPX sample times, a WHOOP row, an Oura row with a fractional-second
offset, a record write, and a baseline. There is no check that only a Mac can
run.

## Order that landed

1. Version 2.0.0 for the program, the record, and the database.
2. Workout names, text, routes, and original field names.
3. The existing upgrade command, extended with one step.
4. The Rust library, with two Linux programs sharing one record.
5. Garmin import, then Strava import.
6. Calculation skill files, checked against the worked zone example.
7. The Linux checks above. Old TypeScript stays where the Rust library has not
   yet repeated that work. WHOOP and Oura reading were already in this
   repository before this version. They now run in the Rust library, and the
   tests have to show the same records, including sleep sessions and timestamps.
