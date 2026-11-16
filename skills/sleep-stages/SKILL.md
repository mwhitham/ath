---
name: sleep-stages
description: Splits one night's sleep into the stages the source supplied, such as deep, light, and REM, keeps awake minutes apart, and gives each stage's share of sleep. Use when someone asks how much deep or REM sleep they got, or how their sleep was made up. Reads stage_minutes. Not for total sleep (use sleep-duration); never invents a stage the record lacks.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.1.2 database. Runs with ath apply sleep-stages --record <sleep-session-id>.
metadata:
  version: "2.1.2"
  kind: calculation
---

# Sleep stages

The minutes in each sleep stage, exactly as the source supplied them, with awake
time counted apart.

## Run it

```sh
ath apply sleep-stages --record <sleep-session-id> --json
```

## Method

Operation `sleep_stage_split` (D242).

1. Read `stage_minutes`, an object of stage names and minutes.
2. A stage named `awake` (any letter case) is awake time. Every other stage is
   sleep.
3. `asleep_minutes` adds the sleep stages; `total_minutes` adds awake too.
4. `percent_of_asleep` gives each sleep stage as a percentage of asleep minutes,
   to 0.1.

## Reading the result

| Field | Meaning |
|---|---|
| `stages` | The stage minutes as supplied. |
| `asleep_minutes`, `awake_minutes`, `total_minutes` | The totals. |
| `percent_of_asleep` | Each sleep stage's share. |

How to describe it:

- Consumer wearables estimate stages from movement and heart rate. Their stage
  estimates agree with laboratory sleep studies much less well than their total
  sleep does. Describe stages as the device's estimate.
- Night-to-night swings in deep or REM minutes are common and rarely mean much on
  their own.
- Stage names differ by device: Apple's "core" is close to light sleep. Keep the
  names the source used.

## Limits

- A stage the source did not supply is absent, not zero.
- Never compare stage minutes between two devices.

## Worked example

Deep 63, REM 90, light 197, and awake 30 minutes: 350 minutes asleep, of which deep
is 18.0 %, REM 25.7 %, and light 56.3 %.

```json ath-example
{
  "records": [
    { "id": "night", "record_type": "sleep_session", "start": "2026-10-01T23:00:00.000Z", "end": "2026-10-02T07:00:00.000Z",
      "stage_minutes": { "deep": 63, "rem": 90, "light": 197, "awake": 30 } }
  ],
  "apply": "night",
  "result": {
    "stages": { "deep": 63, "rem": 90, "light": 197, "awake": 30 },
    "asleep_minutes": 350, "awake_minutes": 30, "total_minutes": 380,
    "percent_of_asleep": { "deep": 18.0, "rem": 25.7, "light": 56.3 }
  }
}
```

## Related

`sleep-duration` for total sleep, `sleep-efficiency` for sleep against time in
bed.

```ath-calculation
name: sleep-stages
use: sleep_stage_split
field: stage_minutes
```
