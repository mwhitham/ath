---
name: sleep-interruptions
description: Reads how many times sleep was interrupted on one sleep session, as the source counted it. Use when someone asks how broken or restless a night was, or how often they woke up. Reads the interruptions field. Not for minutes awake (use sleep-stages or sleep-efficiency) or comparison with other nights (use summary or sleep-change).
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.1.2 database. Runs with ath apply sleep-interruptions --record <sleep-session-id>.
metadata:
  version: "2.1.2"
  kind: calculation
---

# Sleep interruptions

How many times the source says sleep was interrupted during one session.

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
