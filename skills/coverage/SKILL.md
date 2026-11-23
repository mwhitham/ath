---
name: coverage
description: Lists which calendar days have at least one reading of a measurement and which are missing, separately for each device, with the share of days covered. Use before describing a week or month of HRV, resting heart rate, sleep-related readings, or any other quantity, or when someone asks how consistently they wore their device or why a summary looks odd. Not for the values themselves (use summary or baseline).
license: MIT
compatibility: Requires the ath CLI 2.1.3 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath calculate coverage <quantity>.
metadata:
  version: "2.1.3"
  kind: calculation
---

# Coverage

The calendar days that have a reading, the days that do not, and the share covered,
for each source.

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
ath calculate coverage hrv_rmssd --from 2026-10-01 --to 2026-10-31 --timezone Europe/London --json
```

A time zone is required: from `--timezone` or the athlete profile.

## Method

1. Each reading's instant becomes a local calendar date in the time zone.
2. The expected days run from `--from` to `--to`, or from the source's first to
   last reading.
3. `missing_dates` lists expected days with no reading. `coverage_ratio` (D244) is
   `days_present / days_expected`, to 0.0001.
4. Manual sources are left out; each source is separate.

## Reading the result

| Field | Meaning |
|---|---|
| `days_expected`, `days_present`, `coverage_ratio` | How complete the period is. |
| `missing_dates` | The days with no reading. |
| `first_date`, `last_date` | The period checked. |

How to describe it:

- Run this before drawing conclusions from a week. A "drop" in a weekly mean can
  be two missing good days.
- No reading is not a zero and not a rest day. It usually means the device was
  not worn, not charged, or not synced.
- Missing days are rarely random. Athletes often skip wearing a device when ill,
  travelling, or after a late night, so the missing days can be the unusual ones.

## Limits

- One reading in a day counts the day as covered, even if most of the night is
  missing.

## Worked example

Readings on 1, 2, 4, and 5 October, checked from 1 to 5 October: 3 October is
missing, and 4 of 5 days are covered.

```json ath-example
{
  "records": [
    { "id": "d1", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-01T06:00:00.000Z", "value": 60 },
    { "id": "d2", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-02T06:00:00.000Z", "value": 62 },
    { "id": "d4", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-04T06:00:00.000Z", "value": 58 },
    { "id": "d5", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-05T06:00:00.000Z", "value": 61 }
  ],
  "calculate": { "operation": "coverage", "quantity": "hrv_rmssd", "time_zone": "UTC", "from": "2026-10-01", "to": "2026-10-05" },
  "result": [
    { "source_id": "a", "first_date": "2026-10-01", "last_date": "2026-10-05", "days_expected": 5, "days_present": 4,
      "missing_dates": ["2026-10-03"], "coverage_ratio": 0.8 }
  ]
}
```

## Related

Run before `summary`, `baseline`, `trend`, and `correlation`.

```ath-calculation
name: coverage
needs: quantity
needs: timezone
allows: source
allows: from
allows: to
use: missing_days
method: calendar dates containing at least one record, calculated separately per source
```
