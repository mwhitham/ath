# Athletic Standard 2.1.2 — calculation repairs, coaching skills, and session effort

Status: **approved direction; build in progress; not released**. Nothing in this
plan publishes a package or lists a skill on skills.sh.

Version 2.1.1 rebuilt the program without changing any calculation method. Its
release notes record two methods that were knowingly kept wrong
([deliberate changes](../v2.1.1/appendix/deliberate-changes.md)). An audit of
the shipped skills found more. Version 2.1.2 repairs every calculation skill,
adds the record fields those repairs need, and adds two complete coaching
skills: CrossFit and HYROX. The audit is in
[appendix/skill-audit.md](appendix/skill-audit.md).

The program, the npm package, the record package, and the database metadata use
2.1.2 (D152, D227). The SQL migration step becomes 6.

## 1. The result

There are three kinds of skill under `skills/` (D228):

| Kind | Skills | What it does |
|---|---|---|
| Tool | `athletic-standard` | Finds, reads, records, and checks athlete data. |
| Calculation | 23 repaired skills and `hyrox-race-splits` | Produces a reproducible result and states its limits. |
| Coaching | `crossfit`, `hyrox` | Assesses, plans, adapts, and reviews training. Its advice is saved as authored records. |

A calculation result names its method, its inputs, its comparison window, its
coverage, and what it left out. A coaching recommendation names the evidence it
used and keeps observed facts apart from coaching hypotheses.

The two-tier wall stays. Measured records need a source. Athlete reports cannot
claim one. Coaching advice is a third thing: an authored record with an author,
a time, and exact evidence revisions. It never becomes a measured fact or an
athlete report.

## 2. Version 2.1.2 and the record

### 2.1 Version and migration

`ATHLETIC_STANDARD_VERSION` becomes `2.1.2`. Records and packages written as
1.0.0, 2.0.0, 2.1.0, and 2.1.1 stay valid. A 2.0.0, 2.1.0, or 2.1.1 package
still imports. `schema/2.1.2/` is generated. `schema/2.1.1/` is not rewritten.

Migration `006-version-2-1-2.sql` sets the metadata to 2.1.2 and SQL schema 6.
It adds one nullable column, `athlete_reports.workout_session_id`, with a
foreign key to `workout_sessions` and a partial index. It changes no existing
row. Migrations 001–005 stay byte-identical (D165, D211). The migration keeps
the preview, verified backup, confirmation, and rollback rules of D124 and D211.
A 2.1.1 program refuses a 2.1.2 database.

### 2.2 Session effort (D238)

`athlete_report.report_type` gains `session_effort`. It is the athlete's rating
of a whole training session, such as a session RPE. A `session_effort` report
needs `rating` and `rating_scale`.

`athlete_report` gains optional `workout_session_id`. It names the workout the
report describes. Any report type may use it. SQLite enforces the link (D104).
Erasing the workout erases the reports that name it (D107). The link points from
the report to the measured record. The report still has no source.

For a session effort, `duration_seconds` is the time the rating covers. The rating
and the duration describe the same period. If the report has no duration, the
calculation uses the linked workout's start and end and says so.

### 2.3 Strength sets (D239)

`StrengthSet` gains three optional fields:

- `movement`: the exercise, such as `back squat`.
- `load_basis`: `total` or `per_implement`.
- `implement_count`: how many implements carried that load. It is required with
  `per_implement` and not allowed otherwise.

Two 22.5 kg dumbbells are `weight: 22.5`, `load_basis: per_implement`,
`implement_count: 2`. A barbell squat at 100 kg is `weight: 100`,
`load_basis: total`.

### 2.4 What stays out of the official fields

HYROX season, format, and division, and CrossFit variants, time caps, and
tiebreaks, are written in documented extensions on `benchmark_result`
(D118, D245):

- `org.athleticstandard/hyrox-race`
- `org.athleticstandard/crossfit-score`

The coaching skills define and validate these shapes. They become official fields
only after the D118 conditions are met. A zero-repetition score stays
unrepresentable as a benchmark score, because grading divides by the actual score
(D249).

The Zod compatibility schemas in `packages/npm/src/schema.ts` accept the new
fields so JavaScript callers can read 2.1.2 records (D209).

## 3. Calculation repairs

### 3.1 Rules for every calculation

Each calculation skill states its inputs, formula, units, time window,
exclusions, minimum data, interpretation, and known limits. The method in the
skill and the method in the program agree. A test runs every worked example in a
skill and compares the result with the number printed in the skill (D246).

A changed method gets a new operation name (`use:`). The old name keeps its old
arithmetic, so an edited file still gives the answer it gave before. Its result
adds a `method_notice` that names the replacement (D230).

`ath skill install` and `ath --update` replace an installed calculation file only
when its bytes match a file an earlier release shipped. The person never edited
that copy. An edited copy stays, and `ath skill` reports that the shipped method
changed (D231).

`ath apply` on a range calculation such as `baseline` refuses with the
`ath calculate` command to run instead (D243).

### 3.2 One-record calculations

| Skill | 2.1.1 problem | 2.1.2 method (operation) |
|---|---|---|
| `hrv-change` | Averaged later readings into the "earlier" mean. Ignored SDNN samples. Named its result `same-quantity-change`. | `earlier_window_change`: the day's mean against the mean of earlier daily means from the same source, quantity, record type, and basis, over 28 days with at least 7 days (D232, D233). |
| `resting-heart-rate-change` | Same as above. | `earlier_window_change` (D232). |
| `sleep-change` | Averaged later nights and naps. | `earlier_window_change` on main sleep periods; naps excluded (D234). |
| `heart-rate-recovery` | Looked for a sample after the last sample, so it always failed. | `drop_after_effort_end`: the drop from the end of effort to 60 s later, anchored at a `recovery` segment or the workout end (D235). |
| `sleep-regularity` | Compared two nights at the same clock instant, not 24 hours apart. Seven identical nights scored 0.5. | `sleep_regularity_index`: Phillips et al. arithmetic on sleep periods, −100 to 100 (D236). |
| `strength-volume` | Added pounds to kilograms. | `volume_load`: converts to kilograms, multiplies per-implement loads, refuses unknown units (D237). |
| `training-load-effort` | Multiplied any rating, including soreness, by any duration. | `session_effort_load`: a `session_effort` report on a 10-point or 6–20 scale, times the covered minutes (D238). |
| `training-load-heart-rate` | Used the male weighting for everyone and left out Banister's coefficient. Credited sample gaps in full. | `banister_trimp`: Banister's sex-specific weighting, gap-limited durations (D240). |
| `heart-rate-zones` | Credited long sample gaps in full; the last sample guessed its own length. | `time_in_zones`: gap-limited durations to the workout end, with uncovered time reported (D240). |
| `resting-heart-rate` | Required a heart rate that never changed for 60 s. | `lowest_rolling_mean`: the lowest 5-minute mean; a sleep session reads heart-rate samples from its source (D241). |
| `sleep-efficiency` | Accepted more sleep than time in bed. | `sleep_efficiency`: checks the denominator and reports a percentage (D242). |
| `sleep-stages` | Counted awake minutes in the stage total. | `sleep_stage_split`: asleep and awake minutes apart, with stage shares (D242). |
| `sleep-timing` | Reported only UTC start and midpoint. | `sleep_clock_times`: start, midpoint, and end in the athlete's time zone (D242). |
| `sleep-duration`, `sleep-interruptions` | Method correct; skill too thin. | `field`, unchanged. |

`hyrox-race-splits` is new: `hyrox_splits` divides a HYROX result into runs,
stations, Roxzone, and penalties without counting time twice (D245).

### 3.3 Range calculations (D244)

The receipt version becomes `ath-runtime-v2`. Results gain fields; existing
fields keep their meaning except where noted.

- `baseline` adds a summary of calendar-day means when a time zone is known, so a
  source that reads many times a day does not outweigh one that reads once.
- `trend` adds the days spanned and the slope's standard error.
- `coverage` adds the share of expected days that have a reading.
- `compare-sources` adds a comparison of days on which both sources read, when a
  time zone is known.
- `correlation` needs at least three paired days and says why it returned null.
  Two days always gave ±1.
- `workout-drift` reports the number of samples in each half.
- `summary` and `provider-period-summary` are unchanged.

## 4. Heart-rate variability end to end

RMSSD (`hrv_rmssd`) and SDNN (`hrv_sdnn`) never share a baseline, a change, a
trend, or a correlation. Both are supported by every calculation that reads a
measurement. Point measurements and measurement samples both count.

Within one statistic, a comparison also keeps apart:

- different sources;
- point measurements and intraday samples, because a nightly average and a spot
  reading are recorded differently;
- a provider value and a value calculated during import (`field_basis.value`).

The `athletic-standard` skill explains which device writes which statistic. Apple
writes SDNN samples and `ath` calculates RMSSD from Apple beats. WHOOP and Oura
write nightly RMSSD. Garmin's overnight value does not name its statistic and is not
labelled as either (D156).

## 5. Coaching skills

Both skills follow the Agent Skills layout: a focused `SKILL.md`, detailed
`references/`, and worked examples. Each states when to use it and when not to.
Each separates research findings, coaching practice, and individual hypotheses.
Each refers pain, injury, illness, and medical questions to a qualified person.

They are installed only when named: `ath skill install crossfit`,
`ath skill install hyrox`, or a skills.sh install that names the skill (D229). An
installed coaching skill is refreshed when a newer one ships.

### 5.1 CrossFit

| Capability | What good support looks like |
|---|---|
| Athlete assessment | Goals, experience, movement competence, equipment, days available, class programming, and limits. |
| Programming | Coherent weeks and blocks across strength, gymnastics, and conditioning, with the purpose of each session. |
| Scaling | Adjust load, volume, movement, and time to preserve the intended stimulus, with a reason and a route back. |
| Existing gym programming | Fit extra work around the real class demands; name sessions to reduce or replace. |
| Benchmark analysis | Compare attempts only when standards, loads, scaling, and scoring match closely enough. |
| Competition preparation | Use the named season's rules, divisions, time caps, and tiebreaks. |
| Weekly review | Compare planned and completed work, name plausible limiting factors, make a few justified changes. |

### 5.2 HYROX

| Capability | What good support looks like |
|---|---|
| Race preparation | A plan from the event date, current training, experience, equipment, and target division. |
| Training progression | Running and station capacity, running after stations, race combinations, and a taper. |
| Race analysis | Runs, stations, Roxzone, and penalties from `hyrox-race-splits`, with no time counted twice. |
| Pacing | A race budget with uncertainty, from comparable race or training evidence. |
| Stations | All eight stations, their standards by division, progressions, and practical substitutions. |
| Team formats | Doubles and Relay rules from the named season. Both Doubles athletes run every run. |
| Adaptive | Uses the athlete's confirmed classification. Never assigns eligibility. |
| Post-race review | Priorities, competing explanations, and training that would test them. |

Rules are read from the 2026/27 rulebooks. A saved analysis records the season and
rulebook that applied.

### 5.3 Saving coaching work

Coaching output is written as `training_plan`, `planned_session`, `coaching_note`,
`saved_training_review`, or `goal`. Each uses agent authorship and lists the
exact evidence revisions it relied on. The skills show the structured `ath log`
batch and wait for approval before writing.

## 6. The tool skill

`athletic-standard` keeps its rule that the record describes and does not
prescribe. The coaching skills are the only place advice is given (D228). The
tool skill gains:

- logging a session effort with its scale, duration, and workout link;
- logging strength sets with movement and load basis;
- the HRV rules in section 4;
- the extension shapes for HYROX and CrossFit results, by reference.

## 7. Verification

| Check | Requirement |
|---|---|
| Calculation correctness | Each method passes independently calculated examples, boundary cases, and missing-data cases. The future-reading, SDNN, recovery, regularity, mixed-unit, and command-routing failures have permanent regression tests. |
| Documentation matches behaviour | Every worked example in a calculation skill runs in a test (D246). |
| Skill files | Frontmatter, names, description length, links, and kinds are checked by a test. |
| Installation | `ath skill install` with and without names, update of unedited and edited copies, and a local skills.sh CLI install with telemetry disabled. |
| Agent behaviour | Fresh-session runs on synthetic databases, with the new skills, the 2.1.1 skills, and no skill. Tool calls, saved records, and claims are inspected (D247). |
| Coaching quality | Independent qualified CrossFit and HYROX coaches review representative outputs. An AI review does not count. |
| Repository checks | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and the npm checks. |

Automated checks never call a model service (spec 2.1.1 section 10). Agent
evaluations are run separately and their results are recorded in
[appendix/agent-evaluation.md](appendix/agent-evaluation.md).

## 8. Preparing for skills.sh without publishing

`skills.sh.json` groups the repository page into the tool skill, coaching, and
calculations. `docs/skills.md` lists every skill, how to install it, and what was
verified. `docs/coaching-skill-standard.md` is the bar a contributed running or
cycling skill must meet. Nothing is published, submitted, or installed through a
remote repository. Local install tests set `DISABLE_TELEMETRY=1` and
`DO_NOT_TRACK=1` and give the CLI no network route to skills.sh (D248).

## 9. Build order

1. Plan, decisions, and research in this folder.
2. Version 2.1.2, migration 6, and the model additions, with the Zod facade and
   generated files.
3. One-record calculation repairs with regression and example tests.
4. Range receipt version 2.
5. Installer: kinds, named coaching installs, unedited-copy updates.
6. Calculation skill rewrites with executed examples.
7. The tool skill update.
8. The CrossFit and HYROX skills and `hyrox-race-splits`.
9. skills.sh preparation and the contribution standard.
10. Agent evaluations, installation tests, and the full repository checks.

## 10. Primary references

The method research is in [appendix/calculation-methods.md](appendix/calculation-methods.md).
The CrossFit research is in [appendix/crossfit-research.md](appendix/crossfit-research.md).
The HYROX rules and research are in [appendix/hyrox-rules.md](appendix/hyrox-rules.md)
and [appendix/hyrox-research.md](appendix/hyrox-research.md). The Agent Skills
format is at <https://agentskills.io/specification>. The skills.sh repository
grouping file is described at <https://skills.sh/docs/customize>.
