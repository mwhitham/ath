---
name: sleep-change
description: Compares one night's time asleep with the athlete's earlier main sleeps from the same device, leaving naps out. Use when someone asks whether they slept less than usual, how last night compares with their normal, or whether a short night is unusual for them. Reads asleep_seconds on a sleep session. Not for total daily sleep including naps, sleep timing (use sleep-timing), or regularity (use sleep-regularity).
license: MIT
compatibility: Requires the ath CLI 2.1.3 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath apply sleep-change --record <record-id>.
metadata:
  version: "2.1.3"
  kind: calculation
---

# Sleep change

This answers: did this athlete sleep more or less than usual last night,
compared with their recent main sleeps on the same device?

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
ath records list --type sleep_session --limit 10000 --json
ath apply sleep-change --record <sleep-session-id> --json
```

## Method

Operation `earlier_window_change` with `field: asleep_seconds` (D232, D234).

1. The target must be a main sleep: a session spanning at least 3 hours.
2. The comparison is every earlier main sleep from the same source that ended
   before the target started, in the 28 days before it. At least 7 are needed.
3. Shorter sessions are naps. They are left out and counted in `exclusions`.
   Sessions that overlap the target are left out too.
4. `change` in seconds, `change_percent`, and `standardized_change` in sample
   standard deviations of the earlier nights.

## Reading the result

| Field | Meaning |
|---|---|
| `value` | Time asleep this night, in seconds. Divide by 3600 for hours. |
| `comparison.mean`, `standard_deviation`, `nights` | The recent normal night. |
| `change`, `change_percent`, `standardized_change` | How far this night is from normal. |

How to describe it:

- Say it in hours and minutes. "4 h 15 min less" reads better than −15300.
- One short night is common. Several short nights in a row matter more for
  training and recovery than one.
- Time asleep is the device's estimate. Consumer wearables estimate sleep from
  movement and heart rate; they are better at total sleep than at stages.
- Read it beside `hrv-change` and `resting-heart-rate-change`, as separate
  results.

## Limits

- This compares the night, not the day. Naps are excluded on purpose. A person
  who naps every afternoon may still be short of sleep overall.
- A night the device missed is not a night of no sleep. Check `coverage`.
- Two devices that both record sleep are two sources with separate histories.
- The 3-hour line between a nap and a main sleep is a setting
  (`main_sleep_hours`). A shift worker may need a different value.

## Worked example

Seven nights of 7 h 20 min, 7 h 30 min, 7 h 10 min, 7 h 40 min, 7 h 15 min,
7 h 35 min, and 7 h 25 min average 26 700 s (7 h 25 min). A 2-hour afternoon nap
is left out. Last night was 6 h 10 min (22 200 s): 4500 s, or 1 h 15 min, less
than usual, a 16.9 % drop and 6.94 standard deviations, because these nights
vary little.

```json ath-example
{
  "records": [
    { "id": "n1", "record_type": "sleep_session", "start": "2026-10-01T22:30:00.000Z", "end": "2026-10-02T06:30:00.000Z", "asleep_seconds": 26400 },
    { "id": "n2", "record_type": "sleep_session", "start": "2026-10-02T22:30:00.000Z", "end": "2026-10-03T06:30:00.000Z", "asleep_seconds": 27000 },
    { "id": "n3", "record_type": "sleep_session", "start": "2026-10-03T22:30:00.000Z", "end": "2026-10-04T06:30:00.000Z", "asleep_seconds": 25800 },
    { "id": "n4", "record_type": "sleep_session", "start": "2026-10-04T22:30:00.000Z", "end": "2026-10-05T06:30:00.000Z", "asleep_seconds": 27600 },
    { "id": "nap", "record_type": "sleep_session", "start": "2026-10-05T13:00:00.000Z", "end": "2026-10-05T15:00:00.000Z", "asleep_seconds": 6600 },
    { "id": "n5", "record_type": "sleep_session", "start": "2026-10-05T22:30:00.000Z", "end": "2026-10-06T06:30:00.000Z", "asleep_seconds": 26100 },
    { "id": "n6", "record_type": "sleep_session", "start": "2026-10-06T22:30:00.000Z", "end": "2026-10-07T06:30:00.000Z", "asleep_seconds": 27300 },
    { "id": "n7", "record_type": "sleep_session", "start": "2026-10-07T22:30:00.000Z", "end": "2026-10-08T06:30:00.000Z", "asleep_seconds": 26700 },
    { "id": "last", "record_type": "sleep_session", "start": "2026-10-08T23:30:00.000Z", "end": "2026-10-09T06:30:00.000Z", "asleep_seconds": 22200 }
  ],
  "apply": "last",
  "result": {
    "field": "asleep_seconds", "value": 22200,
    "change": -4500, "change_percent": -16.9, "standardized_change": -6.94,
    "comparison": { "mean": 26700, "standard_deviation": 648.074, "nights": 7, "from": "2026-09-10T23:30:00.000Z", "to": "2026-10-08T23:30:00.000Z" },
    "compared_records": 7
  },
  "exclusions": [
    "other sources are not compared",
    "sessions that end after this one starts are excluded",
    "1 naps shorter than 3 h excluded"
  ]
}
```

## Related

`sleep-duration` for one night's total, `sleep-efficiency` for sleep against
time in bed, `sleep-regularity` for consistency, and `hrv-change` and
`resting-heart-rate-change` for the other recovery signals.

```ath-calculation
name: sleep-change
use: earlier_window_change
field: asleep_seconds
window_days: 28
minimum_days: 7
main_sleep_hours: 3
```
