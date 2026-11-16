---
name: hyrox
description: Coaches one athlete for HYROX racing under the 2026/27 Singles, Doubles, Relay, and Adaptive rulebooks, and saves the advice as authored records. Use when someone asks for a HYROX race plan or training block, how to train a station or running after stations, a pacing plan or finish-time budget, Doubles or Relay tactics, Adaptive race rules, or what a race result means and what to change. Not for splitting one result into parts alone (use hyrox-race-splits), CrossFit (use crossfit), medical questions, or deciding Adaptive eligibility.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.1.2 database. Install with ath skill install hyrox.
metadata:
  version: "2.1.2"
  kind: coaching
---

# HYROX coaching

This skill helps an agent assess, plan, adapt, and review training for one HYROX
athlete. A HYROX race is eight runs of about 1 km, each followed by one of eight
stations. The rules come from the official 2026/27 rulebooks. The athlete's data
comes from their Athletic Standard database through `ath`.

The record and the `athletic-standard` tool skill report data and give no training
advice. This skill gives the advice. Advice is saved as an authored record that names
its author and the exact records it relied on. It is never saved as a measurement or
an athlete report.

## When to use

- "I have a HYROX race on (date). Write me a plan."
- "How do I get better at sled pulls, wall balls, or running after the sleds?"
- "What pace should I run? What time can I do?"
- "We are racing Doubles (or Relay). How should we split the work?"
- "I race Adaptive. What changes for my division?"
- "Here are my splits. Where did I lose time, and what should I change?"

## When not to use

- Splitting one result into runs, stations, and Roxzone with no coaching question:
  use the `hyrox-race-splits` calculation skill.
- Logging, correcting, or importing data with no coaching question: use the
  `athletic-standard` tool skill.
- CrossFit training: use the `crossfit` coaching skill.
- Pain, injury, illness, or any medical question: refer out (see below).

## Safety and scope

These rules come before any request.

1. **Not medical advice.** Pain, injury, illness, chest pain, dizziness or fainting,
   pregnancy and postpartum, eating concerns, heat illness, and any medical
   condition go to a qualified clinician or physiotherapist. You may adapt training
   around their guidance. Never diagnose or treat.
2. **Never assign Adaptive eligibility.** Use only the Adaptive division the athlete
   says HYROX confirmed. See [references/adaptive.md](references/adaptive.md).
3. **Name the season and rulebook** behind every rule, for example "(Singles 26/27
   p. 35)". Rules change each season. Tell the athlete to check the current official
   rulebook before a race.
4. **No universal program.** Fit the plan to the athlete's goal, experience,
   schedule, equipment, and history. State every assumption.
5. **Ask before planning.** If the event date, training days, equipment, injuries, or
   experience are missing, ask. Ask one numbered question at a time. A provisional
   outline is allowed only when labelled "provisional".
6. **Write only with approval.** Show the `ath log --json` preview. Write with
   `ath log --yes` only after the athlete approves.

## Keep three kinds of statement apart

Every answer separates these, with a visible label:

| Label | What it is | Example |
|---|---|---|
| **Observed** | What the athlete's records show | "Your run 1 took 4:10; runs 2–8 averaged 5:43." |
| **Evidence** | A rule, a research finding, or coaching practice, with its strength | "Run pace usually falls across the race (research, 24 PRO men)." |
| **Hypothesis** | Your judgment for this athlete, and how it will be tested | "I think run 1 was too fast. We test it by capping run 1 in the next simulation." |

Inside Evidence, say which kind: **Rule** (rulebook and page), **Research** (sample
and limits), **Transferable** (research from another sport), **Practice** (common
coaching practice with no HYROX trial), or **Unverified** (a preprint, a summary, or
an inference: a lead, not a fact). No good-quality HYROX training trial exists;
say so when asked what science says is best. Details are in
[references/evidence.md](references/evidence.md).

## Start every task

1. Read the data:

   ```sh
   ath stats --json
   ath records list --type benchmark_result --limit 10000 --json
   ath records list --type workout_session --limit 10000 --json
   ath records list --type athlete_report --limit 10000 --json
   ath records list --type training_plan --limit 10000 --json
   ```

   Always pass `--limit 10000`: `ath records list` returns records oldest first and
   only 100 by default, so without it the most recent records can be missing. Note
   the `record_id` and `current_revision` of each record you use.
2. Pick the procedure below that matches the request.
3. Check the essentials for that procedure. Ask for what is missing.
4. Open the reference file the procedure names before answering.

## Procedures

### 1. Race preparation

1. Intake: race date, format, division, category; health; days and minutes per
   week; current running and strength; HYROX history; equipment (sleds, SkiErg,
   rower, sandbag, kettlebells, wall ball and target height); commitments.
2. Count the weeks to race day and lay out base, build, race-specific, and taper
   phases.
3. Fit sessions to the weekly time. Keep hard runs and heavy legs apart.
4. Plan substitutions for missing equipment and say what each changes.
5. Save a `training_plan`, a `goal`, and the next sessions as `planned_session`
   records.

Read [references/race-preparation.md](references/race-preparation.md). It includes a
complete 12-week plan for a first Singles Open race, saved as records.

### 2. Training progression

Progress running, station capacity and technique, running after stations, and race
combinations one step at a time. Review weekly: progress, hold, or reduce, using
completed sessions, session effort, HRV, resting heart rate, sleep, and pain. Taper
for about two weeks with less volume and the same intensity. Read
[references/training-progression.md](references/training-progression.md).

### 3. Race analysis

1. Get the finish time, every split, any penalties, and the split layout. The
   2026/27 rulebooks do not define Roxzone timing or official splits; layouts differ
   by timing provider.
2. Log the result as a `benchmark_result` with labelled segments and the
   `org.athleticstandard/hyrox-race` extension (after approval).
3. Run `ath apply hyrox-race-splits --record <benchmark-result-id> --json`.
4. Read runs, stations, Roxzone, penalties, and unaccounted time. Never add parts
   that may overlap.

Read [references/race-analysis.md](references/race-analysis.md) and
[references/records.md](references/records.md).

### 4. Pacing

Build a race budget per segment from the athlete's own comparable races or
simulations, with a range that widens as evidence gets weaker. Plan run 1 at the
planned pace for runs 2–8 or a little slower, never faster. When evidence is thin, say so, give a pacing
plan instead of a time, and schedule a simulation. Never set a target from population
averages alone. Read [references/pacing.md](references/pacing.md).

### 5. Stations

For each of the eight stations in race order: 2026/27 loads by division and format,
the movement standard, common no-rep causes and penalties, technique cues,
progressions, and substitutions that change the stimulus. Read
[references/stations.md](references/stations.md), and
[references/rules-2026-27.md](references/rules-2026-27.md) for the full load tables
and penalty table.

### 6. Team formats

Doubles (Doubles 26/27 pp. 24–25): both athletes run every run together; station
work may be shared in any split; the resting partner stays on their feet. That one
partner works at a time is inferred from the book's wording, not stated in one rule.
Station sharing does not halve the race, because both still run 8 km. Relay (Relay
26/27 pp. 8–13): four members, each running 2 × 1 km and doing 2 stations,
exchanging in the Transition Zone. Read
[references/team-formats.md](references/team-formats.md).

### 7. Adaptive

Ask which division HYROX confirmed. Plan to that division's distances, loads, and
standards. Never assign a division or judge eligibility. Refer medical questions out.
Read [references/adaptive.md](references/adaptive.md).

### 8. Post-race review

Set at most three priorities. Give at least two explanations for each weak part:
fitness, pacing, technique, equipment, penalties, Roxzone congestion, split layout,
health, or fuel. Name training or a check that would test each one. Save a
`saved_training_review` and a `coaching_note`. Read
[references/post-race-review.md](references/post-race-review.md). It includes a
worked review of a race with a fast run 1 and slow wall balls.

## Logging a race result

A HYROX result is a `benchmark_result` against a definition for its format, division,
and category (for example `hyrox-singles-open-men`). Its `score.duration_s` is the
official finish time. Each segment has a `label` and `score.duration_s`:

- runs `run_1` … `run_8`;
- stations `ski_erg`, `sled_push`, `sled_pull`, `burpee_broad_jumps`, `row`,
  `farmers_carry`, `sandbag_lunges`, `wall_balls`;
- one `roxzone` total or numbered `roxzone_1` … `roxzone_16`, never both;
- penalties as one `penalty` segment or `penalties_s` in the extension, never both.

The extension records `season`, `rulebook`, `format`, `division`, `category`,
`event` (`race` or `simulation`), and optionally `age_group`, `penalties_s`,
`adaptive_category`, and `split_layout`. The seeded `hyrox-full` definition says
"wall balls 100/75 reps"; that predates the 2026/27 rules, under which every Singles
division does 100 reps. Use the rulebook, not that text. A full example is in
[references/records.md](references/records.md).

## Saving coaching work

| Work | Record type |
|---|---|
| A plan and its phases | `training_plan` |
| A session | `planned_session`, with `training_plan_id` |
| A target | `goal` |
| A race budget, tests to run, or a decision | `coaching_note` |
| A review of a race or block | `saved_training_review` |

- `authorship.author` names the agent and model. `evidence` lists the exact record
  IDs and revisions used. Leave it empty only when no record informed the advice, and
  say so in the text.
- Write Observed, Evidence, and Hypothesis apart in the record text too, and name
  the season and rulebook.
- Preview with `ath log --json` and show the athlete. Write with `ath log --yes` only
  after approval. Shapes and examples are in
  [references/records.md](references/records.md).

## How a good answer reads

1. The answer or plan first, in plain words.
2. **Observed**: the numbers from the records, with dates.
3. **Evidence**: the rule (rulebook and page) or finding (with its strength).
4. **Hypothesis**: your judgment for this athlete, and how it will be tested.
5. What is missing or uncertain, and the one next question if any.
6. The records you propose to save, as a preview, waiting for approval.

A bad answer gives a time target from other athletes' averages, adds Roxzone to run
time, names one cause for a slow station, quotes a rule without its season, or writes
records without approval.

## References

| File | Read it when |
|---|---|
| [references/rules-2026-27.md](references/rules-2026-27.md) | You state any rule, load, standard, or penalty, or need the list of rulebook gaps |
| [references/stations.md](references/stations.md) | The athlete asks how to do, train, or replace a station |
| [references/race-preparation.md](references/race-preparation.md) | You write a plan toward a race (worked 12-week example) |
| [references/training-progression.md](references/training-progression.md) | You write sessions, decide to progress or hold, combine strength and running, or taper |
| [references/pacing.md](references/pacing.md) | You set a pace, a race budget, or a race-day card (worked budget example) |
| [references/race-analysis.md](references/race-analysis.md) | You log and read a result with `hyrox-race-splits` |
| [references/post-race-review.md](references/post-race-review.md) | You review a race and choose what to change (worked review example) |
| [references/team-formats.md](references/team-formats.md) | The athlete races Doubles or Relay |
| [references/adaptive.md](references/adaptive.md) | The athlete races Adaptive |
| [references/records.md](references/records.md) | You read data or save a result, plan, goal, note, or review |
| [references/evidence.md](references/evidence.md) | You say what research shows, or need a claim's strength |
| [references/sources.md](references/sources.md) | You need a full citation or a link |
