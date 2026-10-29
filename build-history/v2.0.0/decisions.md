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

## D164. An older tool skill is replaced

`ath skill install` and `ath --update` replace `athletic-standard` when the
installed version is older than the version this program ships. The agent
then reads the instructions that belong to this program. D81 rejected leaving
that copy, because the agent would keep reading the old instructions. That
reason still holds.

A calculation skill that is already installed is not replaced. That file is
the method, and the person may have edited it. A tool skill whose version is
already this version is not replaced, because that copy may have been edited.

This supersedes D81, D79, and D130.

D79 warned when the installed skill was older. There is no such warning. The
older tool skill is replaced instead. A calculation skill is left alone, so
there is nothing to warn about.

D130 kept the npm package version apart from `athleticstandard_version`.
D152 made them the same number, `2.0.0`. D130's split does not apply from
2.0.0 on. D152 is not rewritten.

Rejected alternatives:

- Leave every installed file, including an older tool skill. Rejected because
  the agent would keep reading the old instructions. That is the reason D81
  gave, and it is still the reason.
- Replace a calculation skill when its version is older. Rejected because
  that file is the calculation the person may have changed.
- Replace a tool skill that is already this version. Rejected because the
  person may have edited it, and the version says it matches this program.
- Keep the npm package version and the record version as two numbers.
  Rejected because D152 already made them one number, and a change should
  not have to pick which number moves.

## D165. The published 1.0.0 migration stays as it was published

`spec/sql/migrations/001-initial.sql` and `src/db/migrations/001-initial.ts`
are the files from version 1.0.0. They are not edited to add the 2.0.0
columns. A database created by this program runs migration 003 immediately
after the initial tables, and its schema version is already 3. A database
from version 1.0.0 does not have the workout name, description, text result,
or route table. `ath migrate` adds them.

D154 said a database that already has the new columns only updates its
version. That sentence described an edit to the published migration. That
edit is not how this version works. A published 1.0.0 database does not
already have the columns. A database created by this version is already at
schema 3, so migrate does not run the step again.

Repair and check still run on a version 1.0.0 database before that step.
The workout name, description, text result, and route table are not there
yet. A typed-row rebuild writes a column only when the table has it. A
route is written or compared only when the route table exists. The
published 1.0.0 fixture reaches both of those states. The upgrade step
itself always adds the columns. It does not look to see whether they are
already there.

Rejected alternatives:

- Edit the published 001 migration in place. Rejected because a database
  created by the published 1.0.0 package used the old file, and changing
  that file does not change those databases.
- Skip the column adds when the columns are already there. Rejected because
  a real 1.0.0 database does not have them, and a new database does not run
  the step twice.

## D166. WHOOP and Oura CSV reading stay in ath-core

WHOOP and Oura CSV files are read in `ath-core` in version 2.0.0. The
command-line tool assigns the source id and writes the records. The
TypeScript readers are not a second copy.

A timestamp may include fractional seconds before the offset, as in
`2026-08-09T07:00:00.000-04:00`. That string is a time. The offset is at the
end, after the fraction. Sleep sessions are created when both bedtimes parse
and the end is later than the start. HRV, respiratory rate, and resting heart
rate points use that time. A synthetic Oura file with those timestamps is in
`tests/fixtures/exports/oura-offset/sleep.csv`.

D162 and D163 are not rewritten. This decision records that the port stays
in this version, and that the time parser accepts a fractional second.

Rejected alternatives:

- Take the WHOOP and Oura readers back out of this version. Rejected because
  the readers produce the same records those files already produced, once the
  fractional-second offset is read.
- Treat a bedtime with fractional seconds as a date only. Rejected because
  the time is in the file, and dropping it drops the sleep session.

## D167. The npm package includes a built ath-core

The published package includes a built `ath-core` binary for linux-x64,
darwin-arm64, and win32-x64, under `vendor/ath-core/<platform>-<arch>/`.
`ath` runs that binary. It does not look for a Rust crate inside the
installed package. When this repository is present, `ath` rebuilds the
binary if it is older than the Rust source, and it refuses to run the old
binary if the rebuild fails. The check packs the package, installs that
tarball in a clean directory, and runs `ath init`, `ath log`, `ath import`
of a synthetic GPX file, and `ath apply`.

Rejected alternatives:

- Build the crate from source when someone installs the package. Rejected
  because the crate is not in the package, and a person who installed `ath`
  should not need a Rust compiler.
- Use whatever binary is already in `target/`. Rejected because that file
  can be left over from an older checkout, and it then writes with the old
  rules.

## D168. The shipped ath-core binary is marked executable before it runs

The check builds `ath-core` on Linux, macOS, and Windows, then downloads
those files before it packs the npm package. That download does not keep
the executable permission. The check marks each binary executable before
`npm pack`. `ath` does the same before it runs the binary, so an install
that lost the permission still runs.

Rejected alternatives:

- Leave the permission as the download stored it. Rejected because `ath log`
  then fails with EACCES on the Linux binary.
- Commit the binaries to Git so the permission is stored there. Rejected
  because the check builds them for each platform, and Git is not where
  those built files belong.
