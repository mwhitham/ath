# Athlete assessment

The assessment tells you who you are coaching before you advise them. It ends with
a saved `coaching_note` that holds the assessment, and one `goal` record for each
goal the athlete agrees to.

Run it for a new athlete. Run it again when the goal changes, after a break of
more than two weeks, or after an injury.

## Step 1. Read the record first

Read before you ask. The athlete should not have to repeat what the record
already holds.

```sh
ath stats --json
ath records list --type goal --limit 10000 --json
ath records list --type training_plan --limit 10000 --json
ath records list --type planned_session --limit 10000 --json
ath records list --type coaching_note --limit 10000 --json
ath records list --type saved_training_review --limit 10000 --json
ath records list --type benchmark_definition --limit 10000 --json
ath records list --type benchmark_result --limit 10000 --json
ath records list --type workout_session --limit 10000 --json
ath records list --type athlete_report --limit 10000 --json
ath records list --type point_measurement --limit 10000 --json
ath records list --type measurement_sample --limit 10000 --json
ath records list --type sleep_session --limit 10000 --json
```

`ath records list` returns records oldest first, and only 100 by default. Without
`--limit 10000`, the most recent records can be missing. It has no date filter, so
read the dates in the output. Open one record with
`ath records show <record-id> --history --links --json`.

Extract these facts, and name the record behind each one:

| Fact | Where it comes from |
|---|---|
| Sessions per week over the last 4 to 8 weeks | `workout_session` start dates; `ath stats` counts |
| Typical session length | `workout_session` start and end |
| Benchmark history, with the version of each attempt | `benchmark_result`, its `scaling`, and its `org.athleticstandard/crossfit-score` extension |
| Lifts and loads | `sets` on `workout_session` records (movement, reps, weight) |
| How sessions felt | `athlete_report` with `report_type: session_effort` |
| Soreness, notes, and pain mentions | `athlete_report` with `soreness` or `note` |
| Earlier coaching and how it went | `goal`, `training_plan`, `planned_session` completion, `saved_training_review` |
| Recovery monitoring available | `ath stats` coverage; `point_measurement` for nightly `hrv_rmssd` and `resting_heart_rate`; `measurement_sample` for Apple Watch `hrv_sdnn`; `sleep_session`. Note which HRV statistic the device records. Use that one only; never mix RMSSD and SDNN. |

If a note mentions pain, an injury, or illness, stop and read the red flags below
before anything else.

## Step 2. Ask for what is missing

Ask one question at a time. Start with the ones that block a plan.

1. **Goal.** What do you want to be able to do, and by when? Turn a vague goal into
   a test. "Better at gymnastics" becomes "five strict pull-ups in one set" or
   "Helen as prescribed".
2. **Schedule.** Which days can you train, for how long, and which of those are
   classes?
3. **Class programming.** Does your gym post the week's workouts in advance? Where?
4. **Equipment.** What can you use outside class? A pull-up bar, rings, a barbell,
   dumbbells, a rower, a rope, a box?
5. **Training age.** How long have you done CrossFit? What other sport? How many
   weeks in a row have you trained recently?
6. **Limits.** Any pain, injury, surgery, or medical condition? Has a clinician given
   guidance?
7. **Competition.** Do you plan to do the Open or another event? Which division?
8. **Movement check.** Which of these can you do today, and how many in one set?
   (See step 4.)

## Step 3. Check the red flags

Stop and refer the athlete to a qualified clinician or physiotherapist when any of
these is true. Do not plan around it until they have guidance. You may then adapt
training to that guidance. You never diagnose or treat.

| Red flag | Action | Basis |
|---|---|---|
| Chest pain, fainting, dizziness, palpitations, or unusual breathlessness with exercise | Stop. Refer before any further training. | Practice |
| Severe muscle pain with dark or cola-coloured urine, nausea, vomiting, or stomach cramps after training | Seek medical attention at once. | Official: L1 p. 153 |
| Pain that changes how the athlete moves, pain that lasts more than a few days, swelling, numbness, or tingling | Refer. Remove the painful movement until assessed. | Practice |
| A recent injury or surgery | Ask for the clinician's guidance. Plan within it. | Official: L1 p. 156 says a trainer's scope does not include diagnosis or treatment, and an athlete with a medical condition needs a physician's clearance |
| Pregnancy, or the months after giving birth | Refer for guidance. Plan only within it. | Practice |
| Eating concerns, fast weight loss, or missed periods | Refer. Do not give diet targets. | Practice |
| Signs of heat illness: confusion, collapse, or feeling very hot and unwell during hard work in heat | Stop. Get urgent help. | Practice |
| Any diagnosed medical condition or regular medication | Ask whether a clinician has cleared training. | Official: L1 p. 156 |

Some athletes have a higher risk of rhabdomyolysis. **Official** (L1 p. 153): fit
people new to CrossFit from other training, people returning after a layoff, and
experienced athletes who go "significantly outside their established 'norm.'"

- **Official.** For beginners, cut intensity and volume by about half for at least
  two weeks (L1 p. 78). Beginners keep "negatives" (slow lowering) to a minimum
  (L1 p. 152).
- **Practice.** Apply the same caution to the other higher-risk groups above: start
  at about half the usual intensity and volume, and add slow lowering work
  gradually. The guide states the halving rule for beginners only.

## Step 4. Rate movement competence

**Official.** "Mechanics, consistency, then—and only then—intensity" (L1 p. 77).
Movements should be done correctly and consistently before load and speed are
added. Skipping this order "increases the risk for injury and potentially blunts
long-term progress" (L1 p. 77).

Rate each family on four levels. This grid is **Practice**: it applies the Level 1
order to a record you can keep.

| Level | Meaning | What it allows |
|---|---|---|
| 0. Not yet | Cannot do the movement, or cannot do it safely | Teach a progression only. Not in timed work. |
| 1. Mechanics | Good positions when slow, light, and fresh | Practice it alone, unloaded or light. |
| 2. Consistency | Keeps good positions across many reps and under moderate fatigue | Use it in workouts at moderate load and speed. |
| 3. Intensity | Keeps good positions at workout speed and prescribed load | Use the prescribed version. |

You cannot see the athlete move. Record where each rating came from: the athlete's
own report, their class coach, or a video the athlete described. Write "reported"
next to every rating that you did not see confirmed in the record.

Movement families and the order inside each one:

| Family | Order to build | Basis |
|---|---|---|
| Squat | Air squat, front squat, overhead squat | Official: L1 pp. 171–178 |
| Press | Shoulder press, push press, push jerk | Official: L1 pp. 184–191 |
| Hinge and pull from the floor | Deadlift, sumo deadlift high pull, medicine-ball clean | Official: L1 pp. 201–208 |
| Snatch | Needs an overhead squat, which needs an air squat | Official: L1 p. 159 |
| Pull-up | Ring row, strict pull-up, kipping pull-up, then chest-to-bar or butterfly | Official for strict before kipping (L1 p. 219). The rest is Practice. |
| Hanging core | Hanging knee raise, knees-to-chest, toes-to-bar | Practice |
| Handstand push-up | Pike push-up, box pike, wall handstand hold, strict handstand push-up, then kipping | Practice |
| Muscle-up | Strict pull-up and dip strength, then a strict muscle-up, then kipping | Official for strict before kipping (L1 p. 232). The rest is Practice. |
| Rope | Single-unders, then double-unders | Practice |
| Monostructural | Run, row, bike, ski: can the athlete hold an even pace? | Practice |

Useful numbers to ask for, if the record does not already hold them:

- Strict pull-ups in one set, and kipping pull-ups in one set.
- Toes-to-bar in one set, or knees-to-chest if none.
- Double-unders in one set.
- Handstand push-ups in one set, and which version.
- A recent heavy single or set of three for back squat, front squat, deadlift, and
  press, if the athlete knows them.
- A recent time for 400 m run, 1 mile run, or 2,000 m row.

Do not test everything in one week. Use what the record holds. Test only what the
plan needs. A test result the athlete agrees to log is a `benchmark_result` (see
[records.md](records.md)).

## Step 5. Place the athlete

Training age changes what a sensible plan looks like. These bands are **Practice**.

| Band | Typical sign | What it means for the plan |
|---|---|---|
| New | Under 6 months of regular CrossFit | Mostly mechanics. Scale load and volume by about half for at least two weeks (L1 p. 78). No extra sessions yet. |
| Developing | 6 months to 2 years | Consistency. One or two priorities. Extra work short and low in fatigue. |
| Experienced | Over 2 years, prescribed loads in most workouts | Clear weak points. Targeted blocks of 6 to 8 weeks with tests at each end. |
| Competitive | Trains for the Open leaderboard or later stages | Season plan around the Open. More volume, managed carefully. |

**Research.** In a survey of 3,049 participants, injury was more often reported in
the first year of training and with fewer than three sessions a week (Feito et al.,
2018). This is an association from self-reported data, not proof of cause.
Competitors reported more injuries than non-competitors in a survey of 191 athletes
(Montalvo et al., 2017). Treat both as reasons for care, not as predictions for one
athlete.

## Step 6. Write the assessment

Use these headings. Keep each line to one fact.

1. **Goals**, as tests with dates.
2. **Schedule and equipment.**
3. **Observed from the record**, with record IDs and dates.
4. **Reported by the athlete**, marked as reported.
5. **Movement competence**, with the level and the source of each rating.
6. **Limits and red flags**, and any clinician guidance.
7. **Priorities** for the next block. Label these as hypotheses.
8. **Assumptions and missing facts.**

A good line: "Strict pull-ups: 3 in one set, reported by the athlete on 2 November
2026 (record `01890f3e-e7d3-7cc0-88c8-000000000202`, revision 1). Level 2 for
strict pulling."

A bad line: "Pulling is weak." It names no number, no source, and no date.

## Step 7. Save it

After the athlete approves, save:

- the assessment as a `coaching_note` titled "Assessment", with `applies_from` set
  to today;
- each agreed goal as a `goal`, with `target_benchmark_definition_id` and
  `target_score` when the goal is a benchmark.

Shapes and an example batch are in [records.md](records.md). A complete example is
in [example-block.md](example-block.md).
