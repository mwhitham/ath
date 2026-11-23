---
name: training-load-heart-rate
description: Computes Banister's heart-rate training load (TRIMP) for one workout from its heart-rate samples, the athlete's resting and maximum heart rate, and their sex-specific weighting. Use when someone asks how much load a session put on them by heart rate, or wants to compare the internal load of two sessions or weeks. Not for load from the athlete's own effort rating (use training-load-effort) or time in zones (use heart-rate-zones).
license: MIT
compatibility: Requires the ath CLI 2.1.3 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath apply training-load-heart-rate --record <workout-session-id>.
metadata:
  version: "2.1.3"
  kind: calculation
---

# Training load from heart rate

Banister's training impulse (TRIMP): minutes of exercise weighted by how high the
heart rate sat between resting and maximum. Harder minutes count more.

## Before the first ath command

This skill runs `ath`, the Athletic Standard program, version 2.1.3 or later.
Check it once with `ath --version`.

- If `ath` is not found, it is not installed. It comes from the npm package
  `athleticstandard`. The npm package named `ath` is a different program. Install it
  with `npm install -g athleticstandard`, which needs Node.js 22 or later. Other ways
  to install it are at https://www.npmjs.com/package/athleticstandard.
- If the version is older than 2.1.3, update it with `ath --update`.
- Install or update only after the person agrees. Then run `ath --version` again. If
  it is still older than 2.1.3, say so and stop.

If the answer needs the athlete's records or a calculation and `ath` cannot run, stop.
Tell the person what is missing and how to fix it. Do not read the database another
way or do the calculation yourself: the calculations and their checks run only in
`ath`. For any other message from `ath`, follow what it says, and ask before anything
that creates or changes a file.

## Run it

```sh
ath apply training-load-heart-rate --record <workout-session-id> --json
```

It needs, on the workout, `heart_rate_samples`, `resting_heart_rate_bpm`, and
`maximum_heart_rate_bpm` (the athlete's known values), and `sex` on the athlete
profile. If any is missing, `missing` names it. Nothing is guessed.

## Method

Operation `banister_trimp` (D240).

For each sample, the heart-rate reserve fraction is
`x = (bpm − resting) / (maximum − resting)`, limited to 0–1. The load adds
`minutes × x × a × e^(b × x)` over the samples, where:

| Athlete profile `sex` | a | b |
|---|---|---|
| `male` | 0.64 | 1.92 |
| `female` | 0.86 | 1.67 |

These are the weightings later papers give for Banister (1991) and Morton et al.
(1990). Each sample lasts until the next one, and the last until the workout end,
never longer than 60 s. Longer gaps are reported as `uncovered_minutes` and add
no load.

## Reading the result

| Field | Meaning |
|---|---|
| `load` | TRIMP in arbitrary units, to 0.1. |
| `weighting`, `coefficient`, `exponent` | Which weighting was used. |
| `covered_minutes`, `uncovered_minutes` | How much of the session the samples cover. |

How to describe it:

- TRIMP compares sessions for the **same athlete** with the same device. It is
  not comparable between people or with a session-RPE load.
- A week's load is the sum of its sessions. Compare weeks, not single sessions,
  and say when a week is missing sessions.
- Heart rate underestimates the load of short, heavy, or intermittent work, such
  as lifting, sled pushes, or a 3-minute CrossFit couplet. For those, also use
  `training-load-effort`.

## Limits

- The resting and maximum heart rates must be the athlete's real values. A wrong
  maximum changes every weight.
- The sex-specific weighting comes from lactate studies in small groups. It is a
  convention, not an individual measurement.
- Optical sensors drop out during grip-heavy work; check `uncovered_minutes`.

## Worked example

A 10-minute effort, one sample a minute at 110, 125, 135, 145, 150, 160, 165, 172,
178, and 182 bpm, with resting 50 and maximum 190 bpm. For a female athlete the
weighted minutes add up to 23.2. With the male weighting the same session would
be 21.1.

```json ath-example
{
  "records": [
    { "id": "me", "record_type": "athlete_profile", "sex": "female" },
    { "id": "w", "record_type": "workout_session", "start": "2026-10-01T10:00:00.000Z", "end": "2026-10-01T10:10:00.000Z",
      "resting_heart_rate_bpm": 50, "maximum_heart_rate_bpm": 190,
      "heart_rate_samples": [
        { "offset_ms": 0, "bpm": 110 }, { "offset_ms": 60000, "bpm": 125 }, { "offset_ms": 120000, "bpm": 135 },
        { "offset_ms": 180000, "bpm": 145 }, { "offset_ms": 240000, "bpm": 150 }, { "offset_ms": 300000, "bpm": 160 },
        { "offset_ms": 360000, "bpm": 165 }, { "offset_ms": 420000, "bpm": 172 }, { "offset_ms": 480000, "bpm": 178 },
        { "offset_ms": 540000, "bpm": 182 }] }
  ],
  "apply": "w",
  "result": { "load": 23.2, "weighting": "female", "coefficient": 0.86, "exponent": 1.67, "covered_minutes": 10, "uncovered_minutes": 0 }
}
```

## Related

`heart-rate-zones` for time in zones, `training-load-effort` for the athlete's own
rating of the session.

```ath-calculation
name: training-load-heart-rate
use: banister_trimp
sample_field: heart_rate_samples
rest_field: resting_heart_rate_bpm
maximum_field: maximum_heart_rate_bpm
max_gap_ms: 60000
male_coefficient: 0.64
male_exponent: 1.92
female_coefficient: 0.86
female_exponent: 1.67
```
