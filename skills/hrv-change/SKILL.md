---
name: hrv-change
description: Compares one day's heart-rate variability with the athlete's own earlier days, keeping RMSSD and SDNN apart. Use when someone asks whether their HRV is up or down, how today's or last night's HRV compares with normal, or whether HRV has dropped after hard training, travel, illness, or poor sleep. Works for nightly RMSSD (WHOOP, Oura) and intraday SDNN samples (Apple Watch). Not for a long-run HRV trend (use trend) or a plain average (use baseline).
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.1.2 database. Runs with ath apply hrv-change --record <record-id>.
metadata:
  version: "2.1.2"
  kind: calculation
---

# Heart-rate variability change

This answers one question: is this day's HRV different from this athlete's own
recent normal, measured the same way by the same device?

## Run it

```sh
ath records list --type point_measurement --limit 10000 --json     # nightly RMSSD, provider HRV
ath records list --type measurement_sample --limit 10000 --json    # Apple SDNN samples
ath apply hrv-change --record <record-id> --json
```

Pick the reading for the day you are asked about. Check its `quantity` first:
`hrv_rmssd` and `hrv_sdnn` are different statistics and are never compared with
each other.

## Method

Operation `earlier_window_change` (decisions D232 and D233).

1. The target value is the mean of the target's local day, using only readings
   at or before the target reading. A nightly reading has one per day. Apple
   SDNN can have several.
2. The comparison is the mean of earlier **daily means** from the 28 days before
   that day. Each day counts once, however many readings it has.
3. Only readings that match the target on all of these count: the same source,
   the same quantity, the same record type (a nightly point measurement or an
   intraday sample), and the same basis (`field_basis.value`, for example
   `provider` or `calculated_on_import`).
4. At least 7 earlier days are needed. With fewer, the result is null and
   `missing` says how many days were found.
5. Days use the athlete profile's `default_time_zone`, or UTC. The result names
   the zone.
6. Results: `change = value − mean`, `change_percent = change / mean × 100`, and
   `standardized_change = change / standard deviation`, where the standard
   deviation is the sample standard deviation (n − 1) of the daily means.

Settings in the block below can be edited: `window_days`, `minimum_days`.

## Reading the result

| Field | Meaning |
|---|---|
| `value` | The target day's mean so far, in ms. |
| `target_day_readings` | How many readings made that mean. |
| `comparison.mean`, `comparison.standard_deviation` | The athlete's recent normal and its day-to-day spread. |
| `comparison.days`, `from`, `to` | The days compared, in the named `time_zone`. |
| `change`, `change_percent` | The difference in ms and percent. |
| `standardized_change` | The difference in standard deviations of recent days. |
| `basis` | Which kind of value was compared. |

How to describe it:

- Within about ±0.5 standard deviations is ordinary day-to-day variation.
  Athlete-monitoring studies use a band of this size as the smallest change worth
  noting (Vesterinen et al., 2016; Plews et al.).
- One low day is common and says little. A run of low days, or a weekly mean
  that falls below the band, is more informative. Run this on each day of the
  week, or use `trend`.
- HRV moves with sleep, alcohol, illness, heat, travel, menstrual cycle, stress,
  and hard training. The data cannot say which one caused a change. Name the
  candidates the records support and say which are unknown.
- Read it beside `resting-heart-rate-change` and `sleep-change`. Report them as
  three separate results. There is no Athletic Standard recovery score (D159).

## Limits

- RMSSD and SDNN are different statistics with different normal values. Never
  put them in one comparison or one sentence about "your HRV" without naming
  which.
- A nightly average and a spot reading are recorded differently. Apple Watch
  SDNN is sampled at moments while the wearer is still, so a single reading
  varies more than a nightly RMSSD.
- A new device is a new source. Its first weeks have no history; do not compare
  it with the old device's numbers. Use `compare-sources` over a period when
  both were worn.
- A hand-entered reading uses the `manual` source and is only compared with other
  hand-entered readings.
- Garmin's overnight HRV does not name its statistic and is not stored as RMSSD or
  SDNN (D156).

## Worked example

Nightly RMSSD from one ring: 62, 58, 65, 60, 63, 59, 61, 64 ms on 1–8 October,
then 54 ms on 9 October. The eight earlier days average 61.5 ms with a standard
deviation of 2.449 ms. The change is 54 − 61.5 = −7.5 ms, −12.2 %, or −3.06
standard deviations: well outside ordinary variation for this athlete. One night
is still one night; check the following days and the sleep record.

```json ath-example
{
  "records": [
    { "id": "d1", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-01T06:00:00.000Z", "value": 62 },
    { "id": "d2", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-02T06:00:00.000Z", "value": 58 },
    { "id": "d3", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-03T06:00:00.000Z", "value": 65 },
    { "id": "d4", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-04T06:00:00.000Z", "value": 60 },
    { "id": "d5", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-05T06:00:00.000Z", "value": 63 },
    { "id": "d6", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-06T06:00:00.000Z", "value": 59 },
    { "id": "d7", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-07T06:00:00.000Z", "value": 61 },
    { "id": "d8", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-08T06:00:00.000Z", "value": 64 },
    { "id": "today", "record_type": "point_measurement", "quantity": "hrv_rmssd", "at": "2026-10-09T06:00:00.000Z", "value": 54 }
  ],
  "apply": "today",
  "result": {
    "quantity": "hrv_rmssd", "basis": "unstated", "value": 54, "target_day_readings": 1,
    "change": -7.5, "change_percent": -12.2, "standardized_change": -3.06,
    "comparison": { "mean": 61.5, "standard_deviation": 2.449, "days": 8, "from": "2026-09-11", "to": "2026-10-08" },
    "compared_records": 8, "time_zone": "UTC"
  }
}
```

The same works for Apple Watch SDNN samples. Here each earlier day has two
samples, so each day's mean counts once: the daily means are 45, 45, 40, 50, 44,
44, and 45 ms, averaging 44.714 ms. A morning sample of 36 ms is 8.714 ms lower.

```json ath-example
{
  "records": [
    { "id": "a1", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-01T06:00:00.000Z", "value": 40 },
    { "id": "b1", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-01T18:00:00.000Z", "value": 50 },
    { "id": "a2", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-02T06:00:00.000Z", "value": 44 },
    { "id": "b2", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-02T18:00:00.000Z", "value": 46 },
    { "id": "a3", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-03T06:00:00.000Z", "value": 38 },
    { "id": "b3", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-03T18:00:00.000Z", "value": 42 },
    { "id": "a4", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-04T06:00:00.000Z", "value": 45 },
    { "id": "b4", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-04T18:00:00.000Z", "value": 55 },
    { "id": "a5", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-05T06:00:00.000Z", "value": 41 },
    { "id": "b5", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-05T18:00:00.000Z", "value": 47 },
    { "id": "a6", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-06T06:00:00.000Z", "value": 43 },
    { "id": "b6", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-06T18:00:00.000Z", "value": 45 },
    { "id": "a7", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-07T06:00:00.000Z", "value": 39 },
    { "id": "b7", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-07T18:00:00.000Z", "value": 51 },
    { "id": "today", "record_type": "measurement_sample", "quantity": "hrv_sdnn", "at": "2026-10-08T07:00:00.000Z", "value": 36 }
  ],
  "apply": "today",
  "result": {
    "quantity": "hrv_sdnn", "basis": "unstated", "value": 36, "target_day_readings": 1,
    "change": -8.714, "change_percent": -19.5, "standardized_change": -2.98,
    "comparison": { "mean": 44.714, "standard_deviation": 2.928, "days": 7, "from": "2026-09-10", "to": "2026-10-07" },
    "compared_records": 14, "time_zone": "UTC"
  }
}
```

## Related

`baseline` for a plain average, `trend` for direction over weeks,
`coverage` to check for missing days, `compare-sources` for two devices,
`resting-heart-rate-change` and `sleep-change` for the other two recovery
signals.

```ath-calculation
name: hrv-change
use: earlier_window_change
window_days: 28
minimum_days: 7
```
