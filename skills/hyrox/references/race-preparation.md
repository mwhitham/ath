# Race preparation

Read this file when the athlete asks for a plan toward a HYROX race. It covers the
intake, how to lay out phases from now to race day, and how to save the plan. Session
content and progression rules are in [training-progression.md](training-progression.md).

## Step 1. Intake

Do not write a plan until you have these essentials. Read the records first, then
ask for what is missing, one numbered question at a time.

| Essential | Where to look first | If missing |
|---|---|---|
| Race date, format, division, category | Goals, notes | Ask |
| Pain, injury, illness, pregnancy, medical conditions | Athlete reports, notes | Ask. Any current issue goes to a clinician or physiotherapist first |
| Training days and minutes per week | Planned sessions, recent workouts | Ask |
| Current running: runs per week, weekly distance, longest run, a recent 5 km or similar | Workouts, benchmark results | Ask; offer a 5 km test in week 1 |
| Current strength and station experience | Strength sets, workouts, notes | Ask |
| HYROX history: races, simulations, splits | Benchmark results | Ask |
| Equipment: sleds (push and pull), SkiErg, rower, sandbag, kettlebells, wall ball and target height, space for burpee broad jumps and lunges | Notes | Ask |
| Other commitments: travel, shifts, other sports, class programming | Notes | Ask |

If the athlete wants something now, you may give a provisional outline. Label it
"provisional" and list each assumption. Save nothing until the essentials are known
and the athlete approves.

## Step 2. Fit phases to the time left

Count whole weeks from the plan start to race day. Use the row that matches.
These phase lengths are coaching practice, not tested values.

| Weeks to race | Base | Build | Race-specific | Taper | Notes |
|---|---|---|---|---|---|
| 16 or more | 6+ | 5–6 | 3 | 2 | Room for a lighter week every 3–4 weeks |
| 12–15 | 4 | 4 | 2 | 2 | The common case |
| 8–11 | 2–3 | 3 | 2 | 1–2 | Keep running increases small |
| 4–7 | 0–1 | 2 | 1–2 | 1 | Learn stations and pacing; little fitness change |
| under 4 | — | — | 1–2 | 1 | Rehearse, rest, set a safe pacing plan |

What each phase is for:

- **Base:** easy running volume, all eight stations learned at light load, general
  strength. One quality run a week.
- **Build:** threshold and race-pace running, stations at race load,
  station-then-run pairs, and a half simulation 5–6 weeks before the race.
- **Race-specific:** a full simulation 3–4 weeks before the race, race-pace
  combinations, and the race budget from the athlete's own evidence (see
  [pacing.md](pacing.md)).
- **Taper:** less volume, the same intensity, the same number of sessions.

Put a lighter week (about 20–30% less volume) every third or fourth week, and where
life is busy (travel, holidays, exams).

## Step 3. Fit sessions to the weekly time

| Weekly time | Sessions | Core of the week |
|---|---|---|
| About 3 h | 3 | One run with quality, one strength plus stations, one station-then-run or longer easy run |
| About 5 h | 4–5 | Two runs (one quality, one long easy), one strength, one station-then-run, one easy run or technique |
| About 7 h or more | 5–6 | Three runs, one or two strength, one station-then-run, one technique or easy aerobic session |

Rules for the week (coaching practice, with the reason):

- Keep hard runs and heavy leg work on different days, or at least 3 hours apart,
  because explosive strength gains were lower when both were done in the same
  session (Transferable: Schumann et al. 2022; Petré et al. 2021).
- Write the planned effort for each session in its block instructions, on the
  athlete's own rating scale, so the weekly review can compare it with the session
  effort they report (see
  [training-progression.md](training-progression.md#progress-hold-or-reduce)).
- Do not put two hard days back to back early in the plan.
- Keep most running easy. No single split of easy and hard running was clearly
  best in other sports, and none has been tested in HYROX (see
  [evidence.md](evidence.md#running-intensity-hrn-53)).

## Step 4. Adjust for the athlete

| Background | Usual starting point (Practice) |
|---|---|
| Runner, little strength work | Station technique and general strength first; keep running near current volume |
| Strength or CrossFit background, little running | Build running slowly; keep strength to maintenance plus station work |
| New to both | Easy running, technique for all stations, and one simple strength session |
| Returning from injury | Only with a clinician's clearance, and within their limits |

| Target | What changes |
|---|---|
| Singles Open | The common case |
| Singles Pro | Heavier sleds, carries, sandbag, and ball (see [rules-2026-27.md](rules-2026-27.md#divisions-and-loads)); more strength work |
| Doubles | Both partners run all 8 km together; practise change-overs (see [team-formats.md](team-formats.md)) |
| Relay | Short, faster legs; plan who does what (see [team-formats.md](team-formats.md)) |
| Adaptive | Only from the division HYROX confirmed (see [adaptive.md](adaptive.md)) |

## Step 5. Plan around equipment

List each station as available, substitutable, or missing. Use
[stations.md](stations.md) for substitutions, and write each one in the session's
block instructions. Plan at least one session on real race-type equipment (a HYROX
gym, an affiliate, or a training event) if the athlete can reach one before the full
simulation.

## Step 6. Write and save the plan

1. Show the phase summary and the weekly structure in plain words.
2. Mark what is observed, what is evidence, and what is your hypothesis.
3. Save a `training_plan`, a `goal`, and representative `planned_session` records
   (the next one or two weeks in full, plus key sessions such as simulations). Save
   later weeks when they come close.
4. Preview with `ath log --json`, show the athlete, and write with `ath log --yes`
   only after approval.
5. Review the plan each week with the athlete's records (see
   [training-progression.md](training-progression.md#progress-hold-or-reduce)).

---

## Worked example: Athlete A, first Singles Open race in 12 weeks

Athlete A is invented. The plan is written on 15 November 2026.

### Observed (from her records)

- Three runs a week for the last 6 weeks, about 24 km a week; longest run 10 km.
- 5 km time trial on 25 October 2026: 26:40, or 5:20 per km (record `…0a01`).
- Two gym sessions a week: squats, rows, and presses at moderate loads (workout
  `…0a02` is the most recent).
- Session effort reports on the 0–10 scale for most sessions.
- No HYROX race or simulation on record.

### What she told us (athlete reports and conversation)

- Race: HYROX Singles, Women (Open), Saturday 6 February 2027.
- Five training days and about 6 hours a week. She travels 24–28 December.
- Two years ago she had shin pain when running more than about 30 km a week
  (athlete report `…0a03`). No pain now.
- Gym equipment: SkiErg, rower, a push sled on a 15 m turf lane, 2 × 16 kg
  kettlebells, 4 kg and 6 kg wall balls, and one target line she measured at 2.74 m.
  No sandbag. No rope for the sled.

### Rules that apply (Singles 26/27 p. 8, p. 35)

Sled push 102 kg and sled pull 78 kg including the sled; farmers carry 2 × 16 kg;
sandbag lunges 10 kg; 100 wall balls with 4 kg to a 2.70 m target.

### Evidence used

- Running is about half of race time in Top 100 PRO results, and running rank
  tracks overall rank (Research: Rappelt et al. 2026). Strength stations cost slower
  athletes more (Research: Rappelt et al. 2026).
- Hard runs and heavy legs on different days, or at least 3 hours apart
  (Transferable: Schumann et al. 2022; Practice).
- A taper of about 2 weeks with 41–60% less volume and the same intensity and
  frequency (Transferable: Bosquet et al. 2007; Wang et al. 2023).
- Raise running gradually and stay below any distance that caused pain before
  (Practice; no study gives a safe weekly increase).

### Coach's hypotheses and choices

- Her running base is fair, so running grows slowly. Her shin pain began when she
  ran more than about 30 km a week, so weekly distance stays at or under 28 km,
  below that level. Station work adds leg load on top of running. This cap is a
  judgment, not a rule.
- Stations are new, so weeks 1–4 teach all eight at light load.
- Substitutions: lunges with a 10 kg backpack; she buys a rope for sled pulls; wall
  balls at her 2.74 m line, which is 4 cm above the race target.
- Race run pace starts as an estimate: her 5 km pace plus about 35 seconds per km,
  so about 5:55 per km. This is a guess to train with. The week-9 simulation
  replaces it.
- A full simulation in week 9 gives the evidence for her race budget. No time target
  until then.

### Weekly structure

| Day | Session |
|---|---|
| Monday | Rest |
| Tuesday | Quality run, then 15–20 min station technique |
| Wednesday | Easy run, 40–50 min |
| Thursday | Strength (legs and pulling) plus sled work |
| Friday | Rest |
| Saturday | Station-then-run session (the key HYROX session) |
| Sunday | Long easy run |

No two hard days are back to back: Tuesday (hard run) and Thursday (heavy legs) have
an easy day between them, and Saturday (station-then-run) is followed by an easy
long run. Friday rest follows the heavy leg day.

### Phases

| Weeks | Dates | Phase | Running per week | Key sessions |
|---|---|---|---|---|
| 1–3 | 16 Nov – 6 Dec | Base | 24, 25, 26 km | Tue intervals (6 × 3 min, then 5 × 4 min, then 4 × 6 min at about 10 km effort); Sat week 1: technique for all 8 stations and an easy run; Sat weeks 2–3: technique, then 2–3 station-then-run pairs |
| 4 | 7–13 Dec | Base, lighter | 22 km | Tue 5 km time trial; Sat fresh station checks: SkiErg 1,000 m, row 1,000 m, 100 wall balls |
| 5 | 14–20 Dec | Build | 26 km | Tue 5 × 1 km at about 5:55 per km; Sat 4 station-then-run pairs |
| 6 | 21–27 Dec | Build, travel | 20 km | Runs only, plus one bodyweight session while away |
| 7 | 28 Dec – 3 Jan | Build | 27 km | Tue 3 × 2 km at threshold; Sat 2 Jan half simulation (runs 5–8 with stations 5–8, 5 weeks before the race), logged under its own definition |
| 8 | 4–10 Jan | Build | 28 km | Tue 6 × 1 km at about 5:55 per km; Sat 5 pairs including lunges then run, and wall balls |
| 9 | 11–17 Jan | Race-specific | 26 km | Sat 16 Jan full simulation at planned race effort (3 weeks before the race), logged as a result |
| 10 | 18–24 Jan | Race-specific | 28 km | Race budget written; Tue 6 × 1 km at race pace; Sat race-pace pairs for stations 1–4 and a wall-ball set plan |
| 11 | 25–31 Jan | Taper | 18 km | Same sessions, about 35% less volume, the same intensity |
| 12 | 1–6 Feb | Taper and race | 12 km plus the race | Tue race-pace touch: 3 × (1 km + 25 wall balls); Thu 20 min easy with 3 × 1 min at race pace; Sat race |

Thursday strength: 3 × 8–10 at moderate loads in weeks 1–4; 4 × 5–6 heavier in
weeks 5–8; 2–3 sets to keep strength in weeks 9–10; one light session in week 11;
none in week 12.

Progress or hold each week using
[training-progression.md](training-progression.md#progress-hold-or-reduce). If any
shin or other pain appears, stop or hold that activity and advise her to see a
physiotherapist or clinician.

### Saved records

The batch saves the benchmark definition for her race, the plan, her goal, and four
representative sessions. The evidence is her 5 km result, her latest workout, and
her shin-history report.

```json ath-record-example
[
  {
    "record_type": "benchmark_definition",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a12",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "agent-harness", "model": "model-id" },
        "authored_at": "2026-11-15T19:00:00.000Z",
        "evidence": [],
        "content_version": 1,
        "status": "active"
      },
      "benchmark_key": "hyrox-singles-open-women",
      "benchmark_type": "hyrox",
      "score_type": "time",
      "definition": "HYROX Singles, Women (Open), from the HYROX Rulebook Singles 26/27, p. 8: 8 x (1 km run + station), in this order: SkiErg 1000 m; Sled Push 4 x 12.5 m at 102 kg including the sled; Sled Pull 4 x 12.5 m at 78 kg including the sled; Burpee Broad Jumps 80 m; Rowing 1000 m; Farmers Carry 200 m with 2 x 16 kg; Sandbag Lunges 100 m with 10 kg; Wall Balls 100 reps with 4 kg to a 2.70 m target. Score: official finish time, including time penalties.",
      "tags": ["hyrox", "singles", "open", "women", "2026/27"]
    }
  },
  {
    "record_type": "training_plan",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a10",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "agent-harness", "model": "model-id" },
        "authored_at": "2026-11-15T19:00:00.000Z",
        "evidence": [
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a01", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a02", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a03", "revision": 1 }
        ],
        "content_version": 1,
        "status": "active"
      },
      "title": "First HYROX Singles Women (Open): 12-week preparation",
      "objective": "Finish HYROX Singles Women (Open) on 6 February 2027 with no penalties and an even run pace, under the HYROX Rulebook Singles 26/27. Phases: base weeks 1-4 (week 4 lighter, with a 5 km test and fresh station checks); build weeks 5-8 (week 6 lighter for travel; half simulation on 2 January); race-specific weeks 9-10 (full simulation on 16 January, then a race budget); taper weeks 11-12. Five sessions a week: quality run (Tuesday), easy run (Wednesday), strength with sleds (Thursday), station-then-run (Saturday), long easy run (Sunday). Her shin pain began above about 30 km a week, so running stays at or under 28 km a week; this cap is a coaching judgment. Substitutions: 10 kg backpack for the sandbag; wall balls at her 2.74 m target. Assumptions: about 6 hours a week, no current pain, travel 24-28 December.",
      "start_date": "2026-11-16",
      "end_date": "2027-02-06"
    }
  },
  {
    "record_type": "goal",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a11",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "agent-harness", "model": "model-id" },
        "authored_at": "2026-11-15T19:00:00.000Z",
        "evidence": [
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a01", "revision": 1 }
        ],
        "content_version": 1,
        "status": "active"
      },
      "title": "Finish the first HYROX Singles Women (Open) race with no penalties",
      "description": "No time target yet. A race budget will be set from her own full simulation on 16 January 2027.",
      "target_date": "2027-02-06",
      "target_benchmark_definition_id": "01890f3e-e7d3-7cc0-88c8-000000000a12"
    }
  },
  {
    "record_type": "planned_session",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a21",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "agent-harness", "model": "model-id" },
        "authored_at": "2026-11-15T19:00:00.000Z",
        "evidence": [
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a02", "revision": 1 }
        ],
        "content_version": 1,
        "status": "active"
      },
      "training_plan_id": "01890f3e-e7d3-7cc0-88c8-000000000a10",
      "title": "Week 1: station technique and easy run",
      "scheduled_date": "2026-11-21",
      "activity_type": "hyrox_training",
      "blocks": [
        { "block_type": "warm-up", "title": "Easy jog and mobility", "instructions": "Planned effort for the whole session: 4 of 10 (0-10 scale).", "planned_duration_s": 900 },
        { "block_type": "station", "title": "SkiErg technique", "instructions": "4 x 250 m at an easy effort, 1 min rest. Hinge at the hips; finish with hands past the hips.", "planned_distance_m": 1000 },
        { "block_type": "station", "title": "Sled push technique", "instructions": "4 x 12.5 m at a light load. Low body angle, short quick steps. Whole sled past the line before turning.", "planned_distance_m": 50 },
        { "block_type": "station", "title": "Sled pull technique", "instructions": "4 x 12.5 m with the new rope on the turf sled at a light load. Stay standing in the box; pile the rope behind you. If the rope has not arrived, do hand-over-hand rope pulls on a cable machine (a substitution with no sled friction).", "planned_distance_m": 50 },
        { "block_type": "station", "title": "Burpee broad jump technique", "instructions": "4 x 10 m. Chest on the floor each rep, feet together on take-off and landing, no shuffle steps.", "planned_distance_m": 40 },
        { "block_type": "station", "title": "Row technique", "instructions": "4 x 250 m easy. Legs, body, arms.", "planned_distance_m": 1000 },
        { "block_type": "station", "title": "Farmers carry", "instructions": "4 x 50 m with one 16 kg kettlebell in each hand. Tall posture, short steps.", "planned_distance_m": 200, "planned_weight_kg": 16 },
        { "block_type": "station", "title": "Lunges with a 10 kg backpack", "instructions": "Substitution for the 10 kg sandbag. 4 x 12.5 m. Back knee touches the floor gently; stand fully tall each rep.", "planned_distance_m": 50, "planned_weight_kg": 10 },
        { "block_type": "station", "title": "Wall balls", "instructions": "5 x 10 with 4 kg to the 2.74 m line. Hips below knees; start each set standing tall.", "planned_reps": 50, "planned_weight_kg": 4 },
        { "block_type": "run", "title": "Easy run", "instructions": "Conversational pace.", "planned_duration_s": 1200 }
      ],
      "completion_status": "planned"
    }
  },
  {
    "record_type": "planned_session",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a22",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "agent-harness", "model": "model-id" },
        "authored_at": "2026-11-15T19:00:00.000Z",
        "evidence": [
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a01", "revision": 1 }
        ],
        "content_version": 1,
        "status": "active"
      },
      "training_plan_id": "01890f3e-e7d3-7cc0-88c8-000000000a10",
      "title": "Week 5: station-then-run pairs",
      "scheduled_date": "2026-12-19",
      "activity_type": "hyrox_training",
      "blocks": [
        { "block_type": "warm-up", "title": "Easy jog, drills, two light station rehearsals", "instructions": "Planned effort for the whole session: 7 of 10 (0-10 scale).", "planned_duration_s": 900 },
        { "block_type": "pair", "title": "SkiErg 500 m, then 1 km run", "instructions": "Run at about 5:55 per km. Walk 2 min after the run." },
        { "block_type": "pair", "title": "Sled push 2 x 12.5 m at race load, then 1 km run", "instructions": "Load the turf sled to 102 kg including the sled. Run at about 5:55 per km. Walk 2 min." },
        { "block_type": "pair", "title": "Burpee broad jumps 40 m, then 1 km run", "instructions": "Steady rhythm. Run at about 5:55 per km. Walk 2 min." },
        { "block_type": "pair", "title": "Wall balls 50, then 1 km run", "instructions": "4 kg to the 2.74 m line in sets of 15, 15, 10, 10. Run at about 5:55 per km." },
        { "block_type": "cool-down", "title": "Easy walk and jog", "planned_duration_s": 600 }
      ],
      "completion_status": "planned"
    }
  },
  {
    "record_type": "planned_session",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a23",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "agent-harness", "model": "model-id" },
        "authored_at": "2026-11-15T19:00:00.000Z",
        "evidence": [
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a01", "revision": 1 }
        ],
        "content_version": 1,
        "status": "active"
      },
      "training_plan_id": "01890f3e-e7d3-7cc0-88c8-000000000a10",
      "title": "Week 9: full race simulation",
      "scheduled_date": "2027-01-16",
      "activity_type": "hyrox_simulation",
      "blocks": [
        { "block_type": "warm-up", "title": "Race warm-up", "instructions": "The warm-up she will use on race day. Planned effort for the whole session: 9 of 10 (0-10 scale).", "planned_duration_s": 1200 },
        { "block_type": "run", "title": "8 x 1 km, one before each station", "instructions": "Runs 2-8 at about 5:55 per km, her current estimate. Run 1 at that pace or a little slower, never faster. Lap the watch at each run and station.", "planned_distance_m": 8000 },
        { "block_type": "station", "title": "SkiErg", "planned_distance_m": 1000 },
        { "block_type": "station", "title": "Sled push", "instructions": "4 x 12.5 m on the turf sled at 102 kg including the sled.", "planned_distance_m": 50, "planned_weight_kg": 102 },
        { "block_type": "station", "title": "Sled pull", "instructions": "4 x 12.5 m with the rope at 78 kg including the sled.", "planned_distance_m": 50, "planned_weight_kg": 78 },
        { "block_type": "station", "title": "Burpee broad jumps", "planned_distance_m": 80 },
        { "block_type": "station", "title": "Row", "planned_distance_m": 1000 },
        { "block_type": "station", "title": "Farmers carry", "instructions": "One 16 kg kettlebell in each hand.", "planned_distance_m": 200, "planned_weight_kg": 16 },
        { "block_type": "station", "title": "Lunges with a 10 kg backpack", "instructions": "Substitution for the sandbag.", "planned_distance_m": 100, "planned_weight_kg": 10 },
        { "block_type": "station", "title": "Wall balls", "instructions": "4 kg to the 2.74 m line. Log the set breakdown.", "planned_reps": 100, "planned_weight_kg": 4 },
        { "block_type": "cool-down", "title": "Easy walk", "planned_duration_s": 600 }
      ],
      "completion_status": "planned"
    }
  },
  {
    "record_type": "planned_session",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a24",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "agent-harness", "model": "model-id" },
        "authored_at": "2026-11-15T19:00:00.000Z",
        "evidence": [],
        "content_version": 1,
        "status": "active"
      },
      "training_plan_id": "01890f3e-e7d3-7cc0-88c8-000000000a10",
      "title": "Week 12: first race-pace touch",
      "scheduled_date": "2027-02-02",
      "activity_type": "hyrox_training",
      "blocks": [
        { "block_type": "warm-up", "title": "Easy jog and drills", "instructions": "Planned effort for the whole session: 5 of 10 (0-10 scale).", "planned_duration_s": 900 },
        { "block_type": "pair", "title": "3 x (1 km at race pace + 25 wall balls)", "instructions": "Race pace from her race budget. Wall balls with 4 kg to the 2.74 m line. 3 min walk between rounds. Finish feeling she could do more." },
        { "block_type": "cool-down", "title": "Easy jog", "planned_duration_s": 600 }
      ],
      "completion_status": "planned"
    }
  }
]
```

The week-12 session has an empty evidence list because no record informed its
content beyond the plan itself; its pace comes from the race budget written in week
10.
