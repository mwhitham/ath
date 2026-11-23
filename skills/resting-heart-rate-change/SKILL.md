---
name: resting-heart-rate-change
description: Compares one day's resting heart rate with the athlete's own earlier days from the same device. Use when someone asks whether their resting heart rate is up, whether a raised resting pulse after hard training, travel, illness, or poor sleep is unusual for them, or how this morning compares with normal. Reads resting_heart_rate readings. Not for estimating resting heart rate from raw samples (use resting-heart-rate).
license: MIT
compatibility: Requires the ath CLI 2.1.3 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath apply resting-heart-rate-change --record <record-id>.
metadata:
  version: "2.1.3"
  kind: calculation
---

# Resting heart rate change

This answers: is this day's resting heart rate different from this athlete's
recent normal, as the same device measures it?

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
ath records list --type point_measurement --limit 10000 --json
ath apply resting-heart-rate-change --record <record-id> --json
```

Choose a `resting_heart_rate` reading. A device's resting heart rate is its own
estimate; this compares it only with earlier estimates from that device.

## Method

Operation `earlier_window_change` (D232, D233), the same method as `hrv-change`:

1. The target value is the mean of the target's local day, up to the target.
2. The comparison is the mean of earlier daily means in the 28 days before.
3. Only the same source, quantity, record type, and basis are compared.
4. At least 7 earlier days are needed.
5. Days use the athlete profile's `default_time_zone`, or UTC.
6. `change` in bpm, `change_percent`, and `standardized_change` (the change in
   sample standard deviations of the daily means).

## Reading the result

| Field | Meaning |
|---|---|
| `value` | The day's resting heart rate in bpm. |
| `comparison.mean`, `standard_deviation`, `days` | The recent normal, its spread, and how many days made it. |
| `change`, `change_percent`, `standardized_change` | How far today is from normal. |

How to describe it:

- A rise is the direction that usually goes with strain, illness, heat, alcohol,
  dehydration, or short sleep. A fall is usually unremarkable.
- Because day-to-day spread in resting heart rate is often small, a rise of a few
  bpm can be several standard deviations. Say both: the bpm and the
  standardized change.
- A rise of several bpm that lasts more than a day or two is worth mentioning.
  It is not a diagnosis. Suggest rest and, for symptoms, a qualified clinician.
- Read it beside `hrv-change` and `sleep-change`, as three separate results.

## Limits

- Devices define resting heart rate differently. Apple estimates the lowest rate
  at rest; Garmin is reported to use the lowest 30-minute average of the day;
  rings use sleep. Never compare one device's number with another's.
- A day with no reading is missing, not normal. Check `coverage` before
  describing a week.
- A hand-entered reading (`manual` source) is only compared with other
  hand-entered readings.

## Worked example

Resting heart rate of 52, 51, 53, 52, 50, 51, 52 bpm on 1–7 October, then 57 bpm
on 8 October. The earlier mean is 51.571 bpm with a standard deviation of
0.976 bpm, so 57 bpm is 5.429 bpm (10.5 %) higher, or 5.56 standard deviations.
For this athlete that is a large rise, even though 57 bpm is an ordinary number
for an adult.

```json ath-example
{
  "records": [
    { "id": "d1", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-01T07:00:00.000Z", "value": 52 },
    { "id": "d2", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-02T07:00:00.000Z", "value": 51 },
    { "id": "d3", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-03T07:00:00.000Z", "value": 53 },
    { "id": "d4", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-04T07:00:00.000Z", "value": 52 },
    { "id": "d5", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-05T07:00:00.000Z", "value": 50 },
    { "id": "d6", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-06T07:00:00.000Z", "value": 51 },
    { "id": "d7", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-07T07:00:00.000Z", "value": 52 },
    { "id": "today", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-08T07:00:00.000Z", "value": 57 }
  ],
  "apply": "today",
  "result": {
    "quantity": "resting_heart_rate", "basis": "unstated", "value": 57, "target_day_readings": 1,
    "change": 5.429, "change_percent": 10.5, "standardized_change": 5.56,
    "comparison": { "mean": 51.571, "standard_deviation": 0.976, "days": 7, "from": "2026-09-10", "to": "2026-10-07" },
    "compared_records": 7, "time_zone": "UTC"
  }
}
```

With only a few days of history the result is null and says why:

```json ath-example
{
  "records": [
    { "id": "d1", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-01T07:00:00.000Z", "value": 52 },
    { "id": "today", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-02T07:00:00.000Z", "value": 57 }
  ],
  "apply": "today",
  "result": null,
  "missing": ["found 1 earlier days of resting_heart_rate in the 28 days before 2026-10-02, need 7"]
}
```

## Related

`resting-heart-rate` estimates a resting rate from samples. `hrv-change` and
`sleep-change` are the other recovery signals. `trend` shows the direction over
weeks.

```ath-calculation
name: resting-heart-rate-change
use: earlier_window_change
window_days: 28
minimum_days: 7
```
