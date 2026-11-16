# Training progression

Read this file when you write sessions, decide whether to progress, or adjust a plan
week by week. It covers running, stations, running after stations, race
combinations and simulations, strength and endurance in one week, and the taper.

No controlled HYROX training trial exists. Everything here is transferable research
or coaching practice, and it is labelled. Study details are in
[evidence.md](evidence.md). Tailor everything to the athlete's records.

## Principles

1. **Running is about half the race in Top 100 PRO results** (Research: Rappelt et
   al. 2026, about 48–51% of time). In a small recreational simulation it was about
   59% (Brandt et al. 2025, n = 11; Unverified arithmetic in the research notes).
   Most athletes need steady running work all through the plan.
2. **Stations matter more the slower the athlete is** (Research: Rappelt et al.
   2026). Use the athlete's own splits or fresh station tests to pick which stations
   get extra time.
3. **There is more than one route to a fast time.** "Strong runner" and "strong
   station" profiles reached similar times in PRO men (Research: Yustres Amores et
   al. 2026). Build on strengths as well as fixing gaps.
4. **Overuse injuries of the knee and lower leg are common** (Research: Chittenden et
   al. 2026). Raise running, lunge, and sled volume gradually.
5. **One change at a time.** When you change several things in one week, the
   records cannot show which one worked.

## Running capacity

| Session | Purpose | Typical content (Practice) |
|---|---|---|
| Easy run | Aerobic base, most of the weekly distance | Conversational pace, 30–75 min |
| Long easy run | Endurance for 8 km plus stations | Build to about 60–90 min for most athletes |
| Threshold | Raise the pace that can be held for a long time | 3–4 × 6–10 min, or 2–3 × 2 km, at a hard but controlled effort |
| Intervals | Speed reserve | 5–8 × 2–4 min at about 5 km to 10 km effort |
| Race-pace 1 km repeats | Learn the race run pace | 4–8 × 1 km at planned race run pace, with short rests or stations between |

How to progress running:

- **Practice.** Add distance in small steps and hold for a week before adding more.
  No study gives a safe weekly increase. Stay below any distance at which the
  athlete has had pain before.
- **Practice.** Progress one of volume or intensity in a week, not both.
- **Practice.** Use a lighter week every third or fourth week.
- **Practice.** Keep most running easy and add some hard sessions. No single split
  of easy and hard running was clearly best across recent meta-analyses
  (Transferable: Silva Oliveira et al. 2024; Rosenblat et al. 2025).
- **Transferable.** Strength and jump training 2–3 times a week improved running
  economy in runners (Balsalobre-Fernández et al. 2016; Blagrove et al. 2018), most
  clearly with heavy loads (Llanos-Lagos et al. 2024). Lower-body strength work is
  part of running preparation.

Use heart-rate zones only with a measured maximum heart rate or threshold. Age
formulas can be about 11 beats per minute off for one person (Research: Nes et al.
2013). Different threshold tests give different numbers, so use the same test each
time (Transferable: Faude et al. 2009). See the `heart-rate-zones` calculation skill.

## Station capacity and technique

Use the four stages in [stations.md](stations.md#how-to-progress-any-station-practice):
technique, fresh capacity, under fatigue, race pace.

- Teach all eight stations and their standards early, even the ones the athlete
  knows, so no-reps do not come from not knowing a rule (Practice).
- Practise every station at least every two weeks from the build phase on.
- Give extra time to the two or three stations that cost the most in the athlete's
  own splits or fresh tests.
- Keep the standard while tired. A rep that would be a no-rep in a race is not
  progress. Squat depth falls with fatigue (Transferable: Hooper et al. 2013).
- Sleds and lunges load the legs and knees heavily. Do not add sled and lunge volume
  in the same week as a running increase.

## Running after stations ("compromised running")

"Compromised running" is a coaching term for running while tired from a station. It
is not a defined scientific measure.

- **Transferable.** In triathlon, running after a 40 km ride costs 1.6–11.6% more
  energy than fresh running (Millet and Vleck 2000). Repeated short ride-run blocks
  sped up the transition and the first 333 m, but not the whole ride-run, in 12
  triathletes (Hue et al. 2002).
- **Unverified (preprint).** In HYROX, runs after sandbag lunges and the sled push
  slowed most (Zhu et al. 2026).
- **Research.** In a small lab simulation, the run after burpee broad jumps was
  slowest (Brandt et al. 2025, n = 11).
- **Practice.** Station-then-run pairs once a week from the build phase on. No study
  shows they improve total race time.

How to progress pairs (Practice):

| Step | Content |
|---|---|
| 1 | 2–3 pairs: half a station, then 1 km easy-to-steady |
| 2 | 3–4 pairs: half or full station, then 1 km at race run pace |
| 3 | 4–5 pairs, including the leg-heavy ones: sled push then run, lunges then run |
| 4 | Race-order blocks: for example stations 5–8 with runs, at race pace |

The check for moving on: the run after the station is within the planned race run
band, and the station keeps its standard.

## Race-specific combinations and simulations

| Session | When (Practice) | What to log |
|---|---|---|
| Half simulation (4 runs and 4 stations in race order) | 5–6 weeks before the race | A `benchmark_result` with only those segments, `"event": "simulation"`, under its own half-simulation definition |
| Full simulation at race effort | 3–4 weeks before the race | A `benchmark_result` with all 16 segments, under a full-simulation definition |
| Race-pace pairs | Final 2–3 weeks | Session effort and splits in the workout record |

- Log a half simulation under its own benchmark definition, never under a
  full-race or full-simulation definition, so half and full times are not mixed.
  See [records.md](records.md#benchmark-definitions-for-hyrox).
- A full simulation is as tiring as a race for many athletes. Plan 2–4 easy days
  after it.
- Do not do full simulations often. One or two in a 12-week plan is common practice.
- Log every substitution. Compare a simulation with a race segment by segment only.
- Use the simulation to set the race budget ([pacing.md](pacing.md)) and to rehearse
  kit, food, drink, warm-up, and wall-ball set plans.

## Strength and endurance in one week

What the evidence says (Transferable, see [evidence.md](evidence.md)):

- Adding endurance work cost little maximal strength or muscle size (Schumann et al.
  2022), but it did cost some explosive strength, more when both were in the same
  session than when they were at least 3 hours apart.
- Trained people lost some lower-body strength gain, mainly in same-session training
  (Petré et al. 2021). The loss was seen in men but not in women (Huiberts et al.
  2024).
- An older meta-analysis found running interfered more than cycling (Wilson et al.
  2012); a later one did not confirm this (Schumann et al. 2022).
- In a shared session, strength first gave slightly more lower-body strength gain
  and no difference in VO2max (Murlasits et al. 2018; Eddens et al. 2018).

Rules (Practice):

- Put hard runs and heavy leg sessions on different days, or at least 3 hours
  apart.
- When the athlete must combine them, put the session's main goal first.
- Do not put two hard days back to back early in the plan.
- Keep the day after a heavy leg session for easy running or rest.
- Count station work as leg work. A sled and lunge session is a heavy leg day.
- Strength sessions: 1–2 a week. Squat or leg press, a hinge (deadlift or Romanian
  deadlift), single-leg work (split squats, step-ups), pulling (rows, pull-downs),
  and trunk work. Heavier, lower-rep work in the build phase; maintenance in the
  last 3–4 weeks.

Log strength sets with movement and load basis, so `strength-volume` can read them.

## Taper

- **Transferable.** The best taper across 27 studies lasted 2 weeks, cut volume by
  41–60% in a falling curve, and kept intensity and frequency the same (Bosquet et
  al. 2007). In endurance athletes, effective tapers cut volume by 41–60%, kept
  intensity and frequency, and lasted 21 days or less (Wang et al. 2023). No HYROX
  taper study exists.
- **Template (Practice).** Two weeks before: about 30–40% less volume than the last
  full week, the same intensity, the same number of sessions. Race week: about
  50–60% less, two short race-pace touches, nothing new, no heavy legs in the last
  5 days.
- Keep a little station work in race week (short sets) so the movements feel normal.
- A shorter taper (7–10 days) can suit athletes with low training volume. This is a
  judgment; test it on a lesser race if possible.

## Progress, hold, or reduce

Decide once a week. Read the records first. Then ask the athlete.

**Where the planned effort lives.** A `planned_session` has no field for a planned
effort rating. Write it in the block's `instructions`, on the athlete's own scale,
for example "Planned effort: 6 of 10 (0-10 scale)". Compare it with the
`session_effort` report for the completed workout.

**How far is too far.** These thresholds are coaching judgment, not tested values:

| Athlete's scale | Near the plan | Clearly higher than planned |
|---|---|---|
| `0-10` or `1-10` | Within 1 point | 2 or more points above |
| `6-20` | Within 2 points | 3 or more points above |

The 6–20 scale has 15 steps against 10, so 3 points on it is about the same share of
the scale as 2 points on a 10-point scale. Never compare ratings across scales.

| What the records show | Decision |
|---|---|
| Sessions completed as planned; effort ratings near the plan; no pain | Progress one thing next week |
| One or two sessions missed for life reasons; ratings normal | Hold: repeat the week |
| Ratings clearly higher than planned for the same sessions, for a week or more | Hold or reduce |
| Weekly mean HRV below the athlete's usual band, or weekly resting heart rate above it, for a week or more, with poor sleep | Reduce intensity for a few days and recheck; ask about illness, stress, travel |
| Any pain | Stop or hold that activity; advise a clinician or physiotherapist |
| Illness, chest pain, dizziness, fainting | Stop training; refer to a clinician |
| A test better than the last | Progress, and update paces |

How to use monitoring data:

- One low HRV day or one high resting-heart-rate day says little. Look at weekly
  means and several days in a row (Research: Plews et al. 2013, 2014).
- Never mix RMSSD and SDNN, or two devices, in one baseline.
- HRV, resting heart rate, and sleep move with many things. Name the candidates; the
  data cannot pick one.
- Compare session-effort loads only when they share the same scale, the same duration
  rule (whole session or main work), and the same delay after the session (from the
  calculation-methods notes, section 8).
- **Practice.** Do not use an acute-to-chronic load ratio, or any load threshold, to
  predict injury. No HYROX study supports one.

Commands:

```sh
ath apply training-load-effort --record <athlete-report-id> --json   # one session's load
ath apply hrv-change --record <hrv-record-id> --json                  # one day against recent days
ath apply resting-heart-rate-change --record <record-id> --json
ath apply sleep-change --record <sleep-session-id> --json
ath calculate summary hrv_rmssd --group week --timezone <zone> --json
ath calculate trend resting_heart_rate --from <date> --to <date> --timezone <zone> --json
ath apply heart-rate-zones --record <workout-session-id> --json
ath apply strength-volume --record <workout-session-id> --json
```

For weekly load, run `training-load-effort` on each session-effort report in the
week, add the loads, and show each one.

## Good and bad progression advice

**Bad:** "Add 10% to everything every week." No proven safe rate exists, and raising
running, sleds, and lunges together stacks leg load.

**Bad:** "Your HRV was low today, so skip training." One day says little.

**Good:** "Observed: you completed all five sessions, and your effort ratings were
within 1 point of the planned efforts on your 0–10 scale; your weekly resting heart
rate is in your usual range. Next week we add 2 km to the long run and keep everything
else the same. This step size is a coaching judgment."
