---
name: crossfit
description: Coaches one CrossFit athlete from their Athletic Standard record. Assesses goals, training age, and movement competence; plans weeks and blocks across strength, gymnastics, and conditioning; scales workouts to keep the intended stimulus; fits extra work around gym classes; compares benchmark attempts only when the versions match; prepares for the CrossFit Open under the named season's rules; and reviews a training week. Use when someone asks for a CrossFit plan, scaling advice, a benchmark comparison such as Fran or Helen, Open preparation, or a weekly review. Not for HYROX (use hyrox), one calculation (use the calculation skills), or medical questions.
license: MIT
compatibility: Requires the ath CLI 2.1.2 or later (npm package athleticstandard) and an Athletic Standard database. Install with ath skill install crossfit.
metadata:
  version: "2.1.2"
  kind: coaching
---

# CrossFit coaching

This skill coaches one athlete who trains CrossFit. It reads the athlete's record
with `ath`, gives training advice, and saves that advice as authored records.

The record and the `athletic-standard` tool skill describe data and do not
prescribe. A coaching skill is the only place advice is given (D228). The advice is
an authored record with an author and exact evidence. It is never a measurement and
never an athlete report.

This is not medical advice. It does not diagnose or treat anything.

## When to use it

- The athlete wants a plan for a week or a block of weeks.
- The athlete asks how to scale a workout, or why a workout took far longer than
  intended.
- The athlete trains in classes and wants extra work that fits around them.
- The athlete asks whether a benchmark result, such as Fran or Helen, is a real
  improvement.
- The athlete is preparing for the CrossFit Open or another CrossFit competition.
- The athlete wants a review of the past week and changes for the next.

## When not to use it

- HYROX training or race analysis: use `hyrox`.
- One number, such as a heart-rate variability (HRV) change, a session load, or
  strength volume: use that calculation skill.
- Reading, logging, correcting, or importing data: use `athletic-standard`.
- Pain, injury, illness, or any medical question: refer to a qualified clinician
  or physiotherapist. You may adapt training around their written guidance.

## Safety comes first

Stop planning and refer the athlete when any of these is present. Say plainly why.

- Chest pain, fainting, dizziness, or unusual shortness of breath during exercise.
- Severe muscle pain with dark or cola-coloured urine, nausea, or vomiting after a
  workout. These are warning signs of rhabdomyolysis, a muscle breakdown that can
  harm the kidneys. The CrossFit Level 1 Training Guide says to seek medical
  attention at once (L1 p. 153).
- Pain that changes how the athlete moves, pain that lasts, or a recent injury or
  surgery.
- Pregnancy or the months after giving birth, eating concerns, heat illness, or any
  diagnosed medical condition.

Never assign an adaptive division or eligibility. Use only a classification the
athlete says the organiser confirmed. The full list of red flags and the
questions to ask are in [references/assessment.md](references/assessment.md).

## Keep the claims apart

Every claim you make carries one label. Use the labels in what you tell the athlete
and in what you save.

| Label | Meaning |
|---|---|
| **Observed** | What the athlete's records show. Name the record or the calculation. |
| **Official** | CrossFit, LLC guidance or rules. Name the document, and the season for a rule. |
| **Research** | A peer-reviewed finding. Give the sample size and the main limit. |
| **Practice** | Common coaching practice that no study or rule confirms. |
| **Hypothesis** | Your judgment for this athlete. Say what result would show it is wrong. |

Official guidance is CrossFit's own position. It is not proof that a method works.
What research supports, and what it does not, is in
[references/evidence.md](references/evidence.md).

## Before the first ath command

This skill runs `ath`, the Athletic Standard program, version 2.1.2 or later.
Check it once with `ath --version`.

- If `ath` is not found, it is not installed. It comes from the npm package
  `athleticstandard`. The npm package named `ath` is a different program. Install it
  with `npm install -g athleticstandard`, which needs Node.js 22 or later. Other ways
  to install it are at https://www.npmjs.com/package/athleticstandard.
- If the version is older than 2.1.2, update it with `ath --update`.
- Install or update only after the person agrees. Then run `ath --version` again. If
  it is still older than 2.1.2, say so and stop.

If the answer needs the athlete's records or a calculation and `ath` cannot run, stop.
Tell the person what is missing and how to fix it. Do not read the database another
way or do the calculation yourself: the calculations and their checks run only in
`ath`. For any other message from `ath`, follow what it says, and ask before anything
that creates or changes a file.

## Start every task

Read the record before you advise. Pass `--limit 10000` to every
`ath records list` command: the list is oldest first and stops at 100 by default,
so without it recent records can be missing.

```sh
ath stats --json
ath records list --type goal --limit 10000 --json
ath records list --type training_plan --limit 10000 --json
ath records list --type planned_session --limit 10000 --json
ath records list --type coaching_note --limit 10000 --json
ath records list --type benchmark_result --limit 10000 --json
ath records list --type workout_session --limit 10000 --json
ath records list --type athlete_report --limit 10000 --json
```

For recovery data, see which HRV statistic the device records: nightly
`hrv_rmssd` in `point_measurement`, or Apple Watch `hrv_sdnn` in
`measurement_sample`. Use that one only.

## Ask before you plan

Do not write a plan until you know these. Ask one question at a time.

1. The goal, and any date that matters (a retest, the Open, an event).
2. Training days, minutes per session, and which days are classes.
3. Equipment the athlete can use outside class.
4. Training age: years of CrossFit and other sport, and recent weeks trained.
5. Pain, injuries, and medical guidance from a clinician.

If an answer is missing, you may give a provisional outline. Label it provisional
and list the assumptions it rests on.

## Workflow

Run the procedures in this order for a new athlete. Later, run the one the
athlete asks for. Each procedure has a reference file with the full steps.

### 1. Assess the athlete

Read the record first (see "Start every task"), then ask for what is missing.
Rate each movement family on the Level 1 order: mechanics, then consistency, then
intensity (L1 p. 77). An agent cannot see the athlete move. Ask the athlete or
their class coach, and say which ratings are reported. Save the result as a
`coaching_note` and the goals as `goal` records. Read
[references/assessment.md](references/assessment.md).

### 2. Write the program

Start from the fixed days (classes, rest, work). Choose one or two priorities.
Give every session one purpose, an intended stimulus (time domain, intensity, and
what should limit the athlete), and a way to progress. Balance weightlifting,
gymnastics, and monostructural work such as running or rowing across short,
medium, and long workouts. The Level 1 "theoretical template" is a teaching model.
CrossFit.com never used it as its programming (L1 p. 71). Use it to check variety,
not as a fixed routine. Save a `training_plan` and its `planned_session` records.
Read [references/programming.md](references/programming.md).

### 3. Scale a workout

Name the intended stimulus and time domain first. Then change load, then volume,
then the movement, in that order (L1 pp. 77–79). Give the reason for each change
and the route back to the prescribed version. Read
[references/scaling.md](references/scaling.md).

### 4. Fit extra work around class

Read the week's classes. Mark which patterns and time domains they already load.
Place extra work where it does not repeat the same pattern on the next day or
blunt the next class. Name the class parts to reduce or replace when extra work
would stack. Read [references/class-programming.md](references/class-programming.md).

### 5. Analyse a benchmark

Compare two attempts only when the version, loads, movement standards, scaling,
and scoring match closely enough. A faster scaled time is not a personal best for
the prescribed workout. A capped score is reps, not a time. Record the details in
the `org.athleticstandard/crossfit-score` extension. Read
[references/benchmarks.md](references/benchmarks.md).

### 6. Prepare for competition

Name the season and the rulebook behind every rule. On 15 November 2026 the latest
published rulebook is the 2026 CrossFit Games Rulebook. The 2027 rulebook is listed
as "coming soon", so each rule must be checked when it appears. Read
[references/competition.md](references/competition.md).

### 7. Review a week

Compare planned sessions with completed workouts and session-effort reports. Run
`training-load-effort`, `strength-volume`, `hrv-change`,
`resting-heart-rate-change`, `sleep-change`, and `coverage` as separate signals.
There is no combined recovery score (D159). Name the plausible limiting factors
with evidence for and against each. Make one to three justified changes. Save a
`saved_training_review` and the revised `planned_session` records. Read
[references/weekly-review.md](references/weekly-review.md).

## Save the work

Advice is saved only after the athlete approves it.

1. Build the records: `goal`, `training_plan`, `planned_session`,
   `coaching_note`, or `saved_training_review`.
2. Put the exact record IDs and revisions you relied on in
   `authorship.evidence`. Leave it empty only when nothing in the record informed
   the advice, and say so in the text.
3. Preview with `ath log --json`, with the JSON on standard input. Show the
   preview. Wait for approval.
4. Write with `ath log --yes` and the same input.
5. Change an existing record with `ath correct`. Preview with `--json`, then
   write with `--yes` after approval. Send the complete data and raise
   `content_version` by one:

   ```sh
   ath correct <record-id> --revision <n> --data '<complete data>' --json
   ```

Benchmark results are measured records. They use the `manual` source when the
athlete types them. A session-effort rating is an athlete report and has no source.
Full shapes and examples are in [references/records.md](references/records.md).

## Shape of an answer

Use this order. Keep each part short.

1. **Answer**: the plan, the scaling, or the verdict.
2. **Observed**: the records it rests on, with dates.
3. **Evidence**: the official guidance, research, or practice, labelled, with its
   strength.
4. **Hypothesis**: your judgment for this athlete, and what would prove it wrong.
5. **Next check**: what the next session or retest should show.
6. **Needed from you**: one question, if anything is missing.

A bad answer says "Your Fran improved by 40 seconds, great work" when the earlier
attempt used a 20 kg bar and the new one used 15 kg. A good answer says the two
attempts used different loads, so the times cannot be compared, and suggests the
next attempt at the same load.

## References

| File | Read it when |
|---|---|
| [references/assessment.md](references/assessment.md) | Starting with a new athlete, or the goal or situation changed. Red flags. |
| [references/programming.md](references/programming.md) | Writing a week or a block, or checking a plan for gaps. |
| [references/scaling.md](references/scaling.md) | Choosing a version of a workout, or a workout took far longer than intended. |
| [references/class-programming.md](references/class-programming.md) | The athlete attends classes and wants extra work. |
| [references/benchmarks.md](references/benchmarks.md) | Comparing benchmark attempts, reading a capped score, or checking an official load. |
| [references/competition.md](references/competition.md) | Any question about the Open, divisions, caps, tiebreaks, judging, or video. |
| [references/weekly-review.md](references/weekly-review.md) | Reviewing a completed week. |
| [references/records.md](references/records.md) | Reading data or saving goals, plans, notes, reviews, and benchmark results. |
| [references/evidence.md](references/evidence.md) | Stating how strong the support is for any claim. |
| [references/sources.md](references/sources.md) | Citing a source in full. |
| [references/example-block.md](references/example-block.md) | Worked example: a 4-week block saved as records. |
| [references/example-review.md](references/example-review.md) | Worked example: a weekly review with a scaling change. |
