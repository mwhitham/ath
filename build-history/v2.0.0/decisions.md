# v2.0.0 decisions

Decisions continue after D151. Earlier decisions stay as they are.

## D152. One software version, 2.0.0

The program version, the record version, and
`database_metadata.athleticstandard_version` are `2.0.0`. A person sees that
number from `ath --version` and from the database. The upgrade command still
stores an integer step count because SQLite's `user_version` header is an
integer. That integer is the upgrade step. It is not a second version of the
software.

Rejected alternatives:

- Keep the package at 1.1.0 while the record stays 1.0.0. Rejected because a
  change then has to pick which number moves.
- Show the SQLite header integer as the database version. Rejected because the
  header cannot store `2.0.0`.

## D153. Keep supplied fields under their original names

A workout stores a name, description, text result, heart-rate samples, maximum
and resting heart rate, strength sets, and route points when the file has them.
Anything else on that record is kept in `supplied_fields` under the name it
arrived with. The import result says which names were understood. The original
file is not stored.

Rejected alternatives:

- Drop unknown columns. Rejected because the person supplied them.
- Save a copy of the import file. Rejected because the record should hold the
  fields, not a second copy of the file.

## D154. Upgrade version 1 databases with the existing command

`ath migrate` previews, saves a checked copy, and asks before it writes. The
new step adds the workout columns and route table and sets the database version
to 2.0.0. History, links, attachments, and deletion records stay. A failed
upgrade leaves the original file for restore from that copy. A database that
already has the new columns only updates its version.

Rejected alternatives:

- Write a second upgrade command. Rejected because `ath migrate` already does
  the preview, copy, and confirmation.
- Upgrade on open. Rejected because opening a file must not change it.

## D155. New imports and calculation files run in one Rust library

`crates/ath-core` writes the new imports, runs calculation files, and applies a
correction to an existing record. `ath` calls that program. A test writes with
one and reads with the other on the same file. TypeScript keeps the commands
and the imports it already tested. Those pieces move only after the same tests
pass in Rust.

Rejected alternatives:

- An iPhone app in this version. Rejected because this environment cannot build
  or run one, so the check would never pass.
- A server between the command-line tool and the library. Rejected because both
  can open the same file.

## D156. Garmin files and the personal connection; Strava files only

Garmin import reads account JSON, GPX, and FIT record points. `ath pull garmin`
exchanges a personal key for a token and reads activities and `LastNightAvg`
for one date. The key is not stored in the database. Strava import reads
`activities.csv` and linked GPX files. Strava's online API is not called. The
permissions question stays unanswered.

Rejected alternatives:

- Label `LastNightAvg` as RMSSD. Rejected because Garmin's field does not say
  which statistic it is.
- Decide the Strava terms question inside this repository. Rejected because the
  cited pages do not settle it.

## D157. Calculation steps live in skill files

`skill/calculations/` holds one file per calculation. The program runs the
installed copy. A file that is already installed is not replaced by
`ath skill install` or `ath --update`. A different method is a new file name.
The shipped zone file is not edited to change old results.

Rejected alternatives:

- Hide the steps in the Rust library only. Rejected because a person must be
  able to change a calculation by editing a file.
- Put a second copy beside an edited file. Rejected because the program would
  have to choose which file to run.

## D158. One record per calculation

`ath apply` takes one record. The result names that record and the fields it
read. Run the same file on another record to see the other device. Sleep
regularity and the change calculations may read earlier records from that same
source. They do not mix sources, and they do not add two copies of one activity.

Rejected alternatives:

- Pick a source automatically and hide the other record. Rejected because the
  person needs to see how the calculation maps onto each device.
- Combine RMSSD and SDNN in one change. Rejected because they are different
  statistics.

## D159. Recovery is three changes

Heart-rate variability change, resting heart rate change, and sleep change are
separate results. There is no Athletic Standard recovery score. A score computed
by a device maker stays that maker's score.

Rejected alternatives:

- Add an overall recovery number. Rejected because the three inputs do not
  become one measurement by being added together.
