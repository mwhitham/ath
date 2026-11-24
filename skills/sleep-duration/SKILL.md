---
name: sleep-duration
description: Reads how long the athlete slept on one sleep session, as the device estimated it. Use when someone asks how much they slept last night or on a given night. Reads asleep_seconds; awake time in bed is not sleep. Not for comparison with their usual sleep (use sleep-change), sleep against time in bed (use sleep-efficiency), or consistency (use sleep-regularity).
license: MIT
compatibility: Requires the ath CLI 2.1.3 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath apply sleep-duration --record <sleep-session-id>.
metadata:
  version: "2.1.3"
  kind: calculation
---

# Sleep duration

Time asleep on one sleep session, in seconds, exactly as the source recorded it.

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
ath apply sleep-duration --record <sleep-session-id> --json
```

## Method

Operation `field`: read `asleep_seconds`. If it is absent, the result is null and
`missing` names it. The session's start and end are not used, because the span of
a session includes time awake.

## Reading the result

| Field | Meaning |
|---|---|
| `value` | Seconds asleep. Divide by 3600 for hours. |

How to describe it:

- Say hours and minutes: 27 000 s is 7 h 30 min.
- This is the device's estimate. Consumer wearables are reasonably good at total
  sleep and weaker at wake inside the night.
- "Last night" means the main sleep. If the athlete also napped, that is a
  separate session.

## Limits

- A missing field is missing, not zero.
- Two devices that both recorded the night give two sessions and two answers.
  Report both, with their sources.

## Worked example

```json ath-example
{
  "records": [
    { "id": "night", "record_type": "sleep_session", "start": "2026-10-01T23:00:00.000Z", "end": "2026-10-02T07:00:00.000Z",
      "asleep_seconds": 27000, "in_bed_seconds": 28800 }
  ],
  "apply": "night",
  "result": { "value": 27000 }
}
```

The athlete slept 7 h 30 min of the 8 hours in bed.

## Related

`sleep-change` compares this with earlier nights. `sleep-efficiency` divides it by
time in bed. `sleep-stages` splits it by stage.

```ath-calculation
name: sleep-duration
use: field
field: asleep_seconds
```
