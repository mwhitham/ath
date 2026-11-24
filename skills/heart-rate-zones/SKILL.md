---
name: heart-rate-zones
description: Splits one workout's heart-rate samples into five zones by percentage of the athlete's known maximum heart rate, crediting only time the samples cover. Use when someone asks how long a run, ride, row, or conditioning session spent in each zone, or how hard a session was by heart rate. Needs maximum_heart_rate_bpm on the workout; never estimates it from age. Not for training load (use training-load-heart-rate).
license: MIT
compatibility: Requires the ath CLI 2.1.3 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath apply heart-rate-zones --record <workout-session-id>.
metadata:
  version: "2.1.3"
  kind: calculation
---

# Heart-rate zones

Time in five zones, split at 60, 70, 80, and 90 % of the athlete's known maximum
heart rate.

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
ath records list --type workout_session --limit 10000 --json
ath apply heart-rate-zones --record <workout-session-id> --json
```

`maximum_heart_rate_bpm` on a workout is the athlete's known maximum (D172), not
that session's peak. If it is missing, the result says so. Do not fill it in from
an age formula: those formulas miss an individual's maximum by about 10 bpm or
more, which moves every zone. Ask the athlete for a measured maximum and correct
the workout with `ath correct`.

## Method

Operation `time_in_zones` (D240).

1. Samples are sorted by `offset_ms`.
2. Each sample lasts until the next sample, and the last one until the workout
   end, but never longer than 60 s (`max_gap_ms`). A longer gap is reported as
   `uncovered_seconds`, not credited to a zone. Time before the first sample is
   uncovered too.
3. A sample at `bpm / maximum` below 0.60 is zone 1; 0.60–0.70 zone 2; 0.70–0.80
   zone 3; 0.80–0.90 zone 4; 0.90 and above zone 5. A boundary value belongs to
   the higher zone.

## Reading the result

| Field | Meaning |
|---|---|
| `seconds` | Seconds in zones 1 to 5, in order. |
| `uncovered_seconds` | Time with no sample close enough. |
| `boundaries_bpm` | The four boundaries in bpm. |
| `maximum_heart_rate_bpm` | The maximum used. |

How to describe it:

- Give minutes, and the share of covered time. "11 of 30 minutes above 80 %."
- Zones from a percentage of maximum are a convention. Lactate or ventilatory
  thresholds sit at different percentages for different people.
- For short, intermittent work, such as a CrossFit triplet or HYROX stations,
  heart rate lags the effort. Zones describe the cardiovascular response, not
  the power produced.
- If `uncovered_seconds` is large, say the zones describe only part of the
  session.

## Limits

- Wrist sensors drop out and lag during grip-heavy or high-impact work such as
  sled pulls, farmers carries, and burpees.
- Zone boundaries move if the stored maximum is wrong.
- Changing `zone:` lines changes the boundaries. An edited file keeps its edits.

## Worked example

A 10-minute effort with a known maximum of 190 bpm, one sample a minute: 110,
125, 135, 145, 150, 160, 165, 172, 178, 182 bpm. The boundaries are 114, 133, 152,
and 171 bpm. Each sample covers 60 s, so: zone 1 has 60 s, zone 2 60 s, zone 3
180 s, zone 4 120 s, and zone 5 180 s.

```json ath-example
{
  "records": [
    { "id": "w", "record_type": "workout_session", "start": "2026-10-01T10:00:00.000Z", "end": "2026-10-01T10:10:00.000Z",
      "maximum_heart_rate_bpm": 190,
      "heart_rate_samples": [
        { "offset_ms": 0, "bpm": 110 }, { "offset_ms": 60000, "bpm": 125 }, { "offset_ms": 120000, "bpm": 135 },
        { "offset_ms": 180000, "bpm": 145 }, { "offset_ms": 240000, "bpm": 150 }, { "offset_ms": 300000, "bpm": 160 },
        { "offset_ms": 360000, "bpm": 165 }, { "offset_ms": 420000, "bpm": 172 }, { "offset_ms": 480000, "bpm": 178 },
        { "offset_ms": 540000, "bpm": 182 }] }
  ],
  "apply": "w",
  "result": { "seconds": [60, 60, 180, 120, 180], "uncovered_seconds": 0, "boundaries_bpm": [114, 133, 152, 171], "maximum_heart_rate_bpm": 190 }
}
```

## Related

`training-load-heart-rate` weights the same samples into one load number.
`heart-rate-recovery` reads the drop after the effort. `workout-drift` compares
the two halves of a steady session.

```ath-calculation
name: heart-rate-zones
use: time_in_zones
sample_field: heart_rate_samples
maximum_field: maximum_heart_rate_bpm
max_gap_ms: 60000
zone: 0.60
zone: 0.70
zone: 0.80
zone: 0.90
```
