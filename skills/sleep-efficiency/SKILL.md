---
name: sleep-efficiency
description: Divides time asleep by time in bed for one sleep session and checks that the two agree. Use when someone asks what share of their time in bed they actually slept, or whether they lie awake a lot. Needs asleep_seconds and in_bed_seconds. Not for total sleep (use sleep-duration) or interruptions (use sleep-interruptions).
license: MIT
compatibility: Requires the ath CLI 2.1.2 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath apply sleep-efficiency --record <sleep-session-id>.
metadata:
  version: "2.1.2"
  kind: calculation
---

# Sleep efficiency

Sleep efficiency is total sleep time divided by time in bed.

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

```sh
ath apply sleep-efficiency --record <sleep-session-id> --json
```

## Method

Operation `sleep_efficiency` (D242).

1. Read `asleep_seconds` and `in_bed_seconds`. Both are required; time in bed must
   be above zero.
2. If time asleep is more than time in bed, the record is inconsistent and the
   result is null.
3. `efficiency = asleep / in bed`, to four places, and `percent` to 0.1.

The session's start and end are not used as time in bed. A wearable's session span
is its detected sleep period, which is not the same thing.

## Reading the result

| Field | Meaning |
|---|---|
| `efficiency`, `percent` | The share of time in bed spent asleep. |
| `asleep_seconds`, `in_bed_seconds` | The two inputs. |

How to describe it:

- Compare a night with the athlete's own other nights from the same device.
  Studies do not even agree on the denominator (Reed and Sacco, 2016), so
  published cut-offs from other methods do not transfer. Do not present it as a
  diagnosis.
- Low efficiency means time in bed awake: falling asleep slowly, waking in the
  night, or lying in. The number cannot say which.

## Limits

- Devices define "in bed" differently. Some take it from the phone or the user;
  some infer it. Compare only within one source.
- If `in_bed_seconds` is missing, the result is null. Do not substitute the
  session span.

## Worked example

7 h 30 min asleep (27 000 s) out of 8 hours in bed (28 800 s) is 93.75 %.

```json ath-example
{
  "records": [
    { "id": "night", "record_type": "sleep_session", "start": "2026-10-01T23:00:00.000Z", "end": "2026-10-02T07:00:00.000Z",
      "asleep_seconds": 27000, "in_bed_seconds": 28800 }
  ],
  "apply": "night",
  "result": { "efficiency": 0.9375, "percent": 93.8, "asleep_seconds": 27000, "in_bed_seconds": 28800 }
}
```

A record that claims more sleep than time in bed is refused:

```json ath-example
{
  "records": [
    { "id": "odd", "record_type": "sleep_session", "start": "2026-10-01T23:00:00.000Z", "end": "2026-10-02T07:00:00.000Z",
      "asleep_seconds": 30000, "in_bed_seconds": 28800 }
  ],
  "apply": "odd",
  "result": null,
  "missing": ["asleep_seconds is more than in_bed_seconds, so the record is inconsistent"]
}
```

## Related

`sleep-duration`, `sleep-stages`, and `sleep-interruptions` describe the same
night.

```ath-calculation
name: sleep-efficiency
use: sleep_efficiency
asleep_field: asleep_seconds
in_bed_field: in_bed_seconds
```
