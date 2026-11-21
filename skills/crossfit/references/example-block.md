# Worked example: a 4-week block around three classes

This example runs procedures 1, 2, and 4 for an invented athlete. It ends with the
records saved. Every ID is invented. Labels: **Observed** (the record),
**Official** and **Research** (evidence), **Practice**, and **Hypothesis** (the
coach's judgment).

## The athlete

- Trains at a CrossFit gym. Classes on Monday, Wednesday, and Friday, 18:00 to
  19:00. Time zone `Europe/London`, which is UTC in November and December.
- Two years of CrossFit classes. Recreational running before that.
- Open gym on Tuesday 12:15 to 12:45 and Saturday 10:00 to 10:45. The gym has a rig,
  rings, bands, kettlebells, barbells, rowers, ropes, and boxes.
- In the athlete's words: "I want to get better at gymnastics, and I want to do
  Helen without a band."
- No pain, injury, or medical condition reported. Not planning to enter the Open
  at the prescribed (Rx) level this season.
- The gym posts each class workout the evening before.

## Step 1. What the record shows

Commands run on 15 November: `ath stats --json`, then
`ath records list --type <type> --limit 10000 --json` for `benchmark_definition`,
`benchmark_result`, `workout_session`, `athlete_report`, `goal`, and
`planned_session`. The limit matters: the list is oldest first and stops at 100 by
default.

**Observed.** Record IDs are shortened to their last four digits. The full form is
`01890f3e-e7d3-7cc0-88c8-00000000` followed by those digits.

| Fact | Record |
|---|---|
| 24 workouts in the 8 weeks to 15 November, all classes, about 60 minutes each | `workout_session` list; `ath stats` |
| Helen on 17 October: 12:48 with a 16 kg kettlebell and a thin band on the pull-ups | `0201`, revision 1, variant `modified` |
| Helen on 11 July: 13:35 with a 12 kg kettlebell and a medium band | `0204`, revision 1, variant `modified` |
| Class coach's test on 2 November: 3 strict pull-ups, 5 kipping pull-ups unbroken, no toes-to-bar, 10 knees-to-chest, 25 double-unders unbroken | athlete note `0202`, revision 1 |
| Schedule and open gym times | athlete note `0203`, revision 1 |
| Session-effort load for the 3 weeks to 15 November: 1,380, 1,500, and 1,440 AU (arbitrary units: rating times minutes; 0–10 scale, whole session). Mean 1,440 AU. | 9 reports `0211` to `0219`, each run through `training-load-effort` |
| Nightly RMSSD from a ring, 26 of the last 28 nights covered | `ath calculate coverage hrv_rmssd` |

The two Helen attempts are not comparable. The kettlebell and the band both
changed (see [benchmarks.md](benchmarks.md)).

## Step 2. Assessment

| Family | Level | Source |
|---|---|---|
| Squat, press, deadlift | 3. Intensity | Reported: prescribed loads in most classes |
| Kettlebell swing at 16 kg | 3. Intensity | Observed: Helen 17 October at 16 kg. Reported: swings unbroken. |
| Strict pull-up | 2. Consistency | Reported: 3 in one set |
| Kipping pull-up | 2. Consistency | Reported: 5 unbroken when fresh |
| Toes-to-bar | 1. Mechanics | Reported: knees-to-chest only |
| Double-unders | 2. Consistency | Reported: 25 unbroken |
| Running | 3. Intensity | Reported: about 2:00 per 400 m in Helen |

Priorities. **Hypothesis**: pull-up capacity limits both goals. Toes-to-bar shares
the hang and grip, so it comes second.

## Step 3. Evidence used

- **Official.** Develop a strict pull-up before kipping (L1 p. 219).
- **Official.** A single gymnastics day practises a skill not yet ready for timed
  work (L1 p. 74).
- **Official.** Mechanics, then consistency, then intensity (L1 p. 77).
- **Official.** Accessory work fits after the workout, one movement at a time; avoid
  the same region on back-to-back days (O'Connor, 2025).
- **Official.** Targeted blocks of 6 to 8 weeks with tests at each end (O'Connor,
  2023). This block is 4 weeks, so it can show the direction of change only.
- **Research.** No trials test pull-up progressions in CrossFit athletes. Climbers
  gained 2 to 5% in pull-up strength in 5 weeks of twice-weekly training
  (Vigouroux and Devise, 2024; 30 climbers). Very limited evidence.
- **Research.** Sets to failure slowed recovery for 24 to 48 hours (Morán-Navarro et
  al., 2017; 10 trained men).
- **Practice.** Short, frequent skill sets while fresh, stopped before form fails.

## Step 4. Hypotheses and assumptions

1. **Hypothesis.** The thin band is worth about one to two minutes in Helen. Helen
   as prescribed under 13:30 by 12 December is possible. Confidence is low. The
   check is on 5 December. 13:30 is 4:30 a round with no rest. Rounds done with
   2:00 rest run faster than continuous ones, so 4:30 a round there is not enough.
   Rounds of about 4:00 or less make the goal plausible. Rounds over 4:30 make it
   unlikely. The 4:00 mark is itself a judgment.
2. **Hypothesis.** Two short extra sessions fit on top of three classes. They add
   about 70 minutes a week. Tuesday is low-fatigue skill work. Saturday is the only
   new session with conditioning, which keeps to one new hard session at a time
   (see [programming.md](programming.md)). Expected weekly load is
   about 1,700 to 1,850 AU. It is wrong if class effort ratings rise or class
   workouts stretch well past their intended times.
3. **Hypothesis, with a known risk.** The athlete asked to practise unassisted
   kipping in class. Scaling Step 2 predicts trouble (see
   [scaling.md](scaling.md)). A fresh maximum of 5 suggests 2 to 3 a set under
   fatigue. So 45 class pull-ups would need 15 to 23 sets, which would likely run
   well past a 7 to 10 minute window. The load-first option is the thin band. We
   run the athlete's choice as a short trial with a stop rule (class note, rule 1).
   If two class workouts with pull-ups finish more than 2 minutes past their
   window, the thin band starts at the next weekly review. Confidence is low.
4. **Assumption.** Class content is unknown until the evening before. So the class
   rules go in a coaching note, not in planned sessions.

## Step 5. The block

| Week | Tuesday, about 30 minutes | Saturday, about 40 minutes |
|---|---|---|
| 1. 16–22 Nov: baseline and technique | Strict pull-up test. Strict 5 × 2. Slow lowering 3 × 2. Hanging knee raises 3 × 8. | Kipping every minute for 10 minutes, 3 a minute. 4 rounds of 400 m run and 15 swings, 2:00 rest. |
| 2. 23–29 Nov: build | Strict 6 × 2. Slow lowering 3 × 2. Kip swing to knees-to-chest 4 × 6. Hollow rocks 3 × 12. | Kipping every minute for 12 minutes, 4 a minute. 3 rounds of 400 m, 21 swings, 6 pull-ups, 1:30 rest. |
| 3. 30 Nov–6 Dec: closer to Helen | Strict 5 × 3. Toes-to-bar singles every minute for 8 minutes. Hollow hold. | Kipping every minute for 10 minutes, 4 a minute. Helen in 3 rounds with 2:00 rest, pull-ups in sets of 4. |
| 4. 7–13 Dec: lighter, then retest | Strict pull-up retest. Strict 3 × 2. Toes-to-bar 3 × 3. | Helen retest, as prescribed. |

Classes on Monday, Wednesday, and Friday follow the gym's programming and the rules
in the coaching note. Thursday and Sunday are rest days.

Why this shape:

- Strict work on Tuesday builds the base for kipping (Official: L1 p. 219).
- Kipping practice on Saturday is in small sets, every minute, while fresh
  (Practice).
- Saturday's conditioning rehearses Helen's run and swings, then adds pull-ups week
  by week (Practice).
- Week 4 cuts the extra sessions' volume by about half so the retests are fair
  (Practice, borrowed from taper research that cut volume by 41 to 60%; see
  [evidence.md](evidence.md)). Classes stay as programmed, except Friday 11
  December.
- No hard pulling on two days in a row (Official: O'Connor, 2025; Practice for the
  threshold of 30 pull-ups). After a pull-up-heavy Monday, Tuesday drops its strict
  sets. Before Saturday, a pull-up-heavy Friday class uses ring rows instead
  (class note, rule 5).
- Both strict pull-up tests start fresh: the Monday before each test uses ring rows
  in class (class note, rule 4). The test and the retest then have the same
  conditions.

## Step 6. What the athlete is told

> **Plan.** Four weeks. Two short extra sessions: Tuesday for strict pulling and
> hanging core, Saturday for the kip and Helen pacing. Retest strict pull-ups on 8
> December and Helen on 12 December.
>
> **Observed.** Your record shows Helen at 12:48 with a thin band on 17 October, and
> 3 strict and 5 kipping pull-ups on 2 November. You train three classes a week.
>
> **Evidence.** CrossFit advises a strict pull-up before kipping (official). No
> study tests pull-up progressions in CrossFit, so the sets and reps are coaching
> practice.
>
> **Hypothesis.** Helen without a band under 13:30 is possible by 12 December. I
> am not confident. December against October is not a like-for-like comparison,
> because the band changes the work. December will be your first prescribed Helen.
>
> **Next check.** On Sunday 22 November we review the week. I will look at how long
> class workouts with pull-ups took. You asked to do class pull-ups without a band.
> I expect that to make some workouts run long. If two of them finish more than 2
> minutes past their intended time, we switch to the thin band.
>
> **Needed from you.** Rate each session 0 to 10 about 30 minutes after it ends.

## Step 7. The records

Preview with `ath log --json`, show the athlete, then `ath log --yes` after
approval. The batch holds a custom test, two goals, the plan, the assessment, the
class rules, and eight planned sessions.

```json ath-record-example
[
  {
    "record_type": "benchmark_definition",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a001",
    "data": {
      "authorship": { "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" }, "authored_at": "2026-11-15T20:00:00.000Z", "evidence": [], "content_version": 1, "status": "active" },
      "benchmark_key": "strict-pull-ups-max",
      "benchmark_type": "custom",
      "score_type": "reps",
      "definition": "One set of strict pull-ups. Start from a dead hang with straight arms. Pull until the chin is over the bar. No kip, no band, no rest on the ground. Score is the number of full reps.",
      "tags": ["gymnastics", "test"]
    }
  },
  {
    "record_type": "goal",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a002",
    "data": {
      "authorship": { "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" }, "authored_at": "2026-11-15T20:00:00.000Z", "evidence": [ { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000201", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000204", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000202", "revision": 1 } ], "content_version": 1, "status": "active" },
      "title": "Helen as prescribed, no band, under 13:30",
      "description": "Women's prescribed Helen: 16 kg kettlebell, pull-ups with no band, kipping allowed. Baseline 17 October: 12:48 with a thin band, which is not comparable. The 13:30 target is a coaching hypothesis with low confidence.",
      "target_date": "2026-12-12",
      "target_benchmark_definition_id": "01890f3e-e7d3-7cc0-88c8-000000000103",
      "target_score": { "duration_s": 810 }
    }
  },
  {
    "record_type": "goal",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a003",
    "data": {
      "authorship": { "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" }, "authored_at": "2026-11-15T20:00:00.000Z", "evidence": [ { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000202", "revision": 1 } ], "content_version": 1, "status": "active" },
      "title": "Five strict pull-ups in one set",
      "description": "Baseline: 3, reported after the class coach's test on 2 November. The first test in this format is planned for 17 November. Retest on 8 December.",
      "target_date": "2026-12-08",
      "target_benchmark_definition_id": "01890f3e-e7d3-7cc0-88c8-00000000a001",
      "target_score": { "reps": 5 }
    }
  },
  {
    "record_type": "training_plan",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a004",
    "data": {
      "authorship": { "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" }, "authored_at": "2026-11-15T20:00:00.000Z", "evidence": [
        { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000201", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000202", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000203", "revision": 1 },
        { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000211", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000212", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000213", "revision": 1 },
        { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000214", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000215", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000216", "revision": 1 },
        { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000217", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000218", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000219", "revision": 1 }
      ], "content_version": 1, "status": "active" },
      "title": "Pull-up block around three classes",
      "objective": "Build strict and kipping pull-up capacity so Helen can be done as prescribed without a band, and start the toes-to-bar progression. Retest strict pull-ups on 8 December and Helen on 12 December under the baseline conditions. Pre-block weekly session-effort load: 1,440 AU (mean of 3 weeks). Four weeks can show the direction of change only.",
      "start_date": "2026-11-16",
      "end_date": "2026-12-13"
    }
  },
  {
    "record_type": "coaching_note",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a006",
    "data": {
      "authorship": { "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" }, "authored_at": "2026-11-15T20:00:00.000Z", "evidence": [ { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000201", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000204", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000202", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000203", "revision": 1 } ], "content_version": 1, "status": "active" },
      "title": "Assessment",
      "note": "Goals: Helen as prescribed with no band; better gymnastics, starting with pull-ups and toes-to-bar. Schedule: classes Monday, Wednesday, Friday 18:00-19:00; open gym Tuesday 12:15-12:45 and Saturday 10:00-10:45; full gym equipment. Observed: 24 class workouts in the 8 weeks to 15 November (count from ath stats run on 15 November, so the workout records are not listed as evidence); Helen 12:48 on 17 October with a 16 kg kettlebell and a thin band; Helen 13:35 on 11 July with a 12 kg kettlebell and a medium band, not comparable. Reported on 2 November: 3 strict pull-ups, 5 kipping, 10 knees-to-chest, no toes-to-bar, 25 double-unders. Competence (Level 1 order, mostly reported): squat, press, deadlift, swing, running at intensity; strict and kipping pull-ups and double-unders at consistency; toes-to-bar at mechanics. Limits: none reported; no clinician guidance needed. Priorities (hypothesis): pull-up capacity first, toes-to-bar second. Missing: class content until the evening before.",
      "applies_from": "2026-11-15"
    }
  },
  {
    "record_type": "coaching_note",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a005",
    "data": {
      "authorship": { "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" }, "authored_at": "2026-11-15T20:00:00.000Z", "evidence": [ { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000202", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000203", "revision": 1 } ], "content_version": 1, "status": "active" },
      "title": "Class rules for the pull-up block",
      "note": "1. Class pull-ups, a trial the athlete asked for: kipping, no band, sets of 3 to 5 from the first round, class reps kept. Known risk: scaling step 2 predicts these workouts may run long. Stop rule: if two class workouts with pull-ups finish more than 2 minutes past their intended window, use the thin band at the class reps, sets of 5 to 7, from the next weekly review to the end of the block. 2. If Monday's class has more than 30 pull-ups or any toes-to-bar, Tuesday skips the strict pull-up sets and keeps the rest. 3. No extra pulling after class. 4. Test days: on Monday 16 November and Monday 7 December, do class pull-ups as ring rows so the Tuesday strict pull-up tests start fresh. On Friday 11 December, before the Helen retest, use lighter class loads and ring rows instead of pull-ups. 5. If Friday's class has more than 30 pull-ups, do them as ring rows so Saturday's pull-up work starts fresh. If it has 1 to 30, Saturday's kipping practice is 8 minutes of 3 a minute. Rules 4 and 5 override rule 1 on those days. 6. Rate every session 0-10 about 30 minutes after it ends, for the whole session's minutes.",
      "applies_from": "2026-11-16",
      "applies_to": "2026-12-13"
    }
  },
  {
    "record_type": "planned_session",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a011",
    "data": {
      "authorship": { "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" }, "authored_at": "2026-11-15T20:00:00.000Z", "evidence": [ { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000202", "revision": 1 } ], "content_version": 1, "status": "active" },
      "training_plan_id": "01890f3e-e7d3-7cc0-88c8-00000000a004",
      "title": "Week 1 Tuesday: strict pulling and hanging core",
      "scheduled_date": "2026-11-17",
      "activity_type": "crossfit",
      "blocks": [
        { "block_type": "warm_up", "title": "Shoulders, grip, and hollow", "instructions": "Arm circles, band pull-aparts 2 x 15, scapular pull-ups 2 x 8, hollow hold 3 x 20 s.", "planned_duration_s": 420 },
        { "block_type": "test", "title": "Strict pull-ups, one set to the limit", "instructions": "Purpose: baseline for the strict pull-up goal. Fresh: Monday's class pull-ups were ring rows (class note, rule 4). After this warm-up. Dead hang, chin over the bar, no kip, no band. Stop when a rep fails. Log as a strict-pull-ups-max result." },
        { "block_type": "strength", "title": "Strict pull-ups 5 x 2", "instructions": "Purpose: strict base before more kipping. Rest 90 s. Stop each set one or two reps short of failure. If a rep fails, drop to 1 rep per set.", "planned_reps": 10, "planned_duration_s": 600 },
        { "block_type": "skill", "title": "Slow lowering 3 x 2", "instructions": "Step up to chin over bar, lower over 3 to 5 s. Keep the number low: the guide links high volumes of slow lowering with rhabdomyolysis in beginners (L1 p. 152), and this volume is new for the athlete.", "planned_reps": 6 },
        { "block_type": "accessory", "title": "Hanging knee raises 3 x 8", "instructions": "Controlled, no swing.", "planned_reps": 24 }
      ],
      "completion_status": "planned"
    }
  },
  {
    "record_type": "planned_session",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a012",
    "data": {
      "authorship": { "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" }, "authored_at": "2026-11-15T20:00:00.000Z", "evidence": [ { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000201", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000202", "revision": 1 } ], "content_version": 1, "status": "active" },
      "training_plan_id": "01890f3e-e7d3-7cc0-88c8-00000000a004",
      "title": "Week 1 Saturday: kipping practice and Helen pacing",
      "scheduled_date": "2026-11-21",
      "activity_type": "crossfit",
      "blocks": [
        { "block_type": "warm_up", "title": "Easy run, swings, kip swings", "instructions": "400 m easy, 2 x 10 light swings, kip swings 3 x 8.", "planned_duration_s": 600 },
        { "block_type": "skill", "title": "Kipping pull-ups every minute for 10 minutes, 3 a minute", "instructions": "Purpose: kip timing in small sets. Stop a set when the swing loses rhythm; do 2 the next minute. If Friday's class had 1 to 30 pull-ups, do 8 minutes of 3 a minute (class note, rule 5).", "planned_duration_s": 600, "planned_reps": 30 },
        { "block_type": "conditioning", "title": "4 rounds: 400 m run, 15 kettlebell swings, rest 2:00", "instructions": "Purpose: Helen run and swing pacing. Each 400 m in 1:55 to 2:05. Swings unbroken, to overhead. Breathing should limit, not grip. Log each round's time.", "planned_distance_m": 1600, "planned_reps": 60, "planned_weight_kg": 16 },
        { "block_type": "cool_down", "title": "Walk and stretch", "planned_duration_s": 300 }
      ],
      "completion_status": "planned"
    }
  },
  {
    "record_type": "planned_session",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a021",
    "data": {
      "authorship": { "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" }, "authored_at": "2026-11-15T20:00:00.000Z", "evidence": [ { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000202", "revision": 1 } ], "content_version": 1, "status": "active" },
      "training_plan_id": "01890f3e-e7d3-7cc0-88c8-00000000a004",
      "title": "Week 2 Tuesday: strict pulling and knees-to-chest",
      "scheduled_date": "2026-11-24",
      "activity_type": "crossfit",
      "blocks": [
        { "block_type": "warm_up", "title": "Shoulders, grip, and hollow", "instructions": "As week 1.", "planned_duration_s": 420 },
        { "block_type": "strength", "title": "Strict pull-ups 6 x 2", "instructions": "Rest 90 s. One or two reps short of failure. Skip this block if Monday's class had more than 30 pull-ups or any toes-to-bar (class note, rule 2).", "planned_reps": 12 },
        { "block_type": "skill", "title": "Slow lowering 3 x 2", "instructions": "Lower over 3 to 5 s.", "planned_reps": 6 },
        { "block_type": "skill", "title": "Kip swing to knees-to-chest 4 x 6", "instructions": "Purpose: start the toes-to-bar progression. Knees as high as control allows.", "planned_reps": 24 },
        { "block_type": "accessory", "title": "Hollow rocks 3 x 12", "planned_reps": 36 }
      ],
      "completion_status": "planned"
    }
  },
  {
    "record_type": "planned_session",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a022",
    "data": {
      "authorship": { "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" }, "authored_at": "2026-11-15T20:00:00.000Z", "evidence": [ { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000201", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000202", "revision": 1 } ], "content_version": 1, "status": "active" },
      "training_plan_id": "01890f3e-e7d3-7cc0-88c8-00000000a004",
      "title": "Week 2 Saturday: kipping sets and half Helen",
      "scheduled_date": "2026-11-28",
      "activity_type": "crossfit",
      "blocks": [
        { "block_type": "warm_up", "title": "Easy run, swings, kip swings", "instructions": "As week 1.", "planned_duration_s": 600 },
        { "block_type": "skill", "title": "Kipping pull-ups every minute for 12 minutes, 4 a minute", "instructions": "Stop a set when the swing loses rhythm; do 3 the next minute. If Friday's class had 1 to 30 pull-ups, do 8 minutes of 3 a minute (class note, rule 5).", "planned_duration_s": 720, "planned_reps": 48 },
        { "block_type": "conditioning", "title": "3 rounds: 400 m run, 21 kettlebell swings, 6 pull-ups, rest 1:30", "instructions": "Purpose: Helen with half the pull-ups. Kipping, no band, sets of 3. Log each round's time.", "planned_distance_m": 1200, "planned_reps": 81, "planned_weight_kg": 16 },
        { "block_type": "cool_down", "title": "Walk and stretch", "planned_duration_s": 300 }
      ],
      "completion_status": "planned"
    }
  },
  {
    "record_type": "planned_session",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a031",
    "data": {
      "authorship": { "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" }, "authored_at": "2026-11-15T20:00:00.000Z", "evidence": [ { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000202", "revision": 1 } ], "content_version": 1, "status": "active" },
      "training_plan_id": "01890f3e-e7d3-7cc0-88c8-00000000a004",
      "title": "Week 3 Tuesday: strict triples and toes-to-bar singles",
      "scheduled_date": "2026-12-01",
      "activity_type": "crossfit",
      "blocks": [
        { "block_type": "warm_up", "title": "Shoulders, grip, and hollow", "instructions": "As week 1.", "planned_duration_s": 420 },
        { "block_type": "strength", "title": "Strict pull-ups 5 x 3", "instructions": "Rest 2:00. Do 4 x 3 if week 2's sets of 2 were close to failure. Skip if Monday's class had more than 30 pull-ups.", "planned_reps": 15 },
        { "block_type": "skill", "title": "Toes-to-bar singles every minute for 8 minutes, 2 a minute", "instructions": "Attempt full toes-to-bar. If the toes do not reach, do knees-to-chest.", "planned_duration_s": 480, "planned_reps": 16 },
        { "block_type": "accessory", "title": "Hollow hold 3 x 30 s", "planned_duration_s": 90 }
      ],
      "completion_status": "planned"
    }
  },
  {
    "record_type": "planned_session",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a032",
    "data": {
      "authorship": { "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" }, "authored_at": "2026-11-15T20:00:00.000Z", "evidence": [ { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000201", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000202", "revision": 1 } ], "content_version": 1, "status": "active" },
      "training_plan_id": "01890f3e-e7d3-7cc0-88c8-00000000a004",
      "title": "Week 3 Saturday: kipping sets and Helen in pieces",
      "scheduled_date": "2026-12-05",
      "activity_type": "crossfit",
      "blocks": [
        { "block_type": "warm_up", "title": "Easy run, swings, kip swings", "instructions": "As week 1.", "planned_duration_s": 600 },
        { "block_type": "skill", "title": "Kipping pull-ups every minute for 10 minutes, 4 a minute", "instructions": "Stop a set when the swing loses rhythm. If Friday's class had 1 to 30 pull-ups, do 8 minutes of 3 a minute (class note, rule 5).", "planned_duration_s": 600, "planned_reps": 40 },
        { "block_type": "conditioning", "title": "Helen in pieces: 3 rounds of 400 m run, 21 swings, 12 pull-ups, rest 2:00", "instructions": "Purpose: rehearse the retest. Kipping, no band, sets of 4. Log each round's time. The goal pace is 4:30 a round with no rest, and rested rounds run faster. Rounds of about 4:00 or less make 13:30 plausible; rounds over 4:30 make it unlikely.", "planned_distance_m": 1200, "planned_reps": 99, "planned_weight_kg": 16 },
        { "block_type": "cool_down", "title": "Walk and stretch", "planned_duration_s": 300 }
      ],
      "completion_status": "planned"
    }
  },
  {
    "record_type": "planned_session",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a041",
    "data": {
      "authorship": { "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" }, "authored_at": "2026-11-15T20:00:00.000Z", "evidence": [ { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000202", "revision": 1 } ], "content_version": 1, "status": "active" },
      "training_plan_id": "01890f3e-e7d3-7cc0-88c8-00000000a004",
      "title": "Week 4 Tuesday: strict pull-up retest, light skill",
      "scheduled_date": "2026-12-08",
      "activity_type": "crossfit",
      "blocks": [
        { "block_type": "warm_up", "title": "Shoulders, grip, and hollow", "instructions": "The same warm-up as 17 November.", "planned_duration_s": 420 },
        { "block_type": "test", "title": "Strict pull-ups, one set to the limit (retest)", "instructions": "Same conditions as 17 November: fresh (Monday's class pull-ups were ring rows, class note rule 4), same warm-up, dead hang, chin over the bar. Log as a strict-pull-ups-max result." },
        { "block_type": "strength", "title": "Strict pull-ups 3 x 2", "instructions": "Light. Rest 2:00.", "planned_reps": 6 },
        { "block_type": "skill", "title": "Toes-to-bar attempts 3 x 3", "instructions": "Quality only. Stop if the hang or grip fades.", "planned_reps": 9 }
      ],
      "completion_status": "planned"
    }
  },
  {
    "record_type": "planned_session",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a042",
    "data": {
      "authorship": { "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" }, "authored_at": "2026-11-15T20:00:00.000Z", "evidence": [ { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000201", "revision": 1 } ], "content_version": 1, "status": "active" },
      "training_plan_id": "01890f3e-e7d3-7cc0-88c8-00000000a004",
      "title": "Week 4 Saturday: Helen retest as prescribed",
      "scheduled_date": "2026-12-12",
      "activity_type": "crossfit",
      "blocks": [
        { "block_type": "warm_up", "title": "Run, swings, kip swings, two practice pull-up sets", "instructions": "400 m easy, 2 x 10 swings building to 16 kg, kip swings, 2 sets of 3 pull-ups.", "planned_duration_s": 900 },
        { "block_type": "test", "title": "Helen as prescribed", "instructions": "3 rounds for time: 400 m run, 21 kettlebell swings at 16 kg, 12 pull-ups. No band; kipping allowed. Same loop and time of day as 17 October. Plan: pull-ups in sets of 4. Log with the crossfit-score extension, variant rx.", "planned_distance_m": 1200, "planned_reps": 99, "planned_weight_kg": 16 },
        { "block_type": "cool_down", "title": "Walk and stretch", "planned_duration_s": 600 }
      ],
      "completion_status": "planned"
    }
  }
]
```

## What happens next

- Each Sunday, run the weekly review in [weekly-review.md](weekly-review.md).
- After each test, log the result with the athlete's approval. The Helen retest
  uses `variant: rx` in the `org.athleticstandard/crossfit-score` extension (see
  [records.md](records.md)).
- The Helen retest is the first prescribed Helen in the record. It sets a new
  baseline. It is compared with the goal, not with the banded attempts.
- The week 2 review of this athlete is in [example-review.md](example-review.md).
  It tests hypothesis 3.
