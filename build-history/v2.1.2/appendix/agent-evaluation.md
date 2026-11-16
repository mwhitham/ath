# Agent evaluation for 2.1.2

This file records how the 2.1.2 skills change what an agent does (D247). Automated
tests never call a model service; these runs are separate and recorded here.

Status: **in progress**. Results are added as each run completes.

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

Pending.

## Local installation with the skills CLI

Pending.

## Independent coaching review

Not done. Review by qualified CrossFit and HYROX coaches is a release requirement
and is outstanding. No AI review counts as this review.
