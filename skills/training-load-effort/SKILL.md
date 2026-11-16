---
name: training-load-effort
description: Computes session-RPE training load, the athlete's rating of a whole session times the minutes it covers, from a session_effort athlete report. Use when someone rates how hard a session felt and asks for its training load, or wants weekly load from their own effort ratings, including for lifting, CrossFit, or HYROX work where heart rate misleads. Refuses soreness, mood, or any other rating. Not for heart-rate load (use training-load-heart-rate).
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.1.2 database. Runs with ath apply training-load-effort --record <athlete-report-id>.
metadata:
  version: "2.1.2"
  kind: calculation
---

# Training load from reported effort

Session-RPE load (Foster et al., 2001): the athlete's rating of how hard the whole
session was, times its duration in minutes. It is the athlete's report, not a
measurement, and it stays on the athlete side of the two-tier wall.

## Log the report first

The rating must be a `session_effort` athlete report. Show the preview and wait
for approval before writing:

```json
{
  "record_type": "athlete_report",
  "data": {
    "report_type": "session_effort",
    "reported_at": "2026-10-01T18:15:00.000Z",
    "input_reported_at": { "timestamp_text": "2026-10-01T18:15:00.000Z", "utc_offset_text": "Z", "utc_offset_meaning": "documented_recording_offset", "timestamp_basis": "source_instant" },
    "reported_via": "text",
    "rating": 7,
    "rating_scale": "0-10",
    "duration_seconds": 3600,
    "workout_session_id": "<the workout's record ID, when there is one>"
  }
}
```

Ask for the rating about 30 minutes after the session, on one scale the athlete
keeps using. `duration_seconds` is the time the rating covers: the whole session
including warm-up, or only the main work, as long as the athlete always does the
same. If the report has no duration, the linked workout's start and end are used.

## Run it

```sh
ath apply training-load-effort --record <athlete-report-id> --json
```

## Method

Operation `session_effort_load` (D238).

1. The record must be an athlete report with `report_type: session_effort`. Any
   other report type is refused, so a soreness or mood rating never becomes load.
2. `rating_scale` must be `0-10` (also written `CR10`), `1-10`, or `6-20`, and the
   rating must lie on that scale.
3. Minutes come from the report's `duration_seconds`, or else from the linked
   workout's span.
4. `load = rating × minutes`, in arbitrary units (AU), to 0.1.

## Reading the result

| Field | Meaning |
|---|---|
| `load`, `unit` | The load in AU. |
| `rating`, `rating_scale`, `scale_family` | The rating and its scale: `10-point` or `6-20`. |
| `duration_minutes`, `duration_from` | The minutes and where they came from: `report` or `linked workout session`. |

How to describe it:

- Loads on different scale families are never comparable. A Borg 6–20 rating
  times minutes is a different number from a 0–10 rating times minutes. Keep one
  athlete on one scale.
- Weekly load is the sum of the week's sessions. Its week-to-week change is more
  useful than any single session.
- A high load is not bad. It describes the dose, not whether the dose was right.

## Limits

- The rating is the athlete's perception. It is valid for that athlete over time,
  not for comparing athletes.
- If the athlete rated only the main work in one session and the whole session
  in another, the loads describe different things.
- No session effort report, no load. A heart-rate load can fill a gap, but the
  two are separate results.

## Worked example

A 0–10 rating of 7 for a 60-minute session is a load of 420 AU. A Borg rating of
15 with no duration, linked to a 45-minute workout, is 675 AU on the 6–20 scale,
which is not comparable with the first.

```json ath-example
{
  "records": [
    { "id": "rpe", "record_type": "athlete_report", "report_type": "session_effort", "at": "2026-10-01T18:15:00.000Z",
      "rating": 7, "rating_scale": "0-10", "duration_seconds": 3600 }
  ],
  "apply": "rpe",
  "result": { "load": 420, "unit": "AU", "rating": 7, "rating_scale": "0-10", "scale_family": "10-point", "duration_minutes": 60, "duration_from": "report" }
}
```

```json ath-example
{
  "records": [
    { "id": "w", "record_type": "workout_session", "start": "2026-10-02T17:00:00.000Z", "end": "2026-10-02T17:45:00.000Z" },
    { "id": "rpe", "record_type": "athlete_report", "report_type": "session_effort", "at": "2026-10-02T18:15:00.000Z",
      "rating": 15, "rating_scale": "6-20", "workout": "w" }
  ],
  "apply": "rpe",
  "result": { "load": 675, "unit": "AU", "rating": 15, "rating_scale": "6-20", "scale_family": "6-20", "duration_minutes": 45, "duration_from": "linked workout session" }
}
```

A soreness rating is refused:

```json ath-example
{
  "records": [
    { "id": "sore", "record_type": "athlete_report", "report_type": "soreness", "at": "2026-10-03T08:00:00.000Z",
      "rating": 4, "rating_scale": "1-5", "duration_seconds": 3600 }
  ],
  "apply": "sore",
  "result": null,
  "missing": ["a session_effort athlete report; this is a soreness athlete report, and only a session effort becomes training load"]
}
```

## Related

`training-load-heart-rate` for heart-rate load. To total a week, run this on each
session effort report in the week, add the loads, and show each one.

```ath-calculation
name: training-load-effort
use: session_effort_load
duration_field: duration_seconds
effort_field: rating
```
