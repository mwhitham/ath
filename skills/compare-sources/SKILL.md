---
name: compare-sources
description: Compares the same measurement from two devices, such as a ring and a watch, by their separate averages and, when the day is known, on the days both devices read. Use when someone asks why two devices disagree on HRV, resting heart rate, or another reading, how far apart they are, or before switching devices. Never merges the two sources. Not for a single device's normal (use baseline).
license: MIT
compatibility: Requires the ath CLI 2.1.3 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath calculate compare-sources <quantity> --source <id> --compare-source <id>.
metadata:
  version: "2.1.3"
  kind: calculation
---

# Compare sources

Two devices' summaries of one quantity, the difference between their means, and,
with a time zone, the mean difference on days both devices read.

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
ath stats --json          # find the source IDs
ath calculate compare-sources resting_heart_rate --source <id> --compare-source <id> --timezone Europe/London --json
```

## Method

1. Each source is summarised on its own: count, minimum, maximum, mean, sum,
   population standard deviation.
2. `mean_difference` is the first source's mean minus the second's. Its records
   are not paired: different days and different numbers of readings can differ.
3. `paired` (D244) needs a time zone (`--timezone` or the profile). Each source's
   readings are averaged per calendar day; on days both have, the daily
   differences are averaged. `days` says how many days paired.
4. One quantity only. Manual sources cannot be compared here.

## Reading the result

| Field | Meaning |
|---|---|
| `source`, `comparison_source` | Each device's summary. |
| `mean_difference` | First minus second, all readings. |
| `paired.days`, `paired.mean_difference` | First minus second, same days only. |

How to describe it:

- Prefer the paired difference. The unpaired one can even have the wrong sign
  when the devices covered different days, as in the example below.
- A steady offset is normal: devices measure at different times and with
  different methods. It does not say which device is right.
- Do not average two devices into one number. If the athlete switches devices,
  start a new baseline for the new one.

## Limits

- Few paired days give an unstable difference. Report `paired.days`.
- Even the same statistic can be measured under different conditions (overnight
  versus spot readings), so a difference may be real and stable.

## Worked example

Device a reads resting heart rate of 50, 60, 70 bpm on 1–3 October. Device b reads
45 on 1 October, 60 on 3 October, and 100 on 4 October. Over all readings, a is
8.3333 bpm lower than b, because of b's single high day. On the two days both read,
a is 7.5 bpm higher.

```json ath-example
{
  "records": [
    { "id": "a1", "record_type": "point_measurement", "source": "a", "quantity": "resting_heart_rate", "at": "2026-10-01T07:00:00.000Z", "value": 50 },
    { "id": "a2", "record_type": "point_measurement", "source": "a", "quantity": "resting_heart_rate", "at": "2026-10-02T07:00:00.000Z", "value": 60 },
    { "id": "a3", "record_type": "point_measurement", "source": "a", "quantity": "resting_heart_rate", "at": "2026-10-03T07:00:00.000Z", "value": 70 },
    { "id": "b1", "record_type": "point_measurement", "source": "b", "quantity": "resting_heart_rate", "at": "2026-10-01T07:00:00.000Z", "value": 45 },
    { "id": "b3", "record_type": "point_measurement", "source": "b", "quantity": "resting_heart_rate", "at": "2026-10-03T07:00:00.000Z", "value": 60 },
    { "id": "b4", "record_type": "point_measurement", "source": "b", "quantity": "resting_heart_rate", "at": "2026-10-04T07:00:00.000Z", "value": 100 }
  ],
  "calculate": { "operation": "compare-sources", "quantity": "resting_heart_rate", "source": "a", "compare_source": "b", "time_zone": "UTC" },
  "result": {
    "source": { "source_id": "a", "count": 3, "minimum": 50, "maximum": 70, "mean": 60, "sum": 180, "standard_deviation": 8.165 },
    "comparison_source": { "source_id": "b", "count": 3, "minimum": 45, "maximum": 100, "mean": 68.3333, "sum": 205, "standard_deviation": 23.2140 },
    "mean_difference": -8.3333,
    "paired": { "time_zone": "UTC", "days": 2, "mean_difference": 7.5 }
  }
}
```

## Related

`baseline` for one device's normal, `coverage` to see which days each device read.

```ath-calculation
name: compare-sources
needs: quantity
needs: source
needs: compare_source
allows: from
allows: to
use: mean
use: subtract
method: separate source summaries and difference between their means
```
