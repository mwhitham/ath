---
name: correlation
description: Measures how two measurements move together day by day, such as HRV and resting heart rate or HRV and respiratory rate, using each calendar day's mean and needing at least three paired days. Use when someone asks whether two of their readings are related, or whether one tends to be low when the other is high. Reports association only; never shows cause. Not for one quantity over time (use trend).
license: MIT
compatibility: Requires the ath CLI 2.1.2 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath calculate correlation <quantity> --other-quantity <quantity> --timezone <zone>.
metadata:
  version: "2.1.2"
  kind: calculation
---

# Correlation

The Pearson correlation between two quantities' calendar-day means, on the days
both have a reading.

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
ath calculate correlation hrv_rmssd --other-quantity resting_heart_rate --source <id> --timezone Europe/London --json
```

## Method

1. Each quantity's readings are averaged per calendar day in the time zone.
2. Only days with both quantities pair. Other days are left out.
3. Pearson r is computed on the paired day means, to 0.0001.
4. With fewer than three paired days, or when one quantity does not vary, the
   correlation is null and `reason` says why (D244). Two points always gave ±1.
5. `--source` reads both quantities from one device; `--compare-source` reads the
   second quantity from another. Manual sources are left out.

## Reading the result

| Field | Meaning |
|---|---|
| `correlation` | −1 to 1, or null. |
| `paired_day_count`, `paired_dates` | The days used. |
| `reason` | Why the correlation is null. |

How to describe it:

- Association, not cause. HRV and resting heart rate move together partly
  because both respond to the same things: sleep, illness, and training.
- With few days, r is very uncertain. With 10 days, an r of 0.5 is compatible
  with almost no relation. Say how many days, and avoid strong words below about
  30 days.
- Daily data is autocorrelated: consecutive days resemble each other. That makes
  correlations look stronger and more certain than they are.
- A relation that holds for this athlete may not hold for anyone else.

## Limits

- Day means hide timing within the day.
- One unusual day can dominate a short series. Look at the paired values.

## Worked example

Five days of RMSSD (60, 55, 65, 50, 70 ms) and resting heart rate (52, 55, 50, 57,
49 bpm): r = −0.9878. On these days, higher HRV went with lower resting heart rate.
Five days is far too few to call it a stable pattern.

```json ath-example
{
  "records": [
    { "id": "h1", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-01T06:00:00.000Z", "value": 60 },
    { "id": "h2", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-02T06:00:00.000Z", "value": 55 },
    { "id": "h3", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-03T06:00:00.000Z", "value": 65 },
    { "id": "h4", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-04T06:00:00.000Z", "value": 50 },
    { "id": "h5", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-05T06:00:00.000Z", "value": 70 },
    { "id": "r1", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-01T06:00:00.000Z", "value": 52 },
    { "id": "r2", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-02T06:00:00.000Z", "value": 55 },
    { "id": "r3", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-03T06:00:00.000Z", "value": 50 },
    { "id": "r4", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-04T06:00:00.000Z", "value": 57 },
    { "id": "r5", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-05T06:00:00.000Z", "value": 49 }
  ],
  "calculate": { "operation": "correlation", "quantity": "hrv_rmssd", "other_quantity": "resting_heart_rate", "source": "a", "time_zone": "UTC" },
  "result": {
    "other_quantity": "resting_heart_rate", "paired_day_count": 5, "correlation": -0.9878,
    "paired_dates": ["2026-10-01", "2026-10-02", "2026-10-03", "2026-10-04", "2026-10-05"]
  }
}
```

## Related

`trend` for each quantity over time, `coverage` for missing days.

```ath-calculation
name: correlation
needs: quantity
needs: other_quantity
needs: timezone
allows: source
allows: compare_source
allows: from
allows: to
use: correlation
method: Pearson correlation of same-calendar-day means
```
