---
name: trend
description: Fits a straight-line trend to one measurement over time, separately for each device, and reports the slope per day with its standard error. Use when someone asks whether their HRV, resting heart rate, body weight, or another reading is going up or down over weeks, or how fast it is changing. Not for one day against normal (use hrv-change), an average (use baseline), or cause and effect (no calculation shows that).
license: MIT
compatibility: Requires the ath CLI 2.1.2 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath calculate trend <quantity>.
metadata:
  version: "2.1.2"
  kind: calculation
---

# Trend

An ordinary least-squares line through one quantity against elapsed days, for each
source.

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
ath calculate trend resting_heart_rate --from 2026-09-01 --to 2026-10-31 --timezone Europe/London --json
```

Name one source with `--source <id>` when the athlete has several devices.

## Method

1. One quantity, one source at a time. Manual sources are left out.
2. x is days since the source's first reading; y is the value. The slope is
   `Σ(x − x̄)(y − ȳ) / Σ(x − x̄)²`, to 0.0001 units a day.
3. `slope_standard_error` (D244) is `√(Σ residual² / (n − 2) / Σ(x − x̄)²)`, null
   with fewer than three readings.
4. `days_spanned` is the days from the first reading to the last.
5. Every reading counts once.

## Reading the result

| Field | Meaning |
|---|---|
| `slope_per_day` | Change per day in the quantity's unit. Multiply by 7 for a week. |
| `slope_standard_error` | The slope's uncertainty. |
| `count`, `days_spanned`, `first_value`, `last_value` | What the line rests on. |

How to describe it:

- A slope smaller than about twice its standard error is not clearly different
  from flat. Say "no clear trend" rather than "slightly down".
- Daily wellness data is autocorrelated: today resembles yesterday. That makes a
  slope look more certain than it is. Treat the standard error as a lower bound
  on the uncertainty.
- A straight line can hide a dip and recovery. Look at `summary --group week`
  too.
- Over a few days, the trend is mostly noise.

## Limits

- Missing days are skipped, not filled. Check `coverage`.
- A trend is a description, not a cause. Training, season, illness, and device
  changes all move it.

## Worked example

Resting heart rate on five consecutive days: 54, 53, 53, 51, 50 bpm. The line falls
1.0 bpm a day with a standard error of 0.1633: a clear fall over these four days,
though four days say little about the month.

```json ath-example
{
  "records": [
    { "id": "d1", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-01T07:00:00.000Z", "value": 54 },
    { "id": "d2", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-02T07:00:00.000Z", "value": 53 },
    { "id": "d3", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-03T07:00:00.000Z", "value": 53 },
    { "id": "d4", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-04T07:00:00.000Z", "value": 51 },
    { "id": "d5", "record_type": "point_measurement", "quantity": "resting_heart_rate", "at": "2026-10-05T07:00:00.000Z", "value": 50 }
  ],
  "calculate": { "operation": "trend", "quantity": "resting_heart_rate" },
  "result": [
    { "source_id": "a", "count": 5, "slope_per_day": -1, "first_value": 54, "last_value": 50, "days_spanned": 4, "slope_standard_error": 0.1633 }
  ]
}
```

## Related

`summary --group week` for weekly means, `baseline` for the normal, `coverage` for
gaps, `correlation` for two quantities together.

```ath-calculation
name: trend
needs: quantity
allows: source
allows: from
allows: to
per: source
use: slope_per_day
method: ordinary least-squares slope against elapsed days, calculated separately per source
```
