# Pacing and race budgets

Read this file when the athlete asks how fast to go, wants a target time, or needs a
race-day plan. A race budget is a planned time for each segment, with a range,
built from the athlete's own comparable evidence. It is a plan and a hypothesis, not
a prediction from population averages.

## What the evidence says

- **Research.** Run speed falls across HYROX races. In 24 PRO men it fell from
  16.1 km/h in run 1 to 13.6 km/h in run 8 (Gutiérrez-Hellín et al. 2026). Run 8
  slows most across large samples (Li et al. 2026).
- **Research.** Among top athletes, fade size was not clearly linked to finish time
  (Shi et al. 2026). There was no sign of a stable pacing trait within athletes.
- **Transferable.** For events longer than about 2 minutes, fairly even pacing may
  give the best times (Abbiss and Laursen 2008). In 13 triathletes, a fast first km
  of the run raised oxygen use, lactate, heart rate, and effort in that km (Skroce et
  al. 2022). In 13 CrossFit competitors, an all-out start gave round scores of 114,
  80, and 80; a controlled start gave 99, 98, and 97 (Ribeiro et al. 2024).
- **Practice.** A controlled run 1, a little slower than goal pace, then a steady
  middle race (HYROX research notes, sections 5.8 and 8).
- No HYROX pacing trial exists. First-run discipline below is coaching practice
  supported only by observation and other sports. Details are in
  [evidence.md](evidence.md#pacing-hrn-58).

## Evidence for a budget, best first

| Rank | Evidence | Why |
|---|---|---|
| 1 | An earlier race in the same format and division | Same loads, real Roxzone, real fatigue |
| 2 | A full simulation at race loads | Real order and fatigue; equipment and Roxzone differ |
| 3 | A half simulation or station-then-run sessions | Fatigue for part of the race |
| 4 | Fresh tests: 5 km run, 1 km repeats, single stations at race load | No race fatigue; needs larger adjustments |
| 5 | Population percentiles or other athletes' times | Not this athlete; use only to check a budget is not absurd |

Never build a budget from rank 5 alone.

## Building a budget

1. **List the evidence** with dates, record IDs, and what differed from race day:
   equipment, surface, target height, loads, and whether there was a Roxzone.
2. **Budget each run.** Use the athlete's own run pace after stations (coaches call
   this "compromised running": running while tired from a station), from rank 1–3
   evidence. Budget run 1 separately (see below).
3. **Budget each station.** Start from the most comparable time. Adjust for known
   differences, and say each adjustment and why.
4. **Budget Roxzone.** Use the athlete's own Roxzone time from an earlier race. If
   none exists, carry a wide placeholder and say it is not evidence.
5. **Give each segment a range.** Make it wider when the evidence is further from
   race conditions. Sled times from a gym and Roxzone with no race history get the
   widest ranges.
6. **Add up the central values for the total.** Give a total range narrower than
   the sum of every segment's best and worst case, because all segments rarely go
   best or worst on one day. Say the range is a judgment.
7. **State confidence** (low, moderate, high) and the two or three things that most
   affect it.
8. **Write a race-day card**: run 1 target, run band for runs 2–8, one cue per
   station, and what to do if behind or ahead.
9. **Save** the budget as a `coaching_note` with the evidence it used. To have the
   finish time graded after the race, also save a `prediction` with the tool skill's
   `references/predicting.md`.

## Run 1 discipline (Practice)

- Plan run 1 at the planned pace for runs 2–8, or a little slower. Never faster.
  Race start excitement and fresh legs make run 1 feel easy at a pace the athlete
  cannot hold.
- Treat a run 1 more than about 5% faster than the planned pace for runs 2–8 as a
  warning in review. This threshold is coaching judgment, not a tested value.
- In review, compare run 1 with the athlete's fresh 5 km pace. A run 1 faster than
  fresh 5 km pace is a strong sign of a fast start.

## Station pacing cues (Practice)

- SkiErg and row: settle into a split you could hold for twice the distance. The row
  can steady breathing before the farmers carry.
- Sleds: keep the sled moving. Plan short rests at the ends rather than stopping
  mid-lane.
- Burpee broad jumps: one steady rhythm from the start.
- Farmers carry: plan any put-downs before grip fails.
- Lunges: steady steps; short planned standing pauses beat slowing every rep.
- Wall balls: planned sets with short planned rests, decided in training.

## When evidence is thin

Thin evidence means no race and no full simulation in the format and division.

1. Say so first: "We do not have enough of your own data for a reliable finish
   time."
2. Give a pacing plan instead of a time target: run 1 at an effort the athlete
   could hold for 8 km, a run pace band from a recent 5 km test plus an
   adjustment you label as a hypothesis, and station effort cues.
3. Schedule evidence: a half simulation 5–6 weeks before the race and a full
   simulation 3–4 weeks before. Log each as a benchmark result under its own
   definition (see [records.md](records.md#benchmark-definitions-for-hyrox)).
4. If the athlete still wants a number, give a wide range with low confidence and
   list the assumptions. Do not tighten it to look precise.
5. After the race, replace the placeholders with the athlete's own splits.

## Fast Lane

"Racers running at a 4/km pace or faster are required to use the Fast Lane" (Singles
26/27 pp. 23–24). The unit is not printed; it most likely means 4 minutes per km.
If the budget has runs at or faster than 4:00 per km, tell the athlete to use the
Fast Lane.

Doubles and Relay pacing are in [team-formats.md](team-formats.md).

---

## Worked example: Athlete A's budget

Athlete A is invented. She is 29 and racing her first HYROX, Singles Women (Open), on
6 February 2027. Her 12-week plan is in [race-preparation.md](race-preparation.md).
On 16 January 2027, three weeks before the race, she did a full simulation in her
gym. It is logged under its own definition, because the gym sled, surface, and
target differ from the race.

```json ath-record-example
[
  {
    "record_type": "benchmark_definition",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a30",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "agent-harness", "model": "model-id" },
        "authored_at": "2027-01-16T08:00:00.000Z",
        "evidence": [],
        "content_version": 1,
        "status": "active"
      },
      "benchmark_key": "hyrox-sim-home-gym",
      "benchmark_type": "hyrox",
      "score_type": "time",
      "definition": "Full HYROX simulation in the athlete's gym, following the Women (Open) loads of the HYROX Rulebook Singles 26/27, p. 8, with substitutions: 8 x (1 km outdoor loop + station). SkiErg 1000 m and Rowing 1000 m on the gym machines. Sled Push 4 x 12.5 m on the gym's turf sled loaded to 102 kg including the sled. Sled Pull 4 x 12.5 m with a rope on the same sled at 78 kg. Burpee Broad Jumps 80 m. Farmers Carry 200 m with 2 x 16 kg. Lunges 100 m with a 10 kg backpack instead of a sandbag. Wall Balls 100 reps with 4 kg to a 2.74 m target. No Roxzone; the walk into each station is inside the run lap.",
      "tags": ["hyrox", "simulation", "singles", "open", "women"]
    }
  },
  {
    "record_type": "benchmark_result",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a31",
    "data": {
      "source_id": "01890f3e-e7d3-7cc0-88c8-000000000001",
      "received_at": "2027-01-16T12:00:00.000Z",
      "received_at_basis": "local_write_time",
      "benchmark_definition_id": "01890f3e-e7d3-7cc0-88c8-000000000a30",
      "recorded_at": "2027-01-16T09:00:00.000Z",
      "input_timestamp": {
        "timestamp_text": "2027-01-16T09:00:00.000Z",
        "utc_offset_text": "Z",
        "utc_offset_meaning": "documented_recording_offset",
        "timestamp_basis": "source_instant"
      },
      "score": { "duration_s": 5357 },
      "input_score_text": "1:29:17",
      "segments": [
        { "label": "run_1", "score": { "duration_s": 335 } },
        { "label": "ski_erg", "score": { "duration_s": 305 } },
        { "label": "run_2", "score": { "duration_s": 352 } },
        { "label": "sled_push", "score": { "duration_s": 220 } },
        { "label": "run_3", "score": { "duration_s": 365 } },
        { "label": "sled_pull", "score": { "duration_s": 330 } },
        { "label": "run_4", "score": { "duration_s": 358 } },
        { "label": "burpee_broad_jumps", "score": { "duration_s": 405 } },
        { "label": "run_5", "score": { "duration_s": 370 } },
        { "label": "row", "score": { "duration_s": 310 } },
        { "label": "run_6", "score": { "duration_s": 355 } },
        { "label": "farmers_carry", "score": { "duration_s": 130 } },
        { "label": "run_7", "score": { "duration_s": 372 } },
        { "label": "sandbag_lunges", "score": { "duration_s": 320 } },
        { "label": "run_8", "score": { "duration_s": 380 } },
        { "label": "wall_balls", "score": { "duration_s": 450 } }
      ]
    },
    "extensions": {
      "org.athleticstandard/hyrox-race": {
        "season": "2026/27",
        "rulebook": "HYROX Rulebook Singles 26/27",
        "format": "singles",
        "division": "open",
        "category": "women",
        "event": "simulation",
        "split_layout": "watch laps: 8 runs and 8 stations; each run lap includes the walk into the next station; no Roxzone"
      }
    }
  }
]
```

`hyrox-race-splits` on this record gives runs 48:07 (53.9%), stations 41:10
(46.1%), no Roxzone, and nothing unaccounted. Run 1 was 5:35; runs 2–8 averaged
about 6:05.

### Budget

**Observed** values come from the simulation. **Budget** values are the coach's
hypothesis for race day.

| Segment | Simulation | Budget | Range | Reason for the change and the range |
|---|---|---|---|---|
| Run 1 | 5:35 | 6:05 | 6:00–6:10 | A plan, not a forecast: run 1 at the planned pace for runs 2–8. In the simulation she ran it 30 s faster than her later runs |
| Runs 2–8, each | about 6:05 | 6:05 | 5:55–6:20 | Her own later-run pace; venue laps and crowds unknown |
| All runs | 48:07 | 48:40 | 47:30–50:00 | |
| SkiErg | 5:05 | 5:05 | 4:55–5:20 | Same machine type |
| Sled Push | 3:40 | 4:00 | 3:20–5:00 | Race sled and floor differ from the gym; widest station range |
| Sled Pull | 5:30 | 5:30 | 4:45–6:30 | Race sled and rope differ |
| Burpee Broad Jumps | 6:45 | 6:45 | 6:30–7:15 | Same movement |
| Rowing | 5:10 | 5:10 | 5:00–5:20 | Same machine type |
| Farmers Carry | 2:10 | 2:10 | 2:00–2:30 | Same load |
| Sandbag Lunges | 5:20 | 5:30 | 5:00–6:00 | A sandbag moves more than her backpack |
| Wall Balls | 7:30 | 7:40 | 7:00–8:30 | Target 2.70 m against her gym's 2.74 m; race-day fatigue |
| All stations | 41:10 | 41:50 | 40:00–45:00 | |
| Roxzone | none | 7:30 | 5:30–10:00 | No evidence of her own; a placeholder, not a forecast |
| **Total** | 1:29:17 without Roxzone | **1:38:00** | **1:33:00–1:45:00** | Confidence: low |

What most affects the range: the race sleds, the venue's Roxzone, and whether she
holds run 1 at 6:05. The total range is narrower than adding every segment's best or
worst case, because all segments rarely go best or worst on one day.

Race-day card:

- Run 1 at 6:05, at an effort that feels easier than it should. Watch pace can be
  wrong indoors, so go by effort. If run 1 ends faster than 5:55, settle run 2 into
  the band and do not try to bank time.
- Runs 2–8 between 5:55 and 6:20. Do not chase lost time on the runs.
- Sleds: keep moving, rest briefly at the ends.
- Wall balls: sets of 15, 15, 15, 15, 10, 10, 10, 10, the plan she tested in week 10.
- If more than 3 minutes behind the budget at the row, hold the run band and accept
  the total.

### Saved note

```json ath-record-example
[
  {
    "record_type": "coaching_note",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a32",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "agent-harness", "model": "model-id" },
        "authored_at": "2027-01-18T18:00:00.000Z",
        "evidence": [
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a31", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000a01", "revision": 1 }
        ],
        "content_version": 1,
        "status": "active"
      },
      "title": "Race budget for HYROX Singles Women (Open), 6 February 2027",
      "note": "Rules: HYROX Rulebook Singles 26/27. OBSERVED (gym simulation, 16 January 2027, with a turf sled, a 10 kg backpack for lunges, a 2.74 m target, and no Roxzone): runs 48:07, run 1 5:35, runs 2-8 about 6:05; stations 41:10. In the simulation run 1 was 30 s faster than her later runs. HYPOTHESIS (race budget, low confidence): run 1 6:05 (6:00-6:10), at the planned pace for runs 2-8, never faster; runs 2-8 5:55-6:20 each; runs total 48:40 (47:30-50:00); stations 41:50 (40:00-45:00), with sled push 4:00 (3:20-5:00) and wall balls 7:40 (7:00-8:30); Roxzone 7:30 (5:30-10:00) as a placeholder because she has no race data; total 1:38:00 (1:33:00-1:45:00). Race-day card: run 1 at 6:05 by effort; if it ends faster than 5:55, settle run 2 into the band; do not chase lost time on the runs; wall balls in sets of 15, 15, 15, 15, 10, 10, 10, 10. After the race, replace the placeholders with her own splits.",
      "applies_from": "2027-01-18",
      "applies_to": "2027-02-06"
    }
  }
]
```

## Good and bad budget answers

**Bad:** "Women in your age group average about 1:30, so aim for 1:30." This uses
other people's times as the only evidence.

**Bad:** "Your simulation was 1:29:17, so you will race 1:29." This ignores the
missing Roxzone, the different sleds, and the fast run 1.

**Good:** the budget above: each segment from her own simulation, each change
explained, a range that is wider where the evidence is weaker, and a placeholder
labelled as a placeholder.
