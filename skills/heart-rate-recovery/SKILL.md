---
name: heart-rate-recovery
description: Measures how far heart rate falls in the 60 seconds after an effort ends, from one workout's heart-rate samples. Use when someone asks how fast their heart rate came down after an interval, a workout, or a test, or whether their one-minute recovery is improving. Needs samples recorded after the effort ends, such as a lap marked recovery. Not for resting heart rate (use resting-heart-rate) or a device's own recovery number (read the hr_recovery measurement).
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.1.2 database. Runs with ath apply heart-rate-recovery --record <workout-session-id>.
metadata:
  version: "2.1.2"
  kind: calculation
---

# Heart-rate recovery

One-minute heart-rate recovery is the heart rate at the end of an effort minus the
heart rate 60 seconds later (Cole et al., 1999). A larger drop usually means faster
recovery.

## Run it

```sh
ath records list --type workout_session --limit 10000 --json
ath apply heart-rate-recovery --record <workout-session-id> --json
```

Check the device first. Apple Health often stores its own one-minute recovery as a
`hr_recovery` point measurement. Prefer that value when it exists, and say which
one you used.

## Method

Operation `drop_after_effort_end` (D235).

1. **End of effort.** The start of a workout segment labelled `recovery`, if there
   is one. Otherwise the workout's end (`end_at − start_at`).
2. **Heart rate at the end.** The sample at that moment, or straight-line
   interpolation between neighbouring samples no more than 10 s apart, or the
   nearest sample within 5 s.
3. **Heart rate 60 s later.** Found the same way.
4. `drop_bpm = end_bpm − later_bpm`, rounded to 0.1 bpm.

If there are no samples 60 s after the end of effort, the result is null and
`missing` says so. Many watches stop recording the moment the workout stops.
To measure recovery, the athlete keeps recording, marks a lap when the effort
ends, and names that segment `recovery`.

## Reading the result

| Field | Meaning |
|---|---|
| `drop_bpm` | The fall in 60 s. |
| `end_bpm`, `later_bpm` | The two heart rates. |
| `end_offset_ms`, `later_offset_ms` | Where they were read, from the workout start. |
| `anchor` | `recovery segment` or `workout end`. |

How to describe it:

- Compare drops only between similar efforts with the same recovery posture:
  standing, walking, or lying down changes the drop. Clinical cut-offs, such as
  12 bpm, apply only to the protocol that produced them; do not quote them for a
  CrossFit or HYROX session.
- A faster drop over weeks, after the same test, is a reasonable sign of
  improving fitness. One workout is not a trend.
- Heat, dehydration, and an effort that ended with a sprint all change the drop.

## Limits

- Wrist sensors lag fast changes in heart rate. The end value can be lower than
  the true peak.
- The workout end is when the athlete pressed stop, which may be after a
  cool-down. Then the "end of effort" is wrong. Use a `recovery` lap.
- This reads samples on the workout record only.

## Worked example

A workout from 10:00 with a lap named `recovery` at 30 min (1 800 000 ms). The
heart rate there is 180 bpm. Sixty seconds later, at 1 860 000 ms, it lies between
148 bpm (1 855 000 ms) and 144 bpm (1 865 000 ms), so it is read as 146 bpm. The
drop is 34 bpm.

```json ath-example
{
  "records": [
    { "id": "w", "record_type": "workout_session", "start": "2026-10-01T10:00:00.000Z", "end": "2026-10-01T10:32:00.000Z",
      "segments": [{ "label": "recovery", "start_offset_ms": 1800000, "source_reported_summaries": [] }],
      "heart_rate_samples": [
        { "offset_ms": 1790000, "bpm": 178 }, { "offset_ms": 1800000, "bpm": 180 },
        { "offset_ms": 1830000, "bpm": 160 }, { "offset_ms": 1855000, "bpm": 148 },
        { "offset_ms": 1865000, "bpm": 144 }, { "offset_ms": 1900000, "bpm": 135 }] }
  ],
  "apply": "w",
  "result": { "drop_bpm": 34, "end_bpm": 180, "later_bpm": 146, "end_offset_ms": 1800000, "later_offset_ms": 1860000, "anchor": "recovery segment" }
}
```

When the watch stopped at the workout end, there is nothing to read:

```json ath-example
{
  "records": [
    { "id": "w", "record_type": "workout_session", "start": "2026-10-01T10:00:00.000Z", "end": "2026-10-01T10:30:00.000Z",
      "heart_rate_samples": [{ "offset_ms": 1790000, "bpm": 178 }, { "offset_ms": 1800000, "bpm": 180 }] }
  ],
  "apply": "w",
  "result": null,
  "missing": ["heart-rate samples 60 s after the end of effort, at offset 1860000; many devices stop recording when the workout ends"]
}
```

## Related

`heart-rate-zones` and `training-load-heart-rate` read the same samples.
`resting-heart-rate` estimates resting heart rate.

```ath-calculation
name: heart-rate-recovery
use: drop_after_effort_end
sample_field: heart_rate_samples
later_offset_ms: 60000
tolerance_ms: 5000
max_interpolation_gap_ms: 10000
recovery_label: recovery
```
