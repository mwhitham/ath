# Weekly review

A weekly review compares what was planned with what happened, reads each
monitoring signal on its own, names the likely limiting factors, and makes a small
number of justified changes. It ends with a saved `saved_training_review` and any
revised `planned_session` records. A complete example is in
[example-review.md](example-review.md).

## Procedure

### Step 1. Set the period

Use Monday to Sunday in the athlete's time zone, unless the athlete's week starts
on another day. Write the dates as `YYYY-MM-DD`. Use the same zone in every
`--timezone` option.

### Step 2. Read the plan

`ath records list` returns records oldest first and only 100 by default. Pass
`--limit 10000` so the most recent records are not left out.

```sh
ath records list --type training_plan --limit 10000 --json
ath records list --type planned_session --limit 10000 --json
ath records list --type coaching_note --limit 10000 --json
```

Keep the planned sessions whose `scheduled_date` or `scheduled_start_at` falls in
the period. Read the coaching notes that apply to it, such as class scaling rules.

### Step 3. Read what happened

```sh
ath records list --type workout_session --limit 10000 --json
ath records list --type athlete_report --limit 10000 --json
ath records list --type benchmark_result --limit 10000 --json
```

Keep the records in the period. Note each workout's `text_result`, `description`,
and `sets`. Note each `session_effort`, `soreness`, and `note` report.

If a session the athlete did is missing from the record, ask for it. Log it through
the `athletic-standard` skill with the athlete's approval. Do not guess it.

### Step 4. Match planned to completed

Make one row per planned session and per class.

| Date | Planned | Done | Difference |
|---|---|---|---|

A difference is anything that changed: skipped, shortened, a different version, a
different load, much longer or shorter than intended. For class workouts, compare
the time with the intended time domain.

With the athlete's approval, mark each planned session: `completed` with
`completed_workout_session_id`, or `skipped`, or `cancelled`. Use `ath correct`
(see [records.md](records.md)).

### Step 5. Session-effort load

Run `training-load-effort` on each `session_effort` report in the week:

```sh
ath apply training-load-effort --record <athlete-report-id> --json
```

Add the loads. Show each one. Load is in arbitrary units (AU): rating times minutes.
Compare the week with the previous 2 to 4 weeks.

- Check that every report uses the same `rating_scale`. Loads on the 0–10 scale and
  the 6–20 scale cannot be added or compared.
- Check that every report covers the same span: the whole session, or only the
  workout part. Mixed spans describe different things.
- Split the change into planned extra work and changed class ratings. A rise that the
  plan added on purpose is expected.

### Step 6. Strength volume

For each workout with logged sets:

```sh
ath apply strength-volume --record <workout-session-id> --json
```

Report volume per movement. Do not add different movements into one number. Compare
with the plan and the previous weeks. Body-weight movements such as pull-ups have
no external load, so count their reps separately from notes and planned blocks.

### Step 7. Recovery signals, one at a time

Find out which HRV statistic the athlete's device records. Use `hrv_rmssd` or
`hrv_sdnn`, whichever it is, and never both in one result. Nightly RMSSD from a
ring or strap is stored as `point_measurement` records. Apple Watch SDNN is stored
as `measurement_sample` records, often several a day.

```sh
ath records list --type point_measurement --limit 10000 --json    # nightly RMSSD, resting heart rate
ath records list --type measurement_sample --limit 10000 --json   # Apple Watch SDNN samples
ath records list --type sleep_session --limit 10000 --json
```

Check coverage first. A weekly mean from three nights is weaker than one from seven.

```sh
ath calculate coverage <hrv_rmssd or hrv_sdnn> --from <start> --to <end> --timezone <zone> --json
ath calculate coverage resting_heart_rate --from <start> --to <end> --timezone <zone> --json
```

Then run each signal on the days that matter, usually the last day of the week and
the day before any hard or overlong session:

```sh
ath apply hrv-change --record <point-measurement-or-measurement-sample-id> --json
ath apply resting-heart-rate-change --record <point-measurement-id> --json
ath apply sleep-change --record <sleep-session-id> --json
ath calculate baseline <hrv_rmssd or hrv_sdnn> --from <start> --to <end> --timezone <zone> --json
```

`baseline` with a time zone weights each day once. That matters for Apple SDNN,
where one day can have many samples.

Report three separate results: HRV, resting heart rate, and sleep. There is no
Athletic Standard recovery score (D159). Never combine RMSSD and SDNN.

### Step 8. Performance markers

- Did class workouts land in their intended time domains?
- Did the extra sessions hit their planned reps, loads, and times?
- Any benchmark or test? Check comparability with [benchmarks.md](benchmarks.md).

### Step 9. Check safety, then name the limiting factors

First check safety. If the athlete reports soreness after a large rise in one
movement's volume, ask about the rhabdomyolysis warning signs: severe muscle pain,
dark or cola-coloured urine, nausea, vomiting, or stomach cramps (L1 p. 153). If
any is present, stop and tell them to seek medical attention at once. **Official.**
Experienced athletes who go "significantly outside their established 'norm'" are
at higher risk (L1 p. 153). Record the athlete's answer as their own report.

Then list each plausible factor. For each, write the evidence for it, the evidence
against it, and what would test it. Rank them.

Common factors in CrossFit:

| Factor | What would point to it |
|---|---|
| A movement beyond the athlete's set size | Overlong workouts share that movement; notes say "singles"; workouts without it land on time |
| Scaling that misses the stimulus | Times far outside the window; the same version each time |
| Stacking one pattern on consecutive days | The second day's session falls short; soreness in that region |
| A rise in total load | Session-effort load up, with ratings up on the same work |
| Short or broken sleep | `sleep-change` shows short nights before poor sessions |
| Life stress | Athlete notes or stress reports |
| Pacing | Fast first round, then long rests; the athlete reports going out too hard |
| Soreness after a large rise in one movement's volume | Ask about the rhabdomyolysis warning signs (L1 p. 153). If any is present, stop and refer. |
| Illness, pain, or injury | Athlete notes. Refer to a clinician; do not interpret. |
| Missing data | Low coverage; sessions not logged |

### Step 10. Decide the changes

Rules:

1. Make one to three changes. More changes make it impossible to tell what worked.
2. Each change names its reason, its evidence, and its expected result.
3. Prefer changing scaling or placement to adding volume.
4. Leave what worked alone.
5. Do not change the plan because of one low HRV night or one high resting heart
   rate morning.
6. Say what the next session or week should show if the change is right, and what
   you will conclude if it does not.

### Step 11. Save

After the athlete approves:

- one `saved_training_review` with `period_start`, `period_end`, a `summary` that
  keeps **Observed**, **Evidence**, and **Hypothesis** apart, and the changes in
  `recommendations`;
- each changed future session as a new revision of its `planned_session`, with
  `content_version` raised by one;
- a changed standing rule as a new revision of its `coaching_note`;
- completion status on the past week's planned sessions.

List every record you relied on, with its revision, in `authorship.evidence`.

## Reading the signals

### Session-effort load

- **Research.** Session RPE (rating of perceived exertion, the athlete's rating of
  how hard the whole session was) times minutes was valid against heart-rate load
  in a review of 36 studies, mostly team and endurance sports (Haddad et al., 2017).
- **Research.** In one study in this style of training, 25 adults over 6 weeks,
  session RPE load agreed well with heart-rate load across the group (r = 0.889).
  For a single athlete, the rating matched the heart-rate band only 52% of the time
  (Crawford et al., 2018). Crawford used the Borg 6–20 scale, about 5 minutes after
  the workout, times the workout minutes only.
- **Research.** Session RPE separated an all-out session from a moderate one better
  than heart-rate methods in 8 trained men (Falk Neto et al., 2020).
- **Practice.** For this skill, use the 0–10 scale times the whole session's
  minutes (Foster et al., 2001), asked about 30 minutes after the session, as later
  papers describe Foster's method. The original text was not checked for the
  timing. Crawford's
  method is also valid. What matters is that the athlete keeps one method. Write the
  method in a `coaching_note`.
- **Research.** No load number has been shown to predict injury. The acute-to-chronic
  workload ratio is not supported for that use (Impellizzeri et al., 2020).

### Heart-rate variability and resting heart rate

- **Research.** Weekly means tracked fitness change better than single days in 10
  runners (Plews et al., 2013). Use at least 3 valid days a week (Plews et al., 2014).
- **Research.** A change within about half a standard deviation of the athlete's
  recent days is ordinary variation (Vesterinen et al., 2016; 40 runners).
- **Research.** These rules came from morning chest-strap recordings, not overnight
  ring or wrist data. Treat them as a starting point.
- **Research.** Heart-rate variability fell after short and 15-minute CrossFit-style
  workouts and recovered within 2 hours in 10 men (Kliszczewicz et al., 2018). An
  overnight value reflects much more than one workout.
- **Practice.** Heart-rate variability is commonly said to move with sleep,
  alcohol, illness, heat, travel, stress, the menstrual cycle, and hard training.
  The research files behind this skill do not test those causes. The data cannot
  say which caused a change, so name candidates and ask.

### Sleep

`sleep-change` compares one night's time asleep with the athlete's earlier main
sleeps from the same device. Report short nights with their dates. Ask about them.
Do not give sleep treatment advice. Persistent sleep problems go to a clinician.

## Review summary template

```
Period: <start> to <end>

Observed
- Sessions: <n> planned, <n> done, <n> changed, <n> skipped.
- Class workouts against intended times: ...
- Session-effort load: <total> AU (previous weeks: ...), scale 0-10, whole session.
- Strength volume: ...
- HRV: ... Resting heart rate: ... Sleep: ... Coverage: ...

Evidence
- <official guidance or research, with strength>

Hypotheses (ranked)
1. <factor>: for ...; against ...; test ...

Changes
1. <change>: reason; expected result; check on <date>.

Next check
- <what the next session or week should show>
```

## Good and bad reviews

Bad: "HRV was down 3 ms and you seem tired, so take an extra rest day and do more
mobility." One signal, no coverage check, no link to what happened in training,
and a change with no test.

Good: "Three class workouts meant for 7 to 10 minutes took about 16 minutes. All
three had 45 or more pull-ups. On Friday you did ring rows instead of pull-ups, and
that workout finished inside its window. HRV, resting heart rate, and sleep were
within your normal range except two short nights. The most likely limit is pull-up
volume. Use the thin band in class pull-ups so sets stay at 5 to 7. The next class
with pull-ups should finish inside its 7 to 10 minute window."
