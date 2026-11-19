# v2.1.2 decisions

Decisions continue after D226. Earlier decisions stay as they are. The plan is
[spec.md](spec.md).

## D227. Calculation repairs, session effort, and coaching skills ship as 2.1.2

The program, npm package, record package, and database metadata use 2.1.2. D152's
single version rule stays. The SQL migration step becomes 6.

The number moves because the record gains optional fields and a new report type
(D238, D239), and because calculation methods change. The maintainer chose 2.1.2
for this work on 2026-11-15.

The published version rule says a minor version may add optional fields and a
patch version adds none (D96; SPEC.md section 13). Version 2.1.2 is an exception to
that rule, made by the maintainer's choice. SPEC.md section 13 records the
exception: 2.1.2 adds optional fields and one report-type value, and removes or
changes nothing. Every 2.1.1 record and package stays valid 2.1.2 data. A 2.1.1
program refuses a 2.1.2 database by its SQL step, as every older program refuses a
newer step. The rule stays in force for later releases.

Rejected alternatives:

- Ship the repairs as a later 2.1.1 build. Rejected because 2.1.1 promised
  unchanged methods and fields (D209), and the repairs change both.
- Use 2.2.0, which the version rule suggests. Not chosen: the maintainer chose
  2.1.2. Changing the number later is mechanical: the constants, the schema folder,
  and the migration file name.

## D228. Three kinds of skill; coaching advice is authored

`skills/` holds three kinds of skill. The tool skill reads and writes the record.
A calculation skill produces one reproducible result. A coaching skill assesses,
plans, adapts, and reviews training for one sport.

The record and the tool skill stay "not a coach": they describe what the data
shows and do not prescribe training. A coaching skill is the only place advice is
given. Its advice is saved as an authored record (`training_plan`,
`planned_session`, `coaching_note`, `saved_training_review`, or `goal`) with agent
authorship and exact evidence revisions. It is never written as a measured record
or an athlete report. No skill gives medical advice; pain, injury, and illness go
to a qualified person.

Each shipped `SKILL.md` names its kind in `metadata.kind`: `tool`, `calculation`,
or `coaching`.

Rejected alternatives:

- Let the tool skill coach. Rejected because the record format would then imply
  that one coaching method is part of the standard.
- Keep coaching out of the repository. Rejected because the maintainer wants
  official CrossFit and HYROX skills held to the same tests as the calculations.

## D229. A coaching skill is installed only when named

`ath init`, `ath skill install`, and `ath --update` install the tool skill and the
calculation skills. A coaching skill is installed only when a person or agent names
it: `ath skill install crossfit`. A coaching skill that is already installed is
refreshed when an older version is there, as the tool skill is (D164), because an
agent would otherwise keep reading old advice. `ath skill` lists coaching skills
that are available and not installed.

This follows D120: a person chooses to receive coaching advice. A named install
of a skill this program does not ship is refused.

Rejected alternatives:

- Install every shipped skill by default. Rejected because coaching advice should
  be a choice, and a future set of sports would fill every project.
- Never refresh an installed coaching skill. Rejected because coaching guidance,
  such as a season's rules, goes out of date.

## D230. A changed calculation method gets a new operation name

When a calculation method changes, the shipped file uses a new `use:` operation
name. The old operation name keeps its exact old arithmetic. A file that still
names it gives the same answer it gave before. Its result adds `method_notice`,
which says the method was replaced and why.

This refines D157. D157 required a new file name for a different method. The
file name is the skill name that people and agents ask for, so it stays. The
operation name inside the file carries the method instead.

Rejected alternatives:

- Change the arithmetic behind the old operation name. Rejected because an edited
  file would silently give a different answer for the same records.
- Rename the skills, for example `hrv-change-2`. Rejected because agents and
  people find calculations by their plain names.
- Delete the old operations. Rejected because an edited file would stop working.

## D231. An unedited installed calculation file is replaced

`ath skill install` and `ath --update` replace an installed calculation skill when
its `SKILL.md` bytes exactly match a file that an earlier release shipped. The
program carries the SHA-256 of every shipped calculation file since 2.0.0. Those
copies were never edited, so replacing them loses nothing. A copy with other bytes
was edited. It is kept, and `ath skill --json` marks it `edited` with the shipped
version, so the person can see that the shipped method changed.

This supersedes D157's rule that an installed file is never replaced, for unedited
copies only. D157's rule that the program runs the installed copy stays.

Rejected alternatives:

- Never replace an installed calculation. Rejected because every 2.1.1 user would
  keep the methods this release repairs, with no warning.
- Replace every copy. Rejected because a person's edit is a deliberate choice.
- Compare versions instead of bytes. Rejected because an edited file keeps its old
  version line.

## D232. A change compares the day with earlier days from a bounded window

`hrv-change` and `resting-heart-rate-change` use `earlier_window_change`. The
target value is the mean of the target record's local day, using only readings at
or before the target. The comparison is the mean of earlier daily means from the
28 days before that day. At least 7 earlier days are needed. Only readings from the
same source, quantity, record type, and basis count. Days use the athlete's
`default_time_zone`, or UTC when there is none, and the result names the zone.

The result gives the change, the percentage change, the comparison mean and
standard deviation, the change in standard deviations, the days used, and the
window. It does not judge whether the change matters; the skill explains how to
read it.

Daily means keep a source that reads many times a day (Apple SDNN) on the same
footing as one that reads once (a nightly RMSSD). The 28-day window follows
athlete-monitoring studies that compare a recent value with a reference of several
weeks: Vesterinen et al. (2016) used a 4-week baseline with a band of ±0.5
standard deviations, and Plews et al. used a rolling reference with a smallest
worthwhile change of half the variation. The 7-day minimum is a floor for a
meaningful spread, not a published threshold. The standardized change lets the
skill apply those bands without the program judging them. The sources are in
[appendix/calculation-methods.md](appendix/calculation-methods.md).

This fixes the 2.1.1 behaviour recorded in
[deliberate changes](../v2.1.1/appendix/deliberate-changes.md), where later readings
entered the "earlier" mean.

Rejected alternatives:

- Every earlier reading, unbounded. Rejected because a reading from a year ago
  says little about this week, and readings per day differ by device.
- Compare one spot reading with earlier spot readings. Rejected because a single
  intraday SDNN sample varies more than a daily mean.
- Report a verdict such as "recovered". Rejected by D159: there is no Athletic
  Standard recovery score.

## D233. A comparison keeps record types and bases apart

Within one statistic, a change or baseline compares only readings of the same
record type and the same `field_basis.value`. A nightly point measurement and an
intraday sample are recorded differently. A provider value and a value calculated
during import are made differently. A record without a basis forms its own group.

RMSSD and SDNN stay separate everywhere (D158). Both work through every
calculation that reads a measurement, including samples.

Rejected alternatives:

- Mix point measurements and samples of one statistic. Rejected because recording
  conditions change what the number means.

## D234. Sleep change compares main sleep periods

`sleep-change` uses `earlier_window_change` on sleep sessions. A session counts as
a main sleep when its span is at least 3 hours. The target must be a main sleep.
Earlier main sleeps from the same source that ended before the target started, in
the 28 days before it, form the comparison. At least 7 are needed. Shorter sessions
are naps and are excluded, and the result counts them.

Rejected alternatives:

- Include naps. Rejected because a 20-minute nap pulls down the mean of nightly
  sleep.
- Sum all sleep in a calendar day. Rejected for this skill because the question it
  answers is about one night. Total daily sleep is a separate question.

## D235. Heart-rate recovery starts at the end of effort

`heart-rate-recovery` uses `drop_after_effort_end`. The end of effort is the start
of a workout segment labelled `recovery`, or else the workout end. The heart rate
at that moment comes from the nearest sample at or before it, within 5 seconds.
The heart rate 60 seconds later comes from a sample within 5 seconds of that time,
or from straight-line interpolation between samples no more than 10 seconds apart.
If there are no samples after the end of effort, the result says so.

Many devices stop recording when the workout stops. The skill tells the athlete to
keep recording and mark a `recovery` lap, and to prefer a device's own one-minute
recovery reading (`hr_recovery`) when there is one.

Rejected alternatives:

- Keep the 2.1.1 rule. Rejected because it looked for a sample after the last
  sample and could never succeed.
- Use the peak heart rate instead of the end of effort. Rejected because the peak
  can occur well before the effort ends.

## D236. Sleep regularity uses the Sleep Regularity Index on sleep periods

`sleep-regularity` uses `sleep_regularity_index`. It applies the Phillips et al.
(2017) formula: −100 plus 200 times the share of minute pairs, 24 hours apart, that
are in the same state. A minute is asleep when it lies inside a sleep session from
the same source. Awake time inside a session counts as asleep, because the record
has no minute-by-minute wake data; the result says so. The window is the 14 days
ending at the target session's end. A 24-hour day with no session is unobserved,
and pairs that touch it are left out. At least 6 observed day pairs (7 days) are
needed, the span Lunsford-Avery et al. (2018) used. Phillips et al. used 1-minute
epochs, as this method does.

Seven identical schedules score 100. The 2.1.1 method compared two nights at the
same clock instant rather than 24 hours apart, so identical nights scored 0.5.

Rejected alternatives:

- Keep nightly agreement under a new name. Rejected because it does not measure
  regularity.
- Claim the published index exactly. Rejected because the published index uses
  sleep and wake epochs inside the night.

## D237. Strength volume converts units and load basis

`strength-volume` uses `volume_load`. Each set adds repetitions times load in
kilograms. A pound is 0.45359237 kg. `kg` and `lb`, with common spellings, are
recognised; any other unit stops the calculation and names the set. A
`per_implement` load is multiplied by `implement_count`. A set without a load basis
is counted as written, and the result says how many such sets there were. Volume is
also split by movement when sets name one.

Rejected alternatives:

- Add numbers in whatever unit they came in. Rejected because 100 kg plus 100 lb
  is not 200 kg.
- Refuse a set without a load basis. Rejected because older records never had one.

## D238. Session effort is its own report type, linked to its workout

`athlete_report.report_type` gains `session_effort`, which needs `rating` and
`rating_scale`. `athlete_report` gains optional `workout_session_id`, stored as a
SQLite column with a foreign key (D104) added by migration step 6. Erasing a workout
erases the reports that name it (D107).

`training-load-effort` uses `session_effort_load` (Foster et al., 2001). It needs a
`session_effort` report. It accepts a 10-point scale (`0-10` or `1-10`) or the Borg
`6-20` scale, and the rating must lie on that scale. The duration is the report's
`duration_seconds`, or else the linked workout's span. Load is rating times
minutes, in arbitrary units, and the result names the scale family and where the
duration came from. Loads on different scales are never comparable.

A soreness, mood, or other rating can never become training load.

Rejected alternatives:

- Keep reading any report's `rating`. Rejected because a soreness score is not
  session effort.
- Store the effort on the workout. Rejected by the two-tier wall: a workout is a
  measured record and the effort is the athlete's report.
- Link through an extension. Rejected because D104 requires every logical link to
  have a foreign key.

## D239. A strength set names its movement and load basis

`StrengthSet` gains optional `movement`, `load_basis` (`total` or
`per_implement`), and `implement_count`. `implement_count` is required with
`per_implement` and refused otherwise. The fields live in the stored JSON; no SQL
column changes.

Rejected alternatives:

- Put movement and basis in an extension. Rejected because volume, CrossFit, and
  HYROX all need them, and D118's conditions for an official field are met by the
  calculation that reads them.

## D240. Heart-rate load and zones credit only covered time

`heart-rate-zones` uses `time_in_zones` and `training-load-heart-rate` uses
`banister_trimp`. A sample lasts until the next sample, or until the workout end
for the last one, but never longer than 60 seconds. Longer gaps are reported as
uncovered time.

`banister_trimp` is Banister's TRIMP with the published weighting: 0.64 e^(1.92 x)
for men and 0.86 e^(1.67 x) for women, where x is the heart-rate reserve fraction.
Every later paper we found gives these constants and cites Morton et al. (1990)
and Banister (1991); the originals are closed access and were not read.
It reads `sex` from the athlete profile. Without it, the result says the weighting
is missing rather than guessing. The 2.1.1 method used e^(1.92 x) without the
coefficient for everyone.

Rejected alternatives:

- Treat every gap as covered. Rejected because a lost sensor signal is not time in
  a zone.
- Default to the male weighting. Rejected because it overstates load for women.

## D241. Resting heart rate is the lowest sustained mean

`resting-heart-rate` uses `lowest_rolling_mean`. It is the lowest mean over any
5-minute window that has at least 3 samples and no gap longer than 2 minutes. On a
sleep session it reads `heart_rate` samples from the same source inside the session.
On a workout it reads the workout's own samples, and the skill explains that this is
the lowest sustained rate during that workout, not resting heart rate.

The 5-minute window is a stated convention, not a published definition. No research
source we found defines resting heart rate as a lowest 5-minute mean, and device
makers differ: Apple estimates the lowest rate at rest, and Garmin is reported to
use the lowest 30-minute average in a day. The result therefore differs from a
device's own resting heart rate, and the skill says so. A person can change
`window_ms` in the file.

Rejected alternatives:

- Keep the 2.1.1 rule. Rejected because it needed a heart rate that did not change
  at all for a minute, which real data almost never shows.

## D242. Sleep denominators, stages, and clock times are defined

`sleep-efficiency` uses `sleep_efficiency`: time asleep over time in bed. It refuses
a night with more sleep than time in bed. `sleep-stages` uses `sleep_stage_split`:
a stage named `awake` is counted apart from asleep minutes, and each sleep stage is
reported as a share of asleep minutes. `sleep-timing` uses `sleep_clock_times`: the
start, midpoint, and end in the athlete's time zone, else the offset the source
wrote, else UTC, with the basis named.

## D243. `ath apply` refuses a range calculation with the command to run

A range calculation such as `baseline` needs a quantity and a window, not one
record. `ath apply baseline --record <id>` now refuses and prints the
`ath calculate` command to run. In 2.1.1 it returned "unknown calculation mean",
while the skill advertised `ath apply`.

## D244. Range receipts are version 2

`calculation_version` becomes `ath-runtime-v2`. Results gain fields: a daily-mean
summary in `baseline`, days spanned and slope standard error in `trend`, a coverage
share in `coverage`, a paired-day comparison in `compare-sources`, and sample counts
in `workout-drift`. `correlation` needs three paired days and gives a reason when it
returns null. Existing fields keep their meaning.

Rejected alternatives:

- Leave the range methods alone. Rejected because the plan requires daily weighting
  and honest minimums.
- Change existing fields in place. Rejected because callers read them.

## D245. HYROX splits are a calculation; sport details are extensions

`hyrox-race-splits` is a calculation skill with the `hyrox_splits` operation. It
reads a `benchmark_result` whose segments use the labels in its skill. It adds runs,
stations, Roxzone, and penalties, refuses a result whose parts add up to more than
the total, and reports any unaccounted time. Doing this in code keeps an agent from
counting time twice.

HYROX season, format, division, and penalties are written in the
`org.athleticstandard/hyrox-race` extension. CrossFit variant, loads, substitutions,
time cap, capped status, and tiebreak are written in
`org.athleticstandard/crossfit-score`. Both follow D118: they are documented
conventions with tests, not official fields.

Rejected alternatives:

- Let the agent add the splits. Rejected because a convincing answer can still
  count Roxzone time inside run time.
- Add HYROX and CrossFit fields to the record. Rejected until D118's conditions are
  met across builders.

## D246. Every worked example in a calculation skill runs in a test

A calculation skill's worked example is a fenced `ath-example` block: the records,
the record to run on, and the expected result. `tests/skill_examples.rs` writes the
records to a new database, runs the shipped file, and compares the result. A skill
whose example and program disagree fails the build.

Rejected alternatives:

- Prose examples checked by review. Rejected because the 2.1.1 skills advertised
  commands that did not work.

## D247. Agent evaluations are recorded; human coaching review is separate

Agent behaviour is evaluated in fresh sessions on synthetic databases with the new
skills, the 2.1.1 skills, and no skill. The tool calls, saved records, and claims are
inspected, not only the final prose. Results are recorded in
[appendix/agent-evaluation.md](appendix/agent-evaluation.md). Automated tests never
call a model service.

Independent review by qualified CrossFit and HYROX coaches is a release requirement.
An AI review does not satisfy it. Until it is done, the release evidence lists it as
outstanding.

## D248. skills.sh is prepared, not published

The repository gains `skills.sh.json`, which groups the page into the tool skill,
coaching, and calculations. Nothing is published, submitted, or listed. Local
installation tests set `DISABLE_TELEMETRY=1` and `DO_NOT_TRACK=1` and give the
skills CLI no network route to skills.sh, whose listings come from install
telemetry. The maintainer publishes manually.

## D249. A zero-repetition result is not a benchmark score

`Score.reps` stays positive. Grading divides by the actual score, so a zero would
make the error undefined. A capped "for time" attempt stores the repetitions
completed in `score.reps` and the cap in `org.athleticstandard/crossfit-score`. An
attempt with no completed repetitions is logged as a note.

Rejected alternatives:

- Allow zero repetitions. Rejected because grades and predictions would need a
  second error rule.

## D250. Every skill says how to check for `ath`, from one generated source

Every skill runs `ath`, but a skill installed with the skills CLI arrives as text
only, and any subset can be installed. So every `SKILL.md` carries a section,
`## Before the first ath command`, that says:

- the skill needs `ath` at this version or later, checked with `ath --version`;
- `ath` comes from the npm package `athleticstandard`, and the npm package named
  `ath` is a different program;
- install or update only after the person agrees, then check the version again,
  and stop if it is still too old;
- when the answer needs records or a calculation and `ath` cannot run, stop and say
  how to fix it, rather than reading the database another way or calculating by
  hand.

The text has one source, `assets::skill::prerequisite_section`.
`tests/generated.rs` writes it into every skill and the coaching template and fails
when a copy differs. The version comes from `ATHLETIC_STANDARD_VERSION`, so a release
updates every copy. `compatibility` also names the minimum version and the package.

What `ath` can say for itself stays in its own messages. The no-database message
already names `--file` and `ath init`. A database written by a newer `ath`, and a
skill that names an operation this `ath` lacks, now say to update `ath`
([cli-changes.md](appendix/cli-changes.md), item 6).

Before the change, fresh agents with today's text read the database with `sqlite3`
and did the arithmetic by hand when `ath` was missing or was 1.1.0. They never named
the package. After the change, all six runs stopped, named the right package, and
asked before installing. The study and the runs are in
[skill-prerequisites.md](appendix/skill-prerequisites.md) and the
[agent evaluation](appendix/agent-evaluation.md).

Rejected alternatives:

- Rely on `compatibility`. Rejected because Claude Code accepts the field but does
  not act on it, and Cursor has no such field.
- Point every skill to the tool skill. Rejected because the tool skill may not be
  installed. The skills CLI has no way to declare one skill's need for another.
- A table of every `ath` error in every skill. Rejected because `ath` already
  explains the errors it can see. Repeating them costs tokens on every use.
- Install `ath` automatically, as some published skills do. Rejected because this
  project asks before every write, and Anthropic advises against global installs
  from skills.
- Offer a pinned `npx -y athleticstandard@<version>` in every skill. Rejected
  because every command would then have to change. The README and the npm page
  keep it.
- Put the requirement in `description`. Rejected because the description is loaded
  in every session and is for choosing a skill.
- Write the section by hand in each skill. Rejected because 27 copies drift. The
  generator keeps them identical.
- Change the no-database message. Rejected for now because the baseline agent
  found the database and used it, and never offered `ath init`.

## D251. A version tag stages the npm package; the maintainer approves it

A version tag still runs the full check and builds nothing after it (D225). The
release job now stages the checked tarball with `npm stage publish` instead of
publishing it. It signs in to npm through trusted publishing: GitHub gives the job a
short-lived identity token (`id-token: write`), and npm accepts it only from
`release-cross-platform.yml` in the `release` environment of this repository. No npm
token is stored in GitHub. The trust link on npmjs.com allows staging only, not
`npm publish`.

A staged version cannot be installed. It becomes public when the maintainer approves
it on npmjs.com or with `npm stage approve <stage-id>`, and both ask for the
maintainer's two-factor code. The GitHub release with the native archives is still
created by the same job.

The job uses Node 24 and npm 12. Node 22 ships npm 10, which has neither trusted
publishing (npm 11.5.1) nor staging. A dry run of npm 12.2.3 confirmed that
`npm stage publish` accepts a tarball path, as `npm publish` does, so the tested
tarball is the one staged.

npm expires a new trust link that has not been used within 2 days. The maintainer
creates it, or recreates an expired one, and pushes the version tag within those 2
days. A link that has been used does not expire.

Rejected alternatives:

- Keep `npm publish` with the `NPM_TOKEN` secret. Rejected because a stored token
  can publish without the maintainer, and npm advises deleting tokens that bypass
  two-factor sign-in.
- Allow `npm publish` on the trust link. Rejected because a pushed tag would then
  make the package public with no human step. Staging keeps the maintainer's
  approval that D248 and the onboarding rules require.
- Publish from the maintainer's computer. Rejected because the tarball would not be
  the one the full check tested on three systems.
- Use `npm@latest`. Rejected because a future major version could change its Node
  requirement or behaviour without a change to this file. npm 12 is pinned by major
  version.

## D252. The maintainer starts a release from a pull request comment

A release no longer needs a terminal. After a merge to `main`, if the version in
`packages/npm/package.json` has no tag, `.github/workflows/release-commands.yml`
comments on the merged pull request with how to release it. When a repository admin
comments `/release` on that pull request, the workflow starts Release cross-platform
on `main` with `publish` set. The same run can be started by hand from Actions > Run
workflow, on the website or in GitHub Mobile.

A release from `main` runs the full check, then tags the tested commit with the
version, stages the npm package (D251), and creates the GitHub release. The tag is
made only after every check has passed, so a failed check leaves no tag. Pushing a
version tag still works as before. The last job comments on the merged pull request
with the result: how to approve the staged package, or which run failed.

Only an admin can start a release this way. The job runs only for comments whose
author association is owner, member, or collaborator, and then asks GitHub's
permission API whether the commenter is an admin. Anyone else gets no release. The
comment workflow never checks out or runs pull request code, and it passes the
comment text through an environment variable, so a comment cannot inject commands.
Starting a run by hand needs write access, which GitHub enforces.

The comment workflow does not publish. npm has refused trusted publishing from runs
started by `issue_comment` since October 2026. The comment workflow starts a
separate `workflow_dispatch` run, which npm accepts, and that run's workflow file,
`release-cross-platform.yml`, is the one the npm trust link names.

Rejected alternatives:

- Publish from the comment workflow itself. Rejected because npm refuses tokens
  from `issue_comment` runs.
- Tag from the comment workflow and let the tag start the release. Rejected because
  a tag pushed with the workflow's own token starts no other workflow, and the tag
  would exist before the check passed.
- Create the release in the GitHub web interface. Rejected because the release job
  creates the release from the tested archives, and the two would conflict.
- Let any collaborator start a release. Rejected because a release publishes under
  the maintainer's name.
- A personal access token for the comment workflow. Rejected because it is a stored
  secret that would outlive its use, and the workflow's own token can start a
  `workflow_dispatch` run.

Verified before merging: shell syntax of every step, and each decision path with a
stand-in for the `gh` command. The workflows themselves run only on GitHub, so the
first real release is their first full test.

## D253. A tool skill left by an older `ath` is noted until it is replaced

When a folder's `athletic-standard` skill is older than this `ath`, every command
prints one line on standard error: "ath: the agent skill in .agents/skills is from
1.1.0, older than this ath (2.1.2); run ath skill install to update it". It prints on
`--json` runs too, because agents use them; the JSON on standard output does not
change. Bare `ath`, `ath --update`, `init`, and `skill` skip it, because they show or
replace the copy. Only the tool skill is checked: it is the only skill 0.x and 1.x
installed, and the first file an agent reads.

D164 removed D79's warning because `ath skill install` and `ath --update` replace an
older tool skill. That holds when this `ath` does the update. 2.1.2 is the first 2.x
version on npm, so everyone who has `ath` today has 0.x or 1.x, and their first
update runs through the old program:

- 0.1.0 to 0.4.1 have no `--update`. People run `npm install -g`, which copies no
  skill files.
- 0.4.3, 1.0.0, and 1.1.0 install the new version and then copy their own `skill/`
  folder into the project. The install has just removed that folder, so nothing is
  copied.

So the old tool skill stays, and before this decision 2.1.2 said nothing. Given only
"Where did I lose time in my HYROX race?", an agent in such a folder followed the
1.1.0 instructions. It never learned that `hyrox-race-splits` exists, and it did the
arithmetic itself. With the notice, the agent saw the line on its first command,
refreshed the skills, and used the calculation. The runs are in
[skill-prerequisites.md](appendix/skill-prerequisites.md), section 6.

This supersedes D164 only where D164 says there is no warning. D164's replacement
rules stand.

Rejected alternatives:

- Rely on D164's replacement. Rejected because no 0.x or 1.x updater runs it.
- Replace the old tool skill automatically when any command runs. Rejected for the
  reason D79 gave: a command that reads should not write into the project folder.
- Keep the old rule of skipping `--json` runs. Rejected because agents use `--json`.
  The note goes to standard error, so the JSON is unchanged.
- Add a line to the D250 section in every skill. Rejected because an agent in such a
  folder reads the old 1.x tool skill, not the new section.
- Upgrade guide only. Rejected because agents in old folders do not read it. The guide
  now says it as well.
