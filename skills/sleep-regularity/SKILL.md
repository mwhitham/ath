---
name: sleep-regularity
description: Scores how regular sleep timing has been over the last 14 days with the Sleep Regularity Index, from -100 to 100, using the sleep sessions from one device. Use when someone asks whether they keep a consistent sleep schedule, how irregular their bedtimes are, or whether shifting sleep around weekends or shifts is showing. Needs at least 7 consecutive recorded days. Not for one night's times (use sleep-timing) or amount (use sleep-duration).
license: MIT
compatibility: Requires the ath CLI 2.1.2 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath apply sleep-regularity --record <sleep-session-id>.
metadata:
  version: "2.1.2"
  kind: calculation
---

# Sleep regularity

The Sleep Regularity Index (Phillips et al., 2017) asks: at each minute of the day,
is the athlete in the same state, asleep or awake, as at the same minute 24 hours
later? 100 means the same pattern every day; 0 means no relation; −100 means the
opposite pattern.

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

Run it on the most recent sleep session. It looks back 14 days from that session's
end.

```sh
ath apply sleep-regularity --record <sleep-session-id> --json
```

## Method

Operation `sleep_regularity_index` (D236).

1. The window is the 14 days of 24 hours ending at the target session's end.
2. A minute is asleep if it lies inside a sleep session from the same source.
   Awake time inside a session counts as asleep, because the record has no
   minute-by-minute wake data. Naps count as sleep.
3. A 24-hour day with no session is unobserved: the device may not have been worn.
   Every pair of consecutive days where both are observed is compared minute by
   minute.
4. `index = −100 + 200 × (minutes in the same state) / (pairs × 1440)`, to 0.1.
5. At least 6 observed day pairs (7 days) are needed.

## Reading the result

| Field | Meaning |
|---|---|
| `index` | −100 to 100. Higher is more regular. |
| `day_pairs`, `days_observed`, `window_days` | How much data made the score. |
| `window_from`, `window_to`, `sessions` | The window and sessions read. |

How to describe it:

- Compare the athlete with their own earlier windows, not with study averages.
  Published averages come from actigraphy with minute-level wake data; this score
  uses sleep periods, so the two are not directly comparable.
- A drop usually means shifting bed or wake times, such as late weekends, shift
  work, or travel. `sleep-timing` on single nights shows where.
- The score says nothing about how long or how well the athlete slept.

## Limits

- Wake during the night is invisible here. The published index used wake epochs.
- Unrecorded nights shrink the data. A score from 6 pairs is less stable than one
  from 13.
- Two devices are two sources; their sessions are never merged.

## Worked example

Eight nights from 23:00 to 07:00 score 100. If every other night instead runs from
03:00 to 11:00, the same minute 24 hours apart agrees for only two thirds of the
day, and the score falls to 33.3. Both use 7 pairs from 8 observed days.

```json ath-example
{
  "records": [
    { "id": "n0", "record_type": "sleep_session", "start": "2026-10-01T23:00:00.000Z", "end": "2026-10-02T07:00:00.000Z" },
    { "id": "n1", "record_type": "sleep_session", "start": "2026-10-02T23:00:00.000Z", "end": "2026-10-03T07:00:00.000Z" },
    { "id": "n2", "record_type": "sleep_session", "start": "2026-10-03T23:00:00.000Z", "end": "2026-10-04T07:00:00.000Z" },
    { "id": "n3", "record_type": "sleep_session", "start": "2026-10-04T23:00:00.000Z", "end": "2026-10-05T07:00:00.000Z" },
    { "id": "n4", "record_type": "sleep_session", "start": "2026-10-05T23:00:00.000Z", "end": "2026-10-06T07:00:00.000Z" },
    { "id": "n5", "record_type": "sleep_session", "start": "2026-10-06T23:00:00.000Z", "end": "2026-10-07T07:00:00.000Z" },
    { "id": "n6", "record_type": "sleep_session", "start": "2026-10-07T23:00:00.000Z", "end": "2026-10-08T07:00:00.000Z" },
    { "id": "n7", "record_type": "sleep_session", "start": "2026-10-08T23:00:00.000Z", "end": "2026-10-09T07:00:00.000Z" }
  ],
  "apply": "n7",
  "result": { "index": 100, "day_pairs": 7, "days_observed": 8, "window_days": 14,
    "window_from": "2026-09-25T07:00:00.000Z", "window_to": "2026-10-09T07:00:00.000Z", "sessions": 8 }
}
```

```json ath-example
{
  "records": [
    { "id": "n0", "record_type": "sleep_session", "start": "2026-10-01T23:00:00.000Z", "end": "2026-10-02T07:00:00.000Z" },
    { "id": "n1", "record_type": "sleep_session", "start": "2026-10-03T03:00:00.000Z", "end": "2026-10-03T11:00:00.000Z" },
    { "id": "n2", "record_type": "sleep_session", "start": "2026-10-03T23:00:00.000Z", "end": "2026-10-04T07:00:00.000Z" },
    { "id": "n3", "record_type": "sleep_session", "start": "2026-10-05T03:00:00.000Z", "end": "2026-10-05T11:00:00.000Z" },
    { "id": "n4", "record_type": "sleep_session", "start": "2026-10-05T23:00:00.000Z", "end": "2026-10-06T07:00:00.000Z" },
    { "id": "n5", "record_type": "sleep_session", "start": "2026-10-07T03:00:00.000Z", "end": "2026-10-07T11:00:00.000Z" },
    { "id": "n6", "record_type": "sleep_session", "start": "2026-10-07T23:00:00.000Z", "end": "2026-10-08T07:00:00.000Z" },
    { "id": "n7", "record_type": "sleep_session", "start": "2026-10-09T03:00:00.000Z", "end": "2026-10-09T11:00:00.000Z" }
  ],
  "apply": "n7",
  "result": { "index": 33.3, "day_pairs": 7, "days_observed": 8, "window_days": 14,
    "window_from": "2026-09-25T11:00:00.000Z", "window_to": "2026-10-09T11:00:00.000Z", "sessions": 8 }
}
```

## Related

`sleep-timing` for single nights, `coverage` for missing nights, `sleep-change`
for amount.

```ath-calculation
name: sleep-regularity
use: sleep_regularity_index
window_days: 14
minimum_day_pairs: 6
```
