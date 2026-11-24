---
name: summary
description: Counts and summarises one measurement over a period, separately for each device, with optional day, week, or month groups. Use when someone asks for their average, minimum, maximum, or number of readings of one quantity, such as RMSSD, SDNN, resting heart rate, heart rate, or body weight, over a period, or per week. Not for a change on one day (use hrv-change), a trend (use trend), or missing days (use coverage).
license: MIT
compatibility: Requires the ath CLI 2.1.3 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath calculate summary <quantity>.
metadata:
  version: "2.1.3"
  kind: calculation
---

# Summary

Count, minimum, maximum, mean, sum, and population standard deviation of one
quantity, for each source separately.

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
ath calculate summary hrv_rmssd --from 2026-10-01 --to 2026-10-31 --timezone Europe/London --json
ath calculate summary resting_heart_rate --group week --timezone Europe/London --json
```

Options: `--source <id>`, `--from`, `--to` (a calendar day or an exact UTC
instant), `--timezone`, `--group day|week|month`, and `--record-kind
provider-score|provider-summary` for a provider's scores or summaries.
`ath calculate --help` lists them.

## Method

1. One request names one quantity. RMSSD and SDNN, the temperature signals, and
   the distance types never share a summary.
2. Manual sources are left out. Sources are never pooled: each source gets its
   own summary.
3. Each reading counts once, so a source that reads many times a day weighs more
   than one that reads once. For a fair daily average use `baseline` with a time
   zone, or `--group day`.
4. More than 100 000 inputs is refused; narrow the dates.

## Reading the result

The receipt names the method, the unit, the sources, the input record revisions
(at most 1 000 listed), and the exclusions. `result` has one entry per source:
`count`, `minimum`, `maximum`, `mean`, `sum`, `standard_deviation` (population),
or, with `--group`, `groups` of the same per period.

How to describe it:

- Always say which source and which statistic. "Your ring's RMSSD averaged
  65 ms in October."
- The sum only makes sense for quantities that add up, such as steps.
- A short period with few readings gives an unstable mean. Report the count.

## Limits

- Days with no reading are not zero; check `coverage`.
- A date-only provider value (an Oura day) has no instant and is not read here;
  use `provider-period-summary`.

## Worked example

RMSSD from two devices. Device a: 60, 70, 65 ms. Device b: 50, 55 ms. They stay
apart: device a averages 65 ms (population standard deviation 4.0825 ms) and
device b 52.5 ms.

```json ath-example
{
  "records": [
    { "id": "a1", "record_type": "point_measurement", "source": "a", "quantity": "hrv_rmssd", "at": "2026-10-01T06:00:00.000Z", "value": 60 },
    { "id": "a2", "record_type": "point_measurement", "source": "a", "quantity": "hrv_rmssd", "at": "2026-10-02T06:00:00.000Z", "value": 70 },
    { "id": "a3", "record_type": "point_measurement", "source": "a", "quantity": "hrv_rmssd", "at": "2026-10-03T06:00:00.000Z", "value": 65 },
    { "id": "b1", "record_type": "point_measurement", "source": "b", "quantity": "hrv_rmssd", "at": "2026-10-01T06:00:00.000Z", "value": 50 },
    { "id": "b2", "record_type": "point_measurement", "source": "b", "quantity": "hrv_rmssd", "at": "2026-10-02T06:00:00.000Z", "value": 55 }
  ],
  "calculate": { "operation": "summary", "quantity": "hrv_rmssd" },
  "result": [
    { "source_id": "a", "count": 3, "minimum": 60, "maximum": 70, "mean": 65, "sum": 195, "standard_deviation": 4.0825 },
    { "source_id": "b", "count": 2, "minimum": 50, "maximum": 55, "mean": 52.5, "sum": 105, "standard_deviation": 2.5 }
  ]
}
```

## Related

`baseline` for a daily-weighted normal, `trend` for direction, `coverage` for
missing days, `provider-period-summary` for a provider's own days.

```ath-calculation
name: summary
needs: quantity
allows: source
allows: from
allows: to
allows: group
allows: timezone
per: source
use: count
use: minimum
use: maximum
use: mean
use: sum
method: count, minimum, maximum, mean, and sum per source
```
