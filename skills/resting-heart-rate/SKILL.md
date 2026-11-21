---
name: resting-heart-rate
description: Finds the lowest sustained heart rate, the lowest 5-minute mean, during a sleep session from that device's heart-rate samples, or during a workout from its own samples. Use when someone asks for their overnight or sleeping resting heart rate from raw samples, or how low their heart rate settled during a night. Not for a device's own resting heart rate value (read the resting_heart_rate measurement) or day-to-day change (use resting-heart-rate-change).
license: MIT
compatibility: Requires the ath CLI 2.1.2 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath apply resting-heart-rate --record <sleep-session-id>.
metadata:
  version: "2.1.2"
  kind: calculation
---

# Resting heart rate from samples

The lowest mean heart rate over any 5 minutes with enough samples. Run on a sleep
session it is a nocturnal resting heart rate. Run on a workout it is only the
lowest sustained rate during that workout.

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
ath records list --type sleep_session --limit 10000 --json
ath apply resting-heart-rate --record <sleep-session-id> --json
```

On a sleep session it reads the `heart_rate` samples (`ath series heart_rate`)
from the same source between the session's start and end.

## Method

Operation `lowest_rolling_mean` (D241).

1. Starting at each sample, take the window of the next 5 minutes (`window_ms`).
2. The window counts only if it holds at least 3 samples (`minimum_samples`), no
   two neighbouring samples are more than 2 minutes apart (`max_gap_ms`), and its
   last sample is within 2 minutes of the window's end.
3. The result is the lowest mean among counted windows, to 0.1 bpm.

The 5-minute window is a stated convention, not a published definition of
resting heart rate. It is editable.

## Reading the result

| Field | Meaning |
|---|---|
| `bpm` | The lowest 5-minute mean. |
| `window_start_at` (sleep) or `window_start_offset_ms` (workout) | Where it was. |
| `samples_in_window`, `samples_read` | How many samples made it, and how many were read. |

How to describe it:

- Name the method: "the lowest 5-minute average while asleep". It will not equal
  the device's own resting heart rate, which uses its own rule. Apple estimates
  the lowest rate at rest; Garmin is reported to use the lowest 30-minute average
  of the day.
- Compare this number only with the same calculation on other nights from the
  same device.

## Limits

- Sparse samples cannot fill a 5-minute window. Some devices record heart rate
  every few minutes while idle; then the result is null and says so.
- Movement, a hot room, alcohol, or a late meal raise overnight heart rate.
- On a workout this is not resting heart rate. Say so if asked.

## Worked example

Heart-rate samples during a night, one a minute from 02:00: 58, 57, 56, 50, 50, 50,
50, 50, 50, 55 bpm. The 5 minutes from 02:03 hold six samples of 50 bpm, the
lowest mean.

```json ath-example
{
  "records": [
    { "id": "night", "record_type": "sleep_session", "start": "2026-10-01T23:00:00.000Z", "end": "2026-10-02T07:00:00.000Z" },
    { "id": "h0", "record_type": "measurement_sample", "quantity": "heart_rate", "at": "2026-10-02T02:00:00.000Z", "value": 58 },
    { "id": "h1", "record_type": "measurement_sample", "quantity": "heart_rate", "at": "2026-10-02T02:01:00.000Z", "value": 57 },
    { "id": "h2", "record_type": "measurement_sample", "quantity": "heart_rate", "at": "2026-10-02T02:02:00.000Z", "value": 56 },
    { "id": "h3", "record_type": "measurement_sample", "quantity": "heart_rate", "at": "2026-10-02T02:03:00.000Z", "value": 50 },
    { "id": "h4", "record_type": "measurement_sample", "quantity": "heart_rate", "at": "2026-10-02T02:04:00.000Z", "value": 50 },
    { "id": "h5", "record_type": "measurement_sample", "quantity": "heart_rate", "at": "2026-10-02T02:05:00.000Z", "value": 50 },
    { "id": "h6", "record_type": "measurement_sample", "quantity": "heart_rate", "at": "2026-10-02T02:06:00.000Z", "value": 50 },
    { "id": "h7", "record_type": "measurement_sample", "quantity": "heart_rate", "at": "2026-10-02T02:07:00.000Z", "value": 50 },
    { "id": "h8", "record_type": "measurement_sample", "quantity": "heart_rate", "at": "2026-10-02T02:08:00.000Z", "value": 50 },
    { "id": "h9", "record_type": "measurement_sample", "quantity": "heart_rate", "at": "2026-10-02T02:09:00.000Z", "value": 55 }
  ],
  "apply": "night",
  "result": { "bpm": 50, "window_start_at": "2026-10-02T02:03:00.000Z", "window_minutes": 5, "samples_in_window": 6, "samples_read": 10 }
}
```

## Related

`resting-heart-rate-change` compares a device's resting heart rate with earlier
days. `sleep-duration` and `sleep-efficiency` describe the same night.

```ath-calculation
name: resting-heart-rate
use: lowest_rolling_mean
sample_field: heart_rate_samples
window_ms: 300000
minimum_samples: 3
max_gap_ms: 120000
```
