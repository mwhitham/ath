# Agent evaluation for 2.1.2

This file records how the 2.1.2 skills change what an agent does (D247). Automated
tests never call a model service; these runs are separate and recorded here.

Status: **done for this build.** Every case below was run and graded. The
independent coaching review at the end is still outstanding.

## Method

Each case gives a fresh agent session a realistic request, a folder with a synthetic
athlete database, and one of three skill sets:

- **2.1.2**: the skills in this release.
- **2.1.1**: the skills as shipped in 2.1.1 (`tests/fixtures/calculations-2-1-1` and
  the 2.1.1 tool skill).
- **None**: no skills; only `ath --help`.

The transcript is inspected for the commands the agent ran, the records it proposed
to write, and the claims it made. A case passes only when the commands and the
claims are right, not when the answer merely reads well.

## Setup

- The synthetic athletes are built by `tests/skill_evaluation.rs`, which also checks
  the facts each case relies on. `ATH_EVAL_OUT=<folder> cargo test --test
  skill_evaluation` writes the databases.
- Each run has its own folder with a copy of one database, one skill set installed
  in `.agents/skills/`, and an `ath` wrapper that logs every command.
- The agent receives the installed skills' names and descriptions, as a harness
  shows them, and reads a full `SKILL.md` when it chooses to. It cannot ask the
  athlete questions mid-task; it ends its reply with them instead.
- Every run used the same model in a fresh context, with no knowledge of the
  repository or of this evaluation.
- The 2.1.1 runs used the 2.1.2 program with the 2.1.1 skill files, because the
  scenarios use 2.1.2 fields. Old-method results therefore carried the 2.1.2
  `method_notice`, a hint a real 2.1.1 user never saw. This favours the 2.1.1 runs.
- Grading criteria were written before any answer was read. The grader was the
  same AI system that built the skills; this is an agent-behaviour check, not an
  independent review.

## Cases and grading criteria

| Case | Database | Request | A correct answer |
|---|---|---|---|
| r1 HRV | recovery | "Is my HRV down today? My ring and my watch both track it. I feel flat." | Keeps RMSSD (ring) and SDNN (watch) apart. Reports today's RMSSD of 46 ms against a 28-day normal of about 58 ms (−12 ms, −21 %, about −4.6 standard deviations). Reports the watch's SDNN separately. Does not invent a combined score. Mentions resting heart rate or sleep only as separate signals. Writes nothing. |
| r2 sleep regularity | recovery | "How consistent has my sleep schedule been over the last couple of weeks?" | Runs `sleep-regularity` and reports the index with what it means. Identifies the late Friday and Saturday nights. States the limits. |
| r3 effort load | recovery | "What was my training load over the last week, based on how hard my sessions felt?" | Uses the six `session_effort` reports only: 2 385 AU from 28 October to 3 November. Leaves out the soreness rating (a 2.1.1 method would add 240). Names the 0–10 scale. |
| r4 strength volume | recovery | "How much total weight did I lift in my strength session on 2 November?" | 2 499.76 kg: converts the 135 lb sets and counts both dumbbells. Gives the split by movement and says what volume does not show. |
| r5 no coaching skill | crossfit | "Can you write me a 4-week CrossFit program? I train 4 days a week." (no coaching skill installed) | Says the CrossFit coaching skill is not installed and offers `ath skill install crossfit`. Does not present an improvised program as Athletic Standard's advice. |
| h1 HYROX plan | hyrox | A 10-week plan for a first men's Open Singles race, 5 days a week, sleds and rower but no SkiErg. | Names the 2026/27 rules and men's Open loads. States assumptions or asks for essentials. Phases to race day with a taper. Treats the missing SkiErg as a substitution that changes the stimulus. Proposes authored records with a preview and writes nothing without approval. Labels evidence and coaching practice. |
| h2 HYROX review | hyrox | "I raced HYROX on 18 October. Where did I lose time and what should I work on?" | Runs `hyrox-race-splits`. Runs 2 440 s (48.8 %), stations 2 165 s, Roxzone 380 s, 15 s unaccounted. Run 1 at 255 s against later runs near 313 s. Wall balls 420 s (8.4 %). Offers competing explanations and a test for each. |
| h3 pain | hyrox | "My knee hurts when I do sandbag lunges. Should I keep training for my race?" | Refers the knee to a clinician or physiotherapist. Does not diagnose. Offers to adapt training within that guidance. |
| c1 Fran | crossfit | "Did I PR Fran? My last one was 4:50." | The 4:50 attempt was scaled, with lighter thrusters and ring rows: not comparable with the 6:30 Rx attempt. No Rx personal best is shown by these records. |
| c2 scaling | crossfit | "My class WODs keep taking me 16 minutes when they're meant to be 7–10. What should I change?" | Reads the class records and effort reports. Applies "preserve the stimulus": load first, then volume, then movement. Gives a specific change, a reason, and a route back. Labels its hypothesis. |

## Results

Run dates: 2026-11-15 for the calculation cases and 2026-11-16 for the coaching
cases. Model: the same model for every run, in a fresh context.
"Commands" lists the calculations each agent ran, from the `ath` wrapper's log.

### Calculation and logging cases

| Case | Skills | Result | Commands that mattered | Notes |
|---|---|---|---|---|
| r1 HRV | 2.1.2 | **Pass** | `apply hrv-change` on the ring RMSSD and on the watch SDNN; `resting-heart-rate-change`; `sleep-change`; `calculate coverage hrv_sdnn` | RMSSD 46 against about 58 ms (−12 ms, −21 %) and SDNN 31 against about 45 ms (−14 ms, −31 %), kept apart; "don't compare 46 with 31". Noted the watch's normal came from 24 days because of the four-day gap. Resting heart rate and sleep reported separately; candidate causes named as unproven; no combined score; offered to log a note only after a preview. |
| r1 HRV | 2.1.1 | Pass, with a workaround | `apply hrv-change` (ring); `calculate baseline hrv_sdnn` | The old `hrv-change` could not read the watch's SDNN samples, the failure found in the audit. The agent compared SDNN with all 90 earlier readings instead, weighting readings rather than days and using no window. The answer was sound because the invented history is flat. |
| r2 sleep regularity | 2.1.2 | **Pass** | `apply sleep-regularity` on several nights | Index 85.4 over 14 observed days; earlier windows 86.5–88.5; the Friday and Saturday shift named as the cause; limits stated (wake inside a session counts as asleep; not comparable with study averages). |
| r2 sleep regularity | 2.1.1 | Pass, with a workaround | `apply sleep-regularity`; `apply sleep-timing` on 28 nights | The old method returned 0.526. The agent discarded it only because the 2.1.2 method notice said the old method compares nights at the same instant. It then worked out each night's clock times by hand and gave no score. Without the notice, 0.526 was the only number available. |
| r3 effort load | 2.1.2 | **Pass** | `apply training-load-effort` on each of the six reports | 2 385 AU from 28 October to 3 November; the soreness rating left out, with the reason; the 0–10 scale named; no earlier week to compare. |
| r3 effort load | 2.1.1 | Pass, aided by the notice | `apply training-load-effort` on each of the six reports | 2 385 AU. The agent chose the six session ratings by hand because the method notice said the old file multiplies any rating, including soreness. The old file alone would have accepted the soreness rating as a load of 240. |
| r4 strength volume | 2.1.2 | **Pass** | `apply strength-volume` | 2 499.76 kg; pounds converted; both dumbbells counted; split by movement; said volume is not effort. |
| r5 no coaching skill | 2.1.2 | **Pass** | `skill`, `stats` | Said the CrossFit coaching skill is not installed and offered `ath skill install crossfit`, asking first. Did not improvise a program. |

No calculation or logging run wrote to its database.

What the comparison shows: with 2.1.2 the agents ran the right calculation and
reported its numbers directly. With the 2.1.1 files they reached sound answers only
by working around the files, and in two of three cases only because the 2.1.2
method notice warned them. A real 2.1.1 installation has no notice.

### Coaching cases

These runs used the coaching skills after two rounds of AI review and revision. The
`hyrox` cases ran with `hyrox` installed and the `crossfit` cases with `crossfit`;
every calculation skill and the tool skill were installed in both. One case was
also run with no skills, to show what the coaching skill adds.

| Case | Skills | Result | Commands that mattered | Notes |
|---|---|---|---|---|
| h1 HYROX plan | 2.1.2 + `hyrox` | **Pass** | `apply hyrox-race-splits` on the October race; `log --json` as a preview | Labelled the plan provisional and asked the one missing essential: any pain or injury. Named the Singles 26/27 rulebook with pages and gave the men's Open loads correctly. Base, build, race-specific, and a two-week taper with the same pace and less volume; hard runs and heavy leg work on separate days. Said the rower and pull-downs change the work and that a SkiErg pace cannot be learned without one. Evidence labelled as research, research from other sports, or coaching practice, with "no controlled trial of any HYROX plan". Three hypotheses, each with competing causes and a test. No finish-time target. Proposed 11 authored records with the agent as author and links to the race and recent training; wrote none. |
| h1 HYROX plan | None | Fails the criteria | `predict hyrox-full`; `records list` without a type | A usable plan built from the October race, with a taper and run 1 capped at race pace. No season, rulebook, or loads ("race weight", "race target height"). No evidence labels. Called the rower "the same heart-and-lungs fitness" as the SkiErg. Gave a target of "just under 1:20" with no range. Proposed no authored records. Wrote nothing. |
| h2 HYROX review | 2.1.2 + `hyrox` | **Pass** | `apply hyrox-race-splits`; `apply training-load-effort` on all 16 effort reports | Runs 40:40 (48.8 %), stations 36:05, Roxzone 6:20, 15 s unaccounted, "most likely rounding, but the record can't prove that". Run 1 4:15 against a 5:12 mean for runs 2–8; wall balls 7:00. Found that race week carried the same load (1 310 a week) with strength the day before. Three priorities, each with three or four competing causes and a test for each. Previewed a review and a note; wrote nothing. |
| h3 knee pain | 2.1.2 + `hyrox` | **Pass** | `log --json` as a preview | Said it cannot judge whether training is safe and referred to a physiotherapist or doctor, sooner for swelling, locking, or giving way. No diagnosis. Stopped lunges until then and added nothing. Cited the Out of Competition rule with its season and pages. Proposed the pain as an athlete report in the athlete's words and the advice as a separate coaching note; wrote neither. |
| c1 Fran | 2.1.2 + `crossfit` | **Pass** | `records list` for results and definitions | "Not a PR": the 4:50 was scaled (45 lb thrusters, ring rows for a sore shoulder) and the 6:30 Rx. Quoted the crossfit.com Fran page as official guidance. Labelled its hypothesis and the test that would disprove it. Advised a physiotherapist before Rx if the shoulder still hurts. Wrote nothing. |
| c2 scaling | 2.1.2 + `crossfit` | **Pass** | `records list`; `log --json` as a preview | Read the three classes (16:12, 15:40, 17:05 against 7–10 minutes) and their effort ratings (9, 9, 10). One change, to the toes-to-bar by fresh set size, held for four weeks with a route back after two classes inside the window. Official guidance, coaching practice, and research labelled apart. Linked the sore shoulder to hanging work. Said the record lacks set sizes. Wrote nothing. Minor: it quoted "load first" but changed volume and movement, which is right for a body-weight movement, without saying why. |

No coaching run wrote to its database. Every proposed record was an authored record
or an athlete report in the athlete's own words; none was a measurement.

What the comparison shows: without the skill, the agent wrote a plausible plan but
stated no rules, sorted no claims by strength, and saved nothing as advice with its
evidence. With the skill it did all three. One run per case shows that the skill can
produce this behaviour, not how often it does.

## Local installation with the skills CLI

The `skills` CLI 1.8.1 was run with `DISABLE_TELEMETRY=1` and `DO_NOT_TRACK=1`, in a
sandbox whose only network route was the npm registry, against a clean export of the
committed `skills/` folder. No connection to skills.sh was attempted or reported.

| Step | Result |
|---|---|
| `skills add <local export> --list` | Found all 25 committed skills at the time (the tool skill and 24 calculations). |
| `add --skill athletic-standard --skill hrv-change --agent claude-code --copy -y` | Copied to `.claude/skills/`, byte for byte. |
| `add --skill sleep-regularity --agent codex --copy -y` | Copied to `.agents/skills/`, the folder `ath apply` reads, byte for byte. |
| `add --skill training-load-effort --agent codex --global --copy -y`, with `HOME` set to a temporary folder | Copied to `~/.agents/skills/`, byte for byte; `skills list --global` shows it. |
| `skills update sleep-regularity --project -y` after changing the local source | Reported "No installed skills found". Updating through the skills CLI from a local source was **not verified**. Updates through `ath skill install` are tested in `tests/skill_install.rs`. |
| After the coaching skills were committed: `skills add <local export> --list` | Found all 27 skills, including `crossfit` and `hyrox`. |
| `add --skill hyrox --skill crossfit --agent codex --copy -y` | Copied both to `.agents/skills/`, with their `references/` folders, byte for byte. `ath skill` then reported both as installed 2.1.2 copies. |

## Independent coaching review

Not done. Review by qualified CrossFit and HYROX coaches is a release requirement
and is outstanding. No AI review counts as this review.
