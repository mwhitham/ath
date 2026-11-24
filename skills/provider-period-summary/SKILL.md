---
name: provider-period-summary
description: Summarises one measurement, provider score, or provider summary by the provider's own period stored on each record, such as an Oura sleep day, a WHOOP physiological cycle, or an Apple activity-summary day, without turning it into a calendar day. Use when someone asks for values per Oura day or per WHOOP cycle, or when a value has only a provider date and no exact time. Not for calendar-day summaries (use summary).
license: MIT
compatibility: Requires the ath CLI 2.1.3 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath calculate provider-period-summary <quantity> --period-type <provider-period-type>.
metadata:
  version: "2.1.3"
  kind: calculation
---

# Provider period summary

Count, minimum, maximum, mean, and sum for each provider period and source, exactly
as the provider defined the period.

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
ath calculate provider-period-summary recovery --record-kind provider-score --period-type whoop_physiological_cycle --json
ath calculate provider-period-summary hrv_rmssd --period-type oura_sleep_day --json
```

`--period-type` is required. `--record-kind` chooses measurements (the default),
`provider-score`, `provider-summary`, or `session-summary` (a summary nested in a
sleep or workout session).

## Method

1. Only records that carry the named provider period are read.
2. Records are grouped by the exact stored period: its ID, date, start, and end.
3. No period is converted to a calendar day. A WHOOP cycle runs from one sleep to
   the next, not midnight to midnight; an Oura sleep day is the day the sleep
   ended.
4. Manual sources are left out; each source is separate.

## Reading the result

One entry per source, with `periods`: the stored `provider_period` and its
`count`, `minimum`, `maximum`, `mean`, `sum`, and `standard_deviation`.

How to describe it:

- Use the provider's own words: "Oura's sleep day for 3 October", "your WHOOP
  cycle that began Tuesday evening".
- Provider scores, such as a recovery or readiness score, are the provider's
  composites. Quote them as the provider's, never as a measurement.

## Limits

- Two providers' periods never line up. Do not compare an Oura day with a WHOOP
  cycle as if they covered the same hours.

## Worked example

Two Oura RMSSD values carry the sleep day 3 October (a main sleep at 48 ms and a
nap at 52 ms) and one carries 4 October (55 ms). The 3 October sleep day has a
mean of 50 ms from two values.

```json ath-example
{
  "records": [
    { "id": "p1", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-03T06:00:00.000Z", "value": 48,
      "provider_reporting_periods": [{ "period_type": "oura_sleep_day", "provider_date": "2026-10-03" }] },
    { "id": "p2", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-03T14:00:00.000Z", "value": 52,
      "provider_reporting_periods": [{ "period_type": "oura_sleep_day", "provider_date": "2026-10-03" }] },
    { "id": "p3", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-04T06:00:00.000Z", "value": 55,
      "provider_reporting_periods": [{ "period_type": "oura_sleep_day", "provider_date": "2026-10-04" }] }
  ],
  "calculate": { "operation": "provider-period-summary", "quantity": "hrv_rmssd", "period_type": "oura_sleep_day" },
  "result": [
    { "source_id": "a", "periods": [
      { "provider_period": { "period_type": "oura_sleep_day", "provider_date": "2026-10-03" }, "count": 2, "minimum": 48, "maximum": 52, "mean": 50, "sum": 100, "standard_deviation": 2 },
      { "provider_period": { "period_type": "oura_sleep_day", "provider_date": "2026-10-04" }, "count": 1, "minimum": 55, "maximum": 55, "mean": 55, "sum": 55, "standard_deviation": 0 }
    ] }
  ]
}
```

## Related

`summary` for calendar days.

```ath-calculation
name: provider-period-summary
needs: quantity
needs: period_type
allows: source
allows: from
allows: to
allows: record_kind
per: provider_period
use: count
use: minimum
use: maximum
use: mean
use: sum
method: count, minimum, maximum, mean, and sum per exact provider period and source; no calendar boundary conversion
```
