---
name: sleep-timing
description: Gives when one sleep session started, its midpoint, and when it ended, in the athlete's own time zone. Use when someone asks what time they fell asleep or woke up, how late they went to bed, or the clock time of their sleep midpoint. Not for how regular sleep is across nights (use sleep-regularity) or how long it was (use sleep-duration).
license: MIT
compatibility: Requires the ath CLI 2.1.3 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath apply sleep-timing --record <sleep-session-id>.
metadata:
  version: "2.1.3"
  kind: calculation
---

# Sleep timing

The start, midpoint, and end of one sleep session, in UTC and on the athlete's
local clock.

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
ath apply sleep-timing --record <sleep-session-id> --json
```

## Method

Operation `sleep_clock_times` (D242).

1. Read `start_at` and `end_at` (UTC). The midpoint is halfway between them.
2. Local clock times use, in order: the athlete profile's `default_time_zone`; else
   the UTC offset the source wrote for the start; else UTC. `time_zone_basis` says
   which.
3. `duration_hours` is the session span, which includes time awake.

## Reading the result

| Field | Meaning |
|---|---|
| `start_at`, `midpoint_at`, `end_at` | UTC instants. |
| `local_start`, `local_midpoint`, `local_end` | The same instants with the local offset. |
| `time_zone`, `time_zone_basis` | The zone used and why. |
| `duration_hours` | The session's span. |

How to describe it:

- Use the local times. "You fell asleep at 23:30 and woke at 07:30."
- The midpoint is a steady marker of sleep timing. A midpoint that moves by an
  hour or more from night to night is a sign of irregular timing; `sleep-regularity`
  measures that across a week or two.
- After travel the profile's time zone may no longer be where the athlete slept.
  Say which zone you used.

## Limits

- The session's start is when the device decided sleep began, not when the
  athlete went to bed.

## Worked example

An athlete in Chicago slept from 04:30 to 12:30 UTC on 3 October. With the profile
time zone `America/Chicago` (UTC−5 in October), that is 23:30 on 2 October to 07:30
on 3 October, with a midpoint of 03:30.

```json ath-example
{
  "records": [
    { "id": "me", "record_type": "athlete_profile", "default_time_zone": "America/Chicago" },
    { "id": "night", "record_type": "sleep_session", "start": "2026-10-03T04:30:00.000Z", "end": "2026-10-03T12:30:00.000Z" }
  ],
  "apply": "night",
  "result": {
    "start_at": "2026-10-03T04:30:00.000Z", "midpoint_at": "2026-10-03T08:30:00.000Z", "end_at": "2026-10-03T12:30:00.000Z",
    "local_start": "2026-10-02T23:30:00-05:00", "local_midpoint": "2026-10-03T03:30:00-05:00", "local_end": "2026-10-03T07:30:00-05:00",
    "time_zone": "America/Chicago", "time_zone_basis": "athlete_profile.default_time_zone", "duration_hours": 8
  }
}
```

## Related

`sleep-regularity` for consistency across days, `sleep-duration` for how long.

```ath-calculation
name: sleep-timing
use: sleep_clock_times
start_field: start_at
end_field: end_at
```
