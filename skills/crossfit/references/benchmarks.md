# Benchmarks

A benchmark is a named workout repeated over time to measure change. This file
gives the official definitions, the rules for comparing attempts, and how to record
a score so a later comparison is honest. "Rx" means the workout as prescribed.

## Official definitions and loads

All rows are **Official**, from the crossfit.com benchmark pages read on
2026-11-15. The kilogram figures come from the conversion table in the 2026
CrossFit Games Rulebook, Appendix D. They are not on the benchmark pages.

| Name | Workout as prescribed | Men | Women | Scored by |
|---|---|---|---|---|
| Fran | 21-15-9 reps for time: thrusters, pull-ups | 95 lb (43 kg) | 65 lb (29 kg) | Time |
| Grace | 30 clean and jerks for time | 135 lb (61 kg) | 95 lb (43 kg) | Time |
| Helen | 3 rounds for time: 400 m run, 21 kettlebell swings, 12 pull-ups | 53 lb (24 kg) | 35 lb (16 kg) | Time |
| Diane | 21-15-9 reps for time: deadlifts, handstand push-ups | 225 lb (102 kg) | 155 lb (70 kg) | Time |
| Elizabeth | 21-15-9 reps for time: cleans, ring dips | 135 lb (61 kg) | 95 lb (43 kg) | Time |
| Isabel | 30 snatches for time | 135 lb (61 kg) | 95 lb (43 kg) | Time |
| Karen | 150 wall-ball shots for time | 20 lb ball to 10 ft | 14 lb ball to 9 ft | Time |
| Annie | 50-40-30-20-10 reps for time: double-unders, sit-ups | Body weight | Body weight | Time |
| Cindy | As many rounds and reps as possible in 20 minutes: 5 pull-ups, 10 push-ups, 15 squats | Body weight | Body weight | Rounds and reps |
| Jackie | For time: 1,000 m row, 50 thrusters, 30 pull-ups | 45 lb (20 kg) | 35 lb (15 kg) | Time |
| Nancy | 5 rounds for time: 400 m run, 15 overhead squats | 95 lb (43 kg) | 65 lb (29 kg) | Time |
| Murph | For time: 1 mile run, 100 pull-ups, 200 push-ups, 300 air squats, 1 mile run. "Partition the pull-ups, push-ups, and squats as needed." | 20 lb vest "if you've got" one | 14 lb vest if available | Time |
| Linda | 10 down to 1 reps for time: deadlift 1½ × body weight, bench press 1 × body weight, squat clean ¾ × body weight | By body weight | By body weight | Time |
| Amanda | 9-7-5 reps for time: muscle-ups, snatches | 135 lb (61 kg) | 95 lb (43 kg) | Time |
| Kelly | 5 rounds for time: 400 m run, 30 box jumps, 30 wall-ball shots | 24 in box, 20 lb ball | 20 in box, 14 lb ball | Time |
| DT | 5 rounds for time: 12 deadlifts, 9 hang power cleans, 6 push jerks | 155 lb (70 kg) | 105 lb (47 kg) | Time |
| Fight Gone Bad | 3 rounds, 1 minute each: wall-ball shots, sumo deadlift high pulls, box jumps, push presses, row for calories; 1 minute rest | 20 lb ball to 10 ft; 75 lb (34 kg); 20 in box | 14 lb ball to 9 ft; 55 lb (25 kg); 20 in box | Total reps |
| Filthy 50 | For time, 50 each: box jumps, jumping pull-ups, kettlebell swings, walking-lunge steps, knees-to-elbows, push presses, back extensions, wall-ball shots, burpees, double-unders | 24 in box, 16 kg bell, 45 lb press, 20 lb ball | 20 in box, 12 kg bell, 35 lb press, 14 lb ball | Time |

Murph and DT are Hero workouts. Source URLs are in [sources.md](sources.md).

### Caveats found in the sources

- **Official.** Grace: "The power clean and jerk variation is the intended
  variation" (crossfit.com Grace page).
- **Official.** Elizabeth: the 2003 CrossFit Journal lists "Clean 135 lbs" but its
  text says hang cleans. The current page says cleans. The clean style is not fixed.
- **Official.** Linda: the current page's text has a typo about the clean load.
  Its prescription box and the 2004 CrossFit Journal both give ¾ × body weight.
- **Official.** Murph: the vest is conditional. Partitioning is allowed as
  prescribed. The page also lists an intermediate version with half reps.
- **Official.** The 2003 and 2004 CrossFit Journal articles give only one load,
  the men's. The women's loads come from the current pages.
- **Official.** The Fran, Grace, Diane, Helen, Karen, Jackie, Nancy, Murph, and
  Linda pages also list intermediate and beginner versions with their own loads.
  Example: Fran intermediate is thrusters at 75/55 lb with 12-9-6 pull-ups.
- **Not confirmed.** How Fight Gone Bad counts row calories; wall-ball target
  heights for Kelly and Filthy 50; snatch style (squat or power) for Isabel and
  Amanda; the sit-up type for Annie. The pages do not say. Record what the athlete
  used in `standards`.
- **Not confirmed.** First-posted dates are approximate. The Jackie page and the
  2004 CrossFit Journal disagree.

### What `ath init` seeds

`ath init` creates seven benchmark definitions: `fran`, `grace`, `helen`, `murph`,
`5k-run`, `1-mile-run`, and `hyrox-full`. The CrossFit four match the official
loads above. The seeded `murph` text does not mention the vest, so record a vest in
the `loads` of the extension. `hyrox-full` belongs to the `hyrox` skill.

Before creating a definition, check what exists:

```sh
ath records list --type benchmark_definition --limit 10000 --json
```

Reuse a definition whose text matches the workout exactly. For any other workout,
create a `benchmark_definition` first (see [records.md](records.md)).
One definition holds one exact workout. A different version, such as Helen with a
12 kg kettlebell, is the same definition with the change written in the extension,
or a separate custom definition if the athlete wants its own history.

## When two attempts can be compared

**Official.** The work in a benchmark stays the same only while load, range of
motion, height, and body weight stay the same. Then "the difference in time is the
difference in power produced" (L1 p. 36). A Fran time that drops from 4:30 to 2:45
is about a 60% rise in power (L1 p. 35). Progress means "faster with the same
weight or at a similar time with increased weight" (crossfit.com Fran page).

Check each row. The worst row decides.

| What to check | Same | Small difference | Different |
|---|---|---|---|
| Version (`variant`) | Same tier | | Rx against scaled, scaled against foundations, or a different modified version |
| Loads | Same | Under 5% apart, written down | 5% or more apart |
| Movement standard | Same | | Kipping against strict, chest-to-bar against chin over bar, different wall-ball target, band against no band |
| Reps and distances | Same | | Any change |
| Scoring | Both finished, or both capped at the same cap | | One finished and one capped, or different caps |
| Conditions | Same session position, similar time of day | Different time of day or after a strength part | After a hard day against fresh, or ill |
| Body weight (Linda, Murph with vest) | Within about 2% | Within about 5% | More |
| Validation | Both self-scored, or both judged | One judged | |

Outcomes:

- **All "Same"**: directly comparable. Report the change in time, or reps, or load.
- **Any "Small difference"**: comparable with a caveat. Name the difference in the
  same sentence as the change.
- **Any "Different"**: not comparable. Report each attempt on its own. Suggest a
  retest that matches the earlier one.

The thresholds of 2% and 5% are **Practice**. They are a judgment call, not a rule.

### Why a faster scaled time is not a personal best

A scaled attempt moves less load, or fewer reps, or an easier version. The work is
different, so the times measure different things (L1 p. 36). **Official.** In the
Open, scaled athletes rank below every athlete who did the workout as prescribed
(2026 Rulebook §1.29). The tiers are separate, not points on one scale.

Example. Helen in October: 12:48 with a thin band on the pull-ups. Suppose Helen in
December is 13:20 with no band. The December time is slower. It is still the first
prescribed Helen in the record. It is a new baseline, not a worse result.

## Capped results

A time cap ends a "for time" workout at a fixed time.

- **Record.** When the cap ends the attempt, `score.reps` holds the reps completed,
  `capped` is `true`, and `time_cap_s` holds the cap. `score` has no `duration_s`.
  Never write the cap as the time.
- **Zero reps.** An attempt with no completed reps is not a benchmark score. Log it
  as a note (D249).
- **Official.** Open scorecards in 2026 rank capped athletes by reps, and break ties
  on reps by tiebreak time. "Athletes who complete the workout under the time cap
  will not enter a tiebreak score" (Open 26.1 and 26.3 scorecards).
- **Official.** For teams, a capped score is total reps, ranked "below any team
  whose score is a valid time and above any team that completes fewer reps"
  (2026 Rulebook §1.31).
- **Practice**, derived from the scorecards and the team rule. For individuals: any
  finish ranks above any capped score; capped scores rank by reps, then by tiebreak
  time. The 2026 rulebook has no single sentence stating this for individual Open
  athletes.
- **Practice.** No 2026 rule turns a capped score into a time, such as one second
  per missing rep. Do not invent an estimated finish time.

Comparing two training attempts:

- Finished against finished: compare times.
- Capped against capped, same cap: compare reps. With equal reps, compare tiebreak
  times if both were recorded.
- Finished against capped, same version: the finish is better.
- Different caps: not comparable as scores. Compare reps at a shared point only if
  the athlete recorded split times.

## Tiebreaks

A tiebreak time is the clock time at a set point in a workout, such as the end of
a round. **Official.** Each Open workout sets its own tiebreak point. In 26.1 it was
after each set of box jump-overs and step-overs; in 26.3, after each set of
thrusters (2026 scorecards). Record it in `tiebreak_s` as seconds from the start.
It matters only in competition, or when two attempts end on the same reps.

## Recording a score

A plain prescribed time may skip the extension. It can be logged as text:
`ath log Fran in 4:41 rx`. Add the `org.athleticstandard/crossfit-score` extension
when the attempt differs from the definition, was capped, was a competition score,
or when the standard left a choice, such as pull-up style. The
field table and complete examples are in [records.md](records.md).

Minimum to record, by case:

| Case | `variant` | Also record |
|---|---|---|
| As prescribed, finished | `rx`, or no extension | Add the extension with `loads` and `standards` only when the standard left a choice, such as pull-up style |
| Official scaled version | `scaled` | Which version in `standards`, and `loads` |
| Open Foundations tier | `foundations` | `loads`, because Foundations loads may be changed |
| Any other change | `modified` | Every change in `substitutions` or `loads`, with the reason |
| Capped | as above | `time_cap_s`, `capped: true`, reps in `score.reps` |
| Competition | as above | `competition`, `judged`, `tiebreak_s` |

## Procedure: analyse a benchmark

1. List every attempt of the benchmark:
   `ath records list --type benchmark_result --limit 10000 --json`, then
   `ath records show <id> --json` for each. Read `scaling` and the extension.
2. Group attempts by version and standard.
3. Inside each group, check the comparison table above.
4. For comparable pairs, compute the change: seconds and percent for time, reps for
   reps, kilograms for load. Faster is better for time; more is better for reps and
   load.
5. Look for other explanations: time of day, the session it followed, sleep the
   night before, illness or pain notes, a judged attempt against a self-scored one.
6. Report with labels: **Observed** (the attempts and the change), **Evidence** (the
   comparability rule and its source), **Hypothesis** (why it changed).
7. Suggest the next attempt that would be comparable, and when.

## How to use norms

- **Research.** The only large norms are from 10,000 public profiles collected in
  2017. In the individual (non-masters, non-teen) class, mean Fran was 250 ± 106
  seconds for men and 331 ± 181 seconds for women; mean Helen was 9.5 ± 1.9 and
  11.1 ± 2.4 minutes (Mangine et al., 2018).
  Scores were self-reported, unverified, and scaled entries could not be separated.
- **Practice.** Use norms only as rough context. Never set a goal from them.
  Compare the athlete with their own earlier attempts.
- **Research.** In 14 competitive athletes, a strength total (back squat, press,
  deadlift) predicted Grace and Fran times; nothing predicted Cindy (Butcher et al.,
  2015). The sample is small.

## How often to retest

- **Official.** Bookend targeted blocks of six to eight weeks with tests. Judge the
  program by "repeating benchmarks and comparing the results to previous scores"
  (O'Connor, 2023).
- **Practice.** Retest a benchmark every 6 to 12 weeks. A retest every week mostly
  measures day-to-day variation.
