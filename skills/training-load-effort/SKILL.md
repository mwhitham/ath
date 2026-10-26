---
name: training-load-effort
description: Multiplies minutes by a reported effort number on a personal report. Use when asking for training load from reported effort.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply training-load-effort --record <record-id>.
metadata:
  version: "2.0.0"
---

# Training load from reported effort

Minutes times the effort number the person reported. This reads a personal
report. It is not a measured heart-rate load. If duration or the effort
number is missing, the result says so.

```ath-calculation
name: training-load-effort
duration_field: duration_seconds
effort_field: rating
```
