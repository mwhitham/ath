---
name: workout-drift
description: Compares the average of a measurement, usually heart rate, in the first and second halves of one workout, separately for each device. Use when someone asks whether their heart rate drifted upward during a steady run, ride, or row, or how much it rose late in a long session. This is cardiac drift only, not aerobic efficiency. Not for zones (use heart-rate-zones) or load (use training-load-heart-rate).
license: MIT
compatibility: Requires the ath CLI 2.1.2 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath calculate workout-drift <quantity> --session <workout-session-id>.
metadata:
  version: "2.1.2"
  kind: calculation
---

# Workout drift

The mean of a measurement in the second half of a workout minus the mean in the
first half, with the percentage change.

## Before the first ath command

This skill runs `ath`, the Athletic Standard program, version 2.1.2 or later.
Check it once with `ath --version`.

- If `ath` is not found, it is not installed. It comes from the npm package
  `athleticstandard`. The npm package named `ath` is a different program. Install it
  with `npm install -g athleticstandard`, which needs Node.js 22 or later. Other ways
  to install it are at https://www.npmjs.com/package/athleticstandard.
- If the version is older than 2.1.2, update it with `ath --update`.
- Install or update only after the person agrees. Then run `ath --version` again. If
  it is still older than 2.1.2, say so and stop.

If the answer needs the athlete's records or a calculation and `ath` cannot run, stop.
Tell the person what is missing and how to fix it. Do not read the database another
way or do the calculation yourself: the calculations and their checks run only in
`ath`. For any other message from `ath`, follow what it says, and ask before anything
that creates or changes a file.

## Run it

```sh
ath calculate workout-drift heart_rate --session <workout-session-id> --json
```

It reads measurement samples, such as `heart_rate` series samples, recorded inside
the workout's start and end. Inline `heart_rate_samples` on the workout are not
read.

## Method

1. The session's midpoint splits it into two halves of equal time.
2. Each source's samples are averaged in each half. Each sample counts once.
3. `change = second − first`; `change_percentage = change / first × 100`, from the
   means rounded to four places.
4. A source with no samples in one half is reported as unavailable.
5. `first_half_count` and `second_half_count` (D244) say how many samples each
   mean rests on.

## Reading the result

| Field | Meaning |
|---|---|
| `first_half_mean`, `second_half_mean` | The two means. |
| `change`, `change_percentage` | The drift. |
| `first_half_count`, `second_half_count` | The samples behind each mean. |

How to describe it:

- Drift is meaningful only for a steady effort at a constant pace or power. In
  intervals, a CrossFit workout, or a HYROX race, the halves contain different
  work, and the "drift" is the workout's design.
- Heart rate rising at a steady pace is cardiac drift. It grows with heat,
  dehydration, and duration (Coyle and González-Alonso, 2001).
- Aerobic efficiency needs pace or power with heart rate. Coaches who track that
  ratio often treat a change under about 5 % across a long steady session as good
  aerobic conditioning; that is coaching practice, not a validated rule. This
  calculation uses heart rate alone, so it cannot say whether the athlete slowed
  down.

## Limits

- Uneven sampling weights one half more than the other. Check the counts.
- Sensor dropouts in one half can bias it.

## Worked example

A 10-minute session from 10:00 with heart-rate samples at 10:01 (120), 10:03 (130),
10:06 (140), 10:08 (150), and 10:09 (150 bpm). The midpoint is 10:05. The first
half averages 125 bpm from two samples and the second 146.6667 bpm from three: a
rise of 21.6667 bpm, or 17.3334 %. (The percentage is computed from the means
after they are rounded to four places.) This was a build-up, not a steady effort,
so it is not drift in the useful sense.

```json ath-example
{
  "records": [
    { "id": "w", "record_type": "workout_session", "start": "2026-10-01T10:00:00.000Z", "end": "2026-10-01T10:10:00.000Z" },
    { "id": "s1", "record_type": "measurement_sample", "quantity": "heart_rate", "at": "2026-10-01T10:01:00.000Z", "value": 120 },
    { "id": "s3", "record_type": "measurement_sample", "quantity": "heart_rate", "at": "2026-10-01T10:03:00.000Z", "value": 130 },
    { "id": "s6", "record_type": "measurement_sample", "quantity": "heart_rate", "at": "2026-10-01T10:06:00.000Z", "value": 140 },
    { "id": "s8", "record_type": "measurement_sample", "quantity": "heart_rate", "at": "2026-10-01T10:08:00.000Z", "value": 150 },
    { "id": "s9", "record_type": "measurement_sample", "quantity": "heart_rate", "at": "2026-10-01T10:09:00.000Z", "value": 150 }
  ],
  "calculate": { "operation": "workout-drift", "quantity": "heart_rate", "session": "w" },
  "result": {
    "workout_session_id": "w",
    "sources": [
      { "source_id": "a", "available": true, "first_half_mean": 125, "second_half_mean": 146.6667, "change": 21.6667,
        "change_percentage": 17.3334, "first_half_count": 2, "second_half_count": 3 }
    ]
  }
}
```

## Related

`heart-rate-zones` and `training-load-heart-rate` for the same session.

```ath-calculation
name: workout-drift
needs: quantity
needs: session
allows: source
use: first_half
use: second_half
method: difference between means in equal-duration first and second workout halves
```
