---
name: sleep-interruptions
description: Reads how many times sleep was interrupted on one sleep session, as the source counted it. Use when someone asks how broken or restless a night was, or how often they woke up. Reads the interruptions field. Not for minutes awake (use sleep-stages or sleep-efficiency) or comparison with other nights (use summary or sleep-change).
license: MIT
compatibility: Requires the ath CLI 2.1.3 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath apply sleep-interruptions --record <sleep-session-id>.
metadata:
  version: "2.1.3"
  kind: calculation
---

# Sleep interruptions

How many times the source says sleep was interrupted during one session.

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
ath apply sleep-interruptions --record <sleep-session-id> --json
```

## Method

Operation `field`: read `interruptions`. If it is absent, the result is null and
`missing` names it.

## Reading the result

| Field | Meaning |
|---|---|
| `value` | The count of interruptions. |

How to describe it:

- Each source counts differently: some count every wake bout, some only longer
  ones, and some count movement. Compare counts only within one source.
- A count says how often, not how long. Pair it with awake minutes from
  `sleep-stages` when the source supplies them.

## Limits

- A missing field is missing, not zero.
- Short wakes that the athlete does not remember are normal. Several interruptions
  are not a disorder. Persistent poor sleep is a question for a clinician.

## Worked example

```json ath-example
{
  "records": [
    { "id": "night", "record_type": "sleep_session", "start": "2026-10-01T23:00:00.000Z", "end": "2026-10-02T07:00:00.000Z",
      "interruptions": 3 }
  ],
  "apply": "night",
  "result": { "value": 3 }
}
```

The source counted three interruptions in the night.

## Related

`sleep-stages` for awake minutes, `sleep-efficiency` for sleep against time in
bed.

```ath-calculation
name: sleep-interruptions
use: field
field: interruptions
```
