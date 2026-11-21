# Post-race review

Read this file after a race or full simulation, when the athlete wants to know what
to change. A review sets a few priorities, gives more than one explanation for each
weak part, and names training that would test each explanation. It is saved as a
`saved_training_review` and a `coaching_note`.

## Procedure

1. **Collect.** Run [race-analysis.md](race-analysis.md) steps 1–2. Read the
   athlete's notes and effort rating for the race, and the training records of the
   block before it (sessions, session-effort loads, any run or station tests).
   Check sleep, HRV, and resting heart rate in the week before, with
   `sleep-change`, `hrv-change`, and `resting-heart-rate-change`, if the athlete
   has that data.
2. **Compare with the plan.** If a race budget exists (see [pacing.md](pacing.md)),
   put each segment next to its budget. Note where the athlete was more than the
   planned range away.
3. **Pick at most three priorities.** Rank by time that could be won back and by
   how confident you are. A segment that took many minutes and has a clear
   cause comes first. A large share alone is not a priority if it was on budget.
4. **List competing explanations for each priority.** Use the checklist below.
   Name at least two for every weak part. Say which records support or weaken each.
5. **Name a test for each explanation.** A test is training or a check that would
   give a different answer depending on which explanation is true.
6. **Write recommendations.** Few and specific: what changes, for how long, and
   when it will be checked.
7. **Save** a `saved_training_review` and a `coaching_note`, after the athlete
   approves the preview.

## Competing explanations checklist

| Explanation | Signs in the records | A test that would separate it |
|---|---|---|
| Fitness: running capacity | Runs slow from run 2 on, fresh run tests also slow | A 5 km or 3 × 2 km test; compare race run pace with fresh pace |
| Fitness: station capacity | The station is slow fresh too | A fresh, timed station test at race load |
| Pacing | Run 1 much faster than planned or than fresh test pace; later parts fall away | Repeat a simulation with run 1 capped; compare the later parts |
| Technique | No-reps, warnings, an awkward pattern on video | Film a set; count faults a judge would call |
| Equipment and standards | Gym target, ball, sled, or surface differs from race | Measure the gym equipment; train at the race standard; re-test |
| Penalties | `penalties_seconds` above zero | Read the penalty reason; train that standard |
| Roxzone congestion or venue | Long Roxzone, waiting for equipment, a busy wave | Ask the athlete; compare only within the same event; little to train |
| Split layout | Run 1 or one station unusually short or long | Check what the provider's field contains |
| Health, sleep, or life load | Low HRV or high resting heart rate in race week, poor sleep, illness | Check the records; refer health questions to a clinician |
| Fuel and drink | Late fade, the athlete reports feeling empty | Rehearse race-day food and drink in the next simulation |

Do not diagnose an injury or illness. If the athlete reports pain, dizziness, chest
pain, or illness during or after the race, advise them to see a qualified clinician
before the next hard session.

## Good and bad review statements

**Bad:** "Your wall balls are your weakness. Do 300 wall balls a week." One cause,
no test, and a large jump in volume.

**Bad:** "Your runs faded because your fitness is poor." A fade has several causes,
and among top athletes fade size was not clearly linked to finish time (Shi et al.
2026).

**Good:** "Observed: wall balls took 11:30, 11.9% of your time, and you reported
no-reps for depth. Hypotheses: fatigue from a fast start, wall-ball capacity,
technique, or a lower target in your gym. Tests: a fresh 100 wall balls for time,
the same after lunges and a run, a filmed set, and measuring your gym target."

---

## Worked example: Athlete B

Athlete B is invented. He is a 34-year-old man who raced his second HYROX, Singles
Men (Open), on 24 October 2026. The result is logged as shown in
[records.md](records.md#a-complete-race-result) (record `…0b02`, revision 1). His
notes are athlete reports `…0b03` and `…0b06`.

### Splits (calculation output)

```json
{
  "total_seconds": 5809, "runs_seconds": 2651, "stations_seconds": 2685,
  "roxzone_seconds": 470, "penalties_seconds": 0, "unaccounted_seconds": 3,
  "share_percent": { "runs": 45.6, "stations": 46.2, "roxzone": 8.1, "penalties": 0, "unaccounted": 0.1 },
  "runs": [
    { "label": "run_1", "seconds": 250 }, { "label": "run_2", "seconds": 318 },
    { "label": "run_3", "seconds": 341 }, { "label": "run_4", "seconds": 332 },
    { "label": "run_5", "seconds": 352 }, { "label": "run_6", "seconds": 330 },
    { "label": "run_7", "seconds": 356 }, { "label": "run_8", "seconds": 372 }],
  "run_summary": { "count": 8, "mean_seconds": 331.4,
    "fastest": { "label": "run_1", "seconds": 250 }, "slowest": { "label": "run_8", "seconds": 372 },
    "spread_seconds": 122, "first_run_seconds": 250, "later_runs_mean_seconds": 343 },
  "stations": [
    { "label": "ski_erg", "seconds": 285, "share_percent": 4.9 },
    { "label": "sled_push", "seconds": 225, "share_percent": 3.9 },
    { "label": "sled_pull", "seconds": 330, "share_percent": 5.7 },
    { "label": "burpee_broad_jumps", "seconds": 380, "share_percent": 6.5 },
    { "label": "row", "seconds": 300, "share_percent": 5.2 },
    { "label": "farmers_carry", "seconds": 140, "share_percent": 2.4 },
    { "label": "sandbag_lunges", "seconds": 335, "share_percent": 5.8 },
    { "label": "wall_balls", "seconds": 690, "share_percent": 11.9 }],
  "season": "2026/27", "format": "singles", "division": "open"
}
```

### Observed

- Finish 1:36:49. Runs 44:11 (45.6%), stations 44:45 (46.2%), one Roxzone total of
  7:50 (8.1%). Three seconds are unaccounted, most likely rounding in the
  provider's splits.
- No penalties were listed when he checked, 48 hours after the event.
- Run 1 took 4:10. Runs 2–8 averaged 5:43, which is 93 seconds slower per run.
  Run 8 was the slowest at 6:12.
- His 5 km time trial on 26 September (record `…0b04`) was 21:30, which is 4:18 per
  km. Run 1 was faster than his fresh 5 km pace.
- Wall balls took 11:30 for 100 reps, the largest single part (11.9%). Sandbag
  lunges, just before run 8, took 5:35.
- Workout records for the 8 weeks before the race show wall balls in 2 of 31
  sessions (`…0b07` and `…0b08`), both fresh sets of 20.
- He reported (athlete report, not a measurement): "Run 1 felt easy. Legs were gone
  after the lunges. At wall balls the judge called several no-reps for depth and I
  lost count." His session effort for the race was 10 out of 10.
- His gym's wall-ball target is a single line he measured at 2.74 m. The men's
  target is 3.00 m (Singles 26/27 p. 35).

### Evidence

- Run speed falls across HYROX races, and run 8 slows most (Research: Gutiérrez-Hellín
  et al. 2026, 24 PRO men; Li et al. 2026, 61,541 records).
- Among top athletes, fade size was not clearly linked to finish time (Research: Shi
  et al. 2026). So a fade alone does not prove a fitness or pacing problem.
- Slower athletes lose relatively more time at strength stations, and wall balls
  are one of the largest single sources of differences between athletes
  (Research: Rappelt et al. 2026; Yustres Amores et al. 2026, PRO men).
- For events longer than about 2 minutes, more even pacing may improve times
  (Transferable: Abbiss and Laursen 2008). No HYROX pacing trial exists.
- A wall-ball rep needs hips below knees and a hit on the strike zone; there are no
  warnings, only no-reps (Rule: Singles 26/27 pp. 34–36).

### Priorities

1. Pacing of run 1, because it may have cost time in every later part.
2. Wall balls, because 11:30 is the largest single part.
3. Runs 6–8, which follow the row, farmers carry, and lunges.

### Where the 5:15 run pace comes from (Hypothesis)

His eight runs took 44:11. A steadier start could plausibly bring them to about
42:00: run 1 about a minute slower than his 4:10, and runs 2–8 about 28 seconds
faster than their 5:43 average. 42:00 ÷ 8 = 5:15 per run. Under the run 1 rule in
[pacing.md](pacing.md#run-1-discipline-practice), run 1 goes at 5:15 or a little
slower, never faster. The 42:00 is a coaching judgment to test, not a forecast.

### Competing explanations and tests

**Wall balls (11:30)**

| Hypothesis | For | Against | Test |
|---|---|---|---|
| Fatigue from the fast start | Run 1 faster than 5 km pace; legs "gone" after lunges | No fresh 100-rep time to compare with | Full simulation on 5 December with run 1 capped at 5:15; compare wall balls |
| Wall-ball capacity | 2 wall-ball sessions in 8 weeks, sets of 20 only | Unknown until the fresh test | Fresh 100 wall balls for time, judged, this week |
| Technique and no-reps | He reported no-reps for depth | Count unknown | Film a set of 30 with a partner judging depth; count no-reps |
| Equipment mismatch | Gym target 2.74 m, 26 cm below the 3.00 m race target | How much it slows him is unknown until he throws to 3.00 m | Do the fresh test at a marked 3.00 m line; train there for 4 weeks, then re-test |
| Venue or waiting for a target | None recorded | He did not mention it | Ask him; it cannot be trained |

How to read the tests (Hypothesis): if the fresh 100 takes under about 5:30 but the
same set after lunges and a run takes over 8:00, fatigue and pacing explain more.
If the fresh 100 is also slow, capacity or technique explain more. The cut-offs are
coaching judgment for this athlete, not tested thresholds.

**Run fade (run 1 4:10, runs 2–8 mean 5:43)**

| Hypothesis | For | Against | Test |
|---|---|---|---|
| Pacing: run 1 too fast | Faster than fresh 5 km pace | Some fade is normal | Run 1 capped at 5:15 in the full simulation on 5 December |
| Running capacity under fatigue | Runs 6–8 all 5:30 or slower | Fresh 5 km is reasonable | Weekly station-then-run pairs; compare run splits over 6 weeks |
| Split layout | Run 1 has no station before it | Layout gave one Roxzone total | Check whether the provider's run 1 includes the start area |

### Recommendations (for 27 October – 6 December 2026)

1. Plan runs at 5:15 (42:00 of running), with run 1 at 5:15 or a little slower,
   never faster, in every full simulation and the next race.
2. Fresh 100 wall balls for time in week 1. Then wall balls twice a week, at 6 kg to a
   marked 3.00 m line, building from fresh sets to sets after lunges and a run.
3. Film one wall-ball set each fortnight and count depth faults.
4. One station-then-run session a week, using lunges then a 1 km run then wall
   balls.
5. A full simulation on 5 December, six weeks after the race, with run 1 capped at
   5:15. Compare wall balls and runs 6–8 with the race. A half simulation cannot
   test run 1 pacing, because it has no run 1.

### Saved records

The review cites the race, his notes, the 5 km result, his effort rating, and the
two workouts with wall balls. All 31 workout records of the block were read to count
the wall-ball sessions; only those two are listed as evidence, and the summary says
so. IDs for the review and note are new.

```json ath-record-example
[
  {
    "record_type": "saved_training_review",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b10",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "agent-harness", "model": "model-id" },
        "authored_at": "2026-10-27T18:00:00.000Z",
        "evidence": [
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b02", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b03", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b04", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b06", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b07", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b08", "revision": 1 }
        ],
        "content_version": 1,
        "status": "active"
      },
      "period_start": "2026-08-31",
      "period_end": "2026-10-24",
      "summary": "Race: HYROX Singles Men (Open), 24 October 2026, rules from the HYROX Rulebook Singles 26/27. Split layout: 8 runs, 8 stations, one Roxzone total. OBSERVED: finish 1:36:49; runs 44:11 (45.6%), stations 44:45 (46.2%), Roxzone 7:50 (8.1%), 3 s unaccounted (most likely rounding), no penalties after 48 hours. Run 1 4:10, faster than his fresh 5 km pace of 4:18/km; runs 2-8 averaged 5:43; run 8 slowest at 6:12. Wall balls 11:30 (11.9%), the largest single part. Wall balls appeared in 2 of 31 training sessions in the 8-week block (all 31 workout records were read; only the 2 with wall balls are listed as evidence). He reported no-reps for depth and legs gone after the lunges. His gym target is 2.74 m; the race target is 3.00 m. EVIDENCE: run speed usually falls across HYROX races (research, PRO samples); fade size was not clearly linked to finish time among top athletes (research); more even pacing may help in longer events (transferable, other sports; no HYROX pacing trial). HYPOTHESES: the fast run 1, wall-ball capacity, wall-ball technique, and the gym target 26 cm below the race target may each explain part of the wall-ball time. Planned run pace for the next race is 5:15 per run (42:00 of running, about 2 minutes faster than 44:11, from a steadier start); run 1 at 5:15 or a little slower. Tests are in the coaching note of 27 October 2026.",
      "recommendations": [
        "Plan runs at 5:15 per km (42:00 of running), with run 1 at 5:15 or a little slower, never faster, in every full simulation and race.",
        "Test a fresh, judged 100 wall balls for time in the week of 27 October.",
        "Train wall balls twice a week at 6 kg to a marked 3.00 m line, from fresh sets to sets after lunges and a run.",
        "Film one wall-ball set every two weeks and count depth faults.",
        "Do one station-then-run session a week: lunges, then 1 km run, then wall balls.",
        "Do a full simulation on 5 December 2026 with run 1 capped at 5:15, and compare runs 6-8 and wall balls with the race."
      ]
    }
  },
  {
    "record_type": "coaching_note",
    "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b11",
    "data": {
      "authorship": {
        "author": { "author_type": "agent", "agent": "agent-harness", "model": "model-id" },
        "authored_at": "2026-10-27T18:05:00.000Z",
        "evidence": [
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b02", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b03", "revision": 1 },
          { "record_id": "01890f3e-e7d3-7cc0-88c8-000000000b04", "revision": 1 }
        ],
        "content_version": 1,
        "status": "active"
      },
      "title": "Tests for the 24 October 2026 race review",
      "note": "Coach's hypotheses, to be tested. (1) Pacing: run 1 at 4:10 was faster than his fresh 5 km pace. Test: cap run 1 at 5:15 (the planned pace for runs 2-8, from 42:00 of running divided by 8) in the full simulation on 5 December and compare runs 6-8 and wall balls with the race. (2) Wall-ball capacity: test a fresh, judged 100 wall balls for time this week. (3) Fatigue at wall balls: in week 3, do 100 m lunges at 20 kg, a 1 km run, then 100 wall balls; compare with the fresh test. If the fresh 100 is under about 5:30 and the fatigued 100 is over 8:00, pacing and fatigue explain more; if the fresh 100 is also slow, capacity or technique explain more. These cut-offs are coaching judgment, not tested thresholds. (4) Technique: film a set of 30 with a partner judging depth and count no-reps. (5) Equipment: his gym target is 26 cm below the race target; do the fresh test and train at a marked 3.00 m line, as the HYROX Rulebook Singles 26/27 p. 35 requires for men. Not tested here: Roxzone congestion, which cannot be trained, and the split layout, which looked normal.",
      "applies_from": "2026-10-27",
      "applies_to": "2026-12-06"
    }
  }
]
```

After the tests, write a new review. When it replaces this note, mark the note
`superseded`:

1. Read it: `ath records show <note-id> --history --json`. Note its current revision.
2. Copy its complete `data`, change only `authorship.status` to `"superseded"`, and
   preview: `ath correct <note-id> --revision <n> --data '<complete data>' --json`.
3. Show the athlete the preview. After approval, run the same command with `--yes`
   instead of `--json`.
