# Worked example: a weekly review with a scaling change

This is the week 2 review for the athlete in [example-block.md](example-block.md).
Class workouts meant to last 7 to 10 minutes took about 16 minutes, three times in
two weeks. The review checks safety, confirms the likely cause, applies the stop
rule the plan set in advance, and says what the next attempt should show.

Every ID is invented. Record IDs are shortened to their last four characters; the
full form is `01890f3e-e7d3-7cc0-88c8-00000000` followed by those characters.

Period: Monday 23 November to Sunday 29 November 2026, `Europe/London`.

## Step 1. Planned against completed

**Observed.** From `planned_session`, `workout_session`, and `athlete_report`
records (each listed with `--limit 10000`), and the class rules note `a005`.

| Date | Planned | Done | Against intent |
|---|---|---|---|
| Mon 23 | Class | Class `b123`: back squat 5 × 3 at 60 kg, then 21-15-9 wall-ball shots (6 kg ball to 9 ft) and pull-ups. Coach's intended time 7 to 10 minutes. | **16:05.** Kipping, no band (rule 1 trial). Note `d123`: "Singles from the 15s. With the thin band in Helen I did sets of 6 or 7." |
| Tue 24 | `a021`: strict 6 × 2, slow lowering, knees-to-chest, hollow rocks | `b124`, 25 minutes. Strict sets skipped under rule 2, because Monday had 45 pull-ups. The other blocks done. | As the rules intend |
| Wed 25 | Class | Class `b125`: power clean every minute, 10 × 2 at 45 kg, then 5 rounds of 10 pull-ups and 15 box jumps. Intended 8 to 10 minutes. | **15:50.** Kipping, no band. Note `d125`: "Singles after round 2." |
| Thu 26 | Rest | Rest | |
| Fri 27 | Class | Class `b127`: deadlift 5 × 5 at 80 kg, then 4 rounds of 200 m run, 12 dumbbell snatches, and 10 pull-ups. Intended 8 to 10 minutes. | **9:40.** Inside the window. The 40 pull-ups were done as ring rows under rule 5. |
| Sat 28 | `a022`: kipping every minute for 12 minutes, 4 a minute; 3 rounds of 400 m, 21 swings, 6 pull-ups | `b128`. All 12 minutes done (48 reps). Rounds: 3:35, 3:50, 4:05. | As planned |
| Sun 29 | Rest | Rest. Soreness report `d129`: "Lats sore, 3 out of 5. Forearms tight." | |

Earlier: on Wednesday 18 November, class `b118` was 21-15-9 thrusters at 25 kg and
pull-ups, intended 7 to 10 minutes. It took **16:20**, kipping with no band. Note
`d118`: "Singles from the 15s." The week 1 review `a101` noted it and kept the
trial, because the stop rule needs two such workouts.

## Step 2. The signals, one at a time

**Session-effort load.** **Observed.** `ath apply training-load-effort` on reports
`c123`, `c124`, `c125`, `c127`, and `c128`. All on the 0–10 scale, for the whole
session. Load is in arbitrary units (AU): rating times minutes.

| Day | Rating × minutes | Load (AU) |
|---|---|---|
| Mon 23 | 9 × 60 | 540 |
| Tue 24 | 4 × 25 | 100 |
| Wed 25 | 9 × 60 | 540 |
| Fri 27 | 7 × 60 | 420 |
| Sat 28 | 7 × 40 | 280 |
| **Week** | | **1,880** |

Week 1 was 1,740 AU. The 3 weeks before the block averaged 1,440 AU (plan `a004`).
This week is 440 AU higher, about 31%. Classes alone were 1,500 AU, 4% above the
pre-block mean. The planned extra sessions add 380 AU. So the planned extra work
explains most of the rise. The two 16-minute classes were rated 9. The on-time
class was rated 7.

**Strength volume.** **Observed.** `ath apply strength-volume` on the three classes:
back squat 900 kg, power clean 900 kg, deadlift 2,000 kg. These are the lifts the
class programmed. The plan added no lifting. Not a candidate cause.

**Pull-up count.** **Observed** from blocks and notes, because pull-ups have no
external load: Monday 45, Tuesday 6 (slow lowering only), Wednesday 50, Friday 0
(ring rows), Saturday 66 (48 in practice and 18 in conditioning). Total 167. Counts
before the block were not recorded, so the size of the rise is unknown.

**Heart-rate variability.** **Observed.** The ring records nightly RMSSD as
`point_measurement` records. `ath calculate coverage hrv_rmssd`: 7 of 7 nights.
`ath apply hrv-change` on Sunday's reading `e129`: 52 ms against a 28-day mean of
53.2 ms (standard deviation 4.0), a change of −1.2 ms or −0.30 standard deviations.
That is within ordinary day-to-day variation.

**Resting heart rate.** **Observed.** Coverage 7 of 7 nights.
`ath apply resting-heart-rate-change` on `e229`: 54 beats per minute against a mean
of 53.1 (standard deviation 2.0), +0.45 standard deviations. Ordinary.

**Sleep.** **Observed.** `ath apply sleep-change`: the night starting Tuesday 24
(`f124`) was 5 h 50 min asleep, 80 minutes under the usual 7 h 10 min. The night
starting Thursday 26 (`f126`) was 6 h 05 min, 65 minutes under. Other nights were
within 20 minutes.

## Step 3. Safety check

**Observed.** Lats sore 3 of 5 on Sunday, after a week of 167 pull-ups, with no
record of the athlete's usual weekly count.

**Official.** Experienced athletes who go "significantly outside their established
'norm'" are at higher risk of rhabdomyolysis. The warning signs are severe muscle
pain, nausea, vomiting, stomach cramps, and dark or cola-coloured urine (L1 p. 153).

Question asked: "Have you had dark or cola-coloured urine, nausea, vomiting, or
muscle pain that is severe or getting worse?" The athlete's answer, saved as their
own note `d229`: "No dark urine, no feeling sick. Just sore lats, easing off
today."

No warning signs, so the review continues. If any had been present, the review
would stop and the athlete would be told to seek medical attention at once.

## Step 4. Evidence that applies

- **Official.** Preserve the stimulus: movements, time domain, and load together
  (L1 p. 77). Scaled times should be "relatively similar" to advanced athletes'
  times (L1 p. 78).
- **Official.** Scale load first, then volume, then the movement (L1 pp. 78–79).
- **Official.** A couplet's time domain is about 7 to 10 minutes (O'Connor, 2023).
- **Official.** Hitting the same region on back-to-back days raises overuse risk
  (O'Connor, 2025).
- **Research.** A change in HRV within about half a standard deviation is ordinary
  (Vesterinen et al., 2016; 40 runners). The rule came from morning chest-strap
  data.
- **Research.** One athlete's session rating matched heart-rate effort only about
  half the time, although group agreement was good (Crawford et al., 2018; 25
  adults).

## Step 5. Limiting factors

**1. Class pull-up volume is beyond the athlete's repeatable set. Most likely.**

- For: all three overlong workouts had 45 to 50 pull-ups, done without a band. Each
  time the athlete went to singles from the second round or after it. Friday's
  workout had 40 pull-ups programmed; done with ring rows under rule 5, it finished
  inside its window. A fresh maximum of 5 kipping pull-ups suggests 2 to 3 a set
  under fatigue (Practice), so 45 reps means 15 to 23 sets.
- Against: nothing in the record.
- What it means: the plan predicted this risk (hypothesis 3). The stop rule in
  class note rule 1 was met on 23 November, the second workout more than 2 minutes
  past its window.
- Test: with the thin band, sets of 5 or more should be possible. The next such
  class should finish inside its window.

**2. A large rise in total pulling.**

- For: 167 pull-ups in the week and lats sore 3 of 5, with the usual count unknown.
- Against, as a cause of the long class times: Monday's class came after a rest day
  and still took 16 minutes. Saturday's practice, which started fresh, was
  completed in full.
- What it means: it does not explain the class times. It is a reason not to raise
  pulling volume next week.

**3. Short sleep.**

- For: Wednesday's long class followed the short Tuesday night.
- Against: the long classes on 18 and 23 November followed normal sleep. Friday's
  on-time class followed the short Thursday night.
- Action: ask the athlete. No training change.

**4. Rising total load.**

- For: weekly load up 31% on the pre-block mean.
- Against: class load rose 4%. The rise matches the planned extra sessions. HRV and
  resting heart rate are ordinary.
- Action: expected. Watch next week.

## Step 6. Changes

Two changes, and one decision to change nothing.

1. **Class pull-ups: apply the stop rule.** From now to the end of the block, class
   pull-ups use the thin band at the class reps, in sets of 5 to 7 from the first
   round. This is one fixed rule for the movement for the rest of the block (see
   [scaling.md](scaling.md)).
   - Reason: load first (L1 p. 78). A band lowers the load. The athlete reported
     sets of 6 or 7 with the thin band in Helen (`d123`), so 45 reps is about 7
     sets. **Hypothesis:** that fits a 7 to 10 minute workout.
   - Why not fewer reps without a band: it is a valid second option. Load comes
     first in the official order. Unassisted practice continues on Tuesday and
     Saturday, where the purpose is the skill.
   - Why not ring rows: that changes the movement, which comes last (L1 p. 79).
     Rule 5 keeps ring rows on a pull-up-heavy Friday only to protect Saturday.
   - Route back, in the next block: after two class workouts with pull-ups inside
     their window with the band, use no band with reps cut to about 60%
     (21-15-9 becomes 12-9-6). Then about 75%. Then the class reps.
2. **Hold week 3 pulling volume at week 2 levels.** Tuesday 1 December: strict 6 ×
   2 instead of 5 × 3 (`a031`). Saturday 5 December: kipping practice 3 a minute
   instead of 4 (`a032`). The Helen rehearsal stays as planned. The athlete notes
   total pull-ups after each session from now on.
   - Reason: soreness after a large rise of unknown size, and the L1 p. 153 caution
     about going far outside the usual amount (Official). Not raising volume the
     next week is Practice.
3. **No change from HRV, resting heart rate, or sleep.** They are within ordinary
   range. One question to the athlete about the two short nights.

The Helen retest on 12 December stays as planned. The band in class does not change
it.

## Step 7. What the next attempt should show

- The next class with pull-ups and an intended 7 to 10 minutes finishes **inside
  its window, by 10:00**, with band pull-up sets of 5 or more in every round.
- Its effort rating may stay at 8 or 9 for a shorter, faster workout. That is
  expected.
- Tuesday 1 December and Saturday 5 December are completed as revised. Lats
  soreness next Sunday is no higher than 3 of 5.
- If the next pull-up class still finishes more than 2 minutes past its window
  with the band, factor 1 is incomplete. Then look at the other movement and
  pacing, using split times per round.

## Step 8. What the athlete is told

> **Change.** In class, use the thin band for pull-ups from now to 13 December, in
> sets of 5 to 7 from the first round. Next week, keep pulling at this week's
> amount: strict 6 × 2 on Tuesday and 3 kipping pull-ups a minute on Saturday.
> Please note how many pull-ups you do in each session.
>
> **Observed.** Three class workouts meant for 7 to 10 minutes took 16:20, 16:05,
> and 15:50. All three had 45 or more pull-ups without a band, and you went to
> singles from the second round or after it. On Friday you did ring rows instead
> of 40 pull-ups, and that workout took 9:40, inside its window. Your HRV and
> resting heart rate were in your normal range. You had two short nights. You did
> 167 pull-ups this week and your lats were sore.
>
> **Evidence.** CrossFit's guidance is to keep the intended time, and to scale the
> load first (official). There is no study that tests this for pull-ups.
>
> **Hypothesis.** The pull-up volume, not your fitness, is what stretched those
> workouts. We agreed to try class pull-ups without a band and to switch if two
> workouts ran long. They did, so we switch.
>
> **Next check.** The next class with pull-ups should finish inside its window, by
> 10 minutes.
>
> **Needed from you.** Was there a reason for the short nights on Tuesday and
> Thursday?

## Step 9. The records

### New record: preview with `ath log --json`, then `ath log --yes`

```json ath-record-example
[
  {
    "record_type": "saved_training_review",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a102",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" },
        "authored_at": "2026-11-29T19:00:00.000Z",
        "evidence": [
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a004", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a005", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a021", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a022", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a031", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a032", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a101", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000b118", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000b123", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000b124", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000b125", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000b127", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000b128", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000c123", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000c124", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000c125", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000c127", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000c128", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000d118", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000d123", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000d125", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000d129", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000d229", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000e129", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000e229", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000f124", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000f126", "revision": 1 }
        ],
        "content_version": 1,
        "status": "active"
      },
      "period_start": "2026-11-23",
      "period_end": "2026-11-29",
      "summary": "Observed: 3 classes and 2 extra sessions done. Class workouts meant for 7 to 10 minutes took 16:20 (18 Nov), 16:05 (23 Nov), and 15:50 (25 Nov); each had 45 to 50 pull-ups without a band, with singles from the second round or after it. On 27 Nov the 40 programmed pull-ups were done as ring rows under rule 5 and the workout took 9:40, inside its window. Saturday practice was completed in full. 167 pull-ups in the week; usual count not recorded; lats sore 3 of 5 on 29 Nov. Warning signs of rhabdomyolysis asked about; the athlete reported none (d229). Session-effort load 1,880 AU (0-10 scale, whole session) against 1,440 AU before the block; classes alone 1,500 AU, so the planned extra sessions explain most of the rise. Strength volume matched the class programming. HRV -0.30 and resting heart rate +0.45 standard deviations from their 28-day means on 29 Nov, both ordinary, 7 of 7 nights covered. Two short nights (24 Nov: 5 h 50 min; 26 Nov: 6 h 05 min). Evidence: CrossFit advises preserving the intended time domain and scaling load first, then volume, then movement (L1 pp. 77-79, official); athletes far outside their usual volume are at higher rhabdomyolysis risk (L1 p. 153, official). Hypotheses, ranked: 1. class pull-up volume exceeds the athlete's repeatable set size, as the plan predicted, and the stop rule is met; 2. the rise in total pulling does not explain the class times but argues against raising volume next week; 3. short sleep may have added to Wednesday only; 4. the load rise is the planned extra work.",
      "recommendations": [
        "Class pull-ups to the end of the block: thin band at the class reps, sets of 5 to 7 from the first round. Reason: the stop rule is met; scale load first (L1 p. 78). Check: the next class with pull-ups finishes inside its intended window with band sets of 5 or more.",
        "Hold week 3 pulling at week 2 levels: Tuesday strict 6 x 2, Saturday kipping practice 3 a minute. The athlete notes total pull-ups after each session. Check: both sessions completed and lats soreness no higher than 3 of 5 next Sunday.",
        "No change from HRV, resting heart rate, or sleep. Ask the athlete about the two short nights.",
        "Route back for class pull-ups in the next block: after two workouts inside the window with the band, no band with reps cut to about 60%, then about 75%, then the class reps."
      ]
    }
  }
]
```

### Data objects for `ath correct`

Do not send this block to `ath log`: every record in it already exists. For each
item, read the current revision with `ath records show <record-id> --json`, then
pass only its `data` object:

```sh
ath correct 01890f3e-e7d3-7cc0-88c8-00000000a005 --revision 1 --data '<data>' --json
ath correct 01890f3e-e7d3-7cc0-88c8-00000000a031 --revision 1 --data '<data>' --json
ath correct 01890f3e-e7d3-7cc0-88c8-00000000a032 --revision 1 --data '<data>' --json
ath correct 01890f3e-e7d3-7cc0-88c8-00000000a022 --revision 1 --data '<data>' --json
```

Repeat each with `--yes` after approval. Mark `a021` completed in the same way as
`a022`.

```json ath-record-example
[
  {
    "record_type": "coaching_note",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a005",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" },
        "authored_at": "2026-11-29T19:00:00.000Z",
        "evidence": [
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000202", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000203", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000b118", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000b123", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000b125", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000b127", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000d123", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000d129", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000d229", "revision": 1 }
        ],
        "content_version": 2,
        "status": "active"
      },
      "title": "Class rules for the pull-up block",
      "note": "1. Class pull-ups: thin band at the class reps, sets of 5 to 7 from the first round, to the end of the block. Changed on 29 November because the stop rule was met: 16:20 on 18 November and 16:05 on 23 November against a 7 to 10 minute window, then 15:50 on 25 November. Route back in the next block: after two class workouts with pull-ups inside their window with the band, no band with reps cut to about 60%, then about 75%, then the class reps. 2. If Monday's class has more than 30 pull-ups or any toes-to-bar, Tuesday skips the strict pull-up sets and keeps the rest. 3. No extra pulling after class. 4. Test days: on Monday 7 December, do class pull-ups as ring rows so the Tuesday strict pull-up retest starts fresh. On Friday 11 December, before the Helen retest, use lighter class loads and ring rows instead of pull-ups. 5. If Friday's class has more than 30 pull-ups, do them as ring rows so Saturday's pull-up work starts fresh. If it has 1 to 30, Saturday's kipping practice is 8 minutes of 3 a minute. Rules 4 and 5 override rule 1 on those days. 6. Rate every session 0-10 about 30 minutes after it ends, for the whole session's minutes. 7. Added on 29 November: week 3 pulling stays at week 2 levels, and the athlete notes total pull-ups after each session.",
      "applies_from": "2026-11-16",
      "applies_to": "2026-12-13"
    }
  },
  {
    "record_type": "planned_session",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a031",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" },
        "authored_at": "2026-11-29T19:00:00.000Z",
        "evidence": [
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000202", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000b124", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000b128", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000d129", "revision": 1 }
        ],
        "content_version": 2,
        "status": "active"
      },
      "training_plan_id": "01890f3e-e7d3-7cc0-88c8-00000000a004",
      "title": "Week 3 Tuesday: strict pulling and toes-to-bar singles",
      "scheduled_date": "2026-12-01",
      "activity_type": "crossfit",
      "blocks": [
        { "block_type": "warm_up", "title": "Shoulders, grip, and hollow", "instructions": "As week 1.", "planned_duration_s": 420 },
        { "block_type": "strength", "title": "Strict pull-ups 6 x 2", "instructions": "Changed on 29 November from 5 x 3: pulling stays at week 2 levels after lats soreness of 3 of 5 following 167 pull-ups. Rest 90 s. One or two reps short of failure. Skip if Monday's class had more than 30 pull-ups.", "planned_reps": 12 },
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
      "authorship": {
        "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" },
        "authored_at": "2026-11-29T19:00:00.000Z",
        "evidence": [
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000201", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000202", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000b128", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000d129", "revision": 1 }
        ],
        "content_version": 2,
        "status": "active"
      },
      "training_plan_id": "01890f3e-e7d3-7cc0-88c8-00000000a004",
      "title": "Week 3 Saturday: kipping sets and Helen in pieces",
      "scheduled_date": "2026-12-05",
      "activity_type": "crossfit",
      "blocks": [
        { "block_type": "warm_up", "title": "Easy run, swings, kip swings", "instructions": "As week 1.", "planned_duration_s": 600 },
        { "block_type": "skill", "title": "Kipping pull-ups every minute for 10 minutes, 3 a minute", "instructions": "Changed on 29 November from 4 a minute: pulling stays at week 2 levels. Stop a set when the swing loses rhythm. If Friday's class had 1 to 30 pull-ups, do 8 minutes of 3 a minute (class note, rule 5).", "planned_duration_s": 600, "planned_reps": 30 },
        { "block_type": "conditioning", "title": "Helen in pieces: 3 rounds of 400 m run, 21 swings, 12 pull-ups, rest 2:00", "instructions": "Purpose: rehearse the retest. Kipping, no band, sets of 4. Log each round's time. The goal pace is 4:30 a round with no rest, and rested rounds run faster. Rounds of about 4:00 or less make 13:30 plausible; rounds over 4:30 make it unlikely.", "planned_distance_m": 1200, "planned_reps": 99, "planned_weight_kg": 16 },
        { "block_type": "cool_down", "title": "Walk and stretch", "planned_duration_s": 300 }
      ],
      "completion_status": "planned"
    }
  },
  {
    "record_type": "planned_session",
    "record_id": "01890f3e-e7d3-7cc0-88c8-00000000a022",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "example-agent", "model": "example-model" },
        "authored_at": "2026-11-29T19:00:00.000Z",
        "evidence": [
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000201", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000202", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000b127", "revision": 1 }, { "record_id": "01890f3e-e7d3-7cc0-88c8-00000000b128", "revision": 1 }
        ],
        "content_version": 2,
        "status": "completed"
      },
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
      "completion_status": "completed",
      "completed_workout_session_id": "01890f3e-e7d3-7cc0-88c8-00000000b128"
    }
  }
]
```

The planned blocks of `a022` stay as they were planned. What actually happened is
in the workout `b128` and in the review.
