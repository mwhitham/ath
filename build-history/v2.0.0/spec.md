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
and FIT record points. `ath pull garmin --date YYYY-MM-DD` reads Garmin's
personal connection for that day. The personal key stays in the password store
or `GARMIN_PERSONAL_API_KEY`. It is not written into the database.

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
skill on one record. Editing that file changes the result. An installed skill
file is not replaced. A new calculation is a new skill folder. `ath skill
install` and `ath --update` add files that are not already there.

The shipped files are heart-rate zones, heart-rate recovery, resting heart
rate, training load from heart rate, training load from reported effort, sleep
duration, efficiency, interruptions, timing, stages, and regularity, strength
volume, and the separate changes in heart-rate variability, resting heart rate,
and sleep. There is no combined recovery score. RMSSD and SDNN stay separate.
Garmin's `LastNightAvg` stays under that name.

Worked numbers are in [appendix/calculations.md](appendix/calculations.md).

## What stays in TypeScript

The command-line tool still reads commands, prints help, installs the skill,
stores keys, and calls a language model. Storage, checks, and the Apple, WHOOP,
and Oura imports that already matched their tests stay in TypeScript until the
Rust library has been shown to do that same work. New Garmin and Strava
imports, calculation files, and corrections made through `ath-core` run in the
Rust library. The command-line tool calls that library.

The iPhone app is not in this version. Nothing in this build can run it. Two
programs on Linux share one file instead: `ath` and `ath-core`.

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

`pnpm test` includes the shared-file example. Rust tests check the zone
arithmetic, a FIT record point, and the Garmin token request against a local
server. There is no check that only a Mac can run.

## Order that landed

1. Version 2.0.0 for the program, the record, and the database.
2. Workout names, text, routes, and original field names.
3. The existing upgrade command, extended with one step.
4. The Rust library, with two Linux programs sharing one record.
5. Garmin import, then Strava import.
6. Calculation skill files, checked against the worked zone example.
7. The Linux checks above. Old TypeScript stays where the Rust library has not
   yet repeated that work.
