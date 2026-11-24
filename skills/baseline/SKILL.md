---
name: baseline
description: Gives an athlete's normal value and spread for one measurement over a period, separately for each device, with a fair daily-mean version when the time zone is known. Use when someone asks what their normal or baseline HRV (RMSSD or SDNN), resting heart rate, or other reading is, or what range counts as normal for them. Not for whether one day differs from normal (use hrv-change or resting-heart-rate-change) or direction over time (use trend).
license: MIT
compatibility: Requires the ath CLI 2.1.3 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath calculate baseline <quantity>.
metadata:
  version: "2.1.3"
  kind: calculation
---

# Baseline

The mean and population standard deviation of one quantity for each source, plus,
when the day is known, the same for calendar-day means.

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
ath calculate baseline hrv_rmssd --from 2026-09-01 --to 2026-09-30 --timezone Europe/London --json
```

Give `--timezone`, or set `default_time_zone` on the athlete profile, to get the
daily-mean summary. Use a period of normal training: a baseline taken during an
illness or a training camp is not this athlete's normal.

## Method

1. One request names one quantity; RMSSD and SDNN never share a baseline.
2. Manual sources are left out; each source has its own baseline.
3. `mean` and `standard_deviation` weigh every reading equally (receipt version 1
   meaning).
4. `daily` (D244) first averages each calendar day, then gives the mean and
   population standard deviation of those day means. A device that records
   many times a day, such as Apple Watch SDNN, then counts each day once.
5. Without a time zone there is no `daily`, and the exclusions say so.

## Reading the result

| Field | Meaning |
|---|---|
| `count`, `minimum`, `maximum`, `mean`, `sum`, `standard_deviation` | Every reading counted once. |
| `daily.days`, `daily.mean`, `daily.standard_deviation`, `daily.time_zone` | Each day counted once. |

How to describe it:

- Prefer `daily` for HRV and resting heart rate. Say the normal as "about 61 ms,
  usually within ±3 ms".
- Half a standard deviation either side is a common band for "no real change"
  in athlete monitoring. `hrv-change` reports a day in those units.
- A baseline belongs to one device and one statistic. A new watch needs its own.

## Limits

- Fewer than about two weeks of days gives an unstable spread.
- Missing days are not in the baseline. Check `coverage`.

## Worked example

Apple Watch SDNN: two readings on 1 October (40, 60), one on 2 October (50), three
on 3 October (80, 80, 80). Counting readings, the mean is 65 ms, pulled up by the
day with three readings. Counting days, the day means are 50, 50, and 80, so the
daily mean is 60 ms with a population standard deviation of 14.1421 ms.

```json ath-example
{
  "records": [
    { "id": "s1", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-01T06:00:00.000Z", "value": 40 },
    { "id": "s2", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-01T18:00:00.000Z", "value": 60 },
    { "id": "s3", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-02T06:00:00.000Z", "value": 50 },
    { "id": "s4", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-03T06:00:00.000Z", "value": 80 },
    { "id": "s5", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-03T12:00:00.000Z", "value": 80 },
    { "id": "s6", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-03T18:00:00.000Z", "value": 80 }
  ],
  "calculate": { "operation": "baseline", "quantity": "hrv_sdnn", "time_zone": "UTC" },
  "result": [
    { "source_id": "a", "count": 6, "minimum": 40, "maximum": 80, "mean": 65, "sum": 390, "standard_deviation": 16.0728,
      "daily": { "time_zone": "UTC", "days": 3, "mean": 60, "standard_deviation": 14.1421 } }
  ]
}
```

## Related

`hrv-change` and `resting-heart-rate-change` compare one day with this normal.
`summary` groups by week or month. `coverage` checks the days.

```ath-calculation
name: baseline
needs: quantity
allows: source
allows: from
allows: to
allows: group
allows: timezone
per: source
use: mean
use: population_standard_deviation
method: population mean and population standard deviation per source
```
