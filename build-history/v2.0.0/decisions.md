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

## D160. Each calculation is its own skill

The source folder is `skills/`. `skills/athletic-standard/` is the tool skill.
Each calculation is `skills/<name>/SKILL.md`, and the folder name matches the
`name` in that file. `ath apply` reads that file. `ath init` copies every skill
into `.agents/skills/<name>/`. This is the layout `npx skills add` looks for.
D49 still describes the earlier single-skill choice. It is not rewritten.

Rejected alternatives:

- Keep the calculation notes inside one `skill/calculations/` folder. Rejected
  because those files have no skill name or description, so they cannot be
  installed as skills.
- Replace an installed skill when the folder layout changes. Rejected because
  an installed file stays as the person left it.

## D161. A new source is a field mapping, not a new record format

Open Wearables and Junction rewrite each device into their own objects. The
usual read is that object, not the original export. Athletic Standard keeps
its own record. A pull, when it is built, reads one service's summaries and
applies a mapping file. A mapped field becomes the typed value. Any other
field stays in `supplied_fields` under the name it arrived with. Their record
id is the match key. The source names the device and the path. The key stays
in the password store.

After a reader has turned the bytes into named fields, a new source is a new
mapping file. The file lists their field and our field. A person can add one.
XML, FIT, GPX, and CSV still need a reader. The mapping cannot turn a personal
report into a measured record. It cannot rename a generic HRV field to
`hrv_rmssd` or `hrv_sdnn`. Shipped mappings for the later pull are Open
Wearables summaries and Junction summaries. Junction's raw endpoint is not the
default. The comparison and the field table are in
[appendix/source-formats.md](appendix/source-formats.md). D153 still says
unknown fields keep their original names. It is not rewritten.

Rejected alternatives:

- Use the Open Wearables objects as the Athletic Standard record. Rejected
  because a personal report, a provider period, an original field name, and a
  second device's copy of the same workout would have nowhere to stay.
- Treat every HRV number from a summary as RMSSD. Rejected because a field
  that does not name the statistic is not one. `LastNightAvg` stays
  `LastNightAvg`.
- Make the default pull Junction's raw payload. Rejected because that payload
  is a different format for each provider, which is a provider reader rather
  than one mapping.

## D162. Record writes and the eight range calculations run in ath-core

`ath-core persist` writes a checked record, a correction, a deletion, and a
provider import. `ath-core calculate-range` runs summary, baseline, trend,
coverage, compare-sources, correlation, workout-drift, and
provider-period-summary. Each of those eight is a skill under `skills/`. The
command `ath calculate` reads that file and calls the library. WHOOP CSV
reading also runs in `ath-core`. The field checks stay in `src/v1/schema.ts`.

Apple Health, Oura, package import, checks, the upgrade, export, and erasure
still run in TypeScript. D155 said those pieces move after the same tests
pass in Rust. That wait is not finished.

Rejected alternatives:

- Leave the record write in TypeScript because the tests already passed there.
  Rejected because a second copy of the write will drift from the library.
- Hide the eight calculations inside the command and skip the skill files.
  Rejected because a calculation is a skill, and the file names the steps.

## D163. Oura CSV reading runs in ath-core

`importOura` asks `ath-core` to read the CSV files. The command-line tool
still assigns the source id and folds the signals into the record. The Oura
reader is no longer a second copy in TypeScript. D162 listed Oura with the
pieces that were still TypeScript. This decision moves that one reader. Apple
Health remains in TypeScript.
